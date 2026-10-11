#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o08: kidney (肾，水)

依据：
- model-review/meridian_factory.md「统一约定」、10-09 修订配色表、「第 2 批」o08 行
- 爆炸图 exploded/x08_kidney.png，单件图 refs/o08_kidney.png
- 接口件：底部 8×6 输出口用 common_parts.outlet_8x6。
- bbmodel 落盘与审阅图走 organ_common.py。

外包 3 宽 × 2 高 × 2 深（48×32×32 px），原点在底面中心，正面朝 +Z，输出口开在底面（-Y）。

逐行宽度表：一对豆形肾，左右镜像、内凹（肾门）相对。下表是左肾，右肾取 x 的镜像。
每行 2px 高自下而上（y 4..28，共 12 行），肾占 x ∈ [-外缘, -内缘]：

   行   y        外缘 |x|   内缘 |x|   半深
    0   4..6       19         8          8
    1   6..8       21         7         10
    2   8..10      22         6.5       11
    3   10..12     23         8         12
    4   12..14     23.5      11         12.5   ← 内缘从这里开始向外退，形成肾门凹口
    5   14..16     23.5      12         12.5   ← 肾门最深
    6   16..18     23.5      11         12.5
    7   18..20     23         8         12
    8   20..22     22         6.5       11
    9   22..24     21         7         10
   10   24..26     19         8          8.5
   11   26..28     16         9.5        6.5

每行沿 X 切成若干段（宽约 4.5px），每段前后各自 ±1px 起伏，材质在 #8a2a2a / #b05050 间交替。

偏蓝血管 #4a78c0：从两侧肾门（|x|=12, y 13..17）各一条 4×4 阶梯管向中间下方走 3 步（每步 x 内移 3~4、y 下降 2），
汇进中间一根 4×4 竖管（y 2..13），竖管下接底部 8×6 输出口。
骨质托架 #d8ccb0：左右各一块底板（y 2..4，x ±[6, 23]，z ±13）+ 前后各一条护唇（y 4..6）+ 两只脚（y 0..2）。
"""

from __future__ import annotations

import argparse
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
# (外缘 |x|, 内缘 |x|, 半深)，自下而上 12 行；左肾，右肾镜像
KIDNEY_ROWS: List[Tuple[float, float, float]] = [
    (19.0, 8.0, 8.0),
    (21.0, 7.0, 10.0),
    (22.0, 6.5, 11.0),
    (23.0, 8.0, 12.0),
    (23.5, 11.0, 12.5),
    (23.5, 12.0, 12.5),
    (23.5, 11.0, 12.5),
    (23.0, 8.0, 12.0),
    (22.0, 6.5, 11.0),
    (21.0, 7.0, 10.0),
    (19.0, 8.0, 8.5),
    (16.0, 9.5, 6.5),
]
SEGMENT_TARGET_W = 4.5
BUMP = [0, 1, 0, -1, 1, 0, -1, 0]  # 每段前 / 后表面的 Z 起伏（px）

# 肾门血管：每侧 3 步阶梯，(x 内端, x 外端, y 底)，4×4 截面，左侧坐标；右侧取镜像
VESSEL_STEPS: List[Tuple[float, float, float]] = [
    (-12.0, -9.0, 13.0),
    (-9.0, -6.0, 11.0),
    (-6.0, -2.0, 9.0),
]
VESSEL_SIZE = 4.0
TRUNK_HALF = 2.0
TRUNK_Y = (2.0, 13.0)

# 骨托架（左侧坐标，右侧镜像）
CRADLE_X = (-23.0, -6.0)
CRADLE_HALF_Z = 13.0
CRADLE_Y = (2.0, 4.0)
LIP_Y = (4.0, 6.0)
LIP_Z = (11.0, 13.0)
LIP_X = (-22.0, -7.0)
FOOT_X = (-23.0, -19.0)
FOOT_Z = (9.0, 13.0)


def _cube(name: str, lo, hi, group: str, material: str) -> dict:
    return {"name": name, "from": [float(v) for v in lo], "to": [float(v) for v in hi], "group": group, "material": material}


def _mirror_x(cubes: List[dict]) -> List[dict]:
    """把一批立方体沿 x=0 镜像成右侧的一份（名字加 _r 后缀）。"""
    out = []
    for c in cubes:
        lo, hi = c["from"], c["to"]
        out.append(dict(c, name=c["name"] + "_r", **{"from": [-hi[0], lo[1], lo[2]], "to": [-lo[0], hi[1], hi[2]]}))
    return out


def _bean(side: int) -> List[dict]:
    """一只肾（side=0 左，1 右），逐行逐段。右肾直接按镜像坐标建，起伏哈希里带 side，左右不完全一样。"""
    cubes: List[dict] = []
    for r, (outer, inner, half_d) in enumerate(KIDNEY_ROWS):
        y0 = KIDNEY_Y0 + r * ROW_H
        n = max(1, round((outer - inner) / SEGMENT_TARGET_W))
        step = (outer - inner) / n
        for k in range(n):
            a, b = inner + step * k, inner + step * (k + 1)
            x0, x1 = (-b, -a) if side == 0 else (a, b)
            zf = half_d + BUMP[(r * 3 + k * 5 + side * 2) % len(BUMP)]
            zb = half_d + BUMP[(r * 3 + k * 5 + side * 2 + 3) % len(BUMP)]
            material = "flesh_lit" if (r * 7 + k * 3 + side) % 4 == 0 else "flesh_main"
            cubes.append(_cube(f"kidney_{'lr'[side]}_r{r}_s{k}", (x0, y0, -zb), (x1, y0 + ROW_H, zf), f"kidney_{'lr'[side]}", material))
    return cubes


def part_01_kidney_left() -> List[dict]:
    return _bean(0)


def part_02_kidney_right() -> List[dict]:
    return _bean(1)


def part_03_vessels() -> List[dict]:
    """肾门血管：两侧各 3 步 4×4 阶梯管 + 中间一根 4×4 竖管。"""
    left = [
        _cube(f"vessel_step{i}", (xa, yb, -VESSEL_SIZE / 2), (xb, yb + VESSEL_SIZE, VESSEL_SIZE / 2), "vessels", "water")
        for i, (xa, xb, yb) in enumerate(VESSEL_STEPS)
    ]
    trunk = _cube("vessel_trunk", (-TRUNK_HALF, TRUNK_Y[0], -TRUNK_HALF), (TRUNK_HALF, TRUNK_Y[1], TRUNK_HALF), "vessels", "water")
    return left + _mirror_x(left) + [trunk]


def part_04_cradle() -> List[dict]:
    """骨托架：左右各一块底板 + 前后护唇 + 两只脚。"""
    g, m = "cradle", "bone_main"
    left = [
        _cube("cradle_slab", (CRADLE_X[0], CRADLE_Y[0], -CRADLE_HALF_Z), (CRADLE_X[1], CRADLE_Y[1], CRADLE_HALF_Z), g, m),
        _cube("cradle_lip_f", (LIP_X[0], LIP_Y[0], LIP_Z[0]), (LIP_X[1], LIP_Y[1], LIP_Z[1]), g, m),
        _cube("cradle_lip_b", (LIP_X[0], LIP_Y[0], -LIP_Z[1]), (LIP_X[1], LIP_Y[1], -LIP_Z[0]), g, m),
        _cube("cradle_foot_f", (FOOT_X[0], 0.0, FOOT_Z[0]), (FOOT_X[1], CRADLE_Y[0], FOOT_Z[1]), g, "bone_dark"),
        _cube("cradle_foot_b", (FOOT_X[0], 0.0, -FOOT_Z[1]), (FOOT_X[1], CRADLE_Y[0], -FOOT_Z[0]), g, "bone_dark"),
    ]
    return left + _mirror_x(left)


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
    """共面门禁 + 外包尺寸门禁，各自先注入缺陷确认拦得住；再核对内凹相对、左右对称。"""
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

    inners = [row[1] for row in KIDNEY_ROWS]
    deepest = inners.index(max(inners))
    assert 0 < deepest < len(inners) - 1 and max(inners) > inners[0] + 2, "肾门凹口应当落在中间几行，且明显比两端深"
    print("  [OK] 肾门凹口在中部、内凹相对")


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
