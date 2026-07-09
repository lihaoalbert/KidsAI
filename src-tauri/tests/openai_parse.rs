// OpenAI 兼容响应解析测试
// 不发真实 HTTP，只验证 JSON 解析逻辑

use std::sync::Mutex;

use kidsai_studio_lib::model::Model;
use kidsai_studio_lib::model_openai::{parse_decision_from_response, ChatResponse, Choice};
use kidsai_studio_lib::test_helpers::run_agent_sync;

// 串行化"会动环境变量"的测试
static ENV_LOCK: Mutex<()> = Mutex::new(());


#[test]
fn mock_model_runs_l1_without_real_api() {
    // 没设任何 API key，应该自动走 mock
    let result = run_agent_sync(
        "L1",
        "一只小猫",
        "你是小启",
        vec!["generate_image".to_string(), "image_to_video".to_string()],
    )
    .expect("mock should run without API");
    assert_eq!(result.model, "mock-1");
    assert!(!result.assets.is_empty());
}

#[test]
fn model_factory_falls_back_to_mock_without_keys() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let saved = (
        std::env::var("MINIMAX_API_KEY").ok(),
        std::env::var("DEEPSEEK_API_KEY").ok(),
        std::env::var("OPENAI_API_KEY").ok(),
        std::env::var("DASHSCOPE_API_KEY").ok(),
    );
    unsafe {
        std::env::remove_var("MINIMAX_API_KEY");
        std::env::remove_var("DEEPSEEK_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("DASHSCOPE_API_KEY");
    }
    let selected = kidsai_studio_lib::model_factory::select_model();
    assert_eq!(selected.source, "mock");
    unsafe {
        if let Some(v) = saved.0 { std::env::set_var("MINIMAX_API_KEY", v); }
        if let Some(v) = saved.1 { std::env::set_var("DEEPSEEK_API_KEY", v); }
        if let Some(v) = saved.2 { std::env::set_var("OPENAI_API_KEY", v); }
        if let Some(v) = saved.3 { std::env::set_var("DASHSCOPE_API_KEY", v); }
    }
}

#[test]
fn model_factory_picks_minimax_when_key_set() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let saved_minimax = std::env::var("MINIMAX_API_KEY").ok();
    unsafe {
        std::env::set_var("MINIMAX_API_KEY", "eyJhbGciOiJIUzI1NiJ9.test");
        std::env::remove_var("DEEPSEEK_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
        std::env::remove_var("DASHSCOPE_API_KEY");
    }
    let selected = kidsai_studio_lib::model_factory::select_model();
    assert_eq!(selected.source, "minimax");
    assert!(selected.model.name().starts_with("minimax:"));
    unsafe {
        match saved_minimax {
            Some(v) => std::env::set_var("MINIMAX_API_KEY", v),
            None => std::env::remove_var("MINIMAX_API_KEY"),
        }
    }
}

#[test]
fn parses_deepseek_tool_call_response() {
    // DeepSeek 真实返回的格式示例
    let json = r#"{
        "id": "chatcmpl-abc123",
        "object": "chat.completion",
        "created": 1715000000,
        "model": "deepseek-chat",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "我先根据描述生成一张图",
                "tool_calls": [{
                    "id": "call_001",
                    "type": "function",
                    "function": {
                        "name": "generate_image",
                        "arguments": "{\"prompt\":\"一只小猫在月光下追蝴蝶\",\"style\":\"cartoon\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }],
        "usage": {"prompt_tokens": 120, "completion_tokens": 30, "total_tokens": 150}
    }"#;

    let resp: ChatResponse = serde_json::from_str(json).expect("parse response");
    let choice: &Choice = &resp.choices[0];
    let usage = resp.usage.as_ref();

    let decision = parse_decision_from_response(choice, usage);

    assert_eq!(decision.tool.as_deref(), Some("generate_image"));
    assert!(decision.final_answer.is_none());
    assert!(decision.thought.contains("生成一张图"));
    let args: serde_json::Value =
        serde_json::from_str(decision.tool_args.as_deref().unwrap()).unwrap();
    assert_eq!(args["prompt"], "一只小猫在月光下追蝴蝶");
    assert_eq!(args["style"], "cartoon");
    assert_eq!(decision.tokens_used, 150);
}

#[test]
fn parses_deepseek_final_answer_response() {
    let json = r#"{
        "id": "chatcmpl-xyz",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "🎉 太棒啦！你的作品完成啦！"
            },
            "finish_reason": "stop"
        }],
        "usage": {"total_tokens": 80}
    }"#;

    let resp: ChatResponse = serde_json::from_str(json).expect("parse");
    let decision = parse_decision_from_response(&resp.choices[0], resp.usage.as_ref());

    assert!(decision.tool.is_none());
    assert!(decision.tool_args.is_none());
    assert!(decision
        .final_answer
        .as_deref()
        .unwrap()
        .contains("太棒啦"));
    assert_eq!(decision.tokens_used, 80);
}

#[test]
fn parses_deepseek_no_usage() {
    let json = r#"{
        "choices": [{
            "message": {"role": "assistant", "content": "hi"}
        }]
    }"#;
    let resp: ChatResponse = serde_json::from_str(json).expect("parse");
    let decision = parse_decision_from_response(&resp.choices[0], resp.usage.as_ref());
    assert!(decision.tool.is_none());
    assert!(decision.tokens_used == 0); // 缺省视为 0
}

#[test]
fn openai_compatible_constructs_with_dummy_key() {
    // 不发请求，只验证构造和 name
    use kidsai_studio_lib::model_openai::OpenAiCompatible;
    let m = OpenAiCompatible::new("deepseek", "deepseek-chat", "https://api.deepseek.com", "sk-fake");
    assert_eq!(m.name(), "deepseek:deepseek-chat");
}
