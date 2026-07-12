"""W6 B2: 资产 manifest endpoint.

GET /api/v1/asset-manifest
  → 读 assets/asset_manifest.json, 加 1h cache + ETag.
  → 返 {version, images: {key: full_url}}.

设计:
- 无 auth (公开只读, 资产是公开素材).
- 路径: ASSETS_DIR env 配 (默认 <project_root>/assets/asset_manifest.json).
- full_url = ASSETS_BASE_URL env + "/" + rel_path (nginx 在那个域)
- 找不到 manifest → 返 503 (assets 还没跑批), 不是 404 — 让前端能区分
  "还没生成" vs "key 真的不存在".
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path

from fastapi import APIRouter, HTTPException, Response

router = APIRouter(prefix="/api/v1", tags=["asset"])


def _manifest_path() -> Path:
    raw = os.getenv("ASSETS_DIR", "").strip()
    if raw:
        return Path(raw) / "asset_manifest.json"
    # 默认 <server 包目录>/../../assets/asset_manifest.json
    return Path(__file__).resolve().parents[2] / "assets" / "asset_manifest.json"


def _base_url() -> str:
    return os.getenv("ASSETS_BASE_URL", "https://assets.kids.ibi.ren").rstrip("/")


def _load_manifest() -> dict:
    p = _manifest_path()
    if not p.exists():
        raise HTTPException(503, "asset manifest not generated yet")
    try:
        return json.loads(p.read_text(encoding="utf-8"))
    except Exception as e:
        raise HTTPException(500, f"manifest parse: {e}") from e


@router.get("/asset-manifest")
def get_asset_manifest(response: Response) -> dict:
    """无 auth. 1h cache + ETag. 桌面端启动时拉一次."""
    raw = _load_manifest()
    images = raw.get("images") or {}
    base = _base_url()
    full = {k: f"{base}/{v}" for k, v in images.items()}
    payload = {
        "version": raw.get("version", 0),
        "generated_count": raw.get("generated_count", len(full)),
        "images": full,
    }
    body = json.dumps(payload, ensure_ascii=False, sort_keys=True).encode("utf-8")
    etag = hashlib.md5(body).hexdigest()
    response.headers["Cache-Control"] = "public, max-age=3600"
    response.headers["ETag"] = f'"{etag}"'
    response.headers["Content-Type"] = "application/json; charset=utf-8"
    return payload