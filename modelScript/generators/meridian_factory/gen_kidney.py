#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o08: kidney (肾，水)

依据：
- model-review/meridian_factory.md「统一约定」、10-09 修订配色表、「第 2 批」o08 行、
  「o08 kidney 调度审第 1 次」的逐行宽度表
- 爆炸图 exploded/x08_kidney.png，单件图 refs/o08_kidney.png
- 接口件：底部 8×6 输出口用 common_parts.outlet_8x6。
- bbmodel 落盘与审阅图走 organ_common.py。

外包 3 宽 × 2 高 × 2 深（48×32×32 px），原点在底面中心，正面朝 +Z，输出口开在底面（-Y）。

逐行宽度表：每个肾单独按豆形做，12 行 × 2px = 24px 高（y 4..28，托架在它下面 y 0..6），
下表是左肾，右肾 x 取镜像。「内侧」指朝另一个肾的那一侧，内侧基线固定在 |x| = 6：

   行   y        外侧边界 w   内缘 |x|   行宽    半深
    0   4..6        7          6         7        8
    1   6..8       10          6        10       10
    2   8..10      12          6        12       11.5
    3   10..12     13          6        13       13
    4   12..14     14         10        10       14     ← 肾门：内缘向外凹 4px，共 4 行（y 12..20）
    5   14..16     14         10        10       14
    6   16..18     14         10        10       14
    7   18..20     14         10        10       14
    8   20..22     13          6        13       13
    9   22..24     12          6        12       11.5
   10   24..26     10          6        10       10
   11   26..28      7          6         7        8

外缘 |x| = 6 + w，所以外侧是一条 7→14→7 的弧、内侧是直线加一个 4px 深的凹口，合起来是豆形。
每行沿 X 切成约 2px 一段，半深按 sin 曲线向两侧收（中央满、两侧约 60%），俯视也是弧。
材质 #8a2a2a 主、#b05050 斑；肾门凹口外沿一圈 #5a1a1a。

肾门血管 #4a78c0（3×3）：从凹口里 4 步斜向下走，汇进中间 4×4 主干（y 2..11.5），主干下接 8×6 输出口。
骨托（#d8ccb0，2px 粗）：每个肾前后各一个 U 形（底条 + 外侧臂 + 内侧臂），
左右两组之间前后各连一根横骨，U 下面有脚。
"""

from __future__ import annotations

import argparse
import math
from pathlib import Path
from typing import List, Tuple

from common_parts import _assert_no_coplanar_faces, outlet_8x6
from organ_common import bounds, extend_materials, render_review, write_bbmodel

REPO = Path(__file__).resolve().parents[3]
ORGAN_ID = "kidney"
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "kidney.bbmodel"
EXPLODED_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/exploded/x08_kidney.png")
TMP_DIR = Path("/tmp/claude-1000/-home-serverkizuna-Code-Bong--agent-worktrees-model-b/e8024e15-89b9-4c94-bc09-9db5a4565345/scratchpad/kidney_parts")
VISIBLE_BOX = [3, 2, 2]

# 配色表「真元载荷·水」#4a78c0，用作肾门血管
PALETTE, MAT_UV = extend_materials({"water": ((74, 120, 192, 255), [16, 32, 32, 48])})

ROW_H = 2.0
KIDNEY_Y0 = 4.0
INNER_BASE = 6.0  # 内侧基线 |x|
HILUM_DEPTH = 4.0  # 肾门凹口深度
# (外侧边界 w, 是否肾门行, 半深)，自下而上 12 行；左肾，右肾镜像
KIDNEY_ROWS: List[Tuple[float, bool, float]] = [
    (7.0, False, 8.0),
    (10.0, False, 10.0),
    (12.0, False, 11.5),
    (13.0, False, 13.0),
    (14.0, True, 14.0),
    (14.0, True, 14.0),
    (14.0, True, 14.0),
    (14.0, True, 14.0),
    (13.0, False, 13.0),
    (12.0, False, 11.5),
    (10.0, False, 10.0),
    (7.0, False, 8.0),
]
SEGMENT_TARGET_W = 2.0
EDGE_THICKNESS = 0.6  # 行宽两侧边缘的半深占中央的比例

# 肾门周围一圈 #5a1a1a：外 7×7、孔 3×3、厚 2、凸出凹口面 0.5
HILUM_RING_Y = (12.5, 19.5)
HILUM_RING_HALF_Z = 3.5
HILUM_HOLE_Y = (14.5, 17.5)
HILUM_HOLE_HALF_Z = 1.5
HILUM_RING_PROUD = 0.5

# 肾门血管：左侧 4 步 3×3 阶梯管，(x 外端, x 内端, y 底)，右侧镜像
VESSEL_STEPS: List[Tuple[float, float, float]] = [
    (-10.0, -8.0, 14.5),
    (-8.0, -6.0, 12.5),
    (-6.0, -4.0, 10.5),
    (-4.0, -2.0, 8.5),
]
VESSEL_SIZE = 3.0
TRUNK_HALF = 2.0
TRUNK_Y = (2.0, 11.5)

# 骨托（左侧坐标，右侧镜像）：每个肾前后各一个 U（XY 平面，厚 2px）
U_BAR_X = (-15.0, -4.0)
U_BAR_Y = (2.0, 4.0)
U_ARM_Y = (4.0, 6.0)
U_OUTER_ARM_X = (-15.0, -13.0)
U_INNER_ARM_X = (-6.0, -4.0)
U_Z = (6.0, 8.0)  # 前 U 的 z 范围，后 U 取负
FOOT_Y = (0.0, 2.0)
CROSS_X = (-4.0, 4.0)  # 左右两组 U 之间的横骨


def _cube(name: str, lo, hi, group: str, material: str) -> dict:
    return {"name": name, "from": [float(v) for v in lo], "to": [float(v) for v in hi], "group": group, "material": material}


def _mirror_x(cubes: List[dict]) -> List[dict]:
    """把一批立方体沿 x=0 镜像成右侧的一份（名字加 _r 后缀）。"""
    out = []
    for c in cubes:
        lo, hi = c["from"], c["to"]
        out.append(dict(c, name=c["name"] + "_r", **{"from": [-hi[0], lo[1], lo[2]], "to": [-lo[0], hi[1], hi[2]]}))
    return out


def _row_edges(row: int) -> Tuple[float, float]:
    """第 row 行的 (内缘 |x|, 外缘 |x|)。"""
    width, hilum, _ = KIDNEY_ROWS[row]
    return (INNER_BASE + HILUM_DEPTH if hilum else INNER_BASE), INNER_BASE + width


def _bean(side: int) -> List[dict]:
    """一只肾（side=0 左，1 右），逐行逐段；右肾按镜像坐标直接建，斑块哈希带 side，左右不完全一样。"""
    cubes: List[dict] = []
    for r, (_, _, half_d) in enumerate(KIDNEY_ROWS):
        inner, outer = _row_edges(r)
        y0 = KIDNEY_Y0 + r * ROW_H
        n = max(2, round((outer - inner) / SEGMENT_TARGET_W))
        step = (outer - inner) / n
        for k in range(n):
            a, b = inner + step * k, inner + step * (k + 1)
            u = (k + 0.5) / n  # 0 内缘 → 1 外缘
            hd = half_d * (EDGE_THICKNESS + (1 - EDGE_THICKNESS) * math.sin(math.pi * u))
            x0, x1 = (-b, -a) if side == 0 else (a, b)
            material = "flesh_lit" if (r * 7 + k * 3 + side * 2) % 5 == 0 else "flesh_main"
            cubes.append(_cube(f"kidney_{'lr'[side]}_r{r}_s{k}", (x0, y0, -hd), (x1, y0 + ROW_H, hd), f"kidney_{'lr'[side]}", material))
    return cubes


def _hilum_ring() -> List[dict]:
    """左肾肾门凹口面（x = -(INNER_BASE+HILUM_DEPTH)）上凸出的一圈 #5a1a1a，中间 3×3 孔让血管进去。"""
    face = -(INNER_BASE + HILUM_DEPTH)
    x0, x1 = face, face + HILUM_RING_PROUD
    (y0, y3), (y1, y2) = HILUM_RING_Y, HILUM_HOLE_Y
    hz, hh = HILUM_RING_HALF_Z, HILUM_HOLE_HALF_Z
    g, m = "kidney_l", "flesh_dark"
    return [
        _cube("hilum_ring_top", (x0, y2, -hz), (x1, y3, hz), g, m),
        _cube("hilum_ring_bot", (x0, y0, -hz), (x1, y1, hz), g, m),
        _cube("hilum_ring_f", (x0, y1, hh), (x1, y2, hz), g, m),
        _cube("hilum_ring_b", (x0, y1, -hz), (x1, y2, -hh), g, m),
    ]


def part_01_kidney_left() -> List[dict]:
    return _bean(0) + _hilum_ring()


def part_02_kidney_right() -> List[dict]:
    return _bean(1) + _mirror_x(_hilum_ring())


def part_03_vessels() -> List[dict]:
    """肾门血管：两侧各 4 步 3×3 阶梯管，从凹口里斜向下汇进中间 4×4 主干。"""
    h = VESSEL_SIZE / 2
    left = [
        _cube(f"vessel_step{i}", (xa, yb, -h), (xb, yb + VESSEL_SIZE, h), "vessels", "water")
        for i, (xa, xb, yb) in enumerate(VESSEL_STEPS)
    ]
    trunk = _cube("vessel_trunk", (-TRUNK_HALF, TRUNK_Y[0], -TRUNK_HALF), (TRUNK_HALF, TRUNK_Y[1], TRUNK_HALF), "vessels", "water")
    return left + _mirror_x(left) + [trunk]


def part_04_cradle() -> List[dict]:
    """骨托：每个肾前后各一个 U（底条 + 两条短臂 + 脚），左右之间前后各连一根横骨。"""
    g, m = "cradle", "bone_main"
    left: List[dict] = []
    for tag, (z0, z1) in (("f", U_Z), ("b", (-U_Z[1], -U_Z[0]))):
        left += [
            _cube(f"u_{tag}_bar", (U_BAR_X[0], U_BAR_Y[0], z0), (U_BAR_X[1], U_BAR_Y[1], z1), g, m),
            _cube(f"u_{tag}_arm_out", (U_OUTER_ARM_X[0], U_ARM_Y[0], z0), (U_OUTER_ARM_X[1], U_ARM_Y[1], z1), g, m),
            _cube(f"u_{tag}_arm_in", (U_INNER_ARM_X[0], U_ARM_Y[0], z0), (U_INNER_ARM_X[1], U_ARM_Y[1], z1), g, m),
            _cube(f"u_{tag}_foot_out", (U_OUTER_ARM_X[0], FOOT_Y[0], z0), (U_OUTER_ARM_X[1], FOOT_Y[1], z1), g, "bone_dark"),
            _cube(f"u_{tag}_foot_in", (U_INNER_ARM_X[0], FOOT_Y[0], z0), (U_INNER_ARM_X[1], FOOT_Y[1], z1), g, "bone_dark"),
        ]
    cross = [
        _cube("cross_f", (CROSS_X[0], U_BAR_Y[0], U_Z[0]), (CROSS_X[1], U_BAR_Y[1], U_Z[1]), g, m),
        _cube("cross_b", (CROSS_X[0], U_BAR_Y[0], -U_Z[1]), (CROSS_X[1], U_BAR_Y[1], -U_Z[0]), g, m),
    ]
    return left + _mirror_x(left) + cross


def part_05_outlet() -> List[dict]:
    """底部 8×6 统一截面输出口（common_parts.outlet_8x6，朝下，管套高 2px，底边贴地）。"""
    return outlet_8x6(center_x=0.0, center_y=2.0, center_z=0.0, facing="-y", length=2.0, group="outlet", pfx="outlet")


PARTS = [
    ("kidney_left", part_01_kidney_left),
    ("kidney_right", part_02_kidney_right),
    ("vessels", part_03_vessels),
    ("cradle", part_04_cradle),
    ("outlet", part_05_outlet),
]


def all_cubes() -> List[dict]:
    cubes: List[dict] = []
    for _, build in PARTS:
        cubes += build()
    return cubes


def self_test() -> None:
    """共面门禁 + 外包尺寸门禁，各自先注入缺陷确认拦得住；再核对豆形（外弧 + 4 行 4px 肾门）。"""
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print(f"  [OK] {len(cubes)} 个立方体 0 项共面冲突")

    lo, hi = bounds(cubes)
    assert lo[0] >= -24 and hi[0] <= 24 and lo[2] >= -16 and hi[2] <= 16 and lo[1] >= 0 and hi[1] <= 32, f"外包超出 48×32×32: lo={lo} hi={hi}"
    print(f"  [OK] 外包 x[{lo[0]},{hi[0]}] y[{lo[1]},{hi[1]}] z[{lo[2]},{hi[2]}] ⊂ 48×32×32")

    try:
        _assert_no_coplanar_faces(cubes + [dict(cubes[0], name="inject_dup")])
    except AssertionError:
        print("  [OK] 共面门禁拦住了注入的重复立方体")
    else:
        raise RuntimeError("门禁失效：注入的共面缺陷没被拦住")

    _, hi_bad = bounds(cubes + [_cube("inject_tall", (0, 0, 0), (1, 33, 1), "x", "flesh_main")])
    assert hi_bad[1] > 32, "外包门禁失效：注入的超高立方体没把包围盒顶出 32"
    print("  [OK] 外包门禁能识别超高缺陷")

    outers = [w for w, _, _ in KIDNEY_ROWS]
    assert outers == [7, 10, 12, 13, 14, 14, 14, 14, 13, 12, 10, 7], "外侧边界应是 7,10,12,13,14,14,14,14,13,12,10,7"
    hilum_rows = [i for i, (_, h, _) in enumerate(KIDNEY_ROWS) if h]
    assert hilum_rows == [4, 5, 6, 7] and all(_row_edges(i)[0] - INNER_BASE == HILUM_DEPTH for i in hilum_rows), "肾门应是中段 4 行、各凹 4px"
    print("  [OK] 豆形：外侧弧线表 + 中段 4 行凹 4px 的肾门")


def main() -> None:
    parser = argparse.ArgumentParser(description="经脉工厂内景器官 o08 kidney 生成器")
    parser.add_argument("--self-test", action="store_true", help="只跑门禁")
    args = parser.parse_args()
    print("gen_kidney 自检…")
    self_test()
    if args.self_test:
        return
    path = write_bbmodel(BBMODEL_OUT, ORGAN_ID, all_cubes(), PALETTE, MAT_UV, VISIBLE_BOX)
    print(f"✓ bbmodel: {path}")
    out = render_review(ORGAN_ID, path, PARTS, EXPLODED_REF, PALETTE, MAT_UV, VISIBLE_BOX, TMP_DIR)
    print(f"✓ 审阅图: {out}")


if __name__ == "__main__":
    main()
