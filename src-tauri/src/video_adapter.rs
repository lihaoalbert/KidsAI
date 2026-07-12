// Video adapter abstraction（W? — Seedance 接入）
// 模型 → 图生视频的 provider 抽象。当前实现：
//   - VolcanoArkVideoAdapter：火山方舟 Seedance（doubao-seedance-2-0-* / fast / mini）
//   - MockVideoAdapter：保留原 w3schools 示例视频，用于无 key 环境
//
// 选择顺序（看 env）：
//   SEEDANCE_API_KEY（Bearer）→ VolcanoArk
//   否则                            → Mock
//
// 用户持有多套 Seedance 端点（2.0 / 2.0-fast / 2.0-mini），通过 SEEDANCE_MODEL 切换。
// 三套端点的 API surface 一致，所以三个 variant 共用同一个 adapter，只换 model 名。
//
// API spec 权威来源（SDK 源码）：
//   https://github.com/volcengine/volcengine-python-sdk/blob/master/
//     volcenginesdkarkruntime/resources/content_generation/tasks.py
//   volcenginesdkarkruntime/types/content_generation/content_generation_task.py
//
// 关键约束（来自 SDK 源码 + 官方 curl 示例，不要轻易改）：
//   - POST body：model + content[] + (ratio/duration/resolution/generate_audio/watermark/...)
//     **全部字段都在顶层，没有 `parameters` 包装对象**
//   - 官方 curl 示例里 resolution 几乎都不写、generate_audio 仅在需要时写 →
//     客户端 None 时不发送这些字段，让服务端走默认值
//   - content[] 元素：text 类型无 role；image_url 是 conditional role（None=首帧 i2v 不写 role，
//     Some("first_frame")/Some("last_frame")=首尾帧模式，Some("reference_image")=多模态参考）
//   - GET 响应 status 枚举：queued | running | succeeded | failed | cancelled
//   - 成功响应：content.video_url；失败：error.code + error.message（结构化对象）
//   - DELETE：/contents/generations/tasks/{id}（取消/删除任务）

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct VideoGenArgs {
    /// 触发本轮生成的运动描述（来自 LLM 工具调用的 motion / prompt）
    pub prompt: String,
    /// 可选的首帧图 URL（来自 LLM 工具调用的 image_url）。支持 http(s) 和 data:base64。
    pub image_url: Option<String>,
    /// image_url 的 role 值：
    ///   - None（默认）= 单图 i2v 首帧模式，不带 role 字段（见官方示例 1、3）
    ///   - Some("first_frame") / Some("last_frame") = 首尾帧模式
    ///   - Some("reference_image") = 多模态参考模式（SDK multimodal 示例）
    /// 不设置时，整段 content[] 也不会有 role 字段，与官方 curl 一致
    pub image_role: Option<String>,
    /// 时长（秒），默认 5。顶层字段
    pub duration_seconds: Option<u32>,
    /// 画面比例，默认 "16:9"（也合法值 "adaptive"）
    pub ratio: Option<String>,
    /// 分辨率（"480p" / "720p" / "1080p"）。**官方示例都不带**，设为 None 时省略
    pub resolution: Option<String>,
    /// 是否生成有声视频。**官方示例不一定带**，设为 None 时省略
    pub generate_audio: Option<bool>,
    /// Per-call model override（v1 导演流程: 试拍用 mini、定稿用 2.0）。
    /// None 时使用 adapter 实例的默认 model（来自 SEEDANCE_MODEL env）。
    pub model: Option<String>,
    /// Per-call seed（固定随机种子,角色一致性的杠杆之一）。
    /// None 时不发送该字段。
    pub seed: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoAsset {
    pub url: String,
    /// 可选的封面图（ARK 任务成功返回里给 cover_image_url，没有则回退到 last_frame_url）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    /// provider 内部任务 id（ARK 的 task id），便于日志排障
    pub provider_task_id: String,
    /// "ark" / "mock"
    pub provider: String,
    /// 用的具体模型（仅 ark；mock 为空）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

pub trait VideoAdapter: Send + Sync {
    fn provider_name(&self) -> &'static str;
    fn generate(&self, args: &VideoGenArgs) -> Result<VideoAsset, String>;
    /// 取消一个进行中的任务。mock 永远返回 ok。
    fn cancel(&self, _task_id: &str) -> Result<(), String> {
        Ok(())
    }
}

// ============ Mock ============

pub struct MockVideoAdapter;

impl VideoAdapter for MockVideoAdapter {
    fn provider_name(&self) -> &'static str {
        "mock"
    }
    fn generate(&self, args: &VideoGenArgs) -> Result<VideoAsset, String> {
        // 行为对齐原 ImageToVideoTool：返回 w3schools 示例 mp4
        Ok(VideoAsset {
            url: "https://www.w3schools.com/html/mov_bbb.mp4".to_string(),
            thumbnail_url: Some("https://picsum.photos/seed/kidsaivid/640/360".to_string()),
            provider_task_id: format!("mock_{}", simple_hash(&args.prompt)),
            provider: "mock".to_string(),
            model: None,
        })
    }
}

// ============ Volcano ARK ============

const DEFAULT_ARK_BASE_URL: &str = "https://ark.cn-beijing.volces.com/api/v3";
const DEFAULT_ARK_MODEL: &str = "doubao-seedance-2-0-260128";
const POLL_INTERVAL_MS: u64 = 1500;
const POLL_MAX_ATTEMPTS: u32 = 120; // 最多等 ~3 分钟
const HTTP_TIMEOUT_SECS: u64 = 30;

pub struct VolcanoArkVideoAdapter {
    base_url: String,
    api_key: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl VolcanoArkVideoAdapter {
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

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    /// 构造符合官方 SDK 规范的 POST body：
    /// - content[] 顶层、扁平字段（无 parameters 包装）
    /// - 字段缺省时不发送（resolution / generate_audio 都是 optional）
    /// - image_url 的 role 是 conditional：None 时不写 role 字段
    fn build_request_body(&self, args: &VideoGenArgs) -> serde_json::Value {
        let mut content: Vec<serde_json::Value> = Vec::new();
        content.push(serde_json::json!({
            "type": "text",
            "text": args.prompt,
        }));
        if let Some(url) = &args.image_url {
            let mut part = serde_json::json!({
                "type": "image_url",
                "image_url": { "url": url },
            });
            if let Some(role) = &args.image_role {
                part["role"] = serde_json::Value::String(role.clone());
            }
            content.push(part);
        }

        let mut body = serde_json::json!({
            "model": args.model.as_deref().unwrap_or(&self.model),
            "content": content,
            "ratio": args.ratio.clone().unwrap_or_else(|| "16:9".to_string()),
            "duration": args.duration_seconds.unwrap_or(5),
        });
        // 可选字段：None 时不写（避免覆盖服务端默认值）
        if let Some(res) = &args.resolution {
            body["resolution"] = serde_json::Value::String(res.clone());
        }
        if let Some(audio) = args.generate_audio {
            body["generate_audio"] = serde_json::Value::Bool(audio);
        }
        if let Some(s) = args.seed {
            body["seed"] = serde_json::Value::Number(s.into());
        }
        body
    }
}

impl VideoAdapter for VolcanoArkVideoAdapter {
    fn provider_name(&self) -> &'static str {
        "ark"
    }

    fn generate(&self, args: &VideoGenArgs) -> Result<VideoAsset, String> {
        // 1. POST 创建任务
        let body = self.build_request_body(args);
        let create_url = format!("{}/contents/generations/tasks", self.base_url);
        let resp = self
            .client
            .post(&create_url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| format!("seedance create request failed: {e}"))?;

        let status = resp.status();
        let body_text = resp.text().unwrap_or_default();
        if !status.is_success() {
            // 安全：不要把 api_key 回显在错误里。HTTP body 通常是 {"error":{"code":..,"message":..}}
            return Err(format!(
                "seedance create failed (HTTP {}): {}",
                status,
                truncate(&body_text, 240)
            ));
        }

        let parsed: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| format!("seedance create: invalid JSON: {e}"))?;
        let task_id = parsed
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "seedance create: missing 'id' in response".to_string())?
            .to_string();

        // 2. 轮询 GET /tasks/{id}
        self.poll_until_done(&task_id)
    }

    fn cancel(&self, task_id: &str) -> Result<(), String> {
        let url = format!("{}/contents/generations/tasks/{}", self.base_url, task_id);
        let resp = self
            .client
            .delete(&url)
            .header("Authorization", self.auth_header())
            .send()
            .map_err(|e| format!("seedance cancel request failed: {e}"))?;
        let status = resp.status();
        let body_text = resp.text().unwrap_or_default();
        if status.is_success() || status.as_u16() == 404 {
            // 204 No Content / 200 OK / 404 (already cancelled/deleted) 都视为成功
            return Ok(());
        }
        Err(format!(
            "seedance cancel HTTP {}: {}",
            status,
            truncate(&body_text, 240)
        ))
    }
}

impl VolcanoArkVideoAdapter {
    /// 同步轮询直到任务结束。SDK 源码注释列出的 status 枚举：
    /// queued / running / succeeded / failed / cancelled
    fn poll_until_done(&self, task_id: &str) -> Result<VideoAsset, String> {
        let poll_url = format!("{}/contents/generations/tasks/{}", self.base_url, task_id);
        for attempt in 0..POLL_MAX_ATTEMPTS {
            std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            let poll = self
                .client
                .get(&poll_url)
                .header("Authorization", self.auth_header())
                .send()
                .map_err(|e| format!("seedance poll failed: {e}"))?;

            let pstatus = poll.status();
            let pbody = poll.text().unwrap_or_default();
            if !pstatus.is_success() {
                return Err(format!(
                    "seedance poll HTTP {}: {}",
                    pstatus,
                    truncate(&pbody, 240)
                ));
            }
            let pval: serde_json::Value = serde_json::from_str(&pbody)
                .map_err(|e| format!("seedance poll: invalid JSON: {e}"))?;
            let task_status = pval
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            match task_status {
                "succeeded" => {
                    // content.video_url（SDK 源码 cgt.py 第 30 行确认）
                    let url = pval
                        .get("content")
                        .and_then(|c| c.get("video_url"))
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            "seedance succeeded but missing content.video_url".to_string()
                        })?
                        .to_string();
                    // 封面：优先 last_frame_url（SDK 注释：URL of the last frame），再退回 cover_image_url
                    let thumb = pval
                        .get("content")
                        .and_then(|c| c.get("last_frame_url"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| {
                            pval.get("content")
                                .and_then(|c| c.get("cover_image_url"))
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        });
                    return Ok(VideoAsset {
                        url,
                        thumbnail_url: thumb,
                        provider_task_id: task_id.to_string(),
                        provider: "ark".to_string(),
                        model: Some(self.model.clone()),
                    });
                }
                "failed" => {
                    // error.code + error.message（SDK cgt.py 第 40-46 行）
                    let code = pval
                        .get("error")
                        .and_then(|e| e.get("code"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let msg = pval
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    return Err(format!(
                        "seedance task failed (code={}): {}",
                        code,
                        if msg.is_empty() { truncate(&pbody, 200) } else { msg.to_string() }
                    ));
                }
                "cancelled" => {
                    return Err("seedance task cancelled".to_string());
                }
                "queued" | "running" => {
                    if attempt == POLL_MAX_ATTEMPTS - 1 {
                        return Err(format!(
                            "seedance task still {} after {} polls",
                            task_status, POLL_MAX_ATTEMPTS
                        ));
                    }
                    // 继续轮询
                }
                "" => {
                    // 响应里连 status 都没有 — 协议被破坏
                    return Err(format!(
                        "seedance poll: missing 'status' in response: {}",
                        truncate(&pbody, 200)
                    ));
                }
                other => {
                    // 未文档化的 status — 按 SDK 注释这不应该发生；保守起见继续轮询
                    if attempt == POLL_MAX_ATTEMPTS - 1 {
                        return Err(format!(
                            "seedance task unknown status '{}' after {} polls",
                            other, POLL_MAX_ATTEMPTS
                        ));
                    }
                }
            }
        }
        Err(format!(
            "seedance task did not complete in {} polls",
            POLL_MAX_ATTEMPTS
        ))
    }
}

// ============ Factory ============

pub struct SelectedVideoAdapter {
    pub adapter: Box<dyn VideoAdapter>,
    pub source: String, // "ark" / "mock"
}

/// 按 env 选择一个 video adapter
///
/// SEEDANCE_API_KEY 命中 → VolcanoArkVideoAdapter；
/// 否则 → MockVideoAdapter
pub fn select_video_adapter() -> SelectedVideoAdapter {
    // 生产入口负责 dotenv()；测试可以在外部覆盖 env 后再调本函数。
    if let Ok(key) = std::env::var("SEEDANCE_API_KEY") {
        if !key.is_empty() {
            let base_url = std::env::var("SEEDANCE_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_ARK_BASE_URL.to_string());
            let model = std::env::var("SEEDANCE_MODEL")
                .unwrap_or_else(|_| DEFAULT_ARK_MODEL.to_string());
            return SelectedVideoAdapter {
                adapter: Box::new(VolcanoArkVideoAdapter::new(base_url, key, model)),
                source: "ark".to_string(),
            };
        }
    }

    SelectedVideoAdapter {
        adapter: Box::new(MockVideoAdapter),
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
    fn mock_returns_w3schools_url() {
        let a = MockVideoAdapter;
        let out = a
            .generate(&VideoGenArgs {
                prompt: "hello".into(),
                image_url: None,
                image_role: None,
                duration_seconds: Some(5),
                ratio: None,
                resolution: None,
                generate_audio: None,
                model: None,
                seed: None,
            })
            .unwrap();
        assert_eq!(out.provider, "mock");
        assert!(out.url.contains("w3schools.com"));
    }

    #[test]
    fn mock_provider_name() {
        assert_eq!(MockVideoAdapter.provider_name(), "mock");
    }

    #[test]
    fn mock_cancel_is_noop() {
        // mock 默认 trait impl 就返回 Ok(())
        let a = MockVideoAdapter;
        assert!(a.cancel("any-id").is_ok());
    }

    #[test]
    fn select_video_adapter_defaults_to_mock_when_no_env() {
        std::env::remove_var("SEEDANCE_API_KEY");
        let s = select_video_adapter();
        assert_eq!(s.source, "mock");
        assert_eq!(s.adapter.provider_name(), "mock");
    }

    #[test]
    fn select_video_adapter_picks_ark_when_key_present() {
        std::env::set_var("SEEDANCE_API_KEY", "test-fake-key");
        std::env::set_var("SEEDANCE_BASE_URL", "http://127.0.0.1:9999");
        std::env::set_var("SEEDANCE_MODEL", "doubao-seedance-2-0-fast-test");
        let s = select_video_adapter();
        assert_eq!(s.source, "ark");
        assert_eq!(s.adapter.provider_name(), "ark");
        std::env::remove_var("SEEDANCE_API_KEY");
    }

    #[test]
    fn truncate_string_short() {
        assert_eq!(truncate("abc", 10), "abc");
    }

    #[test]
    fn truncate_string_long() {
        let t = truncate(&"x".repeat(20), 5);
        assert_eq!(t, "xxxxx…");
    }

    /// 校验 POST body 形状（对照官方 curl 示例 1 — 有声视频-首帧）：
    /// - content[] 顶层、扁平字段（无 parameters 包装）
    /// - 单图 i2v 模式：image_url **不带 role 字段**
    /// - resolution 字段缺省时不发送
    /// - generate_audio 在 args.generate_audio = Some(true) 时发送
    #[test]
    fn ark_request_body_matches_i2v_first_frame_shape() {
        let mut server = mockito::Server::new();
        let create_mock = server
            .mock("POST", "/contents/generations/tasks")
            .match_body(mockito::Matcher::PartialJson(serde_json::json!({
                "model": "doubao-test",
                "content": [
                    {"type": "text", "text": "spin around"},
                    {
                        "type": "image_url",
                        "image_url": {"url": "https://e/i.jpg"}
                        // 注意：这里**没有** role 字段
                    }
                ],
                "ratio": "adaptive",
                "duration": 5,
                "generate_audio": true
                // 注意：这里**没有** resolution 字段
            })))
            .with_status(200)
            .with_body(r#"{"id":"task_x"}"#)
            .create();
        server
            .mock("GET", "/contents/generations/tasks/task_x")
            .with_status(200)
            .with_body(
                r#"{"id":"task_x","status":"succeeded","content":{"video_url":"https://e/v.mp4"}}"#,
            )
            .create();

        let adapter =
            VolcanoArkVideoAdapter::new(server.url(), "k".into(), "doubao-test".into());
        let asset = adapter
            .generate(&VideoGenArgs {
                prompt: "spin around".into(),
                image_url: Some("https://e/i.jpg".into()),
                image_role: None, // ← 不写 role 字段
                duration_seconds: Some(5),
                ratio: Some("adaptive".into()),
                resolution: None, // ← 不发 resolution
                generate_audio: Some(true), // ← 发 generate_audio: true
                model: None,
                seed: None,
            })
            .expect("ok");
        create_mock.assert();
        assert_eq!(asset.url, "https://e/v.mp4");
    }

    /// 校验首尾帧模式（对照官方 curl 示例 2）：
    /// image_url 带 role: "first_frame" / "last_frame"
    #[test]
    fn ark_request_body_first_last_frame_adds_roles() {
        // 这个测试只检查 build_request_body，不实际发 HTTP
        let adapter = VolcanoArkVideoAdapter::new(
            "http://unused".into(),
            "k".into(),
            "doubao-test".into(),
        );
        // 用一个有 first_frame 的 args 测 build_request_body（通过镜像 adapter 字段）
        let body = adapter.build_request_body(&VideoGenArgs {
            prompt: "go around".into(),
            image_url: Some("https://e/first.jpg".into()),
            image_role: Some("first_frame".into()),
            duration_seconds: Some(5),
            ratio: Some("adaptive".into()),
            resolution: None,
            generate_audio: None,
                model: None,
                seed: None,
        });
        assert_eq!(body["content"][1]["role"], "first_frame");
        assert_eq!(body["content"][1]["type"], "image_url");

        // last_frame 验证
        let body = adapter.build_request_body(&VideoGenArgs {
            prompt: "x".into(),
            image_url: Some("https://e/last.jpg".into()),
            image_role: Some("last_frame".into()),
            duration_seconds: Some(5),
            ratio: None,
            resolution: None,
            generate_audio: None,
                model: None,
                seed: None,
        });
        assert_eq!(body["content"][1]["role"], "last_frame");

        // reference_image 验证（多模态参考模式）
        let body = adapter.build_request_body(&VideoGenArgs {
            prompt: "x".into(),
            image_url: Some("https://e/ref.jpg".into()),
            image_role: Some("reference_image".into()),
            duration_seconds: Some(5),
            ratio: None,
            resolution: None,
            generate_audio: None,
                model: None,
                seed: None,
        });
        assert_eq!(body["content"][1]["role"], "reference_image");
    }

    /// 校验可选字段未设置时不发送：
    /// - resolution: None 不出现在 body
    /// - generate_audio: None 不出现在 body
    #[test]
    fn ark_request_body_omits_optional_fields_when_none() {
        let adapter = VolcanoArkVideoAdapter::new(
            "http://unused".into(),
            "k".into(),
            "doubao-test".into(),
        );
        let body = adapter.build_request_body(&VideoGenArgs {
            prompt: "x".into(),
            image_url: None,
            image_role: None,
            duration_seconds: Some(5),
            ratio: None,
            resolution: None,
            generate_audio: None,
                model: None,
                seed: None,
        });
        assert!(body.get("resolution").is_none(), "resolution should be absent");
        assert!(body.get("generate_audio").is_none(), "generate_audio should be absent");
        // ratio / duration 仍按默认值填
        assert_eq!(body["ratio"], "16:9");
        assert_eq!(body["duration"], 5);
    }

    /// 文本-only 请求（对照官方 curl 示例 4 — 文生视频）：content[] 只含 1 条 text，
    /// 不带 role、resolution、generate_audio
    #[test]
    fn ark_request_body_text_only_when_no_image() {
        let mut server = mockito::Server::new();
        let create_mock = server
            .mock("POST", "/contents/generations/tasks")
            .match_body(mockito::Matcher::PartialJson(serde_json::json!({
                "model": "doubao-test",
                "content": [{"type": "text", "text": "hello"}],
                "ratio": "16:9",
                "duration": 5
                // 注意：官方文生视频示例里 resolution / generate_audio 都不发
            })))
            .with_status(200)
            .with_body(r#"{"id":"t"}"#)
            .create();
        server
            .mock("GET", "/contents/generations/tasks/t")
            .with_status(200)
            .with_body(
                r#"{"id":"t","status":"succeeded","content":{"video_url":"https://e/v.mp4"}}"#,
            )
            .create();

        let adapter =
            VolcanoArkVideoAdapter::new(server.url(), "k".into(), "doubao-test".into());
        let _ = adapter
            .generate(&VideoGenArgs {
                prompt: "hello".into(),
                image_url: None,
                image_role: None,
                duration_seconds: None,
                ratio: None,
                resolution: None,
                generate_audio: None,
                model: None,
                seed: None,
            })
            .expect("ok");
        create_mock.assert();
    }

    /// 完整成功路径：POST 创建 → GET 第一次 running → GET 第二次 succeeded
    #[test]
    fn ark_polls_running_then_succeeds() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/contents/generations/tasks")
            .with_status(200)
            .with_body(r#"{"id":"task_abc"}"#)
            .create();
        // 两次 GET：第一次 running，第二次 succeeded
        let m1 = server
            .mock("GET", "/contents/generations/tasks/task_abc")
            .match_header("authorization", "Bearer k")
            .with_status(200)
            .with_body(r#"{"id":"task_abc","status":"running"}"#)
            .expect(1)
            .create();
        let m2 = server
            .mock("GET", "/contents/generations/tasks/task_abc")
            .with_status(200)
            .with_body(
                r#"{"id":"task_abc","status":"succeeded","content":{"video_url":"https://e/seedance.mp4","last_frame_url":"https://e/last.jpg"}}"#,
            )
            .expect(1)
            .create();

        let adapter =
            VolcanoArkVideoAdapter::new(server.url(), "k".into(), "doubao-test".into());
        let asset = adapter
            .generate(&VideoGenArgs {
                prompt: "dance".into(),
                image_url: None,
                image_role: None,
                duration_seconds: Some(5),
                ratio: None,
                resolution: None,
                generate_audio: None,
                model: None,
                seed: None,
            })
            .expect("ok");

        m1.assert();
        m2.assert();
        assert_eq!(asset.url, "https://e/seedance.mp4");
        assert_eq!(asset.thumbnail_url.as_deref(), Some("https://e/last.jpg"));
    }

    /// SDK cgt.py 第 64 行 status 注释列出的所有 5 个枚举值都覆盖
    #[test]
    fn ark_handles_queued_running_succeeded_failed_cancelled() {
        // queued + succeeded
        {
            let mut server = mockito::Server::new();
            server
                .mock("POST", "/contents/generations/tasks")
                .with_status(200)
                .with_body(r#"{"id":"t1"}"#)
                .create();
            let m1 = server
                .mock("GET", "/contents/generations/tasks/t1")
                .with_status(200)
                .with_body(r#"{"status":"queued"}"#)
                .expect(1)
                .create();
            let m2 = server
                .mock("GET", "/contents/generations/tasks/t1")
                .with_status(200)
                .with_body(r#"{"status":"succeeded","content":{"video_url":"https://e/v.mp4"}}"#)
                .expect(1)
                .create();
            let adapter = VolcanoArkVideoAdapter::new(
                server.url(),
                "k".into(),
                "doubao-test".into(),
            );
            let a = adapter
                .generate(&VideoGenArgs {
                    prompt: "x".into(),
                    image_url: None,
                image_role: None,
                    duration_seconds: None,
                    ratio: None,
                    resolution: None,
                    generate_audio: None,
                model: None,
                seed: None,
                })
                .expect("ok");
            assert_eq!(a.url, "https://e/v.mp4");
            m1.assert();
            m2.assert();
        }
        // failed with structured error
        {
            let mut server = mockito::Server::new();
            server
                .mock("POST", "/contents/generations/tasks")
                .with_status(200)
                .with_body(r#"{"id":"t2"}"#)
                .create();
            server
                .mock("GET", "/contents/generations/tasks/t2")
                .with_status(200)
                .with_body(
                    r#"{"status":"failed","error":{"code":"InvalidParameter","message":"ratio not supported"}}"#,
                )
                .create();
            let adapter = VolcanoArkVideoAdapter::new(
                server.url(),
                "test-key".into(),
                "doubao-test".into(),
            );
            let err = adapter
                .generate(&VideoGenArgs {
                    prompt: "x".into(),
                    image_url: None,
                image_role: None,
                    duration_seconds: None,
                    ratio: None,
                    resolution: None,
                    generate_audio: None,
                model: None,
                seed: None,
                })
                .unwrap_err();
            assert!(err.contains("InvalidParameter"), "got: {err}");
            assert!(err.contains("ratio not supported"), "got: {err}");
            assert!(!err.contains("test-key"), "API key leaked: {err}");
        }
        // cancelled
        {
            let mut server = mockito::Server::new();
            server
                .mock("POST", "/contents/generations/tasks")
                .with_status(200)
                .with_body(r#"{"id":"t3"}"#)
                .create();
            server
                .mock("GET", "/contents/generations/tasks/t3")
                .with_status(200)
                .with_body(r#"{"status":"cancelled"}"#)
                .create();
            let adapter = VolcanoArkVideoAdapter::new(
                server.url(),
                "k".into(),
                "doubao-test".into(),
            );
            let err = adapter
                .generate(&VideoGenArgs {
                    prompt: "x".into(),
                    image_url: None,
                image_role: None,
                    duration_seconds: None,
                    ratio: None,
                    resolution: None,
                    generate_audio: None,
                model: None,
                seed: None,
                })
                .unwrap_err();
            assert!(err.contains("cancelled"), "got: {err}");
        }
    }

    /// 取消任务：DELETE 返回 200/204 都视为成功
    #[test]
    fn ark_cancel_sends_delete_with_auth() {
        let mut server = mockito::Server::new();
        let m = server
            .mock("DELETE", "/contents/generations/tasks/task_to_cancel")
            .match_header("authorization", "Bearer k")
            .with_status(204)
            .create();

        let adapter =
            VolcanoArkVideoAdapter::new(server.url(), "k".into(), "doubao-test".into());
        adapter.cancel("task_to_cancel").expect("cancel ok");
        m.assert();
    }

    /// 取消失败：API key 不回显
    #[test]
    fn ark_cancel_401_does_not_leak_key() {
        let mut server = mockito::Server::new();
        server
            .mock("DELETE", "/contents/generations/tasks/task_x")
            .with_status(401)
            .with_body(r#"{"error":{"code":"Authentication","message":"bad key"}}"#)
            .create();

        let adapter = VolcanoArkVideoAdapter::new(
            server.url(),
            "super-secret".into(),
            "doubao-test".into(),
        );
        let err = adapter.cancel("task_x").unwrap_err();
        assert!(err.contains("HTTP 401"));
        assert!(!err.contains("super-secret"), "API key leaked: {err}");
    }

    /// 取消时任务已被删除（404）→ 也算成功
    #[test]
    fn ark_cancel_404_is_treated_as_success() {
        let mut server = mockito::Server::new();
        server
            .mock("DELETE", "/contents/generations/tasks/task_gone")
            .with_status(404)
            .create();

        let adapter =
            VolcanoArkVideoAdapter::new(server.url(), "k".into(), "doubao-test".into());
        adapter.cancel("task_gone").expect("cancel 404 ok");
    }

    /// POST 401：error 文本不含 api_key
    #[test]
    fn ark_http_401_error_does_not_leak_key() {
        let mut server = mockito::Server::new();
        server
            .mock("POST", "/contents/generations/tasks")
            .with_status(401)
            .with_body(r#"{"error":{"code":"Authentication","message":"invalid api key"}}"#)
            .create();

        let adapter = VolcanoArkVideoAdapter::new(
            server.url(),
            "super-secret-key".into(),
            "doubao-test".into(),
        );
        let err = adapter
            .generate(&VideoGenArgs {
                prompt: "x".into(),
                image_url: None,
                image_role: None,
                duration_seconds: None,
                ratio: None,
                resolution: None,
                generate_audio: None,
                model: None,
                seed: None,
            })
            .unwrap_err();
        assert!(err.contains("HTTP 401"));
        assert!(!err.contains("super-secret-key"), "API key leaked: {err}");
    }

    /// v1 导演流程:per-call model override 优先于 adapter 默认 model。
    /// adapter 默认 doubao-test;args 传 doubao-mini-override → POST body 用 override。
    #[test]
    fn per_call_model_override_overrides_default() {
        let mut server = mockito::Server::new();
        let create_mock = server
            .mock("POST", "/contents/generations/tasks")
            .match_body(mockito::Matcher::PartialJson(serde_json::json!({
                "model": "doubao-mini-override"  // ← 用 override,不用 adapter 默认
            })))
            .with_status(200)
            .with_body(r#"{"id":"task_ovr"}"#)
            .create();
        server
            .mock("GET", "/contents/generations/tasks/task_ovr")
            .with_status(200)
            .with_body(
                r#"{"id":"task_ovr","status":"succeeded","content":{"video_url":"https://e/ovr.mp4"}}"#,
            )
            .create();

        let adapter = VolcanoArkVideoAdapter::new(
            server.url(),
            "k".into(),
            "doubao-test".into(), // ← adapter 默认;会被 override 覆盖
        );
        let asset = adapter
            .generate(&VideoGenArgs {
                prompt: "x".into(),
                image_url: None,
                image_role: None,
                duration_seconds: None,
                ratio: None,
                resolution: None,
                generate_audio: None,
                model: Some("doubao-mini-override".into()), // ← override
                seed: None,
            })
            .expect("ok");
        create_mock.assert();
        assert_eq!(asset.url, "https://e/ovr.mp4");
    }

    /// seed 参数有值时出现在 POST body 顶层,无值时不出现(对照 resolution/generate_audio 的策略)。
    #[test]
    fn ark_request_body_seed_present_when_set_absent_when_none() {
        let adapter = VolcanoArkVideoAdapter::new(
            "http://unused".into(),
            "k".into(),
            "doubao-test".into(),
        );
        // 有 seed → 出现
        let body = adapter.build_request_body(&VideoGenArgs {
            prompt: "x".into(),
            image_url: None,
            image_role: None,
            duration_seconds: Some(5),
            ratio: None,
            resolution: None,
            generate_audio: None,
            model: None,
            seed: Some(42),
        });
        assert_eq!(body["seed"], 42, "seed should be present when set");
        // 无 seed → 不出现
        let body = adapter.build_request_body(&VideoGenArgs {
            prompt: "x".into(),
            image_url: None,
            image_role: None,
            duration_seconds: Some(5),
            ratio: None,
            resolution: None,
            generate_audio: None,
            model: None,
            seed: None,
        });
        assert!(body.get("seed").is_none(), "seed should be absent when None");
    }
}
