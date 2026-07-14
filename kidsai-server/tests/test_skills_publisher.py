"""W10 Day 5 — skills_publisher tests.

覆盖:
- placeholder_png 是合法 PNG (PNG signature + 稳定)
- manifest_canonical 移除 publisher_signature 后序列化稳定
- sign_canonical + 独立 verify 闭环
- publish 写出 manifest + assets/cover.png + prompts/*.yaml
- manifest schema 字段齐全
- 6 个种子 skill 数量: 3 child + 3 adult
- 篡改 manifest → 签名验证失败
- manifest.assets[0].sha256 与实际 cover.png 字节匹配
"""

import base64
import hashlib
import json
from pathlib import Path

import pytest
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding, rsa

from kidsai_server import skills_publisher as sp
from kidsai_server.skills_publisher import (
    DEFAULT_VERSION,
    PUBKEY_ID,
    PUBLISHER,
    SCHEMA_VERSION,
    SEED_SKILLS,
    build_manifest,
    manifest_canonical,
    placeholder_png,
    publish,
    sign_canonical,
)


@pytest.fixture
def keypair() -> tuple[rsa.RSAPrivateKey, rsa.RSAPublicKey]:
    priv = rsa.generate_private_key(65537, 2048)
    return priv, priv.public_key()


@pytest.fixture
def skills_root(tmp_path: Path) -> Path:
    return tmp_path / "skills"


@pytest.fixture
def patched_publish(monkeypatch, keypair):
    """用临时 keypair 替换 load_signing_key, 避免污染 dev keypair."""
    priv, _ = keypair
    monkeypatch.setattr(sp, "load_signing_key", lambda: priv)
    return priv


def _verify_with_pubkey(skills_root: Path, pub: rsa.RSAPublicKey) -> bool:
    """用给定 pubkey 校验 skills_root 下所有 manifest 签名."""
    ok = True
    for skill_dir in sorted(skills_root.iterdir()):
        if not skill_dir.is_dir():
            continue
        for ver_dir in sorted(skill_dir.iterdir()):
            if not ver_dir.is_dir():
                continue
            mp = ver_dir / "manifest.json"
            if not mp.is_file():
                continue
            m = json.loads(mp.read_text())
            sig_b64 = m.pop("publisher_signature", "")
            canonical = manifest_canonical(m)
            try:
                pub.verify(
                    base64.b64decode(sig_b64),
                    canonical,
                    padding.PSS(
                        mgf=padding.MGF1(hashes.SHA256()),
                        salt_length=padding.PSS.MAX_LENGTH,
                    ),
                    hashes.SHA256(),
                )
            except Exception:
                ok = False
            m["publisher_signature"] = sig_b64
    return ok


# ===== placeholder_png =====

def test_placeholder_png_has_png_signature():
    png = placeholder_png()
    assert png[:8] == b"\x89PNG\r\n\x1a\n", "not a valid PNG signature"


def test_placeholder_png_is_stable_across_calls():
    a = placeholder_png()
    b = placeholder_png()
    assert a == b
    assert hashlib.sha256(a).hexdigest() == hashlib.sha256(b).hexdigest()


# ===== manifest_canonical =====

def test_manifest_canonical_strips_publisher_signature():
    m = {"id": "x", "name": "X", "publisher_signature": "SHOULD-BE-REMOVED"}
    canonical = manifest_canonical(m)
    assert b"publisher_signature" not in canonical
    assert b"SHOULD-BE-REMOVED" not in canonical


def test_manifest_canonical_is_deterministic():
    m = {"id": "x", "version": "v1", "assets": [{"path": "a.png", "sha256": "abc", "size": 1}]}
    assert manifest_canonical(m) == manifest_canonical(m)


def test_manifest_canonical_sorts_keys():
    canonical = manifest_canonical({"z": 1, "a": 2, "m": 3})
    decoded = json.loads(canonical)
    assert list(decoded.keys()) == sorted(decoded.keys())


# ===== sign_canonical + verify =====

def test_sign_canonical_roundtrips(keypair):
    priv, pub = keypair
    canonical = b'{"id":"x","v":1}'
    sig_b64 = sign_canonical(canonical, priv)
    pub.verify(
        base64.b64decode(sig_b64),
        canonical,
        padding.PSS(mgf=padding.MGF1(hashes.SHA256()), salt_length=padding.PSS.MAX_LENGTH),
        hashes.SHA256(),
    )


def test_sign_canonical_rejects_tampered(keypair):
    priv, pub = keypair
    sig_b64 = sign_canonical(b'{"id":"x"}', priv)
    with pytest.raises(Exception):
        pub.verify(
            base64.b64decode(sig_b64),
            b'{"id":"y"}',
            padding.PSS(mgf=padding.MGF1(hashes.SHA256()), salt_length=padding.PSS.MAX_LENGTH),
            hashes.SHA256(),
        )


def test_sign_canonical_rejects_wrong_keypair(keypair):
    priv_a, _ = keypair
    _, pub_b = _gen_keypair()
    canonical = b"{}"
    sig_b64 = sign_canonical(canonical, priv_a)
    with pytest.raises(Exception):
        pub_b.verify(
            base64.b64decode(sig_b64),
            canonical,
            padding.PSS(mgf=padding.MGF1(hashes.SHA256()), salt_length=padding.PSS.MAX_LENGTH),
            hashes.SHA256(),
        )


# ===== build_manifest =====

def test_build_manifest_has_required_fields(keypair):
    priv, _ = keypair
    m = build_manifest(SEED_SKILLS[0], "v1.test", priv)
    required = [
        "schema", "id", "name", "version", "publisher",
        "min_app_version", "age_tier", "category", "audience",
        "assets", "prompts", "templates", "extends",
        "credits_per_use", "daily_quota", "size_bytes",
        "publisher_signature", "publisher_pubkey_id",
    ]
    for k in required:
        assert k in m, f"missing: {k}"
    assert m["schema"] == SCHEMA_VERSION
    assert m["publisher"] == PUBLISHER
    assert m["publisher_pubkey_id"] == PUBKEY_ID
    assert m["publisher_signature"]


def test_build_manifest_audience_per_skill(keypair):
    priv, _ = keypair
    for seed in SEED_SKILLS:
        m = build_manifest(seed, "v1", priv)
        assert m["audience"] == seed["audience"]
        assert m["id"] == seed["id"]


def test_build_manifest_signature_verifies_with_correct_pubkey(keypair):
    priv, pub = keypair
    m = build_manifest(SEED_SKILLS[0], "v1.test", priv)
    canonical = manifest_canonical(m)
    pub.verify(
        base64.b64decode(m["publisher_signature"]),
        canonical,
        padding.PSS(mgf=padding.MGF1(hashes.SHA256()), salt_length=padding.PSS.MAX_LENGTH),
        hashes.SHA256(),
    )


# ===== SEED_SKILLS invariants =====

def test_seed_skills_count():
    assert len(SEED_SKILLS) == 6


def test_seed_skills_three_child_three_adult():
    audiences = [s["audience"] for s in SEED_SKILLS]
    assert audiences.count("child") == 3
    assert audiences.count("adult") == 3


def test_seed_skills_unique_ids():
    ids = [s["id"] for s in SEED_SKILLS]
    assert len(set(ids)) == 6, f"duplicate ids: {ids}"


def test_seed_skills_all_have_required_fields():
    for s in SEED_SKILLS:
        for f in [
            "id", "name", "audience", "category", "age_tier",
            "credits_per_use", "daily_quota", "description",
            "characters", "story_arcs", "tabs", "tools", "prompts",
        ]:
            assert f in s, f"seed {s.get('id', '?')} missing {f}"


def test_seed_skills_expected_ids():
    ids = {s["id"] for s in SEED_SKILLS}
    expected = {
        "eng-adventure", "ink-painting", "coding-primer",  # child
        "commercial-ad-director", "doc-shortfilm", "resume-reel",  # adult
    }
    assert ids == expected


# ===== publish (写盘 + 签名) =====

def test_publish_writes_manifest_assets_prompts(skills_root, patched_publish):
    written = publish(skills_root, "v1.0.0")
    assert len(written) == 6
    # 抽样: eng-adventure 应有 manifest + cover.png + 2 prompts
    e = skills_root / "eng-adventure" / "v1.0.0"
    assert (e / "manifest.json").is_file()
    assert (e / "assets" / "cover.png").is_file()
    assert (e / "prompts" / "opening.yaml").is_file()
    assert (e / "prompts" / "grammar.yaml").is_file()


def test_published_manifest_sha256_matches_cover_png(skills_root, patched_publish):
    publish(skills_root, "v1.0.0")
    for skill_dir in skills_root.iterdir():
        ver_dir = skill_dir / "v1.0.0"
        m = json.loads((ver_dir / "manifest.json").read_text())
        cover = ver_dir / "assets" / "cover.png"
        actual_sha = hashlib.sha256(cover.read_bytes()).hexdigest()
        entry = next(a for a in m["assets"] if a["path"] == "assets/cover.png")
        assert entry["sha256"] == actual_sha, f"sha mismatch in {skill_dir.name}"


def test_publish_then_verify_all_signatures_valid(skills_root, patched_publish, keypair):
    _, pub = keypair
    publish(skills_root, "v1.0.0")
    assert _verify_with_pubkey(skills_root, pub) is True


def test_tampered_manifest_fails_verify(skills_root, patched_publish, keypair):
    _, pub = keypair
    publish(skills_root, "v1.0.0")
    # 改 eng-adventure manifest 的 version
    target = skills_root / "eng-adventure" / "v1.0.0" / "manifest.json"
    m = json.loads(target.read_text())
    m["version"] = "v9.tampered"
    target.write_text(json.dumps(m, indent=2, ensure_ascii=False))
    assert _verify_with_pubkey(skills_root, pub) is False


def test_tampered_cover_png_fails_sha_check(skills_root, patched_publish):
    """改 cover.png 字节 → manifest.assets[0].sha256 不再匹配 (本测试只断言 sha 不一致)."""
    publish(skills_root, "v1.0.0")
    target = skills_root / "eng-adventure" / "v1.0.0" / "assets" / "cover.png"
    target.write_bytes(b"NOT-A-PNG-AT-ALL")
    m = json.loads((skills_root / "eng-adventure" / "v1.0.0" / "manifest.json").read_text())
    cover_entry = next(a for a in m["assets"] if a["path"] == "assets/cover.png")
    actual = hashlib.sha256(target.read_bytes()).hexdigest()
    assert cover_entry["sha256"] != actual, "tampered png should mismatch manifest sha256"


# ===== helpers =====

def _gen_keypair():
    priv = rsa.generate_private_key(65537, 2048)
    return priv, priv.public_key()


# ===== 模块常量 sanity =====

def test_default_version_is_string():
    assert isinstance(DEFAULT_VERSION, str)
    assert DEFAULT_VERSION  # non-empty


def test_schema_version_is_kidsai_skill_1():
    assert SCHEMA_VERSION == "kidsai.skill/1"


def test_publisher_id():
    assert PUBLISHER == "kidsai-official"
    assert PUBKEY_ID
