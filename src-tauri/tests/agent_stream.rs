// Agent 流式 + 取消集成测试（W3.2）
// 验证 SSE chunks 发射 + Arc<AtomicBool> 取消信号

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use kidsai_studio_lib::agent::{AgentRunRequest, SessionRegistry};
use kidsai_studio_lib::model::ModelRouter;
use kidsai_studio_lib::model_mock::{MockConfig, MockModel};
use kidsai_studio_lib::test_helpers::CollectingSink;

/// 1. MockModel 发射 5 个 chunk + final_answer，断言事件流有 5× Chunk + 1× FinalAnswer
#[tokio::test]
async fn mock_emits_five_chunks_then_final_answer() {
    let model = MockModel::with_config(MockConfig {
        chunks: vec![
            "你".into(),
            "好".into(),
            "，".into(),
            "小".into(),
            "启".into(),
        ],
        final_answer: Some("你好，小启！".into()),
        tool_call: None,
        chunk_delay_ms: 0,
        cancel_flag: None,
    });
    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(model));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "test".to_string(),
        system_prompt: "test".to_string(),
        tools: vec!["generate_image".to_string()],
    };
    let result = kidsai_studio_lib::agent::run_loop(&sink, &registry, &router, request)
        .await
        .expect("run should succeed");

    // 5 个 chunk delta
    let deltas = sink.chunk_deltas();
    assert_eq!(deltas, vec!["你", "好", "，", "小", "启"]);

    // 事件顺序
    let kinds = sink.kinds();
    let chunk_count = kinds.iter().filter(|k| k.as_str() == "chunk").count();
    assert_eq!(chunk_count, 5, "should have 5 chunks, kinds: {:?}", kinds);

    // final_answer 出现
    assert!(kinds.contains(&"final_answer".to_string()));
    // final_answer 之后不应该再有 chunk
    let last_chunk_idx = kinds.iter().rposition(|k| k == "chunk").unwrap();
    let first_final_idx = kinds.iter().position(|k| k == "final_answer").unwrap();
    assert!(
        last_chunk_idx < first_final_idx,
        "no chunk should appear after final_answer (chunk@{last_chunk_idx} final@{first_final_idx})"
    );

    // done 仍是最后
    assert_eq!(kinds.last(), Some(&"done".to_string()));

    assert!(!result.cancelled);
    assert_eq!(result.final_answer, "你好，小启！");
}

/// 2. 取消 mid-stream：chunk_delay_ms=200,50ms 时 cancel → Cancelled 事件 + response.cancelled=true
#[tokio::test]
async fn cancel_mid_stream_emits_cancelled_event() {
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_for_spawn = cancel_flag.clone();

    let model = MockModel::with_config(MockConfig {
        chunks: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
        final_answer: Some("never reached".into()),
        tool_call: None,
        chunk_delay_ms: 200,
        cancel_flag: Some(cancel_flag.clone()),
    });
    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(model));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "test".to_string(),
        system_prompt: "test".to_string(),
        tools: vec!["generate_image".to_string()],
    };

    // 50ms 后翻转 cancel_flag
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        cancel_for_spawn.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    let result = kidsai_studio_lib::agent::run_loop(&sink, &registry, &router, request)
        .await
        .expect("run should return Ok with cancelled=true");

    // 收到的 chunk 应该少于 5（被中途打断了）
    let chunk_count = sink.kinds().iter().filter(|k| k.as_str() == "chunk").count();
    assert!(
        chunk_count < 5,
        "expected cancel to interrupt stream, got {} chunks",
        chunk_count
    );

    // 事件流含 cancelled
    let kinds = sink.kinds();
    assert!(
        kinds.contains(&"cancelled".to_string()),
        "kinds: {:?}",
        kinds
    );
    // done 仍在最后
    assert_eq!(kinds.last(), Some(&"done".to_string()));

    assert!(result.cancelled, "response should have cancelled=true");
}

/// 3. 取消在 step 之间：第一轮快完成（无 chunk），第二轮前 cancel
#[tokio::test]
async fn cancel_between_steps_emits_cancelled_event() {
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_for_spawn = cancel_flag.clone();

    // 第一轮：直接给 tool_call（无 chunks），触发 tool 执行
    // 第二轮：在 chunk 间检查 cancel
    let model = MockModel::with_config(MockConfig {
        chunks: vec!["should".into(), "not".into(), "reach".into()],
        final_answer: None,
        tool_call: Some(kidsai_studio_lib::model_openai::OaiToolCall {
            id: "call_test".to_string(),
            kind: "function".to_string(),
            function: kidsai_studio_lib::model_openai::OaiFunction {
                name: "generate_image".to_string(),
                arguments: r#"{"prompt":"test"}"#.to_string(),
            },
        }),
        chunk_delay_ms: 500,
        cancel_flag: Some(cancel_flag.clone()),
    });
    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(model));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "test".to_string(),
        system_prompt: "test".to_string(),
        tools: vec!["generate_image".to_string(), "image_to_video".to_string()],
    };

    // 300ms 后 cancel（在第一轮 tool 执行完之后、第二次 model 调用 chunk 期间）
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        cancel_for_spawn.store(true, std::sync::atomic::Ordering::Relaxed);
    });

    let result = kidsai_studio_lib::agent::run_loop(&sink, &registry, &router, request)
        .await
        .expect("run should return");

    let kinds = sink.kinds();
    assert!(
        kinds.contains(&"cancelled".to_string()),
        "should emit Cancelled event, kinds: {:?}",
        kinds
    );
    assert!(result.cancelled);
}

/// 4. 工具调用决策不产生 chunk 事件（因为 tool deltas 走 buffer）
#[tokio::test]
async fn tool_call_produces_no_chunks() {
    let model = MockModel::with_config(MockConfig {
        chunks: vec![],
        final_answer: None,
        tool_call: Some(kidsai_studio_lib::model_openai::OaiToolCall {
            id: "call_x".to_string(),
            kind: "function".to_string(),
            function: kidsai_studio_lib::model_openai::OaiFunction {
                name: "generate_image".to_string(),
                arguments: r#"{"prompt":"x"}"#.to_string(),
            },
        }),
        chunk_delay_ms: 0,
        cancel_flag: None,
    });
    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(model));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "test".to_string(),
        system_prompt: "test".to_string(),
        tools: vec!["generate_image".to_string()],
    };
    let result = kidsai_studio_lib::agent::run_loop(&sink, &registry, &router, request)
        .await
        .expect("run should succeed");

    let chunk_count = sink.kinds().iter().filter(|k| k.as_str() == "chunk").count();
    assert_eq!(chunk_count, 0, "tool calls should not emit chunks");
    assert!(!result.cancelled);
    // 工具执行了，所以有 image 资产
    let kinds: Vec<&str> = result.assets.iter().map(|a| a.kind.as_str()).collect();
    assert!(kinds.contains(&"image"), "should have image asset, got {:?}", kinds);
}
