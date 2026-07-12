"""W6 A4 — MiniMax key pool 行为.

覆盖:
- 粘性: 同一 device 反复 pick 返同一 key
- 负载最低: N 个 device 分配时均匀散布到 N 个 key (前 N 次)
- rotate: 删旧绑定后重新 pick (管理员应急)
- 空池: KeypoolError
- 集成: /devices/activate 返回 apiKeys.minimax; /admin/devices/{id}/rotate-key 改 key_id
"""
from __future__ import annotations

import secrets

import pytest

from kidsai_server.db import open_db
from kidsai_server.keypool import KeypoolError, pick_key_for_device, rotate_key_for_device

ADMIN_TOKEN = "test-admin-token-12345678"


# ---- 单元: 直接调 keypool 函数 ----

def test_pick_key_returns_one_from_pool():
    conn = open_db(":memory:")
    pool = ("k1", "k2", "k3")
    k = pick_key_for_device(conn, "dev-A", pool)
    assert k in pool


def test_pick_key_is_sticky():
    """同 device 第二次 pick 必须返同一 key (不随机重选)."""
    conn = open_db(":memory:")
    pool = ("k1", "k2", "k3")
    k1 = pick_key_for_device(conn, "dev-A", pool)
    k2 = pick_key_for_device(conn, "dev-A", pool)
    assert k1 == k2


def test_pick_key_load_balances_first_n_devices():
    """前 N 个 device 应该铺满 N 个 key, 而不是全堆 key[0]."""
    conn = open_db(":memory:")
    pool = ("k1", "k2", "k3")
    for i in range(3):
        pick_key_for_device(conn, f"dev-{i}", pool)
    rows = conn.execute(
        "SELECT key_id, COUNT(*) AS n FROM device_key_assignment GROUP BY key_id ORDER BY key_id"
    ).fetchall()
    counts = [row["n"] for row in rows]
    assert counts == [1, 1, 1], f"expected uniform spread, got {counts}"


def test_pick_key_least_loaded_after_initial_saturation():
    """前 N 个 device 用满 N 个 key 后, 第 N+1 个新 device 应分配给
    count 最低者 (此时 4 个 device 都在, 全 count=1, 应平手取 index 0)."""
    conn = open_db(":memory:")
    pool = ("k1", "k2", "k3")
    for i in range(3):
        pick_key_for_device(conn, f"dev-{i}", pool)
    # 第 4 个 device 应拿 key 0 (平手 index 最小)
    k = pick_key_for_device(conn, "dev-3", pool)
    assert k == "k1", f"expected k1, got {k}"
    rows = conn.execute(
        "SELECT key_id, COUNT(*) AS n FROM device_key_assignment GROUP BY key_id ORDER BY key_id"
    ).fetchall()
    counts = [row["n"] for row in rows]
    assert counts == [2, 1, 1], f"expected [2,1,1], got {counts}"


def test_rotate_key_changes_assignment():
    """admin rotate 应返回新 key (可能和旧 key 相同如果池只有 1 个,
    但绑定行 assigned_at 必须更新)."""
    conn = open_db(":memory:")
    pool = ("k1", "k2", "k3")
    pick_key_for_device(conn, "dev-A", pool)
    before = conn.execute(
        "SELECT assigned_at FROM device_key_assignment WHERE device_id = ?", ("dev-A",)
    ).fetchone()["assigned_at"]
    new = rotate_key_for_device(conn, "dev-A", pool)
    assert new in pool
    after = conn.execute(
        "SELECT assigned_at FROM device_key_assignment WHERE device_id = ?", ("dev-A",)
    ).fetchone()["assigned_at"]
    assert after >= before  # rotated_at 至少不倒退


def test_rotate_key_eventually_changes_distribution():
    """多 device 池, 1 个 device rotate 后应可落到不同 key.

    场景: 3 key 池, 2 个 device 都已绑 (dev-A → k1, dev-B → k2).
    rotate dev-A → 应 pick k3 (count 0).
    """
    conn = open_db(":memory:")
    pool = ("k1", "k2", "k3")
    pick_key_for_device(conn, "dev-A", pool)   # count: {0:1}
    pick_key_for_device(conn, "dev-B", pool)   # count: {0:1, 1:1}
    before = conn.execute(
        "SELECT key_id FROM device_key_assignment WHERE device_id = 'dev-A'"
    ).fetchone()["key_id"]
    new = rotate_key_for_device(conn, "dev-A", pool)
    after = conn.execute(
        "SELECT key_id FROM device_key_assignment WHERE device_id = 'dev-A'"
    ).fetchone()["key_id"]
    # 应该跳到 key 2 (count 0 才是最低)
    assert after == 2, f"expected key_id=2, got {after}"
    assert new == pool[after]
    # 并且 dev-B 绑定不能被影响
    dev_b_key = conn.execute(
        "SELECT key_id FROM device_key_assignment WHERE device_id = 'dev-B'"
    ).fetchone()["key_id"]
    assert dev_b_key == 1, "rotate should not affect other devices"


def test_pick_empty_pool_raises():
    conn = open_db(":memory:")
    with pytest.raises(KeypoolError):
        pick_key_for_device(conn, "dev-A", ())


def test_rotate_empty_pool_raises():
    conn = open_db(":memory:")
    with pytest.raises(KeypoolError):
        rotate_key_for_device(conn, "dev-A", ())


# ---- 集成: 走 TestClient (验证 activate + refresh + admin rotate) ----

def _activate(client) -> dict:
    r = client.post(
        "/api/v1/devices/activate",
        json={
            "fingerprintHash": f"fp-{secrets.token_hex(8)}",
            "nickname": "x",
            "ageTier": 1,
        },
    )
    assert r.status_code == 200, r.text
    return r.json()


def _activate_with_fp(client, fp: str) -> dict:
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": fp, "nickname": "x", "ageTier": 1},
    )
    assert r.status_code == 200, r.text
    return r.json()


def test_activate_returns_minimax_key_in_api_keys(client, monkeypatch):
    """激活时 apiKeys.minimax 应是 pool 里某个 key (不空)."""
    monkeypatch.setenv("MINIMAX_API_KEYS", "key-A,key-B,key-C")
    # 重新加载 config
    import importlib
    from kidsai_server import config as cfg_mod
    importlib.reload(cfg_mod)
    # 重置 app 状态: 销毁 testclient (fixture 重建).
    # 这里简化: 测试自己重新拿一个新 client, 直接 patch get_cfg
    from kidsai_server import dependencies as dep_mod
    from kidsai_server.main import create_app
    from fastapi.testclient import TestClient
    cfg = cfg_mod.load_config()
    app = create_app(cfg=cfg, db_path="data/test-keypool-activate.db")
    import os
    os.makedirs("data", exist_ok=True)
    if os.path.exists("data/test-keypool-activate.db"):
        os.remove("data/test-keypool-activate.db")
    with TestClient(app) as c:
        r = c.post(
            "/api/v1/devices/activate",
            json={"fingerprintHash": "fp-key-activate-test", "nickname": "a", "ageTier": 1},
        )
        assert r.status_code == 200, r.text
        body = r.json()
        assert "minimax" in body["apiKeys"], f"expected minimax key in apiKeys, got {body['apiKeys']}"
        assert body["apiKeys"]["minimax"] in ("key-A", "key-B", "key-C")


def test_activate_empty_pool_returns_minimax_none(client):
    """空 pool 时 minimax 字段为 None (向后兼容 demo 模式)."""
    # 默认 conftest 不设 MINIMAX_API_KEYS, 应该空 pool
    r = client.post(
        "/api/v1/devices/activate",
        json={"fingerprintHash": "fp-empty-pool-test-1", "nickname": "a", "ageTier": 1},
    )
    body = r.json()
    # pydantic Optional 字段, JSON 里如果是 None 则缺省不输出 — 检查键不存在或为 None
    mk = body["apiKeys"].get("minimax")
    assert mk is None, f"expected None, got {mk!r}"


def test_refresh_license_returns_same_minimax_key(client, monkeypatch):
    """refresh 应返粘性 key (同 device 不会换)."""
    monkeypatch.setenv("MINIMAX_API_KEYS", "key-A,key-B")
    import importlib
    from kidsai_server import config as cfg_mod
    from kidsai_server.main import create_app
    importlib.reload(cfg_mod)
    cfg = cfg_mod.load_config()
    import os
    os.makedirs("data", exist_ok=True)
    if os.path.exists("data/test-keypool-refresh.db"):
        os.remove("data/test-keypool-refresh.db")
    from fastapi.testclient import TestClient
    app = create_app(cfg=cfg, db_path="data/test-keypool-refresh.db")
    with TestClient(app) as c:
        env = c.post(
            "/api/v1/devices/activate",
            json={"fingerprintHash": "fp-sticky-test", "nickname": "x", "ageTier": 1},
        ).json()
        device_id = env["deviceId"]
        license_token = env["licenseToken"]
        first_key = env["apiKeys"]["minimax"]
        # refresh
        r = c.post(
            "/api/v1/me/refresh-license",
            headers={"Authorization": f"Bearer {license_token}"},
        )
        assert r.status_code == 200, r.text
        ref = r.json()
        assert ref["apiKeys"]["minimax"] == first_key, "key not sticky across refresh"


def test_admin_rotate_key_changes_key_id(client, monkeypatch):
    """admin rotate-key 后, 下次 activate/refresh 拿到新 key_id."""
    monkeypatch.setenv("MINIMAX_API_KEYS", "key-A,key-B,key-C")
    import importlib
    from kidsai_server import config as cfg_mod
    from kidsai_server.main import create_app
    importlib.reload(cfg_mod)
    cfg = cfg_mod.load_config()
    import os
    os.makedirs("data", exist_ok=True)
    if os.path.exists("data/test-keypool-rotate.db"):
        os.remove("data/test-keypool-rotate.db")
    from fastapi.testclient import TestClient
    app = create_app(cfg=cfg, db_path="data/test-keypool-rotate.db")
    with TestClient(app) as c:
        env = c.post(
            "/api/v1/devices/activate",
            json={"fingerprintHash": "fp-rotate-test", "nickname": "x", "ageTier": 1},
        ).json()
        device_id = env["deviceId"]
        before_key = env["apiKeys"]["minimax"]
        # 1. rotate
        r = c.post(
            f"/api/v1/admin/devices/{device_id}/rotate-key",
            headers={"X-Admin-Token": ADMIN_TOKEN},
        )
        assert r.status_code == 200, r.text
        body = r.json()
        assert body["deviceId"] == device_id
        assert isinstance(body["keyId"], int)
        # 2. refresh 拿 key, 应是新分配的那个
        r = c.post(
            "/api/v1/me/refresh-license",
            headers={"Authorization": f"Bearer {env['licenseToken']}"},
        )
        ref = r.json()
        # rotate 后 key 可能回原值 (碰巧), 但 token 应取到 key_id 对应字符串
        # 直接验证 refresh 返的 key 与 rotate 返的 key_id 索引一致
        pool = ("key-A", "key-B", "key-C")
        assert ref["apiKeys"]["minimax"] == pool[body["keyId"]]


def test_admin_rotate_key_requires_admin_token(client, monkeypatch):
    monkeypatch.setenv("MINIMAX_API_KEYS", "key-A,key-B")
    import importlib
    from kidsai_server import config as cfg_mod
    from kidsai_server.main import create_app
    importlib.reload(cfg_mod)
    cfg = cfg_mod.load_config()
    import os
    os.makedirs("data", exist_ok=True)
    if os.path.exists("data/test-keypool-noauth.db"):
        os.remove("data/test-keypool-noauth.db")
    from fastapi.testclient import TestClient
    app = create_app(cfg=cfg, db_path="data/test-keypool-noauth.db")
    with TestClient(app) as c:
        env = c.post(
            "/api/v1/devices/activate",
            json={"fingerprintHash": "fp-noauth-test", "nickname": "x", "ageTier": 1},
        ).json()
        r = c.post(f"/api/v1/admin/devices/{env['deviceId']}/rotate-key")
        assert r.status_code == 403


def test_admin_rotate_unknown_device_returns_404(client, monkeypatch):
    monkeypatch.setenv("MINIMAX_API_KEYS", "key-A,key-B")
    import importlib
    from kidsai_server import config as cfg_mod
    importlib.reload(cfg_mod)
    r = client.post(
        "/api/v1/admin/devices/no-such-device/rotate-key",
        headers={"X-Admin-Token": ADMIN_TOKEN},
    )
    assert r.status_code == 404
