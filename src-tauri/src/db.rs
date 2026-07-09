// 本地 SQLite 持久化（W2.3）
// - 关卡进度：start_level / submit_level 落盘
// - 作品：用户输入 + Agent 输出 + 生成资产
//
// 设计要点：
// 1. 单例 Connection（Mutex<Connection>），MVP 单用户不涉及并发写
// 2. 启动时自动 migrate（幂等 CREATE TABLE IF NOT EXISTS）
// 3. 时间戳用 INTEGER 存毫秒

use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

use crate::types::*;

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        // 启用外键（虽然 MVP 没外键，但好习惯）
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Self::migrate(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn migrate(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS level_progress (
                level_id      TEXT PRIMARY KEY,
                status        TEXT NOT NULL,
                attempts      INTEGER NOT NULL DEFAULT 0,
                best_score    INTEGER,
                completed_at  INTEGER
            );

            CREATE TABLE IF NOT EXISTS creations (
                id            TEXT PRIMARY KEY,
                level_id      TEXT NOT NULL,
                user_input    TEXT NOT NULL,
                agent_output  TEXT NOT NULL,
                score         INTEGER,
                rubric        TEXT,
                feedback      TEXT,
                created_at    INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS assets (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                creation_id   TEXT NOT NULL,
                kind          TEXT NOT NULL,   -- image | video | audio
                url           TEXT NOT NULL,
                thumbnail_url TEXT,
                prompt        TEXT NOT NULL,
                tool          TEXT NOT NULL,
                tokens_cost   INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_creations_level ON creations(level_id);
            CREATE INDEX IF NOT EXISTS idx_assets_creation ON assets(creation_id);
            "#,
        )?;
        Ok(())
    }

    // ============ 关卡进度 ============

    pub fn upsert_progress_in_progress(&self, level_id: &str) -> rusqlite::Result<LevelProgress> {
        let conn = self.conn.lock().unwrap();
        // 原子 UPSERT：attempts +1
        conn.execute(
            r#"
            INSERT INTO level_progress (level_id, status, attempts)
            VALUES (?1, 'in_progress', 1)
            ON CONFLICT(level_id) DO UPDATE SET
                status = 'in_progress',
                attempts = attempts + 1
            "#,
            params![level_id],
        )?;
        Self::read_progress(&conn, level_id)
    }

    pub fn mark_completed(
        &self,
        level_id: &str,
        score: u32,
    ) -> rusqlite::Result<LevelProgress> {
        let conn = self.conn.lock().unwrap();
        let now = now_millis();
        conn.execute(
            r#"
            UPDATE level_progress
            SET status = 'completed',
                best_score = CASE
                    WHEN best_score IS NULL OR best_score < ?2 THEN ?2
                    ELSE best_score
                END,
                completed_at = ?3
            WHERE level_id = ?1
            "#,
            params![level_id, score, now],
        )?;
        Self::read_progress(&conn, level_id)
    }

    pub fn list_progress(&self) -> rusqlite::Result<Vec<LevelProgress>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT level_id, status, attempts, best_score, completed_at
             FROM level_progress
             ORDER BY level_id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(LevelProgress {
                level_id: row.get(0)?,
                status: parse_status(row.get::<_, String>(1)?.as_str()),
                attempts: row.get::<_, u32>(2)?,
                best_score: row.get::<_, Option<u32>>(3)?,
                completed_at: row.get::<_, Option<i64>>(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn list_completed_ids(&self) -> rusqlite::Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT level_id FROM level_progress WHERE status = 'completed'")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect()
    }

    fn read_progress(conn: &Connection, level_id: &str) -> rusqlite::Result<LevelProgress> {
        conn.query_row(
            "SELECT level_id, status, attempts, best_score, completed_at
             FROM level_progress WHERE level_id = ?1",
            params![level_id],
            |row| {
                Ok(LevelProgress {
                    level_id: row.get(0)?,
                    status: parse_status(row.get::<_, String>(1)?.as_str()),
                    attempts: row.get::<_, u32>(2)?,
                    best_score: row.get::<_, Option<u32>>(3)?,
                    completed_at: row.get::<_, Option<i64>>(4)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows)
    }

    // ============ 作品 ============

    pub fn insert_creation(
        &self,
        creation_id: &str,
        level_id: &str,
        user_input: &str,
        agent_output_json: &str,
        score: Option<u32>,
        rubric_json: Option<&str>,
        feedback: Option<&str>,
    ) -> rusqlite::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO creations (id, level_id, user_input, agent_output, score, rubric, feedback, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                creation_id,
                level_id,
                user_input,
                agent_output_json,
                score,
                rubric_json,
                feedback,
                now_millis()
            ],
        )?;
        Ok(())
    }

    pub fn list_creations(&self, level_id: Option<&str>) -> rusqlite::Result<Vec<CreationRow>> {
        let conn = self.conn.lock().unwrap();
        let (sql, has_filter) = match level_id {
            Some(_) => (
                "SELECT id, level_id, user_input, agent_output, score, rubric, feedback, created_at
                 FROM creations WHERE level_id = ?1
                 ORDER BY created_at DESC",
                true,
            ),
            None => (
                "SELECT id, level_id, user_input, agent_output, score, rubric, feedback, created_at
                 FROM creations ORDER BY created_at DESC",
                false,
            ),
        };
        let mut stmt = conn.prepare(sql)?;
        let mapper = |row: &rusqlite::Row| -> rusqlite::Result<CreationRow> {
            Ok(CreationRow {
                id: row.get(0)?,
                level_id: row.get(1)?,
                user_input: row.get(2)?,
                agent_output: row.get(3)?,
                score: row.get::<_, Option<u32>>(4)?,
                rubric: row.get(5)?,
                feedback: row.get(6)?,
                created_at: row.get(7)?,
            })
        };
        if has_filter {
            let rows = stmt.query_map(params![level_id.unwrap()], mapper)?;
            rows.collect()
        } else {
            let rows = stmt.query_map([], mapper)?;
            rows.collect()
        }
    }

    pub fn insert_asset(
        &self,
        creation_id: &str,
        kind: &str,
        url: &str,
        thumbnail_url: Option<&str>,
        prompt: &str,
        tool: &str,
        tokens_cost: u32,
    ) -> rusqlite::Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO assets (creation_id, kind, url, thumbnail_url, prompt, tool, tokens_cost)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![creation_id, kind, url, thumbnail_url, prompt, tool, tokens_cost],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_assets(&self, creation_id: &str) -> rusqlite::Result<Vec<AssetRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT kind, url, thumbnail_url, prompt, tool, tokens_cost
             FROM assets WHERE creation_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![creation_id], |row| {
            Ok(AssetRow {
                kind: row.get(0)?,
                url: row.get(1)?,
                thumbnail_url: row.get(2)?,
                prompt: row.get(3)?,
                tool: row.get(4)?,
                tokens_cost: row.get(5)?,
            })
        })?;
        rows.collect()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CreationRow {
    pub id: String,
    pub level_id: String,
    pub user_input: String,
    pub agent_output: String,
    pub score: Option<u32>,
    pub rubric: Option<String>,
    pub feedback: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AssetRow {
    pub kind: String,
    pub url: String,
    pub thumbnail_url: Option<String>,
    pub prompt: String,
    pub tool: String,
    pub tokens_cost: u32,
}

fn parse_status(s: &str) -> LevelStatus {
    match s {
        "completed" => LevelStatus::Completed,
        "in_progress" => LevelStatus::InProgress,
        "available" => LevelStatus::Available,
        _ => LevelStatus::Locked,
    }
}

fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
