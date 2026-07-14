"""W11 Day 6 — Secrets API tests.

覆盖:
- GET /api/v1/secrets/manifest (无鉴权) 返 JSON 字典
- GET /api/v1/secrets/manifest?profile=child 不存在 → 404
- GET /api/v1/secrets/manifest?profile=adult 不存在 → 404
- GET /api/v1/secrets/manifest?profile=invalid → 422 (pattern 校验)
- GET /api/v1/secrets/bundle 返 bytes, 404 不存在
- POST /api/v1/secrets/wrap (需鉴权) → 用 master_key 包裹, 返 ciphertext
- wrap 返回的 ciphertext 用 license_token 派生 KEK 能解开 → master_key 32 bytes
- wrap 鉴权失败 → 401
- 端到端: 客户端用 license_token unwrap → 解 bundle → 拿到原文
"""

import base64
import json
import os
from pathlib import Path

import pytest
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from fastapi.testclient import TestClient

from kidsai_server.main import create_app
from kidsai_server.secrets_publisher import publish

# 跟 Rust 端 / python 端共用同一组 KDF 参数
KDF_SALT = b"kidsai-secrets-v1"
KDF_INFO_WRAP = b"kidsai-secrets/wrap-master"


@pytest.fixture
def app_and_creds(tmp_path, monkeypatch):
    """建一个临时 app + secrets_root + master_key, publish 一对 child+adult bundle."""
    master_key = os.urandom(32)
    monkeypatch.setenv("KIDSAI_SECRETS_MASTER_KEY", master_key.hex())

    # 建临时 prompts 目录
    prompts_dir = tmp_path / "prompts"
    (prompts_dir / "child").mkdir(parents=True)
    (prompts_dir / "child" / "director.yaml").write_text("role: child director\n")
    (prompts_dir / "child" / "safety.yaml").write_text("blocked: []\n")
    (prompts_dir / "adult").mkdir(parents=True)
    (prompts_dir / "adult" / "director_pro.yaml").write_text("role: adult pro\n")

    # publish child + adult
    from kidsai_server.secrets_publisher import (
        SUPPORTED_PROFILES,
        load_signing_key,
        derive_pubkey_id,
    )
    signing_key = load_signing_key()
    pubkey_id = derive_pubkey_id(signing_key)

    secrets_root = tmp_path / "secrets_out"
    for profile in SUPPORTED_PROFILES:
        result = publish(
            profile=profile,
            version="v1.test",
            input_dir=prompts_dir / profile,
            master_key=master_key,
            signing_key=signing_key,
            pubkey_id=pubkey_id,
        )
        ver_dir = secrets_root / profile / "v1.test"
        ver_dir.mkdir(parents=True, exist_ok=True)
        (ver_dir / "manifest.json").write_text(
            json.dumps(result["manifest"], indent=2, ensure_ascii=False)
        )
        (ver_dir / "bundle.bin").write_bytes(result["bundle_bytes"])

    # 建 db + 启动 app
    db_path = tmp_path / "kidsai.db"
    monkeypatch.setenv("DATABASE_PATH", str(db_path))
    monkeypatch.setenv("SECRETS_ROOT", str(secrets_root))
    monkeypatch.setenv("SKILLS_ROOT", str(tmp_path / "skills"))  # 防止 None
    monkeypatch.setenv("JWT_SECRET", "x" * 32)
    monkeypatch.setenv("ADMIN_TOKEN", "adm" * 8)
    monkeypatch.setenv("STARTING_BALANCE", "100")
    monkeypatch.setenv("DAILY_QUOTA", "30")

    app = create_app()
    with TestClient(app) as client:
        # 激活拿 license_token (jwt)
        r = client.post(
            "/api/v1/devices/activate",
            json={"fingerprintHash": "f" * 64, "nickname": "测试", "ageTier": 2},
        )
        assert r.status_code == 200, r.text
        token = r.json()["licenseToken"]
        # 把 client 持久到 yield 之后
        yield client, token, master_key


def _activate(client):
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": "f" * 64, "nickname": "测试", "ageTier": 2},
    )
    return r.json()["licenseToken"]


def _derive_kek(license_token: str) -> bytes:
    hk = HKDF(algorithm=hashes.SHA256(), length=32, salt=KDF_SALT, info=KDF_INFO_WRAP)
    return hk.derive(license_token.encode())


# ===== /manifest =====

def test_manifest_no_auth_required(app_and_creds):
    client, _, _ = app_and_creds
    r = client.get("/api/v1/secrets/manifest?profile=child")
    assert r.status_code == 200
    m = r.json()
    assert m["schema"] == "kidsai.secrets/1"
    assert m["profile"] == "child"
    assert m["version"] == "v1.test"
    assert m["publisher_signature"]


def test_manifest_adult(app_and_creds):
    client, _, _ = app_and_creds
    r = client.get("/api/v1/secrets/manifest?profile=adult")
    assert r.status_code == 200
    assert r.json()["profile"] == "adult"


def test_manifest_invalid_profile(app_and_creds):
    client, _, _ = app_and_creds
    r = client.get("/api/v1/secrets/manifest?profile=teenz")
    assert r.status_code == 422


def test_manifest_unknown_profile_returns_404(tmp_path, monkeypatch):
    monkeypatch.setenv("JWT_SECRET", "x" * 32)
    monkeypatch.setenv("ADMIN_TOKEN", "adm" * 8)
    monkeypatch.setenv("DATABASE_PATH", str(tmp_path / "kidsai.db"))
    monkeypatch.setenv("SECRETS_ROOT", str(tmp_path / "empty_secrets"))
    monkeypatch.setenv("SKILLS_ROOT", str(tmp_path / "empty_skills"))
    (tmp_path / "empty_secrets").mkdir()
    app = create_app()
    with TestClient(app) as client:
        r = client.get("/api/v1/secrets/manifest?profile=child")
        assert r.status_code == 404


# ===== /bundle =====

def test_bundle_returns_bytes(app_and_creds):
    client, _, _ = app_and_creds
    r = client.get("/api/v1/secrets/bundle?profile=child&version=v1.test")
    assert r.status_code == 200
    assert r.headers["content-type"].startswith("application/octet-stream")
    assert len(r.content) > 0


def test_bundle_unknown_version_404(app_and_creds):
    client, _, _ = app_and_creds
    r = client.get("/api/v1/secrets/bundle?profile=child&version=v9.nope")
    assert r.status_code == 404


def test_bundle_invalid_profile_422(app_and_creds):
    client, _, _ = app_and_creds
    r = client.get("/api/v1/secrets/bundle?profile=teenz&version=v1")
    assert r.status_code == 422


# ===== /wrap =====

def test_wrap_requires_auth(app_and_creds):
    client, _, _ = app_and_creds
    r = client.post("/api/v1/secrets/wrap?profile=child&version=v1.test")
    assert r.status_code == 401


def test_wrap_returns_ciphertext_with_iv(app_and_creds):
    client, token, _ = app_and_creds
    r = client.post(
        f"/api/v1/secrets/wrap?profile=child&version=v1.test",
        headers={"Authorization": f"Bearer {token}"},
    )
    assert r.status_code == 200
    data = r.json()
    assert data["profile"] == "child"
    assert data["version"] == "v1.test"
    assert data["algo"] == "AES-256-GCM"
    assert data["kdf"] == "HKDF-SHA256"
    # 字段名是 iv (不是 iv_b64)
    assert "iv" in data
    # iv 长度 12 bytes → base64 16 chars
    iv = base64.b64decode(data["iv"])
    assert len(iv) == 12
    # ciphertext 至少 32 字节 (master_key) + 16 字节 tag
    ct = base64.b64decode(data["ciphertext_b64"])
    assert len(ct) >= 48


def test_wrap_default_version_uses_latest(app_and_creds):
    client, token, _ = app_and_creds
    r = client.post(
        "/api/v1/secrets/wrap?profile=child",
        headers={"Authorization": f"Bearer {token}"},
    )
    assert r.status_code == 200
    assert r.json()["version"] == "v1.test"


def test_wrap_then_unwrap_recovers_master_key(app_and_creds):
    """端到端: client 拿 wrap, 用 license_token 派生 KEK, 解开拿到 master_key."""
    client, token, master_key = app_and_creds
    r = client.post(
        "/api/v1/secrets/wrap?profile=child&version=v1.test",
        headers={"Authorization": f"Bearer {token}"},
    )
    assert r.status_code == 200
    data = r.json()

    # 客户端派生 KEK (与 server 同样参数)
    kek = _derive_kek(token)
    aes = AESGCM(kek)
    iv = base64.b64decode(data["iv"])
    ct = base64.b64decode(data["ciphertext_b64"])
    # AAD = license_token 原文
    master_recovered = aes.decrypt(iv, ct, token.encode())
    assert master_recovered == master_key
    assert len(master_recovered) == 32


def test_wrap_different_tokens_get_different_keks(app_and_creds, monkeypatch, tmp_path):
    """device-A 的 wrap 不能用 device-B 的 license_token 解开."""
    client, token_a, master_key = app_and_creds

    # 激活 device-B
    db_path = tmp_path / "kidsai.db"
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": "e" * 64, "nickname": "B", "ageTier": 3},
    )
    token_b = r.json()["licenseToken"]

    # 拿 device-A 的 wrap
    r = client.post(
        "/api/v1/secrets/wrap?profile=child&version=v1.test",
        headers={"Authorization": f"Bearer {token_a}"},
    )
    data = r.json()
    ct = base64.b64decode(data["ciphertext_b64"])
    iv = base64.b64decode(data["iv"])

    # 用 device-B 的 token 派生 KEK → 应该解不开
    kek_b = _derive_kek(token_b)
    aes_b = AESGCM(kek_b)
    from cryptography.exceptions import InvalidTag
    with pytest.raises(InvalidTag):
        aes_b.decrypt(iv, ct, token_b.encode())


def test_end_to_end_unwrap_decrypt_split(app_and_creds):
    """完整链路: wrap → unwrap master → decrypt bundle → split → 拿到原文."""
    client, token, master_key = app_and_creds

    # 1. wrap
    r = client.post(
        "/api/v1/secrets/wrap?profile=child&version=v1.test",
        headers={"Authorization": f"Bearer {token}"},
    )
    wrap_data = r.json()

    # 2. manifest
    r = client.get("/api/v1/secrets/manifest?profile=child")
    manifest = r.json()

    # 3. bundle
    r = client.get("/api/v1/secrets/bundle?profile=child&version=v1.test")
    bundle_bytes = r.content

    # 4. unwrap master
    kek = _derive_kek(token)
    aes = AESGCM(kek)
    iv = base64.b64decode(wrap_data["iv"])
    ct = base64.b64decode(wrap_data["ciphertext_b64"])
    master = aes.decrypt(iv, ct, token.encode())
    assert master == master_key

    # 5. decrypt bundle
    bundle_iv = base64.b64decode(manifest["cipher"]["iv"])
    bundle_aes = AESGCM(master)
    plaintext = bundle_aes.decrypt(bundle_iv, bundle_bytes, None)

    # 6. sha256 校验
    import hashlib
    actual_sha = hashlib.sha256(plaintext).hexdigest()
    assert actual_sha == manifest["cipher"]["plaintext_sha256"]

    # 7. 找到 director.yaml 原文
    assert b"role: child director" in plaintext
    assert b"blocked: []" in plaintext


def test_wrap_invalid_profile_422(app_and_creds):
    client, token, _ = app_and_creds
    r = client.post(
        f"/api/v1/secrets/wrap?profile=alien&version=v1.test",
        headers={"Authorization": f"Bearer {token}"},
    )
    assert r.status_code == 422


def test_wrap_404_when_version_missing(app_and_creds):
    client, token, _ = app_and_creds
    r = client.post(
        f"/api/v1/secrets/wrap?profile=child&version=v99.nope",
        headers={"Authorization": f"Bearer {token}"},
    )
    assert r.status_code == 404
