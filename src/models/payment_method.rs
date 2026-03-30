use serde::{Deserialize, Serialize};
use surrealdb::types::SurrealValue;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct PaymentMethod {
    pub id: String,
    pub name: String,
    pub method_type: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

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
            id: crate::models::extract_id(&pm.id),
            name: pm.name,
            method_type: pm.method_type,
            description: pm.description,
            is_active: pm.is_active,
            created_at: pm.created_at,
            updated_at: pm.updated_at,
        }
    }
}
