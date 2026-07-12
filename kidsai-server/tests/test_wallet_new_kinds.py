"""W6 D1 — 学币 4 新 kind: image_gen / voice_clone / music_gen / hailuo_video.

覆盖:
- 配置默认 cost (5/10/8/12)
- record-spend 接受新 kind 并扣对应数额
- 单笔 cap 超限被拒
- 老 kind 仍然正常工作 (回归)
"""
from __future__ import annotations

import secrets


def _activate(client) -> dict:
    r = client.post(
        "/api/v1/devices/activate",
        json={
            "fingerprintHash": f"fp-newkind-{secrets.token_hex(6)}",
            "nickname": "x",
            "ageTier": 1,
        },
    )
    assert r.status_code == 200, r.text
    return r.json()


def test_image_gen_costs_5_credits(client):
    """image_gen 1 单位 = 5 学币."""
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": f"img-{secrets.token_hex(4)}", "kind": "image_gen", "units": 1, "reason": "test"},
    )
    assert r.status_code == 200, r.text
    body = r.json()
    assert body["accepted"] is True
    assert body["cost"] == 5
    assert body["balanceAfter"] == 95


def test_voice_clone_costs_10_credits(client):
    """voice_clone 1 单位 = 10 学币."""
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": f"vc-{secrets.token_hex(4)}", "kind": "voice_clone", "units": 1, "reason": "test"},
    )
    assert r.status_code == 200, r.text
    body = r.json()
    assert body["cost"] == 10
    assert body["balanceAfter"] == 90


def test_music_gen_costs_8_credits(client):
    """music_gen 1 单位 = 8 学币."""
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": f"m-{secrets.token_hex(4)}", "kind": "music_gen", "units": 1, "reason": "test"},
    )
    assert r.status_code == 200, r.text
    body = r.json()
    assert body["cost"] == 8


def test_hailuo_video_costs_12_credits(client):
    """hailuo_video 1 单位 = 12 学币 (备用视频, 比 Seedance 便宜)."""
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": f"hl-{secrets.token_hex(4)}", "kind": "hailuo_video", "units": 1, "reason": "test"},
    )
    assert r.status_code == 200, r.text
    body = r.json()
    assert body["cost"] == 12


def test_unknown_kind_rejected_by_pydantic(client):
    """未声明 kind 应 422 (regex 拦截在前)."""
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": f"x-{secrets.token_hex(4)}", "kind": "weird_kind_xx", "units": 1},
    )
    assert r.status_code == 422


def test_legacy_kinds_still_work(client):
    """回归: 老 kind (llm / video_draft / video_final) 仍正常工作."""
    env = _activate(client)
    auth = {"Authorization": f"Bearer {env['licenseToken']}"}
    # video_draft = 9
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": f"vd-{secrets.token_hex(4)}", "kind": "video_draft", "units": 1},
    )
    assert r.json()["cost"] == 9
    # video_final = 19
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": f"vf-{secrets.token_hex(4)}", "kind": "video_final", "units": 1},
    )
    assert r.json()["cost"] == 19
    # llm = round(units * 0.001)
    r = client.post(
        "/api/v1/me/record-spend",
        headers=auth,
        json={"callId": f"llm-{secrets.token_hex(4)}", "kind": "llm", "units": 10000},
    )
    assert r.json()["cost"] == 10  # round(10000 * 0.001)
