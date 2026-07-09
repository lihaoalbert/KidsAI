// SQLite 集成冒烟测试
// 验证 Db::open + 进度 upsert / complete + 列出都能跑通

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_db_path(test_name: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let mut p = std::env::temp_dir();
    p.push(format!(
        "kidsai-test-{}-{}-{}.db",
        std::process::id(),
        test_name,
        n
    ));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn db_open_and_migrate() {
    let path = fresh_db_path("open");
    let db = kidsai_studio_lib::Db::open(&path).expect("open db");
    let progress = db.list_progress().expect("list_progress");
    assert!(progress.is_empty(), "fresh db should have no progress");
}

#[test]
fn db_start_and_complete_progress() {
    let path = fresh_db_path("start_complete");
    let db = kidsai_studio_lib::Db::open(&path).expect("open db");

    let p1 = db
        .upsert_progress_in_progress("L1")
        .expect("start L1");
    assert_eq!(p1.level_id, "L1");
    assert_eq!(p1.attempts, 1);
    assert_eq!(p1.status, kidsai_studio_lib::LevelStatus::InProgress);

    let p2 = db.upsert_progress_in_progress("L1").expect("start L1 again");
    assert_eq!(p2.attempts, 2);

    let done = db.mark_completed("L1", 85).expect("complete L1");
    assert_eq!(done.status, kidsai_studio_lib::LevelStatus::Completed);
    assert_eq!(done.best_score, Some(85));
    assert!(done.completed_at.is_some());

    let ids = db.list_completed_ids().expect("completed ids");
    assert_eq!(ids, vec!["L1".to_string()]);
}

#[test]
fn db_best_score_takes_max() {
    let path = fresh_db_path("best_score");
    let db = kidsai_studio_lib::Db::open(&path).expect("open db");
    db.upsert_progress_in_progress("L1").unwrap();
    let _ = db.mark_completed("L1", 60).unwrap();
    let _ = db.upsert_progress_in_progress("L1").unwrap();
    let p = db.mark_completed("L1", 75).unwrap();
    assert_eq!(p.best_score, Some(75), "best_score should be 75 not 60");

    let _ = db.upsert_progress_in_progress("L1").unwrap();
    let p2 = db.mark_completed("L1", 50).unwrap();
    assert_eq!(p2.best_score, Some(75), "best_score should remain 75");
}

#[test]
fn db_creations_and_assets() {
    let path = fresh_db_path("creations");
    let db = kidsai_studio_lib::Db::open(&path).expect("open db");

    db.insert_creation(
        "c1",
        "L1",
        "一只小猫",
        r#"{"thoughts":["a"],"finalAnswer":"b"}"#,
        Some(80),
        Some(r#"{"creativity":30,"technical":25,"narrative":20,"aesthetic":15,"compliance":10}"#),
        Some("good"),
    )
    .expect("insert creation");

    db.insert_asset("c1", "image", "https://x/1.png", None, "一只小猫", "generate_image", 5)
        .expect("insert asset 1");
    db.insert_asset("c1", "video", "https://x/1.mp4", Some("https://x/1.jpg"), "一只小猫动起来", "image_to_video", 20)
        .expect("insert asset 2");

    let rows = db.list_creations(None).expect("list creations");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "c1");
    assert_eq!(rows[0].level_id, "L1");
    assert_eq!(rows[0].score, Some(80));

    let assets = db.list_assets("c1").expect("list assets");
    assert_eq!(assets.len(), 2);
    assert_eq!(assets[0].kind, "image");
    assert_eq!(assets[1].kind, "video");
    assert_eq!(assets[1].thumbnail_url.as_deref(), Some("https://x/1.jpg"));

    let only_l1 = db.list_creations(Some("L1")).expect("filter L1");
    assert_eq!(only_l1.len(), 1);
    let only_l2 = db.list_creations(Some("L2")).expect("filter L2");
    assert_eq!(only_l2.len(), 0);
}
