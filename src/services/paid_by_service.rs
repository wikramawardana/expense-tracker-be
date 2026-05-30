use chrono::Utc;
use surrealdb::types::RecordId;
use uuid::Uuid;
use validator::Validate;

use crate::errors::{AppError, AppResult};
use crate::models::{CreatePaidByRequest, PaidBy, PaidByResponse, UpdatePaidByRequest};
use crate::repositories::PaidByRepository;

#[derive(Clone)]
pub struct PaidByService {
    repository: PaidByRepository,
}

impl PaidByService {
    pub fn new(repository: PaidByRepository) -> Self {
        Self { repository }
    }

    pub async fn create(&self, request: CreatePaidByRequest) -> AppResult<PaidBy> {
        request
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let pb = PaidBy {
            id: RecordId::new("paid_by", Uuid::new_v4().to_string()),
            name: request.name,
            is_active: true,
            created_at: now.clone(),
            updated_at: now,
        };

        self.repository.create(pb).await
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<PaidBy> {
        self.repository.find_by_id(id).await
    }

    pub async fn get_all(&self) -> AppResult<Vec<PaidByResponse>> {
        let items = self.repository.find_all().await?;
        Ok(items.into_iter().map(PaidByResponse::from).collect())
    }

    pub async fn update(&self, id: &str, request: UpdatePaidByRequest) -> AppResult<PaidBy> {
        let mut pb = self.repository.find_by_id(id).await?;

        if let Some(name) = request.name {
            pb.name = name;
        }
        if let Some(is_active) = request.is_active {
            pb.is_active = is_active;
        }

        pb.updated_at = Utc::now().to_rfc3339();
        self.repository.update(id, pb).await
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        self.repository.delete(id).await
    }
}
