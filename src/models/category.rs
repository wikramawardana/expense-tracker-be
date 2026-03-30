use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use surrealdb::sql::{Id, Thing};
use validator::Validate;

fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let t = Thing::deserialize(deserializer)?;
    match t.id {
        Id::String(s) => Ok(s),
        _ => Ok(t.id.to_string()),
    }
}

// ========== Main Entity ==========

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    pub name: String,          // e.g., "Transportation", "E-commerce", "F&B"
    pub icon: Option<String>,  // Optional icon name/emoji
    pub color: Option<String>, // Optional hex color for UI
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ========== Request DTOs ==========

#[derive(Debug, Deserialize, Validate)]
pub struct CreateCategoryRequest {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateCategoryRequest {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

// ========== Response DTOs ==========

#[derive(Debug, Serialize)]
pub struct CategoryResponse {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Category> for CategoryResponse {
    fn from(cat: Category) -> Self {
        CategoryResponse {
            id: cat.id,
            name: cat.name,
            icon: cat.icon,
            color: cat.color,
            description: cat.description,
            is_active: cat.is_active,
            created_at: cat.created_at.to_rfc3339(),
            updated_at: cat.updated_at.to_rfc3339(),
        }
    }
}
