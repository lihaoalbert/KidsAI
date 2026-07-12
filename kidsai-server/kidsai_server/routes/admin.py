"""/api/v1/admin/* — 管理员操作 (X-Admin-Token header)."""
from __future__ import annotations

import secrets as _secrets

from fastapi import APIRouter, Depends, HTTPException, status

from ..auth import require_admin
from ..db import now_ms
from ..dependencies import get_conn
from ..models import AdminGrantRequest, AdminRevokeRequest, RecordSpendResponse
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