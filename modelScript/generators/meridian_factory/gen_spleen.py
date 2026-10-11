#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o07: spleen (脾，土)

依据：
- model-review/meridian_factory.md「统一约定」、配色表、「第 2 批」o07 行
- 爆炸图 exploded/x07_spleen.png，单件图 refs/o07_spleen.png
- 接口件：底部 8×6 输出口用 common_parts.outlet_8x6；bbmodel 落盘与审阅图走 organ_common.py。

外包 2 × 2 × 2（32×32×32 px），原点在底面中心，正面朝 +Z，输出口开在底面（-Y）。
单个弯豆形侧躺：凹口朝 -X（左），上下两个叶尖向左探出，右侧是一条大弧。

逐行宽度表：15 行 × 2px = 30px 高（y 2..32，输出口套管在 y 0..2），
沿 X 按全局 2px 网格切段（行首尾不足 2px 的截短）：

   行   y        左缘 x   右缘 x   行宽   半深
    0   2..4      -6        8       14     8
    1   4..6     -10       11       21    10
    2   6..8     -12       13       25    11.5
    3   8..10    -14       14       28    12.5
    4   10..12   -14       15       29    13      ← 下叶
    5   12..14   -12       16       28    13
    6   14..16    -6       16       22    13      ← 凹口（脾门）5 行 x = -6，比叶尖浅 8px
    7   16..18    -6       16       22    13
    8   18..20    -6       16       22    13
    9   20..22    -6       16       22    13
   10   22..24    -6       16       22    13
   11   24..26   -11       16       27    13      ← 上叶
   12   26..28   -13       15       28    12.5
   13   28..30   -13       13       26    11
   14   30..32    -9       10       19     8.5

每段半深 = 行半深 × (0.45 + 0.55 × sin(π·u))，u 为该段中心在本行内的相对位置（0 左缘 → 1 右缘），
所以俯视和侧视都是圆弧。主色 #6a2a3a，#8a2a2a 成片的斑（2×2 格一组）。

叶脉 #b08a40：1×1px 格的路径，前面（+Z）薄薄凸出 0.5px，贴在所在本体格的表面上。
每条分支都是 4 连通，细支 1px 宽，脾门处主干 2px 宽。从脾门出发一根主干，
在脾门右侧分成上下两支，再各自分出向右、向叶尖的小支，照 o01 支气管那样逐格连续。

入口管：凹口面中段 y 18..22 一根 4×4 筋管向 -X 伸出，管口一圈骨环（6×6 外廓、4×4 内孔），
凹口面上有一圈 #5a1a1a 脾门环（外 8×8，凸出 0.5）。
"""

from __future__ import annotations

import argparse
import math
from pathlib import Path
from typing import Dict, List, Set, Tuple

from common_parts import _assert_no_coplanar_faces, outlet_8x6
from organ_common import bounds, extend_materials, render_review, write_bbmodel

REPO = Path(__file__).resolve().parents[3]
ORGAN_ID = "spleen"
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "spleen.bbmodel"
EXPLODED_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/exploded/x07_spleen.png")
TMP_DIR = Path("/tmp/claude-1000/-home-serverkizuna-Code-Bong--agent-worktrees-model-b/e8024e15-89b9-4c94-bc09-9db5a4565345/scratchpad/spleen_parts")
VISIBLE_BOX = [2, 2, 2]

# 配色表「脾」主色 #6a2a3a；「土」细脉 #b08a40
PALETTE, MAT_UV = extend_materials({
    "spleen_main": ((106, 42, 58, 255), [16, 32, 32, 48]),
    "earth_vein": ((176, 138, 64, 255), [32, 32, 48, 48]),
})

ROW_H = 2.0
BEAN_Y0 = 2.0
CELL_W = 2.0
# (左缘 x, 右缘 x, 半深)，自下而上 15 行
SPLEEN_ROWS: List[Tuple[float, float, float]] = [
    (-6.0, 8.0, 8.0),
    (-10.0, 11.0, 10.0),
    (-12.0, 13.0, 11.5),
    (-14.0, 14.0, 12.5),
    (-14.0, 15.0, 13.0),
    (-12.0, 16.0, 13.0),
    (-6.0, 16.0, 13.0),
    (-6.0, 16.0, 13.0),
    (-6.0, 16.0, 13.0),
    (-6.0, 16.0, 13.0),
    (-6.0, 16.0, 13.0),
    (-11.0, 16.0, 13.0),
    (-13.0, 15.0, 12.5),
    (-13.0, 13.0, 11.0),
    (-9.0, 10.0, 8.5),
]
HILUM_ROWS = (6, 10)  # 凹口所在行（含）
HILUM_FACE_X = -6.0
EDGE_THICKNESS = 0.45  # 行宽两端半深占中央的比例

VEIN_PROUD = 0.5  # 叶脉薄片凸出本体表面
VEIN_Y0 = BEAN_Y0

# 叶脉折线路点（px 坐标 (x, y)，一格 = 1×1px，格以左下角计）。
# 每对相邻路点之间补成 4 连通的阶梯，细线 1px 宽，只有主干 2px。
VEIN_POLYLINES: List[List[Tuple[int, int]]] = [
    [(-6, 19), (2, 19)],  # 脾门主干（入口管对着 y 18..22），2px 宽
    [(-6, 20), (2, 20)],
    [(2, 19), (2, 23), (4, 27), (4, 30)],  # 上支
    [(2, 23), (-6, 29)],  # 上支 → 上叶左尖
    [(4, 27), (12, 29)],  # 上支 → 右上
    [(2, 21), (10, 22), (14, 24)],  # 上支 → 右
    [(10, 22), (12, 26)],
    [(2, 19), (8, 16), (14, 13)],  # 主干继续向右
    [(2, 19), (2, 15), (0, 11), (0, 7), (0, 4)],  # 下支
    [(2, 15), (8, 13), (14, 10)],  # 下支 → 右下
    [(0, 11), (-8, 8), (-12, 8)],  # 下支 → 下叶左尖
    [(0, 7), (6, 4)],  # 下支 → 底部右
]

# 入口管（4×4，凹口面中段）与脾门环
TUBE_Y = (18.0, 22.0)
TUBE_HALF_Z = 2.0
RING_Y = (16.0, 24.0)
RING_HALF_Z = 4.0
RING_PROUD = 0.5
TUBE_END_X = -13.5
PORT_OUTER_X = -15.0  # 管口骨环最外面
PORT_Y = (17.0, 23.0)  # 骨环外廓 6×6
PORT_HALF_Z = 3.0


def _cube(name: str, lo, hi, group: str, material: str) -> dict:
    return {"name": name, "from": [float(v) for v in lo], "to": [float(v) for v in hi], "group": group, "material": material}


def _row_cells(r: int) -> List[Tuple[int, float, float, float]]:
    """第 r 行的所有格子：[(列 c, x0, x1, 半深)]，按全局 2px 网格切段，行首尾截短。"""
    left, right, half_d = SPLEEN_ROWS[r]
    cells = []
    c = math.floor(left / CELL_W)
    while c * CELL_W < right:
        x0, x1 = max(left, c * CELL_W), min(right, (c + 1) * CELL_W)
        u = ((x0 + x1) / 2 - left) / (right - left)
        hd = half_d * (EDGE_THICKNESS + (1 - EDGE_THICKNESS) * math.sin(math.pi * u))
        cells.append((c, x0, x1, round(hd * 2) / 2))  # 半深取到 0.5px，避免出现碎小数
        c += 1
    return cells


def _body_half_depth(x: int, y: int) -> float:
    """1×1px 格 (x, y) 所在本体格的半深；格在豆形外返回 0。"""
    r = int((y - BEAN_Y0) // ROW_H)
    if not 0 <= r < len(SPLEEN_ROWS):
        return 0.0
    for c, x0, x1, hd in _row_cells(r):
        if x0 <= x and x + 1 <= x1:
            return hd
    return 0.0


def _vein_cells() -> Set[Tuple[int, int]]:
    """所有叶脉 1px 格 (x, y)：折线补成 4 连通阶梯后取并集，再只留在豆形本体内的。"""
    cells: Set[Tuple[int, int]] = set()
    for line in VEIN_POLYLINES:
        for (c0, r0), (c1, r1) in zip(line, line[1:]):
            cells |= _stair(c0, r0, c1, r1)
    return {(x, y) for x, y in cells if _body_half_depth(x, y) > 0}


def _stair(c0: int, r0: int, c1: int, r1: int) -> Set[Tuple[int, int]]:
    """两点之间的 4 连通阶梯：沿大方向每步走一格，小方向按斜率进位，进位时先走水平再走竖直。"""
    dc, dr = c1 - c0, r1 - r0
    steps = max(abs(dc), abs(dr))
    out = {(c0, r0)}
    prev = (c0, r0)
    for i in range(1, steps + 1):
        cur = (c0 + round(dc * i / steps), r0 + round(dr * i / steps))
        if cur[0] != prev[0] and cur[1] != prev[1]:
            out.add((cur[0], prev[1]))  # 斜着走的那一步补一个肘格
        out.add(cur)
        prev = cur
    return out


def _is_patch(c: int, r: int) -> bool:
    """#8a2a2a 斑：按 2×2 格一组取哈希，约 2/7 的组是斑。"""
    gc, gr = c // 2, r // 2
    return (gc * 37 + gr * 91 + gc * gr * 11) % 7 < 2


def part_01_body() -> List[dict]:
    cubes = []
    for r in range(len(SPLEEN_ROWS)):
        y0 = BEAN_Y0 + r * ROW_H
        for c, x0, x1, hd in _row_cells(r):
            material = "flesh_main" if _is_patch(c, r) else "spleen_main"
            cubes.append(_cube(f"spleen_r{r}_c{c}", (x0, y0, -hd), (x1, y0 + ROW_H, hd), "spleen", material))
    return cubes


def part_02_veins() -> List[dict]:
    """叶脉：1px 格薄片，贴在所在本体格的前面（+Z）上，凸出 0.5px。"""
    cubes = []
    for x, y in sorted(_vein_cells(), key=lambda p: (p[1], p[0])):
        hd = _body_half_depth(x, y)
        cubes.append(_cube(f"vein_x{x}_y{y}", (x, y, hd), (x + 1, y + 1, hd + VEIN_PROUD), "veins", "earth_vein"))
    return cubes


def part_03_inlet() -> List[dict]:
    """脾门环 + 入口筋管 + 管口骨环。"""
    g = "inlet"
    ring_x0, ring_x1 = HILUM_FACE_X - RING_PROUD, HILUM_FACE_X
    (ry0, ry1), rz = RING_Y, RING_HALF_Z
    ty0, ty1 = TUBE_Y
    tz = TUBE_HALF_Z
    cubes = [
        _cube("hilum_ring_top", (ring_x0, ty1, -rz), (ring_x1, ry1, rz), g, "flesh_dark"),
        _cube("hilum_ring_bot", (ring_x0, ry0, -rz), (ring_x1, ty0, rz), g, "flesh_dark"),
        _cube("hilum_ring_f", (ring_x0, ty0, tz), (ring_x1, ty1, rz), g, "flesh_dark"),
        _cube("hilum_ring_b", (ring_x0, ty0, -rz), (ring_x1, ty1, -tz), g, "flesh_dark"),
        _cube("tube_main", (TUBE_END_X, ty0, -tz), (ring_x0, ty1, tz), g, "tendon_tube"),
        _cube("tube_hl_top", (TUBE_END_X + 0.1, ty1 - 0.4, tz - 0.1), (ring_x0 - 0.1, ty1 - 0.1, tz + 0.1), g, "tendon_highlight"),
    ]
    py0, py1 = PORT_Y
    pz = PORT_HALF_Z
    px0, px1 = PORT_OUTER_X, TUBE_END_X
    cubes += [
        _cube("port_top", (px0, ty1, -pz), (px1, py1, pz), g, "bone_main"),
        _cube("port_bot", (px0, py0, -pz), (px1, ty0, pz), g, "bone_main"),
        _cube("port_f", (px0, ty0, tz), (px1, ty1, pz), g, "bone_main"),
        _cube("port_b", (px0, ty0, -pz), (px1, ty1, -tz), g, "bone_main"),
        _cube("port_hole", (px0 + 0.05, ty0, -tz), (px0 + 0.4, ty1, tz), g, "flesh_dark"),
    ]
    return cubes


def part_04_outlet() -> List[dict]:
    """底部 8×6 统一截面输出口（common_parts.outlet_8x6，朝下，管套高 2px，底边贴地）。"""
    return outlet_8x6(center_x=0.0, center_y=2.0, center_z=0.0, facing="-y", length=2.0, group="outlet", pfx="outlet")


PARTS = [
    ("spleen_body", part_01_body),
    ("veins", part_02_veins),
    ("inlet", part_03_inlet),
    ("outlet", part_04_outlet),
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
    """共面 / 外包 / 叶脉连续三道门禁，各自先注入缺陷确认拦得住；再核对豆形凹口。"""
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
    assert _components(veins) == 1, f"叶脉应连成一整棵树，实际 {_components(veins)} 块"
    # 注入缺陷：拿掉任何一个单点瓶颈格，连通门禁必须报出断开
    breaks = [cell for cell in veins if _components(veins - {cell}) > 1]
    assert breaks, "叶脉连续门禁失效：整棵树里找不到任何一个拿掉就断开的格子"
    print(f"  [OK] 叶脉 {len(veins)} 格连成 1 棵树；注入缺口（如拿掉 {breaks[0]}）后能识别为断开")

    hilum = range(HILUM_ROWS[0], HILUM_ROWS[1] + 1)
    assert all(SPLEEN_ROWS[i][0] == HILUM_FACE_X for i in hilum), "凹口 5 行左缘应都在 x = -6"
    assert SPLEEN_ROWS[4][0] - HILUM_FACE_X <= -8 and SPLEEN_ROWS[12][0] - HILUM_FACE_X <= -7, "上下叶应比凹口面向左多出 ≥ 7px"
    print("  [OK] 弯豆形：凹口 5 行，上下叶向左探出")


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
