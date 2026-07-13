// MiniMax KeyPool 失败转移集成测试（mockito）
//
// 三个场景：
// 1. 429 触发切 key：key A 返 429 → key B 返 200 SSE → 成功
// 2. 401 触发切 key：同上
// 3. 5xx 不触发转移：key A 返 500 → 立即返回 Err，不调 key B
//
// ENV_LOCK 与 openai_parse.rs 串行化，避免并发改 env

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use kidsai_studio_lib::key_pool::KeyPool;
use kidsai_studio_lib::model::Model;
use kidsai_studio_lib::model_openai::OpenAiCompatible;

/// 拿一个 mockito 服务器地址 + 简单的 ModelRequest
async fn setup() -> (mockito::ServerGuard, OpenAiCompatible, kidsai_studio_lib::model::ModelRequest) {
    let server = mockito::Server::new_async().await;
    let url = server.url();
    let base = url.trim_start_matches("http://");
    let pool = KeyPool::from_str("sk-key-a,sk-key-b").expect("non-empty");
    let m = OpenAiCompatible::new_pool("test", "test-model", &format!("http://{base}"), pool);
    let req = kidsai_studio_lib::model::ModelRequest {
        system_prompt: "you are test".to_string(),
        messages: vec![kidsai_studio_lib::model::ModelMessage {
            role: "user".to_string(),
            content: "hi".to_string(),
            tool_call_id: None,
            tool_calls: None,
            name: None,
        }],
        allowed_tools: vec![],
        temperature: 0.0,
    };
    (server, m, req)
}

fn sse_ok_body() -> String {
    // 最简 SSE 流：[DONE] 结束 + 一个 content delta
    "data: {\"id\":\"x\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"}}]}\n\n\
     data: [DONE]\n\n"
        .to_string()
}

#[tokio::test]
async fn transfers_on_429_to_next_key() {
    let (mut server, m, req) = setup().await;

    let m_a = server
        .mock("POST", "/chat/completions")
        .match_header("authorization", "Bearer sk-key-a")
        .with_status(429)
        .with_body("rate limited")
        .expect(1)
        .create_async()
        .await;
    let m_b = server
        .mock("POST", "/chat/completions")
        .match_header("authorization", "Bearer sk-key-b")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(sse_ok_body())
        .expect(1)
        .create_async()
        .await;

    let cancel = Arc::new(AtomicBool::new(false));
    let (decision, _chunks) = m.decide_stream(&req, cancel).await.expect("should succeed via key-b");

    assert_eq!(decision.final_answer.as_deref(), Some("ok"));
    assert!(decision.tool.is_none());
    m_a.assert_async().await;
    m_b.assert_async().await;
}

#[tokio::test]
async fn transfers_on_401_to_next_key() {
    let (mut server, m, req) = setup().await;

    let m_a = server
        .mock("POST", "/chat/completions")
        .match_header("authorization", "Bearer sk-key-a")
        .with_status(401)
        .with_body("unauthorized")
        .expect(1)
        .create_async()
        .await;
    let m_b = server
        .mock("POST", "/chat/completions")
        .match_header("authorization", "Bearer sk-key-b")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(sse_ok_body())
        .expect(1)
        .create_async()
        .await;

    let cancel = Arc::new(AtomicBool::new(false));
    let (decision, _) = m.decide_stream(&req, cancel).await.expect("should succeed via key-b");
    assert_eq!(decision.final_answer.as_deref(), Some("ok"));
    m_a.assert_async().await;
    m_b.assert_async().await;
}

#[tokio::test]
async fn does_not_transfer_on_500() {
    let (mut server, m, req) = setup().await;

    let m_a = server
        .mock("POST", "/chat/completions")
        .match_header("authorization", "Bearer sk-key-a")
        .with_status(500)
        .with_body("internal error")
        .expect(1)
        .create_async()
        .await;
    // key-b 永远不应被命中（500 不触发转移）
    let m_b = server
        .mock("POST", "/chat/completions")
        .match_header("authorization", "Bearer sk-key-b")
        .with_status(200)
        .with_body("should-not-reach")
        .expect(0)
        .create_async()
        .await;

    let cancel = Arc::new(AtomicBool::new(false));
    let err = m.decide_stream(&req, cancel).await.expect_err("500 should fail fast");
    assert!(err.contains("upstream 500"), "err: {err}");

    m_a.assert_async().await;
    m_b.assert_async().await;
}

#[tokio::test]
async fn all_keys_exhausted_returns_error() {
    let (mut server, m, req) = setup().await;

    // A 和 B 都返 429，没有第三个 key
    let m_a = server
        .mock("POST", "/chat/completions")
        .match_header("authorization", "Bearer sk-key-a")
        .with_status(429)
        .expect(1)
        .create_async()
        .await;
    let m_b = server
        .mock("POST", "/chat/completions")
        .match_header("authorization", "Bearer sk-key-b")
        .with_status(429)
        .expect(1)
        .create_async()
        .await;

    let cancel = Arc::new(AtomicBool::new(false));
    let err = m.decide_stream(&req, cancel).await.expect_err("all keys exhausted");
    assert!(err.contains("all keys exhausted"), "err: {err}");

    m_a.assert_async().await;
    m_b.assert_async().await;
}