"""W11 server-side — Secrets publisher.

管理 kidsai-server 上的 prompts bundle 发布:
- 读取 prompts/{profile}/ 目录 (profile = child / adult)
- 拼接所有 YAML/JSON 为单个 plaintext (按 path 排序, 保证确定性)
- 用 master_key (AES-256-GCM) 加密 → bundle.bin
- 构造 manifest: 文件列表 + sha256 + cipher info + RSA-PSS signature
- 写入 server secrets storage

master_key 不直接存放; 启动期从 env KIDSAI_SECRETS_MASTER_KEY (hex 64 chars) 加载.
RSA 私钥从 env KIDSAI_SECRETS_SIGNING_KEY_PEM 加载.

设计取舍 (vs 原 plan):
原 plan 写 HKDF(license_token) 加密 bundle, 实际让 server 无法让所有设备复用
同一个 bundle (每台设备 key 不同 = CDN 没意义). 改为:
- bundle.bin 用 master_key (共享, CDN 友好)
- 设备 fetch 时, server 用 license_token 派生 KEK 包裹 master_key → wrapped_master_for_device.bin
- 客户端: license_token → HKDF → KEK → unwrap master_key → decrypt bundle
这样 bundle 在 CDN/edge 缓存, 只有 wrapped_master 是 per-device (很小, ~60 bytes).
"""

import base64
import json
import os
import secrets as _secrets
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import padding, rsa, utils
from cryptography.hazmat.primitives.ciphers.aead import AESGCM


SCHEMA_VERSION = "kidsai.secrets/1"
SUPPORTED_PROFILES = ("child", "adult")


@dataclass
class PublishError(Exception):
    msg: str


def load_master_key(env_name: str = "KIDSAI_SECRETS_MASTER_KEY") -> bytes:
    """从 env 读 hex 编码的 32-byte master AES key. 未设置则生成 (dev only)."""
    raw = os.environ.get(env_name, "").strip()
    if raw:
        try:
            key = bytes.fromhex(raw)
        except ValueError as e:
            raise PublishError(f"{env_name} 不是合法 hex: {e}") from e
        if len(key) != 32:
            raise PublishError(f"{env_name} 长度必须是 32 字节 (64 hex chars)")
        return key
    # dev fallback: 生成并打印 (供本地测试)
    key = _secrets.token_bytes(32)
    print(
        f"[secrets] ⚠️  {env_name} 未设置, 生成 dev key (重启会失效):\n"
        f"  export {env_name}={key.hex()}"
    )
    return key


def load_signing_key(env_name: str = "KIDSAI_SECRETS_SIGNING_KEY_PEM") -> rsa.RSAPrivateKey:
    """从 env 读 PEM 私钥 (RSA-PSS signing). 未设置则生成 (dev only)."""
    pem = os.environ.get(env_name, "").strip()
    if pem:
        try:
            return serialization.load_pem_private_key(pem.encode(), password=None)
        except Exception as e:
            raise PublishError(f"{env_name} PEM 解析失败: {e}") from e
    # dev fallback: 生成 2048-bit RSA
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
    print(
        f"[secrets] ⚠️  {env_name} 未设置, 生成 dev keypair (重启会失效):\n"
        f"  pub (放客户端):\n{pub_pem}\n"
        f"  export {env_name}='{priv_pem}'"
    )
    return key


def collect_files(root: Path) -> list[tuple[Path, bytes]]:
    """递归读取 root 下所有文件, 返 (相对路径, bytes) 列表 (按 path 排序)."""
    if not root.is_dir():
        raise PublishError(f"profile 目录不存在: {root}")
    files: list[tuple[Path, bytes]] = []
    for p in sorted(root.rglob("*")):
        if p.is_file():
            rel = p.relative_to(root)
            files.append((rel, p.read_bytes()))
    if not files:
        raise PublishError(f"profile 目录为空: {root}")
    return files


def concat_for_bundle(files: list[tuple[Path, bytes]]) -> bytes:
    """确定性格式化: 每段 '\n---FILE:{path}---\\n{bytes}', 用于 split 回原文件."""
    parts: list[bytes] = []
    for rel, data in files:
        parts.append(f"\n---FILE:{rel.as_posix()}---\n".encode())
        parts.append(data)
    parts.append(b"\n---END---\n")
    return b"".join(parts)


def split_bundle(blob: bytes) -> dict[str, bytes]:
    """concat_for_bundle 的反向操作 — 验签/测试用."""
    out: dict[str, bytes] = {}
    chunks = blob.split(b"\n---FILE:")
    # chunks[0] 是 '' 或开头空白, 跳过
    for c in chunks[1:]:
        if not c or b"\n---END---\n" not in c + b"\n---END---\n":
            continue
        head, _, rest = c.partition(b"---\n")
        path = head.decode()
        body, _, tail = rest.partition(b"\n---END---\n")
        out[path] = body
    return out


def encrypt_bundle(plaintext: bytes, master_key: bytes) -> tuple[bytes, bytes]:
    """AES-256-GCM 加密 — 返回 (iv, ciphertext_with_tag)."""
    if len(master_key) != 32:
        raise PublishError(f"master_key 长度 {len(master_key)} ≠ 32")
    iv = _secrets.token_bytes(12)
    aes = AESGCM(master_key)
    ct = aes.encrypt(iv, plaintext, associated_data=None)
    return iv, ct


def sign_canonical(canonical_bytes: bytes, priv: rsa.RSAPrivateKey) -> str:
    """RSA-PSS-SHA256 签名, 返 base64.

    直接对 canonical_bytes 签名, 让 cryptography 内部 SHA256 + PSS 一步走完.
    不要用 utils.Prehashed — Prehashed 要求传入 32 字节的 digest, 不是任意长度原文.
    """
    sig = priv.sign(
        canonical_bytes,
        padding.PSS(
            mgf=padding.MGF1(hashes.SHA256()),
            salt_length=padding.PSS.MAX_LENGTH,
        ),
        hashes.SHA256(),
    )
    return base64.b64encode(sig).decode()


def file_sha256(data: bytes) -> str:
    h = hashes.Hash(hashes.SHA256())
    h.update(data)
    return h.finalize().hex()


def sha256_of(data: bytes) -> bytes:
    """用于构造 manifest 之前先把整个 plaintext 哈希一次 — 校验 manifest 完整性."""
    h = hashes.Hash(hashes.SHA256())
    h.update(data)
    return h.finalize()


def derive_pubkey_id(priv: rsa.RSAPrivateKey) -> str:
    """pubkey id = SHA256(SPKI DER) 前 16 hex chars."""
    pub = priv.public_key().public_bytes(
        encoding=serialization.Encoding.DER,
        format=serialization.PublicFormat.SubjectPublicKeyInfo,
    )
    return sha256_of(pub).hex()[:16]


def publish(
    profile: str,
    version: str,
    input_dir: Path,
    master_key: bytes,
    signing_key: rsa.RSAPrivateKey,
    pubkey_id: str,
    previous_version: str | None = None,
) -> dict[str, Any]:
    """发布一个 bundle — 返 manifest dict (不含 signature) + bundle bytes.

    manifest 字段不含 signature (先序列化, 再签名, 签名放 manifest["publisher_signature"]).
    这样保证「签的是 manifest 序列化结果」, 不是任意拼接.
    """
    if profile not in SUPPORTED_PROFILES:
        raise PublishError(f"profile 必须是 {SUPPORTED_PROFILES} 之一, 拿到 {profile!r}")
    if not version or len(version) < 3:
        raise PublishError("version 太短 (例: 'v1.a3f9c2')")
    if not input_dir.is_dir():
        raise PublishError(f"input_dir 不存在: {input_dir}")

    files = collect_files(input_dir)
    plaintext = concat_for_bundle(files)
    iv, ciphertext = encrypt_bundle(plaintext, master_key)

    file_entries = [
        {"path": rel.as_posix(), "sha256": file_sha256(data), "size": len(data)}
        for rel, data in files
    ]

    manifest_core = {
        "schema": SCHEMA_VERSION,
        "version": version,
        "previous_version": previous_version,
        "profile": profile,
        "created_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "publisher_pubkey_id": pubkey_id,
        "files": file_entries,
        "cipher": {
            "algo": "AES-256-GCM",
            "iv": base64.b64encode(iv).decode(),
            "plaintext_sha256": sha256_of(plaintext).hex(),
        },
        "bundle": {
            "size_bytes": len(ciphertext),
            "sha256": file_sha256(ciphertext),
        },
        "wrap": {
            "kdf": "HKDF-SHA256",
            "kdf_salt": "kidsai-secrets-v1",
            "kdf_info": "kidsai-secrets/wrap-master",
            "wrap_algo": "AES-256-GCM",
            "note": "master_key 在 fetch 时由 server 用 license_token 包裹, 客户端 KEK 解包",
        },
    }

    canonical_bytes = json.dumps(manifest_core, sort_keys=True, separators=(",", ":")).encode()
    sig_b64 = sign_canonical(canonical_bytes, signing_key)

    manifest = dict(manifest_core)
    manifest["publisher_signature"] = sig_b64

    return {
        "manifest": manifest,
        "canonical_bytes": canonical_bytes,
        "bundle_bytes": ciphertext,
        "bundle_plaintext": plaintext,
    }


def cmd_publish(args) -> int:
    """CLI entrypoint — `python -m kidsai_server.secrets_publisher publish ...`"""
    try:
        master_key = load_master_key()
        signing_key = load_signing_key()
        pubkey_id = derive_pubkey_id(signing_key)

        result = publish(
            profile=args.profile,
            version=args.version,
            input_dir=Path(args.input),
            master_key=master_key,
            signing_key=signing_key,
            pubkey_id=pubkey_id,
            previous_version=args.previous,
        )
    except PublishError as e:
        print(f"[publish] ❌ {e.msg}")
        return 2

    base = Path(args.output) if args.output else Path("./secrets_out")
    out_root = base / args.profile
    out_root.mkdir(parents=True, exist_ok=True)
    ver_dir = out_root / args.version
    ver_dir.mkdir(exist_ok=True)

    manifest_path = ver_dir / "manifest.json"
    bundle_path = ver_dir / "bundle.bin"

    manifest_path.write_text(json.dumps(result["manifest"], indent=2, ensure_ascii=False))
    bundle_path.write_bytes(result["bundle_bytes"])

    print(
        f"[publish] ✅ profile={args.profile} version={args.version}\n"
        f"  manifest: {manifest_path} ({len(result['canonical_bytes'])} bytes canonical)\n"
        f"  bundle:   {bundle_path} ({len(result['bundle_bytes'])} bytes)\n"
        f"  pubkey:   {pubkey_id}\n"
        f"  signature:{result['manifest']['publisher_signature'][:24]}..."
    )
    return 0


def cmd_verify(args) -> int:
    """CLI entrypoint — `python -m kidsai_server.secrets_publisher verify ...`
    读 manifest + bundle, 用 server 公钥验签, 解密看 plaintext 是否匹配 sha256."""
    from cryptography.hazmat.primitives.asymmetric import rsa as _rsa

    pub_pem_path = Path(args.pubkey_pem)
    if not pub_pem_path.is_file():
        print(f"[verify] ❌ pubkey 文件不存在: {pub_pem_path}")
        return 2
    pub = serialization.load_pem_public_key(pub_pem_path.read_bytes())
    if not isinstance(pub, _rsa.RSAPublicKey):
        print(f"[verify] ❌ 不是 RSA 公钥")
        return 2

    manifest = json.loads(Path(args.manifest).read_text())
    bundle = Path(args.bundle).read_bytes()
    expected_pubkey_id = manifest.get("publisher_pubkey_id", "")

    # 验签: 把不含 signature 字段的 manifest 序列化 → 比对
    manifest_no_sig = {k: v for k, v in manifest.items() if k != "publisher_signature"}
    canonical = json.dumps(manifest_no_sig, sort_keys=True, separators=(",", ":")).encode()
    sig_b64 = manifest["publisher_signature"]
    sig = base64.b64decode(sig_b64)
    try:
        pub.verify(
            sig,
            canonical,
            padding.PSS(
                mgf=padding.MGF1(hashes.SHA256()),
                salt_length=padding.PSS.MAX_LENGTH,
            ),
            hashes.SHA256(),
        )
        print(f"[verify] ✅ signature valid (pubkey {expected_pubkey_id})")
    except Exception as e:
        print(f"[verify] ❌ signature invalid: {e}")
        return 3

    # 解密 (master_key 来自 env; 这里只验证格式)
    master_key = load_master_key()
    iv = base64.b64decode(manifest["cipher"]["iv"])
    aes = AESGCM(master_key)
    try:
        plaintext = aes.decrypt(iv, bundle, associated_data=None)
    except Exception as e:
        print(f"[verify] ❌ decrypt failed: {e}")
        return 4

    actual_sha = sha256_of(plaintext).hex()
    expected_sha = manifest["cipher"]["plaintext_sha256"]
    if actual_sha != expected_sha:
        print(f"[verify] ❌ plaintext sha256 mismatch:\n  got:      {actual_sha}\n  expected: {expected_sha}")
        return 5

    files = split_bundle(plaintext)
    print(f"[verify] ✅ decrypt ok, {len(files)} files in bundle")
    for path, data in sorted(files.items()):
        print(f"  - {path} ({len(data)} bytes)")
    return 0


def main() -> int:
    import argparse

    p = argparse.ArgumentParser(prog="secrets_publisher", description="W11 secrets publisher CLI")
    sub = p.add_subparsers(dest="cmd", required=True)

    pub_p = sub.add_parser("publish", help="publish a new bundle")
    pub_p.add_argument("--profile", choices=list(SUPPORTED_PROFILES), required=True)
    pub_p.add_argument("--version", required=True, help="例: v1.a3f9c2")
    pub_p.add_argument("--input", required=True, help="源目录 (含 YAML/JSON)")
    pub_p.add_argument("--output", default=None)
    pub_p.add_argument("--previous", default=None, help="上一个版本号 (可选)")
    pub_p.set_defaults(func=cmd_publish)

    ver_p = sub.add_parser("verify", help="verify a published bundle")
    ver_p.add_argument("--manifest", required=True)
    ver_p.add_argument("--bundle", required=True)
    ver_p.add_argument("--pubkey-pem", required=True)
    ver_p.set_defaults(func=cmd_verify)

    args = p.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())