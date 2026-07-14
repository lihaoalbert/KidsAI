// W10/W11 共享底座 — LicenseSigner (RSA-PSS 公钥验签)
//
// 嵌入式公钥 + 启动期一次性加载 → 提供 verify_manifest 单一入口.
// skills/secrets 两类 manifest 复用: 都走 kidsai-server 同私钥签, 客户端同公钥验.
//
// 公钥在客户端 binary 里是公开的 — 验签只防篡改, 不防偷看 (后者走 secrets cipher).
// 私钥只在 server (admin) 一侧, 永不落地客户端.
//
// dev/test 模式: 通过 env `KIDSAI_DEV_SIGNING_KEY_PEM` 注入 dev 公钥 PEM,
// 否则使用内置 fallback (dev 仅用, 不签生产 manifest).

use std::sync::OnceLock;

use base64::Engine;
use rsa::pkcs8::DecodePublicKey;
use rsa::pss::{Signature, VerifyingKey};
use rsa::signature::Verifier;
use rsa::RsaPublicKey;
use serde::{Deserialize, Serialize};
use sha2::Sha256;

const DEV_FALLBACK_PUBKEY_PEM: &str = include_str!("../assets/dev_signing_pubkey.pem");

#[derive(Debug, thiserror::Error)]
pub enum SignerError {
    #[error("parse pubkey: {0}")]
    Parse(String),
    #[error("verify: {0}")]
    Verify(String),
    #[error("base64 decode: {0}")]
    Base64(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPayload {
    /// 待签名的 canonical bytes (serde_json canonical 序列化)
    pub canonical: String,
    /// base64(RSA-PSS signature over SHA-256 of canonical)
    pub signature_b64: String,
    /// pubkey id (用于多 key 轮换期兼容)
    pub pubkey_id: String,
}

pub struct LicenseSigner {
    pubkey_id: String,
    key: VerifyingKey<Sha256>,
}

static INSTANCE: OnceLock<LicenseSigner> = OnceLock::new();

impl LicenseSigner {
    /// 初始化全局单例 — Tauri setup 阶段调用一次, 后续 get() 直接拿.
    pub fn init_from_env() -> Result<(), SignerError> {
        let pem = std::env::var("KIDSAI_DEV_SIGNING_KEY_PEM")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEV_FALLBACK_PUBKEY_PEM.to_string());
        let key = RsaPublicKey::from_public_key_pem(&pem)
            .map_err(|e| SignerError::Parse(e.to_string()))?;
        let vk = VerifyingKey::<Sha256>::new(key);
        let id = std::env::var("KIDSAI_PUBKEY_ID")
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "kidsai-dev-2026-q3".to_string());
        let signer = LicenseSigner {
            pubkey_id: id,
            key: vk,
        };
        INSTANCE
            .set(signer)
            .map_err(|_| SignerError::Verify("already initialized".into()))?;
        Ok(())
    }

    pub fn get() -> Option<&'static LicenseSigner> {
        INSTANCE.get()
    }

    pub fn pubkey_id(&self) -> &str {
        &self.pubkey_id
    }

    /// 验签: canonical = serde_json::to_string(payload_without_signature)
    ///      signature_b64 = base64(RSA-PSS-SHA256 over canonical)
    pub fn verify(&self, canonical: &str, signature_b64: &str, pubkey_id: &str) -> Result<(), SignerError> {
        if pubkey_id != self.pubkey_id {
            return Err(SignerError::Verify(format!(
                "pubkey id mismatch: got {pubkey_id}, expected {}",
                self.pubkey_id
            )));
        }
        let sig_bytes = base64::engine::general_purpose::STANDARD
            .decode(signature_b64)
            .map_err(|e| SignerError::Base64(e.to_string()))?;
        let signature = Signature::try_from(sig_bytes.as_slice())
            .map_err(|e| SignerError::Verify(e.to_string()))?;
        self.key
            .verify(canonical.as_bytes(), &signature)
            .map_err(|e| SignerError::Verify(e.to_string()))
    }
}

/// 启动期若未 init, 调用此函数降级 (只允许在 demo 模式, 用于 cargo test 跑通).
pub fn ensure_init_or_demo() -> bool {
    if LicenseSigner::get().is_some() {
        return true;
    }
    LicenseSigner::init_from_env().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use rsa::pss::SigningKey;
    use rsa::signature::{RandomizedSigner, SignatureEncoding};
    use rsa::RsaPrivateKey;
    use rand::rngs::OsRng;

    fn fixture_keypair() -> (RsaPrivateKey, RsaPublicKey) {
        let priv_key = RsaPrivateKey::new(&mut OsRng, 2048).expect("genkey");
        let pub_key = RsaPublicKey::from(&priv_key);
        (priv_key, pub_key)
    }

    #[test]
    fn verify_accepts_valid_signature() {
        let (priv_key, pub_key) = fixture_keypair();
        let vk = VerifyingKey::<Sha256>::new(pub_key);
        let signer = LicenseSigner {
            pubkey_id: "test-key".into(),
            key: vk,
        };
        let canonical = "{\"id\":\"x\",\"v\":1}";
        let signing_key = SigningKey::<Sha256>::new(priv_key);
        let sig = signing_key.sign_with_rng(&mut OsRng, canonical.as_bytes());
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());

        signer.verify(canonical, &sig_b64, "test-key").expect("ok");
    }

    #[test]
    fn verify_rejects_tampered_canonical() {
        let (priv_key, pub_key) = fixture_keypair();
        let vk = VerifyingKey::<Sha256>::new(pub_key);
        let signer = LicenseSigner {
            pubkey_id: "test-key".into(),
            key: vk,
        };
        let canonical = "{\"id\":\"x\"}";
        let signing_key = SigningKey::<Sha256>::new(priv_key);
        let sig = signing_key.sign_with_rng(&mut OsRng, canonical.as_bytes());
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
        let tampered = "{\"id\":\"y\"}";
        assert!(signer.verify(tampered, &sig_b64, "test-key").is_err());
    }

    #[test]
    fn verify_rejects_wrong_pubkey_id() {
        let (priv_key, pub_key) = fixture_keypair();
        let vk = VerifyingKey::<Sha256>::new(pub_key);
        let signer = LicenseSigner {
            pubkey_id: "test-key".into(),
            key: vk,
        };
        let canonical = "{}";
        let signing_key = SigningKey::<Sha256>::new(priv_key);
        let sig = signing_key.sign_with_rng(&mut OsRng, canonical.as_bytes());
        let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
        assert!(signer.verify(canonical, &sig_b64, "wrong-id").is_err());
    }

    #[test]
    fn verify_rejects_garbage_b64() {
        let (_, pub_key) = fixture_keypair();
        let vk = VerifyingKey::<Sha256>::new(pub_key);
        let signer = LicenseSigner {
            pubkey_id: "test-key".into(),
            key: vk,
        };
        assert!(signer.verify("{}", "not-base64!!!", "test-key").is_err());
    }

    #[test]
    fn init_from_dev_fallback_pem_works() {
        // 至少能在没有 env 的情况下从嵌入 PEM 初始化.
        std::env::remove_var("KIDSAI_DEV_SIGNING_KEY_PEM");
        // 不能直接测 init_from_env 因为 OnceLock 是模块级单例,
        // 其它测试可能已经 init 过. 这里只验证 PEM 解析可行.
        let key = RsaPublicKey::from_public_key_pem(DEV_FALLBACK_PUBKEY_PEM).expect("parse pem");
        let _vk = VerifyingKey::<Sha256>::new(key);
    }
}