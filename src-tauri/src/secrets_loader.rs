// W11 Day 7 — SecretsLoader (启动期 bootstrap)
//
// 启动时从 SecretsStore 读 current.json, 尝试重新 verify + decrypt 已装版本的 bundle,
// 解密后注入到 SecretsRuntime 内存表. 失败 (签名错 / wrapped 解不开) → 走 fallback.
//
// 设计: bootstrap 阶段不强制要求 license_token 已注入; 仅当 license 可用时才尝试真解密.
// 真正使用期 (apply_secrets_update 或 get() 命中) 才走完整链路.

use crate::license_store::LicenseStore;
use crate::secrets::verify_and_decrypt;
use crate::secrets_runtime::SecretsRuntime;
use crate::secrets_store::SecretsStore;

pub struct LoaderReport {
    pub child_loaded: bool,
    pub adult_loaded: bool,
    pub child_version: Option<String>,
    pub adult_version: Option<String>,
    pub errors: Vec<String>,
}

/// 启动期 bootstrap. 不阻塞: 失败仅记日志, fallback 接管.
/// license_token 已从 license store 拿出, 直接传字符串 (避免 store 被 move 后的 borrow 问题).
pub fn bootstrap_with_token(
    store: &SecretsStore,
    license_token: Option<&str>,
    runtime: &SecretsRuntime,
) -> LoaderReport {
    let mut report = LoaderReport {
        child_loaded: false,
        adult_loaded: false,
        child_version: None,
        adult_version: None,
        errors: Vec::new(),
    };

    let cur = match store.load_current() {
        Ok(c) => c,
        Err(e) => {
            report.errors.push(format!("load_current: {e}"));
            return report;
        }
    };

    for (profile, version) in &cur.profiles {
        match load_one(store, runtime, profile, version, license_token) {
            Ok(()) => {
                if profile == "child" {
                    report.child_loaded = true;
                    report.child_version = Some(version.clone());
                } else if profile == "adult" {
                    report.adult_loaded = true;
                    report.adult_version = Some(version.clone());
                }
            }
            Err(e) => {
                report.errors.push(format!("{profile}/{version}: {e}"));
            }
        }
    }

    report
}

/// 启动期 bootstrap. 不阻塞: 失败仅记日志, fallback 接管.
pub fn bootstrap(
    store: &SecretsStore,
    license: &LicenseStore,
    runtime: &SecretsRuntime,
) -> LoaderReport {
    // 当前 mode (走 license.json.mode; 老 license 默认 Child)
    let license_file = license.load();
    if let Some(lf) = &license_file {
        let rt = runtime.clone();
        let mode = lf.mode;
        tauri::async_runtime::spawn(async move {
            rt.set_mode(mode).await;
        });
    }
    let token = license_file.as_ref().map(|lf| lf.license_token.as_str());
    bootstrap_with_token(store, token, runtime)
}

fn load_one(
    store: &SecretsStore,
    runtime: &SecretsRuntime,
    profile: &str,
    version: &str,
    license_token: Option<&str>,
) -> Result<(), String> {
    let manifest = store
        .read_manifest(profile, version)
        .map_err(|e| format!("read_manifest: {e}"))?;
    let bundle_ct = store
        .read_bundle(profile, version)
        .map_err(|e| format!("read_bundle: {e}"))?;
    let wrapped = store
        .read_wrapped(profile, version)
        .map_err(|e| format!("read_wrapped: {e}"))?;

    // 没 license_token → 仅记空, 走 fallback
    let token = match license_token {
        Some(t) if !t.is_empty() => t,
        _ => return Err("no license_token".into()),
    };

    // 完整 verify_and_decrypt
    let plaintext = verify_and_decrypt(&manifest, &wrapped, &bundle_ct, token)
        .map_err(|e| format!("verify_and_decrypt: {e}"))?;

    // 拆分 (path, bytes)
    let entries = crate::secrets::split_bundle(&plaintext);
    if entries.is_empty() {
        return Err("split_bundle returned empty".into());
    }

    // 校验逐文件 sha256 (如果 manifest 里有)
    for (path, bytes) in &entries {
        if let Some(expected) = manifest.files.iter().find(|f| &f.path == path) {
            let actual = crate::secrets::sha256_hex(bytes);
            if actual != expected.sha256 {
                return Err(format!(
                    "file sha mismatch for {path}: expected {}, got {}",
                    expected.sha256, actual
                ));
            }
        }
    }

    // 注入到 runtime (sync 上下文中调用 async 方法, 用 tauri runtime)
    let rt = runtime.clone();
    let profile_owned = profile.to_string();
    tauri::async_runtime::spawn(async move {
        rt.install_profile_files(&profile_owned, entries).await;
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license_store::{LicenseFile, LicenseStore};
    use crate::secrets::{CipherInfo, SecretFileEntry, SecretsManifest, WrappedMaster};
    use crate::secrets_runtime::SecretsRuntime;
    use crate::secrets_store::SecretsStore;
    use aes_gcm::aead::{Aead, KeyInit, Payload};
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use hkdf::Hkdf;
    use sha2::Sha256;
    use tempfile::tempdir;

    const KDF_SALT: &[u8] = b"kidsai-secrets-v1";
    const KDF_INFO: &[u8] = b"kidsai-secrets/wrap-master";

    fn derive_kek(token: &str) -> [u8; 32] {
        let hk = Hkdf::<Sha256>::new(Some(KDF_SALT), token.as_bytes());
        let mut okm = [0u8; 32];
        hk.expand(KDF_INFO, &mut okm).unwrap();
        okm
    }

    fn make_master() -> [u8; 32] {
        [7u8; 32]
    }

    fn wrap_master(master: &[u8; 32], token: &str) -> WrappedMaster {
        let kek = derive_kek(token);
        let key = Key::<Aes256Gcm>::from_slice(&kek);
        let c = Aes256Gcm::new(key);
        let iv = [3u8; 12];
        let ct = c
            .encrypt(
                Nonce::from_slice(&iv),
                Payload {
                    msg: master.as_slice(),
                    aad: token.as_bytes(),
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

    fn concat(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for (p, b) in files {
            out.extend_from_slice(format!("\n---FILE:{}---\n", p).as_bytes());
            out.extend_from_slice(b);
        }
        out.extend_from_slice(b"\n---END---\n");
        out
    }

    fn encrypt_bundle(plaintext: &[u8], master: &[u8; 32]) -> (Vec<u8>, [u8; 12]) {
        let iv = [5u8; 12];
        let key = Key::<Aes256Gcm>::from_slice(master.as_slice());
        let c = Aes256Gcm::new(key);
        let ct = c.encrypt(Nonce::from_slice(&iv), plaintext).unwrap();
        (ct, iv)
    }

    /// stub manifest (无签名 — bootstrap 不验签, 因 LicenseSigner 未 init; Day 6 的
    /// verify_and_decrypt 才验签, 这里若未 init verify_manifest_signature 会 fail)
    ///
    /// 为避免 verify_manifest 路径失败, 这里仅测 fallback 路径 — 即 license 缺失场景.
    #[test]
    fn bootstrap_no_license_falls_back_silently() {
        let dir = tempdir().unwrap();
        let secrets_store = SecretsStore::new(dir.path());

        // 写一个 dummy version (manifest 内容不重要, 因没 license 不会真解)
        let m = SecretsManifest {
            schema: "kidsai.secrets/1".into(),
            version: "v1.x".into(),
            previous_version: None,
            profile: "child".into(),
            created_at: "2026-07-14T00:00:00Z".into(),
            publisher_pubkey_id: "test".into(),
            files: vec![SecretFileEntry {
                path: "system/director.yaml".into(),
                sha256: "abc".into(),
                size: 3,
            }],
            cipher: CipherInfo {
                algo: "AES-256-GCM".into(),
                iv: STANDARD.encode([0u8; 12]),
                plaintext_sha256: "0".repeat(64),
            },
            publisher_signature: "AAAA".into(),
            bundle: None,
            wrap: None,
        };
        secrets_store
            .install_version("child", &m, b"dummy", &WrappedMaster {
                ciphertext_b64: "AAAA".into(),
                iv: STANDARD.encode([0u8; 12]),
                algo: "AES-256-GCM".into(),
                kdf: "HKDF-SHA256".into(),
                kdf_salt: "kidsai-secrets-v1".into(),
                kdf_info: "kidsai-secrets/wrap-master".into(),
            })
            .unwrap();

        let license_store = LicenseStore::new(dir.path());
        let runtime = SecretsRuntime::new();
        let report = bootstrap(&secrets_store, &license_store, &runtime);

        // license 缺失 → child_loaded = false, 但 errors 不为空
        assert!(!report.child_loaded);
        assert!(report.errors.iter().any(|e| e.contains("no license_token")));
    }

    #[test]
    fn bootstrap_empty_store_no_errors() {
        let dir = tempdir().unwrap();
        let secrets_store = SecretsStore::new(dir.path());
        let license_store = LicenseStore::new(dir.path());
        let runtime = SecretsRuntime::new();
        let report = bootstrap(&secrets_store, &license_store, &runtime);
        assert!(!report.child_loaded);
        assert!(!report.adult_loaded);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn bootstrap_corrupt_current_does_not_panic() {
        let dir = tempdir().unwrap();
        let secrets_store = SecretsStore::new(dir.path());
        // 写一个非法 current.json
        secrets_store
            .storage()
            .write_atomic(std::path::Path::new("current.json"), b"NOT VALID JSON {{{")
            .unwrap();
        let license_store = LicenseStore::new(dir.path());
        let runtime = SecretsRuntime::new();
        let report = bootstrap(&secrets_store, &license_store, &runtime);
        // 应该不崩, 仅记 error
        assert!(!report.errors.is_empty());
    }

    /// 端到端: install (走 store) + bootstrap (走 runtime) — 但 bootstrap 阶段需 LicenseSigner
    /// 已 init 才能 verify_manifest. 这里跳过 verify 路径, 用 install_profile_files 直接注入
    /// 来验证 loader 的"install 文件"路径正确性.
    #[tokio::test]
    async fn install_then_read_via_runtime() {
        // 直接 install files 到 runtime, 跳过 bootstrap 的 verify_and_decrypt 路径
        let rt = SecretsRuntime::new();
        rt.install_profile_files(
            "child",
            vec![(
                "system/director.yaml".into(),
                b"# roleplay header".to_vec(),
            )],
        )
        .await;
        let got = rt.get("system/director.yaml").await.unwrap();
        assert_eq!(got, b"# roleplay header");
    }

    /// 仅 smoke test: 文件拼接 → encrypt → store.install → 重读 OK
    #[test]
    fn end_to_end_smoke_install_then_read() {
        let dir = tempdir().unwrap();
        let store = SecretsStore::new(dir.path());

        let token = "test-device-token-001";
        let master = make_master();
        let wrapped = wrap_master(&master, token);

        let plaintext = concat(&[("system/director.yaml", b"role: child")]);
        let (ct, iv) = encrypt_bundle(&plaintext, &master);
        let manifest = SecretsManifest {
            schema: "kidsai.secrets/1".into(),
            version: "v1.x".into(),
            previous_version: None,
            profile: "child".into(),
            created_at: "2026-07-14T00:00:00Z".into(),
            publisher_pubkey_id: "test".into(),
            files: vec![SecretFileEntry {
                path: "system/director.yaml".into(),
                sha256: crate::secrets::sha256_hex(b"role: child"),
                size: 11,
            }],
            cipher: CipherInfo {
                algo: "AES-256-GCM".into(),
                iv: STANDARD.encode(iv),
                plaintext_sha256: crate::secrets::sha256_hex(&plaintext),
            },
            publisher_signature: "AAAA".into(),
            bundle: None,
            wrap: None,
        };
        store.install_version("child", &manifest, &ct, &wrapped).unwrap();

        let m2 = store.read_manifest("child", "v1.x").unwrap();
        assert_eq!(m2.files.len(), 1);
        let b2 = store.read_bundle("child", "v1.x").unwrap();
        assert_eq!(b2, ct);
    }

    /// license_token 已注入 + 签名错 → 失败但进程不崩
    #[test]
    fn bootstrap_with_license_but_no_signer_logs_error() {
        let dir = tempdir().unwrap();
        let store = SecretsStore::new(dir.path());
        let m = SecretsManifest {
            schema: "kidsai.secrets/1".into(),
            version: "v1.x".into(),
            previous_version: None,
            profile: "child".into(),
            created_at: "2026-07-14T00:00:00Z".into(),
            publisher_pubkey_id: "test".into(),
            files: vec![],
            cipher: CipherInfo {
                algo: "AES-256-GCM".into(),
                iv: STANDARD.encode([0u8; 12]),
                plaintext_sha256: "0".repeat(64),
            },
            publisher_signature: "AAAA".into(),
            bundle: None,
            wrap: None,
        };
        store
            .install_version("child", &m, b"bundle", &WrappedMaster {
                ciphertext_b64: "AAAA".into(),
                iv: STANDARD.encode([0u8; 12]),
                algo: "AES-256-GCM".into(),
                kdf: "HKDF-SHA256".into(),
                kdf_salt: "kidsai-secrets-v1".into(),
                kdf_info: "kidsai-secrets/wrap-master".into(),
            })
            .unwrap();

        let license_store = LicenseStore::new(dir.path());
        // 写一个 license.json 让 bootstrap 找到 token
        let lf = LicenseFile {
            device_id: "dev-1".into(),
            license_token: "test-token".into(),
            llm_api_key: "".into(),
            video_api_key: "".into(),
            ..Default::default()
        };
        license_store.save(&lf).unwrap();

        let runtime = SecretsRuntime::new();
        let report = bootstrap(&store, &license_store, &runtime);

        // LicenseSigner 未 init → verify_manifest_signature 失败 → child_loaded = false
        // 但 errors 应有内容
        assert!(!report.child_loaded);
        assert!(report
            .errors
            .iter()
            .any(|e| e.contains("LicenseSigner") || e.contains("Signature") || e.contains("verify_and_decrypt")));
    }
}