use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OpenFlags};

use crate::error::{AppError, AppResult};
use crate::paths;

pub struct ReadOnlyConnection {
    conn: Connection,
    _snapshot: Option<SqliteSnapshot>,
}

impl Deref for ReadOnlyConnection {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

struct SqliteSnapshot {
    dir: PathBuf,
}

impl Drop for SqliteSnapshot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

pub fn open(codex_dir: &Path) -> AppResult<Connection> {
    let conn = Connection::open_with_flags(
        codex_dir.join("state_5.sqlite"),
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

pub fn open_ro(codex_dir: &Path) -> AppResult<ReadOnlyConnection> {
    open_ro_db(codex_dir, "state_5.sqlite")
}

pub(crate) fn open_ro_db(codex_dir: &Path, db_name: &str) -> AppResult<ReadOnlyConnection> {
    let (db_path, snapshot) = if paths::is_wsl_unc_path(codex_dir) {
        snapshot_sqlite_db(codex_dir, db_name)?
    } else {
        (codex_dir.join(db_name), None)
    };
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    Ok(ReadOnlyConnection {
        conn,
        _snapshot: snapshot,
    })
}

pub fn count_threads(codex_dir: &Path) -> AppResult<u32> {
    let conn = open_ro(codex_dir)?;
    let n: u32 = conn.query_row("SELECT COUNT(*) FROM threads", [], |r| r.get(0))?;
    Ok(n)
}

fn snapshot_sqlite_db(
    source_dir: &Path,
    db_name: &str,
) -> AppResult<(PathBuf, Option<SqliteSnapshot>)> {
    let source_db = source_dir.join(db_name);
    let snapshot_dir = std::env::temp_dir().join(format!(
        "cc-session-manager-sqlite-{}-{}-{}",
        sanitize_snapshot_name(db_name),
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&snapshot_dir)?;

    let snapshot_db = snapshot_dir.join(db_name);
    copy_sqlite_file(&source_db, &snapshot_db, true)?;
    copy_sqlite_file(
        &sqlite_sidecar_path(&source_db, "-wal"),
        &sqlite_sidecar_path(&snapshot_db, "-wal"),
        false,
    )?;
    copy_sqlite_file(
        &sqlite_sidecar_path(&source_db, "-shm"),
        &sqlite_sidecar_path(&snapshot_db, "-shm"),
        false,
    )?;

    Ok((snapshot_db, Some(SqliteSnapshot { dir: snapshot_dir })))
}

fn copy_sqlite_file(source: &Path, dest: &Path, required: bool) -> AppResult<()> {
    let attempts = if required { 6 } else { 1 };
    let mut last_err: Option<std::io::Error> = None;
    for attempt in 0..attempts {
        match fs::copy(source, dest) {
            Ok(_) => return Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound && !required => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                if attempt + 1 < attempts {
                    if let Some(parent) = source.parent() {
                        let _ = fs::read_dir(parent).map(|entries| entries.count());
                    }
                    thread::sleep(Duration::from_millis(250));
                }
            }
        }
    }
    let Some(err) = last_err else {
        return Ok(());
    };
    if err.kind() == std::io::ErrorKind::NotFound {
        return Err(AppError::NotFound(format!(
            "SQLite 文件不存在，无法创建只读快照: {}",
            source.to_string_lossy()
        )));
    }
    Err(AppError::Other(format!(
        "复制 SQLite 只读快照失败: {} -> {} ({})",
        source.to_string_lossy(),
        dest.to_string_lossy(),
        err
    )))
}

fn sqlite_sidecar_path(db_path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}{}", db_path.to_string_lossy(), suffix))
}

fn sanitize_snapshot_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}
