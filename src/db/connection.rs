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

    let arc = Arc::new(db);
    DB_INSTANCE.set(arc.clone()).ok();
    arc
}
