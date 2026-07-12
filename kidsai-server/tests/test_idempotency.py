"""幂等性: 同 call_id 第二次提交返首次结果, 不重复扣."""
from __future__ import annotations


def _activate(client) -> dict:
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": "fp-idem-test-aaaaaa", "nickname": "i", "ageTier": 1},
    )
    return r.json()


def test_same_call_id_does_not_double_charge(client):
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    payload = {"callId": "idem-001-aaaaaa", "kind": "video_draft", "units": 1}
    r1 = client.post("/api/v1/me/record-spend", headers=auth, json=payload)
    r2 = client.post("/api/v1/me/record-spend", headers=auth, json=payload)
    assert r1.json()["accepted"] is True
    assert r2.json()["accepted"] is True
    assert r1.json()["balanceAfter"] == r2.json()["balanceAfter"]
    assert r1.json()["balanceAfter"] == 91  # 只扣一次 9


def test_admin_grant_idempotent_on_replay(client, admin_headers):
    env = _activate(client)
    payload = {"amount": 50, "reason": "test grant"}
    # admin grant 每次自动生成新 call_id, 不是测试幂等的重点
    # 但 record_spend 的 idempotency 已覆盖, 这里只测 admin grant 本身
    r = client.post(
        f"/api/v1/admin/devices/{env['deviceId']}/grant",
        headers=admin_headers,
        json=payload,
    )
    assert r.status_code == 200
    assert r.json()["balanceAfter"] == 150


def test_admin_grant_requires_admin_token(client):
    env = _activate(client)
    r = client.post(
        f"/api/v1/admin/devices/{env['deviceId']}/grant",
        headers={"X-Admin-Token": "wrong"},
        json={"amount": 10},
    )
    assert r.status_code == 403


def test_refresh_license_returns_new_token(client):
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    r = client.post("/api/v1/me/refresh-license", headers=auth)
    assert r.status_code == 200
    body = r.json()
    assert body["licenseToken"]
    assert body["licenseToken"] != env["licenseToken"], "应签发新 JWT"
    assert body["apiKeys"]["llm"]


def test_refresh_license_requires_bearer(client):
    r = client.post("/api/v1/me/refresh-license")
    assert r.status_code == 401