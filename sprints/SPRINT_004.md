# Sprint 004 — Render Deployment Handoff

**Created:** 2025-02-11  
**Status:** Ready  
**Predecessor:** SPRINT_003 (Options A, C, D implemented)

---

## 1. Sprint 003 Retrospective

### 1.1 What Went Right

| Area | Notes |
|------|-------|
| **Option A (models)** | `GET /api/models` added to relayer; Chat fetches dynamically. Fallback to `composer-1.5` if API fails. |
| **Option C (WebSocket)** | Chat parses `command_update` payloads, updates single command in state. No full refetch on status change. |
| **Option D (polish)** | Repos: "Path must be under ~/repos/". Chat: "No repos. Add one in Repos." with link. Setup: "Save the device key before leaving." |
| **No blocking bugs** | All verification passed. Web app builds; Rust tests pass. |

### 1.2 What Went Wrong / Pain Points

| Issue | Impact | Mitigation |
|-------|--------|------------|
| **Translation JSON format** | Executor uses `agent --output-format text` for translation step but parses output as JSON. PLAN §7.3 says `--output-format json`. | May fail if model wraps JSON in markdown. Consider adding `--output-format json` for translation call when Cursor CLI supports it. |
| **Model list static** | Relayer returns hardcoded model list. No `agent models` integration. | Acceptable for MVP. |

### 1.3 Gaps Remaining (vs PLAN)

| Gap | Severity | Location |
|-----|----------|----------|
| **Translation output format** | Medium | `crates/executor/src/cursor/mod.rs` — `run_agent` uses `--output-format text` for all calls; translation needs JSON. |
| **Command presets** | Future | PLAN §8: sprints, plan flows. Out of scope. |
| **Frontend tests** | Medium | No Vitest/Playwright. |
| **E2E validation** | Low | Manual run not documented. |

---

## 2. Current State Summary

### Implemented and Working

- **Relayer**: axum HTTP + WebSocket, SQLite (with persistent disk on Render), migrations, setup/login/JWT, device registration, commands CRUD, repos list/add, **models list** (`GET /api/models`), WebSocket broadcast, `/health` endpoint.
- **Executor**: WebSocket client, Cursor CLI (`agent -p`), `register-device` subcommand. Runs locally (not on Render).
- **Web**: Setup, Login, AddDevice, Repos, Chat with repo/model selectors, **incremental WebSocket** updates, polish (hints, links).
- **Render**: `render.yaml` defines relayer (web service) and webapp (static site). Relayer uses persistent disk for SQLite.

### Verification Commands

```bash
cargo fmt
RUSTFLAGS="-D warnings" cargo build
cargo test
cd apps/web && npm run build
```

### Key Files

| Purpose | Path |
|---------|------|
| Render blueprint | `render.yaml` |
| Post-change rule | `.cursor/rules/post-change-verification.mdc` |
| Relayer API | `crates/relayer/src/api/routes.rs` |
| Executor cursor | `crates/executor/src/cursor/mod.rs` |
| Chat page | `apps/web/src/pages/Chat.tsx` |

---

## 3. Local Test (Before Render Deploy)

Run the full stack locally to verify everything works before deploying to Render.

### 3.1 Start Services (3 terminals)

**Terminal 1 — Relayer:**
```bash
export EXECUTOR_API_KEY=$(openssl rand -hex 32)
export JWT_SECRET=$(openssl rand -hex 32)
JWT_SECRET=$JWT_SECRET EXECUTOR_API_KEY=$EXECUTOR_API_KEY cargo run -p relayer
```
Runs at http://localhost:8080. DB at `./data/relayer.db` (created on first run).

**Terminal 2 — Executor** (requires Cursor CLI `agent` on PATH):
```bash
EXECUTOR_API_KEY=$EXECUTOR_API_KEY cargo run -p executor
```
Connects to ws://localhost:8080/ws.

**Terminal 3 — Web app:**
```bash
cd apps/web && npm install && npm run dev
```
Runs at http://localhost:5173. Vite proxies /api and /ws to localhost:8080 (no `VITE_RELAYER_URL` needed).

### 3.2 Local Verification Checklist

- [ ] http://localhost:8080/health → `ok`
- [ ] http://localhost:5173 loads → redirects to Setup or Login
- [ ] **Setup** (first-run): create account → save device key + add TOTP to authenticator
- [ ] **Login**: device key, password, TOTP code → lands on Chat
- [ ] **Add device**: Chat → Add device → generate code → (optional) run `cargo run -p executor -- register-device <code> <password>` in a 4th terminal
- [ ] **Repos**: Chat → Repos → add a repo (e.g. `~/repos/default`, create that dir if needed)
- [ ] **Chat command**: select repo, enter task, Send → command appears, executor picks it up, status updates (running → done/failed)
- [ ] **Incremental WebSocket**: status changes without full page refresh

### 3.3 Optional: Simulate Render Env

To mirror Render’s relayer config:
```bash
PORT=8080 DATABASE_PATH=./data/relayer.db JWT_SECRET=... EXECUTOR_API_KEY=... cargo run -p relayer
```
(Same as default; useful if you ever change `DATABASE_PATH` in the blueprint.)

---

## 4. Render Deployment

### 4.1 Blueprint Layout

`render.yaml` defines two services:

| Service | Type | Runtime | Purpose |
|---------|------|---------|---------|
| `dev-pm-relayer` | web | rust | Backend API + WebSocket. SQLite on persistent disk. |
| `dev-pm-webapp` | web | static | Vite React SPA. Serves `apps/web/dist`. |

### 4.2 Required Env Vars (set in Render Dashboard)

**Relayer** (must be set before deploy):

| Key | Description | Example |
|-----|-------------|---------|
| `JWT_SECRET` | Secret for JWT signing. | `openssl rand -hex 32` |
| `EXECUTOR_API_KEY` | Shared with executor; same value. | `openssl rand -hex 32` |

**Webapp** (set after first relayer deploy):

| Key | Description | Example |
|-----|-------------|---------|
| `VITE_RELAYER_URL` | Relayer base URL. | `https://dev-pm-relayer.onrender.com` |

If `VITE_RELAYER_URL` is not set, the webapp will try to use same-origin (works if proxied; for Render static site, set this).

### 4.3 Deploy Steps

**Before deploying:** Run the [Local Test (§3)](#3-local-test-before-render-deploy) to verify the full flow.

1. **Generate secrets** (local):
   ```bash
   openssl rand -hex 32  # → JWT_SECRET
   openssl rand -hex 32  # → EXECUTOR_API_KEY (use same for relayer + executor)
   ```

2. **Create Blueprint** in Render: New → Blueprint → connect repo → Apply.

3. **Set env vars** for `dev-pm-relayer`: `JWT_SECRET`, `EXECUTOR_API_KEY` (mark as secret).

4. After relayer deploys, **set** `VITE_RELAYER_URL` for `dev-pm-webapp` to `https://<relayer-service>.onrender.com`. Trigger a redeploy of the webapp so the build picks up the var.

5. **Executor** runs locally (your dev PC). Set:
   ```bash
   export EXECUTOR_API_KEY=<same-as-relayer>
   export RELAYER_WS_URL=wss://<relayer-service>.onrender.com/ws
   cargo run -p executor
   ```

### 4.4 Persistent Disk (Relayer)

Relayer uses a 1 GB disk at `/data`. `DATABASE_PATH` is set to `/data/relayer.db` in the blueprint. SQLite data persists across deploys.

### 4.5 SPA Routing (Webapp)

The blueprint includes a rewrite rule `/*` → `/index.html` for React Router. If this format is rejected, add the rule manually in the webapp's **Redirects/Rewrites** tab: Source `/*`, Destination `/index.html`, Action **Rewrite**.

---

## 5. Post-Deploy Checklist

- [ ] Relayer health: `curl https://<relayer>.onrender.com/health` → `ok`
- [ ] Webapp loads at `https://<webapp>.onrender.com`
- [ ] Setup flow works (first-run)
- [ ] Login with device key + password + TOTP
- [ ] Add repo, create command from Chat
- [ ] Executor (local) receives command, runs, PATCHes result
- [ ] Chat shows command status update (incremental WebSocket)

---

## 6. Handoff Checklist for Next Agent

Before starting:

- [ ] Read `PLAN.md` §1–10, §7
- [ ] Read `docs/WEBSOCKET_PROTOCOL.md`
- [ ] Run `cargo fmt`, `RUSTFLAGS="-D warnings" cargo build`, `cargo test`
- [ ] `cd apps/web && npm run build`

After changes:

- [ ] Run post-change verification (per `.cursor/rules/post-change-verification.mdc`)
- [ ] Fix any new warnings

---

## 7. Reflection (Sprint 003/004 Agent)

**What helped:** Options A, C, D were well-scoped. Render blueprint spec is clear; relayer has `/health` for zero-downtime deploys. Persistent disk for SQLite is straightforward.

**For next agent:** After Render deploy, run the post-deploy checklist. If translation fails (JSON parse error), consider switching the translation `run_agent` call to `--output-format json` if Cursor CLI supports it. Frontend tests remain a gap for future sprints.
