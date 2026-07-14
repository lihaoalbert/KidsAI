// 关卡相关的 Tauri 命令
// W2.3: 改用 SQLite 持久化

use std::sync::Mutex;
use tauri::State;

use crate::db::Db;
use crate::types::*;

/// 关卡进度存储（仍保留以便未来扩展非 DB 缓存）
#[derive(Default)]
pub struct LevelStore {
    _lock: Mutex<()>,
}

#[tauri::command]
pub fn list_levels() -> Vec<Level> {
    crate::content::builtin_levels()
}

#[tauri::command]
pub fn get_level(id: String) -> Option<Level> {
    crate::content::builtin_levels()
        .into_iter()
        .find(|l| l.id == id)
}

#[tauri::command]
pub fn list_progress(db: State<'_, Db>) -> Result<Vec<LevelProgress>, String> {
    db.list_progress().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_level(id: String, db: State<'_, Db>) -> Result<LevelProgress, String> {
    db.upsert_progress_in_progress(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn submit_level(
    level_id: String,
    score: u32,
    rubric: ScoringCriteria,
    feedback: String,
    db: State<'_, Db>,
) -> Result<LevelProgress, String> {
    if score > 100 {
        return Err("score must be 0-100".to_string());
    }
    let progress = db
        .mark_completed(&level_id, score)
        .map_err(|e| e.to_string())?;
    eprintln!(
        "[submit_level] {} score={} rubric={:?} feedback={}",
        level_id, score, rubric, feedback
    );
    Ok(progress)
}

#[tauri::command]
pub fn completed_level_ids(db: State<'_, Db>) -> Result<Vec<String>, String> {
    db.list_completed_ids().map_err(|e| e.to_string())
}
