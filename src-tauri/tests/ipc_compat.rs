// IPC 边界兼容性测试 (W4.5 A3)
// 验证 src/api/tauri.ts 前端 camelCase 与 src-tauri 后端 snake_case Rust 字段
// 在 JSON 层完全对齐 — 之前缺 #[serde(rename_all = "camelCase")] 时,
// Studio W6 save_creation 第一次在桌面跑就 IPC 报错。
//
// 测试策略: 拼一个前端会发的 SaveCreationRequest JSON, 直接 deserialize,
// 再 serialize 一次 CreationRow + AssetRow, 确认 JSON key 全是 camelCase。

use kidsai_studio_lib::creations::SaveCreationRequest;
use serde_json::{json, Value};

#[test]
fn frontend_camelcase_request_deserializes_cleanly() {
    // 前端 src/api/tauri.ts:55-64 SaveCreationRequest 的实际 JSON 形态
    let frontend_payload = json!({
        "id": "studio_smoke_001",
        "levelId": "director",
        "userInput": "喷火龙的冒险",
        "agentOutput": {"title": "喷火龙的冒险", "shots": 3},
        "score": null,
        "rubric": null,
        "feedback": null,
        "assets": [
            {
                "type": "video",
                "url": "https://example.com/seedance-output.mp4",
                "thumbnailUrl": null,
                "prompt": "喷火龙找回宝贝 3 镜合成",
                "tool": "image_to_video",
                "tokensCost": 19
            }
        ]
    });

    let req: SaveCreationRequest =
        serde_json::from_value(frontend_payload).expect("camelCase IPC should parse");
    assert_eq!(req.id, "studio_smoke_001");
    assert_eq!(req.level_id, "director");
    assert_eq!(req.user_input, "喷火龙的冒险");
    assert_eq!(req.assets.len(), 1);
    assert_eq!(req.assets[0].kind, "video", "type → kind rename");
    assert_eq!(req.assets[0].thumbnail_url, None);
    assert_eq!(req.assets[0].tokens_cost, 19);
}

#[test]
fn snake_case_payload_fails_clearly() {
    // 反向验证: 如果前端某天忘了 camelCase (regression),
    // 应该立刻看到错误, 而不是悄悄 fallback 到默认值.
    let snake_payload = json!({
        "id": "x",
        "level_id": "director",        // ← 错! 应该是 levelId
        "user_input": "x",              // ← 错! 应该是 userInput
        "agent_output": {},
        "assets": []
    });
    let result: Result<SaveCreationRequest, _> = serde_json::from_value(snake_payload);
    assert!(
        result.is_err(),
        "snake_case payload 必须 deserialization 失败 (防止悄悄回退)"
    );
}

#[test]
fn backend_serialization_emits_camelcase_for_frontend() {
    // 验证后端返回 (CreationRow + AssetRow) 序列化后是 camelCase,
    // 对齐 src/api/tauri.ts:66-83 CreationWithAssets 接口
    use kidsai_studio_lib::db::{AssetRow, CreationRow};

    let creation = CreationRow {
        id: "studio_smoke_001".to_string(),
        level_id: "director".to_string(),
        user_input: "喷火龙的冒险".to_string(),
        agent_output: r#"{"title":"喷火龙的冒险"}"#.to_string(),
        score: None,
        rubric: None,
        feedback: None,
        created_at: 1718234567890,
    };
    let asset = AssetRow {
        kind: "video".to_string(),
        url: "https://example.com/v.mp4".to_string(),
        thumbnail_url: None,
        prompt: "x".to_string(),
        tool: "image_to_video".to_string(),
        tokens_cost: 19,
    };

    let creation_json: Value = serde_json::to_value(&creation).expect("serialize creation");
    assert_eq!(creation_json["levelId"], "director");
    assert_eq!(creation_json["userInput"], "喷火龙的冒险");
    assert_eq!(creation_json["createdAt"], 1718234567890i64);
    // 确保没有 snake_case 残留 (regression guard)
    assert!(creation_json.get("level_id").is_none());
    assert!(creation_json.get("user_input").is_none());
    assert!(creation_json.get("created_at").is_none());

    let asset_json: Value = serde_json::to_value(&asset).expect("serialize asset");
    assert_eq!(asset_json["kind"], "video", "kind 保持原名 (前端无 type → video)");
    assert_eq!(asset_json["thumbnailUrl"], Value::Null);
    assert_eq!(asset_json["tokensCost"], 19);
    assert!(asset_json.get("thumbnail_url").is_none());
    assert!(asset_json.get("tokens_cost").is_none());
}