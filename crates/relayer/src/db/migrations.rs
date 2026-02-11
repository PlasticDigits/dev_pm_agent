//! Migration runner.

use anyhow::Result;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

/// Run all migrations from migrations/ directory.
pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _schema_migrations (name TEXT PRIMARY KEY)",
        [],
    )?;

    let migrations_dir: PathBuf = std::env::var("MIGRATIONS_DIR")
        .map(Into::into)
        .unwrap_or_else(|_| Path::new("migrations").to_path_buf());
    if !migrations_dir.exists() {
        return Ok(());
    }

    let mut entries: Vec<_> = fs::read_dir(migrations_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |e| e == "sql"))
        .collect();
    entries.sort();

    for path in entries {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let applied: bool = conn
            .query_row(
                "SELECT 1 FROM _schema_migrations WHERE name = ?1",
                [&name],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if applied {
            continue;
        }

        let sql = fs::read_to_string(&path)?;
        conn.execute_batch(&sql)?;
        conn.execute("INSERT INTO _schema_migrations (name) VALUES (?1)", [&name])?;
    }

    Ok(())
}
