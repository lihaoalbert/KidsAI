// OpenAI 兼容 provider（W3.1 + W3.2 流式）
// 支持：DeepSeek / OpenAI / Qwen (DashScope) / Moonshot / 任何 /v1/chat/completions 兼容的服务
// ReAct 模式：Tool Calling（模型自己选 tool + args）
//
// W3.2: 支持 SSE 流式输出 + 取消（通过 Arc<AtomicBool> 在 chunk 间轮询）
//
// 参考：https://platform.deepseek.com/api-docs/

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::model::{Chunk, Model, ModelDecision, ModelRequest, ModelToolCall};

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
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub function: OaiFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OaiFunction {
    pub name: String,
    pub arguments: String,
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

/// 非流式响应（保留以便单步 final / 测试）
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

/// 流式响应块
#[derive(Debug, Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: StreamDelta,
    #[allow(dead_code)]
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<StreamToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct StreamToolCallDelta {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<StreamFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct StreamFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
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
                .timeout(std::time::Duration::from_secs(180))
                .connect_timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        }
    }

    /// 把 ModelRequest 翻译成 OpenAI 格式（流式 + 非流式共用）
    fn build_messages(&self, req: &ModelRequest) -> Vec<OaiMessage> {
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
                    let oai_tool_calls = m.tool_calls.as_ref().map(|tcs| {
                        tcs.iter()
                            .map(|tc: &ModelToolCall| OaiToolCall {
                                id: tc.id.clone(),
                                kind: "function".to_string(),
                                function: OaiFunction {
                                    name: tc.name.clone(),
                                    arguments: tc.args.clone(),
                                },
                            })
                            .collect()
                    });
                    messages.push(OaiMessage {
                        role: "assistant".to_string(),
                        content: if m.content.is_empty() {
                            None
                        } else {
                            Some(m.content.clone())
                        },
                        tool_call_id: None,
                        tool_calls: oai_tool_calls,
                    });
                }
                "tool" => {
                    messages.push(OaiMessage {
                        role: "tool".to_string(),
                        content: Some(m.content.clone()),
                        tool_call_id: m
                            .tool_call_id
                            .clone()
                            .or_else(|| m.name.clone().or(Some("call_0".to_string()))),
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

        messages
    }

    fn build_tools(&self, allowed: &[String]) -> Vec<OaiTool> {
        allowed
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
            .collect()
    }
}

#[async_trait]
impl Model for OpenAiCompatible {
    fn name(&self) -> String {
        format!("{}:{}", self.name, self.model)
    }

    async fn decide_stream(
        &self,
        req: &ModelRequest,
        cancel: Arc<AtomicBool>,
    ) -> Result<(ModelDecision, Vec<Chunk>), String> {
        let messages = self.build_messages(req);
        let tools = self.build_tools(&req.allowed_tools);

        let body = ChatRequest {
            model: &self.model,
            messages,
            tools,
            tool_choice: "auto",
            temperature: req.temperature,
            stream: true,
        };

        // base_url 形如 "https://api.minimaxi.com/v1"，直接拼 path
        let url = format!("{}/chat/completions", self.base_url);

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("http request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(format!("upstream {}: {}", status, txt));
        }

        // 边读边解析 SSE
        let mut stream = resp.bytes_stream();
        let mut parser = SseParser::new();
        let mut pending = String::new(); // SSE 一行可能被 TCP 拆成多块，缓存拼接
        let tokens_used: u32 = 0;

        loop {
            tokio::select! {
                // 50ms 轮询取消 — 取消响应延迟上限 50ms
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    if cancel.load(Ordering::Relaxed) {
                        return Err("cancelled".into());
                    }
                }
                chunk = stream.next() => {
                    match chunk {
                        Some(Ok(bytes)) => {
                            pending.push_str(&String::from_utf8_lossy(&bytes));
                            // SSE 事件以 \n\n 分隔
                            while let Some(idx) = pending.find("\n\n") {
                                let evt: String = pending.drain(..idx + 2).collect();
                                parser.feed_event(&evt);
                            }
                        }
                        Some(Err(e)) => return Err(format!("stream read: {e}")),
                        None => break, // EOF
                    }
                }
            }
        }

        // 处理最后残留（无 trailing \n\n）
        if !pending.trim().is_empty() {
            parser.feed_event(&pending);
        }

        let (text, tool_bufs) = parser.take_state();
        let chunks = parser.into_chunks();
        let decision = parse_decision(text, tool_bufs, tokens_used);
        Ok((decision, chunks))
    }
}

/// SSE 解析器：累积 content deltas + 按 index 缓存 tool_call 碎片
struct SseParser {
    content: String,
    tool_bufs: HashMap<usize, ToolBuf>,
    chunks: Vec<Chunk>,
}

#[derive(Default, Debug)]
struct ToolBuf {
    id: Option<String>,
    name: Option<String>,
    args: String,
}

impl SseParser {
    fn new() -> Self {
        Self {
            content: String::new(),
            tool_bufs: HashMap::new(),
            chunks: Vec::new(),
        }
    }

    /// 处理一个完整 SSE 事件（\n\n 之间的内容）
    fn feed_event(&mut self, event: &str) {
        for line in event.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // OpenAI / DeepSeek / MiniMax 都用 "data: " 前缀；可能有 "data:" 无空格
            let data = if let Some(rest) = line.strip_prefix("data:") {
                rest.trim()
            } else {
                continue;
            };
            if data == "[DONE]" {
                return; // 留给 finalize
            }
            let parsed: StreamChunk = match serde_json::from_str(data) {
                Ok(p) => p,
                Err(_) => continue, // 心跳 / 注释 / 不完整 — 忽略
            };
            for choice in parsed.choices {
                if let Some(text) = choice.delta.content {
                    if !text.is_empty() {
                        self.content.push_str(&text);
                        self.chunks.push(Chunk {
                            text,
                            step: 0, // agent.rs 后续会重写
                        });
                    }
                }
                if let Some(tcs) = choice.delta.tool_calls {
                    for tc in tcs {
                        let entry = self.tool_bufs.entry(tc.index).or_default();
                        if let Some(id) = tc.id {
                            entry.id = Some(id);
                        }
                        if let Some(f) = tc.function {
                            if let Some(name) = f.name {
                                entry.name = Some(name);
                            }
                            if let Some(args) = f.arguments {
                                entry.args.push_str(&args);
                            }
                        }
                    }
                }
            }
        }
    }

    fn take_state(&mut self) -> (String, HashMap<usize, ToolBuf>) {
        (std::mem::take(&mut self.content), std::mem::take(&mut self.tool_bufs))
    }

    fn into_chunks(self) -> Vec<Chunk> {
        self.chunks
    }
}

fn parse_decision(
    text: String,
    tool_bufs: HashMap<usize, ToolBuf>,
    _tokens_used: u32,
) -> ModelDecision {
    // 流式响应通常不返回 usage（按调用计费；usage 在非流式响应里）。
    // tokens 留给后续按字符估算或上游单独提供。
    let tokens_used = if text.is_empty() { 0 } else { (text.len() as u32) / 4 + 1 };
    // W3.3: 推理模型会在 content 里塞 <think>...</think> 思考片段，必须剥除再交给前端
    let text = strip_think_tags(&text);

    if let Some((_, buf)) = tool_bufs.into_iter().max_by_key(|(idx, _)| *idx) {
        if let (Some(id), Some(name)) = (buf.id, buf.name) {
            return ModelDecision {
                thought: if text.is_empty() {
                    format!("调用 {}", name)
                } else {
                    text
                },
                tool: Some(name),
                tool_args: Some(buf.args),
                tool_call_id: Some(id),
                final_answer: None,
                tokens_used,
            };
        }
    }

    ModelDecision {
        thought: "直接给出最终回答".to_string(),
        tool: None,
        tool_args: None,
        tool_call_id: None,
        final_answer: Some(text),
        tokens_used,
    }
}

/// 剥除推理模型的 <think>...</think> 思考片段
/// - 完整配对的全部剥掉（greedy，连续多段也支持）
/// - 未闭合（没有 `</think>`）的保守原样保留
pub fn strip_think_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("<think>") {
        out.push_str(&rest[..start]);
        rest = &rest[start + "<think>".len()..];
        match rest.find("</think>") {
            Some(end) => rest = &rest[end + "</think>".len()..],
            None => {
                // 未闭合 — 原样保留（保守）
                out.push_str("<think>");
                out.push_str(rest);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// 从非流式 OpenAI 响应解析出 ModelDecision
/// 公开出来供测试验证 JSON 解析逻辑（不依赖网络）
pub fn parse_decision_from_response(
    choice: &Choice,
    usage: Option<&Usage>,
) -> ModelDecision {
    let msg = &choice.message;
    let tokens_used = usage.map(|u| u.total_tokens).unwrap_or(0);

    if let Some(tool_calls) = &msg.tool_calls {
        if let Some(tc) = tool_calls.first() {
            let raw = msg.content.clone().unwrap_or_default();
            let thought = if raw.is_empty() {
                format!("调用 {}", tc.function.name.clone())
            } else {
                strip_think_tags(&raw)
            };
            return ModelDecision {
                thought,
                tool: Some(tc.function.name.clone()),
                tool_args: Some(tc.function.arguments.clone()),
                tool_call_id: Some(tc.id.clone()),
                final_answer: None,
                tokens_used,
            };
        }
    }

    let raw = msg.content.clone().unwrap_or_default();
    let answer = strip_think_tags(&raw);
    ModelDecision {
        thought: "直接给出最终回答".to_string(),
        tool: None,
        tool_args: None,
        tool_call_id: None,
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
