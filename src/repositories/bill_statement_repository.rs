use crate::db::Database;
use crate::errors::{AppError, AppResult};
use crate::models::BillStatement;

#[derive(Clone)]
pub struct BillStatementRepository {
    db: Database,
}

impl BillStatementRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn create(&self, bs: BillStatement) -> AppResult<BillStatement> {
        let created: Option<BillStatement> = self
            .db
            .create(("bill_statements", bs.id.clone()))
            .content(bs)
            .await?;
        created.ok_or_else(|| AppError::Internal("Failed to create bill statement".to_string()))
    }

    pub async fn find_by_id(&self, id: &str) -> AppResult<BillStatement> {
        let result: Option<BillStatement> = self.db.select(("bill_statements", id)).await?;
        result.ok_or_else(|| AppError::NotFound(format!("Bill statement with id {} not found", id)))
    }

    pub async fn find_all(&self) -> AppResult<Vec<BillStatement>> {
        let sql = "SELECT * FROM bill_statements WHERE is_active = true ORDER BY name ASC";
        let mut result = self.db.query(sql).await?;
        let items: Vec<BillStatement> = result.take(0)?;
        Ok(items)
    }

    /// Find bill statement by exact name match
    pub async fn find_by_name(&self, name: &str) -> AppResult<Option<BillStatement>> {
        let sql = "SELECT * FROM bill_statements WHERE name = $name AND is_active = true LIMIT 1";
        let mut result = self.db.query(sql).bind(("name", name.to_string())).await?;
        let items: Vec<BillStatement> = result.take(0)?;
        Ok(items.into_iter().next())
    }

    pub async fn update(&self, id: &str, bs: BillStatement) -> AppResult<BillStatement> {
        let updated: Option<BillStatement> =
            self.db.update(("bill_statements", id)).content(bs).await?;
        updated
            .ok_or_else(|| AppError::NotFound(format!("Bill statement with id {} not found", id)))
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let deleted: Option<BillStatement> = self.db.delete(("bill_statements", id)).await?;
        match deleted {
            Some(_) => Ok(()),
            None => Err(AppError::NotFound(format!(
                "Bill statement with id {} not found",
                id
            ))),
        }
    }
}
