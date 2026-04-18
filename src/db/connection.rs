use crate::config::Config;
use std::sync::Arc;
use surrealdb::{engine::any::Any, opt::auth::Root, Surreal};
use tokio::sync::OnceCell;

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

    let arc = Arc::new(db);
    DB_INSTANCE.set(arc.clone()).ok();
    arc
}
