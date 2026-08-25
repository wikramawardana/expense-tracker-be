use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use axum_extra::extract::Multipart;

use crate::db::Database;
use crate::errors::{AppError, AppResult};
use crate::models::{
    ApiResponse, BulkCreateExpensesResponse, BulkExpenseActionRequest, CreateExpenseRequest,
    CreateExpensesBulkRequest, ExpenseNavigationResponse, ExpenseQueryParams, ExpenseResponse,
    ExpenseSummaryResponse, ImportExpensesCsvResponse, PaginatedExpensesResponse,
    UpdateExpenseRequest,
};
use crate::repositories::{
    BillStatementRepository, CategoryRepository, ExpenseRepository, PaymentMethodRepository,
};
use crate::services::{ExpenseService, EXPENSE_IMPORT_TEMPLATE};

#[derive(Clone)]
pub struct ExpenseHandler {
    service: ExpenseService,
}

impl ExpenseHandler {
    pub fn new(db: Database) -> Self {
        let repository = ExpenseRepository::new(db.clone());
        let category_repository = CategoryRepository::new(db.clone());
        let bill_statement_repository = BillStatementRepository::new(db.clone());
        let payment_method_repository = PaymentMethodRepository::new(db);
        let service = ExpenseService::new(
            repository,
            category_repository,
            bill_statement_repository,
            payment_method_repository,
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

    /// Create multiple expenses in a single request
    pub async fn create_bulk(
        State(handler): State<Self>,
        Json(request): Json<CreateExpensesBulkRequest>,
    ) -> AppResult<impl IntoResponse> {
        let expenses = handler.service.create_bulk(request).await?;
        let created: Vec<ExpenseResponse> =
            expenses.into_iter().map(ExpenseResponse::from).collect();
        let count = created.len();
        let response = BulkCreateExpensesResponse { created, count };
        Ok((
            StatusCode::CREATED,
            ApiResponse::success(
                response,
                &format!("{} expense(s) created successfully", count),
            ),
        ))
    }

    /// Import expenses from a CSV file
    pub async fn import_csv(
        State(handler): State<Self>,
        mut multipart: Multipart,
    ) -> AppResult<impl IntoResponse> {
        let mut file_bytes = None;

        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|e| AppError::Validation(format!("Could not read multipart form: {}", e)))?
        {
            let field_name = field.name().unwrap_or_default().to_string();
            if field_name == "file" || file_bytes.is_none() {
                file_bytes = Some(field.bytes().await.map_err(|e| {
                    AppError::Validation(format!("Could not read CSV file: {}", e))
                })?);
                if field_name == "file" {
                    break;
                }
            }
        }

        let bytes = file_bytes.ok_or_else(|| {
            AppError::Validation("CSV upload must include a file field".to_string())
        })?;
        let expenses = handler.service.import_csv(&bytes).await?;
        let created: Vec<ExpenseResponse> =
            expenses.into_iter().map(ExpenseResponse::from).collect();
        let count = created.len();
        let response = ImportExpensesCsvResponse { created, count };

        Ok((
            StatusCode::CREATED,
            ApiResponse::success(
                response,
                &format!("{} expense(s) imported successfully", count),
            ),
        ))
    }

    /// Download a CSV import template
    pub async fn import_template() -> AppResult<impl IntoResponse> {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("text/csv"));
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=\"expense-import-template.csv\""),
        );

        Ok((headers, EXPENSE_IMPORT_TEMPLATE))
    }

    /// Apply one action to multiple expenses
    pub async fn apply_bulk_action(
        State(handler): State<Self>,
        Json(request): Json<BulkExpenseActionRequest>,
    ) -> AppResult<impl IntoResponse> {
        let response = handler.service.apply_bulk_action(request).await?;
        Ok(ApiResponse::success(
            response,
            "Bulk expense action completed successfully",
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

    /// Get backend-calculated totals and breakdowns for a filtered expense scope.
    pub async fn get_summary(
        State(handler): State<Self>,
        Query(query): Query<ExpenseQueryParams>,
    ) -> AppResult<impl IntoResponse> {
        let response: ExpenseSummaryResponse = handler.service.get_summary(query).await?;
        Ok(ApiResponse::success(
            response,
            "Expense summary retrieved successfully",
        ))
    }

    /// Get payment-method and bill-statement facets for nested navigation.
    pub async fn get_navigation(State(handler): State<Self>) -> AppResult<impl IntoResponse> {
        let response: ExpenseNavigationResponse = handler.service.get_navigation().await?;
        Ok(ApiResponse::success(
            response,
            "Expense navigation retrieved successfully",
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
