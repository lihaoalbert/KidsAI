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
    /// 视觉描述，会注入到 system_prompt 和 image prompt (给 LLM 看的, 自然语言)
    pub description: String,
    /// 风格标签（cartoon / ink_wash / pixel_art ...）
    pub style_tags: Vec<String>,
    /// W4.6 #3: Seedance 2.0 专用工业级 keyword (给 Seedance 看的, 短关键词串).
    /// 与 description 区别: description 是给 LLM 看的完整描述, 这个是给 Seedance 用的紧凑关键词链.
    /// 示例 (ghibli): "Studio-Ghibli inspired soft watercolor, cel-shaded, pastel palette".
    /// 没填时, video_adapter 沿用 description 作为 fallback (W4.6 #5 临时行为, 老数据兼容).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub seedance_style_keyword: Option<String>,
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
///
/// W4.6 #3: seedance_style_keyword 字段 (Option<String>) 由 LLM 预生成 (或手工审阅).
/// 7 个内置值是手工写的工业级 Seedance 关键词串, 跟调研文档 (docs/reffrence/Seedance调研)
/// 验证有效的关键词对齐. description 是给 LLM 看的完整描述, seedance_style_keyword 是给 Seedance 用的紧凑关键词.
pub fn builtin_styles() -> Vec<StylePreset> {
    vec![
        StylePreset {
            id: "anime".into(),
            name: "🌸 日系动漫".into(),
            description: "日系动漫插画风格，新海诚 / 宫崎骏风，精致细腻，唯美光影".into(),
            style_tags: vec!["anime".into(), "illustration".into()],
            seedance_style_keyword: Some(
                "anime style, Studio Ghibli inspired, soft watercolor background, cel shading, \
                 hand-painted look, vibrant but soft palette, detailed eyes, atmospheric lighting"
                    .into(),
            ),
        },
        StylePreset {
            id: "cartoon".into(),
            name: "🎨 卡通".into(),
            description: "Pixar/迪士尼风格卡通，鲜艳色彩，圆润造型，亲切可爱".into(),
            style_tags: vec!["cartoon".into(), "vibrant".into()],
            seedance_style_keyword: Some(
                "3D Pixar-like rounded character design, soft volumetric lighting, \
                 vibrant saturated colors, smooth subsurface scattering, child-friendly aesthetic"
                    .into(),
            ),
        },
        StylePreset {
            id: "clay".into(),
            name: "🌈 3D 黏土".into(),
            description: "黏土材质 3D 渲染，柔软、可爱、有手工质感".into(),
            style_tags: vec!["3d_render".into(), "claymation".into()],
            seedance_style_keyword: Some(
                "claymation 3D render, soft handcrafted texture, \
                 Aardman-like stop motion style, rounded forms, warm studio lighting, \
                 tactile material feel"
                    .into(),
            ),
        },
        StylePreset {
            id: "ink".into(),
            name: "🖌️ 水墨".into(),
            description: "中国传统水墨画风格，留白、墨韵、淡雅写意".into(),
            style_tags: vec!["ink_wash".into(), "minimalist".into()],
            seedance_style_keyword: Some(
                "traditional Chinese ink wash painting, sumi-e style, monochrome ink with \
                 single color accent, generous negative space, brush stroke texture, \
                 minimalist composition, philosophical mood"
                    .into(),
            ),
        },
        StylePreset {
            id: "line-drawing".into(),
            name: "✏️ 简笔画".into(),
            description: "黑白线条简笔画，简洁干净，童趣手绘".into(),
            style_tags: vec!["line_art".into(), "minimalist".into()],
            seedance_style_keyword: Some(
                "black and white line drawing, clean ink outlines, child-friendly doodle style, \
                 pure white background, no shading, no color fills, hand-drawn marker aesthetic"
                    .into(),
            ),
        },
        StylePreset {
            id: "photo".into(),
            name: "📷 写实摄影".into(),
            description: "真实摄影质感，自然光，浅景深".into(),
            style_tags: vec!["photographic".into(), "realistic".into()],
            seedance_style_keyword: Some(
                "photorealistic cinematic photography, 85mm portrait lens, shallow depth of field, \
                 natural daylight, soft skin tones, subtle film grain, Rec.709 color science"
                    .into(),
            ),
        },
        StylePreset {
            id: "pixel".into(),
            name: "📺 像素".into(),
            description: "复古 8-bit 像素艺术，颗粒感、轮廓清晰、鲜艳".into(),
            style_tags: vec!["pixel_art".into(), "retro".into()],
            seedance_style_keyword: Some(
                "8-bit pixel art, retro game sprite aesthetic, limited NES palette, \
                 crisp pixel outlines, 320x240 resolution feel, no anti-aliasing, \
                 blocky chunky shading"
                    .into(),
            ),
        },
    ]
}

/// 构造带风格上下文的 system_prompt
/// 风格描述作为独立段落追加；与 character 块并行不冲突
pub fn build_system_prompt_with_style(base: &str, style: Option<&StylePreset>) -> String {
    match style {
        Some(s) => format!(
            "{}\n\n[当前风格]\n名称: {}\n视觉描述: {}\n（请在生成图片 / 视频时保持该视觉风格一致）",
            base, s.name, s.description,
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

/// W4.6 #3: 构造"1 张 2x2 四宫格场景卡"的 image-01 prompt.
///
/// 工业版场景卡 (来自分镜指令-生图提示词-场景生图+描述后缀.txt 调研):
/// - 4 视角, 同一空间布局, 同一光线方向, 同一地面/材质
/// - 高度 1.6m, 焦距 35mm, 全景
/// - 4 视角:
///   - 左上: 正面立面正视全景
///   - 右上: 反打视角 (180° 翻转)
///   - 左下: 左侧立面 (纵向, 占画面 70%+)
///   - 右下: 右侧立面 (纵向, 占画面 70%+)
/// - 风格走 Seedance keyword + 工业版"地面/材质/光线方向一致"
/// - 输出字段化 (sceneSpec) 留给后续 stages 处理 (W4.6 #4 用)
pub fn build_multiview_scene_prompt(
    scene_name: &str,
    setting: &str,
    time_of_day: &str,
    weather: &str,
    seedance_style_keyword: &str,
) -> String {
    format!(
        "[W4.6 scene card] one image, 2x2 grid, 4 views of the same space.\n\
         [Space] {scene}, {setting}, time of day: {time}, weather: {weather}\n\
         [Camera] 1.6m height, 35mm focal length, full panoramic view, consistent 4 angles\n\
         [4 views]\n\
           top-left: front elevation (main entrance view, full body)\n\
           top-right: reverse angle (180° flip of front)\n\
           bottom-left: left side elevation (vertical, 70%+ frame height)\n\
           bottom-right: right side elevation (vertical, 70%+ frame height)\n\
         [Constraints] floor material identical, light direction consistent, no human figures, \
         no modern objects (cars / electronics unless scene implies them)\n\
         [Style] {style_kw}\n\
         [sceneSpec output required] environment_type: \"\", time_of_day: \"\", mood: \"\", \
         main_features: [], space_state: \"\", visible_objects: [] — append this as a JSON object in your tool call args prompt field so downstream stages can use it",
        scene = scene_name,
        setting = setting,
        time = time_of_day,
        weather = weather,
        style_kw = seedance_style_keyword,
    )
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
            seedance_style_keyword: None, // 测试 fixture 不必设
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
        assert!(
            prompt.contains("在花园里玩"),
            "original preserved: {}",
            prompt
        );
        assert!(
            prompt.contains("中国传统水墨画风格"),
            "style description injected: {}",
            prompt
        );
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

    // ─── W4.6 #3 2x2 场景卡 + seedance_style_keyword ────

    /// 2x2 四宫格场景卡 prompt 包含 4 视角 + 1.6m/35mm 工业参数 + sceneSpec 输出约束
    #[test]
    fn build_multiview_scene_prompt_has_4_views_and_industrial_params() {
        let p = build_multiview_scene_prompt(
            "森林",
            "夏季下午阳光穿过树梢",
            "afternoon",
            "sunny",
            "Studio-Ghibli inspired soft watercolor",
        );
        assert!(p.contains("W4.6 scene card"), "got: {p}");
        assert!(p.contains("[Space] 森林"), "got: {p}");
        assert!(p.contains("夏季下午阳光穿过树梢"), "got: {p}");
        assert!(p.contains("time of day: afternoon"), "got: {p}");
        assert!(p.contains("weather: sunny"), "got: {p}");
        // 工业参数
        assert!(p.contains("1.6m height"), "got: {p}");
        assert!(p.contains("35mm focal length"), "got: {p}");
        // 4 视角
        assert!(p.contains("front elevation"), "got: {p}");
        assert!(p.contains("reverse angle"), "got: {p}");
        assert!(p.contains("left side elevation"), "got: {p}");
        assert!(p.contains("right side elevation"), "got: {p}");
        // 一致性约束
        assert!(p.contains("floor material identical"), "got: {p}");
        assert!(p.contains("light direction consistent"), "got: {p}");
        assert!(p.contains("no human figures"), "got: {p}");
        assert!(p.contains("no modern objects"), "got: {p}");
        // sceneSpec 输出字段化 (W4.6 #4 用)
        assert!(p.contains("sceneSpec"), "got: {p}");
        assert!(p.contains("environment_type"), "got: {p}");
        assert!(p.contains("visible_objects"), "got: {p}");
        // 风格注入
        assert!(p.contains("[Style] Studio-Ghibli"), "got: {p}");
    }

    /// 7 个内置风格都应有 seedance_style_keyword (不能 fallback)
    #[test]
    fn builtin_styles_all_have_seedance_style_keyword() {
        let styles = builtin_styles();
        assert_eq!(styles.len(), 7);
        for s in &styles {
            let kw = s
                .seedance_style_keyword
                .as_ref()
                .unwrap_or_else(|| panic!("style {} 缺 seedance_style_keyword", s.id));
            assert!(!kw.is_empty(), "{} keyword 不能空", s.id);
            // 不应出现 ARRI / IRE / Hue 这种工业黑话 (儿童化降级)
            assert!(
                !kw.contains("ARRI") && !kw.contains("IRE") && !kw.contains("Hue"),
                "{} keyword 不应有工业黑话: {kw}",
                s.id
            );
        }
    }

    /// seedance_style_keyword 字段能正确反序列化 (向前向后兼容)
    #[test]
    fn seedance_style_keyword_serializes_as_optional_field() {
        let s = sample();
        // description + id + 都不含 seedance_style_keyword (set to None)
        let json = serde_json::to_string(&s).unwrap();
        // skip_serializing_if = "Option::is_none" → 序列化为空时跳过
        assert!(
            !json.contains("seedance_style_keyword"),
            "None 时应被跳过 (compat 老 client): {json}"
        );

        // 反序列化: 老 client 发的 JSON 没这字段 → None
        let old_json = r#"{"id":"ink","name":"水墨","description":"中国传统水墨画风格","style_tags":["ink_wash"]}"#;
        let parsed: StylePreset = serde_json::from_str(old_json).unwrap();
        assert!(
            parsed.seedance_style_keyword.is_none(),
            "老 JSON 无字段应反序列化为 None"
        );

        // 新 JSON 有字段
        let new_json = r#"{"id":"ink","name":"水墨","description":"X","style_tags":["a"],"seedance_style_keyword":"sumi-e style"}"#;
        let parsed2: StylePreset = serde_json::from_str(new_json).unwrap();
        assert_eq!(
            parsed2.seedance_style_keyword.as_deref(),
            Some("sumi-e style")
        );
    }

    /// 7 个 builtin 风格对应的 seedance keyword 应该差异化 (防止重复)
    #[test]
    fn builtin_styles_have_unique_seedance_keywords() {
        let styles = builtin_styles();
        let mut kws: Vec<String> = styles
            .iter()
            .map(|s| s.seedance_style_keyword.clone().unwrap())
            .collect();
        kws.sort();
        let original_len = kws.len();
        kws.dedup();
        assert_eq!(
            kws.len(),
            original_len,
            "7 个 builtin 风格的 seedance_style_keyword 必须各不相同, 防混淆"
        );
    }

    /// 验证 inject_style_into_video_args 老行为不变 (向后兼容)
    #[test]
    fn inject_style_video_path_unchanged_after_w46_field_added() {
        let s = sample();
        // 老 path: 仅追加 description 到 motion
        let args = r#"{"motion":"跳跃"}"#;
        let out = inject_style_into_video_args(args, &s);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let motion = v["motion"].as_str().unwrap();
        assert!(motion.contains("跳跃"));
        assert!(motion.contains("中国传统水墨画风格"));
    }
}
