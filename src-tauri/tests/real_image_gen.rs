// W6 D2: 真实 MiniMax image-01 集成测试 (--ignored)
//
// 仅当 .env (或环境变量) 有 MINIMAX_API_KEY 时有意义, 否则 eprintln SKIP.
// 真跑 ~¥1 (单次 image-01) + 5 学币 (后端消费); 用 --ignored 才执行.
//
// 验证:
// - select_image_adapter() 走 minimax 而非 mock
// - POST /image_generation 真接通, 返 data[0].url 是 HTTPS URL
// - URL 可 HTTP GET 200, 字节数 > 1KB (真图非空)
//
// 不测 (留 W6+ 后):
// - prompt 内容质量 (主观, 人评)
// - image-01 多 aspect_ratio 切换 (W3 分镜用, 后续 stage E2E 再覆盖)

use kidsai_studio_lib::image_adapter::{select_image_adapter, ImageGenArgs};

fn real_key_available() -> bool {
    let _ = dotenvy::dotenv();
    std::env::var("MINIMAX_API_KEY")
        .map(|v| !v.is_empty() && v.len() > 20)
        .unwrap_or(false)
}

#[test]
#[ignore = "requires real MiniMax API key; run with: cargo test --test real_image_gen -- --ignored"]
fn real_minimax_image_01_one_shot() {
    if !real_key_available() {
        eprintln!("[SKIP] MINIMAX_API_KEY not set or too short");
        return;
    }

    let sel = select_image_adapter();
    assert_eq!(sel.source, "minimax", "should pick minimax provider");

    let asset = sel
        .adapter
        .generate(&ImageGenArgs {
            prompt: "a tiny cute robot child waving hello, kids illustration, white background"
                .into(),
            aspect_ratio: Some("1:1".into()),
        })
        .expect("image-01 should succeed");

    eprintln!(
        "[INFO] provider={} task_id={} url={}",
        asset.provider, asset.provider_task_id, asset.url
    );

    assert_eq!(asset.provider, "minimax");
    assert!(
        asset.url.starts_with("https://"),
        "url must be https: {}",
        asset.url
    );

    // 二次 GET 验证 URL 真可下载
    let resp = reqwest::blocking::get(&asset.url).expect("download should succeed");
    assert!(
        resp.status().is_success(),
        "download HTTP {}",
        resp.status()
    );
    let bytes = resp.bytes().expect("body");
    assert!(bytes.len() > 1024, "image too small: {} bytes", bytes.len());
    eprintln!("[OK] image downloaded: {} bytes", bytes.len());
}
