//! 家族树存储：`~/.codex/session_family.json`
//!
//! 设计要点：
//! - manager 自行维护，Codex 原生不感知。
//! - 同一会话线只有一个 `active` 节点对 Codex app 可见，其他节点落入
//!   `archived_sessions/`。这保证"新对话只写进 active 节点"，切换 provider 时
//!   通过整份复制 + 立即归档做到内容连续。
//! - 每次归档时固化 sha256 + line_count，支持后续完整性校验。

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::models::{
    BranchStatus, Family, FamilyBranch, FamilyIntegrityItem, FamilyIntegrityReport, FamilyOverlay,
    FamilyStore,
};
use crate::paths;
use crate::state_db;

/// 进程内并发保护：所有 family store 的 load → mutate → save 都需要持有这把锁。
/// Tauri 的 command 各自跑在独立线程池里，不加锁会出现"读A→读B→写A→写B"覆盖丢数据。
#[derive(Default)]
pub struct FamilyLock(pub Mutex<()>);

/// 封装：持锁执行回调。调用方闭包里做 load / mutate / save。
/// 只有 Tauri command 需要持锁；内部辅助函数（已持锁的调用链下层）直接调 load/save 即可。
pub fn with_lock<R>(
    lock: &FamilyLock,
    f: impl FnOnce(MutexGuard<'_, ()>) -> AppResult<R>,
) -> AppResult<R> {
    let g = lock.0.lock().unwrap_or_else(PoisonError::into_inner);
    f(g)
}

pub fn load(codex_dir: &Path) -> AppResult<FamilyStore> {
    let p = paths::family_store_path(codex_dir);
    if !p.is_file() {
        return Ok(FamilyStore::default());
    }
    let raw = fs::read_to_string(&p)?;
    if raw.trim().is_empty() {
        return Ok(FamilyStore::default());
    }
    let store: FamilyStore = serde_json::from_str(&raw)?;
    Ok(store)
}

pub fn save(codex_dir: &Path, store: &FamilyStore) -> AppResult<()> {
    let final_path = paths::family_store_path(codex_dir);
    let tmp = final_path.with_extension("json.tmp");
    let data = serde_json::to_vec_pretty(store)?;
    fs::write(&tmp, data)?;
    fs::rename(&tmp, &final_path)?;
    Ok(())
}

/// 计算 rollout 文件的字节级 sha256 + 总行数（与 bundle 导出 sha256_file 语义一致）。
///
/// sha256 对**原字节流**哈希（包括换行符原样、BOM、空行等），因此与 `bundle.rs::sha256_file`
/// 同值；`line_count` 是物理行数（按 `\n` 切分得到的非空片段数），用作参考指标。
/// 两者语义彼此独立。
pub fn compute_integrity(path: &Path) -> AppResult<(String, u64)> {
    let mut f = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut f, &mut hasher)?;
    let sha = hex::encode(hasher.finalize());

    let f = fs::File::open(path)?;
    let mut lines: u64 = 0;
    for line in BufReader::new(f).lines() {
        let line = line?;
        if !line.is_empty() {
            lines += 1;
        }
    }
    Ok((sha, lines))
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// 首次遇到的 session 注册为独立家族（root == self），返回 family_id。
pub fn ensure_family_for(
    store: &mut FamilyStore,
    session_id: &str,
    provider: &str,
    rollout_relpath: &str,
    title: &str,
) -> String {
    if let Some(fid) = store.index.get(session_id).cloned() {
        return fid;
    }
    let fid = session_id.to_string();
    let branch = FamilyBranch {
        id: session_id.to_string(),
        provider: provider.to_string(),
        created_at: now_iso(),
        status: BranchStatus::Active,
        rollout_relpath: rollout_relpath.to_string(),
        sha256: None,
        line_count: None,
        note: None,
    };
    let family = Family {
        family_id: fid.clone(),
        root_id: session_id.to_string(),
        title: title.to_string(),
        chain: vec![branch],
        active_id: session_id.to_string(),
        updated_at: now_iso(),
    };
    store.families.insert(fid.clone(), family);
    store.index.insert(session_id.to_string(), fid.clone());
    fid
}

/// 把某分支设为 active，其他活跃分支一律降级为 archived（同一时刻最多一个 active）。
pub fn set_active(store: &mut FamilyStore, family_id: &str, branch_id: &str) -> AppResult<()> {
    let family = store
        .families
        .get_mut(family_id)
        .ok_or_else(|| AppError::NotFound(format!("family not found: {}", family_id)))?;
    let mut found = false;
    for b in family.chain.iter_mut() {
        if b.id == branch_id {
            b.status = BranchStatus::Active;
            found = true;
        } else if matches!(b.status, BranchStatus::Active) {
            b.status = BranchStatus::Archived;
        }
    }
    if !found {
        return Err(AppError::NotFound(format!(
            "branch not in family {}: {}",
            family_id, branch_id
        )));
    }
    family.active_id = branch_id.to_string();
    family.updated_at = now_iso();
    Ok(())
}

/// 追加一个新分支（默认 status=active，所有其他 active 降级）。
pub fn append_branch(
    store: &mut FamilyStore,
    family_id: &str,
    branch: FamilyBranch,
) -> AppResult<()> {
    let new_id = branch.id.clone();
    {
        let family = store
            .families
            .get_mut(family_id)
            .ok_or_else(|| AppError::NotFound(format!("family not found: {}", family_id)))?;
        for b in family.chain.iter_mut() {
            if matches!(b.status, BranchStatus::Active) {
                b.status = BranchStatus::Archived;
            }
        }
        family.chain.push(branch);
        family.active_id = new_id.clone();
        family.updated_at = now_iso();
    }
    store.index.insert(new_id, family_id.to_string());
    Ok(())
}

/// 归档指定分支时固化 sha256 + line_count（rollout 文件必须存在）。
pub fn archive_with_integrity(
    store: &mut FamilyStore,
    codex_dir: &Path,
    family_id: &str,
    branch_id: &str,
) -> AppResult<()> {
    let family = store
        .families
        .get_mut(family_id)
        .ok_or_else(|| AppError::NotFound(format!("family not found: {}", family_id)))?;
    for b in family.chain.iter_mut() {
        if b.id == branch_id {
            let rel = paths::checked_relative_path(&b.rollout_relpath)?;
            let abs = codex_dir.join(&rel);
            if abs.is_file() {
                if let Ok((sha, lines)) = compute_integrity(&abs) {
                    b.sha256 = Some(sha);
                    b.line_count = Some(lines);
                }
            }
            b.status = BranchStatus::Archived;
            family.updated_at = now_iso();
            return Ok(());
        }
    }
    Err(AppError::NotFound(format!(
        "branch not in family {}: {}",
        family_id, branch_id
    )))
}

/// 扫描 family store，对每个已固化的分支比对 rollout 文件。
pub fn verify_integrity(codex_dir: &Path) -> AppResult<FamilyIntegrityReport> {
    let store = load(codex_dir)?;
    let mut items: Vec<FamilyIntegrityItem> = Vec::new();
    let mut all_ok = true;
    for (fid, family) in store.families.iter() {
        for b in family.chain.iter() {
            let expected_sha = b.sha256.clone();
            let expected_lines = b.line_count;
            if expected_sha.is_none() {
                continue; // 没固化过的分支不参与校验
            }
            let rel = paths::checked_relative_path(&b.rollout_relpath)?;
            let abs_main = codex_dir.join(&rel);
            let abs_archived =
                paths::archived_sessions_dir(codex_dir).join(rel.file_name().unwrap_or_default());
            let candidate = if abs_main.is_file() {
                abs_main
            } else if abs_archived.is_file() {
                abs_archived
            } else {
                all_ok = false;
                items.push(FamilyIntegrityItem {
                    family_id: fid.clone(),
                    branch_id: b.id.clone(),
                    ok: false,
                    expected_sha,
                    actual_sha: None,
                    expected_lines,
                    actual_lines: None,
                    missing: true,
                });
                continue;
            };
            match compute_integrity(&candidate) {
                Ok((sha, lines)) => {
                    let sha_ok = expected_sha.as_deref() == Some(sha.as_str());
                    let lines_ok = expected_lines.map(|l| l == lines).unwrap_or(true);
                    let ok = sha_ok && lines_ok;
                    if !ok {
                        all_ok = false;
                    }
                    items.push(FamilyIntegrityItem {
                        family_id: fid.clone(),
                        branch_id: b.id.clone(),
                        ok,
                        expected_sha,
                        actual_sha: Some(sha),
                        expected_lines,
                        actual_lines: Some(lines),
                        missing: false,
                    });
                }
                Err(_) => {
                    all_ok = false;
                    items.push(FamilyIntegrityItem {
                        family_id: fid.clone(),
                        branch_id: b.id.clone(),
                        ok: false,
                        expected_sha,
                        actual_sha: None,
                        expected_lines,
                        actual_lines: None,
                        missing: false,
                    });
                }
            }
        }
    }
    Ok(FamilyIntegrityReport { items, all_ok })
}

/// 从 rollout 文件读第一行 session_meta 的 payload（id / model_provider / cwd / originator）。
pub fn read_session_meta(rollout: &Path) -> AppResult<Value> {
    let f = fs::File::open(rollout)?;
    let mut reader = BufReader::new(f);
    let mut first = String::new();
    reader.read_line(&mut first)?;
    let v: Value = serde_json::from_str(first.trim())?;
    Ok(v)
}

fn scan_rollouts_in(root: PathBuf) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.is_dir() {
        return out;
    }
    for entry in walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if name.starts_with("rollout-") && name.ends_with(".jsonl") {
            out.push(entry.path().to_path_buf());
        }
    }
    out
}

/// 扫描 sessions/ 目录下所有 active `rollout-*.jsonl`。
pub fn scan_rollouts(codex_dir: &Path) -> Vec<PathBuf> {
    scan_rollouts_in(paths::sessions_dir(codex_dir))
}

/// 扫描 archived_sessions/ 目录下所有 archived `rollout-*.jsonl`。
pub fn scan_archived_rollouts(codex_dir: &Path) -> Vec<PathBuf> {
    scan_rollouts_in(paths::archived_sessions_dir(codex_dir))
}

#[tauri::command]
pub fn get_family_store(
    codex_dir: String,
    lock: tauri::State<'_, FamilyLock>,
) -> AppResult<FamilyStore> {
    with_lock(&lock, |_g| {
        let p = PathBuf::from(&codex_dir);
        load(&p)
    })
}

#[tauri::command]
pub fn verify_family_integrity(
    codex_dir: String,
    lock: tauri::State<'_, FamilyLock>,
) -> AppResult<FamilyIntegrityReport> {
    with_lock(&lock, |_g| {
        let p = PathBuf::from(&codex_dir);
        verify_integrity(&p)
    })
}

/// 把 threads 表 + family store + current provider 聚合成 per-session 覆盖信息，
/// 用于 Sessions 列表的 Badge 与 provider / 本地索引维护提示。
#[tauri::command]
pub fn get_session_family_overlay(
    codex_dir: String,
    lock: tauri::State<'_, FamilyLock>,
) -> AppResult<Vec<FamilyOverlay>> {
    let codex = PathBuf::from(&codex_dir);
    let _g = lock.0.lock().unwrap_or_else(PoisonError::into_inner);

    // 1) 读 threads 表（id, model_provider, source, archived）
    let mut thread_state_of: std::collections::BTreeMap<
        String,
        (Option<String>, Option<String>, bool),
    > = std::collections::BTreeMap::new();
    if paths::state_db_path(&codex).is_file() {
        let conn = state_db::open_ro(&codex)?;
        let mut stmt =
            conn.prepare("SELECT id, model_provider, source, COALESCE(archived,0) FROM threads")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                (
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, i64>(3)? != 0,
                ),
            ))
        })?;
        for row in rows.flatten() {
            thread_state_of.insert(row.0, row.1);
        }
    }

    // 2) 读 family store
    let store = load(&codex).unwrap_or_default();

    // 3) 读 current provider
    let cur = crate::repair::read_current_provider_export(&codex);

    let mut out: Vec<FamilyOverlay> = Vec::with_capacity(thread_state_of.len());
    for (id, (provider, source, archived)) in &thread_state_of {
        let family_id = store.index.get(id).cloned();
        let (branch_count, is_active_branch, clone_state) = match family_id.as_ref() {
            None => {
                let cs = compute_clone_state(
                    provider.as_deref(),
                    None,
                    cur.as_deref(),
                    false,
                    source.as_deref(),
                    *archived,
                );
                (0u32, false, cs)
            }
            Some(fid) => {
                let family = store.families.get(fid);
                let branch_count = family.map(|f| f.chain.len() as u32).unwrap_or(0);
                let is_active = family.map(|f| f.active_id == *id).unwrap_or(false);
                let has_clone_in_current = match (family, cur.as_deref()) {
                    (Some(f), Some(cur_p)) => f.chain.iter().any(|b| b.provider == cur_p),
                    _ => false,
                };
                let cs = compute_clone_state(
                    provider.as_deref(),
                    family,
                    cur.as_deref(),
                    has_clone_in_current,
                    source.as_deref(),
                    *archived,
                );
                (branch_count, is_active, cs)
            }
        };
        out.push(FamilyOverlay {
            session_id: id.clone(),
            provider: provider.clone(),
            family_id,
            branch_count,
            is_active_branch,
            clone_state,
        });
    }
    Ok(out)
}

fn compute_clone_state(
    provider: Option<&str>,
    _family: Option<&Family>,
    current: Option<&str>,
    has_clone_in_current: bool,
    source: Option<&str>,
    archived: bool,
) -> String {
    if archived {
        return "matches".into();
    }
    if crate::repair::is_subagent_source(source) {
        return "subagent".into();
    }
    match (provider, current) {
        (Some(p), Some(cur)) if p == cur => {
            if crate::repair::is_desktop_visible_source(source) {
                "matches".into()
            } else {
                "resync".into()
            }
        }
        (Some(_), Some(_)) if has_clone_in_current => "has_clone".into(),
        (Some(_), Some(_)) => "clonable".into(),
        _ => "unknown".into(),
    }
}
