# Sprint 002 — Handoff to Next Agent

**Created:** 2025-02-11  
**Status:** Ready  
**Predecessor:** SPRINT_001 (implemented Phase 1–3)

---

## 1. Sprint 001 Retrospective

### 1.1 What Went Right

| Area | Notes |
|------|-------|
| **Phased delivery** | All three phases shipped: Shared + Relayer (HTTP, DB, auth, WebSocket), Executor (WS client, Cursor CLI, `register-device`), Web (Setup, AddDevice, Login, Chat with WebSocket). |
| **Docs as source of truth** | `WEBSOCKET_PROTOCOL.md`, `WORD_STYLE_CODES.md`, `MIGRATION_STRATEGY.md`, `RECOVERY_CLI.md` were followed; minimal drift from PLAN. |
| **Unit tests** | Auth (JWT, bcrypt, TOTP), DB (in-memory SQLite with generated keys), shared (serde round-trip), executor (repo path validation). No hardcoded secrets. |
| **Post-change verification** | Cursor rule: `cargo fmt` → `RUSTFLAGS="-D warnings" cargo build` → `cargo test`. Warnings treated as errors. |
| **Relayer config** | Simple env-based config (`JWT_SECRET`, `EXECUTOR_API_KEY`, etc.); no TOML parsing complexity. |
| **Auth flow** | Setup → Login (device key + password + TOTP) → Chat; AddDevice with word-style keygen → executor `register-device`. Clean separation. |

### 1.2 What Went Wrong / Pain Points

| Issue | Impact | Mitigation |
|-------|--------|------------|
| **rust-analyzer "unresolved imports"** | `shared::WsEnvelope` etc. showed as unresolved despite compiling. | Fixed by: explicit re-exports in `shared/lib.rs`, `rustup component add rust-analyzer`, `.vscode/settings.json` with `linkedProjects`. |
| **Outdated executor config** | `config/executor.example.toml` still has `poll_interval_secs`, `[qr]` — PLAN says WebSocket only. | Executor reads env vars; TOML not used. Config example misleading. |
| **Chat missing PLAN features** | No repo selector, no model selectors. Chat sends `{ input }` only. | Backend supports `repo_path`, `translator_model`, `workload_model`; frontend not wired. |
| **WebSocket simplification** | Chat uses `ws.onmessage = () => refreshCommands()` — refetches full list on any message. | Works but not incremental; no parsing of `command_update` payload for live status. |
| **Setup → Login UX** | Setup stores `device_api_key` and navigates to Login. Login prefills from store. | Works; minor: Setup could clarify that user should save the key before leaving. |
| **Dependency version churn** | `time` crate downgrade for rusqlite; `totp-rs` v4 API changes. | Resolved but slowed iteration. |

### 1.3 Gaps Remaining (vs PLAN)

| Gap | Severity | Location |
|-----|----------|----------|
| **Repo selector in Chat** | Medium | `apps/web/src/pages/Chat.tsx` — no UI to choose repo. `listRepos`/`addRepo` exist but unused in Chat. |
| **Model selectors in Chat** | Medium | PLAN §7.2: user selects translator and workload model per command. Chat doesn’t pass these to `createCommand`. |
| **Add repo UI** | Low | No page to add repos; only API. Needed to populate repo selector. |
| **Config example alignment** | Low | `config/executor.example.toml` should drop `poll_interval_secs`, `[qr]`, add `RELAYER_WS_URL`, `DEFAULT_REPO`, `TRANSLATOR_MODEL`, `WORKLOAD_MODEL`. |
| **Model list endpoint** | Medium | PLAN §7.2: "webapp fetches from relayer endpoint that queries executor or caches model list". No such endpoint. |
| **Command presets** | Future | PLAN §8: sprints, plan flows (monorepo init, gap analysis, security review). Out of scope for MVP. |

---

## 2. Current State Summary

### Implemented and Working

- **Relayer**: axum HTTP + WebSocket, SQLite, migrations, setup/login/JWT, device registration (reserve-code, register-device), commands CRUD, repos list/add, WebSocket broadcast for `command_new` and `command_update`.
- **Executor**: WebSocket client for commands, Cursor CLI integration (`agent -p`), `register-device` subcommand, repo path validation.
- **Web**: Setup, Login, AddDevice (word-style keygen), Chat with WebSocket-triggered refresh. Basic command create/list.

### Verification Commands

```bash
cargo fmt
RUSTFLAGS="-D warnings" cargo build
cargo test
```

### Key Files

| Purpose | Path |
|---------|------|
| Post-change rule | `.cursor/rules/post-change-verification.mdc` |
| rust-analyzer | `.vscode/settings.json` |
| Relayer API | `crates/relayer/src/api/routes.rs` |
| Executor WS | `crates/executor/src/relay_client/ws.rs` |
| Chat page | `apps/web/src/pages/Chat.tsx` |

---

## 3. Recommended Sprint 002 Scope

### Option A: Complete Chat per PLAN (recommended)

1. **Add Repos page**  
   - New page `/repos` to list and add repos (path + optional name).  
   - Use `listRepos`, `addRepo` from `api/repos.ts`.  
   - Link from Chat header.

2. **Repo selector in Chat**  
   - Dropdown or list of repos from `listRepos`.  
   - Pass selected `repo_path` to `createCommand`.

3. **Model selectors in Chat**  
   - Two dropdowns: translator model, workload model.  
   - Options: hardcode a few (e.g. `composer-1.5`, `claude-4`) or add `GET /api/models` later.  
   - Pass `translator_model`, `workload_model` to `createCommand`.

4. **Update executor config example**  
   - `config/executor.example.toml`: remove `poll_interval_secs`, `[qr]`; document `RELAYER_WS_URL`, `EXECUTOR_API_KEY`, `DEFAULT_REPO`, `TRANSLATOR_MODEL`, `WORKLOAD_MODEL`.

### Option B: Polish and Production Prep

- End-to-end manual test: Setup → Login → AddDevice (with real executor) → Chat command → executor runs and updates.
- README: Render deploy notes, env vars for prod.
- Optional: `GET /api/models` if executor can expose model list.

### Option C: Incremental WebSocket UX

- Chat: parse `command_update` WebSocket payloads instead of full refetch. Update command list incrementally for better perceived performance.

---

## 4. Handoff Checklist for Next Agent

Before starting:

- [ ] Read `PLAN.md` §1–10, §7 (Cursor CLI)
- [ ] Read `docs/WEBSOCKET_PROTOCOL.md`
- [ ] Run `cargo fmt`, `RUSTFLAGS="-D warnings" cargo build`, `cargo test` — all must pass
- [ ] Cursor CLI `agent` command available (for executor tests)
- [ ] `EXECUTOR_API_KEY` generated: `openssl rand -hex 32`

After changes:

- [ ] Run `cargo fmt`, `RUSTFLAGS="-D warnings" cargo build`, `cargo test` (per `.cursor/rules/post-change-verification.mdc`)
- [ ] Fix any new warnings before finishing

---

## 5. Reflection (Sprint 001 Agent)

**What helped:** Clear PLAN, phase breakdown, and docs. Having `WEBSOCKET_PROTOCOL.md` and `WORD_STYLE_CODES.md` reduced guesswork. Env-based config kept deployment simple. Treating warnings as errors caught issues early.

**What slowed:** rust-analyzer false positives took time to debug. Dependency upgrades (time, totp-rs) required version fixes. Chat UX was simplified to ship faster, leaving repo/model selectors for later.

**For next agent:** The backend is ready for full Chat features. Focus on wiring up existing APIs (repos, models) in the UI. Option A is the highest leverage. End-to-end testing with a real executor and Cursor CLI is valuable before adding more complexity.
