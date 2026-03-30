use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
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

// ========== Main Entity ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurrenceType {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    pub name: String, // e.g., "None", "Installment", "Subscription", "Recurring"
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ========== Request DTOs ==========

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

// ========== Response DTOs ==========

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
            id: rt.id,
            name: rt.name,
            description: rt.description,
            is_active: rt.is_active,
            created_at: rt.created_at.to_rfc3339(),
            updated_at: rt.updated_at.to_rfc3339(),
        }
    }
}
