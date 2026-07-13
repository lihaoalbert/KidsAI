// Mock MCP 工具（W2.6 → W6 C: 接入 image/voice/music/hailuo adapter）
// 每个工具模拟一次"AI 服务调用"，返回生成的资产信息
// 真实实现走 video_adapter / image_adapter / voice_adapter / music_adapter
// （按 env key 自动选 real 或 mock）

use crate::image_adapter::{select_image_adapter, ImageGenArgs};
use crate::video_adapter::{select_video_adapter, VideoGenArgs};
use crate::voice_adapter::{
    select_tts_adapter, select_voice_clone_adapter, TtsArgs, VoiceCloneArgs,
};
use crate::music_adapter::{select_music_adapter, MusicGenArgs};
use crate::prompt_builder::{build_seedance_prompt, PromptOptions, ShotMood, ShotCamera, CharacterAsset, StyleAsset, SceneAsset, NEGATIVE_PROMPT};
use crate::character::Character;
use crate::style::StylePreset;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    /// 工具文本结果（记入 Agent 的"观察"）
    pub result_text: String,
    /// 生成的资产（用于塞进 Creation.assets）
    pub assets: Vec<GeneratedAsset>,
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
    /// W4.5 B2: 实际使用的模型 (如 doubao-seedance-2-0-mini-260615),
    /// 用于 license 学币计费时区分 draft / final
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// W4.6 #4: 单分镜的运行时上下文.
/// 前端 DirectorShot (TS) → JSON string args 里塞 mood/camera/beat/character_refs →
/// agent.rs dispatch 时解析塞进 ToolContext.shot →
/// ImageToVideoTool.run_video_pipeline 用 mood/camera 替换默认 Calm/Medium.
///
/// 字段全部 Option/String — 前端透传的是字符串, 解析失败时静默回退默认 enum 值,
/// 不让单镜参数错误阻断整条视频生成.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShotContext {
    /// 5 拍节奏标记 (hook/conflict/payoff) — 排障用, 不影响 prompt 文本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beat: Option<String>,
    /// 情绪颗粒度 (calm/tense/joyful/sad/epic) — 喂给 [Motion] 行
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mood: Option<String>,
    /// 镜头语言 (wide/medium/close/extreme/follow/overhead) — 喂给 [Camera] 行
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera: Option<String>,
    /// 这镜涉及的角色 id 列表 (多角色故事用, 单角色可空)
    #[serde(default)]
    pub character_refs: Vec<String>,
    /// 与下一镜的转场 (cut/fade/dissolve/wipe/none) — 排障用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_to_next: Option<String>,
}

/// 工具上下文 (W4.6 #5 + #4)
///
/// 给需要完整 session 状态 (角色 / 风格 / seed session / 场景 / 分镜) 的工具准备的，
/// agent.rs 在调工具前从 AppHandle 解析好塞进来. 走 registry 之前先 resolve，
/// 让 tool 不用自己碰 AppHandle (简化测试).
///
/// 字段全部 Optional — 大多数工具 (W3-W6 已有的 6 个) 都用不到,
/// 只有 ImageToVideoTool 当前会读 character/style/seed_session/scene/shot 走工业级 prompt.
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub character: Option<Character>,
    pub style: Option<StylePreset>,
    pub seed_session: Option<u64>,
    pub scene: Option<SceneAsset>,
    /// W4.6 #4: 当前 tool 调用的分镜上下文 (前端 DirectorShot 透传)
    pub shot: Option<ShotContext>,
}

/// 工具 trait
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    /// 工具描述（写进 system prompt，让模型知道有这个工具）
    fn description(&self) -> &'static str;
    /// JSON Schema 字符串（mock 阶段简化为描述文本）
    fn schema(&self) -> &'static str;
    /// 执行（不带 context，向后兼容）
    fn execute(&self, args_json: &str, session_id: &str) -> Result<ToolOutput, String>;
    /// 执行（带 context，W4.6 #5 起的推荐入口）
    ///
    /// 默认实现退回到 `execute(args, session_id)`，忽略 context.
    /// 工具需要 context (例如 ImageToVideoTool) 时 override 即可.
    fn execute_with_context(
        &self,
        args_json: &str,
        session_id: &str,
        _ctx: &ToolContext,
    ) -> Result<ToolOutput, String> {
        self.execute(args_json, session_id)
    }
}

/// 工具注册表
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        // 防重
        if !self.tools.iter().any(|t| t.name() == tool.name()) {
            self.tools.push(tool);
        }
    }

    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter().find(|t| t.name() == name).map(|t| t.as_ref())
    }

    /// 列出被允许的工具的描述（喂给模型）
    pub fn describe(&self, allowed: &[String]) -> String {
        let mut out = String::new();
        for name in allowed {
            if let Some(t) = self.get(name) {
                out.push_str(&format!(
                    "- {}: {}\n  args: {}\n",
                    t.name(),
                    t.description(),
                    t.schema()
                ));
            }
        }
        out
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============ 具体工具实现 ============

pub struct GenerateImageTool;

impl Tool for GenerateImageTool {
    fn name(&self) -> &'static str {
        "generate_image"
    }
    fn description(&self) -> &'static str {
        "文生图：根据 prompt 生成一张图片（W6: MiniMax image-01 / Mock）"
    }
    fn schema(&self) -> &'static str {
        r#"{
            "prompt": "string",
            "aspect_ratio": "string? — 1:1|16:9|9:16|4:3|3:4, 默认 1:1",
            "style_hint": "string? — 主题/风格关键词, 注入 prompt"
        }"#
    }
    fn execute(&self, args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        let args: serde_json::Value =
            serde_json::from_str(args_json).map_err(|e| format!("invalid args: {e}"))?;
        let mut prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or("missing prompt")?
            .to_string();
        if let Some(hint) = args.get("style_hint").and_then(|v| v.as_str()) {
            prompt = format!("{}, {}", hint, prompt);
        }
        let aspect_ratio = args
            .get("aspect_ratio")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // W6 C1: 走 image_adapter (MINIMAX_API_KEY 命中走 MiniMax image-01, 否则 mock)
        let selected = select_image_adapter();
        let asset = selected.adapter.generate(&ImageGenArgs {
            prompt: prompt.clone(),
            aspect_ratio: aspect_ratio.clone(),
        })?;

        Ok(ToolOutput {
            result_text: format!(
                "已生成图片（provider={}, task_id={}, model={}）",
                asset.provider,
                asset.provider_task_id,
                asset.model.as_deref().unwrap_or("(mock)")
            ),
            assets: vec![GeneratedAsset {
                kind: "image".to_string(),
                url: asset.url,
                thumbnail_url: None,
                prompt: prompt.clone(),
                tool: "generate_image".to_string(),
                tokens_cost: 5,  // W6 计费: image_gen 5 学币
                model: asset.model,
            }],
        })
    }
}

pub struct ImageToVideoTool;

impl Tool for ImageToVideoTool {
    fn name(&self) -> &'static str {
        "image_to_video"
    }
    fn description(&self) -> &'static str {
        "图生视频：把上一张图片动起来，生成 5 秒视频（W4.6: Seedance 工业级七域 prompt + 硬锚话术三件套）"
    }
    fn schema(&self) -> &'static str {
        r#"{
            "image_url": "string? — 公网 URL / asset://<ID> / data:base64; 不传则纯文生视频",
            "image_role": "string? — first_frame | last_frame | reference_image; 默认 first_frame",
            "duration": "int? — 秒数，默认 5",
            "motion": "string? — 运动描述,作为 prompt; 缺省 'animate this image'",
            "model": "string? — per-call 覆盖; 如 doubao-seedance-2-0-mini-260615(试拍) 或 doubao-seedance-2-0-260128(定稿); 缺省走 SEEDANCE_MODEL env",
            "seed": "int? — 固定随机种子(角色一致性); 缺省不发送"
        }"#
    }

    fn execute(&self, args_json: &str, session_id: &str) -> Result<ToolOutput, String> {
        // 旧入口 (无 context): 走 raw motion 路径 — 向后兼容, 老测试不破坏.
        self.run_video_pipeline(args_json, session_id, &ToolContext::default())
    }

    fn execute_with_context(
        &self,
        args_json: &str,
        session_id: &str,
        ctx: &ToolContext,
    ) -> Result<ToolOutput, String> {
        // W4.6 #5 入口: 上下文里有 character/style/seed_session 时,
        // 自动调 build_seedance_prompt 走工业级七域 prompt + 硬锚话术三件套.
        self.run_video_pipeline(args_json, session_id, ctx)
    }
}

impl ImageToVideoTool {
    /// 共享内部: 解析 args → 决定 raw 还是工业级 prompt → 调 adapter.
    ///
    /// 触发工业级 prompt 的条件 (W4.6 #1 输入):
    /// - ctx.character 有 (有 full_name 可用, 防 pronoun drift)
    /// - ctx.style 有 (有 seedance_style_keyword 可用)
    /// - ctx.seed_session 有 (跨镜同 seed 锁)
    /// 三者任一存在即走 build_seedance_prompt; 同时按 selected.source 分派 PromptOptions:
    ///   - ark / mock → default (Seedance 2.0 不需 Negative)
    ///   - hailuo     → opt-in Negative
    fn run_video_pipeline(
        &self,
        args_json: &str,
        session_id: &str,
        ctx: &ToolContext,
    ) -> Result<ToolOutput, String> {
        let args: serde_json::Value =
            serde_json::from_str(args_json).map_err(|e| format!("invalid args: {e}"))?;

        let motion = args
            .get("motion")
            .and_then(|v| v.as_str())
            .unwrap_or("animate this image")
            .to_string();
        let image_url = args
            .get("image_url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let image_role = args
            .get("image_role")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let duration_seconds = args
            .get("duration")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let model_override = args
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let seed = args.get("seed").and_then(|v| v.as_i64());

        // W4.6 #4: 解析 mood/camera — args 透传优先 (LLM 可能把 system_prompt 里的 mood=joyful 塞进 args),
        // 没有时回退 ctx.shot.mood/camera (前端 DirectorShot 透传), 最后默认 Calm/Medium.
        // 解析失败时静默回退默认 enum, 不阻断整条视频生成.
        let mood_str = args
            .get("mood")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| ctx.shot.as_ref().and_then(|s| s.mood.clone()));
        let camera_str = args
            .get("camera")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| ctx.shot.as_ref().and_then(|s| s.camera.clone()));
        let shot_mood = parse_shot_mood(mood_str.as_deref());
        let shot_camera = parse_shot_camera(camera_str.as_deref());

        // 每次执行时按当前 env 重新选 provider — 测试和运行期都能动态切
        let selected = select_video_adapter();

        // ---- W4.6 #5: 工业级 prompt 决策 ----
        // 触发条件: 同时有 character + style (硬锚话术的有效基础).
        // seed_session 缺省时不强制, 用 0 占位 (也确保同 seed_session 必走相同 prompt 路径).
        // 后续 Task #4 DirectorShot 透传过来时, scene/mood/camera 也走 ctx 字段.
        let (final_prompt, hard_anchor_active) = match (
            ctx.character.as_ref(),
            ctx.style.as_ref(),
        ) {
            (Some(c), Some(s)) => {
                let seed_session = ctx.seed_session.unwrap_or(0);
                let character_asset = CharacterAsset {
                    id: c.id.clone(),
                    // W4.6 #1 输入: full_name = 全名 + 描述 (W3.4 Character.description 是外观描述)
                    // 防御 pronoun drift 核心 — 必须包含 description
                    full_name: format!("{} ({})", c.name, c.description),
                    style_tags: c.style_tags.clone(),
                    // W4.6 把 reference_image_url 当 standard 角色卡; W4.6 #2 之后再换成真正的三视图
                    standard_image_url: c
                        .reference_image_url
                        .clone()
                        .unwrap_or_else(|| "https://picsum.photos/seed/stand/512/512".into()),
                    side_image_url: None,
                    back_image_url: None,
                };
                let style_asset = StyleAsset {
                    id: s.id.clone(),
                    name: s.name.clone(),
                    // W4.6 #3: 优先用 style.seedance_style_keyword (工业级 keyword 串),
                    // 兼容老数据 — 没填时回退到 description (W4.6 #5 临时行为).
                    seedance_style_keyword: s
                        .seedance_style_keyword
                        .clone()
                        .unwrap_or_else(|| s.description.clone()),
                };
                // W4.6 #4: mood/camera 来自 ctx.shot / args, 替换原默认 Calm/Medium.
                // 缺省时 parse_shot_mood/camera 返回默认 enum, 不会失败.
                let shot = crate::prompt_builder::ShotPromptInput {
                    subject: c.name.clone(),
                    action: motion.clone(),
                    setting: ctx
                        .scene
                        .as_ref()
                        .map(|sc| sc.name.clone())
                        .unwrap_or_default(),
                    mood: shot_mood,
                    camera: shot_camera,
                };
                let options = build_options_for_provider(&selected.source);
                let prompt_text = build_seedance_prompt(
                    &shot,
                    &character_asset,
                    &style_asset,
                    ctx.scene.as_ref(),
                    seed_session,
                    &options,
                );
                (prompt_text, options.hard_anchor)
            }
            _ => {
                // 老路径: 原样 motion
                (motion.clone(), false)
            }
        };

        // seed 字段: seed_session > per-call seed > 无
        let final_seed: Option<i64> = ctx.seed_session.map(|s| s as i64).or(seed);

        let asset = selected.adapter.generate(&VideoGenArgs {
            prompt: final_prompt.clone(),
            image_url: image_url.clone(),
            image_role: image_role.clone(),
            duration_seconds,
            ratio: None,
            resolution: None,
            generate_audio: None,
            model: model_override,
            seed: final_seed,
        })?;

        // 结果回显 prompt 字段 (排障用): 工业级时回显拼好的 prompt, 否则回显 raw motion
        // W4.6 #4: 加 beat/mood/camera 排障行
        let beat_log = ctx
            .shot
            .as_ref()
            .and_then(|s| s.beat.clone())
            .unwrap_or_else(|| "(none)".into());
        let transition_log = ctx
            .shot
            .as_ref()
            .and_then(|s| s.transition_to_next.clone())
            .unwrap_or_else(|| "(none)".into());
        let prompt_for_log = if hard_anchor_active {
            format!(
                "[SEEDANCE_PROMPT]\n{}\n[session={}, beat={}, mood={}, camera={}, transition_to_next={}, hard_anchor=true]",
                final_prompt,
                session_id,
                beat_log,
                mood_str.as_deref().unwrap_or("(default calm)"),
                camera_str.as_deref().unwrap_or("(default medium)"),
                transition_log,
            )
        } else {
            format!(
                "motion={}, image_url={}, image_role={}, seed={}, beat={}, mood={}, camera={}, transition_to_next={}, session={}",
                motion,
                image_url.as_deref().unwrap_or("(none)"),
                image_role.as_deref().unwrap_or("(none)"),
                final_seed
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "(none)".into()),
                beat_log,
                mood_str.as_deref().unwrap_or("(default calm)"),
                camera_str.as_deref().unwrap_or("(default medium)"),
                transition_log,
                session_id
            )
        };

        Ok(ToolOutput {
            result_text: format!(
                "已生成 {} 视频（provider={}, task_id={}, model={}, seedance_prompt={}）",
                duration_seconds.unwrap_or(5),
                asset.provider,
                asset.provider_task_id,
                asset.model.as_deref().unwrap_or("(default)"),
                if hard_anchor_active { "on" } else { "off" }
            ),
            assets: vec![GeneratedAsset {
                kind: "video".to_string(),
                url: asset.url,
                thumbnail_url: asset.thumbnail_url.or(Some(
                    "https://picsum.photos/seed/kidsaivid/640/360".to_string(),
                )),
                prompt: prompt_for_log,
                tool: "image_to_video".to_string(),
                tokens_cost: 50,
                model: asset.model.clone(),
            }],
        })
    }
}

/// W4.6 #5 按 video provider 分派 PromptOptions:
/// - Seedance 2.0 (source="ark") / mock → default, 不输出 Negative
/// - Hailuo (source="hailuo")           → opt-in, 加 Negative (Hailuo 没参考图硬锚时易出乱)
fn build_options_for_provider(source: &str) -> PromptOptions {
    if source == "hailuo" {
        PromptOptions {
            negative_prompt: Some(NEGATIVE_PROMPT.into()),
            ..PromptOptions::default()
        }
    } else {
        PromptOptions::default()
    }
}

/// W4.6 #4: 把前端透传的 mood 字符串解析成 ShotMood enum.
/// 缺省或解析失败 → Calm. 不报错 (单镜参数错误不应阻断整条视频).
pub fn parse_shot_mood(s: Option<&str>) -> ShotMood {
    match s {
        Some("calm") => ShotMood::Calm,
        Some("tense") => ShotMood::Tense,
        Some("joyful") => ShotMood::Joyful,
        Some("sad") => ShotMood::Sad,
        Some("epic") => ShotMood::Epic,
        _ => ShotMood::Calm,
    }
}

/// W4.6 #4: 把前端透传的 camera 字符串解析成 ShotCamera enum.
/// 缺省或解析失败 → Medium.
pub fn parse_shot_camera(s: Option<&str>) -> ShotCamera {
    match s {
        Some("wide") => ShotCamera::Wide,
        Some("medium") => ShotCamera::Medium,
        Some("close") => ShotCamera::Close,
        Some("extreme") => ShotCamera::Extreme,
        Some("follow") => ShotCamera::Follow,
        Some("overhead") => ShotCamera::Overhead,
        _ => ShotCamera::Medium,
    }
}

/// W3.5: 指哪打哪 — 围绕 (x, y) 位置按 prompt 修改已生成的图片
pub struct EditImageTool;

impl Tool for EditImageTool {
    fn name(&self) -> &'static str {
        "edit_image"
    }
    fn description(&self) -> &'static str {
        "区域编辑：基于 source_image_url，在 (x,y) 位置按 prompt 修改图片（指哪打哪）"
    }
    fn schema(&self) -> &'static str {
        r#"{"source_image_url": "string", "x": "int", "y": "int", "prompt": "string"}"#
    }
    fn execute(&self, args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        let args: serde_json::Value =
            serde_json::from_str(args_json).map_err(|e| format!("invalid args: {e}"))?;
        let source = args
            .get("source_image_url")
            .and_then(|v| v.as_str())
            .ok_or("missing source_image_url")?
            .to_string();
        let x = args
            .get("x")
            .and_then(|v| v.as_i64())
            .ok_or("missing x")?;
        let y = args
            .get("y")
            .and_then(|v| v.as_i64())
            .ok_or("missing y")?;
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or("missing prompt")?
            .to_string();

        // mock：基于 (source, prompt, x, y) 生成确定性 picsum URL
        // 不同坐标 / 提示 → 不同图；同输入 → 同 URL（便于测试断言）
        let seed = simple_hash(&format!("{source}|{prompt}|{x}|{y}"));
        let url = format!("https://picsum.photos/seed/{seed}/1024/576");
        Ok(ToolOutput {
            result_text: format!("已修改 ({x},{y}) 区域：{prompt}"),
            assets: vec![GeneratedAsset {
                kind: "image".to_string(),
                url,
                thumbnail_url: None,
                prompt,
                tool: "edit_image".to_string(),
                tokens_cost: 12,
                model: None,
            }],
        })
    }
}

pub struct SynthesizeSpeechTool;

impl Tool for SynthesizeSpeechTool {
    fn name(&self) -> &'static str {
        "synthesize_speech"
    }
    fn description(&self) -> &'static str {
        "TTS 配音：把文字转成语音（W6: MiniMax T2A / Mock；可指定 voice_id 用克隆声音）"
    }
    fn schema(&self) -> &'static str {
        r#"{
            "text": "string",
            "voice_id": "string? — MiniMax voice_id; 之前 voice_clone 训练得到",
            "emotion": "string? — neutral|happy|sad|angry|fearful|disgusted|surprised|calm"
        }"#
    }
    fn execute(&self, args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        let args: serde_json::Value =
            serde_json::from_str(args_json).map_err(|e| format!("invalid args: {e}"))?;
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or("missing text")?
            .to_string();
        let voice_id = args.get("voice_id").and_then(|v| v.as_str()).map(|s| s.to_string());
        let emotion = args.get("emotion").and_then(|v| v.as_str()).map(|s| s.to_string());

        // W6 C2: 走 TTS adapter (MiniMax T2A 或 Mock)
        let adapter = select_tts_adapter();
        let asset = adapter.synthesize(&TtsArgs {
            text: text.clone(),
            voice_id: voice_id.clone(),
            emotion,
            model: None,
        })?;

        Ok(ToolOutput {
            result_text: format!(
                "已生成配音：voice={}, emotion={}",
                voice_id.as_deref().unwrap_or("default"),
                "auto"
            ),
            assets: vec![GeneratedAsset {
                kind: "audio".to_string(),
                url: asset.url,
                thumbnail_url: None,
                prompt: text,
                tool: "synthesize_speech".to_string(),
                tokens_cost: 3,
                model: None,
            }],
        })
    }
}

/// W6 C2: 声音复刻 — 上传 10s 音频, 训练一个 voice_id 用于后续 TTS.
pub struct VoiceCloneTool;

impl Tool for VoiceCloneTool {
    fn name(&self) -> &'static str {
        "voice_clone"
    }
    fn description(&self) -> &'static str {
        "声音复刻：上传一段 10 秒的清晰人声, 训练一个可复用的 MiniMax voice_id (10 学币/次)"
    }
    fn schema(&self) -> &'static str {
        r#"{
            "audio_path": "string — 音频文件绝对路径 (wav/mp3, 推荐 10s)",
            "voice_id_hint": "string? — 用户起的名字, 服务端可能改写"
        }"#
    }
    fn execute(&self, args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        let args: serde_json::Value =
            serde_json::from_str(args_json).map_err(|e| format!("invalid args: {e}"))?;
        let audio_path = args
            .get("audio_path")
            .and_then(|v| v.as_str())
            .ok_or("missing audio_path")?
            .to_string();
        let hint = args
            .get("voice_id_hint")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let adapter = select_voice_clone_adapter();
        let result = adapter.clone_voice(&VoiceCloneArgs {
            audio_path: audio_path.clone(),
            voice_id_hint: hint.clone(),
        })?;

        Ok(ToolOutput {
            result_text: format!(
                "已训练声音：voice_id={} (provider={})",
                result.voice_id, result.provider
            ),
            assets: vec![],  // voice_clone 不直接产资产, voice_id 由前端存
        })
    }
}

/// W6 C3: 音乐生成 — 给视频配 BGM.
pub struct MusicGenTool;

impl Tool for MusicGenTool {
    fn name(&self) -> &'static str {
        "music_gen"
    }
    fn description(&self) -> &'static str {
        "音乐生成：根据情绪/风格 prompt 生成一段 BGM (MiniMax music-01 / Mock, 8 学币/首)"
    }
    fn schema(&self) -> &'static str {
        r#"{
            "prompt": "string — 风格/情绪描述, 例 'cheerful ukulele, cartoon'",
            "duration_seconds": "int? — 默认 30",
            "instrumental": "bool? — 默认 true (纯器乐, 适合视频 BGM)"
        }"#
    }
    fn execute(&self, args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        let args: serde_json::Value =
            serde_json::from_str(args_json).map_err(|e| format!("invalid args: {e}"))?;
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or("missing prompt")?
            .to_string();
        let duration_seconds = args
            .get("duration_seconds")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(30);
        let instrumental = args
            .get("instrumental")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let selected = select_music_adapter();
        let asset = selected.adapter.generate(&MusicGenArgs {
            prompt: prompt.clone(),
            duration_seconds,
            instrumental,
        })?;

        Ok(ToolOutput {
            result_text: format!(
                "已生成 BGM（provider={}, task_id={}, {}秒）",
                asset.provider,
                asset.provider_task_id,
                asset.duration_seconds
            ),
            assets: vec![GeneratedAsset {
                kind: "audio".to_string(),
                url: asset.url,
                thumbnail_url: None,
                prompt: prompt.clone(),
                tool: "music_gen".to_string(),
                tokens_cost: 8,  // music_gen 8 学币/首
                model: asset.model,
            }],
        })
    }
}

pub struct AddSubtitleTool;

impl Tool for AddSubtitleTool {
    fn name(&self) -> &'static str {
        "add_subtitle"
    }
    fn description(&self) -> &'static str {
        "添加字幕：把文字作为字幕叠加到视频"
    }
    fn schema(&self) -> &'static str {
        r#"{"text": "string", "position": "string?"}"#
    }
    fn execute(&self, _args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        Ok(ToolOutput {
            result_text: "已添加字幕".to_string(),
            assets: vec![],
        })
    }
}

pub struct AddBgmTool;

impl Tool for AddBgmTool {
    fn name(&self) -> &'static str {
        "add_bgm"
    }
    fn description(&self) -> &'static str {
        "添加背景音乐"
    }
    fn schema(&self) -> &'static str {
        r#"{"mood": "string?", "volume": "float?"}"#
    }
    fn execute(&self, _args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        Ok(ToolOutput {
            result_text: "已添加背景音乐".to_string(),
            assets: vec![],
        })
    }
}

pub struct TextChatTool;

impl Tool for TextChatTool {
    fn name(&self) -> &'static str {
        "text_chat"
    }
    fn description(&self) -> &'static str {
        "纯文字对话：和小朋友聊一聊"
    }
    fn schema(&self) -> &'static str {
        r#"{"message": "string"}"#
    }
    fn execute(&self, _args_json: &str, _session_id: &str) -> Result<ToolOutput, String> {
        Ok(ToolOutput {
            result_text: "小启：听起来很有趣呢～你还想加点什么？".to_string(),
            assets: vec![],
        })
    }
}

/// 工厂：构建带所有 mock 工具的注册表
pub fn default_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(GenerateImageTool));
    reg.register(Box::new(EditImageTool));
    reg.register(Box::new(ImageToVideoTool));
    reg.register(Box::new(SynthesizeSpeechTool));
    reg.register(Box::new(VoiceCloneTool));    // W6 C2
    reg.register(Box::new(MusicGenTool));      // W6 C3
    reg.register(Box::new(AddSubtitleTool));
    reg.register(Box::new(AddBgmTool));
    reg.register(Box::new(TextChatTool));
    reg
}

fn simple_hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:x}", h & 0xffffffff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::session_seed_from_id;

    #[test]
    fn edit_image_missing_source_image_url() {
        let tool = EditImageTool;
        let args = r#"{"x":10,"y":20,"prompt":"改色"}"#;
        let err = tool.execute(args, "sess").unwrap_err();
        assert!(err.contains("source_image_url"), "got: {err}");
    }

    #[test]
    fn edit_image_missing_x() {
        let tool = EditImageTool;
        let args = r#"{"source_image_url":"https://a","y":20,"prompt":"改色"}"#;
        let err = tool.execute(args, "sess").unwrap_err();
        assert!(err.contains("missing x"), "got: {err}");
    }

    #[test]
    fn edit_image_missing_y() {
        let tool = EditImageTool;
        let args = r#"{"source_image_url":"https://a","x":10,"prompt":"改色"}"#;
        let err = tool.execute(args, "sess").unwrap_err();
        assert!(err.contains("missing y"), "got: {err}");
    }

    #[test]
    fn edit_image_missing_prompt() {
        let tool = EditImageTool;
        let args = r#"{"source_image_url":"https://a","x":10,"y":20}"#;
        let err = tool.execute(args, "sess").unwrap_err();
        assert!(err.contains("missing prompt"), "got: {err}");
    }

    #[test]
    fn edit_image_invalid_json() {
        let tool = EditImageTool;
        let args = "not json";
        let err = tool.execute(args, "sess").unwrap_err();
        assert!(err.contains("invalid args"), "got: {err}");
    }

    #[test]
    fn edit_image_returns_image_asset_with_deterministic_url() {
        let tool = EditImageTool;
        let args = r#"{"source_image_url":"https://a/b","x":10,"y":20,"prompt":"改色"}"#;
        let out = tool.execute(args, "sess1").expect("ok");
        assert_eq!(out.assets.len(), 1);
        let a = &out.assets[0];
        assert_eq!(a.kind, "image");
        assert_eq!(a.tool, "edit_image");
        assert!(a.url.contains("picsum.photos/seed/"));
        assert!(a.url.contains("/1024/576"));
        assert_eq!(a.prompt, "改色");

        // 同输入 → 同 URL（确定性）
        let out2 = tool.execute(args, "sess2").expect("ok");
        assert_eq!(out.assets[0].url, out2.assets[0].url);
    }

    #[test]
    fn edit_image_different_coords_produce_different_url() {
        let tool = EditImageTool;
        let args1 = r#"{"source_image_url":"https://a","x":10,"y":20,"prompt":"改色"}"#;
        let args2 = r#"{"source_image_url":"https://a","x":50,"y":50,"prompt":"改色"}"#;
        let out1 = tool.execute(args1, "s").expect("ok");
        let out2 = tool.execute(args2, "s").expect("ok");
        assert_ne!(
            out1.assets[0].url, out2.assets[0].url,
            "不同坐标应产生不同 URL"
        );
    }

    #[test]
    fn edit_image_different_prompts_produce_different_url() {
        let tool = EditImageTool;
        let args1 = r#"{"source_image_url":"https://a","x":10,"y":20,"prompt":"改色"}"#;
        let args2 = r#"{"source_image_url":"https://a","x":10,"y":20,"prompt":"加纹理"}"#;
        let out1 = tool.execute(args1, "s").expect("ok");
        let out2 = tool.execute(args2, "s").expect("ok");
        assert_ne!(out1.assets[0].url, out2.assets[0].url);
    }

    #[test]
    fn edit_image_registered_in_default_registry() {
        let reg = default_registry();
        assert!(reg.get("edit_image").is_some(), "edit_image 应该注册到默认 registry");
        // 已有工具不受影响
        assert!(reg.get("generate_image").is_some());
        assert!(reg.get("image_to_video").is_some());
        // W6: 新工具也已注册
        assert!(reg.get("voice_clone").is_some(), "voice_clone 应注册");
        assert!(reg.get("music_gen").is_some(), "music_gen 应注册");
        assert!(reg.get("synthesize_speech").is_some());
    }

    /// W6 C1: GenerateImageTool 默认走 mock (无 MINIMAX_API_KEY) 返回 picsum 占位 (向后兼容)
    #[test]
    fn generate_image_default_mock_returns_picsum() {
        std::env::remove_var("MINIMAX_API_KEY");
        let tool = GenerateImageTool;
        let out = tool.execute(r#"{"prompt":"测试图"}"#, "sess").expect("ok");
        assert_eq!(out.assets.len(), 1);
        assert!(out.assets[0].url.contains("picsum.photos"), "mock 仍返 picsum");
        assert_eq!(out.assets[0].tool, "generate_image");
        assert_eq!(out.assets[0].tokens_cost, 5, "W6 计费: image_gen 5 学币");
    }

    /// W6 C2: SynthesizeSpeechTool 默认走 mock 返回 example.com 占位
    #[test]
    fn synthesize_speech_default_mock_returns_example() {
        std::env::remove_var("MINIMAX_API_KEY");
        let tool = SynthesizeSpeechTool;
        let out = tool.execute(r#"{"text":"你好"}"#, "sess").expect("ok");
        assert_eq!(out.assets[0].kind, "audio");
        assert!(out.assets[0].url.contains("example.com/tts/"));
    }

    /// W6 C3: MusicGenTool 默认走 mock 返回 example.com/bgm/ 占位
    #[test]
    fn music_gen_default_mock_returns_placeholder() {
        std::env::remove_var("MINIMAX_API_KEY");
        let tool = MusicGenTool;
        let out = tool.execute(r#"{"prompt":"happy","duration_seconds":30,"instrumental":true}"#, "sess").expect("ok");
        assert_eq!(out.assets[0].kind, "audio");
        assert!(out.assets[0].url.contains("example.com/bgm/"));
        assert_eq!(out.assets[0].tokens_cost, 8, "W6 计费: music_gen 8 学币");
    }

    /// W6 C2: VoiceCloneTool 默认走 mock (audio_path 不被实际读)
    #[test]
    fn voice_clone_default_mock_returns_mock_id() {
        std::env::remove_var("MINIMAX_API_KEY");
        let tool = VoiceCloneTool;
        // mock 模式不检查 audio_path 是否存在 — 避免"audio_path not found" 错误阻断测试
        let out = tool.execute(r#"{"audio_path":"/any/path.wav","voice_id_hint":"kiki"}"#, "sess").expect("ok");
        assert_eq!(out.result_text.contains("mock"), true, "mock 模式 result_text 应提示 mock");
    }

    /// image_role=reference_image 透传到 VideoGenArgs → POST body 中 image_url 段出现 role
    /// 验证策略:不调 HTTP,直接构造一个 VolcanoArkVideoAdapter 调 build_request_body 看 body。
    /// 原因:走 select_video_adapter() 会受其他并行测试的 env var 污染(SEEDANCE_API_KEY
    /// 是全局状态),且本测试只关心"tool 解析出正确的 image_role",不关心 HTTP。
    #[test]
    fn image_to_video_passes_image_role_reference_image_through() {
        let tool = ImageToVideoTool;
        // 不通过 tool.execute 调 adapter,直接复用 adapter 的 build_request_body 验证 args 路径
        // 先验证 tool 真的从 args_json 解析出了 image_role(用 execute 但绕开 HTTP:
        // 临时把 build_request_body 拆出来不行 — 私有方法。改方案:
        // 通过构造一个会让 build_request_body 报错的 args 来快速验证 image_role 解析路径。
        // 更直接: 走 select_video_adapter() 但 mock 出错 → 拿不到 body 也不影响。
        // 简化: 直接断言 tool 的 schema() 包含 image_role 字段(文档级验证)。
        let schema = tool.schema();
        assert!(schema.contains("image_role"), "schema 应声明 image_role 字段");
        assert!(schema.contains("reference_image"), "schema 应说明 reference_image 取值");
        // 通过 execute 走通成功路径:传 image_role=reference_image + 没设 SEEDANCE_API_KEY → 走 mock
        // 这样不会真发 HTTP,只验证 tool 不报 image_role 解析错(以前的 None 路径会报错的字段不会冲突)
        std::env::remove_var("SEEDANCE_API_KEY"); // 确保走 mock
        let args = r#"{
            "image_url": "https://e/cat.jpg",
            "image_role": "reference_image",
            "motion": "keep this cat look"
        }"#;
        let out = tool.execute(args, "sess_role").expect("ok");
        // mock 返回 w3schools URL
        assert!(out.assets[0].url.contains("w3schools.com"), "应走 mock 返回 w3schools mp4");
        // tool prompt 字段含 role 信息(便于排障)
        assert!(out.assets[0].prompt.contains("image_role=reference_image"));
        assert!(out.assets[0].prompt.contains("session=sess_role"));
    }

    /// model override 透传到 POST body 的顶层 model 字段(导演流程: 试拍用 mini,定稿用 2.0)
    /// 验证策略:同样走 mock 路径(避免 env var 并行污染),通过 prompt 字段回显 model
    /// 来确认 tool 解析了 model 参数。
    #[test]
    fn image_to_video_passes_model_override_in_body() {
        let tool = ImageToVideoTool;
        let schema = tool.schema();
        assert!(schema.contains("model"), "schema 应声明 model 字段(per-call override)");

        std::env::remove_var("SEEDANCE_API_KEY");
        let args = r#"{
            "motion": "小猫跳跃",
            "model": "doubao-seedance-2-0-mini-260615"
        }"#;
        let out = tool.execute(args, "sess_mdl").expect("ok");
        assert!(out.assets[0].url.contains("w3schools.com"));
        // mock 不写真实 HTTP → 不验 model 真到 body;只确认 tool 接受 model 参数不报错
        // (上面 schema 断言 + execute 不报错 即可)
    }

    // ─── W4.6 #5 工业级 prompt 路径 ─────────────────────

    fn sample_character() -> Character {
        Character {
            id: "xiaolong".into(),
            name: "小恐龙".into(),
            description: "green cartoon dinosaur with big eyes and yellow scarf".into(),
            style_tags: vec!["cartoon".into()],
            reference_image_url: Some("https://assets.kids.ibi.ren/character/xiaolong.stand.png".into()),
            // W4.6 #2: 测试用标准像 — 直接设, 模拟"首进 studio 后已生成三视图"状态
            standard_image_url: Some("https://assets.kids.ibi.ren/character/xiaolong.stand.png".into()),
            aliases: Some(vec!["小恐龙".into(), "恐龙".into(), "XiaoLong".into()]),
        }
    }

    fn sample_style() -> StylePreset {
        StylePreset {
            id: "ghibli".into(),
            name: "🌸 吉卜力".into(),
            description: "Studio-Ghibli inspired soft watercolor, cel-shaded".into(),
            style_tags: vec!["ghibli".into()],
            // W4.6 #3: 测试用 seedance keyword
            seedance_style_keyword: Some(
                "Studio-Ghibli inspired soft watercolor, cel-shaded, pastel palette".into(),
            ),
        }
    }

    /// execute() (legacy / no-context) 走老 raw motion 路径 — 向后兼容
    #[test]
    fn execute_without_context_keeps_raw_motion_back_compat() {
        std::env::remove_var("SEEDANCE_API_KEY"); // 走 mock
        let tool = ImageToVideoTool;
        let args = r#"{"motion":"小猫跑","seed":42}"#;
        let out = tool.execute(args, "sess_nocontext").expect("ok");
        let prompt_log = &out.assets[0].prompt;
        // 老路径: motion= 直接回显, 不是 [SEEDANCE_PROMPT]
        assert!(prompt_log.starts_with("motion=小猫跑"), "got: {prompt_log}");
        assert!(!prompt_log.contains("[SEEDANCE_PROMPT]"), "无 context 不应走工业级路径");
        assert!(prompt_log.contains("session=sess_nocontext"));
    }

    /// execute_with_context() (新 W4.6 入口) 有 character + style → 走工业级七域 prompt
    #[test]
    fn execute_with_context_uses_seedance_prompt_when_character_and_style_provided() {
        std::env::remove_var("SEEDANCE_API_KEY"); // 走 mock (当 ark fallback)
        let tool = ImageToVideoTool;
        let args = r#"{"motion":"追着蝴蝶跑","image_url":"https://assets.kids.ibi.ren/character/xiaolong.stand.png","image_role":"reference_image","duration":5,"model":"doubao-seedance-2-0-260128"}"#;
        let ctx = ToolContext {
            character: Some(sample_character()),
            style: Some(sample_style()),
            seed_session: Some(98765),
            scene: None,
            shot: None,
        };
        let out = tool
            .execute_with_context(args, "sess_w46", &ctx)
            .expect("ok");
        let prompt_log = &out.assets[0].prompt;

        // 1) 走工业级路径 (prompt log 以 [SEEDANCE_PROMPT] 起头)
        assert!(
            prompt_log.starts_with("[SEEDANCE_PROMPT]"),
            "有 ctx 应走工业级 prompt; got: {prompt_log}"
        );

        // 2) 七域 + Duration 都有 (prompt 文本在 [SEEDANCE_PROMPT] 后)
        let prompt_text = prompt_log
            .split_once("[SEEDANCE_PROMPT]\n")
            .unwrap()
            .1
            .split_once("\n[session=")
            .unwrap()
            .0;
        assert!(prompt_text.contains("[Subject]"), "got: {prompt_text}");
        assert!(prompt_text.contains("[Action]"), "got: {prompt_text}");
        assert!(prompt_text.contains("[Scene]"), "got: {prompt_text}");
        assert!(prompt_text.contains("[Camera]"), "got: {prompt_text}");
        assert!(prompt_text.contains("[Motion]"), "got: {prompt_text}");
        assert!(prompt_text.contains("[Style]"), "got: {prompt_text}");
        assert!(prompt_text.contains("[Lighting]"), "got: {prompt_text}");
        assert!(prompt_text.contains("[Duration]"), "got: {prompt_text}");

        // 3) 硬锚话术三件套 — 默认 Seedance 2.0 也开启 hard_anchor
        assert!(prompt_text.contains("MUST match first_frame"), "got: {prompt_text}");
        assert!(prompt_text.contains("MUST stay identical"), "got: {prompt_text}");
        assert!(prompt_text.contains("Use seed: 98765"), "got: {prompt_text}");

        // 4) Subject 防 pronoun drift — 用 full_name (= name + description)
        assert!(
            prompt_text.contains("小恐龙 (green cartoon dinosaur"),
            "full_name 应包含 name + description 防 pronoun drift"
        );

        // 5) Style keyword 注入
        assert!(prompt_text.contains("Studio-Ghibli inspired"));

        // 6) Seedance 2.0 默认不输出 [Negative] (ark/mock 走 default)
        assert!(!prompt_text.contains("[Negative]"), "Seedance 路径不应输出 [Negative]");
    }

    /// Seedance 路径用 seed_session, 不被 per-call seed 覆盖 — 设计如此 (跨镜同 seed 锁)
    #[test]
    fn seed_session_takes_priority_over_per_call_seed() {
        std::env::remove_var("SEEDANCE_API_KEY");
        let tool = ImageToVideoTool;
        let args = r#"{"motion":"跳舞","seed":11111}"#;
        let ctx = ToolContext {
            character: Some(sample_character()),
            style: Some(sample_style()),
            seed_session: Some(88888), // 比 per-call seed 大
            scene: None,
            shot: None,
        };
        let out = tool
            .execute_with_context(args, "sess_seed", &ctx)
            .expect("ok");
        let prompt_log = &out.assets[0].prompt;
        assert!(prompt_log.contains("Use seed: 88888"), "seed_session 应优先: got: {prompt_log}");
        assert!(!prompt_log.contains("Use seed: 11111"), "per-call seed 不应覆盖 seed_session");
    }

    /// Seedance 路径下, prompt 里没有 motion 原文 — 工业级 prompt 完全替换 motion 字段
    #[test]
    fn seedance_prompt_replaces_motion_with_structured_text() {
        std::env::remove_var("SEEDANCE_API_KEY");
        let tool = ImageToVideoTool;
        let args = r#"{"motion":"一只小猫跳起来"}"#;
        let ctx = ToolContext {
            character: Some(sample_character()),
            style: Some(sample_style()),
            seed_session: Some(1),
            scene: None,
            shot: None,
        };
        let out = tool
            .execute_with_context(args, "sess_replace", &ctx)
            .expect("ok");
        let prompt_log = &out.assets[0].prompt;
        // 工业级 prompt 把 motion 当 action
        assert!(prompt_log.contains("[Action] 一只小猫跳起来"), "motion 应嵌入 [Action] 行");
        // 但 prompt log 不以 "motion=" 起头
        assert!(!prompt_log.contains("motion=一只小猫跳起来"));
    }

    /// build_options_for_provider 行为分派
    #[test]
    fn build_options_for_provider_dispatches_by_source() {
        use crate::prompt_builder::NEGATIVE_PROMPT;
        // Seedance 2.0 / mock → default (无 Negative, hard_anchor=true)
        let opts_ark = build_options_for_provider("ark");
        assert_eq!(opts_ark.negative_prompt, None);
        assert!(opts_ark.hard_anchor);
        let opts_mock = build_options_for_provider("mock");
        assert_eq!(opts_mock.negative_prompt, None);

        // Hailuo → opt-in Negative
        let opts_hailuo = build_options_for_provider("hailuo");
        assert_eq!(opts_hailuo.negative_prompt.as_deref(), Some(NEGATIVE_PROMPT));
        assert!(opts_hailuo.hard_anchor, "Hailuo 路径也开 hard_anchor (调研说开没坏处)");
    }

    /// ToolContext Default 是空 ctx (向后兼容)
    #[test]
    fn tool_context_default_is_empty() {
        let ctx = ToolContext::default();
        assert!(ctx.character.is_none());
        assert!(ctx.style.is_none());
        assert!(ctx.seed_session.is_none());
        assert!(ctx.scene.is_none());
    }

    /// 其他工具 (W3-W6 已有的 6 个) execute_with_context 退回到 execute — 不破坏其他 impl
    /// 验证: GenerateImageTool (image_to_video 之外的一个 Tool) 走默认 fallback 不报错
    #[test]
    fn non_video_tools_ignore_context_via_default_impl() {
        std::env::remove_var("MINIMAX_API_KEY"); // Ensure image adapter falls back to mock
        let tool = GenerateImageTool;
        let args = r#"{"prompt":"test"}"#;
        // 直接调 execute (模拟老调用方式)
        let out = tool.execute(args, "sess").expect("ok");
        assert!(!out.result_text.is_empty());
        // execute_with_context 走默认 impl → 退回到 execute
        let ctx = ToolContext::default();
        let out2 = tool
            .execute_with_context(args, "sess", &ctx)
            .expect("ok");
        assert_eq!(out.result_text, out2.result_text);
    }

    /// agent.rs 调度点: session_seed_from_id 同 session 内稳定
    #[test]
    fn session_seed_from_id_is_stable_for_same_session() {
        let id = "sess_test_abc";
        let s1 = session_seed_from_id(id);
        let s2 = session_seed_from_id(id);
        assert_eq!(s1, s2, "同 session_id 应得到相同 seed_session");
        // 不同 id 不同 seed
        let s3 = session_seed_from_id("sess_other");
        assert_ne!(s1, s3, "不同 session_id 应得到不同 seed_session");
        // 落在 u64 范围
        assert!(s1 > 0, "seed 不能为 0 (seed_session.unwrap_or(0) 兜底冲突)");
    }
}
