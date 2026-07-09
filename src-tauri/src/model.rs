// 多模型路由（W2.5）
// MVP 阶段：完全 mock（按关卡 + 输入生成确定的 ReAct trajectory）
// 后续阶段：接入 LiteLLM，统一 OpenAI / Anthropic / Qwen 接口
//
// 设计：把"调用模型"抽象成一个 trait，后续替换实现不影响 agent loop

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
}

/// 模型响应：模型"想说"的话 + 决定要不要调工具、调哪个
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDecision {
    pub thought: String,
    /// 选哪个工具；None 表示模型决定给出 final answer
    pub tool: Option<String>,
    /// 工具参数（JSON 字符串）
    pub tool_args: Option<String>,
    /// 当 tool=None 时的最终回复
    pub final_answer: Option<String>,
    pub tokens_used: u32,
}

/// 模型 trait
pub trait Model: Send + Sync {
    fn name(&self) -> &'static str;
    fn decide(&self, req: &ModelRequest) -> Result<ModelDecision, String>;
}

/// 路由：MVP 直接返回唯一的 mock 模型
pub struct ModelRouter {
    primary: Box<dyn Model>,
}

impl ModelRouter {
    pub fn new(primary: Box<dyn Model>) -> Self {
        Self { primary }
    }

    pub fn primary_name(&self) -> &str {
        self.primary.name()
    }

    pub fn decide(&self, req: &ModelRequest) -> Result<ModelDecision, String> {
        // MVP：直接走 primary。W3+ 会按 cost / capability 选模型
        self.primary.decide(req)
    }
}
