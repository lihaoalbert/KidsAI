// Agent Loop（W2.4 + W2.5 + W2.6 + W3.1 + W3.2 流式 + 取消 + W4.5 license 上报）
// ReAct 循环：Model -> Tool -> Observation -> ... -> Final Answer
// 事件流：每步通过 tauri::Emitter 发向前端
// W3.2: 改 async + 串接 SSE chunks + 支持 session 级取消
// W4.5 B2: 完成后向 server 上报学币 spend (llm 按 token, video 按次)
//         demo 模式 (KIDSAI_SERVER_URL 未设) → 不上报, 不影响现有测试

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::character::{
    build_system_prompt_with_character, inject_character_into_image_args,
    inject_character_into_video_args, Character,
};
use crate::license_client::LicenseClient;
use crate::model::{ModelMessage, ModelRequest, ModelRouter, ModelToolCall};
use crate::safety::{KeywordFilter, SafetyVerdict};
use crate::style::{
    build_system_prompt_with_style, inject_style_into_image_args, inject_style_into_video_args,
    StylePreset,
};
use crate::tools::{GeneratedAsset, ShotContext, ToolContext, ToolOutput};

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
    /// W3.4: 当前 session 绑定的角色 ID（可选）
    #[serde(default)]
    pub character_id: Option<String>,
    /// W3.6: 当前 session 绑定的视觉风格 ID（可选）
    #[serde(default)]
    pub style_id: Option<String>,
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
    // W3.4: 解析角色（按 character_id 查 CharacterRegistry）
    let character = request
        .character_id
        .as_ref()
        .and_then(|cid| app.state::<crate::character::CharacterRegistry>().get(cid));
    // W3.6: 解析风格（按 style_id 查 StyleRegistry）
    let style = request
        .style_id
        .as_ref()
        .and_then(|sid| app.state::<crate::style::StyleRegistry>().get(sid));
    // W11 Day 8: 在 move 进 run_loop 之前保留 user_input (上报 telemetry 用)
    let user_input_for_telemetry = request.user_input.clone();
    let level_id_for_telemetry = request.level_id.clone();
    let response = run_loop(&sink, registry.inner(), &router, request, character, style).await?;

    // W4.5 B2: 上报学币 spend (fire-and-forget)
    // - llm: tokens_used 按 LLM_API_COST_RATE 学币/token (server 侧定价, 这里按 cfg 同步对齐)
    // - video: 计数, 每个按 draft 9 / final 19 学币
    // demo 模式 (无 server) → noop, 不影响开发
    let license_client = app.state::<LicenseClient>().inner().clone();
    let license_file = app.state::<crate::license_store::LicenseStore>().load();
    if let Some(ref lf) = license_file {
        spawn_spend_report(
            license_client,
            lf.license_token.clone(),
            response.tokens_used,
            response.assets.clone(),
        );
    }

    // W11 Day 8: 上报 telemetry — AgentRun. 仅在 server 模式下有意义,
    // 失败 eprintln 而不阻塞. Child mode 含 input/output hash; Adult mode 强制 None.
    {
        let marketplace = app
            .state::<crate::marketplace_client::MarketplaceClient>()
            .inner()
            .clone();
        let device_id = license_file.as_ref().map(|lf| lf.device_id.clone());
        let level_id = response.level_id.clone();
        let final_answer = response.final_answer.clone();
        let outcome = if response.cancelled {
            "cancelled"
        } else if response.final_answer.is_empty() {
            "err"
        } else {
            "ok"
        };
        let _ = level_id_for_telemetry; // currently unused; keep for parity
        crate::telemetry::report(
            &marketplace,
            crate::telemetry::TelemetryEvent::AgentRun {
                call_id: response.session_id.clone(),
                level_id,
                agent_kind: "agent".to_string(),
                outcome: outcome.to_string(),
                latency_ms: response.duration_ms,
                // Child mode 全填; Adult mode 会被 wrap() 强制置 None
                input_hash: Some(short_hash(&user_input_for_telemetry)),
                output_hash: Some(short_hash(&final_answer)),
                satisfaction_signal: None,
                secret_version: None,
                skill_versions: None,
            },
            device_id,
        )
        .await;
    }

    Ok(response)
}

fn spawn_spend_report(
    client: LicenseClient,
    license_token: String,
    tokens_used: u32,
    assets: Vec<GeneratedAsset>,
) {
    if client.is_demo() {
        return;
    }
    tokio::spawn(async move {
        // LLM spend (1 学币/tokens 聚合一次上报)
        if tokens_used > 0 {
            let call_id = format!("llm-{}", now_millis());
            if let Err(e) = client
                .record_spend(&license_token, &call_id, "llm", tokens_used)
                .await
            {
                eprintln!("[agent] llm spend report failed: {e}");
            }
        }
        // W6: 资产按 kind + tool 分发到对应 server 端 kind
        for a in &assets {
            let spend_kind = match (a.kind.as_str(), a.tool.as_str()) {
                ("video", "image_to_video") => {
                    // video 按 model 区分 draft/final/hailuo
                    let m = a.model.as_deref().unwrap_or("");
                    if m.contains("mini") {
                        "video_draft"
                    } else if m.contains("hailuo") {
                        "hailuo_video"
                    } else {
                        "video_final"
                    }
                }
                ("image", "generate_image") => "image_gen",
                ("image", "edit_image") => "image_gen",
                ("audio", "synthesize_speech") => "tts", // 暂归 llm cost, 留 tts 单独 kind 后续
                ("audio", "music_gen") => "music_gen",
                _ => continue, // 其他资产不计费 (subtitle / bgm placeholder)
            };
            let call_id = format!("{}-{}-{}", spend_kind, now_millis(), short_hash(&a.url));
            if let Err(e) = client
                .record_spend(&license_token, &call_id, spend_kind, 1)
                .await
            {
                eprintln!("[agent] {spend_kind} spend report failed: {e}");
            }
        }
    });
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
/// W3.4: character 可选，绑定后 system_prompt 会追加角色段，generate_image 工具的 prompt 也会注入角色描述
/// W3.6: style 可选，与 character 并行叠加 — system_prompt 加风格段，generate_image prompt 加风格描述
pub async fn run_loop(
    sink: &dyn EventSink,
    registry: &SessionRegistry,
    router: &ModelRouter,
    request: AgentRunRequest,
    character: Option<Character>,
    style: Option<StylePreset>,
) -> Result<AgentRunResponse, String> {
    let started = std::time::Instant::now();
    let session_id = format!("sess_{}", now_millis());
    // W4.6 #5: 同 session 内跨视频调用共享 seed_session (跨镜一致性杠杆).
    // 用 session_id 哈希成 u64, 简单且确定. 不需要时设为 None.
    let seed_session: Option<u64> = Some(session_seed_from_id(&session_id));
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
    // W3.4: 角色 + W3.6: 风格 — 两个独立维度叠加，模型能稳定看到
    let with_character =
        build_system_prompt_with_character(&request.system_prompt, character.as_ref());
    let base_prompt = build_system_prompt_with_style(&with_character, style.as_ref());
    let system_prompt = format!(
        "{}\n\n[可用工具]\n{}\n[{}]\n",
        base_prompt, tools_desc, level_id_tag
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
            let args_str = decision
                .tool_args
                .clone()
                .unwrap_or_else(|| "{}".to_string());
            let tool_calls_struct = vec![ModelToolCall {
                id: tool_call_id,
                name: t.clone(),
                args: args_str.clone(),
            }];
            let assistant_msg = format!(
                "[thought] {}\n[action] {}({})",
                decision.thought, t, args_str
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
            let args_str = decision
                .tool_args
                .clone()
                .unwrap_or_else(|| "{}".to_string());
            // W3.4 + W3.6: 角色 + 风格 可独立叠加 — 4 种组合都覆盖到
            // v1 导演流程:同样扩到 image_to_video(自动填 image_url + image_role, 追加 motion 描述)
            let args_str = match (character.as_ref(), style.as_ref(), tool_name.as_str()) {
                (Some(c), Some(s), "generate_image") => {
                    let with_char = inject_character_into_image_args(&args_str, c);
                    inject_style_into_image_args(&with_char, s)
                }
                (Some(c), None, "generate_image") => inject_character_into_image_args(&args_str, c),
                (None, Some(s), "generate_image") => inject_style_into_image_args(&args_str, s),
                (Some(c), Some(s), "image_to_video") => {
                    let with_char = inject_character_into_video_args(&args_str, c);
                    inject_style_into_video_args(&with_char, s)
                }
                (Some(c), None, "image_to_video") => inject_character_into_video_args(&args_str, c),
                (None, Some(s), "image_to_video") => inject_style_into_video_args(&args_str, s),
                _ => args_str,
            };
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

            // W4.6 #5 + #4: 组装 ToolContext (character/style/seed_session/scene/shot).
            // 所有 image_to_video 调用都拿到相同的 seed_session 锁,
            // 让 build_seedance_prompt 的硬锚话术三件套跨镜生效.
            // W4.6 #4: 从 args_val 抽 mood/camera/beat/character_refs/transition_to_next 塞进 shot.
            let shot = ShotContext {
                beat: args_val
                    .get("beat")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                mood: args_val
                    .get("mood")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                camera: args_val
                    .get("camera")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                character_refs: args_val
                    .get("character_refs")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                transition_to_next: args_val
                    .get("transition_to_next")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };
            let ctx = ToolContext {
                character: character.clone(),
                style: style.clone(),
                seed_session,
                scene: None, // 场景资产: W4.6 #4 阶段 4 之前还没建 bg, 暂留 None
                shot: Some(shot),
            };
            let exec_result: Result<ToolOutput, String> = match tool_registry.get(tool_name) {
                Some(t) => t.execute_with_context(&args_str, &session_id, &ctx),
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
                final_answer =
                    "（小启想了一下，觉得这个回答不太合适，换个方向继续吧～）".to_string();
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

/// pub 包装, lib.rs 等 crate 外模块可调用
pub fn now_millis_pub() -> i64 {
    now_millis()
}

// 让 default_registry 可以从 tools 模块导入
use crate::tools::default_registry;

fn short_hash(s: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    format!("{:x}", h.finish())
}

/// W4.6 #5: 把 session_id 字符串哈希成 u64, 作为跨视频调用共享 seed.
/// 用 std DefaultHasher (与 short_hash 一致) — 不同 session 拿不同 seed,
/// 同 session 内多次 video_to_image 调用 seed 相同 (跨镜角色一致).
pub fn session_seed_from_id(id: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    id.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn insert_returns_independent_flags() {
        let reg = SessionRegistry::default();
        let f1 = reg.insert("s1".into());
        let f2 = reg.insert("s2".into());
        // 各自独立：翻转 f1 不影响 f2
        f1.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(f1.load(std::sync::atomic::Ordering::Relaxed));
        assert!(!f2.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[test]
    fn cancel_existing_session_returns_true() {
        let reg = SessionRegistry::default();
        let flag = reg.insert("s1".into());
        assert!(reg.cancel("s1"));
        assert!(flag.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[test]
    fn cancel_missing_session_returns_false() {
        let reg = SessionRegistry::default();
        assert!(!reg.cancel("never_inserted"));
    }

    #[test]
    fn cancel_is_idempotent() {
        let reg = SessionRegistry::default();
        reg.insert("s1".into());
        assert!(reg.cancel("s1"));
        assert!(reg.cancel("s1"), "second cancel should also return true");
    }

    #[test]
    fn remove_clears_session() {
        let reg = SessionRegistry::default();
        reg.insert("s1".into());
        reg.remove("s1");
        // remove 后 cancel 返回 false
        assert!(!reg.cancel("s1"));
    }

    #[test]
    fn insert_overwrites_previous_flag() {
        let reg = SessionRegistry::default();
        let f1 = reg.insert("s1".into());
        f1.store(true, std::sync::atomic::Ordering::Relaxed);
        // 重新 insert 同 id：返回新 flag，f1 被孤儿化但不再可访问
        let f2 = reg.insert("s1".into());
        assert!(!f2.load(std::sync::atomic::Ordering::Relaxed));
        // cancel 翻转新 flag
        reg.cancel("s1");
        assert!(f2.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[test]
    fn registry_guard_removes_session_on_drop() {
        use std::sync::Arc;
        let reg = SessionRegistry::default();
        let flag: Arc<std::sync::atomic::AtomicBool> = reg.insert("guarded".into());
        {
            let _guard = RegistryGuard {
                registry: &reg,
                id: "guarded".into(),
            };
            assert!(reg.cancel("guarded"));
            assert!(flag.load(std::sync::atomic::Ordering::Relaxed));
        }
        // guard 离开作用域 → 自动 remove
        assert!(
            !reg.cancel("guarded"),
            "guard should have removed the session"
        );
    }

    #[test]
    fn concurrent_inserts_and_cancels_do_not_panic() {
        use std::sync::Arc;
        use std::thread;
        let reg = Arc::new(SessionRegistry::default());
        let mut handles = vec![];
        for i in 0..10 {
            let r = reg.clone();
            handles.push(thread::spawn(move || {
                let id = format!("session_{i}");
                let _flag = r.insert(id.clone());
                r.cancel(&id);
                r.remove(&id);
            }));
        }
        for h in handles {
            h.join().expect("thread should not panic");
        }
        // 所有 session 都被清理
        for i in 0..10 {
            assert!(!reg.cancel(&format!("session_{i}")));
        }
    }

    /// v1 导演流程:agent.rs 路由 image_to_video 到 inject_character_into_video_args +
    /// inject_style_into_video_args。本测试模拟"LLM 决策 image_to_video + 有 character/style"路径
    /// 的最终 args_str(避开 run_loop 端到端测试,只验 dispatch 后的结果形状)。
    #[test]
    fn image_to_video_injection_dispatch_path() {
        use crate::character::inject_character_into_video_args;
        use crate::style::inject_style_into_video_args;

        // 模拟一个内置角色 + 风格
        let c = Character {
            id: "xiaoqi".into(),
            name: "小启".into(),
            description: "黄发女孩".into(),
            style_tags: vec!["cartoon".into()],
            reference_image_url: Some("https://picsum.photos/seed/xiaoqi-ref/512/512".into()),
            standard_image_url: None,
            aliases: None,
        };
        let s = StylePreset {
            id: "cartoon".into(),
            name: "卡通".into(),
            description: "明亮卡通风格".into(),
            style_tags: vec!["cartoon".into()],
            seedance_style_keyword: None,
        };

        // LLM 决策出的 image_to_video args(只有 motion)
        let llm_args = r#"{"motion":"跳跃","duration":4}"#;
        // 模拟 agent.rs match 块 (Some(c), Some(s), "image_to_video") 分支
        let with_char = inject_character_into_video_args(llm_args, &c);
        let final_args = inject_style_into_video_args(&with_char, &s);

        let v: serde_json::Value = serde_json::from_str(&final_args).unwrap();
        // 1) 自动填了 image_url
        assert_eq!(
            v["image_url"],
            "https://picsum.photos/seed/xiaoqi-ref/512/512"
        );
        // 2) 自动设了 image_role
        assert_eq!(v["image_role"], "reference_image");
        // 3) motion 包含 motion 原文 + 角色描述 + 风格描述
        let motion = v["motion"].as_str().unwrap();
        assert!(motion.contains("跳跃"), "原 motion 保留");
        assert!(motion.contains("小启"), "角色名注入");
        assert!(motion.contains("黄发女孩"), "角色描述注入");
        assert!(motion.contains("明亮卡通风格"), "风格描述注入");
        // 4) duration 透传
        assert_eq!(v["duration"], 4);
    }
}
