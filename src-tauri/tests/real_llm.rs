// 真实 LLM 集成测试
// 仅当 .env（或环境变量）有 MINIMAX_API_KEY 时运行
// 验证：工厂选 minimax → HTTP 调用成功 → tool_call 解析正确 → 真实 LLM 至少跑起来
//
// 注意：真实 LLM 走的是 ReAct，但 L1 的 system_prompt 设计是交互式老师
// （不直接给答案、用问题引导孩子），所以模型可能跑 1 轮就反问孩子
// （"要继续做视频吗？"）然后等不到回答就结束。
// 我们只验证"真实 LLM 真的接上了"，不验证轨迹。
// 轨迹正确性由 mock 测试覆盖（agent_smoke.rs）。

use kidsai_studio_lib::test_helpers::run_agent_with_model;

fn real_key_available() -> bool {
    let _ = dotenvy::dotenv();
    std::env::var("MINIMAX_API_KEY")
        .map(|v| !v.is_empty() && v.len() > 20)
        .unwrap_or(false)
}

#[tokio::test]
async fn real_minimax_l1_full_loop() {
    if !real_key_available() {
        eprintln!("[SKIP] MINIMAX_API_KEY not set or too short");
        return;
    }
    let selected = kidsai_studio_lib::model_factory::select_model();
    assert_eq!(selected.source, "minimax", "should pick minimax provider");
    eprintln!("[INFO] using model: {}", selected.model.name());

    // 让模型不要反问、直接调用 generate_image 即可。真实视频链路由 Seedance 适配器测试覆盖。
    let l1_prompt = format!(
        "{}\n\n[演示模式] 这是一键演示，不要向用户提问，直接调用 generate_image 工具生成一张图，然后总结即可。",
        kidsai_studio_lib::content::builtin_levels()
            .into_iter()
            .find(|l| l.id == "L1")
            .map(|l| l.system_prompt)
            .expect("L1 level should exist")
    );

    let result = run_agent_with_model(
        selected.model,
        "L1",
        "一只小猫在月光下追蝴蝶",
        &l1_prompt,
        vec!["generate_image".to_string()],
    )
    .await
    .expect("L1 should complete");

    eprintln!(
        "[RESULT] steps={} tokens={} assets={}",
        result.steps,
        result.tokens_used,
        result.assets.len()
    );
    eprintln!(
        "[RESULT] tool_calls: {:?}",
        result
            .tool_calls
            .iter()
            .map(|t| t.tool.as_str())
            .collect::<Vec<_>>()
    );
    eprintln!(
        "[RESULT] final_answer preview: {}",
        result.final_answer.chars().take(200).collect::<String>()
    );

    // 真实 LLM 至少要：
    // 1. 跑 ≥1 步
    // 2. 产出非空 final_answer
    // 3. 用真实 token（>100 = 至少调了一次推理）
    // 4. 调过至少一个工具（证明 tool calling 接通了）
    assert!(result.steps >= 1, "should run at least 1 step (got {})", result.steps);
    assert!(!result.final_answer.is_empty(), "should produce final answer");
    assert!(
        !result.tool_calls.is_empty(),
        "real LLM should call at least one tool, got 0"
    );
    assert!(
        result.tokens_used > 100,
        "real LLM should use >100 tokens, got {}",
        result.tokens_used
    );
    assert!(
        result.model.starts_with("minimax:"),
        "model name should start with 'minimax:', got {}",
        result.model
    );
}
