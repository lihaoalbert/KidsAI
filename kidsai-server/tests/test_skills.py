"""W10 — Skills Marketplace API + set-mode endpoint (server side) tests.

覆盖:
- GET  /api/v1/skills/index  (按 audience + device.mode 过滤)
- GET  /api/v1/skills/{id}/manifest (单 manifest 拉取)
- GET  /api/v1/skills/{id}/blob (单文件下载, 路径越权防护, manifest 白名单)
- POST /api/v1/skills/install-authorize (PIN 必填 + 未知 skill_id 拒绝)
- GET  /api/v1/me/mode (默认 child)
- POST /api/v1/me/set-mode (PIN 必填, 写 devices.mode)
- audience 双向过滤 (child mode 看不到 adult skill, adult mode 看不到 child skill)
- mode 切换持久化 (重新读 GET /me/mode 应返回新 mode)
"""
from __future__ import annotations

import json
from dataclasses import replace
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from kidsai_server.main import create_app


def _activate(client: TestClient) -> str:
    """Activate 拿 license_token."""
    r = client.post(
        "/api/v1/devices/activate",
        json={
            "fingerprintHash": "f" * 64,
            "nickname": "测试娃",
            "ageTier": 2,
        },
    )
    assert r.status_code == 200, r.text
    return r.json()["licenseToken"]


def _auth(token: str) -> dict[str, str]:
    return {"Authorization": f"Bearer {token}"}


def _write_skill(skills_root: Path, skill_id: str, version: str, audience: str) -> None:
    """写一个最小 skill manifest + 一个 asset."""
    skill_dir = skills_root / skill_id / version
    skill_dir.mkdir(parents=True, exist_ok=True)
    manifest = {
        "schema": "kidsai.skill/1",
        "id": skill_id,
        "name": skill_id,
        "version": version,
        "audience": audience,
        "age_tier": [1, 2],
        "category": "language",
        "assets": [
            {"path": "assets/cover.png", "sha256": "abc123", "size": 100},
        ],
        "prompts": [],
        "templates": {"characters": [], "story_arcs": []},
        "extends": {"tabs": [], "tools": []},
    }
    (skill_dir / "manifest.json").write_text(json.dumps(manifest))
    (skill_dir / "assets").mkdir(exist_ok=True)
    (skill_dir / "assets" / "cover.png").write_bytes(b"FAKE PNG DATA")


@pytest.fixture()
def skills_root(tmp_path: Path) -> Path:
    d = tmp_path / "skills"
    d.mkdir()
    return d


@pytest.fixture()
def skills_env(cfg, temp_db_path: str, skills_root: Path):
    """返回 (cfg_with_skills_root, db_path) — 测试自己 create_app."""
    custom_cfg = replace(cfg, skills_root=str(skills_root))
    return custom_cfg, temp_db_path


# ---- /skills/index ----

def test_skills_index_empty(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.get("/api/v1/skills/index", headers=_auth(token))
        assert r.status_code == 200
        body = r.json()
        assert body["skills"] == []
        assert body["mode"] == "child"


def test_skills_index_lists_child_skills_in_child_mode(skills_env):
    cfg, db = skills_env
    sr = Path(cfg.skills_root)
    _write_skill(sr, "eng-adventure", "v1", audience="child")
    _write_skill(sr, "ink-painting", "v1", audience="child")
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.get("/api/v1/skills/index", headers=_auth(token))
        ids = [s["id"] for s in r.json()["skills"]]
        assert "eng-adventure" in ids
        assert "ink-painting" in ids


def test_skills_index_hides_adult_skills_in_child_mode(skills_env):
    cfg, db = skills_env
    sr = Path(cfg.skills_root)
    _write_skill(sr, "commercial-ad", "v1", audience="adult")
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.get("/api/v1/skills/index", headers=_auth(token))
        ids = [s["id"] for s in r.json()["skills"]]
        assert "commercial-ad" not in ids
        assert r.json()["mode"] == "child"


def test_skills_index_shows_adult_skills_in_adult_mode(skills_env):
    cfg, db = skills_env
    sr = Path(cfg.skills_root)
    _write_skill(sr, "commercial-ad", "v1", audience="adult")
    _write_skill(sr, "doc-shortfilm", "v1", audience="adult")
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        c.post(
            "/api/v1/me/set-mode",
            headers=_auth(token),
            json={"mode": "adult", "parentPinProof": "pin-1234"},
        )
        r = c.get("/api/v1/skills/index", headers=_auth(token))
        ids = [s["id"] for s in r.json()["skills"]]
        assert "commercial-ad" in ids
        assert "doc-shortfilm" in ids
        assert r.json()["mode"] == "adult"


def test_skills_index_hides_child_skills_in_adult_mode(skills_env):
    """成人模式默认不显示儿童 skill — 设计: 避免成人专业用户被儿童内容打扰."""
    cfg, db = skills_env
    sr = Path(cfg.skills_root)
    _write_skill(sr, "eng-adventure", "v1", audience="child")
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        c.post(
            "/api/v1/me/set-mode",
            headers=_auth(token),
            json={"mode": "adult", "parentPinProof": "pin-1234"},
        )
        r = c.get("/api/v1/skills/index", headers=_auth(token))
        ids = [s["id"] for s in r.json()["skills"]]
        assert "eng-adventure" not in ids


def test_skills_index_both_audience_always_visible(skills_env):
    """audience=both 在 child + adult 两种 mode 都出现."""
    cfg, db = skills_env
    sr = Path(cfg.skills_root)
    _write_skill(sr, "ink-painting", "v1", audience="both")
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        # child mode
        r = c.get("/api/v1/skills/index", headers=_auth(token))
        assert "ink-painting" in [s["id"] for s in r.json()["skills"]]
        # 切 adult
        c.post(
            "/api/v1/me/set-mode",
            headers=_auth(token),
            json={"mode": "adult", "parentPinProof": "pin-1234"},
        )
        r = c.get("/api/v1/skills/index", headers=_auth(token))
        assert "ink-painting" in [s["id"] for s in r.json()["skills"]]


def test_skills_index_requires_auth(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        r = c.get("/api/v1/skills/index")
        assert r.status_code == 401


# ---- /skills/{id}/manifest ----

def test_get_skill_manifest(skills_env):
    cfg, db = skills_env
    sr = Path(cfg.skills_root)
    _write_skill(sr, "eng-adventure", "v1", audience="child")
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.get("/api/v1/skills/eng-adventure/manifest", headers=_auth(token))
        assert r.status_code == 200
        m = r.json()
        assert m["id"] == "eng-adventure"
        assert m["audience"] == "child"


def test_get_skill_manifest_404_unknown(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.get("/api/v1/skills/nonexistent/manifest", headers=_auth(token))
        assert r.status_code == 404


# ---- /skills/{id}/blob ----

def test_get_skill_blob(skills_env):
    cfg, db = skills_env
    sr = Path(cfg.skills_root)
    _write_skill(sr, "eng-adventure", "v1", audience="child")
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.get(
            "/api/v1/skills/eng-adventure/blob",
            params={"file": "assets/cover.png"},
            headers=_auth(token),
        )
        assert r.status_code == 200
        assert r.content == b"FAKE PNG DATA"


def test_get_skill_blob_rejects_path_traversal(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.get(
            "/api/v1/skills/eng-adventure/blob",
            params={"file": "../etc/passwd"},
            headers=_auth(token),
        )
        assert r.status_code == 400


def test_get_skill_blob_rejects_unlisted_file(skills_env):
    cfg, db = skills_env
    sr = Path(cfg.skills_root)
    _write_skill(sr, "eng-adventure", "v1", audience="child")
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        # secret.yaml 不在 manifest.assets 里
        r = c.get(
            "/api/v1/skills/eng-adventure/blob",
            params={"file": "secret.yaml"},
            headers=_auth(token),
        )
        assert r.status_code == 403


# ---- /skills/install-authorize ----

def test_install_authorize_requires_pin(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.post(
            "/api/v1/skills/install-authorize",
            headers=_auth(token),
            json={"skillId": "eng-adventure", "parentPinProof": ""},
        )
        assert r.status_code == 400


def test_install_authorize_unknown_skill(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.post(
            "/api/v1/skills/install-authorize",
            headers=_auth(token),
            json={"skillId": "nonexistent", "parentPinProof": "1234"},
        )
        assert r.status_code == 404


def test_install_authorize_happy_path(skills_env):
    cfg, db = skills_env
    sr = Path(cfg.skills_root)
    _write_skill(sr, "eng-adventure", "v1", audience="child")
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.post(
            "/api/v1/skills/install-authorize",
            headers=_auth(token),
            json={"skillId": "eng-adventure", "parentPinProof": "1234"},
        )
        assert r.status_code == 200
        body = r.json()
        assert body["authorized"] is True
        assert body["skillId"] == "eng-adventure"
        assert body["receiptId"].startswith("auth-")


# ---- /me/mode ----

def test_get_mode_default_child(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.get("/api/v1/me/mode", headers=_auth(token))
        assert r.status_code == 200
        body = r.json()
        assert body["mode"] == "child"
        assert "deviceId" in body


# ---- /me/set-mode ----

def test_set_mode_requires_pin(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.post(
            "/api/v1/me/set-mode",
            headers=_auth(token),
            json={"mode": "adult", "parentPinProof": ""},
        )
        assert r.status_code == 400


def test_set_mode_rejects_invalid_value(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.post(
            "/api/v1/me/set-mode",
            headers=_auth(token),
            json={"mode": "god", "parentPinProof": "1234"},
        )
        assert r.status_code == 422  # pydantic pattern 校验


def test_set_mode_to_adult_persists(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        r = c.post(
            "/api/v1/me/set-mode",
            headers=_auth(token),
            json={"mode": "adult", "parentPinProof": "1234"},
        )
        assert r.status_code == 200
        body = r.json()
        assert body["mode"] == "adult"
        assert body["switchedAt"] > 0
        # 再次读取
        r2 = c.get("/api/v1/me/mode", headers=_auth(token))
        assert r2.json()["mode"] == "adult"


def test_set_mode_back_to_child_persists(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        c.post(
            "/api/v1/me/set-mode",
            headers=_auth(token),
            json={"mode": "adult", "parentPinProof": "1234"},
        )
        r = c.post(
            "/api/v1/me/set-mode",
            headers=_auth(token),
            json={"mode": "child", "parentPinProof": "1234"},
        )
        assert r.status_code == 200
        assert r.json()["mode"] == "child"
        r2 = c.get("/api/v1/me/mode", headers=_auth(token))
        assert r2.json()["mode"] == "child"


def test_set_mode_requires_auth(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        r = c.post(
            "/api/v1/me/set-mode",
            json={"mode": "adult", "parentPinProof": "1234"},
        )
        assert r.status_code == 401


# ---- 端到端: child → adult 切换后, 重新 activate 应保留 adult mode? ----
# 设计: activate 总是从 default child 起步 — 历史 mode 由 devices.mode 持久化.
# 此 case 验证 db 跨连接持久化.

def test_mode_persists_across_db_connection(skills_env):
    cfg, db = skills_env
    app = create_app(cfg=cfg, db_path=db)
    with TestClient(app) as c:
        token = _activate(c)
        c.post(
            "/api/v1/me/set-mode",
            headers=_auth(token),
            json={"mode": "adult", "parentPinProof": "1234"},
        )
    # 重建 app 模拟重启
    app2 = create_app(cfg=cfg, db_path=db)
    with TestClient(app2) as c:
        r = c.post(
            "/api/v1/devices/refresh-license",
            headers=_auth(token),
            json={},
        )
        # refresh 应能拿到 — 说明 token 还有效
        # 然后查 mode
        r = c.get("/api/v1/me/mode", headers=_auth(token))
        assert r.json()["mode"] == "adult"