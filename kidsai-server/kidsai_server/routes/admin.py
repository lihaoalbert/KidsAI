"""/api/v1/admin/* — 管理员操作 (X-Admin-Token header)."""
from __future__ import annotations

import secrets as _secrets

from fastapi import APIRouter, Depends, HTTPException, status

from ..auth import require_admin
from ..config import Config
from ..db import now_ms
from ..dependencies import get_cfg, get_conn
from ..keypool import KeypoolError, rotate_key_for_device
from ..models import (
    AdminGrantRequest,
    AdminRevokeRequest,
    AdminRotateKeyResponse,
    RecordSpendResponse,
)
from ..wallet import grant

router = APIRouter(prefix="/api/v1/admin", tags=["admin"])


@router.post(
    "/devices/{device_id}/grant",
    response_model=RecordSpendResponse,
)
def admin_grant(
    device_id: str,
    body: AdminGrantRequest,
    conn=Depends(get_conn),
    _admin: None = Depends(require_admin),
) -> RecordSpendResponse:
    if conn.execute("SELECT 1 FROM devices WHERE id = ?", (device_id,)).fetchone() is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "device not found")
    call_id = f"admin-grant-{_secrets.token_urlsafe(8)}"
    outcome = grant(
        conn,
        device_id,
        body.amount,
        body.reason or "admin grant",
        call_id,
    )
    conn.execute(
        "INSERT INTO audit_log (device_id, action, payload_json, created_at) VALUES (?, 'grant', ?, ?)",
        (device_id, f'{{"amount": {body.amount}, "reason": "{body.reason}"}}', now_ms()),
    )
    return RecordSpendResponse(
        call_id=outcome.call_id,
        balance_after=outcome.balance_after,
        cost=outcome.cost,
        accepted=outcome.accepted,
    )


@router.post("/devices/{device_id}/revoke", status_code=204)
def admin_revoke(
    device_id: str,
    body: AdminRevokeRequest,
    conn=Depends(get_conn),
    _admin: None = Depends(require_admin),
) -> None:
    if conn.execute("SELECT 1 FROM devices WHERE id = ?", (device_id,)).fetchone() is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "device not found")
    conn.execute(
        "UPDATE devices SET revoked_at = ? WHERE id = ?",
        (now_ms(), device_id),
    )
    conn.execute(
        "INSERT INTO audit_log (device_id, action, payload_json, created_at) VALUES (?, 'revoke', ?, ?)",
        (device_id, f'{{"reason": "{body.reason}"}}', now_ms()),
    )


# W6 A4: 强制轮换某设备的 MiniMax key — lihao 应急用 (key 泄漏 / 单 key 限流).
@router.post(
    "/devices/{device_id}/rotate-key",
    response_model=AdminRotateKeyResponse,
)
def admin_rotate_key(
    device_id: str,
    conn=Depends(get_conn),
    cfg: Config = Depends(get_cfg),
    _admin: None = Depends(require_admin),
) -> AdminRotateKeyResponse:
    if conn.execute("SELECT 1 FROM devices WHERE id = ?", (device_id,)).fetchone() is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "device not found")
    if not cfg.minimax_api_keys:
        raise HTTPException(status.HTTP_503_SERVICE_UNAVAILABLE, "minimax key pool empty")
    try:
        new_key = rotate_key_for_device(conn, device_id, cfg.minimax_api_keys)
    except KeypoolError as e:
        raise HTTPException(status.HTTP_500_INTERNAL_SERVER_ERROR, str(e)) from e
    # 查新分配的 key_id (rotate 后 re-read)
    row = conn.execute(
        "SELECT key_id, assigned_at FROM device_key_assignment WHERE device_id = ?",
        (device_id,),
    ).fetchone()
    if row is None:
        # rotate 后 pick 不可能返 None — 这里只是防御
        raise HTTPException(status.HTTP_500_INTERNAL_SERVER_ERROR, "key_id not found after rotate")
    rotated_at = row["assigned_at"]
    conn.execute(
        "INSERT INTO audit_log (device_id, action, payload_json, created_at) VALUES (?, 'rotate_key', ?, ?)",
        (device_id, f'{{"new_key_id": {row["key_id"]}}}', now_ms()),
    )
    # 显式抛一下让 lihao 知道 key 真换了 (调试)
    _ = new_key  # 已写入 DB, 但不暴露明文 (避免 audit_log 漏)
    return AdminRotateKeyResponse(
        device_id=device_id,
        key_id=row["key_id"],
        rotated_at=rotated_at,
    )