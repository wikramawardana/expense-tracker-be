use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;
use validator::Validate;

use crate::errors::{AppError, AppResult};
use crate::models::{ApiKey, ApiKeyListItem, CreateApiKeyRequest, CreatedApiKeyResponse};

/// Plaintext keys are formatted as `etk_<32 hex chars>` (uuid v4, dashes stripped).
const KEY_PREFIX: &str = "etk_";
const KEY_DISPLAY_PREFIX_LEN: usize = 12;

pub fn hash_key(plaintext: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(plaintext.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Clone)]
pub struct ApiKeyService {
    pool: PgPool,
}

impl ApiKeyService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Ensure the api_key table exists. Called once on startup.
    pub async fn init_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS api_key (
                id            TEXT PRIMARY KEY,
                user_id       TEXT NOT NULL,
                name          TEXT NOT NULL,
                key_hash      TEXT NOT NULL UNIQUE,
                key_prefix    TEXT NOT NULL,
                created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                last_used_at  TIMESTAMPTZ,
                revoked_at    TIMESTAMPTZ
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS api_key_user_id_idx ON api_key(user_id)")
            .execute(pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS api_key_key_hash_idx ON api_key(key_hash)")
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Mints a new API key for the given user. Returns the plaintext key exactly once.
    pub async fn create(
        &self,
        user_id: &str,
        request: CreateApiKeyRequest,
    ) -> AppResult<CreatedApiKeyResponse> {
        request
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let id = Uuid::new_v4().to_string();
        let raw = Uuid::new_v4().simple().to_string();
        let key = format!("{}{}", KEY_PREFIX, raw);
        let key_hash = hash_key(&key);
        let key_prefix: String = key.chars().take(KEY_DISPLAY_PREFIX_LEN).collect();

        let row: (chrono::DateTime<chrono::Utc>,) = sqlx::query_as(
            r#"
            INSERT INTO api_key (id, user_id, name, key_hash, key_prefix)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING created_at
            "#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(&request.name)
        .bind(&key_hash)
        .bind(&key_prefix)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create API key: {e}")))?;

        Ok(CreatedApiKeyResponse {
            id,
            name: request.name,
            key_prefix,
            key,
            created_at: row.0,
        })
    }

    pub async fn list_for_user(&self, user_id: &str) -> AppResult<Vec<ApiKeyListItem>> {
        let rows: Vec<ApiKey> = sqlx::query_as(
            r#"
            SELECT id, user_id, name, key_hash, key_prefix, created_at, last_used_at, revoked_at
            FROM api_key
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to list API keys: {e}")))?;

        Ok(rows.into_iter().map(ApiKeyListItem::from).collect())
    }

    /// Mark an API key as revoked. Only the owning user can revoke.
    pub async fn revoke(&self, user_id: &str, key_id: &str) -> AppResult<()> {
        let result = sqlx::query(
            r#"
            UPDATE api_key
            SET revoked_at = $3
            WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
            "#,
        )
        .bind(key_id)
        .bind(user_id)
        .bind(Utc::now())
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to revoke API key: {e}")))?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!(
                "API key not found or already revoked: {key_id}"
            )));
        }
        Ok(())
    }
}
