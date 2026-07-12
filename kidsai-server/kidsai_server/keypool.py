"""MiniMax API key 池 (W6 A2).

设计:
- 后端持有 N 个 MiniMax API key (Config.minimax_api_keys),
  每个桌面激活时**粘性**绑定一个, 避免跨 key 切导致 token 状态断 / 浪费.
- 分配策略: 同 key_id 的活跃 device 计数最小者优先 (>=1h 算闲置).
- admin rotate: 删绑定, 重新 pick — 期间同 device 并发请求可能短暂拿旧 key,
  重复 record-spend 自动 dedup (`call_id UNIQUE` 兜底).
- 空池回退: 若 minimax_api_keys 为空 (demo / 未配), 返空字符串,
  desktop 端走 mock fallback (保留向后兼容).
"""
from __future__ import annotations

import sqlite3
from typing import Sequence

from .db import now_ms


class KeypoolError(Exception):
    pass


def _pick_least_loaded_key(
    conn: sqlite3.Connection,
    pool_size: int,
    exclude_key_id: int | None = None,
) -> int:
    """在 [0, pool_size) 全量枚举, 选 key_id 引用数最少者 (平手取小 index).

    exclude_key_id: 若指定, 跳过该 key (admin rotate 强制换 key 用).
    """
    rows = conn.execute(
        "SELECT key_id, COUNT(*) AS n FROM device_key_assignment GROUP BY key_id"
    ).fetchall()
    used = {row["key_id"]: row["n"] for row in rows}
    if pool_size <= 0:
        raise KeypoolError("pool_size must be > 0")
    # 若所有可选项都被 exclude (单 key 池) → 兜底返 0
    if exclude_key_id is not None and pool_size == 1:
        return 0
    best_id = -1
    best_n = None
    for kid in range(pool_size):
        if kid == exclude_key_id:
            continue
        n = used.get(kid, 0)
        if best_n is None or n < best_n:
            best_id = kid
            best_n = n
    if best_id < 0:
        # 所有 key 都被 exclude 了 → 兜底返 0 (理论上 pool_size>=1 时不会)
        return 0 if exclude_key_id != 0 else 1
    return best_id


def pick_key_for_device(
    conn: sqlite3.Connection,
    device_id: str,
    pool: Sequence[str],
    exclude_key_id: int | None = None,
) -> str:
    """粘性选 key: 已绑返原值; 未绑按负载最低分配并 INSERT.

    exclude_key_id: 让分配跳过该 index (rotate 路径强制换 key).

    Raises KeypoolError if pool is empty (caller should fall back to ""/mock).
    """
    if not pool:
        raise KeypoolError("minimax key pool is empty")

    existing = conn.execute(
        "SELECT key_id FROM device_key_assignment WHERE device_id = ?",
        (device_id,),
    ).fetchone()
    if existing is not None and exclude_key_id is None:
        kid = existing["key_id"]
        if 0 <= kid < len(pool):
            return pool[kid]
        # 越界 (池缩减) → 删旧绑定, 重选
        conn.execute(
            "DELETE FROM device_key_assignment WHERE device_id = ?",
            (device_id,),
        )

    # 简化: 总是从 index 0 开始 round-robin 分配, 直到所有 key 都至少绑过一台
    # 然后挑绑定数最少者 (least-loaded). 用 `INSERT ... ON CONFLICT DO NOTHING`
    # 保证并发幂等 (race 时拿到 INSERT 失败的人 re-read).
    assigned_id = _pick_least_loaded_key(conn, len(pool), exclude_key_id=exclude_key_id)
    conn.execute(
        """
        INSERT INTO device_key_assignment (device_id, key_id, assigned_at)
        VALUES (?, ?, ?)
        ON CONFLICT(device_id) DO NOTHING
        """,
        (device_id, assigned_id, now_ms()),
    )
    # 兜底: 如果 ON CONFLICT 把我们拒了 (并发 race), 重新读别人的写入.
    final = conn.execute(
        "SELECT key_id FROM device_key_assignment WHERE device_id = ?",
        (device_id,),
    ).fetchone()
    if final is None:
        raise KeypoolError("failed to persist key assignment")
    kid = final["key_id"]
    if not (0 <= kid < len(pool)):
        raise KeypoolError(f"assigned key_id {kid} out of pool range {len(pool)}")
    return pool[kid]


def rotate_key_for_device(
    conn: sqlite3.Connection,
    device_id: str,
    pool: Sequence[str],
) -> str:
    """admin 强制轮换: 删旧绑定, 强制 pick 一个不同的 key.

    返回新分配的 key 字符串. 若池只有 1 个 key, 兜底返同一个 (但 assigned_at 更新).
    """
    if not pool:
        raise KeypoolError("minimax key pool is empty")
    old_row = conn.execute(
        "SELECT key_id FROM device_key_assignment WHERE device_id = ?",
        (device_id,),
    ).fetchone()
    old_kid = old_row["key_id"] if old_row else None
    conn.execute(
        "DELETE FROM device_key_assignment WHERE device_id = ?",
        (device_id,),
    )
    return pick_key_for_device(conn, device_id, pool, exclude_key_id=old_kid)
