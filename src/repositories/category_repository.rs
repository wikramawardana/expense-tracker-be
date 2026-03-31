use crate::db::Database;
use crate::errors::{AppError, AppResult};
use crate::models::Category;

#[derive(Clone)]
pub struct CategoryRepository {
    db: Database,
}

impl CategoryRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn create(&self, category: Category) -> AppResult<Category> {
        let key = crate::models::record_key_to_string(&category.id.key);
        let created: Option<Category> = self
            .db
            .create::<Option<Category>>(("categories", key))
            .content(category)
            .await?;
        created.ok_or_else(|| AppError::Internal("Failed to create category".to_string()))
    }

    pub async fn find_by_id(&self, id: &str) -> AppResult<Category> {
        let result: Option<Category> = self.db.select(("categories", id)).await?;
        result.ok_or_else(|| AppError::NotFound(format!("Category with id {} not found", id)))
    }

    pub async fn find_all(&self) -> AppResult<Vec<Category>> {
        let sql = "SELECT * FROM categories WHERE is_active = true ORDER BY name ASC";
        let mut result = self.db.query(sql).await?;
        let items: Vec<Category> = result.take(0)?;
        Ok(items)
    }

    pub async fn update(&self, id: &str, category: Category) -> AppResult<Category> {
        let updated: Option<Category> =
            self.db.update(("categories", id)).content(category).await?;
        updated.ok_or_else(|| AppError::NotFound(format!("Category with id {} not found", id)))
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let deleted: Option<Category> = self.db.delete(("categories", id)).await?;
        match deleted {
            Some(_) => Ok(()),
            None => Err(AppError::NotFound(format!(
                "Category with id {} not found",
                id
            ))),
        }
    }
}
