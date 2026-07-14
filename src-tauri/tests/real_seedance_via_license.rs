// 真实 Seedance 集成测试 (W4.5 C1)
//
// 端到端管线: 直连 ECS kidsai-server (admin grant → activate → 拿 Seedance key)
// → 直连火山方舟 Seedance (image_to_video 真实生成 1 段视频, ¥花费)
// → 上报 spend 到 server → 校验余额正确递减 + 拿到真 video_url.
//
// ⚠️ 真金白银: 1 段 Seedance mini 视频 ≈ ¥0.x + 19 学币. 仅在 --ignored 下运行.
//
// gate env: KIDSAI_SERVER_URL + KIDSAI_ADMIN_TOKEN 必须设置.
// 跳过条件: 任意一个缺失或值为空.
//
// 用法 (在 src-tauri/):
//   export KIDSAI_SERVER_URL=https://api.kids.ibi.ren
//   export KIDSAI_ADMIN_TOKEN=$(grep '^ADMIN_TOKEN=' /etc/kidsai-server/.env | cut -d= -f2)
//   cargo test --test real_seedance_via_license -- --ignored --nocapture
//
// 需要时间: 30s - 3min (Seedance 轮询 + 视频生成).

use kidsai_studio_lib::license_client::{LicenseClient, RecordSpendResponse};
use kidsai_studio_lib::video_adapter::{select_video_adapter, VideoGenArgs};

const FIXED_FP: &str = "fp-real-seedance-smoke-aaaaaaaa"; // 固定指纹 → activate 幂等
const NICKNAME: &str = "seedance-smoke";
const AGE_TIER: u8 = 1;

fn both_envs() -> bool {
    let url = std::env::var("KIDSAI_SERVER_URL").unwrap_or_default();
    let tok = std::env::var("KIDSAI_ADMIN_TOKEN").unwrap_or_default();
    !url.trim().is_empty() && !tok.trim().is_empty()
}

#[tokio::test]
#[ignore]
async fn real_seedance_end_to_end_via_license() {
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

    // ---- 步骤 1: 固定指纹激活 (幂等: 已激活则返回原 license, 学币不重置) ----
    let act = license
        .activate(FIXED_FP, NICKNAME, AGE_TIER)
        .await
        .expect("activate should succeed");
    eprintln!(
        "[1/6] activate device_id={} balance={} daily_quota={} (key_len={})",
        act.device_id,
        act.balance,
        act.daily_quota,
        act.api_keys.video.len(),
    );
    assert!(!act.license_token.is_empty(), "license_token empty");
    assert!(
        act.balance >= 19,
        "balance {} < 19 video_final cost",
        act.balance
    );

    // ---- 步骤 2: admin grant +50, 防止 daily_quota 用完 (90 学币) ----
    let grant: serde_json::Value = admin_http
        .post(format!(
            "{}/api/v1/admin/devices/{}/grant",
            server_url, act.device_id
        ))
        .header("X-Admin-Token", &admin_token)
        .json(&serde_json::json!({ "amount": 50, "reason": "C1 真实 Seedance smoke 预置" }))
        .send()
        .await
        .expect("grant http")
        .json()
        .await
        .expect("grant parse");
    eprintln!("[2/6] admin grant +50 → {}", grant);

    // ---- 步骤 3: 调前余额查询 (证明 control plane 工作) ----
    let pre = license
        .get_balance(&act.license_token)
        .await
        .expect("balance http");
    eprintln!(
        "[3/6] pre-call balance={} daily_consumed={}/{}",
        pre.balance, pre.daily_consumed, pre.daily_quota
    );
    let balance_before = pre.balance;

    // ---- 步骤 4: 直连火山方舟 Seedance mini (cost-min + 5s) ----
    // 用 adapter 工厂: 临时把 api_key 设到 env, 让 select_video_adapter() 返回 ark adapter.
    // 真实生产里 license_client.activate() 已经把 key 放进 license_store, 这只是让工厂选对 provider.
    std::env::set_var("SEEDANCE_API_KEY", &act.api_keys.video);
    let prompt = "一只小猫在月光下的花园里追蝴蝶, 儿童绘本风格, 柔和色调, 缓慢移动, 5秒";
    let args = VideoGenArgs {
        prompt: prompt.into(),
        image_url: None, // 文本生视频 (t2v) — 最便宜, 不需要首帧图
        image_role: None,
        duration_seconds: Some(5),
        ratio: Some("16:9".to_string()),
        resolution: Some("480p".to_string()),
        generate_audio: Some(false), // 无声, 进一步省钱
        model: Some("doubao-seedance-2-0-mini-260615".to_string()), // mini 是最便宜档
        seed: None,
    };

    eprintln!("[4/6] → Seedance POST (mini, 5s, 480p, no audio)...");
    let start = std::time::Instant::now();
    let video = tokio::task::spawn_blocking(move || {
        let sel = select_video_adapter();
        assert_eq!(sel.source, "ark", "应该选 ark provider");
        sel.adapter.generate(&args)
    })
    .await
    .expect("spawn_blocking join");
    let elapsed = start.elapsed();
    let video = video.expect("Seedance 应成功");
    eprintln!(
        "[4/6] ← Seedance succeeded in {:?} task_id={} url={}",
        elapsed,
        video.provider_task_id,
        &video.url[..80.min(video.url.len())],
    );
    assert_eq!(video.provider, "ark");
    assert!(!video.url.is_empty());
    assert!(video.url.starts_with("http"), "video_url 应是 https URL");

    // ---- 步骤 5: 上报 spend (cost video_final = 19 学币) ----
    let spend: RecordSpendResponse = license
        .record_spend(
            &act.license_token,
            &format!("seedance-smoke-{}", video.provider_task_id),
            "video_final",
            1,
        )
        .await
        .expect("record_spend http");
    eprintln!(
        "[5/6] record_spend → accepted={} cost={} balanceAfter={}",
        spend.accepted, spend.cost, spend.balance_after
    );
    assert!(
        spend.accepted,
        "spend 应 accepted, rejected_reason={:?}",
        spend.rejected_reason
    );
    assert_eq!(spend.cost, 19, "video_final cost=19");
    assert_eq!(spend.balance_after, balance_before - 19);

    // ---- 步骤 6: 调后再查余额, 校验 server 记账一致 ----
    let post = license
        .get_balance(&act.license_token)
        .await
        .expect("balance http");
    eprintln!(
        "[6/6] post-call balance={} daily_consumed={}",
        post.balance, post.daily_consumed
    );
    assert_eq!(post.balance, balance_before - 19);
    assert!(
        post.daily_consumed >= 19,
        "daily_consumed 应 ≥ 19 计入 video_final"
    );

    eprintln!(
        "\n✅ C1 端到端 PASS — video_url={} ({:.1}s elapsed, key={:?})",
        video.url,
        elapsed.as_secs_f32(),
        act.api_keys.video.len()
    );
}
