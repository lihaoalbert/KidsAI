"""W10 Day 5 — Skill marketplace publisher (server side).

生成 6 个官方种子 skill:
- 3 child: eng-adventure (英语冒险岛), ink-painting (国风水墨), coding-primer (编程启蒙)
- 3 adult: commercial-ad-director (商业广告导演), doc-shortfilm (纪录片分镜), resume-reel (求职作品集)

每个 skill:
- 写 manifest.json (含 publisher_signature RSA-PSS-SHA256 over canonical)
- 写 assets/cover.png 占位 (1x1 PNG)
- 写 prompts/opening.yaml (skill system prompt hint)
- 写 templates/*.json (角色 / 故事弧)
- 安装目录: {skills_root}/{skill_id}/{version}/

CLI:
    python -m kidsai_server.skills_publisher publish
    python -m kidsai_server.skills_publisher verify

设计取舍:
- 全部由 kidsai-official publisher 签发 (单一 publisher)
- 用 RSA-PSS-SHA256 与 secrets_publisher 共享签名通道
- asset 文件只是 placeholder (生产时 publisher 替换为真实 cover.png / bgm.mp3)
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import shutil
import struct
import time
import zlib
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding, rsa


SCHEMA_VERSION = "kidsai.skill/1"
PUBLISHER = "kidsai-official"
PUBKEY_ID = "kidsai-dev-2026-q3"
DEFAULT_VERSION = "v1.0.0"


# ---------- 6 个种子 skill 描述 ----------

SEED_SKILLS: list[dict[str, Any]] = [
    # ---- 3 child ----
    {
        "id": "eng-adventure",
        "name": "英语冒险岛",
        "audience": "child",
        "category": "language",
        "age_tier": [1, 2, 3],
        "credits_per_use": 3,
        "daily_quota": 5,
        "description": "和小探险家 Lily 一起在神秘岛屿学英语 — 寻宝、解谜、闯关，30 个常用单词和 5 句核心句型。",
        "characters": [
            {"id": "lily", "name": "小探险家 Lily", "defaultFormImage": "assets/cover.png"},
            {"id": "parrot", "name": "会说话的鹦鹉 Polly", "defaultFormImage": None},
        ],
        "story_arcs": [
            {
                "id": "treasure-hunt",
                "name": "神秘岛屿寻宝",
                "paragraphs": [
                    "Lily 乘小船来到神秘岛屿, 沙滩上有一封信.",
                    "信封里写着: 'Find 5 magic words to open the treasure chest!'",
                    "Lily 找到 5 个单词, 打开宝箱获得友谊宝石.",
                ],
            },
        ],
        "tabs": ["narrative", "storyboard"],
        "tools": ["translate-hint", "pronunciation-check"],
        "prompts": [
            {"id": "opening", "hint": "Set scene: tropical island, sunset, gentle waves. Lily holds a letter."},
            {"id": "grammar", "hint": "Teach 5 simple English words through Lily's journey."},
        ],
    },
    {
        "id": "ink-painting",
        "name": "国风水墨",
        "audience": "child",
        "category": "art",
        "age_tier": [2, 3],
        "credits_per_use": 4,
        "daily_quota": 3,
        "description": "用水墨画风格创作中国风短片 — 山水、仙鹤、渔翁，体会东方美学意境。",
        "characters": [
            {"id": "scholar", "name": "小书童", "defaultFormImage": None},
            {"id": "crane", "name": "仙鹤", "defaultFormImage": None},
        ],
        "story_arcs": [
            {
                "id": "mountain-walk",
                "name": "山水漫步",
                "paragraphs": [
                    "晨雾笼罩着远山, 小书童提灯出村.",
                    "溪边仙鹤单足而立, 引路向云深处.",
                    "登顶远眺, 天地一色.",
                ],
            },
        ],
        "tabs": ["narrative", "storyboard"],
        "tools": ["ink-wash-style", "seal-stamp"],
        "prompts": [
            {"id": "opening", "hint": "Traditional Chinese ink-wash painting. Mountain ranges, mist, calligraphy aesthetic."},
        ],
    },
    {
        "id": "coding-primer",
        "name": "编程启蒙",
        "audience": "child",
        "category": "stem",
        "age_tier": [3, 4],
        "credits_per_use": 5,
        "daily_quota": 3,
        "description": "用故事学编程基础 — 循环、条件、函数，编程不再是抽象代码而是小英雄的冒险。",
        "characters": [
            {"id": "byte", "name": "小机器人 Byte", "defaultFormImage": None},
            {"id": "loopy", "name": "循环小精灵 Loopy", "defaultFormImage": None},
        ],
        "story_arcs": [
            {
                "id": "loop-adventure",
                "name": "循环之森",
                "paragraphs": [
                    "Byte 进入循环之森, 同一段路走了三遍才找到出口.",
                    "Loopy 教他: 重复三次, 每次向前一步.",
                    "他们用 'for 3 times' 走出了森林.",
                ],
            },
        ],
        "tabs": ["narrative"],
        "tools": ["loop-visualizer", "if-then-hint"],
        "prompts": [
            {"id": "opening", "hint": "Friendly robot character teaches programming basics through adventure. Loop, condition, function."},
        ],
    },
    # ---- 3 adult ----
    {
        "id": "commercial-ad-director",
        "name": "商业广告导演",
        "audience": "adult",
        "category": "commercial",
        "age_tier": [],
        "credits_per_use": 8,
        "daily_quota": 10,
        "description": "30 秒商业广告分镜模板 + 产品特写 prompt 库 — 适合电商带货、品牌宣传、活动预热。",
        "characters": [
            {"id": "model", "name": "产品模特", "defaultFormImage": None},
            {"id": "voiceover", "name": "广告旁白", "defaultFormImage": None},
        ],
        "story_arcs": [
            {
                "id": "30s-product-reveal",
                "name": "30 秒产品揭晓",
                "paragraphs": [
                    "Hook (0-5s): 极端特写 + 悬念文案.",
                    "Problem (5-15s): 用户痛点场景.",
                    "Solution (15-25s): 产品登场, 360° 旋转.",
                    "CTA (25-30s): 行动召唤 + 品牌徽标.",
                ],
            },
        ],
        "tabs": ["narrative", "storyboard", "shotlist"],
        "tools": ["product-shot", "logo-overlay", "voiceover-script"],
        "prompts": [
            {"id": "opening", "hint": "Professional 30-second commercial ad. Product hero shot, dynamic camera, brand color palette."},
            {"id": "voiceover", "hint": "Persuasive copy in Mandarin Chinese, 80-100 characters total."},
        ],
    },
    {
        "id": "doc-shortfilm",
        "name": "纪录片分镜",
        "audience": "adult",
        "category": "documentary",
        "age_tier": [],
        "credits_per_use": 8,
        "daily_quota": 10,
        "description": "纪录片 / 短片分镜 + 真实光摄影 prompt — 适合纪实创作、城市记录、人生故事。",
        "characters": [
            {"id": "subject", "name": "纪录片主人公", "defaultFormImage": None},
            {"id": "narrator", "name": "旁白", "defaultFormImage": None},
        ],
        "story_arcs": [
            {
                "id": "city-24h",
                "name": "城市 24 小时",
                "paragraphs": [
                    "06:00 环卫工人扫街.",
                    "12:00 写字楼白领午休.",
                    "18:00 放学孩子回家.",
                    "24:00 出租车司机末班.",
                ],
            },
        ],
        "tabs": ["narrative", "storyboard", "shotlist"],
        "tools": ["natural-light", "interview-setup", "broll-shot"],
        "prompts": [
            {"id": "opening", "hint": "Cinematic documentary style. Natural light, handheld camera, real-life moments, 24fps."},
        ],
    },
    {
        "id": "resume-reel",
        "name": "求职作品集",
        "audience": "adult",
        "category": "career",
        "age_tier": [],
        "credits_per_use": 6,
        "daily_quota": 10,
        "description": "求职 / 作品集片头模板 + 简约专业风格 — 适合应届毕业生、转行人士、自由职业者。",
        "characters": [
            {"id": "candidate", "name": "求职者", "defaultFormImage": None},
            {"id": "interviewer", "name": "面试官", "defaultFormImage": None},
        ],
        "story_arcs": [
            {
                "id": "60s-intro",
                "name": "60 秒自我介绍",
                "paragraphs": [
                    "Hook: 一句个人宣言.",
                    "Experience: 3 个关键项目截图.",
                    "Skills: 技能清单动画.",
                    "Contact: 邮箱 + LinkedIn QR.",
                ],
            },
        ],
        "tabs": ["narrative", "storyboard"],
        "tools": ["minimal-style", "typography", "qr-overlay"],
        "prompts": [
            {"id": "opening", "hint": "Minimal professional intro reel. Clean typography, muted color palette, 60 seconds."},
        ],
    },
]


# ---------- placeholder PNG (1x1 transparent) ----------

def placeholder_png() -> bytes:
    """最小合法 PNG: 1x1 透明像素. 占位用, 真实 asset 由 publisher 替换."""
    sig = b"\x89PNG\r\n\x1a\n"
    # IHDR
    ihdr_data = struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0)
    ihdr_crc = zlib.crc32(b"IHDR" + ihdr_data) & 0xFFFFFFFF
    ihdr = struct.pack(">I", 13) + b"IHDR" + ihdr_data + struct.pack(">I", ihdr_crc)
    # IDAT
    raw = zlib.compress(b"\x00\x00\x00\x00\x00")
    idat_crc = zlib.crc32(b"IDAT" + raw) & 0xFFFFFFFF
    idat = struct.pack(">I", len(raw)) + b"IDAT" + raw + struct.pack(">I", idat_crc)
    # IEND
    iend_crc = zlib.crc32(b"IEND") & 0xFFFFFFFF
    iend = struct.pack(">I", 0) + b"IEND" + struct.pack(">I", iend_crc)
    return sig + ihdr + idat + iend


# ---------- canonical + sign ----------

def manifest_canonical(m: dict[str, Any]) -> bytes:
    """移除 publisher_signature 字段, 然后 JSON 序列化 (sort_keys + 无空格)."""
    m2 = {k: v for k, v in m.items() if k != "publisher_signature"}
    return json.dumps(m2, sort_keys=True, separators=(",", ":")).encode()


def sign_canonical(canonical: bytes, signing_key: rsa.RSAPrivateKey) -> str:
    """RSA-PSS-SHA256 over canonical bytes, base64.

    客户端 verify 用的是 VerifyingKey<Sha256> 的默认 verify (非 Prehashed),
    所以签的时候也用 signing_key.sign(canonical_bytes, ...) 不指定 Prehashed.
    cryptography 库会内部 sha256(canonical) 再签.
    """
    sig = signing_key.sign(
        canonical,
        padding.PSS(
            mgf=padding.MGF1(hashes.SHA256()),
            salt_length=padding.PSS.MAX_LENGTH,
        ),
        hashes.SHA256(),
    )
    return base64.b64encode(sig).decode()


# ---------- 加载 / 生成签名 key ----------

def load_signing_key(env_name: str = "KIDSAI_SKILL_SIGNING_KEY_PEM") -> rsa.RSAPrivateKey:
    pem = os.environ.get(env_name, "").strip()
    if pem:
        return serialization.load_pem_private_key(pem.encode(), password=None)
    # dev fallback: 持久化到 src-tauri/assets/dev_signing_key.pem
    # 同一台机器上 publish + verify 用同一对 key
    assets_dir = Path(__file__).resolve().parents[2] / "src-tauri" / "assets"
    priv_path = assets_dir / "dev_signing_key.pem"
    pub_path = assets_dir / "dev_signing_pubkey.pem"
    if priv_path.exists():
        priv_pem = priv_path.read_text()
        key = serialization.load_pem_private_key(priv_pem.encode(), password=None)
        return key
    key = rsa.generate_private_key(public_exponent=65537, key_size=2048)
    pub_pem = key.public_key().public_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PublicFormat.SubjectPublicKeyInfo,
    ).decode()
    priv_pem = key.private_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PrivateFormat.PKCS8,
        encryption_algorithm=serialization.NoEncryption(),
    ).decode()
    assets_dir.mkdir(parents=True, exist_ok=True)
    priv_path.write_text(priv_pem)
    priv_path.chmod(0o600)
    pub_path.write_text(pub_pem)
    print(f"[skills] dev keypair 已写入 {priv_path} (chmod 600) + {pub_path}")
    return key


# ---------- publisher ----------

def build_manifest(seed: dict[str, Any], version: str, signing_key: rsa.RSAPrivateKey) -> dict[str, Any]:
    """构造单 skill manifest dict, 含 publisher_signature."""
    # 计算 placeholder asset 大小
    png_size = len(placeholder_png())

    assets = [
        {
            "path": "assets/cover.png",
            "sha256": __import__("hashlib").sha256(placeholder_png()).hexdigest(),
            "size": png_size,
        },
    ]

    prompts = []
    for i, p in enumerate(seed["prompts"]):
        # 占位 yaml — 真实发布时 publisher 用专业 prompt 替换
        yaml_text = f"# {p['id']}\nhint: |\n  {p['hint']}\n"
        prompts.append(
            {
                "id": p["id"],
                "file": f"prompts/{p['id']}.yaml",
                "sha256": __import__("hashlib").sha256(yaml_text.encode()).hexdigest(),
            }
        )

    templates = {
        "characters": [
            {
                "id": c["id"],
                "name": c["name"],
                "defaultFormImage": c.get("defaultFormImage"),
            }
            for c in seed["characters"]
        ],
        "story_arcs": seed["story_arcs"],
    }

    extends = {
        "tabs": seed["tabs"],
        "tools": seed["tools"],
        "characters_inject_into": "directorStore.characterMetas",
    }

    manifest = {
        "schema": SCHEMA_VERSION,
        "id": seed["id"],
        "name": seed["name"],
        "version": version,
        "publisher": PUBLISHER,
        "min_app_version": "0.4.0",
        "age_tier": seed["age_tier"],
        "category": seed["category"],
        "audience": seed["audience"],
        "assets": assets,
        "prompts": prompts,
        "templates": templates,
        "extends": extends,
        "credits_per_use": seed["credits_per_use"],
        "daily_quota": seed["daily_quota"],
        "homepage": f"https://skills.kidsai.example/{seed['id']}",
        "size_bytes": png_size + sum(len(p["hint"]) for p in seed["prompts"]) + 500,
        "publisher_signature": "",  # 占位, 签名后填充
        "publisher_pubkey_id": PUBKEY_ID,
    }

    # 签名
    canonical = manifest_canonical(manifest)
    manifest["publisher_signature"] = sign_canonical(canonical, signing_key)
    return manifest


def publish(skills_root: Path, version: str = DEFAULT_VERSION) -> list[Path]:
    """把 6 个种子 skill 写到 {skills_root}/{skill_id}/{version}/.
    返回写出的目录路径列表."""
    signing_key = load_signing_key()
    written: list[Path] = []
    for seed in SEED_SKILLS:
        skill_dir = skills_root / seed["id"] / version
        skill_dir.mkdir(parents=True, exist_ok=True)
        manifest = build_manifest(seed, version, signing_key)
        (skill_dir / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False))
        # assets/cover.png placeholder
        (skill_dir / "assets").mkdir(exist_ok=True)
        (skill_dir / "assets" / "cover.png").write_bytes(placeholder_png())
        # prompts/<id>.yaml
        (skill_dir / "prompts").mkdir(exist_ok=True)
        for p in seed["prompts"]:
            yaml_text = f"# {p['id']}\nhint: |\n  {p['hint']}\n"
            (skill_dir / "prompts" / f"{p['id']}.yaml").write_text(yaml_text)
        written.append(skill_dir)
        print(f"[skills] published {seed['id']} ({seed['audience']}) → {skill_dir}")
    return written


def verify(skills_root: Path) -> bool:
    """校验所有已发布 skill 的 manifest 签名 (client-side verify 模拟)."""
    from cryptography.hazmat.primitives.asymmetric import rsa as _rsa
    pub_path = Path(__file__).resolve().parents[2] / "src-tauri" / "assets" / "dev_signing_pubkey.pem"
    if not pub_path.exists():
        print(f"[skills] 没找到 pubkey {pub_path}")
        return False
    pub_pem = pub_path.read_text()
    pub_key = serialization.load_pem_public_key(pub_pem.encode())

    ok = True
    for skill_dir in sorted(skills_root.iterdir()):
        if not skill_dir.is_dir():
            continue
        for ver_dir in sorted(skill_dir.iterdir()):
            if not ver_dir.is_dir():
                continue
            manifest_path = ver_dir / "manifest.json"
            if not manifest_path.is_file():
                continue
            m = json.loads(manifest_path.read_text())
            sig_b64 = m.pop("publisher_signature", "")
            canonical = manifest_canonical(m)
            try:
                pub_key.verify(
                    base64.b64decode(sig_b64),
                    canonical,
                    padding.PSS(
                        mgf=padding.MGF1(hashes.SHA256()),
                        salt_length=padding.PSS.MAX_LENGTH,
                    ),
                    hashes.SHA256(),
                )
                print(f"[skills] ✓ {m['id']} v{m['version']} signature valid")
            except Exception as e:
                print(f"[skills] ✗ {m['id']} v{m['version']} signature INVALID: {e}")
                ok = False
            m["publisher_signature"] = sig_b64  # 恢复
    return ok


def main() -> int:
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)
    p_pub = sub.add_parser("publish")
    p_pub.add_argument("--skills-root", default="./skills", help="输出根目录")
    p_pub.add_argument("--version", default=DEFAULT_VERSION)
    p_vfy = sub.add_parser("verify")
    p_vfy.add_argument("--skills-root", default="./skills")
    args = parser.parse_args()

    if args.cmd == "publish":
        root = Path(args.skills_root).resolve()
        root.mkdir(parents=True, exist_ok=True)
        publish(root, args.version)
        return 0
    if args.cmd == "verify":
        root = Path(args.skills_root).resolve()
        return 0 if verify(root) else 1
    return 1


if __name__ == "__main__":
    raise SystemExit(main())