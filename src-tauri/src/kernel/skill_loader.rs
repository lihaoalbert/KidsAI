// kernel/skill_loader.rs — 内核侧的 skill loader
//
// W10 已经实现了 `skills_runtime::mount_enabled_skills` (按 audience 过滤).
// 内核层再加薄包装:
//   1. 接收当前 mode + 已装 skill manifest
//   2. 调 mount_enabled_skills 拿到 MountedSkill 列表
//   3. **广播 SkillMounted / SkillUnmounted 事件** (EventBus)
//      - 第一次 mount 时 broadcast SkillMounted
//      - mode 切换导致某些 skill 被过滤掉时 broadcast SkillUnmounted
//   4. UI 扩展点声明 (extends.tabs + characters_inject_into)
//      透传到 frontend, frontend 决定渲染
//
// 红线 8 关联: 删除 L1-L7 后, 必须有 skill 钩子接住. 这个 loader 是 skill 钩子的来源.

use crate::kernel::event_bus::{EventBus, KernelEvent};
use crate::skills::Audience;
use crate::skills::SkillManifestFull;
use crate::skills_runtime::{mount_enabled_skills, MountedSkill};
use std::collections::HashSet;
use std::sync::Mutex;

/// 内核侧的 skill loader. 缓存"当前已挂载"快照, 切换 mode 时 diff 出 mount/unmount.
pub struct KernelSkillLoader {
    event_bus: EventBus,
    last_mounted: Mutex<HashSet<String>>,
}

impl KernelSkillLoader {
    pub fn new(event_bus: EventBus) -> Self {
        Self {
            event_bus,
            last_mounted: Mutex::new(HashSet::new()),
        }
    }

    /// 给定已装 skill 列表 + 当前 mode, 计算 mount 集合, 广播变更事件.
    /// 返回当前应挂载的 MountedSkill 列表.
    pub fn mount_for_mode(
        &self,
        manifests: &[SkillManifestFull],
        mode: Audience,
    ) -> Vec<MountedSkill> {
        let mounted = mount_enabled_skills(manifests, mode);
        let now_ids: HashSet<String> =
            mounted.iter().map(|m| m.skill_id.clone()).collect();

        let mut last = self.last_mounted.lock().expect("skill loader lock");
        // 新挂载的 → broadcast SkillMounted
        for id in now_ids.difference(&last) {
            self.event_bus.publish(KernelEvent::SkillMounted {
                skill_id: id.clone(),
            });
        }
        // 卸载的 → broadcast SkillUnmounted
        for id in last.difference(&now_ids) {
            self.event_bus.publish(KernelEvent::SkillUnmounted {
                skill_id: id.clone(),
            });
        }
        *last = now_ids;
        mounted
    }

    /// 强制清空缓存 (用于重启或测试).
    pub fn reset(&self) {
        self.last_mounted.lock().expect("skill loader lock").clear();
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
            category: "lang".into(),
            audience: aud,
            assets: vec![SkillFile {
                path: "a".into(),
                sha256: "x".into(),
                size: 1,
            }],
            prompts: vec![SkillPromptRef {
                id: "p".into(),
                file: "p.yaml".into(),
                sha256: "y".into(),
            }],
            templates: SkillTemplates {
                characters: vec![SkillTemplate {
                    id: "c".into(),
                    name: "C".into(),
                    default_form_image: None,
                }],
                story_arcs: vec![],
            },
            extends: SkillExtends {
                tabs: vec!["narrative".into()],
                tools: vec!["t".into()],
                characters_inject_into: None,
            },
            credits_per_use: 1,
            daily_quota: 1,
            homepage: None,
            size_bytes: 1,
            publisher_signature: "s".into(),
            publisher_pubkey_id: "kidsai-dev".into(),
            stages: vec![],
        }
    }

    #[test]
    fn first_mount_broadcasts_mounted_event() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        let loader = KernelSkillLoader::new(bus.clone());
        let _ = loader.mount_for_mode(
            &[fixture("video-director", Audience::Both)],
            Audience::Child,
        );
        let ev_arc = rx.try_recv().expect("event");
        match &*ev_arc {
            KernelEvent::SkillMounted { skill_id } => {
                assert_eq!(skill_id, "video-director");
            }
            other => panic!("expected SkillMounted, got {other:?}"),
        }
    }

    #[test]
    fn mode_switch_unmounts_child_skill_in_adult() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        let loader = KernelSkillLoader::new(bus.clone());
        let manifests = vec![
            fixture("video-director", Audience::Both),
            fixture("lesson-children", Audience::Child),
            fixture("commercial-ad", Audience::Adult),
        ];
        // Child 模式
        let child = loader.mount_for_mode(&manifests, Audience::Child);
        let child_ids: Vec<_> = child.iter().map(|m| m.skill_id.clone()).collect();
        assert!(child_ids.contains(&"video-director".to_string()));
        assert!(child_ids.contains(&"lesson-children".to_string()));
        assert!(!child_ids.contains(&"commercial-ad".to_string()));
        // 收 child 模式的 mount 事件
        for _ in 0..4 {
            let _ = rx.try_recv();
        }
        // 切到 Adult 模式
        let adult = loader.mount_for_mode(&manifests, Audience::Adult);
        let adult_ids: Vec<_> = adult.iter().map(|m| m.skill_id.clone()).collect();
        assert!(adult_ids.contains(&"video-director".to_string()));
        assert!(adult_ids.contains(&"commercial-ad".to_string()));
        // 红线 8 关联: lesson-children 必须被卸载 (否则 16+ 看到儿童内容)
        assert!(!adult_ids.contains(&"lesson-children".to_string()));
        // 收到 lesson-children 的 SkillUnmounted 事件
        let mut found_unmount = false;
        for _ in 0..6 {
            match rx.try_recv() {
                Ok(ev_arc) => {
                    if let KernelEvent::SkillUnmounted { skill_id } = &*ev_arc {
                        if skill_id == "lesson-children" {
                            found_unmount = true;
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }
        assert!(found_unmount, "lesson-children 应该被卸载");
    }

    #[test]
    fn same_mode_no_redundant_events() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();
        let loader = KernelSkillLoader::new(bus.clone());
        let manifests = vec![fixture("video-director", Audience::Both)];
        loader.mount_for_mode(&manifests, Audience::Child);
        for _ in 0..4 {
            let _ = rx.try_recv();
        }
        // 再次 mount 同样的 → 不应发新事件
        loader.mount_for_mode(&manifests, Audience::Child);
        let res = rx.try_recv();
        assert!(res.is_err(), "重复 mount 不应发新事件");
    }

    #[test]
    fn reset_clears_state() {
        let bus = EventBus::new();
        let loader = KernelSkillLoader::new(bus.clone());
        let manifests = vec![fixture("a", Audience::Both)];
        loader.mount_for_mode(&manifests, Audience::Child);
        loader.reset();
        // reset 后再 mount 应重新发事件 (last 为空)
        let mut rx = bus.subscribe();
        loader.mount_for_mode(&manifests, Audience::Child);
        let ev_arc = rx.try_recv().expect("event");
        assert!(matches!(*ev_arc, KernelEvent::SkillMounted { .. }));
    }
}