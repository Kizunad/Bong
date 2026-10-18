#!/usr/bin/env python3
"""末土装备参考图与爆炸图：走本机 CPA 的 gpt-image-2.5-sunburst（同 gen_exploded_ref.py 的通道）。

CPA 地址与密钥只在内存里用，不打印不落盘；这里只换成「穿戴件」的提示词风格。

用法::

    python3 modelScript/gear/_kit/gen_gear_ref.py gu_jia_bei_lou --kind ref \\
        --prompt "backpack made of bent beast bones, woven straw basket body ..."
    python3 modelScript/gear/_kit/gen_gear_ref.py gu_jia_bei_lou --kind exploded --prompt "..."

输出：<out>/<id>_ref.png（穿在方块人身上的正 / 侧 / 背）与 <id>_exploded.png（分件爆炸图）。
out 缺省为 $BONG_MODEL_REVIEW_DIR/img/gear/<id>。
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
from gen_exploded_ref import cpa_client  # noqa: E402

REVIEW_ROOT = Path(os.environ.get(
    "BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"))

GEAR_REF_STYLE = (
    "Minecraft voxel style orthographic reference sheet of a wearable piece of equipment in a cultivation "
    "wasteland: the item shown worn on a plain blocky grey mannequin (Minecraft player proportions), three views "
    "side by side — front, right side, back — plus one close-up of the item alone, "
    "blocky chunky cuboid voxel geometry with visible texels, "
    "dharma-ending xianxia wasteland aesthetic: scavenged, patched and mended from beast bone, hide, coarse linen, "
    "straw rope, rusted iron scrap, desaturated earthy tones, weathered, no fantasy gloss, no glowing neon, "
    "plain neutral grey studio background (#777777), clean lighting, clear proportions for 3D modeling reference, "
    "no text, no watermark"
)

GEAR_EXPLODED_STYLE = (
    "Minecraft voxel style technical exploded breakdown schematic (爆炸分解图) of a wearable piece of equipment "
    "in a cultivation wasteland: the item disassembled into its separate components floating with clear structural "
    "spacing in midair (frame, body, straps, fasteners, hanging trinkets, patches), "
    "each part a chunky cuboid voxel block with visible texels, "
    "dharma-ending xianxia wasteland aesthetic: scavenged, patched and mended from beast bone, hide, coarse linen, "
    "straw rope, rusted iron scrap, desaturated, weathered, no fantasy gloss, "
    "plain neutral grey studio background (#777777), soft shadow, no text, no callout labels, no watermark"
)

STYLES = {"ref": GEAR_REF_STYLE, "exploded": GEAR_EXPLODED_STYLE}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("gear_id")
    ap.add_argument("--prompt", required=True, help="这件装备的外观与分件描述（英文）")
    ap.add_argument("--kind", choices=sorted(STYLES), required=True)
    ap.add_argument("--out", type=Path, default=None)
    args = ap.parse_args()

    out = args.out or REVIEW_ROOT / "img" / "gear" / args.gear_id
    out.mkdir(parents=True, exist_ok=True)
    image = cpa_client().text_to_image(f"{STYLES[args.kind]}. Subject: {args.prompt}", size="1536x1024")
    target = out / f"{args.gear_id}_{args.kind}.png"
    target.write_bytes(image)
    print(f"{args.gear_id}: {args.kind} → {target}")


if __name__ == "__main__":
    main()
