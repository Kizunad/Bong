#!/usr/bin/env python3
"""灵龛诡雷三档（niche_zhenfa_trap_basic / middle / advanced）模型生成器。

物品依据 server/assets/items/niche/zhenfa_trap_*.toml：
- 初：埋在灵龛入口的初级阵法陷阱，触发一次后阵纹散尽。
- 中：可多次放射真元延缓入侵。
- 高：触发时制造短时迷乱。

三档共用一套部件，逐档叠加（调度审第 1 次后按参考的立体结构重做）：
  初  底座 14×14×2 石板，顶面一圈阵纹凹槽；四角各一根 2×2×4 骨柱；正中一根 1×1×4 骨刺
  中  + 底座外缘 1px 高护边；中间 8×8、高 2 的方形内框，框上嵌红色符纹；骨柱加高到 6、各缠两道红箍；
      正中换成 2×2 红色符块，上面立骨刺
  高  + 内框上叠一层 6×6 第二层框；内框四角各伸出一根向外弯、高 6 的骨角（每 2px 向外偏一格）；
      正中是 3×3 红色核心，放在 2px 高的石台上；底座四边各伸出一截短管

颜色统一：石灰 + 骨白 + 暗红。坐标：1 block = 16px，原点在石板底面中心，+y 向上。
放在地上，所以 check.png 用 3/4 俯视 + 正上方。

    python3 modelScript/generators/gen_niche_zhenfa_trap.py            # 三档都出
    python3 modelScript/generators/gen_niche_zhenfa_trap.py basic
"""

from __future__ import annotations

import sys
from typing import Callable, Dict, List

import handheld_common as hc
from handheld_common import Cube, cube

TIERS = ("basic", "middle", "advanced")
HALF = 7.0                  # 底座半边长（14×14）
LOWER_H = 1.5               # 底座下层（整块）
SLAB_H = 2.0                # 底座总厚；上层 0.5 开出阵纹凹槽
GROOVE = (4.2, 5.0)         # 凹槽内外半边长，刚好落在角柱（x 5..7）内侧
POST_IN = 5.0               # 骨柱占 x、z 5..7
POST_TOP = {"basic": 6.0, "middle": 8.0, "advanced": 8.0}     # 柱顶高度（基础 2 + 柱高 4 / 6）
SPIKE_H = 4.0
RIM_H = 1.0                 # 外缘护边高
FRAME_HALF = 4.0            # 内框 8×8
FRAME_W = 1.6               # 内框边宽
FRAME_TOP = 4.0             # 内框高 2（从 2 到 4）
FRAME2_HALF = 3.0           # 第二层框 6×6
FRAME2_W = 0.6
FRAME2_TOP = 6.0
HORN_STEP = 1.0             # 骨角每升 2px，沿 x、z 各向外偏的量（对角线上约 1.4px）
RED_BAND_Y = ((4.5, 5.0), (6.0, 6.5))   # 骨柱上两道红箍


def part_slab() -> List[Cube]:
    """底座：下层整块 + 上层 0.5px，上层在 4.2~5.0 半边长之间留出一圈凹槽，槽底是暗石。"""
    g0, g1 = GROOVE
    top = (LOWER_H, SLAB_H)
    cubes = [cube("slab_lower", (-HALF, 0.0, -HALF), (HALF, LOWER_H, HALF), "stone", "slab")]
    cubes += [
        cube("slab_outer_n", (-HALF, top[0], -HALF), (HALF, top[1], -g1), "stone", "slab"),
        cube("slab_outer_s", (-HALF, top[0], g1), (HALF, top[1], HALF), "stone", "slab"),
        cube("slab_outer_w", (-HALF, top[0], -g1), (-g1, top[1], g1), "stone", "slab"),
        cube("slab_outer_e", (g1, top[0], -g1), (HALF, top[1], g1), "stone", "slab"),
        cube("slab_inner", (-g0, top[0], -g0), (g0, top[1], g0), "stone", "slab"),
    ]
    floor_y = LOWER_H + 0.1
    cubes += [
        cube("groove_n", (-g1, LOWER_H, -g1), (g1, floor_y, -g0), "stone_dark", "slab"),
        cube("groove_s", (-g1, LOWER_H, g0), (g1, floor_y, g1), "stone_dark", "slab"),
        cube("groove_w", (-g1, LOWER_H, -g0), (-g0, floor_y, g0), "stone_dark", "slab"),
        cube("groove_e", (g0, LOWER_H, -g0), (g1, floor_y, g0), "stone_dark", "slab"),
    ]
    return cubes


def part_posts(tier: str) -> List[Cube]:
    """四角骨柱：2×2 骨柱，顶上略缩一圈的骨帽；中、高档柱身缠两道暗红箍。"""
    cubes = []
    top = POST_TOP[tier]
    for sx, tag_x in ((-1, "w"), (1, "e")):
        for sz, tag_z in ((-1, "n"), (1, "s")):
            x0, x1 = sorted((sx * POST_IN, sx * HALF))
            z0, z1 = sorted((sz * POST_IN, sz * HALF))
            cubes.append(cube(f"post_{tag_z}{tag_x}", (x0, SLAB_H, z0), (x1, top - 0.5, z1), "bone_main", "posts"))
            cubes.append(cube(f"post_cap_{tag_z}{tag_x}", (x0 + 0.2, top - 0.5, z0 + 0.2), (x1 - 0.2, top, z1 - 0.2),
                              "bone_lit", "posts"))
            if tier != "basic":
                for i, (y0, y1) in enumerate(RED_BAND_Y):
                    # 红箍比柱身四面各粗 0.1，裹住柱身
                    cubes.append(cube(f"post_band_{tag_z}{tag_x}_{i}", (x0 - 0.1, y0, z0 - 0.1), (x1 + 0.1, y1, z1 + 0.1),
                                      "cinnabar", "posts"))
    return cubes


def part_spike(base: float) -> List[Cube]:
    """正中骨刺：1×1×4，从 base 高度立起，顶端收细。"""
    return [
        cube("spike_shaft", (-0.5, base, -0.5), (0.5, base + SPIKE_H - 0.8, 0.5), "bone_main", "spike"),
        cube("spike_tip", (-0.25, base + SPIKE_H - 0.8, -0.25), (0.25, base + SPIKE_H, 0.25), "bone_lit", "spike"),
    ]


def part_rim() -> List[Cube]:
    """底座外缘 1px 高护边，夹在四个骨柱之间。"""
    top = SLAB_H + RIM_H
    return [
        cube("rim_n", (-POST_IN, SLAB_H, -HALF), (POST_IN, top, -HALF + 1.0), "stone", "rim"),
        cube("rim_s", (-POST_IN, SLAB_H, HALF - 1.0), (POST_IN, top, HALF), "stone", "rim"),
        cube("rim_w", (-HALF, SLAB_H, -POST_IN), (-HALF + 1.0, top, POST_IN), "stone", "rim"),
        cube("rim_e", (HALF - 1.0, SLAB_H, -POST_IN), (HALF, top, POST_IN), "stone", "rim"),
    ]


def part_frame() -> List[Cube]:
    """8×8、高 2 的方形内框：南北条含四角、东西条夹在中间；框顶面嵌红色符纹（每边中点一道横纹 + 四角一个点）。"""
    h, w = FRAME_HALF, FRAME_W
    cubes = [
        cube("frame_n", (-h, SLAB_H, -h), (h, FRAME_TOP, -h + w), "stone", "frame"),
        cube("frame_s", (-h, SLAB_H, h - w), (h, FRAME_TOP, h), "stone", "frame"),
        cube("frame_w", (-h, SLAB_H, -h + w), (-h + w, FRAME_TOP, h - w), "stone", "frame"),
        cube("frame_e", (h - w, SLAB_H, -h + w), (h, FRAME_TOP, h - w), "stone", "frame"),
    ]
    y1 = FRAME_TOP + 0.15
    mid = h - w / 2
    cubes += [
        cube("rune_n", (-1.2, FRAME_TOP, -mid - 0.25), (1.2, y1, -mid + 0.25), "cinnabar", "frame_rune"),
        cube("rune_s", (-1.2, FRAME_TOP, mid - 0.25), (1.2, y1, mid + 0.25), "cinnabar", "frame_rune"),
        cube("rune_w", (-mid - 0.25, FRAME_TOP, -1.0), (-mid + 0.25, y1, 1.0), "cinnabar", "frame_rune"),
        cube("rune_e", (mid - 0.25, FRAME_TOP, -1.0), (mid + 0.25, y1, 1.0), "cinnabar", "frame_rune"),
    ]
    for sx, tag_x in ((-1, "w"), (1, "e")):
        for sz, tag_z in ((-1, "n"), (1, "s")):
            cx, cz = sx * (h - w / 2), sz * (h - w / 2)
            cubes.append(cube(f"rune_corner_{tag_z}{tag_x}", (cx - 0.3, FRAME_TOP, cz - 0.3), (cx + 0.3, y1, cz + 0.3),
                              "cinnabar", "frame_rune"))
    return cubes


def part_frame2() -> List[Cube]:
    """高档独有：内框上叠一层 6×6 的第二层框（立在内框的边上）。"""
    h, w = FRAME2_HALF, FRAME2_W
    y0 = FRAME_TOP + 0.15           # 在内框的红纹之上
    return [
        cube("frame2_n", (-h, y0, -h), (h, FRAME2_TOP, -h + w), "stone", "frame2"),
        cube("frame2_s", (-h, y0, h - w), (h, FRAME2_TOP, h), "stone", "frame2"),
        cube("frame2_w", (-h, y0, -h + w), (-h + w, FRAME2_TOP, h - w), "stone", "frame2"),
        cube("frame2_e", (h - w, y0, -h + w), (h, FRAME2_TOP, h - w), "stone", "frame2"),
    ]


def part_center(tier: str) -> List[Cube]:
    """正中：初档直接立骨刺；中档 2×2 红色符块 + 骨刺；高档 2px 高石台 + 3×3 红色核心。"""
    if tier == "basic":
        return part_spike(SLAB_H)
    if tier == "middle":
        cubes = [cube("rune_block", (-1.0, SLAB_H, -1.0), (1.0, SLAB_H + 1.5, 1.0), "cinnabar", "center")]
        return cubes + part_spike(SLAB_H + 1.5)
    return [
        cube("pedestal", (-2.0, SLAB_H, -2.0), (2.0, SLAB_H + 2.0, 2.0), "stone", "center"),
        cube("core", (-1.5, SLAB_H + 2.0, -1.5), (1.5, SLAB_H + 5.0, 1.5), "cinnabar", "center"),
    ]


def part_horns() -> List[Cube]:
    """高档：内框四角各伸出一根向外弯、高 6 的骨角，每升 2px 沿 x、z 各向外偏一格（共三节）。"""
    cubes = []
    base_center = FRAME2_HALF + 0.4                  # 骨角根部紧贴第二层框外侧，往外弯 3 节
    for sx, tag_x in ((-1, "w"), (1, "e")):
        for sz, tag_z in ((-1, "n"), (1, "s")):
            for i in range(3):
                c = base_center + i * HORN_STEP
                x0, x1 = sorted((sx * (c - 0.4), sx * (c + 0.4)))
                z0, z1 = sorted((sz * (c - 0.4), sz * (c + 0.4)))
                y0 = FRAME_TOP + 0.15 + 2.0 * i
                cubes.append(cube(f"horn_{tag_z}{tag_x}_{i}", (x0, y0, z0), (x1, y0 + 2.0, z1),
                                  "bone_main" if i < 2 else "bone_lit", "horns"))
    return cubes


def part_pipes() -> List[Cube]:
    """高档：底座四边各伸出一截短管（2×2 外截面，长 2px，端面一个暗孔）。"""
    cubes = []
    y0, y1 = 0.0, 2.0
    for tag, (ax, az) in (("n", (0, -1)), ("s", (0, 1)), ("w", (-1, 0)), ("e", (1, 0))):
        if az:       # 南北：沿 z 伸出
            z0, z1 = sorted((az * HALF, az * (HALF + 2.0)))
            cubes.append(cube(f"pipe_{tag}", (-1.0, y0, z0), (1.0, y1, z1), "stone", "pipes"))
            hz0, hz1 = sorted((az * (HALF + 2.0), az * (HALF + 2.1)))
            cubes.append(cube(f"pipe_{tag}_hole", (-0.5, 0.5, hz0), (0.5, 1.5, hz1), "void", "pipes"))
        else:        # 东西：沿 x 伸出
            x0, x1 = sorted((ax * HALF, ax * (HALF + 2.0)))
            cubes.append(cube(f"pipe_{tag}", (x0, y0, -1.0), (x1, y1, 1.0), "stone", "pipes"))
            hx0, hx1 = sorted((ax * (HALF + 2.0), ax * (HALF + 2.1)))
            cubes.append(cube(f"pipe_{tag}_hole", (hx0, 0.5, -0.5), (hx1, 1.5, 0.5), "void", "pipes"))
    return cubes


def build_basic() -> List[Cube]:
    return part_slab() + part_posts("basic") + part_center("basic")


def build_middle() -> List[Cube]:
    return part_slab() + part_posts("middle") + part_rim() + part_frame() + part_center("middle")


def build_advanced() -> List[Cube]:
    return (part_slab() + part_posts("advanced") + part_rim() + part_frame() + part_frame2()
            + part_center("advanced") + part_horns() + part_pipes())


BUILDERS: Dict[str, Callable[[], List[Cube]]] = {
    "basic": build_basic, "middle": build_middle, "advanced": build_advanced,
}

VIEWS = (
    ("3/4 TOP-DOWN FRONT-RIGHT (yaw35 pitch45)", 35.0, 45.0),
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
        if hc.review_available():
            print(f"✓ check.png → {hc.write_check(path, model_id(tier), VIEWS)}")


if __name__ == "__main__":
    main()
