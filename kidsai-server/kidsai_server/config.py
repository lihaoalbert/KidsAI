"""环境变量集中加载 (W4.5 B1).

设计:
- 单一 Config 对象从 .env 读取, 启动时 assert 必填项
- 不在代码里硬编码任何密钥/默认值 — 缺 JWT_SECRET 直接报错
- 学币计费常量从环境读, 让 lihao 调单价不需要改代码
"""
from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

from dotenv import load_dotenv


@dataclass(frozen=True)
class Config:
    jwt_secret: str
    jwt_ttl_seconds: int
    admin_token: str
    starting_balance: int
    daily_quota: int
    cost_per_llm_token: float
    cost_video_draft: int
    cost_video_final: int
    single_tx_cap: int
    database_path: str
    llm_api_key: str
    seedance_api_key: str
    port: int
    # W6: 新能力成本 + MiniMax key 池
    minimax_api_keys: tuple[str, ...]
    cost_image_gen: int
    cost_voice_clone: int
    cost_music_gen: int
    cost_hailuo_video: int
    # W10: Skill marketplace + W11: Secrets bundle — 公开包/加密包分别落盘
    skills_root: str
    secrets_root: str


def load_config(env_file: str | None = None) -> Config:
    if env_file:
        load_dotenv(env_file)
    else:
        # 从项目根向上找 .env
        root = Path(__file__).resolve().parents[1]
        for candidate in (root / ".env", root.parent / ".env"):
            if candidate.exists():
                load_dotenv(candidate)
                break

    jwt_secret = os.getenv("JWT_SECRET", "").strip()
    if not jwt_secret or len(jwt_secret) < 32:
        raise RuntimeError(
            "JWT_SECRET 缺失或 < 32 字节; 用 `openssl rand -hex 32` 生成"
        )

    admin_token = os.getenv("ADMIN_TOKEN", "").strip()
    if not admin_token or len(admin_token) < 8:
        raise RuntimeError("ADMIN_TOKEN 缺失或太短")

    # W6 A1: MiniMax API key 池 — 优先读 MINIMAX_API_KEYS (逗号分隔),
    # 回退到旧单 key 字段 MINIMAX_API_KEY (兼容老部署).
    minimax_keys_raw = os.getenv("MINIMAX_API_KEYS", "").strip()
    if minimax_keys_raw:
        minimax_api_keys = tuple(k.strip() for k in minimax_keys_raw.split(",") if k.strip())
    else:
        legacy = os.getenv("MINIMAX_API_KEY", "").strip()
        minimax_api_keys = (legacy,) if legacy else ()

    return Config(
        jwt_secret=jwt_secret,
        jwt_ttl_seconds=int(os.getenv("JWT_TTL_SECONDS", "86400")),
        admin_token=admin_token,
        starting_balance=int(os.getenv("STARTING_BALANCE", "100")),
        daily_quota=int(os.getenv("DAILY_QUOTA", "30")),
        cost_per_llm_token=float(os.getenv("COST_PER_LLM_TOKEN", "0.001")),
        cost_video_draft=int(os.getenv("COST_VIDEO_DRAFT", "9")),
        cost_video_final=int(os.getenv("COST_VIDEO_FINAL", "19")),
        single_tx_cap=int(os.getenv("SINGLE_TX_CAP", "20")),
        database_path=os.getenv("DATABASE_PATH", "./data/kidsai.db"),
        llm_api_key=os.getenv("LLM_API_KEY", ""),
        seedance_api_key=os.getenv("SEEDANCE_API_KEY", ""),
        port=int(os.getenv("PORT", "8080")),
        minimax_api_keys=minimax_api_keys,
        cost_image_gen=int(os.getenv("COST_IMAGE_GEN", "5")),
        cost_voice_clone=int(os.getenv("COST_VOICE_CLONE", "10")),
        cost_music_gen=int(os.getenv("COST_MUSIC_GEN", "8")),
        cost_hailuo_video=int(os.getenv("COST_HAILUO_VIDEO", "12")),
        skills_root=os.getenv("SKILLS_ROOT", "./skills"),
        secrets_root=os.getenv("SECRETS_ROOT", "./secrets_out"),
    )