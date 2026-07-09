// Agent Loop 命令（W2.2 仅占位，W2.4 实现核心循环）
// 当前返回 mock 数据，验证前后端通信链路

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunRequest {
    pub level_id: String,
    pub user_input: String,
    pub system_prompt: String,
    #[serde(default)]
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunResponse {
    pub session_id: String,
    pub level_id: String,
    pub final_answer: String,
    pub thoughts: Vec<String>,
    pub tool_calls: Vec<ToolCallRecord>,
    pub assets: Vec<GeneratedAsset>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool: String,
    pub args: serde_json::Value,
    pub result: String,
    pub timestamp: i64,
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

/// 同步运行 Agent（W2.2 占位）
/// 真实实现见 W2.4：ReAct 循环 + 事件流
#[tauri::command]
pub async fn run_agent(
    _app: AppHandle,
    request: AgentRunRequest,
) -> Result<AgentRunResponse, String> {
    let started = std::time::Instant::now();
    let session_id = format!("sess_{}", now_millis());

    // 占位：只回显输入，W2.4 会替换为真正的 ReAct 循环
    Ok(AgentRunResponse {
        session_id,
        level_id: request.level_id,
        final_answer: format!(
            "（W2.2 占位）我收到了你的输入：\"{}\"。\n\nW2.4 会接入真正的 Agent Loop。",
            request.user_input
        ),
        thoughts: vec![
            "读取用户输入".to_string(),
            "加载关卡 system_prompt".to_string(),
            "（占位）W2.4 将在此执行 ReAct 循环".to_string(),
        ],
        tool_calls: vec![],
        assets: vec![],
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
