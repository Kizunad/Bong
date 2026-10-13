#!/usr/bin/env python3
"""末世生物 / 器物参考图与爆炸图生成工具：走本机 CPA 的 gpt-image-2.5-sunburst。

CPA 地址与密钥仅在内存中使用，严禁打印和落盘。
复用 gen_pipeline_refs.TKImageClient 结构。

用法::

    # 生成爆炸图
    python3 modelScript/tools/gen_exploded_ref.py can_bei_tuo \\
        --prompt "low-slung stocky beast of burden between ox and rhino..." \\
        --out /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/can_bei_tuo \\
        --kind exploded

    # 生成参考图
    python3 modelScript/tools/gen_exploded_ref.py can_bei_tuo \\
        --prompt "low-slung stocky beast of burden between ox and rhino..." \\
        --out /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/can_bei_tuo \\
        --kind ref
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import yaml

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_pipeline_refs import TKImageClient

CPA_BASE_URL = "http://127.0.0.1:8317"
CPA_CONFIG = Path.home() / "cpa" / "config.yaml"
CPA_IMAGE_MODEL = "gpt-image-2.5-sunburst"

CREATURE_REF_STYLE = (
    "Minecraft voxel style orthographic full-body 3D concept reference of a cultivation wasteland creature, "
    "blocky chunky cuboid voxel geometry with distinct voxel segments, "
    "dharma-ending xianxia wasteland aesthetic: desaturated earthy tones, weathered textures, no glowing neon fantasy gloss, "
    "plain neutral grey studio background (#777777), clean lighting, clear proportions for 3D modeling reference, no text, no watermark"
)

CREATURE_EXPLODED_STYLE = (
    "Minecraft voxel style technical exploded breakdown schematic (爆炸分解图) of a cultivation wasteland creature, "
    "disassembled anatomical body parts floating with clear structural spacing in midair: "
    "separate skull, neck, torso/spine body, dorsal moss/shell layers, four stout legs, and tail, "
    "each part a chunky cuboid voxel block with visible texels, "
    "dharma-ending xianxia wasteland aesthetic: desaturated, weathered, no fantasy gloss, "
    "plain neutral grey studio background (#777777), soft shadow, no text, no callout labels, no watermark"
)


def cpa_client() -> TKImageClient:
    keys = yaml.safe_load(CPA_CONFIG.read_text(encoding="utf-8"))["api-keys"]
    return TKImageClient(api_key=keys[0], base_url=CPA_BASE_URL, model=CPA_IMAGE_MODEL)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("item_id", help="生物/器物 ID，如 can_bei_tuo")
    parser.add_argument("--prompt", required=True, help="外观与结构要点描述（英文）")
    parser.add_argument("--out", type=Path, required=True, help="输出目录")
    parser.add_argument(
        "--kind",
        choices=["exploded", "ref", "both"],
        default="both",
        help="生成类型: exploded(爆炸图), ref(整兽参考图), both(两者都出)",
    )
    args = parser.parse_args()

    args.out.mkdir(parents=True, exist_ok=True)
    client = cpa_client()

    if args.kind in ("ref", "both"):
        print(f"正在生成 {args.item_id} 概念参考图...")
        ref_prompt = f"{CREATURE_REF_STYLE}. Subject: {args.prompt}"
        img_bytes = client.text_to_image(ref_prompt, size="1536x1024")
        ref_target = args.out / f"{args.item_id}_ref.png"
        ref_target.write_bytes(img_bytes)
        print(f"✓ 概念参考图已保存: {ref_target}")

    if args.kind in ("exploded", "both"):
        print(f"正在生成 {args.item_id} 爆炸分解图...")
        exploded_prompt = f"{CREATURE_EXPLODED_STYLE}. Subject: {args.prompt}"
        img_bytes = client.text_to_image(exploded_prompt, size="1536x1024")
        exp_target = args.out / f"{args.item_id}_exploded.png"
        exp_target.write_bytes(img_bytes)
        print(f"✓ 爆炸分解图已保存: {exp_target}")


if __name__ == "__main__":
    main()
