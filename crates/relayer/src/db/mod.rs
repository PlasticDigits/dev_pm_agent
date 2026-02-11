//! Database access.

mod migrations;

use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use uuid::Uuid;

pub use migrations::run_migrations;

/// Database connection wrapper.
pub struct Db(pub Mutex<Connection>);

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Self(Mutex::new(conn)))
    }

    pub fn run_migrations(&self) -> Result<()> {
        run_migrations(&self.0.lock().unwrap())
    }
}

/// Check if admin exists.
pub fn admin_exists(conn: &Connection) -> Result<bool> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM admin", [], |row| row.get(0))?;
    Ok(count > 0)
}

/// Create admin and first controller device (setup).
pub fn setup_admin(
    conn: &Connection,
    username: &str,
    password_hash: &str,
    totp_secret: &str,
    device_api_key_hash: &str,
) -> Result<()> {
    let admin_id = Uuid::new_v4();
    let device_id = Uuid::new_v4();
    let now = chrono_iso8601();

    conn.execute(
        "INSERT INTO admin (id, username, password_hash, totp_secret, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![
            admin_id.to_string(),
            username,
            password_hash,
            totp_secret,
            now,
        ],
    )?;

    conn.execute(
        "INSERT INTO devices (id, admin_id, device_id, name, role, token_hash, registered_at, last_seen_at)
         VALUES (?1, ?2, ?1, 'default', 'controller', ?3, ?4, ?4)",
        params![device_id.to_string(), admin_id.to_string(), device_api_key_hash, now],
    )?;

    Ok(())
}

/// Get admin by username.
pub fn get_admin(conn: &Connection, username: &str) -> Result<Option<(String, String, String)>> {
    let mut stmt =
        conn.prepare("SELECT id, password_hash, totp_secret FROM admin WHERE username = ?1")?;
    let row = stmt.query_row([username], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    });
    match row {
        Ok(r) => Ok(Some(r)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Validate device by API key hash; returns (device_id, admin_id, role).
pub fn validate_device(
    conn: &Connection,
    api_key_hash: &str,
) -> Result<Option<(Uuid, Uuid, String)>> {
    let mut stmt = conn.prepare("SELECT id, admin_id, role FROM devices WHERE token_hash = ?1")?;
    let row = stmt.query_row([api_key_hash], |row| {
        Ok((
            Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
            Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
            row.get::<_, String>(2)?,
        ))
    });
    match row {
        Ok(r) => Ok(Some(r)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Reserve a device registration code.
pub fn reserve_code(
    conn: &Connection,
    code: &str,
    created_by_device_id: Uuid,
    expires_at: &str,
) -> Result<()> {
    let id = Uuid::new_v4();
    let now = chrono_iso8601();
    conn.execute(
        "INSERT INTO device_registration_codes (id, code, created_by_device_id, used, expires_at, created_at)
         VALUES (?1, ?2, ?3, 0, ?4, ?5)",
        params![id.to_string(), code, created_by_device_id.to_string(), expires_at, now],
    )?;
    Ok(())
}

/// Consume a registration code and create new controller device.
/// Returns totp_secret on success.
pub fn register_device(
    conn: &Connection,
    code: &str,
    password: &str,
    device_api_key_hash: &str,
) -> Result<Option<String>> {
    let now = chrono_iso8601();

    // Find code and validate
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT created_by_device_id, expires_at FROM device_registration_codes WHERE code = ?1 AND used = 0",
            [code],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok();

    let (created_by_device_id, expires_at) = match row {
        Some(r) => r,
        None => return Ok(None),
    };

    if expires_at < now {
        return Ok(None);
    }

    // Get admin_id from creating device
    let admin_id: String = conn.query_row(
        "SELECT admin_id FROM devices WHERE id = ?1",
        [&created_by_device_id],
        |row| row.get(0),
    )?;

    // Verify password (password is plain text from user)
    let stored_hash: String = conn.query_row(
        "SELECT password_hash FROM admin WHERE id = ?1",
        [&admin_id],
        |row| row.get(0),
    )?;
    if !bcrypt::verify(password, &stored_hash).unwrap_or(false) {
        return Ok(None);
    }

    // Get totp_secret
    let totp_secret: String = conn.query_row(
        "SELECT totp_secret FROM admin WHERE id = ?1",
        [&admin_id],
        |row| row.get(0),
    )?;

    let device_id = Uuid::new_v4();
    conn.execute(
        "INSERT INTO devices (id, admin_id, device_id, name, role, token_hash, registered_at, last_seen_at)
         VALUES (?1, ?2, ?1, 'controller', 'controller', ?3, ?4, ?4)",
        params![device_id.to_string(), admin_id, device_api_key_hash, now],
    )?;

    conn.execute(
        "UPDATE device_registration_codes SET used = 1 WHERE code = ?1",
        [code],
    )?;

    Ok(Some(totp_secret))
}

/// Create a new command.
pub fn create_command(
    conn: &Connection,
    device_id: Uuid,
    input: &str,
    repo_path: Option<&str>,
    context_mode: Option<&str>,
    translator_model: Option<&str>,
    workload_model: Option<&str>,
) -> Result<Uuid> {
    let id = Uuid::new_v4();
    let now = chrono_iso8601();
    conn.execute(
        "INSERT INTO commands (id, device_id, input, status, output, summary, repo_path, context_mode, translator_model, workload_model, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'pending', NULL, NULL, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            id.to_string(),
            device_id.to_string(),
            input,
            repo_path,
            context_mode,
            translator_model,
            workload_model,
            now,
        ],
    )?;
    Ok(id)
}

/// Get command by id.
pub fn get_command(
    conn: &Connection,
    id: Uuid,
) -> Result<
    Option<(
        Uuid,
        Uuid,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
    )>,
> {
    let mut stmt = conn.prepare(
        "SELECT id, device_id, input, status, output, summary, repo_path, context_mode, translator_model, workload_model, created_at, updated_at FROM commands WHERE id = ?1",
    )?;
    let row = stmt.query_row([id.to_string()], |row| {
        Ok((
            Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
            Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(7)?,
            row.get(8)?,
            row.get(9)?,
            row.get(10)?,
            row.get(11)?,
        ))
    });
    match row {
        Ok(r) => Ok(Some(r)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// List commands for admin.
pub fn list_commands(
    conn: &Connection,
    admin_id: Uuid,
    limit: i64,
) -> Result<
    Vec<(
        Uuid,
        Uuid,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        String,
    )>,
> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.device_id, c.input, c.status, c.output, c.summary, c.repo_path, c.context_mode, c.translator_model, c.workload_model, c.created_at, c.updated_at
         FROM commands c
         JOIN devices d ON c.device_id = d.id
         WHERE d.admin_id = ?1
         ORDER BY c.created_at DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![admin_id.to_string(), limit], |row| {
        Ok((
            Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
            Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            row.get(5)?,
            row.get(6)?,
            row.get(7)?,
            row.get(8)?,
            row.get(9)?,
            row.get(10)?,
            row.get(11)?,
        ))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Update command status, output, summary.
pub fn update_command(
    conn: &Connection,
    id: Uuid,
    status: Option<&str>,
    output: Option<&str>,
    summary: Option<&str>,
) -> Result<bool> {
    let now = chrono_iso8601();
    let rows = if let Some(s) = status {
        conn.execute(
            "UPDATE commands SET status = ?1, output = COALESCE(?2, output), summary = COALESCE(?3, summary), updated_at = ?4 WHERE id = ?5",
            params![s, output, summary, now, id.to_string()],
        )?
    } else {
        conn.execute(
            "UPDATE commands SET output = COALESCE(?1, output), summary = COALESCE(?2, summary), updated_at = ?3 WHERE id = ?4",
            params![output, summary, now, id.to_string()],
        )?
    };
    Ok(rows > 0)
}

/// Get next pending command for executor (by admin_id).
pub fn get_pending_command(
    conn: &Connection,
    admin_id: Uuid,
) -> Result<
    Option<(
        Uuid,
        Uuid,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )>,
> {
    let row = conn.query_row(
        "SELECT c.id, c.device_id, c.input, c.repo_path, c.context_mode, c.translator_model, c.workload_model
         FROM commands c
         JOIN devices d ON c.device_id = d.id
         WHERE d.admin_id = ?1 AND c.status = 'pending'
         ORDER BY c.created_at ASC
         LIMIT 1",
        [admin_id.to_string()],
        |row| {
            Ok((
                Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
                Uuid::parse_str(&row.get::<_, String>(1)?).unwrap(),
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        },
    );
    match row {
        Ok(r) => Ok(Some(r)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// List repos for admin.
pub fn list_repos(
    conn: &Connection,
    admin_id: Uuid,
) -> Result<Vec<(Uuid, String, Option<String>, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, name, created_at FROM repos WHERE admin_id = ?1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([admin_id.to_string()], |row| {
        Ok((
            Uuid::parse_str(&row.get::<_, String>(0)?).unwrap(),
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
        ))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Add repo. Validates path is under ~/repos/
pub fn add_repo(conn: &Connection, admin_id: Uuid, path: &str, name: Option<&str>) -> Result<Uuid> {
    let expanded = shellexpand::tilde(path).to_string();
    if !expanded.contains("repos") {
        return Err(anyhow::anyhow!("repo path must be under ~/repos/"));
    }
    let id = Uuid::new_v4();
    let now = chrono_iso8601();
    conn.execute(
        "INSERT INTO repos (id, admin_id, path, name, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id.to_string(), admin_id.to_string(), expanded, name, now],
    )?;
    Ok(id)
}

fn chrono_iso8601() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{generate_api_key, generate_totp_secret, hash_api_key};

    fn in_memory_db_with_migrations() -> Connection {
        let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../migrations")
            .canonicalize()
            .unwrap();
        std::env::set_var("MIGRATIONS_DIR", migrations_dir);
        let conn = Connection::open(":memory:").unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn admin_exists_false_initially() {
        let conn = in_memory_db_with_migrations();
        assert!(!admin_exists(&conn).unwrap());
    }

    #[test]
    fn setup_admin_and_admin_exists() {
        let conn = in_memory_db_with_migrations();
        let api_key = generate_api_key();
        let api_key_hash = hash_api_key(&api_key).unwrap();
        let totp_secret = generate_totp_secret().unwrap();
        let password_hash = bcrypt::hash("testpass123", bcrypt::DEFAULT_COST).unwrap();

        setup_admin(&conn, "admin1", &password_hash, &totp_secret, &api_key_hash).unwrap();

        assert!(admin_exists(&conn).unwrap());
    }

    #[test]
    fn validate_device_with_generated_key() {
        let conn = in_memory_db_with_migrations();
        let api_key = generate_api_key();
        let api_key_hash = hash_api_key(&api_key).unwrap();
        let totp_secret = generate_totp_secret().unwrap();
        let password_hash = bcrypt::hash("testpass123", bcrypt::DEFAULT_COST).unwrap();

        setup_admin(&conn, "admin1", &password_hash, &totp_secret, &api_key_hash).unwrap();

        let result = validate_device(&conn, &api_key_hash).unwrap();
        assert!(result.is_some());
        let (device_id, admin_id, role) = result.unwrap();
        assert_eq!(role, "controller");
        assert_ne!(device_id, admin_id);
    }

    #[test]
    fn validate_device_rejects_unknown_hash() {
        let conn = in_memory_db_with_migrations();
        let unknown_hash = hash_api_key(&generate_api_key()).unwrap();
        let result = validate_device(&conn, &unknown_hash).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn reserve_code_and_register_device() {
        let conn = in_memory_db_with_migrations();
        let api_key = generate_api_key();
        let api_key_hash = hash_api_key(&api_key).unwrap();
        let totp_secret = generate_totp_secret().unwrap();
        let password_hash = bcrypt::hash("regpass", bcrypt::DEFAULT_COST).unwrap();

        setup_admin(&conn, "admin1", &password_hash, &totp_secret, &api_key_hash).unwrap();

        let (device_id, _, _) = validate_device(&conn, &api_key_hash).unwrap().unwrap();
        let code = "test-code-abc-def";
        let expires_at = (chrono::Utc::now() + chrono::Duration::minutes(10))
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string();

        reserve_code(&conn, code, device_id, &expires_at).unwrap();

        let new_api_key = generate_api_key();
        let new_api_key_hash = hash_api_key(&new_api_key).unwrap();

        let out = register_device(&conn, code, "regpass", &new_api_key_hash).unwrap();
        assert!(out.is_some());
        assert_eq!(out.as_deref(), Some(totp_secret.as_str()));

        let validated = validate_device(&conn, &new_api_key_hash).unwrap();
        assert!(validated.is_some());
    }

    #[test]
    fn create_command_and_get_command() {
        let conn = in_memory_db_with_migrations();
        let api_key = generate_api_key();
        let api_key_hash = hash_api_key(&api_key).unwrap();
        let totp_secret = generate_totp_secret().unwrap();
        let password_hash = bcrypt::hash("p", bcrypt::DEFAULT_COST).unwrap();

        setup_admin(&conn, "a", &password_hash, &totp_secret, &api_key_hash).unwrap();
        let (device_id, _admin_id, _) = validate_device(&conn, &api_key_hash).unwrap().unwrap();

        let id = create_command(
            &conn,
            device_id,
            "hello world",
            Some("~/repos/foo"),
            Some("continue"),
            Some("claude-4"),
            Some("cursor"),
        )
        .unwrap();

        let cmd = get_command(&conn, id).unwrap().unwrap();
        assert_eq!(cmd.2, "hello world");
        assert_eq!(cmd.3, "pending");
        assert_eq!(cmd.6, Some("~/repos/foo".to_string()));
        assert_eq!(cmd.8, Some("claude-4".to_string()));

        update_command(&conn, id, Some("done"), Some("output"), Some("summary")).unwrap();
        let cmd2 = get_command(&conn, id).unwrap().unwrap();
        assert_eq!(cmd2.3, "done");
        assert_eq!(cmd2.4, Some("output".to_string()));
    }
}
