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
import re
import math
from pathlib import Path
from typing import Dict, List, Set, Tuple

from common_parts import _assert_no_coplanar_faces, bone_ring_port, outlet_8x6
from organ_common import bounds, extend_materials, path_cells, render_review, surface_vein_cubes, thicken, write_bbmodel

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

# 血管树（几何）：从脾门（凹口面 x≈1、y≈16）出发，主干 2×2，分 2 级、每级一分二，末梢 1×1，贴脾体前表面（+Z）连续走。
# 格 (ix, iy)：x∈[ix+X_OFF, ix+X_OFF+1]、y∈[iy+Y_OFF, iy+Y_OFF+1]；x 错开 0.4（避开柱界整数和侧面补平块的 0.5 界）、y 错开 0.25，避开豆身层界。
# 相邻格表面高差超过 0.5px 的地方，在较低那格上垫半格（见 organ_common.surface_vein_cubes）。
X_OFF = 0.4
Y_OFF = 0.25
Y_OFF_TRUNK = Y_OFF           # 主干格和支脉格是互不重叠的两组，用同一个偏移，交接处只是贴边
TRUNK_PROUD = 1.8             # 主干 2×2 凸出
BRANCH_PROUD = 1.0            # 支脉 / 末梢 1×1 凸出
SINK = 3.0                    # 背面埋进豆身的深度，要大于相邻柱前后起伏之差
SINK_STEP = 0.1
TREE_TRUNK = [(1, 15), (4, 16)]
TREE_B1 = {"u": [(4, 16), (6, 21)], "d": [(4, 16), (5, 10)]}
TREE_B2 = {
    "u1": [(6, 21), (8, 26)],
    "u2": [(6, 21), (8, 20)],
    "d1": [(5, 10), (7, 6)],
    "d2": [(5, 10), (9, 12)],
}

# 入口血管：4×4 截面，水平段从凹口面向 -X，竖直段在 x -11..-7
# 输出口：底面中心标准 outlet_8x6（朝下），不再有出口血管
TUBE_SIZE = 4.0
TUBE_HALF = TUBE_SIZE / 2
RUN_X = (-11.0, -7.0)  # 拐角块 / 竖直段的 x 范围
FACE_X = 1.0  # 凹口面 x（= cx - 3）
TUBE_INTO_BODY = 0.5  # 水平段插进豆身的深度
INLET_Y = (15.75, 19.75)  # 4 高，整根落在脾门凹口层 y 12..20 内，且避开层界 y=16 / 20
INLET_TOP_Y = 29.5  # 入口竖直段顶端（环在这里）
COLLAR_HALF = 3.0  # 6×6 肉箍套 4×4 管
COLLAR_LEN = 1.5



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


def _front_z(x0: float, x1: float, y0: float, y1: float) -> float:
    """区域 [x0,x1]×[y0,y1] 内豆身最靠前的 z（逐 1px 柱取半深最大值）；区域不在豆身上则抛 ValueError。"""
    best = 0.0
    for x in range(math.floor(x0), math.ceil(x1)):
        for y in range(math.floor(y0), math.ceil(y1)):
            best = max(best, _body_half_depth(x, y))
    if best <= 0:
        raise ValueError(f"区域 x[{x0},{x1}] y[{y0},{y1}] 不在豆身上")
    return best


def part_02_vessel_tree() -> List[dict]:
    """血管树 #c07868（贴图加 #e8bca8 高光）：2×2 主干 → 上 / 下两支一级 → 每支再分两个 1×1 末梢。"""
    g = "vessel_tree"
    trunk = thicken(path_cells(TREE_TRUNK))
    b1 = {k: path_cells(wp) for k, wp in TREE_B1.items()}
    b2 = {k: path_cells(wp) for k, wp in TREE_B2.items()}
    used = set(trunk)
    levels = [("vein_trunk", trunk, SINK, "tendon_fiber", TRUNK_PROUD, Y_OFF_TRUNK)]
    for i, (k, cells) in enumerate(b1.items()):
        cells = cells - used
        used |= cells
        levels.append((f"vein_b1{k}", cells, SINK + SINK_STEP, "tendon_fiber", BRANCH_PROUD, Y_OFF))
    for k, cells in b2.items():
        cells = cells - used
        used |= cells
        levels.append((f"vein_b2{k}", cells, SINK + 2 * SINK_STEP, "tendon_fiber", BRANCH_PROUD, Y_OFF))
    return surface_vein_cubes(levels, _front_z, X_OFF, Y_OFF, g)


def part_05_step_fill() -> List[dict]:
    """豆身侧面台阶补平：相邻两层边缘错开 1px（x 向）或 0.5~1px（z 向）的地方，
    在凸出较少的那一层靠台阶的半层高处补一块 0.5px 的半格，把一级台阶拆成两级半格，沿豆身绕一圈。
    脉络格上不补（脉络贴在柱前面，补块会和它共面）。
    """
    cubes: List[dict] = []
    cols = [dict(_layer_columns(k)) for k in range(LAYERS)]
    ext = []
    for k in range(LAYERS):
        xs = sorted(cols[k])
        ext.append((xs[0], xs[-1] + 1))

    def add(name, lo, hi):
        cubes.append(_cube(name, lo, hi, "spleen_fill", "spleen_main"))

    for k in range(LAYERS - 1):
        top = BEAN_Y0 + (k + 1) * ROW_H
        (l0, r0), (l1, r1) = ext[k], ext[k + 1]
        # 右缘（+X）：下层 k 与上层 k+1 的右缘差 1px
        if r1 - r0 == 1:
            hd = cols[k][r0 - 1]
            add(f"fill_r_k{k}", (r0, top - 1, -hd), (r0 + 0.5, top, hd))
        elif r1 - r0 == -1:
            hd = cols[k + 1][r1 - 1]
            add(f"fill_r_k{k}", (r1, top, -hd), (r1 + 0.5, top + 1, hd))
        # 左缘（-X）：脾门层的左缘整段切进去 4px，那里是凹口，不补
        if l1 - l0 == -1:
            hd = cols[k][l0]
            add(f"fill_l_k{k}", (l0 - 0.5, top - 1, -hd), (l0, top, hd))
        elif l1 - l0 == 1:
            hd = cols[k + 1][l1]
            add(f"fill_l_k{k}", (l1 - 0.5, top, -hd), (l1, top + 1, hd))
        # 前后表面（±Z）：同一列在相邻两层半深不同
        for x in sorted(set(cols[k]) & set(cols[k + 1])):
            hd_a, hd_b = cols[k][x], cols[k + 1][x]
            if hd_a == hd_b:
                continue
            if hd_a < hd_b:
                y0, hd = top - 1, hd_a
            else:
                y0, hd = top, hd_b
            add(f"fill_f_k{k}_x{x}", (x, y0, hd), (x + 1, y0 + 1, hd + 0.5))
            add(f"fill_b_k{k}_x{x}", (x, y0, -hd - 0.5), (x + 1, y0 + 1, -hd))
    return cubes


def _collar(name: str, centre: Tuple[float, float, float], axis: str, group: str) -> dict:
    """6×6 肉箍，沿 axis（"x" 或 "y"）长 COLLAR_LEN，套在 4×4 管上。"""
    cx, cy, cz = centre
    hx, hy = (COLLAR_LEN / 2, COLLAR_HALF) if axis == "x" else (COLLAR_HALF, COLLAR_LEN / 2)
    return _cube(name, (cx - hx, cy - hy, cz - COLLAR_HALF), (cx + hx, cy + hy, cz + COLLAR_HALF), group, "flesh_main")


def part_03_inlet() -> List[dict]:
    """入口血管（凹面那根）：4×4 短弯管，水平段 → 拐角块 → 竖直段，两个 6×6 肉箍，管口朝上骨环。"""
    g = "vessel_inlet"
    y0, y1 = INLET_Y
    run_x0, run_x1 = RUN_X
    z0, z1 = -TUBE_HALF, TUBE_HALF
    horizontal = _cube("inlet_seg1", (run_x1, y0, z0), (FACE_X + TUBE_INTO_BODY, y1, z1), g, "tendon_tube")
    corner = _cube("inlet_seg2", (run_x0, y0, z0), (run_x1, y1, z1), g, "tendon_tube")
    vertical = _cube("inlet_seg3", (run_x0, y1, z0), (run_x1, INLET_TOP_Y, z1), g, "tendon_tube")
    collars = [
        _collar("inlet_collar_h", ((run_x1 + FACE_X) / 2 - 1.0, (y0 + y1) / 2, 0.0), "x", g),
        _collar("inlet_collar_v", ((run_x0 + run_x1) / 2, (y1 + INLET_TOP_Y) / 2, 0.0), "y", g),
    ]
    port = bone_ring_port(center_x=(run_x0 + run_x1) / 2, center_y=INLET_TOP_Y, center_z=0.0, face="up", group=g, pfx="inlet_port")
    return [horizontal, corner, vertical] + collars + port


def part_04_outlet() -> List[dict]:
    """底面中心的标准 8×6 输出口（common_parts.outlet_8x6，朝下，套管 y 0..2）。"""
    return outlet_8x6(center_x=0.0, center_y=2.0, center_z=0.0, facing="-y", length=2.0, group="outlet", pfx="outlet")


PARTS = [
    ("spleen_body", part_01_body),
    ("step_fill", part_05_step_fill),
    ("vessel_tree", part_02_vessel_tree),
    ("inlet_vessel", part_03_inlet),
    ("outlet", part_04_outlet),
]


def all_cubes() -> List[dict]:
    cubes: List[dict] = []
    for _, build in PARTS:
        cubes += build()
    return cubes


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

    check_vessel_tree(cubes)
    print("  [OK] 血管树：2×2 主干、上下两支一级、四个 1×1 末梢，整棵连成一块、相邻格台阶 ≤0.5px")
    tree = [c for c in cubes if c["group"] == "vessel_tree"]
    rest = [c for c in cubes if c["group"] != "vessel_tree"]
    mid = [c for c in tree if c["name"].startswith("vein_b1u_") and re.match(r".*_(18|19)(_half)?$", c["name"])]
    assert mid, "注入用的中段格不存在"
    for name, bad in (
        ("拿掉上支中段", rest + [c for c in tree if c not in mid]),
        ("去掉过台阶半格", rest + [c for c in tree if not c["name"].endswith("_half")]),
        ("少一个末梢", rest + [c for c in tree if not c["name"].startswith("vein_b2d2_")]),
        ("血管改回骨色", rest + [dict(c, material="bone_main") for c in tree]),
        ("末梢变 2 宽", rest + [dict(c, to=[c["to"][0] + 1.0, c["to"][1], c["to"][2]]) if c["name"].startswith("vein_b2u1_") and not c["name"].endswith("_half") else c for c in tree]),
    ):
        try:
            check_vessel_tree(bad)
        except AssertionError:
            print(f"  [OK] 注入「{name}」被拦截")
        else:
            raise RuntimeError(f"门禁失效：注入「{name}」没被拦住")

    body = part_01_body()
    ys = [c["from"][1] for c in body]
    assert min(ys) == BEAN_Y0 and max(c["to"][1] for c in body) == BEAN_Y0 + LAYERS * ROW_H, "豆身应是 y 2..30 共 14 层"
    assert max(c["to"][2] for c in body) == HALF_D, "豆身最厚处半厚应为 6（厚 12）"
    full_layers = [k for k in range(LAYERS) if not HILUM_LAYERS[0] <= k <= HILUM_LAYERS[1]]
    assert all(len(_layer_columns(k)) == 2 * HALF_W for k in full_layers), "非凹口层的宽度应都是 14"
    assert [_layer_cx(k) for k in (0, 13)] == [0, 0] and max(_layer_cx(k) for k in range(LAYERS)) == 4, "中心线两端 x=0、中段 +4"
    assert all(len(_layer_columns(k)) == 2 * HALF_W - HILUM_DEPTH for k in range(HILUM_LAYERS[0], HILUM_LAYERS[1] + 1)), "凹口层应各比整层少 4px"
    print("  [OK] 豆身：14 层、宽 14、厚 12，中心线弧形（两端 0、中段 +4），凹口 4 层 × 4px")

    check_round3(cubes)
    print("  [OK] 侧面台阶 ≤0.5px；入口管 4×4；输出口在底面中心、8×6 标准")

    def must_fail(name: str, bad: List[dict]) -> None:
        try:
            check_round3(bad)
        except AssertionError as e:
            print(f"  [OK] 注入「{name}」被拦截: {str(e)[:50]}")
            return
        raise RuntimeError(f"门禁失效：注入「{name}」没被拦住")

    must_fail("去掉台阶补平", [c for c in cubes if c["group"] != "spleen_fill"])
    must_fail("入口管改回 3×3", [dict(c, **{"from": [c["from"][0], c["from"][1], -1.5], "to": [c["to"][0], c["to"][1], 1.5]}) if c["name"] == "inlet_seg1" else c for c in cubes])
    must_fail("输出口移走", [dict(c, **{"from": [c["from"][0] + 6, c["from"][1], c["from"][2]], "to": [c["to"][0] + 6, c["to"][1], c["to"][2]]}) if c["name"].startswith("outlet_") else c for c in cubes])


def _profile(cubes: List[dict], side: str) -> Dict[int, float]:
    """豆身（含补平块）每 1px 行的最右 / 最左 x。"""
    prof: Dict[int, float] = {}
    for c in cubes:
        if c["group"] not in ("spleen", "spleen_fill"):
            continue
        for row in range(int(c["from"][1]), int(c["to"][1] + 0.999)):
            x = c["to"][0] if side == "right" else c["from"][0]
            if row not in prof or (x > prof[row] if side == "right" else x < prof[row]):
                prof[row] = x
    return prof


def check_round3(cubes: List[dict]) -> None:
    """第 3 轮门禁：侧面台阶补成 0.5px；入口管 4×4；输出口在底面中心、内衬 8×6。"""
    right = _profile(cubes, "right")
    rows = sorted(right)
    assert max(abs(right[b] - right[a]) for a, b in zip(rows, rows[1:])) <= 0.5 + 1e-6, "右缘台阶超过 0.5px"
    left = _profile(cubes, "left")
    y_base, row_h = int(BEAN_Y0), int(ROW_H)
    hilum_rows = range(y_base + HILUM_LAYERS[0] * row_h - 1, y_base + (HILUM_LAYERS[1] + 1) * row_h + 1)
    steps = [abs(left[b] - left[a]) for a, b in zip(rows, rows[1:]) if a not in hilum_rows and b not in hilum_rows]
    assert max(steps) <= 0.5 + 1e-6, "左缘台阶超过 0.5px"
    seg = next(c for c in cubes if c["name"] == "inlet_seg1")
    assert abs(seg["to"][2] - seg["from"][2] - 4.0) < 1e-6 and abs(seg["to"][1] - seg["from"][1] - 4.0) < 1e-6, "入口管应为 4×4"
    lining = next(c for c in cubes if c["name"] == "outlet_lining")
    assert abs(lining["to"][0] - lining["from"][0] - 8.0) < 1e-6 and abs(lining["to"][2] - lining["from"][2] - 6.0) < 1e-6, "输出口内衬应为 8 × 6"
    assert abs((lining["from"][0] + lining["to"][0]) / 2) < 1e-6 and lining["from"][1] < 0.5, "输出口应在底面中心"


_CELL_NAME = re.compile(r"^(?P<prefix>.+)_(?P<ix>-?\d+)_(?P<iy>-?\d+)(?P<half>_half)?$")


def _touch(a: dict, b: dict) -> bool:
    return all(a["from"][i] <= b["to"][i] + 1e-6 and b["from"][i] <= a["to"][i] + 1e-6 for i in range(3))


def check_vessel_tree(cubes: List[dict]) -> None:
    """血管树门禁：#c07868；2×2 主干（有 2×2 的格块）；上 / 下两支一级、四个末梢（u1 u2 d1 d2）都在；
    支脉 / 末梢格 1×1；整棵连成一块；相邻格（含半格）前表面高差 ≤0.5px。"""
    tree = [c for c in cubes if c["group"] == "vessel_tree"]
    assert tree and all(c["material"] == "tendon_fiber" for c in tree), "血管树必须是 #c07868（tendon_fiber）"
    cells = {}
    tops: Dict[str, float] = {}
    for c in tree:
        m = _CELL_NAME.match(c["name"])
        key = f"{m['prefix']}_{m['ix']}_{m['iy']}"
        tops[key] = max(tops.get(key, 0.0), c["to"][2])
        if not m["half"]:
            cells[(int(m["ix"]), int(m["iy"]))] = (m["prefix"], key)
            if m["prefix"] != "vein_trunk":
                assert abs(c["to"][0] - c["from"][0] - 1.0) < 1e-6 and abs(c["to"][1] - c["from"][1] - 1.0) < 1e-6, f"{c['name']} 应是 1×1"
    trunk_cells = {k for k, (prefix, _) in cells.items() if prefix == "vein_trunk"}
    assert any({(x + 1, y), (x, y + 1), (x + 1, y + 1)} <= trunk_cells for x, y in trunk_cells), "主干应是 2×2 截面"
    for part in ("vein_b1u", "vein_b1d", "vein_b2u1", "vein_b2u2", "vein_b2d1", "vein_b2d2"):
        assert any(prefix == part for prefix, _ in cells.values()), f"血管树缺 {part}"
    seen, stack = {0}, [0]
    while stack:
        i = stack.pop()
        for j in range(len(tree)):
            if j not in seen and _touch(tree[i], tree[j]):
                seen.add(j)
                stack.append(j)
    assert len(seen) == len(tree), f"血管树断成多块: 连通 {len(seen)} / {len(tree)}"
    for (ix, iy), (_, key) in cells.items():
        for nb in ((ix + 1, iy), (ix, iy + 1)):
            if nb in cells:
                other = cells[nb][1]
                assert abs(tops[key] - tops[other]) <= 0.5 + 1e-6, f"{key} 与 {other} 前表面高差 {abs(tops[key] - tops[other]):.2f} > 0.5"


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
