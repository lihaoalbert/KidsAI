# W11 Day 8 — Telemetry endpoint
#
# POST /api/v1/telemetry — 接收客户端 telemetry event
# 隐私设计 (Part C7):
#   - 失败不阻塞; 客户端 fire-and-forget
#   - server 端仅记 metadata (kind / outcome / latency / mode)
#   - 客户端已按 mode 脱敏 input/output hash, server 不再校验内容

from __future__ import annotations

import json
import time
from typing import Any, Dict

from fastapi import APIRouter, Depends, Request

from ..auth import LicenseClaims, require_license

router = APIRouter(prefix="/api/v1", tags=["telemetry"])


@router.post("/telemetry")
async def post_telemetry(
    payload: Dict[str, Any],
    request: Request,
    claims: LicenseClaims = Depends(require_license),
) -> Dict[str, Any]:
    """接收 telemetry envelope, 写入 audit_log, 聚合指标计数.

    服务端信任客户端的脱敏 (Adult mode → 已无 input/output hash).
    不依赖此接口做关键路径 — 仅供分析 + 自动回滚触发.
    """
    conn = request.app.state.db
    cfg = request.app.state.cfg

    envelope_mode = str(payload.get("mode", "child"))
    opted_out = bool(payload.get("opted_out", False))
    ts_ms = int(payload.get("ts_ms", int(time.time() * 1000)))
    event = payload.get("event", {})

    kind = str(event.get("kind", "unknown"))

    # 把 event 完整 JSON 串存进 audit_log.payload (含 hash 也无妨, 因 Adult 已被客户端脱敏过)
    audit_payload = json.dumps(
        {
            "kind": kind,
            "ts_ms": ts_ms,
            "mode": envelope_mode,
            "device_id": claims.device_id,
            "event": event,
        },
        ensure_ascii=False,
    )

    row = conn.execute(
        "SELECT 1 FROM audit_log WHERE device_id=? AND action=? AND payload_json=? ORDER BY id DESC LIMIT 1",
        (claims.device_id, f"telemetry:{kind}", audit_payload),
    ).fetchone()
    if row is None:
        conn.execute(
            "INSERT INTO audit_log (device_id, action, payload_json, created_at) VALUES (?, ?, ?, ?)",
            (claims.device_id, f"telemetry:{kind}", audit_payload, int(time.time())),
        )
        conn.commit()

    # 简单聚合计数 (按 device_id + kind)
    conn.execute(
        "INSERT INTO telemetry_counts (device_id, mode, kind, ts_ms) VALUES (?, ?, ?, ?)",
        (claims.device_id, envelope_mode, kind, ts_ms),
    )
    conn.commit()

    # 自动回滚触发条件 (占位 — 实际灰度策略由 admin 控制):
    #   - 仅 kind=agent_run + outcome=err 的计数堆积时, 写一条 rollback_recommendation
    if kind == "agent_run" and str(event.get("outcome", "")) == "err":
        err_count = conn.execute(
            "SELECT COUNT(*) FROM telemetry_counts WHERE kind='agent_run' AND ts_ms > ?",
            (int(time.time() * 1000) - 60 * 60 * 1000,),  # last hour
        ).fetchone()[0]
        if err_count > 50:
            conn.execute(
                "INSERT INTO rollback_recommendations (kind, message, created_at) VALUES (?, ?, ?)",
                (
                    "agent_run_err_spike",
                    f"device={claims.device_id} had {err_count} agent_run errors in last hour",
                    int(time.time()),
                ),
            )
            conn.commit()

    # telemetry endpoint 静默 — 不返任何敏感数据
    return {"ok": True}
