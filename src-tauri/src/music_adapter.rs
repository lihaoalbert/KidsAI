// MiniMax 音乐生成 adapter (W6 C3) — music-01 async + polling
//
// 文档权威来源: MiniMax API Reference (music_generation).
// 1) POST /v1/music_generation with JSON: { "model": "music-01", "prompt": "...", "duration": 30, "instrumental": false }
//    响应: { "task_id": "abc123" }
// 2) GET /v1/query/music?task_id=abc123 → { "task_id", "status": "queued|running|succeeded|failed", "data": {"audio_url": "..."}}
// 3) 轮询直到 succeeded 或 failed (类似 Seedance 视频)
//
// 对齐 video_adapter.rs 的同步轮询模式.

use serde::{Deserialize, Serialize};
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "https://api.minimaxi.com/v1";
const DEFAULT_MODEL: &str = "music-01";
const POLL_INTERVAL_MS: u64 = 2000;
const POLL_MAX_ATTEMPTS: u32 = 90; // ~3 分钟上限 (比视频短, 音乐是 30s)
const HTTP_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone)]
pub struct MusicGenArgs {
    /// 风格/情绪描述, 例 "playful ukulele, cartoon intro, cheerful"
    pub prompt: String,
    /// 时长 (秒), 默认 30 (MiniMax music-01 范围 ~10-60s)
    pub duration_seconds: u32,
    /// true = 纯器乐, false = 含人声 (W6 默认 true, 适合视频 BGM)
    pub instrumental: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicAsset {
    pub url: String,
    pub provider_task_id: String,
    pub provider: String, // "minimax" | "mock"
    pub duration_seconds: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

pub trait MusicAdapter: Send + Sync {
    fn provider_name(&self) -> &'static str;
    fn generate(&self, args: &MusicGenArgs) -> Result<MusicAsset, String>;
}

// ============ Mock ============

pub struct MockMusicAdapter;

impl MusicAdapter for MockMusicAdapter {
    fn provider_name(&self) -> &'static str {
        "mock"
    }
    fn generate(&self, args: &MusicGenArgs) -> Result<MusicAsset, String> {
        // placeholder mp3 (silent frame); 真实 MiniMax 返回 BGM URL
        Ok(MusicAsset {
            url: format!("https://example.com/bgm/{}.mp3", simple_hash(&args.prompt)),
            provider_task_id: format!("mock_music_{}", simple_hash(&args.prompt)),
            provider: "mock".to_string(),
            duration_seconds: args.duration_seconds,
            model: None,
        })
    }
}

// ============ MiniMax ============

pub struct MiniMaxMusicAdapter {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl MiniMaxMusicAdapter {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .expect("reqwest client");
        Self {
            base_url,
            api_key,
            model,
            client,
        }
    }

    fn post_create(&self, args: &MusicGenArgs) -> Result<String, String> {
        let url = format!("{}/music_generation", self.base_url);
        let body = serde_json::json!({
            "model": self.model,
            "prompt": args.prompt,
            "duration": args.duration_seconds,
            "instrumental": args.instrumental,
        });
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| format!("music_generation create failed: {e}"))?;
        let status = resp.status();
        let body_text = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "music_generation HTTP {}: {}",
                status,
                truncate(&body_text, 240)
            ));
        }
        let parsed: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| format!("music_generation invalid JSON: {e}"))?;
        parsed
            .get("task_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                format!(
                    "music_generation missing task_id: {}",
                    truncate(&body_text, 200)
                )
            })
    }

    fn poll_until_done(&self, task_id: &str, duration_seconds: u32) -> Result<MusicAsset, String> {
        // GET /v1/query/music?task_id=...
        let poll_url = format!("{}/query/music?task_id={}", self.base_url, task_id);
        for attempt in 0..POLL_MAX_ATTEMPTS {
            std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            let resp = self
                .client
                .get(&poll_url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .send()
                .map_err(|e| format!("music poll failed: {e}"))?;
            let status = resp.status();
            let body_text = resp.text().unwrap_or_default();
            if !status.is_success() {
                return Err(format!(
                    "music poll HTTP {}: {}",
                    status,
                    truncate(&body_text, 240)
                ));
            }
            let parsed: serde_json::Value = serde_json::from_str(&body_text)
                .map_err(|e| format!("music poll invalid JSON: {e}"))?;
            let task_status = parsed.get("status").and_then(|v| v.as_str()).unwrap_or("");
            match task_status {
                "succeeded" => {
                    let url = parsed
                        .pointer("/data/audio_url")
                        .and_then(|v| v.as_str())
                        .or_else(|| parsed.pointer("/data/url").and_then(|v| v.as_str()))
                        .ok_or_else(|| {
                            format!(
                                "music succeeded but missing audio_url: {}",
                                truncate(&body_text, 200)
                            )
                        })?;
                    return Ok(MusicAsset {
                        url: url.to_string(),
                        provider_task_id: task_id.to_string(),
                        provider: "minimax".to_string(),
                        duration_seconds,
                        model: Some(self.model.clone()),
                    });
                }
                "failed" => {
                    let msg = parsed
                        .pointer("/error/message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("(no message)");
                    return Err(format!("music task failed: {}", msg));
                }
                "queued" | "running" => {
                    if attempt == POLL_MAX_ATTEMPTS - 1 {
                        return Err(format!(
                            "music still {} after {} polls",
                            task_status, POLL_MAX_ATTEMPTS
                        ));
                    }
                }
                _ => {
                    if attempt == POLL_MAX_ATTEMPTS - 1 {
                        return Err(format!(
                            "music unknown status '{}' after {} polls",
                            task_status, POLL_MAX_ATTEMPTS
                        ));
                    }
                }
            }
        }
        Err(format!(
            "music did not complete in {} polls",
            POLL_MAX_ATTEMPTS
        ))
    }
}

impl MusicAdapter for MiniMaxMusicAdapter {
    fn provider_name(&self) -> &'static str {
        "minimax"
    }
    fn generate(&self, args: &MusicGenArgs) -> Result<MusicAsset, String> {
        let task_id = self.post_create(args)?;
        self.poll_until_done(&task_id, args.duration_seconds)
    }
}

// ============ Factory ============

pub struct SelectedMusicAdapter {
    pub adapter: Box<dyn MusicAdapter>,
    pub source: String,
}

pub fn select_music_adapter() -> SelectedMusicAdapter {
    if let Ok(key) = std::env::var("MINIMAX_API_KEY") {
        if !key.is_empty() {
            let base_url =
                std::env::var("MINIMAX_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
            let model =
                std::env::var("MINIMAX_MUSIC_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
            return SelectedMusicAdapter {
                adapter: Box::new(MiniMaxMusicAdapter::new(base_url, key, model)),
                source: "minimax".to_string(),
            };
        }
    }
    SelectedMusicAdapter {
        adapter: Box::new(MockMusicAdapter),
        source: "mock".to_string(),
    }
}

// ============ helpers ============

fn simple_hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:x}", h & 0xffffffff)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut t = s[..max].to_string();
        t.push('…');
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_returns_placeholder_url() {
        let a = MockMusicAdapter;
        let out = a
            .generate(&MusicGenArgs {
                prompt: "happy".into(),
                duration_seconds: 30,
                instrumental: true,
            })
            .unwrap();
        assert_eq!(out.provider, "mock");
        assert!(out.url.contains("example.com/bgm/"));
        assert_eq!(out.duration_seconds, 30);
    }

    #[test]
    fn select_mock_by_default() {
        std::env::remove_var("MINIMAX_API_KEY");
        let s = select_music_adapter();
        assert_eq!(s.source, "mock");
    }

    #[test]
    fn select_minimax_when_key_set() {
        std::env::set_var("MINIMAX_API_KEY", "test");
        let s = select_music_adapter();
        assert_eq!(s.source, "minimax");
        std::env::remove_var("MINIMAX_API_KEY");
    }

    #[test]
    fn minimax_polls_running_then_succeeds() {
        let mut server = mockito::Server::new();
        // 1) create
        server
            .mock("POST", "/music_generation")
            .with_status(200)
            .with_body(r#"{"task_id":"m_1","base_resp":{"status_code":0}}"#)
            .create();
        // 2) first poll: running
        let m1 = server
            .mock("GET", "/query/music")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"status":"running"}"#)
            .expect(1)
            .create();
        let m2 = server
            .mock("GET", "/query/music")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(
                r#"{"status":"succeeded","data":{"audio_url":"https://cdn.example/bgm.mp3"}}"#,
            )
            .expect(1)
            .create();
        std::env::set_var("MINIMAX_BASE_URL", server.url());
        std::env::set_var("MINIMAX_API_KEY", "k");
        let a = select_music_adapter();
        let out = a
            .adapter
            .generate(&MusicGenArgs {
                prompt: "cheerful ukulele".into(),
                duration_seconds: 30,
                instrumental: true,
            })
            .expect("ok");
        m1.assert();
        m2.assert();
        assert_eq!(out.url, "https://cdn.example/bgm.mp3");
        assert_eq!(out.provider, "minimax");
        std::env::remove_var("MINIMAX_API_KEY");
    }

    #[test]
    fn minimax_failed_task_surfaces_error() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/music_generation")
            .with_status(200)
            .with_body(r#"{"task_id":"m_fail"}"#)
            .create();
        server
            .mock("GET", "/query/music")
            .match_query(mockito::Matcher::Any)
            .with_status(200)
            .with_body(r#"{"status":"failed","error":{"message":"prompt rejected"}}"#)
            .create();
        std::env::set_var("MINIMAX_BASE_URL", server.url());
        std::env::set_var("MINIMAX_API_KEY", "k");
        let a = select_music_adapter();
        let err = a
            .adapter
            .generate(&MusicGenArgs {
                prompt: "bad".into(),
                duration_seconds: 30,
                instrumental: true,
            })
            .unwrap_err();
        assert!(err.contains("prompt rejected"), "got: {err}");
        std::env::remove_var("MINIMAX_API_KEY");
    }

    #[test]
    fn minimax_401_does_not_leak_key() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/music_generation")
            .with_status(401)
            .with_body(r#"{"error":{"message":"bad key"}}"#)
            .create();
        std::env::set_var("MINIMAX_BASE_URL", server.url());
        std::env::set_var("MINIMAX_API_KEY", "super-secret-music-key");
        let a = select_music_adapter();
        let err = a
            .adapter
            .generate(&MusicGenArgs {
                prompt: "x".into(),
                duration_seconds: 30,
                instrumental: true,
            })
            .unwrap_err();
        assert!(err.contains("HTTP 401"));
        assert!(!err.contains("super-secret-music-key"), "key leaked: {err}");
        std::env::remove_var("MINIMAX_API_KEY");
    }
}
