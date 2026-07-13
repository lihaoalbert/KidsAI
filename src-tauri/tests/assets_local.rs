use std::sync::{Arc, Mutex};

use kidsai_studio_lib::assets_local::{AssetEventSink, AssetLocalEvent, AssetsLocal};
use kidsai_studio_lib::projects::Projects;
use kidsai_studio_lib::Db;
use tempfile::TempDir;

#[derive(Default)]
struct CollectingSink {
    events: Mutex<Vec<AssetLocalEvent>>,
}

impl AssetEventSink for CollectingSink {
    fn emit(&self, event: &AssetLocalEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

fn setup(sink: Arc<dyn AssetEventSink>) -> (TempDir, Db, Projects, AssetsLocal, String) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("kidsai.db");
    let db = Db::open(&db_path).unwrap();
    let projects = Projects::new(dir.path()).unwrap();
    let project = projects.create(&db, "下载测试", None).unwrap();
    let assets = AssetsLocal::new(dir.path(), &db_path, sink).unwrap();
    (dir, db, projects, assets, project.id)
}

#[tokio::test]
async fn downloads_nested_asset_and_resolves_local_path() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/preview.mp4")
        .with_status(200)
        .with_body(b"video-bytes")
        .expect(1)
        .create_async()
        .await;
    let sink = Arc::new(CollectingSink::default());
    let (_dir, _db, _projects, assets, project_id) = setup(sink.clone());
    let url = format!("{}/preview.mp4", server.url());

    let id = assets
        .enqueue(&project_id, &url, "video", "shots/01-hook/preview.mp4")
        .unwrap();
    assets.wait(id).await.unwrap();

    mock.assert_async().await;
    let record = assets.get_record(id).unwrap().unwrap();
    assert_eq!(record.status, "downloaded");
    assert_eq!(record.bytes, Some(11));
    let local = assets.resolve(&project_id, &url).unwrap().unwrap();
    assert_eq!(std::fs::read(&local).unwrap(), b"video-bytes");
    let events = sink.events.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].status, "downloaded");
    assert_eq!(events[0].local_path, local);
}

#[tokio::test]
async fn enqueue_is_idempotent_after_download() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/image.png")
        .with_status(200)
        .with_body(b"png")
        .expect(1)
        .create_async()
        .await;
    let (_dir, _db, _projects, assets, project_id) = setup(Arc::new(CollectingSink::default()));
    let url = format!("{}/image.png", server.url());

    let first = assets
        .enqueue(&project_id, &url, "image", "character/stand.png")
        .unwrap();
    assets.wait(first).await.unwrap();
    let second = assets
        .enqueue(&project_id, &url, "image", "character/stand.png")
        .unwrap();

    mock.assert_async().await;
    assert_eq!(first, second);
    assert_eq!(
        assets.get_record(second).unwrap().unwrap().status,
        "downloaded"
    );
}

#[tokio::test]
async fn failed_download_retries_three_times_and_marks_failed() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/broken.mp4")
        .with_status(503)
        .expect(3)
        .create_async()
        .await;
    let (_dir, _db, _projects, assets, project_id) = setup(Arc::new(CollectingSink::default()));
    let url = format!("{}/broken.mp4", server.url());

    let id = assets
        .enqueue(&project_id, &url, "video", "shots/broken.mp4")
        .unwrap();
    assets.wait(id).await.unwrap();

    mock.assert_async().await;
    let record = assets.get_record(id).unwrap().unwrap();
    assert_eq!(record.status, "failed");
    assert!(assets.resolve(&project_id, &url).unwrap().is_none());
}

#[tokio::test]
async fn duplicate_enqueue_while_running_reuses_task() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/audio.mp3")
        .with_status(200)
        .with_body(b"audio")
        .expect(1)
        .create_async()
        .await;
    let (_dir, _db, _projects, assets, project_id) = setup(Arc::new(CollectingSink::default()));
    let url = format!("{}/audio.mp3", server.url());

    let first = assets
        .enqueue(&project_id, &url, "audio", "audio/line-1.mp3")
        .unwrap();
    let second = assets
        .enqueue(&project_id, &url, "audio", "audio/line-1.mp3")
        .unwrap();
    assert_eq!(first, second);
    assets.wait(first).await.unwrap();
    mock.assert_async().await;
}

#[test]
fn rejects_path_traversal_and_invalid_inputs() {
    let (_dir, _db, _projects, assets, project_id) = setup(Arc::new(CollectingSink::default()));
    assert!(assets
        .enqueue(
            &project_id,
            "https://example.com/a",
            "video",
            "../escape.mp4"
        )
        .is_err());
    assert!(assets
        .enqueue(&project_id, "file:///tmp/a", "video", "safe.mp4")
        .is_err());
    assert!(assets
        .enqueue(&project_id, "https://example.com/a", "binary", "safe.bin")
        .is_err());
    assert!(assets
        .enqueue("not-a-uuid", "https://example.com/a", "video", "safe.mp4")
        .is_err());
}
