#!/usr/bin/env python3
"""缓阵符（slow_trap）模型生成器。

物品依据 server/assets/items/zhenfa.toml：「埋入地下后释放负压场减速上方来者。三次触发后耗尽。」
符底与警示符、爆阵符共用（zhenfa_talisman_parts.py）；符纸是霜灰蓝，符纹是向内收拢的方形螺旋（三圈），
正中一个空心点（负压的"眼"）。符角还有三道刻痕表示三次触发。

    python3 modelScript/generators/gen_slow_trap.py
"""

from __future__ import annotations

from typing import List, Tuple

import handheld_common as hc
import zhenfa_talisman_parts as tp
from handheld_common import Cube

MODEL_ID = "slow_trap"
PAPER = "paper_frost"
INK = "ink_blue"
LINE_WIDTH = 0.6
# 方形螺旋的拐点（x, z），由外向内，每圈收 1.2px；最后一段是自由端
SPIRAL: Tuple[Tuple[float, float], ...] = (
    (-4.8, -4.8), (4.8, -4.8), (4.8, 4.8), (-4.8, 4.8), (-4.8, -3.6),
    (3.6, -3.6), (3.6, 3.6), (-3.6, 3.6), (-3.6, -2.4),
    (2.4, -2.4), (2.4, 2.4), (-2.4, 2.4), (-2.4, -1.2),
    (1.2, -1.2), (1.2, 1.2),
)
HOLLOW_DOT = 0.7            # 中心空心点的半边长


def part_spiral() -> List[Cube]:
    """横向线段含两端拐角，纵向线段缩进拐角方块，保证各条互不重叠（共面门禁）。自由端（最后一点）向外多出半个线宽。"""
    cubes = []
    half = LINE_WIDTH / 2
    last = len(SPIRAL) - 2
    for k, ((x0, z0), (x1, z1)) in enumerate(zip(SPIRAL, SPIRAL[1:])):
        if z0 == z1:        # 横向
            cubes.append(tp.bar(f"spiral_{k}", min(x0, x1) - half, max(x0, x1) + half, z0 - half, z0 + half, INK))
        else:               # 纵向：起点接着横向段，缩进；终点除自由端外也缩进
            lo, hi = min(z0, z1) + half, max(z0, z1) - half
            if k == last:
                lo, hi = (lo, hi + half) if z1 > z0 else (lo - half, hi)
            cubes.append(tp.bar(f"spiral_{k}", x0 - half, x0 + half, lo, hi, INK))
    return cubes


def part_eye() -> List[Cube]:
    """中心的空心点：一个暗色小方坑（略低于线条）。"""
    return [hc.cube("hollow_dot", (-HOLLOW_DOT, tp.PAPER_TOP, -HOLLOW_DOT), (HOLLOW_DOT, tp.PAPER_TOP + 0.1, HOLLOW_DOT),
                    "void", "glyph")]


def part_tally() -> List[Cube]:
    """北边框上三道短刻痕：三次触发后耗尽。"""
    return [hc.cube(f"tally_{i}", (-1.0 + i * 1.0 - 0.15, tp.FRAME_TOP, -tp.HALF + 0.1),
                    (-1.0 + i * 1.0 + 0.15, tp.FRAME_TOP + 0.12, -tp.INNER - 0.1), "stone_dark", "glyph")
            for i in range(3)]


def all_cubes() -> List[Cube]:
    return tp.base_cubes(PAPER) + part_spiral() + part_eye() + part_tally()


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
