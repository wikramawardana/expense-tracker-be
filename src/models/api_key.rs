use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

/// API Key row as stored in Postgres.
#[derive(Debug, Clone, sqlx::FromRow)]
#[allow(dead_code)]
pub struct ApiKey {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Request body for minting a new API key.
#[derive(Debug, Deserialize, Validate)]
pub struct CreateApiKeyRequest {
    #[validate(length(
        min = 1,
        max = 64,
        message = "Name must be between 1 and 64 characters"
    ))]
    pub name: String,
}

/// Metadata returned when listing keys (never includes the plaintext).
#[derive(Debug, Serialize)]
pub struct ApiKeyListItem {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl From<ApiKey> for ApiKeyListItem {
    fn from(k: ApiKey) -> Self {
        Self {
            id: k.id,
            name: k.name,
            key_prefix: k.key_prefix,
            created_at: k.created_at,
            last_used_at: k.last_used_at,
            revoked_at: k.revoked_at,
        }
    }
}

/// Response body when a new key is minted. The plaintext `key` is ONLY returned
/// on creation and cannot be retrieved again.
#[derive(Debug, Serialize)]
pub struct CreatedApiKeyResponse {
    pub id: String,
    pub name: String,
    pub key_prefix: String,
    pub key: String,
    pub created_at: DateTime<Utc>,
}
