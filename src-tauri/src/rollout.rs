use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use serde_json::Value;

use crate::error::AppResult;
use crate::models::{PreviewEvent, SessionMetaBrief};

fn classify(index: usize, raw: Value) -> PreviewEvent {
    let timestamp = raw
        .get("timestamp")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let outer_type = raw
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let payload_type = raw
        .get("payload")
        .and_then(|p| p.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let (role, kind, text_summary) = match (outer_type.as_str(), payload_type.as_str()) {
        ("session_meta", _) => ("meta".into(), "session_meta".into(), "会话元数据".into()),
        ("event_msg", "task_started") => ("meta".into(), "task_started".into(), "任务开始".into()),
        ("event_msg", "token_count") => {
            let total = raw
                .get("payload")
                .and_then(|p| p.get("total_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            (
                "meta".into(),
                "token_count".into(),
                format!("tokens: {}", total),
            )
        }
        ("event_msg", "agent_message") => {
            let text = raw
                .get("payload")
                .and_then(|p| p.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ("assistant".into(), "agent_message".into(), trim(&text, 120))
        }
        ("event_msg", "user_message") => {
            let text = raw
                .get("payload")
                .and_then(|p| p.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ("user".into(), "user_message".into(), trim(&text, 120))
        }
        ("response_item", "message") => {
            let role_name = raw
                .get("payload")
                .and_then(|p| p.get("role"))
                .and_then(|v| v.as_str())
                .unwrap_or("assistant")
                .to_string();
            let text = flatten_content(raw.get("payload").and_then(|p| p.get("content")));
            (role_name, "message".into(), trim(&text, 120))
        }
        ("response_item", "reasoning") => {
            let text = flatten_content(raw.get("payload").and_then(|p| p.get("content")));
            ("reasoning".into(), "reasoning".into(), trim(&text, 80))
        }
        ("response_item", "function_call") => {
            let name = raw
                .get("payload")
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            ("tool_call".into(), "function_call".into(), name)
        }
        ("response_item", "function_call_output") => (
            "tool_result".into(),
            "function_call_output".into(),
            "工具返回".into(),
        ),
        _ => (
            "other".into(),
            format!("{}/{}", outer_type, payload_type),
            String::new(),
        ),
    };

    PreviewEvent {
        index,
        timestamp,
        role,
        kind,
        text_summary,
        raw,
    }
}

fn flatten_content(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|x| {
                x.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| x.as_str().map(|s| s.to_string()))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn trim(s: &str, n: usize) -> String {
    let flat: String = s.chars().filter(|c| *c != '\n').collect();
    if flat.chars().count() <= n {
        flat
    } else {
        let mut out: String = flat.chars().take(n).collect();
        out.push('…');
        out
    }
}

#[tauri::command]
pub fn preview_session_head(
    provider: Option<String>,
    rollout_path: String,
    limit: usize,
) -> AppResult<Vec<PreviewEvent>> {
    preview_range_by_provider(provider, &rollout_path, 0, limit)
}

#[tauri::command]
pub fn preview_session_range(
    provider: Option<String>,
    rollout_path: String,
    offset: usize,
    limit: usize,
) -> AppResult<Vec<PreviewEvent>> {
    preview_range_by_provider(provider, &rollout_path, offset, limit)
}

fn preview_range_by_provider(
    provider: Option<String>,
    path: &str,
    offset: usize,
    limit: usize,
) -> AppResult<Vec<PreviewEvent>> {
    match provider.as_deref().unwrap_or("codex") {
        "codex" => preview_range_impl(path, offset, limit),
        "claude" => crate::claude_sessions::preview_range(path, offset, limit),
        other => Err(crate::error::AppError::Other(format!(
            "不支持的 provider: {other}"
        ))),
    }
}

fn preview_range_impl(path: &str, offset: usize, limit: usize) -> AppResult<Vec<PreviewEvent>> {
    let f = File::open(PathBuf::from(path))?;
    let reader = BufReader::new(f);
    let mut out = Vec::with_capacity(limit);
    let mut event_index = 0usize;
    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        // 预览允许跳过损坏行，完整修复功能会负责诊断这类文件。
        if let Ok(raw) = serde_json::from_str::<Value>(&line) {
            if event_index < offset {
                event_index += 1;
                continue;
            }
            out.push(classify(i, raw));
            event_index += 1;
            if out.len() >= limit {
                break;
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn preview_session_meta(
    provider: Option<String>,
    rollout_path: String,
) -> AppResult<SessionMetaBrief> {
    if provider.as_deref().unwrap_or("codex") == "claude" {
        return crate::claude_sessions::preview_meta(&rollout_path);
    }
    let f = File::open(PathBuf::from(&rollout_path))?;
    let mut reader = BufReader::new(f);
    let mut first = String::new();
    reader.read_line(&mut first)?;
    let raw: Value = serde_json::from_str(first.trim())?;
    let payload = raw.get("payload");
    let brief = SessionMetaBrief {
        id: payload
            .and_then(|p| p.get("id"))
            .and_then(|v| v.as_str())
            .map(String::from),
        timestamp: payload
            .and_then(|p| p.get("timestamp"))
            .and_then(|v| v.as_str())
            .map(String::from),
        cwd: payload
            .and_then(|p| p.get("cwd"))
            .and_then(|v| v.as_str())
            .map(String::from),
        originator: payload
            .and_then(|p| p.get("originator"))
            .and_then(|v| v.as_str())
            .map(String::from),
        cli_version: payload
            .and_then(|p| p.get("cli_version"))
            .and_then(|v| v.as_str())
            .map(String::from),
        source: payload
            .and_then(|p| p.get("source"))
            .and_then(|v| v.as_str())
            .map(String::from),
        model_provider: payload
            .and_then(|p| p.get("model_provider"))
            .and_then(|v| v.as_str())
            .map(String::from),
    };
    Ok(brief)
}
