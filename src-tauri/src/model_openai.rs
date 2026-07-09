// OpenAI 兼容 provider（W3.1）
// 支持：DeepSeek / OpenAI / Qwen (DashScope) / Moonshot / 任何 /v1/chat/completions 兼容的服务
// ReAct 模式：Tool Calling（模型自己选 tool + args）
//
// 参考：https://platform.deepseek.com/api-docs/

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::model::{Model, ModelDecision, ModelRequest};

/// OpenAI 兼容 chat completion 请求
#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<OaiMessage>,
    tools: Vec<OaiTool>,
    tool_choice: &'a str,
    temperature: f32,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct OaiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OaiToolCall>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OaiToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    function: OaiFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct OaiFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OaiTool {
    #[serde(rename = "type")]
    kind: &'static str, // "function"
    function: OaiFunctionDef,
}

#[derive(Debug, Serialize)]
struct OaiFunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub choices: Vec<Choice>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub message: ResponseMessage,
    #[allow(dead_code)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<OaiToolCall>>,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub total_tokens: u32,
}

/// OpenAI 兼容 model
pub struct OpenAiCompatible {
    pub name: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    client: reqwest::Client,
}

impl OpenAiCompatible {
    pub fn new(name: &str, model: &str, base_url: &str, api_key: &str) -> Self {
        Self {
            name: name.to_string(),
            model: model.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .expect("reqwest client"),
        }
    }
}

impl Model for OpenAiCompatible {
    fn name(&self) -> String {
        format!("{}:{}", self.name, self.model)
    }

    fn decide(&self, req: &ModelRequest) -> Result<ModelDecision, String> {
        // 把 ModelRequest 翻译成 OpenAI 格式
        let mut messages = vec![OaiMessage {
            role: "system".to_string(),
            content: Some(req.system_prompt.clone()),
            tool_call_id: None,
            tool_calls: None,
        }];

        for m in &req.messages {
            match m.role.as_str() {
                "user" => messages.push(OaiMessage {
                    role: "user".to_string(),
                    content: Some(m.content.clone()),
                    tool_call_id: None,
                    tool_calls: None,
                }),
                "assistant" => {
                    messages.push(OaiMessage {
                        role: "assistant".to_string(),
                        content: Some(m.content.clone()),
                        tool_call_id: None,
                        tool_calls: None,
                    });
                }
                "tool" => {
                    messages.push(OaiMessage {
                        role: "tool".to_string(),
                        content: Some(m.content.clone()),
                        tool_call_id: m.name.clone().or(Some("call_0".to_string())),
                        tool_calls: None,
                    });
                }
                other => {
                    messages.push(OaiMessage {
                        role: "user".to_string(),
                        content: Some(format!("[{}] {}", other, m.content)),
                        tool_call_id: None,
                        tool_calls: None,
                    });
                }
            }
        }

        let tools: Vec<OaiTool> = req
            .allowed_tools
            .iter()
            .map(|name| {
                let (description, params) = tool_spec(name);
                OaiTool {
                    kind: "function",
                    function: OaiFunctionDef {
                        name: name.clone(),
                        description: description.to_string(),
                        parameters: params,
                    },
                }
            })
            .collect();

        let body = ChatRequest {
            model: &self.model,
            messages,
            tools,
            tool_choice: "auto",
            temperature: req.temperature,
            stream: false,
        };

        // 同步阻塞：尝试当前 runtime，否则起新 runtime
        let url = format!("{}/v1/chat/completions", self.base_url);
        let client = self.client.clone();
        let api_key = self.api_key.clone();

        let send_fut = async move {
            let resp = client
                .post(&url)
                .bearer_auth(&api_key)
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("http request failed: {e}"))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let txt = resp.text().await.unwrap_or_default();
                return Err(format!("upstream {}: {}", status, txt));
            }

            let parsed: ChatResponse = resp
                .json()
                .await
                .map_err(|e| format!("parse response: {e}"))?;
            Ok(parsed)
        };

        let parsed: ChatResponse = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.block_on(send_fut)?
        } else {
            tokio::runtime::Runtime::new()
                .map_err(|e| format!("tokio runtime: {e}"))?
                .block_on(send_fut)?
        };

        let choice = parsed
            .choices
            .first()
            .ok_or_else(|| "no choices in response".to_string())?;

        Ok(parse_decision_from_response(choice, parsed.usage.as_ref()))
    }
}

/// 从 OpenAI 响应解析出 ModelDecision
/// 公开出来供测试验证 JSON 解析逻辑（不依赖网络）
pub fn parse_decision_from_response(
    choice: &Choice,
    usage: Option<&Usage>,
) -> ModelDecision {
    let msg = &choice.message;
    let tokens_used = usage.map(|u| u.total_tokens).unwrap_or(0);

    if let Some(tool_calls) = &msg.tool_calls {
        if let Some(tc) = tool_calls.first() {
            let thought = msg
                .content
                .clone()
                .unwrap_or_else(|| format!("调用 {}", tc.function.name));
            return ModelDecision {
                thought,
                tool: Some(tc.function.name.clone()),
                tool_args: Some(tc.function.arguments.clone()),
                final_answer: None,
                tokens_used,
            };
        }
    }

    let answer = msg.content.clone().unwrap_or_default();
    ModelDecision {
        thought: "直接给出最终回答".to_string(),
        tool: None,
        tool_args: None,
        final_answer: Some(answer),
        tokens_used,
    }
}

/// 工具的 OpenAI 格式描述
fn tool_spec(name: &str) -> (&'static str, serde_json::Value) {
    match name {
        "generate_image" => (
            "文生图：根据 prompt 生成一张图片。返回图片 URL。",
            json!({
                "type": "object",
                "properties": {
                    "prompt": {"type": "string", "description": "图片描述，建议包含主体+动作+场景+风格"},
                    "style": {"type": "string", "description": "风格，可选 cartoon / realistic / watercolor"}
                },
                "required": ["prompt"]
            }),
        ),
        "image_to_video" => (
            "图生视频：把上一张图片动起来，生成 5 秒视频。",
            json!({
                "type": "object",
                "properties": {
                    "image_url": {"type": "string", "description": "可选，要动的图片 URL"},
                    "duration": {"type": "integer", "description": "时长秒数，默认 5"},
                    "motion": {"type": "string", "description": "auto / pan_left / zoom_in"}
                }
            }),
        ),
        "synthesize_speech" => (
            "TTS 配音：把文字转成语音。",
            json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "voice": {"type": "string"},
                    "emotion": {"type": "string"}
                },
                "required": ["text"]
            }),
        ),
        "add_subtitle" => (
            "添加字幕。",
            json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "position": {"type": "string"}
                },
                "required": ["text"]
            }),
        ),
        "add_bgm" => (
            "添加背景音乐。",
            json!({
                "type": "object",
                "properties": {
                    "mood": {"type": "string"},
                    "volume": {"type": "number"}
                }
            }),
        ),
        "text_chat" => (
            "纯文字对话。",
            json!({
                "type": "object",
                "properties": {
                    "message": {"type": "string"}
                },
                "required": ["message"]
            }),
        ),
        _ => (
            "未知工具",
            json!({"type": "object", "properties": {}}),
        ),
    }
}
