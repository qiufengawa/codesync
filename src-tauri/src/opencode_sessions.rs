use std::fs;
use std::path::{Path, PathBuf};

use chrono::{TimeZone, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{AppError, AppResult};
use crate::models::{DeleteResult, PreviewEvent, SessionMetaBrief, SessionSummary};
use crate::paths;

const PROVIDER: &str = "opencode";
const TITLE_MAX_CHARS: usize = 80;

#[derive(Serialize, Deserialize, Clone)]
pub struct OpenCodeSessionExport {
    pub session_id: String,
    pub session_row: Value,
    pub messages: Vec<Value>,
    pub parts: Vec<Value>,
    pub session_messages: Vec<Value>,
    pub events: Vec<Value>,
    pub event_sequences: Vec<Value>,
}

struct OpenCodeSessionRow {
    id: String,
    directory: String,
    title: String,
    version: String,
    agent: Option<String>,
    model: Option<String>,
    tokens_input: i64,
    tokens_output: i64,
    tokens_reasoning: i64,
    tokens_cache_read: i64,
    tokens_cache_write: i64,
    time_created: i64,
    time_updated: i64,
    time_archived: Option<i64>,
}

#[derive(Clone)]
struct OpenCodeEventRow {
    message_id: String,
    message_time: i64,
    message_data: Value,
    part_id: Option<String>,
    part_time: Option<i64>,
    part_data: Option<Value>,
}

pub fn count_sessions(opencode_dir: &Path) -> AppResult<u32> {
    let db = paths::opencode_db_path(opencode_dir);
    if !db.is_file() {
        return Ok(0);
    }
    let conn = open_ro_db(&db)?;
    if !table_exists(&conn, "session")? {
        return Ok(0);
    }
    let count = conn.query_row("SELECT COUNT(*) FROM session", [], |row| row.get::<_, i64>(0))?;
    Ok(count.max(0) as u32)
}

pub fn has_session_table(opencode_dir: &Path) -> AppResult<bool> {
    let db = paths::opencode_db_path(opencode_dir);
    if !db.is_file() {
        return Ok(false);
    }
    let conn = open_ro_db(&db)?;
    table_exists(&conn, "session")
}

pub fn scan_sessions(opencode_dir: &Path) -> AppResult<Vec<SessionSummary>> {
    let db = paths::opencode_db_path(opencode_dir);
    if !db.is_file() {
        return Ok(Vec::new());
    }
    let conn = open_ro_db(&db)?;
    if !table_exists(&conn, "session")? {
        return Ok(Vec::new());
    }

    let rows = query_session_rows(&conn)?;
    let db_ref = db.to_string_lossy().into_owned();
    let db_bytes = fs::metadata(&db).map(|meta| meta.len()).unwrap_or(0);
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let model = parse_model(row.model.as_deref());
        let first_user_message = first_user_message(&conn, &row.id)?;
        let fallback_tokens = if token_sum(&row) <= 0 {
            message_token_sum(&conn, &row.id)?
        } else {
            0
        };
        let tokens_used = token_sum(&row).max(fallback_tokens);
        let (message_count, part_count, data_bytes) = session_counts(&conn, &row.id)?;
        let cwd = paths::strip_verbatim(&row.directory);
        let title = first_non_empty(&[Some(row.title.as_str()), Some(first_user_message.as_str())])
            .map(|value| truncate_summary(&value, TITLE_MAX_CHARS))
            .unwrap_or_else(|| row.id.clone());
        out.push(SessionSummary {
            provider: PROVIDER.to_string(),
            id: row.id.clone(),
            rollout_path: encode_ref(&db_ref, &row.id),
            cwd: cwd.clone(),
            cwd_display: paths::basename_display(&cwd),
            title,
            first_user_message,
            model: model.model,
            reasoning_effort: model.variant,
            source: Some(PROVIDER.to_string()),
            agent_nickname: None,
            agent_role: row.agent,
            tokens_used,
            created_at: millis_or_seconds_to_seconds(row.time_created),
            updated_at: millis_or_seconds_to_seconds(row.time_updated),
            archived: row.time_archived.is_some(),
            git_branch: None,
            rollout_bytes: data_bytes.unwrap_or(db_bytes),
            logs_count: message_count + part_count,
            has_backup: false,
            resume_command: format!("opencode --session {}", row.id),
        });
    }
    out.sort_by_key(|session| std::cmp::Reverse(session.updated_at.max(session.created_at)));
    Ok(out)
}

pub fn preview_range(path: &str, offset: usize, limit: usize) -> AppResult<Vec<PreviewEvent>> {
    let (db, session_id) = decode_ref(path)?;
    let conn = open_ro_db(&db)?;
    let rows = query_event_rows(&conn, &session_id)?;
    let mut out = Vec::with_capacity(limit.min(rows.len()));
    for (event_index, row) in rows.into_iter().enumerate() {
        if event_index < offset {
            continue;
        }
        out.push(classify_event(event_index, row));
        if out.len() >= limit {
            break;
        }
    }
    Ok(out)
}

pub fn preview_meta(path: &str) -> AppResult<SessionMetaBrief> {
    let (db, session_id) = decode_ref(path)?;
    let conn = open_ro_db(&db)?;
    let row = query_session_row(&conn, &session_id)?.ok_or_else(|| {
        AppError::NotFound(format!("OpenCode session not found: {session_id}"))
    })?;
    let model = parse_model(row.model.as_deref());
    Ok(SessionMetaBrief {
        id: Some(row.id),
        timestamp: Some(timestamp_string(row.time_created)),
        cwd: Some(row.directory),
        originator: row.agent,
        cli_version: Some(row.version),
        source: Some(PROVIDER.to_string()),
        model_provider: model.provider,
    })
}

pub fn delete_one(opencode_dir: &Path, id: &str) -> AppResult<DeleteResult> {
    let db = paths::opencode_db_path(opencode_dir);
    if !db.is_file() {
        return Err(AppError::NotFound(format!(
            "OpenCode database not found: {}",
            db.to_string_lossy()
        )));
    }
    let mut conn = open_db(&db)?;
    let mut result = DeleteResult {
        id: id.to_string(),
        threads_rows_deleted: 0,
        logs_rows_deleted: 0,
        history_rows_deleted: 0,
        rollout_deleted: false,
        rollout_missing: false,
        ok: false,
        error: None,
    };
    let tx = conn.transaction()?;
    let mut deleted_aux = 0usize;
    deleted_aux += delete_if_table_exists(&tx, "part", "session_id", id)?;
    deleted_aux += delete_if_table_exists(&tx, "message", "session_id", id)?;
    deleted_aux += delete_if_table_exists(&tx, "session_message", "session_id", id)?;
    if table_exists(&tx, "event")? {
        deleted_aux += tx.execute("DELETE FROM event WHERE aggregate_id = ?", [id])?;
    }
    if table_exists(&tx, "event_sequence")? {
        deleted_aux += tx.execute("DELETE FROM event_sequence WHERE aggregate_id = ?", [id])?;
    }
    if table_exists(&tx, "session")? {
        result.threads_rows_deleted = tx.execute("DELETE FROM session WHERE id = ?", [id])? as u32;
    }
    tx.commit()?;
    result.logs_rows_deleted = deleted_aux as u32;
    result.ok = result.threads_rows_deleted > 0 || result.logs_rows_deleted > 0;
    if !result.ok {
        result.error = Some("OpenCode 数据库中未找到该 session".to_string());
    }
    Ok(result)
}

pub fn export_session(opencode_dir: &Path, id: &str) -> AppResult<OpenCodeSessionExport> {
    let db = paths::opencode_db_path(opencode_dir);
    if !db.is_file() {
        return Err(AppError::NotFound(format!(
            "OpenCode database not found: {}",
            db.to_string_lossy()
        )));
    }
    let conn = open_ro_db(&db)?;

    let session_row = query_single_row_by_id(&conn, "session", "id", id)?;
    let session_row = session_row
        .ok_or_else(|| AppError::NotFound(format!("OpenCode session not found: {id}")))?;

    let messages = query_rows_by_id(&conn, "message", "session_id", id)?;
    let parts = query_rows_by_id(&conn, "part", "session_id", id)?;
    let session_messages = query_rows_by_id(&conn, "session_message", "session_id", id)?;
    let events = if table_exists(&conn, "event")? {
        query_rows_by_id(&conn, "event", "aggregate_id", id)?
    } else {
        Vec::new()
    };
    let event_sequences = if table_exists(&conn, "event_sequence")? {
        query_rows_by_id(&conn, "event_sequence", "aggregate_id", id)?
    } else {
        Vec::new()
    };

    Ok(OpenCodeSessionExport {
        session_id: id.to_string(),
        session_row,
        messages,
        parts,
        session_messages,
        events,
        event_sequences,
    })
}

pub fn import_session(opencode_dir: &Path, export: &OpenCodeSessionExport, overwrite: bool) -> AppResult<bool> {
    let db = paths::opencode_db_path(opencode_dir);
    if !db.is_file() {
        return Err(AppError::NotFound(format!(
            "OpenCode database not found: {}",
            db.to_string_lossy()
        )));
    }
    let mut conn = open_db(&db)?;
    let id = &export.session_id;

    if overwrite {
        delete_one(opencode_dir, id).ok();
    } else {
        let existing = query_single_row_by_id(&conn, "session", "id", id)?;
        if existing.is_some() {
            return Ok(false);
        }
    }

    let tx = conn.transaction()?;

    if table_exists(&tx, "session")? {
        insert_row(&tx, "session", &export.session_row)?;
    }
    for msg in &export.messages {
        insert_row(&tx, "message", msg)?;
    }
    for part in &export.parts {
        insert_row(&tx, "part", part)?;
    }
    for sm in &export.session_messages {
        insert_row(&tx, "session_message", sm)?;
    }
    if table_exists(&tx, "event")? {
        for evt in &export.events {
            insert_row(&tx, "event", evt)?;
        }
    }
    if table_exists(&tx, "event_sequence")? {
        for es in &export.event_sequences {
            insert_row(&tx, "event_sequence", es)?;
        }
    }

    tx.commit()?;
    Ok(true)
}

fn query_rows_by_id(conn: &Connection, table: &str, column: &str, id: &str) -> AppResult<Vec<Value>> {
    if !table_exists(conn, table)? {
        return Ok(Vec::new());
    }
    let sql = format!("SELECT * FROM {table} WHERE {column} = ?");
    let mut stmt = conn.prepare(&sql)?;
    let cols: Vec<String> = stmt
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let rows = stmt.query_map([id], |row| {
        let mut obj = serde_json::Map::new();
        for (i, col) in cols.iter().enumerate() {
            let val: Value = match row.get_ref(i) {
                Ok(rusqlite::types::ValueRef::Null) => Value::Null,
                Ok(rusqlite::types::ValueRef::Integer(v)) => Value::from(v),
                Ok(rusqlite::types::ValueRef::Real(v)) => Value::from(v),
                Ok(rusqlite::types::ValueRef::Text(bytes)) => {
                    Value::from(std::str::from_utf8(bytes).unwrap_or(""))
                }
                Ok(rusqlite::types::ValueRef::Blob(bytes)) => {
                    Value::from(String::from_utf8_lossy(bytes).to_string())
                }
                Err(_) => Value::Null,
            };
            obj.insert(col.clone(), val);
        }
        Ok(Value::Object(obj))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn query_single_row_by_id(conn: &Connection, table: &str, column: &str, id: &str) -> AppResult<Option<Value>> {
    let rows = query_rows_by_id(conn, table, column, id)?;
    Ok(rows.into_iter().next())
}

fn insert_row(tx: &Connection, table: &str, row: &Value) -> AppResult<()> {
    let obj = row.as_object().ok_or_else(|| {
        AppError::Other(format!("export row for {table} is not an object"))
    })?;
    let columns: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
    let placeholders: Vec<String> = (0..columns.len()).map(|i| format!("?{}", i + 1)).collect();
    let sql = format!(
        "INSERT OR REPLACE INTO {table} ({}) VALUES ({})",
        columns.join(", "),
        placeholders.join(", ")
    );

    let owned_values: Vec<(String, String)> = columns
        .iter()
        .map(|col| {
            let val = obj.get(*col);
            let key = col.to_string();
            let vstr = match val {
                Some(Value::Null) | None => "\0NULL\0".to_string(),
                Some(Value::Number(n)) => {
                    if let Some(i) = n.as_i64() {
                        format!("\0INT:{i}\0")
                    } else if let Some(f) = n.as_f64() {
                        format!("\0REAL:{f}\0")
                    } else {
                        "\0NULL\0".to_string()
                    }
                }
                Some(Value::String(s)) => s.clone(),
                Some(Value::Bool(b)) => if *b { "1".to_string() } else { "0".to_string() },
                _ => "\0NULL\0".to_string(),
            };
            (key, vstr)
        })
        .collect();

    let params: Vec<Box<dyn rusqlite::ToSql>> = owned_values
        .iter()
        .map(|(_, v)| {
            if v == "\0NULL\0" {
                Box::new(rusqlite::types::Null) as Box<dyn rusqlite::ToSql>
            } else if let Some(rest) = v.strip_prefix("\0INT:") {
                let i: i64 = rest.trim_end_matches('\0').parse().unwrap_or(0);
                Box::new(i) as Box<dyn rusqlite::ToSql>
            } else if let Some(rest) = v.strip_prefix("\0REAL:") {
                let f: f64 = rest.trim_end_matches('\0').parse().unwrap_or(0.0);
                Box::new(f) as Box<dyn rusqlite::ToSql>
            } else {
                Box::new(v.as_str()) as Box<dyn rusqlite::ToSql>
            }
        })
        .collect();

    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    tx.execute(&sql, param_refs.as_slice())?;
    Ok(())
}

fn query_session_rows(conn: &Connection) -> AppResult<Vec<OpenCodeSessionRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, directory, title, version, agent, model,
                tokens_input, tokens_output, tokens_reasoning, tokens_cache_read, tokens_cache_write,
                time_created, time_updated, time_archived
         FROM session
         ORDER BY time_updated DESC, id DESC",
    )?;
    let rows = stmt.query_map([], session_row_from_sql)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn query_session_row(conn: &Connection, session_id: &str) -> AppResult<Option<OpenCodeSessionRow>> {
    conn.query_row(
        "SELECT id, directory, title, version, agent, model,
                tokens_input, tokens_output, tokens_reasoning, tokens_cache_read, tokens_cache_write,
                time_created, time_updated, time_archived
         FROM session
         WHERE id = ?",
        [session_id],
        session_row_from_sql,
    )
    .optional()
    .map_err(AppError::Sqlite)
}

fn session_row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<OpenCodeSessionRow> {
    Ok(OpenCodeSessionRow {
        id: row.get(0)?,
        directory: row.get(1)?,
        title: row.get(2)?,
        version: row.get(3)?,
        agent: row.get(4)?,
        model: row.get(5)?,
        tokens_input: row.get(6)?,
        tokens_output: row.get(7)?,
        tokens_reasoning: row.get(8)?,
        tokens_cache_read: row.get(9)?,
        tokens_cache_write: row.get(10)?,
        time_created: row.get(11)?,
        time_updated: row.get(12)?,
        time_archived: row.get(13)?,
    })
}

fn query_event_rows(conn: &Connection, session_id: &str) -> AppResult<Vec<OpenCodeEventRow>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.time_created, m.data, p.id, p.time_created, p.data
         FROM message m
         LEFT JOIN part p ON p.message_id = m.id
         WHERE m.session_id = ?
         ORDER BY m.time_created, p.time_created, p.id",
    )?;
    let rows = stmt.query_map([session_id], |row| {
        let message_raw: String = row.get(2)?;
        let part_raw: Option<String> = row.get(5)?;
        Ok(OpenCodeEventRow {
            message_id: row.get(0)?,
            message_time: row.get(1)?,
            message_data: parse_json_or_raw(&message_raw),
            part_id: row.get(3)?,
            part_time: row.get(4)?,
            part_data: part_raw.as_deref().map(parse_json_or_raw),
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn first_user_message(conn: &Connection, session_id: &str) -> AppResult<String> {
    let rows = query_event_rows(conn, session_id)?;
    for row in rows {
        if role_from_message(&row.message_data) != "user" {
            continue;
        }
        if let Some(part) = row.part_data.as_ref() {
            if part_type(part) == "text" {
                let text = part_text(part);
                if !text.trim().is_empty() {
                    return Ok(truncate_summary(&text, 240));
                }
            }
        }
    }
    Ok(String::new())
}

fn message_token_sum(conn: &Connection, session_id: &str) -> AppResult<i64> {
    if !table_exists(conn, "message")? {
        return Ok(0);
    }
    let mut stmt = conn.prepare("SELECT data FROM message WHERE session_id = ?")?;
    let rows = stmt.query_map([session_id], |row| row.get::<_, String>(0))?;
    let mut total = 0i64;
    for row in rows {
        total += token_total_from_message(&parse_json_or_raw(&row?));
    }
    Ok(total)
}

fn session_counts(conn: &Connection, session_id: &str) -> AppResult<(i64, i64, Option<u64>)> {
    let message_count = count_table_rows(conn, "message", "session_id", session_id)?;
    let part_count = count_table_rows(conn, "part", "session_id", session_id)?;
    let message_bytes = sum_data_len(conn, "message", "session_id", session_id)?;
    let part_bytes = sum_data_len(conn, "part", "session_id", session_id)?;
    Ok((message_count, part_count, Some((message_bytes + part_bytes).max(0) as u64)))
}

fn classify_event(index: usize, row: OpenCodeEventRow) -> PreviewEvent {
    let message_role = role_from_message(&row.message_data);
    let part = row.part_data.clone().unwrap_or_else(|| json!({"type":"message"}));
    let kind = part_type(&part).to_string();
    let timestamp = timestamp_string(row.part_time.unwrap_or(row.message_time));
    let (role, text_summary) = match kind.as_str() {
        "text" => (message_role.clone(), truncate_summary(&part_text(&part), 120)),
        "reasoning" => ("reasoning".to_string(), truncate_summary(&part_text(&part), 120)),
        "tool" => (tool_role(&part), truncate_summary(&tool_summary(&part), 120)),
        "step-start" => ("meta".to_string(), "步骤开始".to_string()),
        "step-finish" => ("meta".to_string(), step_finish_summary(&part)),
        "compaction" => ("meta".to_string(), "上下文压缩".to_string()),
        _ => ("other".to_string(), truncate_summary(&part_text(&part), 120)),
    };
    let content = if matches!(kind.as_str(), "text" | "reasoning") {
        part_text(&part)
    } else {
        text_summary.clone()
    };
    let raw = json!({
        "type": "opencode_part",
        "message": {
            "id": row.message_id,
            "role": message_role,
            "content": content,
            "time_created": row.message_time,
        },
        "payload": part,
        "opencode": {
            "part_id": row.part_id,
            "part_time_created": row.part_time,
            "message": row.message_data,
        }
    });
    PreviewEvent {
        index,
        timestamp,
        role,
        kind,
        text_summary,
        raw,
    }
}

fn token_sum(row: &OpenCodeSessionRow) -> i64 {
    row.tokens_input
        + row.tokens_output
        + row.tokens_reasoning
        + row.tokens_cache_read
        + row.tokens_cache_write
}

fn token_total_from_message(message: &Value) -> i64 {
    let Some(tokens) = message.get("tokens") else {
        return 0;
    };
    if let Some(total) = tokens.get("total").and_then(Value::as_i64) {
        return total.max(0);
    }
    let mut sum = 0i64;
    for key in ["input", "output", "reasoning"] {
        sum += tokens.get(key).and_then(Value::as_i64).unwrap_or(0).max(0);
    }
    if let Some(cache) = tokens.get("cache") {
        sum += cache.get("read").and_then(Value::as_i64).unwrap_or(0).max(0);
        sum += cache.get("write").and_then(Value::as_i64).unwrap_or(0).max(0);
    }
    sum
}

struct ParsedModel {
    provider: Option<String>,
    model: Option<String>,
    variant: Option<String>,
}

fn parse_model(raw: Option<&str>) -> ParsedModel {
    let Some(raw) = raw else {
        return ParsedModel { provider: None, model: None, variant: None };
    };
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return ParsedModel { provider: None, model: Some(raw.to_string()), variant: None };
    };
    ParsedModel {
        provider: string_field(&value, "providerID"),
        model: string_field(&value, "id").or_else(|| string_field(&value, "modelID")),
        variant: string_field(&value, "variant"),
    }
}

fn role_from_message(message: &Value) -> String {
    message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("other")
        .to_string()
}

fn part_type(part: &Value) -> &str {
    part.get("type").and_then(Value::as_str).unwrap_or("part")
}

fn part_text(part: &Value) -> String {
    part.get("text")
        .and_then(Value::as_str)
        .or_else(|| part.get("content").and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn tool_role(part: &Value) -> String {
    let status = part
        .get("state")
        .and_then(|state| state.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if matches!(status, "completed" | "error") {
        "tool_result".to_string()
    } else {
        "tool_call".to_string()
    }
}

fn tool_summary(part: &Value) -> String {
    let tool = part.get("tool").and_then(Value::as_str).unwrap_or("tool");
    let status = part
        .get("state")
        .and_then(|state| state.get("status"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let description = part
        .get("state")
        .and_then(|state| state.get("metadata"))
        .and_then(|metadata| metadata.get("description"))
        .and_then(Value::as_str)
        .or_else(|| {
            part.get("state")
                .and_then(|state| state.get("input"))
                .and_then(|input| input.get("description"))
                .and_then(Value::as_str)
        })
        .unwrap_or("");
    match (status.is_empty(), description.is_empty()) {
        (true, true) => tool.to_string(),
        (true, false) => format!("{tool}: {description}"),
        (false, true) => format!("{tool}: {status}"),
        (false, false) => format!("{tool}: {status} · {description}"),
    }
}

fn step_finish_summary(part: &Value) -> String {
    let tokens = part
        .get("tokens")
        .and_then(|tokens| tokens.get("total"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if tokens > 0 {
        format!("步骤完成 · tokens: {tokens}")
    } else {
        "步骤完成".to_string()
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(String::from)
}

fn first_non_empty(values: &[Option<&str>]) -> Option<String> {
    values
        .iter()
        .flatten()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .map(String::from)
}

fn truncate_summary(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut out = trimmed.chars().take(max_chars).collect::<String>();
    out.push_str("...");
    out
}

fn parse_json_or_raw(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

fn open_ro_db(path: &Path) -> AppResult<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(AppError::Sqlite)
}

fn open_db(path: &Path) -> AppResult<Connection> {
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(AppError::Sqlite)
}

fn table_exists(conn: &Connection, table: &str) -> AppResult<bool> {
    let exists = conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=? LIMIT 1",
        [table],
        |_| Ok(true),
    );
    match exists {
        Ok(true) => Ok(true),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
        Err(err) => Err(AppError::Sqlite(err)),
        Ok(false) => Ok(false),
    }
}

fn count_table_rows(conn: &Connection, table: &str, column: &str, value: &str) -> AppResult<i64> {
    if !table_exists(conn, table)? {
        return Ok(0);
    }
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE {column} = ?");
    conn.query_row(&sql, [value], |row| row.get::<_, i64>(0))
        .map_err(AppError::Sqlite)
}

fn sum_data_len(conn: &Connection, table: &str, column: &str, value: &str) -> AppResult<i64> {
    if !table_exists(conn, table)? {
        return Ok(0);
    }
    let sql = format!("SELECT COALESCE(SUM(LENGTH(data)), 0) FROM {table} WHERE {column} = ?");
    conn.query_row(&sql, [value], |row| row.get::<_, i64>(0))
        .map_err(AppError::Sqlite)
}

fn delete_if_table_exists(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    column: &str,
    value: &str,
) -> AppResult<usize> {
    if !table_exists(tx, table)? {
        return Ok(0);
    }
    let sql = format!("DELETE FROM {table} WHERE {column} = ?");
    tx.execute(&sql, [value]).map_err(AppError::Sqlite)
}

fn encode_ref(db_path: &str, session_id: &str) -> String {
    format!("{db_path}#{session_id}")
}

fn decode_ref(value: &str) -> AppResult<(PathBuf, String)> {
    let (db, session_id) = value.rsplit_once('#').ok_or_else(|| {
        AppError::Path("OpenCode 预览路径缺少 #session_id".to_string())
    })?;
    if session_id.trim().is_empty() {
        return Err(AppError::Path("OpenCode session id 不能为空".to_string()));
    }
    Ok((PathBuf::from(db), session_id.to_string()))
}

fn millis_or_seconds_to_seconds(value: i64) -> i64 {
    if value > 1_000_000_000_000 {
        value / 1000
    } else {
        value
    }
}

fn timestamp_string(value: i64) -> String {
    let millis = if value > 1_000_000_000_000 {
        value
    } else {
        value.saturating_mul(1000)
    };
    Utc.timestamp_millis_opt(millis)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "{name}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ))
    }

    fn create_db(root: &Path) -> AppResult<PathBuf> {
        fs::create_dir_all(root)?;
        let db = root.join("opencode.db");
        let conn = Connection::open(&db)?;
        conn.execute_batch(
            "CREATE TABLE session (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                workspace_id TEXT,
                parent_id TEXT,
                slug TEXT NOT NULL,
                directory TEXT NOT NULL,
                path TEXT,
                title TEXT NOT NULL,
                version TEXT NOT NULL,
                share_url TEXT,
                summary_additions INTEGER,
                summary_deletions INTEGER,
                summary_files INTEGER,
                summary_diffs TEXT,
                metadata TEXT,
                cost REAL DEFAULT 0 NOT NULL,
                tokens_input INTEGER DEFAULT 0 NOT NULL,
                tokens_output INTEGER DEFAULT 0 NOT NULL,
                tokens_reasoning INTEGER DEFAULT 0 NOT NULL,
                tokens_cache_read INTEGER DEFAULT 0 NOT NULL,
                tokens_cache_write INTEGER DEFAULT 0 NOT NULL,
                revert TEXT,
                permission TEXT,
                agent TEXT,
                model TEXT,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL,
                time_compacting INTEGER,
                time_archived INTEGER
            );
            CREATE TABLE message (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL,
                data TEXT NOT NULL
            );
            CREATE TABLE part (
                id TEXT PRIMARY KEY,
                message_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL,
                data TEXT NOT NULL
            );
            CREATE TABLE session_message (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                type TEXT NOT NULL,
                seq INTEGER NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL,
                data TEXT NOT NULL
            );
            CREATE TABLE event_sequence (aggregate_id TEXT PRIMARY KEY, seq INTEGER NOT NULL, owner_id TEXT);
            CREATE TABLE event (id TEXT PRIMARY KEY, aggregate_id TEXT NOT NULL, seq INTEGER NOT NULL, type TEXT NOT NULL, data TEXT NOT NULL);",
        )?;
        conn.execute(
            "INSERT INTO session (
                id, project_id, slug, directory, path, title, version, tokens_input,
                tokens_output, tokens_reasoning, tokens_cache_read, tokens_cache_write,
                model, time_created, time_updated
            ) VALUES (?1, 'global', 'sample', '/tmp/project', 'tmp/project', 'OpenCode title', '1.0.0', 1, 2, 3, 4, 5, ?2, 1781513951000, 1781514051000)",
            [
                "ses_test",
                r#"{"id":"gpt-5.5","providerID":"jisuan","variant":"xhigh"}"#,
            ],
        )?;
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            [
                "msg_user",
                "ses_test",
                "1781513951001",
                "1781513951001",
                r#"{"role":"user","time":{"created":1781513951001}}"#,
            ],
        )?;
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            ["prt_user", "msg_user", "ses_test", "1781513951002", "1781513951002", r#"{"type":"text","text":"hello opencode"}"#],
        )?;
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data) VALUES (?1, ?2, ?3, ?4, ?5)",
            [
                "msg_assistant",
                "ses_test",
                "1781513952000",
                "1781513953000",
                r#"{"role":"assistant","tokens":{"total":99},"modelID":"gpt-5.5","providerID":"jisuan","time":{"created":1781513952000}}"#,
            ],
        )?;
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            ["prt_assistant", "msg_assistant", "ses_test", "1781513953000", "1781513953000", r#"{"type":"text","text":"assistant answer"}"#],
        )?;
        conn.execute("INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) VALUES ('sm1', 'ses_test', 'x', 1, 1, 1, '{}')", [])?;
        conn.execute("INSERT INTO event_sequence (aggregate_id, seq) VALUES ('ses_test', 1)", [])?;
        conn.execute("INSERT INTO event (id, aggregate_id, seq, type, data) VALUES ('evt1', 'ses_test', 1, 'session.updated.1', '{}')", [])?;
        Ok(db)
    }

    #[test]
    fn scans_and_previews_opencode_sessions() -> AppResult<()> {
        let root = temp_dir("codesync-opencode-scan-test");
        let db = create_db(&root)?;

        let sessions = scan_sessions(&root)?;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].provider, PROVIDER);
        assert_eq!(sessions[0].first_user_message, "hello opencode");
        assert_eq!(sessions[0].model.as_deref(), Some("gpt-5.5"));
        assert_eq!(sessions[0].reasoning_effort.as_deref(), Some("xhigh"));
        assert_eq!(sessions[0].tokens_used, 15);
        assert_eq!(sessions[0].updated_at, 1_781_514_051);

        let path = encode_ref(&db.to_string_lossy(), "ses_test");
        let events = preview_range(&path, 0, 10)?;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].role, "user");
        assert_eq!(events[1].role, "assistant");

        fs::remove_dir_all(root).ok();
        Ok(())
    }

    #[test]
    fn deletes_opencode_session_rows() -> AppResult<()> {
        let root = temp_dir("codesync-opencode-delete-test");
        create_db(&root)?;

        let result = delete_one(&root, "ses_test")?;
        assert!(result.ok);
        assert_eq!(result.threads_rows_deleted, 1);
        assert!(result.logs_rows_deleted >= 4);
        assert_eq!(count_sessions(&root)?, 0);

        fs::remove_dir_all(root).ok();
        Ok(())
    }
}
