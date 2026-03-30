use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::models::ApiResponse;

// ========== Auth User (the authenticated user) ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub image: Option<String>,
    pub role: Option<String>,
}

// ========== Database Models for Auth ==========

/// Session data from the session table
#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Session {
    pub id: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    pub token: String,
    #[sqlx(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,
}

/// User data from the user table
#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct User {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub image: Option<String>,
    pub role: Option<String>,
}

/// Combined session and user data for optimized single-query lookup
#[derive(Debug, sqlx::FromRow)]
#[allow(dead_code)]
pub struct SessionWithUser {
    // Session fields
    pub session_id: String,
    pub user_id: String,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    // User fields
    pub u_id: String,
    pub email: String,
    pub name: Option<String>,
    pub image: Option<String>,
    pub role: Option<String>,
}

// ========== Auth State ==========

/// Auth state that holds the PostgreSQL connection pool for session verification
#[derive(Clone)]
pub struct AuthState {
    pub pg_pool: PgPool,
}

impl AuthState {
    pub fn new(pg_pool: PgPool) -> Self {
        Self { pg_pool }
    }
}

// ========== Auth Errors ==========

#[derive(Debug)]
#[allow(dead_code)]
pub enum AuthError {
    MissingToken,
    InvalidToken,
    ExpiredSession,
    UserNotFound,
    Forbidden,
    DatabaseError(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingToken => (
                StatusCode::UNAUTHORIZED,
                "Missing authorization header. Please provide a Bearer token.",
            ),
            AuthError::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "Invalid or malformed authorization token.",
            ),
            AuthError::ExpiredSession => (
                StatusCode::UNAUTHORIZED,
                "Session has expired. Please login again.",
            ),
            AuthError::UserNotFound => (StatusCode::UNAUTHORIZED, "User not found."),
            AuthError::Forbidden => (
                StatusCode::FORBIDDEN,
                "You don't have permission to access this resource.",
            ),
            AuthError::DatabaseError(ref _e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Authentication service unavailable.",
            ),
        };

        let body = ApiResponse::<()>::error(message);
        (status, body).into_response()
    }
}

// ========== Helper Functions ==========

/// Extracts the Bearer token from the Authorization header
fn extract_bearer_token(auth_header: &str) -> Option<&str> {
    if auth_header.starts_with("Bearer ") {
        Some(&auth_header[7..])
    } else {
        None
    }
}

// ========== Auth Middleware ==========

/// Middleware function to authenticate requests using session tokens
pub async fn auth_middleware(
    State(auth_state): State<AuthState>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, AuthError> {
    // Extract the Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AuthError::MissingToken)?;

    // Extract the Bearer token
    let token = extract_bearer_token(auth_header).ok_or(AuthError::InvalidToken)?;

    // Single query with JOIN to get session + user in one round-trip
    let result: Option<SessionWithUser> = sqlx::query_as(
        r#"
        SELECT 
            s.id AS session_id, s."userId" AS user_id, s.token, s."expiresAt" AS expires_at,
            u.id AS u_id, u.email, u.name, u.image, u.role
        FROM session s
        INNER JOIN "user" u ON s."userId" = u.id
        WHERE s.token = $1
        "#,
    )
    .bind(token)
    .fetch_optional(&auth_state.pg_pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error while validating session: {:?}", e);
        AuthError::DatabaseError(e.to_string())
    })?;

    let session_user = result.ok_or(AuthError::InvalidToken)?;

    // Check if the session has expired
    if session_user.expires_at < Utc::now() {
        return Err(AuthError::ExpiredSession);
    }

    // Create AuthUser and insert into request extensions
    let auth_user = AuthUser {
        id: session_user.u_id,
        email: session_user.email,
        name: session_user.name,
        image: session_user.image,
        role: session_user.role,
    };

    request.extensions_mut().insert(auth_user);

    // Continue to the next handler
    Ok(next.run(request).await)
}

// ========== Role-Based Authorization Middleware ==========

/// Creates a middleware that checks if the user has one of the required roles
#[allow(dead_code)]
pub fn require_roles(
    allowed_roles: Vec<&'static str>,
) -> impl Fn(Request<Body>) -> std::future::Ready<Result<Request<Body>, AuthError>> + Clone {
    move |request: Request<Body>| {
        let allowed = allowed_roles.clone();
        std::future::ready(check_role(request, &allowed))
    }
}

#[allow(dead_code)]
fn check_role(request: Request<Body>, allowed_roles: &[&str]) -> Result<Request<Body>, AuthError> {
    let auth_user = request
        .extensions()
        .get::<AuthUser>()
        .ok_or(AuthError::MissingToken)?;

    let user_role = auth_user.role.as_deref().unwrap_or("user");

    if allowed_roles.contains(&user_role) {
        Ok(request)
    } else {
        Err(AuthError::Forbidden)
    }
}

// ========== CurrentUser Extractor ==========

/// Extractor to get the authenticated user in handlers
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CurrentUser(pub AuthUser);

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .map(CurrentUser)
            .ok_or(AuthError::MissingToken)
    }
}

// ========== Optional Auth Middleware (for public/private hybrid routes) ==========

/// Middleware that attempts authentication but doesn't fail if no token is provided
/// Use this for routes that work for both authenticated and anonymous users
#[allow(dead_code)]
pub async fn optional_auth_middleware(
    State(auth_state): State<AuthState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    // Try to extract the Authorization header
    if let Some(auth_header) = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        // Try to extract and validate the token
        if let Some(token) = extract_bearer_token(auth_header) {
            // Single query with JOIN to get session + user in one round-trip
            if let Ok(Some(session_user)) = sqlx::query_as::<_, SessionWithUser>(
                r#"
                SELECT 
                    s.id AS session_id, s."userId" AS user_id, s.token, s."expiresAt" AS expires_at,
                    u.id AS u_id, u.email, u.name, u.image, u.role
                FROM session s
                INNER JOIN "user" u ON s."userId" = u.id
                WHERE s.token = $1
                "#,
            )
            .bind(token)
            .fetch_optional(&auth_state.pg_pool)
            .await
            {
                if session_user.expires_at >= Utc::now() {
                    let auth_user = AuthUser {
                        id: session_user.u_id,
                        email: session_user.email,
                        name: session_user.name,
                        image: session_user.image,
                        role: session_user.role,
                    };
                    request.extensions_mut().insert(auth_user);
                }
            }
        }
    }

    // Continue regardless of authentication status
    next.run(request).await
}
