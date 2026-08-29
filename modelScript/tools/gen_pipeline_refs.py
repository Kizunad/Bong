#!/usr/bin/env python3
"""Bong 概念设计与 3D 资产参考图生成流水线工具 (Art Concept & Reference Pipeline)。

分步式可控流水线：
- 步骤 1: 概念图 (concept) -> 文生图
- 步骤 2: 物品图标 (icon) -> 图生图 (需要参考图)
- 步骤 3: MC 正交三视图 (three_view) -> 图生图 (需要参考图，物品类型会自动附带灰色无面模特玩家)
- 步骤 4: MC 爆炸分解图 (exploded) -> 图生图 (需要参考图，通常以三视图或概念图为基准)

支持单独跑某一步（--step），跑完后在会话中通过 read 工具向用户展示图片，经用户确认后再进行下一步。
"""

from __future__ import annotations

import argparse
import base64
import io
import os
import sys
import time
from pathlib import Path
from typing import Literal

import requests
from PIL import Image

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_ENV_PATH = REPO_ROOT / "scripts" / "images" / ".env"
DEFAULT_OUTPUT_DIR = REPO_ROOT / "modelScript" / "assets" / "refs"

# 末法残土修仙世界观底色
XIANXIA_STYLE_BASE = (
    "concept art for a dharma-ending xianxia wasteland (末法残土), "
    "dark cultivation aesthetic, Sekiro / Beksiński visual tone, "
    "desaturated ash-grey, bone-white, weathered bronze and rust, "
    "high contrast, dramatic lighting, grim realistic texture"
)

# 各阶段 Prompt 模板
PROMPT_TEMPLATES = {
    "concept": (
        "{prompt}, {base_style}, high aesthetic value, detailed cinematic concept art"
    ),
    "icon": (
        "game item icon of the referenced subject, pure solid black background (#000000), "
        "dramatic rim light, centered photorealistic 3D item render, high contrast, clean silhouette, no watermark"
    ),
    "three_view_item": (
        "Minecraft voxel style orthographic three-view reference sheet of the referenced item, "
        "equipped on a plain neutral matte grey featureless mannequin player biped model (纯灰色模特玩家), "
        "displaying Front view, Side view, and Back view side by side, "
        "clean Minecraft cuboid blocky aesthetic, distinct voxel armor and weapon parts, "
        "plain solid neutral background, clear proportions for 3D modeling reference"
    ),
    "three_view_creature": (
        "Minecraft voxel style orthographic three-view reference sheet of the referenced creature, "
        "displaying Front view, Side view, and Back view side by side, "
        "clean Minecraft cuboid blocky aesthetic, textured voxel geometry, "
        "plain solid neutral background, clear biped/quadruped proportions for 3D modeling reference"
    ),
    "exploded": (
        "Minecraft voxel style technical exploded breakdown schematic diagram (爆炸分解图) of the referenced asset, "
        "disassembled components floating with clear structural separation, showing individual 3D voxel armor plates, straps, cords, core bones and inner cloth linings, "
        "technical blueprint aesthetics with callout lines and clean explanatory text annotations, schematic draft paper background"
    ),
}


class TKImageClient:
    """TokensKingdom 极简生图客户端 (文生图 + 图生图)"""

    def __init__(
        self,
        api_key: str | None = None,
        base_url: str | None = None,
        model: str = "gpt-image-2",
    ):
        self.base_url = (base_url or self._load_env_val("TK_IMAGE_BASE_URL") or "https://image.tokenskingdom.com").rstrip("/")
        self.api_key = api_key or self._load_env_val("TK_IMAGE_API_KEY") or self._load_env_val("OPENAI_API_KEY")
        self.model = model or self._load_env_val("TK_IMAGE_MODEL") or "gpt-image-2"

        if not self.api_key:
            raise ValueError(f"未找到可用 API Key，请在 scripts/images/.env 中配置 TK_IMAGE_API_KEY")

    def _load_env_val(self, key: str) -> str:
        if key in os.environ:
            return os.environ[key]
        candidates = [
            DEFAULT_ENV_PATH,
            Path("/home/kiz/Code/Bong/scripts/images/.env"),
            REPO_ROOT.parent / "Bong" / "scripts" / "images" / ".env",
            REPO_ROOT.parent / ".worktree" / "bbmodel-maker" / "scripts" / "images" / ".env",
        ]
        for env_file in candidates:
            if env_file.exists():
                for line in env_file.read_text(encoding="utf-8").splitlines():
                    line = line.strip()
                    if line.startswith(f"{key}=") and not line.startswith("#"):
                        return line.split("=", 1)[1].strip().strip('"').strip("'")
        return ""

    def text_to_image(self, prompt: str, size: str = "1024x1024") -> bytes:
        """纯文生图 (Generations)"""
        url = f"{self.base_url}/v1/images/generations"
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }
        payload = {
            "model": self.model,
            "prompt": prompt,
            "n": 1,
            "size": size,
        }
        t0 = time.time()
        resp = requests.post(url, headers=headers, json=payload, timeout=90)
        elapsed = time.time() - t0
        if resp.status_code != 200:
            raise RuntimeError(f"文生图请求失败 ({resp.status_code}, 耗时 {elapsed:.1f}s): {resp.text}")
        print(f"    (耗时: {elapsed:.1f}s)")
        return self._extract_image_bytes(resp.json())

    def image_to_image(
        self,
        prompt: str,
        reference_image: bytes | Path | str,
        size: str = "1024x1024",
    ) -> bytes:
        """图生图 (Edits，基于参考图)"""
        url = f"{self.base_url}/v1/images/edits"
        headers = {
            "Authorization": f"Bearer {self.api_key}",
        }
        raw_bytes = self._read_bytes(reference_image)
        img_bytes = self._prepare_ref_image(raw_bytes)

        files = {
            "image": ("reference.png", img_bytes, "image/png"),
        }
        data = {
            "model": self.model,
            "prompt": prompt,
            "n": "1",
            "size": size,
        }
        t0 = time.time()
        resp = requests.post(url, headers=headers, files=files, data=data, timeout=90)
        elapsed = time.time() - t0
        if resp.status_code != 200:
            raise RuntimeError(f"图生图请求失败 ({resp.status_code}, 耗时 {elapsed:.1f}s): {resp.text}")
        print(f"    (耗时: {elapsed:.1f}s)")
        return self._extract_image_bytes(resp.json())

    def _prepare_ref_image(self, raw_bytes: bytes, target_side: int = 512) -> bytes:
        """将参考图压缩为中等尺寸 PNG，加快传输并稳定图生图效果"""
        try:
            im = Image.open(io.BytesIO(raw_bytes))
            w, h = im.size
            if max(w, h) > target_side:
                scale = target_side / max(w, h)
                new_size = (int(w * scale), int(h * scale))
                im = im.resize(new_size, Image.Resampling.LANCZOS)
            buf = io.BytesIO()
            im.save(buf, format="PNG", optimize=True)
            return buf.getvalue()
        except Exception:
            return raw_bytes

    def _read_bytes(self, source: bytes | Path | str) -> bytes:
        if isinstance(source, bytes):
            return source
        path = Path(source)
        if not path.exists():
            raise FileNotFoundError(f"参考图文件不存在: {path}")
        return path.read_bytes()

    def _extract_image_bytes(self, data: dict) -> bytes:
        items = data.get("data", [])
        if not items:
            raise RuntimeError(f"响应中未包含图像数据: {data}")
        first = items[0]
        if "b64_json" in first and first["b64_json"]:
            return base64.b64decode(first["b64_json"])
        if "url" in first and first["url"]:
            img_resp = requests.get(first["url"], timeout=30)
            return img_resp.content
        raise RuntimeError(f"无法解析图像格式: {first}")


def generate_single_step(
    step: Literal["concept", "icon", "three_view", "exploded"],
    subject_name: str,
    prompt: str = "",
    target_type: Literal["item", "creature"] = "item",
    ref_path: Path | None = None,
    out_dir: Path = DEFAULT_OUTPUT_DIR,
    model: str = "gpt-image-2",
) -> Path:
    """生成单步产物并落盘。"""
    out_dir.mkdir(parents=True, exist_ok=True)
    client = TKImageClient(model=model)

    out_file = out_dir / f"ref_{subject_name}_{step}.png"

    if step == "concept":
        print(f"\n[执行步骤: 概念图 (Concept Art)]")
        full_prompt = PROMPT_TEMPLATES["concept"].format(
            prompt=prompt, base_style=XIANXIA_STYLE_BASE
        )
        print(f"  Prompt: {full_prompt}")
        img_bytes = client.text_to_image(full_prompt)
        out_file.write_bytes(img_bytes)
        print(f"  ✓ 概念图生成成功: {out_file.relative_to(REPO_ROOT)}")

    elif step == "icon":
        if not ref_path or not ref_path.exists():
            ref_path = out_dir / f"ref_{subject_name}_concept.png"
        print(f"\n[执行步骤: 物品黑底图标 (Item Icon)]")
        print(f"  参考图: {ref_path}")
        full_prompt = PROMPT_TEMPLATES["icon"]
        print(f"  Prompt: {full_prompt}")
        img_bytes = client.image_to_image(full_prompt, reference_image=ref_path)
        out_file.write_bytes(img_bytes)
        print(f"  ✓ 物品图标生成成功: {out_file.relative_to(REPO_ROOT)}")

    elif step == "three_view":
        if not ref_path or not ref_path.exists():
            ref_path = out_dir / f"ref_{subject_name}_concept.png"
        print(f"\n[执行步骤: MC 体素正交三视图 (Three-View)]")
        print(f"  参考图: {ref_path}")
        template_key = "three_view_item" if target_type == "item" else "three_view_creature"
        full_prompt = PROMPT_TEMPLATES[template_key]
        print(f"  Prompt: {full_prompt}")
        img_bytes = client.image_to_image(full_prompt, reference_image=ref_path)
        out_file.write_bytes(img_bytes)
        print(f"  ✓ 三视图生成成功: {out_file.relative_to(REPO_ROOT)}")

    elif step == "exploded":
        if not ref_path or not ref_path.exists():
            # 优先尝试使用三视图作为参考图，其次概念图
            tv_path = out_dir / f"ref_{subject_name}_three_view.png"
            ref_path = tv_path if tv_path.exists() else (out_dir / f"ref_{subject_name}_concept.png")
        print(f"\n[执行步骤: MC 体素爆炸分解图 (Exploded View)]")
        print(f"  参考图: {ref_path}")
        full_prompt = PROMPT_TEMPLATES["exploded"]
        print(f"  Prompt: {full_prompt}")
        img_bytes = client.image_to_image(full_prompt, reference_image=ref_path)
        out_file.write_bytes(img_bytes)
        print(f"  ✓ 爆炸分解图生成成功: {out_file.relative_to(REPO_ROOT)}")

    return out_file


def main() -> None:
    parser = argparse.ArgumentParser(description="Bong 3D 资产概念设计与参考图生成流水线（单步执行）")
    parser.add_argument("name", help="资产代号/英文标识 (如: mutated_bone_armor, wooden_club)")
    parser.add_argument("--step", choices=["concept", "icon", "three_view", "exploded"], required=True, help="要执行的单步阶段")
    parser.add_argument("--prompt", default="", help="概念描述（生成 concept 必需，后续步骤默认使用标准图生图模板）")
    parser.add_argument(
        "--type",
        choices=["item", "creature"],
        default="item",
        help="目标类型: item (物品/装备/武器/方块，含灰色模特装备) 或 creature (生物)",
    )
    parser.add_argument("--ref", type=Path, help="图生图的参考图路径（若不提供则自动在输出目录寻找前序产物）")
    parser.add_argument("--out", type=Path, default=DEFAULT_OUTPUT_DIR, help="输出目录")
    parser.add_argument("--model", default="gpt-image-2", help="生图模型 (默认: gpt-image-2)")

    args = parser.parse_args()
    if args.step == "concept" and not args.prompt:
        parser.error("生成 concept 时必须提供 --prompt 参数")

    generate_single_step(
        step=args.step,
        subject_name=args.name,
        prompt=args.prompt,
        target_type=args.type,
        ref_path=args.ref,
        out_dir=args.out,
        model=args.model,
    )


if __name__ == "__main__":
    main()
