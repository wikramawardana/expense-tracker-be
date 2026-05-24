use serde::{Deserialize, Deserializer, Serialize};
use surrealdb::types::RecordId;
use surrealdb::types::SurrealValue;
use validator::Validate;

#[derive(Debug, Clone, PartialEq)]
pub enum NullableUpdate<T> {
    Unset,
    Null,
    Value(T),
}

impl<T> Default for NullableUpdate<T> {
    fn default() -> Self {
        Self::Unset
    }
}

impl<'de, T> Deserialize<'de> for NullableUpdate<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<T>::deserialize(deserializer).map(|value| match value {
            Some(value) => Self::Value(value),
            None => Self::Null,
        })
    }
}

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
    #[serde(default)]
    pub payment_method_id: NullableUpdate<String>,
    #[serde(default)]
    pub statement_date: NullableUpdate<String>,
    #[serde(default)]
    pub due_date: NullableUpdate<String>,
    #[serde(default)]
    pub description: NullableUpdate<String>,
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

#[cfg(test)]
mod tests {
    use super::{NullableUpdate, UpdateBillStatementRequest};

    #[test]
    fn update_request_distinguishes_missing_null_and_value() {
        let missing: UpdateBillStatementRequest = serde_json::from_value(serde_json::json!({}))
            .expect("missing nullable fields should deserialize");
        assert_eq!(missing.description, NullableUpdate::Unset);

        let null: UpdateBillStatementRequest =
            serde_json::from_value(serde_json::json!({"description": null}))
                .expect("null nullable field should deserialize");
        assert_eq!(null.description, NullableUpdate::Null);

        let value: UpdateBillStatementRequest =
            serde_json::from_value(serde_json::json!({"description": "manual"}))
                .expect("string nullable field should deserialize");
        assert_eq!(
            value.description,
            NullableUpdate::Value("manual".to_string())
        );
    }
}
