// W10 Day 5 — Skill Runtime
//
// 把已装 / 启用的 skill "挂载" 到 agent runtime:
//   1. 拼 system_prompt 末尾加 "[Skill:id] 提示词片段"
//   2. 工具列表追加 skill.extends.tools 声明的 tools (后续 Day 5+ 真接)
//   3. 角色 + 剧本模板注入 directorStore (前端拿到后追加)
//
// 设计: mount_skill 是纯函数 (skill manifest → 输出), 不持有 state.
// agent.rs 调用 mount_enabled_skills → 把追加的 system_prompt + characters 返回给调用方.
//
// 注意: 当前 Rust 端只能"看到" SkillManifestFull; 完整 assets (image bytes)
// 需要前端通过 convertFileSrc 加载. 此处只返回 path 让前端自己拉.
//
// 范围 (Day 5): mount_skill + 6 种子 skill manifest; directorStore 集成留 Day 5 收尾.

use serde::{Deserialize, Serialize};

use crate::skills::{Audience, SkillManifestFull};

/// mount_skill 输出 — agent.rs 拿到后追加到自己的 system prompt / tools / characters.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MountedSkill {
    pub skill_id: String,
    pub name: String,
    /// 拼到 system_prompt 末尾的额外片段 (含 [Skill:id] 标记)
    pub system_prompt_addendum: String,
    /// 注入 directorStore.characterMetas 的角色列表
    pub character_templates: Vec<MountedCharacter>,
    /// 注入 directorStore.storyArcTemplates 的剧本骨架
    pub story_arcs: Vec<serde_json::Value>,
    /// skill.tools 声明的工具 id (Day 6+ 接 tool adapter)
    pub tools: Vec<String>,
    pub audience: Audience,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountedCharacter {
    pub id: String,
    pub name: String,
    pub default_form_image: Option<String>,
    pub source_skill: String,
}

/// 给定 skill manifest → 输出 mount 结果.
/// mount 一次针对一个 skill; agent.rs 多次调用聚合所有 enabled skill.
pub fn mount_skill(manifest: &SkillManifestFull) -> MountedSkill {
    let mut addendum = String::new();
    addendum.push_str(&format!(
        "\n\n[Skill:{}] (audience={:?}, version={})\n",
        manifest.id, manifest.audience, manifest.version
    ));
    addendum.push_str(&format!(
        "skill name: {}\ncategory: {}\n\n",
        manifest.name, manifest.category
    ));

    // 把 prompt 文件作为 hint 列出 (实际 yaml 内容由前端通过 blob 端点拉)
    if !manifest.prompts.is_empty() {
        addendum.push_str("prompt fragments (按需 inject):\n");
        for p in &manifest.prompts {
            addendum.push_str(&format!("  - id={} file={}\n", p.id, p.file));
        }
    }

    // 工具声明
    let tools: Vec<String> = manifest.extends.tools.clone();

    // 角色模板
    let character_templates: Vec<MountedCharacter> = manifest
        .templates
        .characters
        .iter()
        .map(|t| MountedCharacter {
            id: t.id.clone(),
            name: t.name.clone(),
            default_form_image: t.default_form_image.clone(),
            source_skill: manifest.id.clone(),
        })
        .collect();

    MountedSkill {
        skill_id: manifest.id.clone(),
        name: manifest.name.clone(),
        system_prompt_addendum: addendum,
        character_templates,
        story_arcs: manifest.templates.story_arcs.clone(),
        tools,
        audience: manifest.audience.clone(),
    }
}

/// 聚合多个 mount_skill 结果, 按 mode 过滤 audience:
///   - child mode → 只挂 child / both 的 skill
///   - adult mode → 只挂 adult / both 的 skill (child skill 在成人模式隐藏)
pub fn mount_enabled_skills(
    manifests: &[SkillManifestFull],
    audience_filter: Audience,
) -> Vec<MountedSkill> {
    manifests
        .iter()
        .filter(|m| audience_matches(&m.audience, &audience_filter))
        .map(mount_skill)
        .collect()
}

fn audience_matches(skill_aud: &Audience, mode: &Audience) -> bool {
    use Audience::*;
    match (skill_aud, mode) {
        (Both, _) | (_, Both) => true, // both 通透
        (Child, Child) => true,
        (Adult, Adult) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{SkillExtends, SkillFile, SkillPromptRef, SkillTemplate, SkillTemplates};

    fn fixture(id: &str, aud: Audience) -> SkillManifestFull {
        SkillManifestFull {
            schema: "kidsai.skill/1".into(),
            id: id.into(),
            name: format!("Test {id}"),
            version: "v1".into(),
            publisher: "kidsai-official".into(),
            min_app_version: "0.4.0".into(),
            age_tier: vec![1, 2],
            category: "language".into(),
            audience: aud,
            assets: vec![SkillFile {
                path: "assets/cover.png".into(),
                sha256: "abc".into(),
                size: 100,
            }],
            prompts: vec![SkillPromptRef {
                id: "opening".into(),
                file: "prompts/opening.yaml".into(),
                sha256: "def".into(),
            }],
            templates: SkillTemplates {
                characters: vec![SkillTemplate {
                    id: "explorer".into(),
                    name: "小探险家".into(),
                    default_form_image: Some("assets/cover.png".into()),
                }],
                story_arcs: vec![],
            },
            extends: SkillExtends {
                tabs: vec!["narrative".into()],
                tools: vec!["translate".into()],
                characters_inject_into: Some("directorStore.characterMetas".into()),
            },
            credits_per_use: 3,
            daily_quota: 5,
            homepage: None,
            size_bytes: 1000,
            publisher_signature: "sig".into(),
            publisher_pubkey_id: "kidsai-dev".into(),
            stages: vec![],
        }
    }

    #[test]
    fn mount_skill_emits_system_prompt_with_marker() {
        let m = fixture("eng-adventure", Audience::Child);
        let r = mount_skill(&m);
        assert!(r.system_prompt_addendum.contains("[Skill:eng-adventure]"));
        assert!(r.system_prompt_addendum.contains("audience=Child"));
        assert!(r.system_prompt_addendum.contains("prompts/opening.yaml"));
    }

    #[test]
    fn mount_skill_includes_character_templates() {
        let m = fixture("eng-adventure", Audience::Child);
        let r = mount_skill(&m);
        assert_eq!(r.character_templates.len(), 1);
        assert_eq!(r.character_templates[0].id, "explorer");
        assert_eq!(r.character_templates[0].source_skill, "eng-adventure");
    }

    #[test]
    fn mount_skill_includes_tools() {
        let m = fixture("eng-adventure", Audience::Child);
        let r = mount_skill(&m);
        assert_eq!(r.tools, vec!["translate".to_string()]);
    }

    #[test]
    fn mount_enabled_skills_filters_by_child_mode() {
        let manifests = vec![
            fixture("eng-adventure", Audience::Child),
            fixture("ink-painting", Audience::Child),
            fixture("commercial-ad", Audience::Adult),
        ];
        let r = mount_enabled_skills(&manifests, Audience::Child);
        assert_eq!(r.len(), 2);
        assert!(r.iter().any(|m| m.skill_id == "eng-adventure"));
        assert!(r.iter().any(|m| m.skill_id == "ink-painting"));
        assert!(!r.iter().any(|m| m.skill_id == "commercial-ad"));
    }

    #[test]
    fn mount_enabled_skills_filters_by_adult_mode() {
        let manifests = vec![
            fixture("eng-adventure", Audience::Child),
            fixture("commercial-ad", Audience::Adult),
            fixture("ink-painting", Audience::Both),
        ];
        let r = mount_enabled_skills(&manifests, Audience::Adult);
        assert_eq!(r.len(), 2);
        assert!(r.iter().any(|m| m.skill_id == "commercial-ad"));
        assert!(r.iter().any(|m| m.skill_id == "ink-painting"));
        assert!(!r.iter().any(|m| m.skill_id == "eng-adventure"));
    }

    #[test]
    fn both_audience_appears_in_either_mode() {
        let manifests = vec![fixture("ink-painting", Audience::Both)];
        let r_child = mount_enabled_skills(&manifests, Audience::Child);
        let r_adult = mount_enabled_skills(&manifests, Audience::Adult);
        assert_eq!(r_child.len(), 1);
        assert_eq!(r_adult.len(), 1);
    }
}