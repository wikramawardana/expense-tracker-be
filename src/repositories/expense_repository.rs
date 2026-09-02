use crate::db::Database;
use crate::errors::{AppError, AppResult};
use crate::models::{Expense, ExpenseQueryParams};
use surrealdb::types::SurrealValue;

#[derive(Clone)]
pub struct ExpenseRepository {
    db: Database,
}

fn build_query_conditions(
    query: &ExpenseQueryParams,
    owner_id: &str,
) -> (String, Vec<(String, serde_json::Value)>) {
    let mut conditions: Vec<String> = vec![];
    let mut bindings: Vec<(String, serde_json::Value)> = vec![];
    conditions.push("owner_id = $owner_id".to_string());
    bindings.push(("owner_id".to_string(), serde_json::json!(owner_id)));

    // Date range filters
    if let Some(date_from) = &query.expense_date_from {
        let trimmed = date_from.trim();
        if !trimmed.is_empty() {
            conditions.push("expense_date >= $date_from".to_string());
            bindings.push(("date_from".to_string(), serde_json::json!(trimmed)));
        }
    }
    if let Some(date_to) = &query.expense_date_to {
        let trimmed = date_to.trim();
        if !trimmed.is_empty() {
            conditions.push("expense_date <= $date_to".to_string());
            bindings.push(("date_to".to_string(), serde_json::json!(trimmed)));
        }
    }

    // Payment method filter
    if let (Some(payment_method_id), Some(payment_method)) =
        (&query.payment_method_id, &query.payment_method)
    {
        let id_trimmed = payment_method_id.trim();
        let method_trimmed = payment_method.trim();
        if !id_trimmed.is_empty() && id_trimmed != "all" && !method_trimmed.is_empty() && method_trimmed != "all" {
            conditions.push(
                "(payment_method_id = $payment_method_id OR (payment_method != NONE AND payment_method != NULL AND string::lowercase(payment_method) = $payment_method))"
                    .to_string(),
            );
            bindings.push((
                "payment_method_id".to_string(),
                serde_json::json!(id_trimmed),
            ));
            bindings.push((
                "payment_method".to_string(),
                serde_json::json!(method_trimmed.to_lowercase()),
            ));
        } else if !id_trimmed.is_empty() && id_trimmed != "all" {
            conditions.push("payment_method_id = $payment_method_id".to_string());
            bindings.push((
                "payment_method_id".to_string(),
                serde_json::json!(id_trimmed),
            ));
        } else if !method_trimmed.is_empty() && method_trimmed != "all" {
            conditions.push("(payment_method != NONE AND payment_method != NULL AND string::lowercase(payment_method) = $payment_method)".to_string());
            bindings.push((
                "payment_method".to_string(),
                serde_json::json!(method_trimmed.to_lowercase()),
            ));
        }
    } else if let Some(payment_method_id) = &query.payment_method_id {
        let trimmed = payment_method_id.trim();
        if !trimmed.is_empty() && trimmed != "all" {
            conditions.push("payment_method_id = $payment_method_id".to_string());
            bindings.push((
                "payment_method_id".to_string(),
                serde_json::json!(trimmed),
            ));
        }
    } else if let Some(payment_method) = &query.payment_method {
        let trimmed = payment_method.trim();
        if !trimmed.is_empty() && trimmed != "all" {
            conditions.push("(payment_method != NONE AND payment_method != NULL AND string::lowercase(payment_method) = $payment_method)".to_string());
            bindings.push((
                "payment_method".to_string(),
                serde_json::json!(trimmed.to_lowercase()),
            ));
        }
    }

    // Paid by filter
    if let Some(paid_by) = &query.paid_by {
        let trimmed = paid_by.trim();
        if !trimmed.is_empty() && trimmed != "all" {
            conditions.push("(paid_by != NONE AND paid_by != NULL AND string::lowercase(paid_by) = $paid_by)".to_string());
            bindings.push(("paid_by".to_string(), serde_json::json!(trimmed.to_lowercase())));
        }
    }

    // Status filter - use string::lowercase for case-insensitive matching in SurrealDB
    if let Some(status) = &query.status {
        let trimmed = status.trim().to_lowercase();
        if !trimmed.is_empty() && trimmed != "all" {
            conditions.push("(status != NONE AND status != NULL AND string::lowercase(status) = $status)".to_string());
            bindings.push(("status".to_string(), serde_json::json!(trimmed)));
        }
    }

    // Bill statement filter
    if let Some(bill_statement_id) = &query.bill_statement_id {
        let trimmed = bill_statement_id.trim();
        if !trimmed.is_empty() && trimmed != "all" {
            conditions.push("bill_statement_id = $bill_statement_id".to_string());
            bindings.push((
                "bill_statement_id".to_string(),
                serde_json::json!(trimmed),
            ));
        }
    }

    // Category / Category ID filter
    if let Some(category_id) = &query.category_id {
        let trimmed = category_id.trim();
        if !trimmed.is_empty() && trimmed != "all" {
            conditions.push("category_id = $category_id".to_string());
            bindings.push(("category_id".to_string(), serde_json::json!(trimmed)));
        }
    } else if let Some(category) = &query.category {
        let trimmed = category.trim();
        if !trimmed.is_empty() && trimmed != "all" {
            conditions.push("(category_id = $category OR (category != NONE AND category != NULL AND string::lowercase(category) = $category))".to_string());
            bindings.push(("category".to_string(), serde_json::json!(trimmed.to_lowercase())));
        }
    }

    // Search filter
    if let Some(search) = &query.search {
        let trimmed = search.trim();
        if !trimmed.is_empty() {
            conditions.push(
                "((title != NONE AND title != NULL AND string::lowercase(title) CONTAINS $search) OR (description != NONE AND description != NULL AND string::lowercase(description) CONTAINS $search))".to_string(),
            );
            bindings.push(("search".to_string(), serde_json::json!(trimmed.to_lowercase())));
        }
    }

    // Keep regular transactions, installments, and subscriptions in the
    // same table while allowing the product areas to query them separately.
    if let Some(expense_type) = query.expense_type.as_deref() {
        match expense_type.trim().to_lowercase().as_str() {
            "transaction" | "regular" | "none" => conditions.push(
                "(recurrence_type = NONE OR recurrence_type = NULL OR recurrence_type = '' OR (recurrence_type != NONE AND recurrence_type != NULL AND string::lowercase(recurrence_type) = 'none'))"
                    .to_string(),
            ),
            "installment" => {
                conditions.push("(recurrence_type != NONE AND recurrence_type != NULL AND string::lowercase(recurrence_type) = 'installment')".to_string());
            }
            "subscription" => {
                conditions.push("(recurrence_type != NONE AND recurrence_type != NULL AND string::lowercase(recurrence_type) = 'subscription')".to_string());
            }
            _ => {}
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    (where_clause, bindings)
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

    pub async fn find_by_id(&self, id: &str, owner_id: &str) -> AppResult<Expense> {
        let sql = "SELECT * FROM expenses WHERE id = type::record('expenses', $id) AND owner_id = $owner_id LIMIT 1";
        let mut result = self
            .db
            .query(sql)
            .bind(("id", id.to_string()))
            .bind(("owner_id", owner_id.to_string()))
            .await?;
        let rows: Vec<Expense> = result.take(0)?;
        let result = rows.into_iter().next();
        result.ok_or_else(|| AppError::NotFound(format!("Expense with id {} not found", id)))
    }

    pub async fn find_all(&self, owner_id: &str) -> AppResult<Vec<Expense>> {
        let mut result = self
            .db
            .query("SELECT * FROM expenses WHERE owner_id = $owner_id")
            .bind(("owner_id", owner_id.to_string()))
            .await?;
        let expenses: Vec<Expense> = result.take(0)?;
        Ok(expenses)
    }

    pub async fn find_with_query(
        &self,
        query: &ExpenseQueryParams,
        owner_id: &str,
    ) -> AppResult<Vec<Expense>> {
        let offset = (query.page - 1) * query.page_size;
        let (where_clause, bindings) = build_query_conditions(query, owner_id);

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

    pub async fn count_with_query(
        &self,
        query: &ExpenseQueryParams,
        owner_id: &str,
    ) -> AppResult<u32> {
        let (where_clause, bindings) = build_query_conditions(query, owner_id);
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

    pub async fn update(&self, id: &str, owner_id: &str, expense: Expense) -> AppResult<Expense> {
        let existing = self.find_by_id(id, owner_id).await?;
        let updated: Option<Expense> = self.db.update(existing.id).content(expense).await?;
        updated.ok_or_else(|| AppError::NotFound(format!("Expense with id {} not found", id)))
    }

    pub async fn delete(&self, id: &str, owner_id: &str) -> AppResult<()> {
        let existing = self.find_by_id(id, owner_id).await?;
        let deleted: Option<Expense> = self.db.delete(existing.id).await?;
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
        owner_id: &str,
    ) -> AppResult<u32> {
        let sql = "DELETE FROM expenses WHERE recurrence_group_id = $group_id AND owner_id = $owner_id AND id != $except_id";
        let mut result = self
            .db
            .query(sql)
            .bind(("group_id", group_id.to_string()))
            .bind(("owner_id", owner_id.to_string()))
            .bind(("except_id", format!("expenses:{}", except_id)))
            .await?;

        // Get the count of deleted records
        let deleted: Vec<Expense> = result.take(0).unwrap_or_default();
        Ok(deleted.len() as u32)
    }

    /// Get the latest expense date in a recurrence group
    pub async fn get_latest_expense_in_group(
        &self,
        group_id: &str,
        owner_id: &str,
    ) -> AppResult<Option<Expense>> {
        let sql = "SELECT * FROM expenses WHERE recurrence_group_id = $group_id AND owner_id = $owner_id ORDER BY expense_date DESC LIMIT 1";
        let mut result = self
            .db
            .query(sql)
            .bind(("group_id", group_id.to_string()))
            .bind(("owner_id", owner_id.to_string()))
            .await?;
        let expenses: Vec<Expense> = result.take(0)?;
        Ok(expenses.into_iter().next())
    }
}
