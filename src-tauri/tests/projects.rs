use std::sync::Arc;
use std::thread;

use kidsai_studio_lib::projects::{ProjectStatePatch, Projects};
use kidsai_studio_lib::Db;
use serde_json::json;
use tempfile::TempDir;

fn setup() -> (TempDir, Arc<Db>, Arc<Projects>) {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&dir.path().join("kidsai.db")).unwrap());
    let projects = Arc::new(Projects::new(dir.path()).unwrap());
    (dir, db, projects)
}

#[test]
fn create_list_and_load_round_trip() {
    let (_dir, db, projects) = setup();
    let created = projects.create(&db, "森林小冒险", Some("L4")).unwrap();

    let list = projects.list(&db).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, created.id);
    assert_eq!(list[0].title, "森林小冒险");
    assert_eq!(list[0].level_id.as_deref(), Some("L4"));

    let full = projects.load(&db, &created.id).unwrap();
    assert_eq!(full.meta, created);
    assert_eq!(full.plan, json!({}));
    assert_eq!(full.transcript, json!([]));
    let project_dir = projects.root().join(&created.id);
    assert!(project_dir.join("project.json").is_file());
    assert!(project_dir.join("plan.json").is_file());
    assert!(project_dir.join("transcript.json").is_file());
}

#[test]
fn save_state_updates_json_and_metadata() {
    let (_dir, db, projects) = setup();
    let created = projects.create(&db, "我的电影", None).unwrap();
    let plan = json!({"cursor": 4, "shots": [{"id": "shot-1"}]});
    let transcript = json!([{"kind": "kid", "text": "飞到月球"}]);

    let saved = projects
        .save_state(
            &db,
            &created.id,
            &plan,
            &transcript,
            &ProjectStatePatch {
                cursor: Some(4),
                thumb_path: Some("thumb.png".to_string()),
                total_credits: Some(18),
            },
        )
        .unwrap();

    assert_eq!(saved.cursor, 4);
    assert_eq!(saved.thumb_path.as_deref(), Some("thumb.png"));
    assert_eq!(saved.total_credits, 18);
    let full = projects.load(&db, &created.id).unwrap();
    assert_eq!(full.plan, plan);
    assert_eq!(full.transcript, transcript);
}

#[test]
fn rename_updates_database_and_project_json() {
    let (_dir, db, projects) = setup();
    let created = projects.create(&db, "旧标题", None).unwrap();
    projects.rename(&db, &created.id, "  新标题  ").unwrap();

    let full = projects.load(&db, &created.id).unwrap();
    assert_eq!(full.meta.title, "新标题");
    let project_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(projects.root().join(&created.id).join("project.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(project_json["title"], "新标题");
}

#[test]
fn delete_moves_directory_to_trash_and_removes_metadata() {
    let (_dir, db, projects) = setup();
    let created = projects.create(&db, "待删除", None).unwrap();
    projects.delete(&db, &created.id).unwrap();

    assert!(projects.list(&db).unwrap().is_empty());
    assert!(projects.load(&db, &created.id).is_err());
    let trash = projects.root().join("_trash").join(&created.id);
    assert!(trash.is_dir());
    assert!(trash.join("deleted.json").is_file());
}

#[test]
fn invalid_project_ids_and_titles_are_rejected() {
    let (_dir, db, projects) = setup();
    assert!(projects.create(&db, "   ", None).is_err());
    assert!(projects.load(&db, "../../escape").is_err());
    assert!(projects.rename(&db, "../../escape", "x").is_err());
    assert!(projects.delete(&db, "../../escape").is_err());
}

#[test]
fn concurrent_renames_leave_valid_state() {
    let (_dir, db, projects) = setup();
    let created = projects.create(&db, "原标题", None).unwrap();
    let mut threads = Vec::new();
    for index in 0..8 {
        let db = Arc::clone(&db);
        let projects = Arc::clone(&projects);
        let id = created.id.clone();
        threads.push(thread::spawn(move || {
            projects.rename(&db, &id, &format!("标题 {index}")).unwrap();
        }));
    }
    for thread in threads {
        thread.join().unwrap();
    }

    let full = projects.load(&db, &created.id).unwrap();
    assert!(full.meta.title.starts_with("标题 "));
    let project_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(projects.root().join(&created.id).join("project.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(project_json["title"], full.meta.title);
}

#[test]
fn corrupt_project_state_returns_an_error() {
    let (_dir, db, projects) = setup();
    let created = projects.create(&db, "损坏测试", None).unwrap();
    std::fs::write(
        projects.root().join(&created.id).join("plan.json"),
        "not-json",
    )
    .unwrap();
    let error = projects.load(&db, &created.id).unwrap_err();
    assert!(error.contains("parse"), "got: {error}");
}
