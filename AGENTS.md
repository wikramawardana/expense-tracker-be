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
  - Production service: Systemd user service `hermes-gateway-wikrassist-expense.service`.

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

## 5. Deployment & Production Infrastructure
- **VPS Server**: `wikra@72.61.210.144` (accessible via SSH).
- **Production MCP Directory**: `/home/wikra/production-projects/expense-tracker/mcp/server.py`.
- **Systemd User Service**:
  ```bash
  ssh wikra@72.61.210.144 "systemctl --user status hermes-gateway-wikrassist-expense.service"
  ssh wikra@72.61.210.144 "systemctl --user restart hermes-gateway-wikrassist-expense.service"
  ```
- **Scheduled Cron**:
  - Job ID: `cf8c32d745a1` (Daily Bank Sync & Expense Summary at 22:00 WIB / `0 22 * * *`).

## 6. Available Skills
- `.agents/skills/bank-sync`: Runbook for testing, dry-running, and troubleshooting bank email sync.
- `.agents/skills/deploy-vps`: Step-by-step procedure for deploying MCP and BE changes to the production VPS.
