"""W11 secrets_publisher tests.

覆盖: 加密 roundtrip, signature 验签, tampered manifest 拒签, 多 profile 隔离.
"""

import json
from pathlib import Path

import pytest
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa

from kidsai_server.secrets_publisher import (
    SCHEMA_VERSION,
    SUPPORTED_PROFILES,
    collect_files,
    concat_for_bundle,
    encrypt_bundle,
    publish,
    sign_canonical,
    split_bundle,
    sha256_of,
    derive_pubkey_id,
    PublishError,
)


@pytest.fixture
def fixtures_dir(tmp_path: Path) -> Path:
    """建临时 prompts/child + prompts/adult 目录."""
    child = tmp_path / "child"
    child.mkdir()
    (child / "director.yaml").write_text("role: child director\n")
    (child / "safety_blacklist.yaml").write_text("blocked: []\n")

    adult = tmp_path / "adult"
    adult.mkdir()
    (adult / "director_pro.yaml").write_text("role: adult pro\n")
    return tmp_path


@pytest.fixture
def keypair() -> tuple[rsa.RSAPrivateKey, rsa.RSAPublicKey]:
    priv = rsa.generate_private_key(65537, 2048)
    pub = priv.public_key()
    return priv, pub


@pytest.fixture
def master_key() -> bytes:
    return b"\x00" * 32  # 32-byte 测试用 master key


def test_collect_files_sorts_deterministically(fixtures_dir):
    files = collect_files(fixtures_dir / "child")
    paths = [p.as_posix() for p, _ in files]
    assert paths == ["director.yaml", "safety_blacklist.yaml"]


def test_concat_then_split_roundtrip(fixtures_dir):
    files = collect_files(fixtures_dir / "child")
    blob = concat_for_bundle(files)
    out = split_bundle(blob)
    assert set(out.keys()) == {"director.yaml", "safety_blacklist.yaml"}
    assert out["director.yaml"] == b"role: child director\n"
    assert out["safety_blacklist.yaml"] == b"blocked: []\n"


def test_encrypt_then_decrypt_roundtrip(master_key):
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

    plaintext = b"hello world"
    iv, ct = encrypt_bundle(plaintext, master_key)
    aes = AESGCM(master_key)
    recovered = aes.decrypt(iv, ct, None)
    assert recovered == plaintext


def test_publish_produces_manifest_and_bundle(fixtures_dir, master_key, keypair):
    priv, pub = keypair
    pubkey_id = derive_pubkey_id(priv)

    result = publish(
        profile="child",
        version="v1.test01",
        input_dir=fixtures_dir / "child",
        master_key=master_key,
        signing_key=priv,
        pubkey_id=pubkey_id,
    )

    m = result["manifest"]
    assert m["schema"] == SCHEMA_VERSION
    assert m["version"] == "v1.test01"
    assert m["profile"] == "child"
    assert m["publisher_pubkey_id"] == pubkey_id
    assert m["publisher_signature"]
    assert {f["path"] for f in m["files"]} == {"director.yaml", "safety_blacklist.yaml"}
    assert m["cipher"]["algo"] == "AES-256-GCM"
    assert m["wrap"]["kdf"] == "HKDF-SHA256"
    assert m["wrap"]["wrap_algo"] == "AES-256-GCM"


def test_signature_verifies_with_correct_pubkey(fixtures_dir, master_key, keypair):
    from cryptography.hazmat.primitives.asymmetric import padding

    priv, pub = keypair
    pubkey_id = derive_pubkey_id(priv)
    result = publish(
        profile="child",
        version="v1.test02",
        input_dir=fixtures_dir / "child",
        master_key=master_key,
        signing_key=priv,
        pubkey_id=pubkey_id,
    )
    m = result["manifest"]
    # 验签: 移除 signature 后重新序列化, 比对
    m_no_sig = {k: v for k, v in m.items() if k != "publisher_signature"}
    canonical = json.dumps(m_no_sig, sort_keys=True, separators=(",", ":")).encode()
    import base64
    sig = base64.b64decode(m["publisher_signature"])
    pub.verify(
        sig,
        canonical,
        padding.PSS(
            mgf=padding.MGF1(hashes.SHA256()),
            salt_length=padding.PSS.MAX_LENGTH,
        ),
        hashes.SHA256(),
    )  # 不抛 = 验签成功


def test_signature_rejects_tampered_version(fixtures_dir, master_key, keypair):
    from cryptography.hazmat.primitives.asymmetric import padding

    priv, pub = keypair
    pubkey_id = derive_pubkey_id(priv)
    result = publish(
        profile="child",
        version="v1.test03",
        input_dir=fixtures_dir / "child",
        master_key=master_key,
        signing_key=priv,
        pubkey_id=pubkey_id,
    )
    m = result["manifest"]
    # 改 version 但保留旧签名
    m["version"] = "v1.tampered"
    m_no_sig = {k: v for k, v in m.items() if k != "publisher_signature"}
    canonical = json.dumps(m_no_sig, sort_keys=True, separators=(",", ":")).encode()
    import base64
    sig = base64.b64decode(m["publisher_signature"])
    with pytest.raises(Exception):
        pub.verify(
            sig,
            canonical,
            padding.PSS(
                mgf=padding.MGF1(hashes.SHA256()),
                salt_length=padding.PSS.MAX_LENGTH,
            ),
            hashes.SHA256(),
        )


def test_publish_rejects_unknown_profile(fixtures_dir, master_key, keypair):
    priv, _ = keypair
    with pytest.raises(PublishError, match="profile 必须是"):
        publish(
            profile="unknown",
            version="v1.x",
            input_dir=fixtures_dir / "child",
            master_key=master_key,
            signing_key=priv,
            pubkey_id="x",
        )


def test_publish_rejects_empty_input_dir(tmp_path, master_key, keypair):
    priv, _ = keypair
    empty = tmp_path / "empty"
    empty.mkdir()
    with pytest.raises(PublishError, match="profile 目录为空"):
        publish(
            profile="child",
            version="v1.x",
            input_dir=empty,
            master_key=master_key,
            signing_key=priv,
            pubkey_id="x",
        )


def test_publish_rejects_missing_input_dir(tmp_path, master_key, keypair):
    priv, _ = keypair
    with pytest.raises(PublishError, match="input_dir 不存在"):
        publish(
            profile="child",
            version="v1.x",
            input_dir=tmp_path / "nope",
            master_key=master_key,
            signing_key=priv,
            pubkey_id="x",
        )


def test_child_and_adult_bundles_are_independent(fixtures_dir, master_key, keypair):
    """两次 publish (child + adult) 用同一 master_key, bundle 不同, 解密不串."""
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

    priv, _ = keypair
    pubkey_id = derive_pubkey_id(priv)

    child_result = publish(
        profile="child",
        version="v1.c01",
        input_dir=fixtures_dir / "child",
        master_key=master_key,
        signing_key=priv,
        pubkey_id=pubkey_id,
    )
    adult_result = publish(
        profile="adult",
        version="v1.a01",
        input_dir=fixtures_dir / "adult",
        master_key=master_key,
        signing_key=priv,
        pubkey_id=pubkey_id,
    )

    assert child_result["bundle_bytes"] != adult_result["bundle_bytes"]
    aes = AESGCM(master_key)
    child_pt = aes.decrypt(
        base64.b64decode(child_result["manifest"]["cipher"]["iv"]),
        child_result["bundle_bytes"],
        None,
    )
    adult_pt = aes.decrypt(
        base64.b64decode(adult_result["manifest"]["cipher"]["iv"]),
        adult_result["bundle_bytes"],
        None,
    )
    assert b"child director" in child_pt
    assert b"adult pro" in adult_pt


def test_derive_pubkey_id_stable(keypair):
    priv, _ = keypair
    id1 = derive_pubkey_id(priv)
    id2 = derive_pubkey_id(priv)
    assert id1 == id2
    assert len(id1) == 16
    assert all(c in "0123456789abcdef" for c in id1)


def test_supported_profiles_constant():
    assert "child" in SUPPORTED_PROFILES
    assert "adult" in SUPPORTED_PROFILES
    assert len(SUPPORTED_PROFILES) == 2


# 局部 import base64 避免顶端污染, 上面那个 test_child_and_adult 用到了
import base64  # noqa: E402