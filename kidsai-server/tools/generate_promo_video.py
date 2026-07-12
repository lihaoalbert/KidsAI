"""W7: 宣传视频生成脚本 (single-concepts → mini-movie).

设计:
- 写死 8 个镜 (1 条 30s 视频), 主角 xiaoxing, Pixar 风格
- 每镜: image-01 still → hailuo-02 image-to-video (5s motion)
- ffmpeg 拼镜 + 配 BGM (music-01)

用法:
    python tools/generate_promo_video.py --stage stills   # 只生成 still images (¥4)
    python tools/generate_promo_video.py --stage videos   # 从已有 stills 生成视频 (¥16)
    python tools/generate_promo_video.py --stage all      # 全跑 (¥22)
    python tools/generate_promo_video.py --stage bgm      # 只生成 BGM (¥2)
    python tools/generate_promo_video.py --stage stitch   # 拼镜 + BGM (无 API)
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import requests


# ─── 路径 / 常量 ───────────────────────────────────────────
THIS_DIR = Path(__file__).resolve().parent
ROOT_DIR = THIS_DIR.parent.parent
PROMO_DIR = ROOT_DIR / "promo" / "A_5min_movie"

MINIMAX_BASE = "https://api.minimaxi.com/v1"
IMAGE_URL = f"{MINIMAX_BASE}/image_generation"
VIDEO_URL = f"{MINIMAX_BASE}/video_generation"
VIDEO_QUERY_URL = f"{MINIMAX_BASE}/query/video_generation"
MUSIC_URL = f"{MINIMAX_BASE}/music_generation"
MUSIC_QUERY_URL = f"{MINIMAX_BASE}/query/music_generation"

IMAGE_TIMEOUT = 60
VIDEO_TIMEOUT = 30
VIDEO_POLL_INTERVAL = 3
VIDEO_POLL_MAX = 90  # ~4.5 min cap
MUSIC_POLL_INTERVAL = 5
MUSIC_POLL_MAX = 60


# ─── 5 个镜 (8 合并) ────────────────────────────────────────
# Token Plan 限制: 每轮 3 视频, 15 分钟重置.
# 当前已生成 3 个视频 (01_opening / 02_idea / 03_character), 等 15min 跑 2 个新的.
# 合并策略:
#   04_stage3_style + 05_stage4_storyboard → 04_production (storyboard still + 风格→分镜→预览 motion)
#   06_stage5_preview + 07_stage6_finalize + 08_closing → 05_finalize (closing still + 定稿→庆祝→家长 motion)
# 主角: xiaoxing = 10 岁男孩, 黑框眼镜 + 蓝色卫衣 + 大眼睛
# 风格: Pixar 3D render — 圆润, 暖光, 大眼睛, 电影感构图
# image_prompt: image-01 still (16:9), 用于动画 first frame
# motion_prompt: hailuo-02 5s 视频 motion 描述 (镜头怎么动 + 角色怎么动)

SHOTS: list[dict[str, Any]] = [
    {
        "id": "01_opening",
        "image_prompt": (
            "Pixar 3D render style, a friendly 10 year old boy with black-framed glasses "
            "and blue hoodie sitting at a glowing computer in a cozy bedroom at sunrise, "
            "warm golden light streaming through curtains, the screen shows a glowing "
            "magical 'KidsAI' logo, big expressive eyes full of wonder, soft rounded shapes, "
            "cinematic composition, 16:9"
        ),
        "motion_prompt": (
            "Slow camera push in towards the glowing computer screen, the boy's eyes "
            "widen with wonder as the screen light brightens"
        ),
    },
    {
        "id": "02_idea",
        "image_prompt": (
            "Pixar 3D render style, the 10 year old boy with glasses typing on a backlit "
            "keyboard, the computer screen shows floating colorful story idea bubbles "
            "with text '太空冒险 / Space Adventure' and '海底世界 / Ocean World', warm "
            "ambient lighting, cozy bedroom, the boy smiles excitedly, cinematic, 16:9"
        ),
        "motion_prompt": (
            "The boy types thoughtfully, story idea bubbles float up from the screen "
            "and gently rotate in the air around his head"
        ),
    },
    {
        "id": "03_character",
        "image_prompt": (
            "Pixar 3D render style, the 10 year old boy looking at a holographic character "
            "selection panel showing the same boy avatar with different poses (standing, "
            "running, smiling), magical sparkles around the panel, soft teal glow, "
            "cozy room background, big eyes, cinematic, 16:9"
        ),
        "motion_prompt": (
            "The holographic character avatar spins and changes poses while the boy "
            "looks on amazed, sparkles drift around the panel"
        ),
    },
    {
        "id": "04_production",
        "image_prompt": (
            "Pixar 3D render style, the 10 year old boy looking at a cinematic "
            "storyboard with 4 panels on a glowing screen showing a space adventure "
            "sequence (wide shot of rocket, close-up of astronaut, dialogue scene, "
            "triumphant landing), magical blue glow, cozy room, 16:9"
        ),
        "motion_prompt": (
            "The 4 storyboard panels light up one by one from left to right, "
            "transitioning into a glowing video preview that plays across them, "
            "the boy watches with growing excitement"
        ),
    },
    {
        "id": "05_finalize",
        "image_prompt": (
            "Pixar 3D render style, the 10 year old boy proudly showing a finished "
            "video on his tablet to a smiling parent sitting beside him, both bathed "
            "in warm screen glow, the video shows a tiny space adventure on the "
            "tablet screen, cozy evening room, heartwarming, 16:9"
        ),
        "motion_prompt": (
            "The boy presses an unseen finalize button, magical sparkles burst, "
            "the tablet flashes with success, then he turns it toward the parent, "
            "the parent nods and smiles warmly, the boy beams with pride"
        ),
    },
]


# ─── helpers ───────────────────────────────────────────────
def load_key() -> str:
    raw = os.getenv("MINIMAX_API_KEYS", "").strip()
    if raw:
        return raw.split(",")[0].strip()
    legacy = os.getenv("MINIMAX_API_KEY", "").strip()
    if not legacy:
        sys.exit("MINIMAX_API_KEY(S) 未设置")
    return legacy


def call_image_gen(api_key: str, prompt: str, aspect: str) -> str:
    body = {
        "model": "image-01",
        "prompt": prompt,
        "aspect_ratio": aspect,
        "response_format": "url",
        "n": 1,
    }
    headers = {"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"}
    r = requests.post(IMAGE_URL, json=body, headers=headers, timeout=IMAGE_TIMEOUT)
    r.raise_for_status()
    data = r.json()
    base = data.get("base_resp") or {}
    if base.get("status_code", 0) not in (0, 1000, "0", "1000"):
        raise RuntimeError(f"minimax image err: {base}")
    urls = data.get("data", {}).get("image_urls") or []
    if not urls:
        raise RuntimeError(f"no url: {data}")
    return urls[0]


def call_hailuo_create(api_key: str, prompt: str, image_url: str | None, duration: int = 6) -> str:
    body: dict[str, Any] = {
        "model": "MiniMax-hailuo-02",
        "prompt": prompt,
        "duration": duration,
        "ratio": "16:9",
    }
    if image_url:
        body["image_url"] = image_url
    headers = {"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"}
    r = requests.post(VIDEO_URL, json=body, headers=headers, timeout=VIDEO_TIMEOUT)
    r.raise_for_status()
    data = r.json()
    task_id = data.get("task_id")
    if not task_id:
        raise RuntimeError(f"no task_id: {data}")
    return task_id


def poll_hailuo(api_key: str, task_id: str) -> str:
    """轮询 hailuo task 直到 Success, 返视频 download_url.

    MiniMax 现状: poll 返 Status=Success + file_id (无 download_url),
    需要再调 /v1/files/retrieve?file_id=... 拿 download_url.
    """
    poll_url = f"{VIDEO_QUERY_URL}?task_id={task_id}"
    headers = {"Authorization": f"Bearer {api_key}"}
    for attempt in range(VIDEO_POLL_MAX):
        time.sleep(VIDEO_POLL_INTERVAL)
        r = requests.get(poll_url, headers=headers, timeout=VIDEO_TIMEOUT)
        r.raise_for_status()
        data = r.json()
        status = data.get("status", "")
        if status in ("Success", "Succeeded", "succeeded"):
            file_id = data.get("file_id")
            if not file_id:
                raise RuntimeError(f"success but no file_id: {data}")
            # 第二步: 取 download_url
            r2 = requests.get(
                f"{MINIMAX_BASE}/files/retrieve",
                params={"file_id": file_id},
                headers=headers,
                timeout=30,
            )
            r2.raise_for_status()
            file_data = r2.json()
            url = file_data.get("file", {}).get("download_url")
            if not url:
                raise RuntimeError(f"file retrieve but no download_url: {file_data}")
            return url
        if status in ("Fail", "Failed", "failed"):
            raise RuntimeError(f"hailuo failed: {data}")
    raise RuntimeError(f"hailuo timeout after {VIDEO_POLL_MAX} polls")


def call_music_create(api_key: str, prompt: str, duration: int = 30) -> bytes:
    """调 MiniMax Music 2.6 (同步, 返 hex-encoded mp3).

    跟 music-01 不一样: 必须传 lyrics 字段 (instrumental 用
    "[Instrumental]" 占位), response.data.audio 是 hex 字符串, 直接解码写文件.
    """
    body = {
        "model": "music-2.6",
        "prompt": prompt,
        "lyrics": "[Instrumental]",
        "audio_setting": {"sample_rate": 44100, "bitrate": 256000, "format": "mp3"},
    }
    headers = {"Authorization": f"Bearer {api_key}", "Content-Type": "application/json"}
    r = requests.post(MUSIC_URL, json=body, headers=headers, timeout=120)
    r.raise_for_status()
    data = r.json()
    base = data.get("base_resp") or {}
    if base.get("status_code", 0) not in (0, "0"):
        raise RuntimeError(f"music err: {base}")
    audio_hex = data.get("data", {}).get("audio", "")
    if not audio_hex:
        raise RuntimeError(f"no audio in response: {data}")
    import binascii
    return binascii.unhexlify(audio_hex)


def download(url: str, dest: Path) -> None:
    with requests.get(url, stream=True, timeout=60) as r:
        r.raise_for_status()
        dest.parent.mkdir(parents=True, exist_ok=True)
        with dest.open("wb") as f:
            for chunk in r.iter_content(8192):
                if chunk:
                    f.write(chunk)


# ─── stages ────────────────────────────────────────────────
def stage_stills(api_key: str) -> list[Path]:
    print(f"=== stage_stills: {len(SHOTS)} images ===")
    out: list[Path] = []
    for shot in SHOTS:
        dest = PROMO_DIR / "stills" / f"{shot['id']}.png"
        if dest.exists():
            print(f"  [skip] {dest.name} (exists)")
            out.append(dest)
            continue
        print(f"  [gen]  {shot['id']}.png ...", end=" ", flush=True)
        try:
            url = call_image_gen(api_key, shot["image_prompt"], "16:9")
            download(url, dest)
            print("ok")
        except Exception as e:
            print(f"FAIL: {e}")
            continue
        out.append(dest)
    return out


def stage_videos(api_key: str) -> list[Path]:
    print(f"=== stage_videos: {len(SHOTS)} videos ===")
    out: list[Path] = []
    for shot in SHOTS:
        still = PROMO_DIR / "stills" / f"{shot['id']}.png"
        video = PROMO_DIR / "clips" / f"{shot['id']}.mp4"
        if video.exists():
            print(f"  [skip] {video.name} (exists)")
            out.append(video)
            continue
        if not still.exists():
            print(f"  [skip] {shot['id']}: no still, run --stage stills first")
            continue
        print(f"  [gen]  {shot['id']}.mp4 ...", end=" ", flush=True)
        try:
            # 上传 still 到 MiniMax 拿 url (image-01 是 url, 我们 cache 在 .still_url 里)
            still_url_file = PROMO_DIR / "stills" / f"{shot['id']}.url"
            if still_url_file.exists():
                image_url = still_url_file.read_text().strip()
            else:
                # 重传 still 到 MiniMax (image-01 response_format=url)
                # 但 stills 是下载的本地 png, 我们需要重新调 image_gen 拿 url
                # 简化: 用 image_gen 重新跑一遍拿 url (浪费一次 API, 但简单)
                # 优化: 第一次跑 stills 时把 url 存下来 — 见 stage_stills_v2
                print(f"\n    re-uploading still for {shot['id']} ...", end=" ", flush=True)
                image_url = call_image_gen(api_key, shot["image_prompt"], "16:9")
                still_url_file.write_text(image_url)
            task_id = call_hailuo_create(api_key, shot["motion_prompt"], image_url)
            download_url = poll_hailuo(api_key, task_id)
            download(download_url, video)
            print("ok")
        except Exception as e:
            print(f"FAIL: {e}")
            continue
        out.append(video)
    return out


def stage_stills_v2(api_key: str) -> list[Path]:
    """改进版 stills stage: 同时保存 image url 给后续 videos 用."""
    print(f"=== stage_stills (v2 with url cache): {len(SHOTS)} images ===")
    out: list[Path] = []
    for shot in SHOTS:
        dest = PROMO_DIR / "stills" / f"{shot['id']}.png"
        url_file = PROMO_DIR / "stills" / f"{shot['id']}.url"
        if dest.exists() and url_file.exists():
            print(f"  [skip] {dest.name} (exists)")
            out.append(dest)
            continue
        print(f"  [gen]  {shot['id']}.png ...", end=" ", flush=True)
        try:
            url = call_image_gen(api_key, shot["image_prompt"], "16:9")
            download(url, dest)
            url_file.write_text(url)
            print("ok")
        except Exception as e:
            print(f"FAIL: {e}")
            continue
        out.append(dest)
    return out


def stage_bgm(api_key: str) -> Path | None:
    dest = PROMO_DIR / "bgm.mp3"
    if dest.exists():
        print(f"  [skip] bgm.mp3 (exists)")
        return dest
    prompt = (
        "Pixar-style orchestral background music for a kids product promo video, "
        "30 seconds, warm playful melody with pizzicato strings, gentle woodwind, "
        "soft piano accents, uplifting and heartwarming, cinematic feel"
    )
    print(f"  [gen]  bgm.mp3 ...", end=" ", flush=True)
    try:
        mp3_bytes = call_music_create(api_key, prompt, duration=30)
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(mp3_bytes)
        print(f"ok ({len(mp3_bytes)//1024} KB)")
        return dest
    except Exception as e:
        print(f"FAIL: {e}")
        return None


def stage_stitch() -> Path:
    """ffmpeg 拼 clips + bgm, 输出 final.mp4 (30s 目标时长)."""
    print("=== stage_stitch ===")
    clips_dir = PROMO_DIR / "clips"
    bgm = PROMO_DIR / "bgm.mp3"
    final = PROMO_DIR / "final.mp4"

    clips = sorted(clips_dir.glob("*.mp4"))
    if not clips:
        sys.exit("no clips, run --stage videos first")

    # 拼镜 (concat), 每镜固定 5s (hailuo 给 6s, ffmpeg -t 5 截断)
    concat_list = PROMO_DIR / "concat.txt"
    concat_list.write_text("\n".join(f"file '{c}'" for c in clips))

    # 第一步: 拼镜
    merged = PROMO_DIR / "merged.mp4"
    subprocess.run([
        "ffmpeg", "-y", "-f", "concat", "-safe", "0", "-i", str(concat_list),
        "-c", "copy", str(merged),
    ], check=True, capture_output=True)

    # 第二步: 截到 30s + 加 BGM.
    # BGM 可能是 17s (music-2.6 默认), 必须 -stream_loop -1 循环填满 30s,
    # 否则 -shortest 会把视频切到 BGM 长度, 留下 13s 静音黑屏.
    if bgm.exists():
        bgm_args = ["-stream_loop", "-1", "-i", str(bgm)]
    else:
        bgm_args = []
    subprocess.run([
        "ffmpeg", "-y", "-i", str(merged), *bgm_args,
        "-t", "30",  # 30s 上限
        "-c:v", "libx264", "-preset", "fast", "-crf", "23",
        "-c:a", "aac", "-b:a", "128k",
        "-shortest",
        str(final),
    ], check=True, capture_output=True)
    print(f"  → {final}")
    return final


# ─── main ──────────────────────────────────────────────────
def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--stage", choices=["stills", "videos", "bgm", "stitch", "all"],
                   default="all")
    p.add_argument("--limit", type=int, default=None,
                   help="只跑前 N 个 shot (调试用)")
    args = p.parse_args()

    if args.limit:
        global SHOTS
        SHOTS = SHOTS[: args.limit]

    PROMO_DIR.mkdir(parents=True, exist_ok=True)

    if args.stage in ("stills", "all"):
        api_key = load_key()
        stage_stills_v2(api_key)

    if args.stage in ("videos", "all"):
        api_key = load_key()
        stage_videos(api_key)

    if args.stage in ("bgm", "all"):
        api_key = load_key()
        stage_bgm(api_key)

    if args.stage == "stitch":
        stage_stitch()
    elif args.stage == "all":
        if (PROMO_DIR / "clips").exists() and list((PROMO_DIR / "clips").glob("*.mp4")):
            stage_stitch()

    return 0


if __name__ == "__main__":
    sys.exit(main())