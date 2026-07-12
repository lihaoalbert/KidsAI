// 作品（Creation）相关 Tauri 命令
// W2.3: 持久化用户每次提交的作品 + Agent 输出 + 生成资产

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::{AssetRow, CreationRow, Db, InsertAsset, InsertCreation};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCreationRequest {
    pub id: String,
    pub level_id: String,
    pub user_input: String,
    pub agent_output: serde_json::Value, // 完整 AgentOutput 结构，存为 JSON
    pub score: Option<u32>,
    pub rubric: Option<serde_json::Value>,
    pub feedback: Option<String>,
    pub assets: Vec<AssetInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetInput {
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    pub prompt: String,
    pub tool: String,
    pub tokens_cost: u32,
}

#[derive(Debug, Serialize)]
pub struct CreationWithAssets {
    #[serde(flatten)]
    pub creation: CreationRow,
    pub assets: Vec<AssetRow>,
}

#[tauri::command]
pub fn save_creation(
    request: SaveCreationRequest,
    db: State<'_, Db>,
) -> Result<(), String> {
    let agent_output_str = serde_json::to_string(&request.agent_output)
        .map_err(|e| format!("serialize agent_output: {e}"))?;
    let rubric_str = match &request.rubric {
        Some(r) => Some(serde_json::to_string(r).map_err(|e| format!("serialize rubric: {e}"))?),
        None => None,
    };

    db.insert_creation(&InsertCreation {
        creation_id: &request.id,
        level_id: &request.level_id,
        user_input: &request.user_input,
        agent_output_json: &agent_output_str,
        score: request.score,
        rubric_json: rubric_str.as_deref(),
        feedback: request.feedback.as_deref(),
    })
    .map_err(|e| e.to_string())?;

    for asset in &request.assets {
        db.insert_asset(&InsertAsset {
            creation_id: &request.id,
            kind: &asset.kind,
            url: &asset.url,
            thumbnail_url: asset.thumbnail_url.as_deref(),
            prompt: &asset.prompt,
            tool: &asset.tool,
            tokens_cost: asset.tokens_cost,
        })
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn list_creations(
    level_id: Option<String>,
    db: State<'_, Db>,
) -> Result<Vec<CreationWithAssets>, String> {
    let rows = db.list_creations(level_id.as_deref()).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let assets = db.list_assets(&row.id).map_err(|e| e.to_string())?;
        out.push(CreationWithAssets {
            creation: row,
            assets,
        });
    }
    Ok(out)
}
