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
pub struct PaymentMethod {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    pub name: String,        // e.g., "BCA Credit Card", "GoPay", "Cash"
    pub method_type: String, // e.g., "credit_card", "debit_card", "e_wallet", "bank_transfer", "cash"
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ========== Request DTOs ==========

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePaymentMethodRequest {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,
    #[validate(length(min = 1, message = "Method type cannot be empty"))]
    pub method_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePaymentMethodRequest {
    pub name: Option<String>,
    pub method_type: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

// ========== Response DTOs ==========

#[derive(Debug, Serialize)]
pub struct PaymentMethodResponse {
    pub id: String,
    pub name: String,
    pub method_type: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<PaymentMethod> for PaymentMethodResponse {
    fn from(pm: PaymentMethod) -> Self {
        PaymentMethodResponse {
            id: pm.id,
            name: pm.name,
            method_type: pm.method_type,
            description: pm.description,
            is_active: pm.is_active,
            created_at: pm.created_at.to_rfc3339(),
            updated_at: pm.updated_at.to_rfc3339(),
        }
    }
}
