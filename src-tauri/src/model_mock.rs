// Mock 模型（W2.5）
// 按关卡 + 输入生成确定的 ReAct 轨迹
// 当真实 LLM 接入时，这个实现会被替换

use super::model::{Model, ModelDecision, ModelRequest};

pub struct MockModel;

impl Model for MockModel {
    fn name(&self) -> &'static str {
        "mock-1"
    }

    fn decide(&self, req: &ModelRequest) -> Result<ModelDecision, String> {
        let user_input = req
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // 决定本轮做什么：基于 system_prompt 中夹带的关卡 ID
        // MVP：关卡 ID 在 system_prompt 末尾以 `LEVEL_ID: L1` 形式附带
        let level_id = extract_level_id(&req.system_prompt);
        let tools = &req.allowed_tools;

        // 第几轮：累计 assistant 数
        let turn = req
            .messages
            .iter()
            .filter(|m| m.role == "assistant")
            .count();

        // 根据关卡 + 工具白名单生成计划
        let (thought, tool, tool_args, final_answer) =
            plan_step(level_id.as_deref(), tools, &user_input, turn);

        Ok(ModelDecision {
            thought,
            tool,
            tool_args,
            final_answer,
            tokens_used: 50, // mock 不计真实 token
        })
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

fn build_final_answer(
    level_id: Option<&str>,
    user_input: &str,
    asset_desc: &str,
) -> String {
    let n = level_id.unwrap_or("关卡");
    format!(
        "🎉 太棒啦！你在「{}」做出了自己的「{}」！\n\n你写的：\"{}\"\n\n下次可以试试：让画面更具体（颜色 / 数量 / 场景），效果会更好哦～",
        n, asset_desc, user_input
    )
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
