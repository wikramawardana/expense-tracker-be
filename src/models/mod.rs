use surrealdb::types::RecordIdKey;

pub fn record_key_to_string(key: &RecordIdKey) -> String {
    match key {
        RecordIdKey::String(s) => s.clone(),
        RecordIdKey::Number(n) => n.to_string(),
        _ => format!("{:?}", key),
    }
}

pub mod response;
pub use response::ApiResponse;

pub mod expense;
pub use expense::{
    BulkCreateExpensesResponse, BulkExpenseAction, BulkExpenseActionRequest,
    BulkExpenseActionResponse, CreateExpenseRequest, CreateExpensesBulkRequest, Expense,
    ExpensePaginationMeta, ExpenseQueryParams, ExpenseResponse, ExpenseStatus,
    PaginatedExpensesResponse, UpdateExpenseRequest,
};

pub mod payment_method;
pub use payment_method::{
    CreatePaymentMethodRequest, PaymentMethod, PaymentMethodResponse, UpdatePaymentMethodRequest,
};

pub mod category;
pub use category::{Category, CategoryResponse, CreateCategoryRequest, UpdateCategoryRequest};

pub mod bill_statement;
pub use bill_statement::{
    BillStatement, BillStatementResponse, CreateBillStatementRequest, NullableUpdate,
    UpdateBillStatementRequest,
};

pub mod api_key;
pub use api_key::{ApiKey, ApiKeyListItem, CreateApiKeyRequest, CreatedApiKeyResponse};

pub mod paid_by;
pub use paid_by::{CreatePaidByRequest, PaidBy, PaidByResponse, UpdatePaidByRequest};
