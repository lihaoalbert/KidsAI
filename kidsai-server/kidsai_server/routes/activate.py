"""POST /api/v1/devices/activate — 设备首次激活.

无鉴权 (用 fingerprint_hash 自报家门).
行为:
- 若 fingerprint 已激活 → 复用原 device_id, 不重新签发 (保留余额)
- 否则创建 devices + wallets, 签发新 license, 返回 api_keys
"""
from __future__ import annotations

import hashlib
import secrets

from fastapi import APIRouter, Depends

from ..auth import issue_license
from ..config import Config
from ..db import now_ms
from ..dependencies import get_cfg, get_conn
from ..models import ActivateRequest, ActivateResponse, ApiKeys
from ..wallet import create_wallet

router = APIRouter(prefix="/api/v1", tags=["activate"])


def _hash_fp(fp: str) -> str:
    return hashlib.sha256(fp.encode("utf-8")).hexdigest()[:32]


@router.post("/devices/activate", response_model=ActivateResponse)
def activate(
    body: ActivateRequest,
    conn=Depends(get_conn),
    cfg: Config = Depends(get_cfg),
) -> ActivateResponse:
    fp_hash = _hash_fp(body.fingerprint_hash)

    existing = conn.execute(
        "SELECT id FROM devices WHERE fingerprint_hash = ? AND revoked_at IS NULL",
        (fp_hash,),
    ).fetchone()
    if existing is not None:
        device_id = existing["id"]
    else:
        device_id = secrets.token_urlsafe(16)

    conn.execute(
        """
        INSERT INTO devices (id, fingerprint_hash, nickname, age_tier, activated_at)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET last_seen_at = excluded.activated_at
        """,
        (device_id, fp_hash, body.nickname, body.age_tier, now_ms()),
    )
    create_wallet(conn, device_id, cfg.starting_balance, cfg.daily_quota)

    bal_row = conn.execute(
        "SELECT balance FROM wallets WHERE device_id = ?", (device_id,)
    ).fetchone()
    balance = bal_row["balance"] if bal_row else 0

    token, _claims = issue_license(cfg.jwt_secret, device_id, cfg.jwt_ttl_seconds)

    return ActivateResponse(
        device_id=device_id,
        license_token=token,
        api_keys=ApiKeys(llm=cfg.llm_api_key, video=cfg.seedance_api_key),
        balance=balance,
        daily_quota=cfg.daily_quota,
    )