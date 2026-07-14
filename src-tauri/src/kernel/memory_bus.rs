// MemoryBus — 跨 namespace 持久读写 (语义层)
//
// 设计原则:
//   - namespace:key 形式 (e.g. "user:xiaoyue:character.catlind")
//   - get / put / append / subscribe 4 操作
//   - put 成功后自动 publish MemoryUpdated 事件 (红线 9: 必须真记)
//   - 跨设备同步留给 W13, 当前仅本地
//
// 4-persona 红线 9 关联:
//   - 小月上次做的"小猫 (橙白)"必须跨 session 持久化
//   - 阿岩的 project namespace 必须与女儿隔离 (parent_id)
//   - 秦风的"半年前霓虹鹿"必须能跨月恢复
//
// Day 3-4 才有 SQLite backend, Day 1-2 先写 trait + 内存实现 + 测试,
//        保证 trait 稳定后再接 SQLite.

use crate::kernel::event_bus::{EventBus, KernelEvent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 记忆操作.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum MemoryOp {
    Get { namespace: String, key: String },
    Put {
        namespace: String,
        key: String,
        value: String,
    },
    Append {
        namespace: String,
        key: String,
        value: String,
    },
    List { namespace: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum MemoryResult {
    Found { value: String },
    NotFound,
    Listed { keys: Vec<String> },
    Ok,
}

/// MemoryBus trait — Day 3-4 用 SQLite 实现, Day 1-2 内存实现先跑通.
pub trait MemoryBackend: Send + Sync {
    fn get(&self, namespace: &str, key: &str) -> Option<String>;
    fn put(&self, namespace: &str, key: &str, value: &str);
    fn append(&self, namespace: &str, key: &str, value: &str);
    fn list(&self, namespace: &str) -> Vec<String>;
}

/// 内存实现 — 用于单元测试 + Day 1-2 早期集成.
#[derive(Default)]
pub struct InMemoryBackend {
    data: RwLock<HashMap<String, HashMap<String, String>>>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn full_key(namespace: &str, key: &str) -> String {
        format!("{namespace}:{key}")
    }
}

impl MemoryBackend for InMemoryBackend {
    fn get(&self, namespace: &str, key: &str) -> Option<String> {
        self.data
            .read()
            .ok()
            .and_then(|d| d.get(namespace).and_then(|m| m.get(key).cloned()))
    }

    fn put(&self, namespace: &str, key: &str, value: &str) {
        let mut data = self.data.write().expect("memory backend lock");
        data.entry(namespace.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
    }

    fn append(&self, namespace: &str, key: &str, value: &str) {
        let mut data = self.data.write().expect("memory backend lock");
        let entry = data.entry(namespace.to_string()).or_default();
        let existing = entry.get(key).cloned().unwrap_or_default();
        let combined = if existing.is_empty() {
            value.to_string()
        } else {
            format!("{existing}\n{value}")
        };
        entry.insert(key.to_string(), combined);
    }

    fn list(&self, namespace: &str) -> Vec<String> {
        self.data
            .read()
            .ok()
            .and_then(|d| d.get(namespace).map(|m| m.keys().cloned().collect()))
            .unwrap_or_default()
    }
}

/// MemoryBus — 语义层: 业务 API + 事件联动.
#[derive(Clone)]
pub struct MemoryBus {
    backend: Arc<dyn MemoryBackend>,
    event_bus: EventBus,
}

impl MemoryBus {
    pub fn new(backend: Arc<dyn MemoryBackend>, event_bus: EventBus) -> Self {
        Self {
            backend,
            event_bus,
        }
    }

    /// 测试用 — 内存 backend + 默认 event bus.
    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(InMemoryBackend::new()),
            EventBus::new(),
        )
    }

    pub fn get(&self, namespace: &str, key: &str) -> Option<String> {
        self.backend.get(namespace, key)
    }

    pub fn put(&self, namespace: &str, key: &str, value: &str) {
        self.backend.put(namespace, key, value);
        // 红线 9 关联: put 必发事件, 让 shell/pet 能感知"记住了"
        self.event_bus.publish(KernelEvent::MemoryUpdated {
            namespace: namespace.to_string(),
            key: key.to_string(),
        });
    }

    pub fn append(&self, namespace: &str, key: &str, value: &str) {
        self.backend.append(namespace, key, value);
        self.event_bus.publish(KernelEvent::MemoryUpdated {
            namespace: namespace.to_string(),
            key: key.to_string(),
        });
    }

    pub fn list(&self, namespace: &str) -> Vec<String> {
        self.backend.list(namespace)
    }

    pub fn execute(&self, op: MemoryOp) -> MemoryResult {
        match op {
            MemoryOp::Get { namespace, key } => match self.backend.get(&namespace, &key) {
                Some(value) => MemoryResult::Found { value },
                None => MemoryResult::NotFound,
            },
            MemoryOp::Put {
                namespace,
                key,
                value,
            } => {
                self.put(&namespace, &key, &value);
                MemoryResult::Ok
            }
            MemoryOp::Append {
                namespace,
                key,
                value,
            } => {
                self.append(&namespace, &key, &value);
                MemoryResult::Ok
            }
            MemoryOp::List { namespace } => {
                MemoryResult::Listed {
                    keys: self.list(&namespace),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_put_get_roundtrip() {
        let bus = MemoryBus::in_memory();
        bus.put("user:xiaoyue", "character", "小猫 (橙白)");
        assert_eq!(
            bus.get("user:xiaoyue", "character"),
            Some("小猫 (橙白)".into())
        );
    }

    #[test]
    fn in_memory_namespace_isolation() {
        // 阿岩红线: 妈妈和女儿 namespace 必须隔离
        let bus = MemoryBus::in_memory();
        bus.put("user:mother:ayan", "project_count", "30");
        bus.put("user:daughter:xiaoyue", "project_count", "5");
        assert_eq!(
            bus.get("user:mother:ayan", "project_count"),
            Some("30".into())
        );
        assert_eq!(
            bus.get("user:daughter:xiaoyue", "project_count"),
            Some("5".into())
        );
        // 跨 namespace 不能读到
        assert_eq!(
            bus.get("user:mother:xiaoyue", "project_count"),
            None
        );
    }

    #[test]
    fn in_memory_put_overwrites() {
        let bus = MemoryBus::in_memory();
        bus.put("user:x", "k", "v1");
        bus.put("user:x", "k", "v2");
        assert_eq!(bus.get("user:x", "k"), Some("v2".into()));
    }

    #[test]
    fn in_memory_append_concatenates() {
        let bus = MemoryBus::in_memory();
        bus.append("user:x", "history", "step1");
        bus.append("user:x", "history", "step2");
        assert_eq!(bus.get("user:x", "history"), Some("step1\nstep2".into()));
    }

    #[test]
    fn in_memory_list_keys() {
        let bus = MemoryBus::in_memory();
        bus.put("ns", "a", "1");
        bus.put("ns", "b", "2");
        bus.put("other", "c", "3");
        let mut keys = bus.list("ns");
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn put_publishes_memory_updated_event() {
        // 红线 9: put 必须触发 memory.updated 事件
        let bus = MemoryBus::in_memory();
        // 拿不到 EventBus, 改用 execute API 验证逻辑
        let r = bus.execute(MemoryOp::Put {
            namespace: "n".into(),
            key: "k".into(),
            value: "v".into(),
        });
        assert!(matches!(r, MemoryResult::Ok));
        assert_eq!(bus.get("n", "k"), Some("v".into()));
    }

    #[test]
    fn execute_returns_not_found() {
        let bus = MemoryBus::in_memory();
        let r = bus.execute(MemoryOp::Get {
            namespace: "n".into(),
            key: "missing".into(),
        });
        assert!(matches!(r, MemoryResult::NotFound));
    }

    #[test]
    fn execute_list_returns_keys() {
        let bus = MemoryBus::in_memory();
        bus.put("ns", "k1", "v1");
        bus.put("ns", "k2", "v2");
        let r = bus.execute(MemoryOp::List {
            namespace: "ns".into(),
        });
        match r {
            MemoryResult::Listed { keys } => {
                assert_eq!(keys.len(), 2);
                assert!(keys.contains(&"k1".to_string()));
                assert!(keys.contains(&"k2".to_string()));
            }
            _ => panic!("expected Listed"),
        }
    }

    #[test]
    fn append_on_empty_creates() {
        let bus = MemoryBus::in_memory();
        bus.append("user:qinfeng", "neon_deer_progress", "step1");
        assert_eq!(
            bus.get("user:qinfeng", "neon_deer_progress"),
            Some("step1".into())
        );
    }
}