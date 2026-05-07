use std::fs;
use std::path::PathBuf;

use crate::error::{AppError, AppResult};
use crate::models::{DirValidation, Settings};
use crate::paths;
use crate::state_db;

fn config_file(app: &tauri::AppHandle) -> AppResult<PathBuf> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Path(e.to_string()))?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("settings.json"))
}

#[tauri::command]
pub fn get_settings(app: tauri::AppHandle) -> AppResult<Settings> {
    let file = config_file(&app)?;
    if !file.exists() {
        return Ok(Settings::default());
    }
    let raw = fs::read_to_string(&file)?;
    let parsed: Settings = serde_json::from_str(&raw).unwrap_or_default();
    Ok(parsed)
}

#[tauri::command]
pub fn save_settings(app: tauri::AppHandle, settings: Settings) -> AppResult<()> {
    let file = config_file(&app)?;
    let content = serde_json::to_string_pretty(&settings)?;
    fs::write(file, content)?;
    Ok(())
}

#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn default_codex_dir() -> String {
    paths::default_codex_dir().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn default_claude_dir() -> String {
    paths::default_claude_dir().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn validate_codex_dir(path: String) -> AppResult<DirValidation> {
    let p = PathBuf::from(&path);
    let (exists, has_state, has_sessions) = paths::validate_codex_dir(&p);
    let threads_count = if has_state {
        state_db::count_threads(&p).unwrap_or(0)
    } else {
        0
    };
    Ok(DirValidation {
        valid: exists && has_state && has_sessions,
        has_state_db: has_state,
        has_sessions,
        threads_count,
    })
}

#[tauri::command]
pub fn validate_claude_dir(path: String) -> AppResult<DirValidation> {
    let p = PathBuf::from(&path);
    let (exists, has_projects) = paths::validate_claude_dir(&p);
    let threads_count = if has_projects {
        crate::claude_sessions::scan_sessions(&p)?.len() as u32
    } else {
        0
    };
    Ok(DirValidation {
        valid: exists && has_projects,
        has_state_db: false,
        has_sessions: has_projects,
        threads_count,
    })
}
