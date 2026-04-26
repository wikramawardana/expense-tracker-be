# OpenClaw Skills

Skills for [OpenClaw](https://openclaw.ai/) that let the agent interact with this expense tracker over HTTP.

## Installing the `expense-tracker` skill

1. Mint a bot API key. Log into the frontend as your user and:
   ```bash
   curl -X POST https://<your-host>/api/v1/api-keys \
     -H "Authorization: Bearer <session-token>" \
     -H "Content-Type: application/json" \
     -d '{"name":"openclaw"}'
   ```
   Copy the `key` from the response (starts with `etk_`) — it is shown **exactly once**.

   > If your FE has an "API Keys" settings page, mint one there instead.

2. On the machine running OpenClaw:
   ```bash
   # Point the agent at this repo's skill:
   cp -r openclaw/expense-tracker ~/.openclaw/skills/expense-tracker

   # Tell the skill where the API lives and how to auth:
   export EXPENSE_TRACKER_BASE_URL="https://<your-api-host>/api/v1"
   export EXPENSE_TRACKER_API_KEY="etk_..."
   ```

   (Persist both env vars in whatever OpenClaw loads on startup — typically `~/.openclaw/env` or your shell rc.)

3. Restart OpenClaw. From Discord/WhatsApp/etc., try:
   > "log 3.50 coffee and 12 lunch today, paid cash"

   The agent should fetch your categories/payment-methods, echo back what it parsed, and — after you confirm — POST to `/bot/expenses/bulk`.

## Security

- Keys are hashed (SHA-256) at rest; the plaintext is never stored or re-displayed.
- Revoke a compromised key via `DELETE /api/v1/api-keys/:id` (or the FE settings page).
- Keys are scoped to whatever expenses the minting user can create — there's no additional permission model.
