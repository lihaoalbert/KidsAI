"""W6 E4: spend-summary endpoint 测试.

覆盖:
- 空 transactions → today_total=0, by_kind={}
- 多 kind 消耗 → 按 kind 分组, sum 正确
- 'grant' 类不计入消耗
- 不同日期不混入今日
"""
from __future__ import annotations

import sqlite3
import time
from pathlib import Path

import pytest
from fastapi.testclient import TestClient


def _activate(client: TestClient) -> dict:
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": "fp-spend-summary-aaa", "nickname": "t", "ageTier": 1},
    )
    return r.json()


def _add_tx(
    conn: sqlite3.Connection,
    *,
    call_id: str,
    kind: str,
    amount: int,
    created_at: int,
    device_id: str,
) -> None:
    conn.execute(
        "INSERT INTO transactions (call_id, device_id, kind, amount, reason, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        (call_id, device_id, kind, amount, None, created_at),
    )
    conn.commit()


def _db_path(client: TestClient) -> str:
    """从 app.state 取 db path, 用它直接连一个新的 conn 加 tx."""
    # app.state.db is the open conn; we need path. Use the cfg's db path.
    # Workaround: TestClient.app has db in state, but no path. We need to
    # access the path via Config. Simpler: write transactions via the API
    # endpoints (record-spend). But that needs admin flow.
    # Easier path: use the same conn the app uses (via state) and INSERT
    # directly.
    db_conn = client.app.state.db  # type: ignore[attr-defined]
    return db_conn


def test_empty_summary_returns_zeros(client: TestClient) -> None:
    env = _activate(client)
    r = client.get(
        "/api/v1/me/spend-summary",
        headers={"Authorization": f"Bearer {env['licenseToken']}"},
    )
    assert r.status_code == 200
    body = r.json()
    assert body["todayTotal"] == 0
    assert body["byKind"] == {}
    assert body["dailyQuota"] == 30


def test_multiple_kinds_grouped(client: TestClient) -> None:
    env = _activate(client)
    device_id = env["deviceId"]
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}

    today_ms = int(time.time() // 86400 * 86400) * 1000
    db = _db_path(client)
    _add_tx(db, call_id="c1", kind="image_gen", amount=-5, created_at=today_ms + 1000, device_id=device_id)
    _add_tx(db, call_id="c2", kind="image_gen", amount=-5, created_at=today_ms + 2000, device_id=device_id)
    _add_tx(db, call_id="c3", kind="voice_clone", amount=-10, created_at=today_ms + 3000, device_id=device_id)
    _add_tx(db, call_id="c4", kind="music_gen", amount=-8, created_at=today_ms + 4000, device_id=device_id)

    r = client.get("/api/v1/me/spend-summary", headers=auth)
    body = r.json()
    assert body["byKind"]["image_gen"] == 10
    assert body["byKind"]["voice_clone"] == 10
    assert body["byKind"]["music_gen"] == 8
    assert body["todayTotal"] == 28


def test_grant_excluded_from_total(client: TestClient) -> None:
    env = _activate(client)
    device_id = env["deviceId"]
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}

    today_ms = int(time.time() // 86400 * 86400) * 1000
    db = _db_path(client)
    _add_tx(db, call_id="c-grant", kind="grant", amount=100, created_at=today_ms, device_id=device_id)
    _add_tx(db, call_id="c1", kind="image_gen", amount=-5, created_at=today_ms + 1000, device_id=device_id)

    r = client.get("/api/v1/me/spend-summary", headers=auth)
    body = r.json()
    assert body["todayTotal"] == 5
    assert "grant" not in body["byKind"]


def test_yesterday_not_counted(client: TestClient) -> None:
    env = _activate(client)
    device_id = env["deviceId"]
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}

    today_ms = int(time.time() // 86400 * 86400) * 1000
    yesterday_ms = today_ms - 86400 * 1000
    db = _db_path(client)
    _add_tx(db, call_id="c-yesterday", kind="image_gen", amount=-5, created_at=yesterday_ms, device_id=device_id)
    _add_tx(db, call_id="c-today", kind="image_gen", amount=-5, created_at=today_ms + 1000, device_id=device_id)

    r = client.get("/api/v1/me/spend-summary", headers=auth)
    body = r.json()
    assert body["todayTotal"] == 5


def test_unauthenticated_returns_401(client: TestClient) -> None:
    r = client.get("/api/v1/me/spend-summary")
    assert r.status_code == 401