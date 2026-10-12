#!/usr/bin/env python3
"""灵龛诡雷三档（niche_zhenfa_trap_basic / middle / advanced）模型生成器。

物品依据 server/assets/items/niche/zhenfa_trap_*.toml：
- 初：埋在灵龛入口的初级阵法陷阱，触发一次后阵纹散尽。
- 中：可多次放射真元延缓入侵。
- 高：触发时制造短时迷乱。

三档共用一套部件，逐档加复杂度：
  初  石板 + 四个骨角钉 + 一圈方形阵纹槽 + 中央骨触发钉
  中  + 凸起石边框 + 四个侧面放射口（带暗孔）+ 中央阵核（朱砂符点）
  高  + 加高边框与角钉 + 四个斜角放射口（共八口）+ 阵核换成朱砂眼（暗瞳）+ 四根弯骨角

坐标：1 block = 16px，原点在石板底面中心，+y 向上；石板 10×10px。放在地上，所以 check.png 用 3/4 俯视 + 正上方。

    python3 modelScript/generators/gen_niche_zhenfa_trap.py            # 三档都出
    python3 modelScript/generators/gen_niche_zhenfa_trap.py basic
"""

from __future__ import annotations

import sys
from typing import Callable, Dict, List

import handheld_common as hc
from handheld_common import Cube, cube

TIERS = ("basic", "middle", "advanced")
HALF = 5.0                  # 石板半边长
SLAB_H = 1.5
RIM_W = 0.8                 # 边框宽，也是角钉的边长
RIM_INNER = HALF - RIM_W
RIM_TOP = {"middle": 2.3, "advanced": 3.0}
POST_TOP = {"basic": 2.3, "middle": 2.8, "advanced": 3.6}
RING_HALF = {"basic": 3.4, "middle": 2.4, "advanced": 2.2}
RING_W = {"basic": 0.5, "middle": 0.5, "advanced": 0.4}
NOZZLE_TOP = {"middle": 2.5, "advanced": 2.8}
NOZZLE_FRONT = 3.2          # 侧面放射口朝中心的前脸位置（离中心的距离）
DIAGONAL_NOZZLE = (2.8, 3.8)    # 斜角放射口占据的 |x|、|z| 范围
CORE_HALF = {"middle": 1.2, "advanced": 0.8}
CORE_TOP = {"middle": 2.2, "advanced": 2.4}
PIN_HALF = 0.35


def part_slab() -> List[Cube]:
    return [cube("slab", (-HALF, 0.0, -HALF), (HALF, SLAB_H, HALF), "stone", "slab")]


def part_posts(tier: str) -> List[Cube]:
    """四个骨角钉：石板四角的方墩，顶上一个骨帽。"""
    cubes = []
    top = POST_TOP[tier]
    for sx, tag_x in ((-1, "w"), (1, "e")):
        for sz, tag_z in ((-1, "n"), (1, "s")):
            x0, x1 = sorted((sx * RIM_INNER, sx * HALF))
            z0, z1 = sorted((sz * RIM_INNER, sz * HALF))
            cubes.append(cube(f"post_{tag_z}{tag_x}", (x0, SLAB_H, z0), (x1, top - 0.4, z1), "bone_main", "posts"))
            cubes.append(cube(f"post_cap_{tag_z}{tag_x}", (x0 + 0.1, top - 0.4, z0 + 0.1), (x1 - 0.1, top, z1 - 0.1),
                              "bone_lit", "posts"))
    return cubes


def part_rim(tier: str) -> List[Cube]:
    """边框四条边，夹在四个角钉之间（不含角），互不重叠。"""
    top = RIM_TOP[tier]
    return [
        cube("rim_n", (-RIM_INNER, SLAB_H, -HALF), (RIM_INNER, top, -RIM_INNER), "stone", "rim"),
        cube("rim_s", (-RIM_INNER, SLAB_H, RIM_INNER), (RIM_INNER, top, HALF), "stone", "rim"),
        cube("rim_w", (-HALF, SLAB_H, -RIM_INNER), (-RIM_INNER, top, RIM_INNER), "stone", "rim"),
        cube("rim_e", (RIM_INNER, SLAB_H, -RIM_INNER), (HALF, top, RIM_INNER), "stone", "rim"),
    ]


def part_ring(tier: str) -> List[Cube]:
    """方形阵纹槽：四条暗石细条，南北条含四角、东西条夹在中间。"""
    r, w = RING_HALF[tier], RING_W[tier]
    a, b = r - w / 2, r + w / 2
    y1 = SLAB_H + 0.15
    return [
        cube("ring_n", (-b, SLAB_H, -b), (b, y1, -a), "stone_dark", "ring"),
        cube("ring_s", (-b, SLAB_H, a), (b, y1, b), "stone_dark", "ring"),
        cube("ring_w", (-b, SLAB_H, -a), (-a, y1, a), "stone_dark", "ring"),
        cube("ring_e", (a, SLAB_H, -a), (b, y1, a), "stone_dark", "ring"),
    ]


def part_nozzles(tier: str) -> List[Cube]:
    """放射口：每个是一块小石墩，朝向中心的那一面有一个暗孔。中档四个侧面口，高档再加四个斜角口。"""
    top = NOZZLE_TOP[tier]
    cubes = []
    # (名, 墩的 x 范围, 墩的 z 范围, 暗孔 x 范围, 暗孔 z 范围)
    f = NOZZLE_FRONT
    sides = (
        ("n", (-0.8, 0.8), (-RIM_INNER, -f), (-0.4, 0.4), (-f, -f + 0.1)),
        ("s", (-0.8, 0.8), (f, RIM_INNER), (-0.4, 0.4), (f - 0.1, f)),
        ("w", (-RIM_INNER, -f), (-0.8, 0.8), (-f, -f + 0.1), (-0.4, 0.4)),
        ("e", (f, RIM_INNER), (-0.8, 0.8), (f - 0.1, f), (-0.4, 0.4)),
    )
    for tag, xr, zr, hx, hz in sides:
        cubes.append(cube(f"nozzle_{tag}", (xr[0], SLAB_H, zr[0]), (xr[1], top, zr[1]), "stone", "nozzles"))
        cubes.append(cube(f"nozzle_{tag}_hole", (hx[0], SLAB_H + 0.5, hz[0]), (hx[1], top - 0.4, hz[1]), "void", "nozzles"))
    if tier == "advanced":
        lo, hi = DIAGONAL_NOZZLE
        for sx, tag_x in ((-1, "w"), (1, "e")):
            for sz, tag_z in ((-1, "n"), (1, "s")):
                x0, x1 = sorted((sx * lo, sx * hi))
                z0, z1 = sorted((sz * lo, sz * hi))
                cubes.append(cube(f"nozzle_{tag_z}{tag_x}", (x0, SLAB_H, z0), (x1, top - 0.3, z1), "stone", "nozzles"))
                # 斜角口的孔开在朝中心的两个面交角上：用一个小暗块嵌在内角
                hx0, hx1 = sorted((sx * (lo - 0.1), sx * (lo + 0.4)))
                hz0, hz1 = sorted((sz * (lo - 0.1), sz * (lo + 0.4)))
                cubes.append(cube(f"nozzle_{tag_z}{tag_x}_hole", (hx0, SLAB_H + 0.6, hz0), (hx1, top - 0.6, hz1),
                                  "void", "nozzles"))
    return cubes


def part_core(tier: str) -> List[Cube]:
    """阵核：中档是一块方石 + 朱砂符点；高档缩小一圈，顶上换成朱砂眼（暗瞳）。"""
    h = CORE_HALF[tier]
    top = CORE_TOP[tier]
    cubes = [cube("core", (-h, SLAB_H, -h), (h, top, h), "stone", "core")]
    if tier == "middle":
        cubes.append(cube("core_dot", (-0.4, top, -0.4), (0.4, top + 0.3, 0.4), "cinnabar", "core"))
    else:
        cubes.append(cube("eye", (-0.5, top, -0.5), (0.5, top + 0.5, 0.5), "cinnabar", "core"))
        cubes.append(cube("eye_pupil", (-0.2, top + 0.5, -0.2), (0.2, top + 0.7, 0.2), "void", "core"))
    return cubes


def part_pin(tier: str) -> List[Cube]:
    """中央骨触发钉：初档直接插在石板上；中档在阵核上；高档的眼已经居中，不再单独加钉。"""
    if tier == "advanced":
        return []
    base = SLAB_H if tier == "basic" else CORE_TOP[tier] + 0.3
    return [
        cube("pin_shaft", (-PIN_HALF, base, -PIN_HALF), (PIN_HALF, base + 1.2, PIN_HALF), "bone_main", "pin"),
        cube("pin_tip", (-0.15, base + 1.2, -0.15), (0.15, base + 1.8, 0.15), "bone_lit", "pin"),
    ]


def part_horns() -> List[Cube]:
    """高档独有：四根弯骨角，从阵核四角（斜向）分三节逐节外倾、变细地升起。"""
    cubes = []
    for sx, tag_x in ((-1, "w"), (1, "e")):
        for sz, tag_z in ((-1, "n"), (1, "s")):
            for i, (lo, hi, y0, y1) in enumerate(((1.0, 1.6, SLAB_H, 2.3), (1.2, 1.7, 2.3, 3.1), (1.5, 1.8, 3.1, 3.8))):
                x0, x1 = sorted((sx * lo, sx * hi))
                z0, z1 = sorted((sz * lo, sz * hi))
                cubes.append(cube(f"horn_{tag_z}{tag_x}_{i}", (x0, y0, z0), (x1, y1, z1), "bone_main" if i < 2 else "bone_lit",
                                  "horns"))
    return cubes


def build_basic() -> List[Cube]:
    return part_slab() + part_posts("basic") + part_ring("basic") + part_pin("basic")


def build_middle() -> List[Cube]:
    return (part_slab() + part_posts("middle") + part_rim("middle") + part_ring("middle")
            + part_nozzles("middle") + part_core("middle") + part_pin("middle"))


def build_advanced() -> List[Cube]:
    return (part_slab() + part_posts("advanced") + part_rim("advanced") + part_ring("advanced")
            + part_nozzles("advanced") + part_core("advanced") + part_horns())


BUILDERS: Dict[str, Callable[[], List[Cube]]] = {
    "basic": build_basic, "middle": build_middle, "advanced": build_advanced,
}

VIEWS = (
    ("3/4 TOP-DOWN FRONT-RIGHT (yaw35 pitch55)", 35.0, 55.0),
    ("TOP-DOWN (pitch89)", 0.0, 89.0),
)


def model_id(tier: str) -> str:
    return f"niche_zhenfa_trap_{tier}"


def main() -> None:
    tiers = sys.argv[1:] or list(TIERS)
    for tier in tiers:
        cubes = BUILDERS[tier]()
        path = hc.write_model(cubes, model_id(tier))
        print(f"✓ {model_id(tier)}: {len(cubes)} 个立方体 → {path}")
        print(f"✓ check.png → {hc.write_check(path, model_id(tier), VIEWS)}")


if __name__ == "__main__":
    main()
