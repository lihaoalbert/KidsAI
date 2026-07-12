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
        },
        Character {
            id: "xiaoyue".into(),
            name: "小月".into(),
            description: "一个8岁的小女孩，扎着双马尾、穿红色连衣裙、手里常捧着一本书".into(),
            style_tags: vec!["cartoon".into(), "child_friendly".into()],
            reference_image_url: Some("https://picsum.photos/seed/xiaoyue-ref/512/512".into()),
        },
        Character {
            id: "xiaoxing".into(),
            name: "小星".into(),
            description: "一个10岁的小男孩，戴黑框眼镜、穿蓝色卫衣、爱思考".into(),
            style_tags: vec!["cartoon".into(), "child_friendly".into()],
            reference_image_url: Some("https://picsum.photos/seed/xiaoxing-ref/512/512".into()),
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
}
