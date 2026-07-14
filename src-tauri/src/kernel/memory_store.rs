// MemoryStore — SQLite 持久化 backend
//
// 设计:
//   - 单表 (namespace, key, value, updated_at)
//   - namespace 索引, 跨设备同步留给 W13
//   - put 覆盖式 (key 主键 (namespace, key))
//   - append 在 SQL 里做 value = value || '\n' || new (注意并发!)
//
// 关联红线 9: 必须真记 — sqlite WAL mode + 同步 fsync 保证落盘.

use crate::kernel::memory_bus::MemoryBackend;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

/// SQLite 实现的 MemoryBackend. 用于生产 (Day 3-4 起替代 InMemory).
pub struct SqliteMemoryBackend {
    conn: Mutex<Connection>,
}

impl SqliteMemoryBackend {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(e))
            })?;
        }
        let conn = Connection::open(path)?;
        // WAL = 读写并发更友好, 适合后台 sync 不阻塞主写
        conn.pragma_update(None, "journal_mode", "WAL")?;
        // FULL = 写事务 fsync, 落盘保证 (红线 9)
        conn.pragma_update(None, "synchronous", "FULL")?;
        Self::migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn migrate(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS kernel_memory (
                namespace TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (namespace, key)
            );
            CREATE INDEX IF NOT EXISTS idx_kernel_memory_namespace
                ON kernel_memory (namespace);
            "#,
        )
    }
}

impl MemoryBackend for SqliteMemoryBackend {
    fn get(&self, namespace: &str, key: &str) -> Option<String> {
        let conn = self.conn.lock().expect("memory store lock");
        conn.query_row(
            "SELECT value FROM kernel_memory WHERE namespace = ?1 AND key = ?2",
            params![namespace, key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
    }

    fn put(&self, namespace: &str, key: &str, value: &str) {
        let now = chrono_now_ms();
        let conn = self.conn.lock().expect("memory store lock");
        let _ = conn.execute(
            "INSERT INTO kernel_memory (namespace, key, value, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (namespace, key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
            params![namespace, key, value, now],
        );
    }

    fn append(&self, namespace: &str, key: &str, value: &str) {
        let now = chrono_now_ms();
        let conn = self.conn.lock().expect("memory store lock");
        // 用 INSERT...ON CONFLICT + 表达式拼接; 简单且无并发原子性问题
        // (Mutex 串行化写, 不会有 race)
        let existing: Option<String> = conn
            .query_row(
                "SELECT value FROM kernel_memory WHERE namespace = ?1 AND key = ?2",
                params![namespace, key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten();
        let new_value = match existing {
            Some(prev) if !prev.is_empty() => format!("{prev}\n{value}"),
            _ => value.to_string(),
        };
        let _ = conn.execute(
            "INSERT INTO kernel_memory (namespace, key, value, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (namespace, key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
            params![namespace, key, new_value, now],
        );
    }

    fn list(&self, namespace: &str) -> Vec<String> {
        let conn = self.conn.lock().expect("memory store lock");
        let mut stmt = match conn.prepare(
            "SELECT key FROM kernel_memory WHERE namespace = ?1 ORDER BY key",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(params![namespace], |row| row.get::<_, String>(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }
}

fn chrono_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn open_test() -> (tempfile::TempDir, SqliteMemoryBackend) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("memory.sqlite");
        let backend = SqliteMemoryBackend::open(&path).unwrap();
        (dir, backend)
    }

    #[test]
    fn put_get_roundtrip() {
        let (_dir, b) = open_test();
        b.put("user:xiaoyue", "character", "小猫 (橙白)");
        assert_eq!(b.get("user:xiaoyue", "character"), Some("小猫 (橙白)".into()));
    }

    #[test]
    fn put_overwrites() {
        let (_dir, b) = open_test();
        b.put("n", "k", "v1");
        b.put("n", "k", "v2");
        assert_eq!(b.get("n", "k"), Some("v2".into()));
    }

    #[test]
    fn append_on_empty_creates() {
        let (_dir, b) = open_test();
        b.append("user:qinfeng", "neon_deer_progress", "step1");
        assert_eq!(
            b.get("user:qinfeng", "neon_deer_progress"),
            Some("step1".into())
        );
    }

    #[test]
    fn append_concatenates() {
        let (_dir, b) = open_test();
        b.append("n", "k", "a");
        b.append("n", "k", "b");
        b.append("n", "k", "c");
        assert_eq!(b.get("n", "k"), Some("a\nb\nc".into()));
    }

    #[test]
    fn namespace_isolation() {
        let (_dir, b) = open_test();
        b.put("user:mother", "project_count", "30");
        b.put("user:daughter", "project_count", "5");
        assert_eq!(b.get("user:mother", "project_count"), Some("30".into()));
        assert_eq!(b.get("user:daughter", "project_count"), Some("5".into()));
        assert_eq!(b.get("user:other", "project_count"), None);
    }

    #[test]
    fn list_keys_in_namespace() {
        let (_dir, b) = open_test();
        b.put("ns", "k1", "v1");
        b.put("ns", "k2", "v2");
        b.put("other", "k3", "v3");
        let mut keys = b.list("ns");
        keys.sort();
        assert_eq!(keys, vec!["k1", "k2"]);
    }

    #[test]
    fn get_missing_returns_none() {
        let (_dir, b) = open_test();
        assert_eq!(b.get("nowhere", "nothing"), None);
    }

    #[test]
    fn persist_across_reopen() {
        // 红线 9: 关闭 + 重开必须能读到
        let dir = tempdir().unwrap();
        let path = dir.path().join("memory.sqlite");
        {
            let b = SqliteMemoryBackend::open(&path).unwrap();
            b.put("user:xiaoyue", "last_project", "小猫 1");
        }
        let b2 = SqliteMemoryBackend::open(&path).unwrap();
        assert_eq!(
            b2.get("user:xiaoyue", "last_project"),
            Some("小猫 1".into())
        );
    }
}