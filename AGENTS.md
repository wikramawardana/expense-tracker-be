# Expense Tracker Backend - Agent Operational Manual

## 1. Overview & Purpose
- **System**: Personal & family expense tracking engine with automated bank transaction ingestion, multi-account isolation, bill statement management, and AI agent integration (Hermes Agent Discord bot).
- **Core Repositories**:
  - Backend: `expense-tracker-be` (Rust Axum/Actix + SurrealDB + Python MCP)
  - Frontend: `expense-tracker-fe` (Next.js App Router + Tailwind CSS)

## 2. Architecture & Tech Stack
- **API Server**: Rust (`cargo check`, `cargo build`), layered architecture: `handler` → `service` → `repository`.
  - Local port: `8202` (via `SERVER_PORT=8202`)
  - Server host: `127.0.0.1` / `0.0.0.0`
- **Database**:
  - Primary DB: **SurrealDB** (v2.x WebSocket) namespace `expense_tracker`, database `expense_tracker`.
  - Auth DB: PostgreSQL database for session and user authentication.
- **MCP Integration**:
  - File: `mcp/server.py` using FastMCP.
  - Exposes tools for Hermes Agent (`create_expense`, `list_expenses`, `sync_bank_expenses`, `get_today_summary`, `categorize_expense`).
  - Former production service `hermes-gateway-wikrassist-expense.service` was removed on 2026-09-29.

## 3. Core Guidelines & Data Rules
1. **Bank Email Sync Rule**:
   - When transactions are parsed from bank notification emails (BCA, BNI, Mandiri) via `sync_bank_expenses`, **NEVER generate or store synthetic descriptions**.
   - `description` MUST remain `None` / empty to prevent raw merchant/card details from cluttering user views.
2. **Expense Status**:
   - Allowed statuses: `pending`, `unpaid`, `paid`.
   - Stored in lowercase. Bank synced items default to `pending`.
3. **Empty Values Handling**:
   - In API requests/updates, empty strings (`""`) for `description` or `paid_by` indicate clearing the field (stored as `None`). Do NOT ignore empty strings.
4. **Bill Statements**:
   - Credit card expenses automatically link to a `bill_statement` matching `<PaymentMethod> - <Month YYYY>`.

## 4. Development & Verification Commands
- **Rust Backend**:
  ```bash
  cargo check
  cargo test
  cargo fmt --check
  ```
- **MCP Server (Python)**:
  ```bash
  python3 -m py_compile mcp/server.py
  ```

## 5. Deployment Status
- The expense tracker API, web app, and Hermes MCP deployment were retired from the VPS on 2026-09-29. The source repositories remain available.
- Automated bank email sync to Google Sheets remains active in `/Users/wikra/MyProjects/expense-tracker/temporal-sync/` and on the VPS under `temporal-expense-sync-worker.service`.
- The deployment workflow is preserved as `.github/workflows/deploy.yml.disabled` and does not run automatically.

## 6. Available Skills
- `.agents/skills/bank-sync`: Runbook for testing, dry-running, and troubleshooting bank email sync.
- `.agents/skills/deploy-vps`: Step-by-step procedure for deploying MCP and BE changes to the production VPS.
