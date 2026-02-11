# Dev PM Agent

Remote Cursor CLI controller — run development tasks from your phone via web chat.

See [PLAN.md](./PLAN.md) for the full project design and implementation plan.

## Quick start

```bash
# 1. Generate secrets
export EXECUTOR_API_KEY=$(openssl rand -hex 32)
export JWT_SECRET=$(openssl rand -hex 32)

# 2. Run relayer (backend)
JWT_SECRET=$JWT_SECRET EXECUTOR_API_KEY=$EXECUTOR_API_KEY cargo run -p relayer

# 3. Run executor (in another terminal; requires Cursor CLI)
EXECUTOR_API_KEY=$EXECUTOR_API_KEY cargo run -p executor

# 4. Frontend
cd apps/web && npm install && npm run dev
```

Then open http://localhost:5173 → Setup (first-run) or Login.

## Deploy to Render

1. Create a Blueprint in Render (New → Blueprint → connect repo → Apply).
2. Set env vars for `dev-pm-relayer`: `JWT_SECRET`, `EXECUTOR_API_KEY` (generate with `openssl rand -hex 32`).
3. After relayer deploys, set `VITE_RELAYER_URL` for `dev-pm-webapp` to `https://<relayer-service>.onrender.com`, then redeploy.
4. Run executor locally with:
   ```bash
   export EXECUTOR_API_KEY=<same-as-relayer>
   export RELAYER_WS_URL=wss://<relayer-service>.onrender.com/ws
   cargo run -p executor
   ```

See [sprints/SPRINT_004.md](./sprints/SPRINT_004.md) for the full deployment checklist and local test flow.

## Executor subcommands

- `cargo run -p executor` — run daemon (default)
- `cargo run -p executor -- register-device <word-code> <password>` — register new webapp device
