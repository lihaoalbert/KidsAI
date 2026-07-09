// 供集成测试使用的辅助 API
// 暴露 run_loop + 真实组件，让测试不依赖 Tauri AppHandle
//
// W3.2: 改 async + 加 run_agent_stream_with_model

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::agent::{
    run_loop, AgentEvent, AgentRunRequest, AgentRunResponse, NoopEventSink, SessionRegistry,
};
use crate::model::{Model, ModelRouter};

/// 兼容旧测试：默认用 mock
pub async fn run_agent_sync(
    level_id: &str,
    user_input: &str,
    system_prompt: &str,
    tools: Vec<String>,
) -> Result<AgentRunResponse, String> {
    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(crate::model_mock::MockModel::default()));
    let request = AgentRunRequest {
        level_id: level_id.to_string(),
        user_input: user_input.to_string(),
        system_prompt: system_prompt.to_string(),
        tools,
    };
    run_loop(&NoopEventSink, &registry, &router, request).await
}

/// 新版：调用方传入任意模型（真实 LLM / mock / 等）
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
    };
    run_loop(&NoopEventSink, &registry, &router, request).await
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
    };
    run_loop(&NoopEventSink, registry, &router, request).await
}

/// 收集事件用的 sink（测试用）
pub struct CollectingSink {
    pub events: std::sync::Mutex<Vec<AgentEvent>>,
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
