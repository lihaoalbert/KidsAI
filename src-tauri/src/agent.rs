// Agent Loop（W2.4 + W2.5 + W2.6 + W3.1 + W3.2 流式 + 取消）
// ReAct 循环：Model -> Tool -> Observation -> ... -> Final Answer
// 事件流：每步通过 tauri::Emitter 发向前端
// W3.2: 改 async + 串接 SSE chunks + 支持 session 级取消

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::model::{ModelMessage, ModelRequest, ModelRouter, ModelToolCall};
use crate::safety::{KeywordFilter, SafetyVerdict};
use crate::tools::{GeneratedAsset, ToolOutput};

/// 取消会话的注册表（Tauri state）
/// session_id -> 取消 flag (true 表示请求取消)
#[derive(Default)]
pub struct SessionRegistry {
    map: std::sync::Mutex<std::collections::HashMap<String, Arc<AtomicBool>>>,
}

impl SessionRegistry {
    pub fn insert(&self, id: String) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.map.lock().unwrap().insert(id, flag.clone());
        flag
    }

    /// 翻转取消 flag，返回是否找到该 session
    pub fn cancel(&self, id: &str) -> bool {
        if let Some(flag) = self.map.lock().unwrap().get(id) {
            flag.store(true, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    pub fn remove(&self, id: &str) {
        self.map.lock().unwrap().remove(id);
    }
}

/// RAII guard：run_loop 结束（任何路径）时自动从 registry 移除 session
struct RegistryGuard<'a> {
    registry: &'a SessionRegistry,
    id: String,
}

impl Drop for RegistryGuard<'_> {
    fn drop(&mut self) {
        self.registry.remove(&self.id);
    }
}

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
    pub model: String,
    pub steps: u32,
    #[serde(default)]
    pub tokens_used: u32,
    #[serde(default)]
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool: String,
    pub args: serde_json::Value,
    pub result: String,
    pub timestamp: i64,
}

// ============ 事件流 payload ============
// 前端通过 `agent://event` channel 监听

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentEvent {
    Started {
        session_id: String,
    },
    /// 流式 content delta（W3.2）
    Chunk {
        session_id: String,
        step: u32,
        delta: String,
    },
    Thought {
        session_id: String,
        step: u32,
        thought: String,
    },
    ToolCall {
        session_id: String,
        step: u32,
        tool: String,
        args: serde_json::Value,
    },
    ToolResult {
        session_id: String,
        step: u32,
        tool: String,
        result: String,
        assets: Vec<GeneratedAsset>,
    },
    FinalAnswer {
        session_id: String,
        answer: String,
    },
    Done {
        session_id: String,
        steps: u32,
        duration_ms: u64,
    },
    /// 取消生效（W3.2）
    Cancelled {
        session_id: String,
    },
    Error {
        session_id: String,
        message: String,
    },
}

const EVENT_CHANNEL: &str = "agent://event";
const MAX_STEPS: u32 = 6;

/// Tauri command：前端调用启动 agent
#[tauri::command]
pub async fn run_agent(
    app: AppHandle,
    request: AgentRunRequest,
) -> Result<AgentRunResponse, String> {
    // 生产入口：进程启动时加载一次 .env
    let _ = dotenvy::dotenv();
    let selected = crate::model_factory::select_model();
    eprintln!("[agent] using model source: {}", selected.source);
    let router = ModelRouter::new(selected.model);
    let sink = TauriEventSink { app: &app };
    // 通过 app.state 取注册表，避免在 async 命令签名里带 State ref
    let registry = app.state::<SessionRegistry>();
    run_loop(&sink, registry.inner(), &router, request).await
}

/// 取消 Tauri command
#[tauri::command]
pub async fn cancel_agent(app: AppHandle, session_id: String) -> Result<bool, String> {
    let registry = app.state::<SessionRegistry>();
    Ok(registry.cancel(&session_id))
}

fn emit(app: &AppHandle, event: &AgentEvent) {
    if let Err(e) = app.emit(EVENT_CHANNEL, event) {
        eprintln!("[agent] emit failed: {e}");
    }
}

/// 事件接收抽象：让测试不依赖 Tauri AppHandle
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &AgentEvent);
}

pub struct TauriEventSink<'a> {
    pub app: &'a AppHandle,
}

impl<'a> EventSink for TauriEventSink<'a> {
    fn emit(&self, event: &AgentEvent) {
        emit(self.app, event);
    }
}

pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: &AgentEvent) {}
}

/// 纯函数版 Agent Loop（事件 sink 可注入）
/// 真实运行用 TauriEventSink；测试用 NoopEventSink
/// W3.2: async + 流式 + 取消
pub async fn run_loop(
    sink: &dyn EventSink,
    registry: &SessionRegistry,
    router: &ModelRouter,
    request: AgentRunRequest,
) -> Result<AgentRunResponse, String> {
    let started = std::time::Instant::now();
    let session_id = format!("sess_{}", now_millis());
    let cancel = registry.insert(session_id.clone());
    let _guard = RegistryGuard {
        registry,
        id: session_id.clone(),
    };

    let filter = KeywordFilter::new();

    // 入口审核
    match filter.check(&request.user_input) {
        SafetyVerdict::Block { reason } => {
            sink.emit(&AgentEvent::Error {
                session_id: session_id.clone(),
                message: format!("输入未通过审核：{}", reason),
            });
            return Ok(AgentRunResponse {
                session_id,
                level_id: request.level_id,
                final_answer: format!(
                    "🚫 小启觉得这个内容不太合适：{}\n换个其他有意思的想法吧～",
                    reason
                ),
                thoughts: vec!["入口审核拦截".to_string()],
                tool_calls: vec![],
                assets: vec![],
                duration_ms: started.elapsed().as_millis() as u64,
                model: router.primary_name().to_string(),
                steps: 0,
                tokens_used: 0,
                cancelled: false,
            });
        }
        SafetyVerdict::Warn { reason } => {
            eprintln!("[agent] warn on input: {}", reason);
        }
        SafetyVerdict::Pass => {}
    }

    sink.emit(&AgentEvent::Started {
        session_id: session_id.clone(),
    });

    let tool_registry = default_registry();
    let tools_desc = tool_registry.describe(&request.tools);
    let level_id_tag = format!("LEVEL_ID: {}", request.level_id);
    let system_prompt = format!(
        "{}\n\n[可用工具]\n{}\n[{}]\n",
        request.system_prompt, tools_desc, level_id_tag
    );

    let mut history: Vec<ModelMessage> = vec![ModelMessage {
        role: "user".to_string(),
        content: request.user_input.clone(),
        name: None,
        tool_call_id: None,
        tool_calls: None,
    }];

    let mut thoughts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<ToolCallRecord> = Vec::new();
    let mut assets: Vec<GeneratedAsset> = Vec::new();
    let mut final_answer = String::new();
    let mut step: u32 = 0;
    let mut last_error: Option<String> = None;
    let mut tokens_used: u32 = 0;
    let mut cancelled = false;

    while step < MAX_STEPS {
        // step 间的取消检查
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }

        step += 1;
        let model_req = ModelRequest {
            system_prompt: system_prompt.clone(),
            messages: history.clone(),
            allowed_tools: request.tools.clone(),
            temperature: 0.0,
        };

        let (decision, chunks) = match router.decide_stream(&model_req, cancel.clone()).await {
            Ok(v) => v,
            Err(e) if e == "cancelled" => {
                cancelled = true;
                break;
            }
            Err(e) => {
                last_error = Some(format!("model error: {e}"));
                break;
            }
        };
        tokens_used += decision.tokens_used;

        // 发射流式 chunks
        for chunk in chunks {
            sink.emit(&AgentEvent::Chunk {
                session_id: session_id.clone(),
                step,
                delta: chunk.text,
            });
        }

        thoughts.push(decision.thought.clone());
        sink.emit(&AgentEvent::Thought {
            session_id: session_id.clone(),
            step,
            thought: decision.thought.clone(),
        });

        // 把 assistant 的决策 push 进 history
        if let Some(t) = &decision.tool {
            let tool_call_id = decision
                .tool_call_id
                .clone()
                .unwrap_or_else(|| format!("call_{}", now_millis()));
            let args_str = decision.tool_args.clone().unwrap_or_else(|| "{}".to_string());
            let tool_calls_struct = vec![ModelToolCall {
                id: tool_call_id,
                name: t.clone(),
                args: args_str.clone(),
            }];
            let assistant_msg = format!(
                "[thought] {}\n[action] {}({})",
                decision.thought,
                t,
                args_str
            );
            history.push(ModelMessage {
                role: "assistant".to_string(),
                content: assistant_msg,
                name: None,
                tool_call_id: None,
                tool_calls: Some(tool_calls_struct),
            });
        } else {
            let assistant_msg = format!(
                "[thought] {}\n[final] {}",
                decision.thought,
                decision.final_answer.as_deref().unwrap_or("")
            );
            history.push(ModelMessage {
                role: "assistant".to_string(),
                content: assistant_msg,
                name: None,
                tool_call_id: None,
                tool_calls: None,
            });
        }

        if let Some(tool_name) = &decision.tool {
            let args_str = decision.tool_args.clone().unwrap_or_else(|| "{}".to_string());
            let args_val: serde_json::Value =
                serde_json::from_str(&args_str).unwrap_or(serde_json::Value::Null);

            if !request.tools.iter().any(|t| t == tool_name) {
                let err = format!("tool {} not in whitelist", tool_name);
                sink.emit(&AgentEvent::Error {
                    session_id: session_id.clone(),
                    message: err.clone(),
                });
                last_error = Some(err);
                break;
            }

            sink.emit(&AgentEvent::ToolCall {
                session_id: session_id.clone(),
                step,
                tool: tool_name.clone(),
                args: args_val.clone(),
            });

            let exec_result: Result<ToolOutput, String> = match tool_registry.get(tool_name) {
                Some(t) => t.execute(&args_str, &session_id),
                None => Err(format!("tool not found: {tool_name}")),
            };

            let (result_text, new_assets) = match exec_result {
                Ok(out) => {
                    for a in &out.assets {
                        assets.push(a.clone());
                    }
                    (out.result_text, out.assets)
                }
                Err(e) => {
                    let msg = format!("tool {tool_name} failed: {e}");
                    sink.emit(&AgentEvent::Error {
                        session_id: session_id.clone(),
                        message: msg.clone(),
                    });
                    last_error = Some(msg);
                    break;
                }
            };

            sink.emit(&AgentEvent::ToolResult {
                session_id: session_id.clone(),
                step,
                tool: tool_name.clone(),
                result: result_text.clone(),
                assets: new_assets,
            });

            tool_calls.push(ToolCallRecord {
                tool: tool_name.clone(),
                args: args_val,
                result: result_text.clone(),
                timestamp: now_millis(),
            });

            history.push(ModelMessage {
                role: "tool".to_string(),
                content: result_text,
                name: Some(tool_name.clone()),
                tool_call_id: decision.tool_call_id.clone(),
                tool_calls: None,
            });
        } else if let Some(ans) = &decision.final_answer {
            final_answer = ans.clone();
            sink.emit(&AgentEvent::FinalAnswer {
                session_id: session_id.clone(),
                answer: ans.clone(),
            });
            break;
        } else {
            let fallback = "（小启没有想好怎么回答，先给你这段鼓励吧～再试试看？）".to_string();
            final_answer = fallback.clone();
            sink.emit(&AgentEvent::FinalAnswer {
                session_id: session_id.clone(),
                answer: fallback,
            });
            break;
        }
    }

    // 出口阶段 — 取消可能在 tool / final 后到达
    if cancel.load(Ordering::Relaxed) && !cancelled {
        cancelled = true;
    }

    let duration_ms = started.elapsed().as_millis() as u64;

    if !final_answer.is_empty() && !cancelled {
        match filter.check(&final_answer) {
            SafetyVerdict::Block { reason } => {
                eprintln!("[agent] exit block: {}", reason);
                final_answer = "（小启想了一下，觉得这个回答不太合适，换个方向继续吧～）".to_string();
            }
            SafetyVerdict::Warn { reason } => {
                eprintln!("[agent] exit warn: {}", reason);
            }
            SafetyVerdict::Pass => {}
        }
    }

    if let Some(err) = last_error {
        sink.emit(&AgentEvent::Error {
            session_id: session_id.clone(),
            message: err.clone(),
        });
        if final_answer.is_empty() {
            final_answer = format!("（出错了：{}，但已生成部分内容）", err);
        }
    }

    if cancelled {
        sink.emit(&AgentEvent::Cancelled {
            session_id: session_id.clone(),
        });
        if final_answer.is_empty() {
            final_answer = "（已被用户取消）".to_string();
        }
    }

    sink.emit(&AgentEvent::Done {
        session_id: session_id.clone(),
        steps: step,
        duration_ms,
    });

    Ok(AgentRunResponse {
        session_id,
        level_id: request.level_id,
        final_answer,
        thoughts,
        tool_calls,
        assets,
        duration_ms,
        model: router.primary_name().to_string(),
        steps: step,
        tokens_used,
        cancelled,
    })
}

fn now_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// 让 default_registry 可以从 tools 模块导入
use crate::tools::default_registry;
