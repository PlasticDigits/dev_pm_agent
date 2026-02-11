# Sprint 003 — Handoff to Next Agent

**Created:** 2025-02-11  
**Status:** Ready  
**Predecessor:** SPRINT_002 (implemented Option A)

---

## 1. Sprint 002 Retrospective

### 1.1 What Went Right

| Area | Notes |
|------|-------|
| **Scope clarity** | Option A from SPRINT_002 was well-defined: Repos page, repo selector, model selectors, config update. All four items completed. |
| **Existing APIs** | `listRepos`, `addRepo`, and `createCommand` already supported the needed fields. Frontend wiring was straightforward. |
| **Consistent patterns** | Repos page followed AddDevice/Chat patterns: token guard, refresh callback, shared header/nav, error handling. |
| **No backend changes** | All work was frontend + config docs. No Rust changes; `cargo build` and `cargo test` remained green. |
| **Config alignment** | `config/executor.example.toml` now documents env vars only; no misleading `poll_interval_secs` or `[qr]`. |

### 1.2 What Went Wrong / Pain Points

| Issue | Impact | Mitigation |
|-------|--------|------------|
| **Hardcoded model list** | PLAN §7.2 specifies "webapp fetches from relayer endpoint that queries executor or caches model list". Implemented with hardcoded `['composer-1.5', 'claude-4', ...]`. | Works for MVP; `GET /api/models` can be added later if executor exposes `agent models`. |
| **No incremental WebSocket** | Chat still uses `ws.onmessage = () => refreshCommands()` — full refetch on any message. Option C from SPRINT_002 not done. | Acceptable for now; incremental parsing of `command_update` would improve perceived performance. |
| **Empty repo selector UX** | If no repos exist, selector shows only "— None —". User must go to Repos page first. | Minor; could add inline "Add repo" link or hint in Chat. |
| **No frontend tests** | Web app has no unit or E2E tests. Changes are verified manually only. | Vitest or Playwright could be added in a later sprint. |

### 1.3 Gaps Remaining (vs PLAN)

| Gap | Severity | Location |
|-----|----------|----------|
| **Model list endpoint** | Medium | PLAN §7.2: "webapp fetches from relayer endpoint that queries executor or caches model list". No `GET /api/models`; webapp uses hardcoded list. |
| **Incremental WebSocket UX** | Low | Chat refetches full command list on any `command_update`. Could parse payload and update single command in state. |
| **Command presets** | Future | PLAN §8: sprints, plan flows (monorepo init, gap analysis, security review). Out of scope for MVP. |
| **Frontend tests** | Medium | No Vitest/Playwright for web app. Rust crates have unit tests; frontend is untested. |
| **E2E manual validation** | Low | Option B from SPRINT_002: end-to-end manual test (Setup → Login → AddDevice → Chat command → executor runs) not documented as completed. |

---

## 2. Current State Summary

### Implemented and Working

- **Relayer**: axum HTTP + WebSocket, SQLite, migrations, setup/login/JWT, device registration, commands CRUD, repos list/add, WebSocket broadcast for `command_new` and `command_update`.
- **Executor**: WebSocket client for commands, Cursor CLI integration (`agent -p`), `register-device` subcommand, repo path validation.
- **Web**: Setup, Login, AddDevice (word-style keygen), **Repos** (list and add repos), Chat with **repo selector**, **translator/workload model selectors**, WebSocket-triggered refresh. Commands created with `repo_path`, `translator_model`, `workload_model`.
- **Config**: `config/executor.example.toml` documents env vars: `RELAYER_WS_URL`, `EXECUTOR_API_KEY`, `DEFAULT_REPO`, `TRANSLATOR_MODEL`, `WORKLOAD_MODEL`.

### Verification Commands

```bash
cargo fmt
RUSTFLAGS="-D warnings" cargo build
cargo test
# Web app:
cd apps/web && npm run build
```

### Key Files

| Purpose | Path |
|---------|------|
| Post-change rule | `.cursor/rules/post-change-verification.mdc` |
| Relayer API | `crates/relayer/src/api/routes.rs` |
| Executor WS | `crates/executor/src/relay_client/ws.rs` |
| Chat page | `apps/web/src/pages/Chat.tsx` |
| Repos page | `apps/web/src/pages/Repos.tsx` |

---

## 3. Recommended Sprint 003 Scope

### Option A: Model List Endpoint + Dynamic Models (recommended)

1. **Add `GET /api/models`**  
   - Relayer endpoint that returns list of models. Options: (a) hardcoded list for now, (b) cache from executor if executor can expose `agent models`, (c) config-based.  
   - Chat fetches models from this endpoint instead of hardcoding.

2. **Executor model exposure**  
   - If Cursor CLI has `agent models` or similar, executor could query and relay. Otherwise, relayer uses static list.

### Option B: Testing and Production Prep

- **Frontend tests**: Add Vitest for API layer and key components, or Playwright for smoke E2E.
- **E2E runbook**: Document manual E2E steps (Setup → Login → AddDevice → Chat command → executor runs). Run once and record result.
- **README**: Expand with deploy notes, env vars for prod (Render, systemd).

### Option C: Incremental WebSocket UX

- Chat: parse `command_update` WebSocket payloads. Instead of `refreshCommands()` on every message, update the single command in local state. Reduces redundant HTTP calls and improves perceived latency.

### Option D: Polish and UX

- Repos: add path validation hint (e.g. "Path must be under ~/repos/").
- Chat: when no repos, show "No repos. Add one in Repos." or inline link.
- Setup: clarify "Save the device key before leaving."

---

## 4. Areas Still Needing Testing

| Area | Current State | Recommendation |
|------|---------------|----------------|
| **Rust unit tests** | Present: auth, db, shared, executor (repo validation). | Keep running `cargo test` per post-change rule. |
| **Frontend unit tests** | None. | Add Vitest for `api/*.ts` and critical pages. |
| **E2E** | None. | Manual runbook first; Playwright later if desired. |
| **WebSocket reconnection** | Executor has reconnect logic; webapp connects on mount. | Manual test: kill relayer, restart; verify executor reconnects. |
| **Repo path validation** | Relayer `add_repo` validates `~/repos/`. | Try invalid path from Repos page; expect BAD_REQUEST. |
| **Command flow** | Create command → executor runs → status updates. | End-to-end manual test with real Cursor CLI. |

---

## 5. Handoff Checklist for Next Agent

Before starting:

- [ ] Read `PLAN.md` §1–10, §7 (Cursor CLI)
- [ ] Read `docs/WEBSOCKET_PROTOCOL.md`
- [ ] Run `cargo fmt`, `RUSTFLAGS="-D warnings" cargo build`, `cargo test` — all must pass
- [ ] `cd apps/web && npm run build` — must pass
- [ ] Cursor CLI `agent` command available (for executor tests)
- [ ] `EXECUTOR_API_KEY` generated: `openssl rand -hex 32`

After changes:

- [ ] Run `cargo fmt`, `RUSTFLAGS="-D warnings" cargo build`, `cargo test` (per `.cursor/rules/post-change-verification.mdc`)
- [ ] Fix any new warnings before finishing

---

## 6. Reflection (Sprint 002 Agent)

**What helped:** SPRINT_002 provided a clear Option A scope. The backend was already in place; the work was mostly wiring and one new page. Following existing patterns (AddDevice, Chat) kept the code consistent.

**What could improve:** The model list is hardcoded; implementing `GET /api/models` per PLAN would require deciding how the executor exposes models (if at all). Deferring that kept the sprint small and finishable. Frontend testing remains a gap.

**For next agent:** Option A (model endpoint) completes the PLAN §7.2 vision. Option B (testing) would increase confidence before production. Option C (incremental WebSocket) is a nice UX win with low risk. The system is functionally complete for MVP; focus can shift to testing, polish, or model discovery.
