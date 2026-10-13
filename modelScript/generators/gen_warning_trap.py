#!/usr/bin/env python3
"""警示符（warning_trap）模型生成器。

物品依据 server/assets/items/zhenfa.toml：「埋入地下后感知上方三格内的真元体。触发时向放置者传递方向与距离。」
符底与缓阵符、爆阵符共用（zhenfa_talisman_parts.py）；符纸是旧赭黄，符纹是一只睁开的眼加三圈向外扩的感应环。

    python3 modelScript/generators/gen_warning_trap.py
"""

from __future__ import annotations

from typing import List

import handheld_common as hc
import zhenfa_talisman_parts as tp
from handheld_common import Cube

MODEL_ID = "warning_trap"
PAPER = "paper_ochre"
INK = "ink_ochre"
RING_HALF_SIZES = (4.8, 3.6, 2.4)       # 三圈方环的中线半边长，向外扩
RING_WIDTH = 0.6


def part_rings() -> List[Cube]:
    """三圈同心方环（感应范围），每圈四条边，南北边含四角、东西边夹在中间，互不重叠。"""
    cubes = []
    for i, r in enumerate(RING_HALF_SIZES):
        a, b = r - RING_WIDTH / 2, r + RING_WIDTH / 2
        cubes.append(tp.bar(f"ring{i}_n", -b, b, -b, -a, INK))
        cubes.append(tp.bar(f"ring{i}_s", -b, b, a, b, INK))
        cubes.append(tp.bar(f"ring{i}_w", -b, -a, -a, a, INK))
        cubes.append(tp.bar(f"ring{i}_e", a, b, -a, a, INK))
    return cubes


def part_eye() -> List[Cube]:
    """眼：三层叠成的杏仁形 + 抬高一层的瞳点。"""
    return [
        tp.bar("eye_mid", -2.0, 2.0, -0.4, 0.4, INK),
        tp.bar("eye_upper", -1.5, 1.5, 0.4, 1.0, INK),
        tp.bar("eye_upper_tip", -0.8, 0.8, 1.0, 1.5, INK),
        tp.bar("eye_lower", -1.5, 1.5, -1.0, -0.4, INK),
        tp.bar("eye_lower_tip", -0.8, 0.8, -1.5, -1.0, INK),
        hc.cube("eye_pupil", (-0.5, tp.GLYPH_TOP, -0.5), (0.5, tp.GLYPH_TOP + 0.3, 0.5), "stone_dark", "glyph"),
    ]


def all_cubes() -> List[Cube]:
    return tp.base_cubes(PAPER) + part_rings() + part_eye()


VIEWS = (
    ("3/4 TOP-DOWN FRONT-RIGHT (yaw35 pitch55)", 35.0, 55.0),
    ("TOP-DOWN (pitch89)", 0.0, 89.0),
)


def main() -> None:
    cubes = all_cubes()
    model = hc.write_model(cubes, MODEL_ID)
    print(f"✓ {MODEL_ID}: {len(cubes)} 个立方体 → {model}")
    if hc.review_available():
        print(f"✓ check.png → {hc.write_check(model, MODEL_ID, VIEWS)}")


if __name__ == "__main__":
    main()
