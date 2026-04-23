#!/usr/bin/env python3
"""Bong 图像生成统一入口。

两个 backend：
- **cliproxy**（默认） —— 走自建 CLIProxyAPI `/v1/responses` + image_generation 工具（SSE）。
- **openai** —— 直连 OpenAI `/v1/images/generations`（`gpt-image-1.5`）。

默认 `--backend auto` 优先 cliproxy；网络错误 / 空返回时自动 fallback 到 openai
（需要 `.env` 里有 `OPENAI_API_KEY`）。强制某一端用 `--backend cliproxy|openai`。

用法（最常用）：

    # 直接 prompt（物品画风）
    python scripts/images/gen.py "a cracked iron sword with glowing runes" --name iron_sword

    # 从已有的 *_prompt.md 读取
    python scripts/images/gen.py --prompt-file local_images/毒蛊飞针_prompt.md

    # 粒子（透明底 + 黑底 prefix，走 particle 画风）
    python scripts/images/gen.py "a horizontal streak of sword qi" \\
        --name sword_qi_trail --style particle --transparent --out local_images/particles/

    # HUD overlay（水墨边框这类，透明 RGBA）
    python scripts/images/gen.py "four-corner sumi-e ink splashes, center transparent" \\
        --name ink_wash_vignette --style hud --transparent --size 1536x1024

环境（`scripts/images/.env`，拷贝 `.env.example` 起步）：
    CLIPROXY_API_KEY    默认 "kiz"
    CLIPROXY_BASE_URL   默认 https://cliproxy.kizunadesu.cc
    CLIPROXY_MODEL      默认 gpt-image-1536x1024
    OPENAI_API_KEY      openai 直连 / fallback 必需
    OPENAI_BASE_URL     默认 https://api.openai.com/v1
    OPENAI_ORG_ID       可选
    OPENAI_PROJECT_ID   可选
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))
import style as style_mod  # noqa: E402

# CLI Proxy 默认
CLIPROXY_DEFAULT_BASE = "https://cliproxy.kizunadesu.cc"
CLIPROXY_DEFAULT_KEY = "kiz"
CLIPROXY_DEFAULT_MODEL = "gpt-image-1536x1024"
# urllib 默认 UA 会被 Cloudflare 1010 拦，必须伪装
CLIPROXY_FAKE_UA = "curl/8.5.0"

# OpenAI 直连默认
OPENAI_DEFAULT_BASE = "https://api.openai.com/v1"
OPENAI_MODEL = "gpt-image-1.5"


# -------- env helper ---------------------------------------------------------


def load_env(env_path: Path) -> dict[str, str]:
    """极简 .env 解析：KEY=VALUE，忽略注释和空行，支持可选引号。"""
    if not env_path.exists():
        return {}
    out: dict[str, str] = {}
    for line in env_path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        out[k.strip()] = v.strip().strip('"').strip("'")
    return out


def _env(env: dict[str, str], key: str, default: str | None = None) -> str | None:
    return env.get(key) or os.environ.get(key) or default


# -------- prompt helper ------------------------------------------------------


def read_prompt_file(path: Path, style: str, transparent: bool = False) -> str:
    """读 *_prompt.md，按 style 拼 prefix。"""
    body = path.read_text(encoding="utf-8").strip()
    return style_mod.apply(style, body, transparent=transparent)


def derive_name_from_prompt_file(path: Path) -> str:
    stem = path.stem
    return stem[: -len("_prompt")] if stem.endswith("_prompt") else stem


# -------- backend: CLI Proxy (SSE) -------------------------------------------


def _extract_from_call(call: dict, seen: set[str]) -> bytes | None:
    if not call or call.get("type") != "image_generation_call":
        return None
    result = call.get("result")
    if not result:
        return None
    call_id = call.get("id") or f"anon:{len(seen)}"
    if call_id in seen:
        return None
    seen.add(call_id)
    return base64.b64decode(result)


def generate_cliproxy(
    prompt: str,
    api_key: str,
    base_url: str,
    model: str,
    size: str,
    quality: str,
    output_format: str,
    background: str,
    n: int,
    moderation: str,
    verbose: bool,
) -> list[bytes]:
    tool: dict = {
        "type": "image_generation",
        "output_format": output_format,
        "quality": quality,
        "size": size,
        "background": background,
        "moderation": moderation,
    }
    if n and n > 1:
        tool["n"] = n

    payload = {
        "model": model,
        "instructions": (
            "You are an image generation assistant. When the user describes an "
            "image, call the image_generation tool exactly once."
        ),
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": prompt}],
            }
        ],
        "tools": [tool],
        "tool_choice": {"type": "image_generation"},
        "parallel_tool_calls": False,
        "reasoning": {"effort": "low", "summary": "auto"},
        "stream": True,
        "store": False,
    }
    body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        f"{base_url}/v1/responses",
        data=body,
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
            "Accept": "text/event-stream",
            "User-Agent": CLIPROXY_FAKE_UA,
        },
        method="POST",
    )

    images: list[bytes] = []
    seen: set[str] = set()

    def handle(ev_type: str, data: str) -> None:
        data = data.strip()
        if not data or data == "[DONE]":
            return
        try:
            obj = json.loads(data)
        except json.JSONDecodeError:
            return
        if verbose and ev_type and not ev_type.startswith("response.reasoning"):
            print(f"  [ev] {ev_type}", file=sys.stderr)
        if ev_type == "response.output_item.done":
            blob = _extract_from_call(obj.get("item") or {}, seen)
            if blob:
                images.append(blob)
        elif ev_type == "response.completed":
            out = obj.get("response", {}).get("output", []) or []
            for item in out:
                blob = _extract_from_call(item, seen)
                if blob:
                    images.append(blob)
            usage = obj.get("response", {}).get("usage", {}) or {}
            if usage:
                print(
                    f"  usage: input={usage.get('input_tokens')} "
                    f"output={usage.get('output_tokens')} "
                    f"total={usage.get('total_tokens')}",
                    file=sys.stderr,
                )
        elif ev_type == "error":
            print(f"  [stream error] {data[:500]}", file=sys.stderr)

    with urllib.request.urlopen(req, timeout=600) as resp:
        if resp.status != 200:
            raise urllib.error.URLError(f"cliproxy status {resp.status}")
        ev_type: str | None = None
        data_lines: list[str] = []
        for raw in resp:
            line = raw.decode("utf-8", errors="replace").rstrip("\r\n")
            if line.startswith("event:"):
                if ev_type or data_lines:
                    handle(ev_type or "", "\n".join(data_lines))
                    data_lines = []
                ev_type = line[6:].strip()
            elif line.startswith("data:"):
                data_lines.append(line[5:].lstrip())
            elif line == "":
                handle(ev_type or "", "\n".join(data_lines))
                ev_type = None
                data_lines = []
        if ev_type or data_lines:
            handle(ev_type or "", "\n".join(data_lines))

    return images


# -------- backend: OpenAI direct ---------------------------------------------


def generate_openai(
    prompt: str,
    api_key: str,
    base_url: str,
    size: str,
    quality: str,
    output_format: str,
    background: str,
    n: int,
    moderation: str,
    org_id: str | None,
    project_id: str | None,
) -> list[bytes]:
    payload = {
        "model": OPENAI_MODEL,
        "prompt": prompt,
        "n": n,
        "size": size,
        "quality": quality,
        "output_format": output_format,
        "background": background,
        "moderation": moderation,
    }
    body = json.dumps(payload).encode("utf-8")
    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {api_key}",
    }
    if org_id:
        headers["OpenAI-Organization"] = org_id
    if project_id:
        headers["OpenAI-Project"] = project_id

    req = urllib.request.Request(
        f"{base_url}/images/generations",
        data=body,
        headers=headers,
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=300) as resp:
        if resp.status != 200:
            raise urllib.error.URLError(f"openai status {resp.status}")
        data = json.loads(resp.read())

    images: list[bytes] = []
    for item in data.get("data", []):
        b64 = item.get("b64_json")
        if not b64:
            continue
        images.append(base64.b64decode(b64))

    usage = data.get("usage", {})
    if usage:
        print(
            f"  usage: input={usage.get('input_tokens')} "
            f"output={usage.get('output_tokens')} "
            f"total={usage.get('total_tokens')}",
            file=sys.stderr,
        )
    return images


# -------- dispatch -----------------------------------------------------------


def dispatch(
    backend: str,
    prompt: str,
    env: dict[str, str],
    size: str,
    quality: str,
    output_format: str,
    background: str,
    n: int,
    moderation: str,
    verbose: bool,
) -> list[bytes]:
    """auto → cliproxy（有 openai key 则允许 fallback）；强制用指定 backend。"""
    has_openai = bool(_env(env, "OPENAI_API_KEY"))

    def _try_cliproxy() -> list[bytes]:
        key = _env(env, "CLIPROXY_API_KEY", CLIPROXY_DEFAULT_KEY)
        base = (_env(env, "CLIPROXY_BASE_URL", CLIPROXY_DEFAULT_BASE) or "").rstrip("/")
        model = _env(env, "CLIPROXY_MODEL", CLIPROXY_DEFAULT_MODEL)
        print(
            f"[cliproxy] model={model} size={size} quality={quality} "
            f"bg={background} fmt={output_format} n={n}",
            file=sys.stderr,
        )
        return generate_cliproxy(
            prompt, key or "", base, model or "", size, quality,
            output_format, background, n, moderation, verbose,
        )

    def _try_openai() -> list[bytes]:
        key = _env(env, "OPENAI_API_KEY")
        if not key:
            raise RuntimeError("OPENAI_API_KEY 未配置，无法走 openai backend")
        base = (_env(env, "OPENAI_BASE_URL", OPENAI_DEFAULT_BASE) or "").rstrip("/")
        org = _env(env, "OPENAI_ORG_ID")
        proj = _env(env, "OPENAI_PROJECT_ID")
        print(
            f"[openai]   model={OPENAI_MODEL} size={size} quality={quality} "
            f"bg={background} fmt={output_format} n={n}",
            file=sys.stderr,
        )
        return generate_openai(
            prompt, key, base, size, quality, output_format, background,
            n, moderation, org, proj,
        )

    if backend == "openai":
        return _try_openai()
    if backend == "cliproxy":
        return _try_cliproxy()

    # auto
    try:
        images = _try_cliproxy()
        if images:
            return images
        if has_openai:
            print("[auto] cliproxy 返回 0 张，fallback → openai", file=sys.stderr)
            return _try_openai()
        return images
    except (urllib.error.URLError, RuntimeError) as e:
        if has_openai:
            print(f"[auto] cliproxy 失败 ({e})，fallback → openai", file=sys.stderr)
            return _try_openai()
        raise


# -------- save / main --------------------------------------------------------


def save_images(images: list[bytes], out_dir: Path, name: str, ext: str) -> list[Path]:
    out_dir.mkdir(parents=True, exist_ok=True)
    paths: list[Path] = []
    if len(images) == 1:
        p = out_dir / f"{name}.{ext}"
        p.write_bytes(images[0])
        paths.append(p)
    else:
        for i, blob in enumerate(images, 1):
            p = out_dir / f"{name}_{i}.{ext}"
            p.write_bytes(blob)
            paths.append(p)
    return paths


def main() -> None:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("prompt", nargs="?", help="prompt 文本（与 --prompt-file 二选一）")
    ap.add_argument("--prompt-file", type=Path, help="从 *_prompt.md 读取 prompt")
    ap.add_argument("--name", help="输出文件名主干（默认从 --prompt-file 派生）")
    ap.add_argument(
        "--out",
        type=Path,
        default=Path("local_images"),
        help="输出目录（默认 local_images/）",
    )
    ap.add_argument(
        "--style",
        default="item",
        choices=style_mod.available_styles() + ["none"],
        help="画风 prefix 档位（item=物品 / particle=VFX 黑底 / hud=透明 overlay / none=不拼）",
    )
    ap.add_argument(
        "--backend",
        default="auto",
        choices=["auto", "cliproxy", "openai"],
        help="后端选择：auto=cliproxy 优先失败 fallback openai",
    )
    ap.add_argument(
        "--size",
        default="1024x1024",
        choices=["auto", "1024x1024", "1536x1024", "1024x1536"],
    )
    ap.add_argument(
        "--quality",
        default="high",
        choices=["auto", "low", "medium", "high"],
    )
    ap.add_argument(
        "--format",
        dest="output_format",
        default="png",
        choices=["png", "jpeg", "webp"],
    )
    ap.add_argument(
        "--transparent",
        action="store_true",
        help="透明背景（粒子/HUD 必开；jpeg 下无效会强制切 png）",
    )
    ap.add_argument("--n", type=int, default=1, help="生成张数")
    ap.add_argument(
        "--moderation", default="auto", choices=["auto", "low"],
    )
    ap.add_argument(
        "--save-prompt",
        action="store_true",
        help="同时把最终 prompt 写到 <out>/<name>_prompt.md",
    )
    ap.add_argument("-v", "--verbose", action="store_true", help="打印 SSE 事件")
    args = ap.parse_args()

    if not args.prompt and not args.prompt_file:
        ap.error("必须提供 prompt 或 --prompt-file")

    style = args.style
    if args.prompt_file:
        prompt = read_prompt_file(args.prompt_file, style, transparent=args.transparent)
        name = args.name or derive_name_from_prompt_file(args.prompt_file)
    else:
        prompt = (
            style_mod.apply(style, args.prompt, transparent=args.transparent)
            if style != "none"
            else args.prompt
        )
        if not args.name:
            ap.error("直接给 prompt 时必须指定 --name")
        name = args.name

    background = "transparent" if args.transparent else "auto"
    if args.transparent and args.output_format == "jpeg":
        print("警告: --transparent 在 jpeg 下无效，切 png", file=sys.stderr)
        args.output_format = "png"

    env = load_env(SCRIPT_DIR / ".env")

    print(
        f"style={style} backend={args.backend} out={args.out} name={name}",
        file=sys.stderr,
    )
    print(
        f"prompt ({len(prompt)} chars): "
        f"{prompt[:140]}{'...' if len(prompt) > 140 else ''}",
        file=sys.stderr,
    )

    try:
        images = dispatch(
            backend=args.backend,
            prompt=prompt,
            env=env,
            size=args.size,
            quality=args.quality,
            output_format=args.output_format,
            background=background,
            n=args.n,
            moderation=args.moderation,
            verbose=args.verbose,
        )
    except urllib.error.HTTPError as e:
        err_body = e.read().decode("utf-8", errors="replace")
        print(f"HTTP {e.code}: {err_body[:2000]}", file=sys.stderr)
        sys.exit(2)
    except urllib.error.URLError as e:
        print(f"网络错误: {e.reason}", file=sys.stderr)
        sys.exit(2)
    except RuntimeError as e:
        print(f"错误: {e}", file=sys.stderr)
        sys.exit(2)

    if not images:
        print("未获得任何图片", file=sys.stderr)
        sys.exit(3)

    paths = save_images(images, args.out, name, args.output_format)
    for p in paths:
        print(f"  保存 → {p}")

    if args.save_prompt:
        prompt_file = args.out / f"{name}_prompt.md"
        prompt_file.write_text(prompt + "\n", encoding="utf-8")
        print(f"  prompt → {prompt_file}")


if __name__ == "__main__":
    main()
