#!/usr/bin/env python3
"""警示符 / 缓阵符 / 爆阵符共用的符底部件（三张符只差符纸颜色和符纹）。

放在地上的方形符：木底板 + 一圈木边框 + 四角骨钉 + 符纸层 + 一小截绳环，符纹是符纸上凸起 0.3px 的颜料条。
坐标：1 block = 16px，原点在底板底面中心，+y 向上；底板 12×12px，整件约 3.2px 高。

三张符各自的 gen_*.py 调 base_cubes(paper_material) 拿符底，再加上自己的 glyph_cubes()。
"""

from __future__ import annotations

from typing import List

from handheld_common import Cube, cube

HALF = 6.0                  # 底板半边长
BASE_H = 1.2                # 底板厚
FRAME_W = 0.8               # 边框宽
FRAME_TOP = 2.0             # 边框顶面高
PAPER_TOP = 1.5             # 符纸顶面高
GLYPH_TOP = 1.8             # 符纹顶面高（凸出符纸 0.3px）
INNER = HALF - FRAME_W      # 符纸半边长


def part_base() -> List[Cube]:
    return [cube("base_plate", (-HALF, 0.0, -HALF), (HALF, BASE_H, HALF), "wood", "base")]


def part_frame() -> List[Cube]:
    y0, y1 = BASE_H, FRAME_TOP
    return [
        cube("frame_n", (-HALF, y0, -HALF), (HALF, y1, -INNER), "wood", "frame"),
        cube("frame_s", (-HALF, y0, INNER), (HALF, y1, HALF), "wood", "frame"),
        cube("frame_w", (-HALF, y0, -INNER), (-INNER, y1, INNER), "wood", "frame"),
        cube("frame_e", (INNER, y0, -INNER), (HALF, y1, INNER), "wood", "frame"),
    ]


def part_pins() -> List[Cube]:
    """四角骨钉：细钉身 + 略宽的钉帽，钉在边框四角上。"""
    cubes = []
    for sx, tag_x in ((-1, "w"), (1, "e")):
        for sz, tag_z in ((-1, "n"), (1, "s")):
            cx, cz = sx * (HALF - FRAME_W / 2), sz * (HALF - FRAME_W / 2)
            cubes.append(cube(f"pin_shaft_{tag_z}{tag_x}", (cx - 0.3, FRAME_TOP, cz - 0.3), (cx + 0.3, 2.8, cz + 0.3),
                              "bone_main", "pins"))
            cubes.append(cube(f"pin_head_{tag_z}{tag_x}", (cx - 0.5, 2.8, cz - 0.5), (cx + 0.5, 3.2, cz + 0.5),
                              "bone_lit", "pins"))
    return cubes


def part_paper(paper_material: str) -> List[Cube]:
    return [cube("paper", (-INNER, BASE_H, -INNER), (INNER, PAPER_TOP, INNER), paper_material, "paper")]


def part_cord() -> List[Cube]:
    """南边缘的一小截绳环（平躺在地面上）：结 + 两条边 + 末端横条。"""
    y0, y1 = 0.3, 0.8
    return [
        cube("cord_knot", (-0.6, y0, HALF), (0.6, y1 + 0.2, HALF + 0.7), "rope", "cord"),
        cube("cord_leg_w", (-1.4, y0, HALF + 0.7), (-0.9, y1, HALF + 2.4), "rope", "cord"),
        cube("cord_leg_e", (0.9, y0, HALF + 0.7), (1.4, y1, HALF + 2.4), "rope", "cord"),
        cube("cord_tip", (-1.4, y0, HALF + 2.4), (1.4, y1, HALF + 2.9), "rope", "cord"),
    ]


def base_cubes(paper_material: str, with_cord: bool = True) -> List[Cube]:
    cubes = part_base() + part_frame() + part_pins() + part_paper(paper_material)
    return cubes + part_cord() if with_cord else cubes


def bar(name: str, x0: float, x1: float, z0: float, z1: float, material: str, y1: float = GLYPH_TOP) -> Cube:
    """符纹条：落在符纸上、凸起到 y1。"""
    return cube(name, (x0, PAPER_TOP, z0), (x1, y1, z1), material, "glyph")
