// kernel/seed_skills.rs — 内核自带的种子 skill 加载
//
// Day 9-10: 第一刀 — 把 video-director 的 manifest + prompts 从源码
// (hardcode) 解耦成 seed_skill 目录. 运行时由 SeedSkillLoader 加载.
//
// 不动现有 skill 安装/卸载链路, 不引入 marketplace. 这是"内置"的 path.

use crate::skills::{Audience, SkillManifestFull};
use serde_json::Value;
use std::path::Path;

/// 加载指定目录下的 skill manifest, 返回解析结果.
pub fn load_seed_manifest(manifest_path: &Path) -> Result<SkillManifestFull, String> {
    let bytes = std::fs::read(manifest_path).map_err(|e| format!("read manifest: {e}"))?;
    serde_json::from_slice::<SkillManifestFull>(&bytes)
        .map_err(|e| format!("parse manifest: {e}"))
}

/// 校验 seed manifest 必须包含 "stages" 字段 (Day 9 第一刀的核心约定),
/// 且每个 stage 必须有 requires_confirm=true (红线 7 强制).
pub fn validate_video_director_red_line(manifest: &SkillManifestFull) -> Result<(), String> {
    // manifest 本身必须有 "stages" 字段, 但 SkillManifestFull schema 当前没声明.
    // 我们用 raw json 再校验一次.
    let raw: Value = serde_json::to_value(manifest).map_err(|e| format!("re-serialize: {e}"))?;
    let stages = raw
        .get("stages")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "stages 字段缺失 — 红线 7 无法验证".to_string())?;
    if stages.is_empty() {
        return Err("stages 不能为空".into());
    }
    for (i, stage) in stages.iter().enumerate() {
        let id = stage
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("(unknown)");
        let requires_confirm = stage
            .get("requires_confirm")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !requires_confirm {
            return Err(format!(
                "stage[{i}] ({id}) requires_confirm=false — 红线 7: 每阶段必须用户确认"
            ));
        }
    }
    Ok(())
}

/// audience 校验 (沿用 W10 audience_matches 语义).
pub fn audience_ok(skill: &Audience, mode: &Audience) -> bool {
    use Audience::*;
    matches!(
        (skill, mode),
        (Both, _) | (_, Both) | (Child, Child) | (Adult, Adult)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn video_director_manifest_path() -> PathBuf {
        // seed_skills 位于 src-tauri/seed_skills/video-director/manifest.json
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("seed_skills")
            .join("video-director")
            .join("manifest.json");
        manifest
    }

    #[test]
    fn video_director_manifest_loads() {
        let path = video_director_manifest_path();
        assert!(path.exists(), "manifest 必须在 {}", path.display());
        let m = load_seed_manifest(&path).expect("manifest 解析");
        assert_eq!(m.id, "video-director");
        assert_eq!(m.audience, Audience::Both);
        assert_eq!(m.extends.tools.len(), 4);
    }

    #[test]
    fn video_director_red_line_passes() {
        // 红线 7: 6 阶段全部 requires_confirm=true
        let path = video_director_manifest_path();
        let m = load_seed_manifest(&path).expect("manifest 解析");
        validate_video_director_red_line(&m).expect("红线 7 校验");
    }

    #[test]
    fn video_director_audience_both_matches_either_mode() {
        // 既能在 child 也能在 adult 模式挂载 (红线 8 关联)
        let m = load_seed_manifest(&video_director_manifest_path()).unwrap();
        assert!(audience_ok(&m.audience, &Audience::Child));
        assert!(audience_ok(&m.audience, &Audience::Adult));
    }

    #[test]
    fn red_line_violation_detected() {
        // 构造一个 requires_confirm=false 的假 manifest, 验证红线 7 拦截
        let raw = r#"{
            "schema": "kidsai.skill/1",
            "id": "evil-skill",
            "name": "bad",
            "version": "v1",
            "publisher": "x",
            "min_app_version": "0.4.0",
            "age_tier": [],
            "category": "x",
            "audience": "both",
            "assets": [],
            "prompts": [],
            "templates": {"characters": [], "story_arcs": []},
            "extends": {"tabs": [], "tools": [], "characters_inject_into": null},
            "credits_per_use": 0,
            "daily_quota": 0,
            "size_bytes": 0,
            "publisher_signature": "x",
            "publisher_pubkey_id": "x",
            "stages": [
                {"id": "stage-1", "name": "x", "ui": "x", "requires_confirm": false}
            ]
        }"#;
        let m: SkillManifestFull = serde_json::from_str(raw).unwrap();
        let result = validate_video_director_red_line(&m);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("红线 7"), "错误信息应该提到红线 7: {err}");
    }

    #[test]
    fn red_line_no_stages_fails() {
        let raw = r#"{
            "schema": "kidsai.skill/1",
            "id": "empty-skill",
            "name": "x",
            "version": "v1",
            "publisher": "x",
            "min_app_version": "0.4.0",
            "age_tier": [],
            "category": "x",
            "audience": "both",
            "assets": [],
            "prompts": [],
            "templates": {"characters": [], "story_arcs": []},
            "extends": {"tabs": [], "tools": [], "characters_inject_into": null},
            "credits_per_use": 0,
            "daily_quota": 0,
            "size_bytes": 0,
            "publisher_signature": "x",
            "publisher_pubkey_id": "x"
        }"#;
        let m: SkillManifestFull = serde_json::from_str(raw).unwrap();
        assert!(validate_video_director_red_line(&m).is_err());
    }

    #[test]
    fn audience_matching_table() {
        use Audience::*;
        assert!(audience_ok(&Both, &Child));
        assert!(audience_ok(&Both, &Adult));
        assert!(audience_ok(&Child, &Child));
        assert!(audience_ok(&Adult, &Adult));
        assert!(!audience_ok(&Child, &Adult)); // 红线 8: 儿童 skill 不在成人模式
        assert!(!audience_ok(&Adult, &Child)); // 红线 8: 成人 skill 不在儿童模式
    }
}