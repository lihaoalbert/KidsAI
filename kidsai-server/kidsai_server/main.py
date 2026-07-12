"""FastAPI app entrypoint (W4.5 B1).

启动: uvicorn kidsai_server.main:app --host 0.0.0.0 --port 8080
或:    python -m kidsai_server.main

路由 (7 endpoints + healthz):
- GET  /healthz
- POST /api/v1/devices/activate
- GET  /api/v1/me/balance
- POST /api/v1/me/record-spend
- POST /api/v1/me/refresh-license
- POST /api/v1/admin/devices/{id}/grant
- POST /api/v1/admin/devices/{id}/revoke
"""
from __future__ import annotations

import os
import secrets as _secrets
import sqlite3
from contextlib import asynccontextmanager

from fastapi import Depends, FastAPI, Header, HTTPException, Request, status

from . import __version__
from .auth import assert_device_active, require_admin, require_license, verify_license
from .config import Config, load_config
from .db import open_db
from .dependencies import get_cfg, get_conn
from .models import HealthResponse
from .routes import activate, admin, me


def _make_license_dep(secret: str):
    """require_license 替代: secret 在 closure 里, 不再靠全局.

    每个 /me/* request 都查一次 devices.revoked_at (PK 索引, ~µs)
    — revoke 立即生效, 不需要等 JWT exp.
    """

    def _dep(
        authorization: str | None = Header(default=None),
        conn=Depends(get_conn),
    ):
        if not authorization or not authorization.lower().startswith("bearer "):
            raise HTTPException(
                status.HTTP_401_UNAUTHORIZED, "missing Authorization: Bearer <license_token>"
            )
        claims = verify_license(secret, authorization.split(" ", 1)[1].strip())
        assert_device_active(conn, claims.device_id)
        return claims

    return _dep


def _make_admin_dep(expected: str):
    def _dep(x_admin_token: str | None = Header(default=None)):
        if not x_admin_token or not _secrets.compare_digest(x_admin_token, expected):
            raise HTTPException(status.HTTP_403_FORBIDDEN, "invalid admin token")

    return _dep


def create_app(cfg: Config | None = None, db_path: str | None = None) -> FastAPI:
    cfg = cfg or load_config()
    db_path = db_path or cfg.database_path

    @asynccontextmanager
    async def lifespan(app: FastAPI):
        app.state.db = open_db(db_path)
        app.state.cfg = cfg
        try:
            yield
        finally:
            app.state.db.close()

    app = FastAPI(
        title="KidsAI Server",
        version=__version__,
        description="License + Quota 控制平面 (不代理 LLM/Seedance)",
        lifespan=lifespan,
    )

    app.dependency_overrides[require_license] = _make_license_dep(cfg.jwt_secret)
    app.dependency_overrides[require_admin] = _make_admin_dep(cfg.admin_token)

    @app.get("/healthz", response_model=HealthResponse)
    def healthz() -> HealthResponse:
        return HealthResponse(status="ok", version=__version__)

    app.include_router(activate.router)
    app.include_router(me.router)
    app.include_router(admin.router)

    return app


# 模块顶层 app (uvicorn 入口)
# 部署时由 systemd EnvironmentFile 注入 JWT_SECRET / ADMIN_TOKEN 等
try:
    _cfg = load_config()
    app = create_app(cfg=_cfg)
except RuntimeError as _e:
    # 让 uvicorn 仍然能启动, healthz 会返回 ok 但其他路由会因 secret 空而 401
    app = FastAPI(title="KidsAI Server (unconfigured)", version=__version__)

    @app.get("/healthz", response_model=HealthResponse)
    def _unconfigured_healthz() -> HealthResponse:
        return HealthResponse(status="degraded", version=__version__)


if __name__ == "__main__":
    import uvicorn

    port = int(os.getenv("PORT", "8080"))
    uvicorn.run("kidsai_server.main:app", host="0.0.0.0", port=port, reload=False)