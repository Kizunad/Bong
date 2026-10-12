#!/usr/bin/env python3
"""爆阵符（blast_trap）模型生成器。

物品依据 server/assets/items/zhenfa.toml：「贴附在方块表面，水平感知来者后引爆封存真元。一次性。」
符底与警示符、缓阵符共用（zhenfa_talisman_parts.py）；符纸是焦黑的，符纹是朱砂红的八尖放射爆裂纹，
南边缘没有绳环，改成一截捻出来的引信（拧弯，末端烧黑）。

    python3 modelScript/generators/gen_blast_trap.py
"""

from __future__ import annotations

from typing import List

import handheld_common as hc
import zhenfa_talisman_parts as tp
from handheld_common import Cube

MODEL_ID = "blast_trap"
PAPER = "paper_scorch"
INK = "ink_cinnabar"
CORE_TOP = 2.0              # 中心块比别的纹路再凸起一点
# 轴向尖刺：由内向外三节，逐节变窄 (x 起, x 止, 半宽)
AXIS_SPIKE = ((1.0, 2.6, 0.6), (2.6, 4.0, 0.4), (4.0, 5.0, 0.2))
# 斜向尖刺：沿对角线的三个方块 (中心偏移, 边长)
DIAGONAL_SPIKE = ((1.9, 1.0), (2.9, 1.0), (3.9, 0.8))


def part_burst() -> List[Cube]:
    cubes = [tp.bar("burst_core", -1.0, 1.0, -1.0, 1.0, INK, y1=CORE_TOP)]
    for i, (a, b, w) in enumerate(AXIS_SPIKE):
        cubes.append(tp.bar(f"spike_e_{i}", a, b, -w, w, INK))
        cubes.append(tp.bar(f"spike_w_{i}", -b, -a, -w, w, INK))
        cubes.append(tp.bar(f"spike_s_{i}", -w, w, a, b, INK))
        cubes.append(tp.bar(f"spike_n_{i}", -w, w, -b, -a, INK))
    for sx, tag_x in ((-1, "w"), (1, "e")):
        for sz, tag_z in ((-1, "n"), (1, "s")):
            for i, (center, size) in enumerate(DIAGONAL_SPIKE):
                h = size / 2
                cx, cz = sx * center, sz * center
                cubes.append(tp.bar(f"spike_{tag_z}{tag_x}_{i}", cx - h, cx + h, cz - h, cz + h, INK,
                                    y1=tp.GLYPH_TOP - 0.1 * i))
    return cubes


def part_fuse() -> List[Cube]:
    """南边缘的引信：三节拧弯的绳 + 烧黑的末端。"""
    y0, y1 = 0.3, 0.9
    z = tp.HALF
    return [
        hc.cube("fuse_0", (-0.35, y0, z), (0.35, y1, z + 0.9), "rope", "fuse"),
        hc.cube("fuse_1", (0.0, y0, z + 0.9), (0.7, y1, z + 1.6), "rope", "fuse"),
        hc.cube("fuse_2", (0.35, y0, z + 1.6), (1.05, y1, z + 2.3), "ink_cinnabar", "fuse"),
        hc.cube("fuse_tip", (0.55, y0, z + 2.3), (1.25, y1, z + 2.8), "stone_dark", "fuse"),
    ]


def all_cubes() -> List[Cube]:
    return tp.base_cubes(PAPER, with_cord=False) + part_burst() + part_fuse()


VIEWS = (
    ("3/4 TOP-DOWN FRONT-RIGHT (yaw35 pitch55)", 35.0, 55.0),
    ("TOP-DOWN (pitch89)", 0.0, 89.0),
)


def main() -> None:
    cubes = all_cubes()
    model = hc.write_model(cubes, MODEL_ID)
    print(f"✓ {MODEL_ID}: {len(cubes)} 个立方体 → {model}")
    print(f"✓ check.png → {hc.write_check(model, MODEL_ID, VIEWS)}")


if __name__ == "__main__":
    main()
