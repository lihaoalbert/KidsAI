"""W6 B2: asset manifest endpoint 测试.

覆盖:
- 无 manifest → 503
- 有 manifest → 200, 全 URL 用 ASSETS_BASE_URL 拼接
- 1h cache + ETag 一致性
- version / generated_count 透传
"""
from __future__ import annotations

import json
import os
from pathlib import Path

import pytest
from fastapi.testclient import TestClient


@pytest.fixture()
def manifest_dir(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    d = tmp_path / "assets"
    d.mkdir()
    monkeypatch.setenv("ASSETS_DIR", str(d))
    monkeypatch.setenv("ASSETS_BASE_URL", "https://assets.kids.ibi.ren")
    return d


@pytest.fixture()
def client(manifest_dir: Path):
    from kidsai_server.main import create_app
    from kidsai_server.config import Config

    cfg = Config(
        jwt_secret="x" * 32,
        jwt_ttl_seconds=86400,
        admin_token="adm" * 8,
        starting_balance=100,
        daily_quota=30,
        cost_per_llm_token=0.001,
        cost_video_draft=9,
        cost_video_final=19,
        single_tx_cap=20,
        database_path=str(manifest_dir.parent / "kidsai.db"),
        llm_api_key="",
        seedance_api_key="",
        port=8080,
        minimax_api_keys=("k1",),
        cost_image_gen=5,
        cost_voice_clone=10,
        cost_music_gen=8,
        cost_hailuo_video=12,
    )
    app = create_app(cfg=cfg, db_path=cfg.database_path)
    return TestClient(app)


def _write_manifest(d: Path, payload: dict) -> None:
    (d / "asset_manifest.json").write_text(json.dumps(payload), encoding="utf-8")


def test_missing_manifest_returns_503(client: TestClient) -> None:
    r = client.get("/api/v1/asset-manifest")
    assert r.status_code == 503
    assert "not generated" in r.json()["detail"].lower()


def test_returns_full_urls(client: TestClient, manifest_dir: Path) -> None:
    _write_manifest(manifest_dir, {
        "version": 1700000000,
        "generated_count": 3,
        "images": {
            "xiaoqi.stand": "character/xiaoqi.stand.png",
            "xiaoqi.sit": "character/xiaoqi.sit.png",
            "l1.my_ai_companion": "bg/l1.my_ai_companion.png",
        },
    })
    r = client.get("/api/v1/asset-manifest")
    assert r.status_code == 200
    body = r.json()
    assert body["version"] == 1700000000
    assert body["generated_count"] == 3
    assert body["images"]["xiaoqi.stand"] == "https://assets.kids.ibi.ren/character/xiaoqi.stand.png"
    assert body["images"]["l1.my_ai_companion"] == "https://assets.kids.ibi.ren/bg/l1.my_ai_companion.png"
    # cache + etag headers
    assert "max-age=3600" in r.headers["cache-control"]
    assert r.headers["etag"].startswith('"') and r.headers["etag"].endswith('"')


def test_no_auth_required(client: TestClient, manifest_dir: Path) -> None:
    _write_manifest(manifest_dir, {"version": 1, "images": {}})
    r = client.get("/api/v1/asset-manifest")
    assert r.status_code == 200
    assert r.json()["images"] == {}


def test_etag_is_stable_for_same_payload(client: TestClient, manifest_dir: Path) -> None:
    _write_manifest(manifest_dir, {"version": 1, "images": {"a": "x/a.png"}})
    r1 = client.get("/api/v1/asset-manifest")
    r2 = client.get("/api/v1/asset-manifest")
    assert r1.headers["etag"] == r2.headers["etag"]


def test_corrupt_manifest_returns_500(client: TestClient, manifest_dir: Path) -> None:
    (manifest_dir / "asset_manifest.json").write_text("{ not valid json", encoding="utf-8")
    r = client.get("/api/v1/asset-manifest")
    assert r.status_code == 500


def test_base_url_trailing_slash_stripped(client: TestClient, manifest_dir: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("ASSETS_BASE_URL", "https://assets.kids.ibi.ren/")
    _write_manifest(manifest_dir, {"version": 1, "images": {"k": "v.png"}})
    r = client.get("/api/v1/asset-manifest")
    assert r.json()["images"]["k"] == "https://assets.kids.ibi.ren/v.png"


def test_empty_images_payload_passes_through(client: TestClient, manifest_dir: Path) -> None:
    _write_manifest(manifest_dir, {})  # no images key
    r = client.get("/api/v1/asset-manifest")
    assert r.status_code == 200
    body = r.json()
    assert body["images"] == {}
    assert body["version"] == 0
    assert body["generated_count"] == 0