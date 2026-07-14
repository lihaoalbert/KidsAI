// W11 Day 9 — Secrets rotation 集成测试
//
// 覆盖场景:
//   - HKDF 派生确定性 (同 token → 同 key)
//   - 不同 token 派生出不同 key
//   - WrappedMaster JSON serialize/deserialize
//   - sha256_hex detects modification
//   - split_bundle 拆分 plaintext
//
// 注: encrypt_bundle 是 private (test-only), 完整 round-trip 在 src/secrets.rs::tests 里跑.
// 这里关注「跨模块协作的轮换契约」。

use kidsai_studio_lib::secrets::{
    derive_kek, sha256_hex, split_bundle, CipherInfo, WrappedMaster,
};

#[test]
fn hkdf_kek_derivation_is_deterministic() {
    // 同 token 派生 100 次 → key 完全一致
    let token = "deterministic-test-token";
    let k1 = derive_kek(token);
    for _ in 0..100 {
        let k2 = derive_kek(token);
        assert_eq!(k1, k2, "HKDF 派生必须确定性");
    }
}

#[test]
fn hkdf_different_tokens_yield_different_keys() {
    let k1 = derive_kek("device-A-token-1");
    let k2 = derive_kek("device-B-token-2");
    assert_ne!(k1, k2, "不同 device_token 派生出不同 KEK");
}

#[test]
fn hkdf_neutral_to_case_or_whitespace() {
    // HKDF 不去前导/尾随空白, 区分大小写
    let a = derive_kek("token-v1");
    let b = derive_kek("TOKEN-V1");
    assert_ne!(a, b, "HKDF 应区分大小写");

    let c = derive_kek("token-v1");
    let d = derive_kek(" token-v1");
    assert_ne!(c, d, "HKDF 应区分前导空白");
}

#[test]
fn sha256_known_value() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn sha256_detects_byte_modification() {
    let data = b"prompt: valid content";
    let expected = sha256_hex(data);
    let mut modified = data.to_vec();
    modified[5] = b'X'; // 改 1 byte
    let actual = sha256_hex(&modified);
    assert_ne!(expected, actual, "1 byte 修改应改变 sha256");
}

#[test]
fn wrapped_master_roundtrip() {
    let w = WrappedMaster {
        ciphertext_b64: "AAAA".into(),
        iv: "BBBB".into(),
        algo: "AES-256-GCM".into(),
        kdf: "HKDF-SHA256".into(),
        kdf_salt: "kidsai-secrets-v1".into(),
        kdf_info: "kidsai-secrets/wrap-master".into(),
    };
    let json = serde_json::to_string(&w).unwrap();
    let back: WrappedMaster = serde_json::from_str(&json).unwrap();
    assert_eq!(back.ciphertext_b64, "AAAA");
    assert_eq!(back.iv, "BBBB");
    assert_eq!(back.algo, "AES-256-GCM");
    assert_eq!(back.kdf_salt, "kidsai-secrets-v1");
}

#[test]
fn split_bundle_handles_multiple_files() {
    // bundle format: \n---FILE:path---\nbody\n---FILE:path2---\nbody2\n---END---\n
    let path_a = "system/director.yaml";
    let content_a = b"# role\nassistant: true\n";
    let path_b = "prompt_builder/camera.yaml";
    let content_b = b"# camera\nshot: wide\n";

    let mut plaintext = Vec::new();
    plaintext.extend_from_slice(format!("\n---FILE:{}---\n", path_a).as_bytes());
    plaintext.extend_from_slice(content_a);
    plaintext.extend_from_slice(format!("\n---FILE:{}---\n", path_b).as_bytes());
    plaintext.extend_from_slice(content_b);
    plaintext.extend_from_slice(b"\n---END---\n");

    let parts = split_bundle(&plaintext);
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].0, "system/director.yaml");
    assert_eq!(parts[0].1, content_a);
    assert_eq!(parts[1].0, "prompt_builder/camera.yaml");
    assert_eq!(parts[1].1, content_b);
}

#[test]
fn cipher_info_roundtrip() {
    let c = CipherInfo {
        algo: "AES-256-GCM".into(),
        iv: "abcdef0123456789".into(),
        plaintext_sha256: "deadbeef".into(),
    };
    let json = serde_json::to_string(&c).unwrap();
    let back: CipherInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(back.algo, c.algo);
    assert_eq!(back.iv, c.iv);
    assert_eq!(back.plaintext_sha256, c.plaintext_sha256);
}

#[test]
fn split_bundle_empty_returns_empty() {
    let parts = split_bundle(b"");
    assert!(parts.is_empty());
}

#[test]
fn kek_32_bytes_long() {
    // KEK 必须 32 bytes (AES-256-GCM)
    let k = derive_kek("any-token");
    assert_eq!(k.len(), 32);
}
