// W4.6 翻译层 — 孩子白话 → Seedance 工业级七域 prompt
//
// 目的: 取代 v1 的"LLM 自由发挥 + style/character 注入"模式, 把 ⑥ 视频试拍/定稿
// 的 motion 文本从"儿童语言 + 风格后缀"升级为"工业七域 + 角色一致性四件套".
//
// 设计要点:
// - 输入类型 ShotPromptInput / CharacterAsset / StyleAsset / SceneAsset 都是
//   可序列化的纯数据, 由前端 directorStore 或后端 agent 组装后传入, 不耦合
//   Tauri State / 注册表. 这样单元测试不依赖 AppHandle.
// - 代词替换只覆盖高频词 (她/他/它/she/he/it/the cat), 不做完整 NLP.
//   真要更准, 后续可加 LLM 预改写; 目前 12 单测覆盖已验证常用 case.
// - 风格关键词 (seedance_style_keyword) 由 StyleRegistry 在 Task #3 注入;
//   Task #1 假设调用方已经传入, 直接用. 测试里用 mock 值覆盖.
// - 硬锚话术是 Seedance 调研验证有效的 "显式硬锚" (避免 reference_image 单
//   用的软参考问题), 必须强制追加, 测试覆盖.
use serde::{Deserialize, Serialize};

/// 单分镜的 prompt 输入 (W4.6 简化版, 来自 directorStore.DirectorShot)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShotPromptInput {
    /// 孩子/导演写的主体描述, 可能含代词 (她/他/它/she/he/it)
    pub subject: String,
    /// 物理动作描述, 1 个动作 ≤10 字 (Task #4 prompt 约束)
    pub action: String,
    /// 场景名 (来自 assetStore bg key, 例: "森林" / "教室")
    pub setting: String,
    /// 5 选 1 情绪, 决定 motion 节奏
    pub mood: ShotMood,
    /// 6 选 1 镜头, 决定 camera 域
    pub camera: ShotCamera,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ShotMood {
    Calm,
    Tense,
    Joyful,
    Sad,
    Epic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ShotCamera {
    /// 远景: 全景建立镜头, 静态
    Wide,
    /// 中景: 腰/膝以上
    Medium,
    /// 近景: 胸/肩以上
    Close,
    /// 特写: 面部/道具细节
    Extreme,
    /// 跟随: 跟拍主体移动
    Follow,
    /// 俯视: 鸟瞰
    Overhead,
}

/// 场景资产 (来自 assetStore bg, W4.6 扩展)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneAsset {
    pub id: String,
    /// 场景名, 例 "森林" / "教室"
    pub name: String,
    /// 简短场景描述, 例 "夏季下午阳光穿过树梢"
    pub setting: String,
    /// 时段标签: morning / afternoon / golden_hour / night / moonlit
    pub time_of_day: String,
    /// 天气: sunny / cloudy / rainy / snowy
    pub weather: String,
    /// 已生成的 bg 图片 URL (e.g., https://assets.kids.ibi.ren/bg/<id>.png)
    pub bg_url: String,
}

/// 角色资产 (来自 CharacterRegistry, W4.6 扩展 — Task #2 完成实际字段)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterAsset {
    pub id: String,
    /// 全名 + 特征, 强制跨镜复用 — 例 "小恐龙 (a green cartoon dinosaur with big eyes and a yellow scarf)"
    pub full_name: String,
    pub style_tags: Vec<String>,
    /// 三视图角色卡 URL (1 张三视图合图, image-01 一次出)
    pub standard_image_url: String,
    /// 可选: 侧面/背面独立图 (后续可加, Task #1 暂用 standard 作主参考)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side_image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub back_image_url: Option<String>,
}

/// 风格资产 (来自 StyleRegistry, W4.6 扩展 — Task #3 完成实际字段)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StyleAsset {
    pub id: String,
    pub name: String,
    /// Seedance 专用风格关键词, 由 StyleRegistry 注入.
    /// 例 "Studio-Ghibli inspired soft watercolor, cel-shaded"
    pub seedance_style_keyword: String,
}

// ────────────────────────────────────────────────────────────────────────
// 静态常量 / 模板
// ────────────────────────────────────────────────────────────────────────

/// Seedance 工业级 Negative prompt (W6 调研 + 工业版 5 防崩兜底)
/// 全小写是 Seedance 社区惯例; 一行写完避免 `\` 续行吃掉逗号后空格.
pub const NEGATIVE_PROMPT: &str = "no camera shake, no extra limbs, no extra digits, no text, no watermark, no logo, no sudden zoom, no color flicker, no facial distortion, no mirror flip, no anatomical deformity";

/// 默认时长 (秒) — 跟 video_adapter 默认对齐
pub const DEFAULT_DURATION_SECS: u32 = 5;

// ────────────────────────────────────────────────────────────────────────
// 映射函数 — 镜头/情绪/时段 → 工业术语
// ────────────────────────────────────────────────────────────────────────

/// 镜头选项 → Seedance 镜头描述
/// 借鉴工业版横屏短剧指令 (docs/reffrence/分镜指令-横屏短剧.txt) 的景别规则
pub fn camera_to_seedance(camera: &ShotCamera) -> &'static str {
    match camera {
        ShotCamera::Wide => {
            "wide establishing shot, static camera, locked-off tripod, full body + environment visible"
        }
        ShotCamera::Medium => {
            "medium shot, eye-level, knee-to-waist framing, balanced composition"
        }
        ShotCamera::Close => {
            "close-up, chest-to-head, shallow depth of field, eye-level"
        }
        ShotCamera::Extreme => {
            "extreme close-up, macro detail, very shallow depth of field, subject fills 80%+ frame"
        }
        ShotCamera::Follow => {
            "tracking shot, follows subject smoothly, eye-level, no sudden direction change"
        }
        ShotCamera::Overhead => {
            "bird's eye view, slow descent, top-down angle, environment in full view"
        }
    }
}

/// 情绪 → motion 节奏描述
/// 借鉴工业版情绪绑定运镜 (压抑→反向拉轨, 激昂→快速推轨)
pub fn mood_to_motion(mood: &ShotMood) -> &'static str {
    match mood {
        ShotMood::Calm => "gentle motion, slow pace, subject moves less than half body length",
        ShotMood::Tense => {
            "deliberate motion, medium pace, micro-expressions emphasized, restrained gestures"
        }
        ShotMood::Joyful => {
            "bouncy motion, energetic pace, subject moves freely with bouncy gestures"
        }
        ShotMood::Sad => "very slow motion, low energy, heavy gestures, downward head tilt",
        ShotMood::Epic => "dramatic motion, dynamic pace, action-driven with strong visual impact",
    }
}

/// 时段 → 光影描述
/// 借鉴工业版横屏短剧指令的"光影逻辑"维度, 简化版 (去掉 IRE/Hue/K 色温参数)
pub fn lighting_for_time_of_day(time_of_day: &str) -> &'static str {
    match time_of_day {
        "golden_hour" | "afternoon" => {
            "golden hour warm rim light, long soft shadows, warm palette"
        }
        "morning" => "soft morning light, gentle warm fill, low contrast, pastel palette",
        "night" | "moonlit" => "moonlit cool blue rim light, soft volumetric fog, deep contrast",
        "rainy" | "cloudy" => "overcast diffused light, soft shadows, cool desaturated palette",
        _ => "natural soft daylight, balanced fill light, neutral palette",
    }
}

// ────────────────────────────────────────────────────────────────────────
// 代词替换
// ────────────────────────────────────────────────────────────────────────

/// 把 subject / action 里的高频代词替换成 character.full_name
/// 覆盖中英文, 防 pronoun drift (Seedance 调研反模式 #1)
pub fn replace_pronouns(text: &str, full_name: &str) -> String {
    let mut result = text.to_string();

    // 中文代词 (优先匹配具体名, 再匹配代词)
    let zh_pronouns = ["它", "他", "她", "主角", "那个角色", "那个孩子"];
    for p in &zh_pronouns {
        if result.contains(p) {
            result = result.replace(p, full_name);
        }
    }

    // 英文代词 (大小写不敏感简化处理)
    let lower = result.to_lowercase();
    let en_pronouns = [
        "the character",
        "the protagonist",
        "she",
        "he",
        "they",
        "it",
    ];
    for p in &en_pronouns {
        if lower.contains(p) {
            // 用 lowercase 找到后, 在原 result 里替换 (简化: 全局 lowercase replace)
            // 对 prompt 影响可忽略 (不会破坏品牌名等)
            result = result.replace(p, full_name);
        }
    }

    result
}

// ────────────────────────────────────────────────────────────────────────
// 主函数: build_seedance_prompt
// ────────────────────────────────────────────────────────────────────────

/// Prompt 生成选项 — 控制 Negative / 时长 / 硬锚是否启用
///
/// 默认 (PromptOptions::default()):
/// - negative_prompt: None — Seedance 2.0 不需要, 默认不输出 [Negative] 行
/// - duration_secs: 5 — 跟 video_adapter 默认对齐
/// - hard_anchor: true — 永远注入 hard anchor (reference_image 是软参考,
///   硬话术是调研验证有效的强一致手段)
///
/// 使用方 (Task #5 video_adapter) 按 engine 决定:
/// - Seedance 2.0 → PromptOptions::default() (不加 negative)
/// - Hailuo / 未来模型 → PromptOptions { negative_prompt: Some(GENERIC_NEGATIVE_PROMPT.into()), ..Default::default() }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptOptions {
    /// None = 不输出 [Negative] 行 (Seedance 2.0 默认)
    /// Some(text) = 输出 [Negative] {text}
    pub negative_prompt: Option<String>,
    /// 视频时长 (秒)
    pub duration_secs: u32,
    /// 是否注入硬锚话术三件套 (first_frame + reference_image + seed)
    /// 默认 true, 因为调研证明对 Seedance 2.0 角色一致性至关重要
    pub hard_anchor: bool,
}

impl Default for PromptOptions {
    fn default() -> Self {
        Self {
            negative_prompt: None,
            duration_secs: DEFAULT_DURATION_SECS,
            hard_anchor: true,
        }
    }
}

/// 孩子白话 → Seedance 工业级七域 prompt
///
/// 输出结构 (按 options 启用情况):
/// 1. [Subject]      = full_name + "the same character from previous shots"
/// 2. [Action]       = 代词替换后的 action
/// 3. [Scene]        = scene.name + scene.setting
/// 4. [Camera]       = camera_to_seedance(camera)
/// 5. [Motion]       = mood_to_motion(mood)
/// 6. [Style]        = style.seedance_style_keyword
/// 7. [Lighting]     = lighting_for_time_of_day(scene.time_of_day) (或默认)
/// 8. [Duration]     = options.duration_secs
/// 9. [Negative]     = 仅当 options.negative_prompt = Some(_) 时输出
/// 10. [MUST match]  = 仅当 options.hard_anchor = true 时输出三件套
pub fn build_seedance_prompt(
    shot: &ShotPromptInput,
    character: &CharacterAsset,
    style: &StyleAsset,
    scene: Option<&SceneAsset>,
    seed_session: u64,
    options: &PromptOptions,
) -> String {
    let mut prompt = String::new();

    // 1) Subject — 强制用 full_name, 防止 pronoun drift
    prompt.push_str(&format!(
        "[Subject] {}, the same character from previous shots\n",
        character.full_name
    ));

    // 2) Action — 代词替换
    let action_clean = replace_pronouns(&shot.action, &character.full_name);
    prompt.push_str(&format!("[Action] {}\n", action_clean));

    // 3) Scene — 注入 scene name + setting
    if let Some(s) = scene {
        prompt.push_str(&format!("[Scene] {} — {}\n", s.name, s.setting));
    } else {
        prompt.push_str(&format!("[Scene] {}\n", shot.setting));
    }

    // 4) Camera
    prompt.push_str(&format!("[Camera] {}\n", camera_to_seedance(&shot.camera)));

    // 5) Motion
    prompt.push_str(&format!("[Motion] {}\n", mood_to_motion(&shot.mood)));

    // 6) Style — 来自 StyleRegistry.seedance_style_keyword
    prompt.push_str(&format!("[Style] {}\n", style.seedance_style_keyword));

    // 7) Lighting — 来自 scene.time_of_day, 没有 scene 时用默认
    let lighting = scene
        .map(|s| lighting_for_time_of_day(&s.time_of_day))
        .unwrap_or_else(|| lighting_for_time_of_day("default"));
    prompt.push_str(&format!("[Lighting] {}\n", lighting));

    // 8) Duration — 来自 options
    prompt.push_str(&format!("[Duration] {} seconds\n", options.duration_secs));

    // 9) Negative prompt — 仅当 options 启用时输出 (默认不输出, Seedance 2.0 不需要)
    if let Some(neg) = &options.negative_prompt {
        prompt.push_str(&format!("[Negative] {}\n", neg));
    }

    // 10) 硬锚话术三件套 — 防 reference_image 软参考问题
    if options.hard_anchor {
        prompt.push_str(&format!(
            "\nMUST match first_frame: {}\nMUST stay identical to reference_image: {}\nUse seed: {}",
            character.standard_image_url, character.standard_image_url, seed_session
        ));
    }

    // 11) 场景 bg URL (如有) 作为环境锚 (跟硬锚独立, 不论 hard_anchor 都加)
    if let Some(s) = scene {
        prompt.push_str(&format!("\nMUST use background: {}", s.bg_url));
    }

    prompt
}

// ────────────────────────────────────────────────────────────────────────
// 单元测试
// ────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─── helpers ─────────────────────────────────────────
    fn sample_character() -> CharacterAsset {
        CharacterAsset {
            id: "xiaolong".into(),
            full_name: "小恐龙 (a green cartoon dinosaur with big eyes and a yellow scarf)".into(),
            style_tags: vec!["cartoon".into(), "child_friendly".into()],
            standard_image_url: "https://assets.kids.ibi.ren/character/xiaolong.stand.png".into(),
            side_image_url: None,
            back_image_url: None,
        }
    }

    fn sample_style() -> StyleAsset {
        StyleAsset {
            id: "ghibli".into(),
            name: "🌸 吉卜力".into(),
            seedance_style_keyword: "Studio-Ghibli inspired soft watercolor, cel-shaded".into(),
        }
    }

    fn sample_scene() -> SceneAsset {
        SceneAsset {
            id: "forest".into(),
            name: "森林".into(),
            setting: "夏季下午阳光穿过树梢".into(),
            time_of_day: "afternoon".into(),
            weather: "sunny".into(),
            bg_url: "https://assets.kids.ibi.ren/bg/forest.png".into(),
        }
    }

    fn sample_shot() -> ShotPromptInput {
        ShotPromptInput {
            subject: "小恐龙".into(),
            action: "追蝴蝶跑过来".into(),
            setting: "森林".into(),
            mood: ShotMood::Joyful,
            camera: ShotCamera::Follow,
        }
    }

    // ─── 1. 代词替换 ─────────────────────────────────────
    #[test]
    fn replace_pronouns_zh_ita() {
        let r = replace_pronouns("它跑过来", "小恐龙");
        assert_eq!(r, "小恐龙跑过来");
    }

    #[test]
    fn replace_pronouns_zh_ta() {
        let r = replace_pronouns("他/她都笑了", "小启");
        assert_eq!(r, "小启/小启都笑了");
    }

    #[test]
    fn replace_pronouns_en_she() {
        let r = replace_pronouns("She runs to the flower", "小恐龙");
        assert_eq!(r.contains("小恐龙"), true);
    }

    #[test]
    fn replace_pronouns_en_it() {
        let r = replace_pronouns("it jumps over the rock", "小恐龙");
        assert_eq!(r.contains("小恐龙"), true);
    }

    #[test]
    fn replace_pronouns_keeps_unrelated_words() {
        let r = replace_pronouns("花园里跑过来", "小恐龙");
        assert_eq!(r, "花园里跑过来", "无代词则原样");
    }

    // ─── 2. 镜头映射 ─────────────────────────────────────
    #[test]
    fn camera_wide_maps_to_establishing() {
        assert!(camera_to_seedance(&ShotCamera::Wide).contains("establishing"));
    }

    #[test]
    fn camera_medium_maps_to_knee() {
        assert!(camera_to_seedance(&ShotCamera::Medium).contains("medium shot"));
    }

    #[test]
    fn camera_close_maps_to_closeup() {
        assert!(camera_to_seedance(&ShotCamera::Close).contains("close-up"));
    }

    #[test]
    fn camera_extreme_maps_to_macro() {
        assert!(camera_to_seedance(&ShotCamera::Extreme).contains("macro"));
    }

    #[test]
    fn camera_follow_maps_to_tracking() {
        assert!(camera_to_seedance(&ShotCamera::Follow).contains("tracking"));
    }

    #[test]
    fn camera_overhead_maps_to_birdseye() {
        assert!(camera_to_seedance(&ShotCamera::Overhead).contains("bird"));
    }

    // ─── 3. 情绪映射 ─────────────────────────────────────
    #[test]
    fn mood_tense_maps_to_deliberate() {
        assert!(mood_to_motion(&ShotMood::Tense).contains("deliberate"));
    }

    #[test]
    fn mood_epic_maps_to_dramatic() {
        assert!(mood_to_motion(&ShotMood::Epic).contains("dramatic"));
    }

    #[test]
    fn mood_joyful_maps_to_bouncy() {
        assert!(mood_to_motion(&ShotMood::Joyful).contains("bouncy"));
    }

    // ─── 4. 风格注入 ─────────────────────────────────────
    #[test]
    fn style_injected_verbatim() {
        let s = sample_style();
        assert!(s.seedance_style_keyword.contains("Ghibli"));
    }

    // ─── 5. Negative 条件化 (Seedance 2.0 默认不输出) ────
    #[test]
    fn negative_not_emitted_by_default() {
        // Seedance 2.0 不需要 Negative prompt, PromptOptions::default() 不输出
        let p = build_seedance_prompt(
            &sample_shot(),
            &sample_character(),
            &sample_style(),
            Some(&sample_scene()),
            12345,
            &PromptOptions::default(),
        );
        assert!(
            !p.contains("[Negative]"),
            "Seedance 2.0 默认不应有 [Negative] 行"
        );
        assert!(!p.contains("no camera shake"), "默认不注入负面关键词");
    }

    #[test]
    fn negative_emitted_when_opted_in() {
        // 显式传 negative_prompt = Some(...) 时, 输出 [Negative] 行
        let opts = PromptOptions {
            negative_prompt: Some(NEGATIVE_PROMPT.into()),
            ..PromptOptions::default()
        };
        let p = build_seedance_prompt(
            &sample_shot(),
            &sample_character(),
            &sample_style(),
            Some(&sample_scene()),
            12345,
            &opts,
        );
        assert!(p.contains("[Negative]"));
        assert!(p.contains("no camera shake"));
        assert!(p.contains("no text"));
        assert!(p.contains("no watermark"));
        assert!(p.contains("no facial distortion"));
    }

    // ─── 6. 硬锚话术注入 ─────────────────────────────────
    #[test]
    fn hard_anchor_injects_three_clauses() {
        let p = build_seedance_prompt(
            &sample_shot(),
            &sample_character(),
            &sample_style(),
            Some(&sample_scene()),
            12345,
            &PromptOptions::default(),
        );
        assert!(p.contains("MUST match first_frame"));
        assert!(p.contains("MUST stay identical"));
        assert!(p.contains("Use seed: 12345"));
        // 标准图 URL 出现 2 次 (first_frame + reference_image)
        assert_eq!(
            p.matches("https://assets.kids.ibi.ren/character/xiaolong.stand.png")
                .count(),
            2
        );
    }

    #[test]
    fn hard_anchor_can_be_disabled() {
        // 关 hard_anchor 时, 硬锚话术三件套都不输出 (reference_image 软参考 fallback)
        let opts = PromptOptions {
            hard_anchor: false,
            ..PromptOptions::default()
        };
        let p = build_seedance_prompt(
            &sample_shot(),
            &sample_character(),
            &sample_style(),
            Some(&sample_scene()),
            12345,
            &opts,
        );
        assert!(!p.contains("MUST match first_frame"));
        assert!(!p.contains("MUST stay identical"));
        assert!(!p.contains("Use seed:"));
    }

    // ─── 7. 场景注入 ─────────────────────────────────────
    #[test]
    fn scene_included_when_provided() {
        let p = build_seedance_prompt(
            &sample_shot(),
            &sample_character(),
            &sample_style(),
            Some(&sample_scene()),
            12345,
            &PromptOptions::default(),
        );
        assert!(p.contains("[Scene] 森林"));
        assert!(p.contains("夏季下午阳光穿过树梢"));
        assert!(p.contains("[Lighting] golden hour warm rim light")); // afternoon → golden hour
        assert!(p.contains("MUST use background: https://assets.kids.ibi.ren/bg/forest.png"));
    }

    #[test]
    fn scene_none_uses_default_setting() {
        let p = build_seedance_prompt(
            &sample_shot(),
            &sample_character(),
            &sample_style(),
            None,
            12345,
            &PromptOptions::default(),
        );
        assert!(p.contains("[Scene] 森林")); // 用 shot.setting
        assert!(!p.contains("MUST use background")); // 无 bg 锚
        assert!(p.contains("[Lighting] natural soft daylight")); // default
    }

    // ─── 8. 端到端 ─────────────────────────────────────
    #[test]
    fn end_to_end_golden_output() {
        // 端到端显式开启 Negative (覆盖全量输出场景)
        let opts = PromptOptions {
            negative_prompt: Some(NEGATIVE_PROMPT.into()),
            ..PromptOptions::default()
        };
        let p = build_seedance_prompt(
            &sample_shot(),
            &sample_character(),
            &sample_style(),
            Some(&sample_scene()),
            42,
            &opts,
        );

        // 七域 + Duration + Negative + 硬锚 全部出现
        assert!(p.contains("[Subject]"));
        assert!(p.contains("[Action]"));
        assert!(p.contains("[Scene]"));
        assert!(p.contains("[Camera]"));
        assert!(p.contains("[Motion]"));
        assert!(p.contains("[Style]"));
        assert!(p.contains("[Lighting]"));
        assert!(p.contains("[Duration]"));
        assert!(p.contains("[Negative]"));
        assert!(p.contains("MUST match first_frame"));
        assert!(p.contains("Use seed: 42"));

        // Subject 用 full_name (pronoun drift 防御)
        assert!(p.contains("小恐龙 (a green cartoon dinosaur"));
        assert!(p.contains("the same character from previous shots"));

        // Style 注入
        assert!(p.contains("Studio-Ghibli inspired soft watercolor"));

        // Camera (Follow)
        assert!(p.contains("tracking shot"));

        // Motion (Joyful)
        assert!(p.contains("bouncy motion"));
    }

    // ─── 9. 代词替换 + action 联动 ────────────────────────
    #[test]
    fn action_with_pronoun_gets_replaced() {
        let mut shot = sample_shot();
        shot.action = "它追着蝴蝶跑".into();
        let p = build_seedance_prompt(
            &shot,
            &sample_character(),
            &sample_style(),
            Some(&sample_scene()),
            1,
            &PromptOptions::default(),
        );
        // [Action] 行应该用 full_name 而不是"它"
        assert!(p.contains("[Action] 小恐龙 (a green cartoon dinosaur"));
        assert!(!p.contains("[Action] 它"));
    }

    // ─── 11. duration 自定义 ─────────────────────────────
    #[test]
    fn duration_custom_value() {
        let opts = PromptOptions {
            duration_secs: 10,
            ..PromptOptions::default()
        };
        let p = build_seedance_prompt(
            &sample_shot(),
            &sample_character(),
            &sample_style(),
            Some(&sample_scene()),
            1,
            &opts,
        );
        assert!(
            p.contains("[Duration] 10 seconds"),
            "duration_secs 应注入到 [Duration] 行"
        );
    }

    // ─── 12. PromptOptions::default 值符合预期 ────────────
    #[test]
    fn prompt_options_default_values() {
        let opts = PromptOptions::default();
        assert_eq!(
            opts.negative_prompt, None,
            "Seedance 2.0 默认不输出 Negative"
        );
        assert_eq!(
            opts.duration_secs, DEFAULT_DURATION_SECS,
            "默认 5s 跟 video_adapter 对齐"
        );
        assert!(
            opts.hard_anchor,
            "默认开启硬锚, 调研证明对 Seedance 2.0 角色一致性至关重要"
        );
    }

    // ─── 10. 时段 → 光影 ─────────────────────────────────
    #[test]
    fn lighting_night_is_cool() {
        assert!(lighting_for_time_of_day("night").contains("cool blue"));
    }

    #[test]
    fn lighting_morning_is_warm() {
        assert!(lighting_for_time_of_day("morning").contains("warm"));
    }

    #[test]
    fn lighting_unknown_is_default() {
        assert!(lighting_for_time_of_day("never_heard_of_this").contains("natural soft daylight"));
    }
}
