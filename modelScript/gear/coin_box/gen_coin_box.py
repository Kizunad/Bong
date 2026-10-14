#!/usr/bin/env python3
"""骨币匣（coin_box）穿戴件生成器 —— Round 1/3。

server/assets/items/workbench_materials.toml：「木匣内壁涂丹砂隔灵，骨币少漏。」
任务卡 #5：腰前挂件，小木匣，内壁涂丹砂，外挂一串骨币。参考图见
``model-review/img/gear/coin_box/{ref,exploded}.png``。

穿戴（Bedrock geo 系，脚 y=0，+z 背面，正面 = -z）：
  · 木匣挂在腰前偏右：x∈[-3.3,1.7]、y∈[10.6,13.6]、z∈[-6.25,-2.7]，整个在躯干/腿的前面之外（z<-2.05）
  · 匣盖绕后沿铰链向上掀开约 32°，露出丹砂红内壁；盖和匣都是外壳 + 红色内衬
  · 前面和后面各一条皮带片（y∈[12.2,13.9]），只走躯干前/背面外侧，不绕进两侧手臂体积
  · 匣右侧从腰带垂一串骨币（四枚带方孔的骨币串在草绳上，绳尾散成穗），挂在腿前（z<-2.05）
  · 皮带下垂几条破布条，读作腰裙

bone：
  strap_belt  前后皮带片 + 铁扣（挂点）
  hang_box    匣身 + 铁箍 + 角绳 + 铁搭扣
  hang_lid    匣盖（单独一骨，pivot 在后沿铰链，以后可做开合）
  hang_coins  草绳 + 四枚骨币 + 绳尾
  hang_rags   腰下破布条

用法：
  python3 modelScript/gear/coin_box/gen_coin_box.py              # 全件 + parts.png + check.png
  python3 modelScript/gear/coin_box/gen_coin_box.py --self-test  # 门禁差分自证
  python3 modelScript/gear/coin_box/gen_coin_box.py --check
  python3 modelScript/gear/coin_box/gen_coin_box.py --part box
"""

from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "gear"))
import gear_common as gc  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig  # noqa: E402

GEAR_ID = "coin_box"
HERE = Path(__file__).resolve().parent
BBMODEL_OUT = HERE / "CoinBox.bbmodel"
BBMODEL_COPY = gc.MODELS_DIR / "CoinBox.bbmodel"
SCRATCH = REPO / "modelScript" / "out" / "gear" / GEAR_ID

# 材质 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」
MATS = {
    "wood": (140, 100, 66),          # 旧木板
    "wood_dark": (92, 64, 44),       # 板缝 / 阴面
    "cinnabar": (170, 42, 34),       # 丹砂内壁
    "iron": (104, 82, 70),           # 锈铁箍 / 搭扣 / 铰链
    "bone": (214, 204, 176),         # 骨币
    "cord": (142, 118, 76),          # 草绳
    "leather": (84, 58, 44),         # 皮带
    "rag": (122, 110, 88),           # 破布条
}

# 匣身（穿戴者腰前偏右；z 全在躯干前面 -2.05 之外）
BX0, BX1 = -3.3, 1.7
BY0, BY1 = 10.6, 13.6
BZ0, BZ1 = -6.25, -2.7           # 前沿 / 后沿（后沿贴皮带片）
WALL = 0.5
LID_OPEN_DEG = 32.0              # 盖掀开角度（绕后沿 x 轴）
LID_H = 1.0
HINGE = (0.0, BY1, BZ1)          # 铰链枢轴（盖旋转中心）


def part_belt(rig: Rig) -> None:
    """前后皮带片 + 铁扣：只走躯干前/背面外侧，z 在 ±2.05 之外，不进两侧手臂体积。"""
    rig.bone("strap_belt", (0.0, 13.0, -2.3))
    for tag, z0, z1 in (("f", -2.65, -2.05), ("b", 2.05, 2.65)):
        rig.cube("strap_belt", f"belt_{tag}", (-4.4, 12.2, z0), (4.4, 13.9, z1), mat="leather")
        # 上下缝线：深色细条
        rig.cube("strap_belt", f"belt_{tag}_hem_t", (-4.3, 13.55, z0 - 0.12 if tag == "f" else z1 - 0.1),
                 (4.3, 13.85, z0 + 0.1 if tag == "f" else z1 + 0.12), mat="wood_dark")
        rig.cube("strap_belt", f"belt_{tag}_hem_b", (-4.3, 12.25, z0 - 0.12 if tag == "f" else z1 - 0.1),
                 (4.3, 12.55, z0 + 0.1 if tag == "f" else z1 + 0.12), mat="wood_dark")
    # 前面铁扣：方环搭扣，在匣左侧（-x）
    rig.cube("strap_belt", "buckle_plate", (2.4, 12.35, -2.9), (3.5, 13.75, -2.5), mat="iron")
    rig.cube("strap_belt", "buckle_tongue", (2.75, 12.75, -3.1), (3.15, 13.35, -2.85), mat="wood_dark")
    # 腰带侧边的草绳：前后各缠两股，补强
    for i, y in enumerate((12.55, 13.2)):
        rig.cube("strap_belt", f"rope_f{i}", (-4.45, y, -2.85), (-3.4, y + 0.35, -2.5), mat="cord")
        rig.cube("strap_belt", f"rope_b{i}", (3.4, y, 2.5), (4.45, y + 0.35, 2.85), mat="cord")


def part_box(rig: Rig) -> None:
    """匣身：四壁 + 底的木匣，内壁一层丹砂，前面板缝、四角绳箍、铁搭扣。"""
    rig.bone("hang_box", (0.0, 12.0, BZ1))
    cx = (BX0 + BX1) / 2
    rig.cube("hang_box", "floor", (BX0, BY0, BZ0), (BX1, BY0 + WALL, BZ1), mat="wood_dark")
    rig.cube("hang_box", "wall_front", (BX0, BY0 + WALL, BZ0), (BX1, BY1, BZ0 + WALL), mat="wood")
    rig.cube("hang_box", "wall_back", (BX0, BY0 + WALL, BZ1 - WALL), (BX1, BY1, BZ1), mat="wood")
    rig.cube("hang_box", "wall_l", (BX0, BY0 + WALL, BZ0 + WALL), (BX0 + WALL, BY1, BZ1 - WALL), mat="wood")
    rig.cube("hang_box", "wall_r", (BX1 - WALL, BY0 + WALL, BZ0 + WALL), (BX1, BY1, BZ1 - WALL), mat="wood")
    # 丹砂内衬：底面 + 四壁内侧各一层
    ix0, ix1 = BX0 + WALL, BX1 - WALL
    iz0, iz1 = BZ0 + WALL, BZ1 - WALL
    rig.cube("hang_box", "liner_floor", (ix0, BY0 + WALL, iz0), (ix1, BY0 + WALL + 0.25, iz1), mat="cinnabar")
    rig.cube("hang_box", "liner_f", (ix0, BY0 + WALL, iz0), (ix1, BY1 - 0.15, iz0 + 0.25), mat="cinnabar")
    rig.cube("hang_box", "liner_b", (ix0, BY0 + WALL, iz1 - 0.25), (ix1, BY1 - 0.15, iz1), mat="cinnabar")
    rig.cube("hang_box", "liner_l", (ix0, BY0 + WALL, iz0), (ix0 + 0.25, BY1 - 0.15, iz1), mat="cinnabar")
    rig.cube("hang_box", "liner_r", (ix1 - 0.25, BY0 + WALL, iz0), (ix1, BY1 - 0.15, iz1), mat="cinnabar")
    # 前面板缝：两道横向暗缝，板条错位
    for i, y in enumerate((11.5, 12.5)):
        rig.cube("hang_box", f"seam_f{i}", (BX0 + 0.05, y, BZ0 - 0.12), (BX1 - 0.05, y + 0.22, BZ0 + 0.1), mat="wood_dark")
    for i, y in enumerate((11.4, 12.4)):
        rig.cube("hang_box", f"seam_r{i}", (BX1 - 0.1, y, BZ0 + 0.3), (BX1 + 0.12, y + 0.22, BZ1 - 0.3), mat="wood_dark")
    # 四角草绳箍：竖向小束，压在转角上
    for tag, x, z in (("fl", BX0, BZ0), ("fr", BX1, BZ0), ("bl", BX0, BZ1), ("br", BX1, BZ1)):
        sx = 1 if x == BX0 else -1
        sz = 1 if z == BZ0 else -1
        rig.cube("hang_box", f"corner_{tag}",
                 (x - 0.12 if sx > 0 else x - 0.4, BY0 + 0.3, z - 0.12 if sz > 0 else z - 0.4),
                 (x + 0.4 if sx > 0 else x + 0.12, BY1 - 0.3, z + 0.4 if sz > 0 else z + 0.12), mat="cord")
    # 铁搭扣：前面中线一块竖铁板，带方孔
    rig.cube("hang_box", "hasp", (cx - 0.55, 11.0, BZ0 - 0.3), (cx + 0.55, 13.5, BZ0 + 0.1), mat="iron")
    rig.cube("hang_box", "hasp_slot", (cx - 0.2, 11.7, BZ0 - 0.42), (cx + 0.2, 12.5, BZ0 - 0.18), mat="wood_dark")
    # 铰链：后沿上两个小铁节
    for i, x in enumerate((BX0 + 0.9, BX1 - 1.5)):
        rig.cube("hang_box", f"hinge_{i}", (x, BY1 - 0.6, BZ1 - 0.1), (x + 0.6, BY1 + 0.25, BZ1 + 0.28), mat="iron")


def _lid_cube(rig: Rig, name: str, lo, hi, mat: str) -> None:
    """盖上的件：按合盖位置给坐标，整体绕后沿铰链转 LID_OPEN_DEG（所有件共用同一枢轴 → 刚体）。"""
    rig.cube("hang_lid", name, lo, hi, rot=(LID_OPEN_DEG, 0.0, 0.0), org=HINGE, mat=mat)


def part_lid(rig: Rig) -> None:
    """匣盖：绕后沿铰链掀开，顶板 + 前沿板 + 两端板的外壳，里面一层丹砂，顶上铁条。"""
    rig.bone("hang_lid", HINGE)
    x0, x1 = BX0 - 0.1, BX1 + 0.1
    z0, z1 = BZ0 - 0.1, BZ1
    y0, y1 = BY1, BY1 + LID_H
    _lid_cube(rig, "lid_top", (x0, y1 - WALL, z0), (x1, y1, z1), "wood")
    _lid_cube(rig, "lid_front", (x0, y0, z0), (x1, y1 - WALL, z0 + WALL), "wood")
    _lid_cube(rig, "lid_end_l", (x0, y0, z0 + WALL), (x0 + WALL, y1 - WALL, z1), "wood")
    _lid_cube(rig, "lid_end_r", (x1 - WALL, y0, z0 + WALL), (x1, y1 - WALL, z1), "wood")
    # 丹砂内衬：盖里面（顶板下 + 前板后 + 两端板内）
    _lid_cube(rig, "lid_liner_top", (x0 + WALL, y1 - WALL - 0.25, z0 + WALL), (x1 - WALL, y1 - WALL, z1), "cinnabar")
    _lid_cube(rig, "lid_liner_front", (x0 + WALL, y0, z0 + WALL), (x1 - WALL, y1 - WALL - 0.25, z0 + WALL + 0.25), "cinnabar")
    # 顶面板缝 + 铁条：一道纵向铁条压在盖中线，前沿接搭扣
    _lid_cube(rig, "lid_seam", (x0 + 0.05, y1 - 0.1, (z0 + z1) / 2 - 0.1), (x1 - 0.05, y1 + 0.12, (z0 + z1) / 2 + 0.12), "wood_dark")
    cx = (BX0 + BX1) / 2
    _lid_cube(rig, "lid_strap", (cx - 0.55, y1 - 0.1, z0 + 0.1), (cx + 0.55, y1 + 0.18, z1 - 0.5), "iron")
    _lid_cube(rig, "lid_catch", (cx - 0.55, y0 + 0.2, z0 - 0.3), (cx + 0.55, y1, z0 + 0.1), "iron")


def part_coins(rig: Rig) -> None:
    """骨币串：从皮带垂下的草绳，四枚带方孔的骨币等距串在上面，绳尾散成三缕穗。"""
    rig.bone("hang_coins", (2.7, 13.0, -2.7))
    rx, rz = 2.55, -2.95
    # 绳：竖向两截（币之间露出绳）
    rig.cube("hang_coins", "cord_top", (rx, 11.6, rz), (rx + 0.35, 13.2, rz + 0.35), mat="cord")
    rig.cube("hang_coins", "knot", (rx - 0.25, 12.7, rz - 0.2), (rx + 0.6, 13.4, rz + 0.55), mat="cord")
    rig.cube("hang_coins", "cord_low", (rx, 5.2, rz), (rx + 0.35, 11.7, rz + 0.35), mat="cord")
    # 币：每枚 1.8 方的十字形圆钱（横竖两条宽 1.8 / 1.0 的板叠出切角），正面中间一个深色方孔，
    # 币间留 0.35 的空，露出绳
    for i, ytop in enumerate((11.5, 9.8, 8.1, 6.4)):
        c = rx + 0.175
        y0, y1 = ytop - 1.5, ytop
        z0, z1 = rz - 0.12, rz + 0.45
        ym = (y0 + y1) / 2
        rig.cube("hang_coins", f"coin{i}_h", (c - 0.9, ym - 0.5, z0), (c + 0.9, ym + 0.5, z1), mat="bone")
        rig.cube("hang_coins", f"coin{i}_v", (c - 0.55, y0, z0), (c + 0.55, y1, z1), mat="bone")
        rig.cube("hang_coins", f"coin{i}_hole", (c - 0.25, ym - 0.25, z0 - 0.1), (c + 0.25, ym + 0.25, z0 + 0.2), mat="wood_dark")
    # 绳尾：三缕穗
    for i, (dx, ln) in enumerate(((-0.3, 1.2), (0.05, 1.7), (0.4, 1.0))):
        rig.cube("hang_coins", f"tassel_{i}", (rx + dx, 5.2 - ln, rz), (rx + dx + 0.28, 5.4, rz + 0.3), mat="cord")


def part_rags(rig: Rig) -> None:
    """腰下破布条：皮带下垂的参差布条（前后各五条），读作腰裙。"""
    rig.bone("hang_rags", (0.0, 12.2, 0.0))
    # (x 中心, 宽, 长)：长短参差
    spec = ((-3.6, 1.1, 2.6), (-2.1, 1.3, 3.4), (0.4, 1.0, 2.2), (2.6, 1.3, 3.0), (3.8, 0.9, 2.0))
    for tag, z0, z1 in (("f", -2.62, -2.28), ("b", 2.28, 2.62)):
        for i, (cx, w, ln) in enumerate(spec):
            rig.cube("hang_rags", f"rag_{tag}{i}", (cx - w / 2, 12.3 - ln, z0), (cx + w / 2, 12.5, z1), mat="rag")


PARTS = {
    "belt": part_belt,
    "box": part_box,
    "lid": part_lid,
    "coins": part_coins,
    "rags": part_rags,
}
ORDER = ["belt", "box", "lid", "coins", "rags"]


def build(parts: list[str] | None = None, rig: Rig | None = None) -> Rig:
    rig = rig or Rig(MATS)
    for key in (parts or ORDER):
        PARTS[key](rig)
    return rig


SOFT_OVER = frozenset({
    frozenset(("wood", "wood_dark")), frozenset(("wood", "cinnabar")), frozenset(("wood_dark", "cinnabar")),
    frozenset(("wood", "iron")), frozenset(("wood", "cord")), frozenset(("wood_dark", "iron")),
    frozenset(("wood_dark", "cord")), frozenset(("iron", "cord")), frozenset(("iron", "cinnabar")),
    frozenset(("leather", "wood_dark")), frozenset(("leather", "iron")), frozenset(("leather", "cord")),
    frozenset(("leather", "rag")), frozenset(("leather", "wood")),
    frozenset(("bone", "cord")), frozenset(("cord", "rag")),
})

GATES = gc.GearGates(
    "骨币匣 / coin_box",
    MATS,
    symmetric=False,
    soft_over=SOFT_OVER,
)

NOTE = "注：比例/挂点是否读得出，自检量不出 —— 看 check.png 的灰模三面。"


def check(rig: Rig) -> int:
    return GATES.report(rig, px=gc.PX, note=NOTE)


def main() -> int:
    ap = argparse.ArgumentParser(description="骨币匣穿戴件生成器")
    ap.add_argument("--part", help="只生成单件（调试用）")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()

    if args.list:
        for k in ORDER:
            print(f"  {k:6s} {(PARTS[k].__doc__ or '').strip().splitlines()[0]}")
        return 0
    if args.part and args.part not in PARTS:
        print(f"未知部件 {args.part}；可选：{', '.join(ORDER)}")
        return 2

    rig = build([args.part] if args.part else None)
    if args.self_test:
        return 1 if GATES.self_test(rig) else 0

    bad = check(rig)
    if args.check or args.part:
        return 1 if bad else 0

    gc.write_bbmodel(rig, "CoinBox", BBMODEL_OUT)
    shutil.copyfile(BBMODEL_OUT, BBMODEL_COPY)
    notes = [f"cubes {len(rig.elements)} / bones {len(rig.bones)} / "
             + ("gates 0 violations" if not bad else f"! gates: {bad} violations")]
    parts_png = gc.render_parts(build, ORDER, MATS, GEAR_ID, SCRATCH)
    check_png = gc.render_check(build, MATS, GEAR_ID,
                                "coin_box (round 1/3) - front-waist coin box worn on grey mannequin",
                                SCRATCH, notes=notes, close_at=((0.0, 11.0, -2.0), 20.0))
    print(f"  → {BBMODEL_OUT.relative_to(REPO)}\n  → {parts_png}\n  → {check_png}")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
