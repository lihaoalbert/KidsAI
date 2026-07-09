// 供集成测试使用的辅助 API
// 暴露 run_loop + 真实组件，让测试不依赖 Tauri AppHandle

use crate::agent::{run_loop, AgentEvent, AgentRunRequest, AgentRunResponse, NoopEventSink};
use crate::model::ModelRouter;
use crate::tools::default_registry;

pub fn run_agent_sync(
    level_id: &str,
    user_input: &str,
    system_prompt: &str,
    tools: Vec<String>,
) -> Result<AgentRunResponse, String> {
    let registry = default_registry();
    let router = ModelRouter::new(Box::new(crate::model_mock::MockModel));
    let request = AgentRunRequest {
        level_id: level_id.to_string(),
        user_input: user_input.to_string(),
        system_prompt: system_prompt.to_string(),
        tools,
    };
    run_loop(&NoopEventSink, &registry, &router, request)
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
                AgentEvent::Thought { .. } => "thought",
                AgentEvent::ToolCall { .. } => "tool_call",
                AgentEvent::ToolResult { .. } => "tool_result",
                AgentEvent::FinalAnswer { .. } => "final_answer",
                AgentEvent::Done { .. } => "done",
                AgentEvent::Error { .. } => "error",
            }
            .to_string())
            .collect()
    }
}

impl crate::agent::EventSink for CollectingSink {
    fn emit(&self, event: &AgentEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}
