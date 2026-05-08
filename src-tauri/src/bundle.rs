//! 会话 bundle 导出/导入（面向跨机器迁移）
//!
//! 与 backup.rs 的区别：
//! - backup.rs 是"整机快照"（threads 行 + logs + rollout 集中在一个备份目录）
//! - bundle.rs 是"单会话包"（每个会话一个子目录 + manifest，便于挑选 / 跨机器）
//!
//! 目录结构：
//! ```text
//! <out_dir>/
//!   <machine>/
//!     <export_group>/
//!       <batch_timestamp>/
//!         <session_id>/
//!           codex/<rollout_relpath>         # 原样复制 rollout
//!           history.jsonl                     # 该会话的 history 行（可空）
//!           manifest.json                     # 元数据 + sha256
//! ```
//!
//! zip 打包：直接压整个 bundle 根目录（跨机器：解压后 import_bundles 即可）。

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::family;
use crate::models::{
    BundleListItem, BundleManifest, ExportReport, ImportMode, ImportReport, ZipReport,
};
use crate::paths;
use crate::state_db;

const BUNDLE_VERSION: u32 = 1;
const PROVIDER_CODEX: &str = "codex";
const PROVIDER_CLAUDE: &str = "claude";
const DEFAULT_SANDBOX_POLICY: &str = "read-only";
const DEFAULT_APPROVAL_MODE: &str = "on-request";
const DEFAULT_MEMORY_MODE: &str = "enabled";

struct RolloutSource {
    abs: PathBuf,
    rel: PathBuf,
    meta: Value,
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let mut f = File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut f, &mut hasher)?;
    Ok(hex::encode(hasher.finalize()))
}

fn count_jsonl_lines(path: &Path) -> AppResult<u64> {
    if !path.is_file() {
        return Ok(0);
    }
    let f = File::open(path)?;
    let mut n = 0u64;
    for line in BufReader::new(f).lines() {
        if !line?.trim().is_empty() {
            n += 1;
        }
    }
    Ok(n)
}

fn batch_slug() -> String {
    format!("batch-{}", chrono::Utc::now().format("%Y%m%dT%H%M%S"))
}

fn index_rollouts(codex: &Path) -> AppResult<HashMap<String, RolloutSource>> {
    let mut out = HashMap::new();
    for root in [
        paths::sessions_dir(codex),
        paths::archived_sessions_dir(codex),
    ] {
        if !root.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy();
            if !name.starts_with("rollout-") || !name.ends_with(".jsonl") {
                continue;
            }
            let abs = entry.path().to_path_buf();
            if let Ok((id, source)) = rollout_source_from_path(codex, abs) {
                out.entry(id).or_insert(source);
            }
        }
    }
    Ok(out)
}

fn rollout_source_from_path(codex: &Path, abs: PathBuf) -> AppResult<(String, RolloutSource)> {
    let meta = family::read_session_meta(&abs).map_err(|e| {
        AppError::Other(format!(
            "rollout 首行不是有效 session_meta: {}: {}",
            abs.to_string_lossy(),
            e
        ))
    })?;
    let id = meta
        .get("payload")
        .and_then(|x| x.get("id"))
        .and_then(|x| x.as_str())
        .ok_or_else(|| {
            AppError::Other(format!(
                "rollout 缺少 session_meta.id: {}",
                abs.to_string_lossy()
            ))
        })?
        .to_string();
    let rel = abs
        .strip_prefix(codex)
        .map(|x| x.to_path_buf())
        .map_err(|_| {
            AppError::Path(format!(
                "rollout 不在 Codex 目录下: {}",
                abs.to_string_lossy()
            ))
        })?;
    Ok((id, RolloutSource { abs, rel, meta }))
}

fn rollout_source_from_state(
    codex: &Path,
    state: &rusqlite::Connection,
    id: &str,
) -> AppResult<Option<RolloutSource>> {
    let rollout_path: Option<String> = state
        .query_row("SELECT rollout_path FROM threads WHERE id = ?", [id], |r| {
            r.get(0)
        })
        .optional()?;
    let Some(rollout_path) = rollout_path else {
        return Ok(None);
    };
    let abs = PathBuf::from(paths::strip_verbatim(&rollout_path));
    if !abs.is_file() {
        return Err(AppError::NotFound(format!(
            "threads.rollout_path 指向的文件不存在: {}",
            abs.to_string_lossy()
        )));
    }
    let (actual_id, source) = rollout_source_from_path(codex, abs)?;
    if actual_id != id {
        return Err(AppError::Other(format!(
            "threads.rollout_path 指向的 rollout id 不匹配: 期望 {}, 实际 {}",
            id, actual_id
        )));
    }
    Ok(Some(source))
}

// ========================= 导出 =========================

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn export_session_bundles(
    provider: Option<String>,
    codex_dir: String,
    claude_dir: Option<String>,
    out_dir: String,
    ids: Vec<String>,
    machine_label: Option<String>,
    export_group: Option<String>,
) -> AppResult<Vec<ExportReport>> {
    if provider.as_deref().unwrap_or(PROVIDER_CODEX) == PROVIDER_CLAUDE {
        let claude = PathBuf::from(
            claude_dir
                .unwrap_or_else(|| paths::default_claude_dir().to_string_lossy().into_owned()),
        );
        return export_claude_session_bundles(
            &claude,
            &PathBuf::from(out_dir),
            &ids,
            machine_label.as_deref(),
            export_group.as_deref(),
        );
    }

    let codex = PathBuf::from(&codex_dir);
    let out = PathBuf::from(&out_dir);
    let rollout_index = index_rollouts(&codex)?;
    export_session_bundles_from_index(
        &codex,
        &out,
        &ids,
        machine_label.as_deref(),
        export_group.as_deref(),
        &rollout_index,
    )
}

fn export_session_bundles_from_index(
    codex: &Path,
    out: &Path,
    ids: &[String],
    machine_label: Option<&str>,
    export_group: Option<&str>,
    rollout_index: &HashMap<String, RolloutSource>,
) -> AppResult<Vec<ExportReport>> {
    fs::create_dir_all(out)?;

    let machine = machine_label
        .map(paths::sanitize_slug)
        .unwrap_or_else(paths::machine_label);
    let group = paths::sanitize_slug(export_group.unwrap_or("default"));
    let batch = batch_slug();
    let batch_root = out.join(&machine).join(&group).join(&batch);
    fs::create_dir_all(&batch_root)?;

    // 读 state_5.sqlite 以获取 title / cwd / updated_at（没有也能导出）
    let state_conn = state_db::open_ro(codex).ok();
    // 一次扫完 history.jsonl 建索引，避免每条 id 都重扫
    let history_index = build_history_index(codex)?;

    let mut reports: Vec<ExportReport> = Vec::with_capacity(ids.len());
    for id in ids {
        let r = export_one(
            &codex,
            &batch_root,
            id,
            &machine,
            &group,
            state_conn.as_ref(),
            rollout_index,
            &history_index,
        );
        reports.push(r.unwrap_or_else(|e| ExportReport {
            session_id: id.clone(),
            ok: false,
            bundle_path: None,
            error: Some(e.to_string()),
            skipped_reason: None,
        }));
    }
    Ok(reports)
}

fn export_one(
    codex: &Path,
    batch_root: &Path,
    id: &str,
    machine: &str,
    group: &str,
    state: Option<&rusqlite::Connection>,
    rollout_index: &HashMap<String, RolloutSource>,
    history_index: &HashMap<String, Vec<String>>,
) -> AppResult<ExportReport> {
    let mut report = ExportReport {
        session_id: id.to_string(),
        ok: false,
        bundle_path: None,
        error: None,
        skipped_reason: None,
    };

    let state_source = if rollout_index.contains_key(id) {
        None
    } else if let Some(conn) = state {
        rollout_source_from_state(codex, conn, id)?
    } else {
        None
    };
    let rollout_source = match rollout_index.get(id).or(state_source.as_ref()) {
        Some(source) => source,
        None => {
            report.error = Some(format!(
                "未在 sessions/、archived_sessions/ 或 threads.rollout_path 找到 id={}",
                id
            ));
            return Ok(report);
        }
    };

    // 解析 meta
    let payload = rollout_source
        .meta
        .get("payload")
        .cloned()
        .unwrap_or(Value::Null);
    let cwd = payload
        .get("cwd")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let originator = payload
        .get("originator")
        .and_then(|x| x.as_str())
        .map(String::from);
    let session_source = payload
        .get("source")
        .and_then(|x| x.as_str())
        .map(String::from);
    let provider = payload
        .get("model_provider")
        .and_then(|x| x.as_str())
        .map(String::from);

    // 从 state 读 title / updated_at（可选）
    let (title, updated_at) = if let Some(conn) = state {
        conn.query_row(
            "SELECT COALESCE(title,''), COALESCE(updated_at,0) FROM threads WHERE id = ?",
            [id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
        )
        .unwrap_or((String::new(), 0))
    } else {
        (String::new(), 0)
    };

    // 写 bundle 目录
    let bundle_dir = batch_root.join(id);
    let codex_sub = bundle_dir.join("codex").join(&rollout_source.rel);
    if let Some(p) = codex_sub.parent() {
        fs::create_dir_all(p)?;
    }
    fs::copy(&rollout_source.abs, &codex_sub)?;
    let sha = sha256_file(&codex_sub)?;
    let line_count = count_jsonl_lines(&codex_sub)?;

    // 从索引里查该会话的 history 行（O(1) 查询 + O(k) 写）
    let has_history =
        write_history_from_index(history_index, id, &bundle_dir.join("history.jsonl"))?;

    let manifest = BundleManifest {
        version: BUNDLE_VERSION,
        provider: Some(PROVIDER_CODEX.to_string()),
        session_id: id.to_string(),
        rollout_relpath: rollout_source.rel.to_string_lossy().replace('\\', "/"),
        source_relpath: None,
        sidecar_relpath: None,
        exported_at: chrono::Utc::now().to_rfc3339(),
        updated_at,
        thread_name: title,
        session_cwd: cwd,
        session_source,
        session_originator: originator,
        model_provider: provider,
        export_machine: machine.to_string(),
        export_group: group.to_string(),
        sha256_rollout: sha,
        rollout_line_count: line_count,
        has_history,
    };
    fs::write(
        bundle_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;

    report.ok = true;
    report.bundle_path = Some(bundle_dir.to_string_lossy().into_owned());
    Ok(report)
}

/// 一次扫完 history.jsonl，按 session_id 归档，避免批量导出时的 O(N×H) 复扫。
fn build_history_index(codex: &Path) -> AppResult<HashMap<String, Vec<String>>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    let hist = paths::history_path(codex);
    if !hist.is_file() {
        return Ok(out);
    }
    let f = File::open(&hist)?;
    for line in BufReader::new(f).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let id = match serde_json::from_str::<Value>(&line) {
            Ok(v) => v
                .get("session_id")
                .and_then(|x| x.as_str())
                .or_else(|| v.get("id").and_then(|x| x.as_str()))
                .map(String::from),
            Err(_) => None,
        };
        if let Some(id) = id {
            out.entry(id).or_default().push(line);
        }
    }
    Ok(out)
}

fn write_history_from_index(
    history_index: &HashMap<String, Vec<String>>,
    id: &str,
    out_path: &Path,
) -> AppResult<bool> {
    let rows = match history_index.get(id) {
        Some(r) if !r.is_empty() => r,
        _ => return Ok(false),
    };
    let mut w = BufWriter::new(File::create(out_path)?);
    for line in rows {
        writeln!(w, "{}", line)?;
    }
    w.flush()?;
    Ok(true)
}

fn export_claude_session_bundles(
    claude: &Path,
    out: &Path,
    ids: &[String],
    machine_label: Option<&str>,
    export_group: Option<&str>,
) -> AppResult<Vec<ExportReport>> {
    fs::create_dir_all(out)?;
    let machine = machine_label
        .map(paths::sanitize_slug)
        .unwrap_or_else(paths::machine_label);
    let group = paths::sanitize_slug(export_group.unwrap_or("default"));
    let batch = batch_slug();
    let batch_root = out.join(&machine).join(&group).join(&batch);
    fs::create_dir_all(&batch_root)?;

    let sessions = crate::claude_sessions::scan_sessions(claude)?;
    let by_id: HashMap<String, crate::models::SessionSummary> =
        sessions.into_iter().map(|s| (s.id.clone(), s)).collect();
    let mut reports = Vec::with_capacity(ids.len());
    for id in ids {
        reports.push(
            export_one_claude(claude, &batch_root, id, &machine, &group, &by_id).unwrap_or_else(
                |e| ExportReport {
                    session_id: id.clone(),
                    ok: false,
                    bundle_path: None,
                    error: Some(e.to_string()),
                    skipped_reason: None,
                },
            ),
        );
    }
    Ok(reports)
}

fn export_one_claude(
    claude: &Path,
    batch_root: &Path,
    id: &str,
    machine: &str,
    group: &str,
    sessions: &HashMap<String, crate::models::SessionSummary>,
) -> AppResult<ExportReport> {
    let session = sessions
        .get(id)
        .ok_or_else(|| AppError::NotFound(format!("Claude 会话不存在: {id}")))?;
    let source = PathBuf::from(&session.rollout_path);
    let source_rel = crate::claude_sessions::session_relpath(claude, &source);
    let source_rel_string = source_rel.to_string_lossy().replace('\\', "/");
    let bundle_dir = batch_root.join(id);
    let claude_sub = bundle_dir.join(PROVIDER_CLAUDE).join(&source_rel);
    if let Some(parent) = claude_sub.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&source, &claude_sub)?;
    let sha = sha256_file(&claude_sub)?;
    let line_count = count_jsonl_lines(&claude_sub)?;

    let mut sidecar_rel: Option<String> = None;
    if let Some(sidecar) = crate::claude_sessions::sidecar_path_for(&source) {
        if sidecar.exists() {
            let rel = PathBuf::from("sidecars").join(paths::sanitize_slug(id));
            copy_path_recursive(&sidecar, &bundle_dir.join(&rel))?;
            sidecar_rel = Some(rel.to_string_lossy().replace('\\', "/"));
        }
    }

    let manifest = BundleManifest {
        version: BUNDLE_VERSION,
        provider: Some(PROVIDER_CLAUDE.to_string()),
        session_id: id.to_string(),
        rollout_relpath: source_rel_string.clone(),
        source_relpath: Some(source_rel_string),
        sidecar_relpath: sidecar_rel,
        exported_at: chrono::Utc::now().to_rfc3339(),
        updated_at: session.updated_at,
        thread_name: session.title.clone(),
        session_cwd: session.cwd.clone(),
        session_source: Some(PROVIDER_CLAUDE.to_string()),
        session_originator: None,
        model_provider: session.model.clone(),
        export_machine: machine.to_string(),
        export_group: group.to_string(),
        sha256_rollout: sha,
        rollout_line_count: line_count,
        has_history: false,
    };
    fs::write(
        bundle_dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;

    Ok(ExportReport {
        session_id: id.to_string(),
        ok: true,
        bundle_path: Some(bundle_dir.to_string_lossy().into_owned()),
        error: None,
        skipped_reason: None,
    })
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn export_all_bundles(
    provider: Option<String>,
    codex_dir: String,
    claude_dir: Option<String>,
    out_dir: String,
    machine_label: Option<String>,
    export_group: Option<String>,
    active_only: bool,
) -> AppResult<Vec<ExportReport>> {
    if provider.as_deref().unwrap_or(PROVIDER_CODEX) == PROVIDER_CLAUDE {
        let claude = PathBuf::from(
            claude_dir
                .unwrap_or_else(|| paths::default_claude_dir().to_string_lossy().into_owned()),
        );
        let ids = crate::claude_sessions::scan_sessions(&claude)?
            .into_iter()
            .map(|session| session.id)
            .collect::<Vec<_>>();
        return export_claude_session_bundles(
            &claude,
            &PathBuf::from(out_dir),
            &ids,
            machine_label.as_deref(),
            export_group.as_deref(),
        );
    }

    let codex = PathBuf::from(&codex_dir);
    let out = PathBuf::from(&out_dir);
    let rollout_index = index_rollouts(&codex)?;
    let mut ids: Vec<String> = Vec::new();
    if active_only {
        let store = family::load(&codex)?;
        for f in store.families.values() {
            ids.push(f.active_id.clone());
        }
        if ids.is_empty() {
            ids.extend(rollout_index.keys().cloned());
        }
    } else {
        ids.extend(rollout_index.keys().cloned());
    }
    ids.sort();
    ids.dedup();
    export_session_bundles_from_index(
        &codex,
        &out,
        &ids,
        machine_label.as_deref(),
        export_group.as_deref(),
        &rollout_index,
    )
}

// ========================= 列出 / 校验 =========================

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn list_bundles(src_dir: String, provider: Option<String>) -> AppResult<Vec<BundleListItem>> {
    let root = PathBuf::from(&src_dir);
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut out: Vec<BundleListItem> = Vec::new();
    for entry in walkdir::WalkDir::new(&root)
        .max_depth(6)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() && entry.file_name() == std::ffi::OsStr::new("manifest.json")
        {
            let mp = entry.path();
            if let Ok(raw) = fs::read_to_string(mp) {
                if let Ok(m) = serde_json::from_str::<BundleManifest>(&raw) {
                    if let Some(provider) = provider.as_deref() {
                        if bundle_provider(&m) != provider {
                            continue;
                        }
                    }
                    let bdir = mp.parent().unwrap_or(Path::new(""));
                    out.push(BundleListItem {
                        bundle_dir: bdir.to_string_lossy().into_owned(),
                        manifest: m,
                        verified: None,
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| b.manifest.exported_at.cmp(&a.manifest.exported_at));
    Ok(out)
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn verify_bundles(src_dir: String, provider: Option<String>) -> AppResult<Vec<BundleListItem>> {
    let mut items = list_bundles(src_dir, provider)?;
    for it in items.iter_mut() {
        let rel = paths::checked_relative_path(&it.manifest.rollout_relpath)?;
        let base = if bundle_provider(&it.manifest) == PROVIDER_CLAUDE {
            PROVIDER_CLAUDE
        } else {
            PROVIDER_CODEX
        };
        let file = PathBuf::from(&it.bundle_dir).join(base).join(&rel);
        if !file.is_file() {
            it.verified = Some(false);
            continue;
        }
        let actual = sha256_file(&file).unwrap_or_default();
        it.verified = Some(actual == it.manifest.sha256_rollout);
    }
    Ok(items)
}

// ========================= 导入 =========================

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn import_session_bundles(
    provider: Option<String>,
    src_dir: String,
    codex_dir: String,
    claude_dir: Option<String>,
    mode: ImportMode,
    make_visible: bool,
    strict: bool,
) -> AppResult<Vec<ImportReport>> {
    let codex = PathBuf::from(&codex_dir);
    let claude = PathBuf::from(
        claude_dir.unwrap_or_else(|| paths::default_claude_dir().to_string_lossy().into_owned()),
    );
    let items = list_bundles(src_dir, provider.clone())?;
    let mut reports: Vec<ImportReport> = Vec::with_capacity(items.len());
    for it in items {
        let item_provider = provider
            .as_deref()
            .unwrap_or_else(|| bundle_provider(&it.manifest));
        reports.push(
            (if item_provider == PROVIDER_CLAUDE {
                import_one_claude(&claude, &it, &mode, strict)
            } else {
                import_one(&codex, &it, &mode, make_visible, strict)
            })
            .unwrap_or_else(|e| ImportReport {
                session_id: it.manifest.session_id.clone(),
                ok: false,
                rollout_written: false,
                history_appended: 0,
                threads_upserted: false,
                index_appended: false,
                skipped_reason: None,
                error: Some(e.to_string()),
                verified: false,
                sha_mismatch: false,
            }),
        );
    }
    Ok(reports)
}

fn bundle_provider(manifest: &BundleManifest) -> &str {
    manifest.provider.as_deref().unwrap_or(PROVIDER_CODEX)
}

fn import_one_claude(
    claude: &Path,
    item: &BundleListItem,
    mode: &ImportMode,
    strict: bool,
) -> AppResult<ImportReport> {
    let mut report = ImportReport {
        session_id: item.manifest.session_id.clone(),
        ok: false,
        rollout_written: false,
        history_appended: 0,
        threads_upserted: false,
        index_appended: false,
        skipped_reason: None,
        error: None,
        verified: false,
        sha_mismatch: false,
    };

    let rel = paths::checked_relative_path(&item.manifest.rollout_relpath)?;
    let src_file = PathBuf::from(&item.bundle_dir)
        .join(PROVIDER_CLAUDE)
        .join(&rel);
    if !src_file.is_file() {
        report.error = Some(format!(
            "bundle Claude JSONL 缺失: {}",
            src_file.to_string_lossy()
        ));
        return Ok(report);
    }

    let actual = sha256_file(&src_file)?;
    if actual != item.manifest.sha256_rollout {
        report.sha_mismatch = true;
        if strict {
            report.error = Some("sha256 不一致，strict 模式跳过".into());
            return Ok(report);
        }
    } else {
        report.verified = true;
    }

    let source_rel = item
        .manifest
        .source_relpath
        .as_deref()
        .unwrap_or(&item.manifest.rollout_relpath);
    let dest_abs =
        paths::claude_projects_dir(claude).join(paths::checked_relative_path(source_rel)?);
    if dest_abs.is_file() {
        match mode {
            ImportMode::Skip => {
                report.skipped_reason = Some("本地已存在，Skip 模式".into());
                report.ok = true;
                return Ok(report);
            }
            ImportMode::KeepLocal => {
                let local_mtime = fs::metadata(&dest_abs).and_then(|m| m.modified()).ok();
                let bundle_mtime = fs::metadata(&src_file).and_then(|m| m.modified()).ok();
                if let (Some(local), Some(bundle)) = (local_mtime, bundle_mtime) {
                    if local >= bundle {
                        report.skipped_reason = Some("本地 mtime 更新，KeepLocal 模式".into());
                        report.ok = true;
                        return Ok(report);
                    }
                }
            }
            ImportMode::Overwrite => {}
        }
    }

    if let Some(parent) = dest_abs.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&src_file, &dest_abs)?;
    report.rollout_written = true;

    if let Some(sidecar_rel) = item.manifest.sidecar_relpath.as_deref() {
        let sidecar_src =
            PathBuf::from(&item.bundle_dir).join(paths::checked_relative_path(sidecar_rel)?);
        if sidecar_src.exists() {
            if let Some(sidecar_dest) = crate::claude_sessions::sidecar_path_for(&dest_abs) {
                if sidecar_dest.exists() && matches!(mode, ImportMode::Overwrite) {
                    remove_path_recursive(&sidecar_dest)?;
                }
                if !sidecar_dest.exists() {
                    copy_path_recursive(&sidecar_src, &sidecar_dest)?;
                }
            }
        }
    }

    report.ok = true;
    Ok(report)
}

fn import_one(
    codex: &Path,
    item: &BundleListItem,
    mode: &ImportMode,
    make_visible: bool,
    strict: bool,
) -> AppResult<ImportReport> {
    let mut report = ImportReport {
        session_id: item.manifest.session_id.clone(),
        ok: false,
        rollout_written: false,
        history_appended: 0,
        threads_upserted: false,
        index_appended: false,
        skipped_reason: None,
        error: None,
        verified: false,
        sha_mismatch: false,
    };

    // 1) 找源文件
    let rel = paths::checked_relative_path(&item.manifest.rollout_relpath)?;
    let src_file = PathBuf::from(&item.bundle_dir).join("codex").join(&rel);
    if !src_file.is_file() {
        report.error = Some(format!(
            "bundle rollout 缺失: {}",
            src_file.to_string_lossy()
        ));
        return Ok(report);
    }

    // 2) 校验
    let actual = sha256_file(&src_file)?;
    if actual != item.manifest.sha256_rollout {
        report.sha_mismatch = true;
        if strict {
            report.error = Some("sha256 不一致，strict 模式跳过".into());
            return Ok(report);
        }
    } else {
        report.verified = true;
    }

    // 3) 目标路径决策
    let dest_abs = codex.join(&rel);
    if dest_abs.is_file() {
        match mode {
            ImportMode::Skip => {
                report.skipped_reason = Some("本地已存在，Skip 模式".into());
                report.ok = true;
                return Ok(report);
            }
            ImportMode::KeepLocal => {
                let local_mtime = fs::metadata(&dest_abs).and_then(|m| m.modified()).ok();
                let bundle_mtime = fs::metadata(&src_file).and_then(|m| m.modified()).ok();
                if let (Some(l), Some(b)) = (local_mtime, bundle_mtime) {
                    if l >= b {
                        report.skipped_reason = Some("本地 mtime 更新，KeepLocal 模式".into());
                        report.ok = true;
                        // 仍然尝试补 history
                        let hist_src = PathBuf::from(&item.bundle_dir).join("history.jsonl");
                        if hist_src.is_file() {
                            report.history_appended =
                                append_history(codex, &hist_src, &item.manifest.session_id)?;
                        }
                        return Ok(report);
                    }
                }
            }
            ImportMode::Overwrite => {}
        }
    }

    // 4) 拷 rollout
    if let Some(parent) = dest_abs.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&src_file, &dest_abs)?;
    report.rollout_written = true;

    // 5) 追加 history
    let hist_src = PathBuf::from(&item.bundle_dir).join("history.jsonl");
    if hist_src.is_file() {
        report.history_appended = append_history(codex, &hist_src, &item.manifest.session_id)?;
    }

    // 6) 若需 make_visible，则 upsert threads + 追加 session_index
    if make_visible {
        if paths::state_db_path(codex).is_file() {
            if let Err(e) = upsert_threads_minimal(codex, &item.manifest, &dest_abs) {
                report.error = Some(format!("threads upsert 失败: {}", e));
            } else {
                report.threads_upserted = true;
            }
        }
        let line = serde_json::json!({
            "id": item.manifest.session_id,
            "thread_name": item.manifest.thread_name,
            "rollout_path": dest_abs.to_string_lossy(),
            "updated_at": item.manifest.updated_at,
        });
        let idx = paths::session_index_path(codex);
        let mut exist = false;
        if idx.is_file() {
            for l in BufReader::new(File::open(&idx)?).lines() {
                let l = l?;
                if let Ok(v) = serde_json::from_str::<Value>(&l) {
                    if v.get("id").and_then(|x| x.as_str()) == Some(&item.manifest.session_id) {
                        exist = true;
                        break;
                    }
                }
            }
        }
        if !exist {
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&idx)?;
            writeln!(f, "{}", line)?;
            report.index_appended = true;
        }
    }

    report.ok = true;
    Ok(report)
}

fn append_history(codex: &Path, src: &Path, id: &str) -> AppResult<u32> {
    let hist = paths::history_path(codex);
    let mut existing: std::collections::HashSet<String> = Default::default();
    if hist.is_file() {
        for line in BufReader::new(File::open(&hist)?).lines() {
            let line = line?;
            if !line.trim().is_empty() {
                existing.insert(line);
            }
        }
    }
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&hist)?;
    let mut added = 0u32;
    for line in BufReader::new(File::open(src)?).lines() {
        let line = line?;
        if line.trim().is_empty() || existing.contains(&line) {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            if v.get("session_id").and_then(|x| x.as_str()) == Some(id)
                || v.get("id").and_then(|x| x.as_str()) == Some(id)
            {
                writeln!(out, "{}", line)?;
                added += 1;
            }
        }
    }
    Ok(added)
}

fn upsert_threads_minimal(codex: &Path, m: &BundleManifest, dest_abs: &Path) -> AppResult<()> {
    let conn = state_db::open(codex)?;
    let source = m
        .session_source
        .as_deref()
        .filter(|source| crate::repair::is_desktop_visible_source(Some(source)))
        .unwrap_or("cli")
        .to_string();
    let sql = "INSERT INTO threads (
            id, rollout_path, created_at, updated_at, source, model_provider, cwd, title,
            sandbox_policy, approval_mode, memory_mode, archived, tokens_used, has_user_event,
            first_user_message, cli_version
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, 1, ?, '')
        ON CONFLICT(id) DO UPDATE SET
            rollout_path=excluded.rollout_path,
            updated_at=excluded.updated_at,
            model_provider=excluded.model_provider,
            title=excluded.title,
            first_user_message=excluded.first_user_message";
    conn.execute(
        sql,
        params![
            m.session_id,
            dest_abs.to_string_lossy(),
            m.updated_at / 1000,
            m.updated_at / 1000,
            source,
            m.model_provider.clone().unwrap_or_else(|| "openai".into()),
            m.session_cwd,
            m.thread_name,
            DEFAULT_SANDBOX_POLICY,
            DEFAULT_APPROVAL_MODE,
            DEFAULT_MEMORY_MODE,
            m.thread_name,
        ],
    )?;
    Ok(())
}

fn copy_path_recursive(from: &Path, to: &Path) -> AppResult<()> {
    if from.is_file() {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(from, to)?;
        return Ok(());
    }
    if !from.is_dir() {
        return Err(AppError::NotFound(format!(
            "待复制路径不存在: {}",
            from.to_string_lossy()
        )));
    }
    for entry in walkdir::WalkDir::new(from)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        let rel = entry.path().strip_prefix(from).map_err(|e| {
            AppError::Path(format!(
                "无法计算相对路径 {}: {}",
                entry.path().to_string_lossy(),
                e
            ))
        })?;
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&dest)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

fn remove_path_recursive(path: &Path) -> AppResult<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else if path.is_file() {
        fs::remove_file(path)?;
    }
    Ok(())
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

    fn write_claude_session(claude: &Path, id: &str) -> AppResult<()> {
        let dir = claude.join("projects").join("sample-project");
        fs::create_dir_all(&dir)?;
        let line = serde_json::json!({
            "sessionId": id,
            "cwd": "F:\\work\\sample-project",
            "timestamp": "2026-04-20T10:00:00Z",
            "type": "assistant",
            "message": {
                "role": "assistant",
                "model": "claude-3-5-sonnet",
                "usage": {"input_tokens": 2, "output_tokens": 3},
                "content": "hello"
            }
        });
        fs::write(
            dir.join(format!("{id}.jsonl")),
            format!("{}\n", serde_json::to_string(&line)?),
        )?;
        Ok(())
    }

    #[test]
    fn exports_verifies_and_imports_claude_bundle() -> AppResult<()> {
        let root = temp_dir("cc-session-manager-claude-bundle-test");
        let source_claude = root.join("source-claude");
        let import_claude = root.join("import-claude");
        let bundle_dir = root.join("bundles");
        write_claude_session(&source_claude, "claude-bundle-1")?;

        let reports = export_session_bundles(
            Some(PROVIDER_CLAUDE.to_string()),
            String::new(),
            Some(source_claude.to_string_lossy().into_owned()),
            bundle_dir.to_string_lossy().into_owned(),
            vec!["claude-bundle-1".to_string()],
            Some("test-machine".to_string()),
            Some("default".to_string()),
        )?;
        assert_eq!(reports.len(), 1);
        assert!(reports[0].ok);

        let verified = verify_bundles(
            bundle_dir.to_string_lossy().into_owned(),
            Some(PROVIDER_CLAUDE.to_string()),
        )?;
        assert_eq!(verified.len(), 1);
        assert_eq!(verified[0].verified, Some(true));

        let imported = import_session_bundles(
            Some(PROVIDER_CLAUDE.to_string()),
            bundle_dir.to_string_lossy().into_owned(),
            String::new(),
            Some(import_claude.to_string_lossy().into_owned()),
            ImportMode::Skip,
            false,
            true,
        )?;
        assert_eq!(imported.len(), 1);
        assert!(imported[0].ok);
        assert!(paths::claude_projects_dir(&import_claude)
            .join("sample-project")
            .join("claude-bundle-1.jsonl")
            .is_file());

        fs::remove_dir_all(root).ok();
        Ok(())
    }
}

// ========================= zip 打包 =========================

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn pack_bundles_zip(src_dir: String, zip_path: String) -> AppResult<ZipReport> {
    let src = PathBuf::from(&src_dir);
    let out = PathBuf::from(&zip_path);
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = File::create(&out)?;
    let mut writer = BufWriter::new(file);

    // 手工实现 STORE 模式的 zip：跨机器只要能解包即可。CRC-32 + 无压缩。
    let mut central: Vec<CentralEntry> = Vec::new();
    let mut total_bytes: u64 = 0;
    let mut file_count: u32 = 0;
    let mut offset: u32 = 0;

    for entry in walkdir::WalkDir::new(&src)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(&src)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| entry.path().to_path_buf());
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let mut data: Vec<u8> = Vec::new();
        File::open(entry.path())?.read_to_end(&mut data)?;
        let crc = crc32(&data);
        let size = data.len() as u32;
        total_bytes += size as u64;
        file_count += 1;
        // Local file header
        writer.write_all(&0x04034b50u32.to_le_bytes())?; // signature
        writer.write_all(&20u16.to_le_bytes())?; // version needed
        writer.write_all(&0u16.to_le_bytes())?; // flags
        writer.write_all(&0u16.to_le_bytes())?; // method STORE
        writer.write_all(&0u16.to_le_bytes())?; // mod time
        writer.write_all(&0u16.to_le_bytes())?; // mod date
        writer.write_all(&crc.to_le_bytes())?;
        writer.write_all(&size.to_le_bytes())?; // compressed size
        writer.write_all(&size.to_le_bytes())?; // uncompressed size
        writer.write_all(&(rel_str.len() as u16).to_le_bytes())?;
        writer.write_all(&0u16.to_le_bytes())?; // extra len
        writer.write_all(rel_str.as_bytes())?;
        writer.write_all(&data)?;

        let local_header_size = 30 + rel_str.len() as u32;
        central.push(CentralEntry {
            name: rel_str,
            crc,
            size,
            offset,
        });
        offset += local_header_size + size;
    }
    writer.flush()?;

    // Central directory
    let cd_offset = offset;
    let mut cd_size: u32 = 0;
    for e in &central {
        writer.write_all(&0x02014b50u32.to_le_bytes())?; // signature
        writer.write_all(&20u16.to_le_bytes())?; // version made by
        writer.write_all(&20u16.to_le_bytes())?; // version needed
        writer.write_all(&0u16.to_le_bytes())?; // flags
        writer.write_all(&0u16.to_le_bytes())?; // method
        writer.write_all(&0u16.to_le_bytes())?; // mod time
        writer.write_all(&0u16.to_le_bytes())?; // mod date
        writer.write_all(&e.crc.to_le_bytes())?;
        writer.write_all(&e.size.to_le_bytes())?;
        writer.write_all(&e.size.to_le_bytes())?;
        writer.write_all(&(e.name.len() as u16).to_le_bytes())?;
        writer.write_all(&0u16.to_le_bytes())?; // extra len
        writer.write_all(&0u16.to_le_bytes())?; // comment len
        writer.write_all(&0u16.to_le_bytes())?; // disk start
        writer.write_all(&0u16.to_le_bytes())?; // int attrs
        writer.write_all(&0u32.to_le_bytes())?; // ext attrs
        writer.write_all(&e.offset.to_le_bytes())?;
        writer.write_all(e.name.as_bytes())?;
        cd_size += 46 + e.name.len() as u32;
    }

    // EOCD
    writer.write_all(&0x06054b50u32.to_le_bytes())?;
    writer.write_all(&0u16.to_le_bytes())?; // disk
    writer.write_all(&0u16.to_le_bytes())?; // cd start disk
    writer.write_all(&(central.len() as u16).to_le_bytes())?;
    writer.write_all(&(central.len() as u16).to_le_bytes())?;
    writer.write_all(&cd_size.to_le_bytes())?;
    writer.write_all(&cd_offset.to_le_bytes())?;
    writer.write_all(&0u16.to_le_bytes())?; // comment len
    writer.flush()?;

    Ok(ZipReport {
        path: out.to_string_lossy().into_owned(),
        files: file_count,
        bytes: total_bytes,
    })
}

struct CentralEntry {
    name: String,
    crc: u32,
    size: u32,
    offset: u32,
}

fn crc32(data: &[u8]) -> u32 {
    // 标准 CRC-32（IEEE 802.3），查表法
    static mut TABLE: [u32; 256] = [0; 256];
    static INIT: std::sync::Once = std::sync::Once::new();
    unsafe {
        INIT.call_once(|| {
            for i in 0..256u32 {
                let mut c = i;
                for _ in 0..8 {
                    c = if c & 1 == 1 {
                        0xEDB88320 ^ (c >> 1)
                    } else {
                        c >> 1
                    };
                }
                TABLE[i as usize] = c;
            }
        });
        let mut crc: u32 = 0xFFFFFFFF;
        for &b in data {
            let idx = ((crc ^ b as u32) & 0xFF) as usize;
            crc = TABLE[idx] ^ (crc >> 8);
        }
        crc ^ 0xFFFFFFFF
    }
}

fn checked_zip_slice<'a>(
    data: &'a [u8],
    start: usize,
    len: usize,
    label: &str,
) -> AppResult<&'a [u8]> {
    let end = start
        .checked_add(len)
        .ok_or_else(|| AppError::Other(format!("{label} 偏移溢出")))?;
    data.get(start..end)
        .ok_or_else(|| AppError::Other(format!("{label} 超出 zip 文件边界")))
}

fn read_zip_u16(data: &[u8], start: usize, label: &str) -> AppResult<u16> {
    let bytes = checked_zip_slice(data, start, 2, label)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_zip_u32(data: &[u8], start: usize, label: &str) -> AppResult<u32> {
    let bytes = checked_zip_slice(data, start, 4, label)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg_attr(feature = "desktop", tauri::command)]
pub fn unpack_zip(zip_path: String, dst_dir: String) -> AppResult<ZipReport> {
    // 流式解压：只支持 STORE 模式。只把尾部 + central directory 读进内存，
    // 每个文件的 payload 用 seek + Read::take + io::copy，不再一次性装全文件。
    let dst = PathBuf::from(&dst_dir);
    fs::create_dir_all(&dst)?;

    let mut f = File::open(&zip_path)?;
    let file_len = f.metadata()?.len();
    if file_len < 22 {
        return Err(AppError::Other("不是合法的 zip 文件（过短）".into()));
    }

    // EOCD 最多携带 65535 字节注释，尾部窗口够 65557 字节即可覆盖。
    let tail_window = file_len.min(65557);
    let tail_base = file_len - tail_window;
    f.seek(SeekFrom::Start(tail_base))?;
    let mut tail = vec![0u8; tail_window as usize];
    f.read_exact(&mut tail)?;

    // 在尾部 buffer 中倒序找 EOCD 签名。
    let sig = [0x50u8, 0x4b, 0x05, 0x06];
    let eocd_in_tail = (0..=tail.len().saturating_sub(22))
        .rev()
        .find(|&i| tail[i..i + 4] == sig)
        .ok_or_else(|| AppError::Other("不是合法的 zip 文件（未找到 EOCD）".into()))?;

    let cd_count = read_zip_u16(&tail, eocd_in_tail + 10, "central directory 数量")? as usize;
    let cd_size = read_zip_u32(&tail, eocd_in_tail + 12, "central directory 总大小")? as u64;
    let cd_offset = read_zip_u32(&tail, eocd_in_tail + 16, "central directory 偏移")? as u64;
    if cd_offset
        .checked_add(cd_size)
        .map(|v| v > file_len)
        .unwrap_or(true)
    {
        return Err(AppError::Other("central directory 范围越界".into()));
    }

    // 一次性读 central directory（通常远小于 payload 总和）。
    f.seek(SeekFrom::Start(cd_offset))?;
    let mut cd = vec![0u8; cd_size as usize];
    f.read_exact(&mut cd)?;

    let mut pos: usize = 0;
    let mut total_bytes: u64 = 0;
    let mut file_count: u32 = 0;
    for _ in 0..cd_count {
        if checked_zip_slice(&cd, pos, 4, "central directory 签名")? != [0x50, 0x4b, 0x01, 0x02] {
            return Err(AppError::Other("central directory 损坏".into()));
        }
        checked_zip_slice(&cd, pos, 46, "central directory header")?;
        let method = read_zip_u16(&cd, pos + 10, "central directory 压缩方式")?;
        if method != 0 {
            return Err(AppError::Other(format!(
                "不支持的压缩方式 method={} (仅支持 STORE)",
                method
            )));
        }
        let csize = read_zip_u32(&cd, pos + 20, "central directory 压缩后大小")? as u64;
        let name_len = read_zip_u16(&cd, pos + 28, "central directory 文件名长度")? as usize;
        let extra_len = read_zip_u16(&cd, pos + 30, "central directory extra 长度")? as usize;
        let cmt_len = read_zip_u16(&cd, pos + 32, "central directory 注释长度")? as usize;
        let local_off = read_zip_u32(&cd, pos + 42, "local header 偏移")? as u64;
        let name_bytes = checked_zip_slice(&cd, pos + 46, name_len, "central directory 文件名")?;
        let name = String::from_utf8(name_bytes.to_vec())
            .map_err(|e| AppError::Other(format!("zip 文件名不是 UTF-8: {}", e)))?;
        let advance = 46usize
            .checked_add(name_len)
            .and_then(|v| v.checked_add(extra_len))
            .and_then(|v| v.checked_add(cmt_len))
            .ok_or_else(|| AppError::Other("central directory entry 长度溢出".into()))?;
        pos = pos
            .checked_add(advance)
            .ok_or_else(|| AppError::Other("central directory 游标溢出".into()))?;

        // 读 local header（30 字节固定区）
        if local_off
            .checked_add(30)
            .map(|v| v > file_len)
            .unwrap_or(true)
        {
            return Err(AppError::Other(format!("local header 越界: {}", name)));
        }
        f.seek(SeekFrom::Start(local_off))?;
        let mut lh = [0u8; 30];
        f.read_exact(&mut lh)?;
        if lh[..4] != [0x50, 0x4b, 0x03, 0x04] {
            return Err(AppError::Other(format!("local header 损坏: {}", name)));
        }
        let local_method = read_zip_u16(&lh, 8, "local header 压缩方式")?;
        if local_method != 0 {
            return Err(AppError::Other(format!(
                "local header 压缩方式不支持 method={} (仅支持 STORE): {}",
                local_method, name
            )));
        }
        let l_name_len = read_zip_u16(&lh, 26, "local header 文件名长度")? as u64;
        let l_extra_len = read_zip_u16(&lh, 28, "local header extra 长度")? as u64;
        let payload_start = local_off
            .checked_add(30)
            .and_then(|v| v.checked_add(l_name_len))
            .and_then(|v| v.checked_add(l_extra_len))
            .ok_or_else(|| AppError::Other(format!("payload 偏移溢出: {}", name)))?;
        if payload_start
            .checked_add(csize)
            .map(|v| v > file_len)
            .unwrap_or(true)
        {
            return Err(AppError::Other(format!("payload 范围越界: {}", name)));
        }

        let rel = paths::checked_relative_path(&name)?;
        let out_path = dst.join(&rel);
        if name.ends_with('/') {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // 流式 copy payload：不再把文件内容一次性读进内存
        f.seek(SeekFrom::Start(payload_start))?;
        let mut out_file = File::create(&out_path)?;
        let copied = std::io::copy(&mut (&mut f).take(csize), &mut out_file)?;
        if copied != csize {
            return Err(AppError::Other(format!(
                "payload 实际读取字节 {} ≠ 声明 {}: {}",
                copied, csize, name
            )));
        }
        out_file.flush().ok();

        total_bytes += csize;
        file_count += 1;
    }

    Ok(ZipReport {
        path: dst.to_string_lossy().into_owned(),
        files: file_count,
        bytes: total_bytes,
    })
}
