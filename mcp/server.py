from __future__ import annotations

import base64
import calendar
import email
from email.header import decode_header
from html.parser import HTMLParser
import imaplib
import json
import os
import re
import ssl
import sys
import uuid
from datetime import date, datetime, timedelta, timezone
from typing import Any
from urllib import error, request
from zoneinfo import ZoneInfo

from mcp.server.fastmcp import FastMCP


DEFAULT_ENV_FILE = "/home/wikra/production-projects/expense-tracker/.env.backend"
DEFAULT_SURREAL_HTTP_URL = "http://127.0.0.1:8001/sql"
DEFAULT_TIMEZONE = "Asia/Jakarta"
DEFAULT_OWNER_ID = "AatkLFL4lb6ogZzx1Q4X5We04Icwmuz0"


TABLES = {
    "categories": "categories",
    "payment_methods": "payment_methods",
    "bill_statements": "bill_statements",
    "expenses": "expenses",
}

ALLOWED_STATUSES = {"pending", "unpaid", "paid"}
STATUS_STORAGE_VALUES = {
    "pending": "Pending",
    "unpaid": "Unpaid",
    "paid": "Paid",
}
STORAGE_STATUS_VALUES = {value.casefold(): key for key, value in STATUS_STORAGE_VALUES.items()}
DELETE_ALL_CONFIRMATION = "DELETE ALL EXPENSES"

mcp = FastMCP("expense-tracker")


class ToolError(Exception):
    pass


def load_env_file(path: str) -> None:
    if not path or not os.path.exists(path):
        return

    with open(path, "r", encoding="utf-8") as env_file:
        for raw_line in env_file:
            line = raw_line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            if line.startswith("export "):
                line = line[len("export ") :]
            key, value = line.split("=", 1)
            key = key.strip()
            value = value.strip().strip("'").strip('"')
            if key and key not in os.environ:
                os.environ[key] = value


load_env_file(os.getenv("EXPENSE_TRACKER_ENV_FILE", DEFAULT_ENV_FILE))


def local_tz() -> ZoneInfo:
    tz_name = os.getenv("EXPENSE_TRACKER_TIMEZONE", DEFAULT_TIMEZONE)
    try:
        return ZoneInfo(tz_name)
    except Exception:
        return ZoneInfo(DEFAULT_TIMEZONE)


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def clean_optional(value: str | None) -> str | None:
    if value is None:
        return None
    value = value.strip()
    return value or None


def sql_record_id(table: str, record_id: str) -> str:
    clean_id = plain_id(record_id)
    if not clean_id or not re.fullmatch(r"[A-Za-z0-9_.:-]+", clean_id):
        raise ToolError(f"Invalid {table} id: {record_id}")
    return f"{table}:`{clean_id}`"


def sql_literal(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return str(value)
    if value is None:
        return "NONE"
    return json.dumps(str(value), ensure_ascii=False)


def db_config() -> dict[str, str]:
    config = {
        "url": os.getenv("EXPENSE_TRACKER_SURREAL_HTTP_URL", DEFAULT_SURREAL_HTTP_URL),
        "ns": os.getenv("SURREAL_DB_NS", ""),
        "db": os.getenv("SURREAL_DB_DB", ""),
        "user": os.getenv("SURREAL_DB_USER", ""),
        "password": os.getenv("SURREAL_DB_PASS", ""),
    }
    missing = [key for key, value in config.items() if not value]
    if missing:
        raise ToolError(f"Missing SurrealDB config: {', '.join(missing)}")
    return config


def surreal_query(sql: str) -> list[Any]:
    config = db_config()
    credentials = f"{config['user']}:{config['password']}".encode("utf-8")
    auth = base64.b64encode(credentials).decode("ascii")

    req = request.Request(
        config["url"],
        data=sql.encode("utf-8"),
        method="POST",
        headers={
            "Authorization": f"Basic {auth}",
            "Surreal-NS": config["ns"],
            "Surreal-DB": config["db"],
            "Accept": "application/json",
            "Content-Type": "application/surrealql",
        },
    )

    try:
        with request.urlopen(req, timeout=30) as response:
            payload = response.read().decode("utf-8")
    except error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise ToolError(f"SurrealDB HTTP {exc.code}: {detail}") from exc
    except error.URLError as exc:
        raise ToolError(f"Cannot reach SurrealDB: {exc.reason}") from exc

    try:
        data = json.loads(payload)
    except json.JSONDecodeError as exc:
        raise ToolError(f"SurrealDB returned non-JSON response: {payload[:200]}") from exc

    if not isinstance(data, list):
        raise ToolError(f"Unexpected SurrealDB response: {data}")

    results: list[Any] = []
    for item in data:
        if not isinstance(item, dict):
            raise ToolError(f"Unexpected SurrealDB result item: {item}")
        if item.get("status") != "OK":
            raise ToolError(str(item.get("result") or item))
        results.append(item.get("result"))
    return results


def plain_id(raw_id: Any) -> str:
    if raw_id is None:
        return ""
    if isinstance(raw_id, str):
        value = raw_id.strip()
        if ":" in value:
            value = value.split(":", 1)[1]
        return value.strip("`").strip("⟨⟩")
    if isinstance(raw_id, dict):
        for key in ("id", "key", "value"):
            if key in raw_id:
                return plain_id(raw_id[key])
    return str(raw_id)


def public_record(record: dict[str, Any]) -> dict[str, Any]:
    item = dict(record)
    item["id"] = plain_id(item.get("id"))
    if "status" in item and isinstance(item["status"], str):
        item["status"] = STORAGE_STATUS_VALUES.get(item["status"].casefold(), item["status"])
    return item


def strip_none_values(value: Any) -> Any:
    if isinstance(value, dict):
        return {
            key: strip_none_values(item)
            for key, item in value.items()
            if item is not None
        }
    if isinstance(value, list):
        return [strip_none_values(item) for item in value]
    return value


def simplify_records(records: Any) -> list[dict[str, Any]]:
    if not records:
        return []
    if isinstance(records, dict):
        return [public_record(records)]
    if not isinstance(records, list):
        raise ToolError(f"Expected list result, got: {records}")
    return [public_record(record) for record in records if isinstance(record, dict)]


def clamp_limit(limit: int, *, default: int = 20, maximum: int = 100) -> int:
    try:
        value = int(limit)
    except (TypeError, ValueError):
        value = default
    return max(1, min(value, maximum))


def clamp_offset(offset: int) -> int:
    try:
        value = int(offset)
    except (TypeError, ValueError):
        value = 0
    return max(0, value)


def active_records(table: str) -> list[dict[str, Any]]:
    if table not in TABLES:
        raise ToolError(f"Unknown table: {table}")
    sql = f"SELECT * FROM {table} WHERE is_active = true ORDER BY name ASC;"
    return simplify_records(surreal_query(sql)[0])


def get_record(table: str, record_id: str) -> dict[str, Any] | None:
    sql = f"SELECT * FROM {sql_record_id(table, record_id)};"
    records = simplify_records(surreal_query(sql)[0])
    return records[0] if records else None


def normalized_name(value: str | None) -> str:
    return re.sub(r"\s+", " ", value or "").strip().casefold()


def resolve_record(
    table: str,
    *,
    record_id: str | None = None,
    name: str | None = None,
    required_label: str,
) -> dict[str, Any]:
    record_id = clean_optional(record_id)
    name = clean_optional(name)

    if record_id:
        record = get_record(table, record_id)
        if record:
            return record
        raise ToolError(f"{required_label} id not found: {record_id}")

    if not name:
        raise ToolError(f"{required_label} name or id is required")

    matches = [
        record
        for record in active_records(table)
        if normalized_name(record.get("name")) == normalized_name(name)
    ]
    if len(matches) == 1:
        return matches[0]
    if len(matches) > 1:
        raise ToolError(f"{required_label} name is ambiguous: {name}")
    raise ToolError(f"{required_label} not found: {name}")


def create_record(table: str, payload: dict[str, Any]) -> dict[str, Any]:
    record_id = str(uuid.uuid4())
    body = json.dumps(strip_none_values(payload), ensure_ascii=False, separators=(",", ":"))
    sql = f"CREATE {sql_record_id(table, record_id)} CONTENT {body};"
    result = surreal_query(sql)[0]
    records = simplify_records(result)
    if not records:
        raise ToolError(f"Could not create record in {table}")
    return records[0]


def parse_expense_date(value: str | None) -> tuple[str, date]:
    value = clean_optional(value)
    tz = local_tz()

    if not value or value.casefold() in {"today", "now"}:
        current_date = datetime.now(tz).date()
        return f"{current_date.isoformat()}T00:00:00.000Z", current_date
    if value.casefold() == "yesterday":
        current_date = datetime.now(tz).date() - timedelta(days=1)
        return f"{current_date.isoformat()}T00:00:00.000Z", current_date
    if value.casefold() == "tomorrow":
        current_date = datetime.now(tz).date() + timedelta(days=1)
        return f"{current_date.isoformat()}T00:00:00.000Z", current_date

    if re.fullmatch(r"\d{4}-\d{2}-\d{2}", value):
        parsed_date = date.fromisoformat(value)
        return f"{parsed_date.isoformat()}T00:00:00.000Z", parsed_date

    iso_value = value.replace("Z", "+00:00")
    try:
        parsed_dt = datetime.fromisoformat(iso_value)
    except ValueError as exc:
        raise ToolError("expense_date must be YYYY-MM-DD, ISO datetime, today, yesterday, or tomorrow") from exc

    if parsed_dt.tzinfo is None:
        parsed_dt = parsed_dt.replace(tzinfo=tz)
    local_date = parsed_dt.astimezone(tz).date()
    utc_dt = parsed_dt.astimezone(timezone.utc)
    return utc_dt.isoformat(timespec="milliseconds").replace("+00:00", "Z"), local_date


def parse_expense_date_end(value: str | None) -> tuple[str, date]:
    value = clean_optional(value)
    expense_iso, expense_day = parse_expense_date(value)
    is_whole_day = (
        not value
        or value.casefold() in {"today", "yesterday", "tomorrow"}
        or re.fullmatch(r"\d{4}-\d{2}-\d{2}", value)
    )
    if is_whole_day:
        return f"{expense_day.isoformat()}T23:59:59.999Z", expense_day
    return expense_iso, expense_day


def month_statement_name(expense_day: date) -> str:
    return f"{calendar.month_name[expense_day.month]} {expense_day.year}"


def month_statement_date(expense_day: date) -> str:
    return f"{expense_day.year:04d}-{expense_day.month:02d}-01T00:00:00.000Z"


def ensure_bill_statement(
    *,
    bill_statement_id: str | None,
    bill_statement: str | None,
    expense_day: date,
    payment_method_id: str | None,
    auto_create: bool,
) -> dict[str, Any]:
    if clean_optional(bill_statement_id):
        return resolve_record(
            "bill_statements",
            record_id=bill_statement_id,
            required_label="Bill statement",
        )

    requested_name = clean_optional(bill_statement) or month_statement_name(expense_day)
    matches = [
        record
        for record in active_records("bill_statements")
        if normalized_name(record.get("name")) == normalized_name(requested_name)
    ]
    if len(matches) == 1:
        return matches[0]
    if len(matches) > 1:
        raise ToolError(f"Bill statement name is ambiguous: {requested_name}")
    if not auto_create:
        raise ToolError(f"Bill statement not found: {requested_name}")

    created_at = now_iso()
    return create_record(
        "bill_statements",
        {
            "name": requested_name,
            "payment_method_id": payment_method_id,
            "statement_date": month_statement_date(expense_day),
            "due_date": None,
            "description": "Created by Expense Tracker MCP",
            "is_active": True,
            "created_at": created_at,
            "updated_at": created_at,
        },
    )


def ok(data: dict[str, Any]) -> dict[str, Any]:
    return {"ok": True, **data}


def fail(exc: Exception) -> dict[str, Any]:
    return {"ok": False, "error": str(exc)}


def context_payload() -> dict[str, list[dict[str, Any]]]:
    return {
        "categories": active_records("categories"),
        "payment_methods": active_records("payment_methods"),
        "bill_statements": active_records("bill_statements"),
    }


def normalize_status_filter(status: str | None) -> str | None:
    status = clean_optional(status)
    if not status:
        return None
    status_key = status.casefold()
    if status_key not in ALLOWED_STATUSES:
        raise ToolError("Status must be pending, unpaid, or paid")
    return STATUS_STORAGE_VALUES[status_key]


def expense_filter_conditions(
    *,
    date_from: str | None = None,
    date_to: str | None = None,
    status: str | None = None,
    bill_statement: str | None = None,
    bill_statement_id: str | None = None,
    category: str | None = None,
    category_id: str | None = None,
    payment_method: str | None = None,
    payment_method_id: str | None = None,
    paid_by: str | None = None,
) -> list[str]:
    conditions: list[str] = []

    if clean_optional(date_from):
        date_from_iso, _ = parse_expense_date(date_from)
        conditions.append(f"expense_date >= {sql_literal(date_from_iso)}")
    if clean_optional(date_to):
        date_to_iso, _ = parse_expense_date_end(date_to)
        conditions.append(f"expense_date <= {sql_literal(date_to_iso)}")

    status_storage = normalize_status_filter(status)
    if status_storage:
        status_lower = STORAGE_STATUS_VALUES.get(status_storage.casefold(), status_storage.casefold())
        conditions.append(
            f"(status = {sql_literal(status_storage)} OR status = {sql_literal(status_lower)})"
        )

    if clean_optional(bill_statement_id):
        bill_record = resolve_record(
            "bill_statements",
            record_id=bill_statement_id,
            required_label="Bill statement",
        )
        conditions.append(f"bill_statement_id = {sql_literal(bill_record['id'])}")
    elif clean_optional(bill_statement):
        bill_record = resolve_record(
            "bill_statements",
            name=bill_statement,
            required_label="Bill statement",
        )
        conditions.append(f"bill_statement_id = {sql_literal(bill_record['id'])}")

    if clean_optional(category_id):
        category_record = resolve_record(
            "categories",
            record_id=category_id,
            required_label="Category",
        )
        conditions.append(f"category_id = {sql_literal(category_record['id'])}")
    elif clean_optional(category):
        category_record = resolve_record(
            "categories",
            name=category,
            required_label="Category",
        )
        conditions.append(f"category_id = {sql_literal(category_record['id'])}")

    if clean_optional(payment_method_id):
        payment_record = resolve_record(
            "payment_methods",
            record_id=payment_method_id,
            required_label="Payment method",
        )
        conditions.append(f"payment_method_id = {sql_literal(payment_record['id'])}")
    elif clean_optional(payment_method):
        payment_record = resolve_record(
            "payment_methods",
            name=payment_method,
            required_label="Payment method",
        )
        conditions.append(f"payment_method = {sql_literal(payment_record['name'])}")

    if clean_optional(paid_by):
        conditions.append(f"paid_by = {sql_literal(clean_optional(paid_by))}")

    return conditions


def count_result(result: Any) -> int:
    if isinstance(result, list) and result:
        result = result[0]
    if isinstance(result, dict):
        try:
            return int(result.get("count") or 0)
        except (TypeError, ValueError):
            return 0
    return 0


@mcp.tool()
def list_expense_context() -> dict[str, Any]:
    """List active categories, payment methods, and bill statements before creating expenses."""
    try:
        return ok(context_payload())
    except Exception as exc:
        return fail(exc)


@mcp.tool()
def list_expenses(
    limit: int = 20,
    offset: int = 0,
    date_from: str | None = None,
    date_to: str | None = None,
    status: str | None = None,
    bill_statement: str | None = None,
    bill_statement_id: str | None = None,
    category: str | None = None,
    category_id: str | None = None,
    payment_method: str | None = None,
    payment_method_id: str | None = None,
    paid_by: str | None = None,
) -> dict[str, Any]:
    """List expenses with optional filters. Dates accept YYYY-MM-DD, ISO datetime, today, yesterday, or tomorrow."""
    try:
        limit = clamp_limit(limit)
        offset = clamp_offset(offset)
        conditions = expense_filter_conditions(
            date_from=date_from,
            date_to=date_to,
            status=status,
            bill_statement=bill_statement,
            bill_statement_id=bill_statement_id,
            category=category,
            category_id=category_id,
            payment_method=payment_method,
            payment_method_id=payment_method_id,
            paid_by=paid_by,
        )
        where_clause = f"WHERE {' AND '.join(conditions)}" if conditions else ""
        results = surreal_query(
            "\n".join(
                [
                    f"SELECT * FROM expenses {where_clause} ORDER BY expense_date DESC, created_at DESC LIMIT {limit} START {offset};",
                    f"SELECT count() FROM expenses {where_clause} GROUP ALL;",
                ]
            )
        )
        return ok(
            {
                "expenses": simplify_records(results[0]),
                "total": count_result(results[1]),
                "limit": limit,
                "offset": offset,
            }
        )
    except Exception as exc:
        return fail(exc)


@mcp.tool()
def get_expenses_today(
    limit: int = 50,
    status: str | None = None,
    paid_by: str | None = None,
) -> dict[str, Any]:
    """List today's expenses using the configured local timezone."""
    try:
        today = datetime.now(local_tz()).date().isoformat()
        return list_expenses(
            limit=limit,
            date_from=today,
            date_to=today,
            status=status,
            paid_by=paid_by,
        )
    except Exception as exc:
        return fail(exc)


@mcp.tool()
def delete_expense(expense_id: str) -> dict[str, Any]:
    """Delete one expense by id. Use list_expenses first if the id is unknown."""
    try:
        expense_id = clean_optional(expense_id)
        if not expense_id:
            raise ToolError("expense_id is required")

        result = surreal_query(
            f"DELETE FROM expenses WHERE id = {sql_record_id('expenses', expense_id)} RETURN BEFORE;"
        )[0]
        deleted = simplify_records(result)
        if not deleted:
            raise ToolError(f"Expense not found: {expense_id}")
        return ok({"deleted_count": len(deleted), "deleted": deleted})
    except Exception as exc:
        return fail(exc)


@mcp.tool()
def delete_all_expenses(confirm: str) -> dict[str, Any]:
    """Delete every expense transaction. The exact confirm value must be DELETE ALL EXPENSES."""
    try:
        if clean_optional(confirm) != DELETE_ALL_CONFIRMATION:
            raise ToolError(f'To delete all expenses, pass confirm="{DELETE_ALL_CONFIRMATION}"')

        result = surreal_query("DELETE FROM expenses RETURN BEFORE;")[0]
        deleted = simplify_records(result)
        return ok(
            {
                "deleted_count": len(deleted),
                "sample": deleted[:10],
                "note": "Deleted expenses only. Categories, payment methods, and bill statements were kept.",
            }
        )
    except Exception as exc:
        return fail(exc)


@mcp.tool()
def create_category(
    name: str,
    icon: str | None = None,
    color: str | None = None,
    description: str | None = None,
) -> dict[str, Any]:
    """Create an active expense category. Use this when a requested category is missing."""
    try:
        name = clean_optional(name)
        if not name:
            raise ToolError("Category name is required")

        for record in active_records("categories"):
            if normalized_name(record.get("name")) == normalized_name(name):
                return ok({"category": record, "created": False})

        created_at = now_iso()
        category = create_record(
            "categories",
            {
                "name": name,
                "icon": clean_optional(icon),
                "color": clean_optional(color) or "#4B5563",
                "description": clean_optional(description),
                "is_active": True,
                "created_at": created_at,
                "updated_at": created_at,
            },
        )
        return ok({"category": category, "created": True})
    except Exception as exc:
        return fail(exc)


@mcp.tool()
def create_payment_method(
    name: str,
    method_type: str = "other",
    description: str | None = None,
) -> dict[str, Any]:
    """Create an active payment method, such as Cash, BCA Credit Card, or Digibank."""
    try:
        name = clean_optional(name)
        method_type = clean_optional(method_type) or "other"
        if not name:
            raise ToolError("Payment method name is required")

        for record in active_records("payment_methods"):
            if normalized_name(record.get("name")) == normalized_name(name):
                return ok({"payment_method": record, "created": False})

        created_at = now_iso()
        payment_method = create_record(
            "payment_methods",
            {
                "name": name,
                "method_type": method_type,
                "description": clean_optional(description),
                "is_active": True,
                "created_at": created_at,
                "updated_at": created_at,
            },
        )
        return ok({"payment_method": payment_method, "created": True})
    except Exception as exc:
        return fail(exc)


@mcp.tool()
def create_bill_statement(
    name: str | None = None,
    payment_method: str | None = None,
    payment_method_id: str | None = None,
    statement_date: str | None = None,
    due_date: str | None = None,
    description: str | None = None,
) -> dict[str, Any]:
    """Create an active bill statement. If name is omitted, it uses the month from statement_date or today."""
    try:
        statement_iso, statement_day = parse_expense_date(statement_date)
        requested_name = clean_optional(name) or month_statement_name(statement_day)

        for record in active_records("bill_statements"):
            if normalized_name(record.get("name")) == normalized_name(requested_name):
                return ok({"bill_statement": record, "created": False})

        resolved_payment_method_id = clean_optional(payment_method_id)
        if not resolved_payment_method_id and clean_optional(payment_method):
            payment_record = resolve_record(
                "payment_methods",
                name=payment_method,
                required_label="Payment method",
            )
            resolved_payment_method_id = payment_record["id"]

        created_at = now_iso()
        bill_statement_record = create_record(
            "bill_statements",
            {
                "name": requested_name,
                "payment_method_id": resolved_payment_method_id,
                "statement_date": statement_iso,
                "due_date": clean_optional(due_date),
                "description": clean_optional(description),
                "is_active": True,
                "created_at": created_at,
                "updated_at": created_at,
            },
        )
        return ok({"bill_statement": bill_statement_record, "created": True})
    except Exception as exc:
        return fail(exc)


@mcp.tool()
def create_expense(
    title: str,
    amount: float,
    category: str | None = None,
    category_id: str | None = None,
    payment_method: str | None = None,
    payment_method_id: str | None = None,
    bill_statement: str | None = None,
    bill_statement_id: str | None = None,
    expense_date: str | None = None,
    description: str | None = None,
    paid_by: str | None = None,
    status: str = "pending",
    auto_create_bill_statement: bool = True,
) -> dict[str, Any]:
    """Create one expense. Resolve category/payment by exact name or id. Missing bill statement can be auto-created for the expense month."""
    try:
        title = clean_optional(title)
        if not title:
            raise ToolError("Expense title is required")
        if amount <= 0:
            raise ToolError("Amount must be greater than 0")

        status = (clean_optional(status) or "pending").casefold()
        if status not in ALLOWED_STATUSES:
            raise ToolError("Status must be pending, unpaid, or paid")
        status_storage = STATUS_STORAGE_VALUES[status]

        category_record = resolve_record(
            "categories",
            record_id=category_id,
            name=category,
            required_label="Category",
        )
        payment_record = resolve_record(
            "payment_methods",
            record_id=payment_method_id,
            name=payment_method,
            required_label="Payment method",
        )
        expense_iso, expense_day = parse_expense_date(expense_date)
        bill_record = ensure_bill_statement(
            bill_statement_id=bill_statement_id,
            bill_statement=bill_statement,
            expense_day=expense_day,
            payment_method_id=payment_record["id"],
            auto_create=auto_create_bill_statement,
        )

        created_at = now_iso()
        owner_id = os.getenv("EXPENSE_OWNER_ID", DEFAULT_OWNER_ID)
        expense = create_record(
            "expenses",
            {
                "owner_id": owner_id,
                "title": title,
                "amount": float(amount),
                "payment_method": payment_record["name"],
                "payment_method_id": payment_record["id"],
                "expense_date": expense_iso,
                "description": clean_optional(description),
                "status": status_storage,
                "bill_statement": bill_record["name"],
                "bill_statement_id": bill_record["id"],
                "category_id": category_record["id"],
                "paid_by": clean_optional(paid_by),
                "recurrence_type": None,
                "recurrence_type_id": None,
                "recurrence_count": None,
                "recurrence_current": None,
                "recurrence_end_date": None,
                "recurrence_group_id": None,
                "created_at": created_at,
                "updated_at": created_at,
            },
        )

        return ok(
            {
                "expense": expense,
                "category": category_record,
                "payment_method": payment_record,
                "bill_statement": bill_record,
            }
        )
    except Exception as exc:
        return fail(exc)


class HTMLTextExtractor(HTMLParser):
    def __init__(self):
        super().__init__()
        self.result = []

    def handle_data(self, d):
        self.result.append(d)

    def get_text(self):
        raw = " ".join(self.result)
        raw = raw.replace("\xa0", " ")
        return re.sub(r"\s+", " ", raw)


def _parse_idr(raw_str: str) -> float | None:
    if not raw_str:
        return None
    cleaned = re.sub(r"[^\d,\.]", "", raw_str).strip()
    if not cleaned:
        return None
    if "," in cleaned and "." in cleaned:
        cleaned = cleaned.split(",")[0].replace(".", "")
    elif "," in cleaned:
        cleaned = cleaned.split(",")[0]
    else:
        cleaned = cleaned.replace(".", "")
    try:
        return float(cleaned)
    except ValueError:
        return None


KNOWN_BRANDS = {
    "grab": "Grab",
    "grabfood": "GrabFood",
    "shopee": "Shopee",
    "indomart": "Indomaret",
    "indomaret": "Indomaret",
    "pimart": "PIMArt",
    "ucok wr": "Ucok WR",
    "macbook m5": "MacBook M5",
    "ipad": "iPad",
    "iphone 17 pro max": "iPhone 17 Pro Max",
    "casing iphone 17 pro max": "Casing iPhone 17 Pro Max",
    "youtube premium": "YouTube Premium",
    "github copilot": "GitHub Copilot",
    "icloud": "iCloud",
    "capcut": "CapCut",
    "biznet": "Biznet",
    "hostinger": "Hostinger",
    "fithub": "FitHub",
    "ps5": "PS5",
    "tas fjalraven": "Tas Fjällräven",
    "spp ruby": "SPP Ruby",
    "pelunasan mobil xl7": "Pelunasan Mobil XL7",
    "pajak stargazer": "Pajak Stargazer",
    "pajak kedelai": "Pajak Kedelai",
    "booking fee familia urban": "Booking Fee Familia Urban",
    "motor alva one": "Motor ALVA One",
    "card to cash": "Card to Cash",
    "powercash": "Power Cash",
    "kasbon kepin": "Kasbon Kepin",
    "kasbon teteh": "Kasbon Teteh",
    "sekolah ruby": "Sekolah Ruby",
    "warung makan": "Warung Makan",
    "warung": "Warung",
    "pasar": "Pasar",
    "hotel phuket": "Hotel Phuket",
    "phuket trip": "Phuket Trip",
    "sofa": "Sofa",
    "rumah": "Rumah",
    "motor": "Motor",
    "embah": "Embah",
    "birkenstock": "Birkenstock",
}


def normalize_title(title: str) -> str:
    cleaned = title.strip()
    low = cleaned.lower()
    if low in KNOWN_BRANDS:
        return KNOWN_BRANDS[low]
    return " ".join(word.capitalize() for word in cleaned.split())


def _match_category_name(title: str, merchant: str) -> str:
    combined = f"{title} {merchant}".lower()
    if any(k in combined for k in ["grab", "gojek", "bluebird", "taxi", "parkir", "toll"]):
        return "Transportation"
    if any(k in combined for k in ["shopee", "tokopedia", "lazada", "uniqlo", "zara", "blibli"]):
        return "Shopping"
    if any(k in combined for k in ["makan", "warung", "resto", "kopi", "cafe", "coffee", "starbucks", "indomaret", "idm", "alfamart", "pimart", "ucok"]):
        return "Food & Dining"
    return "Shopping"


@mcp.tool()
def sync_bank_expenses(
    date_query: str = "today",
    bank: str = "all",
    dry_run: bool = False,
) -> dict[str, Any]:
    """Sync transaction notification emails from BCA, BNI, and Mandiri from Gmail into SpendCTRL expenses.
    Supports date_query='today', 'yesterday', 'YYYY-MM-DD', or 'all'.
    """
    try:
        user = os.getenv("GMAIL_BANK_USER")
        password = (os.getenv("GMAIL_BANK_APP_PASSWORD") or "").replace(" ", "")

        if not user or not password:
            raise ToolError("Missing GMAIL_BANK_USER or GMAIL_BANK_APP_PASSWORD in environment")

        # Resolve target dates
        today_str = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        if not date_query or date_query.lower() == "today":
            target_dates = {today_str}
        elif date_query.lower() == "yesterday":
            target_dates = {(datetime.now(timezone.utc) - timedelta(days=1)).strftime("%Y-%m-%d")}
        elif date_query.lower() in ["all", "any"]:
            target_dates = set()
        else:
            try:
                dt = datetime.strptime(date_query, "%Y-%m-%d")
                target_dates = {dt.strftime("%Y-%m-%d")}
            except ValueError:
                target_dates = {today_str}

        try:
            import certifi
            ctx = ssl.create_default_context(cafile=certifi.where())
        except ImportError:
            ctx = ssl._create_unverified_context()

        mail = imaplib.IMAP4_SSL("imap.gmail.com", 993, ssl_context=ctx)
        mail.login(user, password)
        mail.select("INBOX", readonly=True)

        if bank == "bca":
            query = '(FROM "KartuKreditBCA@klikbca.com" SUBJECT "Credit Card Transaction Notification")'
        elif bank == "bni":
            query = '(FROM "bnicreditcard@bni.co.id")'
        elif bank == "mandiri":
            query = '(FROM "mandiri")'
        else:
            query = '(OR (FROM "KartuKreditBCA@klikbca.com") (OR (FROM "bnicreditcard@bni.co.id") (FROM "mandiri")))'

        status, msg_nums = mail.search(None, query)
        if status != "OK" or not msg_nums[0]:
            return ok({"found_total": 0, "new_count": 0, "skipped_duplicates": 0, "ingested": []})

        msg_ids = msg_nums[0].split()
        parsed_txs = []

        for msg_id in msg_ids[-30:]:
            _, msg_data = mail.fetch(msg_id, "(RFC822)")
            for part in msg_data:
                if isinstance(part, tuple):
                    msg = email.message_from_bytes(part[1])
                    body_text = ""
                    for sub in msg.walk():
                        ctype = sub.get_content_type()
                        if ctype == "text/html":
                            payload = sub.get_payload(decode=True)
                            if payload:
                                parser = HTMLTextExtractor()
                                parser.feed(payload.decode("utf-8", errors="ignore"))
                                body_text = parser.get_text()
                                break
                        elif ctype == "text/plain" and not body_text:
                            payload = sub.get_payload(decode=True)
                            if payload:
                                body_text = payload.decode("utf-8", errors="ignore")

                    # Parse BCA
                    if "Pemegang Kartu Kredit BCA" in body_text or "KartuKreditBCA" in body_text:
                        card_m = re.search(r"Nomor Kartu\s*:\s*([0-9Xx]+)", body_text, re.I)
                        merch_m = re.search(r"Merchant\s*/\s*ATM\s*:\s*([^:]+?)(?=\s*Jenis Transaksi|\s*Otentikasi|\s*Pada Tanggal|$)", body_text, re.I)
                        date_m = re.search(r"Pada Tanggal\s*:\s*([0-9]{2}-[0-9]{2}-[0-9]{4}\s+[0-9]{2}:[0-9]{2}:[0-9]{2})", body_text, re.I)
                        amt_m = re.search(r"Sejumlah\s*:\s*(Rp\s*[0-9\.,]+)", body_text, re.I)
                        if amt_m and date_m:
                            amt = _parse_idr(amt_m.group(1))
                            try:
                                exp_date = datetime.strptime(date_m.group(1).strip(), "%d-%m-%Y %H:%M:%S").strftime("%Y-%m-%d")
                            except Exception:
                                exp_date = today_str
                            merch = merch_m.group(1).strip() if merch_m else "BCA Transaction"
                            title = "Grab" if merch.upper().startswith("GRAB") else ("Shopee" if "SHOPEE" in merch.upper() else merch)
                            title = normalize_title(title)
                            last4 = card_m.group(1)[-4:] if card_m else "3888"
                            if amt and (not target_dates or exp_date in target_dates):
                                parsed_txs.append({
                                    "bank": "BCA",
                                    "title": title,
                                    "amount": amt,
                                    "expense_date": exp_date,
                                    "payment_method": "BCA KrisFlyer",
                                    "category": _match_category_name(title, merch),
                                    "description": f"BCA Credit Card (..{last4}) at {merch}",
                                    "paid_by": "Wikra",
                                })

                    # Parse BNI
                    elif "Kartu Kredit BNI" in body_text or "bnicreditcard" in body_text:
                        merch_m = re.search(r"Nama Merchant\s*:\s*([^:]+?)(?=\s*Nominal Transaksi|\s*Tanggal Transaksi|$)", body_text, re.I)
                        amt_m = re.search(r"Nominal Transaksi\s*:\s*(Rp\s*[0-9\.,]+)", body_text, re.I)
                        date_m = re.search(r"Tanggal Transaksi\s*:\s*([0-9]{2}/[0-9]{2}/[0-9]{4}\s+[0-9]{2}:[0-9]{2})", body_text, re.I)
                        card_m = re.search(r"Nomor Kartu Kredit BNI\s*:\s*([A-Za-z0-9Xx]+)", body_text, re.I)
                        if amt_m and date_m:
                            amt = _parse_idr(amt_m.group(1))
                            try:
                                exp_date = datetime.strptime(date_m.group(1).strip(), "%d/%m/%Y %H:%M").strftime("%Y-%m-%d")
                            except Exception:
                                exp_date = today_str
                            merch = merch_m.group(1).strip() if merch_m else "BNI Transaction"
                            title = merch[5:].strip() if merch.startswith("QRIS-") else merch
                            title = normalize_title(title)
                            card = card_m.group(1) if card_m else "BNI"
                            if amt and (not target_dates or exp_date in target_dates):
                                parsed_txs.append({
                                    "bank": "BNI",
                                    "title": title,
                                    "amount": amt,
                                    "expense_date": exp_date,
                                    "payment_method": "BNI Mastercard World",
                                    "category": _match_category_name(title, merch),
                                    "description": f"BNI Credit Card ({card}) at {merch}",
                                    "paid_by": "Wikra",
                                })

                    # Parse Mandiri
                    elif "mandiri" in body_text.lower() or "livin" in body_text.lower():
                        penerima_m = re.search(r"Penerima\s*([^:]+?)(?=\s*Jakarta|\s*Tanggal|$)", body_text, re.I)
                        amt_m = re.search(r"Nominal Transaksi\s*(?:Rp\s*[0-9\.,]+)", body_text, re.I)
                        ref_m = re.search(r"No\.\s*Referensi\s*([0-9A-Za-z]+)", body_text, re.I)
                        sumber_m = re.search(r"Sumber Dana\s*([^:]+?)(?=\s*Simpan Bukti|$)", body_text, re.I)
                        date_m = re.search(r"Tanggal\s*([0-9]{1,2}\s+[A-Za-z]{3}\s+[0-9]{4})", body_text, re.I)
                        if amt_m:
                            amt = _parse_idr(amt_m.group(0).replace("Nominal Transaksi", ""))
                            exp_date = today_str
                            if date_m:
                                try:
                                    exp_date = datetime.strptime(date_m.group(1).strip(), "%d %b %Y").strftime("%Y-%m-%d")
                                except Exception:
                                    pass
                            merch = penerima_m.group(1).strip() if penerima_m else "Mandiri Transaction"
                            title = "Indomaret" if "IDM QRIS" in merch or "INDOMARET" in merch.upper() else merch
                            title = normalize_title(title)
                            sumber = sumber_m.group(1).strip() if sumber_m else "Mandiri"
                            ref_no = ref_m.group(1).strip() if ref_m else ""
                            if amt and (not target_dates or exp_date in target_dates):
                                parsed_txs.append({
                                    "bank": "Mandiri",
                                    "title": title,
                                    "amount": amt,
                                    "expense_date": exp_date,
                                    "payment_method": "Mandiri Marriott Bonvoy" if "Marriott" in sumber or "Marriot" in sumber else sumber,
                                    "category": _match_category_name(title, merch),
                                    "description": f"{sumber} at {merch} (Ref: {ref_no})",
                                    "paid_by": "Wikra",
                                })

        mail.logout()

        # Query existing expenses in SurrealDB to deduplicate
        existing_rows = surreal_query("SELECT expense_date, amount, title FROM expenses;")
        existing_keys = set()
        if existing_rows and isinstance(existing_rows[0], list):
            for row in existing_rows[0]:
                ed = str(row.get("expense_date", ""))[:10]
                am = int(float(row.get("amount", 0)))
                ti = str(row.get("title", "")).lower().strip()[:10]
                existing_keys.add(f"{ed}:{am}:{ti}")

        new_txs = []
        skipped_count = 0
        for tx in parsed_txs:
            k = f"{tx['expense_date'][:10]}:{int(tx['amount'])}:{tx['title'].lower().strip()[:10]}"
            if k in existing_keys:
                skipped_count += 1
            else:
                new_txs.append(tx)
                existing_keys.add(k)  # prevent duplicate within same batch

        ingested_records = []
        if not dry_run:
            for tx in new_txs:
                res = create_expense(
                    title=tx["title"],
                    amount=tx["amount"],
                    expense_date=tx["expense_date"],
                    payment_method=tx["payment_method"],
                    category=tx["category"],
                    description=tx["description"],
                    paid_by=tx["paid_by"],
                    status="pending",
                    auto_create_bill_statement=True,
                )
                if res.get("ok"):
                    ingested_records.append(res["data"]["expense"])

        # Build full summary for today so LLM does NOT need to run any code or additional queries
        today_expenses_res = get_expenses_today()
        today_list = today_expenses_res.get("data", {}).get("expenses", []) if today_expenses_res.get("ok") else []
        
        ctx_res = list_expense_context()
        categories_map = {}
        if ctx_res.get("ok"):
            for c in ctx_res.get("data", {}).get("categories", []):
                categories_map[c["id"]] = c["name"]

        category_summary = {}
        pm_summary = {}
        total_amount = 0.0

        for exp in today_list:
            amt = float(exp.get("amount", 0))
            c_name = categories_map.get(exp.get("category_id"), "General")
            pm = exp.get("payment_method", "Other")
            total_amount += amt
            category_summary[c_name] = category_summary.get(c_name, 0.0) + amt
            pm_summary[pm] = pm_summary.get(pm, 0.0) + amt

        summary_data = {
            "date": today_str,
            "total_idr": total_amount,
            "category_breakdown": [{"category": k, "amount": v} for k, v in sorted(category_summary.items(), key=lambda x: -x[1])],
            "payment_method_breakdown": [{"payment_method": k, "amount": v} for k, v in sorted(pm_summary.items(), key=lambda x: -x[1])],
            "all_expenses_today": [
                {
                    "title": e.get("title"),
                    "amount": e.get("amount"),
                    "category": categories_map.get(e.get("category_id"), "General"),
                    "payment_method": e.get("payment_method"),
                    "description": e.get("description"),
                }
                for e in today_list
            ],
        }

        return ok({
            "status": "success",
            "message": f"Synced {len(new_txs)} new transactions ({skipped_count} skipped duplicates).",
            "query_dates": sorted(list(target_dates)) if target_dates else "all",
            "found_total": len(parsed_txs),
            "new_count": len(new_txs),
            "skipped_duplicates": skipped_count,
            "dry_run": dry_run,
            "ingested_count": len(ingested_records) if not dry_run else 0,
            "today_summary": summary_data,
        })
    except Exception as exc:
        return fail(exc)


@mcp.tool()
def get_today_summary(date_query: str = "today") -> dict[str, Any]:
    """Get the complete expense summary and category breakdown for today or a specific date.
    Use this to get summary tables without querying the database directly.
    """
    try:
        today_str = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        if date_query and date_query.lower() == "yesterday":
            target_date = (datetime.now(timezone.utc) - timedelta(days=1)).strftime("%Y-%m-%d")
        elif date_query and date_query.lower() not in ["today", "all"]:
            target_date = date_query
        else:
            target_date = today_str

        exp_res = list_expenses(expense_date_from=target_date, expense_date_to=target_date, limit=100)
        exp_list = exp_res.get("data", {}).get("expenses", []) if exp_res.get("ok") else []

        ctx_res = list_expense_context()
        categories_map = {}
        if ctx_res.get("ok"):
            for c in ctx_res.get("data", {}).get("categories", []):
                categories_map[c["id"]] = c["name"]

        category_summary = {}
        pm_summary = {}
        total_amount = 0.0

        for exp in exp_list:
            amt = float(exp.get("amount", 0))
            c_name = categories_map.get(exp.get("category_id"), "General")
            pm = exp.get("payment_method", "Other")
            total_amount += amt
            category_summary[c_name] = category_summary.get(c_name, 0.0) + amt
            pm_summary[pm] = pm_summary.get(pm, 0.0) + amt

        return ok({
            "date": target_date,
            "total_idr": total_amount,
            "category_breakdown": [{"category": k, "amount": v} for k, v in sorted(category_summary.items(), key=lambda x: -x[1])],
            "payment_method_breakdown": [{"payment_method": k, "amount": v} for k, v in sorted(pm_summary.items(), key=lambda x: -x[1])],
            "expenses": [
                {
                    "title": e.get("title"),
                    "amount": e.get("amount"),
                    "category": categories_map.get(e.get("category_id"), "General"),
                    "payment_method": e.get("payment_method"),
                    "description": e.get("description"),
                }
                for e in exp_list
            ],
        })
    except Exception as exc:
        return fail(exc)




def self_test() -> None:
    result = list_expense_context()
    if not result.get("ok"):
        raise SystemExit(json.dumps(result, indent=2))
    print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    if "--self-test" in sys.argv:
        self_test()
    else:
        mcp.run()
