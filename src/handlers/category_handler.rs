use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::db::Database;
use crate::errors::AppResult;
use crate::models::{ApiResponse, CategoryResponse, CreateCategoryRequest, UpdateCategoryRequest};
use crate::repositories::CategoryRepository;
use crate::services::CategoryService;

#[derive(Clone)]
pub struct CategoryHandler {
    service: CategoryService,
}

impl CategoryHandler {
    pub fn new(db: Database) -> Self {
        let repository = CategoryRepository::new(db);
        let service = CategoryService::new(repository);
        Self { service }
    }

    pub async fn create(
        State(handler): State<Self>,
        Json(request): Json<CreateCategoryRequest>,
    ) -> AppResult<impl IntoResponse> {
        let category = handler.service.create(request).await?;
        Ok((
            StatusCode::CREATED,
            ApiResponse::success(
                CategoryResponse::from(category),
                "Category created successfully",
            ),
        ))
    }

    pub async fn get_by_id(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let category = handler.service.get_by_id(&id).await?;
        Ok(ApiResponse::success(
            CategoryResponse::from(category),
            "Category retrieved successfully",
        ))
    }

    pub async fn get_all(State(handler): State<Self>) -> AppResult<impl IntoResponse> {
        let items = handler.service.get_all().await?;
        Ok(ApiResponse::success(
            items,
            "Categories retrieved successfully",
        ))
    }

    pub async fn update(
        State(handler): State<Self>,
        Path(id): Path<String>,
        Json(request): Json<UpdateCategoryRequest>,
    ) -> AppResult<impl IntoResponse> {
        let category = handler.service.update(&id, request).await?;
        Ok(ApiResponse::success(
            CategoryResponse::from(category),
            "Category updated successfully",
        ))
    }

    pub async fn delete(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        handler.service.delete(&id).await?;
        Ok(ApiResponse::<()>::success_msg(
            "Category deleted successfully",
        ))
    }
}
