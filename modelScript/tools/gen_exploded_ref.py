#!/usr/bin/env python3
"""手持器物的爆炸参考图：走本机 CPA 的 gpt-image-2.5-sunburst，文生图，一件一张。

CPA 的地址与 key 只在内存里用：key 取 ``~/cpa/config.yaml`` 的 ``api-keys[0]``，不打印、不落盘。
复用 ``gen_pipeline_refs.TKImageClient``，所以请求形状和别的参考图工序一致。

用法::

    python3 modelScript/tools/gen_exploded_ref.py bao_chu \\
        --prompt "iron hoe head with wooden shaft, ..." \\
        --out /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld/exploded

产物是 ``<out>/<id>.png``。提示词里的物品外观以 ``server/assets/items`` 里该模板的
name / description 为准，写在调用处，不在这里硬编码。
"""

from __future__ import annotations

import argparse
from pathlib import Path

import yaml

from gen_pipeline_refs import PROMPT_TEMPLATES, TKImageClient

CPA_BASE_URL = "http://127.0.0.1:8317"
CPA_CONFIG = Path.home() / "cpa" / "config.yaml"
CPA_IMAGE_MODEL = "gpt-image-2.5-sunburst"

HELD_EXPLODED_STYLE = (
    "Minecraft voxel style technical exploded breakdown schematic (爆炸分解图) of a single handheld tool, "
    "disassembled parts floating in a line with clear spacing along the grip axis, "
    "each part a chunky cuboid voxel block with visible 16px-texel texture, "
    "dharma-ending xianxia wasteland aesthetic: weathered, desaturated, rust and old wood, no glowing fantasy gloss, "
    "plain neutral grey studio background, soft shadow, no text, no callout labels, no watermark"
)


def cpa_client() -> TKImageClient:
    keys = yaml.safe_load(CPA_CONFIG.read_text(encoding="utf-8"))["api-keys"]
    return TKImageClient(api_key=keys[0], base_url=CPA_BASE_URL, model=CPA_IMAGE_MODEL)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("item_id", help="模板 id，如 bao_chu")
    parser.add_argument("--prompt", required=True, help="这件器物的外观与分件描述（英文）")
    parser.add_argument("--out", type=Path, required=True, help="输出目录，文件名是 <item_id>.png")
    args = parser.parse_args()

    assert "exploded" in PROMPT_TEMPLATES
    full_prompt = f"{HELD_EXPLODED_STYLE}. Subject: {args.prompt}"
    image_bytes = cpa_client().text_to_image(full_prompt, size="1536x1024")

    args.out.mkdir(parents=True, exist_ok=True)
    target = args.out / f"{args.item_id}.png"
    target.write_bytes(image_bytes)
    print(f"{args.item_id}: 爆炸参考图 → {target}")


if __name__ == "__main__":
    main()
