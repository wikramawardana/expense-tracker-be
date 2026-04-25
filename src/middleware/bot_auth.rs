use axum::{
    body::Body,
    extract::State,
    http::{header, Request},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use sqlx::PgPool;

use crate::middleware::{AuthError, AuthUser};
use crate::services::hash_key;

#[derive(sqlx::FromRow)]
struct ApiKeyRow {
    id: String,
    user_id: String,
}

/// State for the bot-auth middleware. Wraps the same Postgres pool used by
/// the session-auth middleware since API keys live alongside sessions in PG.
#[derive(Clone)]
pub struct BotAuthState {
    pub pg_pool: PgPool,
}

impl BotAuthState {
    pub fn new(pg_pool: PgPool) -> Self {
        Self { pg_pool }
    }
}

/// Authenticates a request using an API key passed as `Authorization: Bearer etk_...`.
///
/// On success, injects the owning `AuthUser` into request extensions (same shape
/// used by the session middleware, so downstream handlers don't need to care
/// which flavor of auth was used).
pub async fn bot_auth_middleware(
    State(state): State<BotAuthState>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, AuthError> {
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AuthError::MissingToken)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AuthError::InvalidToken)?;

    // Only treat etk_* tokens as bot keys. Anything else falls through as
    // invalid so misrouted session tokens don't accidentally match.
    if !token.starts_with("etk_") {
        return Err(AuthError::InvalidToken);
    }

    let key_hash = hash_key(token);

    // Validate the key. We don't join with `user` here because current expense
    // handlers don't need user profile fields — the key existing + not being
    // revoked is sufficient proof of authorization.
    let row: Option<ApiKeyRow> = sqlx::query_as(
        r#"
            SELECT id, user_id
            FROM api_key
            WHERE key_hash = $1 AND revoked_at IS NULL
            "#,
    )
    .bind(&key_hash)
    .fetch_optional(&state.pg_pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error while validating API key: {:?}", e);
        AuthError::DatabaseError(e.to_string())
    })?;

    let ApiKeyRow {
        id: key_id,
        user_id,
    } = row.ok_or(AuthError::InvalidToken)?;

    // Best-effort last_used_at update. Failure here must not block the request.
    if let Err(e) = sqlx::query("UPDATE api_key SET last_used_at = $2 WHERE id = $1")
        .bind(&key_id)
        .bind(Utc::now())
        .execute(&state.pg_pool)
        .await
    {
        tracing::warn!("Failed to update api_key.last_used_at: {:?}", e);
    }

    let auth_user = AuthUser {
        id: user_id,
        email: String::new(),
        name: None,
        image: None,
        role: None,
    };
    request.extensions_mut().insert(auth_user);

    Ok(next.run(request).await)
}
