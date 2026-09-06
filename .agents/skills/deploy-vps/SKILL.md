---
name: expense-deploy-vps
description: Use this skill when deploying updates to the expense tracker MCP server, backend services, or systemd services on the VPS (72.61.210.144).
---

# Expense Tracker VPS Deployment Runbook

## Overview
Deploys updates to the FastMCP server (`mcp/server.py`) and backend services running on the production VPS (`72.61.210.144`).

## Target Environment
- **Host**: `72.61.210.144`
- **SSH User**: `wikra` (SSH key authentication)
- **Production Path**: `/home/wikra/production-projects/expense-tracker/`
- **Python Venv**: `/home/wikra/.hermes/hermes-agent/venv/`
- **Systemd User Service**: `hermes-gateway-wikrassist-expense.service`

## Step-by-Step Procedure

### 1. Pre-deployment Validation
Always check syntax locally before uploading:
```bash
python3 -m py_compile mcp/server.py
```

### 2. Upload MCP Server to VPS
Copy the updated `server.py` to the VPS:
```bash
scp mcp/server.py wikra@72.61.210.144:/home/wikra/production-projects/expense-tracker/mcp/server.py
```

### 3. Verify and Restart Service on VPS
Run compilation check on the remote server and restart the systemd user service:
```bash
ssh wikra@72.61.210.144 "/home/wikra/.hermes/hermes-agent/venv/bin/python -m py_compile /home/wikra/production-projects/expense-tracker/mcp/server.py && systemctl --user restart hermes-gateway-wikrassist-expense.service && systemctl --user status hermes-gateway-wikrassist-expense.service --no-pager"
```

### 4. Post-Deployment Verification
Check the latest service logs to ensure the gateway connected without errors:
```bash
ssh wikra@72.61.210.144 "journalctl --user -u hermes-gateway-wikrassist-expense.service -n 30 --no-pager"
```

### 5. Git Synchronization
Commit and push changes to `main` so other devices stay synchronized:
```bash
git add mcp/server.py
git commit -m "feat(mcp): <description of changes>"
git push origin main
```
