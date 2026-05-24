use crate::config::Config;
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use surrealdb::{engine::any::Any, opt::auth::Root, Surreal};
use tokio::sync::OnceCell;
use uuid::Uuid;

pub type DB = Surreal<Any>;
pub type Database = Arc<DB>;

static DB_INSTANCE: OnceCell<Arc<DB>> = OnceCell::const_new();

pub async fn init_db(cfg: &Config) -> Arc<DB> {
    let db: Surreal<Any> = Surreal::init();

    db.connect(cfg.db_url.as_str())
        .await
        .expect("Failed to connect to database");

    db.signin(Root {
        username: cfg.db_user.clone(),
        password: cfg.db_pass.clone(),
    })
    .await
    .expect("Authentication failed. Please check your SurrealDB credentials.");

    db.use_ns(&cfg.db_ns)
        .use_db(&cfg.db_name)
        .await
        .expect("Failed to select namespace/database");

    // Ensure all tables exist as SCHEMALESS. SurrealDB v3 errors on SELECT
    // from a non-existent table instead of returning an empty set like v2 did,
    // so we explicitly define them on startup.
    let schema = r#"
        DEFINE TABLE IF NOT EXISTS expenses SCHEMALESS;
        DEFINE TABLE IF NOT EXISTS bill_statements SCHEMALESS;
        DEFINE TABLE IF NOT EXISTS categories SCHEMALESS;
        DEFINE TABLE IF NOT EXISTS payment_methods SCHEMALESS;
        DEFINE TABLE IF NOT EXISTS recurrence_types SCHEMALESS;
    "#;
    db.query(schema)
        .await
        .expect("Failed to initialize SurrealDB schema");

    seed_default_records(&db)
        .await
        .expect("Failed to initialize default data");

    let arc = Arc::new(db);
    DB_INSTANCE.set(arc.clone()).ok();
    arc
}

async fn seed_default_records(db: &DB) -> Result<(), surrealdb::Error> {
    normalize_optional_category_fields(db).await?;
    ensure_recurrence_type(
        db,
        "Installment",
        "Fixed-count monthly installment schedule",
    )
    .await?;
    ensure_category(
        db,
        "Subscription",
        Some("Software and service subscriptions"),
        Some("#6366F1"),
    )
    .await?;

    Ok(())
}

async fn normalize_optional_category_fields(db: &DB) -> Result<(), surrealdb::Error> {
    db.query(
        r#"
        UPDATE categories SET icon = NONE WHERE icon = null;
        UPDATE categories SET color = NONE WHERE color = null;
        UPDATE categories SET description = NONE WHERE description = null;
        "#,
    )
    .await?;

    Ok(())
}

async fn ensure_recurrence_type(
    db: &DB,
    name: &str,
    description: &str,
) -> Result<(), surrealdb::Error> {
    let mut existing = db
        .query("SELECT name FROM recurrence_types WHERE name = $name LIMIT 1")
        .bind(("name", name.to_string()))
        .await?;
    let rows: Vec<Value> = existing.take(0)?;
    if !rows.is_empty() {
        return Ok(());
    }

    let now = Utc::now().to_rfc3339();
    let key = Uuid::new_v4().to_string();
    let _: Option<Value> = db
        .create(("recurrence_types", key))
        .content(json!({
            "name": name,
            "description": description,
            "is_active": true,
            "created_at": now,
            "updated_at": now,
        }))
        .await?;

    Ok(())
}

async fn ensure_category(
    db: &DB,
    name: &str,
    description: Option<&str>,
    color: Option<&str>,
) -> Result<(), surrealdb::Error> {
    let mut existing = db
        .query("SELECT name FROM categories WHERE name = $name LIMIT 1")
        .bind(("name", name.to_string()))
        .await?;
    let rows: Vec<Value> = existing.take(0)?;
    if !rows.is_empty() {
        return Ok(());
    }

    let now = Utc::now().to_rfc3339();
    let key = Uuid::new_v4().to_string();
    let mut content = json!({
        "name": name,
        "is_active": true,
        "created_at": now,
        "updated_at": now,
    });
    if let Some(color) = color {
        content["color"] = json!(color);
    }
    if let Some(description) = description {
        content["description"] = json!(description);
    }

    let _: Option<Value> = db.create(("categories", key)).content(content).await?;

    Ok(())
}
