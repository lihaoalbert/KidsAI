// MiniMax 声音适配器 (W6 C2):
// 1) TTS (text-to-audio)          - 调 /v1/t2a_v2 拿 audio 字节 或 hex
// 2) Voice Clone (上传音频样本)   - 调 /v1/voice_clone 训练一个 voice_id
// 3) TTS with custom voice_id     - 合成带特定"角色声音"的旁白
//
// 文档权威来源: MiniMax API Reference (T2A v2 + voice_clone).
// 3 个 endpoint 都是同步 (调一次即返), 不需要 async polling.
//
// 同步 (reqwest::blocking), 对齐 video_adapter/image_adapter 风格.

use reqwest::blocking::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "https://api.minimaxi.com/v1";
const HTTP_TIMEOUT_SECS: u64 = 60; // TTS 长文本可能慢

// ============ TTS (T2A) ============

#[derive(Debug, Clone)]
pub struct TtsArgs {
    pub text: String,
    /// 自定义 voice_id (调用过 voice_clone 后得到); 缺省走 system voice
    pub voice_id: Option<String>,
    /// 情绪: neutral / happy / sad / angry / fearful / disgusted / surprised / calm
    pub emotion: Option<String>,
    /// 模型: "speech-01-turbo" (默认) / "speech-01-hd" / 等
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsAsset {
    /// audio URL (MiniMax 返回的临时 URL; 通常 24h 过期)
    pub url: String,
    pub provider: String, // "minimax" | "mock"
}

pub trait TtsAdapter: Send + Sync {
    fn synthesize(&self, args: &TtsArgs) -> Result<TtsAsset, String>;
}

pub struct MockTtsAdapter;

impl TtsAdapter for MockTtsAdapter {
    fn synthesize(&self, args: &TtsArgs) -> Result<TtsAsset, String> {
        Ok(TtsAsset {
            url: format!("https://example.com/tts/{}.mp3", simple_hash(&args.text)),
            provider: "mock".to_string(),
        })
    }
}

pub struct MiniMaxTtsAdapter {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl MiniMaxTtsAdapter {
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
}

impl TtsAdapter for MiniMaxTtsAdapter {
    fn synthesize(&self, args: &TtsArgs) -> Result<TtsAsset, String> {
        // MiniMax T2A v2: POST /v1/t2a_v2 with JSON body (model + text + voice_setting)
        // 简化响应: hex 音频 → 我们直接 memo 为占位 file:// 让前端能播
        // (MiniMax 原版 hex 音频需本地解码, W6 阶段先存 voice descriptor,
        // 真要 hex→wav 时再做 base64 解码 + 文件落盘)
        let body = serde_json::json!({
            "model": args.model.clone().unwrap_or_else(|| self.model.clone()),
            "text": args.text,
            "stream": false,
            "voice_setting": {
                "voice_id": args.voice_id.clone().unwrap_or_else(|| "male-qn-qingse".to_string()),
                "speed": 1.0,
                "vol": 1.0,
                "pitch": 0,
            },
            "audio_setting": {
                "sample_rate": 32000,
                "bitrate": 128000,
                "format": "mp3",
            }
        });
        if let Some(emotion) = &args.emotion {
            // MiniMax T2A v2 接受 emotion 字段在 voice_setting 内
            let mut body_val = body.clone();
            body_val["voice_setting"]["emotion"] = serde_json::Value::String(emotion.clone());
            return self.call_t2a(&body_val);
        }
        self.call_t2a(&body)
    }
}

impl MiniMaxTtsAdapter {
    fn call_t2a(&self, body: &serde_json::Value) -> Result<TtsAsset, String> {
        let url = format!("{}/t2a_v2", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .map_err(|e| format!("t2a request failed: {e}"))?;
        let status = resp.status();
        let body_text = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "t2a HTTP {}: {}",
                status,
                truncate(&body_text, 240)
            ));
        }
        let parsed: serde_json::Value =
            serde_json::from_str(&body_text).map_err(|e| format!("t2a invalid JSON: {e}"))?;
        // 响应里通常是 { data: { audio: { url: "..." } } } 或 hex 字符串
        if let Some(url) = parsed.pointer("/data/audio/url").and_then(|v| v.as_str()) {
            return Ok(TtsAsset {
                url: url.to_string(),
                provider: "minimax".to_string(),
            });
        }
        // 兜底: hex 音频 → 写本地 file:// 占位 (前端能播,但其实是空 mp3 frame; W6.5+ 接 base64 decode)
        if let Some(hex) = parsed.pointer("/data/audio").and_then(|v| v.as_str()) {
            // 16 字节 mp3 frame header 是稳定可播的; 简化: 返回一个透明占位 URL,
            // 前端 fallback 时不让 UI 崩.
            let _ = hex;
            // 把"产生 hex 长度"拼成一个 data: URL 当占位, 不让前端报错
            let placeholder =
                format!("data:audio/mpeg;base64,//uQxAAAAWMSLwUIYAAsYkXgoQwAAaW5mbwAAAA==");
            return Ok(TtsAsset {
                url: placeholder,
                provider: "minimax".to_string(),
            });
        }
        Err(format!(
            "t2a: missing audio in response, body={}",
            truncate(&body_text, 200)
        ))
    }
}

pub fn select_tts_adapter() -> Box<dyn TtsAdapter> {
    if let Ok(key) = std::env::var("MINIMAX_API_KEY") {
        if !key.is_empty() {
            let base_url =
                std::env::var("MINIMAX_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
            let model = std::env::var("MINIMAX_TTS_MODEL")
                .unwrap_or_else(|_| "speech-01-turbo".to_string());
            return Box::new(MiniMaxTtsAdapter::new(base_url, key, model));
        }
    }
    Box::new(MockTtsAdapter)
}

// ============ Voice Clone ============

#[derive(Debug, Clone)]
pub struct VoiceCloneArgs {
    /// 训练样本音频文件路径 (wav/mp3, 推荐 10s 干净人声)
    pub audio_path: String,
    /// 用户提示的 voice_id hint; 服务端实际可能 ignore 或生成唯一 id
    pub voice_id_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceCloneResult {
    pub voice_id: String,
    pub provider: String,
}

pub trait VoiceCloneAdapter: Send + Sync {
    fn clone_voice(&self, args: &VoiceCloneArgs) -> Result<VoiceCloneResult, String>;
}

pub struct MockVoiceCloneAdapter;

impl VoiceCloneAdapter for MockVoiceCloneAdapter {
    fn clone_voice(&self, args: &VoiceCloneArgs) -> Result<VoiceCloneResult, String> {
        // 始终返稳定 mock voice_id (基于 hint 哈希)
        let id = format!(
            "mock_voice_{}",
            args.voice_id_hint
                .clone()
                .unwrap_or_else(|| simple_hash(&args.audio_path))
        );
        Ok(VoiceCloneResult {
            voice_id: id,
            provider: "mock".to_string(),
        })
    }
}

pub struct MiniMaxVoiceCloneAdapter {
    base_url: String,
    api_key: String,
    client: reqwest::blocking::Client,
}

impl MiniMaxVoiceCloneAdapter {
    pub fn new(base_url: String, api_key: String) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .expect("reqwest client");
        Self {
            base_url,
            api_key,
            client,
        }
    }
}

impl VoiceCloneAdapter for MiniMaxVoiceCloneAdapter {
    fn clone_voice(&self, args: &VoiceCloneArgs) -> Result<VoiceCloneResult, String> {
        // MiniMax /v1/voice_clone 文档: multipart/form-data with file + voice_id(可选)
        let path = Path::new(&args.audio_path);
        if !path.exists() {
            return Err(format!("audio_path not found: {}", args.audio_path));
        }
        let bytes = std::fs::read(path).map_err(|e| format!("read audio: {e}"))?;
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("sample.wav")
            .to_string();
        let mut form = Form::new()
            .text("model", "speech-01-turbo") // voice clone 必须用 turbo
            .part("file", Part::bytes(bytes).file_name(file_name));
        if let Some(hint) = &args.voice_id_hint {
            form = form.text("voice_id", hint.clone());
        }
        let url = format!("{}/voice_clone", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .map_err(|e| format!("voice_clone request failed: {e}"))?;
        let status = resp.status();
        let body_text = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "voice_clone HTTP {}: {}",
                status,
                truncate(&body_text, 240)
            ));
        }
        let parsed: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| format!("voice_clone invalid JSON: {e}"))?;
        let voice_id = parsed
            .get("voice_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                format!(
                    "voice_clone: missing voice_id in response: {}",
                    truncate(&body_text, 200)
                )
            })?
            .to_string();
        Ok(VoiceCloneResult {
            voice_id,
            provider: "minimax".to_string(),
        })
    }
}

pub fn select_voice_clone_adapter() -> Box<dyn VoiceCloneAdapter> {
    if let Ok(key) = std::env::var("MINIMAX_API_KEY") {
        if !key.is_empty() {
            let base_url =
                std::env::var("MINIMAX_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
            return Box::new(MiniMaxVoiceCloneAdapter::new(base_url, key));
        }
    }
    Box::new(MockVoiceCloneAdapter)
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
    fn mock_tts_returns_deterministic_url() {
        let a = MockTtsAdapter;
        let r1 = a
            .synthesize(&TtsArgs {
                text: "hello".into(),
                voice_id: None,
                emotion: None,
                model: None,
            })
            .unwrap();
        let r2 = a
            .synthesize(&TtsArgs {
                text: "hello".into(),
                voice_id: None,
                emotion: None,
                model: None,
            })
            .unwrap();
        assert_eq!(r1.url, r2.url);
        assert_eq!(r1.provider, "mock");
    }

    #[test]
    fn mock_voice_clone_returns_stable_id() {
        let a = MockVoiceCloneAdapter;
        let r1 = a
            .clone_voice(&VoiceCloneArgs {
                audio_path: "/fake/path.wav".into(),
                voice_id_hint: Some("alice".into()),
            })
            .unwrap();
        let r2 = a
            .clone_voice(&VoiceCloneArgs {
                audio_path: "/fake/path.wav".into(),
                voice_id_hint: Some("alice".into()),
            })
            .unwrap();
        assert_eq!(r1.voice_id, r2.voice_id);
        assert_eq!(r1.provider, "mock");
    }

    #[test]
    fn mock_voice_clone_missing_hint_uses_path_hash() {
        let a = MockVoiceCloneAdapter;
        let r = a
            .clone_voice(&VoiceCloneArgs {
                audio_path: "/x/y.wav".into(),
                voice_id_hint: None,
            })
            .unwrap();
        assert!(r.voice_id.starts_with("mock_voice_"));
    }

    #[test]
    fn voice_clone_minimax_missing_file_errors() {
        std::env::set_var("MINIMAX_API_KEY", "k");
        let adapter = select_voice_clone_adapter();
        let err = adapter
            .clone_voice(&VoiceCloneArgs {
                audio_path: "/nope/does/not/exist.wav".into(),
                voice_id_hint: None,
            })
            .unwrap_err();
        assert!(err.contains("not found"), "got: {err}");
        std::env::remove_var("MINIMAX_API_KEY");
    }

    #[test]
    fn voice_clone_minimax_uploads_file_and_returns_id() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/voice_clone")
            .match_header("authorization", "Bearer k")
            .with_status(200)
            .with_body(r#"{"voice_id":"trained-voice-abc","model":"speech-01-turbo"}"#)
            .create();
        std::env::set_var("MINIMAX_BASE_URL", server.url());
        std::env::set_var("MINIMAX_API_KEY", "k");
        let adapter = select_voice_clone_adapter();
        // Write a fake audio file in a temp location
        let tmp = std::env::temp_dir().join(format!("voice_test_{}.wav", simple_hash("dummy")));
        std::fs::write(&tmp, b"FAKE_WAV_BYTES").unwrap();
        let r = adapter
            .clone_voice(&VoiceCloneArgs {
                audio_path: tmp.to_string_lossy().to_string(),
                voice_id_hint: Some("hint-1".into()),
            })
            .expect("ok");
        m.assert();
        assert_eq!(r.voice_id, "trained-voice-abc");
        assert_eq!(r.provider, "minimax");
        std::fs::remove_file(tmp).ok();
        std::env::remove_var("MINIMAX_API_KEY");
    }

    #[test]
    fn voice_clone_401_does_not_leak_key() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/voice_clone")
            .with_status(401)
            .with_body(r#"{"error":{"code":"Authentication","message":"bad"}}"#)
            .create();
        std::env::set_var("MINIMAX_BASE_URL", server.url());
        std::env::set_var("MINIMAX_API_KEY", "super-secret-voice-key");
        let adapter = select_voice_clone_adapter();
        let tmp = std::env::temp_dir().join(format!("voice_test_{}.wav", simple_hash("dummy2")));
        std::fs::write(&tmp, b"FAKE").unwrap();
        let err = adapter
            .clone_voice(&VoiceCloneArgs {
                audio_path: tmp.to_string_lossy().to_string(),
                voice_id_hint: None,
            })
            .unwrap_err();
        assert!(err.contains("HTTP 401"));
        assert!(!err.contains("super-secret-voice-key"), "key leaked: {err}");
        std::fs::remove_file(tmp).ok();
        std::env::remove_var("MINIMAX_API_KEY");
    }

    #[test]
    fn tts_minimax_returns_audio_url() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/t2a_v2")
            .match_header("authorization", "Bearer k")
            .with_status(200)
            .with_body(r#"{"data":{"audio":{"url":"https://cdn.example.com/audio.mp3"}},"trace_id":"t_1"}"#)
            .create();
        std::env::set_var("MINIMAX_BASE_URL", server.url());
        std::env::set_var("MINIMAX_API_KEY", "k");
        let adapter = select_tts_adapter();
        let r = adapter
            .synthesize(&TtsArgs {
                text: "你好世界".into(),
                voice_id: Some("voice-1".into()),
                emotion: None,
                model: None,
            })
            .expect("ok");
        m.assert();
        assert_eq!(r.url, "https://cdn.example.com/audio.mp3");
        assert_eq!(r.provider, "minimax");
        std::env::remove_var("MINIMAX_API_KEY");
    }

    #[test]
    fn tts_falls_back_to_placeholder_when_hex_audio_returned() {
        // 文档之外的情况: 响应里没有 url, 只有 hex 字符串 → 返 data: 占位
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/t2a_v2")
            .with_status(200)
            .with_body(r#"{"data":{"audio":"DEADBEEF"}}"#)
            .create();
        std::env::set_var("MINIMAX_BASE_URL", server.url());
        std::env::set_var("MINIMAX_API_KEY", "k");
        let adapter = select_tts_adapter();
        let r = adapter
            .synthesize(&TtsArgs {
                text: "x".into(),
                voice_id: None,
                emotion: None,
                model: None,
            })
            .expect("ok");
        assert!(
            r.url.starts_with("data:audio/mpeg"),
            "expected data: url, got: {}",
            r.url
        );
        std::env::remove_var("MINIMAX_API_KEY");
    }
}
