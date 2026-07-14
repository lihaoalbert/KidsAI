// 供集成测试使用的辅助 API
// 暴露 run_loop + 真实组件，让测试不依赖 Tauri AppHandle
//
// W3.2: 改 async + 加 run_agent_stream_with_model
// W3.4: 支持 character 参数
// W3.6: 支持 character + style 两个独立维度

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::agent::{
    run_loop, AgentEvent, AgentRunRequest, AgentRunResponse, NoopEventSink, SessionRegistry,
};
use crate::character::Character;
use crate::model::{Model, ModelRouter};
use crate::style::StylePreset;

/// 兼容旧测试：默认用 mock，无 character / style
pub async fn run_agent_sync(
    level_id: &str,
    user_input: &str,
    system_prompt: &str,
    tools: Vec<String>,
) -> Result<AgentRunResponse, String> {
    run_agent_sync_with_character_and_style(level_id, user_input, system_prompt, tools, None, None)
        .await
}

/// W3.4: 带 character 的版本（兼容旧调用）
pub async fn run_agent_sync_with_character(
    level_id: &str,
    user_input: &str,
    system_prompt: &str,
    tools: Vec<String>,
    character: Option<Character>,
) -> Result<AgentRunResponse, String> {
    run_agent_sync_with_character_and_style(
        level_id,
        user_input,
        system_prompt,
        tools,
        character,
        None,
    )
    .await
}

/// W3.6: 角色 + 风格 两个维度都能注入
pub async fn run_agent_sync_with_character_and_style(
    level_id: &str,
    user_input: &str,
    system_prompt: &str,
    tools: Vec<String>,
    character: Option<Character>,
    style: Option<StylePreset>,
) -> Result<AgentRunResponse, String> {
    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(crate::model_mock::MockModel::default()));
    let request = AgentRunRequest {
        level_id: level_id.to_string(),
        user_input: user_input.to_string(),
        system_prompt: system_prompt.to_string(),
        tools,
        character_id: character.as_ref().map(|c| c.id.clone()),
        style_id: style.as_ref().map(|s| s.id.clone()),
    };
    run_loop(
        &NoopEventSink,
        &registry,
        &router,
        request,
        character,
        style,
    )
    .await
}

/// 新版：调用方传入任意模型（真实 LLM / mock / 等），可选 character
pub async fn run_agent_with_model(
    model: Box<dyn Model>,
    level_id: &str,
    user_input: &str,
    system_prompt: &str,
    tools: Vec<String>,
) -> Result<AgentRunResponse, String> {
    let registry = SessionRegistry::default();
    let router = ModelRouter::new(model);
    let request = AgentRunRequest {
        level_id: level_id.to_string(),
        user_input: user_input.to_string(),
        system_prompt: system_prompt.to_string(),
        tools,
        character_id: None,
        style_id: None,
    };
    run_loop(&NoopEventSink, &registry, &router, request, None, None).await
}

/// W3.2: 流式 / 取消测试用 — 暴露 registry 和 cancel flag
pub async fn run_agent_stream_with_model(
    model: Box<dyn Model>,
    level_id: &str,
    user_input: &str,
    system_prompt: &str,
    tools: Vec<String>,
    registry: &SessionRegistry,
) -> Result<AgentRunResponse, String> {
    let router = ModelRouter::new(model);
    let request = AgentRunRequest {
        level_id: level_id.to_string(),
        user_input: user_input.to_string(),
        system_prompt: system_prompt.to_string(),
        tools,
        character_id: None,
        style_id: None,
    };
    run_loop(&NoopEventSink, registry, &router, request, None, None).await
}

/// 收集事件用的 sink（测试用）
pub struct CollectingSink {
    pub events: std::sync::Mutex<Vec<AgentEvent>>,
}

impl Default for CollectingSink {
    fn default() -> Self {
        Self::new()
    }
}

impl CollectingSink {
    pub fn new() -> Self {
        Self {
            events: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn kinds(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .map(|e| match e {
                AgentEvent::Started { .. } => "started",
                AgentEvent::Chunk { .. } => "chunk",
                AgentEvent::Thought { .. } => "thought",
                AgentEvent::ToolCall { .. } => "tool_call",
                AgentEvent::ToolResult { .. } => "tool_result",
                AgentEvent::FinalAnswer { .. } => "final_answer",
                AgentEvent::Done { .. } => "done",
                AgentEvent::Cancelled { .. } => "cancelled",
                AgentEvent::Error { .. } => "error",
            })
            .map(|s| s.to_string())
            .collect()
    }

    /// 返回所有 chunk 事件的 delta 文本
    pub fn chunk_deltas(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| match e {
                AgentEvent::Chunk { delta, .. } => Some(delta.clone()),
                _ => None,
            })
            .collect()
    }
}

impl crate::agent::EventSink for CollectingSink {
    fn emit(&self, event: &AgentEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

/// 兼容 helper：建一个 fresh registry + flag，方便取消测试
#[allow(dead_code)]
pub fn new_test_registry() -> (SessionRegistry, Arc<AtomicBool>) {
    let registry = SessionRegistry::default();
    let flag = registry.insert("preset_id".to_string());
    (registry, flag)
}
