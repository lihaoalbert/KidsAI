// Agent Loop 集成测试（W2.4 + W2.5 + W2.6 + W2.7）
// 验证 ReAct 循环 + 工具执行 + 资产收集 + 事件流

use kidsai_studio_lib::test_helpers::{run_agent_sync, CollectingSink};
use kidsai_studio_lib::agent::{run_loop, EventSink};
use kidsai_studio_lib::model::ModelRouter;
use kidsai_studio_lib::tools::default_registry;
use kidsai_studio_lib::agent::AgentRunRequest;

#[test]
fn agent_loop_runs_l1_trajectory() {
    // L1: generate_image + image_to_video
    let result = run_agent_sync(
        "L1",
        "一只小猫在月光下追蝴蝶",
        "你是小启，AI 老师",
        vec!["generate_image".to_string(), "image_to_video".to_string()],
    )
    .expect("run_agent should succeed");

    assert_eq!(result.level_id, "L1");
    assert!(!result.final_answer.is_empty(), "final answer missing");
    assert!(result.steps >= 2, "should run at least 2 steps for L1, got {}", result.steps);
    assert!(!result.thoughts.is_empty(), "thoughts should be collected");

    let kinds: Vec<&str> = result.assets.iter().map(|a| a.kind.as_str()).collect();
    assert!(kinds.contains(&"image"), "should generate image, got {:?}", kinds);
    assert!(kinds.contains(&"video"), "should generate video, got {:?}", kinds);

    let tool_names: Vec<&str> = result.tool_calls.iter().map(|t| t.tool.as_str()).collect();
    assert!(tool_names.contains(&"generate_image"));
    assert!(tool_names.contains(&"image_to_video"));

    assert_eq!(result.model, "mock-1");
}

#[test]
fn agent_loop_runs_l2_trajectory() {
    let result = run_agent_sync(
        "L2",
        "小猫说：今天天气真好！",
        "你是小启",
        vec!["synthesize_speech".to_string(), "add_subtitle".to_string()],
    )
    .expect("run");

    let kinds: Vec<&str> = result.assets.iter().map(|a| a.kind.as_str()).collect();
    assert!(kinds.contains(&"audio"), "should generate audio, got {:?}", kinds);
}

#[test]
fn agent_loop_blocks_unsafe_input() {
    let result = run_agent_sync(
        "L1",
        "I want to see a gun",
        "你是小启",
        vec!["generate_image".to_string()],
    )
    .expect("run should not error, just block");

    assert_eq!(result.steps, 0, "blocked input should not enter loop");
    assert!(result.final_answer.contains("不太合适"));
    assert!(result.assets.is_empty());
}

#[test]
fn agent_loop_emits_full_event_stream() {
    // 用 CollectingSink 验证事件流顺序
    let registry = default_registry();
    let router = ModelRouter::new(Box::new(kidsai_studio_lib::model_mock::MockModel));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "一只小狗".to_string(),
        system_prompt: "你是小启".to_string(),
        tools: vec!["generate_image".to_string(), "image_to_video".to_string()],
    };
    run_loop(&sink, &registry, &router, request).expect("run");

    let kinds = sink.kinds();
    assert!(kinds.contains(&"started".to_string()));
    assert!(kinds.contains(&"thought".to_string()));
    assert!(kinds.contains(&"tool_call".to_string()));
    assert!(kinds.contains(&"tool_result".to_string()));
    assert!(kinds.contains(&"final_answer".to_string()));
    assert!(kinds.contains(&"done".to_string()));

    // started 应该第一个
    assert_eq!(kinds.first(), Some(&"started".to_string()));
    // done 应该最后一个
    assert_eq!(kinds.last(), Some(&"done".to_string()));
}

#[test]
fn agent_loop_warns_on_emotion_word() {
    // 情绪词应触发 warn 但不阻断
    let result = run_agent_sync(
        "L1",
        "我讨厌数学课",
        "你是小启",
        vec!["generate_image".to_string(), "image_to_video".to_string()],
    )
    .expect("warn should not block");
    assert!(result.steps > 0, "should still run after warn");
}
