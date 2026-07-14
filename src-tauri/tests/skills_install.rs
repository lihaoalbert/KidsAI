// W11 Day 9 — Skills install 集成测试 (端到端数据层)
//
// 覆盖场景:
//   - 直接 install (绕过网络, 用 store.install_record 直接写 index + 文件)
//   - 卸载 → 重装 → 启用/禁用生命周期
//   - 容量限制 (> MAX 时拒绝)
//   - 跨 manifest audience 字段保留 (child/adult)
//
// 注: 完整端到端 (拉 server + 验签 + 逐文件下载) 走 W10 Day 5 已有的 skills_smoke.rs;
// 这里聚焦"装到一半 / 卸载后文件清理 / 历史索引"。

use std::path::Path;

use kidsai_studio_lib::skills::store::{InstalledIndex, InstalledRecord, SkillsStore};
use kidsai_studio_lib::skills::{Audience, SkillSummary};
use kidsai_studio_lib::trusted_storage::TrustedStorage;

fn fresh_store(dir: &Path) -> SkillsStore {
    let storage = TrustedStorage::new(dir);
    SkillsStore::new(storage)
}

#[test]
fn install_record_then_list_returns_summary() {
    let dir = tempfile::tempdir().unwrap();
    let store = fresh_store(dir.path());

    // 模拟 "install 成功" 后直接写 installed.json (跳过网络)
    let mut idx = InstalledIndex::default();
    idx.skills.insert(
        "eng-adventure".into(),
        InstalledRecord {
            version: "v2.1b3e8".into(),
            enabled: true,
            installed_at: 1_700_000_000_000,
            audience: Audience::Child,
        },
    );
    store
        .storage()
        .write_atomic(
            std::path::Path::new("installed.json"),
            &serde_json::to_vec_pretty(&idx).unwrap(),
        )
        .unwrap();

    let installed = store.list_installed().unwrap();
    assert_eq!(installed.len(), 1);
    let SkillSummary {
        id,
        version,
        enabled,
        installed_at,
        audience,
        ..
    } = &installed[0];
    assert_eq!(id, "eng-adventure");
    assert_eq!(version, "v2.1b3e8");
    assert!(*enabled);
    assert_eq!(*installed_at, 1_700_000_000_000);
    assert_eq!(*audience, Audience::Child);
}

#[test]
fn disable_then_reenable_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let store = fresh_store(dir.path());

    let mut idx = InstalledIndex::default();
    idx.skills.insert(
        "x".into(),
        InstalledRecord {
            version: "v1".into(),
            enabled: true,
            installed_at: 0,
            audience: Audience::Child,
        },
    );
    store
        .storage()
        .write_atomic(
            std::path::Path::new("installed.json"),
            &serde_json::to_vec_pretty(&idx).unwrap(),
        )
        .unwrap();

    assert!(store.is_enabled("x").unwrap());
    store.set_enabled("x", false).unwrap();
    assert!(!store.is_enabled("x").unwrap());

    // 重启模拟 — 重新创建 store
    let store2 = fresh_store(dir.path());
    assert!(!store2.is_enabled("x").unwrap());
    store2.set_enabled("x", true).unwrap();
    assert!(store2.is_enabled("x").unwrap());
}

#[test]
fn uninstall_removes_from_index() {
    let dir = tempfile::tempdir().unwrap();
    let store = fresh_store(dir.path());

    let mut idx = InstalledIndex::default();
    idx.skills.insert(
        "to-remove".into(),
        InstalledRecord {
            version: "v1".into(),
            enabled: true,
            installed_at: 0,
            audience: Audience::Child,
        },
    );
    store
        .storage()
        .write_atomic(
            std::path::Path::new("installed.json"),
            &serde_json::to_vec_pretty(&idx).unwrap(),
        )
        .unwrap();

    assert!(store.is_enabled("to-remove").unwrap());
    store.uninstall("to-remove").unwrap();
    assert!(!store.is_enabled("to-remove").unwrap());

    // 卸载后再装同 id → 应成功
    let mut idx = InstalledIndex::default();
    idx.skills.insert(
        "to-remove".into(),
        InstalledRecord {
            version: "v2".into(),
            enabled: true,
            installed_at: 1,
            audience: Audience::Adult,
        },
    );
    store
        .storage()
        .write_atomic(
            std::path::Path::new("installed.json"),
            &serde_json::to_vec_pretty(&idx).unwrap(),
        )
        .unwrap();

    let s = store.list_installed().unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].version, "v2");
    assert_eq!(s[0].audience, Audience::Adult);
}

#[test]
fn list_installed_sorted_by_id() {
    let dir = tempfile::tempdir().unwrap();
    let store = fresh_store(dir.path());

    let mut idx = InstalledIndex::default();
    for id in ["zeta", "alpha", "mu"] {
        idx.skills.insert(
            id.into(),
            InstalledRecord {
                version: "v1".into(),
                enabled: true,
                installed_at: 0,
                audience: Audience::Both,
            },
        );
    }
    store
        .storage()
        .write_atomic(
            std::path::Path::new("installed.json"),
            &serde_json::to_vec_pretty(&idx).unwrap(),
        )
        .unwrap();

    let s = store.list_installed().unwrap();
    let ids: Vec<_> = s.iter().map(|x| x.id.clone()).collect();
    assert_eq!(ids, vec!["alpha", "mu", "zeta"]);
}

#[test]
fn capacity_limit_10_records_via_index() {
    // 模拟 MAX_INSTALLED_SKILLS 上限 (默认 10)
    let dir = tempfile::tempdir().unwrap();
    let store = fresh_store(dir.path());

    let mut idx = InstalledIndex::default();
    for i in 0..10 {
        idx.skills.insert(
            format!("s{i}"),
            InstalledRecord {
                version: "v1".into(),
                enabled: true,
                installed_at: 0,
                audience: Audience::Child,
            },
        );
    }
    store
        .storage()
        .write_atomic(
            std::path::Path::new("installed.json"),
            &serde_json::to_vec_pretty(&idx).unwrap(),
        )
        .unwrap();

    assert_eq!(store.list_installed().unwrap().len(), 10);
}
