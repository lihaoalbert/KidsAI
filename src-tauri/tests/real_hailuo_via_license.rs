// 真实 MiniMax hailuo 集成测试 — 与 real_seedance_via_license.rs 镜像, 仅换 provider.
//
// 端到端管线:
//   直连 ECS kidsai-server (admin grant → activate → 拿 MiniMax hailuo key from api_keys.video)
//   → 直连 MiniMax hailuo-02 image_to_video (1 段视频, ¥花费)
//   → 上报 spend 到 server → 校验余额递减 + 拿到 video_url.
//
// ⚠️ 真金白银: 1 段 hailuo 5s 视频 ≈ ¥0.x + 19 学币. 仅在 --ignored 下运行.
//
// gate env: KIDSAI_SERVER_URL + KIDSAI_ADMIN_TOKEN + (MINIMAX_API_KEY 备用) 必须设置.
// hailuo 是 MiniMax 套餐里自带的视频额度, 但 select_video_adapter() 工厂逻辑是:
//   - 优先 SEEDANCE_API_KEY → ark
//   - 否则 + HAILUO_VIDEO_ENABLED=1 + (MiniMax key) → hailuo
//   - 否则 mock
// 所以本测试用 env override 让它强制走 hailuo.
//
// 用法 (在 src-tauri/):
//   export KIDSAI_SERVER_URL=https://api.kids.ibi.ren
//   export KIDSAI_ADMIN_TOKEN=$(grep '^ADMIN_TOKEN=' /etc/kidsai-server/.env | cut -d= -f2)
//   unset SEEDANCE_API_KEY     # 阻止选 ark
//   cargo test --test real_hailuo_via_license -- --ignored --nocapture
//
// 需要时间: 30s - 3min (MiniMax hailuo 轮询).

use kidsai_studio_lib::license_client::{LicenseClient, RecordSpendResponse};
use kidsai_studio_lib::video_adapter::{select_video_adapter, VideoGenArgs};

const FIXED_FP: &str = "fp-real-hailuo-smoke-bbbbbbbb"; // 固定指纹 → activate 幂等
const NICKNAME: &str = "hailuo-smoke";
const AGE_TIER: u8 = 1;

fn both_envs() -> bool {
    let url = std::env::var("KIDSAI_SERVER_URL").unwrap_or_default();
    let tok = std::env::var("KIDSAI_ADMIN_TOKEN").unwrap_or_default();
    !url.trim().is_empty() && !tok.trim().is_empty()
}

#[tokio::test]
#[ignore]
async fn real_hailuo_end_to_end_via_license() {
    let _ = dotenvy::dotenv();
    if !both_envs() {
        eprintln!("[SKIP] KIDSAI_SERVER_URL / KIDSAI_ADMIN_TOKEN not set");
        return;
    }

    let admin_token = std::env::var("KIDSAI_ADMIN_TOKEN").unwrap();
    let server_url = std::env::var("KIDSAI_SERVER_URL").unwrap();
    let admin_http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();
    let license = LicenseClient::from_env();

    // 1. 固定指纹激活 (MiniMax 套餐里走 hailuo)
    let act = license
        .activate(FIXED_FP, NICKNAME, AGE_TIER)
        .await
        .expect("activate should succeed");
    eprintln!(
        "[1/6] activate device_id={} balance={} video_key_len={}",
        act.device_id,
        act.balance,
        act.api_keys.video.len(),
    );
    assert!(!act.license_token.is_empty());
    assert!(act.balance >= 19, "balance {} < 19 hailuo video cost", act.balance);

    // 2. admin grant +50 (防止 daily quota 用尽)
    let grant: serde_json::Value = admin_http
        .post(format!(
            "{}/api/v1/admin/devices/{}/grant",
            server_url, act.device_id
        ))
        .header("X-Admin-Token", &admin_token)
        .json(&serde_json::json!({ "amount": 50, "reason": "hailuo smoke 预置" }))
        .send()
        .await
        .expect("grant http")
        .json()
        .await
        .expect("grant parse");
    eprintln!("[2/6] admin grant +50 → {}", grant);

    // 3. 调前余额
    let pre = license
        .get_balance(&act.license_token)
        .await
        .expect("balance http");
    eprintln!(
        "[3/6] pre-call balance={} daily_consumed={}/{}",
        pre.balance, pre.daily_consumed, pre.daily_quota
    );
    let balance_before = pre.balance;

    // 4. 强制走 hailuo: 临时清掉 SEEDANCE_API_KEY, 启用 HAILUO_VIDEO_ENABLED,
    //    并把 license 拿到的 api_keys.video 写到 MINIMAX_API_KEY (hailuo 适配器读这个).
    std::env::remove_var("SEEDANCE_API_KEY");
    std::env::set_var("HAILUO_VIDEO_ENABLED", "1");
    std::env::set_var("MINIMAX_API_KEY", &act.api_keys.video);

    let prompt =
        "a curious little cat in a moonlit garden chasing butterflies, soft pastel, gentle motion";
    let args = VideoGenArgs {
        prompt: prompt.into(),
        image_url: None,
        image_role: None,
        duration_seconds: Some(5),
        ratio: Some("16:9".to_string()),
        resolution: None,        // MiniMax hailuo 默认 720p
        generate_audio: Some(false),
        model: None,             // adapter default = hailuo-02
        seed: None,
    };

    eprintln!("[4/6] → hailuo POST (5s, 16:9, no audio)...");
    let start = std::time::Instant::now();
    let video = tokio::task::spawn_blocking(move || {
        let sel = select_video_adapter();
        assert_eq!(
            sel.source, "hailuo",
            "应强制选 hailuo provider, 实际: {}",
            sel.source
        );
        sel.adapter.generate(&args)
    })
    .await
    .expect("spawn_blocking join");
    let elapsed = start.elapsed();
    let video = video.expect("hailuo 应成功");
    eprintln!(
        "[4/6] ← hailuo succeeded in {:?} task_id={} url={}",
        elapsed,
        video.provider_task_id,
        &video.url[..80.min(video.url.len())],
    );
    assert_eq!(video.provider, "hailuo");
    assert!(!video.url.is_empty());
    assert!(
        video.url.starts_with("http"),
        "video_url 应是 https URL, got: {}",
        video.url
    );

    // 5. 上报 spend
    let spend: RecordSpendResponse = license
        .record_spend(
            &act.license_token,
            &format!("hailuo-smoke-{}", video.provider_task_id),
            "video_final",
            1,
        )
        .await
        .expect("record_spend http");
    eprintln!(
        "[5/6] record_spend → accepted={} cost={} balanceAfter={}",
        spend.accepted, spend.cost, spend.balance_after
    );
    assert!(spend.accepted);
    assert_eq!(spend.cost, 19, "video_final cost=19");
    assert_eq!(spend.balance_after, balance_before - 19);

    // 6. 调后余额校验
    let post = license
        .get_balance(&act.license_token)
        .await
        .expect("balance http");
    eprintln!(
        "[6/6] post-call balance={} daily_consumed={}",
        post.balance, post.daily_consumed
    );
    assert_eq!(post.balance, balance_before - 19);
    assert!(post.daily_consumed >= 19);

    eprintln!(
        "\n✅ hailuo 端到端 PASS — video_url={} ({:.1}s elapsed)",
        video.url,
        elapsed.as_secs_f32()
    );
}
