// 模型工厂：按环境变量选真实 LLM 或 mock
// 优先级：MINIMAX_API_KEYS / MINIMAX_API_KEY > DEEPSEEK_API_KEY > OPENAI_API_KEY > DASHSCOPE_API_KEY > mock
//
// 所有真实 LLM 走同一个 OpenAiCompatible 适配器
// （MiniMax / DeepSeek / OpenAI / Qwen 都提供 OpenAI 兼容 chat completions）
//
// 注意：本模块不负责加载 .env。生产入口（main / run_agent Tauri command）启动时
// 调一次 dotenvy::dotenv() 即可。tests 里如果想覆盖 env，必须自己负责加载。

use crate::key_pool::KeyPool;
use crate::model::Model;
use crate::model_mock::MockModel;
use crate::model_openai::OpenAiCompatible;

pub struct SelectedModel {
    pub model: Box<dyn Model>,
    pub source: String, // "minimax" / "deepseek" / "openai" / "qwen" / "mock"
}

pub fn select_model() -> SelectedModel {
    // 1. MiniMax（默认）— 池优先，回退单 key
    if let Some(pool) = KeyPool::from_env("MINIMAX_API_KEYS", "MINIMAX_API_KEY") {
        let model = std::env::var("MINIMAX_MODEL").unwrap_or_else(|_| "MiniMax-M3".to_string());
        let base_url = std::env::var("MINIMAX_BASE_URL")
            .unwrap_or_else(|_| "https://api.minimaxi.com/v1".to_string());
        return SelectedModel {
            model: Box::new(OpenAiCompatible::new_pool(
                "minimax",
                &model,
                &base_url,
                pool,
            )),
            source: "minimax".to_string(),
        };
    }

    // 2. DeepSeek
    if let Ok(key) = std::env::var("DEEPSEEK_API_KEY") {
        if !key.is_empty() {
            let model =
                std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string());
            return SelectedModel {
                model: Box::new(OpenAiCompatible::new(
                    "deepseek",
                    &model,
                    "https://api.deepseek.com/v1",
                    &key,
                )),
                source: "deepseek".to_string(),
            };
        }
    }

    // 3. OpenAI
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            let model =
                std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
            let base =
                std::env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
            return SelectedModel {
                model: Box::new(OpenAiCompatible::new("openai", &model, &base, &key)),
                source: "openai".to_string(),
            };
        }
    }

    // 4. Qwen (DashScope，OpenAI 兼容模式)
    if let Ok(key) = std::env::var("DASHSCOPE_API_KEY") {
        if !key.is_empty() {
            let model =
                std::env::var("QWEN_MODEL").unwrap_or_else(|_| "qwen-plus".to_string());
            return SelectedModel {
                model: Box::new(OpenAiCompatible::new(
                    "qwen",
                    &model,
                    "https://dashscope.aliyuncs.com/compatible-mode/v1",
                    &key,
                )),
                source: "qwen".to_string(),
            };
        }
    }

    // 5. 兜底：mock
    SelectedModel {
        model: Box::new(MockModel::default()),
        source: "mock".to_string(),
    }
}
