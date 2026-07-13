// 角色一致性资产库（W3.4）
// 设计：每个 session 可选绑定一个 Character；character 的描述会被自动追加到 system_prompt
// 和 generate_image 工具的 prompt 里，确保同 session 内多次生成的图片保持角色一致。
//
// 参考 OiiOii 的「全局资产记忆库」思想：MVP 阶段用 in-memory registry + 内置 3-5 个角色，
// W4 切到持久化（SQLite）。

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// 角色定义
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Character {
    pub id: String,
    pub name: String,
    /// 外貌 / 服装 / 性格 描述，会被拼到 system_prompt 和 image prompt
    pub description: String,
    /// 风格标签（cartoon / realistic / 水墨 ...）
    pub style_tags: Vec<String>,
    /// 可选参考图 URL（真实 IP-Adapter / 角色一致性模型接入时用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_image_url: Option<String>,
    /// W4.6 #2: 标准像 URL (1张三视图合图, 由 image-01 生成, 用于 Seedance 硬锚 first_frame/reference_image).
    /// 第一次进 studio 时, 后端检测到 None 会自动生成并回填.
    /// 注: reference_image_url 在 W4.6 前是 picsum 占位 (异步确定性图片), 现在与 standard_image_url 字段并存:
    /// reference_image_url 作为单图参考 (v1 多模态参考模式), standard_image_url 作为三视图角色卡 (W4.6+ 跨镜锚).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub standard_image_url: Option<String>,
    /// W4.6 #2: 角色别名清单 (≥2 个, 第 1 个是基准名). 用于跨镜别名锚定 (防止模型输出"小明/小启/小星"漂移).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub aliases: Option<Vec<String>>,
}

/// 角色注册表（in-memory）
#[derive(Default)]
pub struct CharacterRegistry {
    map: Mutex<HashMap<String, Character>>,
}

impl CharacterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, c: Character) {
        self.map.lock().unwrap().insert(c.id.clone(), c);
    }

    pub fn get(&self, id: &str) -> Option<Character> {
        self.map.lock().unwrap().get(id).cloned()
    }

    pub fn all(&self) -> Vec<Character> {
        let mut v: Vec<Character> = self.map.lock().unwrap().values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }
}

/// 内置角色（演示用 — 真实用户可在 W4 自己创建）
///
/// v1 导演流程:reference_image_url 填 picsum 占位 URL(确定性、零网络成本),
/// 角色一致性靠它作为 Seedance 的 reference_image 软参考。
/// v1.1 替换为 Seedance 文生图生成的真实角色图。
pub fn builtin_characters() -> Vec<Character> {
    vec![
        Character {
            id: "xiaoqi".into(),
            name: "小启".into(),
            description: "一个9岁的好奇小猫女孩，黄色短发、穿黄色T恤、戴小围巾、眼睛又大又亮".into(),
            style_tags: vec!["cartoon".into(), "child_friendly".into()],
            reference_image_url: Some("https://picsum.photos/seed/xiaoqi-ref/512/512".into()),
            // W4.6 #2: 首进 studio 后, 后端检测 None 自动调 image-01 生成三视图并回填.
            standard_image_url: None,
            // W4.6 #2: 别名清单 (第 1 个基准, 后 2 个 LLM 应优先用, 防止 drift).
            aliases: Some(vec![
                "小启".into(),
                "小启猫".into(),
                "XiaoQi".into(),
            ]),
        },
        Character {
            id: "xiaoyue".into(),
            name: "小月".into(),
            description: "一个8岁的小女孩，扎着双马尾、穿红色连衣裙、手里常捧着一本书".into(),
            style_tags: vec!["cartoon".into(), "child_friendly".into()],
            reference_image_url: Some("https://picsum.photos/seed/xiaoyue-ref/512/512".into()),
            standard_image_url: None,
            aliases: Some(vec![
                "小月".into(),
                "小月儿".into(),
                "XiaoYue".into(),
            ]),
        },
        Character {
            id: "xiaoxing".into(),
            name: "小星".into(),
            description: "一个10岁的小男孩，戴黑框眼镜、穿蓝色卫衣、爱思考".into(),
            style_tags: vec!["cartoon".into(), "child_friendly".into()],
            reference_image_url: Some("https://picsum.photos/seed/xiaoxing-ref/512/512".into()),
            standard_image_url: None,
            aliases: Some(vec![
                "小星".into(),
                "星仔".into(),
                "XiaoXing".into(),
            ]),
        },
    ]
}

/// 构造带角色上下文的 system_prompt
/// 角色描述会作为独立段落追加，模型能稳定看到
pub fn build_system_prompt_with_character(base: &str, character: Option<&Character>) -> String {
    match character {
        Some(c) => format!(
            "{}\n\n[当前角色]\n名称: {}\n外貌: {}\n风格: {}\n（请在生成图片 / 视频时保持该角色形象一致）",
            base,
            c.name,
            c.description,
            c.style_tags.join(", "),
        ),
        None => base.to_string(),
    }
}

/// 把角色描述注入 generate_image 工具的 prompt 字段
/// - 工具不是 generate_image：原样返回
/// - args 不是合法 JSON：原样返回（不破坏）
/// - 没有 prompt 字段：原样返回（避免破坏 schema）
/// - 注入成功：返回新 JSON 字符串
pub fn inject_character_into_image_args(args_json: &str, character: &Character) -> String {
    let Ok(mut args) = serde_json::from_str::<serde_json::Value>(args_json) else {
        return args_json.to_string();
    };
    let Some(obj) = args.as_object_mut() else {
        return args_json.to_string();
    };
    let Some(prompt) = obj.get("prompt").and_then(|v| v.as_str()) else {
        return args_json.to_string();
    };
    let style = character.style_tags.join("/");
    let new_prompt = format!(
        "{}. 角色: {}({}), 风格: {}.",
        prompt, character.name, character.description, style
    );
    obj.insert("prompt".into(), serde_json::Value::String(new_prompt));
    serde_json::to_string(&args).unwrap_or_else(|_| args_json.to_string())
}

/// W4.6 #2: 构造"1 张三视图角色卡"的 image-01 prompt.
///
/// 工业版三视图合图 (来自分镜指令-生图提示词-人物生图.txt 调研):
/// - 左 3 视图: 正 / 侧 / 背 全身 (head to toe)
/// - 右 3 特写: 嘴 / 眼 / 发
/// - 8% 边距, 80mm 等效焦距, 柔光均匀
/// - 人物无表情, 无现代物件 (防穿模)
/// - 风格修饰走工业级风格 (Ghibli/Pixar 等), 不写 ARRI/Hue/IRE
fn three_view_background_clause() -> &'static str {
    "pure white background, 8% safe margin, 80mm portrait lens equivalent, soft even lighting, no facial expression, no modern objects"
}

fn three_view_composition_clause() -> &'static str {
    "left half: front view, side view, back view (full body, head to toe); right half: 3 close-ups (mouth, eyes, hair); figures share identical anatomy and identical outfit across all views"
}

/// 工业级三视图角色卡 prompt — 让 image-01 出 1 张合图.
///
/// 用法: 前端在 stage 2 选完角色后首次进 studio 时, 后端检测 standard_image_url 为 None,
/// 用此 prompt 调 generate_image 生成三视图, URL 存到 character.standard_image_url.
pub fn build_three_view_prompt(character: &Character, seedance_style_keyword: &str) -> String {
    format!(
        "[W4.6 character card] one image, 6 panels.\n\
         [Subject] {name}, child-friendly character, key features: {desc}\n\
         [Style] {style_kw}\n\
         [Composition] {composition}\n\
         [Background] {background}\n\
         [Constraints] outfit identical in all 6 panels (≤3 layers: top/outer/bottom+shoes), accessories consistent; hair color/style consistent; skin tone consistent; same character repeated 6 times, not 6 different characters",
        name = character.name,
        desc = character.description,
        style_kw = seedance_style_keyword,
        composition = three_view_composition_clause(),
        background = three_view_background_clause(),
    )
}

/// W4.6 #2: 提示 LLM 为后续跨镜锚定输出别名清单 (≥2 个, 第 1 个是基准名).
///
/// 实际别名仍由 LLM 生成 (含昵称/称呼/简称/English name), 这里给出"角色段"prompt 拼接给 system prompt.
/// 不要写进 user 输入 — 这是 system_prompt 的一段.
pub fn build_aliases_system_prompt_section(character: Option<&Character>) -> String {
    match character {
        Some(c) => format!(
            "\n\n## W4.6 角色卡别名清单 (强制)\n\
             主角: {name}\n\
             你的回复中, 当引用该角色时, 必须用以下任一名称 (不要创造新名字, 防止跨镜 alias drift):\n\
             - {baseline}\n\
             - (LLM 负责在此追加 ≥1 个昵称/称呼/简称, 输出格式: `aliases: [\"基准名\", \"昵称1\", ...]`)",
            name = c.name,
            baseline = c.name,
        ),
        None => String::new(),
    }
}

/// 把 standard_image_url 写回注册表 (用于后端检测到 None 时自动生成 → 回填).
/// in-memory registry 不能直接 mutation from outside; 提供显式 setter 测试 + 流程都用.
impl CharacterRegistry {
    pub fn set_standard_image_url(&self, id: &str, url: &str) -> Result<(), String> {
        let mut map = self.map.lock().unwrap();
        let c = map
            .get_mut(id)
            .ok_or_else(|| format!("character not found: {id}"))?;
        c.standard_image_url = Some(url.to_string());
        Ok(())
    }
}

/// v1 导演流程:把角色注入 image_to_video 工具的 args。
/// 行为:
/// - 自动把 character.reference_image_url 写入 image_url(若原本为空)
/// - 自动设置 image_role="reference_image"(让 Seedance 走多模态参考模式)
/// - 在 motion 文本末尾追加"保持<角色名>的样子:<description>"
///
/// 失败兜底:args_json 不是合法 JSON / 缺 motion 字段 → 原样返回(不破坏)
pub fn inject_character_into_video_args(args_json: &str, character: &Character) -> String {
    let Ok(mut args) = serde_json::from_str::<serde_json::Value>(args_json) else {
        return args_json.to_string();
    };
    let Some(obj) = args.as_object_mut() else {
        return args_json.to_string();
    };
    // 1) 自动填 image_url (若没有)
    if !obj.contains_key("image_url") || obj["image_url"].is_null() {
        if let Some(url) = &character.reference_image_url {
            obj.insert("image_url".into(), serde_json::Value::String(url.clone()));
        }
    }
    // 2) 强制 image_role=reference_image(若 LLM 没设)
    if !obj.contains_key("image_role") || obj["image_role"].is_null() {
        obj.insert(
            "image_role".into(),
            serde_json::Value::String("reference_image".into()),
        );
    }
    // 3) 把角色信息追加到 motion 文本
    if let Some(motion) = obj.get("motion").and_then(|v| v.as_str()) {
        let new_motion = format!(
            "{}. 保持角色 {} 的样子: {}",
            motion, character.name, character.description
        );
        obj.insert("motion".into(), serde_json::Value::String(new_motion));
    }
    serde_json::to_string(&args).unwrap_or_else(|_| args_json.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Character {
        Character {
            id: "x".into(),
            name: "小启".into(),
            description: "黄发女孩".into(),
            style_tags: vec!["cartoon".into()],
            reference_image_url: None,
            standard_image_url: None,
            aliases: None,
        }
    }

    #[test]
    fn build_system_prompt_none_passthrough() {
        let s = build_system_prompt_with_character("你是小启", None);
        assert_eq!(s, "你是小启");
    }

    #[test]
    fn build_system_prompt_with_character_appends_block() {
        let c = sample();
        let s = build_system_prompt_with_character("你是小启", Some(&c));
        assert!(s.contains("你是小启"));
        assert!(s.contains("[当前角色]"));
        assert!(s.contains("小启"));
        assert!(s.contains("黄发女孩"));
        assert!(s.contains("cartoon"));
    }

    #[test]
    fn inject_image_args_appends_character_to_prompt() {
        let c = sample();
        let args = r#"{"prompt":"在花园里玩","style":"cartoon"}"#;
        let out = inject_character_into_image_args(args, &c);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let prompt = v["prompt"].as_str().unwrap();
        assert!(prompt.contains("花园里玩"), "original prompt preserved: {}", prompt);
        assert!(prompt.contains("小启"), "character name injected: {}", prompt);
        assert!(prompt.contains("黄发女孩"), "character description injected: {}", prompt);
        // 其他字段不动
        assert_eq!(v["style"], "cartoon");
    }

    #[test]
    fn inject_image_args_no_prompt_passthrough() {
        let c = sample();
        let args = r#"{"style":"cartoon"}"#;
        let out = inject_character_into_image_args(args, &c);
        assert_eq!(out, args, "no prompt field → return as-is");
    }

    #[test]
    fn inject_image_args_invalid_json_passthrough() {
        let c = sample();
        let args = "not json";
        let out = inject_character_into_image_args(args, &c);
        assert_eq!(out, args);
    }

    #[test]
    fn inject_image_args_non_generate_image_tool_passthrough() {
        // 这个函数只处理 generate_image，其他工具不归它管
        // 但函数本身是「给定 args 注入」语义；测试用例仅覆盖注入逻辑
        let c = sample();
        let args = r#"{"text":"hi"}"#;
        let out = inject_character_into_image_args(args, &c);
        // 因为没有 prompt 字段，原样返回
        assert_eq!(out, args);
    }

    #[test]
    fn registry_register_and_get() {
        let reg = CharacterRegistry::new();
        let c = sample();
        reg.register(c.clone());
        assert_eq!(reg.get("x"), Some(c.clone()));
        assert!(reg.get("y").is_none());
    }

    #[test]
    fn registry_all_sorted_by_id() {
        let reg = CharacterRegistry::new();
        reg.register(Character {
            id: "z".into(),
            ..sample()
        });
        reg.register(Character {
            id: "a".into(),
            ..sample()
        });
        let all = reg.all();
        assert_eq!(all[0].id, "a");
        assert_eq!(all[1].id, "z");
    }

    /// v1 导演流程:3 个内置角色都必须有 reference_image_url(Seedance 多模态参考的素材源)
    #[test]
    fn builtin_characters_have_reference_image_url_set() {
        let builtins = builtin_characters();
        assert_eq!(builtins.len(), 3);
        for c in &builtins {
            assert!(
                c.reference_image_url.is_some(),
                "角色 {} 必须有 reference_image_url",
                c.id
            );
            let url = c.reference_image_url.as_ref().unwrap();
            assert!(
                url.contains(&c.id),
                "reference_image_url 应基于角色 id 命名,got: {url}"
            );
            assert!(url.contains("picsum.photos/seed/"), "应为 picsum 占位 URL,got: {url}");
        }
    }

    fn sample_with_ref() -> Character {
        Character {
            id: "x".into(),
            name: "小启".into(),
            description: "黄发女孩".into(),
            style_tags: vec!["cartoon".into()],
            reference_image_url: Some("https://picsum.photos/seed/x-ref/512/512".into()),
            standard_image_url: None,
            aliases: None,
        }
    }

    /// 角色无 ref 图:不强行注入 image_url,只追加 motion(降级而非报错)
    #[test]
    fn inject_video_args_no_ref_url_skips_image_inject() {
        let c = Character {
            reference_image_url: None,
            ..sample_with_ref()
        };
        let args = r#"{"motion":"小猫跳跃","duration":5}"#;
        let out = inject_character_into_video_args(args, &c);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // 没有 ref_url → 不强写 image_url
        assert!(v.get("image_url").is_none(), "无 ref_url 不应注入 image_url");
        // motion 仍追加了角色描述
        let motion = v["motion"].as_str().unwrap();
        assert!(motion.contains("小猫跳跃"));
        assert!(motion.contains("小启"));
        assert!(motion.contains("黄发女孩"));
    }

    /// 标准注入:自动填 image_url + image_role + 追加 motion
    #[test]
    fn inject_video_args_fills_image_url_and_role_and_motion() {
        let c = sample_with_ref();
        let args = r#"{"motion":"跳跃"}"#;
        let out = inject_character_into_video_args(args, &c);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["image_url"], "https://picsum.photos/seed/x-ref/512/512");
        assert_eq!(v["image_role"], "reference_image");
        let motion = v["motion"].as_str().unwrap();
        assert!(motion.contains("跳跃"));
        assert!(motion.contains("小启"));
    }

    /// LLM 已经显式传了 image_url → 不覆盖
    #[test]
    fn inject_video_args_does_not_overwrite_existing_image_url() {
        let c = sample_with_ref();
        let args = r#"{"motion":"x","image_url":"https://e/llm.jpg"}"#;
        let out = inject_character_into_video_args(args, &c);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["image_url"], "https://e/llm.jpg", "LLM 显式给的图应保留");
    }

    /// 缺 motion 字段 → 原样返回(不破坏 schema,避免误注入)
    #[test]
    fn inject_video_args_no_motion_passthrough() {
        let c = sample_with_ref();
        let args = r#"{"duration":5}"#;
        let out = inject_character_into_video_args(args, &c);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // motion 缺失 → 至少 image_url/image_role 仍被自动填上(ref 路径还是走得到的)
        assert_eq!(v["image_url"], "https://picsum.photos/seed/x-ref/512/512");
        assert_eq!(v["image_role"], "reference_image");
    }

    // ─── W4.6 #2 三视图 + 别名清单 ───────────────────────

    /// 三视图角色卡 prompt 包含 6 panel (3 视图 + 3 特写), 防穿模约束
    #[test]
    fn build_three_view_prompt_has_six_panels_and_constraints() {
        let c = sample();
        let prompt = build_three_view_prompt(&c, "Studio-Ghibli inspired soft watercolor");
        assert!(prompt.contains("W4.6 character card"), "got: {prompt}");
        assert!(prompt.contains("[Subject] 小启"), "got: {prompt}");
        assert!(prompt.contains("黄发女孩"), "got: {prompt}");
        assert!(prompt.contains("[Style] Studio-Ghibli"), "got: {prompt}");
        assert!(prompt.contains("front view"), "got: {prompt}");
        assert!(prompt.contains("side view"), "got: {prompt}");
        assert!(prompt.contains("back view"), "got: {prompt}");
        assert!(prompt.contains("mouth"), "got: {prompt}");
        assert!(prompt.contains("eyes"), "got: {prompt}");
        assert!(prompt.contains("hair"), "got: {prompt}");
        // 背景 + 边距约束
        assert!(prompt.contains("pure white background"), "got: {prompt}");
        assert!(prompt.contains("8% safe margin"), "got: {prompt}");
        // 防穿模约束
        assert!(prompt.contains("no facial expression"), "got: {prompt}");
        assert!(prompt.contains("identical"), "got: {prompt}");
    }

    /// 别名清单 system_prompt 段:有 character 时输出强制段; None 时原样返回空字符串
    #[test]
    fn build_aliases_system_prompt_section_appears_when_character_set() {
        let c = sample();
        let section = build_aliases_system_prompt_section(Some(&c));
        assert!(section.contains("W4.6 角色卡别名清单"), "got: {section}");
        assert!(section.contains("小启"), "got: {section}");
        assert!(section.contains("防止跨镜 alias drift"), "got: {section}");
        // None 时返回空 (向后兼容, 老 system_prompt 不破)
        let empty = build_aliases_system_prompt_section(None);
        assert!(empty.is_empty(), "None 应返空, got: {empty}");
    }

    /// standard_image_url 字段 — registry 能在生成回填时改写
    #[test]
    fn character_registry_set_standard_image_url_writes_back() {
        let reg = CharacterRegistry::new();
        reg.register(sample());
        // 初始 None
        assert!(reg.get("x").unwrap().standard_image_url.is_none());
        // 回填
        reg.set_standard_image_url("x", "https://e/stand.png").unwrap();
        let updated = reg.get("x").unwrap();
        assert_eq!(
            updated.standard_image_url.as_deref(),
            Some("https://e/stand.png")
        );
    }

    /// set_standard_image_url 不存在的角色应报错 (不静默创建)
    #[test]
    fn character_registry_set_standard_image_url_errors_on_unknown_id() {
        let reg = CharacterRegistry::new();
        let err = reg.set_standard_image_url("nope", "url").unwrap_err();
        assert!(err.contains("nope"), "got: {err}");
    }

    /// 3 个 builtin 角色都有 aliases (>=2) — 防止跨镜 alias drift 的前置准备
    #[test]
    fn builtin_characters_have_aliases_set() {
        for c in builtin_characters() {
            let aliases = c
                .aliases
                .as_ref()
                .unwrap_or_else(|| panic!("角色 {} 必须有 aliases", c.id));
            assert!(
                aliases.len() >= 2,
                "角色 {} 的 aliases 至少 2 个 (基准 + 1 昵称), got: {:?}",
                c.id,
                aliases
            );
            // 基准名必须是 names[0]
            assert_eq!(
                aliases[0], c.name,
                "角色 {} 基准名应为 name={}",
                c.id, c.name
            );
        }
    }
}
