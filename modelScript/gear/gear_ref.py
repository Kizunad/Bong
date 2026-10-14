#!/usr/bin/env python3
"""末土穿戴件的参考图 + 爆炸图：本机 CPA 的 gpt-image-2.5-sunburst，一件两张。

和 ``gen_exploded_ref.py`` 同一条链路（key 取 ``~/cpa/config.yaml``，不打印不落盘），
区别只在风格词：这里画的是**穿在人身上的背负/腰挂件**，不是手持器物。

用法::

    python3 modelScript/gear/gear_ref.py herb_pouch \\
        --worn "small coarse-cloth herb pouch hanging at a waist belt ..." \\
        --parts "pouch body, flap, cord, dried herbs ..." \\
        --out /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/gear/herb_pouch

产物：``<out>/ref.png``（穿戴参考）与 ``<out>/exploded.png``（爆炸图）。
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from gen_exploded_ref import cpa_client  # noqa: E402

WASTELAND = (
    "dharma-ending xianxia wasteland aesthetic: scavenged, patched and re-stitched, "
    "weathered desaturated materials (coarse hemp cloth, raw beast hide, grass rope, "
    "bleached bone, rusted iron scrap, cracked clay), no glowing fantasy gloss, "
    "no bright saturated colours, "
)
WORN_STYLE = (
    "Minecraft voxel style character equipment concept reference: a grey blocky "
    "Minecraft-proportioned mannequin wearing the item, three views (front, side, back) "
    "side by side, chunky cuboid voxel blocks with visible 16px-texel texture, "
    + WASTELAND
    + "plain neutral grey studio background, soft shadow, no text, no callout labels, no watermark"
)
EXPLODED_STYLE = (
    "Minecraft voxel style technical exploded breakdown schematic (爆炸分解图) of a single "
    "wearable pack/pouch, disassembled parts floating apart with clear spacing, "
    "each part a chunky cuboid voxel block with visible 16px-texel texture, "
    + WASTELAND
    + "plain neutral grey studio background, soft shadow, no text, no callout labels, no watermark"
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("gear_id")
    parser.add_argument("--worn", required=True, help="穿戴参考的外观与挂点描述（英文）")
    parser.add_argument("--parts", required=True, help="爆炸图的分件清单（英文）")
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--only", choices=("ref", "exploded"), help="只重出其中一张")
    args = parser.parse_args()

    client = cpa_client()
    args.out.mkdir(parents=True, exist_ok=True)
    if args.only in (None, "ref"):
        data = client.text_to_image(f"{WORN_STYLE}. Subject: {args.worn}", size="1536x1024")
        (args.out / "ref.png").write_bytes(data)
        print(f"{args.gear_id}: 穿戴参考 → {args.out / 'ref.png'}")
    if args.only in (None, "exploded"):
        data = client.text_to_image(f"{EXPLODED_STYLE}. Subject: {args.worn}. Parts: {args.parts}",
                                    size="1536x1024")
        (args.out / "exploded.png").write_bytes(data)
        print(f"{args.gear_id}: 爆炸图 → {args.out / 'exploded.png'}")


if __name__ == "__main__":
    main()
