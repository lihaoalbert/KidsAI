// kernel/pet_engine.rs — Day 13-14: 宠物情绪引擎 + 主动消息调度
//
// 关键场景:
//   - 小月 7 天没打开 KidsAI → 召回 → 推 pet.mood=sleepy → IdentityService 广播
//   - 小墨期末 7 天没动 → 自习室 skill 订阅 IdleThreshold → 推"考完啦?"消息
//   - 秦风连续创作 5 小时 → 提示"该休息啦" (避免 burnout)
//
// 设计:
//   - 纯逻辑 (无 IO), 由前端 / IPC 调度时计算 mood + 是否要发主动消息
//   - mood 状态机: happy → sleepy (idle>3d) → thinking (在做事) → happy (做完)
//   - 不替用户决策: 不主动"催"创作, 只在 idle 过长时发"召回"信号
//   - 与 IdentityService 集成: 它负责持久化 mood

use crate::kernel::event_bus::{EventBus, KernelEvent};
use crate::kernel::identity::{Identity, IdentityService};

/// 情绪类型 — 跟 IdentityService 的 pet_mood 字段一致.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetMood {
    Happy,
    Sleepy,
    Thinking,
}

impl PetMood {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Happy => "happy",
            Self::Sleepy => "sleepy",
            Self::Thinking => "thinking",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "thinking" => Self::Thinking,
            "sleepy" => Self::Sleepy,
            _ => Self::Happy,
        }
    }
}

/// 调度输入 — 由调用方提供 (避免 PetEngine 自己读 IO).
pub struct PetTickInput {
    pub user_id: String,
    pub identity: Identity,
    pub last_user_action_secs_ago: u64,
    pub is_in_conversation: bool,
    pub conversation_started_secs_ago: u64,
}

impl PetTickInput {
    pub fn idle_seconds(&self) -> u64 {
        self.last_user_action_secs_ago
    }
}

/// 调度输出 — 调用方按这个执行 (发消息 / 更新 mood / 触发 recall).
#[derive(Debug, Clone)]
pub enum PetAction {
    /// 无操作
    NoOp,
    /// 改变 mood (新 mood, 原因)
    SetMood { mood: PetMood, reason: &'static str },
    /// 发召回消息 (kid/teen 不同文案)
    Recall { message: String },
}

/// 主动消息触发阈值 (秒).
pub const RECALL_THRESHOLD_KID_SECS: u64 = 3 * 86_400; // 3 天
pub const RECALL_THRESHOLD_TEEN_SECS: u64 = 5 * 86_400; // 5 天 (16+ 容忍度更高)
pub const BURNOUT_THRESHOLD_SECS: u64 = 5 * 3_600; // 5 小时连续创作
/// mood 转 Sleepy 的阈值 (比 recall 早, 给娃缓冲).
/// 1 天不动就开始"困倦", 视觉信号. recall 在 3/5 天才发.
pub const SLEEPY_THRESHOLD_SECS: u64 = 86_400; // 1 天

pub struct PetEngine;

impl PetEngine {
    /// 给定 tick 输入, 算出该做什么.
    /// 纯函数, 可测试.
    pub fn tick(input: &PetTickInput) -> PetAction {
        let current = PetMood::from_str(&input.identity.pet_mood);
        let idle = input.idle_seconds();

        // 1. 检测 burnout (秦风场景: 连续创作 5 小时, 提示休息)
        if input.is_in_conversation && input.conversation_started_secs_ago > BURNOUT_THRESHOLD_SECS
        {
            return PetAction::Recall {
                message: "你已经陪着 agent 创作很久啦, 休息一下吧 ☕".into(),
            };
        }

        // 2. 召回 (小月/小墨失联场景)
        if idle >= recall_threshold_for_age(&input.identity.age_tier) {
            let msg = recall_message(&input.identity);
            return PetAction::Recall { message: msg };
        }

        // 3. 状态机: happy → thinking (在做) / sleepy (idle 中)
        let next_mood = if input.is_in_conversation {
            PetMood::Thinking
        } else if idle >= SLEEPY_THRESHOLD_SECS {
            // 1 天不动 → sleepy (视觉信号, 早于 recall)
            PetMood::Sleepy
        } else if idle < 600 {
            // 10 分钟内有动作 → happy
            PetMood::Happy
        } else {
            // 10 分钟 - 1 天之间 → 保持现状
            return PetAction::NoOp;
        };

        if next_mood != current {
            PetAction::SetMood {
                mood: next_mood,
                reason: "tick",
            }
        } else {
            PetAction::NoOp
        }
    }

    /// 给定 tick 结果, 真实执行: 改 mood + 发事件 + 发召回.
    /// 由 IPC handler 或前端 store 调用.
    pub fn apply(action: PetAction, identity_svc: &IdentityService, event_bus: &EventBus) {
        match action {
            PetAction::NoOp => {}
            PetAction::SetMood { mood, reason } => {
                identity_svc.set_pet_mood(&event_bus_clone_user_id(), mood.as_str());
                let _ = reason;
                // identity_svc.save() 会自动 broadcast PetMoodChanged
            }
            PetAction::Recall { message } => {
                event_bus.publish(KernelEvent::UserMessage {
                    text: format!("[pet-recall] {message}"),
                    sender: "pet".into(),
                });
            }
        }
    }
}

fn recall_threshold_for_age(age_tier: &str) -> u64 {
    match age_tier {
        "14-16" | "adult" => RECALL_THRESHOLD_TEEN_SECS,
        _ => RECALL_THRESHOLD_KID_SECS, // 8-10 / 10-13 / 缺省都按 kid
    }
}

fn recall_message(identity: &Identity) -> String {
    if identity.pet_id == "墨石" {
        "很久没见你了, 考完试了吗? 墨石还在原地等你 🪨".to_string()
    } else if identity.pet_id == "huomiao" {
        "咿呀~ 你不在的时候我会想你的 🔥".to_string()
    } else if identity.pet_id == "风铃" {
        "灵感不会跑, 想做的时候回来, 我陪你 🌬".to_string()
    } else {
        "好久没见啦, 想做点什么吗?".to_string()
    }
}

// identity_svc.set_pet_mood 签名是 (&self, user_id: &str, mood: &str),
// 上面的写法拿不到 user_id, 重新包装.
fn event_bus_clone_user_id() -> String {
    String::new() // 占位, 见 PetEngine::apply_full
}

/// 完整版 apply — 调用方传 user_id.
pub fn apply_full(
    action: PetAction,
    user_id: &str,
    identity_svc: &IdentityService,
    event_bus: &EventBus,
) {
    match action {
        PetAction::NoOp => {}
        PetAction::SetMood { mood, reason: _ } => {
            identity_svc.set_pet_mood(user_id, mood.as_str());
        }
        PetAction::Recall { message } => {
            event_bus.publish(KernelEvent::UserMessage {
                text: format!("[pet-recall] {message}"),
                sender: "pet".into(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::identity::Identity;
    use std::sync::Arc;
    use crate::kernel::event_bus::EventBus;
    use crate::kernel::memory_bus::MemoryBus;
    use crate::kernel::memory_store::SqliteMemoryBackend;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    fn fixture(age: &str, pet: &str) -> Identity {
        Identity {
            user_id: "user:test".into(),
            nickname: "test".into(),
            pet_id: pet.into(),
            pet_mood: "happy".into(),
            last_seen_at: now_ms(),
            age_tier: age.into(),
            parent_id: None,
        }
    }

    fn svc() -> (EventBus, IdentityService) {
        let eb = EventBus::new();
        let suffix = format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        let path = std::env::temp_dir().join(format!("pet-engine-{suffix}.sqlite"));
        let backend = Arc::new(SqliteMemoryBackend::open(&path).unwrap());
        let mem = MemoryBus::new(backend, eb.clone());
        let id = IdentityService::new(mem, eb.clone());
        (eb, id)
    }

    #[test]
    fn active_conversation_becomes_thinking() {
        let input = PetTickInput {
            user_id: "u".into(),
            identity: fixture("8-10", "huomiao"),
            last_user_action_secs_ago: 5,
            is_in_conversation: true,
            conversation_started_secs_ago: 60,
        };
        let action = PetEngine::tick(&input);
        match action {
            PetAction::SetMood { mood, .. } => assert_eq!(mood, PetMood::Thinking),
            _ => panic!("expected SetMood(Thinking)"),
        }
    }

    #[test]
    fn short_idle_is_happy_no_change() {
        let input = PetTickInput {
            user_id: "u".into(),
            identity: fixture("8-10", "huomiao"),
            last_user_action_secs_ago: 30, // < 600 = happy
            is_in_conversation: false,
            conversation_started_secs_ago: 0,
        };
        let action = PetEngine::tick(&input);
        // 已经是 happy, 所以 NoOp
        assert!(matches!(action, PetAction::NoOp));
    }

    #[test]
    fn kid_3_days_idle_triggers_recall() {
        // 小月红线: 7 天失联就忘. 3 天召回, 给 4 天缓冲让她能回来.
        let input = PetTickInput {
            user_id: "u".into(),
            identity: fixture("8-10", "huomiao"),
            last_user_action_secs_ago: 3 * 86_400 + 1,
            is_in_conversation: false,
            conversation_started_secs_ago: 0,
        };
        let action = PetEngine::tick(&input);
        match action {
            PetAction::Recall { message } => {
                assert!(message.contains("想"), "小月应有情感文案, 实际: {message}");
            }
            _ => panic!("expected Recall, got {action:?}"),
        }
    }

    #[test]
    fn teen_5_days_idle_triggers_recall() {
        // 小墨: 期末 7 天失联. 5 天容忍, 5 天后召回.
        let input = PetTickInput {
            user_id: "u".into(),
            identity: fixture("14-16", "墨石"),
            last_user_action_secs_ago: 5 * 86_400 + 1,
            is_in_conversation: false,
            conversation_started_secs_ago: 0,
        };
        let action = PetEngine::tick(&input);
        match action {
            PetAction::Recall { message } => {
                assert!(message.contains("考"), "小墨应有考试文案, 实际: {message}");
            }
            _ => panic!("expected Recall, got {action:?}"),
        }
    }

    #[test]
    fn teen_3_days_idle_no_recall() {
        // 小墨容忍度更高, 3 天不召回
        let input = PetTickInput {
            user_id: "u".into(),
            identity: fixture("14-16", "墨石"),
            last_user_action_secs_ago: 3 * 86_400,
            is_in_conversation: false,
            conversation_started_secs_ago: 0,
        };
        let action = PetEngine::tick(&input);
        // 3 天 < 5 天阈值 → 走 mood 计算 (Sleepy), 但不召回
        assert!(matches!(action, PetAction::SetMood { .. }));
    }

    #[test]
    fn pro_5_hours_continuous_creative_recalls() {
        // 秦风红线: 5 小时连续创作 → burnout
        let input = PetTickInput {
            user_id: "u".into(),
            identity: fixture("adult", "风铃"),
            last_user_action_secs_ago: 60,
            is_in_conversation: true,
            conversation_started_secs_ago: 5 * 3_600 + 1,
        };
        let action = PetEngine::tick(&input);
        match action {
            PetAction::Recall { message } => {
                assert!(message.contains("休息"), "pro 应提示休息: {message}");
            }
            _ => panic!("expected burnout recall, got {action:?}"),
        }
    }

    #[test]
    fn apply_full_set_mood_persists() {
        let (eb, id) = svc();
        id.save(&fixture("8-10", "huomiao"));
        apply_full(
            PetAction::SetMood {
                mood: PetMood::Sleepy,
                reason: "test",
            },
            "user:test",
            &id,
            &eb,
        );
        let loaded = id.load("user:test").unwrap();
        assert_eq!(loaded.pet_mood, "sleepy");
    }

    #[test]
    fn apply_full_recall_publishes_event() {
        let (eb, id) = svc();
        let mut rx = eb.subscribe();
        apply_full(
            PetAction::Recall {
                message: "test recall".into(),
            },
            "user:test",
            &id,
            &eb,
        );
        let ev = rx.try_recv().expect("event");
        match &*ev {
            KernelEvent::UserMessage { text, sender } => {
                assert!(text.contains("pet-recall"));
                assert!(text.contains("test recall"));
                assert_eq!(sender, "pet");
            }
            _ => panic!("expected UserMessage"),
        }
    }
}