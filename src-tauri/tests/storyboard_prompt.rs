// W4.6 #4: DirectorShot 6 字段 + 5 拍节奏 + 严格 JSON Schema 端到端集成测试
//
// 验证三件事:
// 1. ToolContext.shot.mood/camera 解析 → build_seedance_prompt 的 [Mood]/[Camera] 行体现
// 2. 不同 mood/camera 产生不同的 Seedance prompt (覆盖 #1 静默回退 Calm/Medium 的风险)
// 3. ShotContext 字段缺失/解析失败 → 静默回退 Calm/Medium, 不阻断 video 生成
//
// 5 case:
// 1. mood/camera 全填 → prompt 里 [Camera] 行是 wide, [Motion] 行是 calm
// 2. mood/camera 部分填 + 部分默认 → 拼好的 prompt 不挂
// 3. mood/camera 全是未知字符串 → 静默回退 Calm/Medium (不报错)
// 4. shot 整个缺失 (None) → 老路径 (默认 Calm/Medium), 不破坏 W4.6 #5 的兼容
// 5. 5 拍节奏下, 三种 beat 都能解析 → prompt_for_log 输出含 beat 字符串 (排障可见)

use kidsai_studio_lib::prompt_builder::{
    build_seedance_prompt, CharacterAsset, PromptOptions, SceneAsset, StyleAsset,
};
use kidsai_studio_lib::tools::{parse_shot_camera, parse_shot_mood, ImageToVideoTool, ShotContext, Tool, ToolContext};
use kidsai_studio_lib::character::Character;
use kidsai_studio_lib::style::StylePreset;

fn sample_character() -> Character {
    Character {
        id: "xiaoqi".into(),
        name: "小启".into(),
        description: "yellow short hair cat girl".into(),
        style_tags: vec!["cartoon".into()],
        reference_image_url: Some("https://picsum.photos/seed/xiaoqi-ref/512/512".into()),
        standard_image_url: None,
        aliases: None,
    }
}

fn sample_style() -> StylePreset {
    StylePreset {
        id: "cartoon".into(),
        name: "卡通".into(),
        description: "明亮卡通风格".into(),
        style_tags: vec!["cartoon".into()],
        seedance_style_keyword: Some("bright cartoon style, bold outlines, saturated palette".into()),
    }
}

fn run_with_shot(shot_ctx: ShotContext, motion: &str) -> String {
    std::env::remove_var("SEEDANCE_API_KEY"); // 走 mock
    let tool = ImageToVideoTool;
    let args_json = format!(
        r#"{{"motion":"{}","image_url":"https://x.test/img.png","image_role":"reference_image","duration":4,"seed":42}}"#,
        motion
    );
    let ctx = ToolContext {
        character: Some(sample_character()),
        style: Some(sample_style()),
        seed_session: Some(98765),
        scene: None,
        shot: Some(shot_ctx),
    };
    let out = tool
        .execute_with_context(&args_json, "sess_w46_4", &ctx)
        .expect("ok");
    // prompt 字段是 [SEEDANCE_PROMPT]\n<七域 prompt>\n[session=..., ...]
    out.assets[0].prompt.clone()
}

#[test]
fn case_1_shot_mood_camera_promote_to_prompt_text() {
    // mood=tense + camera=close → prompt [Camera] 应含 close-up, [Motion] 应含 tense 描述
    let shot = ShotContext {
        beat: Some("conflict".into()),
        mood: Some("tense".into()),
        camera: Some("close".into()),
        character_refs: vec!["xiaoqi".into()],
        transition_to_next: Some("cut".into()),
    };
    let prompt_log = run_with_shot(shot, "小猫紧张地盯着蝴蝶");
    let prompt_text = prompt_log
        .split_once("[SEEDANCE_PROMPT]\n")
        .unwrap()
        .1
        .split_once("\n[session=")
        .unwrap()
        .0;

    // [Camera] 行有 close-up (parse_shot_camera("close") → ShotCamera::Close)
    assert!(
        prompt_text.contains("[Camera] close-up"),
        "camera=close 应输出 close-up; got: {prompt_text}"
    );
    // [Motion] 行用 tense 的描述
    assert!(
        prompt_text.contains("[Motion] deliberate motion"),
        "mood=tense 应输出 deliberate motion; got: {prompt_text}"
    );
    // 排障行有 beat / camera / mood / transition_to_next
    assert!(prompt_log.contains("beat=conflict"));
    assert!(prompt_log.contains("camera=close"));
    assert!(prompt_log.contains("mood=tense"));
    assert!(prompt_log.contains("transition_to_next=cut"));
}

#[test]
fn case_2_different_mood_camera_yields_different_prompt() {
    // 验证 #1 的核心断言: 不同 mood/camera 真的影响 prompt (不是静默 Calm/Medium)
    let hook_wide = run_with_shot(
        ShotContext {
            mood: Some("joyful".into()),
            camera: Some("wide".into()),
            ..Default::default()
        },
        "小猫看到蝴蝶",
    );
    let payoff_extreme = run_with_shot(
        ShotContext {
            mood: Some("epic".into()),
            camera: Some("extreme".into()),
            ..Default::default()
        },
        "小猫扑向蝴蝶",
    );
    assert_ne!(
        hook_wide, payoff_extreme,
        "不同 mood/camera 应产生不同 prompt (否则 #1 静默回退)"
    );

    let hook_text = hook_wide
        .split_once("[SEEDANCE_PROMPT]\n")
        .unwrap()
        .1
        .split_once("\n[session=")
        .unwrap()
        .0;
    let payoff_text = payoff_extreme
        .split_once("[SEEDANCE_PROMPT]\n")
        .unwrap()
        .1
        .split_once("\n[session=")
        .unwrap()
        .0;

    assert!(hook_text.contains("[Camera] wide establishing shot"));
    assert!(hook_text.contains("[Motion] bouncy motion"));
    assert!(payoff_text.contains("[Camera] extreme close-up"));
    assert!(payoff_text.contains("[Motion] dramatic motion"));
}

#[test]
fn case_3_unknown_mood_camera_silently_falls_back_to_default() {
    // 不存在的枚举值 → 静默回退 Calm/Medium, 不报错
    let shot = ShotContext {
        mood: Some("happiness".into()),  // ❌ 不在白名单
        camera: Some("overhead-shot".into()),  // ❌ 不在白名单 (overhead 是)
        ..Default::default()
    };
    let prompt_log = run_with_shot(shot, "小猫跑");
    // 不报错 + 走工业级路径
    assert!(prompt_log.contains("[SEEDANCE_PROMPT]"));
    // prompt_for_log 排障行显示原始字符串 (而不是 enum)
    assert!(prompt_log.contains("mood=happiness"));
    assert!(prompt_log.contains("camera=overhead-shot"));
}

#[test]
fn case_4_no_shot_context_falls_back_to_w46_5_default_path() {
    // shot 整个缺失 (None) → 老路径 + 默认 Calm/Medium, 跟 W4.6 #5 完全兼容
    std::env::remove_var("SEEDANCE_API_KEY");
    let tool = ImageToVideoTool;
    let args_json = r#"{"motion":"小猫跑","image_url":"https://x.test/img.png","image_role":"reference_image","duration":4,"seed":42}"#;
    let ctx = ToolContext {
        character: Some(sample_character()),
        style: Some(sample_style()),
        seed_session: Some(11111),
        scene: None,
        shot: None, // W4.6 #5 老路径
    };
    let out = tool
        .execute_with_context(args_json, "sess_no_shot", &ctx)
        .expect("ok");
    let prompt_log = out.assets[0].prompt.clone();
    let prompt_text = prompt_log
        .split_once("[SEEDANCE_PROMPT]\n")
        .unwrap()
        .1
        .split_once("\n[session=")
        .unwrap()
        .0;

    // 默认 Calm/Medium → [Camera] medium shot, [Motion] gentle motion
    assert!(prompt_text.contains("[Camera] medium shot"));
    assert!(prompt_text.contains("[Motion] gentle motion"));
    // 排障行用 (default calm)/(default medium) 占位
    assert!(prompt_log.contains("mood=(default calm)"));
    assert!(prompt_log.contains("camera=(default medium)"));
}

#[test]
fn case_5_three_beats_all_parseable_and_logged() {
    // 5 拍节奏下, 三种 beat 都解析 → prompt_for_log 排障可见
    for beat in ["hook", "conflict", "payoff"] {
        let shot = ShotContext {
            beat: Some(beat.into()),
            mood: Some("joyful".into()),
            camera: Some("medium".into()),
            ..Default::default()
        };
        let prompt_log = run_with_shot(shot, "小猫跑");
        assert!(
            prompt_log.contains(&format!("beat={beat}")),
            "beat={beat} 应出现在排障行; got: {prompt_log}"
        );
    }
}

#[test]
fn case_6_parse_shot_mood_unit() {
    // parse_shot_mood / parse_shot_camera 单元测试 (覆盖 5 个 + 2 个 default)
    use kidsai_studio_lib::prompt_builder::{ShotCamera, ShotMood};
    assert!(matches!(parse_shot_mood(Some("calm")), ShotMood::Calm));
    assert!(matches!(parse_shot_mood(Some("tense")), ShotMood::Tense));
    assert!(matches!(parse_shot_mood(Some("joyful")), ShotMood::Joyful));
    assert!(matches!(parse_shot_mood(Some("sad")), ShotMood::Sad));
    assert!(matches!(parse_shot_mood(Some("epic")), ShotMood::Epic));
    // 缺省 / 未知 → Calm
    assert!(matches!(parse_shot_mood(None), ShotMood::Calm));
    assert!(matches!(parse_shot_mood(Some("happy")), ShotMood::Calm)); // "happy" 不在白名单 (前端已删)
    assert!(matches!(parse_shot_mood(Some("")), ShotMood::Calm));

    assert!(matches!(parse_shot_camera(Some("wide")), ShotCamera::Wide));
    assert!(matches!(parse_shot_camera(Some("medium")), ShotCamera::Medium));
    assert!(matches!(parse_shot_camera(Some("close")), ShotCamera::Close));
    assert!(matches!(parse_shot_camera(Some("extreme")), ShotCamera::Extreme));
    assert!(matches!(parse_shot_camera(Some("follow")), ShotCamera::Follow));
    assert!(matches!(parse_shot_camera(Some("overhead")), ShotCamera::Overhead));
    assert!(matches!(parse_shot_camera(None), ShotCamera::Medium));
    assert!(matches!(parse_shot_camera(Some("drone")), ShotCamera::Medium)); // "drone" 不在白名单
}

#[test]
fn case_7_build_seedance_prompt_with_scene_uses_shot_camera_and_mood() {
    // 直接验证 build_seedance_prompt 在带 scene 时, shot.camera/mood 仍生效
    // (run_video_pipeline 老路径不传 scene, 这是验证 scene + shot 同时存在的全路径)
    let character = CharacterAsset {
        id: "xiaoqi".into(),
        full_name: "小启 (yellow short hair cat girl)".into(),
        style_tags: vec!["cartoon".into()],
        standard_image_url: "https://x.test/stand.png".into(),
        side_image_url: None,
        back_image_url: None,
    };
    let style = StyleAsset {
        id: "cartoon".into(),
        name: "卡通".into(),
        seedance_style_keyword: "bright cartoon style".into(),
    };
    let scene = SceneAsset {
        id: "garden".into(),
        name: "花园".into(),
        setting: "summer afternoon sun through leaves".into(),
        time_of_day: "afternoon".into(),
        weather: "sunny".into(),
        bg_url: "https://x.test/bg/garden.png".into(),
    };
    let shot = kidsai_studio_lib::prompt_builder::ShotPromptInput {
        subject: "小启".into(),
        action: "追着蝴蝶跑".into(),
        setting: "花园".into(),
        mood: kidsai_studio_lib::prompt_builder::ShotMood::Joyful,
        camera: kidsai_studio_lib::prompt_builder::ShotCamera::Follow,
    };
    let options = PromptOptions::default();
    let prompt = build_seedance_prompt(&shot, &character, &style, Some(&scene), 12345, &options);

    assert!(prompt.contains("[Camera] tracking shot"));
    assert!(prompt.contains("[Motion] bouncy motion"));
    assert!(prompt.contains("[Scene] 花园"));
    assert!(prompt.contains("[Lighting] golden hour warm rim light"));
    assert!(prompt.contains("Use seed: 12345"));
}