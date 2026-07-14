// EventBus — 内核事件广播
//
// 设计原则:
//   - 异步广播 (tokio::sync::broadcast), 多订阅者, 不阻塞发布者
//   - 事件类型严格枚举, 不接受任意 payload (type-safe)
//   - 内核 + skill + shell 都可订阅, 也都可发布
//   - 订阅者掉线不阻塞其他订阅者 (Lagged: skip old events)
//
// 4-persona 红线:
//   - agent.idle_threshold → 触发 pet.mood_changed (小月失联召回)
//   - mode.changed → 触发 skill.mounted/unmounted (skill 跨模式过滤)
//   - memory.updated → 触发 shell 主动问候 ("上次你做的小猫")

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

/// 内核事件枚举 (单一事实源).
///
/// 任何事件扩展必须先在这里加 variant, 避免运行时字符串分发.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum KernelEvent {
    /// 用户输入新消息
    UserMessage { text: String, sender: String },
    /// Agent 思考中
    AgentThinking { session_id: String },
    /// Agent 调用 tool
    AgentToolCall { tool_name: String, args_summary: String },
    /// 资产创建
    AssetCreated { asset_id: String, kind: String, url: String },
    /// 项目保存
    ProjectSaved { project_id: String, cursor: i32 },
    /// 模式切换 (W11 ModeBus 已有, 内核再广播一次)
    ModeChanged { from: String, to: String },
    /// Skill 挂载 / 卸载
    SkillMounted { skill_id: String },
    SkillUnmounted { skill_id: String },
    /// 宠物情绪变化
    PetMoodChanged { from: String, to: String, reason: String },
    /// 记忆读写
    MemoryUpdated { namespace: String, key: String },
    /// 关键: 用户空闲 N 秒 (用于失联召回, 小月 P0)
    IdleThreshold { seconds_idle: u64 },
    /// 关键: 阶段确认 (红线 7: Director 每步必须等用户确认)
    /// 由 video-director skill 在每步 UI 渲染时发, 内核不替用户决策
    StageAwaitingConfirm { skill_id: String, stage: String },
}

impl KernelEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::UserMessage { .. } => "user.message",
            Self::AgentThinking { .. } => "agent.thinking",
            Self::AgentToolCall { .. } => "agent.tool_call",
            Self::AssetCreated { .. } => "asset.created",
            Self::ProjectSaved { .. } => "project.saved",
            Self::ModeChanged { .. } => "mode.changed",
            Self::SkillMounted { .. } => "skill.mounted",
            Self::SkillUnmounted { .. } => "skill.unmounted",
            Self::PetMoodChanged { .. } => "pet.mood_changed",
            Self::MemoryUpdated { .. } => "memory.updated",
            Self::IdleThreshold { .. } => "agent.idle_threshold",
            Self::StageAwaitingConfirm { .. } => "stage.awaiting_confirm",
        }
    }
}

/// 事件总线 — Arc 共享, 多发布者多订阅者.
#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<Arc<KernelEvent>>,
}

impl EventBus {
    /// 创建新总线. capacity = 256 事件 buffer, 足够 30 分钟 agent 操作.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(256);
        Self { tx }
    }

    /// 广播事件. 订阅者掉线 (Lagged) 不影响发布者.
    pub fn publish(&self, event: KernelEvent) {
        // 0 个订阅者时 silently drop, 不报错
        let _ = self.tx.send(Arc::new(event));
    }

    /// 订阅. 订阅者负责处理 Lagged 错误 (落后太多时拿不到早期事件).
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<KernelEvent>> {
        self.tx.subscribe()
    }

    /// 当前订阅者数 (调试 / 健康检查用).
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn publish_subscribe_roundtrip() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.publish(KernelEvent::UserMessage {
            text: "hello".into(),
            sender: "xiaoyue".into(),
        });

        let event = rx.recv().await.unwrap();
        match &*event {
            KernelEvent::UserMessage { text, sender } => {
                assert_eq!(text, "hello");
                assert_eq!(sender, "xiaoyue");
            }
            _ => panic!("unexpected event"),
        }
    }

    #[tokio::test]
    async fn multi_subscriber_broadcast() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        bus.publish(KernelEvent::PetMoodChanged {
            from: "happy".into(),
            to: "sleepy".into(),
            reason: "7d_idle".into(),
        });

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();
        assert!(matches!(*e1, KernelEvent::PetMoodChanged { .. }));
        assert!(matches!(*e2, KernelEvent::PetMoodChanged { .. }));
    }

    #[tokio::test]
    async fn no_subscriber_publish_does_not_panic() {
        let bus = EventBus::new();
        // 无订阅者时 publish 应 silently drop, 不 panic
        bus.publish(KernelEvent::UserMessage {
            text: "orphan".into(),
            sender: "x".into(),
        });
        assert_eq!(bus.subscriber_count(), 0);
    }

    #[tokio::test]
    async fn idle_threshold_event_kind() {
        // 红线 9 关联: IdleThreshold 用于失联召回
        let ev = KernelEvent::IdleThreshold { seconds_idle: 604_800 };
        assert_eq!(ev.kind(), "agent.idle_threshold");
    }

    #[tokio::test]
    async fn stage_awaiting_confirm_event_kind() {
        // 红线 7: 内核不替用户决策, skill 自己发事件表示"等确认"
        let ev = KernelEvent::StageAwaitingConfirm {
            skill_id: "video-director".into(),
            stage: "stage-4".into(),
        };
        assert_eq!(ev.kind(), "stage.awaiting_confirm");
    }

    #[test]
    fn event_kind_table() {
        // 所有变体 kind 字符串稳定 (前端订阅按字符串)
        let cases: Vec<(KernelEvent, &str)> = vec![
            (
                KernelEvent::AgentThinking { session_id: "s".into() },
                "agent.thinking",
            ),
            (
                KernelEvent::ModeChanged {
                    from: "child".into(),
                    to: "adult".into(),
                },
                "mode.changed",
            ),
            (
                KernelEvent::SkillMounted {
                    skill_id: "video-director".into(),
                },
                "skill.mounted",
            ),
            (
                KernelEvent::MemoryUpdated {
                    namespace: "user".into(),
                    key: "nickname".into(),
                },
                "memory.updated",
            ),
        ];
        for (ev, expected) in cases {
            assert_eq!(ev.kind(), expected);
        }
    }
}