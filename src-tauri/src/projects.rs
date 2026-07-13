use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::db::Db;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    pub id: String,
    pub title: String,
    pub level_id: Option<String>,
    pub cursor: u8,
    pub thumb_path: Option<String>,
    pub total_credits: u32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub title: String,
    pub level_id: Option<String>,
    pub cursor: u8,
    pub thumb_path: Option<String>,
    pub total_credits: u32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFull {
    pub meta: ProjectMeta,
    pub plan: Value,
    pub transcript: Value,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatePatch {
    pub cursor: Option<u8>,
    pub thumb_path: Option<String>,
    pub total_credits: Option<u32>,
}

pub struct Projects {
    root: PathBuf,
    file_lock: Mutex<()>,
}

impl Projects {
    pub fn new(app_data_dir: &Path) -> Result<Self, String> {
        let root = app_data_dir.join("projects");
        fs::create_dir_all(root.join("_trash")).map_err(|e| format!("create projects dir: {e}"))?;
        Ok(Self {
            root,
            file_lock: Mutex::new(()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn list(&self, db: &Db) -> Result<Vec<ProjectSummary>, String> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, title, level_id, cursor, thumb_path, total_credits, created_at, updated_at
                 FROM project_meta ORDER BY updated_at DESC, created_at DESC",
            )?;
            let rows = stmt.query_map([], project_summary_from_row)?;
            rows.collect()
        })
        .map_err(|e| format!("list projects: {e}"))
    }

    pub fn create(
        &self,
        db: &Db,
        title: &str,
        level_id: Option<&str>,
    ) -> Result<ProjectMeta, String> {
        let title = normalized_title(title)?;
        let level_id = level_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let now = now_millis();
        let meta = ProjectMeta {
            id: Uuid::new_v4().to_string(),
            title,
            level_id,
            cursor: 0,
            thumb_path: None,
            total_credits: 0,
            created_at: now,
            updated_at: now,
        };
        let dir = self.project_dir(&meta.id)?;
        let _guard = self.file_lock.lock().unwrap();
        fs::create_dir(&dir).map_err(|e| format!("create project dir: {e}"))?;
        if let Err(error) = self.write_initial_files(&dir, &meta) {
            let _ = fs::remove_dir_all(&dir);
            return Err(error);
        }
        let inserted = db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO project_meta
                 (id, title, level_id, cursor, thumb_path, total_credits, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    meta.id,
                    meta.title,
                    meta.level_id,
                    meta.cursor,
                    meta.thumb_path,
                    meta.total_credits,
                    meta.created_at,
                    meta.updated_at
                ],
            )?;
            Ok(())
        });
        if let Err(error) = inserted {
            let _ = fs::remove_dir_all(&dir);
            return Err(format!("insert project: {error}"));
        }
        Ok(meta)
    }

    pub fn load(&self, db: &Db, id: &str) -> Result<ProjectFull, String> {
        let dir = self.project_dir(id)?;
        let _guard = self.file_lock.lock().unwrap();
        let meta = self.get_meta(db, id)?;
        let plan = read_json(&dir.join("plan.json"))?;
        let transcript = read_json(&dir.join("transcript.json"))?;
        Ok(ProjectFull {
            meta,
            plan,
            transcript,
        })
    }

    pub fn rename(&self, db: &Db, id: &str, title: &str) -> Result<(), String> {
        let dir = self.project_dir(id)?;
        let title = normalized_title(title)?;
        let now = now_millis();
        let _guard = self.file_lock.lock().unwrap();
        let changed = db
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE project_meta SET title = ?2, updated_at = ?3 WHERE id = ?1",
                    params![id, title, now],
                )
            })
            .map_err(|e| format!("rename project: {e}"))?;
        if changed == 0 {
            return Err(format!("project not found: {id}"));
        }
        let meta = self.get_meta(db, id)?;
        write_json_atomic(&dir.join("project.json"), &meta)
    }

    pub fn save_state(
        &self,
        db: &Db,
        id: &str,
        plan: &Value,
        transcript: &Value,
        patch: &ProjectStatePatch,
    ) -> Result<ProjectMeta, String> {
        let dir = self.project_dir(id)?;
        if let Some(cursor) = patch.cursor {
            if cursor > 6 {
                return Err("cursor must be 0-6".to_string());
            }
        }
        let _guard = self.file_lock.lock().unwrap();
        self.get_meta(db, id)?;
        write_json_atomic(&dir.join("plan.json"), plan)?;
        write_json_atomic(&dir.join("transcript.json"), transcript)?;
        let now = now_millis();
        db.with_connection(|conn| {
            conn.execute(
                "UPDATE project_meta SET
                   cursor = COALESCE(?2, cursor),
                   thumb_path = COALESCE(?3, thumb_path),
                   total_credits = COALESCE(?4, total_credits),
                   updated_at = ?5
                 WHERE id = ?1",
                params![id, patch.cursor, patch.thumb_path, patch.total_credits, now],
            )?;
            Ok(())
        })
        .map_err(|e| format!("update project state: {e}"))?;
        let meta = self.get_meta(db, id)?;
        write_json_atomic(&dir.join("project.json"), &meta)?;
        Ok(meta)
    }

    pub fn delete(&self, db: &Db, id: &str) -> Result<(), String> {
        let dir = self.project_dir(id)?;
        let trash_dir = self.root.join("_trash").join(id);
        let _guard = self.file_lock.lock().unwrap();
        self.get_meta(db, id)?;
        if trash_dir.exists() {
            return Err(format!("trash project already exists: {id}"));
        }
        fs::rename(&dir, &trash_dir).map_err(|e| format!("move project to trash: {e}"))?;
        let deleted = db.with_connection(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "DELETE FROM assets_local WHERE project_id = ?1",
                params![id],
            )?;
            tx.execute("DELETE FROM project_meta WHERE id = ?1", params![id])?;
            tx.commit()
        });
        if let Err(error) = deleted {
            let _ = fs::rename(&trash_dir, &dir);
            return Err(format!("delete project metadata: {error}"));
        }
        write_json_atomic(
            &trash_dir.join("deleted.json"),
            &serde_json::json!({ "deletedAt": now_millis() }),
        )?;
        Ok(())
    }

    fn get_meta(&self, db: &Db, id: &str) -> Result<ProjectMeta, String> {
        self.project_dir(id)?;
        db.with_connection(|conn| {
            conn.query_row(
                "SELECT id, title, level_id, cursor, thumb_path, total_credits, created_at, updated_at
                 FROM project_meta WHERE id = ?1",
                params![id],
                project_meta_from_row,
            )
            .optional()
        })
        .map_err(|e| format!("load project metadata: {e}"))?
        .ok_or_else(|| format!("project not found: {id}"))
    }

    fn project_dir(&self, id: &str) -> Result<PathBuf, String> {
        Uuid::parse_str(id).map_err(|_| "invalid project id".to_string())?;
        Ok(self.root.join(id))
    }

    fn write_initial_files(&self, dir: &Path, meta: &ProjectMeta) -> Result<(), String> {
        write_json_atomic(&dir.join("project.json"), meta)?;
        write_json_atomic(&dir.join("plan.json"), &serde_json::json!({}))?;
        write_json_atomic(&dir.join("transcript.json"), &serde_json::json!([]))
    }
}

#[tauri::command]
pub fn list_projects(
    projects: tauri::State<'_, Projects>,
    db: tauri::State<'_, Db>,
) -> Result<Vec<ProjectSummary>, String> {
    projects.list(&db)
}

#[tauri::command]
pub fn load_project(
    id: String,
    projects: tauri::State<'_, Projects>,
    db: tauri::State<'_, Db>,
) -> Result<ProjectFull, String> {
    projects.load(&db, &id)
}

#[tauri::command]
pub fn create_project(
    title: String,
    level_id: Option<String>,
    projects: tauri::State<'_, Projects>,
    db: tauri::State<'_, Db>,
) -> Result<ProjectMeta, String> {
    projects.create(&db, &title, level_id.as_deref())
}

#[tauri::command]
pub fn rename_project(
    id: String,
    title: String,
    projects: tauri::State<'_, Projects>,
    db: tauri::State<'_, Db>,
) -> Result<(), String> {
    projects.rename(&db, &id, &title)
}

#[tauri::command]
pub fn delete_project(
    id: String,
    projects: tauri::State<'_, Projects>,
    db: tauri::State<'_, Db>,
) -> Result<(), String> {
    projects.delete(&db, &id)
}

#[tauri::command]
pub fn save_project_state(
    id: String,
    plan: Value,
    transcript: Value,
    meta: ProjectStatePatch,
    projects: tauri::State<'_, Projects>,
    db: tauri::State<'_, Db>,
) -> Result<ProjectMeta, String> {
    projects.save_state(&db, &id, &plan, &transcript, &meta)
}

fn project_meta_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectMeta> {
    Ok(ProjectMeta {
        id: row.get(0)?,
        title: row.get(1)?,
        level_id: row.get(2)?,
        cursor: row.get(3)?,
        thumb_path: row.get(4)?,
        total_credits: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn project_summary_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectSummary> {
    Ok(ProjectSummary {
        id: row.get(0)?,
        title: row.get(1)?,
        level_id: row.get(2)?,
        cursor: row.get(3)?,
        thumb_path: row.get(4)?,
        total_credits: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn normalized_title(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("project title is required".to_string());
    }
    if title.chars().count() > 80 {
        return Err("project title must be at most 80 characters".to_string());
    }
    Ok(title.to_string())
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| format!("serialize json: {e}"))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let tmp = path.with_extension(format!("{extension}.tmp"));
    {
        let mut file =
            fs::File::create(&tmp).map_err(|e| format!("create {}: {e}", tmp.display()))?;
        file.write_all(&json)
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        file.sync_all()
            .map_err(|e| format!("sync {}: {e}", tmp.display()))?;
    }
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("replace {}: {e}", path.display()))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("rename {}: {e}", path.display()))
}

fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
