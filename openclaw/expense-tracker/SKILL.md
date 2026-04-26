---
name: expense-tracker
description: "Log expenses to the user's expense tracker from chat (Discord/WhatsApp/etc). Parse natural-language messages like 'spent 3.50 on coffee' or 'today I paid 12 lunch, 5 snack', pick the right category/payment-method/bill-statement, and POST to the expense-tracker HTTP API."
---

# Expense Tracker Skill

Use this skill when the user wants to **record money they've spent** — e.g. "log 3.50 coffee", "I spent $25 on groceries yesterday", "add these to expenses: 12 lunch, 5 snack". Use `curl` to talk to the HTTP API.

## Configuration

The user must set these env vars before this skill works:

- `EXPENSE_TRACKER_BASE_URL` — e.g. `https://expense-api.example.com/api/v1` or `http://localhost:8000/api/v1`
- `EXPENSE_TRACKER_API_KEY` — a bot token minted by the user from the expense-tracker frontend. Starts with `etk_`.

If either is missing, tell the user:
> "I need you to set `EXPENSE_TRACKER_BASE_URL` and `EXPENSE_TRACKER_API_KEY` in my environment. Mint an API key at `<frontend-url>/settings/api-keys` (or `curl -X POST <base>/api-keys -H 'Authorization: Bearer <session>' -d '{\"name\":\"openclaw\"}'`)."

## Canonical Flow

### 1. Load metadata (once per conversation, cache in memory)

Before creating expenses, fetch the lookup tables so you can map user-friendly names ("Food", "Cash", "April 2026") to the UUIDs the API expects:

```bash
curl -s -H "Authorization: Bearer $EXPENSE_TRACKER_API_KEY" \
  "$EXPENSE_TRACKER_BASE_URL/bot/categories" | jq '.data'

curl -s -H "Authorization: Bearer $EXPENSE_TRACKER_API_KEY" \
  "$EXPENSE_TRACKER_BASE_URL/bot/payment-methods" | jq '.data'

curl -s -H "Authorization: Bearer $EXPENSE_TRACKER_API_KEY" \
  "$EXPENSE_TRACKER_BASE_URL/bot/bill-statements" | jq '.data'
```

Keep the resulting `id`s in memory for the conversation. Do not re-fetch for every expense.

### 2. Parse the user's message

Extract one or more expense rows from what they said. For each row you need:

- **title** (required) — a short description, e.g. `"Coffee"`, `"Grab to office"`
- **amount** (required) — number, no currency symbol. Default interpretation is the user's local currency.
- **category_id** — pick the best match from cached categories. Ask if ambiguous and you can't guess.
- **payment_method_id** — pick the best match from cached payment methods. If the user didn't say, ask or default to the most recently used one.
- **bill_statement_id** — pick the bill statement whose `statement_date` range covers the expense_date. If none matches, ask the user to create one first (don't auto-create).
- **expense_date** (required) — RFC3339 timestamp. "today" / "yesterday" / "last monday" → resolve relative to the user's current date. If unspecified, default to today at 00:00 UTC.
- **description** (optional) — anything else the user said about it.
- **paid_by** (optional) — default to the user's name, or "me" if unknown.

### 3. Confirm before committing

Before POSTing, echo back what you parsed so the user can correct it. Example:

> "I'll log these to your expense tracker:
> - Coffee — 3.50 (Food, Cash, today)
> - Lunch — 12.00 (Food, Cash, today)
>
> Reply 👍 to confirm, or tell me what's wrong."

Wait for the 👍 / "yes" / "confirm" reply. Do not POST on ambiguity.

### 4. POST the expense(s)

**Single expense** → use the singular endpoint so the user can still add recurring expenses via the web UI later:

```bash
curl -s -H "Authorization: Bearer $EXPENSE_TRACKER_API_KEY" \
  -H "Content-Type: application/json" \
  -X POST "$EXPENSE_TRACKER_BASE_URL/bot/expenses" \
  -d '{
    "title": "Coffee",
    "amount": 3.50,
    "expense_date": "2026-04-25T00:00:00Z",
    "category_id": "<uuid>",
    "payment_method_id": "<uuid>",
    "bill_statement_id": "<uuid>",
    "paid_by": "wikrama"
  }'
```

**Multiple expenses on the same day** → use the bulk endpoint (single round-trip, atomic at the HTTP level):

```bash
curl -s -H "Authorization: Bearer $EXPENSE_TRACKER_API_KEY" \
  -H "Content-Type: application/json" \
  -X POST "$EXPENSE_TRACKER_BASE_URL/bot/expenses/bulk" \
  -d '{
    "expenses": [
      {"title":"Lunch","amount":12.00,"expense_date":"2026-04-25T00:00:00Z","category_id":"<food>","payment_method_id":"<cash>","bill_statement_id":"<apr26>","paid_by":"wikrama"},
      {"title":"Snack","amount":5.00,"expense_date":"2026-04-25T00:00:00Z","category_id":"<food>","payment_method_id":"<cash>","bill_statement_id":"<apr26>","paid_by":"wikrama"}
    ]
  }'
```

### 5. Report back

On success (HTTP 201), reply with a short confirmation:
> "Logged 2 expenses totaling 17.00. 👍"

On 401, the key is bad/revoked — ask the user to mint a new one.

On 4xx with a validation message (e.g. `"Expense #2: title: Title cannot be empty"`), paraphrase the error and ask for the missing field.

## Edge Cases

- **User says "I spent 20 today"** with no title → ask for a title. Don't invent one.
- **Unknown category** ("Coffee" but no Coffee category) → suggest the closest match (e.g. "Food") and ask. Don't create new categories from this skill — that's a web-UI action.
- **No bill statement covers the date** → ask the user to create one in the web UI. Do not POST without `bill_statement_id` (the API will 400).
- **Amount with currency symbol** ("$3.50", "Rp 15000") → strip the symbol, submit the number.
- **Date in the future** → confirm with the user before submitting. Users rarely mean to log future expenses.

## Optional: Reading Expenses

If the user asks "what did I spend this week" or similar:

```bash
curl -s -H "Authorization: Bearer $EXPENSE_TRACKER_API_KEY" \
  "$EXPENSE_TRACKER_BASE_URL/bot/expenses?start_date=2026-04-20&end_date=2026-04-26&limit=100" | jq
```

Summarize by category or day as appropriate.

## API Reference (abbreviated)

All bot endpoints live under `/api/v1/bot/*` and require `Authorization: Bearer etk_...`.

| Method | Path                      | Purpose                                 |
|--------|---------------------------|-----------------------------------------|
| GET    | `/bot/categories`         | List active categories                  |
| GET    | `/bot/payment-methods`    | List active payment methods             |
| GET    | `/bot/bill-statements`    | List active bill statements             |
| GET    | `/bot/expenses`           | Query expenses (supports pagination)    |
| POST   | `/bot/expenses`           | Create a single expense                 |
| POST   | `/bot/expenses/bulk`      | Create multiple expenses in one call    |

The `CreateExpenseRequest` shape for POST bodies is documented in `src/models/expense.rs` — `title`, `amount`, `expense_date` are required; `category_id`, `payment_method_id`, `bill_statement_id` are required for a successful create even though the struct marks them optional.
