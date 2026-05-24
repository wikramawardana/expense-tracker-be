# Expense Tracker MCP

MCP server for the `wiclaw-expense-tracker` Hermes agent.

Source path:

```text
expense-tracker-be/mcp/server.py
```

Production runtime path:

```text
/home/wikra/production-projects/expense-tracker/mcp/server.py
```

Deployment:

- The backend GitHub Action copies `mcp/server.py` and `mcp/README.md` to the production runtime path on every `main` deploy.
- After copying, the deploy checks Python syntax and restarts `hermes-gateway-wiclaw-expense-tracker.service` when that service is already active.

It connects directly to the production SurrealDB HTTP endpoint from the VPS and loads credentials from:

```text
/home/wikra/production-projects/expense-tracker/.env.backend
```

Available tools:

- `list_expense_context`
- `create_category`
- `create_payment_method`
- `create_bill_statement`
- `create_expense`

Hermes exposes these as `mcp_expense_tracker_<tool_name>` after the server is configured in the profile.
