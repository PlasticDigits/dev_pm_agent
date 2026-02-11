# Dev PM Agent

Remote Cursor CLI controller — run development tasks from your phone via web chat.

See [PLAN.md](./PLAN.md) for the full project design and implementation plan.

## Quick start

```bash
# 1. Copy .env.example, generate secrets
# Required: JWT_SECRET, EXECUTOR_API_KEY, PASSWORD_SALT, CLIENT_SALT (CLIENT_SALT = VITE_CLIENT_SALT in apps/web/.env)
# Generate each: openssl rand -hex 32  (min 32 bytes = 64 hex chars)
# Rotate: set new env vars, restart relayer; JWTs expire by TTL; users re-login

# 2. Run relayer (backend)
source .env && cargo run -p relayer

# 3. Run executor (in another terminal; requires Cursor CLI)
source .env && cargo run -p executor

# 4. Frontend
cd apps/web
cp .env.example .env   # set VITE_CLIENT_SALT (same value as root CLIENT_SALT)
npm install && npm run dev
```

Then open http://localhost:5173. First-run setup:

1. **Get device key** (CLI): `source .env && cargo run -p executor -- bootstrap-device`
2. **Web Setup**: paste device key → verify → create account (username, password)
3. **Add TOTP** to authenticator, then Login with device key + password + TOTP

The device key is never stored in the browser — enter it at each login.

## Deploy to Render

1. Create a Blueprint in Render (New → Blueprint → connect repo → Apply).
2. Set env vars for `dev-pm-relayer`: `JWT_SECRET`, `EXECUTOR_API_KEY`, `PASSWORD_SALT` (generate with `openssl rand -hex 32`).
3. After relayer deploys, set `VITE_RELAYER_URL` for `dev-pm-webapp` to `https://<relayer-service>.onrender.com`, then redeploy.
4. Run executor locally with:
   ```bash
   export EXECUTOR_API_KEY=<same-as-relayer>
   export RELAYER_WS_URL=wss://<relayer-service>.onrender.com/ws
   cargo run -p executor
   ```

See [sprints/SPRINT_004.md](./sprints/SPRINT_004.md) for the full deployment checklist and local test flow.

## Secrets

- **Length**: Use at least 32 bytes (64 hex characters). Generate: `openssl rand -hex 32`
- **Rotation**: Set new values in env, restart the relayer. Existing JWTs expire per `JWT_TTL_SECS`; users must re-login. Device API keys remain valid until devices are re-registered.

## Executor subcommands

- `cargo run -p executor` — run daemon (default)
- `cargo run -p executor -- bootstrap-device` — get device key for first-run setup (relayer must be running)
- `cargo run -p executor -- register-device <word-code> <password>` — register new webapp device
