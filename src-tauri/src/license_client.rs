// License client (W4.5 B2)
//
// 桌面端 ↔ kidsai-server 控制平面: 4 个 method.
// 不代理 LLM / Seedance 流量 — 桌面拿 server 签的 license + api_keys 后直连 provider.
//
// 两种模式:
//   Server { base_url } — KIDSAI_SERVER_URL env 设置时, 走真实 HTTP
//   Demo               — 未设置 env, 所有 method 返 Ok(DemoXxx) 占位值
//                       现有 99/99 单测 + 126 cargo test 在 demo 模式下不需改动
//
// 不重试 / 不流式: server 流量小, 失败让上层决定 (fire-and-forget 静默 OK).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum LicenseMode {
    Server { base_url: String },
    Demo,
}

#[derive(Clone)]
pub struct LicenseClient {
    mode: LicenseMode,
    http: reqwest::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeys {
    pub llm: String,
    pub video: String,
    /// W6 A3: MiniMax key — 服务端从 key pool 粘性分配 1 个.
    /// 老桌面忽略此字段; 新桌面用此调 image/voice/music/hailuo.
    /// None = 服务端 demo 模式或空池, 桌面走 mock fallback.
    #[serde(default)]
    pub minimax: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivateResponse {
    pub device_id: String,
    pub license_token: String,
    pub api_keys: ApiKeys,
    pub balance: i64,
    pub daily_quota: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub device_id: String,
    pub balance: i64,
    pub daily_consumed: i64,
    pub daily_quota: i64,
    pub daily_remaining: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordSpendResponse {
    pub call_id: String,
    pub balance_after: i64,
    pub cost: i64,
    pub accepted: bool,
    #[serde(default)]
    pub rejected_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshResponse {
    pub device_id: String,
    pub license_token: String,
    pub api_keys: ApiKeys,
}

impl LicenseClient {
    pub fn from_env() -> Self {
        let _ = dotenvy::dotenv();
        let mode = match std::env::var("KIDSAI_SERVER_URL") {
            Ok(url) if !url.trim().is_empty() => LicenseMode::Server {
                base_url: url.trim_end_matches('/').to_string(),
            },
            _ => LicenseMode::Demo,
        };
        Self {
            mode,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
        }
    }

    pub fn is_demo(&self) -> bool {
        matches!(self.mode, LicenseMode::Demo)
    }

    pub async fn activate(
        &self,
        fingerprint_hash: &str,
        nickname: &str,
        age_tier: u8,
    ) -> Result<ActivateResponse, String> {
        let url = match &self.mode {
            LicenseMode::Server { base_url } => format!("{}/api/v1/devices/activate", base_url),
            LicenseMode::Demo => {
                return Ok(ActivateResponse {
                    device_id: format!("demo-{}", uuid_like()),
                    license_token: "demo-token".to_string(),
                    api_keys: ApiKeys {
                        llm: std::env::var("LLM_API_KEY").unwrap_or_default(),
                        video: std::env::var("SEEDANCE_API_KEY").unwrap_or_default(),
                        minimax: std::env::var("MINIMAX_API_KEY")
                            .ok()
                            .filter(|s| !s.is_empty()),
                    },
                    balance: 100,
                    daily_quota: 30,
                });
            }
        };
        let body = serde_json::json!({
            "fingerprintHash": fingerprint_hash,
            "nickname": nickname,
            "ageTier": age_tier,
        });
        let r = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("activate http: {e}"))?;
        if !r.status().is_success() {
            let status = r.status();
            let txt = r.text().await.unwrap_or_default();
            return Err(format!("activate {}: {}", status, txt));
        }
        r.json::<ActivateResponse>()
            .await
            .map_err(|e| format!("activate parse: {e}"))
    }

    pub async fn get_balance(&self, license_token: &str) -> Result<BalanceResponse, String> {
        match &self.mode {
            LicenseMode::Demo => Ok(BalanceResponse {
                device_id: "demo".into(),
                balance: 100,
                daily_consumed: 0,
                daily_quota: 30,
                daily_remaining: 30,
            }),
            LicenseMode::Server { base_url } => {
                let url = format!("{}/api/v1/me/balance", base_url);
                let r = self
                    .http
                    .get(&url)
                    .bearer_auth(license_token)
                    .send()
                    .await
                    .map_err(|e| format!("balance http: {e}"))?;
                if !r.status().is_success() {
                    return Err(format!("balance {}", r.status()));
                }
                r.json::<BalanceResponse>()
                    .await
                    .map_err(|e| format!("balance parse: {e}"))
            }
        }
    }

    pub async fn record_spend(
        &self,
        license_token: &str,
        call_id: &str,
        kind: &str,
        units: u32,
    ) -> Result<RecordSpendResponse, String> {
        match &self.mode {
            LicenseMode::Demo => Ok(RecordSpendResponse {
                call_id: call_id.into(),
                balance_after: 0,
                cost: 0,
                accepted: true,
                rejected_reason: None,
            }),
            LicenseMode::Server { base_url } => {
                let url = format!("{}/api/v1/me/record-spend", base_url);
                let body = serde_json::json!({
                    "callId": call_id,
                    "kind": kind,
                    "units": units,
                });
                let r = self
                    .http
                    .post(&url)
                    .bearer_auth(license_token)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("spend http: {e}"))?;
                if !r.status().is_success() {
                    let status = r.status();
                    let txt = r.text().await.unwrap_or_default();
                    return Err(format!("spend {}: {}", status, txt));
                }
                r.json::<RecordSpendResponse>()
                    .await
                    .map_err(|e| format!("spend parse: {e}"))
            }
        }
    }

    pub async fn refresh_license(&self, license_token: &str) -> Result<RefreshResponse, String> {
        match &self.mode {
            LicenseMode::Demo => Ok(RefreshResponse {
                device_id: "demo".into(),
                license_token: license_token.into(),
                api_keys: ApiKeys {
                    llm: "demo".into(),
                    video: "demo".into(),
                    minimax: std::env::var("MINIMAX_API_KEY")
                        .ok()
                        .filter(|s| !s.is_empty()),
                },
            }),
            LicenseMode::Server { base_url } => {
                let url = format!("{}/api/v1/me/refresh-license", base_url);
                let r = self
                    .http
                    .post(&url)
                    .bearer_auth(license_token)
                    .send()
                    .await
                    .map_err(|e| format!("refresh http: {e}"))?;
                if !r.status().is_success() {
                    return Err(format!("refresh {}", r.status()));
                }
                r.json::<RefreshResponse>()
                    .await
                    .map_err(|e| format!("refresh parse: {e}"))
            }
        }
    }
}

/// 短随机 id, demo 模式占位用
fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_mode_when_env_unset() {
        std::env::remove_var("KIDSAI_SERVER_URL");
        let c = LicenseClient::from_env();
        assert!(c.is_demo());
    }

    #[tokio::test]
    async fn demo_activate_returns_synthetic_envelope() {
        std::env::remove_var("KIDSAI_SERVER_URL");
        let c = LicenseClient::from_env();
        let r = c.activate("fp-test", "test", 1).await.unwrap();
        assert!(r.device_id.starts_with("demo-"));
        assert_eq!(r.balance, 100);
        assert_eq!(r.daily_quota, 30);
    }

    #[tokio::test]
    async fn demo_record_spend_accepts_with_zero_cost() {
        std::env::remove_var("KIDSAI_SERVER_URL");
        let c = LicenseClient::from_env();
        let r = c
            .record_spend("tok", "call-x", "video_draft", 1)
            .await
            .unwrap();
        assert!(r.accepted);
        assert_eq!(r.cost, 0);
    }
}
