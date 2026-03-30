use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::db::Database;
use crate::errors::AppResult;
use crate::models::{
    ApiResponse, CreateRecurrenceTypeRequest, RecurrenceTypeResponse, UpdateRecurrenceTypeRequest,
};
use crate::repositories::RecurrenceTypeRepository;
use crate::services::RecurrenceTypeService;

#[derive(Clone)]
pub struct RecurrenceTypeHandler {
    service: RecurrenceTypeService,
}

impl RecurrenceTypeHandler {
    pub fn new(db: Database) -> Self {
        let repository = RecurrenceTypeRepository::new(db);
        let service = RecurrenceTypeService::new(repository);
        Self { service }
    }

    pub async fn create(
        State(handler): State<Self>,
        Json(request): Json<CreateRecurrenceTypeRequest>,
    ) -> AppResult<impl IntoResponse> {
        let rt = handler.service.create(request).await?;
        Ok((
            StatusCode::CREATED,
            ApiResponse::success(
                RecurrenceTypeResponse::from(rt),
                "Recurrence type created successfully",
            ),
        ))
    }

    pub async fn get_by_id(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let rt = handler.service.get_by_id(&id).await?;
        Ok(ApiResponse::success(
            RecurrenceTypeResponse::from(rt),
            "Recurrence type retrieved successfully",
        ))
    }

    pub async fn get_all(State(handler): State<Self>) -> AppResult<impl IntoResponse> {
        let items = handler.service.get_all().await?;
        Ok(ApiResponse::success(
            items,
            "Recurrence types retrieved successfully",
        ))
    }

    pub async fn update(
        State(handler): State<Self>,
        Path(id): Path<String>,
        Json(request): Json<UpdateRecurrenceTypeRequest>,
    ) -> AppResult<impl IntoResponse> {
        let rt = handler.service.update(&id, request).await?;
        Ok(ApiResponse::success(
            RecurrenceTypeResponse::from(rt),
            "Recurrence type updated successfully",
        ))
    }

    pub async fn delete(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        handler.service.delete(&id).await?;
        Ok(ApiResponse::<()>::success_msg(
            "Recurrence type deleted successfully",
        ))
    }
}
