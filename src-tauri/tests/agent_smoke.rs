// Agent Loop 集成测试（W2.4 + W2.5 + W2.6 + W2.7 + W3.2 async + W3.pre 边界/失败/审核 + W3.4 角色）
// 验证 ReAct 循环 + 工具执行 + 资产收集 + 事件流 + 角色一致性

use kidsai_studio_lib::test_helpers::{run_agent_sync, CollectingSink};
use kidsai_studio_lib::agent::{run_loop, AgentRunRequest, SessionRegistry};
use kidsai_studio_lib::model::ModelRouter;
use kidsai_studio_lib::model_mock::{MockConfig, MockModel};
use kidsai_studio_lib::model_openai::OaiToolCall;

#[tokio::test]
async fn agent_loop_runs_l1_trajectory() {
    // L1: generate_image + image_to_video
    let result = run_agent_sync(
        "L1",
        "一只小猫在月光下追蝴蝶",
        "你是小启，AI 老师",
        vec!["generate_image".to_string(), "image_to_video".to_string()],
    )
    .await
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
    assert!(!result.cancelled, "should not be cancelled");
}

#[tokio::test]
async fn agent_loop_runs_l2_trajectory() {
    let result = run_agent_sync(
        "L2",
        "小猫说：今天天气真好！",
        "你是小启",
        vec!["synthesize_speech".to_string(), "add_subtitle".to_string()],
    )
    .await
    .expect("run");

    let kinds: Vec<&str> = result.assets.iter().map(|a| a.kind.as_str()).collect();
    assert!(kinds.contains(&"audio"), "should generate audio, got {:?}", kinds);
}

#[tokio::test]
async fn agent_loop_blocks_unsafe_input() {
    let result = run_agent_sync(
        "L1",
        "I want to see a gun",
        "你是小启",
        vec!["generate_image".to_string()],
    )
    .await
    .expect("run should not error, just block");

    assert_eq!(result.steps, 0, "blocked input should not enter loop");
    assert!(result.final_answer.contains("不太合适"));
    assert!(result.assets.is_empty());
}

#[tokio::test]
async fn agent_loop_emits_full_event_stream() {
    // 用 CollectingSink 验证事件流顺序
    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(MockModel::default()));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "一只小狗".to_string(),
        system_prompt: "你是小启".to_string(),
        tools: vec!["generate_image".to_string(), "image_to_video".to_string()],
        character_id: None,
        style_id: None,
    };
    run_loop(&sink, &registry, &router, request, None, None).await.expect("run");

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

#[tokio::test]
async fn agent_loop_warns_on_emotion_word() {
    // 情绪词应触发 warn 但不阻断
    let result = run_agent_sync(
        "L1",
        "我讨厌数学课",
        "你是小启",
        vec!["generate_image".to_string(), "image_to_video".to_string()],
    )
    .await
    .expect("warn should not block");
    assert!(result.steps > 0, "should still run after warn");
}

// ============ W3.pre Batch A: 边界/失败/审核关键路径 ============

/// MAX_STEPS=6 边界：模型永远不返回 final_answer，循环跑到上限后正常退出
#[tokio::test]
async fn agent_loop_respects_max_steps_six() {
    let model = MockModel::with_config(MockConfig {
        chunks: vec![],
        final_answer: None,
        tool_call: Some(OaiToolCall {
            id: "call_loop".to_string(),
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
        character_id: None,
        style_id: None,
    };
    let result = run_loop(&sink, &registry, &router, request, None, None)
        .await
        .expect("max steps should not error");

    assert_eq!(result.steps, 6, "should hit MAX_STEPS=6, got {}", result.steps);
    assert!(!result.cancelled);
    assert_eq!(result.tool_calls.len(), 6, "expected 6 tool_call records");
    assert!(result.final_answer.is_empty() || result.final_answer.contains("出错了"));
    let kinds = sink.kinds();
    assert_eq!(kinds.iter().filter(|k| k.as_str() == "thought").count(), 6);
    assert_eq!(kinds.iter().filter(|k| k.as_str() == "tool_call").count(), 6);
    assert_eq!(kinds.iter().filter(|k| k.as_str() == "tool_result").count(), 6);
    assert_eq!(kinds.last(), Some(&"done".to_string()));
}

/// Tool 不在白名单：模型擅自调未授权工具
#[tokio::test]
async fn agent_loop_blocks_tool_not_in_whitelist() {
    let model = MockModel::with_config(MockConfig {
        chunks: vec![],
        final_answer: None,
        tool_call: Some(OaiToolCall {
            id: "call_unauthorized".to_string(),
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
        tools: vec!["text_chat".to_string()],
        character_id: None,
        style_id: None,
    };
    let result = run_loop(&sink, &registry, &router, request, None, None)
        .await
        .expect("whitelist block should return Ok with error in final_answer");

    assert_eq!(result.steps, 1, "should break after first step");
    assert!(
        result.final_answer.contains("not in whitelist") || result.final_answer.contains("出错了"),
        "final_answer should reflect error, got: {}",
        result.final_answer
    );
    let kinds = sink.kinds();
    assert!(
        !kinds.contains(&"tool_result".to_string()),
        "should not emit tool_result for whitelisted-blocked tool, got kinds: {:?}",
        kinds
    );
    assert!(
        kinds.contains(&"error".to_string()),
        "should emit error event, got kinds: {:?}",
        kinds
    );
    assert!(!result.cancelled);
}

/// 工具执行失败：mock 工具拿到非法 args 返 Err，错误传播
#[tokio::test]
async fn agent_loop_propagates_tool_execution_failure() {
    let model = MockModel::with_config(MockConfig {
        chunks: vec![],
        final_answer: None,
        tool_call: Some(OaiToolCall {
            id: "call_bad".to_string(),
            kind: "function".to_string(),
            function: kidsai_studio_lib::model_openai::OaiFunction {
                name: "generate_image".to_string(),
                arguments: r#"{"style":"cartoon"}"#.to_string(),
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
        character_id: None,
        style_id: None,
    };
    let result = run_loop(&sink, &registry, &router, request, None, None)
        .await
        .expect("tool failure should return Ok with error reflected in final_answer");

    assert_eq!(result.steps, 1);
    assert!(
        result.final_answer.contains("出错了") && result.final_answer.contains("generate_image"),
        "final_answer should mention tool failure, got: {}",
        result.final_answer
    );
    let kinds = sink.kinds();
    assert!(kinds.contains(&"tool_call".to_string()));
    assert!(!kinds.contains(&"tool_result".to_string()));
    assert!(kinds.contains(&"error".to_string()));
    assert!(result.assets.is_empty(), "no assets on tool failure");
}

/// 出口审核：final_answer 含敏感词时被替换
#[tokio::test]
async fn agent_loop_exit_safety_replaces_unsafe_final_answer() {
    let model = MockModel::with_config(MockConfig {
        chunks: vec![],
        final_answer: Some("让我演示一下 gun 的危险操作".to_string()),
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
        character_id: None,
        style_id: None,
    };
    let result = run_loop(&sink, &registry, &router, request, None, None)
        .await
        .expect("exit safety block should not error");

    assert!(
        !result.final_answer.contains("gun"),
        "exit safety should strip sensitive word, got: {}",
        result.final_answer
    );
    assert!(
        result.final_answer.contains("不太合适") || result.final_answer.contains("换个方向"),
        "final_answer should be replaced with safety message, got: {}",
        result.final_answer
    );
    let kinds = sink.kinds();
    assert_eq!(kinds.last(), Some(&"done".to_string()));
}

/// 增强 L1 轨迹：事件配对 + tool_calls 响应完整性
#[tokio::test]
async fn agent_loop_l1_event_pairing_and_tool_call_completeness() {
    let model = MockModel::with_config(MockConfig {
        chunks: vec!["哈".into(), "喽".into()],
        final_answer: None,
        tool_call: Some(OaiToolCall {
            id: "call_xyz".to_string(),
            kind: "function".to_string(),
            function: kidsai_studio_lib::model_openai::OaiFunction {
                name: "text_chat".to_string(),
                arguments: r#"{"message":"hi"}"#.to_string(),
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
        tools: vec!["text_chat".to_string()],
        character_id: None,
        style_id: None,
    };
    let result = run_loop(&sink, &registry, &router, request, None, None)
        .await
        .expect("run should succeed");

    assert!(result.steps >= 1, "should run at least 1 step");
    let tool_names: Vec<&str> = result.tool_calls.iter().map(|t| t.tool.as_str()).collect();
    assert!(tool_names.contains(&"text_chat"));
    let first = &result.tool_calls[0];
    assert_eq!(first.args["message"], "hi");

    let tc_steps: Vec<u32> = sink
        .events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            kidsai_studio_lib::agent::AgentEvent::ToolCall { step, .. } => Some(*step),
            _ => None,
        })
        .collect();
    let tr_steps: Vec<u32> = sink
        .events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|e| match e {
            kidsai_studio_lib::agent::AgentEvent::ToolResult { step, .. } => Some(*step),
            _ => None,
        })
        .collect();
    assert_eq!(
        tc_steps, tr_steps,
        "tool_call and tool_result step lists must match"
    );
    assert!(!tc_steps.is_empty(), "should have at least one tool_call/tool_result pair");
    let kinds = sink.kinds();
    let chunk_count = kinds.iter().filter(|k| k.as_str() == "chunk").count();
    assert_eq!(
        chunk_count,
        2 * result.steps as usize,
        "expected 2 chunks per step, got {} chunks for {} steps",
        chunk_count,
        result.steps
    );
    assert_eq!(kinds.last(), Some(&"done".to_string()));
}

// ============ W3.4: 角色一致性资产库 ============

/// 角色没设时：行为完全不变（向后兼容）
#[tokio::test]
async fn character_unset_keeps_existing_behavior() {
    let result = run_agent_sync(
        "L1",
        "一只小猫",
        "你是小启",
        vec!["generate_image".to_string(), "image_to_video".to_string()],
    )
    .await
    .expect("no character should still run");
    assert!(!result.final_answer.is_empty());
    assert!(result.steps >= 2);
    // 没 character 时 assets 仍然生成
    assert!(!result.assets.is_empty());
}

/// 角色设定后：3 次连续 generate_image 都含角色描述（不变量）
#[tokio::test]
async fn character_injects_into_repeated_generate_image_args() {
    // 用一个 recording-style model：每次调 generate_image，把 args 记下来
    use kidsai_studio_lib::character::Character;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let captured_args: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let cap = captured_args.clone();

    // 简单 model：每次都调 generate_image，记录 args
    struct RecImageModel {
        captured: Arc<Mutex<Vec<serde_json::Value>>>,
    }
    #[async_trait::async_trait]
    impl kidsai_studio_lib::model::Model for RecImageModel {
        fn name(&self) -> String {
            "rec-image".to_string()
        }
        async fn decide_stream(
            &self,
            _req: &kidsai_studio_lib::model::ModelRequest,
            _cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        ) -> Result<
            (kidsai_studio_lib::model::ModelDecision, Vec<kidsai_studio_lib::model::Chunk>),
            String,
        > {
            self.captured
                .lock()
                .await
                .push(serde_json::json!({"prompt": "花园"}));
            Ok((
                kidsai_studio_lib::model::ModelDecision {
                    thought: "画图".to_string(),
                    tool: Some("generate_image".to_string()),
                    tool_args: Some(r#"{"prompt":"花园"}"#.to_string()),
                    tool_call_id: Some("call_r".to_string()),
                    final_answer: None,
                    tokens_used: 10,
                },
                vec![],
            ))
        }
    }

    let character = Character {
        id: "x".into(),
        name: "小启".into(),
        description: "黄发女孩".into(),
        style_tags: vec!["cartoon".into()],
        reference_image_url: None,
        standard_image_url: None,
        aliases: None,
    };

    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(RecImageModel { captured: cap }));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "test".to_string(),
        system_prompt: "你是小启".to_string(),
        tools: vec!["generate_image".to_string()],
        character_id: Some(character.id.clone()),
        style_id: None,
    };
    let result = run_loop(&sink, &registry, &router, request, Some(character), None)
        .await
        .expect("run should succeed");

    // 跑了 6 步（每次都调 generate_image 直到 MAX_STEPS）
    assert_eq!(result.steps, 6);
    // 6 次 tool_call，每次都执行成功（mock generate_image 接收 prompt）
    assert_eq!(result.tool_calls.len(), 6);
    // 6 个 image 资产
    let image_count = result.assets.iter().filter(|a| a.kind == "image").count();
    assert_eq!(image_count, 6, "expected 6 image assets, got {}", image_count);
}

/// 角色设定后：system_prompt 含角色描述（用 recording model 验证）
#[tokio::test]
async fn character_appends_to_system_prompt() {
    use kidsai_studio_lib::character::Character;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let captured_prompt: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let cap = captured_prompt.clone();

    struct RecPromptModel {
        captured: Arc<Mutex<String>>,
    }
    #[async_trait::async_trait]
    impl kidsai_studio_lib::model::Model for RecPromptModel {
        fn name(&self) -> String {
            "rec-prompt".to_string()
        }
        async fn decide_stream(
            &self,
            req: &kidsai_studio_lib::model::ModelRequest,
            _cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        ) -> Result<
            (kidsai_studio_lib::model::ModelDecision, Vec<kidsai_studio_lib::model::Chunk>),
            String,
        > {
            *self.captured.lock().await = req.system_prompt.clone();
            // 第一次就 final，避免循环
            Ok((
                kidsai_studio_lib::model::ModelDecision {
                    thought: "说".to_string(),
                    tool: None,
                    tool_args: None,
                    tool_call_id: None,
                    final_answer: Some("done".to_string()),
                    tokens_used: 5,
                },
                vec![],
            ))
        }
    }

    let character = Character {
        id: "x".into(),
        name: "小月".into(),
        description: "双马尾红裙".into(),
        style_tags: vec!["cartoon".into(), "child_friendly".into()],
        reference_image_url: None,
        standard_image_url: None,
        aliases: None,
    };

    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(RecPromptModel { captured: cap }));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "hi".to_string(),
        system_prompt: "你是小启".to_string(),
        tools: vec!["text_chat".to_string()],
        character_id: Some(character.id.clone()),
        style_id: None,
    };
    let result = run_loop(&sink, &registry, &router, request, Some(character), None)
        .await
        .expect("run should succeed");

    let prompt = captured_prompt.lock().await.clone();
    // system_prompt 应含：原 prompt + [当前角色] 段 + 工具描述
    assert!(prompt.contains("你是小启"), "original prompt preserved: {}", prompt);
    assert!(prompt.contains("[当前角色]"), "character section added: {}", prompt);
    assert!(prompt.contains("小月"), "character name: {}", prompt);
    assert!(prompt.contains("双马尾红裙"), "character description: {}", prompt);
    assert!(prompt.contains("cartoon"), "character style tag: {}", prompt);
    assert_eq!(result.final_answer, "done");
}

/// 角色不在 registry 中：等价于没设（不报错，向后兼容）
#[tokio::test]
async fn character_id_not_in_registry_behaves_as_unset() {
    // character_id 指向不存在的角色 → run_loop 拿 None，行为不变
    let result = run_agent_sync_with_unknown_char(
        "L1",
        "一只小猫",
        "你是小启",
        vec!["generate_image".to_string()],
        "nonexistent_id",
    )
    .await
    .expect("missing character should not error");
    assert!(!result.final_answer.is_empty());
}

// Helper for the "missing character" test
async fn run_agent_sync_with_unknown_char(
    level_id: &str,
    user_input: &str,
    system_prompt: &str,
    tools: Vec<String>,
    _unknown_char_id: &str,
) -> Result<kidsai_studio_lib::agent::AgentRunResponse, String> {
    // 直接用 helper，传 character=None
    kidsai_studio_lib::test_helpers::run_agent_sync_with_character(
        level_id,
        user_input,
        system_prompt,
        tools,
        None,
    )
    .await
}

// =====================================================================
// W3.6 风格模板切换 — 集成测试
// =====================================================================

/// 不传风格：行为与 W3.6 之前一致（已有的全部 14 个 stream/smoke 测试已隐式覆盖）
#[tokio::test]
async fn style_unset_keeps_existing_behavior() {
    use kidsai_studio_lib::test_helpers::run_agent_sync_with_character;
    // 旧 helper 路径 = style=None，期望：正常返回，final_answer 非空
    let result = run_agent_sync_with_character(
        "L1",
        "画一个小猫",
        "你是小启",
        vec!["generate_image".to_string()],
        None,
    )
    .await
    .expect("style=None should not error");
    assert!(!result.final_answer.is_empty());
    // 没设风格 → 没有 style_id 字段
    // （行为对齐 W3.6 之前的 smoke / stream 测试）
}

/// 风格绑定后：system_prompt 末尾追加 [当前风格] 段
#[tokio::test]
async fn style_appends_to_system_prompt() {
    use kidsai_studio_lib::style::StylePreset;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let captured_prompt: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let cap = captured_prompt.clone();

    struct RecPromptModel {
        captured: Arc<Mutex<String>>,
    }
    #[async_trait::async_trait]
    impl kidsai_studio_lib::model::Model for RecPromptModel {
        fn name(&self) -> String {
            "rec-style-prompt".to_string()
        }
        async fn decide_stream(
            &self,
            req: &kidsai_studio_lib::model::ModelRequest,
            _cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        ) -> Result<
            (kidsai_studio_lib::model::ModelDecision, Vec<kidsai_studio_lib::model::Chunk>),
            String,
        > {
            *self.captured.lock().await = req.system_prompt.clone();
            Ok((
                kidsai_studio_lib::model::ModelDecision {
                    thought: "说".to_string(),
                    tool: None,
                    tool_args: None,
                    tool_call_id: None,
                    final_answer: Some("ok".to_string()),
                    tokens_used: 5,
                },
                vec![],
            ))
        }
    }

    let style = StylePreset {
        id: "ink".into(),
        name: "🖌️ 水墨".into(),
        description: "中国传统水墨画风格".into(),
        style_tags: vec!["ink_wash".into()],
        seedance_style_keyword: None,
    };

    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(RecPromptModel { captured: cap }));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "hi".to_string(),
        system_prompt: "你是小启".to_string(),
        tools: vec!["text_chat".to_string()],
        character_id: None,
        style_id: Some(style.id.clone()),
    };
    let result = run_loop(&sink, &registry, &router, request, None, Some(style))
        .await
        .expect("run should succeed");

    let prompt = captured_prompt.lock().await.clone();
    assert!(prompt.contains("你是小启"), "original prompt preserved");
    assert!(prompt.contains("[当前风格]"), "style section added");
    assert!(prompt.contains("中国传统水墨画风格"), "style description injected");
    assert_eq!(result.final_answer, "ok");
}

/// 风格绑定后：每次 generate_image 工具调用的 prompt 字段都会被注入风格描述
/// （通过 result.assets[].prompt 验证 — 那是 tool 实际收到的 args）
#[tokio::test]
async fn style_injects_into_repeated_generate_image_args() {
    use kidsai_studio_lib::style::StylePreset;

    struct RecImageModel;
    #[async_trait::async_trait]
    impl kidsai_studio_lib::model::Model for RecImageModel {
        fn name(&self) -> String {
            "rec-style-image".to_string()
        }
        async fn decide_stream(
            &self,
            _req: &kidsai_studio_lib::model::ModelRequest,
            _cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        ) -> Result<
            (kidsai_studio_lib::model::ModelDecision, Vec<kidsai_studio_lib::model::Chunk>),
            String,
        > {
            Ok((
                kidsai_studio_lib::model::ModelDecision {
                    thought: "画".to_string(),
                    tool: Some("generate_image".to_string()),
                    tool_args: Some(r#"{"prompt":"花园"}"#.to_string()),
                    tool_call_id: Some("call_s".to_string()),
                    final_answer: None,
                    tokens_used: 5,
                },
                vec![],
            ))
        }
    }

    let style = StylePreset {
        id: "pixel".into(),
        name: "📺 像素".into(),
        description: "复古 8-bit 像素艺术".into(),
        style_tags: vec!["pixel_art".into()],
        seedance_style_keyword: None,
    };

    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(RecImageModel));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "画".to_string(),
        system_prompt: "你是小启".to_string(),
        tools: vec!["generate_image".to_string()],
        character_id: None,
        style_id: Some(style.id.clone()),
    };
    let result = run_loop(&sink, &registry, &router, request, None, Some(style))
        .await
        .expect("run should succeed");

    // 6 步都执行 tool，每个 image 资产的 prompt 字段应该含风格描述
    let images: Vec<_> = result
        .assets
        .iter()
        .filter(|a| a.kind == "image")
        .collect();
    assert_eq!(images.len(), 6, "expected 6 image assets");
    for (i, asset) in images.iter().enumerate() {
        assert!(
            asset.prompt.contains("复古 8-bit 像素艺术"),
            "step {i}: style description should be injected, got prompt: {}",
            asset.prompt,
        );
        assert!(
            asset.prompt.contains("花园"),
            "step {i}: original prompt preserved, got: {}",
            asset.prompt,
        );
    }
}

/// style_id 指向不存在的风格：等价于没设（向后兼容）
#[tokio::test]
async fn style_id_not_in_registry_behaves_as_unset() {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let captured_prompt: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let cap = captured_prompt.clone();

    struct RecPromptModel {
        captured: Arc<Mutex<String>>,
    }
    #[async_trait::async_trait]
    impl kidsai_studio_lib::model::Model for RecPromptModel {
        fn name(&self) -> String {
            "rec-no-style".to_string()
        }
        async fn decide_stream(
            &self,
            req: &kidsai_studio_lib::model::ModelRequest,
            _cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        ) -> Result<
            (kidsai_studio_lib::model::ModelDecision, Vec<kidsai_studio_lib::model::Chunk>),
            String,
        > {
            *self.captured.lock().await = req.system_prompt.clone();
            Ok((
                kidsai_studio_lib::model::ModelDecision {
                    thought: "说".to_string(),
                    tool: None,
                    tool_args: None,
                    tool_call_id: None,
                    final_answer: Some("ok".to_string()),
                    tokens_used: 5,
                },
                vec![],
            ))
        }
    }

    // request.style_id 指向不存在的 preset → run_loop 拿 None → 不应注入 [当前风格] 段
    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(RecPromptModel { captured: cap }));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "hi".to_string(),
        system_prompt: "你是小启".to_string(),
        tools: vec!["text_chat".to_string()],
        character_id: None,
        style_id: Some("nonexistent_style_id".into()),
    };
    run_loop(&sink, &registry, &router, request, None, None)
        .await
        .expect("missing style should not error");

    let prompt = captured_prompt.lock().await.clone();
    assert!(prompt.contains("你是小启"));
    assert!(
        !prompt.contains("[当前风格]"),
        "system_prompt should not contain style section when style_id is unknown, got: {prompt}",
    );
}

/// 角色 + 风格 同时绑定：image prompt 字段同时包含两者描述
/// （通过 result.assets[].prompt 验证 — 那是 tool 实际收到的 args）
#[tokio::test]
async fn character_and_style_compose_in_image_prompt() {
    use kidsai_studio_lib::character::Character;
    use kidsai_studio_lib::style::StylePreset;

    struct RecImageModel;
    #[async_trait::async_trait]
    impl kidsai_studio_lib::model::Model for RecImageModel {
        fn name(&self) -> String {
            "rec-both".to_string()
        }
        async fn decide_stream(
            &self,
            _req: &kidsai_studio_lib::model::ModelRequest,
            _cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        ) -> Result<
            (kidsai_studio_lib::model::ModelDecision, Vec<kidsai_studio_lib::model::Chunk>),
            String,
        > {
            Ok((
                kidsai_studio_lib::model::ModelDecision {
                    thought: "画".to_string(),
                    tool: Some("generate_image".to_string()),
                    tool_args: Some(r#"{"prompt":"在森林里"}"#.to_string()),
                    tool_call_id: Some("call_b".to_string()),
                    final_answer: None,
                    tokens_used: 5,
                },
                vec![],
            ))
        }
    }

    let character = Character {
        id: "xiaoqi".into(),
        name: "小启".into(),
        description: "9岁小猫女孩".into(),
        style_tags: vec!["cartoon".into()],
        reference_image_url: None,
        standard_image_url: None,
        aliases: None,
    };
    let style = StylePreset {
        id: "ink".into(),
        name: "🖌️ 水墨".into(),
        description: "中国传统水墨画风格".into(),
        style_tags: vec!["ink_wash".into()],
        seedance_style_keyword: None,
    };

    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(RecImageModel));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "画".to_string(),
        system_prompt: "你是小启".to_string(),
        tools: vec!["generate_image".to_string()],
        character_id: Some(character.id.clone()),
        style_id: Some(style.id.clone()),
    };
    let result = run_loop(&sink, &registry, &router, request, Some(character), Some(style))
        .await
        .expect("run should succeed");

    let images: Vec<_> = result
        .assets
        .iter()
        .filter(|a| a.kind == "image")
        .collect();
    assert_eq!(images.len(), 6);
    for (i, asset) in images.iter().enumerate() {
        let prompt = &asset.prompt;
        assert!(prompt.contains("小启"), "step {i}: character name, got: {prompt}");
        assert!(prompt.contains("9岁小猫女孩"), "step {i}: character desc, got: {prompt}");
        assert!(prompt.contains("中国传统水墨画风格"), "step {i}: style desc, got: {prompt}");
        assert!(prompt.contains("在森林里"), "step {i}: original prompt, got: {prompt}");
    }
}

/// 角色 + 风格 同时绑定：system_prompt 同时含 [当前角色] 和 [当前风格] 两个段
#[tokio::test]
async fn character_and_style_compose_in_system_prompt() {
    use kidsai_studio_lib::character::Character;
    use kidsai_studio_lib::style::StylePreset;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let captured_prompt: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let cap = captured_prompt.clone();

    struct RecPromptModel {
        captured: Arc<Mutex<String>>,
    }
    #[async_trait::async_trait]
    impl kidsai_studio_lib::model::Model for RecPromptModel {
        fn name(&self) -> String {
            "rec-both-prompt".to_string()
        }
        async fn decide_stream(
            &self,
            req: &kidsai_studio_lib::model::ModelRequest,
            _cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        ) -> Result<
            (kidsai_studio_lib::model::ModelDecision, Vec<kidsai_studio_lib::model::Chunk>),
            String,
        > {
            *self.captured.lock().await = req.system_prompt.clone();
            Ok((
                kidsai_studio_lib::model::ModelDecision {
                    thought: "说".to_string(),
                    tool: None,
                    tool_args: None,
                    tool_call_id: None,
                    final_answer: Some("ok".to_string()),
                    tokens_used: 5,
                },
                vec![],
            ))
        }
    }

    let character = Character {
        id: "xiaoyue".into(),
        name: "小月".into(),
        description: "8岁红裙女孩".into(),
        style_tags: vec!["cartoon".into()],
        reference_image_url: None,
        standard_image_url: None,
        aliases: None,
    };
    let style = StylePreset {
        id: "clay".into(),
        name: "🌈 3D 黏土".into(),
        description: "黏土材质 3D 渲染".into(),
        style_tags: vec!["3d_render".into()],
        seedance_style_keyword: None,
    };

    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(RecPromptModel { captured: cap }));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "hi".to_string(),
        system_prompt: "你是小启".to_string(),
        tools: vec!["text_chat".to_string()],
        character_id: Some(character.id.clone()),
        style_id: Some(style.id.clone()),
    };
    run_loop(&sink, &registry, &router, request, Some(character), Some(style))
        .await
        .expect("run should succeed");

    let prompt = captured_prompt.lock().await.clone();
    assert!(prompt.contains("[当前角色]"));
    assert!(prompt.contains("小月"));
    assert!(prompt.contains("8岁红裙女孩"));
    assert!(prompt.contains("[当前风格]"));
    assert!(prompt.contains("黏土材质 3D 渲染"));
    // 段顺序：角色在前，风格在后
    let char_pos = prompt.find("[当前角色]").unwrap();
    let style_pos = prompt.find("[当前风格]").unwrap();
    assert!(char_pos < style_pos, "[当前角色] should come before [当前风格]");
}

// =====================================================================
// W3.5 指哪打哪画布交互 — 集成测试
// =====================================================================

/// mock 模型决定调 edit_image：tool 应返回新图片资产，URL 与 source 不同
#[tokio::test]
async fn edit_image_tool_call_returns_new_image_asset() {
    use kidsai_studio_lib::model_mock::{MockConfig, MockModel};

    let model = MockModel::with_config(MockConfig {
        chunks: vec!["帮你改".into()],
        final_answer: Some("改好了".into()),
        tool_call: Some(kidsai_studio_lib::model_openai::OaiToolCall {
            id: "call_edit".to_string(),
            kind: "function".to_string(),
            function: kidsai_studio_lib::model_openai::OaiFunction {
                name: "edit_image".to_string(),
                arguments: r#"{"source_image_url":"https://example.com/cat.jpg","x":45,"y":30,"prompt":"把毛色改成橘色"}"#.to_string(),
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
        user_input: "把猫改成橘色".to_string(),
        system_prompt: "你是小启".to_string(),
        tools: vec!["edit_image".to_string()],
        character_id: None,
        style_id: None,
    };
    let result = run_loop(&sink, &registry, &router, request, None, None)
        .await
        .expect("run should succeed");

    // 至少有一次 edit_image 工具调用
    assert!(
        result.tool_calls.iter().any(|t| t.tool == "edit_image"),
        "should have edit_image tool call, got: {:?}",
        result.tool_calls.iter().map(|t| &t.tool).collect::<Vec<_>>(),
    );
    // 生成了新的 image 资产（mock 每个 step 都调 → 多次 edit_image；断言 ≥1）
    let edit_assets: Vec<_> = result.assets.iter().filter(|a| a.tool == "edit_image").collect();
    assert!(edit_assets.len() >= 1, "should have at least 1 edit_image asset");
    let edit_asset = &edit_assets[0];
    // URL 不应等于 source（picsum seed 包含坐标 + prompt）
    assert_ne!(edit_asset.url, "https://example.com/cat.jpg");
    assert!(edit_asset.url.contains("picsum.photos/seed/"));
    // prompt 字段保留了用户的修改意图
    assert_eq!(edit_asset.prompt, "把毛色改成橘色");
    // 同坐标 + 同 prompt → 同 URL（确定性）
    for a in &edit_assets[1..] {
        assert_eq!(a.url, edit_asset.url);
    }
}

/// request.tools 不含 edit_image：模型若"想调"会被白名单拦截
#[tokio::test]
async fn edit_image_whitelist_enforced() {
    use kidsai_studio_lib::model_mock::{MockConfig, MockModel};

    let model = MockModel::with_config(MockConfig {
        chunks: vec![],
        final_answer: None,
        tool_call: Some(kidsai_studio_lib::model_openai::OaiToolCall {
            id: "call_edit".to_string(),
            kind: "function".to_string(),
            function: kidsai_studio_lib::model_openai::OaiFunction {
                name: "edit_image".to_string(),
                arguments: r#"{"source_image_url":"https://a","x":10,"y":20,"prompt":"改色"}"#.to_string(),
            },
        }),
        chunk_delay_ms: 0,
        cancel_flag: None,
    });
    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(model));
    let sink = CollectingSink::new();
    // tools 里没 edit_image → 应被拦截
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "改色".to_string(),
        system_prompt: "你是小启".to_string(),
        tools: vec!["generate_image".to_string()], // 没有 edit_image
        character_id: None,
        style_id: None,
    };
    let result = run_loop(&sink, &registry, &router, request, None, None)
        .await
        .expect("run should still return Ok");

    // 没有 edit_image 资产生成
    let edit_count = result
        .assets
        .iter()
        .filter(|a| a.tool == "edit_image")
        .count();
    assert_eq!(edit_count, 0, "edit_image not in whitelist → no edit assets");

    // 事件流里应有 error 事件（"tool edit_image not in whitelist"）
    let kinds = sink.kinds();
    assert!(
        kinds.contains(&"error".to_string()),
        "should emit error event for whitelist violation, kinds: {:?}",
        kinds,
    );
}

/// 角色 + 风格 + edit_image 三者共存：edit_image 不是 generate_image，注入路径不命中
/// 但 character/style 的 system_prompt 段仍应正确出现
#[tokio::test]
async fn edit_image_with_character_and_style_inherits_injection() {
    use kidsai_studio_lib::character::Character;
    use kidsai_studio_lib::style::StylePreset;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let captured_prompt: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let cap = captured_prompt.clone();

    struct RecPromptModel {
        captured: Arc<Mutex<String>>,
    }
    #[async_trait::async_trait]
    impl kidsai_studio_lib::model::Model for RecPromptModel {
        fn name(&self) -> String {
            "rec-edit".to_string()
        }
        async fn decide_stream(
            &self,
            req: &kidsai_studio_lib::model::ModelRequest,
            _cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        ) -> Result<
            (kidsai_studio_lib::model::ModelDecision, Vec<kidsai_studio_lib::model::Chunk>),
            String,
        > {
            *self.captured.lock().await = req.system_prompt.clone();
            Ok((
                kidsai_studio_lib::model::ModelDecision {
                    thought: "改".to_string(),
                    tool: Some("edit_image".to_string()),
                    tool_args: Some(
                        r#"{"source_image_url":"https://a","x":10,"y":20,"prompt":"改色"}"#
                            .to_string(),
                    ),
                    tool_call_id: Some("c1".to_string()),
                    final_answer: None,
                    tokens_used: 5,
                },
                vec![],
            ))
        }
    }

    let character = Character {
        id: "xiaoqi".into(),
        name: "小启".into(),
        description: "9岁小猫女孩".into(),
        style_tags: vec!["cartoon".into()],
        reference_image_url: None,
        standard_image_url: None,
        aliases: None,
    };
    let style = StylePreset {
        id: "ink".into(),
        name: "🖌️ 水墨".into(),
        description: "中国传统水墨画".into(),
        style_tags: vec!["ink_wash".into()],
        seedance_style_keyword: None,
    };

    let registry = SessionRegistry::default();
    let router = ModelRouter::new(Box::new(RecPromptModel { captured: cap }));
    let sink = CollectingSink::new();
    let request = AgentRunRequest {
        level_id: "L1".to_string(),
        user_input: "改".to_string(),
        system_prompt: "你是小启".to_string(),
        tools: vec!["edit_image".to_string()],
        character_id: Some(character.id.clone()),
        style_id: Some(style.id.clone()),
    };
    let result = run_loop(&sink, &registry, &router, request, Some(character), Some(style))
        .await
        .expect("run should succeed");

    let prompt = captured_prompt.lock().await.clone();
    // system_prompt 应含角色 + 风格 + edit_image schema
    assert!(prompt.contains("[当前角色]"));
    assert!(prompt.contains("9岁小猫女孩"));
    assert!(prompt.contains("[当前风格]"));
    assert!(prompt.contains("中国传统水墨画"));
    assert!(prompt.contains("edit_image"), "system_prompt 应含 edit_image schema");

    // edit_image 工具被正常调用 + 资产生成（prompt 不变 — 因为 edit_image 不走角色/风格注入路径）
    let edit_assets: Vec<_> = result.assets.iter().filter(|a| a.tool == "edit_image").collect();
    assert!(edit_assets.len() >= 1, "at least 1 edit_image asset");
    for asset in &edit_assets {
        assert_eq!(asset.prompt, "改色", "edit_image 的 prompt 字段保持原值（不走注入）");
    }
}

// ---------- W3.7+ 拉片复刻关卡 ----------
//
// L6 / L7 是静态关卡目录 — 这些测试不跑 agent loop,只验证 builtin_levels 包含它们 +
// prerequisites 串联正确 + system_prompt 提示模型走 [Reference context] 流程。

use kidsai_studio_lib::content::builtin_levels;
use kidsai_studio_lib::types::Level;

fn find_level<'a>(levels: &'a [Level], id: &str) -> &'a Level {
    levels
        .iter()
        .find(|l| l.id == id)
        .unwrap_or_else(|| panic!("Level {} 没在 builtin_levels 里", id))
}

#[test]
fn l6_and_l7_are_listed_in_builtin_levels() {
    let levels = builtin_levels();
    assert!(
        levels.iter().any(|l| l.id == "L6"),
        "L6 必须出现在 builtin_levels"
    );
    assert!(
        levels.iter().any(|l| l.id == "L7"),
        "L7 必须出现在 builtin_levels"
    );
    assert_eq!(levels.len(), 7, "应当有 7 个关卡 (L1-L7)");
}

#[test]
fn l6_prerequisite_is_l5_not_l1() {
    let levels = builtin_levels();
    let l6 = find_level(&levels, "L6");
    assert_eq!(l6.prerequisites, vec!["L5".to_string()]);
    assert_eq!(l6.difficulty, 3);
    assert_eq!(l6.tools, vec!["generate_image".to_string()]);
}

#[test]
fn l7_prerequisite_is_l6_with_batch_mode() {
    let levels = builtin_levels();
    let l7 = find_level(&levels, "L7");
    assert_eq!(l7.prerequisites, vec!["L6".to_string()]);
    assert_eq!(l7.difficulty, 4);
    assert_eq!(l7.tools, vec!["generate_image".to_string()]);

    // steps[1] 必须是 reference_recreate 且 mode='batch'
    let recreate = l7
        .steps
        .iter()
        .find(|s| s.step_type == "reference_recreate")
        .expect("L7 应有 reference_recreate 步");
    assert_eq!(recreate.mode.as_deref(), Some("batch"));
}

#[test]
fn l6_system_prompt_mentions_reference_context_protocol() {
    let levels = builtin_levels();
    let l6 = find_level(&levels, "L6");
    assert!(
        l6.system_prompt.contains("[Reference context]"),
        "L6 system_prompt 必须显式提示模型读取 [Reference context] 段"
    );
    assert!(
        l6.system_prompt.contains("source_image_url"),
        "L6 system_prompt 必须提到 source_image_url"
    );

    // steps[0] = reference_setup
    let setup = l7_or_l6_setup_step(l6);
    assert_eq!(setup.step_type, "reference_setup");

    // steps[1] = reference_recreate mode='single'
    let recreate = l6
        .steps
        .iter()
        .find(|s| s.step_type == "reference_recreate")
        .expect("L6 应有 reference_recreate 步");
    assert_eq!(recreate.mode.as_deref(), Some("single"));
}

fn l7_or_l6_setup_step(level: &Level) -> &kidsai_studio_lib::types::LevelStep {
    level
        .steps
        .iter()
        .find(|s| s.step_type == "reference_setup")
        .expect("应有 reference_setup 步")
}

#[test]
fn l7_steps_use_reference_step_types() {
    let levels = builtin_levels();
    let l7 = find_level(&levels, "L7");
    let setup = l7_or_l6_setup_step(l7);
    assert_eq!(setup.step_type, "reference_setup");
    let recreate = l7
        .steps
        .iter()
        .find(|s| s.step_type == "reference_recreate")
        .expect("L7 应有 reference_recreate 步");
    assert_eq!(recreate.mode.as_deref(), Some("batch"));
}
