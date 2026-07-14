// 图像生成 adapter (W6 C1) — MiniMax image-01 (同步版)
//
// 文档权威来源: MiniMax API Reference (image_generation endpoint).
// POST https://api.minimaxi.com/v1/image_generation
//   body: { "model": "image-01", "prompt": "...", "aspect_ratio": "1:1|16:9|9:16|4:3|3:4", "response_format": "url|base64", "n": 1 }
//   header: Authorization: Bearer <key>
// 响应: { "data": [ { "url": "...", "b64_json": "..." } ], "created": ts, ... }
//
// 行为对齐 video_adapter / MockAdapter 模式:
// - ImageAdapter trait 抽象; MiniMaxImage / MockImage 两种实现
// - select_image_adapter() 按 env 选 provider: MINIMAX_API_KEY → MiniMax; 否则 Mock
// - Mock 仍返回 picsum 占位 (向后兼容), W6 完成后可去掉
//
// 同步调用 (image-01 实际 3-8s 出图, 不需要 async task + polling 复杂度);
// 这样能保留和 video_adapter.rs 一致的 reqwest::blocking 用法.

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ImageGenArgs {
    /// 主提示词: 要画什么
    pub prompt: String,
    /// 画面比例, 默认 "1:1" (头像/角色立绘); Seedance 取 16:9 → 改用 "16:9"
    pub aspect_ratio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageAsset {
    pub url: String,
    pub provider: String, // "minimax" | "mock"
    pub provider_task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

pub trait ImageAdapter: Send + Sync {
    fn provider_name(&self) -> &'static str;
    fn generate(&self, args: &ImageGenArgs) -> Result<ImageAsset, String>;
}

// ============ Mock ============

pub struct MockImageAdapter;

impl ImageAdapter for MockImageAdapter {
    fn provider_name(&self) -> &'static str {
        "mock"
    }
    fn generate(&self, args: &ImageGenArgs) -> Result<ImageAsset, String> {
        // 维持旧 picsum 行为确定性占位图 (种子 = prompt 哈希)
        let seed = simple_hash(&args.prompt);
        let url = format!("https://picsum.photos/seed/{seed}/1024/576");
        Ok(ImageAsset {
            url,
            provider: "mock".to_string(),
            provider_task_id: format!("mock_img_{}", simple_hash(&args.prompt)),
            model: None,
        })
    }
}

// ============ MiniMax ============

const DEFAULT_MINIMAX_BASE_URL: &str = "https://api.minimaxi.com/v1";
const DEFAULT_MINIMAX_MODEL: &str = "image-01";
const HTTP_TIMEOUT_SECS: u64 = 30;

pub struct MiniMaxImageAdapter {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl MiniMaxImageAdapter {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .build()
            .expect("reqwest client should build");
        Self {
            base_url,
            api_key,
            model,
            client,
        }
    }

    fn build_request_body(&self, args: &ImageGenArgs) -> serde_json::Value {
        serde_json::json!({
            "model": &self.model,
            "prompt": args.prompt,
            "aspect_ratio": args.aspect_ratio.clone().unwrap_or_else(|| "1:1".to_string()),
            "response_format": "url",
            "n": 1,
        })
    }
}

impl ImageAdapter for MiniMaxImageAdapter {
    fn provider_name(&self) -> &'static str {
        "minimax"
    }

    fn generate(&self, args: &ImageGenArgs) -> Result<ImageAsset, String> {
        let url = format!("{}/image_generation", self.base_url);
        let body = self.build_request_body(args);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| format!("image_generation request failed: {e}"))?;
        let status = resp.status();
        let body_text = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "image_generation HTTP {}: {}",
                status,
                truncate(&body_text, 240)
            ));
        }
        let parsed: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| format!("image_generation: invalid JSON: {e}"))?;
        // data 是数组 (n 决定长度), 我们只要第一张
        let data_url = parsed
            .get("data")
            .and_then(|d| d.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("url"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                format!(
                    "image_generation: missing data[0].url, body={}",
                    truncate(&body_text, 200)
                )
            })?;
        let task_id = parsed
            .get("created")
            .map(|v| v.to_string())
            .unwrap_or_else(|| format!("img-{}", simple_hash(&args.prompt)));
        Ok(ImageAsset {
            url: data_url.to_string(),
            provider: "minimax".to_string(),
            provider_task_id: task_id,
            model: Some(self.model.clone()),
        })
    }
}

// ============ Factory ============

pub struct SelectedImageAdapter {
    pub adapter: Box<dyn ImageAdapter>,
    pub source: String, // "minimax" | "mock"
}

pub fn select_image_adapter() -> SelectedImageAdapter {
    if let Ok(key) = std::env::var("MINIMAX_API_KEY") {
        if !key.is_empty() {
            let base_url = std::env::var("MINIMAX_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_MINIMAX_BASE_URL.to_string());
            let model = std::env::var("MINIMAX_IMAGE_MODEL")
                .unwrap_or_else(|_| DEFAULT_MINIMAX_MODEL.to_string());
            return SelectedImageAdapter {
                adapter: Box::new(MiniMaxImageAdapter::new(base_url, key, model)),
                source: "minimax".to_string(),
            };
        }
    }
    SelectedImageAdapter {
        adapter: Box::new(MockImageAdapter),
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
    fn mock_returns_picsum_url() {
        let a = MockImageAdapter;
        let out = a
            .generate(&ImageGenArgs {
                prompt: "hello".into(),
                aspect_ratio: None,
            })
            .unwrap();
        assert_eq!(out.provider, "mock");
        assert!(out.url.contains("picsum.photos/seed/"));
    }

    #[test]
    fn select_mock_by_default() {
        std::env::remove_var("MINIMAX_API_KEY");
        let s = select_image_adapter();
        assert_eq!(s.source, "mock");
    }

    #[test]
    fn select_minimax_when_key_set() {
        std::env::set_var("MINIMAX_API_KEY", "test-key");
        let s = select_image_adapter();
        assert_eq!(s.source, "minimax");
        std::env::remove_var("MINIMAX_API_KEY");
    }

    #[test]
    fn minimax_request_body_shape() {
        let a = MiniMaxImageAdapter::new("http://unused".into(), "k".into(), "image-01".into());
        let body = a.build_request_body(&ImageGenArgs {
            prompt: "kitten".into(),
            aspect_ratio: Some("16:9".into()),
        });
        assert_eq!(body["model"], "image-01");
        assert_eq!(body["prompt"], "kitten");
        assert_eq!(body["aspect_ratio"], "16:9");
        assert_eq!(body["response_format"], "url");
        assert_eq!(body["n"], 1);
    }

    #[test]
    fn minimax_default_aspect_is_1x1() {
        let a = MiniMaxImageAdapter::new("http://unused".into(), "k".into(), "image-01".into());
        let body = a.build_request_body(&ImageGenArgs {
            prompt: "x".into(),
            aspect_ratio: None,
        });
        assert_eq!(body["aspect_ratio"], "1:1");
    }

    #[test]
    fn minimax_post_returns_image_url() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("POST", "/image_generation")
            .match_header("authorization", "Bearer k")
            .with_status(200)
            .with_body(r#"{"data":[{"url":"https://cdn.example.com/abc.png"}],"created":1234567}"#)
            .create();
        let a = MiniMaxImageAdapter::new(server.url(), "k".into(), "image-01".into());
        let out = a
            .generate(&ImageGenArgs {
                prompt: "kitten".into(),
                aspect_ratio: None,
            })
            .expect("ok");
        m.assert();
        assert_eq!(out.url, "https://cdn.example.com/abc.png");
        assert_eq!(out.provider, "minimax");
        assert_eq!(out.model.as_deref(), Some("image-01"));
    }

    #[test]
    fn minimax_401_does_not_leak_key() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/image_generation")
            .with_status(401)
            .with_body(r#"{"error":{"code":"Authentication","message":"bad key"}}"#)
            .create();
        let a =
            MiniMaxImageAdapter::new(server.url(), "super-secret-key".into(), "image-01".into());
        let err = a
            .generate(&ImageGenArgs {
                prompt: "x".into(),
                aspect_ratio: None,
            })
            .unwrap_err();
        assert!(err.contains("HTTP 401"));
        assert!(!err.contains("super-secret-key"), "key leaked: {err}");
    }

    #[test]
    fn minimax_missing_data_url_surfaces_error() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/image_generation")
            .with_status(200)
            .with_body(r#"{"data":[]}"#) // 空 data 数组
            .create();
        let a = MiniMaxImageAdapter::new(server.url(), "k".into(), "image-01".into());
        let err = a
            .generate(&ImageGenArgs {
                prompt: "x".into(),
                aspect_ratio: None,
            })
            .unwrap_err();
        assert!(err.contains("missing"), "got: {err}");
    }
}
