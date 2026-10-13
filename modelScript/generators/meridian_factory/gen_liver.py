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
import re
from pathlib import Path
from typing import Dict, List, Tuple

from common_parts import _assert_no_coplanar_faces, bone_ring_port, outlet_8x6
from organ_common import bounds, extend_materials, path_cells, render_review, surface_vein_cubes, thicken, write_bbmodel

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

# 血管树（几何）：胆管树 #c07868（贴图加 1px #e8bca8 高光）+ 左右叶各一棵绿色叶脉树（#5aa060）。
# 都从肝门（正面中部，y≈10）出发：主干 2×2，分 2 级、每级一分二，末梢 1×1，贴肝前表面连续走；
# 相邻格表面高差超过 0.5px 的地方在较低那格上垫半格，血管不断。
# 格 (ix, iy) 占 x [ix+X_OFF, ix+X_OFF+1]、y [iy+Y_OFF, iy+Y_OFF+1]：x 错开 0.5（避开肝列界 4 的倍数），
# y 错开（肝行界在偶数），主干行和各树之间再错开，免得侧面共面。
X_OFF = 0.5
Y_OFF_BRANCH = 0.25
Y_OFF_LEAF = 0.4
Y_OFF_TRUNK = 0.5
SINK = 3.0                  # 背面埋进肝体的深度，要大于相邻行 ±1 起伏之差
SINK_STEP = 0.1             # 各级再错开一点，免得背面共面
TRUNK_PROUD = 2.0           # 主干 2×2：2 宽、凸出 2px
BRANCH_PROUD = 1.0          # 支脉 / 末梢 1×1
LEAF_TRUNK_PROUD = 1.8      # 叶脉主干比胆管主干略矮，两者在肝门重叠处才不共面

# 胆管树（左侧路点 (ix, iy)，右侧镜像）：主干是 x ±1 的竖直行，再分两级
DUCT_TRUNK_ROWS = (10, 16)  # iy 10..15，y 10.5..16.5
DUCT_B1 = [(-2, 16), (-5, 20)]
DUCT_B2 = [[(-5, 20), (-10, 24)], [(-5, 20), (-4, 24)]]

# 叶脉树：主干是 2×2 的斜阶梯，一级两支，每支再分两个末梢
LEAF_TREES = {
    "l": {
        "trunk": [(-2, 12), (-8, 15)],
        "b1": [[(-8, 15), (-13, 19)], [(-8, 15), (-12, 10)]],
        "b2": [[(-13, 19), (-18, 21)], [(-13, 19), (-16, 15)], [(-12, 10), (-18, 11)], [(-12, 10), (-15, 6)]],
    },
    "r": {
        "trunk": [(1, 12), (7, 14)],
        "b1": [[(7, 14), (12, 17)], [(7, 14), (11, 8)]],
        "b2": [[(12, 17), (17, 14)], [(12, 17), (14, 18)], [(11, 8), (17, 9)], [(11, 8), (15, 5)]],
    },
}

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


# ─────────────────────────────────────────────────────────────────────────────
# 部件
# ─────────────────────────────────────────────────────────────────────────────

def part_01_body() -> List[dict]:
    """肝体：12 列楔形，左厚右薄。"""
    return [_cube(s["tag"], (s["x0"], s["y0"], -s["zb"]), (s["x1"], s["y1"], s["zf"]), "liver_body", s["mat"]) for s in SURFACE]




def _mirror_cells(cells: set) -> set:
    """左侧格镜像到右侧：格 (ix, iy) 占 x [ix+0.5, ix+1.5]，镜像后占 [-ix-1.5, -ix-0.5]，即格号 -ix-2。"""
    return {(-ix - 2, iy) for ix, iy in cells}


def _cells(paths) -> set:
    cells: set = set()
    for wp in paths:
        cells |= path_cells(wp)
    return cells


def part_02_leaf_trees() -> List[dict]:
    """左右叶各一棵绿色叶脉树：2×2 斜主干 → 一级两支 → 末梢 1×1，木色 #5aa060。"""
    cubes: List[dict] = []
    for tag, spec in LEAF_TREES.items():
        trunk = thicken(path_cells(spec["trunk"]))
        b1 = _cells(spec["b1"]) - trunk
        b2 = _cells(spec["b2"]) - trunk - b1
        levels = [
            (f"leaf_{tag}_trunk", trunk, SINK, "wood", LEAF_TRUNK_PROUD),
            (f"leaf_{tag}_b1", b1, SINK + SINK_STEP, "wood", BRANCH_PROUD),
            (f"leaf_{tag}_b2", b2, SINK + 2 * SINK_STEP, "wood", BRANCH_PROUD),
        ]
        cubes += surface_vein_cubes(levels, _front_z, X_OFF, Y_OFF_LEAF, "leaf_tree")
    return cubes


def part_03_duct_tree() -> List[dict]:
    """胆管树 #c07868：肝门（正面中部 y≈10）长出 2 宽主干，往上到 y≈16 分两支，每支再分两个末梢。"""
    g = "duct_tree"
    cubes: List[dict] = []
    # 主干：x ±1 的竖直行，每 1px 高一块，前表面贴肝体 + 2px
    for iy in range(*DUCT_TRUNK_ROWS):
        y0, y1 = iy + Y_OFF_TRUNK, iy + Y_OFF_TRUNK + 1.0
        zf = _front_z(-1.0, 1.0, y0, y1)
        cubes.append(_cube(f"duct_trunk_{iy}", (-1.0, y0, zf + TRUNK_PROUD - SINK - 0.3), (1.0, y1, zf + TRUNK_PROUD), g, "tendon_fiber"))
    b1 = _cells([DUCT_B1])
    b2 = _cells(DUCT_B2) - b1
    for tag, flip in (("l", lambda c: c), ("r", _mirror_cells)):
        levels = [
            (f"duct_b1{tag}", flip(b1), SINK, "tendon_fiber", BRANCH_PROUD),
            (f"duct_b2{tag}", flip(b2), SINK + SINK_STEP, "tendon_fiber", BRANCH_PROUD),
        ]
        cubes += surface_vein_cubes(levels, _front_z, X_OFF, Y_OFF_BRANCH, g)
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
    ("leaf_trees", part_02_leaf_trees),
    ("duct_tree", part_03_duct_tree),
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

    check_trees_and_outlet(cubes)
    print("  [OK] 胆管树 #c07868 + 左右叶脉树（木色）：主干 2 宽、支脉 1×1、各分 2 级、整棵连成一块、相邻格台阶 ≤0.5px；输出口 +Z 向 8×6")
    duct = [c for c in cubes if c["group"] == "duct_tree"]
    leaf = [c for c in cubes if c["group"] == "leaf_tree"]
    rest = [c for c in cubes if c["group"] not in ("duct_tree", "leaf_tree")]
    # 把左侧一级支中段（iy 17、18）整段拿掉，末梢侧就和主干断开
    gap = [c for c in duct if c["name"].startswith("duct_b1l_") and re.match(r".*_(17|18)(_half)?$", c["name"])]
    assert gap, "注入用的中段格不存在"
    cases = (
        ("拿掉胆管一级支中段", rest + leaf + [c for c in duct if c not in gap]),
        ("去掉过台阶半格", rest + duct + [c for c in leaf if not c["name"].endswith("_half")]),
        ("胆管改回骨色", rest + leaf + [dict(c, material="bone_main") for c in duct]),
        ("叶脉树少一棵", rest + duct + [c for c in leaf if not c["name"].startswith("leaf_r_")]),
        ("末梢变 2 宽", rest + duct + [dict(c, to=[c["to"][0] + 1.0, c["to"][1], c["to"][2]]) if c["name"].startswith("leaf_l_b2_") and not c["name"].endswith("_half") else c for c in leaf]),
        ("输出口被压扁", [dict(c, to=[c["to"][0], c["from"][1] + 1.0, c["to"][2]]) if c["name"] == "outlet_lining" else c for c in cubes]),
    )
    for name, bad in cases:
        try:
            check_trees_and_outlet(bad)
        except AssertionError:
            print(f"  [OK] 注入「{name}」被拦截")
        else:
            raise RuntimeError(f"门禁失效：注入「{name}」没被拦住")


def _touch(a: dict, b: dict) -> bool:
    return all(a["from"][i] <= b["to"][i] + 1e-6 and b["from"][i] <= a["to"][i] + 1e-6 for i in range(3))


def _connected(tree: List[dict]) -> bool:
    seen, stack = {0}, [0]
    while stack:
        i = stack.pop()
        for j in range(len(tree)):
            if j not in seen and _touch(tree[i], tree[j]):
                seen.add(j)
                stack.append(j)
    return len(seen) == len(tree)


_CELL_NAME = re.compile(r"^(?P<prefix>.+)_(?P<ix>-?\d+)_(?P<iy>-?\d+)(?P<half>_half)?$")


def _check_steps(tree: List[dict], label: str) -> None:
    """同一棵树里，相邻 1×1 格（含补的半格）的前表面高差 ≤0.5px。"""
    tops: Dict[str, float] = {}
    cells: Dict[Tuple[int, int], str] = {}
    for c in tree:
        m = _CELL_NAME.match(c["name"])
        if not m or "trunk" in m["prefix"] and m["prefix"].startswith("duct"):
            continue
        key = f"{m['prefix']}_{m['ix']}_{m['iy']}"
        tops[key] = max(tops.get(key, 0.0), c["to"][2])
        if not m["half"]:
            cells[(int(m["ix"]), int(m["iy"]))] = key
    for (ix, iy), key in cells.items():
        for nb in ((ix + 1, iy), (ix, iy + 1)):
            other = cells.get(nb)
            if other is not None:
                assert abs(tops[key] - tops[other]) <= 0.5 + 1e-6, f"{label}: {key} 与 {other} 前表面高差 {abs(tops[key] - tops[other]):.2f} > 0.5"


def check_trees_and_outlet(cubes: List[dict]) -> None:
    """胆管树 / 叶脉树门禁 + 输出口门禁。"""
    duct = [c for c in cubes if c["group"] == "duct_tree"]
    assert duct and all(c["material"] == "tendon_fiber" for c in duct), "胆管树必须是 #c07868（tendon_fiber）"
    trunk = [c for c in duct if c["name"].startswith("duct_trunk_")]
    assert trunk and all(abs(c["to"][0] - c["from"][0] - 2.0) < 1e-6 and abs(c["from"][0] + c["to"][0]) < 1e-6 for c in trunk), "胆管主干应是居中的 2 宽"
    for tag in ("b1l", "b1r", "b2l", "b2r"):
        assert any(c["name"].startswith(f"duct_{tag}_") for c in duct), f"胆管缺 {tag}"
    for c in duct:
        if c["name"].startswith(("duct_b1", "duct_b2")) and not c["name"].endswith("_half"):
            assert abs(c["to"][0] - c["from"][0] - 1.0) < 1e-6 and abs(c["to"][1] - c["from"][1] - 1.0) < 1e-6, f"{c['name']} 支脉应是 1×1"
    assert _connected(duct), "胆管树断成多块"
    _check_steps(duct, "胆管树")

    leaf = [c for c in cubes if c["group"] == "leaf_tree"]
    for tag in ("l", "r"):
        tree = [c for c in leaf if c["name"].startswith(f"leaf_{tag}_")]
        assert tree and all(c["material"] == "wood" for c in tree), f"叶脉树 {tag} 缺失或不是木色"
        for level in ("trunk", "b1", "b2"):
            assert any(c["name"].startswith(f"leaf_{tag}_{level}_") for c in tree), f"叶脉树 {tag} 缺 {level}"
        for c in tree:
            if c["name"].startswith((f"leaf_{tag}_b1", f"leaf_{tag}_b2")) and not c["name"].endswith("_half"):
                assert abs(c["to"][0] - c["from"][0] - 1.0) < 1e-6 and abs(c["to"][1] - c["from"][1] - 1.0) < 1e-6, f"{c['name']} 应是 1×1"
        assert _connected(tree), f"叶脉树 {tag} 断成多块"
        _check_steps(tree, f"叶脉树 {tag}")

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
