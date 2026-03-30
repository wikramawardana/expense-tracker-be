use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::db::Database;
use crate::errors::AppResult;
use crate::models::{
    ApiResponse, CreateExpenseRequest, ExpenseQueryParams, ExpenseResponse,
    PaginatedExpensesResponse, UpdateExpenseRequest,
};
use crate::repositories::{
    BillStatementRepository, ExpenseRepository, PaymentMethodRepository, RecurrenceTypeRepository,
};
use crate::services::ExpenseService;

#[derive(Clone)]
pub struct ExpenseHandler {
    service: ExpenseService,
}

impl ExpenseHandler {
    pub fn new(db: Database) -> Self {
        let repository = ExpenseRepository::new(db.clone());
        let bill_statement_repository = BillStatementRepository::new(db.clone());
        let payment_method_repository = PaymentMethodRepository::new(db.clone());
        let recurrence_type_repository = RecurrenceTypeRepository::new(db);
        let service = ExpenseService::new(
            repository,
            bill_statement_repository,
            payment_method_repository,
            recurrence_type_repository,
        );
        Self { service }
    }

    /// Create a new expense
    pub async fn create(
        State(handler): State<Self>,
        Json(request): Json<CreateExpenseRequest>,
    ) -> AppResult<impl IntoResponse> {
        let expense = handler.service.create(request).await?;
        let response = ExpenseResponse::from(expense);
        Ok((
            StatusCode::CREATED,
            ApiResponse::success(response, "Expense created successfully"),
        ))
    }

    /// Get a single expense by ID
    pub async fn get_by_id(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        let expense = handler.service.get_by_id(&id).await?;
        let response = ExpenseResponse::from(expense);
        Ok(ApiResponse::success(
            response,
            "Expense retrieved successfully",
        ))
    }

    /// Get all expenses with pagination and filters
    pub async fn get_all(
        State(handler): State<Self>,
        Query(query): Query<ExpenseQueryParams>,
    ) -> AppResult<impl IntoResponse> {
        let paginated_response: PaginatedExpensesResponse = handler.service.get_all(query).await?;
        Ok(ApiResponse::success(
            paginated_response,
            "Expenses retrieved successfully",
        ))
    }

    /// Update an expense
    pub async fn update(
        State(handler): State<Self>,
        Path(id): Path<String>,
        Json(request): Json<UpdateExpenseRequest>,
    ) -> AppResult<impl IntoResponse> {
        let expense = handler.service.update(&id, request).await?;
        let response = ExpenseResponse::from(expense);
        Ok(ApiResponse::success(
            response,
            "Expense updated successfully",
        ))
    }

    /// Delete an expense
    pub async fn delete(
        State(handler): State<Self>,
        Path(id): Path<String>,
    ) -> AppResult<impl IntoResponse> {
        handler.service.delete(&id).await?;
        Ok(ApiResponse::<()>::success_msg(
            "Expense deleted successfully",
        ))
    }
}
