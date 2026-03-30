use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::db::Database;
use crate::errors::AppResult;
use crate::models::{
    ApiResponse, BillStatementResponse, CreateBillStatementRequest, UpdateBillStatementRequest,
};
use crate::repositories::BillStatementRepository;
use crate::services::BillStatementService;

#[derive(Clone)]
pub struct BillStatementHandler {
    service: BillStatementService,
}

impl BillStatementHandler {
    pub fn new(db: Database) -> Self {
        let repository = BillStatementRepository::new(db);
        let service = BillStatementService::new(repository);
        Self { service }
    }

    pub async fn create(
        State(handler): State<Self>,
        Json(request): Json<CreateBillStatementRequest>,
    ) -> AppResult<impl IntoResponse> {
        let bs = handler.service.create(request).await?;
        Ok((
            StatusCode::CREATED,
            ApiResponse::success(
                BillStatementResponse::from(bs),
                "Bill statement created successfully",
            ),
        ))
    }

    pub async fn get_by_id(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let bs = handler.service.get_by_id(&id).await?;
        Ok(ApiResponse::success(
            BillStatementResponse::from(bs),
            "Bill statement retrieved successfully",
        ))
    }

    pub async fn get_all(State(handler): State<Self>) -> AppResult<impl IntoResponse> {
        let items = handler.service.get_all().await?;
        Ok(ApiResponse::success(
            items,
            "Bill statements retrieved successfully",
        ))
    }

    pub async fn update(
        State(handler): State<Self>,
        Path(id): Path<String>,
        Json(request): Json<UpdateBillStatementRequest>,
    ) -> AppResult<impl IntoResponse> {
        let bs = handler.service.update(&id, request).await?;
        Ok(ApiResponse::success(
            BillStatementResponse::from(bs),
            "Bill statement updated successfully",
        ))
    }

    pub async fn delete(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        handler.service.delete(&id).await?;
        Ok(ApiResponse::<()>::success_msg(
            "Bill statement deleted successfully",
        ))
    }
}
