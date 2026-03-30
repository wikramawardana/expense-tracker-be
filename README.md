# Rust Axum API Template

A clean, layered architecture template for building REST APIs with Rust, Axum, and SurrealDB.

## Features

- 🏗️ **Layered Architecture** - Handler → Service → Repository pattern
- 🔐 **Auth Middleware** - Session-based authentication with PostgreSQL
- 👥 **Role-Based Access Control** - Easy role checking in handlers
- 📝 **Request Validation** - Using validator crate
- 🌐 **CORS Support** - Pre-configured for frontend integration
- 📊 **Prometheus Metrics** - Built-in metrics endpoint
- 🔍 **Structured Logging** - JSON logging with tracing
- 🚀 **Health Check** - Ready for container orchestration

## Architecture

```
src/
├── main.rs              # Application entry point
├── config/              # Configuration management
│   ├── mod.rs
│   └── app_config.rs
├── db/                  # Database connection
│   ├── mod.rs
│   └── connection.rs
├── errors/              # Error handling
│   ├── mod.rs
│   └── app_error.rs
├── middleware/          # Auth & custom middleware
│   ├── mod.rs
│   └── auth.rs
├── models/              # Data structures, DTOs
│   ├── mod.rs
│   ├── response.rs      # Generic API response
│   └── {entity}.rs      # Entity-specific models
├── repositories/        # Data access layer
│   ├── mod.rs
│   └── {entity}_repository.rs
├── services/            # Business logic layer
│   ├── mod.rs
│   └── {entity}_service.rs
├── handlers/            # HTTP handlers (controllers)
│   ├── mod.rs
│   └── {entity}_handler.rs
└── routes/              # Route definitions
    ├── mod.rs
    └── api.rs
```

## Layer Responsibilities

### Handlers (Controllers)

- Parse HTTP requests
- Extract path/query parameters
- Call service methods
- Format HTTP responses

### Services

- Business logic
- Validation
- Coordinate multiple repository calls
- Data transformation

### Repositories

- Database operations
- SQL/NoSQL queries
- No business logic

### Models

- Data structures
- Request/Response DTOs
- Validation rules

## Naming Conventions

| Layer      | File Pattern             | Struct Pattern                                                                   |
| ---------- | ------------------------ | -------------------------------------------------------------------------------- |
| Handler    | `{entity}_handler.rs`    | `{Entity}Handler`                                                                |
| Service    | `{entity}_service.rs`    | `{Entity}Service`                                                                |
| Repository | `{entity}_repository.rs` | `{Entity}Repository`                                                             |
| Model      | `{entity}.rs`            | `{Entity}`, `Create{Entity}Request`, `Update{Entity}Request`, `{Entity}Response` |

## Authentication

### Session-Based Auth

The template includes middleware for session-based authentication using PostgreSQL. This is compatible with:

- [Better Auth](https://better-auth.com/)
- [Lucia Auth](https://lucia-auth.com/)
- Custom session implementations

### Using Auth in Handlers

```rust
use crate::middleware::CurrentUser;

pub async fn protected_handler(
    user: CurrentUser,  // Extractor - will 401 if not authenticated
) -> AppResult<impl IntoResponse> {
    // Access user data
    println!("User: {} ({})", user.0.email, user.0.id);

    // Check role
    if user.0.role.as_deref() == Some("admin") {
        // Admin logic
    }

    Ok(ApiResponse::success_msg("Hello, authenticated user!"))
}
```

### Role-Based Access Control

```rust
// In your handler
pub async fn admin_handler(
    user: CurrentUser,
) -> AppResult<impl IntoResponse> {
    // Check for admin role
    if user.0.role.as_deref() != Some("admin") {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    // Admin-only logic...
    Ok(ApiResponse::success_msg("Admin action completed"))
}
```

### Auth Middleware Types

| Middleware                 | Use Case                                     |
| -------------------------- | -------------------------------------------- |
| `auth_middleware`          | Required auth - 401 if no valid token        |
| `optional_auth_middleware` | Optional auth - continues even without token |

## Quick Start

1. Copy this template to your new project:

   ```bash
   cp -r rust-axum-api ~/MyProjects/new-project
   cd ~/MyProjects/new-project
   mv Cargo.toml.template Cargo.toml
   ```

2. Update `Cargo.toml` with your project name

3. Configure `.env`:

   ```bash
   cp .env.example .env
   # Edit .env with your credentials
   ```

4. Run the project:

   ```bash
   cargo run
   ```

5. Create your first entity using the workflow!

## API Response Format

All responses follow this structure:

```json
{
  "status": "success",
  "message": "Operation completed successfully",
  "data": { ... }  // Optional
}
```

Error responses:

```json
{
  "status": "error",
  "message": "Error description"
}
```

## Environment Variables

| Variable          | Description              | Example                 |
| ----------------- | ------------------------ | ----------------------- |
| `SERVER_HOST`     | Server bind address      | `127.0.0.1`             |
| `SERVER_PORT`     | Server port              | `8000`                  |
| `SURREAL_DB_URL`  | SurrealDB connection URL | `wss://...`             |
| `SURREAL_DB_USER` | SurrealDB username       | `root`                  |
| `SURREAL_DB_PASS` | SurrealDB password       | `secret`                |
| `SURREAL_DB_NS`   | SurrealDB namespace      | `myapp`                 |
| `SURREAL_DB_DB`   | SurrealDB database       | `production`            |
| `DATABASE_URL`    | PostgreSQL for auth      | `postgres://...`        |
| `FRONTEND_URL`    | CORS allowed origin      | `http://localhost:3000` |
| `RUST_LOG`        | Log level                | `debug`                 |

## Dependencies

- **axum**: Web framework
- **tokio**: Async runtime
- **surrealdb**: Main database client
- **sqlx**: PostgreSQL client for auth
- **serde**: Serialization
- **chrono**: Date/Time handling
- **uuid**: Unique identifiers
- **validator**: Request validation
- **thiserror**: Error handling
- **tracing**: Structured logging

## Creating New Entities

See the workflow file: `.agent/workflows/create-new-entity.md`

Or just ask me: "Create a Product entity with fields: name, price, stock"

## API Examples (Expense Tracker)

### Expense Field Reference

| Field               | Type      | Input Format                                               |
| ------------------- | --------- | ---------------------------------------------------------- |
| `payment_method`    | String    | ✅ Free text: `"BCA Credit Card"`, `"Cash"`                |
| `payment_method_id` | Linked ID | ⚠️ Requires UUID: `"9f91a1be-c83d-4eea-ad80-601329afc1ef"` |
| `category_id`       | Linked ID | ⚠️ Requires UUID: `"6a8efec3-e489-4cc5-9bd9-963e70781581"` |
| `paid_by`           | String    | ✅ Free text: `"Salary"`, `"Bonus"`                        |

> **Note**: You can use either `payment_method` (free text) OR `payment_method_id` (linked ID).  
> Using IDs enables better filtering and reporting since they link to actual records.

#### Getting IDs

```bash
# List payment methods to get their IDs
curl http://localhost:8000/api/v1/payment-methods

# List categories to get their IDs
curl http://localhost:8000/api/v1/categories
```

### Create One-Time Expense

```bash
curl -X POST http://localhost:8000/api/v1/expenses \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Lunch",
    "amount": 50000,
    "payment_method": "Cash",
    "expense_date": "2026-01-01T12:00:00Z",
    "description": "Makan siang"
  }'
```

### Create Installment (Auto-generates all 12 payments)

```bash
curl -X POST http://localhost:8000/api/v1/expenses \
  -H "Content-Type: application/json" \
  -d '{
    "title": "iPhone 16 Pro",
    "amount": 1500000,
    "payment_method": "Credit Card",
    "expense_date": "2026-01-15T00:00:00Z",
    "description": "Cicilan HP",
    "recurrence_type": "installment",
    "recurrence_count": 12,
    "recurrence_total_amount": 18000000
  }'
```

This creates 12 expenses automatically:

- Jan 2026: iPhone 16 Pro (1/12) - 1,500,000
- Feb 2026: iPhone 16 Pro (2/12) - 1,500,000
- ... through Dec 2026

### Create Subscription (Monthly until end date)

```bash
curl -X POST http://localhost:8000/api/v1/expenses \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Netflix Premium",
    "amount": 199000,
    "payment_method": "Credit Card",
    "expense_date": "2026-01-05T00:00:00Z",
    "description": "Streaming subscription",
    "recurrence_type": "subscription",
    "recurrence_end_date": "2026-12-31T00:00:00Z"
  }'
```

### Create Recurring Monthly Expense

```bash
curl -X POST http://localhost:8000/api/v1/expenses \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Internet Biznet",
    "amount": 500000,
    "payment_method": "Auto Debit",
    "expense_date": "2026-01-20T00:00:00Z",
    "description": "Tagihan internet bulanan",
    "recurrence_type": "recurring",
    "recurrence_end_date": "2026-12-31T00:00:00Z"
  }'
```

### Recurrence Types

| Type           | Description                | Auto-Generated                           |
| -------------- | -------------------------- | ---------------------------------------- |
| `none`         | One-time expense (default) | 1 expense                                |
| `installment`  | Fixed number of payments   | N expenses (based on `recurrence_count`) |
| `subscription` | Monthly subscription       | Until `recurrence_end_date` or 12 months |
| `recurring`    | Regular monthly bill       | Until `recurrence_end_date` or 12 months |

### List Expenses with Filters

```bash
# List all expenses (paginated)
curl "http://localhost:8000/api/v1/expenses?page=1&page_size=10"

# Filter by date range
curl "http://localhost:8000/api/v1/expenses?expense_date_from=2026-01-01T00:00:00Z&expense_date_to=2026-01-31T23:59:59Z"

# Filter by payment method
curl "http://localhost:8000/api/v1/expenses?payment_method=Credit%20Card"

# Filter by status
curl "http://localhost:8000/api/v1/expenses?status=pending"

# Sort by amount descending
curl "http://localhost:8000/api/v1/expenses?sort_by=amount&sort_order=desc"
```

### Update Expense

```bash
curl -X PUT http://localhost:8000/api/v1/expenses/{id} \
  -H "Content-Type: application/json" \
  -d '{
    "status": "paid"
  }'
```

### Delete Expense

```bash
curl -X DELETE http://localhost:8000/api/v1/expenses/{id}
```

---

## Payment Methods API

### Create Payment Method

```bash
curl -X POST http://localhost:8000/api/v1/payment-methods \
  -H "Content-Type: application/json" \
  -d '{
    "name": "BCA Credit Card",
    "method_type": "credit_card",
    "description": "BCA Mastercard"
  }'
```

### List All Payment Methods

```bash
curl http://localhost:8000/api/v1/payment-methods
```

### Method Types

| Type            | Description                 |
| --------------- | --------------------------- |
| `credit_card`   | Credit cards                |
| `debit_card`    | Debit cards                 |
| `e_wallet`      | GoPay, OVO, DANA, ShopeePay |
| `bank_transfer` | Bank transfers              |
| `cash`          | Cash payments               |

---

## Categories API

### Create Category

```bash
curl -X POST http://localhost:8000/api/v1/categories \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Transportation",
    "icon": "🚗",
    "color": "#3B82F6",
    "description": "Grab, Gojek, taxi, parking"
  }'
```

### Suggested Categories

| Category        | Icon | Examples                          |
| --------------- | ---- | --------------------------------- |
| Transportation  | 🚗   | Grab, Gojek, taxi, parking        |
| E-commerce      | 🛒   | Shopee, Tokopedia, Lazada         |
| Food & Beverage | 🍔   | GrabFood, GoFood, restaurants     |
| Subscription    | 📺   | Netflix, Spotify, YouTube Premium |
| Utilities       | 💡   | Electricity, water, internet      |
| Shopping        | 🛍️   | Mall, fashion, electronics        |

### List All Categories

```bash
curl http://localhost:8000/api/v1/categories
```

---

## Bill Statements API

### Create Bill Statement

```bash
curl -X POST http://localhost:8000/api/v1/bill-statements \
  -H "Content-Type: application/json" \
  -d '{
    "name": "BCA CC Jan 2026",
    "payment_method_id": "{payment_method_id}",
    "statement_date": "2026-01-15T00:00:00Z",
    "due_date": "2026-02-10T00:00:00Z",
    "description": "January 2026 statement"
  }'
```

### List All Bill Statements

```bash
curl http://localhost:8000/api/v1/bill-statements
```
