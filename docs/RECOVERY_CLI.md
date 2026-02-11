# Recovery CLI

Manual recovery operations for Dev PM Agent relayer. Run from relayer host or via Render shell.

---

## 1. Prerequisites

- Relayer binary with `recover` subcommand, or a separate `relayer-recover` binary
- Access to SQLite DB: `./data/relayer.db` (or `DATABASE_URL`)
- No automatic recovery in webapp; all recovery is manual

---

## 2. Commands

### 2.1 Password Reset

```
relayer recover reset-password --username <username> --new-password <password>
```

- Verifies admin exists
- Hashes new password (bcrypt/argon2)
- Updates `admin.password_hash`
- User can log in with new password; TOTP unchanged

### 2.2 TOTP Recovery

```
relayer recover reset-totp --username <username>
```

- Generates new TOTP secret
- Updates `admin.totp_secret`
- Prints new secret (base32) for user to add to authenticator
- **Warning:** Invalidates existing TOTP; user must re-add to app

### 2.3 Device Revocation

```
relayer recover revoke-device --device-id <device_id>
```

- Looks up device by `device_id` or `id`
- Deletes row from `devices`
- Device can no longer authenticate; user must re-register via keygen flow

### 2.4 List Devices

```
relayer recover list-devices
```

- Lists all devices (executor + controllers) with id, role, last_seen_at

### 2.5 Clear Stale Registration Codes

```
relayer recover clear-expired-codes
```

- Deletes rows from `device_registration_codes` where `expires_at < now()` or `used = 1`

---

## 3. Implementation Notes

- `recover` subcommands require env or flag: `--db-path ./data/relayer.db`
- No auth for recover (runs locally; user has shell access)
- Consider restricting to `RELAYER_RECOVERY_ENABLED=1` or similar to avoid accidental use in production

---

## 4. Emergency: Re-run Setup

If admin table is empty and setup was never completed, or DB is corrupted:

1. Back up `relayer.db`
2. Delete or truncate `admin`, `devices` (and optionally `commands`, `repos`)
3. Restart relayer; `POST /api/auth/setup` becomes available again
4. Re-create admin and first device
