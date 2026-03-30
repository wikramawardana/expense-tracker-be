use chrono::Utc;
use uuid::Uuid;
use validator::Validate;

use crate::errors::{AppError, AppResult};
use crate::models::{
    BillStatement, BillStatementResponse, CreateBillStatementRequest, UpdateBillStatementRequest,
};
use crate::repositories::BillStatementRepository;

#[derive(Clone)]
pub struct BillStatementService {
    repository: BillStatementRepository,
}

impl BillStatementService {
    pub fn new(repository: BillStatementRepository) -> Self {
        Self { repository }
    }

    pub async fn create(&self, request: CreateBillStatementRequest) -> AppResult<BillStatement> {
        request
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let bs = BillStatement {
            id: Uuid::new_v4().to_string(),
            name: request.name,
            payment_method_id: request.payment_method_id,
            statement_date: request.statement_date,
            due_date: request.due_date,
            description: request.description,
            is_active: true,
            created_at: now.clone(),
            updated_at: now,
        };

        self.repository.create(bs).await
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<BillStatement> {
        self.repository.find_by_id(id).await
    }

    pub async fn get_all(&self) -> AppResult<Vec<BillStatementResponse>> {
        let items = self.repository.find_all().await?;
        Ok(items.into_iter().map(BillStatementResponse::from).collect())
    }

    pub async fn update(
        &self,
        id: &str,
        request: UpdateBillStatementRequest,
    ) -> AppResult<BillStatement> {
        let mut bs = self.repository.find_by_id(id).await?;

        if let Some(name) = request.name {
            bs.name = name;
        }
        if let Some(payment_method_id) = request.payment_method_id {
            bs.payment_method_id = Some(payment_method_id);
        }
        if let Some(statement_date) = request.statement_date {
            bs.statement_date = Some(statement_date);
        }
        if let Some(due_date) = request.due_date {
            bs.due_date = Some(due_date);
        }
        if let Some(description) = request.description {
            bs.description = Some(description);
        }
        if let Some(is_active) = request.is_active {
            bs.is_active = is_active;
        }

        bs.updated_at = Utc::now().to_rfc3339();
        self.repository.update(id, bs).await
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        self.repository.delete(id).await
    }
}
