# Security Review — Dev PM Agent

**Date:** February 12, 2025  
**Scope:** Full codebase (relayer, executor, web app)  
**Focus areas:** Credential generation/storage/rotation, forgery prevention, access control

---

## Executive Summary

This security review examines the Dev PM Agent codebase across three focus areas. The system implements multi-factor authentication (device key, password, TOTP), JWT-based session tokens, and role-based API authorization. Several risks were identified that could lead to unauthorized access; findings are categorized by severity with recommended mitigations.

**Findings summary:**

| Severity | Count |
|----------|-------|
| Critical | 0 |
| High | 1 (1 fixed) |
| Medium | 4 |
| Low | 3 |
| Info | 2 |

---

## 1. Credential Generation, Storage, and Rotation

### Finding 1.1: Device API Key and JWT Stored in localStorage (Medium)

**Location:** `apps/web/src/stores/auth.ts`  
**Observation:** JWT tokens and device API keys are stored in `localStorage`:

```typescript
const TOKEN_KEY = 'jwt';
const DEVICE_KEY = 'device_api_key';
localStorage.setItem(TOKEN_KEY, token);
localStorage.setItem(DEVICE_KEY, key);
```

**Risk:** localStorage is accessible to any JavaScript running in the same origin. XSS vulnerabilities (e.g., via a compromised dependency or third-party script) could exfiltrate tokens and device keys. The device key is particularly sensitive as it is a long-lived credential used for login and persists until explicitly cleared.

**Recommendation:**
- Store JWTs in `sessionStorage` instead of `localStorage` to limit exposure to the tab lifecycle.
- Consider `httpOnly` cookies for JWTs if the relayer can be served from the same origin (requires backend changes).
- Document that the device key should not be stored in browser storage; the README already states "The device key is never stored in the browser — enter it at each login." Enforcing this (e.g., prompt for device key on each login rather than persisting it) would reduce risk.

### Finding 1.2: Weak Client-Side Salt Default (High)

**Location:** `apps/web/src/api/auth.ts`  
**Observation:** Client-side password hashing uses a fallback salt:

```typescript
const CLIENT_SALT = import.meta.env.VITE_CLIENT_SALT || 'dev-pm-agent-default-salt-change-in-env';
```

**Risk:** If `VITE_CLIENT_SALT` is not set at build time (e.g., in development or misconfigured deployment), the default salt is used. This makes client-side hashes predictable across installations and weakens protection against credential reuse across deployments.

**Recommendation:**
- Fail fast at build/runtime if `VITE_CLIENT_SALT` is unset in production; do not allow the default.
- Add a build check or startup validation that errors when the default salt would be used.

### Finding 1.3: No JWT Revocation or Token Blacklist (Medium)

**Location:** `crates/relayer/src/auth/mod.rs`, `crates/relayer/src/api/routes.rs`  
**Observation:** JWTs are validated by signature and expiration only. There is no revocation mechanism (e.g., token blacklist, server-side session store).

**Risk:** A stolen JWT remains valid until it expires (`JWT_TTL_SECS`, default 3600s). The refresh flow accepts expired tokens within a 24-hour grace period (`jwt_refresh_grace_secs`), so a leaked refresh token could be abused for an extended window.

**Recommendation:**
- Document that credential compromise requires changing `JWT_SECRET` and forcing all users to re-login.
- Consider adding optional token revocation (e.g., per-device or per-user revocation list) for high-assurance deployments.
- Reduce refresh grace period for higher-security deployments.

### Finding 1.4: Credential Rotation Documentation (Info)

**Location:** `README.md`, `.env.example`  
**Observation:** README documents rotation: set new env vars, restart relayer; JWTs expire by TTL; users re-login. Device API keys remain valid until devices are re-registered.

**Recommendation:** Add a recovery/rotation runbook (e.g., in `docs/RECOVERY_CLI.md`) that covers:
- Rotating `JWT_SECRET`, `EXECUTOR_API_KEY`, `PASSWORD_SALT`, `CLIENT_SALT`
- Steps to invalidate all sessions and device keys

---

## 2. Preventing Forgery (Spoofing, Tampering, Replay)

### Finding 2.1: No JWT Replay Protection (Medium)

**Location:** `crates/relayer/src/auth/mod.rs`  
**Observation:** JWT claims include `exp` and `iat` but no `jti` (unique token ID). Refreshed tokens are issued without invalidating the previous token.

**Risk:** A captured JWT can be replayed until expiration. There is no one-time-use semantics for refresh; an attacker with a valid (even expired) token can request a new one within the grace period repeatedly.

**Recommendation:**
- Add `jti` to JWT claims and optionally maintain a short-lived allowlist of recently issued tokens for critical operations.
- For refresh: consider issuing refresh tokens that are single-use (store and invalidate on use) to prevent refresh token replay.

### Finding 2.2: JWT Algorithm Explicitly Restricted (Positive)

**Location:** `crates/relayer/src/auth/mod.rs`  
**Observation:** Validation uses `Validation::new(Algorithm::HS256)` and `DecodingKey::from_secret`, ensuring only HS256 is accepted.

**Recommendation:** No action. Algorithm confusion (e.g., `alg: none`) is mitigated.

### Finding 2.3: Constant-Time Comparisons for Credential Verification (Positive)

**Location:** `crates/relayer/src/db/mod.rs`  
**Observation:** `exists_bootstrap_device`, `take_bootstrap_device`, and `validate_device` perform a dummy bcrypt verify when no match is found (`DUMMY_BCRYPT_HASH`), reducing timing side channels.

**Recommendation:** No action. Good practice.

### Finding 2.4: Device Registration Code Predictability (Low)

**Location:** `apps/web/src/utils/wordCode.ts`, `crates/relayer/src/db/mod.rs`  
**Observation:** Word-style codes use 4 words from a 256-word list. Entropy ≈ 4 × log2(256) = 32 bits. Codes expire in 600 seconds (configurable).

**Risk:** 32-bit entropy is brute-forceable with enough attempts. Rate limiting on `auth_register_device` would reduce this; currently only auth routes have rate limiting, and `auth_register_device` requires `EXECUTOR_API_KEY`, so the attack surface is limited to compromised executor or insider.

**Recommendation:**
- Consider 5–6 words for higher entropy if codes can be brute-forced (e.g., if an endpoint ever exposed code verification without the executor key).
- Ensure `auth_register_device` is never exposed to unauthenticated clients.

### Finding 2.5: WebSocket Auth Token in First Message Only (Info)

**Location:** `crates/relayer/src/api/routes.rs` — `handle_socket`  
**Observation:** WebSocket auth is performed once via the first message `{"type":"auth","payload":{"token":"..."}}`. Subsequent messages are not re-authenticated.

**Risk:** Session hijacking after initial auth (e.g., via WebSocket session fixation) is theoretically possible but mitigated by TLS and same-origin policies.

**Recommendation:** Document that TLS is required for production; ensure HTTPS/WSS in deployment.

---

## 3. Access Control

### Finding 3.1: CORS Allows Any Origin (High) — **FIXED**

**Location:** `crates/relayer/src/api/mod.rs`  
**Observation:** Previously used `allow_origin(Any)`, allowing any website to make credentialed requests.

**Fix applied:** CORS is now restricted via `CORS_ALLOWED_ORIGINS` env var (comma-separated list of frontend URLs). If unset, defaults to `http://localhost:5173` and `http://127.0.0.1:5173` for local development. Allowed methods and headers are explicitly listed (GET, POST, PATCH, DELETE, OPTIONS; Authorization, Content-Type).

### Finding 3.2: Per-Route Authorization (Positive)

**Location:** `crates/relayer/src/api/routes.rs`  
**Observation:** Protected routes call `extract_bearer_from_headers` and `verify_bearer`; role checks (e.g., `commands_update` only for executor) are enforced. Controller JWTs cannot update command status; executor API key is required.

**Recommendation:** No action. Role-based checks are correctly applied.

### Finding 3.3: Executor API Key Grants Broad Access (Medium)

**Location:** `crates/relayer/src/api/routes.rs` — `verify_bearer`, `handle_socket`  
**Observation:** When the bearer token equals `EXECUTOR_API_KEY`, the relayer treats the client as "executor" and grants:
- Command status updates
- File read/search responses
- Repo sync, models sync
- WebSocket relay subscription

The executor key is a shared secret between relayer and executor binaries. A single key is used; there is no per-executor identity.

**Risk:** Compromise of `EXECUTOR_API_KEY` (e.g., via env leak, process inspection) gives full executor capabilities: forge command outputs, read arbitrary file content from executor host, inject models/repos.

**Recommendation:**
- Treat `EXECUTOR_API_KEY` as highly sensitive; ensure it is not logged or exposed in error messages.
- Consider separate scoped keys for different operations if the threat model demands it.
- Run executor in a restricted environment (e.g., dedicated user, minimal env).

### Finding 3.4: Path Traversal Mitigation (Positive)

**Location:** `crates/relayer/src/db/mod.rs` — `validate_repo_path`; `crates/executor/src/relay_client/ws.rs`  
**Observation:** Relayer validates repo paths under `~/repos/` and rejects `..`. Executor performs canonical path checks to prevent traversal out of repo root.

**Recommendation:** No action. Path validation is implemented correctly on relayer; executor adds defense in depth.

### Finding 3.5: Executor Repo Path Validation Weaker Than Relayer (Low)

**Location:** `crates/executor/src/cursor/mod.rs`  
**Observation:**

```rust
fn validate_repo_path(path: &str) -> Result<()> {
    let expanded = shellexpand::tilde(path).to_string();
    if !expanded.contains(REPOS_PREFIX) {
        anyhow::bail!("repo path must be under ~/repos/");
    }
    Ok(())
}
```

**Risk:** `contains("repos")` accepts paths like `/tmp/repos/foo` or `~/repos_backup/bar`, which may not align with the intended `~/repos/` policy. Tests show `/home/user/repos/project` is accepted.

**Recommendation:**
- Align with relayer logic: require path to start with `~/repos` or be under `$HOME/repos/` after expansion.
- Reject paths containing `..` or symbolic link tricks that escape the repo root.

### Finding 3.6: Auth Rate Limit Configuration Ambiguity (Low)

**Location:** `crates/relayer/src/api/routes.rs`  
**Observation:** Rate limit uses `GovernorConfigBuilder::default().per_second(15).burst_size(5)`. Comment says "5 requests per burst, 1 replenish every 15 seconds."

**Risk:** If `per_second(15)` means 15 tokens per second, the limit is very permissive and brute-force may be feasible. The intended strict limit (5 initial, slow replenish) should be verified against the governor API.

**Recommendation:**
- Confirm governor semantics (per_second vs replenishment interval).
- Add tests or logging to validate that the intended rate (e.g., ~4 requests/minute) is enforced.
- Consider stricter limits for login/verify-bootstrap if needed.

### Finding 3.7: Unprotected Routes Inventory (Info)

**Location:** `crates/relayer/src/api/routes.rs`  
**Observation:** Routes without Bearer auth:
- `auth/verify-bootstrap` — no auth; returns `{ valid }` for device key check
- `auth/setup` — no auth; requires valid bootstrap device key in body
- `auth/login` — no auth; requires device key, password hash, TOTP in body

Routes with Bearer auth: all other `/api/*` endpoints and WebSocket.

**Recommendation:** Ensure `auth/verify-bootstrap` does not leak sensitive data (it only returns boolean). Rate limiting on auth routes (already applied) helps mitigate enumeration.

---

## 4. Additional Observations

### Secrets Management

- **Gitleaks:** `.gitleaks.toml` extends default rules; example/config paths are allowlisted. Good.
- **Env:** Secrets (`JWT_SECRET`, `EXECUTOR_API_KEY`, `PASSWORD_SALT`, `CLIENT_SALT`) loaded from environment. Render blueprint uses `sync: false` for secrets. Good.
- **Render webapp:** `render.yaml` does not show `VITE_CLIENT_SALT` or `VITE_RELAYER_URL` for the static site. These must be set at build time for production.

### Health Endpoint

- `/health` returns `"ok"` with no auth. Acceptable for load balancer checks; no sensitive data.

---

## 5. Recommended Actions Summary

| ID | Severity | Action |
|----|----------|--------|
| 1.1 | Medium | Prefer sessionStorage for JWT; avoid storing device key in localStorage when possible |
| 1.2 | High | Fail if `VITE_CLIENT_SALT` is unset; disallow default salt |
| 1.3 | Medium | Document token revocation/rotation; consider optional revocation |
| 2.1 | Medium | Add `jti` to JWTs; consider single-use refresh tokens |
| 2.4 | Low | Consider stronger word-code entropy for registration |
| 3.1 | High | ~~Restrict CORS to configured frontend origin(s)~~ Fixed |
| 3.3 | Medium | Harden handling of `EXECUTOR_API_KEY`; avoid exposure in logs/errors |
| 3.5 | Low | Align executor `validate_repo_path` with relayer policy |
| 3.6 | Low | Verify auth rate limit semantics and add tests |

---

## Appendix: File Reference

| Component | Key Files |
|-----------|-----------|
| Relayer auth | `crates/relayer/src/auth/mod.rs` |
| Relayer routes | `crates/relayer/src/api/routes.rs` |
| Relayer config | `crates/relayer/src/config.rs` |
| Database | `crates/relayer/src/db/mod.rs` |
| Web auth | `apps/web/src/api/auth.ts`, `apps/web/src/stores/auth.ts` |
| Executor | `crates/executor/src/relay_client/ws.rs`, `crates/executor/src/cursor/mod.rs` |
