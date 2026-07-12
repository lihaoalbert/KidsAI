"""Pydantic 请求/响应 schema (W4.5 B1).

字段命名: 与前端 src/api/tauri.ts + 桌面 license_client 对齐 (camelCase JSON),
但 Python 内部用 snake_case, model_config = alias_generator 双向兼容.
"""
from __future__ import annotations

from pydantic import BaseModel, ConfigDict, Field


def _to_camel(s: str) -> str:
    parts = s.split("_")
    return parts[0] + "".join(p.title() for p in parts[1:])


_CAMEL = ConfigDict(
    alias_generator=_to_camel,
    populate_by_name=True,
    extra="forbid",
)


class ActivateRequest(BaseModel):
    model_config = _CAMEL
    fingerprint_hash: str = Field(min_length=8)
    nickname: str = Field(min_length=1, max_length=32)
    age_tier: int = Field(ge=0, le=3)


class ApiKeys(BaseModel):
    model_config = _CAMEL
    llm: str
    video: str


class ActivateResponse(BaseModel):
    model_config = _CAMEL
    device_id: str
    license_token: str
    api_keys: ApiKeys
    balance: int
    daily_quota: int


class BalanceResponse(BaseModel):
    model_config = _CAMEL
    device_id: str
    balance: int
    daily_consumed: int
    daily_quota: int
    daily_remaining: int


class RecordSpendRequest(BaseModel):
    model_config = _CAMEL
    call_id: str = Field(min_length=8, max_length=64)
    kind: str = Field(pattern="^(llm|video_draft|video_final)$")
    units: int = Field(ge=1)  # LLM=token 数, video=1
    reason: str | None = None


class RecordSpendResponse(BaseModel):
    model_config = _CAMEL
    call_id: str
    balance_after: int
    cost: int
    accepted: bool
    rejected_reason: str | None = None


class RefreshResponse(BaseModel):
    model_config = _CAMEL
    device_id: str
    license_token: str
    api_keys: ApiKeys


class AdminGrantRequest(BaseModel):
    model_config = _CAMEL
    amount: int = Field(ge=1, le=1000)
    reason: str | None = None


class AdminRevokeRequest(BaseModel):
    model_config = _CAMEL
    reason: str | None = None


class HealthResponse(BaseModel):
    status: str
    version: str