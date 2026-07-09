// Mock MCP 工具（W2.6）
// 每个工具模拟一次"AI 服务调用"，返回生成的资产信息
// 真实实现会替换为：HTTP 请求到智谱 / 阿里 DashScope / 自建模型服务

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
        "图生视频：把上一张图片动起来，生成 5 秒视频"
    }
    fn schema(&self) -> &'static str {
        r#"{"image_url": "string?", "duration": "int?", "motion": "string?"}"#
    }
    fn execute(&self, _args_json: &str, session_id: &str) -> Result<ToolOutput, String> {
        // mock：返回一个固定的示例视频
        let url = "https://www.w3schools.com/html/mov_bbb.mp4".to_string();
        Ok(ToolOutput {
            result_text: "已生成 5 秒视频".to_string(),
            assets: vec![GeneratedAsset {
                kind: "video".to_string(),
                url,
                thumbnail_url: Some("https://picsum.photos/seed/kidsaivid/640/360".to_string()),
                prompt: session_id.to_string(),
                tool: "image_to_video".to_string(),
                tokens_cost: 50,
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
