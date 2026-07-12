// 风格模板切换（W3.6）
// 设计：每个 session 可选绑定一个 StylePreset；style 的描述会被自动追加到 system_prompt
// 和 generate_image 工具的 prompt 里，确保同 session 内多次生成的图片保持视觉风格一致。
//
// 与 Character（"画谁"）独立、可叠加：角色决定形象，风格决定画法。
// 例：小启 + 水墨 → 小启形象的、用水墨画风呈现的角色。

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// 风格模板
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StylePreset {
    pub id: String,
    pub name: String,
    /// 视觉描述，会注入到 system_prompt 和 image prompt
    pub description: String,
    /// 风格标签（cartoon / ink_wash / pixel_art ...）
    pub style_tags: Vec<String>,
}

/// 风格注册表（in-memory）
#[derive(Default)]
pub struct StyleRegistry {
    map: Mutex<HashMap<String, StylePreset>>,
}

impl StyleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, s: StylePreset) {
        self.map.lock().unwrap().insert(s.id.clone(), s);
    }

    pub fn get(&self, id: &str) -> Option<StylePreset> {
        self.map.lock().unwrap().get(id).cloned()
    }

    pub fn all(&self) -> Vec<StylePreset> {
        let mut v: Vec<StylePreset> = self.map.lock().unwrap().values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }
}

/// 内置风格（7 种 — 覆盖卡通 / 国风 / 复古 / 3D / 简笔 / 日系 / 写实）
/// prompt 描述需要让当前主流图像模型（智谱 CogView / SDXL / MJ）都能稳定识别
pub fn builtin_styles() -> Vec<StylePreset> {
    vec![
        StylePreset {
            id: "anime".into(),
            name: "🌸 日系动漫".into(),
            description: "日系动漫插画风格，新海诚 / 宫崎骏风，精致细腻，唯美光影".into(),
            style_tags: vec!["anime".into(), "illustration".into()],
        },
        StylePreset {
            id: "cartoon".into(),
            name: "🎨 卡通".into(),
            description: "Pixar/迪士尼风格卡通，鲜艳色彩，圆润造型，亲切可爱".into(),
            style_tags: vec!["cartoon".into(), "vibrant".into()],
        },
        StylePreset {
            id: "clay".into(),
            name: "🌈 3D 黏土".into(),
            description: "黏土材质 3D 渲染，柔软、可爱、有手工质感".into(),
            style_tags: vec!["3d_render".into(), "claymation".into()],
        },
        StylePreset {
            id: "ink".into(),
            name: "🖌️ 水墨".into(),
            description: "中国传统水墨画风格，留白、墨韵、淡雅写意".into(),
            style_tags: vec!["ink_wash".into(), "minimalist".into()],
        },
        StylePreset {
            id: "line-drawing".into(),
            name: "✏️ 简笔画".into(),
            description: "黑白线条简笔画，简洁干净，童趣手绘".into(),
            style_tags: vec!["line_art".into(), "minimalist".into()],
        },
        StylePreset {
            id: "photo".into(),
            name: "📷 写实摄影".into(),
            description: "真实摄影质感，自然光，浅景深".into(),
            style_tags: vec!["photographic".into(), "realistic".into()],
        },
        StylePreset {
            id: "pixel".into(),
            name: "📺 像素".into(),
            description: "复古 8-bit 像素艺术，颗粒感、轮廓清晰、鲜艳".into(),
            style_tags: vec!["pixel_art".into(), "retro".into()],
        },
    ]
}

/// 构造带风格上下文的 system_prompt
/// 风格描述作为独立段落追加；与 character 块并行不冲突
pub fn build_system_prompt_with_style(base: &str, style: Option<&StylePreset>) -> String {
    match style {
        Some(s) => format!(
            "{}\n\n[当前风格]\n名称: {}\n视觉描述: {}\n（请在生成图片 / 视频时保持该视觉风格一致）",
            base,
            s.name,
            s.description,
        ),
        None => base.to_string(),
    }
}

/// 把风格描述注入 generate_image 工具的 prompt 字段
/// - 工具不是 generate_image / args 不是合法 JSON / 没有 prompt 字段：原样返回
/// - 注入成功：把 `, 风格: <description>.` 追加到 prompt 末尾
pub fn inject_style_into_image_args(args_json: &str, style: &StylePreset) -> String {
    let Ok(mut args) = serde_json::from_str::<serde_json::Value>(args_json) else {
        return args_json.to_string();
    };
    let Some(obj) = args.as_object_mut() else {
        return args_json.to_string();
    };
    let Some(prompt) = obj.get("prompt").and_then(|v| v.as_str()) else {
        return args_json.to_string();
    };
    let new_prompt = format!("{}. 视觉风格: {}.", prompt, style.description);
    obj.insert("prompt".into(), serde_json::Value::String(new_prompt));
    serde_json::to_string(&args).unwrap_or_else(|_| args_json.to_string())
}

/// v1 导演流程:把风格注入 image_to_video 工具的 motion 字段
/// - args 不是合法 JSON / 缺 motion 字段：原样返回
/// - 注入成功:把 `, 视觉风格: <description>.` 追加到 motion 末尾
pub fn inject_style_into_video_args(args_json: &str, style: &StylePreset) -> String {
    let Ok(mut args) = serde_json::from_str::<serde_json::Value>(args_json) else {
        return args_json.to_string();
    };
    let Some(obj) = args.as_object_mut() else {
        return args_json.to_string();
    };
    let Some(motion) = obj.get("motion").and_then(|v| v.as_str()) else {
        return args_json.to_string();
    };
    let new_motion = format!("{}. 视觉风格: {}.", motion, style.description);
    obj.insert("motion".into(), serde_json::Value::String(new_motion));
    serde_json::to_string(&args).unwrap_or_else(|_| args_json.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> StylePreset {
        StylePreset {
            id: "ink".into(),
            name: "🖌️ 水墨".into(),
            description: "中国传统水墨画风格".into(),
            style_tags: vec!["ink_wash".into()],
        }
    }

    #[test]
    fn build_system_prompt_none_passthrough() {
        let s = build_system_prompt_with_style("你是小启", None);
        assert_eq!(s, "你是小启");
    }

    #[test]
    fn build_system_prompt_with_style_appends_block() {
        let s = sample();
        let out = build_system_prompt_with_style("你是小启", Some(&s));
        assert!(out.contains("你是小启"));
        assert!(out.contains("[当前风格]"));
        assert!(out.contains("水墨"));
        assert!(out.contains("中国传统水墨画风格"));
    }

    #[test]
    fn inject_style_appends_to_prompt() {
        let s = sample();
        let args = r#"{"prompt":"在花园里玩","style":"line"}"#;
        let out = inject_style_into_image_args(args, &s);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let prompt = v["prompt"].as_str().unwrap();
        assert!(prompt.contains("在花园里玩"), "original preserved: {}", prompt);
        assert!(prompt.contains("中国传统水墨画风格"), "style description injected: {}", prompt);
        // 其他字段不动
        assert_eq!(v["style"], "line");
    }

    #[test]
    fn inject_style_no_prompt_passthrough() {
        let s = sample();
        let args = r#"{"style":"line"}"#;
        let out = inject_style_into_image_args(args, &s);
        assert_eq!(out, args, "no prompt field → return as-is");
    }

    #[test]
    fn inject_style_invalid_json_passthrough() {
        let s = sample();
        let args = "not json";
        let out = inject_style_into_image_args(args, &s);
        assert_eq!(out, args);
    }

    #[test]
    fn registry_register_and_get() {
        let reg = StyleRegistry::new();
        let s = sample();
        reg.register(s.clone());
        assert_eq!(reg.get("ink"), Some(s.clone()));
        assert!(reg.get("missing").is_none());
    }

    #[test]
    fn registry_all_sorted_by_id() {
        let reg = StyleRegistry::new();
        reg.register(StylePreset {
            id: "z".into(),
            ..sample()
        });
        reg.register(StylePreset {
            id: "a".into(),
            ..sample()
        });
        let all = reg.all();
        assert_eq!(all[0].id, "a");
        assert_eq!(all[1].id, "z");
    }

    #[test]
    fn builtin_styles_returns_seven() {
        let all = builtin_styles();
        assert_eq!(all.len(), 7, "expected 7 builtin styles, got {}", all.len());
        // 7 个 id 必须唯一
        let ids: Vec<&str> = all.iter().map(|s| s.id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "style ids must be unique");
    }

    /// v1 导演流程:把风格描述追加到 motion 字段
    #[test]
    fn inject_style_into_video_args_appends_description() {
        let s = sample();
        let args = r#"{"motion":"小猫跳跃"}"#;
        let out = inject_style_into_video_args(args, &s);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let motion = v["motion"].as_str().unwrap();
        assert!(motion.contains("小猫跳跃"), "原 motion 保留");
        assert!(motion.contains("中国传统水墨画风格"), "风格描述追加");
    }

    /// 缺 motion 字段 → 原样返回(避免 schema 破坏)
    #[test]
    fn inject_style_into_video_args_no_motion_passthrough() {
        let s = sample();
        let args = r#"{"duration":5}"#;
        let out = inject_style_into_video_args(args, &s);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["duration"], 5, "未注入 motion 时原字段保留");
        assert!(v.get("motion").is_none(), "不应凭空创建 motion 字段");
    }
}