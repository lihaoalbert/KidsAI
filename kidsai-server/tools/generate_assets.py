"""W6 B1: 资产批量生成脚本.

读 asset_manifest_spec.yaml (~200 条), 串行调 MiniMax image-01,
下载到 assets/<kind>/<key>.<ext>, 写 assets/asset-manifest.json.

设计:
- 串行 + 重试 3 次 + 指数退避 (MiniMax 文档 QPS 限制不严, 但我们不并发
  以避免 429).
- 失败清单写 assets/_failed.json 待人工补 (skill 10s/张, 跑批 ~17min).
- 默认 dry-run 只打印清单不下发 (用 --execute 真跑).
- API key 从 MINIMAX_API_KEYS (逗号分隔) 任选一个 round-robin 用.
- 命名: 用 hyphen `asset-manifest.json` 跟 nginx vhost + API endpoint /api/v1/asset-manifest 对齐
  (W6 D5 部署后改, 早期可能还有 underscore 旧文件, deploy_assets.sh tar 上传前 rm 掉).

用法:
    # 1. dry-run: 打印将生成的清单
    python tools/generate_assets.py

    # 2. 真跑: 配 .env MINIMAX_API_KEYS=key1,key2 然后 --execute
    python tools/generate_assets.py --execute

    # 3. 只跑某个 kind (例如想先看 character 的质量)
    python tools/generate_assets.py --execute --kind character
"""
from __future__ import annotations

import argparse
import json
import os
import random
import sys
import time
from pathlib import Path
from typing import Any

import requests
import yaml


# ─── 路径 / 常量 ───────────────────────────────────────────
THIS_DIR = Path(__file__).resolve().parent
ROOT_DIR = THIS_DIR.parent.parent
SPEC_PATH = THIS_DIR / "asset_manifest_spec.yaml"
ASSETS_DIR = ROOT_DIR / "assets"
MANIFEST_PATH = ASSETS_DIR / "asset-manifest.json"
FAILED_PATH = ASSETS_DIR / "_failed.json"

MINIMAX_BASE = "https://api.minimaxi.com/v1"
IMAGE_GEN_URL = f"{MINIMAX_BASE}/image_generation"
DEFAULT_TIMEOUT = 60  # 单次 HTTP 60s, image-01 实际 5-15s
MAX_RETRIES = 3
RETRY_BACKOFF = 2.0  # 指数退避基数


# ─── helpers ───────────────────────────────────────────────
def load_keys() -> list[str]:
    """从 env 读 MiniMax key 池, 兼容单 key 字段."""
    raw = os.getenv("MINIMAX_API_KEYS", "").strip()
    if raw:
        keys = [k.strip() for k in raw.split(",") if k.strip()]
        if keys:
            return keys
    legacy = os.getenv("MINIMAX_API_KEY", "").strip()
    if legacy:
        return [legacy]
    return []


def load_spec() -> list[dict[str, Any]]:
    """yaml → list of {key, kind, prompt, aspect, negative}."""
    if not SPEC_PATH.exists():
        sys.exit(f"spec not found: {SPEC_PATH}")
    raw = yaml.safe_load(SPEC_PATH.read_text(encoding="utf-8"))
    defaults = raw.pop("defaults", {}) or {}
    default_negative = defaults.get("negative", "")
    default_aspect = defaults.get("aspect_default", "1:1")
    entries: list[dict[str, Any]] = []
    for kind, items in raw.items():
        if not isinstance(items, list):
            continue
        for item in items:
            entries.append({
                "kind": kind,
                "key": item["key"],
                "prompt": item["prompt"],
                "aspect": item.get("aspect", default_aspect),
                "negative": item.get("negative", default_negative),
            })
    return entries


def call_image_gen(api_key: str, prompt: str, aspect: str) -> str:
    """调 MiniMax image_generation → 返 image url.

    Raises RuntimeError on hard failure (status code or empty url).
    """
    body = {
        "model": "image-01",
        "prompt": prompt,
        "aspect_ratio": aspect,
        "response_format": "url",
        "n": 1,
    }
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json",
    }
    r = requests.post(IMAGE_GEN_URL, json=body, headers=headers, timeout=DEFAULT_TIMEOUT)
    r.raise_for_status()
    data = r.json()
    # MiniMax 响应: {"base_resp": {"status_code": 0, "status_msg": "success"},
    #               "data": {"image_urls": [...]}} 或 "data": [{"url": "..."}]
    base = data.get("base_resp") or {}
    if base.get("status_code", 0) not in (0, 1000, "0", "1000"):
        raise RuntimeError(f"minimax err: {base}")
    # 容错: 多种 schema 都见过
    urls: list[str] = []
    if isinstance(data.get("data"), dict):
        d = data["data"]
        urls = d.get("image_urls") or d.get("urls") or []
    elif isinstance(data.get("data"), list):
        urls = [u.get("url") for u in data["data"] if isinstance(u, dict)]
    if not urls or not urls[0]:
        raise RuntimeError(f"no url in response: {data}")
    return urls[0]


def download(url: str, dest: Path) -> None:
    """流式下载 url → dest, 自动判断扩展名."""
    with requests.get(url, stream=True, timeout=DEFAULT_TIMEOUT) as r:
        r.raise_for_status()
        # 从 Content-Type / url 推断扩展名
        ext = ".png"
        ct = r.headers.get("content-type", "").lower()
        if "jpeg" in ct or "jpg" in ct:
            ext = ".jpg"
        elif "webp" in ct:
            ext = ".webp"
        if dest.suffix.lower() not in {".png", ".jpg", ".webp"}:
            dest = dest.with_suffix(ext)
        dest.parent.mkdir(parents=True, exist_ok=True)
        with dest.open("wb") as f:
            for chunk in r.iter_content(chunk_size=8192):
                if chunk:
                    f.write(chunk)


def rotate_key(idx: int, keys: list[str]) -> tuple[str, int]:
    """Round-robin 取 key. idx 是当前调用计数."""
    return keys[idx % len(keys)], idx + 1


def process_one(
    entry: dict[str, Any],
    keys: list[str],
    dry_run: bool,
    progress_idx: int,
) -> tuple[bool, str]:
    """处理单条 entry. 返 (success, message).

    dry_run=True 时只打印计划不实际调用.
    progress_idx: 当前全局 index (用于 round-robin key).
    """
    kind, key, prompt, aspect = entry["kind"], entry["key"], entry["prompt"], entry["aspect"]
    rel = f"{kind}/{key}.png"
    if dry_run:
        return True, f"[plan] {rel} aspect={aspect}"
    api_key, progress_idx = rotate_key(progress_idx, keys)
    last_err: Exception | None = None
    for attempt in range(MAX_RETRIES):
        try:
            url = call_image_gen(api_key, prompt, aspect)
            dest = ASSETS_DIR / kind / f"{key}.png"
            download(url, dest)
            return True, f"[ok] {rel}"
        except Exception as e:
            last_err = e
            wait = RETRY_BACKOFF ** attempt + random.uniform(0, 1)
            time.sleep(wait)
    return False, f"[fail] {rel}: {last_err}"


# ─── main ──────────────────────────────────────────────────
def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--execute", action="store_true",
                   help="实际调用 MiniMax API (默认 dry-run)")
    p.add_argument("--kind", type=str, default=None,
                   help="只跑某个 kind (character/style/bg/...)")
    p.add_argument("--resume", action="store_true",
                   help="跳过已存在的 png (断点续传)")
    p.add_argument("--limit", type=int, default=None,
                   help="最多跑 N 条 (调试用)")
    args = p.parse_args()

    entries = load_spec()
    if args.kind:
        entries = [e for e in entries if e["kind"] == args.kind]
        if not entries:
            sys.exit(f"no entries for kind={args.kind}")
    if args.limit:
        entries = entries[: args.limit]

    print(f"→ entries: {len(entries)} (dry_run={not args.execute})")
    if args.execute:
        keys = load_keys()
        if not keys:
            sys.exit("MINIMAX_API_KEYS / MINIMAX_API_KEY 未设置, 无法 --execute")
        print(f"→ keys loaded: {len(keys)}")
        ASSETS_DIR.mkdir(parents=True, exist_ok=True)

    manifest: dict[str, str] = {}
    if MANIFEST_PATH.exists():
        try:
            manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
        except Exception:
            manifest = {}

    failed: list[dict[str, str]] = []
    successes = 0
    progress_idx = 0
    started = time.time()
    for i, entry in enumerate(entries, 1):
        rel = f"{entry['kind']}/{entry['key']}.png"
        if args.resume and (ASSETS_DIR / rel).exists():
            successes += 1
            manifest[entry["key"]] = rel
            continue
        ok, msg = process_one(entry, keys if args.execute else [], dry_run=not args.execute, progress_idx=progress_idx)
        if args.execute:
            progress_idx += 1
        print(f"  [{i}/{len(entries)}] {msg}")
        if ok:
            successes += 1
            if args.execute:
                manifest[entry["key"]] = rel
        else:
            failed.append({"key": entry["key"], "kind": entry["kind"], "error": msg})

    if args.execute:
        MANIFEST_PATH.write_text(
            json.dumps(
                {
                    "version": int(time.time()),
                    "generated_count": successes,
                    "images": manifest,
                },
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        if failed:
            FAILED_PATH.write_text(json.dumps(failed, ensure_ascii=False, indent=2), encoding="utf-8")
            print(f"\n!! {len(failed)} 失败, 见 {FAILED_PATH.name}")
        elapsed = time.time() - started
        print(f"\n✓ done: {successes}/{len(entries)} 成功, {elapsed:.1f}s")
        print(f"  manifest: {MANIFEST_PATH.relative_to(ROOT_DIR)}")
    else:
        print(f"\n(dry-run) 共 {len(entries)} 条; 加 --execute 真跑")

    return 0 if not failed else 2


if __name__ == "__main__":
    sys.exit(main())