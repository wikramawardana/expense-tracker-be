use crate::db::Database;
use crate::errors::{AppError, AppResult};
use crate::models::{Expense, ExpenseQueryParams, ExpenseStatus};
use surrealdb::types::SurrealValue;

#[derive(Clone)]
pub struct ExpenseRepository {
    db: Database,
}

impl ExpenseRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn create(&self, expense: Expense) -> AppResult<Expense> {
        let key = crate::models::record_key_to_string(&expense.id.key);
        let created: Option<Expense> = self
            .db
            .create::<Option<Expense>>(("expenses", key))
            .content(expense)
            .await?;
        created.ok_or_else(|| AppError::Internal("Failed to create expense".to_string()))
    }

    pub async fn find_by_id(&self, id: &str) -> AppResult<Expense> {
        let result: Option<Expense> = self.db.select(("expenses", id)).await?;
        result.ok_or_else(|| AppError::NotFound(format!("Expense with id {} not found", id)))
    }

    pub async fn find_all(&self) -> AppResult<Vec<Expense>> {
        let expenses: Vec<Expense> = self.db.select("expenses").await?;
        Ok(expenses)
    }

    pub async fn find_with_query(&self, query: &ExpenseQueryParams) -> AppResult<Vec<Expense>> {
        let offset = (query.page - 1) * query.page_size;

        // Build dynamic query with filters
        let mut conditions: Vec<String> = vec![];
        let mut bindings: Vec<(String, serde_json::Value)> = vec![];

        // Date range filters
        if let Some(date_from) = &query.expense_date_from {
            conditions.push("expense_date >= $date_from".to_string());
            bindings.push(("date_from".to_string(), serde_json::json!(date_from)));
        }
        if let Some(date_to) = &query.expense_date_to {
            conditions.push("expense_date <= $date_to".to_string());
            bindings.push(("date_to".to_string(), serde_json::json!(date_to)));
        }

        // Payment method filter. When both values are supplied, the name
        // fallback keeps historical rows without an ID visible.
        if let (Some(payment_method_id), Some(payment_method)) =
            (&query.payment_method_id, &query.payment_method)
        {
            conditions.push(
                "(payment_method_id = $payment_method_id OR payment_method = $payment_method)"
                    .to_string(),
            );
            bindings.push((
                "payment_method_id".to_string(),
                serde_json::json!(payment_method_id),
            ));
            bindings.push((
                "payment_method".to_string(),
                serde_json::json!(payment_method),
            ));
        } else if let Some(payment_method_id) = &query.payment_method_id {
            conditions.push("payment_method_id = $payment_method_id".to_string());
            bindings.push((
                "payment_method_id".to_string(),
                serde_json::json!(payment_method_id),
            ));
        } else if let Some(payment_method) = &query.payment_method {
            conditions.push("payment_method = $payment_method".to_string());
            bindings.push((
                "payment_method".to_string(),
                serde_json::json!(payment_method),
            ));
        }

        // Paid by filter
        if let Some(paid_by) = &query.paid_by {
            conditions.push("paid_by = $paid_by".to_string());
            bindings.push(("paid_by".to_string(), serde_json::json!(paid_by)));
        }

        // Status filter
        if let Some(status) = &query.status {
            if let Ok(parsed_status) = status.parse::<ExpenseStatus>() {
                conditions.push("status = $status".to_string());
                bindings.push(("status".to_string(), serde_json::json!(parsed_status)));
            }
        }

        // Bill statement filter
        if let Some(bill_statement_id) = &query.bill_statement_id {
            conditions.push("bill_statement_id = $bill_statement_id".to_string());
            bindings.push((
                "bill_statement_id".to_string(),
                serde_json::json!(bill_statement_id),
            ));
        }

        // Keep regular transactions, installments, and subscriptions in the
        // same table while allowing the product areas to query them separately.
        if let Some(expense_type) = query.expense_type.as_deref() {
            match expense_type.trim().to_lowercase().as_str() {
                "transaction" | "regular" | "none" => conditions.push(
                    "(recurrence_type = NONE OR recurrence_type = NULL OR recurrence_type = '' OR recurrence_type = 'none')"
                        .to_string(),
                ),
                "installment" | "subscription" => {
                    conditions.push("recurrence_type = $expense_type".to_string());
                    bindings.push((
                        "expense_type".to_string(),
                        serde_json::json!(expense_type.trim().to_lowercase()),
                    ));
                }
                _ => {}
            }
        }

        // Build WHERE clause
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Validate sort_by to prevent SQL injection
        let valid_sort_fields = [
            "expense_date",
            "amount",
            "created_at",
            "updated_at",
            "title",
            "payment_method",
        ];
        let sort_by = if valid_sort_fields.contains(&query.sort_by.as_str()) {
            query.sort_by.clone()
        } else {
            "expense_date".to_string()
        };

        let sort_order = if query.sort_order.to_uppercase() == "ASC" {
            "ASC"
        } else {
            "DESC"
        };

        let sql = format!(
            "SELECT * FROM expenses {} ORDER BY {} {} LIMIT $limit START $offset",
            where_clause, sort_by, sort_order
        );

        let mut stmt = self.db.query(&sql);

        // Bind all parameters
        for (name, value) in bindings {
            stmt = stmt.bind((name, value));
        }
        stmt = stmt.bind(("limit", query.page_size));
        stmt = stmt.bind(("offset", offset));

        let mut result = stmt.await?;
        let expenses: Vec<Expense> = result.take(0)?;

        Ok(expenses)
    }

    pub async fn count_with_query(&self, query: &ExpenseQueryParams) -> AppResult<u32> {
        // Build dynamic query with filters (same as find_with_query)
        let mut conditions: Vec<String> = vec![];
        let mut bindings: Vec<(String, serde_json::Value)> = vec![];

        if let Some(date_from) = &query.expense_date_from {
            conditions.push("expense_date >= $date_from".to_string());
            bindings.push(("date_from".to_string(), serde_json::json!(date_from)));
        }
        if let Some(date_to) = &query.expense_date_to {
            conditions.push("expense_date <= $date_to".to_string());
            bindings.push(("date_to".to_string(), serde_json::json!(date_to)));
        }
        if let (Some(payment_method_id), Some(payment_method)) =
            (&query.payment_method_id, &query.payment_method)
        {
            conditions.push(
                "(payment_method_id = $payment_method_id OR payment_method = $payment_method)"
                    .to_string(),
            );
            bindings.push((
                "payment_method_id".to_string(),
                serde_json::json!(payment_method_id),
            ));
            bindings.push((
                "payment_method".to_string(),
                serde_json::json!(payment_method),
            ));
        } else if let Some(payment_method_id) = &query.payment_method_id {
            conditions.push("payment_method_id = $payment_method_id".to_string());
            bindings.push((
                "payment_method_id".to_string(),
                serde_json::json!(payment_method_id),
            ));
        } else if let Some(payment_method) = &query.payment_method {
            conditions.push("payment_method = $payment_method".to_string());
            bindings.push((
                "payment_method".to_string(),
                serde_json::json!(payment_method),
            ));
        }
        if let Some(paid_by) = &query.paid_by {
            conditions.push("paid_by = $paid_by".to_string());
            bindings.push(("paid_by".to_string(), serde_json::json!(paid_by)));
        }
        if let Some(status) = &query.status {
            if let Ok(parsed_status) = status.parse::<ExpenseStatus>() {
                conditions.push("status = $status".to_string());
                bindings.push(("status".to_string(), serde_json::json!(parsed_status)));
            }
        }

        // Bill statement filter
        if let Some(bill_statement_id) = &query.bill_statement_id {
            conditions.push("bill_statement_id = $bill_statement_id".to_string());
            bindings.push((
                "bill_statement_id".to_string(),
                serde_json::json!(bill_statement_id),
            ));
        }

        if let Some(expense_type) = query.expense_type.as_deref() {
            match expense_type.trim().to_lowercase().as_str() {
                "transaction" | "regular" | "none" => conditions.push(
                    "(recurrence_type = NONE OR recurrence_type = NULL OR recurrence_type = '' OR recurrence_type = 'none')"
                        .to_string(),
                ),
                "installment" | "subscription" => {
                    conditions.push("recurrence_type = $expense_type".to_string());
                    bindings.push((
                        "expense_type".to_string(),
                        serde_json::json!(expense_type.trim().to_lowercase()),
                    ));
                }
                _ => {}
            }
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let sql = format!("SELECT count() FROM expenses {} GROUP ALL", where_clause);

        let mut stmt = self.db.query(&sql);
        for (name, value) in bindings {
            stmt = stmt.bind((name, value));
        }

        let mut result = stmt.await?;

        // SurrealDB returns count as an object with "count" field
        #[derive(serde::Deserialize, SurrealValue)]
        struct CountResult {
            count: u32,
        }

        let count_result: Option<CountResult> = result.take(0)?;
        Ok(count_result.map(|c| c.count).unwrap_or(0))
    }

    pub async fn update(&self, id: &str, expense: Expense) -> AppResult<Expense> {
        let updated: Option<Expense> = self.db.update(("expenses", id)).content(expense).await?;
        updated.ok_or_else(|| AppError::NotFound(format!("Expense with id {} not found", id)))
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let deleted: Option<Expense> = self.db.delete(("expenses", id)).await?;
        match deleted {
            Some(_) => Ok(()),
            None => Err(AppError::NotFound(format!(
                "Expense with id {} not found",
                id
            ))),
        }
    }

    /// Delete all expenses in a recurrence group except the specified one
    pub async fn delete_by_recurrence_group_id_except(
        &self,
        group_id: &str,
        except_id: &str,
    ) -> AppResult<u32> {
        let sql = "DELETE FROM expenses WHERE recurrence_group_id = $group_id AND id != $except_id";
        let mut result = self
            .db
            .query(sql)
            .bind(("group_id", group_id.to_string()))
            .bind(("except_id", format!("expenses:{}", except_id)))
            .await?;

        // Get the count of deleted records
        let deleted: Vec<Expense> = result.take(0).unwrap_or_default();
        Ok(deleted.len() as u32)
    }

    /// Get the latest expense date in a recurrence group
    pub async fn get_latest_expense_in_group(&self, group_id: &str) -> AppResult<Option<Expense>> {
        let sql = "SELECT * FROM expenses WHERE recurrence_group_id = $group_id ORDER BY expense_date DESC LIMIT 1";
        let mut result = self
            .db
            .query(sql)
            .bind(("group_id", group_id.to_string()))
            .await?;
        let expenses: Vec<Expense> = result.take(0)?;
        Ok(expenses.into_iter().next())
    }
}
