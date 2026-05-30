use crate::db::Database;
use crate::errors::{AppError, AppResult};
use crate::models::PaidBy;

#[derive(Clone)]
pub struct PaidByRepository {
    db: Database,
}

impl PaidByRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn create(&self, pb: PaidBy) -> AppResult<PaidBy> {
        let key = crate::models::record_key_to_string(&pb.id.key);
        let created: Option<PaidBy> = self
            .db
            .create::<Option<PaidBy>>(("paid_by", key))
            .content(pb)
            .await?;
        created.ok_or_else(|| AppError::Internal("Failed to create paid by".to_string()))
    }

    pub async fn find_by_id(&self, id: &str) -> AppResult<PaidBy> {
        let result: Option<PaidBy> = self.db.select(("paid_by", id)).await?;
        result.ok_or_else(|| AppError::NotFound(format!("Paid by with id {} not found", id)))
    }

    pub async fn find_all(&self) -> AppResult<Vec<PaidBy>> {
        let sql = "SELECT * FROM paid_by ORDER BY name ASC";
        let mut result = self.db.query(sql).await?;
        let items: Vec<PaidBy> = result.take(0).unwrap_or_default();
        Ok(items)
    }

    pub async fn update(&self, id: &str, pb: PaidBy) -> AppResult<PaidBy> {
        let updated: Option<PaidBy> = self.db.update(("paid_by", id)).content(pb).await?;
        updated.ok_or_else(|| AppError::NotFound(format!("Paid by with id {} not found", id)))
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let deleted: Option<PaidBy> = self.db.delete(("paid_by", id)).await?;
        match deleted {
            Some(_) => Ok(()),
            None => Err(AppError::NotFound(format!(
                "Paid by with id {} not found",
                id
            ))),
        }
    }
}
