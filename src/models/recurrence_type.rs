use serde::{Deserialize, Serialize};
use surrealdb::types::RecordId;
use surrealdb::types::SurrealValue;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct RecurrenceType {
    pub id: RecordId,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRecurrenceTypeRequest {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRecurrenceTypeRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct RecurrenceTypeResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<RecurrenceType> for RecurrenceTypeResponse {
    fn from(rt: RecurrenceType) -> Self {
        RecurrenceTypeResponse {
            id: crate::models::record_key_to_string(&rt.id.key),
            name: rt.name,
            description: rt.description,
            is_active: rt.is_active,
            created_at: rt.created_at,
            updated_at: rt.updated_at,
        }
    }
}
