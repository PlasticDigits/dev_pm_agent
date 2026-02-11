# Dev PM Agent — Project Plan

## 1. Overview

**Dev PM Agent** is a remote Cursor CLI controller that lets you run development tasks (plans, sprints, security reviews, bugfixing, implementation) from a web chat on your phone. Commands flow through a relayer to a desktop executor that translates intent into Cursor CLI invocations, runs them, and returns LLM-summarized results.

**Stack:**
- **Backend:** Rust
- **Frontend:** Vite + React + Tailwind (static SPA)
- **Database:** SQLite
- **Models:** User-selectable from active Cursor CLI models (separate for translator and workload)

---

## 2. Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────────────────┐     ┌─────────────────┐
│  Web Chat       │     │  Relayer         │     │  Executor (Desktop)         │     │  Cursor CLI     │
│  (Vite/React)   │────►│  (Rust backend)  │────►│  (Rust binary)              │────►│  (agent -p)     │
│  Static SPA     │     │  SQLite          │     │  • WebSocket for commands   │     │  User-selectable│
│                 │     │  Auth + Relay    │     │  • Translates via Composer  │     │  model          │
│                 │◄────│  WebSocket       │◄────│  • Runs agent -p            │     │                 │
└─────────────────┘     └──────────────────┘     └─────────────────────────────┘     └─────────────────┘
```

---

## 3. Monorepo Structure

```
dev_pm_agent/
├── Cargo.toml                    # Workspace root
├── PLAN.md                       # This document
├── README.md
├── docs/
│   ├── WORD_STYLE_CODES.md       # Device registration + executor API key formats
│   ├── WEBSOCKET_PROTOCOL.md     # WebSocket message types and flows
│   ├── MIGRATION_STRATEGY.md     # Schema migration approach
│   └── RECOVERY_CLI.md           # Manual recovery commands
│
├── crates/
│   ├── relayer/                  # Backend server (HTTP API, auth, relay)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── lib.rs
│   │       ├── api/
│   │       ├── auth/
│   │       ├── relay/
│   │       ├── db/
│   │       └── config.rs
│   │
│   ├── executor/                 # Desktop daemon (runs on dev PC)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── lib.rs
│   │       ├── cursor/            # Cursor CLI integration
│   │       ├── relay_client/      # WebSocket + HTTP client to relayer
│   │       └── cli/               # CLI for device registration (register-device subcommand)
│   │
│   └── shared/                   # Shared types, traits (used by relayer + executor)
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           └── models.rs
│
├── apps/
│   └── web/                      # Vite + React + Tailwind frontend
│       ├── package.json
│       ├── vite.config.ts
│       ├── tailwind.config.js
│       ├── index.html
│       └── src/
│           ├── main.tsx
│           ├── App.tsx
│           ├── components/
│           ├── pages/
│           ├── api/              # Fetch wrappers for relayer API
│           └── stores/
│
├── migrations/                   # SQLite schema migrations (used by relayer)
│   └── 001_initial.sql
│
└── config/                      # Example config files
    ├── relayer.example.toml
    └── executor.example.toml
```

---

## 4. Crate Descriptions

### 4.1 `crates/relayer`

**Purpose:** HTTP + WebSocket backend that runs on Render (or self-hosted). Handles auth, device registration, and command relay.

**Responsibilities:**
- Serve REST API and WebSocket for web chat and executor
- SQLite persistence (Render permanent disk)
- TOTP verification (mandatory for controllers), password hashing (bcrypt/argon2)
- Executor auth via shared API key (from `.env`, openssl-generated); no rotation
- Device management (executor + controllers); controllers use per-device API keys (hashed)
- Command CRUD: create, list, update status/output/summary
- WebSocket push for new commands to executor and status updates to webapp
- Input validation; repo paths must be under `~/repos/`
- No rate limiting for verified devices

**Deployment:** Single binary. Reads config from env or TOML. SQLite DB on Render permanent disk. `EXECUTOR_API_KEY` in `.env` (same value as executor).

### 4.2 `crates/executor`

**Purpose:** Desktop daemon that runs on the dev PC. Connects to relayer via WebSocket for commands, translates intent via Cursor CLI, executes, and posts results.

**Responsibilities:**
- Authenticate to relayer via `EXECUTOR_API_KEY` from `.env`
- WebSocket connection to relayer; receive new commands in real time
- CLI subcommand `register-device <word-style-code> <password>`: registers new webapp controller with relayer; displays TOTP secret string for user to add to authenticator
- Invoke Cursor CLI (`agent -p`) for:
  - **Translation:** User intent → structured Cursor prompt (repo, context_mode)
  - **Execution:** Run agent with that prompt in workspace
  - **Summarization:** Raw output → mobile-friendly summary
- Model selection: translator and workload models from config or command
- Post status (running, done, failed) and output/summary to relayer via WebSocket/HTTP

**Deployment:** Single binary. Run as systemd service or manually. Requires Cursor CLI installed and `CURSOR_API_KEY` or `agent login`. `EXECUTOR_API_KEY` in `.env` (same as relayer).

### 4.3 `crates/shared`

**Purpose:** Common types and utilities shared between relayer and executor to avoid drift.

**Contents:**
- Command DTOs (CreateCommand, CommandStatus, etc.)
- Device roles (executor, controller)
- Device registration types (word-style code flow)
- API request/response types (or derive from shared schemas)

---

## 5. Database Schema (SQLite)

### 5.1 Tables

```sql
-- Admin account (single row; single-user design). TOTP mandatory for login.
CREATE TABLE admin (
  id              TEXT PRIMARY KEY,
  username        TEXT UNIQUE NOT NULL,
  password_hash    TEXT NOT NULL,
  totp_secret     TEXT NOT NULL,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);

-- Devices: executor (1, auth by EXECUTOR_API_KEY from env) + controllers (N, per-device API key)
CREATE TABLE devices (
  id              TEXT PRIMARY KEY,
  admin_id        TEXT NOT NULL REFERENCES admin(id) ON DELETE CASCADE,
  device_id       TEXT NOT NULL,
  name            TEXT,
  role            TEXT NOT NULL CHECK (role IN ('executor', 'controller')),
  token_hash      TEXT NOT NULL,  -- hash of per-device API key (executor uses env key)
  registered_at   TEXT NOT NULL,
  last_seen_at    TEXT NOT NULL,
  UNIQUE(admin_id, device_id)
);

-- Only one executor per admin
CREATE UNIQUE INDEX idx_one_executor ON devices(admin_id) WHERE role = 'executor';

-- Device registration codes (word-style, for adding new webapp controllers)
CREATE TABLE device_registration_codes (
  id              TEXT PRIMARY KEY,
  code            TEXT UNIQUE NOT NULL,
  created_by_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  used            INTEGER DEFAULT 0,
  expires_at      TEXT NOT NULL,
  created_at      TEXT NOT NULL
);

-- Commands (relay messages)
CREATE TABLE commands (
  id                  TEXT PRIMARY KEY,
  device_id           TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  input               TEXT NOT NULL,
  status              TEXT NOT NULL DEFAULT 'pending',
  output              TEXT,
  summary             TEXT,
  repo_path           TEXT,
  context_mode        TEXT,
  translator_model    TEXT,
  workload_model      TEXT,
  created_at          TEXT NOT NULL,
  updated_at          TEXT NOT NULL
);

-- Known repos (core: repo selector in UI; user has many projects)
CREATE TABLE repos (
  id              TEXT PRIMARY KEY,
  admin_id        TEXT NOT NULL REFERENCES admin(id) ON DELETE CASCADE,
  path            TEXT NOT NULL,
  name            TEXT,
  created_at      TEXT NOT NULL
);
```

### 5.2 Migrations

- Use `rusqlite` or `sqlx` with compile-time checked migrations
- Migration runner in relayer startup
- Path: `migrations/001_initial.sql`, etc.
- Executor device row: created on first successful connection with valid `EXECUTOR_API_KEY` (auth by env, not DB)

---

## 6. API Design (Relayer)

### 6.1 Auth Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/auth/setup` | None (first-run only) | Create admin and first controller device. Body: `{ username, password }`. Returns `{ totp_secret, device_api_key }`. |
| POST | `/api/auth/login` | None | Body: `{ device_api_key, password, totp_code }`. Returns short-lived JWT. |

### 6.2 Device Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/devices/reserve-code` | Bearer (controller) | Reserve a device registration code. Body: `{ code }` (word-style from webapp keygen). Returns `{ expires_at }`. |
| POST | `/api/auth/register-device` | Executor API key | Register new controller. Body: `{ code, password }`. Called by executor CLI. Returns `{ device_api_key, totp_secret }`; executor displays for user. |

### 6.3 Command Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| POST | `/api/commands` | Bearer (controller) | Create command. Body: `{ input, repo_path?, context_mode?, translator_model?, workload_model? }`. |
| GET | `/api/commands` | Bearer | List commands (filter by device, status). |
| GET | `/api/commands/{id}` | Bearer | Get command details. |
| WS | `/ws` | Bearer (executor or controller) | WebSocket: executor receives new commands; controller receives status/output updates. No polling. |
| PATCH | `/api/commands/{id}` | Bearer (executor) | Update status, output, summary. |

### 6.4 Repo Endpoints

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/repos` | Bearer | List known repos. |
| POST | `/api/repos` | Bearer | Add repo. Body: `{ path, name }`. Path must be under `~/repos/`. |

---

## 7. Cursor CLI Integration

### 7.1 Why Composer / Agent Models

- Advanced agentic model (thinking, self-summarization)
- Cost-effective (covered by Cursor subscription)
- Headless mode (`agent -p`) for automation
- JSON output for structured responses

### 7.2 Model Selection

- Webapp allows user to select from active Cursor CLI models for **translator** (intent → structured prompt) and **workload** (execution) separately.
- Executor fetches model list via `agent models` or from config; webapp fetches from relayer endpoint that queries executor or caches model list.

### 7.3 Usage Modes

**1. Intent → Command Translation**

```
agent -p -m <translator_model> --output-format json \
  "Given this user input, produce a JSON object with: repo_path, cursor_prompt, context_mode. Input: \"...\""
```

- Parse JSON for `repo_path`, `cursor_prompt`, `context_mode`.
- `repo_path` must be under `~/repos/`; validate and error if not.

**2. Cursor Execution**

```
agent -p -m <workload_model> --output-format json --force \
  -C /path/to/repo \
  "<cursor_prompt from step 1>"
```

- `-C` sets working directory (validated: under `~/repos/`).
- `--force` allows file modifications without confirmation.

**3. Output Summarization**

```
agent -p -m <workload_model> --output-format text \
  "Summarize this Cursor CLI output for mobile display in 3-5 bullet points. Keep it under 500 chars: <raw output>"
```

### 7.4 Executor Flow (Pseudocode)

```
1. Connect WebSocket to relayer (auth: EXECUTOR_API_KEY)
2. On new command message:
   a. PATCH status = 'running'
   b. Run translation prompt → get repo_path, cursor_prompt, context_mode (validate repo_path)
   c. Run agent -p with cursor_prompt in repo_path
   d. Run summarization prompt on raw output
   e. PATCH output, summary, status = 'done'
   f. Push status/output via WebSocket to webapp
3. Stay connected
```

### 7.5 Environment

- `CURSOR_API_KEY` or `agent login` for auth
- Executor must have Cursor CLI on PATH

---

## 8. Command Presets (Hardcoded Templates)

All templates are hardcoded (no config). Two categories: **sprints** (implementation) and **plan flows** (create → interactive review/update).

### 8.1 Plan Flows (Create → Interactive Review/Update)

| Preset | Behavior |
|--------|----------|
| **Monorepo init** | Create new monorepo with `plans/` folder and `PLAN_INITIAL.md`. Then interactive review and update of the file. |
| **Gap analysis** | Write `PLAN_GAP_{x}.md`, then interactive review and update. |
| **Security review** | Write `PLAN_SECURITY_{x}.md`, then interactive review and update. |
| **Feature plan** | Write `PLAN_FEAT_{x}.md`, then interactive review and update. |

User selects plan or creates new plan; triggers interactive review/update.

### 8.2 Sprints (Implementation)

| Preset | Behavior |
|--------|----------|
| **Sprint** | Select a plan. If no sprint doc exists, create new sprint doc. Otherwise write new sprint doc for handoff from previous agent to new sprint. If latest sprint doc not yet implemented, implement it. |

---

## 9. Frontend (Vite + React + Tailwind)

### 9.1 Structure

```
apps/web/src/
├── main.tsx
├── App.tsx
├── api/
│   ├── client.ts          # fetch wrapper, base URL, auth header
│   ├── auth.ts
│   ├── commands.ts
│   ├── devices.ts
│   └── ws.ts              # WebSocket client for status updates
├── components/
│   ├── LoginForm.tsx
│   ├── TotpInput.tsx
│   ├── DeviceKeygen.tsx   # "API keygen" button: generate word-style code, reserve via API, display
│   ├── CommandInput.tsx   # Text input + preset selector + model selectors
│   ├── CommandList.tsx
│   ├── CommandCard.tsx    # Status, output preview, summary
│   └── RepoSelector.tsx
├── pages/
│   ├── Login.tsx
│   ├── Setup.tsx          # First-run: username + password → TOTP secret
│   ├── AddDevice.tsx      # Keygen flow for adding new device
│   ├── Chat.tsx           # Main command interface
│   └── Settings.tsx       # Repos
├── stores/
│   ├── auth.ts            # JWT, user state
│   └── commands.ts        # Local cache, WebSocket updates
└── types/
    └── index.ts
```

### 9.2 Key Flows

**Setup (first-run only):**
1. Username + password
2. Relayer creates admin and first controller device, returns `{ totp_secret, device_api_key }`
3. User adds TOTP to authenticator; webapp stores device_api_key
4. Redirect to Chat

**Add Device (additional webapp controller, e.g. new phone):**
1. User on registered device clicks "API keygen" → DeviceKeygen generates word-style code
2. Webapp calls POST `/api/devices/reserve-code` with code
3. Webapp displays code
4. User runs executor CLI: `executor register-device <code> <password>`
5. Executor displays device_api_key and totp_secret (for adding to authenticator on new device)
6. User on new device: enter device_api_key + password + TOTP → login

**Login:**
1. device_api_key + password + TOTP
2. Store JWT, redirect to Chat

**Chat:**
1. Command input (text or preset)
2. Repo selector (core)
3. Model selectors (translator, workload)
4. Optional context mode (continue / new)
5. Submit → WebSocket for real-time status updates
6. Display status (pending, running, done) and summary when ready

### 9.3 Build

- `npm run build` → `dist/` static assets
- Deploy to Render as static site (same Render account as relayer; CORS: same origin or configured)
- `VITE_RELAYER_URL` env for API base URL

---

## 10. Security

### 10.1 Auth

- Password: bcrypt or Argon2 (cost factor 12+)
- TOTP: mandatory for controllers; 6 digits, 30s window
- Executor: authenticated by `EXECUTOR_API_KEY` from `.env` (openssl-generated); no TOTP
- JWT: short-lived (e.g. 1h); no refresh token (re-login on expiry; rotation during annual security review)
- Per-device API keys: hash before storage; controllers use for login identity

### 10.2 Device Registration

- Executor: validated by shared API key in `.env` on both relayer and executor; no rotation
- Controllers: word-style registration code from webapp keygen; executor CLI registers with password; single-use, short expiry
- **Word-style code:** 4 words from EFF short list, hyphen-separated (e.g. `echo-brick-zeta-quip`). See `docs/WORD_STYLE_CODES.md`.

### 10.3 API

- HTTPS only (TLS at Render)
- No rate limiting for verified devices
- Input validation: max command length (e.g. 4KB)
- Repo paths: must be under `~/repos/`; validate and reject with error if not
- CORS: allow frontend origin (both on Render)

### 10.4 Executor

- `CURSOR_API_KEY` and `EXECUTOR_API_KEY` in `.env` only; never in committed config
- File access: only `~/repos/` directory

### 10.5 Recovery

- Done manually via relayer CLI (e.g. password reset, TOTP recovery, device revocation)
- Document relayer CLI recovery commands; no automatic recovery flows in webapp

---

## 11. Configuration

### 11.1 Relayer (env + `config/relayer.example.toml`)

**Required env:** `EXECUTOR_API_KEY` (openssl rand -hex 32), `JWT_SECRET`

```toml
[server]
host = "0.0.0.0"
port = 8080

[database]
path = "./data/relayer.db"

[auth]
token_ttl_secs = 3600

[device_registration]
code_ttl_secs = 600
```

### 11.2 Executor (env + `config/executor.example.toml`)

**Required env:** `EXECUTOR_API_KEY` (same as relayer), `CURSOR_API_KEY`

```toml
[relayer]
url = "https://your-relayer.onrender.com"
ws_url = "wss://your-relayer.onrender.com/ws"

[cursor]
default_translator_model = "composer-1.5"
default_workload_model = "composer-1.5"
default_repo = "~/repos/default"
```

---

## 12. Deployment

**Order:** Relayer → Executor → Webapp. All use `.env` for secrets.

### 12.1 Relayer (Render)

- **Service type:** Web Service
- **Build:** `cargo build --release -p relayer`
- **Start:** `./target/release/relayer` or `relayer`
- **Database:** SQLite on Render permanent disk (attach persistent disk for `./data/relayer.db`)
- **Env:** `EXECUTOR_API_KEY`, `JWT_SECRET` (from `.env`)

### 12.2 Frontend (Render)

- **Service type:** Static Site
- Build `apps/web`, deploy `dist/`
- Set `VITE_RELAYER_URL` to relayer URL
- Both webapp and relayer on Render; configure CORS for same-origin or cross-origin as needed

### 12.3 Executor

- Build: `cargo build --release -p executor`
- Run on desktop: `./executor` or systemd service
- **Env:** `EXECUTOR_API_KEY` (same as relayer), `CURSOR_API_KEY`; add to `.env` on executor PC
- Prereqs: Cursor CLI installed

---

## 13. Development Workflow

### 13.1 Local Dev

1. **Relayer:** `cargo run -p relayer` (SQLite at `./data/relayer.db`)
2. **Executor:** `cargo run -p executor` (point to `http://localhost:8080` and `ws://localhost:8080/ws`)
3. **Frontend:** `cd apps/web && npm run dev` (proxy API and WebSocket to relayer)

### 13.2 Testing

- Relayer: `cargo test -p relayer`
- Shared: `cargo test -p shared`
- Frontend: Vitest or similar for unit tests; Playwright for E2E (optional)

### 13.3 Build All

```bash
cargo build --release
cd apps/web && npm run build
```

---

## 14. Out of Scope (Initial Version)

- Telegram bot integration
- Multiple admin accounts
- Streaming command output to UI (status/summary only)
- Executor clustering (multiple desktops)
- Cursor CLI session persistence (`--resume`)

---

## 15. Future Considerations

- Command cancellation from UI
- Executor self-update
- Backup/restore of SQLite
- Audit log of commands

---

## 16. Dependencies (Rust)

### 16.1 Relayer

- `axum` — HTTP + WebSocket server
- `tokio` — async runtime
- `rusqlite` or `sqlx` — SQLite
- `bcrypt` or `argon2` — password hashing
- `totp` or `oath` — TOTP
- `jsonwebtoken` — JWT
- `serde`, `serde_json` — serialization
- `tower` / `tower-http` — middleware (CORS)
- `tokio-tungstenite` — WebSocket
- `thiserror`, `anyhow` — error handling

### 16.2 Executor

- `reqwest` — HTTP client to relayer
- `tokio-tungstenite` — WebSocket client
- `tokio` — async
- `serde`, `serde_json`
- `clap` — CLI args (including `register-device` subcommand)
- `directories` — config path

### 16.3 Shared

- `serde`, `serde_json`
- `uuid` — IDs

---

## 17. Checklist Before Implementation

- [x] Verify Cursor CLI `agent` command and model names (`agent models`)
- [x] Generate `EXECUTOR_API_KEY`: `openssl rand -hex 32`
- [x] Define word-style code format → `docs/WORD_STYLE_CODES.md`
- [x] WebSocket protocol → `docs/WEBSOCKET_PROTOCOL.md`
- [x] Migration strategy → `docs/MIGRATION_STRATEGY.md`, `migrations/002_plan_schema.sql`
- [x] Recovery CLI → `docs/RECOVERY_CLI.md`

---

*End of plan. No code—review and iterate before implementation.*
