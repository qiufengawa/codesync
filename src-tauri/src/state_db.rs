use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use crate::error::AppResult;

pub fn open(codex_dir: &Path) -> AppResult<Connection> {
    let conn = Connection::open_with_flags(
        codex_dir.join("state_5.sqlite"),
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

pub fn open_ro(codex_dir: &Path) -> AppResult<Connection> {
    let conn = Connection::open_with_flags(
        codex_dir.join("state_5.sqlite"),
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    Ok(conn)
}

pub fn count_threads(codex_dir: &Path) -> AppResult<u32> {
    let conn = open_ro(codex_dir)?;
    let n: u32 = conn.query_row("SELECT COUNT(*) FROM threads", [], |r| r.get(0))?;
    Ok(n)
}
