// W11 Day 8 — Telemetry (按 user_mode 分桶上报)
//
// 调用方 (agent / skill install / mode switch / secret update):
//   telemetry::report(TelemetryEvent::AgentRun { ... }).await;
//
// 隐私分层 (Part C7):
//   - Child mode (default): 含 input_hash / output_hash (用于训练 prompt 改进)
//   - Adult mode (default opt-out): 仅 metadata (kind / latency / outcome)
//                       用户可在设置页 "不上报任何数据" → 全部走 noop
//
// 数据上报走 marketplace_client.post_json → POST /api/v1/telemetry.
// 失败仅 eprintln, 不阻塞主流程.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::license_store::UserMode;
use crate::marketplace_client::MarketplaceClient;

/// 全局 telemetry 开关 (用户设置页可关). 默认 ON (即 "上报").
static TELEMETRY_ENABLED: AtomicBool = AtomicBool::new(true);

/// 全局 current mode (telemetry 自己的缓存; agent.rs set_user_mode 时调 set_mode).
static CURRENT_MODE: std::sync::Mutex<Option<UserMode>> = std::sync::Mutex::new(None);

pub fn set_opt_out(opt_out: bool) {
    TELEMETRY_ENABLED.store(!opt_out, Ordering::SeqCst);
}

pub fn is_enabled() -> bool {
    TELEMETRY_ENABLED.load(Ordering::SeqCst)
}

pub fn set_mode(mode: UserMode) {
    if let Ok(mut g) = CURRENT_MODE.lock() {
        *g = Some(mode);
    }
}

fn current_mode() -> UserMode {
    CURRENT_MODE
        .lock()
        .ok()
        .and_then(|g| *g)
        .unwrap_or(UserMode::Child)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TelemetryEvent {
    AgentRun {
        call_id: String,
        level_id: String,
        /// agent 调用的种类 (例 "director" / "image_gen" / "video_gen")
        agent_kind: String,
        outcome: String, // "ok" | "err"
        latency_ms: u64,
        /// 仅 Child mode 上报; Adult mode 永远 None
        input_hash: Option<String>,
        /// 仅 Child mode 上报
        output_hash: Option<String>,
        satisfaction_signal: Option<String>, // "implicit:video_generated" / "explicit:thumb_up"
        secret_version: Option<String>,
        skill_versions: Option<std::collections::HashMap<String, String>>,
    },
    ModeSwitch {
        from_mode: String,
        to_mode: String,
        success: bool,
    },
    SkillInstall {
        skill_id: String,
        skill_version: String,
        audience: String,
        success: bool,
    },
    SecretUpdate {
        profile: String,
        from_version: Option<String>,
        to_version: String,
        success: bool,
    },
    AntiDebugTrigger {
        tag: String,
        count: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEnvelope {
    pub mode: String,
    pub opted_out: bool,
    pub device_id: Option<String>,
    pub ts_ms: i64,
    pub event: TelemetryEvent,
}

/// 把 event 包成 envelope; Adult mode 强制脱敏 input/output hash.
pub fn wrap(event: TelemetryEvent, device_id: Option<String>) -> TelemetryEnvelope {
    let mode = current_mode();
    let mut event = event;

    // Adult mode → 强制把 input/output hash 拿掉 (隐私)
    if mode == UserMode::Adult {
        if let TelemetryEvent::AgentRun {
            input_hash,
            output_hash,
            ..
        } = &mut event
        {
            *input_hash = None;
            *output_hash = None;
        }
    }

    TelemetryEnvelope {
        mode: match mode {
            UserMode::Child => "child".to_string(),
            UserMode::Adult => "adult".to_string(),
        },
        opted_out: !is_enabled(),
        device_id,
        ts_ms: now_millis(),
        event,
    }
}

/// 上报: 走 marketplace_client POST /api/v1/telemetry.
/// 失败仅 eprintln, 不返回错误 (异步 fire-and-forget).
pub async fn report(client: &MarketplaceClient, event: TelemetryEvent, device_id: Option<String>) {
    if !is_enabled() {
        return; // opt-out → noop
    }
    let envelope = wrap(event, device_id);
    let envelope_for_blocking = envelope.clone();
    let client_clone = client.clone();
    tauri::async_runtime::spawn(async move {
        match client_clone
            .post_json::<TelemetryEnvelope, serde_json::Value>(
                "/api/v1/telemetry",
                &envelope_for_blocking,
            )
            .await
        {
            Ok(_) => {
                crate::crashlog::event(
                    "telemetry",
                    &format!("ok kind={:?}", envelope.event.event_kind_debug()),
                );
            }
            Err(e) => {
                crate::crashlog::event("telemetry_err", &format!("{e}"));
            }
        }
    });
}

impl TelemetryEvent {
    /// 仅用于日志, 不暴露敏感信息.
    pub fn event_kind_debug(&self) -> &'static str {
        match self {
            TelemetryEvent::AgentRun { .. } => "agent_run",
            TelemetryEvent::ModeSwitch { .. } => "mode_switch",
            TelemetryEvent::SkillInstall { .. } => "skill_install",
            TelemetryEvent::SecretUpdate { .. } => "secret_update",
            TelemetryEvent::AntiDebugTrigger { .. } => "anti_debug",
        }
    }
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn wrap_child_mode_keeps_hashes() {
        set_mode(UserMode::Child);
        set_opt_out(false);
        let env = wrap(
            TelemetryEvent::AgentRun {
                call_id: "c1".into(),
                level_id: "L1".into(),
                agent_kind: "director".into(),
                outcome: "ok".into(),
                latency_ms: 100,
                input_hash: Some("abc".into()),
                output_hash: Some("def".into()),
                satisfaction_signal: None,
                secret_version: Some("v1.x".into()),
                skill_versions: Some(HashMap::new()),
            },
            Some("dev-1".into()),
        );
        assert_eq!(env.mode, "child");
        assert!(!env.opted_out);
        if let TelemetryEvent::AgentRun {
            input_hash,
            output_hash,
            ..
        } = env.event
        {
            assert_eq!(input_hash.as_deref(), Some("abc"));
            assert_eq!(output_hash.as_deref(), Some("def"));
        } else {
            panic!("expected AgentRun");
        }
    }

    #[test]
    fn wrap_adult_mode_strips_hashes() {
        set_mode(UserMode::Adult);
        let env = wrap(
            TelemetryEvent::AgentRun {
                call_id: "c1".into(),
                level_id: "L1".into(),
                agent_kind: "director".into(),
                outcome: "ok".into(),
                latency_ms: 100,
                input_hash: Some("abc".into()),
                output_hash: Some("def".into()),
                satisfaction_signal: None,
                secret_version: None,
                skill_versions: None,
            },
            Some("dev-1".into()),
        );
        assert_eq!(env.mode, "adult");
        if let TelemetryEvent::AgentRun {
            input_hash,
            output_hash,
            ..
        } = env.event
        {
            assert!(input_hash.is_none(), "adult mode 应脱敏 input_hash");
            assert!(output_hash.is_none(), "adult mode 应脱敏 output_hash");
        } else {
            panic!("expected AgentRun");
        }
        // 恢复默认, 避免污染其它测试
        set_mode(UserMode::Child);
    }

    #[test]
    fn opt_out_disables_reporting() {
        set_opt_out(true);
        assert!(!is_enabled());
        // 恢复
        set_opt_out(false);
        assert!(is_enabled());
    }

    #[test]
    fn event_kind_debug_does_not_leak_sensitive() {
        let e = TelemetryEvent::AgentRun {
            call_id: "x".into(),
            level_id: "L1".into(),
            agent_kind: "director".into(),
            outcome: "ok".into(),
            latency_ms: 50,
            input_hash: None,
            output_hash: None,
            satisfaction_signal: None,
            secret_version: None,
            skill_versions: None,
        };
        let k = e.event_kind_debug();
        assert_eq!(k, "agent_run");
        assert!(!k.contains("director")); // kind 字段不进日志
    }

    #[test]
    fn mode_switch_event_serializes() {
        let e = TelemetryEvent::ModeSwitch {
            from_mode: "child".into(),
            to_mode: "adult".into(),
            success: true,
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(s.contains("mode_switch"));
        assert!(s.contains("from_mode"));
    }

    #[test]
    fn envelope_serializes_correctly() {
        set_mode(UserMode::Child);
        let env = wrap(
            TelemetryEvent::AntiDebugTrigger {
                tag: "startup".into(),
                count: 1,
            },
            None,
        );
        let s = serde_json::to_string(&env).unwrap();
        assert!(s.contains("\"mode\":\"child\""));
        assert!(s.contains("anti_debug_trigger"));
    }
}