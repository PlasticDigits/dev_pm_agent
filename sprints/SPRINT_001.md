# Sprint 001 — Handoff to Next Agent

**Created:** 2025-02-11  
**Status:** Implemented (Phase 1–3)

---

## 1. Context

Dev PM Agent is a solo-dev tool: a web chat on your phone that sends commands through a relayer to a desktop executor, which runs Cursor CLI (`agent -p`) and returns summarized results.

All planning and spec work is done. This sprint hands off to the next agent to begin implementation.

---

## 2. Current State

### Done

| Item | Location |
|------|----------|
| Project plan | `PLAN.md` |
| Word-style code spec | `docs/WORD_STYLE_CODES.md` |
| WebSocket protocol | `docs/WEBSOCKET_PROTOCOL.md` |
| Migration strategy | `docs/MIGRATION_STRATEGY.md` |
| Recovery CLI spec | `docs/RECOVERY_CLI.md` |
| Initial schema | `migrations/001_initial.sql` |
| Plan schema migration | `migrations/002_plan_schema.sql` |
| Shared models (partial) | `crates/shared/src/models.rs` |
| Relayer stub | `crates/relayer/src/main.rs` |
| Executor stub | `crates/executor/src/main.rs` |
| Web app scaffold | `apps/web/` (Login, Chat, Pairing; naming not yet aligned to plan) |

### Gaps vs PLAN

- Shared: `CreateCommandRequest` missing `translator_model`, `workload_model`
- Shared: no DTOs for device registration, auth, WebSocket envelope
- Relayer: no HTTP server, no WebSocket, no SQLite, no auth
- Executor: no WebSocket client, no Cursor CLI integration, no `register-device` CLI
- Web: still uses old flow (Pairing vs AddDevice); no Setup, DeviceKeygen; no WebSocket client

---

## 3. Recommended Implementation Order

Implement in this sequence to keep each step testable.

### Phase 1: Shared + Relayer core

1. **shared**  
   - Add `translator_model`, `workload_model` to `CreateCommandRequest` and `CommandResponse`  
   - Add WebSocket envelope types per `docs/WEBSOCKET_PROTOCOL.md`  
   - Add auth/device-registration DTOs (setup, login, reserve-code, register-device)

2. **Relayer: HTTP + DB**  
   - Run `001_initial.sql` and `002_plan_schema.sql` on startup  
   - axum HTTP server, health check  
   - Auth: setup, login (JWT), executor API key validation  
   - REST endpoints: commands CRUD, repos, devices/reserve-code, auth/register-device

3. **Relayer: WebSocket**  
   - `/ws` endpoint; auth via query `token` or first message  
   - Broadcast `command_new` to executor, `command_update` to controllers  
   - Implement message types from `docs/WEBSOCKET_PROTOCOL.md`

### Phase 2: Executor

4. **Executor: HTTP + WebSocket client**  
   - Connect to relayer WebSocket with `EXECUTOR_API_KEY`  
   - Handle `command_new`; send `command_ack`, `command_result`

5. **Executor: Cursor CLI**  
   - Spawn `agent -p` for translation, execution, summarization  
   - Validate `repo_path` under `~/repos/`  
   - Use `translator_model` and `workload_model` from command

6. **Executor: register-device CLI**  
   - `executor register-device <word-style-code> <password>`  
   - Call `POST /api/auth/register-device`; display `device_api_key` and `totp_secret`

### Phase 3: Web app

7. **Web: Setup + AddDevice + Login**  
   - Setup page (first-run), AddDevice with word-style keygen, Login with device_api_key + TOTP  
   - Align with PLAN flows (remove old Pairing flow)

8. **Web: Chat + WebSocket**  
   - Command input, repo selector, model selectors  
   - WebSocket for status updates; remove polling

---

## 4. Key References

| Topic | Doc |
|-------|-----|
| Auth flow, API design | `PLAN.md` §6 |
| Word-style codes | `docs/WORD_STYLE_CODES.md` |
| WebSocket messages | `docs/WEBSOCKET_PROTOCOL.md` |
| Repo path validation | `~/repos/` prefix only; see PLAN §10 |
| Cursor CLI invocation | `PLAN.md` §7 |

---

## 5. Decisions Already Made

- Executor auth: `EXECUTOR_API_KEY` in `.env` (no bootstrap token)
- JWT: short-lived, no refresh token
- WebSocket only; no polling
- Models: user-selectable per command (translator + workload)

---

## 6. Handoff Checklist for Next Agent

Before starting, ensure you have:

- [ ] Read `PLAN.md` (at least §1–10)
- [ ] Read `docs/WEBSOCKET_PROTOCOL.md`
- [ ] Cursor CLI `agent` command available locally
- [ ] `EXECUTOR_API_KEY` generated: `openssl rand -hex 32`

Implement Phase 1 first; get relayer serving HTTP and running migrations before adding WebSocket or executor logic.
