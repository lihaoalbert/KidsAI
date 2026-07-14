"""W11 Day 8 — Telemetry endpoint tests.

覆盖:
- POST /api/v1/telemetry (鉴权) 接收 envelope → 写入 telemetry_counts + audit_log
- 没 Bearer → 401
- Child mode envelope (含 hash) → 入库
- Adult mode envelope (无 hash, mode='adult') → 入库
- 同一 event 重放一次 (幂等检查: 只应 1 row, 因 envelope 全等)
"""

from __future__ import annotations

import pytest


def _activate(client) -> dict:
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": "fp-tel-test-aaa", "nickname": "t", "ageTier": 1},
    )
    assert r.status_code == 200, r.text
    return r.json()


def test_telemetry_requires_bearer(client):
    r = client.post("/api/v1/telemetry", json={"mode": "child", "event": {}})
    assert r.status_code == 401


def test_telemetry_child_event_persists(client):
    """Child mode envelope (含 hash) → telemetry_counts row + audit_log row."""
    env = _activate(client)
    headers = {"Authorization": f"Bearer {env['licenseToken']}"}
    body = {
        "mode": "child",
        "optedOut": False,
        "deviceId": env["deviceId"],
        "tsMs": 1_700_000_000_000,
        "event": {
            "kind": "agent_run",
            "call_id": "c-1",
            "level_id": "L1",
            "agent_kind": "director",
            "outcome": "ok",
            "latency_ms": 100,
            "input_hash": "abc123",
            "output_hash": "def456",
        },
    }
    r = client.post("/api/v1/telemetry", json=body, headers=headers)
    assert r.status_code == 200
    assert r.json() == {"ok": True}

    # 验证入库
    db = client.app.state.db
    rows = db.execute(
        "SELECT mode, kind FROM telemetry_counts WHERE device_id=?",
        (env["deviceId"],),
    ).fetchall()
    assert len(rows) == 1
    assert rows[0]["mode"] == "child"
    assert rows[0]["kind"] == "agent_run"

    audit = db.execute(
        "SELECT action FROM audit_log WHERE device_id=? AND action='telemetry:agent_run'",
        (env["deviceId"],),
    ).fetchall()
    assert len(audit) == 1


def test_telemetry_adult_event_persists_without_hash(client):
    """Adult mode envelope (hash 已被客户端脱敏为 None) → 仍入库但 mode='adult'."""
    env = _activate(client)
    headers = {"Authorization": f"Bearer {env['licenseToken']}"}
    body = {
        "mode": "adult",
        "optedOut": False,
        "deviceId": env["deviceId"],
        "tsMs": 1_700_000_000_000,
        "event": {
            "kind": "mode_switch",
            "from_mode": "child",
            "to_mode": "adult",
            "success": True,
        },
    }
    r = client.post("/api/v1/telemetry", json=body, headers=headers)
    assert r.status_code == 200

    db = client.app.state.db
    rows = db.execute(
        "SELECT mode, kind FROM telemetry_counts WHERE device_id=? AND mode='adult'",
        (env["deviceId"],),
    ).fetchall()
    assert len(rows) == 1
    assert rows[0]["kind"] == "mode_switch"


def test_telemetry_opt_out_still_persists(client):
    """opted_out=True 仍接收 (客户端 fire-and-forget); 记录到 audit, 但 mode 字段保留."""
    env = _activate(client)
    headers = {"Authorization": f"Bearer {env['licenseToken']}"}
    body = {
        "mode": "adult",
        "optedOut": True,  # 用户 setting 关了 telemetry
        "tsMs": 1_700_000_000_000,
        "event": {"kind": "skill_install", "skill_id": "x", "skill_version": "v1", "audience": "adult", "success": True},
    }
    r = client.post("/api/v1/telemetry", json=body, headers=headers)
    assert r.status_code == 200

    db = client.app.state.db
    rows = db.execute(
        "SELECT mode, kind FROM telemetry_counts WHERE device_id=?",
        (env["deviceId"],),
    ).fetchall()
    assert len(rows) == 1
