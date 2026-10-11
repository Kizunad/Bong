#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o07: spleen (脾，土)

依据：
- model-review/meridian_factory.md「统一约定」、配色表、「第 2 批」o07 行、「o07 spleen 调度审第 1 次」
- 爆炸图 exploded/x07_spleen.png，单件图 refs/o07_spleen.png
- 接口件：两根血管的管口都用 common_parts.bone_ring_port 收口；bbmodel 落盘与审阅图走 organ_common.py。
  common_parts.flesh_collar 是给 8×6 截面做的，套不上 3×3 的血管，肉箍在这里单做（5×5，各管两个）。

单位换算：调度卡里的「格」按 2px 算，x 原点在 32px 外框的左缘，所以卡里「x=8」就是本模型的 x=0（外框中心），
「x=10」是 +4px。外框 32×32×32，原点在底面中心，正面朝 +Z。

豆身：长轴竖直，14 层 × 2px = 28px 高（y 2..30）。每层横截面是椭圆，宽 14px（半轴 7）、厚 12px（半轴 6），
沿 X 切成 14 条 1px 宽的柱，柱的半深 = 6 × sqrt(1 - (u/7)²)，u 为柱中心到本层中心的距离，取到 0.5px。
中心 x 随高度走弧线 cx = 4 × sin(π × (k+0.5) / 14)，取整，两端 ≈0、中段 y 14..18 到 +4，
所以中段向 +X 鼓、凹面朝 -X：

   层 k   y        中心 cx
    0     2..4        0
    1     4..6        1
    2     6..8        2
    3     8..10       3
    4     10..12      3
    5     12..14      4      ← 脾门凹口：y 12..20 共 4 层，每层把左侧 4px 切掉（凹 2 格），
    6     14..16      4         凹口面在 x = cx - 3 = 1
    7     16..18      4
    8     18..20      4
    9     20..22      3
   10     22..24      3
   11     24..26      2
   12     26..28      1
   13     28..30      0

主色 #6a2a3a，#8a2a2a 成片的斑。

血管（3×3 筋管，各 3 段：水平段 + 拐角块 + 竖直段，竖直段中点和水平段中点各一个 5×5 肉箍，管口骨环）：
- 入口（上）：水平段 y 16.5..19.5 从凹口面向 -X 伸到 x=-7，拐角后向上，管口朝上，环在 y=29.5；
- 出口（下）：水平段 y 12.5..15.5（让开层边界 y=12 / 20），拐角后向下，管口朝下，环在 y=0（底面）。
两根管竖直段的 x 都在 -10..-7，让开豆身的左缘（最靠左的 x=-7）。

脉络 #c07868：1×1px 格，贴在 +Z 面凸 0.5px。主干从脾门（凹口面）沿凹面向上、向下两路延伸，
每路各分 2 级：一级支从主干向右斜出，二级细枝再从一级支分出。其余地方不铺。
"""

from __future__ import annotations

import argparse
import math
from pathlib import Path
from typing import Dict, List, Set, Tuple

from common_parts import _assert_no_coplanar_faces, bone_ring_port
from organ_common import bounds, extend_materials, render_review, write_bbmodel

REPO = Path(__file__).resolve().parents[3]
ORGAN_ID = "spleen"
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "spleen.bbmodel"
EXPLODED_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/exploded/x07_spleen.png")
TMP_DIR = Path("/tmp/claude-1000/-home-serverkizuna-Code-Bong--agent-worktrees-model-b/e8024e15-89b9-4c94-bc09-9db5a4565345/scratchpad/spleen_parts")
VISIBLE_BOX = [2, 2, 2]

# 配色表「脾」主色 #6a2a3a；脉络用 common_parts 里已有的 tendon_fiber #c07868
PALETTE, MAT_UV = extend_materials({"spleen_main": ((106, 42, 58, 255), [16, 32, 32, 48])})

ROW_H = 2.0
BEAN_Y0 = 2.0
LAYERS = 14
HALF_W = 7  # 横截面半宽（x），14 条 1px 柱
HALF_D = 6.0  # 横截面半厚（z）
CX_AMPLITUDE = 4.0  # 中段相对两端向 +X 偏移
HILUM_LAYERS = (5, 8)  # 脾门凹口所在层（含），y 12..20
HILUM_DEPTH = 4  # 凹口深度 px（2 格）

VEIN_PROUD = 0.5

# 血管：3×3 截面，水平段从凹口面向 -X，竖直段在 x -10..-7
TUBE_SIZE = 3.0
TUBE_HALF = TUBE_SIZE / 2
RUN_X = (-10.0, -7.0)  # 拐角块 / 竖直段的 x 范围
FACE_X = 1.0  # 凹口面 x（= cx - 3）
TUBE_INTO_BODY = 0.5  # 水平段插进豆身的深度
INLET_Y = (16.5, 19.5)
OUTLET_Y = (12.5, 15.5)
INLET_TOP_Y = 29.5  # 入口竖直段顶端（环在这里）
OUTLET_BOTTOM_Y = 0.5  # 出口竖直段底端（环在 0..0.5）
COLLAR_HALF = 2.5
COLLAR_LEN = 1.5

# 脉络折线路点（px 坐标 (x, y)，格以左下角计），相邻路点之间补成 4 连通阶梯。
# 主干两路（上 / 下）→ 一级支 → 二级细枝。
VEIN_POLYLINES: List[List[Tuple[int, int]]] = [
    [(1, 16), (1, 19), (0, 22), (-1, 26), (-3, 29)],  # 上主干，贴凹面
    [(1, 19), (7, 21), (8, 24)],  # 上 一级支
    [(7, 21), (9, 20)],  # 上 二级
    [(0, 22), (5, 27), (6, 29)],  # 上 一级支
    [(5, 27), (7, 27)],  # 上 二级
    [(1, 16), (1, 12), (0, 10), (-1, 6), (-2, 4)],  # 下主干，贴凹面
    [(1, 12), (7, 11), (9, 8)],  # 下 一级支
    [(7, 11), (10, 12)],  # 下 二级
    [(0, 10), (5, 6), (6, 4)],  # 下 一级支
    [(5, 6), (8, 7)],  # 下 二级
]


def _cube(name: str, lo, hi, group: str, material: str) -> dict:
    return {"name": name, "from": [float(v) for v in lo], "to": [float(v) for v in hi], "group": group, "material": material}


def _layer_cx(k: int) -> int:
    """第 k 层的中心 x（取整）。"""
    return round(CX_AMPLITUDE * math.sin(math.pi * (k + 0.5) / LAYERS))


def _layer_columns(k: int) -> List[Tuple[int, float]]:
    """第 k 层的 1px 柱：[(柱左缘 x, 半深)]；脾门层把左侧 HILUM_DEPTH 条柱切掉。"""
    cx = _layer_cx(k)
    first = cx - HALF_W + (HILUM_DEPTH if HILUM_LAYERS[0] <= k <= HILUM_LAYERS[1] else 0)
    cols = []
    for x in range(first, cx + HALF_W):
        u = x + 0.5 - cx
        hd = HALF_D * math.sqrt(1 - (u / HALF_W) ** 2)
        cols.append((x, round(hd * 2) / 2))
    return cols


def _body_half_depth(x: int, y: int) -> float:
    """1×1px 格 (x, y) 所在柱的半深；格在豆身外返回 0。"""
    k = int((y - BEAN_Y0) // ROW_H)
    if not 0 <= k < LAYERS:
        return 0.0
    return dict(_layer_columns(k)).get(x, 0.0)


def _stair(c0: int, r0: int, c1: int, r1: int) -> Set[Tuple[int, int]]:
    """两点之间的 4 连通阶梯：沿大方向每步走一格，小方向按斜率进位，斜走的那一步补一个肘格。"""
    dc, dr = c1 - c0, r1 - r0
    steps = max(abs(dc), abs(dr))
    out = {(c0, r0)}
    prev = (c0, r0)
    for i in range(1, steps + 1):
        cur = (c0 + round(dc * i / steps), r0 + round(dr * i / steps))
        if cur[0] != prev[0] and cur[1] != prev[1]:
            out.add((cur[0], prev[1]))
        out.add(cur)
        prev = cur
    return out


def _vein_cells() -> Set[Tuple[int, int]]:
    """所有脉络 1px 格 (x, y)：折线补成 4 连通阶梯后取并集，再只留在豆身内的。"""
    cells: Set[Tuple[int, int]] = set()
    for line in VEIN_POLYLINES:
        for (x0, y0), (x1, y1) in zip(line, line[1:]):
            cells |= _stair(x0, y0, x1, y1)
    return {(x, y) for x, y in cells if _body_half_depth(x, y) > 0}


def _is_patch(x: int, k: int) -> bool:
    """#8a2a2a 斑：按 3×2 柱组取哈希，约 2/7 的组是斑。"""
    gx, gk = x // 3, k // 2
    return (gx * 37 + gk * 91 + gx * gk * 11) % 7 < 2


def part_01_body() -> List[dict]:
    cubes = []
    for k in range(LAYERS):
        y0 = BEAN_Y0 + k * ROW_H
        for x, hd in _layer_columns(k):
            material = "flesh_main" if _is_patch(x, k) else "spleen_main"
            cubes.append(_cube(f"spleen_k{k}_x{x}", (x, y0, -hd), (x + 1, y0 + ROW_H, hd), "spleen", material))
    return cubes


def part_02_veins() -> List[dict]:
    """脉络：1px 格薄片，贴在所在柱的前面（+Z）上，凸出 0.5px。"""
    cubes = []
    for x, y in sorted(_vein_cells(), key=lambda p: (p[1], p[0])):
        hd = _body_half_depth(x, y)
        cubes.append(_cube(f"vein_x{x}_y{y}", (x, y, hd), (x + 1, y + 1, hd + VEIN_PROUD), "veins", "tendon_fiber"))
    return cubes


def _collar(name: str, centre: Tuple[float, float, float], axis: str, group: str) -> dict:
    """5×5 肉箍，沿 axis（"x" 或 "y"）长 COLLAR_LEN，套在 3×3 管上。"""
    cx, cy, cz = centre
    hx, hy = (COLLAR_LEN / 2, COLLAR_HALF) if axis == "x" else (COLLAR_HALF, COLLAR_LEN / 2)
    return _cube(name, (cx - hx, cy - hy, cz - COLLAR_HALF), (cx + hx, cy + hy, cz + COLLAR_HALF), group, "flesh_main")


def _vessel(tag: str, y_range: Tuple[float, float], end_y: float, up: bool) -> List[dict]:
    """一根短弯管：水平段 → 拐角块 → 竖直段（3 段），两个肉箍，管口骨环。

    up=True 向上、管口朝上，竖直段顶端在 end_y；up=False 向下、管口朝下，竖直段底端在 end_y。
    """
    g = f"vessel_{tag}"
    y0, y1 = y_range
    run_x0, run_x1 = RUN_X
    z0, z1 = -TUBE_HALF, TUBE_HALF
    horizontal = _cube(f"{tag}_seg1", (run_x1, y0, z0), (FACE_X + TUBE_INTO_BODY, y1, z1), g, "tendon_tube")
    corner = _cube(f"{tag}_seg2", (run_x0, y0, z0), (run_x1, y1, z1), g, "tendon_tube")
    if up:
        vertical = _cube(f"{tag}_seg3", (run_x0, y1, z0), (run_x1, end_y, z1), g, "tendon_tube")
        collar_y = (y1 + end_y) / 2
        port_face = "up"
    else:
        vertical = _cube(f"{tag}_seg3", (run_x0, end_y, z0), (run_x1, y0, z1), g, "tendon_tube")
        collar_y = (y0 + end_y) / 2
        port_face = "down"
    mid_y = (y0 + y1) / 2
    collars = [
        # 两根管的水平肉箍错开 2px，免得两个 5×5 箍在 y≈16 处叠在一起
        _collar(f"{tag}_collar_h", ((run_x1 + FACE_X) / 2 + (-1.0 if up else 1.0), mid_y, 0.0), "x", g),
        _collar(f"{tag}_collar_v", ((run_x0 + run_x1) / 2, collar_y, 0.0), "y", g),
    ]
    port = bone_ring_port(center_x=(run_x0 + run_x1) / 2, center_y=end_y, center_z=0.0, face=port_face, group=g, pfx=f"{tag}_port")
    return [horizontal, corner, vertical] + collars + port


def part_03_inlet() -> List[dict]:
    return _vessel("inlet", INLET_Y, INLET_TOP_Y, up=True)


def part_04_outlet() -> List[dict]:
    return _vessel("outlet", OUTLET_Y, OUTLET_BOTTOM_Y, up=False)


PARTS = [
    ("spleen_body", part_01_body),
    ("veins", part_02_veins),
    ("inlet_vessel", part_03_inlet),
    ("outlet_vessel", part_04_outlet),
]


def all_cubes() -> List[dict]:
    cubes: List[dict] = []
    for _, build in PARTS:
        cubes += build()
    return cubes


def _components(cells: Set[Tuple[int, int]]) -> int:
    """4 连通分量个数。"""
    left = set(cells)
    count = 0
    while left:
        count += 1
        stack = [left.pop()]
        while stack:
            c, r = stack.pop()
            for nb in ((c + 1, r), (c - 1, r), (c, r + 1), (c, r - 1)):
                if nb in left:
                    left.remove(nb)
                    stack.append(nb)
    return count


def self_test() -> None:
    """共面 / 外包 / 脉络连续三道门禁，各自先注入缺陷确认拦得住；再核对豆身尺寸与凹口。"""
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print(f"  [OK] {len(cubes)} 个立方体 0 项共面冲突")

    try:
        _assert_no_coplanar_faces(cubes + [dict(cubes[0], name="inject_dup")])
    except AssertionError:
        print("  [OK] 共面门禁拦住了注入的重复立方体")
    else:
        raise RuntimeError("门禁失效：注入的共面缺陷没被拦住")

    lo, hi = bounds(cubes)
    assert lo[0] >= -16 and hi[0] <= 16 and lo[2] >= -16 and hi[2] <= 16 and lo[1] >= 0 and hi[1] <= 32, f"外包超出 32×32×32: lo={lo} hi={hi}"
    print(f"  [OK] 外包 x[{lo[0]},{hi[0]}] y[{lo[1]},{hi[1]}] z[{lo[2]},{hi[2]}] ⊂ 32×32×32")
    _, hi_bad = bounds(cubes + [_cube("inject_tall", (0, 0, 0), (1, 33, 1), "x", "flesh_main")])
    assert hi_bad[1] > 32, "外包门禁失效：注入的超高立方体没把包围盒顶出 32"
    print("  [OK] 外包门禁能识别超高缺陷")

    veins = _vein_cells()
    assert _components(veins) == 1, f"脉络应连成一整棵树，实际 {_components(veins)} 块"
    breaks = [cell for cell in veins if _components(veins - {cell}) > 1]
    assert breaks, "脉络连续门禁失效：整棵树里找不到任何一个拿掉就断开的格子"
    print(f"  [OK] 脉络 {len(veins)} 格连成 1 棵树；注入缺口（如拿掉 {breaks[0]}）后能识别为断开")

    body = part_01_body()
    ys = [c["from"][1] for c in body]
    assert min(ys) == BEAN_Y0 and max(c["to"][1] for c in body) == BEAN_Y0 + LAYERS * ROW_H, "豆身应是 y 2..30 共 14 层"
    assert max(c["to"][2] for c in body) == HALF_D, "豆身最厚处半厚应为 6（厚 12）"
    full_layers = [k for k in range(LAYERS) if not HILUM_LAYERS[0] <= k <= HILUM_LAYERS[1]]
    assert all(len(_layer_columns(k)) == 2 * HALF_W for k in full_layers), "非凹口层的宽度应都是 14"
    assert [_layer_cx(k) for k in (0, 13)] == [0, 0] and max(_layer_cx(k) for k in range(LAYERS)) == 4, "中心线两端 x=0、中段 +4"
    assert all(len(_layer_columns(k)) == 2 * HALF_W - HILUM_DEPTH for k in range(HILUM_LAYERS[0], HILUM_LAYERS[1] + 1)), "凹口层应各比整层少 4px"
    print("  [OK] 豆身：14 层、宽 14、厚 12，中心线弧形（两端 0、中段 +4），凹口 4 层 × 4px")


def main() -> None:
    parser = argparse.ArgumentParser(description="经脉工厂内景器官 o07 spleen 生成器")
    parser.add_argument("--self-test", action="store_true", help="只跑门禁")
    args = parser.parse_args()
    print("gen_spleen 自检…")
    self_test()
    if args.self_test:
        return
    path = write_bbmodel(BBMODEL_OUT, ORGAN_ID, all_cubes(), PALETTE, MAT_UV, VISIBLE_BOX)
    print(f"✓ bbmodel: {path}")
    out = render_review(ORGAN_ID, path, PARTS, EXPLODED_REF, PALETTE, MAT_UV, VISIBLE_BOX, TMP_DIR)
    print(f"✓ 审阅图: {out}")


if __name__ == "__main__":
    main()
