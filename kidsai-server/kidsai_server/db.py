"""SQLite 连接 + 迁移 (W4.5 B1).

表:
- devices: id (uuid) PK, fingerprint_hash, nickname, age_tier, activated_at, last_seen_at, revoked_at
- wallets: device_id PK, balance, daily_quota, daily_consumed, last_reset_date
- transactions: id (call_id) PK UNIQUE 幂等, device_id, kind, amount, reason, created_at
- audit_log: admin 操作审计

启动时幂等 CREATE TABLE IF NOT EXISTS, 无外部迁移工具 (单进程).
"""
from __future__ import annotations

import sqlite3
from pathlib import Path
from typing import Iterator

_SCHEMA = """
CREATE TABLE IF NOT EXISTS devices (
    id              TEXT PRIMARY KEY,
    fingerprint_hash TEXT NOT NULL,
    nickname        TEXT NOT NULL,
    age_tier        INTEGER NOT NULL DEFAULT 0,
    activated_at    INTEGER NOT NULL,
    last_seen_at    INTEGER,
    revoked_at      INTEGER
);
CREATE INDEX IF NOT EXISTS idx_devices_fingerprint ON devices(fingerprint_hash);

CREATE TABLE IF NOT EXISTS wallets (
    device_id       TEXT PRIMARY KEY,
    balance         INTEGER NOT NULL DEFAULT 0,
    daily_quota     INTEGER NOT NULL DEFAULT 30,
    daily_consumed  INTEGER NOT NULL DEFAULT 0,
    last_reset_date TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_wallets_device ON wallets(device_id);

CREATE TABLE IF NOT EXISTS transactions (
    call_id         TEXT PRIMARY KEY,
    device_id       TEXT NOT NULL,
    kind            TEXT NOT NULL,   -- consume | refund | grant
    amount          INTEGER NOT NULL, -- 学币, 正=进, 负=出
    reason          TEXT,
    created_at      INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tx_device ON transactions(device_id);
CREATE INDEX IF NOT EXISTS idx_tx_created ON transactions(created_at);

CREATE TABLE IF NOT EXISTS audit_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id       TEXT,
    action          TEXT NOT NULL,
    payload_json    TEXT,
    created_at      INTEGER NOT NULL
);
"""


def open_db(path: str) -> sqlite3.Connection:
    p = Path(path)
    p.parent.mkdir(parents=True, exist_ok=True)
    # check_same_thread=False: FastAPI TestClient 在不同线程调 endpoint,
    # 业务逻辑不跨线程写同一个 conn (单进程服务, 无并发写)
    conn = sqlite3.connect(str(p), isolation_level=None, check_same_thread=False)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    conn.execute("PRAGMA journal_mode = WAL")
    conn.executescript(_SCHEMA)
    return conn


def get_conn(conn: sqlite3.Connection | None) -> sqlite3.Connection:
    """依赖注入桩; 在 FastAPI 启动时 main.py 创建并塞进 app.state."""
    if conn is None:
        raise RuntimeError("DB conn not initialized (call init_db first)")
    return conn


def now_ms() -> int:
    import time
    return int(time.time() * 1000)


def today_str() -> str:
    """YYYY-MM-DD 本地日期, daily quota 重置用."""
    import datetime
    return datetime.date.today().isoformat()


# pytest fixture helper
def make_test_conn(tmp_path) -> Iterator[sqlite3.Connection]:
    conn = open_db(str(tmp_path / "test.db"))
    yield conn
    conn.close()