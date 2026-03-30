use crate::db::Database;
use crate::errors::{AppError, AppResult};
use crate::models::RecurrenceType;

#[derive(Clone)]
pub struct RecurrenceTypeRepository {
    db: Database,
}

impl RecurrenceTypeRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn create(&self, rt: RecurrenceType) -> AppResult<RecurrenceType> {
        let created: Option<RecurrenceType> = self
            .db
            .create(("recurrence_types", rt.id.clone()))
            .content(rt)
            .await?;
        created.ok_or_else(|| AppError::Internal("Failed to create recurrence type".to_string()))
    }

    pub async fn find_by_id(&self, id: &str) -> AppResult<RecurrenceType> {
        let result: Option<RecurrenceType> = self.db.select(("recurrence_types", id)).await?;
        result
            .ok_or_else(|| AppError::NotFound(format!("Recurrence type with id {} not found", id)))
    }

    #[allow(dead_code)]
    pub async fn find_by_name(&self, name: &str) -> AppResult<Option<RecurrenceType>> {
        let sql = "SELECT * FROM recurrence_types WHERE name = $name AND is_active = true LIMIT 1";
        let mut result = self.db.query(sql).bind(("name", name.to_string())).await?;
        let items: Vec<RecurrenceType> = result.take(0)?;
        Ok(items.into_iter().next())
    }

    pub async fn find_all(&self) -> AppResult<Vec<RecurrenceType>> {
        let sql = "SELECT * FROM recurrence_types WHERE is_active = true ORDER BY name ASC";
        let mut result = self.db.query(sql).await?;
        let items: Vec<RecurrenceType> = result.take(0)?;
        Ok(items)
    }

    pub async fn update(&self, id: &str, rt: RecurrenceType) -> AppResult<RecurrenceType> {
        let updated: Option<RecurrenceType> =
            self.db.update(("recurrence_types", id)).content(rt).await?;
        updated
            .ok_or_else(|| AppError::NotFound(format!("Recurrence type with id {} not found", id)))
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let deleted: Option<RecurrenceType> = self.db.delete(("recurrence_types", id)).await?;
        match deleted {
            Some(_) => Ok(()),
            None => Err(AppError::NotFound(format!(
                "Recurrence type with id {} not found",
                id
            ))),
        }
    }
}
