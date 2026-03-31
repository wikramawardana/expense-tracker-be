use chrono::Utc;
use surrealdb::types::RecordId;
use uuid::Uuid;
use validator::Validate;

use crate::errors::{AppError, AppResult};
use crate::models::{
    CreateRecurrenceTypeRequest, RecurrenceType, RecurrenceTypeResponse,
    UpdateRecurrenceTypeRequest,
};
use crate::repositories::RecurrenceTypeRepository;

#[derive(Clone)]
pub struct RecurrenceTypeService {
    repository: RecurrenceTypeRepository,
}

impl RecurrenceTypeService {
    pub fn new(repository: RecurrenceTypeRepository) -> Self {
        Self { repository }
    }

    pub async fn create(&self, request: CreateRecurrenceTypeRequest) -> AppResult<RecurrenceType> {
        request
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let rt = RecurrenceType {
            id: RecordId::new("recurrence_types", Uuid::new_v4().to_string()),
            name: request.name,
            description: request.description,
            is_active: true,
            created_at: now.clone(),
            updated_at: now,
        };

        self.repository.create(rt).await
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<RecurrenceType> {
        self.repository.find_by_id(id).await
    }

    pub async fn get_all(&self) -> AppResult<Vec<RecurrenceTypeResponse>> {
        let items = self.repository.find_all().await?;
        Ok(items
            .into_iter()
            .map(RecurrenceTypeResponse::from)
            .collect())
    }

    pub async fn update(
        &self,
        id: &str,
        request: UpdateRecurrenceTypeRequest,
    ) -> AppResult<RecurrenceType> {
        let mut rt = self.repository.find_by_id(id).await?;

        if let Some(name) = request.name {
            rt.name = name;
        }
        if let Some(description) = request.description {
            rt.description = Some(description);
        }
        if let Some(is_active) = request.is_active {
            rt.is_active = is_active;
        }

        rt.updated_at = Utc::now().to_rfc3339();
        self.repository.update(id, rt).await
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        self.repository.delete(id).await
    }
}
