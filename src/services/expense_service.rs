use chrono::{DateTime, Datelike, Months, NaiveDate, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use surrealdb::types::RecordId;
use uuid::Uuid;
use validator::Validate;

use crate::errors::{AppError, AppResult};
use crate::models::{
    BillStatement, BulkExpenseAction, BulkExpenseActionRequest, BulkExpenseActionResponse,
    CreateExpenseRequest, CreateExpensesBulkRequest, Expense, ExpenseMonthSummary,
    ExpenseNavigationMethod, ExpenseNavigationResponse, ExpensePaginationMeta,
    ExpensePaymentMethodSummary, ExpenseQueryParams, ExpenseResponse, ExpenseStatus,
    ExpenseSummaryResponse, ExpenseTotals, PaginatedExpensesResponse, UpdateExpenseRequest,
};
use crate::repositories::{
    BillStatementRepository, CategoryRepository, ExpenseRepository, PaymentMethodRepository,
};

#[derive(Debug, Deserialize)]
struct ExpenseCsvRow {
    title: String,
    amount: String,
    expense_date: String,
    #[serde(default)]
    category_id: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    bill_statement_id: Option<String>,
    #[serde(default)]
    bill_statement: Option<String>,
    #[serde(default)]
    payment_method_id: Option<String>,
    #[serde(default)]
    payment_method: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    paid_by: Option<String>,
    #[serde(default)]
    recurrence_type: Option<String>,
    #[serde(default)]
    recurrence_count: Option<String>,
    #[serde(default)]
    recurrence_current: Option<String>,
    #[serde(default)]
    recurrence_end_date: Option<String>,
}

pub const EXPENSE_IMPORT_TEMPLATE: &str = "title,amount,expense_date,category_id,category,bill_statement_id,bill_statement,payment_method_id,payment_method,description,paid_by,recurrence_type,recurrence_count,recurrence_current,recurrence_end_date\nLunch at restaurant,75000,2026-06-02,,Food,,June 2026,,Cash,Team lunch,Wikra,,,,\nLaptop installment,1250000,2026-06-02,,Office,,June 2026,,Credit Card,Monthly payment,Wikra,installment,12,1,\n";

fn expense_totals(expenses: &[&Expense]) -> ExpenseTotals {
    let mut totals = ExpenseTotals::default();

    for expense in expenses {
        totals.total_count += 1;
        totals.total_amount += expense.amount;
        match expense.status {
            ExpenseStatus::Paid => totals.paid_amount += expense.amount,
            ExpenseStatus::Pending => totals.pending_amount += expense.amount,
            ExpenseStatus::Unpaid => totals.unpaid_amount += expense.amount,
        }
    }

    totals.outstanding_amount = totals.pending_amount + totals.unpaid_amount;
    totals.completion_rate = if totals.total_amount > 0.0 {
        (totals.paid_amount / totals.total_amount) * 100.0
    } else {
        0.0
    };
    totals
}

fn matches_expense(expense: &Expense, query: &ExpenseQueryParams) -> bool {
    if let Some(date_from) = query.expense_date_from.as_deref() {
        let trimmed = date_from.trim();
        if !trimmed.is_empty() && expense.expense_date.as_str() < trimmed {
            return false;
        }
    }
    if let Some(date_to) = query.expense_date_to.as_deref() {
        let trimmed = date_to.trim();
        if !trimmed.is_empty() && expense.expense_date.as_str() > trimmed {
            return false;
        }
    }

    let method_matches = match (&query.payment_method_id, &query.payment_method) {
        (Some(id), Some(name)) => {
            let id_trimmed = id.trim();
            let name_trimmed = name.trim();
            if id_trimmed.is_empty() || id_trimmed == "all" || name_trimmed.is_empty() || name_trimmed == "all" {
                true
            } else {
                expense.payment_method_id.as_deref() == Some(id_trimmed)
                    || expense.payment_method.eq_ignore_ascii_case(name_trimmed)
            }
        }
        (Some(id), None) => {
            let id_trimmed = id.trim();
            id_trimmed.is_empty()
                || id_trimmed == "all"
                || expense.payment_method_id.as_deref() == Some(id_trimmed)
        }
        (None, Some(name)) => {
            let name_trimmed = name.trim();
            name_trimmed.is_empty()
                || name_trimmed == "all"
                || expense.payment_method.eq_ignore_ascii_case(name_trimmed)
        }
        (None, None) => true,
    };
    if !method_matches {
        return false;
    }

    if let Some(paid_by) = query.paid_by.as_deref() {
        let trimmed = paid_by.trim();
        if !trimmed.is_empty()
            && trimmed != "all"
            && !expense
                .paid_by
                .as_deref()
                .is_some_and(|p| p.eq_ignore_ascii_case(trimmed))
        {
            return false;
        }
    }
    if let Some(status) = query.status.as_deref() {
        let trimmed = status.trim().to_lowercase();
        if !trimmed.is_empty()
            && trimmed != "all"
            && expense.status.to_string().to_lowercase() != trimmed
        {
            return false;
        }
    }
    if let Some(statement_id) = query.bill_statement_id.as_deref() {
        let trimmed = statement_id.trim();
        if !trimmed.is_empty()
            && trimmed != "all"
            && expense.bill_statement_id.as_deref() != Some(trimmed)
        {
            return false;
        }
    }
    if let Some(category_id) = query.category_id.as_deref() {
        let trimmed = category_id.trim();
        if !trimmed.is_empty()
            && trimmed != "all"
            && expense.category_id.as_deref() != Some(trimmed)
        {
            return false;
        }
    } else if let Some(category) = query.category.as_deref() {
        let trimmed = category.trim();
        if !trimmed.is_empty()
            && trimmed != "all"
            && expense.category_id.as_deref() != Some(trimmed)
        {
            return false;
        }
    }
    if let Some(search) = query.search.as_deref() {
        let trimmed = search.trim().to_lowercase();
        if !trimmed.is_empty() {
            let title_matches = expense.title.to_lowercase().contains(&trimmed);
            let desc_matches = expense
                .description
                .as_deref()
                .is_some_and(|d| d.to_lowercase().contains(&trimmed));
            if !title_matches && !desc_matches {
                return false;
            }
        }
    }

    if let Some(expense_type) = query.expense_type.as_deref() {
        let recurrence_type = expense
            .recurrence_type
            .as_deref()
            .unwrap_or("none")
            .trim()
            .to_lowercase();
        let matches_type = match expense_type.trim().to_lowercase().as_str() {
            "transaction" | "regular" | "none" => {
                recurrence_type.is_empty() || recurrence_type == "none"
            }
            "installment" => recurrence_type == "installment",
            "subscription" => recurrence_type == "subscription",
            _ => true,
        };
        if !matches_type {
            return false;
        }
    }

    true
}

fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    s.parse::<DateTime<Utc>>().ok()
}

fn clean_option(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn required_text(value: String, row_number: usize, field: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(format!(
            "CSV row {}: {} is required",
            row_number, field
        )));
    }
    Ok(trimmed.to_string())
}

fn parse_amount(value: &str, row_number: usize) -> AppResult<f64> {
    let normalized = value
        .trim()
        .replace("Rp", "")
        .replace("IDR", "")
        .replace(',', "")
        .replace(' ', "");

    let amount = normalized.parse::<f64>().map_err(|_| {
        AppError::Validation(format!(
            "CSV row {}: amount must be a valid number",
            row_number
        ))
    })?;

    if amount <= 0.0 {
        return Err(AppError::Validation(format!(
            "CSV row {}: amount must be greater than 0",
            row_number
        )));
    }

    Ok(amount)
}

fn parse_csv_date(value: &str, row_number: usize, field: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::Validation(format!(
            "CSV row {}: {} is required",
            row_number, field
        )));
    }

    if let Ok(dt) = trimmed.parse::<DateTime<Utc>>() {
        return Ok(dt.to_rfc3339());
    }

    let date = chrono::NaiveDate::parse_from_str(trimmed, "%Y-%m-%d").map_err(|_| {
        AppError::Validation(format!(
            "CSV row {}: {} must be YYYY-MM-DD or RFC3339",
            row_number, field
        ))
    })?;

    let dt = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::Validation(format!("CSV row {}: invalid date", row_number)))?
        .and_utc();
    Ok(dt.to_rfc3339())
}

fn parse_optional_u32(
    value: Option<String>,
    row_number: usize,
    field: &str,
) -> AppResult<Option<u32>> {
    let Some(value) = clean_option(value) else {
        return Ok(None);
    };

    value.parse::<u32>().map(Some).map_err(|_| {
        AppError::Validation(format!(
            "CSV row {}: {} must be a whole number",
            row_number, field
        ))
    })
}

fn resolve_lookup<T>(
    id: Option<String>,
    name: Option<String>,
    records: &[T],
    get_name: impl Fn(&T) -> &str,
    get_id: impl Fn(&T) -> String,
    row_number: usize,
    label: &str,
) -> AppResult<String> {
    if let Some(id) = id {
        if records.iter().any(|record| get_id(record) == id) {
            return Ok(id);
        }
        return Err(AppError::Validation(format!(
            "CSV row {}: {}_id '{}' was not found",
            row_number, label, id
        )));
    }

    if let Some(name) = name {
        if let Some(record) = records
            .iter()
            .find(|record| get_name(record).eq_ignore_ascii_case(&name))
        {
            return Ok(get_id(record));
        }
        return Err(AppError::Validation(format!(
            "CSV row {}: {} '{}' was not found",
            row_number, label, name
        )));
    }

    Err(AppError::Validation(format!(
        "CSV row {}: {}_id or {} is required",
        row_number, label, label
    )))
}

fn resolve_payment_method_row(
    id: Option<String>,
    name: Option<String>,
    records: &[crate::models::PaymentMethod],
    row_number: usize,
) -> AppResult<(String, String)> {
    if let Some(id) = id {
        if let Some(record) = records
            .iter()
            .find(|record| crate::models::record_key_to_string(&record.id.key) == id)
        {
            return Ok((id, record.name.clone()));
        }
        return Err(AppError::Validation(format!(
            "CSV row {}: payment_method_id '{}' was not found",
            row_number, id
        )));
    }

    if let Some(name) = name {
        if let Some(record) = records
            .iter()
            .find(|record| record.name.eq_ignore_ascii_case(&name))
        {
            return Ok((
                crate::models::record_key_to_string(&record.id.key),
                record.name.clone(),
            ));
        }
        return Err(AppError::Validation(format!(
            "CSV row {}: payment_method '{}' was not found",
            row_number, name
        )));
    }

    Err(AppError::Validation(format!(
        "CSV row {}: payment_method_id or payment_method is required",
        row_number
    )))
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

fn bill_statement_sort_value(statement: &BillStatement) -> i64 {
    if let Some(statement_date) = statement.statement_date.as_deref() {
        if let Some(date) = parse_date(statement_date) {
            return date.timestamp();
        }
    }

    NaiveDate::parse_from_str(&format!("1 {}", statement.name), "%d %B %Y")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|date| date.and_utc().timestamp())
        .unwrap_or(i64::MAX)
}

#[derive(Clone)]
pub struct ExpenseService {
    repository: ExpenseRepository,
    category_repository: CategoryRepository,
    bill_statement_repository: BillStatementRepository,
    payment_method_repository: PaymentMethodRepository,
}

impl ExpenseService {
    pub fn new(
        repository: ExpenseRepository,
        category_repository: CategoryRepository,
        bill_statement_repository: BillStatementRepository,
        payment_method_repository: PaymentMethodRepository,
    ) -> Self {
        Self {
            repository,
            category_repository,
            bill_statement_repository,
            payment_method_repository,
        }
    }

    pub async fn create(
        &self,
        request: CreateExpenseRequest,
        owner_id: &str,
    ) -> AppResult<Expense> {
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
        if is_installment {
            self.validate_installment_progress(
                request.recurrence_count,
                request.recurrence_current,
            )?;
        }

        let (start_num, end_num) =
            self.calculate_range(&request, is_installment, &resolved_recurrence_type);
        let total_to_create = if end_num >= start_num {
            end_num - start_num + 1
        } else {
            1
        };

        let first_bill_statement_id = request.bill_statement_id.clone();

        let first_bill_statement_name = if let Some(ref name) = request.bill_statement {
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
            owner_id: owner_id.to_string(),
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
                    owner_id: owner_id.to_string(),
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

    pub async fn create_bulk(
        &self,
        request: CreateExpensesBulkRequest,
        owner_id: &str,
    ) -> AppResult<Vec<Expense>> {
        if request.expenses.is_empty() {
            return Err(AppError::Validation(
                "At least one expense is required".to_string(),
            ));
        }

        let mut created: Vec<Expense> = Vec::with_capacity(request.expenses.len());
        for (idx, item) in request.expenses.into_iter().enumerate() {
            let expense = self.create(item, owner_id).await.map_err(|e| match e {
                AppError::Validation(msg) => {
                    AppError::Validation(format!("Expense #{}: {}", idx + 1, msg))
                }
                other => other,
            })?;
            created.push(expense);
        }

        Ok(created)
    }

    pub async fn import_csv(&self, bytes: &[u8], owner_id: &str) -> AppResult<Vec<Expense>> {
        if bytes.is_empty() {
            return Err(AppError::Validation("CSV file cannot be empty".to_string()));
        }

        let categories = self.category_repository.find_all().await?;
        let bill_statements = self.bill_statement_repository.find_all().await?;
        let payment_methods = self.payment_method_repository.find_all().await?;

        let mut reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .flexible(true)
            .from_reader(bytes);
        let mut requests = Vec::new();

        for (idx, row) in reader.deserialize::<ExpenseCsvRow>().enumerate() {
            let row_number = idx + 2;
            let row = row.map_err(|e| {
                AppError::Validation(format!("CSV row {} could not be read: {}", row_number, e))
            })?;
            requests.push(self.csv_row_to_request(
                row,
                row_number,
                &categories,
                &bill_statements,
                &payment_methods,
            )?);
        }

        if requests.is_empty() {
            return Err(AppError::Validation(
                "CSV file must contain at least one expense row".to_string(),
            ));
        }

        self.create_bulk(CreateExpensesBulkRequest { expenses: requests }, owner_id)
            .await
    }

    pub async fn apply_bulk_action(
        &self,
        request: BulkExpenseActionRequest,
        owner_id: &str,
    ) -> AppResult<BulkExpenseActionResponse> {
        let expense_ids = self.normalize_bulk_expense_ids(request.expense_ids)?;

        let mut expenses = Vec::with_capacity(expense_ids.len());
        for id in &expense_ids {
            expenses.push(self.repository.find_by_id(id, owner_id).await?);
        }

        match request.action {
            BulkExpenseAction::Delete => {
                for id in &expense_ids {
                    self.repository.delete(id, owner_id).await?;
                }

                Ok(BulkExpenseActionResponse {
                    updated: Vec::new(),
                    deleted_count: expense_ids.len(),
                    count: expense_ids.len(),
                })
            }
            BulkExpenseAction::SetStatus => {
                let status = request.status.ok_or_else(|| {
                    AppError::Validation("status is required for bulk status changes".to_string())
                })?;
                let now = Utc::now().to_rfc3339();
                let mut updated = Vec::with_capacity(expense_ids.len());

                for (id, mut expense) in expense_ids.iter().zip(expenses.into_iter()) {
                    expense.status = status.clone();
                    expense.updated_at = now.clone();
                    updated.push(ExpenseResponse::from(
                        self.repository.update(id, owner_id, expense).await?,
                    ));
                }

                Ok(BulkExpenseActionResponse {
                    count: updated.len(),
                    updated,
                    deleted_count: 0,
                })
            }
            BulkExpenseAction::MoveBillStatement => {
                let bill_statement_id = request
                    .bill_statement_id
                    .filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| {
                        AppError::Validation(
                            "bill_statement_id is required for moving expenses".to_string(),
                        )
                    })?;
                let bill_statement = self
                    .bill_statement_repository
                    .find_by_id(&bill_statement_id)
                    .await?;
                let now = Utc::now().to_rfc3339();
                let mut updated = Vec::with_capacity(expense_ids.len());

                for (id, mut expense) in expense_ids.iter().zip(expenses.into_iter()) {
                    expense.bill_statement_id = Some(bill_statement_id.clone());
                    expense.bill_statement = Some(bill_statement.name.clone());

                    // Auto-increment recurrence_current for installment expenses
                    if expense.recurrence_type.as_deref() == Some("installment") {
                        if let Some(current) = expense.recurrence_current {
                            let max = expense.recurrence_count.unwrap_or(u32::MAX);
                            if current < max {
                                expense.recurrence_current = Some(current + 1);
                                // Also advance the expense_date by 1 month
                                expense.expense_date =
                                    add_months_to_date_str(&expense.expense_date, 1);
                            }
                        }
                    }

                    expense.updated_at = now.clone();
                    updated.push(ExpenseResponse::from(
                        self.repository.update(id, owner_id, expense).await?,
                    ));
                }

                Ok(BulkExpenseActionResponse {
                    count: updated.len(),
                    updated,
                    deleted_count: 0,
                })
            }
            BulkExpenseAction::MoveNextBillStatement => {
                let mut bill_statements = self.bill_statement_repository.find_all().await?;
                bill_statements.retain(|statement| statement.is_active);
                bill_statements.sort_by_key(bill_statement_sort_value);
                let now = Utc::now().to_rfc3339();
                let mut updated = Vec::with_capacity(expense_ids.len());

                for (id, mut expense) in expense_ids.iter().zip(expenses.into_iter()) {
                    let current_index = expense.bill_statement_id.as_ref().and_then(|current_id| {
                        bill_statements.iter().position(|statement| {
                            crate::models::record_key_to_string(&statement.id.key) == *current_id
                        })
                    });
                    let next_statement =
                        current_index.and_then(|index| bill_statements.get(index + 1).cloned());

                    let target = if let Some(statement) = next_statement {
                        statement
                    } else {
                        let base_date = current_index
                            .and_then(|index| bill_statements.get(index))
                            .and_then(|statement| statement.statement_date.as_deref())
                            .unwrap_or(&expense.expense_date);
                        let next_date = add_months_to_date_str(base_date, 1);
                        let next_id = self.get_or_create_bill_statement(&next_date).await?;
                        let statement = self.bill_statement_repository.find_by_id(&next_id).await?;
                        if !bill_statements.iter().any(|existing| {
                            crate::models::record_key_to_string(&existing.id.key) == next_id
                        }) {
                            bill_statements.push(statement.clone());
                            bill_statements.sort_by_key(bill_statement_sort_value);
                        }
                        statement
                    };

                    expense.bill_statement_id =
                        Some(crate::models::record_key_to_string(&target.id.key));
                    expense.bill_statement = Some(target.name);
                    expense.expense_date = add_months_to_date_str(&expense.expense_date, 1);

                    if expense
                        .recurrence_type
                        .as_deref()
                        .is_some_and(|value| value.eq_ignore_ascii_case("installment"))
                    {
                        if let Some(current) = expense.recurrence_current {
                            let max = expense.recurrence_count.unwrap_or(u32::MAX);
                            if current < max {
                                expense.recurrence_current = Some(current + 1);
                            }
                        }
                    }

                    expense.updated_at = now.clone();
                    updated.push(ExpenseResponse::from(
                        self.repository.update(id, owner_id, expense).await?,
                    ));
                }

                Ok(BulkExpenseActionResponse {
                    count: updated.len(),
                    updated,
                    deleted_count: 0,
                })
            }
        }
    }

    fn normalize_bulk_expense_ids(&self, expense_ids: Vec<String>) -> AppResult<Vec<String>> {
        if expense_ids.is_empty() {
            return Err(AppError::Validation(
                "At least one expense id is required".to_string(),
            ));
        }

        let mut seen = std::collections::HashSet::new();
        let mut normalized_ids = Vec::with_capacity(expense_ids.len());

        for raw_id in expense_ids {
            let id = raw_id
                .trim()
                .strip_prefix("expenses:")
                .unwrap_or(raw_id.trim())
                .to_string();

            if id.is_empty() {
                return Err(AppError::Validation(
                    "Expense ids cannot contain empty values".to_string(),
                ));
            }

            if seen.insert(id.clone()) {
                normalized_ids.push(id);
            }
        }

        Ok(normalized_ids)
    }

    fn csv_row_to_request(
        &self,
        row: ExpenseCsvRow,
        row_number: usize,
        categories: &[crate::models::Category],
        bill_statements: &[crate::models::BillStatement],
        payment_methods: &[crate::models::PaymentMethod],
    ) -> AppResult<CreateExpenseRequest> {
        let title = required_text(row.title, row_number, "title")?;
        let amount = parse_amount(&row.amount, row_number)?;
        let expense_date = parse_csv_date(&row.expense_date, row_number, "expense_date")?;
        let bill_statement_name = clean_option(row.bill_statement);
        let category_id = resolve_lookup(
            clean_option(row.category_id),
            clean_option(row.category),
            categories,
            |category| &category.name,
            |category| crate::models::record_key_to_string(&category.id.key),
            row_number,
            "category",
        )?;
        let bill_statement_id = resolve_lookup(
            clean_option(row.bill_statement_id),
            bill_statement_name.clone(),
            bill_statements,
            |bill_statement| &bill_statement.name,
            |bill_statement| crate::models::record_key_to_string(&bill_statement.id.key),
            row_number,
            "bill_statement",
        )?;
        let (payment_method_id, payment_method) = resolve_payment_method_row(
            clean_option(row.payment_method_id),
            clean_option(row.payment_method),
            payment_methods,
            row_number,
        )?;

        let recurrence_type = clean_option(row.recurrence_type).and_then(|value| {
            if value.eq_ignore_ascii_case("none") {
                None
            } else {
                Some(value)
            }
        });

        let recurrence_count =
            parse_optional_u32(row.recurrence_count, row_number, "recurrence_count")?;
        let recurrence_current =
            parse_optional_u32(row.recurrence_current, row_number, "recurrence_current")?;
        let recurrence_end_date = match clean_option(row.recurrence_end_date) {
            Some(value) => Some(parse_csv_date(&value, row_number, "recurrence_end_date")?),
            None => None,
        };

        Ok(CreateExpenseRequest {
            title,
            amount,
            payment_method: Some(payment_method),
            payment_method_id: Some(payment_method_id),
            expense_date,
            description: clean_option(row.description),
            bill_statement: bill_statement_name,
            bill_statement_id: Some(bill_statement_id),
            category_id: Some(category_id),
            paid_by: clean_option(row.paid_by),
            recurrence_type,
            recurrence_type_id: None,
            recurrence_count,
            recurrence_current,
            recurrence_end_date,
        })
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
                // Only create ONE expense, not the full range
                (start, start)
            }
            _ => (1, 1),
        }
    }

    pub async fn get_by_id(&self, id: &str, owner_id: &str) -> AppResult<Expense> {
        self.repository.find_by_id(id, owner_id).await
    }

    pub async fn get_all(
        &self,
        query: ExpenseQueryParams,
        owner_id: &str,
    ) -> AppResult<PaginatedExpensesResponse> {
        let expenses = self.repository.find_with_query(&query, owner_id).await?;
        let total_items = self.repository.count_with_query(&query, owner_id).await?;

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

    pub async fn get_summary(
        &self,
        query: ExpenseQueryParams,
        owner_id: &str,
    ) -> AppResult<ExpenseSummaryResponse> {
        let all_expenses = self.repository.find_all(owner_id).await?;
        let payment_methods = self.payment_method_repository.find_all().await?;
        let bill_statements = self.bill_statement_repository.find_all().await?;

        let method_types_by_id: HashMap<String, String> = payment_methods
            .iter()
            .map(|method| {
                (
                    crate::models::record_key_to_string(&method.id.key),
                    method.method_type.clone(),
                )
            })
            .collect();
        let statement_meta: HashMap<String, (String, Option<String>, Option<String>)> =
            bill_statements
                .into_iter()
                .map(|statement| {
                    (
                        crate::models::record_key_to_string(&statement.id.key),
                        (statement.name, statement.statement_date, statement.due_date),
                    )
                })
                .collect();

        let filtered: Vec<&Expense> = all_expenses
            .iter()
            .filter(|expense| matches_expense(expense, &query))
            .collect();

        let mut method_groups: HashMap<String, Vec<&Expense>> = HashMap::new();
        for expense in &filtered {
            let key = expense
                .payment_method_id
                .clone()
                .unwrap_or_else(|| format!("name:{}", expense.payment_method));
            method_groups.entry(key).or_default().push(expense);
        }

        let mut method_summaries: Vec<ExpensePaymentMethodSummary> = method_groups
            .into_values()
            .map(|expenses| {
                let first = expenses[0];
                ExpensePaymentMethodSummary {
                    payment_method_id: first.payment_method_id.clone(),
                    name: first.payment_method.clone(),
                    method_type: first
                        .payment_method_id
                        .as_ref()
                        .and_then(|id| method_types_by_id.get(id).cloned()),
                    totals: expense_totals(&expenses),
                }
            })
            .collect();
        method_summaries.sort_by(|a, b| {
            b.totals
                .total_amount
                .total_cmp(&a.totals.total_amount)
                .then_with(|| a.name.cmp(&b.name))
        });

        let months = Self::build_month_summaries(&filtered, &statement_meta);

        Ok(ExpenseSummaryResponse {
            totals: expense_totals(&filtered),
            payment_methods: method_summaries,
            months,
        })
    }

    pub async fn get_navigation(
        &self,
        query: ExpenseQueryParams,
        owner_id: &str,
    ) -> AppResult<ExpenseNavigationResponse> {
        let expenses = self.repository.find_all(owner_id).await?;
        let expenses: Vec<Expense> = expenses
            .into_iter()
            .filter(|expense| matches_expense(expense, &query))
            .collect();
        let payment_methods = self.payment_method_repository.find_all().await?;
        let bill_statements = self.bill_statement_repository.find_all().await?;

        let method_meta_by_key: HashMap<String, (Option<String>, String, Option<String>)> =
            payment_methods
                .iter()
                .map(|method| {
                    let id = crate::models::record_key_to_string(&method.id.key);
                    (
                        id.clone(),
                        (
                            Some(id),
                            method.name.clone(),
                            Some(method.method_type.clone()),
                        ),
                    )
                })
                .collect();
        let method_types_by_id: HashMap<String, String> = payment_methods
            .iter()
            .map(|method| {
                (
                    crate::models::record_key_to_string(&method.id.key),
                    method.method_type.clone(),
                )
            })
            .collect();
        let statement_meta: HashMap<String, (String, Option<String>, Option<String>)> =
            bill_statements
                .into_iter()
                .map(|statement| {
                    (
                        crate::models::record_key_to_string(&statement.id.key),
                        (statement.name, statement.statement_date, statement.due_date),
                    )
                })
                .collect();

        let mut method_groups: HashMap<String, Vec<&Expense>> = HashMap::new();
        for expense in &expenses {
            let key = expense
                .payment_method_id
                .clone()
                .unwrap_or_else(|| format!("name:{}", expense.payment_method));
            method_groups.entry(key).or_default().push(expense);
        }

        let mut methods: Vec<ExpenseNavigationMethod> = method_groups
            .into_iter()
            .map(|(method_key, method_expenses)| {
                let fallback_meta = method_meta_by_key.get(&method_key);
                let first = method_expenses.first().copied();
                let payment_method_id = first
                    .and_then(|expense| expense.payment_method_id.clone())
                    .or_else(|| fallback_meta.and_then(|meta| meta.0.clone()));
                let name = first
                    .map(|expense| expense.payment_method.clone())
                    .or_else(|| fallback_meta.map(|meta| meta.1.clone()))
                    .unwrap_or_else(|| "Unknown method".to_string());
                ExpenseNavigationMethod {
                    payment_method_id: payment_method_id.clone(),
                    name,
                    method_type: payment_method_id
                        .as_ref()
                        .and_then(|id| method_types_by_id.get(id).cloned())
                        .or_else(|| fallback_meta.and_then(|meta| meta.2.clone())),
                    totals: expense_totals(&method_expenses),
                    months: Self::build_month_summaries(&method_expenses, &statement_meta),
                }
            })
            .collect();
        methods.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(ExpenseNavigationResponse { methods })
    }

    fn build_month_summaries(
        expenses: &[&Expense],
        statement_meta: &HashMap<String, (String, Option<String>, Option<String>)>,
    ) -> Vec<ExpenseMonthSummary> {
        let mut month_groups: HashMap<String, Vec<&Expense>> = HashMap::new();
        for expense in expenses {
            if let Some(statement_id) = &expense.bill_statement_id {
                month_groups
                    .entry(statement_id.clone())
                    .or_default()
                    .push(expense);
            }
        }

        let mut months: Vec<ExpenseMonthSummary> = month_groups
            .into_iter()
            .map(|(statement_id, month_expenses)| {
                let fallback_name = month_expenses[0]
                    .bill_statement
                    .clone()
                    .unwrap_or_else(|| "Unknown month".to_string());
                let (name, statement_date, due_date) = statement_meta
                    .get(&statement_id)
                    .cloned()
                    .unwrap_or((fallback_name, None, None));
                ExpenseMonthSummary {
                    bill_statement_id: statement_id,
                    name,
                    statement_date,
                    due_date,
                    totals: expense_totals(&month_expenses),
                }
            })
            .collect();
        months.sort_by(|a, b| {
            let a_key = a.statement_date.as_ref().unwrap_or(&a.name);
            let b_key = b.statement_date.as_ref().unwrap_or(&b.name);
            b_key.cmp(a_key)
        });
        months
    }

    pub async fn update(
        &self,
        id: &str,
        request: UpdateExpenseRequest,
        owner_id: &str,
    ) -> AppResult<Expense> {
        let mut expense = self.repository.find_by_id(id, owner_id).await?;
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
            let trimmed = description.trim();
            expense.description = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
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
            let trimmed = paid_by.trim();
            expense.paid_by = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            };
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
        if let Some(recurrence_end_date) = request.recurrence_end_date {
            expense.recurrence_end_date = Some(recurrence_end_date);
        }
        if let Some(recurrence_group_id) = request.recurrence_group_id {
            expense.recurrence_group_id = Some(recurrence_group_id);
        }

        if schedule_type_was_requested {
            let is_installment = self.is_installment_schedule(&expense.recurrence_type)?;
            if is_installment {
                self.validate_installment_progress(
                    expense.recurrence_count,
                    expense.recurrence_current,
                )?;
            } else if expense
                .recurrence_type
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("subscription"))
            {
                expense.recurrence_count = None;
                expense.recurrence_current = None;
                expense.recurrence_group_id = None;
            }
        }

        expense.updated_at = Utc::now().to_rfc3339();

        let new_recurrence_type = expense.recurrence_type.clone();
        let is_changing_to_one_time = request.clear_recurrence
            || self.is_changing_to_one_time(&old_recurrence_type, &new_recurrence_type);

        if is_changing_to_one_time {
            if let Some(ref group_id) = old_recurrence_group_id {
                let _ = self
                    .repository
                    .delete_by_recurrence_group_id_except(group_id, id, owner_id)
                    .await;
            }
            expense.recurrence_type = Some("none".to_string());
            expense.recurrence_type_id = None;
            expense.recurrence_group_id = None;
            expense.recurrence_current = None;
            expense.recurrence_count = None;
            expense.recurrence_end_date = None;
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
                self.extend_recurring_expenses(&expense, owner_id).await?;
            }
        }

        self.repository.update(id, owner_id, expense).await
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

        if lower == "subscription" {
            return Ok(false);
        }

        Err(AppError::Validation(
            "Only Installment and Subscription schedule types are supported".to_string(),
        ))
    }

    fn validate_installment_progress(
        &self,
        recurrence_count: Option<u32>,
        recurrence_current: Option<u32>,
    ) -> AppResult<()> {
        let Some(total) = recurrence_count else {
            return Err(AppError::Validation(
                "recurrence_count is required for installment expenses".to_string(),
            ));
        };

        if total == 0 {
            return Err(AppError::Validation(
                "recurrence_count must be at least 1".to_string(),
            ));
        }

        if let Some(current) = recurrence_current {
            if current == 0 || current > total {
                return Err(AppError::Validation(format!(
                    "recurrence_current must be between 1 and {}",
                    total
                )));
            }
        }

        Ok(())
    }

    fn should_extend_recurrence(
        &self,
        _old_end_date: &Option<String>,
        _new_end_date: &Option<String>,
        _recurrence_type: &Option<String>,
    ) -> bool {
        false
    }

    async fn extend_recurring_expenses(&self, expense: &Expense, owner_id: &str) -> AppResult<()> {
        let Some(ref group_id) = expense.recurrence_group_id else {
            return Ok(());
        };

        let Some(ref new_end_date_str) = expense.recurrence_end_date else {
            return Ok(());
        };

        let latest = self
            .repository
            .get_latest_expense_in_group(group_id, owner_id)
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
                owner_id: owner_id.to_string(),
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
                recurrence_end_date: expense.recurrence_end_date.clone(),
                recurrence_group_id: Some(group_id.clone()),
                created_at: now.clone(),
                updated_at: now.clone(),
            };

            let _ = self.repository.create(new_expense).await;
        }

        Ok(())
    }

    pub async fn delete(&self, id: &str, owner_id: &str) -> AppResult<()> {
        self.repository.delete(id, owner_id).await
    }
}
