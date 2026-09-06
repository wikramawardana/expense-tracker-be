---
name: expense-bank-sync
description: Use this skill when the user asks to test, debug, trigger, or review bank email synchronization (BCA, BNI, Mandiri) for the expense tracker or check the daily bank sync cronjob.
---

# Bank Expense Synchronization Runbook

## Overview
Automates fetching transaction notification emails from BCA, BNI, and Mandiri via IMAP, parsing amounts, dates, and merchants, and recording them into SurrealDB.

## Trigger Scenarios
- User asks to sync expenses from email or test bank sync.
- Checking why bank transactions didn't import or verifying cron runs.
- Modifying email parsing patterns or merchant normalization.

## Key Rules
- **No Description**: Imported expenses must have `description=None` (empty). Do NOT populate descriptions with card/merchant/ref text.
- **Deduplication**: Key format is `{expense_date[:10]}:{int(amount)}:{title[:10].lower()}`. Matches against existing SurrealDB rows to prevent duplicates.
- **Status**: Imported bank expenses are set to status `pending` by default.

## Step-by-Step Procedure

### 1. Test Sync Locally or on VPS (Dry Run)
You can test the MCP bank sync function in dry-run mode without inserting records:
```bash
ssh wikra@72.61.210.144 "python3 -c \"
import sys
sys.path.append('/home/wikra/production-projects/expense-tracker')
from mcp.server import sync_bank_expenses
print(sync_bank_expenses(dry_run=True, days=1))
\""
```

### 2. Verify VPS Scheduled Cronjob
Check status of the scheduled daily bank sync cron:
```bash
ssh wikra@72.61.210.144 "python3 -m hermes_cli.main --profile wikrassist-expense cron list"
```
The job `cf8c32d745a1` runs daily at `22:00 WIB` (`0 22 * * *`).

### 3. Check Service Logs
If bank sync reports errors:
```bash
ssh wikra@72.61.210.144 "journalctl --user -u hermes-gateway-wikrassist-expense.service -n 100 --no-pager"
```

## Parsing Reference
- **BCA**: Searches for `Transaksi Kartu Kredit BCA` or `bcakartukredit`. Target payment: `BCA KrisFlyer`.
- **BNI**: Searches for `Kartu Kredit BNI` or `bnicreditcard`. Target payment: `BNI Mastercard World`.
- **Mandiri**: Searches for `mandiri` or `livin`. Target payment: `Mandiri Marriott Bonvoy` or source account.
