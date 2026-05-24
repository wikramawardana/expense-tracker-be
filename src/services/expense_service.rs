use chrono::{DateTime, Datelike, Months, Utc};
use surrealdb::types::RecordId;
use uuid::Uuid;
use validator::Validate;

use crate::errors::{AppError, AppResult};
use crate::models::{
    BillStatement, CreateExpenseRequest, CreateExpensesBulkRequest, Expense, ExpensePaginationMeta,
    ExpenseQueryParams, ExpenseResponse, ExpenseStatus, PaginatedExpensesResponse,
    UpdateExpenseRequest,
};
use crate::repositories::{BillStatementRepository, ExpenseRepository, PaymentMethodRepository};

fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    s.parse::<DateTime<Utc>>().ok()
}

fn add_months_to_date_str(date_str: &str, months: u32) -> String {
    if let Some(dt) = parse_date(date_str) {
        dt.checked_add_months(Months::new(months))
            .unwrap_or(dt)
            .to_rfc3339()
    } else {
        date_str.to_string()
    }
}

#[derive(Clone)]
pub struct ExpenseService {
    repository: ExpenseRepository,
    bill_statement_repository: BillStatementRepository,
    payment_method_repository: PaymentMethodRepository,
}

impl ExpenseService {
    pub fn new(
        repository: ExpenseRepository,
        bill_statement_repository: BillStatementRepository,
        payment_method_repository: PaymentMethodRepository,
    ) -> Self {
        Self {
            repository,
            bill_statement_repository,
            payment_method_repository,
        }
    }

    pub async fn create(&self, request: CreateExpenseRequest) -> AppResult<Expense> {
        request
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;

        if request.payment_method_id.is_none() && request.payment_method.is_none() {
            return Err(AppError::Validation(
                "payment_method_id is required".to_string(),
            ));
        }
        if request.category_id.is_none() {
            return Err(AppError::Validation("category_id is required".to_string()));
        }
        if request.bill_statement_id.is_none() {
            return Err(AppError::Validation(
                "bill_statement_id is required".to_string(),
            ));
        }

        let now = Utc::now().to_rfc3339();
        let group_id = Uuid::new_v4().to_string();

        let resolved_recurrence_type =
            self.resolve_recurrence_type(&request.recurrence_type, &request.recurrence_type_id);

        let is_installment = self.is_installment_schedule(&resolved_recurrence_type)?;

        let (start_num, end_num) =
            self.calculate_range(&request, is_installment, &resolved_recurrence_type);
        let total_to_create = if end_num >= start_num {
            end_num - start_num + 1
        } else {
            1
        };

        let first_bill_statement_id = if is_installment {
            Some(
                self.get_or_create_bill_statement(&request.expense_date)
                    .await?,
            )
        } else {
            request.bill_statement_id.clone()
        };

        let first_bill_statement_name = if is_installment {
            Some(self.format_bill_statement_name(&request.expense_date))
        } else if let Some(ref name) = request.bill_statement {
            Some(name.clone())
        } else if let Some(ref bs_id) = request.bill_statement_id {
            self.bill_statement_repository
                .find_by_id(bs_id)
                .await
                .ok()
                .map(|bs| bs.name)
        } else {
            None
        };

        let payment_method_name = self.resolve_payment_method(&request).await?;
        let recurrence_type_name = resolved_recurrence_type.clone();

        let first_expense = Expense {
            id: RecordId::new("expenses", Uuid::new_v4().to_string()),
            title: request.title.clone(),
            amount: request.amount,
            payment_method: payment_method_name.clone(),
            payment_method_id: request.payment_method_id.clone(),
            expense_date: request.expense_date.clone(),
            description: request.description.clone(),
            status: ExpenseStatus::Pending,
            bill_statement: first_bill_statement_name,
            bill_statement_id: first_bill_statement_id,
            category_id: request.category_id.clone(),
            paid_by: request.paid_by.clone(),
            recurrence_type: recurrence_type_name.clone(),
            recurrence_type_id: request.recurrence_type_id.clone(),
            recurrence_count: request.recurrence_count,
            recurrence_current: if total_to_create > 1 || request.recurrence_current.is_some() {
                Some(start_num)
            } else {
                None
            },
            recurrence_total_amount: request.recurrence_total_amount,
            recurrence_end_date: request.recurrence_end_date.clone(),
            recurrence_group_id: if total_to_create > 1 {
                Some(group_id.clone())
            } else {
                None
            },
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        let created_first = self.repository.create(first_expense).await?;

        if total_to_create > 1 {
            for i in (start_num + 1)..=end_num {
                let months_offset = i - start_num;
                let future_date = add_months_to_date_str(&request.expense_date, months_offset);

                let bill_statement_id = self.get_or_create_bill_statement(&future_date).await.ok();
                let bill_statement_name = Some(self.format_bill_statement_name(&future_date));

                let future_expense = Expense {
                    id: RecordId::new("expenses", Uuid::new_v4().to_string()),
                    title: request.title.clone(),
                    amount: request.amount,
                    payment_method: payment_method_name.clone(),
                    payment_method_id: request.payment_method_id.clone(),
                    expense_date: future_date,
                    description: request.description.clone(),
                    status: ExpenseStatus::Pending,
                    bill_statement: bill_statement_name,
                    bill_statement_id,
                    category_id: request.category_id.clone(),
                    paid_by: request.paid_by.clone(),
                    recurrence_type: recurrence_type_name.clone(),
                    recurrence_type_id: request.recurrence_type_id.clone(),
                    recurrence_count: request.recurrence_count,
                    recurrence_current: Some(i),
                    recurrence_total_amount: request.recurrence_total_amount,
                    recurrence_end_date: request.recurrence_end_date.clone(),
                    recurrence_group_id: Some(group_id.clone()),
                    created_at: now.clone(),
                    updated_at: now.clone(),
                };

                let _ = self.repository.create(future_expense).await;
            }
        }

        Ok(created_first)
    }

    pub async fn create_bulk(&self, request: CreateExpensesBulkRequest) -> AppResult<Vec<Expense>> {
        if request.expenses.is_empty() {
            return Err(AppError::Validation(
                "At least one expense is required".to_string(),
            ));
        }

        let mut created: Vec<Expense> = Vec::with_capacity(request.expenses.len());
        for (idx, item) in request.expenses.into_iter().enumerate() {
            let expense = self.create(item).await.map_err(|e| match e {
                AppError::Validation(msg) => {
                    AppError::Validation(format!("Expense #{}: {}", idx + 1, msg))
                }
                other => other,
            })?;
            created.push(expense);
        }

        Ok(created)
    }

    fn format_bill_statement_name(&self, date_str: &str) -> String {
        if let Some(dt) = parse_date(date_str) {
            let month_name = match dt.month() {
                1 => "January",
                2 => "February",
                3 => "March",
                4 => "April",
                5 => "May",
                6 => "June",
                7 => "July",
                8 => "August",
                9 => "September",
                10 => "October",
                11 => "November",
                12 => "December",
                _ => "Unknown",
            };
            format!("{} {}", month_name, dt.year())
        } else {
            "Unknown".to_string()
        }
    }

    async fn resolve_payment_method(&self, request: &CreateExpenseRequest) -> AppResult<String> {
        if let Some(ref name) = request.payment_method {
            if !name.is_empty() {
                return Ok(name.clone());
            }
        }

        if let Some(ref pm_id) = request.payment_method_id {
            let payment_method = self.payment_method_repository.find_by_id(pm_id).await?;
            return Ok(payment_method.name);
        }

        Err(AppError::Validation(
            "Either payment_method or payment_method_id must be provided".to_string(),
        ))
    }

    fn resolve_recurrence_type(
        &self,
        name: &Option<String>,
        id: &Option<String>,
    ) -> Option<String> {
        if let Some(ref rt_name) = name {
            if !rt_name.is_empty() {
                return Some(rt_name.clone());
            }
        }

        if id.as_ref().is_some_and(|value| !value.is_empty()) {
            return Some("installment".to_string());
        }

        None
    }

    async fn get_or_create_bill_statement(&self, date_str: &str) -> AppResult<String> {
        let name = self.format_bill_statement_name(date_str);

        if let Some(existing) = self.bill_statement_repository.find_by_name(&name).await? {
            return Ok(crate::models::record_key_to_string(&existing.id.key));
        }

        let now = Utc::now().to_rfc3339();
        let new_bill_statement = BillStatement {
            id: RecordId::new("bill_statements", Uuid::new_v4().to_string()),
            name: name.clone(),
            payment_method_id: None,
            statement_date: Some(date_str.to_string()),
            due_date: None,
            description: Some(format!("Auto-created for {}", name)),
            is_active: true,
            created_at: now.clone(),
            updated_at: now,
        };

        let created = self
            .bill_statement_repository
            .create(new_bill_statement)
            .await?;
        Ok(crate::models::record_key_to_string(&created.id.key))
    }

    fn calculate_range(
        &self,
        request: &CreateExpenseRequest,
        is_installment: bool,
        resolved_recurrence_type: &Option<String>,
    ) -> (u32, u32) {
        if !is_installment {
            return (1, 1);
        }

        let rt_name = resolved_recurrence_type.as_ref().map(|s| s.to_lowercase());

        match rt_name.as_deref() {
            Some("installment") => {
                let total = request.recurrence_count.unwrap_or(1);
                let start = request.recurrence_current.unwrap_or(1).max(1).min(total);
                (start, total)
            }
            _ => (1, 1),
        }
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<Expense> {
        self.repository.find_by_id(id).await
    }

    pub async fn get_all(&self, query: ExpenseQueryParams) -> AppResult<PaginatedExpensesResponse> {
        let expenses = self.repository.find_with_query(&query).await?;
        let total_items = self.repository.count_with_query(&query).await?;

        let total_pages = if total_items == 0 {
            0
        } else {
            (total_items as f32 / query.page_size as f32).ceil() as u32
        };

        let expense_responses: Vec<ExpenseResponse> =
            expenses.into_iter().map(ExpenseResponse::from).collect();

        Ok(PaginatedExpensesResponse {
            data: expense_responses,
            pagination: ExpensePaginationMeta {
                page: query.page,
                page_size: query.page_size,
                total_items,
                total_pages,
            },
        })
    }

    pub async fn update(&self, id: &str, request: UpdateExpenseRequest) -> AppResult<Expense> {
        let mut expense = self.repository.find_by_id(id).await?;
        let old_recurrence_type = expense.recurrence_type.clone();
        let old_recurrence_end_date = expense.recurrence_end_date.clone();
        let old_recurrence_group_id = expense.recurrence_group_id.clone();
        let schedule_type_was_requested =
            request.recurrence_type_id.is_some() || request.recurrence_type.is_some();

        if let Some(title) = request.title {
            expense.title = title;
        }
        if let Some(amount) = request.amount {
            expense.amount = amount;
        }
        if let Some(payment_method) = request.payment_method {
            expense.payment_method = payment_method;
        }
        if let Some(payment_method_id) = request.payment_method_id {
            expense.payment_method_id = Some(payment_method_id);
        }
        if let Some(expense_date) = request.expense_date {
            expense.expense_date = expense_date;
        }
        if let Some(description) = request.description {
            expense.description = Some(description);
        }
        if let Some(status) = request.status {
            expense.status = status;
        }
        if let Some(ref bill_statement_id) = request.bill_statement_id {
            expense.bill_statement_id = Some(bill_statement_id.clone());
            if request.bill_statement.is_none() {
                if let Ok(bs) = self
                    .bill_statement_repository
                    .find_by_id(bill_statement_id)
                    .await
                {
                    expense.bill_statement = Some(bs.name);
                }
            }
        }
        if let Some(bill_statement) = request.bill_statement {
            expense.bill_statement = Some(bill_statement);
        }
        if let Some(category_id) = request.category_id {
            expense.category_id = Some(category_id);
        }
        if let Some(paid_by) = request.paid_by {
            expense.paid_by = Some(paid_by);
        }
        if let Some(ref recurrence_type_id) = request.recurrence_type_id {
            expense.recurrence_type_id = Some(recurrence_type_id.clone());
            if request.recurrence_type.is_none() {
                expense.recurrence_type = Some("installment".to_string());
            }
        }
        if let Some(recurrence_type) = request.recurrence_type {
            expense.recurrence_type = Some(recurrence_type);
        }
        if let Some(recurrence_count) = request.recurrence_count {
            expense.recurrence_count = Some(recurrence_count);
        }
        if let Some(recurrence_current) = request.recurrence_current {
            expense.recurrence_current = Some(recurrence_current);
        }
        if let Some(recurrence_total_amount) = request.recurrence_total_amount {
            expense.recurrence_total_amount = Some(recurrence_total_amount);
        }
        if let Some(recurrence_end_date) = request.recurrence_end_date {
            expense.recurrence_end_date = Some(recurrence_end_date);
        }
        if let Some(recurrence_group_id) = request.recurrence_group_id {
            expense.recurrence_group_id = Some(recurrence_group_id);
        }

        if schedule_type_was_requested {
            self.is_installment_schedule(&expense.recurrence_type)?;
        }

        expense.updated_at = Utc::now().to_rfc3339();

        let new_recurrence_type = expense.recurrence_type.clone();
        let is_changing_to_one_time = request.clear_recurrence
            || self.is_changing_to_one_time(&old_recurrence_type, &new_recurrence_type);

        if is_changing_to_one_time {
            if let Some(ref group_id) = old_recurrence_group_id {
                let _ = self
                    .repository
                    .delete_by_recurrence_group_id_except(group_id, id)
                    .await;
            }
            expense.recurrence_type = Some("none".to_string());
            expense.recurrence_type_id = None;
            expense.recurrence_group_id = None;
            expense.recurrence_current = None;
            expense.recurrence_count = None;
            expense.recurrence_end_date = None;
            expense.recurrence_total_amount = None;
        } else {
            let should_extend = self.should_extend_recurrence(
                &old_recurrence_end_date,
                &expense.recurrence_end_date,
                &expense.recurrence_type,
            );

            if should_extend {
                if expense.recurrence_group_id.is_none() {
                    expense.recurrence_group_id = Some(Uuid::new_v4().to_string());
                    expense.recurrence_current = Some(1);
                }
                self.extend_recurring_expenses(&expense).await?;
            }
        }

        self.repository.update(id, expense).await
    }

    fn is_changing_to_one_time(
        &self,
        old_type: &Option<String>,
        new_type: &Option<String>,
    ) -> bool {
        let old_is_recurring = old_type
            .as_ref()
            .map(|t| {
                let lower = t.to_lowercase();
                lower != "none" && !lower.is_empty()
            })
            .unwrap_or(false);

        let new_is_one_time = new_type
            .as_ref()
            .map(|t| {
                let lower = t.to_lowercase();
                lower == "none" || lower.is_empty()
            })
            .unwrap_or(false);

        old_is_recurring && new_is_one_time
    }

    fn is_installment_schedule(&self, recurrence_type: &Option<String>) -> AppResult<bool> {
        let Some(rt_name) = recurrence_type else {
            return Ok(false);
        };

        let lower = rt_name.to_lowercase();
        if lower == "none" || lower.is_empty() {
            return Ok(false);
        }
        if lower == "installment" {
            return Ok(true);
        }

        Err(AppError::Validation(
            "Only Installment schedule type is supported".to_string(),
        ))
    }

    fn should_extend_recurrence(
        &self,
        _old_end_date: &Option<String>,
        _new_end_date: &Option<String>,
        _recurrence_type: &Option<String>,
    ) -> bool {
        false
    }

    async fn extend_recurring_expenses(&self, expense: &Expense) -> AppResult<()> {
        let Some(ref group_id) = expense.recurrence_group_id else {
            return Ok(());
        };

        let Some(ref new_end_date_str) = expense.recurrence_end_date else {
            return Ok(());
        };

        let latest = self
            .repository
            .get_latest_expense_in_group(group_id)
            .await?;
        let (mut current_date_str, mut current_num) = if let Some(latest_expense) = latest {
            (
                latest_expense.expense_date,
                latest_expense.recurrence_current.unwrap_or(1),
            )
        } else {
            (
                expense.expense_date.clone(),
                expense.recurrence_current.unwrap_or(1),
            )
        };

        let now = Utc::now().to_rfc3339();

        loop {
            let next_date_str = add_months_to_date_str(&current_date_str, 1);

            if next_date_str > *new_end_date_str {
                break;
            }

            current_date_str = next_date_str;
            current_num += 1;

            let bill_statement_id = self
                .get_or_create_bill_statement(&current_date_str)
                .await
                .ok();
            let bill_statement_name = Some(self.format_bill_statement_name(&current_date_str));

            let new_expense = Expense {
                id: RecordId::new("expenses", Uuid::new_v4().to_string()),
                title: expense.title.clone(),
                amount: expense.amount,
                payment_method: expense.payment_method.clone(),
                payment_method_id: expense.payment_method_id.clone(),
                expense_date: current_date_str.clone(),
                description: expense.description.clone(),
                status: ExpenseStatus::Pending,
                bill_statement: bill_statement_name,
                bill_statement_id,
                category_id: expense.category_id.clone(),
                paid_by: expense.paid_by.clone(),
                recurrence_type: expense.recurrence_type.clone(),
                recurrence_type_id: expense.recurrence_type_id.clone(),
                recurrence_count: expense.recurrence_count,
                recurrence_current: Some(current_num),
                recurrence_total_amount: expense.recurrence_total_amount,
                recurrence_end_date: expense.recurrence_end_date.clone(),
                recurrence_group_id: Some(group_id.clone()),
                created_at: now.clone(),
                updated_at: now.clone(),
            };

            let _ = self.repository.create(new_expense).await;
        }

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        self.repository.delete(id).await
    }
}
