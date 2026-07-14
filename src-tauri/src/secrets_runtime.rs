// W11 Day 7 — SecretsRuntime (运行时查询 API + 双源 fallback)
//
// 调用方 (agent.rs / prompt_builder.rs / safety.rs / character.rs / style.rs):
//   secrets_runtime::global().get("system/director.yaml").await
//
// 优先级:
//   1. 当前 profile 内存里的 decrypted plaintext (按文件 path)
//   2. 源码 hardcode fallback (FallbackPrompts)
//
// 模式切换: set_user_mode(UserMode::Adult) → 下次 get() 走 adult profile.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::license_store::UserMode;
use crate::secrets_store::SecretsStore;

/// 全局单例 wrapper (跟 LicenseSigner 一样, 用 Mutex<Option<>> 支持 test 替换)
static INSTANCE: std::sync::Mutex<Option<Arc<SecretsRuntime>>> = std::sync::Mutex::new(None);

#[derive(Debug, Clone)]
pub struct SecretsRuntime {
    inner: Arc<RwLock<SecretsState>>,
}

impl Default for SecretsRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
pub struct SecretsState {
    /// profile ("child"/"adult") → { file_path → plaintext bytes }
    pub by_profile: HashMap<String, HashMap<String, Vec<u8>>>,
    /// 当前 user mode (决定 get() 走哪个 profile)
    pub current_mode: UserMode,
}

impl SecretsRuntime {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(SecretsState {
                current_mode: UserMode::Child,
                ..Default::default()
            })),
        }
    }

    /// 启动期: install 一个 profile 的所有文件 (path, bytes).
    pub async fn install_profile_files(
        &self,
        profile: &str,
        files: Vec<(String, Vec<u8>)>,
    ) {
        let mut st = self.inner.write().await;
        let entry = st.by_profile.entry(profile.to_string()).or_default();
        for (p, b) in files {
            entry.insert(p, b);
        }
    }

    /// 模式切换: set_user_mode(Adult) 时调用 → 下次 get() 路由到 adult.
    pub async fn set_mode(&self, mode: UserMode) {
        let mut st = self.inner.write().await;
        st.current_mode = mode;
    }

    pub async fn mode(&self) -> UserMode {
        self.inner.read().await.current_mode
    }

    /// 按当前 mode 路由: get("x.yaml") → current_mode 是 Child → 走 "child" profile.
    pub async fn get(&self, path: &str) -> Option<Vec<u8>> {
        let st = self.inner.read().await;
        let profile = match st.current_mode {
            UserMode::Child => "child",
            UserMode::Adult => "adult",
        };
        st.by_profile
            .get(profile)
            .and_then(|m| m.get(path))
            .cloned()
    }

    /// 显式按 profile 查 (admin / 调试用).
    pub async fn get_for_profile(&self, profile: &str, path: &str) -> Option<Vec<u8>> {
        let st = self.inner.read().await;
        st.by_profile
            .get(profile)
            .and_then(|m| m.get(path))
            .cloned()
    }

    pub async fn current_versions(&self) -> HashMap<String, String> {
        let st = self.inner.read().await;
        st.by_profile
            .keys()
            .map(|p| (p.clone(), "in-memory".to_string()))
            .collect()
    }

    /// 从 SecretsStore 重新 load 所有已装 profile 的所有文件.
    /// bootstrap 时调用; 失败 (wrapped 不再可解 / 签名失败) 不影响 fallback 路径.
    pub async fn reload_from_store(&self, store: &SecretsStore) -> Result<(), String> {
        let cur = store.load_current().map_err(|e| format!("load_current: {e}"))?;
        for (profile, version) in &cur.profiles {
            // 仅做基础读 (失败 → 跳过, 走 fallback)
            let manifest = match store.read_manifest(profile, version) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let bundle_ct = match store.read_bundle(profile, version) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let wrapped = match store.read_wrapped(profile, version) {
                Ok(w) => w,
                Err(_) => continue,
            };
            // 验证 + 解密需要 license_token, 但 bootstrap 阶段可能还没 license.
            // 这里只把 bundle 原文占位 (Day 8 接 anti_tamper 后再做真解密).
            // 为保持简单: bootstrap 阶段不调 decrypt, get() 永远返 None → 走 fallback.
            // 真正使用 → 在 agent / IPC 调用时按 license_token 解密.
            let _ = (manifest, bundle_ct, wrapped); // 抑制 unused
        }
        Ok(())
    }
}

// ─────── Global singleton ───────

pub fn global() -> Arc<SecretsRuntime> {
    let mut guard = INSTANCE.lock().expect("INSTANCE poisoned");
    if guard.is_none() {
        *guard = Some(Arc::new(SecretsRuntime::new()));
    }
    guard.as_ref().cloned().unwrap()
}

pub fn set_for_test(rt: SecretsRuntime) -> Arc<SecretsRuntime> {
    let arc = Arc::new(rt);
    let mut guard = INSTANCE.lock().expect("INSTANCE poisoned");
    *guard = Some(arc.clone());
    arc
}

pub fn reset_for_test() {
    let mut guard = INSTANCE.lock().expect("INSTANCE poisoned");
    *guard = None;
}

// ─────── Fallback 表 (源码 hardcode, 兜底) ───────
//
// 设计: 当 secrets bundle 未装 / 解密失败, get("path") 返 None,
// 调用方应回退到自身源码常量. 这里集中 export fallback 字符串, 方便审计:
// "源码里仍包含这些字符串 — 但只是兜底, 不是主路径; 主路径是 secret bundle."

pub mod fallback {
    use crate::prompt_builder::{
        lighting_for_time_of_day, mood_to_motion, camera_to_seedance, NEGATIVE_PROMPT,
    };

    /// Fallback: 返回 NEGATIVE prompt 字符串.
    pub fn negative_prompt() -> &'static str {
        NEGATIVE_PROMPT
    }

    /// Fallback: 给定 time_of_day 标签 → 光影描述.
    pub fn lighting(time_of_day: &str) -> String {
        lighting_for_time_of_day(time_of_day).to_string()
    }

    /// Fallback: 给定 ShotMood 枚举 → motion 描述.
    /// 注: 调用方需自己枚举 match, 这里仅返回 hardcode 映射表.
    pub fn motion_for(calm: bool, tense: bool, joyful: bool, sad: bool, epic: bool) -> &'static str {
        match (calm, tense, joyful, sad, epic) {
            (_, true, _, _, _) => mood_to_motion(&crate::prompt_builder::ShotMood::Tense),
            (_, _, true, _, _) => mood_to_motion(&crate::prompt_builder::ShotMood::Joyful),
            (_, _, _, true, _) => mood_to_motion(&crate::prompt_builder::ShotMood::Sad),
            (_, _, _, _, true) => mood_to_motion(&crate::prompt_builder::ShotMood::Epic),
            _ => mood_to_motion(&crate::prompt_builder::ShotMood::Calm),
        }
    }

    pub fn camera_for(wide: bool, medium: bool, close: bool, extreme: bool, follow: bool, overhead: bool) -> &'static str {
        use crate::prompt_builder::ShotCamera::*;
        match (wide, medium, close, extreme, follow, overhead) {
            (true, _, _, _, _, _) => camera_to_seedance(&Wide),
            (_, true, _, _, _, _) => camera_to_seedance(&Medium),
            (_, _, true, _, _, _) => camera_to_seedance(&Close),
            (_, _, _, true, _, _) => camera_to_seedance(&Extreme),
            (_, _, _, _, true, _) => camera_to_seedance(&Follow),
            (_, _, _, _, _, true) => camera_to_seedance(&Overhead),
            _ => camera_to_seedance(&Wide), // default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license_store::UserMode;

    #[test]
    fn default_mode_is_child() {
        let rt = SecretsRuntime::new();
        let mode = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(rt.mode());
        assert_eq!(mode, UserMode::Child);
    }

    #[tokio::test]
    async fn get_returns_none_when_no_profile_installed() {
        let rt = SecretsRuntime::new();
        assert!(rt.get("system/director.yaml").await.is_none());
    }

    #[tokio::test]
    async fn install_then_get_roundtrip() {
        let rt = SecretsRuntime::new();
        rt.install_profile_files(
            "child",
            vec![(
                "system/director.yaml".to_string(),
                b"hello director".to_vec(),
            )],
        )
        .await;
        let got = rt.get("system/director.yaml").await.unwrap();
        assert_eq!(got, b"hello director");
    }

    #[tokio::test]
    async fn get_for_profile_is_explicit() {
        let rt = SecretsRuntime::new();
        rt.install_profile_files(
            "child",
            vec![("p".into(), b"child-data".to_vec())],
        )
        .await;
        rt.install_profile_files(
            "adult",
            vec![("p".into(), b"adult-data".to_vec())],
        )
        .await;
        // 默认 child → 拿 child
        let got_child = rt.get("p").await.unwrap();
        assert_eq!(got_child, b"child-data");
        // 显式拿 adult
        let got_adult = rt.get_for_profile("adult", "p").await.unwrap();
        assert_eq!(got_adult, b"adult-data");
    }

    #[tokio::test]
    async fn set_mode_routes_to_correct_profile() {
        let rt = SecretsRuntime::new();
        rt.install_profile_files(
            "child",
            vec![("p".into(), b"C".to_vec())],
        )
        .await;
        rt.install_profile_files(
            "adult",
            vec![("p".into(), b"A".to_vec())],
        )
        .await;
        // 默认 Child → "C"
        assert_eq!(rt.get("p").await.unwrap(), b"C");
        // 切到 Adult → "A"
        rt.set_mode(UserMode::Adult).await;
        assert_eq!(rt.get("p").await.unwrap(), b"A");
        // 切回 Child → "C"
        rt.set_mode(UserMode::Child).await;
        assert_eq!(rt.get("p").await.unwrap(), b"C");
    }

    #[tokio::test]
    async fn current_versions_reports_loaded_profiles() {
        let rt = SecretsRuntime::new();
        rt.install_profile_files("child", vec![("a".into(), b"1".to_vec())])
            .await;
        rt.install_profile_files("adult", vec![("b".into(), b"2".to_vec())])
            .await;
        let v = rt.current_versions().await;
        assert!(v.contains_key("child"));
        assert!(v.contains_key("adult"));
    }

    #[test]
    fn fallback_negative_prompt_matches_constant() {
        assert!(fallback::negative_prompt().contains("no camera shake"));
        assert!(fallback::negative_prompt().contains("no text"));
    }

    #[test]
    fn fallback_lighting_handles_known_and_unknown() {
        assert!(fallback::lighting("night").contains("cool blue"));
        assert!(fallback::lighting("never_heard").contains("natural soft daylight"));
    }

    #[test]
    fn fallback_motion_maps_each_mood() {
        assert!(fallback::motion_for(false, true, false, false, false).contains("deliberate"));
        assert!(fallback::motion_for(false, false, true, false, false).contains("bouncy"));
        assert!(fallback::motion_for(true, false, false, false, false).contains("gentle"));
        assert!(fallback::motion_for(false, false, false, true, false).contains("very slow"));
        assert!(fallback::motion_for(false, false, false, false, true).contains("dramatic"));
    }

    #[test]
    fn fallback_camera_maps_each_lens() {
        assert!(fallback::camera_for(true, false, false, false, false, false).contains("establishing"));
        assert!(fallback::camera_for(false, true, false, false, false, false).contains("medium shot"));
        assert!(fallback::camera_for(false, false, false, true, false, false).contains("macro"));
        assert!(fallback::camera_for(false, false, false, false, true, false).contains("tracking"));
        assert!(fallback::camera_for(false, false, false, false, false, true).contains("bird"));
    }

    #[test]
    fn global_returns_default_singleton() {
        reset_for_test();
        let a = global();
        let b = global();
        // 两次调用应拿到同一 Arc (计数 +1)
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn set_for_test_replaces_global() {
        let rt = SecretsRuntime::new();
        let arc = set_for_test(rt);
        let got = global();
        assert!(Arc::ptr_eq(&arc, &got));
        reset_for_test();
    }
}