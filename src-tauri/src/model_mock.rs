// Mock 模型（W2.5 + W3.2 流式）
// 按关卡 + 输入生成确定的 ReAct 轨迹
// 当真实 LLM 接入时，这个实现会被替换
//
// W3.2: 改 async streaming；支持 MockConfig 注入 chunks / 取消行为

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;

use super::model::{Chunk, Model, ModelDecision, ModelRequest};
use super::model_openai::OaiToolCall;

/// Mock 模型行为配置
/// 默认 = 老 W2.5 行为：按 level_id + turn 生成确定轨迹，无 chunks
#[derive(Debug, Clone, Default)]
pub struct MockConfig {
    /// 流式 content deltas（按顺序发射）
    /// 若非空，走"先发 chunks，再发 decision"路径
    pub chunks: Vec<String>,
    /// 直接 final_answer（与 plan_step 互斥，优先级最高）
    pub final_answer: Option<String>,
    /// 直接 tool call（与 plan_step 互斥）
    pub tool_call: Option<OaiToolCall>,
    /// 每个 chunk 间 sleep 时长
    pub chunk_delay_ms: u64,
    /// 测试用：注入取消 flag（与 registry 的 cancel 同时检查）
    /// 设置后，外部 `flag.store(true)` 即可触发模型返回 Err("cancelled")
    pub cancel_flag: Option<Arc<AtomicBool>>,
}

pub struct MockModel {
    pub config: Mutex<MockConfig>,
}

impl MockModel {
    pub fn with_config(config: MockConfig) -> Self {
        Self {
            config: Mutex::new(config),
        }
    }
}

impl Default for MockModel {
    fn default() -> Self {
        Self::with_config(MockConfig::default())
    }
}

#[async_trait]
impl Model for MockModel {
    fn name(&self) -> String {
        "mock-1".to_string()
    }

    async fn decide_stream(
        &self,
        req: &ModelRequest,
        cancel: Arc<AtomicBool>,
    ) -> Result<(ModelDecision, Vec<Chunk>), String> {
        // 1) 配置模式：发射 chunks + 返回固定 decision
        let cfg = self.config.lock().unwrap().clone();
        if !cfg.chunks.is_empty() || cfg.final_answer.is_some() || cfg.tool_call.is_some() {
            return emit_configured(req, cfg, cancel).await;
        }

        // 2) 兼容老 W2.5 行为：按 level_id + turn 生成轨迹
        Ok((plan_based_decision(req), Vec::new()))
    }
}

async fn emit_configured(
    _req: &ModelRequest,
    cfg: MockConfig,
    cancel: Arc<AtomicBool>,
) -> Result<(ModelDecision, Vec<Chunk>), String> {
    let check_cancel = |flag: &Arc<AtomicBool>| flag.load(Ordering::Relaxed);
    let mut chunks: Vec<Chunk> = Vec::with_capacity(cfg.chunks.len());
    for text in &cfg.chunks {
        if check_cancel(&cancel) {
            return Err("cancelled".into());
        }
        if let Some(flag) = &cfg.cancel_flag {
            if check_cancel(flag) {
                return Err("cancelled".into());
            }
        }
        if cfg.chunk_delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(cfg.chunk_delay_ms)).await;
        }
        if check_cancel(&cancel) {
            return Err("cancelled".into());
        }
        if let Some(flag) = &cfg.cancel_flag {
            if check_cancel(flag) {
                return Err("cancelled".into());
            }
        }
        chunks.push(Chunk {
            text: text.clone(),
            step: 0,
        });
    }

    let decision = if let Some(tc) = cfg.tool_call {
        ModelDecision {
            thought: format!("调用 {}", tc.function.name),
            tool: Some(tc.function.name),
            tool_args: Some(tc.function.arguments),
            tool_call_id: Some(tc.id),
            final_answer: None,
            tokens_used: 50,
        }
    } else if let Some(ans) = cfg.final_answer {
        ModelDecision {
            thought: "直接给出最终回答".to_string(),
            tool: None,
            tool_args: None,
            tool_call_id: None,
            final_answer: Some(ans),
            tokens_used: 50,
        }
    } else {
        // 仅 chunks，无 final / tool：当作 final answer
        let full: String = cfg.chunks.iter().cloned().collect();
        ModelDecision {
            thought: "直接给出最终回答".to_string(),
            tool: None,
            tool_args: None,
            tool_call_id: None,
            final_answer: Some(full),
            tokens_used: 50,
        }
    };

    Ok((decision, chunks))
}

/// 老 W2.5 行为：按 level_id + tools + turn 决定
fn plan_based_decision(req: &ModelRequest) -> ModelDecision {
    let user_input = req
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    let level_id = extract_level_id(&req.system_prompt);
    let tools = &req.allowed_tools;
    let turn = req
        .messages
        .iter()
        .filter(|m| m.role == "assistant")
        .count();

    let (thought, tool, tool_args, final_answer) =
        plan_step(level_id.as_deref(), tools, &user_input, turn);

    ModelDecision {
        thought,
        tool,
        tool_args,
        tool_call_id: None,
        final_answer,
        tokens_used: 50,
    }
}

fn extract_level_id(prompt: &str) -> Option<String> {
    prompt
        .lines()
        .rev()
        .find(|l| l.starts_with("LEVEL_ID:"))
        .map(|l| l.trim_start_matches("LEVEL_ID:").trim().to_string())
}

fn plan_step(
    level_id: Option<&str>,
    tools: &[String],
    user_input: &str,
    turn: usize,
) -> (String, Option<String>, Option<String>, Option<String>) {
    let has_image = tools.iter().any(|t| t == "generate_image");
    let has_video = tools.iter().any(|t| t == "image_to_video");
    let has_tts = tools.iter().any(|t| t == "synthesize_speech");
    let has_sub = tools.iter().any(|t| t == "add_subtitle");
    let has_bgm = tools.iter().any(|t| t == "add_bgm");
    let has_chat = tools.iter().any(|t| t == "text_chat");

    // L1 / L3 / L4 / L5：图片 + 视频
    if has_image && has_video {
        return match (level_id, turn) {
            (_, 0) => (
                format!("我先根据小朋友的描述「{}」生成一张图～", user_input),
                Some("generate_image".to_string()),
                Some(format!(
                    r#"{{"prompt":"{}","style":"cartoon","width":1024,"height":576}}"#,
                    escape_json(user_input)
                )),
                None,
            ),
            (_, 1) => (
                "图生成好啦！现在让它动起来 🎬".to_string(),
                Some("image_to_video".to_string()),
                Some(r#"{"duration":5,"motion":"auto"}"#.to_string()),
                None,
            ),
            (_, 2) => (
                format!(
                    "完成！做了「{}」的 5 秒小视频。下次可以试着加更多 5 要素哦～",
                    user_input
                ),
                None,
                None,
                Some(build_final_answer(level_id, user_input, "5 秒小视频")),
            ),
            _ => (
                "好啦，今天就到这里吧～".to_string(),
                None,
                None,
                Some("好啦，今天就到这里吧～".to_string()),
            ),
        };
    }

    // L2：TTS + 字幕
    if has_tts && has_sub {
        return match turn {
            0 => (
                "我先把台词变成声音 🔊".to_string(),
                Some("synthesize_speech".to_string()),
                Some(format!(
                    r#"{{"text":"{}","voice":"child_friendly","emotion":"happy"}}"#,
                    escape_json(user_input)
                )),
                None,
            ),
            1 => (
                "声音生成好啦，再加点字幕让小朋友看清楚～".to_string(),
                Some("add_subtitle".to_string()),
                Some(format!(
                    r#"{{"text":"{}","position":"bottom"}}"#,
                    escape_json(user_input)
                )),
                None,
            ),
            _ => (
                "好啦，声音和字幕都加上啦！".to_string(),
                None,
                None,
                Some(build_final_answer(level_id, user_input, "配音 + 字幕")),
            ),
        };
    }

    // L4 后期：bgm
    if has_bgm && turn == 2 {
        return (
            "加一段背景音乐让视频更生动 🎵".to_string(),
            Some("add_bgm".to_string()),
            Some(r#"{"mood":"cheerful","volume":0.4}"#.to_string()),
            None,
        );
    }

    // 兜底：纯 chat
    if has_chat {
        return (
            format!("小朋友说了「{}」，我先陪他聊聊～", user_input),
            Some("text_chat".to_string()),
            Some(format!(r#"{{"message":"{}"}}"#, escape_json(user_input))),
            None,
        );
    }

    // 最终兜底
    (
        "我先理解一下小朋友的想法～".to_string(),
        None,
        None,
        Some(build_final_answer(level_id, user_input, "作品")),
    )
}

fn build_final_answer(level_id: Option<&str>, user_input: &str, asset_desc: &str) -> String {
    let n = level_id.unwrap_or("关卡");
    format!(
        "🎉 太棒啦！你在「{}」做出了自己的「{}」！\n\n你写的：\"{}\"\n\n下次可以试试：让画面更具体（颜色 / 数量 / 场景），效果会更好哦～",
        n, asset_desc, user_input
    )
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
