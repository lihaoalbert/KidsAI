// IdentityService — 用户 + 宠物 + parent_id 统一身份
//
// 4-persona 关联:
//   - 小月: nickname="小月", pet_id=huomiao, pet_mood=happy, parent_id="mother:ayan"
//   - 阿岩: 切成人模式后, parent_id 不变, 但 mode=adult (由 ModeBus 管理)
//   - 小墨: nickname="小墨", pet_id=墨石, age_tier=14-16
//   - 秦风: nickname="秦风", pet_id=风铃 (灵感伙伴), mode=adult, age_tier=adult

use crate::kernel::event_bus::{EventBus, KernelEvent};
use crate::kernel::memory_bus::MemoryBus;
use serde::{Deserialize, Serialize};

/// 用户身份. 不存敏感信息 (nickname / pet_id 是公开的).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Identity {
    /// 内部 user_id, e.g. "user:xiaoyue"
    pub user_id: String,
    /// 显示名, e.g. "小月"
    pub nickname: String,
    /// 宠物 ID, 决定默认 pet 形象
    pub pet_id: String,
    /// 宠物当前情绪 (happy / sleepy / thinking)
    pub pet_mood: String,
    /// 上次活跃时间戳 (ms)
    pub last_seen_at: i64,
    /// 年龄段, 由 Onboarding 决定
    pub age_tier: String,
    /// 家长 ID (可选) — 阿岩家共享 plan, 妈妈和女儿 project 隔离
    pub parent_id: Option<String>,
}

impl Identity {
    /// 内存表示 + JSON 反序列化, 但默认成员来自 MemoryBus 读.
    pub fn namespace(&self) -> &str {
        &self.user_id
    }
}

/// IdentityService — 封装 MemoryBus 的 identity 读写, 简化调用.
pub struct IdentityService {
    memory: MemoryBus,
    event_bus: EventBus,
}

impl IdentityService {
    pub fn new(memory: MemoryBus, event_bus: EventBus) -> Self {
        Self { memory, event_bus }
    }

    /// 读取身份. 不存在时返回 None (Onboarding 未做).
    pub fn load(&self, user_id: &str) -> Option<Identity> {
        let json = self.memory.get(user_id, "identity")?;
        serde_json::from_str(&json).ok()
    }

    /// 保存身份. 同时发 PetMoodChanged 事件 (若 pet_mood 变了).
    pub fn save(&self, identity: &Identity) {
        let prev = self.load(&identity.user_id);
        let json = serde_json::to_string(identity).expect("serialize identity");
        self.memory.put(&identity.user_id, "identity", &json);

        // 红线 9 关联: mood 变了要广播, 让 PetEngine / shell 响应
        if let Some(prev) = prev {
            if prev.pet_mood != identity.pet_mood {
                self.event_bus.publish(KernelEvent::PetMoodChanged {
                    from: prev.pet_mood,
                    to: identity.pet_mood.clone(),
                    reason: "identity_save".into(),
                });
            }
        }
    }

    /// 更新宠物情绪 (短 helper).
    pub fn set_pet_mood(&self, user_id: &str, mood: &str) {
        if let Some(mut id) = self.load(user_id) {
            id.pet_mood = mood.to_string();
            self.save(&id);
        }
    }

    /// 更新 last_seen_at (app 启动时调用).
    pub fn bump_last_seen(&self, user_id: &str) {
        if let Some(mut id) = self.load(user_id) {
            id.last_seen_at = now_ms();
            self.memory.put(user_id, "identity", &serde_json::to_string(&id).unwrap());
        }
    }

    /// 给当前 user 算 idle 时长 (秒).
    pub fn idle_seconds(&self, user_id: &str) -> u64 {
        match self.load(user_id) {
            Some(id) => {
                let now = now_ms();
                if now > id.last_seen_at {
                    ((now - id.last_seen_at) / 1000) as u64
                } else {
                    0
                }
            }
            None => 0,
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::event_bus::EventBus;
    use crate::kernel::memory_bus::MemoryBus;

    fn svc() -> (EventBus, MemoryBus, IdentityService) {
        let eb = EventBus::new();
        let mem = MemoryBus::new(
            std::sync::Arc::new(crate::kernel::memory_store::SqliteMemoryBackend::open(
                &std::env::temp_dir()
                    .join(format!("identity-test-{}.sqlite", rand_suffix())),
            ).unwrap()),
            eb.clone(),
        );
        (eb.clone(), mem.clone(), IdentityService::new(mem, eb))
    }

    fn rand_suffix() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{n}")
    }

    fn xiaoyue() -> Identity {
        Identity {
            user_id: "user:xiaoyue".into(),
            nickname: "小月".into(),
            pet_id: "huomiao".into(),
            pet_mood: "happy".into(),
            last_seen_at: now_ms(),
            age_tier: "8-10".into(),
            parent_id: Some("user:ayan".into()),
        }
    }

    #[test]
    fn save_then_load_roundtrip() {
        let (_eb, _mem, svc) = svc();
        let id = xiaoyue();
        svc.save(&id);
        let loaded = svc.load("user:xiaoyue").unwrap();
        assert_eq!(loaded.nickname, "小月");
        assert_eq!(loaded.pet_id, "huomiao");
        assert_eq!(loaded.parent_id.as_deref(), Some("user:ayan"));
    }

    #[test]
    fn save_load_missing_returns_none() {
        let (_eb, _mem, svc) = svc();
        assert!(svc.load("user:ghost").is_none());
    }

    #[test]
    fn pet_mood_change_publishes_event() {
        let (eb, _mem, svc) = svc();
        let mut rx = eb.subscribe();
        let mut id = xiaoyue();
        svc.save(&id);
        // 改 mood, save
        id.pet_mood = "sleepy".into();
        svc.save(&id);
        // 收事件直到找到 PetMoodChanged (中间可能有 MemoryUpdated)
        let mut found = false;
        for _ in 0..4 {
            match rx.try_recv() {
                Ok(event_arc) => {
                    if matches!(*event_arc, KernelEvent::PetMoodChanged { .. }) {
                        found = true;
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        assert!(found, "expected PetMoodChanged event");
    }

    #[test]
    fn parent_id_isolates_projects() {
        // 阿岩红线: 妈妈和女儿通过 parent_id 关联, 但各自 user_id 不同
        let (_eb, _mem, svc) = svc();
        let mother = Identity {
            user_id: "user:ayan".into(),
            nickname: "阿岩".into(),
            pet_id: "none".into(),
            pet_mood: "happy".into(),
            last_seen_at: now_ms(),
            age_tier: "adult".into(),
            parent_id: None,
        };
        svc.save(&mother);
        svc.save(&xiaoyue());
        let m = svc.load("user:ayan").unwrap();
        let d = svc.load("user:xiaoyue").unwrap();
        assert_eq!(m.user_id, "user:ayan");
        assert_eq!(d.user_id, "user:xiaoyue");
        assert_eq!(d.parent_id.as_deref(), Some("user:ayan"));
    }

    #[test]
    fn set_pet_mood_updates_existing() {
        let (_eb, _mem, svc) = svc();
        svc.save(&xiaoyue());
        svc.set_pet_mood("user:xiaoyue", "thinking");
        let loaded = svc.load("user:xiaoyue").unwrap();
        assert_eq!(loaded.pet_mood, "thinking");
    }

    #[test]
    fn bump_last_seen_updates_timestamp() {
        let (_eb, _mem, svc) = svc();
        svc.save(&xiaoyue());
        let before = svc.load("user:xiaoyue").unwrap().last_seen_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        svc.bump_last_seen("user:xiaoyue");
        let after = svc.load("user:xiaoyue").unwrap().last_seen_at;
        assert!(after > before);
    }

    #[test]
    fn idle_seconds_reasonable() {
        let (_eb, _mem, svc) = svc();
        svc.save(&xiaoyue());
        svc.bump_last_seen("user:xiaoyue");
        let idle = svc.idle_seconds("user:xiaoyue");
        assert!(idle < 5, "刚 bump 过的 idle 应该 < 5 秒, 实际 {idle}");
    }
}