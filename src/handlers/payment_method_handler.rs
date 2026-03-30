use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::db::Database;
use crate::errors::AppResult;
use crate::models::{
    ApiResponse, CreatePaymentMethodRequest, PaymentMethodResponse, UpdatePaymentMethodRequest,
};
use crate::repositories::PaymentMethodRepository;
use crate::services::PaymentMethodService;

#[derive(Clone)]
pub struct PaymentMethodHandler {
    service: PaymentMethodService,
}

impl PaymentMethodHandler {
    pub fn new(db: Database) -> Self {
        let repository = PaymentMethodRepository::new(db);
        let service = PaymentMethodService::new(repository);
        Self { service }
    }

    pub async fn create(
        State(handler): State<Self>,
        Json(request): Json<CreatePaymentMethodRequest>,
    ) -> AppResult<impl IntoResponse> {
        let pm = handler.service.create(request).await?;
        Ok((
            StatusCode::CREATED,
            ApiResponse::success(
                PaymentMethodResponse::from(pm),
                "Payment method created successfully",
            ),
        ))
    }

    pub async fn get_by_id(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let pm = handler.service.get_by_id(&id).await?;
        Ok(ApiResponse::success(
            PaymentMethodResponse::from(pm),
            "Payment method retrieved successfully",
        ))
    }

    pub async fn get_all(State(handler): State<Self>) -> AppResult<impl IntoResponse> {
        let items = handler.service.get_all().await?;
        Ok(ApiResponse::success(
            items,
            "Payment methods retrieved successfully",
        ))
    }

    pub async fn update(
        State(handler): State<Self>,
        Path(id): Path<String>,
        Json(request): Json<UpdatePaymentMethodRequest>,
    ) -> AppResult<impl IntoResponse> {
        let pm = handler.service.update(&id, request).await?;
        Ok(ApiResponse::success(
            PaymentMethodResponse::from(pm),
            "Payment method updated successfully",
        ))
    }

    pub async fn delete(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        handler.service.delete(&id).await?;
        Ok(ApiResponse::<()>::success_msg(
            "Payment method deleted successfully",
        ))
    }
}
