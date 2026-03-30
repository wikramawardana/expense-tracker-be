use chrono::Utc;
use uuid::Uuid;
use validator::Validate;

use crate::errors::{AppError, AppResult};
use crate::models::{
    CreatePaymentMethodRequest, PaymentMethod, PaymentMethodResponse, UpdatePaymentMethodRequest,
};
use crate::repositories::PaymentMethodRepository;

#[derive(Clone)]
pub struct PaymentMethodService {
    repository: PaymentMethodRepository,
}

impl PaymentMethodService {
    pub fn new(repository: PaymentMethodRepository) -> Self {
        Self { repository }
    }

    pub async fn create(&self, request: CreatePaymentMethodRequest) -> AppResult<PaymentMethod> {
        request
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;

        let now = Utc::now().to_rfc3339();
        let pm = PaymentMethod {
            id: Uuid::new_v4().to_string(),
            name: request.name,
            method_type: request.method_type,
            description: request.description,
            is_active: true,
            created_at: now.clone(),
            updated_at: now,
        };

        self.repository.create(pm).await
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<PaymentMethod> {
        self.repository.find_by_id(id).await
    }

    pub async fn get_all(&self) -> AppResult<Vec<PaymentMethodResponse>> {
        let items = self.repository.find_all().await?;
        Ok(items.into_iter().map(PaymentMethodResponse::from).collect())
    }

    pub async fn update(
        &self,
        id: &str,
        request: UpdatePaymentMethodRequest,
    ) -> AppResult<PaymentMethod> {
        let mut pm = self.repository.find_by_id(id).await?;

        if let Some(name) = request.name {
            pm.name = name;
        }
        if let Some(method_type) = request.method_type {
            pm.method_type = method_type;
        }
        if let Some(description) = request.description {
            pm.description = Some(description);
        }
        if let Some(is_active) = request.is_active {
            pm.is_active = is_active;
        }

        pm.updated_at = Utc::now().to_rfc3339();
        self.repository.update(id, pm).await
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        self.repository.delete(id).await
    }
}
