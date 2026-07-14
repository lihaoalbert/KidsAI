// kernel/lesson_templates.rs — Day 11-12: L1-L7 转 skill 提示词模板
//
// 之前: 7 关卡 (L1-L7) 是 levelStore 里的独立数据, 有 AgentRunnerPage 入口.
// 现在: 这 7 个关卡变成 video-director skill 的"提示词模板", 走 skill 的
//       StoryArcTemplates 注入 director Stage 1 (故事卡选择).
//
// 红线 8: 删 L1-L7 必须立刻补 skill 钩子 — 这文件就是钩子.
//         9 岁打开 director 看到 7 张模板卡 → 选哪个就从哪个开始.
//         16+/成人/创作者/pro 看到同样的 7 张卡 (因为 video-director
//         是 audience=both).

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonTemplate {
    pub id: String,
    pub title: String,
    pub emoji: String,
    pub description: String,
    pub target_age: String,
    pub core: String,
    pub spine_skeleton: serde_json::Value,
    pub scene_prompts: Vec<String>,
    pub credits_per_use: u32,
    pub difficulty: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonTemplateBundle {
    #[serde(default)]
    pub templates: Vec<LessonTemplate>,
}

pub fn load_lesson_templates(path: &Path) -> Result<Vec<LessonTemplate>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read: {e}"))?;
    let bundle: LessonTemplateBundle = serde_json::from_slice(&bytes)
        .map_err(|e| format!("parse: {e}"))?;
    Ok(bundle.templates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn template_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("seed_skills")
            .join("video-director")
            .join("lesson-templates")
            .join("l1-l7.json")
    }

    #[test]
    fn load_seven_templates() {
        // 红线 8: 必须至少有 7 个模板, 接住原 L1-L7 流量
        let t = load_lesson_templates(&template_path()).expect("load");
        assert_eq!(t.len(), 7, "应该是 7 个模板, 实际 {}", t.len());
        let ids: Vec<_> = t.iter().map(|x| x.id.clone()).collect();
        for i in 1..=7 {
            assert!(
                ids.contains(&format!("L{i}")),
                "缺少 L{i}: ids={ids:?}"
            );
        }
    }

    #[test]
    fn all_templates_have_emoji_and_spine() {
        let t = load_lesson_templates(&template_path()).expect("load");
        for tmpl in &t {
            assert!(!tmpl.emoji.is_empty(), "{} 缺 emoji", tmpl.id);
            assert!(!tmpl.title.is_empty(), "{} 缺 title", tmpl.id);
            assert!(!tmpl.scene_prompts.is_empty(), "{} 缺场景提示", tmpl.id);
            assert!(
                tmpl.spine_skeleton.is_object(),
                "{} spine_skeleton 必须是 object",
                tmpl.id
            );
        }
    }

    #[test]
    fn difficulty_progression() {
        let t = load_lesson_templates(&template_path()).expect("load");
        // 难度必须递增 (L1=1, L7=3), 不能 L7 比 L1 简单
        let l1 = t.iter().find(|x| x.id == "L1").unwrap();
        let l7 = t.iter().find(|x| x.id == "L7").unwrap();
        assert!(l1.difficulty <= l7.difficulty);
    }

    #[test]
    fn credits_per_use_positive() {
        let t = load_lesson_templates(&template_path()).expect("load");
        for tmpl in &t {
            assert!(tmpl.credits_per_use > 0, "{} 学币必须 > 0", tmpl.id);
        }
    }

    #[test]
    fn target_age_covers_kid_and_teen() {
        // 红线 8: 模板必须覆盖 8-10 / 10-13 / 14-16 三档, 不能让任何年龄无模板
        let t = load_lesson_templates(&template_path()).expect("load");
        let ages: Vec<_> = t.iter().map(|x| x.target_age.clone()).collect();
        assert!(ages.contains(&"8-10".to_string()), "需要 8-10 模板");
        assert!(ages.contains(&"10-13".to_string()), "需要 10-13 模板");
        assert!(ages.contains(&"14-16".to_string()), "需要 14-16 模板");
    }

    #[test]
    fn red_line_8_bundle_present() {
        // 红线 8 关联: 这文件存在本身就是"补 skill 钩子"的证据
        assert!(template_path().exists());
    }
}