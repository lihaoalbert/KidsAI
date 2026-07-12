// Tauri ↔ Frontend 共享的数据类型（Rust 版本）
// 和 frontend 的 `shared/types/level.ts` 字段保持一致

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LevelStatus {
    Locked,
    Available,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelStep {
    pub id: String,
    pub order_num: u32,
    pub title: String,
    pub instruction: String,
    #[serde(rename = "type")]
    pub step_type: String, // "input" | "choice" | "action" | "free" | "reference_setup" | "reference_recreate"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// W3.7+ 当 step_type="reference_recreate" 时告诉前端用哪种模式
    /// "single" = 拉一帧复刻；"batch" = 整段分镜复刻
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringCriteria {
    pub creativity: u32,
    pub technical: u32,
    pub narrative: u32,
    pub aesthetic: u32,
    pub compliance: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    pub id: String,
    pub order_num: u32,
    pub title: String,
    pub description: String,
    pub cover_emoji: String,
    pub estimated_minutes: u32,
    pub reward_tokens: u32,
    pub difficulty: u8,
    pub prerequisites: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_subtitle: Option<String>,
    pub steps: Vec<LevelStep>,
    pub ai_name: String,
    pub ai_avatar: String,
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub scoring_criteria: ScoringCriteria,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelProgress {
    pub level_id: String,
    pub status: LevelStatus,
    pub attempts: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_score: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<i64>,
}
