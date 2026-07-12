"""FastAPI Depends 提供者 — 独立模块避免 main ↔ routes 循环 import."""
from __future__ import annotations

import sqlite3

from fastapi import Request

from .config import Config


def get_conn(request: Request) -> sqlite3.Connection:
    return request.app.state.db


def get_cfg(request: Request) -> Config:
    return request.app.state.cfg