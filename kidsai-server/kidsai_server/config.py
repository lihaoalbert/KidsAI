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
    )