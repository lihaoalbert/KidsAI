// OpenAI 兼容 provider（W3.1 + W3.2 流式）
// 支持：DeepSeek / OpenAI / Qwen (DashScope) / Moonshot / 任何 /v1/chat/completions 兼容的服务
// ReAct 模式：Tool Calling（模型自己选 tool + args）
//
// W3.2: 支持 SSE 流式输出 + 取消（通过 Arc<AtomicBool> 在 chunk 间轮询）
//
// Token Plan task #31: MiniMax 支持 key 池（KeyPool），401/429 自动切下一个 key。
// 失败转移只发生在流开始前的初始响应；流一旦开始就是 2xx，SSE 解析逻辑不变。
//
// 参考：https://platform.deepseek.com/api-docs/

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::model::{Chunk, Model, ModelDecision, ModelRequest, ModelToolCall};
use crate::key_pool::KeyPool;

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
    api_keys: KeyPool,
    client: reqwest::Client,
}

impl OpenAiCompatible {
    /// 单 key 入口：内部包成 1-key KeyPool，非 MiniMax 分支继续用这个
    pub fn new(name: &str, model: &str, base_url: &str, api_key: &str) -> Self {
        let pool = KeyPool::from_str(api_key)
            .expect("OpenAiCompatible::new 收到空 key；上层应在 select_model 阶段拦截");
        Self::new_pool(name, model, base_url, pool)
    }

    /// 多 key 入口：MiniMax 分支用，401/429 自动切下一个 key
    pub fn new_pool(name: &str, model: &str, base_url: &str, pool: KeyPool) -> Self {
        Self {
            name: name.to_string(),
            model: model.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_keys: pool,
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

        // 失败转移循环：仅 401/429 切下一个 key；网络/超时/5xx/其他 4xx 立即返回
        let mut last_err = String::new();
        let resp = loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            let Some((idx, key)) = self.api_keys.next_healthy() else {
                return Err(format!("all keys exhausted: {last_err}"));
            };

            let resp = self
                .client
                .post(&url)
                .bearer_auth(&key)
                .json(&body)
                .send()
                .await;
            let resp = match resp {
                Ok(r) => r,
                // 网络/超时：立即返回，不切 key
                Err(e) => return Err(format!("http request failed: {e}")),
            };

            let status = resp.status();
            if status.is_success() {
                break resp;
            }

            if status == StatusCode::UNAUTHORIZED || status == StatusCode::TOO_MANY_REQUESTS {
                let txt = resp.text().await.unwrap_or_default();
                last_err = format!("upstream {status}: {txt}");
                self.api_keys.mark_failed(idx);
                continue; // 试下一个 key
            }

            // 其他（5xx / 4xx）：立即返回
            let txt = resp.text().await.unwrap_or_default();
            return Err(format!("upstream {status}: {txt}"));
        };

        self.stream_body(resp, cancel).await
    }
}

/// OpenAiCompatible 自身的辅助方法（非 Model trait）
impl OpenAiCompatible {
    /// 解析 SSE 响应体（已确认 2xx）。
    /// 抽出来是为了让 `decide_stream` 的失败转移循环只包裹 send + status，
    /// 不污染后续流式逻辑。
    async fn stream_body(
        &self,
        resp: reqwest::Response,
        cancel: Arc<AtomicBool>,
    ) -> Result<(ModelDecision, Vec<Chunk>), String> {
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
        (
            std::mem::take(&mut self.content),
            std::mem::take(&mut self.tool_bufs),
        )
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
    let tokens_used = if text.is_empty() {
        0
    } else {
        (text.len() as u32) / 4 + 1
    };
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
pub fn parse_decision_from_response(choice: &Choice, usage: Option<&Usage>) -> ModelDecision {
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
        _ => ("未知工具", json!({"type": "object", "properties": {}})),
    }
}

#[cfg(test)]
mod sse_parser_tests {
    use super::*;

    /// 单 chunk：content delta 直接累积
    #[test]
    fn sse_single_chunk_accumulates_content() {
        let mut p = SseParser::new();
        p.feed_event(
            r#"data: {"choices":[{"delta":{"content":"hello"}}]}

"#,
        );
        let (text, tool_bufs) = p.take_state();
        assert_eq!(text, "hello");
        assert!(tool_bufs.is_empty());
        let chunks = p.into_chunks();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "hello");
    }

    /// 多 chunk：跨事件累积 content
    #[test]
    fn sse_multi_chunks_accumulate() {
        let mut p = SseParser::new();
        p.feed_event(
            r#"data: {"choices":[{"delta":{"content":"你"}}]}

"#,
        );
        p.feed_event(
            r#"data: {"choices":[{"delta":{"content":"好"}}]}

"#,
        );
        p.feed_event(
            r#"data: {"choices":[{"delta":{"content":"！"}}]}

"#,
        );
        let (text, _bufs) = p.take_state();
        assert_eq!(text, "你好！");
        let chunks = p.into_chunks();
        assert_eq!(chunks.len(), 3);
    }

    /// 跨 chunk 拼接 tool_call arguments
    #[test]
    fn sse_tool_call_args_stitched_across_chunks() {
        let mut p = SseParser::new();
        // chunk 1: id + name + args 开头 `{"prompt":"`
        p.feed_event(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"generate_image","arguments":"{\"prompt\":\""}}]}}]}

"#,
        );
        // chunk 2: args 中间 `小猫`
        p.feed_event(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"小猫"}}]}}]}

"#,
        );
        // chunk 3: args 结尾 `"}`
        p.feed_event(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"}"}}]}}]}

"#,
        );
        let (_text, bufs) = p.take_state();
        let buf = bufs.get(&0).expect("index 0 should be buffered");
        assert_eq!(buf.id.as_deref(), Some("call_1"));
        assert_eq!(buf.name.as_deref(), Some("generate_image"));
        // arguments 应为完整 JSON 字符串
        let parsed: serde_json::Value =
            serde_json::from_str(&buf.args).expect("args should be valid JSON");
        assert_eq!(parsed["prompt"], "小猫");
    }

    /// 多个并行 tool_call（index 0 和 1 同时累积）
    #[test]
    fn sse_multiple_parallel_tool_calls_buffered_separately() {
        let mut p = SseParser::new();
        p.feed_event(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"generate_image","arguments":"{}"}}]}}]}

"#,
        );
        p.feed_event(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":1,"id":"call_b","function":{"name":"text_chat","arguments":"{}"}}]}}]}

"#,
        );
        let (_text, bufs) = p.take_state();
        assert_eq!(bufs.len(), 2, "should buffer both tool calls separately");
        let buf0 = bufs.get(&0).unwrap();
        let buf1 = bufs.get(&1).unwrap();
        assert_eq!(buf0.id.as_deref(), Some("call_a"));
        assert_eq!(buf0.name.as_deref(), Some("generate_image"));
        assert_eq!(buf1.id.as_deref(), Some("call_b"));
        assert_eq!(buf1.name.as_deref(), Some("text_chat"));
    }

    /// [DONE] 终止符不报错且不破坏状态
    #[test]
    fn sse_done_terminator_is_ignored() {
        let mut p = SseParser::new();
        p.feed_event(
            r#"data: {"choices":[{"delta":{"content":"hi"}}]}

"#,
        );
        p.feed_event("data: [DONE]\n\n");
        let (text, _) = p.take_state();
        assert_eq!(text, "hi");
    }

    /// 心跳 / 注释 / 非法 JSON 不崩溃
    #[test]
    fn sse_ignores_heartbeat_and_invalid_lines() {
        let mut p = SseParser::new();
        p.feed_event(": heartbeat\n\n");
        p.feed_event("data: not-json\n\n");
        p.feed_event("event: ping\n\n");
        p.feed_event(
            r#"data: {"choices":[{"delta":{"content":"ok"}}]}

"#,
        );
        let (text, _) = p.take_state();
        assert_eq!(text, "ok");
    }
}
