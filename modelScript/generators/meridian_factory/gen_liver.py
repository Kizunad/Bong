#!/usr/bin/env python3
"""经脉工厂内景器官生成器 —— o06: liver (肝，木)

依据：
- model-review/meridian_factory.md「统一约定」、10-09 修订配色表、「第 2 批」o06 行
- 爆炸图 exploded/x06_liver.png，单件图 refs/o06_liver.png
- 接口件：8×6 输出口 common_parts.outlet_8x6，顶部入口管的骨环口 common_parts.bone_ring_port。
- bbmodel 落盘与审阅图走 organ_common.py。

外包 3 宽 × 2 高 × 2 深（48×32×32 px），原点在底面中心，正面朝 +Z，输出口开在底面（-Y）。

逐列宽度表（楔形，左厚右薄）。肝沿 X 切成 12 列、每列 4px 宽；每列自己的底 / 顶 / 半深如下，
再按 2px 一行往上堆，首末两行半深各收 1.5px 做圆角，中间各行前后 ±1px 起伏：

   列    x          底    顶    半深
    0   -24..-20     6    20    11     ← 左端，圆钝
    1   -20..-16     4    24    13.5
    2   -16..-12     2    26    14
    3   -12..-8      2    26    14
    4    -8..-4      2    26    13
    5    -4..0       2    26    12
    6     0..4       2    26    11
    7     4..8       2    24    10     ← 往右开始变薄、变矮
    8     8..12      4    22     9
    9    12..16      4    20     8
   10    16..20      6    16     6.5
   11    20..24      6    12     5     ← 右端最薄

主色 #5a1a1a（偏暗），#8a2a2a 斑块；表面 3 条 #5aa060 叶脉（木）；正面底部中间一根 #d8ccb0 胆管；
顶部两根小入口管（4×4，高 5.5，#8a2a2a，顶端 common 骨环口）；底部 8×6 输出口。
"""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import List, Tuple

from common_parts import _assert_no_coplanar_faces, bone_ring_port, outlet_8x6
from organ_common import bounds, extend_materials, render_review, write_bbmodel

REPO = Path(__file__).resolve().parents[3]
ORGAN_ID = "liver"
BBMODEL_OUT = REPO / "modelScript" / "models" / "meridian_factory" / "liver.bbmodel"
EXPLODED_REF = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/exploded/x06_liver.png")
TMP_DIR = Path("/tmp/claude-1000/-home-serverkizuna-Code-Bong--agent-worktrees-model-b/e8024e15-89b9-4c94-bc09-9db5a4565345/scratchpad/liver_parts")
VISIBLE_BOX = [3, 2, 2]

# 配色表「真元载荷·木」#5aa060，用作叶脉
PALETTE, MAT_UV = extend_materials({"wood": ((90, 160, 96, 255), [16, 32, 32, 48])})

ROW_H = 2.0
COLUMN_W = 4.0
# (x0, 底 y, 顶 y, 半深)，12 列，x0 从 -24 起每列 +4
COLUMNS: List[Tuple[float, float, float, float]] = [
    (-24.0, 6.0, 20.0, 11.0),
    (-20.0, 4.0, 24.0, 13.5),
    (-16.0, 2.0, 26.0, 14.0),
    (-12.0, 2.0, 26.0, 14.0),
    (-8.0, 2.0, 26.0, 13.0),
    (-4.0, 2.0, 26.0, 12.0),
    (0.0, 2.0, 26.0, 11.0),
    (4.0, 2.0, 24.0, 10.0),
    (8.0, 4.0, 22.0, 9.0),
    (12.0, 4.0, 20.0, 8.0),
    (16.0, 6.0, 16.0, 6.5),
    (20.0, 6.0, 12.0, 5.0),
]
CAP_INSET = 1.5  # 每列首末行半深收进去的量
BUMP = [0, 1, 0, -1, 1, 0, -1, 0]  # 中间各行前 / 后表面的 Z 起伏（px）

# 叶脉：(折线 (x, y), 浮起 px, 分块范围)。沿 x 每 2px 一块，边界落在半格上以避开列边界（4 的倍数）。
LEAF_VEINS = {
    "left_main": ([(-22.0, 9.0), (-18.0, 13.0), (-14.0, 17.0), (-10.0, 21.0)], 0.5, (-22.5, -9.5)),
    "left_branch": ([(-18.0, 13.0), (-16.0, 9.0), (-15.0, 5.0)], 0.4, (-17.5, -13.5)),
    "right_main": ([(5.0, 18.0), (9.0, 16.0), (13.0, 14.0), (17.0, 11.0), (21.0, 9.0)], 0.5, (4.5, 21.5)),
}
VEIN_HALF_W = 1.3
VEIN_Y_SHIFT = 0.07  # 叶脉整体上抬一点，免得上下沿落在行格线（偶数）上与肝体共面
SINK = 2.5  # 叶脉背面埋进体内的深度，要大于相邻行 ±1 起伏之差
SINK_STEP = 0.1  # 每条叶脉再错开一点，免得背面共面

# 胆管：#e8bca8 肉质管，从肝正面中部伸出，主干往上到 y≈17 后分两叉各朝左上 / 右上。
# 每 1px 高一节，y 整体错开 0.25，避开肝体行格线（偶数）共面。
DUCT_Y_OFFSET = 0.25
DUCT_TRUNK_Y = (10.0, 17.0)   # 主干高度范围（加偏移）
DUCT_TRUNK_W = 4.2            # 主干宽（x）
DUCT_BRANCH_STEPS = 6         # 两叉各 6 节，y 17 → 23（再高右叉就出了肝体）
DUCT_BRANCH_W = 2.6
DUCT_BRANCH_DX = 1.3          # 每上升 1px，叉向外偏的 px
DUCT_T = 2.5                  # 管厚（z），再厚前凸会超出 32 深的外包
DUCT_EMBED = 0.8              # 管后表面埋进肝体前表面的深度

OUTLET_CENTER_X = 0.1         # 管套左右沿 x=±4 正好落在肝列界上，整体错开 0.1
OUTLET_CENTER_Y = 5.05        # 开口 y 2.05..8.05；错开 0.05 避开行格线共面
OUTLET_Z0 = 8.0
OUTLET_LEN = 8.0

# 顶部入口管：x 中心，底 y；4×4 截面，顶端 y 到 32
INLET_X_CENTERS = (-8.0, 0.0)
INLET_BASE_Y = 26.0
INLET_TOP_Y = 32.0
INLET_HALF = 2.0
RING_H = 0.5  # common_parts.bone_ring_port 的环高


def _cube(name: str, lo, hi, group: str, material: str) -> dict:
    return {"name": name, "from": [float(v) for v in lo], "to": [float(v) for v in hi], "group": group, "material": material}


def _build_surface() -> List[dict]:
    """肝体全部行块，每块 dict(x0,x1,y0,y1,zf,zb,mat,tag)。"""
    rows: List[dict] = []
    for k, (x0, y_bot, y_top, half_d) in enumerate(COLUMNS):
        n = int(round((y_top - y_bot) / ROW_H))
        for r in range(n):
            y0 = y_bot + r * ROW_H
            capped = r in (0, n - 1)
            zf = half_d - CAP_INSET if capped else half_d + BUMP[(k * 3 + r * 5) % len(BUMP)]
            zb = half_d - CAP_INSET if capped else half_d + BUMP[(k * 3 + r * 5 + 3) % len(BUMP)]
            patch = (k * 7 + r * 3) % 5 in (0, 1)
            rows.append({"x0": x0, "x1": x0 + COLUMN_W, "y0": y0, "y1": y0 + ROW_H, "zf": zf, "zb": zb,
                         "mat": "flesh_main" if patch else "flesh_dark", "tag": f"liver_c{k}_r{r}"})
    return rows


SURFACE = _build_surface()


def _front_z(x0: float, x1: float, y0: float, y1: float) -> float:
    """区域内肝体最靠前的 Z。"""
    hits = [s["zf"] for s in SURFACE if s["x1"] > x0 and s["x0"] < x1 and s["y1"] > y0 and s["y0"] < y1]
    if not hits:
        raise ValueError(f"区域 x[{x0},{x1}] y[{y0},{y1}] 没有落在肝体表面上")
    return max(hits)


def _interp_x(points: List[Tuple[float, float]], x: float) -> float:
    """折线（x 单调）在 x 处的 y。"""
    pts = sorted(points)
    if x <= pts[0][0]:
        return pts[0][1]
    for (xa, ya), (xb, yb) in zip(pts, pts[1:]):
        if xa <= x <= xb:
            return ya + (yb - ya) * (x - xa) / (xb - xa)
    return pts[-1][1]


# ─────────────────────────────────────────────────────────────────────────────
# 部件
# ─────────────────────────────────────────────────────────────────────────────

def part_01_body() -> List[dict]:
    """肝体：12 列楔形，左厚右薄。"""
    return [_cube(s["tag"], (s["x0"], s["y0"], -s["zb"]), (s["x1"], s["y1"], s["zf"]), "liver_body", s["mat"]) for s in SURFACE]


def part_02_leaf_veins() -> List[dict]:
    """3 条 #5aa060 叶脉（左叶主脉 + 一条分支，右叶一条）。"""
    cubes: List[dict] = []
    for index, (name, (points, proud, (start, end))) in enumerate(LEAF_VEINS.items()):
        sink = SINK + index * SINK_STEP
        t = start
        while t < end - 1e-6:
            t1 = min(t + ROW_H, end)
            yc = _interp_x(points, (t + t1) / 2) + VEIN_Y_SHIFT
            y0, y1 = yc - VEIN_HALF_W, yc + VEIN_HALF_W
            zf = _front_z(t, t1, y0, y1)
            cubes.append(_cube(f"vein_{name}_{t:g}", (t, y0, zf - sink), (t1, y1, zf + proud), "leaf_veins", "wood"))
            t = t1
    return cubes


def part_03_bile_duct() -> List[dict]:
    """胆管：#e8bca8 肉质管，从肝中部正面伸出，先一根主干，再分成左右两叉斜着往上。"""
    g = "bile_duct"
    cubes: List[dict] = []

    def segment(name: str, cx: float, width: float, y0: float, y1: float) -> None:
        x0, x1 = cx - width / 2, cx + width / 2
        zf = _front_z(x0, x1, y0, y1)
        z0 = zf - DUCT_EMBED
        cubes.append(_cube(name, (x0, y0, z0), (x1, y1, z0 + DUCT_T), g, "tendon_highlight"))
        cubes.append(_cube(f"{name}_hl", (cx - 0.3, y0, z0 + DUCT_T), (cx + 0.3, y1, z0 + DUCT_T + 0.3), g, "qi_glow"))

    y = DUCT_TRUNK_Y[0] + DUCT_Y_OFFSET
    i = 0
    while y < DUCT_TRUNK_Y[1] + DUCT_Y_OFFSET - 1e-6:
        segment(f"duct_trunk_{i}", 0.0, DUCT_TRUNK_W, y, y + 1.0)
        y += 1.0
        i += 1

    y_fork = DUCT_TRUNK_Y[1] + DUCT_Y_OFFSET
    for k in range(DUCT_BRANCH_STEPS):
        cx = DUCT_BRANCH_W / 2 + 0.05 + DUCT_BRANCH_DX * k   # 起点错开 0.05，避开 x=0 的肝列界
        for side, sx in (("l", -1.0), ("r", 1.0)):
            segment(f"duct_{side}_{k}", sx * cx, DUCT_BRANCH_W, y_fork + k, y_fork + k + 1.0)
    return cubes


def part_04_inlets() -> List[dict]:
    """顶部两根 4×4 小入口管，顶端 common_parts 的骨环口。"""
    cubes: List[dict] = []
    tube_top = INLET_TOP_Y - RING_H
    for i, cx in enumerate(INLET_X_CENTERS):
        cubes.append(_cube(f"inlet_{i}_tube", (cx - INLET_HALF, INLET_BASE_Y, -INLET_HALF), (cx + INLET_HALF, tube_top, INLET_HALF), "inlets", "flesh_main"))
        cubes += bone_ring_port(center_x=cx, center_y=tube_top, center_z=0.0, face="up", group="inlets", pfx=f"inlet_{i}_ring")
    return cubes


def part_05_outlet() -> List[dict]:
    """正面下方 8×6 统一截面输出口：标准 outlet_8x6 套管朝 +Z，开口底边离地 2，从肝体前壁里伸出
    （原来朝下只有 2px 高，太扁）。"""
    return outlet_8x6(center_x=OUTLET_CENTER_X, center_y=OUTLET_CENTER_Y, center_z=OUTLET_Z0, facing="+z", length=OUTLET_LEN, group="outlet", pfx="outlet")


PARTS = [
    ("body", part_01_body),
    ("leaf_veins", part_02_leaf_veins),
    ("bile_duct", part_03_bile_duct),
    ("inlets", part_04_inlets),
    ("outlet", part_05_outlet),
]


def all_cubes() -> List[dict]:
    cubes: List[dict] = []
    for _, build in PARTS:
        cubes += build()
    return cubes


def self_test() -> None:
    """共面门禁 + 外包尺寸门禁，各自先注入缺陷确认拦得住。"""
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print(f"  [OK] {len(cubes)} 个立方体 0 项共面冲突")

    lo, hi = bounds(cubes)
    assert lo[0] >= -24 and hi[0] <= 24 and lo[2] >= -16 and hi[2] <= 16 and lo[1] >= 0 and hi[1] <= 32, f"外包超出 48×32×32: lo={lo} hi={hi}"
    print(f"  [OK] 外包 x[{lo[0]},{hi[0]}] y[{lo[1]},{hi[1]}] z[{lo[2]},{hi[2]}] ⊂ 48×32×32")

    defect = cubes + [dict(cubes[0], name="inject_dup")]
    try:
        _assert_no_coplanar_faces(defect)
    except AssertionError:
        print("  [OK] 共面门禁拦住了注入的重复立方体")
    else:
        raise RuntimeError("门禁失效：注入的共面缺陷没被拦住")

    _, hi_bad = bounds(cubes + [_cube("inject_tall", (0, 0, 0), (1, 33, 1), "x", "flesh_main")])
    assert hi_bad[1] > 32, "外包门禁失效：注入的超高立方体没把包围盒顶出 32"
    print("  [OK] 外包门禁能识别超高缺陷")

    left, right = COLUMNS[0], COLUMNS[-1]
    thick = [c[3] for c in COLUMNS]
    assert thick.index(max(thick)) < len(thick) // 2 and right[3] < left[3], "楔形方向不对：应当左厚右薄"
    print("  [OK] 楔形左厚右薄")

    check_duct_and_outlet(cubes)
    print("  [OK] 胆管是 #e8bca8 肉管、主干居中后分两叉；输出口是 +Z 向 8×6 标准套管")
    no_fork = [c for c in cubes if not c["name"].startswith("duct_l_")]
    for name, bad in (("胆管只剩一叉", no_fork),
                      ("胆管改回骨色", [dict(c, material="bone_main") if c["name"].startswith("duct_") else c for c in cubes]),
                      ("输出口被压扁", [dict(c, to=[c["to"][0], c["from"][1] + 1.0, c["to"][2]]) if c["name"] == "outlet_lining" else c for c in cubes])):
        try:
            check_duct_and_outlet(bad)
        except AssertionError:
            print(f"  [OK] 注入「{name}」被拦截")
        else:
            raise RuntimeError(f"门禁失效：注入「{name}」没被拦住")


def check_duct_and_outlet(cubes: List[dict]) -> None:
    """胆管：材质 #e8bca8、有主干和左右两叉、两叉向外张开；输出口：内衬 8 宽 × 6 高，底边离地约 2。"""
    duct = [c for c in cubes if c["name"].startswith("duct_") and not c["name"].endswith("_hl")]
    assert duct and all(c["material"] == "tendon_highlight" for c in duct), "胆管必须是 #e8bca8 肉质管"
    trunk = [c for c in duct if c["name"].startswith("duct_trunk_")]
    left = sorted((c for c in duct if c["name"].startswith("duct_l_")), key=lambda c: c["from"][1])
    right = sorted((c for c in duct if c["name"].startswith("duct_r_")), key=lambda c: c["from"][1])
    assert trunk and left and right, "胆管应有主干和左右两叉"
    assert all(abs(c["from"][0] + c["to"][0]) < 0.2 for c in trunk), "主干应在肝正中"
    assert (left[-1]["from"][0] + left[-1]["to"][0]) / 2 < -4 < 4 < (right[-1]["from"][0] + right[-1]["to"][0]) / 2, "两叉没有向外张开"
    lining = next((c for c in cubes if c["name"] == "outlet_lining"), None)
    assert lining is not None, "缺输出口"
    assert abs(lining["to"][0] - lining["from"][0] - 8.0) < 1e-6 and abs(lining["to"][1] - lining["from"][1] - 6.0) < 1e-6, "输出口内衬必须是 8 宽 × 6 高"
    assert abs(lining["from"][1] - 2.0) < 0.2, f"输出口底边应离地 2: {lining['from'][1]}"


def main() -> None:
    parser = argparse.ArgumentParser(description="经脉工厂内景器官 o06 liver 生成器")
    parser.add_argument("--self-test", action="store_true", help="只跑门禁")
    args = parser.parse_args()
    print("gen_liver 自检…")
    self_test()
    if args.self_test:
        return
    path = write_bbmodel(BBMODEL_OUT, ORGAN_ID, all_cubes(), PALETTE, MAT_UV, VISIBLE_BOX)
    print(f"✓ bbmodel: {path}")
    out = render_review(ORGAN_ID, path, PARTS, EXPLODED_REF, PALETTE, MAT_UV, VISIBLE_BOX, TMP_DIR)
    print(f"✓ 审阅图: {out}")


if __name__ == "__main__":
    main()
