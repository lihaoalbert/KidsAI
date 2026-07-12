"""POST /api/v1/devices/activate — 设备激活 + license 签发."""


def test_activate_new_device_returns_full_envelope(client):
    r = client.post(
        "/api/v1/devices/activate",
        json={
            "fingerprintHash": "fp-device-001-abcdef",
            "nickname": "小米",
            "ageTier": 2,
        },
    )
    assert r.status_code == 200, r.text
    body = r.json()
    assert body["deviceId"]
    assert body["licenseToken"]
    # JWT 三段
    assert body["licenseToken"].count(".") == 2
    # apiKeys 在 camelCase
    assert "apiKeys" in body
    assert body["apiKeys"]["llm"]
    assert body["apiKeys"]["video"]
    assert body["balance"] == 100
    assert body["dailyQuota"] == 30


def test_activate_idempotent_for_same_fingerprint(client):
    payload = {
        "fingerprintHash": "fp-same-device-abcdef",
        "nickname": "小明",
        "ageTier": 1,
    }
    r1 = client.post("/api/v1/devices/activate", json=payload)
    r2 = client.post("/api/v1/devices/activate", json=payload)
    assert r1.status_code == 200
    assert r2.status_code == 200
    # 同 fingerprint 应复用 deviceId (余额不重置)
    assert r1.json()["deviceId"] == r2.json()["deviceId"]
    assert r1.json()["balance"] == r2.json()["balance"] == 100


def test_activate_rejects_short_fingerprint(client):
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": "abc", "nickname": "x", "ageTier": 0},
    )
    assert r.status_code == 422


def test_activate_rejects_blank_nickname(client):
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": "long-enough-fp", "nickname": "", "ageTier": 0},
    )
    assert r.status_code == 422


def test_activate_age_tier_out_of_range(client):
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": "long-enough-fp", "nickname": "x", "ageTier": 5},
    )
    assert r.status_code == 422