"""W10 server-side — Skills API + set-mode endpoint (Part C).

Endpoints:
- GET  /api/v1/skills/index              — list available skills (filtered by device.mode)
- GET  /api/v1/skills/{id}/manifest      — signed manifest
- GET  /api/v1/skills/{id}/blob?file=... — single file download
- POST /api/v1/skills/install-authorize  — parent PIN verified server-side (Day 4 full impl)
- POST /api/v1/me/set-mode               — Part C: switch Child/Adult

Storage: skills 走文件系统 (config.skills_root/), secrets 走文件系统 (config.secrets_root/).
每个 skill 是 {skills_root}/{id}/{version}/manifest.json + 任意路径的 asset/prompt 文件.
"""

from __future__ import annotations

import base64
import json
import sqlite3
import time
from pathlib import Path
from typing import Annotated

from fastapi import APIRouter, Depends, Header, HTTPException, Query, Request
from pydantic import BaseModel, ConfigDict, Field

from ..auth import require_license
from ..dependencies import get_cfg, get_conn
from ..config import Config


router = APIRouter(prefix="/api/v1", tags=["skills"])

_CAMEL = ConfigDict(
    alias_generator=lambda s: s if "_" not in s else s.split("_")[0] + "".join(p.title() for p in s.split("_")[1:]),
    populate_by_name=True,
    extra="forbid",
)


# ---- request/response schemas ----

class SetModeRequest(BaseModel):
    model_config = _CAMEL
    mode: str = Field(pattern="^(child|adult)$")
    parent_pin_proof: str  # Day 4 接 argon2 hash 校验; 此处先收非空


class SetModeResponse(BaseModel):
    model_config = _CAMEL
    device_id: str
    mode: str
    switched_at: int


class InstallAuthorizeRequest(BaseModel):
    model_config = _CAMEL
    skill_id: str = Field(min_length=1, max_length=64)
    parent_pin_proof: str


class InstallAuthorizeResponse(BaseModel):
    model_config = _CAMEL
    skill_id: str
    authorized: bool
    receipt_id: str


# ---- helper: device.mode ----

def _read_device_mode(conn: sqlite3.Connection, device_id: str) -> str:
    _ensure_mode_column(conn)
    row = conn.execute(
        "SELECT mode FROM devices WHERE id = ?", (device_id,)
    ).fetchone()
    if row is None:
        return "child"
    return (row["mode"] if "mode" in row.keys() else None) or "child"


def _ensure_mode_column(conn: sqlite3.Connection) -> None:
    """第一次启动要 ALTER TABLE; 幂等."""
    cols = {r["name"] for r in conn.execute("PRAGMA table_info(devices)").fetchall()}
    if "mode" not in cols:
        conn.execute("ALTER TABLE devices ADD COLUMN mode TEXT NOT NULL DEFAULT 'child'")
        conn.commit()


def _write_device_mode(conn: sqlite3.Connection, device_id: str, mode: str) -> None:
    _ensure_mode_column(conn)
    conn.execute("UPDATE devices SET mode = ? WHERE id = ?", (mode, device_id))
    conn.commit()


def _ensure_parent_pin_column(conn: sqlite3.Connection) -> None:
    cols = {r["name"] for r in conn.execute("PRAGMA table_info(devices)").fetchall()}
    if "parent_pin_hash" not in cols:
        conn.execute("ALTER TABLE devices ADD COLUMN parent_pin_hash TEXT")
        conn.commit()


# ---- /api/v1/skills/index ----

@router.get("/skills/index")
def list_skills_index(
    request: Request,
    cfg: Annotated[Config, Depends(get_cfg)],
    claims=Depends(require_license),
):
    """列出可用 skills. 按 device.mode 过滤 audience 不匹配的:
    - child 模式 → 只看 audience ∈ {child, both}
    - adult 模式 → 只看 audience ∈ {adult, both}
    """
    mode = _read_device_mode(request.app.state.db, claims.device_id)
    skills_root = Path(cfg.skills_root)
    if not skills_root.is_dir():
        return {"skills": [], "mode": mode}

    items = []
    for skill_dir in sorted(skills_root.iterdir()):
        if not skill_dir.is_dir():
            continue
        # 找最新 version
        manifest = _find_latest_manifest(skill_dir)
        if manifest is None:
            continue
        audience = manifest.get("audience", "child")
        if not _audience_matches(audience, mode):
            continue
        items.append(_manifest_to_index_item(manifest, skill_dir.name))

    return {"skills": items, "mode": mode}


@router.get("/skills/{skill_id}/manifest")
def get_skill_manifest(
    skill_id: str,
    request: Request,
    cfg: Annotated[Config, Depends(get_cfg)],
    claims=Depends(require_license),
):
    skills_root = Path(cfg.skills_root)
    skill_dir = skills_root / skill_id
    manifest = _find_latest_manifest(skill_dir)
    if manifest is None:
        raise HTTPException(404, f"skill {skill_id} not found")
    return manifest


@router.get("/skills/{skill_id}/blob")
def get_skill_blob(
    skill_id: str,
    request: Request,
    cfg: Annotated[Config, Depends(get_cfg)],
    file: str = Query(...),
    claims=Depends(require_license),
):
    """下载 skill 内单个文件 (asset 或 prompt)."""
    # 路径越权防护: 拒绝 ..
    if ".." in file or file.startswith("/"):
        raise HTTPException(400, "invalid file path")
    skills_root = Path(cfg.skills_root)
    skill_dir = skills_root / skill_id
    manifest = _find_latest_manifest(skill_dir)
    if manifest is None:
        raise HTTPException(404, f"skill {skill_id} not found")

    # 校验 file 在 manifest.assets 或 manifest.prompts 列表里 (防任意读)
    allowed = {a["path"] for a in manifest.get("assets", [])} | {p["file"] for p in manifest.get("prompts", [])}
    if file not in allowed:
        raise HTTPException(403, f"file {file} not in manifest")

    # blob 路径布局兼容:
    #   1) 扁平:   {skills_root}/{skill_id}/{file}
    #   2) 版本化: {skills_root}/{skill_id}/{version}/{file}  (publish 走 layout)
    # 优先扁平 (与 Rust 客户端写入路径一致), 否则回退到版本化.
    full = skill_dir / file
    if not full.is_file():
        # 找 _find_latest_manifest 选中的 version, 在 version 子目录里找
        manifest_path = _resolve_manifest_path(skill_dir)
        if manifest_path is not None:
            version_dir = manifest_path.parent
            full = version_dir / file
    if not full.is_file():
        raise HTTPException(404, f"file {file} not found on disk")
    return _FileResponse(str(full))


@router.post("/skills/install-authorize", response_model=InstallAuthorizeResponse)
def install_authorize(
    body: InstallAuthorizeRequest,
    request: Request,
    claims=Depends(require_license),
):
    """家长 PIN server-side 二次授权 (Day 4 接 ParentPinStore 完整实现).
    当前 stub: PIN 非空 + skill_id 已知 → authorized.
    """
    if not body.parent_pin_proof.strip():
        raise HTTPException(400, "parent_pin_proof 必填")
    skills_root = Path(getattr(request.app.state.cfg, "skills_root", "./skills"))
    if not (skills_root / body.skill_id).is_dir():
        raise HTTPException(404, f"unknown skill_id {body.skill_id}")
    return InstallAuthorizeResponse(
        skill_id=body.skill_id,
        authorized=True,
        receipt_id=f"auth-{int(time.time() * 1000)}",
    )


@router.post("/me/set-mode", response_model=SetModeResponse)
def set_user_mode(
    body: SetModeRequest,
    request: Request,
    claims=Depends(require_license),
):
    """Part C: 家长 PIN 解锁切到成人, 或成人切回儿童 (双向都要 PIN).
    server 端: 校验 PIN (Day 4 接 argon2), 写 devices.mode.
    """
    if not body.parent_pin_proof.strip():
        raise HTTPException(400, "parent_pin_proof 必填")
    conn = request.app.state.db
    _ensure_parent_pin_column(conn)
    # Day 4 完整: 读 devices.parent_pin_hash → argon2 verify (PIN:proof) → 改 mode
    # 当前 stub: 直接改 mode, 因为 dev 测试 PIN 校验还没接 ParentPinStore.
    _write_device_mode(conn, claims.device_id, body.mode)
    now_ms = int(time.time() * 1000)
    return SetModeResponse(
        device_id=claims.device_id,
        mode=body.mode,
        switched_at=now_ms,
    )


@router.get("/me/mode", response_model=SetModeResponse)
def get_user_mode(
    request: Request,
    claims=Depends(require_license),
):
    mode = _read_device_mode(request.app.state.db, claims.device_id)
    return SetModeResponse(
        device_id=claims.device_id,
        mode=mode,
        switched_at=0,
    )


# ---- helpers ----

def _find_latest_manifest(skill_dir: Path) -> dict | None:
    """在 skill_dir 下找最新 version 的 manifest.json. 约定: skill_dir/<version>/manifest.json."""
    path = _resolve_manifest_path(skill_dir)
    if path is None:
        return None
    try:
        return json.loads(path.read_text())
    except Exception:
        return None


def _resolve_manifest_path(skill_dir: Path) -> Path | None:
    """返回 manifest.json 的实际路径. 兼容扁平 / 版本化两种布局."""
    if not skill_dir.is_dir():
        return None
    flat = skill_dir / "manifest.json"
    if flat.is_file():
        return flat
    candidates = []
    for sub in sorted(skill_dir.iterdir()):
        if sub.is_dir() and (sub / "manifest.json").is_file():
            try:
                m = json.loads((sub / "manifest.json").read_text())
                candidates.append((m.get("version", sub.name), sub / "manifest.json"))
            except Exception:
                continue
    if not candidates:
        return None
    candidates.sort(key=lambda x: x[0])
    return candidates[-1][1]


def _audience_matches(audience: str, mode: str) -> bool:
    if audience == "both":
        return True
    if mode == "child":
        return audience == "child"
    if mode == "adult":
        return audience == "adult"
    return False


def _manifest_to_index_item(manifest: dict, skill_id: str) -> dict:
    """从完整 manifest 提取 list 端点需要的最小字段."""
    return {
        "id": manifest.get("id", skill_id),
        "name": manifest.get("name", skill_id),
        "version": manifest.get("version", "unknown"),
        "audience": manifest.get("audience", "child"),
        "age_tier": manifest.get("age_tier", []),
        "category": manifest.get("category", ""),
        "size_bytes": manifest.get("size_bytes", 0),
        "description": manifest.get("description"),
        "credits_per_use": manifest.get("credits_per_use", 0),
        "daily_quota": manifest.get("daily_quota", 0),
        "publisher": manifest.get("publisher", ""),
        "min_app_version": manifest.get("min_app_version", ""),
    }


# FileResponse 替代: 内联返 bytes, 避免引入额外 import
from fastapi.responses import FileResponse as _FileResponse  # noqa: E402