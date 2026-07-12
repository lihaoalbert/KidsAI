"""pytest fixtures for kidsai-server tests (W4.5 B1).

策略:
- 用临时 SQLite 文件 (不污染真库)
- 注入固定 JWT_SECRET / ADMIN_TOKEN, 免去每次生成
- TestClient(app) 走真实 FastAPI HTTP 层 (验证序列化 + 鉴权 + 路由)
"""
from __future__ import annotations

import os
import secrets
import tempfile
from pathlib import Path
from typing import Iterator

import pytest
from fastapi.testclient import TestClient

# 测试环境先注入 secret, 不依赖外部 .env
os.environ.setdefault("JWT_SECRET", secrets.token_hex(32))
os.environ.setdefault("ADMIN_TOKEN", "test-admin-token-12345678")
os.environ.setdefault("STARTING_BALANCE", "100")
os.environ.setdefault("DAILY_QUOTA", "30")
os.environ.setdefault("LLM_API_KEY", "test-llm-key-placeholder")
os.environ.setdefault("SEEDANCE_API_KEY", "test-seedance-key-placeholder")
os.environ.setdefault("COST_PER_LLM_TOKEN", "0.001")
os.environ.setdefault("COST_VIDEO_DRAFT", "9")
os.environ.setdefault("COST_VIDEO_FINAL", "19")
os.environ.setdefault("SINGLE_TX_CAP", "20")

# W6 A: 测试隔离 — 不让 .env 里的真 MiniMax key 漏到测试 fixture / pytest 输出.
# 设空串而不是 pop: pop 之后 load_dotenv 会从 .env 重新塞回来
# (因为 dotenv 默认不覆盖已存在的 env var, 但已 pop = 不存在 = 会塞).
# 测试如需 pool 非空, 用 monkeypatch.setenv 显式注入测试 key.
os.environ["MINIMAX_API_KEYS"] = ""
os.environ["MINIMAX_API_KEY"] = ""

from kidsai_server.config import Config, load_config  # noqa: E402
from kidsai_server.main import create_app  # noqa: E402


@pytest.fixture()
def cfg() -> Config:
    return load_config()


@pytest.fixture()
def temp_db_path() -> Iterator[str]:
    with tempfile.TemporaryDirectory() as d:
        yield str(Path(d) / "test.db")


@pytest.fixture()
def client(cfg: Config, temp_db_path: str) -> Iterator[TestClient]:
    app = create_app(cfg=cfg, db_path=temp_db_path)
    with TestClient(app) as c:
        yield c


@pytest.fixture()
def admin_headers(cfg: Config) -> dict[str, str]:
    return {"X-Admin-Token": cfg.admin_token}