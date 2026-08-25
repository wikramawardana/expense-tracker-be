use axum::{
    http::{header, Method},
    middleware,
    routing::{delete, get, patch, post, put},
    Router,
};
use tower_http::cors::CorsLayer;

use crate::handlers::{
    ApiKeyHandler, BillStatementHandler, CategoryHandler, ExpenseHandler, PaidByHandler,
    PaymentMethodHandler,
};
use crate::middleware::{auth_middleware, bot_auth_middleware, AuthState, BotAuthState};

#[allow(clippy::too_many_arguments)]
pub fn create_router(
    expense_handler: ExpenseHandler,
    payment_method_handler: PaymentMethodHandler,
    category_handler: CategoryHandler,
    bill_statement_handler: BillStatementHandler,
    api_key_handler: ApiKeyHandler,
    paid_by_handler: PaidByHandler,
    auth_state: AuthState,
    bot_auth_state: BotAuthState,
) -> Router {
    // Configure CORS
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    let cors = CorsLayer::new()
        .allow_origin(frontend_url.parse::<axum::http::HeaderValue>().unwrap())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::ORIGIN,
        ])
        .allow_credentials(true);

    // ========== Public Routes ==========
    let public_routes = Router::new().route("/health", get(health_check));

    // ========== Protected Expense Routes ==========
    let protected_expense_routes = Router::new()
        .route("/expenses", post(ExpenseHandler::create))
        .route("/expenses", get(ExpenseHandler::get_all))
        .route("/expenses/summary", get(ExpenseHandler::get_summary))
        .route("/expenses/navigation", get(ExpenseHandler::get_navigation))
        .route("/expenses/bulk", post(ExpenseHandler::create_bulk))
        .route("/expenses/bulk", patch(ExpenseHandler::apply_bulk_action))
        .route("/expenses/import-csv", post(ExpenseHandler::import_csv))
        .route(
            "/expenses/import-template.csv",
            get(ExpenseHandler::import_template),
        )
        .route("/expenses/:id", get(ExpenseHandler::get_by_id))
        .route("/expenses/:id", put(ExpenseHandler::update))
        .route("/expenses/:id", delete(ExpenseHandler::delete))
        .with_state(expense_handler.clone())
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ));

    // ========== Protected Payment Method Routes ==========
    let protected_payment_method_routes = Router::new()
        .route("/payment-methods", post(PaymentMethodHandler::create))
        .route("/payment-methods", get(PaymentMethodHandler::get_all))
        .route("/payment-methods/:id", get(PaymentMethodHandler::get_by_id))
        .route("/payment-methods/:id", put(PaymentMethodHandler::update))
        .route("/payment-methods/:id", delete(PaymentMethodHandler::delete))
        .with_state(payment_method_handler.clone())
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ));

    // ========== Protected Category Routes ==========
    let protected_category_routes = Router::new()
        .route("/categories", post(CategoryHandler::create))
        .route("/categories", get(CategoryHandler::get_all))
        .route("/categories/:id", get(CategoryHandler::get_by_id))
        .route("/categories/:id", put(CategoryHandler::update))
        .route("/categories/:id", delete(CategoryHandler::delete))
        .with_state(category_handler.clone())
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ));

    // ========== Protected Bill Statement Routes ==========
    let protected_bill_statement_routes = Router::new()
        .route("/bill-statements", post(BillStatementHandler::create))
        .route("/bill-statements", get(BillStatementHandler::get_all))
        .route("/bill-statements/:id", get(BillStatementHandler::get_by_id))
        .route("/bill-statements/:id", put(BillStatementHandler::update))
        .route("/bill-statements/:id", delete(BillStatementHandler::delete))
        .with_state(bill_statement_handler.clone())
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ));

    // ========== Protected API Key Management Routes (session-authed) ==========
    let protected_api_key_routes = Router::new()
        .route("/api-keys", post(ApiKeyHandler::create))
        .route("/api-keys", get(ApiKeyHandler::list))
        .route("/api-keys/:id", delete(ApiKeyHandler::revoke))
        .with_state(api_key_handler)
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ));

    // ========== Protected Paid By Routes ==========
    let protected_paid_by_routes = Router::new()
        .route("/paid-by", post(PaidByHandler::create))
        .route("/paid-by", get(PaidByHandler::get_all))
        .route("/paid-by/:id", get(PaidByHandler::get_by_id))
        .route("/paid-by/:id", put(PaidByHandler::update))
        .route("/paid-by/:id", delete(PaidByHandler::delete))
        .with_state(paid_by_handler.clone())
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ));

    // ========== Bot Routes (API-key authed, for openclaw / Discord / etc.) ==========
    // Intentionally a narrow surface: create expenses + read the metadata the
    // bot needs to pick category/payment-method/bill-statement IDs.
    let bot_expense_routes = Router::new()
        .route("/bot/expenses", post(ExpenseHandler::create))
        .route("/bot/expenses", get(ExpenseHandler::get_all))
        .route("/bot/expenses/bulk", post(ExpenseHandler::create_bulk))
        .with_state(expense_handler)
        .layer(middleware::from_fn_with_state(
            bot_auth_state.clone(),
            bot_auth_middleware,
        ));

    let bot_category_routes = Router::new()
        .route("/bot/categories", get(CategoryHandler::get_all))
        .with_state(category_handler)
        .layer(middleware::from_fn_with_state(
            bot_auth_state.clone(),
            bot_auth_middleware,
        ));

    let bot_payment_method_routes = Router::new()
        .route("/bot/payment-methods", get(PaymentMethodHandler::get_all))
        .with_state(payment_method_handler)
        .layer(middleware::from_fn_with_state(
            bot_auth_state.clone(),
            bot_auth_middleware,
        ));

    let bot_bill_statement_routes = Router::new()
        .route("/bot/bill-statements", get(BillStatementHandler::get_all))
        .with_state(bill_statement_handler)
        .layer(middleware::from_fn_with_state(
            bot_auth_state.clone(),
            bot_auth_middleware,
        ));

    let bot_paid_by_routes = Router::new()
        .route("/bot/paid-by", get(PaidByHandler::get_all))
        .with_state(paid_by_handler)
        .layer(middleware::from_fn_with_state(
            bot_auth_state,
            bot_auth_middleware,
        ));

    Router::new()
        .nest("/api/v1", public_routes)
        .nest("/api/v1", protected_expense_routes)
        .nest("/api/v1", protected_payment_method_routes)
        .nest("/api/v1", protected_category_routes)
        .nest("/api/v1", protected_bill_statement_routes)
        .nest("/api/v1", protected_api_key_routes)
        .nest("/api/v1", protected_paid_by_routes)
        .nest("/api/v1", bot_expense_routes)
        .nest("/api/v1", bot_category_routes)
        .nest("/api/v1", bot_payment_method_routes)
        .nest("/api/v1", bot_bill_statement_routes)
        .nest("/api/v1", bot_paid_by_routes)
        .layer(cors)
}

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

// ========== Route Helpers ==========
// Use these helpers to create consistent route patterns for your entities

/// Creates a standard CRUD router for an entity
///
/// Example usage:
/// ```rust
/// let product_routes = create_crud_routes(
///     "/products",
///     ProductHandler::create,
///     ProductHandler::get_all,
///     ProductHandler::get_by_id,
///     ProductHandler::update,
///     ProductHandler::delete,
///     product_handler,
/// );
/// ```
#[allow(dead_code)]
pub fn create_crud_routes<H, C, GA, GI, U, D>(
    base_path: &str,
    create_handler: C,
    get_all_handler: GA,
    get_by_id_handler: GI,
    update_handler: U,
    delete_handler: D,
    handler_state: H,
) -> Router
where
    H: Clone + Send + Sync + 'static,
    C: axum::handler::Handler<(), H> + Clone + Send + 'static,
    GA: axum::handler::Handler<(), H> + Clone + Send + 'static,
    GI: axum::handler::Handler<(), H> + Clone + Send + 'static,
    U: axum::handler::Handler<(), H> + Clone + Send + 'static,
    D: axum::handler::Handler<(), H> + Clone + Send + 'static,
{
    Router::new()
        .route(base_path, post(create_handler))
        .route(base_path, get(get_all_handler))
        .route(&format!("{}/:id", base_path), get(get_by_id_handler))
        .route(&format!("{}/:id", base_path), put(update_handler))
        .route(&format!("{}/:id", base_path), delete(delete_handler))
        .with_state(handler_state)
}
