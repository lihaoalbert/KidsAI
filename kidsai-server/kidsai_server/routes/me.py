"""/api/v1/me/* — 设备端查询/上报 (需 Bearer license_token)."""
from __future__ import annotations

from fastapi import APIRouter, Depends

from ..auth import LicenseClaims, issue_license, require_license
from ..config import Config
from ..db import now_ms
from ..dependencies import get_cfg, get_conn
from ..models import (
    ApiKeys,
    BalanceResponse,
    RecordSpendRequest,
    RecordSpendResponse,
    RefreshResponse,
    SpendSummaryResponse,
)
from ..wallet import get_balance, record_spend

router = APIRouter(prefix="/api/v1/me", tags=["me"])


def _wallet_cfg(cfg: Config) -> dict:
    return {
        "cost_per_llm_token": cfg.cost_per_llm_token,
        "cost_video_draft": cfg.cost_video_draft,
        "cost_video_final": cfg.cost_video_final,
        "cost_image_gen": cfg.cost_image_gen,
        "cost_voice_clone": cfg.cost_voice_clone,
        "cost_music_gen": cfg.cost_music_gen,
        "cost_hailuo_video": cfg.cost_hailuo_video,
        "single_tx_cap": cfg.single_tx_cap,
    }


@router.get("/balance", response_model=BalanceResponse)
def balance(
    claims: LicenseClaims = Depends(require_license),
    conn=Depends(get_conn),
) -> BalanceResponse:
    info = get_balance(conn, claims.device_id)
    conn.execute(
        "UPDATE devices SET last_seen_at = ? WHERE id = ?",
        (now_ms(), claims.device_id),
    )
    return BalanceResponse(**info)


@router.post("/record-spend", response_model=RecordSpendResponse)
def spend(
    body: RecordSpendRequest,
    claims: LicenseClaims = Depends(require_license),
    conn=Depends(get_conn),
    cfg: Config = Depends(get_cfg),
) -> RecordSpendResponse:
    outcome = record_spend(
        conn,
        claims.device_id,
        body.call_id,
        body.kind,
        body.units,
        body.reason,
        _wallet_cfg(cfg),
    )
    return RecordSpendResponse(
        call_id=outcome.call_id,
        balance_after=outcome.balance_after,
        cost=outcome.cost,
        accepted=outcome.accepted,
        rejected_reason=outcome.rejected_reason,
    )


@router.post("/refresh-license", response_model=RefreshResponse)
def refresh(
    claims: LicenseClaims = Depends(require_license),
    conn=Depends(get_conn),
    cfg: Config = Depends(get_cfg),
) -> RefreshResponse:
    conn.execute(
        "UPDATE devices SET last_seen_at = ? WHERE id = ?",
        (now_ms(), claims.device_id),
    )
    new_token, _ = issue_license(cfg.jwt_secret, claims.device_id, cfg.jwt_ttl_seconds)
    # W6 A3: 复用粘性 MiniMax key — 已绑过则返同一个, 也兼容首次 refresh (空池返 None).
    minimax_key = _pick_minimax_key_for_refresh(conn, cfg, claims.device_id)
    return RefreshResponse(
        device_id=claims.device_id,
        license_token=new_token,
        api_keys=ApiKeys(
            llm=cfg.llm_api_key,
            video=cfg.seedance_api_key,
            minimax=minimax_key or None,
        ),
    )


def _pick_minimax_key_for_refresh(conn, cfg: Config, device_id: str) -> str:
    """refresh 路径: 同 activate, 但失败也不影响 license 续签 (None 走桌面 fallback)."""
    if not cfg.minimax_api_keys:
        return ""
    try:
        from ..keypool import pick_key_for_device  # 局部 import 避免循环
        return pick_key_for_device(conn, device_id, cfg.minimax_api_keys)
    except Exception:
        return ""


@router.get("/spend-summary", response_model=SpendSummaryResponse)
def spend_summary(
    claims: LicenseClaims = Depends(require_license),
    conn=Depends(get_conn),
    cfg: Config = Depends(get_cfg),
) -> SpendSummaryResponse:
    """W6 E4: 今日按 kind 分组的消耗 (HomePage BalanceWidget 旁展示).

    SQL: SELECT kind, SUM(amount) FROM transactions
         WHERE device_id=? AND kind<>'grant' AND created_at >= today_zero
         GROUP BY kind

    注: 'grant' 类型不计消耗; amount 字段对 consume 类是负数, 取 abs() 后累加.
    """
    import time
    today_zero_ms = int(time.time() // 86400 * 86400) * 1000
    rows = conn.execute(
        """
        SELECT kind, SUM(amount) AS total
        FROM transactions
        WHERE device_id = ?
          AND kind <> 'grant'
          AND created_at >= ?
        GROUP BY kind
        """,
        (claims.device_id, today_zero_ms),
    ).fetchall()
    by_kind = {row["kind"]: abs(row["total"]) for row in rows}
    today_total = sum(by_kind.values())
    daily_quota = cfg.daily_quota
    # daily_consumed (LLM token 学币 + 视频/图像/声音/音乐) — 复用 get_balance 算剩余
    info = get_balance(conn, claims.device_id)
    return SpendSummaryResponse(
        device_id=claims.device_id,
        today_total=today_total,
        by_kind=by_kind,
        daily_remaining=info["daily_remaining"],
        daily_quota=daily_quota,
    )