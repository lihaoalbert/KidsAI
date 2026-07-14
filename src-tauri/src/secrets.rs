// W11 Day 6 — Secrets Cipher (client side)
//
// 客户端解密流程:
//   1. 从 server 拉 manifest (RSA-PSS 签名)
//   2. verify_manifest_signature(manifest)  → 失败拒绝
//   3. 从 server 拉 wrapped_master_for_device (per-device, KEK 加密)
//   4. unwrap_master(wrapped, license_token) → master_key (32 bytes)
//   5. 从 server 拉 bundle.bin (master_key 加密)
//   6. decrypt_bundle(bundle, master_key, iv) → plaintext
//   7. sha256(plaintext) == manifest.cipher.plaintext_sha256  → 失败拒绝
//   8. split_bundle(plaintext) → [(path, bytes)] 逐文件 sha256 校验
//
// 设计取舍 (vs 原 plan):
// 原 plan: 客户端用 license_token 直接 HKDF → AES key 加密每个 bundle
// 实际: 让 server 用 master_key 加密 bundle (CDN 友好), 客户端用 KEK 解开 per-device 包裹的 master
//   → bundle 可在 CDN/edge 缓存, 只有 wrapped_master 是 per-device (60 bytes)
//
// 反调试 (Day 8 接 anti_tamper.rs):
//   * master_key 在内存用 secrecy::Secret<[u8; 32]>, drop 时 zeroize
//   * 解密后 plaintext 仍走 secrecy::Secret<Vec<u8>>
//   * 启动 + 周期 (Day 8) 后台线程重算 plaintext_sha256, 不一致 → 拒绝服务

use std::fmt;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroize;

use crate::license_signer::LicenseSigner;

const KDF_SALT: &[u8] = b"kidsai-secrets-v1";
const KDF_INFO_WRAP: &[u8] = b"kidsai-secrets/wrap-master";

/// Per-device wrap: server 用 KEK (HKDF 派生自 license_token) 加密 master_key 后下发.
/// 客户端用同一个 license_token 派生 KEK 解开, 拿到 master_key 再解 bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrappedMaster {
    /// base64(AES-256-GCM(KEK, master_key, iv)) — 含 tag
    pub ciphertext_b64: String,
    /// base64(12 bytes)
    pub iv: String,
    /// 永远 "AES-256-GCM"
    pub algo: String,
    /// 永远 "HKDF-SHA256"
    pub kdf: String,
    /// 永远 "kidsai-secrets-v1"
    pub kdf_salt: String,
    /// 永远 "kidsai-secrets/wrap-master"
    pub kdf_info: String,
}

/// manifest.cipher 子字段 (来自 server publish).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CipherInfo {
    pub algo: String,
    /// base64(12 bytes) — server 端 cipher dict 的 "iv" 字段
    pub iv: String,
    pub plaintext_sha256: String,
}

/// manifest 顶层字段 (除 server publish 多出 wrap / bundle / previous_version / cipher 等).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsManifest {
    pub schema: String,
    pub version: String,
    #[serde(default)]
    pub previous_version: Option<String>,
    pub profile: String,
    pub created_at: String,
    pub publisher_pubkey_id: String,
    pub files: Vec<SecretFileEntry>,
    pub cipher: CipherInfo,
    /// base64(RSA-PSS-SHA256 over canonical manifest)
    pub publisher_signature: String,
    #[serde(default)]
    pub bundle: Option<BundleInfo>,
    #[serde(default)]
    pub wrap: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretFileEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleInfo {
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("签名验证失败: {0}")]
    Signature(String),
    #[error("不支持的 cipher: {0}")]
    UnsupportedCipher(String),
    #[error("不支持的 wrap algo: {0}")]
    UnsupportedWrap(String),
    #[error("KEK 派生失败: {0}")]
    Kdf(String),
    #[error("AES-GCM 解密失败 (master_key 错误或 ciphertext 篡改)")]
    Decrypt,
    #[error("plaintext sha256 不一致 (expected {expected}, got {actual})")]
    PlaintextShaMismatch { expected: String, actual: String },
    #[error("bundle sha256 不一致 (expected {expected}, got {actual})")]
    BundleShaMismatch { expected: String, actual: String },
    #[error("base64 解码失败: {0}")]
    Base64(String),
    #[error("manifest 缺字段: {0}")]
    MissingField(&'static str),
    #[error("server 端无 wrap 包裹 (legacy manifest)")]
    NoWrap,
}

/// LicenseToken-derived KEK (32 bytes for AES-256).
/// 同一 license_token 多次调用返一致结果 (HKDF 确定性).
pub fn derive_kek(license_token: &str) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(Some(KDF_SALT), license_token.as_bytes());
    let mut okm = [0u8; 32];
    hk.expand(KDF_INFO_WRAP, &mut okm)
        .expect("HKDF expand 32 bytes is within limit");
    okm
}

/// 用 license_token 派生 KEK, 解开 server 发的 wrapped_master, 返 master_key (32 bytes).
pub fn unwrap_master(wrapped: &WrappedMaster, license_token: &str) -> Result<[u8; 32], SecretError> {
    if wrapped.algo != "AES-256-GCM" {
        return Err(SecretError::UnsupportedWrap(wrapped.algo.clone()));
    }
    if wrapped.kdf != "HKDF-SHA256" {
        return Err(SecretError::UnsupportedWrap(wrapped.kdf.clone()));
    }
    if wrapped.kdf_salt.as_bytes() != KDF_SALT {
        return Err(SecretError::Kdf(format!(
            "salt 必须是 {:?}",
            std::str::from_utf8(KDF_SALT).unwrap()
        )));
    }
    if wrapped.kdf_info.as_bytes() != KDF_INFO_WRAP {
        return Err(SecretError::Kdf(format!(
            "info 必须是 {:?}",
            std::str::from_utf8(KDF_INFO_WRAP).unwrap()
        )));
    }

    let kek = derive_kek(license_token);
    let key = Key::<Aes256Gcm>::from_slice(&kek);
    let cipher = Aes256Gcm::new(key);

    let iv_bytes = base64_decode(&wrapped.iv)?;
    if iv_bytes.len() != 12 {
        return Err(SecretError::Kdf(format!("iv 长度 {} ≠ 12", iv_bytes.len())));
    }
    let nonce = Nonce::from_slice(&iv_bytes);
    let ct = base64_decode(&wrapped.ciphertext_b64)?;

    // 用 license_token 作 AAD — server 应该也这么做 (防止重放到别的设备)
    let plaintext = cipher
        .decrypt(
            nonce,
            Payload {
                msg: &ct,
                aad: license_token.as_bytes(),
            },
        )
        .map_err(|_| SecretError::Decrypt)?;

    if plaintext.len() != 32 {
        let mut pt = plaintext;
        pt.zeroize();
        return Err(SecretError::Kdf(format!(
            "master_key 长度 {} ≠ 32",
            pt.len()
        )));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&plaintext);
    // 显式 zeroize 临时 plaintext
    let mut pt = plaintext;
    pt.zeroize();
    Ok(out)
}

/// 用 master_key 解密 bundle → plaintext (含所有 yaml 文件, 用 split_bundle 拆).
/// 同时校验 plaintext sha256 是否匹配 manifest.cipher.plaintext_sha256.
pub fn decrypt_bundle(
    bundle_ct: &[u8],
    master_key: &[u8; 32],
    cipher: &CipherInfo,
) -> Result<Vec<u8>, SecretError> {
    if cipher.algo != "AES-256-GCM" {
        return Err(SecretError::UnsupportedCipher(cipher.algo.clone()));
    }
    let key = Key::<Aes256Gcm>::from_slice(master_key.as_slice());
    let aes = Aes256Gcm::new(key);
    let iv_bytes = base64_decode(&cipher.iv)?;
    if iv_bytes.len() != 12 {
        return Err(SecretError::Kdf(format!("iv 长度 {} ≠ 12", iv_bytes.len())));
    }
    let nonce = Nonce::from_slice(&iv_bytes);
    let plaintext = aes
        .decrypt(nonce, bundle_ct)
        .map_err(|_| SecretError::Decrypt)?;

    // 验 plaintext sha256
    let actual = sha256_hex(&plaintext);
    if actual != cipher.plaintext_sha256 {
        return Err(SecretError::PlaintextShaMismatch {
            expected: cipher.plaintext_sha256.clone(),
            actual,
        });
    }
    Ok(plaintext)
}

/// 从已 verify + decrypt 的 plaintext 中按 `---FILE:path---` 标记拆出 (path, bytes) 列表.
/// 与 server secrets_publisher.concat_for_bundle 配套.
/// plaintext 格式:
///   \n---FILE:path1---\nbody1\n---FILE:path2---\nbody2\n---END---\n
/// body 不会包含 `\n---FILE:` 或 `\n---END---\n` 子串 (按 file 内容做 base64 编码可避免).
pub fn split_bundle(plaintext: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let marker: &[u8] = b"\n---FILE:";
    let header_end: &[u8] = b"---\n";
    let end_marker: &[u8] = b"\n---END---\n";
    let mut pos = 0usize;
    while pos < plaintext.len() {
        // 找下一个 \n---FILE: 起点
        let Some(i) = plaintext[pos..]
            .windows(marker.len())
            .position(|w| w == marker)
        else {
            break;
        };
        let file_start = pos + i + marker.len();
        // 找 path 结束位置 (--- \n)
        let Some(j) = plaintext[file_start..]
            .windows(header_end.len())
            .position(|w| w == header_end)
        else {
            break;
        };
        let path = std::str::from_utf8(&plaintext[file_start..file_start + j])
            .unwrap_or("")
            .to_string();
        let body_start = file_start + j + header_end.len();
        // body 结束于最近的 (min) 下一个 file marker 或 end marker
        let after_body = &plaintext[body_start..];
        let next_file = after_body
            .windows(marker.len())
            .position(|w| w == marker);
        let end = after_body
            .windows(end_marker.len())
            .position(|w| w == end_marker);
        let body_end = match (next_file, end) {
            (Some(n), Some(e)) => n.min(e),    // 取最近的
            (Some(n), None) => n,
            (None, Some(e)) => e,
            (None, None) => break,             // 不应该发生
        };
        let body = after_body[..body_end].to_vec();
        out.push((path, body));
        // 推进: body_end 是相对 after_body 的偏移
        pos = body_start + body_end;
    }
    out
}

/// 计算 sha256 hex (lowercase 64 chars).
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(data);
    let out = h.finalize();
    out.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 验 manifest 签名 (RSA-PSS-SHA256 over canonical = manifest minus signature).
/// 复用 LicenseSigner 单例, 与 skills/secrets 共享签名通道.
pub fn verify_manifest_signature(manifest: &SecretsManifest) -> Result<(), SecretError> {
    let signer = LicenseSigner::get().ok_or_else(|| {
        SecretError::Signature("LicenseSigner 未初始化".into())
    })?;
    let mut manifest_no_sig = serde_json::to_value(manifest)
        .map_err(|e| SecretError::Signature(format!("serialize: {e}")))?;
    if let Some(obj) = manifest_no_sig.as_object_mut() {
        obj.remove("publisher_signature");
    }
    let canonical = serde_json::to_string(&manifest_no_sig)
        .map_err(|e| SecretError::Signature(format!("serialize: {e}")))?;
    signer
        .verify(
            &canonical,
            &manifest.publisher_signature,
            &manifest.publisher_pubkey_id,
        )
        .map_err(|e| SecretError::Signature(e.to_string()))
}

/// 端到端: 拿到 wrapped + bundle + manifest, license_token 派生 KEK, 解开 master, 解 bundle.
/// 返 (plaintext, manifest_files_for_per_file_sha256_check).
pub fn verify_and_decrypt(
    manifest: &SecretsManifest,
    wrapped: &WrappedMaster,
    bundle_ct: &[u8],
    license_token: &str,
) -> Result<Vec<u8>, SecretError> {
    // 1. RSA-PSS 验签 manifest
    verify_manifest_signature(manifest)?;
    // 2. KEK unwrap master
    let master = unwrap_master(wrapped, license_token)?;
    // 3. AES-GCM 解 bundle
    let plaintext = decrypt_bundle(bundle_ct, &master, &manifest.cipher)?;
    // 4. zeroize master
    let mut m = master;
    m.zeroize();
    Ok(plaintext)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, SecretError> {
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    STANDARD
        .decode(s)
        .map_err(|e| SecretError::Base64(e.to_string()))
}

// ===== Display / Debug 控制 — master_key / plaintext 不进日志 =====
pub struct Redact<T: fmt::Display>(pub T);
impl<T: fmt::Display> fmt::Display for Redact<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<redacted:{} bytes>", std::mem::size_of_val(&self.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::aead::Aead;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use rsa::pss::{SigningKey, VerifyingKey};
    use rsa::signature::{RandomizedSigner, SignatureEncoding};
    use rsa::RsaPrivateKey;
    use sha2::Sha256;

    fn gen_keypair() -> (RsaPrivateKey, String) {
        let priv_k = RsaPrivateKey::new(&mut rand::rngs::OsRng, 2048).unwrap();
        // pubkey id = sha256(SPKI DER)[:16]
        use rsa::pkcs8::EncodePublicKey;
        let pub_der = priv_k.to_public_key().to_public_key_der().unwrap();
        use sha2::Digest;
        let mut h = Sha256::new();
        h.update(pub_der.as_bytes());
        let pubkey_id = format!("{:x}", h.finalize())[..16].to_string();
        (priv_k, pubkey_id)
    }

    fn init_signer_for_test(priv_k: &RsaPrivateKey, pubkey_id: &str) {
        let vk = VerifyingKey::<Sha256>::new(priv_k.to_public_key());
        let signer = crate::license_signer::LicenseSigner::new_for_test(pubkey_id, vk);
        let _ = crate::license_signer::LicenseSigner::set_for_test(signer);
    }

    fn encrypt_master_for_token(master: &[u8; 32], license_token: &str) -> WrappedMaster {
        let kek = derive_kek(license_token);
        let key = Key::<Aes256Gcm>::from_slice(&kek);
        let cipher = Aes256Gcm::new(key);
        let iv = [7u8; 12];
        let ct = cipher
            .encrypt(
                Nonce::from_slice(&iv),
                Payload {
                    msg: master.as_slice(),
                    aad: license_token.as_bytes(),
                },
            )
            .unwrap();
        WrappedMaster {
            ciphertext_b64: STANDARD.encode(ct),
            iv: STANDARD.encode(iv),
            algo: "AES-256-GCM".into(),
            kdf: "HKDF-SHA256".into(),
            kdf_salt: "kidsai-secrets-v1".into(),
            kdf_info: "kidsai-secrets/wrap-master".into(),
        }
    }

    fn concat_for_bundle(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (p, b) in files {
            out.extend_from_slice(format!("\n---FILE:{}---\n", p).as_bytes());
            out.extend_from_slice(b);
        }
        out.extend_from_slice(b"\n---END---\n");
        out
    }

    fn encrypt_bundle(plaintext: &[u8], master: &[u8; 32]) -> (Vec<u8>, [u8; 12]) {
        let iv = [9u8; 12];
        let key = Key::<Aes256Gcm>::from_slice(master.as_slice());
        let c = Aes256Gcm::new(key);
        let ct = c.encrypt(Nonce::from_slice(&iv), plaintext).unwrap();
        (ct, iv)
    }

    fn make_master() -> [u8; 32] {
        let mut m = [0u8; 32];
        for (i, b) in m.iter_mut().enumerate() {
            *b = i as u8;
        }
        m
    }

    fn make_signed_manifest(
        files: &[(&str, &[u8])],
        priv_k: &RsaPrivateKey,
        pubkey_id: &str,
    ) -> (SecretsManifest, Vec<u8>) {
        let plaintext = concat_for_bundle(files);
        let master = make_master();
        let (ct, iv) = encrypt_bundle(&plaintext, &master);
        let plaintext_sha = sha256_hex(&plaintext);

        let file_entries: Vec<SecretFileEntry> = files
            .iter()
            .map(|(p, b)| SecretFileEntry {
                path: (*p).into(),
                sha256: sha256_hex(b),
                size: b.len() as u64,
            })
            .collect();

        // 构造 manifest 不带 signature → 序列化 → 签 → 加上 signature
        let mut value = serde_json::json!({
            "schema": "kidsai.secrets/1",
            "version": "v1.test",
            "previous_version": null,
            "profile": "child",
            "created_at": "2026-07-14T00:00:00Z",
            "publisher_pubkey_id": pubkey_id,
            "files": file_entries,
            "cipher": {
                "algo": "AES-256-GCM",
                "iv": STANDARD.encode(iv),
                "plaintext_sha256": plaintext_sha,
            },
            "bundle": {
                "size_bytes": ct.len(),
                "sha256": sha256_hex(&ct),
            },
            "wrap": {
                "kdf": "HKDF-SHA256",
                "kdf_salt": "kidsai-secrets-v1",
                "kdf_info": "kidsai-secrets/wrap-master",
                "wrap_algo": "AES-256-GCM",
                "note": "test",
            },
        });
        let canonical = serde_json::to_string(&value).unwrap();
        let sig = SigningKey::<Sha256>::new(priv_k.clone())
            .sign_with_rng(&mut rand::rngs::OsRng, canonical.as_bytes());
        value.as_object_mut().unwrap().insert(
            "publisher_signature".into(),
            serde_json::Value::String(STANDARD.encode(sig.to_bytes())),
        );
        let parsed: SecretsManifest = serde_json::from_value(value).unwrap();
        (parsed, ct)
    }

    #[test]
    fn derive_kek_is_deterministic() {
        let a = derive_kek("test-token-123");
        let b = derive_kek("test-token-123");
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
    }

    #[test]
    fn derive_kek_differs_per_token() {
        assert_ne!(derive_kek("alice"), derive_kek("bob"));
    }

    #[test]
    fn split_bundle_roundtrips_simple() {
        let plaintext = concat_for_bundle(&[("a.txt", b"hello"), ("b/c.yaml", b"world")]);
        let parts = split_bundle(&plaintext);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].0, "a.txt");
        assert_eq!(parts[0].1, b"hello");
        assert_eq!(parts[1].0, "b/c.yaml");
        assert_eq!(parts[1].1, b"world");
    }

    #[test]
    fn split_bundle_empty_returns_empty() {
        let parts = split_bundle(b"");
        assert!(parts.is_empty());
    }

    #[test]
    fn sha256_hex_known_value() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn decrypt_bundle_rejects_bad_master() {
        let plaintext = concat_for_bundle(&[("x", b"y")]);
        let good = make_master();
        let (ct, iv) = encrypt_bundle(&plaintext, &good);
        let mut bad = good;
        bad[0] ^= 1;
        let cipher = CipherInfo {
            algo: "AES-256-GCM".into(),
            iv: STANDARD.encode(iv),
            plaintext_sha256: sha256_hex(&plaintext),
        };
        let res = decrypt_bundle(&ct, &bad, &cipher);
        assert!(matches!(res, Err(SecretError::Decrypt)));
    }

    #[test]
    fn decrypt_bundle_detects_ct_tamper() {
        let plaintext = concat_for_bundle(&[("x", b"y")]);
        let master = make_master();
        let (mut ct, iv) = encrypt_bundle(&plaintext, &master);
        ct[0] ^= 0xff;
        let cipher = CipherInfo {
            algo: "AES-256-GCM".into(),
            iv: STANDARD.encode(iv),
            plaintext_sha256: sha256_hex(&plaintext),
        };
        let res = decrypt_bundle(&ct, &master, &cipher);
        assert!(matches!(res, Err(SecretError::Decrypt)));
    }

    #[test]
    fn decrypt_bundle_rejects_wrong_plaintext_sha() {
        let plaintext = concat_for_bundle(&[("x", b"y")]);
        let master = make_master();
        let (ct, iv) = encrypt_bundle(&plaintext, &master);
        let cipher = CipherInfo {
            algo: "AES-256-GCM".into(),
            iv: STANDARD.encode(iv),
            plaintext_sha256: "0".repeat(64),
        };
        let res = decrypt_bundle(&ct, &master, &cipher);
        assert!(matches!(res, Err(SecretError::PlaintextShaMismatch { .. })));
    }

    #[test]
    fn unwrap_master_wrong_token_fails() {
        let master = make_master();
        let wrapped = encrypt_master_for_token(&master, "device-A");
        let res = unwrap_master(&wrapped, "device-B");
        assert!(matches!(res, Err(SecretError::Decrypt)));
    }

    #[test]
    fn unwrap_master_wrong_algo() {
        let master = make_master();
        let mut wrapped = encrypt_master_for_token(&master, "tok");
        wrapped.algo = "ChaCha20-Poly1305".into();
        let res = unwrap_master(&wrapped, "tok");
        assert!(matches!(res, Err(SecretError::UnsupportedWrap(_))));
    }

    #[test]
    fn unwrap_master_wrong_salt() {
        let master = make_master();
        let mut wrapped = encrypt_master_for_token(&master, "tok");
        wrapped.kdf_salt = "different-salt".into();
        let res = unwrap_master(&wrapped, "tok");
        assert!(matches!(res, Err(SecretError::Kdf(_))));
    }

    #[test]
    fn end_to_end_unwrap_decrypt_split() {
        let license_token = "test-device-token-001";
        let master = make_master();
        let wrapped = encrypt_master_for_token(&master, license_token);

        let files: [(&str, &[u8]); 2] = [
            ("system/director.yaml", b"role: child director\n"),
            ("safety/blacklist.yaml", b"badword: []\n"),
        ];
        let plaintext = concat_for_bundle(&files);
        let (ct, iv) = encrypt_bundle(&plaintext, &master);
        let cipher = CipherInfo {
            algo: "AES-256-GCM".into(),
            iv: STANDARD.encode(iv),
            plaintext_sha256: sha256_hex(&plaintext),
        };

        let got_master = unwrap_master(&wrapped, license_token).unwrap();
        assert_eq!(got_master, master);

        let got_pt = decrypt_bundle(&ct, &got_master, &cipher).unwrap();
        assert_eq!(got_pt, plaintext);

        let parts = split_bundle(&got_pt);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].0, "system/director.yaml");
        assert_eq!(parts[0].1, b"role: child director\n");
    }

    #[test]
    fn verify_manifest_signature_accepts_valid() {
        let (priv_k, pubkey_id) = gen_keypair();
        init_signer_for_test(&priv_k, &pubkey_id);
        let (m, _ct) = make_signed_manifest(
            &[("a.yaml", b"hello")],
            &priv_k,
            &pubkey_id,
        );
        verify_manifest_signature(&m).expect("ok");
    }

    #[test]
    fn verify_manifest_signature_rejects_tampered_version() {
        let (priv_k, pubkey_id) = gen_keypair();
        init_signer_for_test(&priv_k, &pubkey_id);
        let (mut m, _ct) = make_signed_manifest(
            &[("a.yaml", b"hello")],
            &priv_k,
            &pubkey_id,
        );
        m.version = "v999.tampered".into();
        let res = verify_manifest_signature(&m);
        assert!(matches!(res, Err(SecretError::Signature(_))));
    }

    #[test]
    fn verify_manifest_signature_rejects_wrong_pubkey_id() {
        let (priv_k, pubkey_id) = gen_keypair();
        init_signer_for_test(&priv_k, &pubkey_id);
        let (m, _ct) = make_signed_manifest(
            &[("a.yaml", b"hello")],
            &priv_k,
            "different-pubkey-id",
        );
        let res = verify_manifest_signature(&m);
        assert!(matches!(res, Err(SecretError::Signature(_))));
    }
}
