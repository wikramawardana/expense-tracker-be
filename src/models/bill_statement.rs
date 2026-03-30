use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use surrealdb::sql::{Id, Thing};
use validator::Validate;

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
pub struct BillStatement {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    pub name: String,                      // e.g., "January 2026", "BCA CC Feb 2026"
    pub payment_method_id: Option<String>, // Link to PaymentMethod
    pub statement_date: Option<DateTime<Utc>>, // Optional statement date
    pub due_date: Option<DateTime<Utc>>,   // Payment due date
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ========== Request DTOs ==========

#[derive(Debug, Deserialize, Validate)]
pub struct CreateBillStatementRequest {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String, // e.g., "January 2026", "BCA CC Feb 2026"
    pub payment_method_id: Option<String>,
    pub statement_date: Option<DateTime<Utc>>, // Optional - just use name as label
    pub due_date: Option<DateTime<Utc>>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateBillStatementRequest {
    pub name: Option<String>,
    pub payment_method_id: Option<String>,
    pub statement_date: Option<DateTime<Utc>>,
    pub due_date: Option<DateTime<Utc>>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

// ========== Response DTOs ==========

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
            id: bs.id,
            name: bs.name,
            payment_method_id: bs.payment_method_id,
            statement_date: bs.statement_date.map(|d| d.to_rfc3339()),
            due_date: bs.due_date.map(|d| d.to_rfc3339()),
            description: bs.description,
            is_active: bs.is_active,
            created_at: bs.created_at.to_rfc3339(),
            updated_at: bs.updated_at.to_rfc3339(),
        }
    }
}
