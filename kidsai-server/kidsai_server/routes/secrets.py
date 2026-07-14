"""W11 server-side — Secrets API (Day 6).

Endpoints:
- GET  /api/v1/secrets/manifest?profile=child|adult   — signed manifest (公开可缓存, CDN 友好)
- GET  /api/v1/secrets/bundle?profile=...&version=... — bundle.bin (公开可缓存, master_key 加密)
- POST /api/v1/secrets/wrap?profile=...&version=...   — per-device wrap of master_key
  (需鉴权; server 用 license_token → KEK → 包裹 master_key; 客户端用同一 KEK 解开)

设计:
- manifest + bundle 走 CDN/edge 缓存, 不鉴权 (per-device wrap 才鉴权)
- wrap 是 per-device 的 ~60 bytes, 每台设备各拿一次; 拿到后本地 cache 到
  app_data_dir/secrets/wrapped_master_{profile}.json
- 版本链: server 维护 secrets_root/{profile}/{version}/; client 启动 GET manifest
  比对 version 决定是否 fetch
"""
from __future__ import annotations

import base64
import json
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

from fastapi import APIRouter, Depends, Header, HTTPException, Query, Response
from pydantic import BaseModel, ConfigDict, Field
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
from cryptography.hazmat.primitives.ciphers.aead import AESGCM
from cryptography.hazmat.primitives.kdf.hkdf import HKDF

from ..auth import require_license_token
from ..config import Config
from ..dependencies import get_cfg

router = APIRouter(prefix="/api/v1/secrets", tags=["secrets"])

_CAMEL = ConfigDict(
    alias_generator=lambda s: s if "_" not in s else s.split("_")[0] + "".join(p.title() for p in s.split("_")[1:]),
    populate_by_name=True,
    extra="forbid",
)

KDF_SALT = b"kidsai-secrets-v1"
KDF_INFO_WRAP = b"kidsai-secrets/wrap-master"
SUPPORTED_PROFILES = ("child", "adult")


# ---- helpers ----

@dataclass
class LatestVersion:
    profile: str
    version: str
    path: Path  # {secrets_root}/{profile}/{version}/


def _find_latest_version(secrets_root: Path, profile: str) -> Optional[LatestVersion]:
    """返回 {secrets_root}/{profile}/ 下字典序最大的 version 目录."""
    profile_dir = secrets_root / profile
    if not profile_dir.is_dir():
        return None
    versions = sorted(
        [p for p in profile_dir.iterdir() if p.is_dir()],
        key=lambda p: p.name,
    )
    if not versions:
        return None
    return LatestVersion(profile=profile, version=versions[-1].name, path=versions[-1])


def _load_master_key() -> bytes:
    """从 KIDSAI_SECRETS_MASTER_KEY env 读 32-byte AES key (hex 64 chars).

    生产环境必须设置; 启动期不设置 → 启动失败 (config 时已 check).
    测试场景允许没设置, 走 dev fallback (随机 32 bytes — 仅 dev 跑通, 重新 publish 之后才能解).
    """
    raw = os.environ.get("KIDSAI_SECRETS_MASTER_KEY", "").strip()
    if raw:
        try:
            key = bytes.fromhex(raw)
        except ValueError as e:
            raise HTTPException(status_code=500, detail=f"master_key 不是合法 hex: {e}")
        if len(key) != 32:
            raise HTTPException(status_code=500, detail="master_key 长度必须 32 字节")
        return key
    # dev fallback: 直接 raise — 没设置就别让 wrap endpoint 返错数据
    raise HTTPException(
        status_code=503,
        detail="KIDSAI_SECRETS_MASTER_KEY 未设置; server 不能包裹 master_key. 设置 env 后重启.",
    )


def _derive_kek(license_token: str) -> bytes:
    hk = HKDF(algorithm=hashes.SHA256(), length=32, salt=KDF_SALT, info=KDF_INFO_WRAP)
    return hk.derive(license_token.encode())


# ---- response schemas ----

class WrapResponse(BaseModel):
    """Wrap 响应: 字段名严格 snake_case (匹配 Rust WrappedMaster struct).

    这里故意不用 _CAMEL — Rust 客户端 deserialize 时用 snake_case 字段名.
    """
    profile: str
    version: str
    ciphertext_b64: str
    iv: str
    algo: str = "AES-256-GCM"
    kdf: str = "HKDF-SHA256"
    kdf_salt: str = "kidsai-secrets-v1"
    kdf_info: str = "kidsai-secrets/wrap-master"


class ManifestResponse(BaseModel):
    """不重定义 manifest 字段 — 直接把磁盘上的 manifest.json 透传 (已经是 JSON).

    客户端会 deserialize 成自己的 SecretsManifest struct.
    """
    model_config = _CAMEL
    profile: str
    version: str
    manifest: dict


# ---- endpoints ----

@router.get("/manifest")
def get_manifest(
    profile: str = Query(..., pattern="^(child|adult)$"),
    cfg: Config = Depends(get_cfg),
) -> dict:
    """返回 {profile} 最新版本的 manifest.json (公开, 不鉴权 — 客户端验签防篡改)."""
    if profile not in SUPPORTED_PROFILES:
        raise HTTPException(status_code=400, detail=f"profile 必须是 {SUPPORTED_PROFILES}")
    root = Path(cfg.secrets_root)
    latest = _find_latest_version(root, profile)
    if latest is None:
        raise HTTPException(status_code=404, detail=f"profile={profile} 没有任何版本")
    manifest_path = latest.path / "manifest.json"
    if not manifest_path.is_file():
        raise HTTPException(status_code=500, detail=f"manifest 文件缺失: {manifest_path}")
    try:
        return json.loads(manifest_path.read_text())
    except json.JSONDecodeError as e:
        raise HTTPException(status_code=500, detail=f"manifest 解析失败: {e}")


@router.get("/bundle")
def get_bundle(
    profile: str = Query(..., pattern="^(child|adult)$"),
    version: str = Query(..., min_length=3),
    cfg: Config = Depends(get_cfg),
) -> Response:
    """返回 {profile}/{version}/bundle.bin (公开, 不鉴权 — master_key 加密防偷看)."""
    if profile not in SUPPORTED_PROFILES:
        raise HTTPException(status_code=400, detail=f"profile 必须是 {SUPPORTED_PROFILES}")
    root = Path(cfg.secrets_root)
    bundle_path = root / profile / version / "bundle.bin"
    if not bundle_path.is_file():
        raise HTTPException(status_code=404, detail=f"bundle 不存在: {profile}/{version}")
    return Response(
        content=bundle_path.read_bytes(),
        media_type="application/octet-stream",
    )


@router.post("/wrap", response_model=WrapResponse)
def wrap_master(
    profile: str = Query(..., pattern="^(child|adult)$"),
    version: Optional[str] = Query(None, min_length=3),
    cfg: Config = Depends(get_cfg),
    license_token: str = Depends(require_license_token),
) -> WrapResponse:
    """per-device wrap: server 用 license_token → KEK → AES-GCM(master_key) → 返 ciphertext.

    客户端拿 wrapped 之后用同一 license_token 派生 KEK 解开, 拿到 master_key, 再解 bundle.
    必须鉴权 (require_license_token), 防止一个设备的 wrap 被另一个设备重放.

    AAD: license_token 原文作 AAD, 防止重放到不同设备 (因为 client 派生 KEK 时也用相同 AAD).
    """
    if profile not in SUPPORTED_PROFILES:
        raise HTTPException(status_code=400, detail=f"profile 必须是 {SUPPORTED_PROFILES}")
    root = Path(cfg.secrets_root)
    if version:
        target = root / profile / version
        if not (target / "manifest.json").is_file():
            raise HTTPException(status_code=404, detail=f"version 不存在: {profile}/{version}")
        resolved_version = version
    else:
        latest = _find_latest_version(root, profile)
        if latest is None:
            raise HTTPException(status_code=404, detail=f"profile={profile} 没有任何版本")
        resolved_version = latest.version

    master_key = _load_master_key()
    kek = _derive_kek(license_token)
    iv = os.urandom(12)
    aes = AESGCM(kek)
    # AAD = license_token 原文 (client 派生 KEK 时也用同样 AAD, 防重放)
    ct = aes.encrypt(iv, master_key, associated_data=license_token.encode())

    return WrapResponse(
        profile=profile,
        version=resolved_version,
        ciphertext_b64=base64.b64encode(ct).decode(),
        iv=base64.b64encode(iv).decode(),
    )
