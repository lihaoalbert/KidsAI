// W6 D2: 真实 MiniMax music-01 集成测试 (--ignored)
//
// 仅当 .env (或环境变量) 有 MINIMAX_API_KEY 时有意义.
// 真跑 ~¥2 (单次 30s music-01) + 8 学币; 用 --ignored 才执行.
//
// 验证:
// - select_music_adapter() 走 minimax
// - POST /music_generation 拿 task_id
// - 轮询直到 succeeded, 返 audio_url
// - audio_url 可 HTTP HEAD 200

use kidsai_studio_lib::music_adapter::{select_music_adapter, MusicGenArgs};

fn real_key_available() -> bool {
    let _ = dotenvy::dotenv();
    std::env::var("MINIMAX_API_KEY")
        .map(|v| !v.is_empty() && v.len() > 20)
        .unwrap_or(false)
}

#[test]
#[ignore = "requires real MiniMax API key; run with: cargo test --test real_music_gen -- --ignored"]
fn real_minimax_music_01_one_shot() {
    if !real_key_available() {
        eprintln!("[SKIP] MINIMAX_API_KEY not set or too short");
        return;
    }

    let sel = select_music_adapter();
    assert_eq!(sel.source, "minimax", "should pick minimax provider");

    // W6 默认 BGM: instrumental 30s, 风格 prompt 跟随 cartoon/anime
    let asset = sel
        .adapter
        .generate(&MusicGenArgs {
            prompt: "playful ukulele, cartoon intro, cheerful and short".into(),
            duration_seconds: 30,
            instrumental: true,
        })
        .expect("music-01 should succeed (polling)");

    eprintln!(
        "[INFO] provider={} task_id={} duration={}s url={}",
        asset.provider, asset.provider_task_id, asset.duration_seconds, asset.url
    );

    assert_eq!(asset.provider, "minimax");
    assert_eq!(asset.duration_seconds, 30);
    assert!(asset.url.starts_with("https://"), "url must be https: {}", asset.url);

    // HEAD 验证音频 URL 可达 (MiniMax 返回临时 URL, 24h 内有效)
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();
    let head = client.head(&asset.url).send().expect("HEAD should succeed");
    assert!(head.status().is_success(), "HEAD HTTP {}", head.status());
    eprintln!("[OK] music URL reachable, content-length={:?}", head.headers().get("content-length"));
}