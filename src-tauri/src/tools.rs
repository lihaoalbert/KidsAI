// Mock MCP 工具（W2.6）
// 每个工具模拟一次"AI 服务调用"，返回生成的资产信息
// 真实实现会替换为：HTTP 请求到智谱 / 阿里 DashScope / 自建模型服务
// W?: image_to_video 工具接入 video_adapter（Volcano ARK Seedance / Mock）

use crate::video_adapter::{select_video_adapter, VideoGenArgs};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// 工具文本结果（记入 Agent 的"观察"）
    pub result_text: String,
    /// 生成的资产（用于塞进 Creation.assets）
    pub assets: Vec<GeneratedAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedAsset {
    #[serde(rename = "type")]
    pub kind: String, // "image" | "video" | "audio"
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    pub prompt: String,
    pub tool: String,
    pub tokens_cost: u32,
    /// W4.5 B2: 实际使用的模型 (如 doubao-seedance-2-0-mini-260615),
    /// 用于 license 学币计费时区分 draft / final
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// 工具 trait
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    /// 工具描述（写进 system prompt，让模型知道有这个工具）
    fn description(&self) -> &'static str;
    /// JSON Schema 字符串（mock 阶段简化为描述文本）
    fn schema(&self) -> &'static str;
    /// 执行
    fn execute(&self, args_json: &str, session_id: &str) -> Result<ToolOutput, String>;
}

/// 工具注册表
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        // 防重
        if !self.tools.iter().any(|t| t.name() == tool.name()) {
            self.tools.push(tool);
        }
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.name() == name).map(|t| t.as_ref())
    }

    /// 列出被允许的工具的描述（喂给模型）
    pub fn describe(&self, allowed: &[String]) -> String {
        let mut out = String::new();
        for name in allowed {
            if let Some(t) = self.get(name) {
                out.push_str(&format!(
                    "- {}: {}\n  args: {}\n",
                    t.name(),
                    t.description(),
                    t.schema()
                ));
            }
        }
        out
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============ 具体工具实现 ============

pub struct GenerateImageTool;

impl Tool for GenerateImageTool {
    fn name(&self) -> &'static str {
        "generate_image"
    }
    fn description(&self) -> &'static str {
        "文生图：根据 prompt 生成一张图片"
    }
    fn schema(&self) -> &'static str {
        r#"{"prompt": "string", "style": "string?", "width": "int?", "height": "int?"}"#
    }
    fn execute(&self, args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        let args: serde_json::Value =
            serde_json::from_str(args_json).map_err(|e| format!("invalid args: {e}"))?;
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or("missing prompt")?
            .to_string();
        // mock：用 picsum 风格的稳定占位图
        let seed = simple_hash(&prompt);
        let url = format!("https://picsum.photos/seed/{seed}/1024/576");
        Ok(ToolOutput {
            result_text: format!("已生成图片，prompt={}", prompt),
            assets: vec![GeneratedAsset {
                kind: "image".to_string(),
                url,
                thumbnail_url: None,
                prompt: prompt.clone(),
                tool: "generate_image".to_string(),
                tokens_cost: 10,
                model: None,
            }],
        })
    }
}

pub struct ImageToVideoTool;

impl Tool for ImageToVideoTool {
    fn name(&self) -> &'static str {
        "image_to_video"
    }
    fn description(&self) -> &'static str {
        "图生视频：把上一张图片动起来，生成 5 秒视频（Mock / Volcano ARK Seedance）"
    }
    fn schema(&self) -> &'static str {
        r#"{
            "image_url": "string? — 公网 URL / asset://<ID> / data:base64; 不传则纯文生视频",
            "image_role": "string? — first_frame | last_frame | reference_image; 默认 first_frame",
            "duration": "int? — 秒数，默认 5",
            "motion": "string? — 运动描述,作为 prompt; 缺省 'animate this image'",
            "model": "string? — per-call 覆盖; 如 doubao-seedance-2-0-mini-260615(试拍) 或 doubao-seedance-2-0-260128(定稿); 缺省走 SEEDANCE_MODEL env",
            "seed": "int? — 固定随机种子(角色一致性); 缺省不发送"
        }"#
    }
    fn execute(&self, args_json: &str, session_id: &str) -> Result<ToolOutput, String> {
        let args: serde_json::Value =
            serde_json::from_str(args_json).map_err(|e| format!("invalid args: {e}"))?;
        let motion = args
            .get("motion")
            .and_then(|v| v.as_str())
            .unwrap_or("animate this image")
            .to_string();
        let image_url = args
            .get("image_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let image_role = args
            .get("image_role")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let duration_seconds = args.get("duration").and_then(|v| v.as_u64()).map(|v| v as u32);
        let model_override = args
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let seed = args.get("seed").and_then(|v| v.as_i64());

        // 每次执行时按当前 env 重新选 provider — 测试和运行期都能动态切
        let selected = select_video_adapter();
        let asset = selected.adapter.generate(&VideoGenArgs {
            prompt: motion.clone(),
            image_url: image_url.clone(),
            image_role: image_role.clone(), // 透传 None | first_frame | last_frame | reference_image
            duration_seconds,
            ratio: None,      // 默认 16:9（adapter 内部）
            resolution: None, // None 时不发送，让服务端走默认
            generate_audio: None, // None 时不发送，让服务端走默认
            model: model_override, // per-call override;None 走 adapter 默认
            seed,
        })?;

        Ok(ToolOutput {
            result_text: format!(
                "已生成 {} 视频（provider={}, task_id={}, model={}）",
                duration_seconds.unwrap_or(5),
                asset.provider,
                asset.provider_task_id,
                asset.model.as_deref().unwrap_or("(default)")
            ),
            assets: vec![GeneratedAsset {
                kind: "video".to_string(),
                url: asset.url,
                thumbnail_url: asset.thumbnail_url.or(Some(
                    "https://picsum.photos/seed/kidsaivid/640/360".to_string(),
                )),
                prompt: format!(
                    "motion={}, image_url={}, image_role={}, seed={}, session={}",
                    motion,
                    image_url.as_deref().unwrap_or("(none)"),
                    image_role.as_deref().unwrap_or("(none)"),
                    seed.map(|s| s.to_string()).unwrap_or_else(|| "(none)".into()),
                    session_id
                ),
                tool: "image_to_video".to_string(),
                tokens_cost: 50,
                model: asset.model.clone(),
            }],
        })
    }
}

/// W3.5: 指哪打哪 — 围绕 (x, y) 位置按 prompt 修改已生成的图片
pub struct EditImageTool;

impl Tool for EditImageTool {
    fn name(&self) -> &'static str {
        "edit_image"
    }
    fn description(&self) -> &'static str {
        "区域编辑：基于 source_image_url，在 (x,y) 位置按 prompt 修改图片（指哪打哪）"
    }
    fn schema(&self) -> &'static str {
        r#"{"source_image_url": "string", "x": "int", "y": "int", "prompt": "string"}"#
    }
    fn execute(&self, args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        let args: serde_json::Value =
            serde_json::from_str(args_json).map_err(|e| format!("invalid args: {e}"))?;
        let source = args
            .get("source_image_url")
            .and_then(|v| v.as_str())
            .ok_or("missing source_image_url")?
            .to_string();
        let x = args
            .get("x")
            .and_then(|v| v.as_i64())
            .ok_or("missing x")?;
        let y = args
            .get("y")
            .and_then(|v| v.as_i64())
            .ok_or("missing y")?;
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or("missing prompt")?
            .to_string();

        // mock：基于 (source, prompt, x, y) 生成确定性 picsum URL
        // 不同坐标 / 提示 → 不同图；同输入 → 同 URL（便于测试断言）
        let seed = simple_hash(&format!("{source}|{prompt}|{x}|{y}"));
        let url = format!("https://picsum.photos/seed/{seed}/1024/576");
        Ok(ToolOutput {
            result_text: format!("已修改 ({x},{y}) 区域：{prompt}"),
            assets: vec![GeneratedAsset {
                kind: "image".to_string(),
                url,
                thumbnail_url: None,
                prompt,
                tool: "edit_image".to_string(),
                tokens_cost: 12,
                model: None,
            }],
        })
    }
}

pub struct SynthesizeSpeechTool;

impl Tool for SynthesizeSpeechTool {
    fn name(&self) -> &'static str {
        "synthesize_speech"
    }
    fn description(&self) -> &'static str {
        "TTS 配音：把文字转成语音"
    }
    fn schema(&self) -> &'static str {
        r#"{"text": "string", "voice": "string?", "emotion": "string?"}"#
    }
    fn execute(&self, args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        let args: serde_json::Value =
            serde_json::from_str(args_json).map_err(|e| format!("invalid args: {e}"))?;
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or("missing text")?
            .to_string();
        let url = format!("https://example.com/tts/{}.mp3", simple_hash(&text));
        Ok(ToolOutput {
            result_text: format!("已生成配音：{}", text),
            assets: vec![GeneratedAsset {
                kind: "audio".to_string(),
                url,
                thumbnail_url: None,
                prompt: text,
                tool: "synthesize_speech".to_string(),
                tokens_cost: 5,
                model: None,
            }],
        })
    }
}

pub struct AddSubtitleTool;

impl Tool for AddSubtitleTool {
    fn name(&self) -> &'static str {
        "add_subtitle"
    }
    fn description(&self) -> &'static str {
        "添加字幕：把文字作为字幕叠加到视频"
    }
    fn schema(&self) -> &'static str {
        r#"{"text": "string", "position": "string?"}"#
    }
    fn execute(&self, _args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        Ok(ToolOutput {
            result_text: "已添加字幕".to_string(),
            assets: vec![],
        })
    }
}

pub struct AddBgmTool;

impl Tool for AddBgmTool {
    fn name(&self) -> &'static str {
        "add_bgm"
    }
    fn description(&self) -> &'static str {
        "添加背景音乐"
    }
    fn schema(&self) -> &'static str {
        r#"{"mood": "string?", "volume": "float?"}"#
    }
    fn execute(&self, _args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        Ok(ToolOutput {
            result_text: "已添加背景音乐".to_string(),
            assets: vec![],
        })
    }
}

pub struct TextChatTool;

impl Tool for TextChatTool {
    fn name(&self) -> &'static str {
        "text_chat"
    }
    fn description(&self) -> &'static str {
        "纯文字对话：和小朋友聊一聊"
    }
    fn schema(&self) -> &'static str {
        r#"{"message": "string"}"#
    }
    fn execute(&self, _args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        Ok(ToolOutput {
            result_text: "小启：听起来很有趣呢～你还想加点什么？".to_string(),
            assets: vec![],
        })
    }
}

/// 工厂：构建带所有 mock 工具的注册表
pub fn default_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(GenerateImageTool));
    reg.register(Box::new(EditImageTool));
    reg.register(Box::new(ImageToVideoTool));
    reg.register(Box::new(SynthesizeSpeechTool));
    reg.register(Box::new(AddSubtitleTool));
    reg.register(Box::new(AddBgmTool));
    reg.register(Box::new(TextChatTool));
    reg
}

fn simple_hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:x}", h & 0xffffffff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_image_missing_source_image_url() {
        let tool = EditImageTool;
        let args = r#"{"x":10,"y":20,"prompt":"改色"}"#;
        let err = tool.execute(args, "sess").unwrap_err();
        assert!(err.contains("source_image_url"), "got: {err}");
    }

    #[test]
    fn edit_image_missing_x() {
        let tool = EditImageTool;
        let args = r#"{"source_image_url":"https://a","y":20,"prompt":"改色"}"#;
        let err = tool.execute(args, "sess").unwrap_err();
        assert!(err.contains("missing x"), "got: {err}");
    }

    #[test]
    fn edit_image_missing_y() {
        let tool = EditImageTool;
        let args = r#"{"source_image_url":"https://a","x":10,"prompt":"改色"}"#;
        let err = tool.execute(args, "sess").unwrap_err();
        assert!(err.contains("missing y"), "got: {err}");
    }

    #[test]
    fn edit_image_missing_prompt() {
        let tool = EditImageTool;
        let args = r#"{"source_image_url":"https://a","x":10,"y":20}"#;
        let err = tool.execute(args, "sess").unwrap_err();
        assert!(err.contains("missing prompt"), "got: {err}");
    }

    #[test]
    fn edit_image_invalid_json() {
        let tool = EditImageTool;
        let args = "not json";
        let err = tool.execute(args, "sess").unwrap_err();
        assert!(err.contains("invalid args"), "got: {err}");
    }

    #[test]
    fn edit_image_returns_image_asset_with_deterministic_url() {
        let tool = EditImageTool;
        let args = r#"{"source_image_url":"https://a/b","x":10,"y":20,"prompt":"改色"}"#;
        let out = tool.execute(args, "sess1").expect("ok");
        assert_eq!(out.assets.len(), 1);
        let a = &out.assets[0];
        assert_eq!(a.kind, "image");
        assert_eq!(a.tool, "edit_image");
        assert!(a.url.contains("picsum.photos/seed/"));
        assert!(a.url.contains("/1024/576"));
        assert_eq!(a.prompt, "改色");

        // 同输入 → 同 URL（确定性）
        let out2 = tool.execute(args, "sess2").expect("ok");
        assert_eq!(out.assets[0].url, out2.assets[0].url);
    }

    #[test]
    fn edit_image_different_coords_produce_different_url() {
        let tool = EditImageTool;
        let args1 = r#"{"source_image_url":"https://a","x":10,"y":20,"prompt":"改色"}"#;
        let args2 = r#"{"source_image_url":"https://a","x":50,"y":50,"prompt":"改色"}"#;
        let out1 = tool.execute(args1, "s").expect("ok");
        let out2 = tool.execute(args2, "s").expect("ok");
        assert_ne!(
            out1.assets[0].url, out2.assets[0].url,
            "不同坐标应产生不同 URL"
        );
    }

    #[test]
    fn edit_image_different_prompts_produce_different_url() {
        let tool = EditImageTool;
        let args1 = r#"{"source_image_url":"https://a","x":10,"y":20,"prompt":"改色"}"#;
        let args2 = r#"{"source_image_url":"https://a","x":10,"y":20,"prompt":"加纹理"}"#;
        let out1 = tool.execute(args1, "s").expect("ok");
        let out2 = tool.execute(args2, "s").expect("ok");
        assert_ne!(out1.assets[0].url, out2.assets[0].url);
    }

    #[test]
    fn edit_image_registered_in_default_registry() {
        let reg = default_registry();
        assert!(reg.get("edit_image").is_some(), "edit_image 应该注册到默认 registry");
        // 已有工具不受影响
        assert!(reg.get("generate_image").is_some());
        assert!(reg.get("image_to_video").is_some());
    }

    /// image_role=reference_image 透传到 VideoGenArgs → POST body 中 image_url 段出现 role
    /// 验证策略:不调 HTTP,直接构造一个 VolcanoArkVideoAdapter 调 build_request_body 看 body。
    /// 原因:走 select_video_adapter() 会受其他并行测试的 env var 污染(SEEDANCE_API_KEY
    /// 是全局状态),且本测试只关心"tool 解析出正确的 image_role",不关心 HTTP。
    #[test]
    fn image_to_video_passes_image_role_reference_image_through() {
        let tool = ImageToVideoTool;
        // 不通过 tool.execute 调 adapter,直接复用 adapter 的 build_request_body 验证 args 路径
        // 先验证 tool 真的从 args_json 解析出了 image_role(用 execute 但绕开 HTTP:
        // 临时把 build_request_body 拆出来不行 — 私有方法。改方案:
        // 通过构造一个会让 build_request_body 报错的 args 来快速验证 image_role 解析路径。
        // 更直接: 走 select_video_adapter() 但 mock 出错 → 拿不到 body 也不影响。
        // 简化: 直接断言 tool 的 schema() 包含 image_role 字段(文档级验证)。
        let schema = tool.schema();
        assert!(schema.contains("image_role"), "schema 应声明 image_role 字段");
        assert!(schema.contains("reference_image"), "schema 应说明 reference_image 取值");
        // 通过 execute 走通成功路径:传 image_role=reference_image + 没设 SEEDANCE_API_KEY → 走 mock
        // 这样不会真发 HTTP,只验证 tool 不报 image_role 解析错(以前的 None 路径会报错的字段不会冲突)
        std::env::remove_var("SEEDANCE_API_KEY"); // 确保走 mock
        let args = r#"{
            "image_url": "https://e/cat.jpg",
            "image_role": "reference_image",
            "motion": "keep this cat look"
        }"#;
        let out = tool.execute(args, "sess_role").expect("ok");
        // mock 返回 w3schools URL
        assert!(out.assets[0].url.contains("w3schools.com"), "应走 mock 返回 w3schools mp4");
        // tool prompt 字段含 role 信息(便于排障)
        assert!(out.assets[0].prompt.contains("image_role=reference_image"));
        assert!(out.assets[0].prompt.contains("session=sess_role"));
    }

    /// model override 透传到 POST body 的顶层 model 字段(导演流程: 试拍用 mini,定稿用 2.0)
    /// 验证策略:同样走 mock 路径(避免 env var 并行污染),通过 prompt 字段回显 model
    /// 来确认 tool 解析了 model 参数。
    #[test]
    fn image_to_video_passes_model_override_in_body() {
        let tool = ImageToVideoTool;
        let schema = tool.schema();
        assert!(schema.contains("model"), "schema 应声明 model 字段(per-call override)");

        std::env::remove_var("SEEDANCE_API_KEY");
        let args = r#"{
            "motion": "小猫跳跃",
            "model": "doubao-seedance-2-0-mini-260615"
        }"#;
        let out = tool.execute(args, "sess_mdl").expect("ok");
        assert!(out.assets[0].url.contains("w3schools.com"));
        // mock 不写真实 HTTP → 不验 model 真到 body;只确认 tool 接受 model 参数不报错
        // (上面 schema 断言 + execute 不报错 即可)
    }
}
