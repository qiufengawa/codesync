use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use serde_json::Value;

use crate::error::{AppError, AppResult};

const SESSION_ID_KEYS: [&str; 3] = ["sessionId", "session_id", "id"];

pub fn line_session_id(line: &str) -> Option<String> {
    if line.trim().is_empty() {
        return None;
    }
    let value = serde_json::from_str::<Value>(line).ok()?;
    for key in SESSION_ID_KEYS {
        if let Some(id) = value
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            return Some(id.to_string());
        }
    }
    None
}

pub fn line_matches_session(line: &str, id: &str) -> bool {
    line_session_id(line).as_deref() == Some(id)
}

pub fn collect_lines_for_ids(
    history_path: &Path,
    ids: &HashSet<String>,
) -> AppResult<HashMap<String, Vec<String>>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    if ids.is_empty() || !history_path.is_file() {
        return Ok(out);
    }

    let file = File::open(history_path)?;
    for line in BufReader::new(file).lines() {
        let line = line?;
        if let Some(id) = line_session_id(&line) {
            if ids.contains(&id) {
                out.entry(id).or_default().push(line);
            }
        }
    }
    Ok(out)
}

pub fn write_lines(path: &Path, lines: &[String]) -> AppResult<u32> {
    if lines.is_empty() {
        return Ok(0);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    for line in lines {
        writeln!(file, "{}", line)?;
    }
    file.sync_all().ok();
    Ok(lines.len() as u32)
}

pub fn append_lines(history_path: &Path, id: &str, lines: &[String]) -> AppResult<u32> {
    if lines.is_empty() {
        return Ok(0);
    }
    if let Some(parent) = history_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut existing: HashSet<String> = HashSet::new();
    if history_path.is_file() {
        for line in BufReader::new(File::open(history_path)?).lines() {
            let line = line?;
            if !line.trim().is_empty() {
                existing.insert(line);
            }
        }
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_path)?;
    let mut added = 0u32;
    for line in lines {
        if !line_matches_session(line, id) {
            continue;
        }
        if existing.insert(line.clone()) {
            writeln!(file, "{}", line)?;
            added += 1;
        }
    }
    file.flush().ok();
    Ok(added)
}

pub fn append_from_file(history_path: &Path, source_path: &Path, id: &str) -> AppResult<u32> {
    if !source_path.is_file() {
        return Ok(0);
    }
    let lines = BufReader::new(File::open(source_path)?)
        .lines()
        .collect::<Result<Vec<_>, _>>()?;
    append_lines(history_path, id, &lines)
}

pub fn filter_file(path: &Path, id: &str) -> AppResult<u32> {
    let content = fs::read_to_string(path)?;
    let tmp = path.with_extension("jsonl.tmp");
    let mut removed = 0u32;
    {
        let mut file = File::create(&tmp)?;
        for line in content.lines() {
            if line_matches_session(line, id) {
                removed += 1;
                continue;
            }
            writeln!(file, "{}", line)?;
        }
        file.sync_all().ok();
    }
    fs::rename(&tmp, path).map_err(AppError::Io)?;
    Ok(removed)
}
