// 关卡相关的 Tauri 命令
// MVP 阶段使用 in-memory 存储；W2.3 会换成 SQLite

use std::sync::Mutex;
use tauri::State;

use crate::types::*;

/// 关卡进度存储
#[derive(Default)]
pub struct LevelStore {
    pub progress: Mutex<Vec<LevelProgress>>,
}

/// 获取所有关卡（这里返回内置 5 个关卡，后续 W2.3 改为从 DB 读）
#[tauri::command]
pub fn list_levels() -> Vec<Level> {
    crate::content::builtin_levels()
}

/// 获取关卡详情
#[tauri::command]
pub fn get_level(id: String) -> Option<Level> {
    crate::content::builtin_levels().into_iter().find(|l| l.id == id)
}

/// 获取当前用户的所有关卡进度
#[tauri::command]
pub fn list_progress(store: State<'_, LevelStore>) -> Vec<LevelProgress> {
    store.progress.lock().unwrap().clone()
}

/// 标记关卡为进行中
#[tauri::command]
pub fn start_level(id: String, store: State<'_, LevelStore>) -> Result<LevelProgress, String> {
    let mut progress = store.progress.lock().unwrap();
    if let Some(p) = progress.iter_mut().find(|p| p.level_id == id) {
        p.status = LevelStatus::InProgress;
        p.attempts += 1;
        return Ok(p.clone());
    }
    let new = LevelProgress {
        level_id: id,
        status: LevelStatus::InProgress,
        attempts: 1,
        best_score: None,
        completed_at: None,
    };
    progress.push(new.clone());
    Ok(new)
}

/// 提交关卡结果
#[tauri::command]
pub fn submit_level(
    level_id: String,
    score: u32,
    rubric: ScoringCriteria,
    feedback: String,
    store: State<'_, LevelStore>,
) -> Result<LevelProgress, String> {
    if score > 100 {
        return Err("score must be 0-100".to_string());
    }
    let mut progress = store.progress.lock().unwrap();
    let entry = progress
        .iter_mut()
        .find(|p| p.level_id == level_id)
        .ok_or_else(|| "level not started; call start_level first".to_string())?;

    entry.status = LevelStatus::Completed;
    entry.best_score = Some(match entry.best_score {
        Some(prev) => prev.max(score),
        None => score,
    });
    entry.completed_at = Some(now_millis());

    // MVP: 把提交记录存到 stderr，便于调试
    eprintln!(
        "[submit_level] {} score={} rubric={:?} feedback={}",
        level_id, score, rubric, feedback
    );
    Ok(entry.clone())
}

/// 获取已完成的关卡 ID 列表（用于解锁判断）
#[tauri::command]
pub fn completed_level_ids(store: State<'_, LevelStore>) -> Vec<String> {
    store
        .progress
        .lock()
        .unwrap()
        .iter()
        .filter(|p| p.status == LevelStatus::Completed)
        .map(|p| p.level_id.clone())
        .collect()
}

fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
