// kernel/ipc.rs — Kernel ↔ Shell IPC 边界.
//
// 严格遵循 3 层架构:
//   - Shell (前端) 仅与本模块的 Tauri command 交互
//   - 本模块只翻译请求 + 调 Kernel 内部 API, 不持有任何业务状态
//   - Kernel 内部状态 (PetEngine / IdentityService / EventBus) 一律由 kernel::state 单例持有
//
// 新增 IPC:
//   - pet_tick         — 前端每 60s 调一次, 推 mood / 触发 recall
//   - save_identity    — Onboarding 完成后写 Identity 进 kernel
//   - load_identity    — 启动期前端拉一次 (用于重启后恢复 pet_mood + age_tier)
//   - bump_last_seen   — 前端任意用户动作调一次, 维持 idle 计时
//
// 设计: 所有 PetMoodChanged 事件由 kernel::state 内的后台 task 桥接到 Tauri "kernel://pet_mood" 事件,
//       前端 listen 这个 channel 即可拿到实时 mood (不需要等下一次 tick).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::kernel::pet_engine::{apply_full, PetAction, PetTickInput};
use crate::kernel::state::KernelState;

pub const PET_MOOD_EVENT: &str = "kernel://pet_mood";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetTickRequest {
    pub user_id: String,
    /// 前端是否正在跟 agent 对话 (streaming 中)
    #[serde(default)]
    pub is_in_conversation: bool,
    /// 当前对话已持续的秒数 (前端从 first user message 计时)
    #[serde(default)]
    pub conversation_started_secs_ago: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PetTickResponse {
    /// 没有用户身份 — 前端不应调 pet_tick, 调了不报错
    NoIdentity,
    /// tick 没产生动作
    Noop { current_mood: String },
    /// mood 切换了
    MoodChanged { from: String, to: String, reason: String },
    /// 触发了 recall — 前端应展示召回消息
    Recall { current_mood: String, message: String },
}

#[tauri::command]
pub async fn pet_tick(app: AppHandle, request: PetTickRequest) -> Result<PetTickResponse, String> {
    let state = app.state::<KernelState>();
    let identity_svc = &state.identity_svc;

    let identity = match identity_svc.load(&request.user_id) {
        Some(id) => id,
        None => return Ok(PetTickResponse::NoIdentity),
    };

    let input = PetTickInput {
        user_id: request.user_id.clone(),
        identity: identity.clone(),
        last_user_action_secs_ago: identity_svc.idle_seconds(&request.user_id),
        is_in_conversation: request.is_in_conversation,
        conversation_started_secs_ago: request.conversation_started_secs_ago,
    };

    let prev_mood = identity.pet_mood.clone();
    let action = crate::kernel::pet_engine::PetEngine::tick(&input);

    // 应用 (改 mood / 发 recall 事件). apply_full 不返回结果, 但它会经 IdentityService.save
    // → PetMoodChanged 事件 → 我们后台 task 桥到 Tauri emit.
    apply_full(
        action.clone(),
        &request.user_id,
        identity_svc,
        &state.event_bus,
    );

    let resp = match action {
        PetAction::NoOp => PetTickResponse::Noop {
            current_mood: prev_mood,
        },
        PetAction::SetMood { mood, reason } => PetTickResponse::MoodChanged {
            from: prev_mood,
            to: mood.as_str().to_string(),
            reason: reason.to_string(),
        },
        PetAction::Recall { message } => PetTickResponse::Recall {
            current_mood: prev_mood,
            message,
        },
    };
    Ok(resp)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveIdentityRequest {
    pub user_id: String,
    pub nickname: String,
    pub pet_id: String,
    pub age_tier: String,
    pub parent_id: Option<String>,
}

#[tauri::command]
pub fn save_identity(app: AppHandle, request: SaveIdentityRequest) -> Result<(), String> {
    let state = app.state::<KernelState>();
    let identity = crate::kernel::identity::Identity {
        user_id: request.user_id,
        nickname: request.nickname,
        pet_id: request.pet_id,
        pet_mood: "happy".to_string(),
        last_seen_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
        age_tier: request.age_tier,
        parent_id: request.parent_id,
    };
    state.identity_svc.save(&identity);
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityDto {
    pub user_id: String,
    pub nickname: String,
    pub pet_id: String,
    pub pet_mood: String,
    pub last_seen_at: i64,
    pub age_tier: String,
    pub parent_id: Option<String>,
}

#[tauri::command]
pub fn load_identity(app: AppHandle, user_id: String) -> Result<Option<IdentityDto>, String> {
    let state = app.state::<KernelState>();
    Ok(state.identity_svc.load(&user_id).map(|i| IdentityDto {
        user_id: i.user_id,
        nickname: i.nickname,
        pet_id: i.pet_id,
        pet_mood: i.pet_mood,
        last_seen_at: i.last_seen_at,
        age_tier: i.age_tier,
        parent_id: i.parent_id,
    }))
}

#[tauri::command]
pub fn bump_last_seen(app: AppHandle, user_id: String) -> Result<(), String> {
    let state = app.state::<KernelState>();
    state.identity_svc.bump_last_seen(&user_id);
    Ok(())
}

/// 启动期 (setup 闭包内) 调一次 — 起一个后台 task 把 KernelEvent::PetMoodChanged 桥到 Tauri emit.
/// 用 spawn 而不是 tokio::spawn, 跟 setup 的 sync 上下文一致.
pub fn spawn_pet_mood_bridge(app: AppHandle) {
    use crate::kernel::event_bus::KernelEvent;
    let state = app.state::<KernelState>();
    let mut rx = state.event_bus.subscribe();
    let bridge_app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event_arc) => {
                    if let KernelEvent::PetMoodChanged { from, to, reason } = &*event_arc {
                        let payload = serde_json::json!({
                            "from": from,
                            "to": to,
                            "reason": reason,
                        });
                        if let Err(e) = bridge_app.emit(PET_MOOD_EVENT, payload) {
                            eprintln!("[kernel/ipc] pet_mood bridge emit failed: {e}");
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}