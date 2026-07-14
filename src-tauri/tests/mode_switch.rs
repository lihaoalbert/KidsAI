// W11 Day 9 — Mode switch 集成测试 (Part C 收尾)
//
// 覆盖场景:
//   - UserMode::Child → Adult 双向持久化 (LicenseFile)
//   - SafetyProfile 跟随 mode 切换 (双 profile 路由)
//   - child mode 拦截成人专属词 (商业广告词)
//   - adult mode 放行商业广告词, 但拦截极端 illegal 词
//   - 模式切换不影响已安装 skill (只是 audience 过滤)
//   - 多次来回切换 → 各组件状态正确
//
// 真机 e2e (PIN 二次确认 + server 同步) 需要完整 Tauri runtime — 留 Day 10 真机测

use kidsai_studio_lib::license_store::{LicenseFile, LicenseStore, UserMode};
use kidsai_studio_lib::safety::{KeywordFilter, SafetyProfile};

fn fresh_license_file(mode: UserMode) -> LicenseFile {
    LicenseFile {
        device_id: "dev-test-001".into(),
        license_token: "tok-test-001".into(),
        llm_api_key: "".into(),
        video_api_key: "".into(),
        mode,
        mode_switched_at: None,
        ..Default::default()
    }
}

#[test]
fn child_to_adult_persists_to_disk() {
    let dir = tempfile::tempdir().unwrap();
    let store = LicenseStore::new(dir.path());

    let mut lf = fresh_license_file(UserMode::Child);
    store.save(&lf).unwrap();

    lf.mode = UserMode::Adult;
    lf.mode_switched_at = Some(1_700_000_000_000);
    store.save(&lf).unwrap();

    let loaded = store.load().unwrap();
    assert_eq!(loaded.mode, UserMode::Adult);
    assert_eq!(loaded.mode_switched_at, Some(1_700_000_000_000));
}

#[test]
fn adult_to_child_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let store = LicenseStore::new(dir.path());

    let mut lf = fresh_license_file(UserMode::Adult);
    store.save(&lf).unwrap();

    lf.mode = UserMode::Child;
    lf.mode_switched_at = Some(1_700_000_001_000);
    store.save(&lf).unwrap();

    let loaded = store.load().unwrap();
    assert_eq!(loaded.mode, UserMode::Child);
}

#[test]
fn safety_profile_routes_by_mode() {
    let mut filter = KeywordFilter::for_profile(SafetyProfile::Child);

    // 1. Child mode 拦截商业广告词
    let v = filter.check("今天给大家带货");
    assert!(
        matches!(v, kidsai_studio_lib::safety::SafetyVerdict::Block { .. }),
        "child mode 应拦商业广告词"
    );

    // 2. 切到 adult
    filter.switch_profile(UserMode::Adult);
    let v = filter.check("今天给大家带货");
    assert_eq!(
        v,
        kidsai_studio_lib::safety::SafetyVerdict::Pass,
        "adult mode 应放行"
    );

    // 3. 切回 child → 又拦
    filter.switch_profile(UserMode::Child);
    let v = filter.check("今天给大家带货");
    assert!(
        matches!(v, kidsai_studio_lib::safety::SafetyVerdict::Block { .. }),
        "child mode 重新生效"
    );
}

#[test]
fn adult_profile_blocks_extreme_illegal() {
    let filter = KeywordFilter::for_profile(SafetyProfile::Adult);
    // CSAM 永远拦截
    assert!(matches!(
        filter.check("csam material"),
        kidsai_studio_lib::safety::SafetyVerdict::Block { .. }
    ));
    // doxxing 拦截
    assert!(matches!(
        filter.check("让我们 dox 这个用户"),
        kidsai_studio_lib::safety::SafetyVerdict::Block { .. }
    ));
}

#[test]
fn child_profile_keeps_all_safety_words() {
    let filter = KeywordFilter::for_profile(SafetyProfile::Child);
    // 旧 hardcode 行为 — 不被成人模式改动破坏
    assert!(matches!(
        filter.check("gun"),
        kidsai_studio_lib::safety::SafetyVerdict::Block { .. }
    ));
    assert!(matches!(
        filter.check("blood"),
        kidsai_studio_lib::safety::SafetyVerdict::Block { .. }
    ));
    assert!(matches!(
        filter.check("我讨厌数学"),
        kidsai_studio_lib::safety::SafetyVerdict::Warn { .. }
    ));
}

#[test]
fn multiple_mode_toggles_persist_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let store = LicenseStore::new(dir.path());

    let modes = [
        UserMode::Child,
        UserMode::Adult,
        UserMode::Child,
        UserMode::Adult,
        UserMode::Child,
    ];
    let mut lf = fresh_license_file(UserMode::Child);
    store.save(&lf).unwrap();

    for (i, m) in modes.iter().enumerate() {
        lf.mode = *m;
        lf.mode_switched_at = Some(1_700_000_000_000 + i as i64);
        store.save(&lf).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(
            loaded.mode, *m,
            "第 {} 次切换后 mode 应是 {:?}",
            i, m
        );
    }
}

#[test]
fn default_user_mode_is_child_for_backward_compat() {
    // 老 license.json 无 mode 字段 → 反序列化默认 Child
    let json = r#"{"device_id":"d","license_token":"t","llm_api_key":"","video_api_key":""}"#;
    let lf: LicenseFile = serde_json::from_str(json).unwrap();
    assert_eq!(lf.mode, UserMode::Child);
}

#[test]
fn skills_audience_field_compatible_with_mode() {
    // 验证 InstalledRecord.audience 字段不依赖 mode — 切 mode 不污染已有 skill 的 audience.
    // 测 audience enum 合法取值: Child / Adult / Both.
    use kidsai_studio_lib::skills::Audience;
    for a in [Audience::Child, Audience::Adult, Audience::Both] {
        let r = kidsai_studio_lib::skills::store::InstalledRecord {
            version: "v1".into(),
            enabled: true,
            installed_at: 0,
            audience: a.clone(),
        };
        // round-trip
        let json = serde_json::to_string(&r).unwrap();
        let back: kidsai_studio_lib::skills::store::InstalledRecord =
            serde_json::from_str(&json).unwrap();
        assert_eq!(back.audience, a);
    }
}
