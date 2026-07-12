"""学币扣减/退款/发放 (W4.5 B1).

设计:
- 所有 wallet 操作走 record_transaction, call_id UNIQUE 幂等
- 同 device 同一 call_id 重复提交 → 返回上次结果 (不重复扣)
- cost 上限 = single_tx_cap, 超过直接拒 (防异常 token 计数)
- daily_consumed 在跨天时自动重置 (last_reset_date 比较)
"""
from __future__ import annotations

import sqlite3
from dataclasses import dataclass
from typing import Literal

from .db import now_ms, today_str

TxKind = Literal["consume", "refund", "grant"]


@dataclass(frozen=True)
class SpendOutcome:
    accepted: bool
    call_id: str
    cost: int
    balance_after: int
    rejected_reason: str | None = None


class WalletError(Exception):
    pass


def _calc_cost(kind: str, units: int, cfg: dict) -> int:
    if kind == "llm":
        # LLM: cost = round(units * cost_per_llm_token). sub-1000 token 调用
        # 会得到 cost=0, 仍插入 transactions 但不扣学币/不烧 quota — 真实
        # 按 token 折算, 不强制 1 学币 minimum (which 否则会把 100 token 短
        # 回复按 1000 token 收费, 跟 COST_PER_LLM_TOKEN 配置不一致).
        return int(round(units * cfg["cost_per_llm_token"]))
    if kind == "video_draft":
        return cfg["cost_video_draft"]
    if kind == "video_final":
        return cfg["cost_video_final"]
    # W6 D: 新 4 种 MiniMax 能力 — units 始终 = 1 (按调用次数计费).
    if kind == "image_gen":
        return cfg["cost_image_gen"]
    if kind == "voice_clone":
        return cfg["cost_voice_clone"]
    if kind == "music_gen":
        return cfg["cost_music_gen"]
    if kind == "hailuo_video":
        return cfg["cost_hailuo_video"]
    raise WalletError(f"unknown kind: {kind}")


def _maybe_reset_daily(conn: sqlite3.Connection, device_id: str) -> None:
    today = today_str()
    row = conn.execute(
        "SELECT last_reset_date FROM wallets WHERE device_id = ?", (device_id,)
    ).fetchone()
    if row is None:
        return
    if row["last_reset_date"] != today:
        conn.execute(
            "UPDATE wallets SET daily_consumed = 0, last_reset_date = ? WHERE device_id = ?",
            (today, device_id),
        )


def get_balance(conn: sqlite3.Connection, device_id: str) -> dict:
    _maybe_reset_daily(conn, device_id)
    row = conn.execute(
        "SELECT balance, daily_consumed, daily_quota FROM wallets WHERE device_id = ?",
        (device_id,),
    ).fetchone()
    if row is None:
        raise WalletError(f"wallet not found for device {device_id}")
    remaining = max(0, row["daily_quota"] - row["daily_consumed"])
    return {
        "device_id": device_id,
        "balance": row["balance"],
        "daily_consumed": row["daily_consumed"],
        "daily_quota": row["daily_quota"],
        "daily_remaining": remaining,
    }


def create_wallet(
    conn: sqlite3.Connection,
    device_id: str,
    starting_balance: int,
    daily_quota: int,
) -> None:
    """新设备激活时调用, 幂等 (INSERT OR IGNORE)."""
    conn.execute(
        """
        INSERT OR IGNORE INTO wallets (device_id, balance, daily_quota, daily_consumed, last_reset_date)
        VALUES (?, ?, ?, 0, ?)
        """,
        (device_id, starting_balance, daily_quota, today_str()),
    )


def grant(
    conn: sqlite3.Connection,
    device_id: str,
    amount: int,
    reason: str | None,
    call_id: str,
) -> SpendOutcome:
    """管理员发放 (正 amount)."""
    if amount < 1:
        raise WalletError("grant amount must be positive")
    existing = _lookup_tx(conn, call_id)
    if existing is not None:
        return existing
    _ensure_wallet_exists(conn, device_id)
    conn.execute(
        "UPDATE wallets SET balance = balance + ? WHERE device_id = ?",
        (amount, device_id),
    )
    conn.execute(
        "INSERT INTO transactions (call_id, device_id, kind, amount, reason, created_at) VALUES (?, ?, 'grant', ?, ?, ?)",
        (call_id, device_id, amount, reason, now_ms()),
    )
    balance = conn.execute(
        "SELECT balance FROM wallets WHERE device_id = ?", (device_id,)
    ).fetchone()["balance"]
    return SpendOutcome(True, call_id, amount, balance)


def record_spend(
    conn: sqlite3.Connection,
    device_id: str,
    call_id: str,
    kind: str,
    units: int,
    reason: str | None,
    cfg: dict,
) -> SpendOutcome:
    """桌面端上报一次调用, 扣学币.

    幂等: 同 call_id 第二次调用返首次结果 (不重复扣).
    拒绝条件: 单笔 cap 超限 / 余额不足 / daily quota 用完.
    """
    existing = _lookup_tx(conn, call_id)
    if existing is not None:
        return existing

    cost = _calc_cost(kind, units, cfg)
    if cost > cfg["single_tx_cap"]:
        return SpendOutcome(
            False, call_id, cost, _peek_balance(conn, device_id),
            rejected_reason=f"cost {cost} > single_tx_cap {cfg['single_tx_cap']}",
        )

    _maybe_reset_daily(conn, device_id)
    bal_row = conn.execute(
        "SELECT balance, daily_consumed, daily_quota FROM wallets WHERE device_id = ?",
        (device_id,),
    ).fetchone()
    if bal_row is None:
        return SpendOutcome(False, call_id, cost, 0, rejected_reason="wallet not found")

    if bal_row["balance"] < cost:
        return SpendOutcome(
            False, call_id, cost, bal_row["balance"],
            rejected_reason="insufficient_balance",
        )
    if bal_row["daily_consumed"] + cost > bal_row["daily_quota"]:
        return SpendOutcome(
            False, call_id, cost, bal_row["balance"],
            rejected_reason="daily_quota_exceeded",
        )

    conn.execute(
        "UPDATE wallets SET balance = balance - ?, daily_consumed = daily_consumed + ? WHERE device_id = ?",
        (cost, cost, device_id),
    )
    conn.execute(
        "INSERT INTO transactions (call_id, device_id, kind, amount, reason, created_at) VALUES (?, ?, 'consume', ?, ?, ?)",
        (call_id, device_id, -cost, reason, now_ms()),
    )
    new_balance = bal_row["balance"] - cost
    return SpendOutcome(True, call_id, cost, new_balance)


# ============ 内部 helpers ============

def _lookup_tx(conn: sqlite3.Connection, call_id: str) -> SpendOutcome | None:
    row = conn.execute(
        "SELECT call_id, device_id, amount FROM transactions WHERE call_id = ?",
        (call_id,),
    ).fetchone()
    if row is None:
        return None
    bal = conn.execute(
        "SELECT balance FROM wallets WHERE device_id = ?", (row["device_id"],)
    ).fetchone()
    return SpendOutcome(
        accepted=True,
        call_id=row["call_id"],
        cost=abs(row["amount"]),
        balance_after=bal["balance"] if bal else 0,
    )


def _peek_balance(conn: sqlite3.Connection, device_id: str) -> int:
    row = conn.execute(
        "SELECT balance FROM wallets WHERE device_id = ?", (device_id,)
    ).fetchone()
    return row["balance"] if row else 0


def _ensure_wallet_exists(conn: sqlite3.Connection, device_id: str) -> None:
    row = conn.execute(
        "SELECT 1 FROM wallets WHERE device_id = ?", (device_id,)
    ).fetchone()
    if row is None:
        raise WalletError(f"wallet not found for device {device_id}")