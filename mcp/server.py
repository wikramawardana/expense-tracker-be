from __future__ import annotations

import base64
import calendar
import json
import os
import re
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

TABLES = {
    "categories": "categories",
    "payment_methods": "payment_methods",
    "bill_statements": "bill_statements",
    "recurrence_types": "recurrence_types",
    "expenses": "expenses",
}

ALLOWED_STATUSES = {"pending", "unpaid", "paid"}

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
    if not isinstance(records, list):
        raise ToolError(f"Expected list result, got: {records}")
    return [public_record(record) for record in records if isinstance(record, dict)]


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
        "recurrence_types": active_records("recurrence_types"),
    }


@mcp.tool()
def list_expense_context() -> dict[str, Any]:
    """List active categories, payment methods, bill statements, and recurrence types before creating expenses."""
    try:
        return ok(context_payload())
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
        expense = create_record(
            "expenses",
            {
                "title": title,
                "amount": float(amount),
                "payment_method": payment_record["name"],
                "payment_method_id": payment_record["id"],
                "expense_date": expense_iso,
                "description": clean_optional(description),
                "status": status,
                "bill_statement": bill_record["name"],
                "bill_statement_id": bill_record["id"],
                "category_id": category_record["id"],
                "paid_by": clean_optional(paid_by),
                "recurrence_type": None,
                "recurrence_type_id": None,
                "recurrence_count": None,
                "recurrence_current": None,
                "recurrence_total_amount": None,
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
