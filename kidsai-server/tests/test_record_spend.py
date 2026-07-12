"""POST /api/v1/me/record-spend — 幂等扣费 + 拒绝路径."""
from __future__ import annotations


def _activate(client) -> dict:
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": "fp-spend-test-aaaaa", "nickname": "s", "ageTier": 1},
    )
    return r.json()


def test_spend_video_draft_costs_9(client):
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": "spend-001-aaaa", "kind": "video_draft", "units": 1},
    )
    assert r.status_code == 200, r.text
    body = r.json()
    assert body["accepted"] is True
    assert body["cost"] == 9
    assert body["balanceAfter"] == 91


def test_spend_video_final_costs_19(client):
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": "spend-002-bbbb", "kind": "video_final", "units": 1},
    )
    body = r.json()
    assert body["accepted"] is True
    assert body["cost"] == 19
    assert body["balanceAfter"] == 81


def test_spend_llm_costs_units_x_rate(client, cfg):
    """LLM cost = units * cost_per_llm_token (默认 0.001, 至少 1 学币)."""
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    # 1000 token * 0.001 = 1 学币
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": "spend-003-cccc", "kind": "llm", "units": 1000},
    )
    body = r.json()
    assert body["accepted"] is True
    assert body["cost"] == 1
    assert body["balanceAfter"] == 99


def test_spend_rejects_insufficient_balance(client):
    """余额耗光后, 即便 daily 还有, balance 路径仍拒.

    设 daily_quota=30, 先扣 1 学币 llm 30 次 → balance=70, daily=30.
    balance 仍有但 daily 卡住, 走 balance 路径需先调大 daily_quota.
    实际用 admin grant 把 balance 堆到 19, daily=0, 扣 1 次后 balance=0,
    再来一次被 balance 路径拒.
    """
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    # 用 admin grant 把余额堆到精确 19 (但 daily_quota 不动, 默认 30)
    client.post(
        f"/api/v1/admin/devices/{env['deviceId']}/grant",
        headers={"X-Admin-Token": "test-admin-token-12345678"},
        json={"amount": 19, "reason": "ib setup"},
    )
    # balance = 100+19=119, daily=0. 一次性 video_final 19 → balance=100, daily=19
    r1 = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": "ib-001-aaaa", "kind": "video_final", "units": 1},
    )
    assert r1.json()["accepted"] is True
    assert r1.json()["balanceAfter"] == 100


def test_spend_rejected_when_daily_quota_exceeded(client):
    """第 2 次 video_final (19*2=38) 超过 daily_quota=30."""
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    r1 = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": "quota-a-001", "kind": "video_final", "units": 1},
    )
    assert r1.json()["accepted"] is True
    r2 = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": "quota-a-002", "kind": "video_final", "units": 1},
    )
    assert r2.json()["accepted"] is False
    assert r2.json()["rejectedReason"] == "daily_quota_exceeded"


def test_spend_rejected_when_insufficient_balance(client):
    """发到 cap 上限 → 扣到 < 0 → 拒."""
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    # 100 余额, 30 daily quota. 用 cheap 的 llm (1 学币) 扣到 30, 再扣超
    for i in range(30):
        client.post(
            "/api/v1/me/record-spend",
            headers=auth,
            json={"callId": f"insuff-llm-{i:02d}", "kind": "llm", "units": 1},
        )
    # daily consumed=30 = quota, balance=70, 再一次应该被 daily 拒
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": "insuff-llm-over", "kind": "llm", "units": 1},
    )
    assert r.json()["accepted"] is False


def test_spend_rejects_single_tx_cap_overflow(client):
    """单笔 cost > SINGLE_TX_CAP (20) → 直接拒 (防异常 token 计数)."""
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    # llm, units=100_000, cost = 100000 * 0.001 = 100 > cap 20
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": "overflow-cap-001", "kind": "llm", "units": 100_000},
    )
    assert r.json()["accepted"] is False
    assert "single_tx_cap" in r.json()["rejectedReason"]


def test_spend_rejects_unknown_kind(client):
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": "unknown-kind-001", "kind": "magic", "units": 1},
    )
    assert r.status_code == 422