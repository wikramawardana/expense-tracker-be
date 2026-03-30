use chrono::{Datelike, Months, Utc};
use uuid::Uuid;
use validator::Validate;

use crate::errors::{AppError, AppResult};
use crate::models::{
    BillStatement, CreateExpenseRequest, Expense, ExpensePaginationMeta, ExpenseQueryParams,
    ExpenseResponse, ExpenseStatus, PaginatedExpensesResponse, UpdateExpenseRequest,
};
use crate::repositories::{
    BillStatementRepository, ExpenseRepository, PaymentMethodRepository, RecurrenceTypeRepository,
};

#[derive(Clone)]
pub struct ExpenseService {
    repository: ExpenseRepository,
    bill_statement_repository: BillStatementRepository,
    payment_method_repository: PaymentMethodRepository,
    recurrence_type_repository: RecurrenceTypeRepository,
}

impl ExpenseService {
    pub fn new(
        repository: ExpenseRepository,
        bill_statement_repository: BillStatementRepository,
        payment_method_repository: PaymentMethodRepository,
        recurrence_type_repository: RecurrenceTypeRepository,
    ) -> Self {
        Self {
            repository,
            bill_statement_repository,
            payment_method_repository,
            recurrence_type_repository,
        }
    }

    pub async fn create(&self, request: CreateExpenseRequest) -> AppResult<Expense> {
        request
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;

        // Validate required fields
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

        let now = Utc::now();
        let group_id = Uuid::new_v4().to_string();

        // Resolve recurrence type name first (needed for both is_recurring check and calculate_range)
        let resolved_recurrence_type = self
            .resolve_recurrence_type(&request.recurrence_type, &request.recurrence_type_id)
            .await;

        // Check if this is a recurring expense
        let is_recurring = if let Some(ref rt_name) = resolved_recurrence_type {
            let lower = rt_name.to_lowercase();
            lower != "none" && !lower.is_empty()
        } else {
            false
        };

        // Validate recurrence_type_id is required for recurring expenses
        if is_recurring && request.recurrence_type_id.is_none() {
            return Err(AppError::Validation(
                "recurrence_type_id is required for recurring expenses".to_string(),
            ));
        }

        // Calculate start and end for installments
        let (start_num, end_num) = self.calculate_range(&request, is_recurring, &resolved_recurrence_type);
        let total_to_create = if end_num >= start_num {
            end_num - start_num + 1
        } else {
            1
        };

        // Get or create bill statement for the first expense date
        let first_bill_statement_id = if is_recurring {
            Some(
                self.get_or_create_bill_statement(request.expense_date)
                    .await?,
            )
        } else {
            request.bill_statement_id.clone()
        };

        // Generate bill statement name for first expense
        let first_bill_statement_name = if is_recurring {
            Some(self.format_bill_statement_name(request.expense_date))
        } else if let Some(ref name) = request.bill_statement {
            // Use provided bill_statement name
            Some(name.clone())
        } else if let Some(ref bs_id) = request.bill_statement_id {
            // Look up bill_statement name from bill_statement_id
            self.bill_statement_repository
                .find_by_id(bs_id)
                .await
                .ok()
                .map(|bs| bs.name)
        } else {
            None
        };

        // Resolve payment method name: use provided name, or lookup from ID
        let payment_method_name = self.resolve_payment_method(&request).await?;

        // Use resolved recurrence type name
        let recurrence_type_name = resolved_recurrence_type.clone();

        let first_expense = Expense {
            id: Uuid::new_v4().to_string(),
            title: request.title.clone(),
            amount: request.amount,
            payment_method: payment_method_name.clone(),
            payment_method_id: request.payment_method_id.clone(),
            expense_date: request.expense_date,
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
            recurrence_end_date: request.recurrence_end_date,
            recurrence_group_id: if total_to_create > 1 {
                Some(group_id.clone())
            } else {
                None
            },
            created_at: now,
            updated_at: now,
        };

        // Create the first expense
        let created_first = self.repository.create(first_expense).await?;

        // Auto-generate remaining occurrences with auto-created bill statements
        if total_to_create > 1 {
            for i in (start_num + 1)..=end_num {
                let months_offset = i - start_num;
                let future_date = request
                    .expense_date
                    .checked_add_months(Months::new(months_offset))
                    .unwrap_or(request.expense_date);

                // Get or create bill statement for this future date
                let bill_statement_id = self.get_or_create_bill_statement(future_date).await.ok();
                let bill_statement_name = Some(self.format_bill_statement_name(future_date));

                let future_expense = Expense {
                    id: Uuid::new_v4().to_string(),
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
                    recurrence_end_date: request.recurrence_end_date,
                    recurrence_group_id: Some(group_id.clone()),
                    created_at: now,
                    updated_at: now,
                };

                let _ = self.repository.create(future_expense).await;
            }
        }

        Ok(created_first)
    }

    /// Format bill statement name from date: "January 2026", "February 2026", etc.
    fn format_bill_statement_name(&self, date: chrono::DateTime<Utc>) -> String {
        let month_name = match date.month() {
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
        format!("{} {}", month_name, date.year())
    }

    /// Resolve payment method name: use provided name, or lookup from ID
    async fn resolve_payment_method(&self, request: &CreateExpenseRequest) -> AppResult<String> {
        // If payment_method is provided, use it directly
        if let Some(ref name) = request.payment_method {
            if !name.is_empty() {
                return Ok(name.clone());
            }
        }

        // If payment_method_id is provided, lookup the name
        if let Some(ref pm_id) = request.payment_method_id {
            let payment_method = self.payment_method_repository.find_by_id(pm_id).await?;
            return Ok(payment_method.name);
        }

        // Neither provided - return error
        Err(AppError::Validation(
            "Either payment_method or payment_method_id must be provided".to_string(),
        ))
    }

    /// Resolve recurrence type name: use provided name, or lookup from ID
    async fn resolve_recurrence_type(
        &self,
        name: &Option<String>,
        id: &Option<String>,
    ) -> Option<String> {
        // If name is provided, use it directly
        if let Some(ref rt_name) = name {
            if !rt_name.is_empty() {
                return Some(rt_name.clone());
            }
        }

        // If id is provided, lookup the name
        if let Some(ref rt_id) = id {
            if let Ok(rt) = self.recurrence_type_repository.find_by_id(rt_id).await {
                return Some(rt.name);
            }
        }

        None
    }

    /// Get existing bill statement by name or create a new one
    async fn get_or_create_bill_statement(&self, date: chrono::DateTime<Utc>) -> AppResult<String> {
        let name = self.format_bill_statement_name(date);

        // Try to find existing bill statement
        if let Some(existing) = self.bill_statement_repository.find_by_name(&name).await? {
            return Ok(existing.id);
        }

        // Create new bill statement
        let now = Utc::now();
        let new_bill_statement = BillStatement {
            id: Uuid::new_v4().to_string(),
            name: name.clone(),
            payment_method_id: None,
            statement_date: Some(date),
            due_date: None,
            description: Some(format!("Auto-created for {}", name)),
            is_active: true,
            created_at: now,
            updated_at: now,
        };

        let created = self
            .bill_statement_repository
            .create(new_bill_statement)
            .await?;
        Ok(created.id)
    }

    /// Calculate the range of installment numbers to generate (start_num, end_num)
    fn calculate_range(&self, request: &CreateExpenseRequest, is_recurring: bool, resolved_recurrence_type: &Option<String>) -> (u32, u32) {
        if !is_recurring {
            return (1, 1);
        }

        let rt_name = resolved_recurrence_type.as_ref().map(|s| s.to_lowercase());

        match rt_name.as_deref() {
            Some("installment") => {
                let total = request.recurrence_count.unwrap_or(1);
                let start = request.recurrence_current.unwrap_or(1).max(1).min(total);
                (start, total)
            }
            Some("subscription") | Some("recurring") => {
                let total_months = if let Some(end_date) = request.recurrence_end_date {
                    let start = request.expense_date;
                    let months_diff = (end_date.year() - start.year()) * 12
                        + (end_date.month() as i32 - start.month() as i32);
                    // Include the end month by adding 1
                    if months_diff >= 0 {
                        ((months_diff + 1) as u32).min(120)
                    } else {
                        1
                    }
                } else {
                    12
                };
                (1, total_months)
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
        let old_recurrence_end_date = expense.recurrence_end_date;
        let old_recurrence_group_id = expense.recurrence_group_id.clone();

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
        // Handle bill_statement_id first to lookup name if needed
        if let Some(ref bill_statement_id) = request.bill_statement_id {
            expense.bill_statement_id = Some(bill_statement_id.clone());
            // If bill_statement_id is provided but bill_statement name is not, look it up
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
        // Handle recurrence_type_id first to lookup name if needed
        if let Some(ref recurrence_type_id) = request.recurrence_type_id {
            expense.recurrence_type_id = Some(recurrence_type_id.clone());
            // If recurrence_type_id is provided but recurrence_type name is not, look it up
            if request.recurrence_type.is_none() {
                if let Ok(rt) = self
                    .recurrence_type_repository
                    .find_by_id(recurrence_type_id)
                    .await
                {
                    expense.recurrence_type = Some(rt.name);
                }
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

        expense.updated_at = Utc::now();

        // Check if clear_recurrence flag is set OR recurrence type changed to "none"
        let new_recurrence_type = expense.recurrence_type.clone();
        let is_changing_to_one_time = request.clear_recurrence 
            || self.is_changing_to_one_time(&old_recurrence_type, &new_recurrence_type);
        
        if is_changing_to_one_time {
            if let Some(ref group_id) = old_recurrence_group_id {
                // Delete all other expenses in the recurrence group
                let _ = self.repository.delete_by_recurrence_group_id_except(group_id, id).await;
            }
            // Clear recurrence fields for this expense
            expense.recurrence_type = Some("none".to_string());
            expense.recurrence_type_id = None;
            expense.recurrence_group_id = None;
            expense.recurrence_current = None;
            expense.recurrence_count = None;
            expense.recurrence_end_date = None;
            expense.recurrence_total_amount = None;
        } else {
            // Check if recurrence_end_date was extended - auto-create missing future expenses
            let should_extend = self.should_extend_recurrence(
                &old_recurrence_end_date,
                &expense.recurrence_end_date,
                &expense.recurrence_type,
            );
            
            if should_extend {
                // If expense doesn't have a group_id yet, generate one
                if expense.recurrence_group_id.is_none() {
                    expense.recurrence_group_id = Some(Uuid::new_v4().to_string());
                    expense.recurrence_current = Some(1); // This is the first in the group
                }
                self.extend_recurring_expenses(&expense).await?;
            }
        }

        self.repository.update(id, expense).await
    }

    /// Check if the recurrence type is changing from recurring to one-time
    fn is_changing_to_one_time(&self, old_type: &Option<String>, new_type: &Option<String>) -> bool {
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

    /// Check if we should extend the recurrence (end date was extended)
    fn should_extend_recurrence(
        &self,
        old_end_date: &Option<chrono::DateTime<Utc>>,
        new_end_date: &Option<chrono::DateTime<Utc>>,
        recurrence_type: &Option<String>,
    ) -> bool {
        // Only extend for subscription/recurring types
        let is_recurring = recurrence_type
            .as_ref()
            .map(|t| {
                let lower = t.to_lowercase();
                lower == "subscription" || lower == "recurring"
            })
            .unwrap_or(false);

        if !is_recurring {
            return false;
        }

        match (old_end_date, new_end_date) {
            (Some(old), Some(new)) => new > old,
            (None, Some(_)) => true, // Setting an end date where there was none
            _ => false,
        }
    }

    /// Extend recurring expenses by creating missing future expenses
    async fn extend_recurring_expenses(&self, expense: &Expense) -> AppResult<()> {
        let Some(ref group_id) = expense.recurrence_group_id else {
            return Ok(());
        };

        let Some(new_end_date) = expense.recurrence_end_date else {
            return Ok(());
        };

        // Find the latest expense in the group, or use current expense as starting point
        let latest = self.repository.get_latest_expense_in_group(group_id).await?;
        let (mut current_date, mut current_num) = if let Some(latest_expense) = latest {
            (latest_expense.expense_date, latest_expense.recurrence_current.unwrap_or(1))
        } else {
            // No existing expenses in group yet, use current expense as starting point
            (expense.expense_date, expense.recurrence_current.unwrap_or(1))
        };
        
        let now = Utc::now();

        // Generate expenses for each month until new_end_date
        loop {
            // Move to next month
            let next_date = current_date
                .checked_add_months(Months::new(1))
                .unwrap_or(current_date);
            
            // Stop if we've passed the end date
            if next_date > new_end_date {
                break;
            }

            current_date = next_date;
            current_num += 1;

            // Get or create bill statement for this date
            let bill_statement_id = self.get_or_create_bill_statement(current_date).await.ok();
            let bill_statement_name = Some(self.format_bill_statement_name(current_date));

            let new_expense = Expense {
                id: Uuid::new_v4().to_string(),
                title: expense.title.clone(),
                amount: expense.amount,
                payment_method: expense.payment_method.clone(),
                payment_method_id: expense.payment_method_id.clone(),
                expense_date: current_date,
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
                recurrence_end_date: expense.recurrence_end_date,
                recurrence_group_id: Some(group_id.clone()),
                created_at: now,
                updated_at: now,
            };

            let _ = self.repository.create(new_expense).await;
        }

        Ok(())
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        self.repository.delete(id).await
    }
}
