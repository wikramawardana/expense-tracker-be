use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::str::FromStr;
use surrealdb::sql::{Id, Thing};
use validator::Validate;

// Helper to deserialize SurrealDB Thing ID to String
fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let t = Thing::deserialize(deserializer)?;
    match t.id {
        Id::String(s) => Ok(s),
        _ => Ok(t.id.to_string()),
    }
}

// Helper to deserialize Decimal from various number formats (integer, float, string)
fn deserialize_decimal<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Decimal::from(i))
            } else if let Some(u) = n.as_u64() {
                Ok(Decimal::from(u))
            } else if let Some(f) = n.as_f64() {
                Decimal::from_str(&f.to_string())
                    .map_err(|e| serde::de::Error::custom(format!("Invalid decimal: {}", e)))
            } else {
                Err(serde::de::Error::custom("Invalid number format"))
            }
        }
        Value::String(s) => Decimal::from_str(&s)
            .map_err(|e| serde::de::Error::custom(format!("Invalid decimal string: {}", e))),
        _ => Err(serde::de::Error::custom(
            "Expected a number or string for Decimal",
        )),
    }
}

// Helper to deserialize Option<Decimal> from various number formats
fn deserialize_option_decimal<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                Ok(Some(Decimal::from(i)))
            } else if let Some(u) = n.as_u64() {
                Ok(Some(Decimal::from(u)))
            } else if let Some(f) = n.as_f64() {
                Decimal::from_str(&f.to_string())
                    .map(Some)
                    .map_err(|e| serde::de::Error::custom(format!("Invalid decimal: {}", e)))
            } else {
                Err(serde::de::Error::custom("Invalid number format"))
            }
        }
        Some(Value::String(s)) => Decimal::from_str(&s)
            .map(Some)
            .map_err(|e| serde::de::Error::custom(format!("Invalid decimal string: {}", e))),
        _ => Err(serde::de::Error::custom(
            "Expected a number or string for Decimal",
        )),
    }
}

// ========== Expense Status Enum ==========

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
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

// ========== Main Entity ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expense {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    pub title: String,
    pub amount: Decimal,
    pub payment_method: String,
    pub payment_method_id: Option<String>, // Link to PaymentMethod entity
    pub expense_date: DateTime<Utc>,
    pub description: Option<String>,
    pub status: ExpenseStatus,
    pub bill_statement: Option<String>,
    pub bill_statement_id: Option<String>, // Link to BillStatement entity
    pub category_id: Option<String>,       // Link to Category entity
    pub paid_by: Option<String>,
    // Recurrence tracking fields
    pub recurrence_type: Option<String>, // e.g., "none", "installment", "subscription", "recurring"
    pub recurrence_type_id: Option<String>, // Link to RecurrenceType entity
    pub recurrence_count: Option<u32>,
    pub recurrence_current: Option<u32>,
    pub recurrence_total_amount: Option<Decimal>,
    pub recurrence_end_date: Option<DateTime<Utc>>,
    pub recurrence_group_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ========== Request DTOs ==========

#[derive(Debug, Deserialize, Validate)]
pub struct CreateExpenseRequest {
    #[validate(length(min = 1, message = "Title cannot be empty"))]
    pub title: String,
    #[serde(deserialize_with = "deserialize_decimal")]
    pub amount: Decimal,
    pub payment_method: Option<String>, // Optional: auto-filled from payment_method_id if not provided
    pub payment_method_id: Option<String>, // Link to PaymentMethod
    pub expense_date: DateTime<Utc>,
    pub description: Option<String>,
    pub bill_statement: Option<String>,
    pub bill_statement_id: Option<String>, // Link to BillStatement
    pub category_id: Option<String>,       // Link to Category
    pub paid_by: Option<String>,
    // Recurrence fields
    pub recurrence_type: Option<String>, // Optional: auto-filled from recurrence_type_id if not provided
    pub recurrence_type_id: Option<String>, // Link to RecurrenceType
    pub recurrence_count: Option<u32>,
    pub recurrence_current: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_option_decimal")]
    pub recurrence_total_amount: Option<Decimal>,
    pub recurrence_end_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateExpenseRequest {
    pub title: Option<String>,
    #[serde(default, deserialize_with = "deserialize_option_decimal")]
    pub amount: Option<Decimal>,
    pub payment_method: Option<String>,
    pub payment_method_id: Option<String>,
    pub expense_date: Option<DateTime<Utc>>,
    pub description: Option<String>,
    pub status: Option<ExpenseStatus>,
    pub bill_statement: Option<String>,
    pub bill_statement_id: Option<String>,
    pub category_id: Option<String>,
    pub paid_by: Option<String>,
    // Recurrence fields
    pub recurrence_type: Option<String>,
    pub recurrence_type_id: Option<String>,
    pub recurrence_count: Option<u32>,
    pub recurrence_current: Option<u32>,
    #[serde(default, deserialize_with = "deserialize_option_decimal")]
    pub recurrence_total_amount: Option<Decimal>,
    pub recurrence_end_date: Option<DateTime<Utc>>,
    pub recurrence_group_id: Option<String>,
    /// Set to true to convert from recurring to one-time (clears all recurrence fields and deletes future expenses)
    #[serde(default)]
    pub clear_recurrence: bool,
}

// ========== Response DTOs ==========

#[derive(Debug, Serialize)]
pub struct ExpenseResponse {
    pub id: String,
    pub title: String,
    pub amount: Decimal,
    pub payment_method: String,
    pub payment_method_id: Option<String>,
    pub expense_date: String,
    pub description: Option<String>,
    pub status: String,
    pub bill_statement: Option<String>,
    pub bill_statement_id: Option<String>,
    pub category_id: Option<String>,
    pub paid_by: Option<String>,
    // Recurrence fields
    pub recurrence_type: Option<String>,
    pub recurrence_type_id: Option<String>,
    pub recurrence_count: Option<u32>,
    pub recurrence_current: Option<u32>,
    pub recurrence_total_amount: Option<Decimal>,
    pub recurrence_end_date: Option<String>,
    pub recurrence_group_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Expense> for ExpenseResponse {
    fn from(expense: Expense) -> Self {
        ExpenseResponse {
            id: expense.id,
            title: expense.title,
            amount: expense.amount,
            payment_method: expense.payment_method,
            payment_method_id: expense.payment_method_id,
            expense_date: expense.expense_date.to_rfc3339(),
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
            recurrence_total_amount: expense.recurrence_total_amount,
            recurrence_end_date: expense.recurrence_end_date.map(|d| d.to_rfc3339()),
            recurrence_group_id: expense.recurrence_group_id,
            created_at: expense.created_at.to_rfc3339(),
            updated_at: expense.updated_at.to_rfc3339(),
        }
    }
}

// ========== Query Params ==========

#[derive(Debug, Deserialize)]
pub struct ExpenseQueryParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    pub expense_date_from: Option<DateTime<Utc>>,
    pub expense_date_to: Option<DateTime<Utc>>,
    pub payment_method: Option<String>,
    pub paid_by: Option<String>,
    pub status: Option<String>,
    pub bill_statement_id: Option<String>,
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
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

// ========== Pagination Response ==========

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
