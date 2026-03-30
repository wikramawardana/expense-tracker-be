/// Extract the ID from a SurrealDB record ID string like "table:`uuid`" or "table:key".
pub fn extract_id(id: &str) -> String {
    id.split(':')
        .last()
        .unwrap_or(id)
        .trim_matches('`')
        .to_string()
}

pub mod response;
pub use response::ApiResponse;

pub mod expense;
pub use expense::{
    CreateExpenseRequest, Expense, ExpensePaginationMeta, ExpenseQueryParams, ExpenseResponse,
    ExpenseStatus, PaginatedExpensesResponse, UpdateExpenseRequest,
};

pub mod payment_method;
pub use payment_method::{
    CreatePaymentMethodRequest, PaymentMethod, PaymentMethodResponse, UpdatePaymentMethodRequest,
};

pub mod category;
pub use category::{Category, CategoryResponse, CreateCategoryRequest, UpdateCategoryRequest};

pub mod bill_statement;
pub use bill_statement::{
    BillStatement, BillStatementResponse, CreateBillStatementRequest, UpdateBillStatementRequest,
};

pub mod recurrence_type;
pub use recurrence_type::{
    CreateRecurrenceTypeRequest, RecurrenceType, RecurrenceTypeResponse,
    UpdateRecurrenceTypeRequest,
};
