#!/usr/bin/env python3
"""KidsAI Admin CLI (W4.5 B3).

用法:
  # 给 device 发学币 (lihao 日常用)
  kidsai-admin.py grant <device_id> 50 --reason "种子用户启动"
  kidsai-admin.py grant <device_id> 30

  # 吊销设备 license (用户退款 / 异常)
  kidsai-admin.py revoke <device_id> --reason "退款"

  # 列出最近激活的设备 (方便客服查 device_id)
  kidsai-admin.py list --limit 20

  # 看后端健康
  kidsai-admin.py health

环境变量 (或 .env 文件):
  KIDSAI_SERVER_URL — 后端 URL, 默认 https://kids.ibi.ren
  KIDSAI_ADMIN_TOKEN — 与后端 ADMIN_TOKEN 一致
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.request
import urllib.error
from pathlib import Path


def _load_env_file() -> None:
    """从 .env 或 /etc/kidsai-server/.env 读 KIDSAI_ADMIN_TOKEN."""
    for path in (Path.cwd() / ".env", Path("/etc/kidsai-server/.env")):
        if path.exists():
            try:
                for line in path.read_text().splitlines():
                    line = line.strip()
                    if not line or line.startswith("#") or "=" not in line:
                        continue
                    k, _, v = line.partition("=")
                    os.environ.setdefault(k.strip(), v.strip().strip('"').strip("'"))
            except PermissionError:
                pass


_load_env_file()

SERVER = os.getenv("KIDSAI_SERVER_URL", "https://kids.ibi.ren").rstrip("/")
TOKEN = os.getenv("KIDSAI_ADMIN_TOKEN", "")


def _request(method: str, path: str, body: dict | None = None) -> dict:
    url = f"{SERVER}{path}"
    data = json.dumps(body).encode("utf-8") if body else None
    req = urllib.request.Request(
        url,
        data=data,
        method=method,
        headers={
            "Content-Type": "application/json",
            "X-Admin-Token": TOKEN,
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        sys.exit(f"HTTP {e.code}: {e.read().decode('utf-8', errors='replace')}")
    except urllib.error.URLError as e:
        sys.exit(f"无法连接 {SERVER}: {e.reason}")


def cmd_health(_: argparse.Namespace) -> None:
    with urllib.request.urlopen(f"{SERVER}/healthz", timeout=5) as resp:
        print(json.dumps(json.loads(resp.read()), indent=2))


def cmd_grant(args: argparse.Namespace) -> None:
    body = {"amount": args.amount, "reason": args.reason}
    result = _request("POST", f"/api/v1/admin/devices/{args.device_id}/grant", body)
    print(json.dumps(result, indent=2, ensure_ascii=False))


def cmd_revoke(args: argparse.Namespace) -> None:
    body = {"reason": args.reason}
    url = f"/api/v1/admin/devices/{args.device_id}/revoke"
    req = urllib.request.Request(
        f"{SERVER}{url}",
        data=json.dumps(body).encode("utf-8"),
        method="POST",
        headers={
            "Content-Type": "application/json",
            "X-Admin-Token": TOKEN,
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=10):
            print(f"✅ device {args.device_id} 已吊销 (reason={args.reason})")
    except urllib.error.HTTPError as e:
        sys.exit(f"HTTP {e.code}: {e.read().decode('utf-8', errors='replace')}")


def cmd_list(args: argparse.Namespace) -> None:
    """简化版: 用 SQLite 直读 (需要本机有 DB 文件).
    后端没有 list_devices endpoint, 日常查 device_id 走这里最方便.
    """
    import sqlite3

    db_paths = [
        Path("./data/kidsai.db"),
        Path("/opt/kidsai-server/data/kidsai.db"),
    ]
    db_path = next((p for p in db_paths if p.exists()), None)
    if db_path is None:
        sys.exit("找不到 kidsai.db; 传 --db PATH 或 cd 到后端目录")

    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row
    rows = conn.execute(
        """
        SELECT id, nickname, age_tier, activated_at, last_seen_at, revoked_at
        FROM devices ORDER BY activated_at DESC LIMIT ?
        """,
        (args.limit,),
    ).fetchall()
    if not rows:
        print("无激活设备")
        return

    print(f"{'DEVICE ID':<28} {'NICKNAME':<14} {'AGE':<5} {'ACTIVATED':<14} {'REVOKED'}")
    print("-" * 80)
    for r in rows:
        ts = r["activated_at"] / 1000
        from datetime import datetime
        activated = datetime.fromtimestamp(ts).strftime("%Y-%m-%d %H:%M")
        revoked = "🚫" if r["revoked_at"] else ""
        print(
            f"{r['id']:<28} {r['nickname']:<14} {r['age_tier']:<5} {activated:<14} {revoked}"
        )
    print(f"\n共 {len(rows)} 个设备 (limit={args.limit})")


def main() -> None:
    parser = argparse.ArgumentParser(description="KidsAI Server Admin CLI")
    sub = parser.add_subparsers(dest="cmd", required=True)

    sub.add_parser("health", help="检查后端健康").set_defaults(func=cmd_health)

    g = sub.add_parser("grant", help="给设备发学币")
    g.add_argument("device_id")
    g.add_argument("amount", type=int)
    g.add_argument("--reason", default="admin grant")
    g.set_defaults(func=cmd_grant)

    r = sub.add_parser("revoke", help="吊销设备")
    r.add_argument("device_id")
    r.add_argument("--reason", default="admin revoke")
    r.set_defaults(func=cmd_revoke)

    l = sub.add_parser("list", help="列出最近激活的设备 (本地 SQLite)")
    l.add_argument("--limit", type=int, default=20)
    l.set_defaults(func=cmd_list)

    args = parser.parse_args()
    if args.cmd != "health" and not TOKEN:
        sys.exit("KIDSAI_ADMIN_TOKEN 未设置 (export 或写到 .env)")

    args.func(args)


if __name__ == "__main__":
    main()