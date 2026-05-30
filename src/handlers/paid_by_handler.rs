use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::db::Database;
use crate::errors::AppResult;
use crate::models::{ApiResponse, CreatePaidByRequest, PaidByResponse, UpdatePaidByRequest};
use crate::repositories::PaidByRepository;
use crate::services::PaidByService;

#[derive(Clone)]
pub struct PaidByHandler {
    service: PaidByService,
}

impl PaidByHandler {
    pub fn new(db: Database) -> Self {
        let repository = PaidByRepository::new(db);
        let service = PaidByService::new(repository);
        Self { service }
    }

    pub async fn create(
        State(handler): State<Self>,
        Json(request): Json<CreatePaidByRequest>,
    ) -> AppResult<impl IntoResponse> {
        let pb = handler.service.create(request).await?;
        Ok((
            StatusCode::CREATED,
            ApiResponse::success(PaidByResponse::from(pb), "Paid by created successfully"),
        ))
    }

    pub async fn get_by_id(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let pb = handler.service.get_by_id(&id).await?;
        Ok(ApiResponse::success(
            PaidByResponse::from(pb),
            "Paid by retrieved successfully",
        ))
    }

    pub async fn get_all(State(handler): State<Self>) -> AppResult<impl IntoResponse> {
        let items = handler.service.get_all().await?;
        Ok(ApiResponse::success(
            items,
            "Paid by items retrieved successfully",
        ))
    }

    pub async fn update(
        State(handler): State<Self>,
        Path(id): Path<String>,
        Json(request): Json<UpdatePaidByRequest>,
    ) -> AppResult<impl IntoResponse> {
        let pb = handler.service.update(&id, request).await?;
        Ok(ApiResponse::success(
            PaidByResponse::from(pb),
            "Paid by updated successfully",
        ))
    }

    pub async fn delete(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        handler.service.delete(&id).await?;
        Ok(ApiResponse::<()>::success_msg(
            "Paid by deleted successfully",
        ))
    }
}
