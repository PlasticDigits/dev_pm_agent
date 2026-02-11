# Migration Strategy

Schema changes for Dev PM Agent (SQLite, relayer).

---

## 1. Tooling

- Use `sqlx` with compile-time checked migrations, or `rusqlite` with a simple migration runner
- Migrations live in `migrations/` as numbered SQL files: `001_initial.sql`, `002_xxx.sql`, …
- Relayer runs migrations on startup before accepting connections

---

## 2. Migration Format

Each file:

- **Up:** Idempotent where possible (`CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`)
- **Down:** Not required for v1 (manual restore from backup if needed)
- **Order:** Sequential by number; never edit applied migrations

---

## 3. Current → Target Schema (002)

**From:** `001_initial.sql` (legacy: pairing_codes, mfa_enabled, executor_registered)

**To:** Plan schema (device_registration_codes, simplified admin, translator/workload_model on commands)

### 002_plan_schema.sql

```sql
-- Drop deprecated admin columns
ALTER TABLE admin DROP COLUMN mfa_enabled;
ALTER TABLE admin DROP COLUMN executor_registered;

-- SQLite doesn't support DROP COLUMN before 3.35; use recreate if needed:
-- CREATE TABLE admin_new (...); INSERT INTO admin_new SELECT ...; DROP TABLE admin; ALTER TABLE admin_new RENAME TO admin;

-- Replace pairing_codes with device_registration_codes
DROP TABLE IF EXISTS pairing_codes;

CREATE TABLE IF NOT EXISTS device_registration_codes (
  id              TEXT PRIMARY KEY,
  code            TEXT UNIQUE NOT NULL,
  created_by_device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
  used            INTEGER DEFAULT 0,
  expires_at      TEXT NOT NULL,
  created_at      TEXT NOT NULL
);

-- Add model columns to commands
ALTER TABLE commands ADD COLUMN translator_model TEXT;
ALTER TABLE commands ADD COLUMN workload_model TEXT;
```

**Note:** SQLite 3.35+ supports `ALTER TABLE ... DROP COLUMN` and `ADD COLUMN`. For older SQLite, use table-recreate pattern for `admin`.

---

## 4. Migration Runner (Pseudocode)

```rust
fn run_migrations(pool: &SqlitePool) -> Result<()> {
    let migrations = std::fs::read_dir("migrations")?;
    for entry in migrations {
        let sql = std::fs::read_to_string(entry.path())?;
        pool.execute(&sql).await?;
    }
    Ok(())
}
```

Use `sqlx migrate run` if using sqlx, or a custom `_migrations` table to track applied migrations and skip already-run files.

---

## 5. Backup Before Migrate

- Copy `./data/relayer.db` before applying migrations in production
- Render: snapshot or copy from persistent disk
