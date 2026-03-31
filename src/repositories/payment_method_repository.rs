use crate::db::Database;
use crate::errors::{AppError, AppResult};
use crate::models::PaymentMethod;

#[derive(Clone)]
pub struct PaymentMethodRepository {
    db: Database,
}

impl PaymentMethodRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn create(&self, pm: PaymentMethod) -> AppResult<PaymentMethod> {
        let key = crate::models::record_key_to_string(&pm.id.key);
        let created: Option<PaymentMethod> = self
            .db
            .create::<Option<PaymentMethod>>(("payment_methods", key))
            .content(pm)
            .await?;
        created.ok_or_else(|| AppError::Internal("Failed to create payment method".to_string()))
    }

    pub async fn find_by_id(&self, id: &str) -> AppResult<PaymentMethod> {
        let result: Option<PaymentMethod> = self.db.select(("payment_methods", id)).await?;
        result.ok_or_else(|| AppError::NotFound(format!("Payment method with id {} not found", id)))
    }

    pub async fn find_all(&self) -> AppResult<Vec<PaymentMethod>> {
        let sql = "SELECT * FROM payment_methods WHERE is_active = true ORDER BY name ASC";
        let mut result = self.db.query(sql).await?;
        let items: Vec<PaymentMethod> = result.take(0)?;
        Ok(items)
    }

    pub async fn update(&self, id: &str, pm: PaymentMethod) -> AppResult<PaymentMethod> {
        let updated: Option<PaymentMethod> =
            self.db.update(("payment_methods", id)).content(pm).await?;
        updated
            .ok_or_else(|| AppError::NotFound(format!("Payment method with id {} not found", id)))
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let deleted: Option<PaymentMethod> = self.db.delete(("payment_methods", id)).await?;
        match deleted {
            Some(_) => Ok(()),
            None => Err(AppError::NotFound(format!(
                "Payment method with id {} not found",
                id
            ))),
        }
    }
}
