use serde::{Deserialize, Serialize};
use surrealdb::types::RecordId;
use surrealdb::types::SurrealValue;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct PaidBy {
    pub id: RecordId,
    pub name: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePaidByRequest {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePaidByRequest {
    pub name: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PaidByResponse {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<PaidBy> for PaidByResponse {
    fn from(pb: PaidBy) -> Self {
        PaidByResponse {
            id: crate::models::record_key_to_string(&pb.id.key),
            name: pb.name,
            is_active: pb.is_active,
            created_at: pb.created_at,
            updated_at: pb.updated_at,
        }
    }
}
