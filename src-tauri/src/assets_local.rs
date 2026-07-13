use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::io::AsyncWriteExt;
use tokio::task::JoinHandle;

const EVENT_CHANNEL: &str = "asset://local";
const MAX_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssetLocalEvent {
    pub project_id: String,
    pub url: String,
    pub local_path: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetLocalRecord {
    pub id: i64,
    pub project_id: String,
    pub url: String,
    pub local_path: String,
    pub kind: String,
    pub bytes: Option<u64>,
    pub status: String,
}

pub trait AssetEventSink: Send + Sync {
    fn emit(&self, event: &AssetLocalEvent);
}

pub struct NoopAssetEventSink;

impl AssetEventSink for NoopAssetEventSink {
    fn emit(&self, _event: &AssetLocalEvent) {}
}

pub struct TauriAssetEventSink {
    app: AppHandle,
}

impl TauriAssetEventSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl AssetEventSink for TauriAssetEventSink {
    fn emit(&self, event: &AssetLocalEvent) {
        if let Err(error) = self.app.emit(EVENT_CHANNEL, event) {
            eprintln!("[assets_local] emit failed: {error}");
        }
    }
}

pub struct AssetsLocal {
    projects_root: PathBuf,
    db_path: PathBuf,
    client: reqwest::Client,
    tasks: Mutex<HashMap<i64, JoinHandle<()>>>,
    sink: Arc<dyn AssetEventSink>,
}

impl AssetsLocal {
    pub fn new(
        app_data_dir: &Path,
        db_path: &Path,
        sink: Arc<dyn AssetEventSink>,
    ) -> Result<Self, String> {
        let projects_root = app_data_dir.join("projects");
        std::fs::create_dir_all(&projects_root).map_err(|e| format!("create projects dir: {e}"))?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|e| format!("build download client: {e}"))?;
        Ok(Self {
            projects_root,
            db_path: db_path.to_path_buf(),
            client,
            tasks: Mutex::new(HashMap::new()),
            sink,
        })
    }

    pub fn new_noop(app_data_dir: &Path, db_path: &Path) -> Result<Self, String> {
        Self::new(app_data_dir, db_path, Arc::new(NoopAssetEventSink))
    }

    pub fn enqueue(
        &self,
        project_id: &str,
        url: &str,
        kind: &str,
        sub_path: &str,
    ) -> Result<i64, String> {
        validate_project_id(project_id)?;
        validate_url(url)?;
        validate_kind(kind)?;
        let project_dir = self.projects_root.join(project_id);
        if !project_dir.is_dir() {
            return Err(format!("project directory not found: {project_id}"));
        }
        let relative_path = validate_sub_path(sub_path)?;
        let target = project_dir.join(relative_path);
        let local_path = target.to_string_lossy().into_owned();
        let created_at = now_millis();
        let conn = open_db(&self.db_path)?;
        conn.execute(
            "INSERT INTO assets_local
             (project_id, url, local_path, kind, status, created_at)
             VALUES (?1, ?2, ?3, ?4, 'pending', ?5)
             ON CONFLICT(project_id, url, local_path) DO NOTHING",
            params![project_id, url, local_path, kind, created_at],
        )
        .map_err(|e| format!("insert asset download: {e}"))?;
        let record = query_record_by_key(&conn, project_id, url, &local_path)?
            .ok_or_else(|| "asset download row missing after insert".to_string())?;

        if target.is_file() {
            let bytes = target.metadata().ok().map(|value| value.len());
            update_status(&self.db_path, record.id, "downloaded", bytes)?;
            self.sink.emit(&AssetLocalEvent {
                project_id: project_id.to_string(),
                url: url.to_string(),
                local_path,
                status: "downloaded".to_string(),
            });
            return Ok(record.id);
        }

        let mut tasks = self.tasks.lock().unwrap();
        tasks.retain(|_, handle| !handle.is_finished());
        if tasks.contains_key(&record.id) {
            return Ok(record.id);
        }
        conn.execute(
            "UPDATE assets_local SET status = 'pending', bytes = NULL WHERE id = ?1",
            params![record.id],
        )
        .map_err(|e| format!("reset asset download: {e}"))?;

        let client = self.client.clone();
        let db_path = self.db_path.clone();
        let sink = Arc::clone(&self.sink);
        let event = AssetLocalEvent {
            project_id: project_id.to_string(),
            url: url.to_string(),
            local_path: target.to_string_lossy().into_owned(),
            status: "downloaded".to_string(),
        };
        let download_url = url.to_string();
        let task_id = record.id;
        let handle = tokio::spawn(async move {
            match download_with_retry(&client, &download_url, &target).await {
                Ok(bytes) => {
                    if let Err(error) = update_status(&db_path, task_id, "downloaded", Some(bytes))
                    {
                        eprintln!("[assets_local] update downloaded status failed: {error}");
                    }
                    sink.emit(&event);
                }
                Err(error) => {
                    eprintln!(
                        "[assets_local] download failed after {MAX_ATTEMPTS} attempts: {error}"
                    );
                    if let Err(db_error) = update_status(&db_path, task_id, "failed", None) {
                        eprintln!("[assets_local] update failed status failed: {db_error}");
                    }
                    let mut failed_event = event;
                    failed_event.status = "failed".to_string();
                    sink.emit(&failed_event);
                }
            }
        });
        tasks.insert(record.id, handle);
        Ok(record.id)
    }

    pub fn resolve(&self, project_id: &str, url: &str) -> Result<Option<String>, String> {
        validate_project_id(project_id)?;
        let conn = open_db(&self.db_path)?;
        let local_path = conn
            .query_row(
                "SELECT local_path FROM assets_local
                 WHERE project_id = ?1 AND url = ?2 AND status = 'downloaded'
                 ORDER BY id DESC LIMIT 1",
                params![project_id, url],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| format!("resolve asset: {e}"))?;
        Ok(local_path.filter(|path| Path::new(path).is_file()))
    }

    pub fn get_record(&self, id: i64) -> Result<Option<AssetLocalRecord>, String> {
        let conn = open_db(&self.db_path)?;
        conn.query_row(
            "SELECT id, project_id, url, local_path, kind, bytes, status
             FROM assets_local WHERE id = ?1",
            params![id],
            asset_record_from_row,
        )
        .optional()
        .map_err(|e| format!("load asset download: {e}"))
    }

    pub async fn wait(&self, id: i64) -> Result<(), String> {
        let handle = self.tasks.lock().unwrap().remove(&id);
        if let Some(handle) = handle {
            handle
                .await
                .map_err(|e| format!("asset download task: {e}"))?;
        }
        Ok(())
    }
}

#[tauri::command]
pub fn download_asset(
    project_id: String,
    url: String,
    kind: String,
    sub_path: String,
    assets: State<'_, AssetsLocal>,
) -> Result<i64, String> {
    assets.enqueue(&project_id, &url, &kind, &sub_path)
}

#[tauri::command]
pub fn resolve_asset(
    project_id: String,
    url: String,
    assets: State<'_, AssetsLocal>,
) -> Result<Option<String>, String> {
    assets.resolve(&project_id, &url)
}

async fn download_with_retry(
    client: &reqwest::Client,
    url: &str,
    target: &Path,
) -> Result<u64, String> {
    let mut last_error = String::new();
    for attempt in 0..MAX_ATTEMPTS {
        match download_once(client, url, target).await {
            Ok(bytes) => return Ok(bytes),
            Err(error) => {
                last_error = error;
                let _ = tokio::fs::remove_file(temp_path(target)).await;
                if attempt + 1 < MAX_ATTEMPTS {
                    tokio::time::sleep(Duration::from_millis(200 * 2_u64.pow(attempt))).await;
                }
            }
        }
    }
    Err(last_error)
}

async fn download_once(client: &reqwest::Client, url: &str, target: &Path) -> Result<u64, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request: {e}"))?
        .error_for_status()
        .map_err(|e| format!("http status: {e}"))?;
    if let Some(parent) = target.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("create asset dir: {e}"))?;
    }
    let tmp = temp_path(target);
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| format!("create asset temp file: {e}"))?;
    let mut bytes = 0_u64;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("read response: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("write asset: {e}"))?;
        bytes += chunk.len() as u64;
    }
    file.sync_all()
        .await
        .map_err(|e| format!("sync asset: {e}"))?;
    drop(file);
    tokio::fs::rename(&tmp, target)
        .await
        .map_err(|e| format!("publish asset: {e}"))?;
    Ok(bytes)
}

fn query_record_by_key(
    conn: &Connection,
    project_id: &str,
    url: &str,
    local_path: &str,
) -> Result<Option<AssetLocalRecord>, String> {
    conn.query_row(
        "SELECT id, project_id, url, local_path, kind, bytes, status
         FROM assets_local WHERE project_id = ?1 AND url = ?2 AND local_path = ?3",
        params![project_id, url, local_path],
        asset_record_from_row,
    )
    .optional()
    .map_err(|e| format!("load asset download: {e}"))
}

fn asset_record_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AssetLocalRecord> {
    Ok(AssetLocalRecord {
        id: row.get(0)?,
        project_id: row.get(1)?,
        url: row.get(2)?,
        local_path: row.get(3)?,
        kind: row.get(4)?,
        bytes: row.get(5)?,
        status: row.get(6)?,
    })
}

fn open_db(db_path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(db_path).map_err(|e| format!("open asset db: {e}"))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| format!("configure asset db: {e}"))?;
    Ok(conn)
}

fn update_status(db_path: &Path, id: i64, status: &str, bytes: Option<u64>) -> Result<(), String> {
    let conn = open_db(db_path)?;
    conn.execute(
        "UPDATE assets_local SET status = ?2, bytes = ?3 WHERE id = ?1",
        params![id, status, bytes],
    )
    .map_err(|e| format!("update asset status: {e}"))?;
    Ok(())
}

fn validate_project_id(project_id: &str) -> Result<(), String> {
    uuid::Uuid::parse_str(project_id)
        .map(|_| ())
        .map_err(|_| "invalid project id".to_string())
}

fn validate_url(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url).map_err(|e| format!("invalid asset url: {e}"))?;
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        _ => Err("asset url must use http or https".to_string()),
    }
}

fn validate_kind(kind: &str) -> Result<(), String> {
    match kind {
        "image" | "video" | "audio" => Ok(()),
        _ => Err("asset kind must be image, video, or audio".to_string()),
    }
}

fn validate_sub_path(sub_path: &str) -> Result<PathBuf, String> {
    let path = Path::new(sub_path);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err("asset sub path must be relative".to_string());
    }
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("asset sub path contains invalid components".to_string());
    }
    Ok(path.to_path_buf())
}

fn temp_path(target: &Path) -> PathBuf {
    let extension = target
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    target.with_extension(format!("{extension}.download"))
}

fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
