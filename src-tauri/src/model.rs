// 多模型路由（W2.5 + W3.2 async 流式）
// MVP 阶段：完全 mock（按关卡 + 输入生成确定的 ReAct trajectory）
// 后续阶段：接入 LiteLLM，统一 OpenAI / Anthropic / Qwen 接口
//
// 设计：把"调用模型"抽象成一个 trait，后续替换实现不影响 agent loop
//
// W3.2: trait 改 async + decide_stream，返回 (decision, chunks) 让 agent 层负责事件发射。

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// 模型路由请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub system_prompt: String,
    pub messages: Vec<ModelMessage>,
    /// 工具清单（白名单），让模型只能挑这些
    pub allowed_tools: Vec<String>,
    /// 温度，0 = 确定性（mock 默认 0）
    #[serde(default)]
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMessage {
    pub role: String, // "user" | "assistant" | "tool"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>, // 工具名（role=tool 时）
    /// tool_call_id：role=tool 时关联到 assistant 消息里 tool_calls[].id
    /// OpenAI / DeepSeek / MiniMax 都强制要求
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// role=assistant 时：模型发起的工具调用列表
    /// 下次请求时必须带回去，下游 API 用它来匹配 tool 消息的 tool_call_id
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ModelToolCall>>,
}

/// assistant 在某一轮发起的工具调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelToolCall {
    pub id: String,
    pub name: String,
    pub args: String, // JSON 字符串
}

/// 模型响应：模型"想说"的话 + 决定要不要调工具、调哪个
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDecision {
    pub thought: String,
    /// 选哪个工具；None 表示模型决定给出 final answer
    pub tool: Option<String>,
    /// 工具参数（JSON 字符串）
    pub tool_args: Option<String>,
    /// 工具调用的 ID（OpenAI / DeepSeek / MiniMax 都要求关联）
    /// agent loop 会在 tool 消息里回传这个 id
    pub tool_call_id: Option<String>,
    /// 当 tool=None 时的最终回复
    pub final_answer: Option<String>,
    pub tokens_used: u32,
}

/// 模型在 streaming 过程中产出的一段文本（content delta）
/// step 字段在 agent.rs 里赋值（model 不关心自己处于第几步）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub text: String,
    #[serde(default)]
    pub step: u32,
}

/// 模型 trait
#[async_trait]
pub trait Model: Send + Sync {
    fn name(&self) -> String;
    /// 流式 decide：返回 (ModelDecision, 该步产出的所有 content deltas)
    /// tool call 的 deltas 不出现在 chunks 里（内部 buffer 到 finalize 才返回）。
    /// `cancel` 是会话级取消信号，model 在 chunk 间 / network 等待间应检查。
    async fn decide_stream(
        &self,
        req: &ModelRequest,
        cancel: Arc<AtomicBool>,
    ) -> Result<(ModelDecision, Vec<Chunk>), String>;
}

/// 路由：MVP 直接返回唯一的 mock 模型
pub struct ModelRouter {
    primary: Box<dyn Model>,
}

impl ModelRouter {
    pub fn new(primary: Box<dyn Model>) -> Self {
        Self { primary }
    }

    pub fn primary_name(&self) -> String {
        self.primary.name()
    }

    pub async fn decide_stream(
        &self,
        req: &ModelRequest,
        cancel: Arc<AtomicBool>,
    ) -> Result<(ModelDecision, Vec<Chunk>), String> {
        // MVP：直接走 primary。W3.4+ 会按 cost / capability 选模型 + 失败回退
        self.primary.decide_stream(req, cancel).await
    }
}
