"""GET /api/v1/me/balance — 鉴权 + 余额 + daily quota 视图."""
from __future__ import annotations

import pytest


def _activate(client) -> dict:
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": "fp-balance-test-aaa", "nickname": "b", "ageTier": 1},
    )
    return r.json()


def test_balance_requires_bearer(client):
    r = client.get("/api/v1/me/balance")
    assert r.status_code == 401


def test_balance_rejects_garbage_bearer(client):
    r = client.get("/api/v1/me/balance", headers={"Authorization": "Bearer not-a-jwt"})
    assert r.status_code == 401


def test_balance_returns_starting_state(client):
    env = _activate(client)
    r = client.get(
        "/api/v1/me/balance",
        headers={"Authorization": f"Bearer {env['licenseToken']}"},
    )
    assert r.status_code == 200
    body = r.json()
    assert body["deviceId"] == env["deviceId"]
    assert body["balance"] == 100
    assert body["dailyConsumed"] == 0
    assert body["dailyQuota"] == 30
    assert body["dailyRemaining"] == 30


def test_balance_reflects_consume(client, cfg):
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}

    # 扣 9 (video_draft)
    client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": "call-balance-001", "kind": "video_draft", "units": 1},
    )
    r = client.get("/api/v1/me/balance", headers=auth)
    body = r.json()
    assert body["balance"] == 91
    assert body["dailyConsumed"] == 9
    assert body["dailyRemaining"] == 21