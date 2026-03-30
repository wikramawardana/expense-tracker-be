mod config;
mod db;
mod errors;
mod handlers;
mod middleware;
mod models;
mod repositories;
mod routes;
mod services;

use config::load;
use db::init_db;
use middleware::AuthState;
use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "expense_tracker_be=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // Load configuration and connect to SurrealDB
    let cfg = load();
    println!("Connecting to SurrealDB...");
    let db_instance = init_db(&cfg).await;
    println!("SurrealDB Connected!");

    // Initialize PostgreSQL connection for session/auth verification
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for session verification");

    println!("Connecting to PostgreSQL for auth...");
    let pg_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to PostgreSQL");
    println!("PostgreSQL connected for auth verification");

    // Create auth state
    let auth_state = AuthState::new(pg_pool);

    // Initialize handlers
    let expense_handler = handlers::ExpenseHandler::new(db_instance.clone());
    let payment_method_handler = handlers::PaymentMethodHandler::new(db_instance.clone());
    let category_handler = handlers::CategoryHandler::new(db_instance.clone());
    let bill_statement_handler = handlers::BillStatementHandler::new(db_instance.clone());
    let recurrence_type_handler = handlers::RecurrenceTypeHandler::new(db_instance.clone());

    // Build the router
    let app = routes::create_router(
        expense_handler,
        payment_method_handler,
        category_handler,
        bill_statement_handler,
        recurrence_type_handler,
        auth_state,
    )
    .layer(TraceLayer::new_for_http());

    // Start the server
    let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("SERVER_PORT").unwrap_or_else(|_| "8000".to_string());
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();

    // ASCII art display
    let display_name = "Expense Tracker";
    let standard_font = figlet_rs::FIGfont::standard().unwrap();
    if let Some(ascii_art) = standard_font.convert(display_name) {
        println!("{}", ascii_art);
    }

    let startup_info = serde_json::json!({
        "status": "Server started",
        "url": format!("http://{}", addr)
    });
    println!("{}", serde_json::to_string(&startup_info).unwrap());

    axum::serve(listener, app).await.unwrap();
}
