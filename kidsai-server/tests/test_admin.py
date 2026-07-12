"""Admin endpoints + revoke 立刻失效 license (W4.5 C 修)."""
from __future__ import annotations


ADMIN_TOKEN = "test-admin-token-12345678"


def _activate(client) -> dict:
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": "fp-admin-test-aaaaa", "nickname": "a", "ageTier": 1},
    )
    return r.json()


def _admin_headers() -> dict:
    return {"X-Admin-Token": ADMIN_TOKEN}


def test_admin_grant_increments_balance(client):
    env = _activate(client)
    device_id = env["deviceId"]
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}

    # 起 balance=100, grant 50 → 150
    r = client.post(
        f"/api/v1/admin/devices/{device_id}/grant",
        headers=_admin_headers(),
        json={"amount": 50, "reason": "test grant"},
    )
    assert r.status_code == 200, r.text
    body = r.json()
    assert body["accepted"] is True
    assert body["balanceAfter"] == 150
    assert body["cost"] == 50

    # 余额实查
    r = client.get("/api/v1/me/balance", headers=auth)
    assert r.json()["balance"] == 150


def test_revoke_invalidates_license_immediately(client):
    """revoke 后已签的 license_token 在 exp 之前也应 401."""
    env = _activate(client)
    device_id = env["deviceId"]
    license = env["licenseToken"]
    auth = {"Authorization": f"Bearer {license}"}

    # 1. revoke 前 — balance 正常
    r = client.get("/api/v1/me/balance", headers=auth)
    assert r.status_code == 200, r.text
    assert r.json()["balance"] == 100

    # 2. admin revoke
    r = client.post(
        f"/api/v1/admin/devices/{device_id}/revoke",
        headers=_admin_headers(),
        json={"reason": "test revoke"},
    )
    # 204 No Content
    assert r.status_code == 204

    # 3. revoked device 在 license exp 前应 401
    r = client.get("/api/v1/me/balance", headers=auth)
    assert r.status_code == 401
    assert "revoked" in r.json()["detail"].lower()

    # 4. record-spend 也应 401
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": "post-revoke-aaa", "kind": "llm", "units": 1000},
    )
    assert r.status_code == 401
    assert "revoked" in r.json()["detail"].lower()


def test_admin_revoke_unknown_device_returns_404(client):
    r = client.post(
        "/api/v1/admin/devices/nonexistent-device-id/revoke",
        headers=_admin_headers(),
        json={"reason": "test unknown"},
    )
    assert r.status_code == 404


def test_admin_grant_requires_admin_token(client):
    """无 X-Admin-Token 或错误 token → 403."""
    env = _activate(client)
    r = client.post(
        f"/api/v1/admin/devices/{env['deviceId']}/grant",
        json={"amount": 10, "reason": "no auth"},
    )
    assert r.status_code == 403

    r = client.post(
        f"/api/v1/admin/devices/{env['deviceId']}/grant",
        headers={"X-Admin-Token": "wrong-token"},
        json={"amount": 10, "reason": "wrong auth"},
    )
    assert r.status_code == 403
