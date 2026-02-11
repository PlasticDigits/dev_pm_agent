# WebSocket Protocol

Dev PM Agent uses WebSockets for real-time communication between relayer, executor, and webapp. No polling.

---

## 1. Connection

### Endpoint

```
wss://<relayer-host>/ws
```

### Authentication

**Executor:** `Authorization: Bearer <EXECUTOR_API_KEY>`
**Controller (webapp):** `Authorization: Bearer <JWT>`

Auth via query param (recommended for WebSocket):

```
wss://<relayer-host>/ws?token=<EXECUTOR_API_KEY>
wss://<relayer-host>/ws?token=<JWT>
```

Or via first text message after connect (see §2).

### Connection Lifecycle

- Executor: persistent connection; reconnect with exponential backoff on disconnect
- Controller: connect when Chat page active; disconnect on leave

---

## 2. Message Format

All messages are JSON. Shared envelope:

```json
{
  "type": "<message_type>",
  "payload": { ... },
  "ts": "<ISO8601>"
}
```

| Field     | Type   | Required | Description                    |
|-----------|--------|----------|--------------------------------|
| `type`    | string | yes      | Message type (see below)       |
| `payload` | object | yes      | Type-specific payload         |
| `ts`      | string | no       | Server timestamp (ISO 8601)   |

---

## 3. Message Types

### 3.1 `command_new` (Relayer → Executor)

Sent when a controller creates a new command. Executor consumes, runs Cursor agent, then sends status/output updates.

```json
{
  "type": "command_new",
  "payload": {
    "id": "uuid",
    "input": "string",
    "repo_path": "string | null",
    "context_mode": "string | null",
    "translator_model": "string | null",
    "workload_model": "string | null"
  },
  "ts": "2025-02-11T12:00:00Z"
}
```

### 3.2 `command_update` (Relayer → Controller)

Sent when command status, output, or summary changes. Controller uses for real-time UI updates.

```json
{
  "type": "command_update",
  "payload": {
    "id": "uuid",
    "status": "pending | running | done | failed",
    "output": "string | null",
    "summary": "string | null",
    "updated_at": "ISO8601"
  },
  "ts": "2025-02-11T12:00:00Z"
}
```

### 3.3 `command_ack` (Executor → Relayer)

Executor acknowledges it has taken ownership of a command and is about to run it.

```json
{
  "type": "command_ack",
  "payload": {
    "id": "uuid"
  }
}
```

### 3.4 `command_result` (Executor → Relayer)

Executor sends final result. Relayer stores and broadcasts `command_update` to controller(s).

```json
{
  "type": "command_result",
  "payload": {
    "id": "uuid",
    "status": "done | failed",
    "output": "string",
    "summary": "string"
  }
}
```

### 3.5 `file_read_request` (Relayer → Executor)

Sent when a controller requests to read a file from a repo. Executor reads the file from disk and POSTs the content to `/api/files/read/response`.

```json
{
  "type": "file_read_request",
  "payload": {
    "request_id": "uuid",
    "repo_path": "~/repos/foo",
    "file_path": "plans/PLAN_GAP_1.md"
  }
}
```

### 3.6 `ping` / `pong`

Keepalive. Either side may send `ping`; receiver responds with `pong`.

```json
{ "type": "ping", "payload": {} }
{ "type": "pong", "payload": {} }
```

### 3.7 `error`

Server or executor reports an error.

```json
{
  "type": "error",
  "payload": {
    "code": "string",
    "message": "string",
    "details": {}
  }
}
```

---

## 4. Flows

### 4.1 New Command (Controller → Executor → Controller)

1. Controller creates command via `POST /api/commands`
2. Relayer inserts into DB, publishes `command_new` to executor over WebSocket
3. Executor receives `command_new`, sends `command_ack`
4. Executor runs translation → execution → summarization
5. Executor sends `command_result` to relayer
6. Relayer updates DB, publishes `command_update` to controller(s) with that command in their view
7. Controller receives `command_update`, updates UI

### 4.2 Status Updates During Execution (Optional)

Executor may send incremental updates before `command_result`:

```json
{
  "type": "command_update",
  "payload": {
    "id": "uuid",
    "status": "running",
    "output": "partial output so far...",
    "summary": null
  }
}
```

Relayer forwards to controller. Controller may show live output if desired (v1: summary only when done).

---

## 5. Subscription / Scoping

- **Executor:** receives all `command_new` for its admin
- **Controller:** receives `command_update` only for commands created by that controller’s device (or all commands for the admin—decide per product)

**Recommendation:** Controller receives updates for all commands under the same admin (single-user design).

---

## 6. Reconnection

- Executor: on reconnect, relayer may send any `pending` commands that arrived while disconnected
- Controller: on reconnect, fetch recent commands via `GET /api/commands` and re-subscribe; no catch-up over WebSocket

---

## 7. Versioning

Reserve a `version` field in the envelope for future changes:

```json
{
  "version": 1,
  "type": "command_new",
  "payload": { ... }
}
```

Current version: `1`.
