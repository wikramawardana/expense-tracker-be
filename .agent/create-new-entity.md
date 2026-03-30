---
description: Create a new entity with full CRUD operations following the project's architecture pattern
---

# Create New Entity Workflow

This workflow creates a new entity following the **Handler → Service → Repository** pattern used in this project.

## Prerequisites

Before starting, you need:

1. **Entity Name** (e.g., `Product`, `User`, `Category`)
2. **Entity Fields** with types
3. **SurrealDB table name** (usually lowercase plural, e.g., `products`)

## Step 1: Create the Model

Create `src/models/{entity_snake}.rs` with:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use surrealdb::sql::{Id, Thing};
use validator::Validate;

// Helper to deserialize SurrealDB Thing ID to String
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
pub struct {EntityName} {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    // Add your fields here
    pub name: String,
    // ... other fields
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ========== Request DTOs ==========

#[derive(Debug, Deserialize, Validate)]
pub struct Create{EntityName}Request {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,
    // ... other fields with validation
}

#[derive(Debug, Deserialize, Validate)]
pub struct Update{EntityName}Request {
    pub name: Option<String>,
    // ... other optional fields
}

// ========== Response DTOs ==========

#[derive(Debug, Serialize)]
pub struct {EntityName}Response {
    pub id: String,
    pub name: String,
    // ... other fields
    pub created_at: String,
    pub updated_at: String,
}

impl From<{EntityName}> for {EntityName}Response {
    fn from(entity: {EntityName}) -> Self {
        {EntityName}Response {
            id: entity.id,
            name: entity.name,
            // ... map other fields
            created_at: entity.created_at.to_rfc3339(),
            updated_at: entity.updated_at.to_rfc3339(),
        }
    }
}

// ========== Query Params (Optional) ==========

#[derive(Debug, Deserialize)]
pub struct {EntityName}QueryParams {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
    pub search: Option<String>,
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
}

fn default_page() -> u32 { 1 }
fn default_page_size() -> u32 { 10 }
fn default_sort_by() -> String { "created_at".to_string() }
fn default_sort_order() -> String { "desc".to_string() }

#[derive(Debug, Serialize)]
pub struct Paginated{EntityName}sResponse {
    pub data: Vec<{EntityName}Response>,
    pub pagination: {EntityName}PaginationMeta,
}

#[derive(Debug, Serialize)]
pub struct {EntityName}PaginationMeta {
    pub page: u32,
    pub page_size: u32,
    pub total_items: u32,
    pub total_pages: u32,
}
```

## Step 2: Update Models mod.rs

Add to `src/models/mod.rs`:

```rust
pub mod {entity_snake};
pub use {entity_snake}::{
    {EntityName}, Create{EntityName}Request, Update{EntityName}Request,
    {EntityName}Response, {EntityName}QueryParams, Paginated{EntityName}sResponse,
};
```

## Step 3: Create the Repository

Create `src/repositories/{entity_snake}_repository.rs`:

```rust
use crate::db::Database;
use crate::errors::{AppError, AppResult};
use crate::models::{EntityName};

#[derive(Clone)]
pub struct {EntityName}Repository {
    db: Database,
}

impl {EntityName}Repository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn create(&self, entity: {EntityName}) -> AppResult<{EntityName}> {
        let created: Option<{EntityName}> = self.db
            .create(("{table_name}", entity.id.clone()))
            .content(entity)
            .await?;
        created.ok_or_else(|| AppError::Internal("Failed to create {entity_name}".to_string()))
    }

    pub async fn find_by_id(&self, id: &str) -> AppResult<{EntityName}> {
        let result: Option<{EntityName}> = self.db.select(("{table_name}", id)).await?;
        result.ok_or_else(|| AppError::NotFound(format!("{EntityName} with id {} not found", id)))
    }

    pub async fn find_all(&self) -> AppResult<Vec<{EntityName}>> {
        let results: Vec<{EntityName}> = self.db.select("{table_name}").await?;
        Ok(results)
    }

    pub async fn update(&self, id: &str, entity: {EntityName}) -> AppResult<{EntityName}> {
        let updated: Option<{EntityName}> = self.db.update(("{table_name}", id)).content(entity).await?;
        updated.ok_or_else(|| AppError::NotFound(format!("{EntityName} with id {} not found", id)))
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        let deleted: Option<{EntityName}> = self.db.delete(("{table_name}", id)).await?;
        match deleted {
            Some(_) => Ok(()),
            None => Err(AppError::NotFound(format!("{EntityName} with id {} not found", id))),
        }
    }

    // Add more methods as needed (find_with_query, count, etc.)
}
```

## Step 4: Update Repositories mod.rs

Add to `src/repositories/mod.rs`:

```rust
pub mod {entity_snake}_repository;
pub use {entity_snake}_repository::{EntityName}Repository;
```

## Step 5: Create the Service

Create `src/services/{entity_snake}_service.rs`:

```rust
use chrono::Utc;
use uuid::Uuid;
use validator::Validate;

use crate::errors::{AppError, AppResult};
use crate::models::{Create{EntityName}Request, Update{EntityName}Request, {EntityName}};
use crate::repositories::{EntityName}Repository;

#[derive(Clone)]
pub struct {EntityName}Service {
    repository: {EntityName}Repository,
}

impl {EntityName}Service {
    pub fn new(repository: {EntityName}Repository) -> Self {
        Self { repository }
    }

    pub async fn create(&self, request: Create{EntityName}Request) -> AppResult<{EntityName}> {
        request.validate().map_err(|e| AppError::Validation(e.to_string()))?;

        let now = Utc::now();
        let entity = {EntityName} {
            id: Uuid::new_v4().to_string(),
            name: request.name,
            // ... map other fields
            created_at: now,
            updated_at: now,
        };

        self.repository.create(entity).await
    }

    pub async fn get_by_id(&self, id: &str) -> AppResult<{EntityName}> {
        self.repository.find_by_id(id).await
    }

    pub async fn get_all(&self) -> AppResult<Vec<{EntityName}>> {
        self.repository.find_all().await
    }

    pub async fn update(&self, id: &str, request: Update{EntityName}Request) -> AppResult<{EntityName}> {
        let mut entity = self.repository.find_by_id(id).await?;

        // Update fields if provided
        if let Some(name) = request.name {
            entity.name = name;
        }
        // ... update other fields

        entity.updated_at = Utc::now();

        self.repository.update(id, entity).await
    }

    pub async fn delete(&self, id: &str) -> AppResult<()> {
        self.repository.delete(id).await
    }
}
```

## Step 6: Update Services mod.rs

Add to `src/services/mod.rs`:

```rust
pub mod {entity_snake}_service;
pub use {entity_snake}_service::{EntityName}Service;
```

## Step 7: Create the Handler

Create `src/handlers/{entity_snake}_handler.rs`:

```rust
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::db::Database;
use crate::errors::AppResult;
use crate::models::{ApiResponse, Create{EntityName}Request, Update{EntityName}Request, {EntityName}Response};
use crate::repositories::{EntityName}Repository;
use crate::services::{EntityName}Service;

#[derive(Clone)]
pub struct {EntityName}Handler {
    service: {EntityName}Service,
}

impl {EntityName}Handler {
    pub fn new(db: Database) -> Self {
        let repository = {EntityName}Repository::new(db);
        let service = {EntityName}Service::new(repository);
        Self { service }
    }

    pub async fn create(
        State(handler): State<Self>,
        Json(request): Json<Create{EntityName}Request>,
    ) -> AppResult<impl IntoResponse> {
        let entity = handler.service.create(request).await?;
        let response = {EntityName}Response::from(entity);
        Ok((
            StatusCode::CREATED,
            ApiResponse::success(response, "{EntityName} created successfully"),
        ))
    }

    pub async fn get_by_id(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let entity = handler.service.get_by_id(&id).await?;
        let response = {EntityName}Response::from(entity);
        Ok(ApiResponse::success(response, "{EntityName} retrieved successfully"))
    }

    pub async fn get_all(
        State(handler): State<Self>,
    ) -> AppResult<impl IntoResponse> {
        let entities = handler.service.get_all().await?;
        let responses: Vec<{EntityName}Response> = entities.into_iter().map({EntityName}Response::from).collect();
        Ok(ApiResponse::success(responses, "{EntityName}s retrieved successfully"))
    }

    pub async fn update(
        State(handler): State<Self>,
        Path(id): Path<String>,
        Json(request): Json<Update{EntityName}Request>,
    ) -> AppResult<impl IntoResponse> {
        let entity = handler.service.update(&id, request).await?;
        let response = {EntityName}Response::from(entity);
        Ok(ApiResponse::success(response, "{EntityName} updated successfully"))
    }

    pub async fn delete(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        handler.service.delete(&id).await?;
        Ok(ApiResponse::<()>::success_msg("{EntityName} deleted successfully"))
    }
}
```

## Step 8: Update Handlers mod.rs

Add to `src/handlers/mod.rs`:

```rust
pub mod {entity_snake}_handler;
pub use {entity_snake}_handler::{EntityName}Handler;
```

## Step 9: Add Routes

Add to `src/routes/api.rs`:

```rust
// In the create_router function, add:

let protected_{entity_snake}_routes = Router::new()
    .route("/{entity_plural}", post({EntityName}Handler::create))
    .route("/{entity_plural}", get({EntityName}Handler::get_all))
    .route("/{entity_plural}/:id", get({EntityName}Handler::get_by_id))
    .route("/{entity_plural}/:id", put({EntityName}Handler::update))
    .route("/{entity_plural}/:id", delete({EntityName}Handler::delete))
    .with_state({entity_snake}_handler)
    .layer(middleware::from_fn_with_state(
        auth_state.clone(),
        auth_middleware,
    ));

// Then merge in the final Router::new():
.nest("/api/v1", protected_{entity_snake}_routes)
```

## Step 10: Update main.rs

Add handler initialization and router setup:

```rust
// In main(), add:
let {entity_snake}_handler = handlers::{EntityName}Handler::new(db_instance.clone());

// Update create_router call to include the new handler
let app = routes::create_router(
    order_handler,
    menu_handler,
    price_list_handler,
    {entity_snake}_handler, // Add this
    auth_state,
    upload_dir,
)
```

## Variable Reference

When using this workflow, replace these placeholders:

| Placeholder       | Example    | Description                  |
| ----------------- | ---------- | ---------------------------- |
| `{EntityName}`    | `Product`  | PascalCase entity name       |
| `{entity_snake}`  | `product`  | snake_case entity name       |
| `{entity_plural}` | `products` | URL path (lowercase plural)  |
| `{table_name}`    | `products` | SurrealDB table name         |
| `{entity_name}`   | `product`  | Lowercase for error messages |

## Example Usage

To create a `Category` entity with fields `name` and `description`:

1. Replace `{EntityName}` with `Category`
2. Replace `{entity_snake}` with `category`
3. Replace `{entity_plural}` with `categories`
4. Replace `{table_name}` with `categories`
5. Add your specific fields to models, service, etc.
