// MiniMax (及任意 OpenAI 兼容 provider) key 池 + 失败转移
//
// 设计目标（最简版）：
//   - 多 key 逗号分隔，trim 空白
//   - round-robin 取下一个 healthy key
//   - 仅 401/429 触发失败转移（其他错误立即返回，不切 key）
//   - 失败 key 在进程生命周期内永久标记，不自动复活（重启即恢复）
//   - 日志只打印前缀 8 字符，绝不漏全 key
//
// 不做（留底）：
//   - per-key 限流计数
//   - 自动复活 / 定时健康检查
//   - SEEDANCE key 池化

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct KeyPool {
    keys: Vec<Arc<Mutex<KeyState>>>,
    cursor: Arc<AtomicUsize>,
}

struct KeyState {
    key: String,
    healthy: bool,
}

impl KeyPool {
    /// 逗号分隔多 key，trim + 过滤空白；至少 1 个非空才返回 Some
    #[allow(clippy::should_implement_trait)] // 与 std::str::FromStr 签名不同：返回 Option 而非 Result
    pub fn from_str(raw: &str) -> Option<Self> {
        let keys: Vec<String> = raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        if keys.is_empty() {
            return None;
        }
        Some(Self {
            keys: keys
                .into_iter()
                .map(|k| {
                    Arc::new(Mutex::new(KeyState {
                        key: k,
                        healthy: true,
                    }))
                })
                .collect(),
            cursor: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// 优先读池变量，回退单 key 变量
    pub fn from_env(pool_var: &str, single_var: &str) -> Option<Self> {
        if let Ok(raw) = std::env::var(pool_var) {
            if let Some(pool) = Self::from_str(&raw) {
                return Some(pool);
            }
        }
        if let Ok(key) = std::env::var(single_var) {
            let trimmed = key.trim();
            if !trimmed.is_empty() {
                return Self::from_str(trimmed);
            }
        }
        None
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// 返回下一个 healthy key 的 (idx, key_str)
    /// round-robin：从 cursor 起扫描，命中 healthy 返回；否则 +1 继续
    /// 全挂返回 None
    pub fn next_healthy(&self) -> Option<(usize, String)> {
        let n = self.keys.len();
        if n == 0 {
            return None;
        }
        let start = self.cursor.load(Ordering::Relaxed);
        for offset in 0..n {
            let idx = (start + offset) % n;
            let state = self.keys[idx].lock().expect("KeyState mutex poisoned");
            if state.healthy {
                self.cursor.store((idx + 1) % n, Ordering::Relaxed);
                return Some((idx, state.key.clone()));
            }
        }
        None
    }

    /// 标记 key 失败，进程内永久生效；打印前缀日志
    pub fn mark_failed(&self, idx: usize) {
        if idx >= self.keys.len() {
            return;
        }
        let mut state = self.keys[idx].lock().expect("KeyState mutex poisoned");
        if !state.healthy {
            return;
        }
        state.healthy = false;
        eprintln!(
            "[key_pool] key[{}] (prefix={}) marked failed; will not retry until restart",
            idx,
            key_prefix(&state.key)
        );
    }
}

/// 日志安全前缀：前 8 字符；短 key 不 panic
pub fn key_prefix(key: &str) -> &str {
    let len = key.len().min(8);
    &key[..len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_parses_comma_separated_with_whitespace() {
        let pool = KeyPool::from_str(" a , b , c ").expect("non-empty");
        assert_eq!(pool.len(), 3);
        let keys: Vec<String> = (0..3)
            .map(|_| pool.next_healthy().unwrap().1)
            .collect();
        // round-robin 顺序不固定（Mutex 顺序保证），但集合正确
        assert!(keys.contains(&"a".to_string()));
        assert!(keys.contains(&"b".to_string()));
        assert!(keys.contains(&"c".to_string()));
    }

    #[test]
    fn from_str_filters_empty_segments() {
        let pool = KeyPool::from_str("a,,,b,  ,c").expect("non-empty");
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn from_str_returns_none_for_empty_or_whitespace() {
        assert!(KeyPool::from_str("").is_none());
        assert!(KeyPool::from_str("   ").is_none());
        assert!(KeyPool::from_str(",,,").is_none());
    }

    #[test]
    fn next_healthy_round_robins() {
        let pool = KeyPool::from_str("a,b,c").expect("non-empty");
        let seq: Vec<String> = (0..6).map(|_| pool.next_healthy().unwrap().1).collect();
        assert_eq!(seq, vec!["a", "b", "c", "a", "b", "c"]);
    }

    #[test]
    fn mark_failed_skips_then_cycles_back() {
        let pool = KeyPool::from_str("a,b,c").expect("non-empty");
        // a 失败
        let _ = pool.next_healthy().unwrap(); // a, cursor -> 1
        pool.mark_failed(0);
        let seq: Vec<String> = (0..4).map(|_| pool.next_healthy().unwrap().1).collect();
        // a 失败 -> b, c, b, c（不再回 a）
        assert_eq!(seq, vec!["b", "c", "b", "c"]);
    }

    #[test]
    fn all_failed_returns_none() {
        let pool = KeyPool::from_str("a,b").expect("non-empty");
        pool.mark_failed(0);
        pool.mark_failed(1);
        assert!(pool.next_healthy().is_none());
    }

    #[test]
    fn mark_failed_idempotent() {
        let pool = KeyPool::from_str("a,b").expect("non-empty");
        pool.mark_failed(0);
        pool.mark_failed(0); // 不应 panic / 不应重复打印（healthy 已 false）
        assert!(pool.next_healthy().unwrap().1 == "b");
    }

    #[test]
    fn key_prefix_handles_short_keys() {
        assert_eq!(key_prefix("sk-cp-abcdefghij"), "sk-cp-ab");
        assert_eq!(key_prefix("abc"), "abc"); // < 8 不 panic
        assert_eq!(key_prefix(""), "");
    }

    #[test]
    fn from_env_prefers_pool_var() {
        let lock = std::sync::Mutex::new(());
        let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var("TEST_POOL", "x,y");
            std::env::set_var("TEST_SINGLE", "z");
        }
        let pool = KeyPool::from_env("TEST_POOL", "TEST_SINGLE").expect("non-empty");
        assert_eq!(pool.len(), 2);
        unsafe {
            std::env::remove_var("TEST_POOL");
            std::env::remove_var("TEST_SINGLE");
        }
    }

    #[test]
    fn from_env_falls_back_to_single() {
        let lock = std::sync::Mutex::new(());
        let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::remove_var("TEST_POOL_EMPTY");
            std::env::set_var("TEST_SINGLE_ONLY", "solo");
        }
        let pool =
            KeyPool::from_env("TEST_POOL_EMPTY", "TEST_SINGLE_ONLY").expect("non-empty");
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.next_healthy().unwrap().1, "solo");
        unsafe {
            std::env::remove_var("TEST_SINGLE_ONLY");
        }
    }
}