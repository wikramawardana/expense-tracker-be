use serde::{Deserialize, Serialize};
use surrealdb::types::RecordId;
use surrealdb::types::SurrealValue;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct BillStatement {
    pub id: RecordId,
    pub name: String,
    pub payment_method_id: Option<String>,
    pub statement_date: Option<String>,
    pub due_date: Option<String>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateBillStatementRequest {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,
    pub payment_method_id: Option<String>,
    pub statement_date: Option<String>,
    pub due_date: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateBillStatementRequest {
    pub name: Option<String>,
    pub payment_method_id: Option<String>,
    pub statement_date: Option<String>,
    pub due_date: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct BillStatementResponse {
    pub id: String,
    pub name: String,
    pub payment_method_id: Option<String>,
    pub statement_date: Option<String>,
    pub due_date: Option<String>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<BillStatement> for BillStatementResponse {
    fn from(bs: BillStatement) -> Self {
        BillStatementResponse {
            id: crate::models::record_key_to_string(&bs.id.key),
            name: bs.name,
            payment_method_id: bs.payment_method_id,
            statement_date: bs.statement_date,
            due_date: bs.due_date,
            description: bs.description,
            is_active: bs.is_active,
            created_at: bs.created_at,
            updated_at: bs.updated_at,
        }
    }
}
