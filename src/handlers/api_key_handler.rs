use axum::{extract::Path, extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::errors::AppResult;
use crate::middleware::CurrentUser;
use crate::models::{ApiResponse, CreateApiKeyRequest};
use crate::services::ApiKeyService;

#[derive(Clone)]
pub struct ApiKeyHandler {
    service: ApiKeyService,
}

impl ApiKeyHandler {
    pub fn new(service: ApiKeyService) -> Self {
        Self { service }
    }

    /// Mint a new API key for the current user. Returns the plaintext key
    /// exactly once — it cannot be retrieved again afterwards.
    pub async fn create(
        State(handler): State<Self>,
        CurrentUser(user): CurrentUser,
        Json(request): Json<CreateApiKeyRequest>,
    ) -> AppResult<impl IntoResponse> {
        let created = handler.service.create(&user.id, request).await?;
        Ok((
            StatusCode::CREATED,
            ApiResponse::success(
                created,
                "API key created successfully. Save the key — it will not be shown again.",
            ),
        ))
    }

    /// List all API keys owned by the current user.
    pub async fn list(
        State(handler): State<Self>,
        CurrentUser(user): CurrentUser,
    ) -> AppResult<impl IntoResponse> {
        let items = handler.service.list_for_user(&user.id).await?;
        Ok(ApiResponse::success(
            items,
            "API keys retrieved successfully",
        ))
    }

    /// Revoke an API key by id (must be owned by the current user).
    pub async fn revoke(
        State(handler): State<Self>,
        CurrentUser(user): CurrentUser,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        handler.service.revoke(&user.id, &id).await?;
        Ok(ApiResponse::<()>::success_msg("API key revoked"))
    }
}
