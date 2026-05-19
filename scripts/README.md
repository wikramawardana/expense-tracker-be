# Scripts

## `migrate_from_sheet.py`

One-shot importer for the legacy expense-tracker spreadsheet
([Google Sheet](https://docs.google.com/spreadsheets/d/1lMIdzhvUBHcOVdXnYTFExT4EeQ8-s89h9NuffplSTuQ/edit)).

A snapshot of the sheet (CSV export) is committed alongside the script at
`scripts/data/sheet.csv` so the migration is fully reproducible.

### What it does

1. **Bootstraps** the four recurrence types (`none`, `installment`,
   `subscription`, `recurring`), nine payment methods, and eight
   categories — idempotently, by name.
2. **Pre-creates** the monthly bill statements that the dated rows need
   (`April 2026`, `May 2026`, ...).
3. **Imports** every row from the sheet:
   - Recurring / installment / subscription rows are POSTed one at a time
     so the API's auto-generation of future months kicks in.
   - One-time dated rows are POSTed via `/expenses/bulk` in batches of 25.
   - Status (`paid` / `unpaid`) is patched in via `PUT /expenses/:id`
     after creation, since `CreateExpenseRequest` doesn't accept a status.

### Migration assumptions (per the agreed plan)

| Topic | Decision |
| --- | --- |
| Start date for recurring/installment/subscription rows | `2026-05-01T00:00:00Z` (first day of current month) |
| Backfilling past installments | **No** — `mobil 31/96` starts at month 31, prior months are not created |
| End date for `subscription` / `recurring` (no fixed count) | `2030-12-01T00:00:00Z` |
| THB conversion (`grab bangkok 907 THB`) | `Rp 469 / THB` → `Rp 425,383`, with a note in the description |
| `MBAINTAN` vs `MBAKINTAN` | Canonicalized to `MBAKINTAN` (same person) |
| Status mapping | `PAID` → `paid`, `UNPAID` → `unpaid`, blank → `pending` |
| `paid_by` | Free text, passed through unchanged |
| Categories | Auto-assigned by keyword match on title + description (8 categories, no row falls into "Other") |

### Usage

```bash
# 1. Dry run — prints every payload, hits no API
python3 scripts/migrate_from_sheet.py \
    --csv scripts/data/sheet.csv \
    --dry-run \
    --save-plan /tmp/plan.json

# 2. Bootstrap only (entities + bill statements, no expense rows)
python3 scripts/migrate_from_sheet.py \
    --csv scripts/data/sheet.csv \
    --base-url https://api-expensetracker.wikra.cloud/api/v1 \
    --token "$EXPENSE_TRACKER_TOKEN" \
    --bootstrap-only

# 3. Live run (reads the same sheet snapshot, posts everything)
python3 scripts/migrate_from_sheet.py \
    --csv scripts/data/sheet.csv \
    --base-url https://api-expensetracker.wikra.cloud/api/v1 \
    --token "$EXPENSE_TRACKER_TOKEN"
```

### Idempotency & re-runs

- **Bootstrap entities** (recurrence types, payment methods, categories,
  bill statements) are looked up by name before creating, so re-running
  the bootstrap is safe.
- **Expense rows are NOT deduplicated** — re-running the expense phase
  will create duplicates. Only run the expense phase **once**. If
  something fails midway, fix the cause, then either resume manually
  (the script doesn't have native resume) or wipe the partially-imported
  expenses by `recurrence_group_id` / date range and re-run.

### Verifying after the run

After the live run completes, sanity-check via the FE or the API:

| Expectation | Why |
| --- | --- |
| 9 payment methods exist | Bootstrap |
| 8 categories exist | Bootstrap |
| 4 recurrence types exist | Bootstrap |
| Bill statements exist for every month from `May 2026` through `October 2031` | The longest installment (`mobil 31/96`) auto-generates 66 future months |
| `bill_statements` for `April 2026` and `May 2026` contain the 156 dated rows | Direct imports |
| ~33 entries with `recurrence_group_id` (the 27 installments + 3 recurring + 3 subscriptions are roots; auto-generated children share the same group_id) | Recurring section |
| `grab bangkok` exists with `Rp 425,383` and a note about THB conversion | Currency conversion |
