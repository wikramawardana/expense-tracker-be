use serde::{Deserialize, Serialize};
use surrealdb::types::RecordId;
use surrealdb::types::SurrealValue;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default, SurrealValue)]
#[serde(rename_all = "lowercase")]
#[surreal(untagged)]
pub enum ExpenseStatus {
    #[default]
    Pending,
    Unpaid,
    Paid,
}

impl std::fmt::Display for ExpenseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpenseStatus::Pending => write!(f, "pending"),
            ExpenseStatus::Unpaid => write!(f, "unpaid"),
            ExpenseStatus::Paid => write!(f, "paid"),
        }
    }
}

impl std::str::FromStr for ExpenseStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(ExpenseStatus::Pending),
            "unpaid" => Ok(ExpenseStatus::Unpaid),
            "paid" => Ok(ExpenseStatus::Paid),
            _ => Err(format!(
                "Invalid status: {}. Valid values: pending, unpaid, paid",
                s
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct Expense {
    pub id: RecordId,
    pub owner_id: String,
    pub title: String,
    pub amount: f64,
    pub payment_method: String,
    pub payment_method_id: Option<String>,
    pub expense_date: String,
    pub description: Option<String>,
    pub status: ExpenseStatus,
    pub bill_statement: Option<String>,
    pub bill_statement_id: Option<String>,
    pub category_id: Option<String>,
    pub paid_by: Option<String>,
    pub recurrence_type: Option<String>,
    pub recurrence_type_id: Option<String>,
    pub recurrence_count: Option<u32>,
    pub recurrence_current: Option<u32>,
    pub recurrence_end_date: Option<String>,
    pub recurrence_group_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateExpenseRequest {
    #[validate(length(min = 1, message = "Title cannot be empty"))]
    pub title: String,
    pub amount: f64,
    pub payment_method: Option<String>,
    pub payment_method_id: Option<String>,
    pub expense_date: String,
    pub description: Option<String>,
    pub bill_statement: Option<String>,
    pub bill_statement_id: Option<String>,
    pub category_id: Option<String>,
    pub paid_by: Option<String>,
    pub recurrence_type: Option<String>,
    pub recurrence_type_id: Option<String>,
    pub recurrence_count: Option<u32>,
    pub recurrence_current: Option<u32>,
    pub recurrence_end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExpensesBulkRequest {
    pub expenses: Vec<CreateExpenseRequest>,
}

#[derive(Debug, Serialize)]
pub struct BulkCreateExpensesResponse {
    pub created: Vec<ExpenseResponse>,
    pub count: usize,
}

#[derive(Debug, Serialize)]
pub struct ImportExpensesCsvResponse {
    pub created: Vec<ExpenseResponse>,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BulkExpenseAction {
    MoveBillStatement,
    MoveNextBillStatement,
    SetStatus,
    Delete,
}

#[derive(Debug, Deserialize)]
pub struct BulkExpenseActionRequest {
    pub expense_ids: Vec<String>,
    pub action: BulkExpenseAction,
    pub status: Option<ExpenseStatus>,
    pub bill_statement_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BulkExpenseActionResponse {
    pub updated: Vec<ExpenseResponse>,
    pub deleted_count: usize,
    pub count: usize,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateExpenseRequest {
    pub title: Option<String>,
    pub amount: Option<f64>,
    pub payment_method: Option<String>,
    pub payment_method_id: Option<String>,
    pub expense_date: Option<String>,
    pub description: Option<String>,
    pub status: Option<ExpenseStatus>,
    pub bill_statement: Option<String>,
    pub bill_statement_id: Option<String>,
    pub category_id: Option<String>,
    pub paid_by: Option<String>,
    pub recurrence_type: Option<String>,
    pub recurrence_type_id: Option<String>,
    pub recurrence_count: Option<u32>,
    pub recurrence_current: Option<u32>,
    pub recurrence_end_date: Option<String>,
    pub recurrence_group_id: Option<String>,
    #[serde(default)]
    pub clear_recurrence: bool,
}

#[derive(Debug, Serialize)]
pub struct ExpenseResponse {
    pub id: String,
    pub owner_id: String,
    pub title: String,
    pub amount: f64,
    pub payment_method: String,
    pub payment_method_id: Option<String>,
    pub expense_date: String,
    pub description: Option<String>,
    pub status: String,
    pub bill_statement: Option<String>,
    pub bill_statement_id: Option<String>,
    pub category_id: Option<String>,
    pub paid_by: Option<String>,
    pub recurrence_type: Option<String>,
    pub recurrence_type_id: Option<String>,
    pub recurrence_count: Option<u32>,
    pub recurrence_current: Option<u32>,
    pub recurrence_end_date: Option<String>,
    pub recurrence_group_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Expense> for ExpenseResponse {
    fn from(expense: Expense) -> Self {
        ExpenseResponse {
            id: crate::models::record_key_to_string(&expense.id.key),
            owner_id: expense.owner_id,
            title: expense.title,
            amount: expense.amount,
            payment_method: expense.payment_method,
            payment_method_id: expense.payment_method_id,
            expense_date: expense.expense_date,
            description: expense.description,
            status: expense.status.to_string(),
            bill_statement: expense.bill_statement,
            bill_statement_id: expense.bill_statement_id,
            category_id: expense.category_id,
            paid_by: expense.paid_by,
            recurrence_type: expense.recurrence_type,
            recurrence_type_id: expense.recurrence_type_id,
            recurrence_count: expense.recurrence_count,
            recurrence_current: expense.recurrence_current,
            recurrence_end_date: expense.recurrence_end_date,
            recurrence_group_id: expense.recurrence_group_id,
            created_at: expense.created_at,
            updated_at: expense.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ExpenseQueryParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    pub expense_date_from: Option<String>,
    pub expense_date_to: Option<String>,
    pub payment_method: Option<String>,
    pub payment_method_id: Option<String>,
    pub paid_by: Option<String>,
    pub status: Option<String>,
    pub bill_statement_id: Option<String>,
    pub category_id: Option<String>,
    pub category: Option<String>,
    pub search: Option<String>,
    /// Logical expense bucket used by the UI navigation.
    /// Supported values: transaction, installment, subscription.
    pub expense_type: Option<String>,
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ExpenseTotals {
    pub total_count: u32,
    pub total_amount: f64,
    pub paid_amount: f64,
    pub pending_amount: f64,
    pub unpaid_amount: f64,
    pub outstanding_amount: f64,
    pub completion_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpensePaymentMethodSummary {
    pub payment_method_id: Option<String>,
    pub name: String,
    pub method_type: Option<String>,
    pub totals: ExpenseTotals,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpenseMonthSummary {
    pub bill_statement_id: String,
    pub name: String,
    pub statement_date: Option<String>,
    pub due_date: Option<String>,
    pub totals: ExpenseTotals,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpenseSummaryResponse {
    pub totals: ExpenseTotals,
    pub payment_methods: Vec<ExpensePaymentMethodSummary>,
    pub months: Vec<ExpenseMonthSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpenseNavigationMethod {
    pub payment_method_id: Option<String>,
    pub name: String,
    pub method_type: Option<String>,
    pub totals: ExpenseTotals,
    pub months: Vec<ExpenseMonthSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpenseNavigationResponse {
    pub methods: Vec<ExpenseNavigationMethod>,
}

fn default_page() -> u32 {
    1
}
fn default_page_size() -> u32 {
    10
}
fn default_sort_by() -> String {
    "expense_date".to_string()
}
fn default_sort_order() -> String {
    "desc".to_string()
}

#[derive(Debug, Serialize)]
pub struct PaginatedExpensesResponse {
    pub data: Vec<ExpenseResponse>,
    pub pagination: ExpensePaginationMeta,
}

#[derive(Debug, Serialize)]
pub struct ExpensePaginationMeta {
    pub page: u32,
    pub page_size: u32,
    pub total_items: u32,
    pub total_pages: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_expense_status_from_str() {
        assert_eq!(ExpenseStatus::from_str("unpaid").unwrap(), ExpenseStatus::Unpaid);
        assert_eq!(ExpenseStatus::from_str("UNPAID").unwrap(), ExpenseStatus::Unpaid);
        assert_eq!(ExpenseStatus::from_str("pending").unwrap(), ExpenseStatus::Pending);
        assert_eq!(ExpenseStatus::from_str("paid").unwrap(), ExpenseStatus::Paid);
        assert!(ExpenseStatus::from_str("invalid").is_err());
    }

    #[test]
    fn test_expense_status_to_string() {
        assert_eq!(ExpenseStatus::Unpaid.to_string(), "unpaid");
        assert_eq!(ExpenseStatus::Pending.to_string(), "pending");
        assert_eq!(ExpenseStatus::Paid.to_string(), "paid");
    }
}
