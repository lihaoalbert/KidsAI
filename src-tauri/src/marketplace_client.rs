// W10/W11 共享底座 — MarketplaceClient
//
// 桌面端 ↔ kidsai-server 控制平面: 通用 HTTPS 客户端, Bearer license_token 鉴权.
// skills (W10 Day 3+) + secrets (W11 Day 6+) + telemetry (W11 Day 8+) 都走它.
//
// 关键能力:
// - 同步 Bearer (用 LicenseStore.load() 拿到的 license_token)
// - 指数退避重试 (网络抖, server 5xx 触发)
// - Offline cache: 请求失败 → 读 last-known-good 缓存 → 至少不崩 (skills index 离线仍能用)
// - 不解密: bundle 解密走 secrets.rs, 这里只负责网络字节流
//
// 两种模式:
//   Server { base_url } — KIDSAI_SERVER_URL env 设置时, 真实 HTTPS
//   Offline              — 未设置 env, 所有请求直接走 cache / 返 Err(OfflineMode)

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub enum MarketplaceMode {
    Server { base_url: String },
    Offline,
}

#[derive(Debug, thiserror::Error)]
pub enum MarketplaceError {
    #[error("offline mode: no KIDSAI_SERVER_URL configured")]
    Offline,
    #[error("http: {0}")]
    Http(String),
    #[error("parse: {0}")]
    Parse(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("retry exhausted after {0} attempts: {1}")]
    RetryExhausted(u32, String),
    #[error("cache io: {0}")]
    CacheIo(String),
}

#[derive(Clone)]
pub struct MarketplaceClient {
    mode: MarketplaceMode,
    http: reqwest::Client,
    cache_dir: PathBuf,
    /// 启动期注入 license_token; 切换账号时由 LicenseStore.save 后调用 set_token.
    token: Arc<RwLock<Option<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedEntry {
    cached_at: i64,        // unix seconds
    bytes_base64: String,
}

impl MarketplaceClient {
    pub fn from_env(cache_dir: PathBuf) -> Self {
        let _ = dotenvy::dotenv();
        let mode = match std::env::var("KIDSAI_SERVER_URL") {
            Ok(url) if !url.trim().is_empty() => MarketplaceMode::Server {
                base_url: url.trim_end_matches('/').to_string(),
            },
            _ => MarketplaceMode::Offline,
        };
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .expect("reqwest client");
        Self {
            mode,
            http,
            cache_dir,
            token: Arc::new(RwLock::new(None)),
        }
    }

    pub fn is_offline(&self) -> bool {
        matches!(self.mode, MarketplaceMode::Offline)
    }

    pub fn mode_label(&self) -> &'static str {
        match self.mode {
            MarketplaceMode::Server { .. } => "server",
            MarketplaceMode::Offline => "offline",
        }
    }

    pub async fn set_token(&self, token: Option<String>) {
        *self.token.write().await = token;
    }

    /// 通用 GET — 返回 raw bytes. 重试 3 次 (200ms / 400ms / 800ms 退避).
    /// 网络失败 + 有 cache → 返 cache 内容 + 标记 stale (调用方决定是否接受).
    pub async fn get_bytes(&self, path: &str) -> Result<(Vec<u8>, ResponseMeta), MarketplaceError> {
        let url = match &self.mode {
            MarketplaceMode::Server { base_url } => format!("{base_url}{path}"),
            MarketplaceMode::Offline => return Err(MarketplaceError::Offline),
        };
        let token = self.token.read().await.clone();

        let mut last_err: Option<String> = None;
        for attempt in 1u32..=3 {
            let mut req = self.http.get(&url);
            if let Some(t) = token.as_ref() {
                req = req.bearer_auth(t);
            }
            match req.send().await {
                Ok(r) => {
                    let status = r.status();
                    if status.is_success() {
                        let bytes = r
                            .bytes()
                            .await
                            .map_err(|e| MarketplaceError::Http(e.to_string()))?;
                        // 写 cache (后台, 不阻塞主路径)
                        self.write_cache(path, &bytes);
                        return Ok((
                            bytes.to_vec(),
                            ResponseMeta {
                                from_cache: false,
                                status: status.as_u16(),
                            },
                        ));
                    } else if status.as_u16() == 404 {
                        return Err(MarketplaceError::NotFound(path.to_string()));
                    } else {
                        last_err = Some(format!("status {}", status.as_u16()));
                    }
                }
                Err(e) => {
                    last_err = Some(format!("{e}"));
                }
            }
            // 退避 200ms * 2^(attempt-1)
            let delay = Duration::from_millis(200u64 * (1 << (attempt - 1)));
            tokio::time::sleep(delay).await;
        }

        // 重试都失败 → 尝试 cache
        if let Some(cached) = self.read_cache(path) {
            return Ok((
                cached,
                ResponseMeta {
                    from_cache: true,
                    status: 0,
                },
            ));
        }
        Err(MarketplaceError::RetryExhausted(
            3,
            last_err.unwrap_or_else(|| "unknown".into()),
        ))
    }

    pub async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
    ) -> Result<(T, ResponseMeta), MarketplaceError> {
        let (bytes, meta) = self.get_bytes(path).await?;
        let v = serde_json::from_slice::<T>(&bytes)
            .map_err(|e| MarketplaceError::Parse(e.to_string()))?;
        Ok((v, meta))
    }

    /// 通用 POST — 不重试 (server 状态写不该 retry, 可能重复扣费).
    pub async fn post_json<B: Serialize, R: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<R, MarketplaceError> {
        let url = match &self.mode {
            MarketplaceMode::Server { base_url } => format!("{base_url}{path}"),
            MarketplaceMode::Offline => return Err(MarketplaceError::Offline),
        };
        let token = self.token.read().await.clone();
        let mut req = self.http.post(&url).json(body);
        if let Some(t) = token.as_ref() {
            req = req.bearer_auth(t);
        }
        let r = req.send().await.map_err(|e| MarketplaceError::Http(e.to_string()))?;
        let status = r.status();
        if status.as_u16() == 404 {
            return Err(MarketplaceError::NotFound(path.to_string()));
        }
        if !status.is_success() {
            let txt = r.text().await.unwrap_or_default();
            return Err(MarketplaceError::Http(format!("{}: {}", status, txt)));
        }
        r.json::<R>()
            .await
            .map_err(|e| MarketplaceError::Parse(e.to_string()))
    }

    fn cache_path(&self, key: &str) -> PathBuf {
        // 简单 hash 化文件名, 防特殊字符 / 超长路径
        let safe = key
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>();
        self.cache_dir.join(format!("mkt_{safe}.bin"))
    }

    fn write_cache(&self, key: &str, bytes: &[u8]) {
        let path = self.cache_path(key);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let entry = CachedEntry {
            cached_at: now_secs(),
            bytes_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        };
        if let Ok(s) = serde_json::to_string(&entry) {
            let _ = std::fs::write(&path, s);
        }
    }

    fn read_cache(&self, key: &str) -> Option<Vec<u8>> {
        let path = self.cache_path(key);
        let text = std::fs::read_to_string(&path).ok()?;
        let entry: CachedEntry = serde_json::from_str(&text).ok()?;
        base64::engine::general_purpose::STANDARD
            .decode(entry.bytes_base64.as_bytes())
            .ok()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ResponseMeta {
    pub from_cache: bool,
    pub status: u16,
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn offline_mode_when_env_unset() {
        std::env::remove_var("KIDSAI_SERVER_URL");
        let c = MarketplaceClient::from_env(tempdir().unwrap().path().to_path_buf());
        assert!(c.is_offline());
        assert_eq!(c.mode_label(), "offline");
    }

    #[tokio::test]
    async fn offline_get_returns_err() {
        std::env::remove_var("KIDSAI_SERVER_URL");
        let c = MarketplaceClient::from_env(tempdir().unwrap().path().to_path_buf());
        let r = c.get_bytes("/anything").await;
        assert!(matches!(r, Err(MarketplaceError::Offline)));
    }

    #[tokio::test]
    async fn offline_post_returns_err() {
        std::env::remove_var("KIDSAI_SERVER_URL");
        let c = MarketplaceClient::from_env(tempdir().unwrap().path().to_path_buf());
        let r = c
            .post_json::<serde_json::Value, serde_json::Value>("/x", &serde_json::json!({}))
            .await;
        assert!(matches!(r, Err(MarketplaceError::Offline)));
    }

    #[test]
    fn cache_path_sanitizes() {
        let c = MarketplaceClient::from_env(tempdir().unwrap().path().to_path_buf());
        let p = c.cache_path("/api/v1/skills/index?foo=bar");
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("mkt_"));
        // 原始路径里的 / ? & = 等都被替换为 _
        assert!(!name.contains('/'));
        assert!(!name.contains('?'));
    }

    #[test]
    fn write_then_read_cache_roundtrip() {
        let dir = tempdir().unwrap();
        let c = MarketplaceClient::from_env(dir.path().to_path_buf());
        c.write_cache("/test/k", b"hello-world");
        let read = c.read_cache("/test/k");
        assert_eq!(read, Some(b"hello-world".to_vec()));
    }

    #[test]
    fn read_cache_missing_returns_none() {
        let dir = tempdir().unwrap();
        let c = MarketplaceClient::from_env(dir.path().to_path_buf());
        assert_eq!(c.read_cache("/never"), None);
    }
}