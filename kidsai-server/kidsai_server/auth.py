"""JWT 签发/验证 + Admin token 校验 (W4.5 B1).

设计:
- JWT HS256, payload 含 sub (device_id), exp, iat, jti
- admin_token 不进 JWT, 单独 header X-Admin-Token (admin 端不走 license 流)
- jwt_ttl_seconds 来自 config (默认 24h)
"""
from __future__ import annotations

import secrets
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from typing import Annotated

from fastapi import Depends, Header, HTTPException, status
from jose import JWTError, jwt


@dataclass(frozen=True)
class LicenseClaims:
    device_id: str
    issued_at: int
    expires_at: int
    jti: str


def issue_license(secret: str, device_id: str, ttl_seconds: int) -> tuple[str, LicenseClaims]:
    now = datetime.now(timezone.utc)
    exp = now + timedelta(seconds=ttl_seconds)
    jti = secrets.token_urlsafe(16)
    payload = {
        "sub": device_id,
        "iat": int(now.timestamp()),
        "exp": int(exp.timestamp()),
        "jti": jti,
    }
    token = jwt.encode(payload, secret, algorithm="HS256")
    return token, LicenseClaims(
        device_id=device_id,
        issued_at=int(now.timestamp()),
        expires_at=int(exp.timestamp()),
        jti=jti,
    )


def verify_license(secret: str, token: str) -> LicenseClaims:
    try:
        payload = jwt.decode(token, secret, algorithms=["HS256"])
    except JWTError as e:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail=f"invalid license: {e}",
        ) from e
    return LicenseClaims(
        device_id=payload["sub"],
        issued_at=payload["iat"],
        expires_at=payload["exp"],
        jti=payload["jti"],
    )


def require_license(
    authorization: Annotated[str | None, Header()] = None,
    secret: str = "",  # 由 main.py 通过 Depends 注入
) -> LicenseClaims:
    """FastAPI dependency: 校验 Authorization: Bearer <jwt>."""
    if not authorization or not authorization.lower().startswith("bearer "):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="missing Authorization: Bearer <license_token>",
        )
    token = authorization.split(" ", 1)[1].strip()
    return verify_license(secret, token)


def require_admin(
    x_admin_token: Annotated[str | None, Header()] = None,
    expected: str = "",  # 由 main.py 通过 Depends 注入
) -> None:
    if not x_admin_token or not secrets.compare_digest(x_admin_token, expected):
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="invalid admin token",
        )