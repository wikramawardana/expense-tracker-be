#!/usr/bin/env python3
"""
One-shot migration: import the legacy expense spreadsheet into the
expense-tracker API.

The script is idempotent for bootstrap entities (payment methods,
categories, recurrence types, bill statements) — it looks up by name
before creating.  Expense rows are NOT deduplicated, so run the expense
phase only once.

USAGE
-----
    # Dry run (prints every payload, hits nothing)
    python3 scripts/migrate_from_sheet.py \
        --csv scripts/data/sheet.csv \
        --dry-run

    # Live run
    python3 scripts/migrate_from_sheet.py \
        --csv scripts/data/sheet.csv \
        --base-url https://api-expensetracker.wikra.cloud/api/v1 \
        --token "$EXPENSE_TRACKER_TOKEN"

    # Bootstrap only (no expense rows)
    python3 scripts/migrate_from_sheet.py --csv scripts/data/sheet.csv \
        --base-url ... --token ... --bootstrap-only

OPEN ASSUMPTIONS (per the agreed migration plan)
------------------------------------------------
1. Recurring / installment / subscription rows use 2026-05-01 as the start date.
   The API will auto-create monthly bill statements + future installments.
2. Past installments are NOT backfilled.  Mobil 31/96 starts at month 31.
3. `recurring`/`subscription` types without a fixed count run until 2030-12-01.
4. The single THB row is converted at Rp 469 / THB.
5. `MBAINTAN` is canonicalized to `MBAKINTAN` (same person).
6. Status mapping: PAID -> paid, UNPAID -> unpaid, blank -> pending.
   Status is patched in via PUT after create, since CreateExpenseRequest
   does not accept a status field.
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import sys
import time
from collections import Counter
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any

import urllib.error
import urllib.request

# -------------------------------------------------------------------- config

THB_TO_IDR = 469.0  # ~rate at 2026-05-19

RECURRING_START_DATE = "2026-05-01T00:00:00Z"
RECURRING_END_DATE_DEFAULT = "2030-12-01T00:00:00Z"  # for subscription/recurring

# title-keyword -> category name.  First match wins (order matters).
# Re-uses the user's existing 3-category system (Transportation, Shopping,
# Responsibility) and adds 4 more focused ones.  "Responsibility" absorbs
# bills, family obligations, education, and dev/utility subscriptions —
# matching the user's existing mental model.
CATEGORY_RULES: list[tuple[re.Pattern[str], str]] = [
    (re.compile(r"\b(grab|gojek|bluebird|grabfood|shopeefood|pertamax|parkir|pesawat|tiket\.com|grab thai|grab bangkok|grabfood thai)\b", re.I), "Transportation"),
    (re.compile(r"\b(netflix|youtube|icloud|roblox|diamond)\b", re.I), "Entertainment"),
    (re.compile(r"\b(hermina|apotek|pt gym|anytime fitness)\b", re.I), "Health & Medical"),
    (re.compile(r"\b(rumah|embah|kasbon|pajak|spp ruby|sekolah ruby|spp|sekolah|biznet|hostinger|github copilot|kuota|xl)\b", re.I), "Responsibility"),
    (re.compile(r"\b(shopee|tokopedia|uniqlo|birkenstock|chopper|case iphone|sofa|ipad|iphone|macbook|hp teteh|sepatu|alva|motor|mobil|paperid|alfamart)\b", re.I), "Shopping"),
    (re.compile(r"\b(pasar|sabana|mixue|pecel|nasi uduk|sembako|bumbu|cabe|ayam|ikan|sayur|tahu|buah|jahitan|kokarmina|kios udin|lapak|pizza hut|dunkin|7 11|grand seafood|noodle klia|solaria|central phuket|litle malaysia|fuji asa foto|cafe|kintan|es podeng)\b", re.I), "Food & Dining"),
]
DEFAULT_CATEGORY = "Other"

# Seed list — only NEW categories are added; the 3 existing ones (Transportation,
# Shopping, Responsibility) are matched by name, so we don't duplicate them.
CATEGORY_SEED: list[dict[str, Any]] = [
    {"name": "Food & Dining",     "icon": "🍔", "color": "#F97316"},
    {"name": "Entertainment",     "icon": "🎬", "color": "#A855F7"},
    {"name": "Health & Medical",  "icon": "🏥", "color": "#EF4444"},
    {"name": "Other",             "icon": "📦", "color": "#6B7280"},
]

# sheet payment string -> (api name, method_type).
# Names match the EXISTING payment methods in the system; only `Digibank Credit Card`
# and `Mandiri Bonvoy` are new and will be auto-created.
PAYMENT_METHOD_MAP: dict[str, tuple[str, str]] = {
    "SALARY":            ("Salary",                "cash"),
    "CC BCA":            ("BCA Credit Card",       "credit_card"),
    "CC BNI":            ("BNI Credit Card",       "credit_card"),
    "CC DIGIBANK":       ("Digibank Credit Card",  "credit_card"),
    "CC JENIUS":         ("Jenius Credit Card",    "credit_card"),
    "CC MANDIRI":        ("Mandiri Credit Card",   "credit_card"),
    "CC MANDIRI BONVOY": ("Mandiri Bonvoy",        "credit_card"),
    "CC TOKPED CARD":    ("Tokopedia Credit Card", "credit_card"),
    "CC UOB":            ("UOB Credit Card",       "credit_card"),
}

# Recurrence type names match what already exists in the system (Capitalized).
RECURRENCE_TYPES_SEED: list[dict[str, Any]] = [
    {"name": "Installment",  "description": "Fixed-count monthly installment"},
    {"name": "Subscription", "description": "Open-ended monthly subscription"},
    {"name": "Recurring",    "description": "Open-ended monthly recurring bill"},
]

INSTALLMENT_RE = re.compile(r"installment:\s*(\d+)\s*/\s*(\d+)", re.I)


# ---------------------------------------------------------------- HTTP client


USER_AGENT = (
    "Mozilla/5.0 (X11; Linux x86_64; expense-tracker-migration/1.0) "
    "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0 Safari/537.36"
)


class Api:
    def __init__(self, base_url: str, token: str, dry_run: bool = False) -> None:
        self.base_url = base_url.rstrip("/")
        self.token = token
        self.dry_run = dry_run
        # If dry-run + no token, we can't talk to the API — treat all GETs
        # as empty so bootstrap "looks new" and POSTs get printed.
        self.offline = dry_run and not token

    def _req(
        self,
        method: str,
        path: str,
        body: dict[str, Any] | None = None,
    ) -> Any:
        url = f"{self.base_url}{path}"
        if self.dry_run and method != "GET":
            print(f"  [dry-run] {method} {path}  body={json.dumps(body, ensure_ascii=False)}")
            return {"data": {"id": f"dry_{abs(hash(json.dumps(body))) % 10**8}", "name": (body or {}).get("name", "dry")}}
        if self.offline and method == "GET":
            return {"data": []}

        data = None
        if body is not None:
            data = json.dumps(body).encode("utf-8")
        req = urllib.request.Request(url, data=data, method=method)
        req.add_header("Authorization", f"Bearer {self.token}")
        req.add_header("Content-Type", "application/json")
        req.add_header("Accept", "application/json")
        req.add_header("User-Agent", USER_AGENT)
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                payload = resp.read().decode("utf-8")
                return json.loads(payload) if payload else {}
        except urllib.error.HTTPError as e:
            err_body = e.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"{method} {url} -> HTTP {e.code}: {err_body[:500]}") from None

    def get(self, path: str) -> Any:
        return self._req("GET", path)

    def post(self, path: str, body: dict[str, Any]) -> Any:
        return self._req("POST", path, body)

    def put(self, path: str, body: dict[str, Any]) -> Any:
        return self._req("PUT", path, body)


# ----------------------------------------------------------------- CSV parser


@dataclass
class SheetRow:
    title: str
    amount: float
    currency: str  # IDR | THB
    payment_label: str  # raw sheet value
    expense_date: str  # ISO RFC3339, may be empty
    description: str
    status: str  # paid | unpaid | pending
    paid_by: str
    bucket: str  # one_time | installment | subscription | recurring
    recurrence_count: int | None = None
    recurrence_current: int | None = None


def parse_amount(s: str) -> tuple[float | None, str]:
    s = s.strip()
    if not s:
        return None, ""
    if s.lower().endswith("thb"):
        try:
            return float(s.lower().replace("thb", "").replace(",", "").strip()), "THB"
        except ValueError:
            return None, ""
    s = s.replace("Rp", "").replace(",", "").strip()
    try:
        return float(s), "IDR"
    except ValueError:
        return None, ""


def parse_date(s: str) -> str:
    """`4/14/2026` -> `2026-04-14T00:00:00Z`.  Empty for empty input."""
    if not s.strip():
        return ""
    dt = datetime.strptime(s.strip(), "%m/%d/%Y")
    return dt.strftime("%Y-%m-%dT00:00:00Z")


def map_status(s: str) -> str:
    s = s.strip().upper()
    if s == "PAID":
        return "paid"
    if s == "UNPAID":
        return "unpaid"
    return "pending"


def map_paid_by(s: str) -> str:
    s = s.strip()
    if s.upper() == "MBAINTAN":
        return "MBAKINTAN"  # canonicalize duplicate spelling
    return s


def categorize(title: str, description: str) -> str:
    haystack = f"{title} {description}"
    for pattern, cat in CATEGORY_RULES:
        if pattern.search(haystack):
            return cat
    return DEFAULT_CATEGORY


def load_rows(csv_path: str) -> list[SheetRow]:
    rows: list[SheetRow] = []
    with open(csv_path, newline="", encoding="utf-8") as f:
        reader = csv.reader(f)
        next(reader)  # header
        for raw in reader:
            while len(raw) < 11:
                raw.append("")
            title = raw[0].strip()
            amount_raw = raw[1].strip()
            payment = raw[2].strip()
            date_raw = raw[3].strip()
            desc = raw[4].strip()
            status = raw[5].strip()
            paid_by = raw[6].strip()
            if not title and not amount_raw:
                continue
            amount, currency = parse_amount(amount_raw)
            if amount is None or not payment:
                # uncategorisable; skip but warn
                print(f"  WARN: skipping row, can't parse amount/payment: {raw}", file=sys.stderr)
                continue

            # Bucket
            inst = INSTALLMENT_RE.search(desc)
            if inst:
                bucket = "installment"
                rec_cur = int(inst.group(1))
                rec_total = int(inst.group(2))
            elif desc.lower() == "subscribe":
                bucket = "subscription"
                rec_cur = None
                rec_total = None
            elif date_raw:
                bucket = "one_time"
                rec_cur = None
                rec_total = None
            else:
                # no date, not installment, not subscription -> open-ended recurring
                bucket = "recurring"
                rec_cur = None
                rec_total = None

            expense_date = parse_date(date_raw) if bucket == "one_time" else RECURRING_START_DATE

            rows.append(
                SheetRow(
                    title=title,
                    amount=amount,
                    currency=currency,
                    payment_label=payment,
                    expense_date=expense_date,
                    description=desc,
                    status=map_status(status),
                    paid_by=map_paid_by(paid_by),
                    bucket=bucket,
                    recurrence_count=rec_total,
                    recurrence_current=rec_cur,
                )
            )
    return rows


# ----------------------------------------------------------------- bootstrap


def _index_by_name(items: list[dict[str, Any]]) -> dict[str, str]:
    """Map name -> id from a list of API objects."""
    out: dict[str, str] = {}
    for it in items or []:
        name = (it.get("name") or "").strip()
        if not name:
            continue
        out[name] = it.get("id") or it.get("ID") or ""
    return out


def _list(api: Api, path: str) -> list[dict[str, Any]]:
    body = api.get(path)
    # ApiResponse wraps payload under "data"
    data = body.get("data") if isinstance(body, dict) else body
    if isinstance(data, dict) and "data" in data:
        data = data["data"]  # paginated wrapper {data: [...], pagination: {...}}
    if isinstance(data, list):
        return data
    return []


def ensure_named(
    api: Api,
    *,
    label: str,
    path: str,
    seed_items: list[dict[str, Any]],
) -> dict[str, str]:
    """Idempotently create entities by name; returns name->id."""
    print(f"\n[bootstrap] {label}")
    existing = _index_by_name(_list(api, path))
    for item in seed_items:
        name = item["name"]
        if name in existing:
            print(f"  - exists: {name} -> {existing[name]}")
            continue
        resp = api.post(path, item)
        # response shape: {success, data: {...}, message}
        node = (resp or {}).get("data") if isinstance(resp, dict) else None
        new_id = (node or {}).get("id", "")
        existing[name] = new_id
        print(f"  + created: {name} -> {new_id}")
    return existing


def ensure_bill_statements(api: Api, names: list[str]) -> dict[str, str]:
    """Pre-create monthly bill statements (`April 2026`, `May 2026`, ...) so
    one-time expenses have something to link to."""
    print("\n[bootstrap] bill statements")
    existing = _index_by_name(_list(api, "/bill-statements"))
    for name in names:
        if name in existing:
            print(f"  - exists: {name} -> {existing[name]}")
            continue
        # name format "<Month> <Year>" -> compute first-day date
        try:
            dt = datetime.strptime(name, "%B %Y")
            statement_date = dt.strftime("%Y-%m-01T00:00:00Z")
        except ValueError:
            statement_date = None
        body = {
            "name": name,
            "statement_date": statement_date,
            "description": f"Migrated bill statement for {name}",
        }
        resp = api.post("/bill-statements", body)
        node = (resp or {}).get("data") if isinstance(resp, dict) else None
        new_id = (node or {}).get("id", "")
        existing[name] = new_id
        print(f"  + created: {name} -> {new_id}")
    return existing


# ---------------------------------------------------------------- planning


@dataclass
class Plan:
    creates: list[dict[str, Any]] = field(default_factory=list)  # POST /expenses
    bulk_creates: list[dict[str, Any]] = field(default_factory=list)  # POST /expenses/bulk
    status_updates: list[tuple[int, str]] = field(default_factory=list)  # (index_in_combined_list, status)


def bill_statement_for(date_iso: str) -> str:
    dt = datetime.strptime(date_iso[:10], "%Y-%m-%d")
    return dt.strftime("%B %Y")


def build_payload(
    row: SheetRow,
    *,
    payment_id: dict[str, str],
    category_id: dict[str, str],
    recurrence_id: dict[str, str],
    bill_id: dict[str, str],
) -> dict[str, Any]:
    pm_name, _ = PAYMENT_METHOD_MAP[row.payment_label]
    pm_id = payment_id[pm_name]

    # Currency conversion
    if row.currency == "THB":
        idr_amount = round(row.amount * THB_TO_IDR)
        desc_extra = f" (converted {row.amount:.0f} THB @ {THB_TO_IDR}/THB)"
    else:
        idr_amount = row.amount
        desc_extra = ""

    cat = categorize(row.title, row.description)
    cat_id = category_id[cat]

    bs_name = bill_statement_for(row.expense_date)
    bs_id = bill_id[bs_name]

    payload: dict[str, Any] = {
        "title": row.title,
        "amount": idr_amount,
        "payment_method_id": pm_id,
        "expense_date": row.expense_date,
        "category_id": cat_id,
        "bill_statement_id": bs_id,
    }
    desc = (row.description + desc_extra).strip()
    if desc:
        payload["description"] = desc
    if row.paid_by:
        payload["paid_by"] = row.paid_by

    if row.bucket == "installment":
        payload["recurrence_type_id"] = recurrence_id["Installment"]
        payload["recurrence_type"] = "installment"
        payload["recurrence_count"] = row.recurrence_count
        payload["recurrence_current"] = row.recurrence_current
    elif row.bucket == "subscription":
        payload["recurrence_type_id"] = recurrence_id["Subscription"]
        payload["recurrence_type"] = "subscription"
        payload["recurrence_end_date"] = RECURRING_END_DATE_DEFAULT
    elif row.bucket == "recurring":
        payload["recurrence_type_id"] = recurrence_id["Recurring"]
        payload["recurrence_type"] = "recurring"
        payload["recurrence_end_date"] = RECURRING_END_DATE_DEFAULT
    # one_time: no recurrence fields

    return payload


# --------------------------------------------------------------- execution


def execute_plan(
    api: Api,
    rows: list[SheetRow],
    *,
    payment_id: dict[str, str],
    category_id: dict[str, str],
    recurrence_id: dict[str, str],
    bill_id: dict[str, str],
    bulk_size: int = 25,
) -> None:
    one_time_payloads: list[dict[str, Any]] = []
    one_time_meta: list[SheetRow] = []
    recurring_rows: list[tuple[SheetRow, dict[str, Any]]] = []

    for r in rows:
        payload = build_payload(
            r,
            payment_id=payment_id,
            category_id=category_id,
            recurrence_id=recurrence_id,
            bill_id=bill_id,
        )
        if r.bucket == "one_time":
            one_time_payloads.append(payload)
            one_time_meta.append(r)
        else:
            recurring_rows.append((r, payload))

    # 1. Recurring expenses one-by-one (they auto-spawn future months)
    print(f"\n[execute] recurring/installment/subscription rows: {len(recurring_rows)}")
    for r, payload in recurring_rows:
        try:
            resp = api.post("/expenses", payload)
            node = (resp or {}).get("data") if isinstance(resp, dict) else None
            exp_id = (node or {}).get("id", "")
            print(f"  + {r.bucket:13s} {r.title:25s} Rp{payload['amount']:>12,.0f} -> {exp_id}")
            if r.status in ("paid", "unpaid") and exp_id and not api.dry_run:
                api.put(f"/expenses/{exp_id}", {"status": r.status})
                print(f"    status -> {r.status}")
        except Exception as e:
            print(f"  ! FAILED {r.title}: {e}", file=sys.stderr)

    # 2. One-time expenses via /expenses/bulk
    print(f"\n[execute] one-time rows via bulk: {len(one_time_payloads)} (batches of {bulk_size})")
    created_ids: list[str] = []
    for batch_start in range(0, len(one_time_payloads), bulk_size):
        batch = one_time_payloads[batch_start : batch_start + bulk_size]
        try:
            resp = api.post("/expenses/bulk", {"expenses": batch})
            node = (resp or {}).get("data") if isinstance(resp, dict) else None
            created = (node or {}).get("created", []) if isinstance(node, dict) else []
            for c in created:
                created_ids.append(c.get("id", ""))
            print(f"  + batch [{batch_start}:{batch_start + len(batch)}] -> {len(created)} created")
        except Exception as e:
            print(f"  ! batch [{batch_start}:{batch_start + len(batch)}] FAILED: {e}", file=sys.stderr)
            # Fall back: post one by one so a single bad row doesn't sink the batch
            for i, p in enumerate(batch):
                try:
                    resp = api.post("/expenses", p)
                    node = (resp or {}).get("data") if isinstance(resp, dict) else None
                    created_ids.append((node or {}).get("id", ""))
                except Exception as e2:
                    print(f"    ! row {batch_start + i} {p.get('title')!r}: {e2}", file=sys.stderr)
                    created_ids.append("")
        time.sleep(0.2)

    # 3. Status updates for one-time PAID/UNPAID rows
    if not api.dry_run:
        n_updates = 0
        for meta, exp_id in zip(one_time_meta, created_ids):
            if meta.status in ("paid", "unpaid") and exp_id:
                try:
                    api.put(f"/expenses/{exp_id}", {"status": meta.status})
                    n_updates += 1
                except Exception as e:
                    print(f"  ! status update {exp_id} ({meta.title}): {e}", file=sys.stderr)
        print(f"\n[execute] status updates applied: {n_updates}")


# ----------------------------------------------------------------------- CLI


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--csv", required=True, help="Path to the sheet CSV export.")
    ap.add_argument("--base-url", default="https://api-expensetracker.wikra.cloud/api/v1")
    ap.add_argument("--token", default="", help="Bearer session token. Required unless --dry-run.")
    ap.add_argument("--dry-run", action="store_true", help="Print every payload, hit nothing.")
    ap.add_argument("--bootstrap-only", action="store_true", help="Create entities & bill statements; skip expense rows.")
    ap.add_argument("--save-plan", default="", help="If set, write the planned expense payloads to this JSON file.")
    args = ap.parse_args()

    if not args.dry_run and not args.token:
        print("ERROR: --token is required for live runs", file=sys.stderr)
        return 2

    rows = load_rows(args.csv)
    print(f"Loaded {len(rows)} rows from {args.csv}")
    print("Bucket counts:", dict(Counter(r.bucket for r in rows)))
    print("Status counts:", dict(Counter(r.status for r in rows)))
    print("Payment label counts:", dict(Counter(r.payment_label for r in rows)))
    unknown_payments = sorted({r.payment_label for r in rows if r.payment_label not in PAYMENT_METHOD_MAP})
    if unknown_payments:
        print(f"ERROR: payment labels not in PAYMENT_METHOD_MAP: {unknown_payments}", file=sys.stderr)
        return 3

    api = Api(args.base_url, args.token, dry_run=args.dry_run)

    # --- bootstrap
    rec_id = ensure_named(api, label="recurrence types", path="/recurrence-types", seed_items=RECURRENCE_TYPES_SEED)
    pm_seed = [{"name": v[0], "method_type": v[1], "description": f"Migrated from sheet ({k})"} for k, v in PAYMENT_METHOD_MAP.items()]
    pm_id = ensure_named(api, label="payment methods", path="/payment-methods", seed_items=pm_seed)
    cat_id = ensure_named(api, label="categories", path="/categories", seed_items=CATEGORY_SEED)

    # bill statements: distinct months across all rows
    months_needed = sorted({bill_statement_for(r.expense_date) for r in rows})
    bill_id = ensure_bill_statements(api, months_needed)

    if args.bootstrap_only:
        print("\n[bootstrap-only] done.")
        return 0

    # --- preview / save plan
    plan_payloads = [
        build_payload(r, payment_id=pm_id, category_id=cat_id, recurrence_id=rec_id, bill_id=bill_id)
        for r in rows
    ]
    if args.save_plan:
        with open(args.save_plan, "w", encoding="utf-8") as f:
            json.dump(
                [{"sheet_title": r.title, "bucket": r.bucket, "status": r.status, "payload": p}
                 for r, p in zip(rows, plan_payloads)],
                f,
                ensure_ascii=False,
                indent=2,
            )
        print(f"\nPlan saved to {args.save_plan}")

    # --- execute
    execute_plan(api, rows, payment_id=pm_id, category_id=cat_id, recurrence_id=rec_id, bill_id=bill_id)
    print("\nDone.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
