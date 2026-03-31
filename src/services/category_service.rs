use chrono::Utc;
use surrealdb::types::RecordId;
use uuid::Uuid;
use validator::Validate;

use crate::errors::{AppError, AppResult};
use crate::models::{Category, CategoryResponse, CreateCategoryRequest, UpdateCategoryRequest};
use crate::repositories::CategoryRepository;

#[derive(Clone)]
pub struct CategoryService {
    repository: CategoryRepository,
}

impl CategoryService {
    pub fn new(repository: CategoryRepository) -> Self {
        Self { repository }
    }

    pub async fn create(&self, request: CreateCategoryRequest) -> AppResult<Category> {
        request
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let category = Category {
            id: RecordId::new("categories", Uuid::new_v4().to_string()),
            name: request.name,
            icon: request.icon,
            color: request.color,
            description: request.description,
            is_active: true,
            created_at: now.clone(),
            updated_at: now,
        };

        self.repository.create(category).await
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<Category> {
        self.repository.find_by_id(id).await
    }

    pub async fn get_all(&self) -> AppResult<Vec<CategoryResponse>> {
        let items = self.repository.find_all().await?;
        Ok(items.into_iter().map(CategoryResponse::from).collect())
    }

    pub async fn update(&self, id: &str, request: UpdateCategoryRequest) -> AppResult<Category> {
        let mut category = self.repository.find_by_id(id).await?;

        if let Some(name) = request.name {
            category.name = name;
        }
        if let Some(icon) = request.icon {
            category.icon = Some(icon);
        }
        if let Some(color) = request.color {
            category.color = Some(color);
        }
        if let Some(description) = request.description {
            category.description = Some(description);
        }
        if let Some(is_active) = request.is_active {
            category.is_active = is_active;
        }

        category.updated_at = Utc::now().to_rfc3339();
        self.repository.update(id, category).await
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        self.repository.delete(id).await
    }
}
