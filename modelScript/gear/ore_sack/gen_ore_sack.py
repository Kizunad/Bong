#!/usr/bin/env python3
"""矿石袋（ore_sack）穿戴件生成器 —— Round 1/3。

server/assets/items/workbench_materials.toml：「粗布加底皮的矿石袋，耐磨。」
任务卡 #2：腰后 / 臀挂件，粗布加底皮，鼓鼓囊囊坠着矿石棱角，底部皮补丁。参考图见
``model-review/img/gear/ore_sack/{ref,exploded}.png``。

穿戴（Bedrock geo 系，脚 y=0，+z 背面）：
  · 袋身挂在臀后偏右：x∈[-4.8,2.4]、y∈[3.0,12.8]、z∈[2.2,8.0]，整个在躯干/腿的背面之外（z>2）
  · 腰上一圈草绳只走背面（z∈[2.05,2.55]），不绕到侧面 —— 侧面 x=±4..±8 是手臂静止体积
  · 袋颈由一条皮环扣在腰绳上，袋口褶边和三块矿石探出袋口
  · 袋腹上有几块矿石棱角顶出布面，单独一骨方便晃

bone：
  strap_waist  腰后草绳三股 + 皮环 + 袋颈系绳（挂点）
  pack_sack    袋身（粗布分层鼓肚 + 布鼓包 + 袋口褶 + 颈绳）
  pack_bottom  底皮杯 + 一圈粗针脚
  hang_ore     顶出布面和袋口的矿石块

用法：
  python3 modelScript/gear/ore_sack/gen_ore_sack.py              # 全件 + parts.png + check.png
  python3 modelScript/gear/ore_sack/gen_ore_sack.py --self-test  # 门禁差分自证
  python3 modelScript/gear/ore_sack/gen_ore_sack.py --check
  python3 modelScript/gear/ore_sack/gen_ore_sack.py --part sack
"""

from __future__ import annotations

import argparse
import math
import shutil
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "gear"))
import gear_common as gc  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig  # noqa: E402

GEAR_ID = "ore_sack"
HERE = Path(__file__).resolve().parent
BBMODEL_OUT = HERE / "OreSack.bbmodel"
BBMODEL_COPY = gc.MODELS_DIR / "OreSack.bbmodel"
SCRATCH = REPO / "modelScript" / "out" / "gear" / GEAR_ID

# 材质 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」
MATS = {
    "cloth": (156, 138, 106),        # 粗麻布
    "cloth_dark": (110, 94, 72),     # 褶 / 暗面 / 袋口内
    "leather": (124, 72, 50),        # 锈褐皮（底皮、皮环）
    "leather_dark": (84, 50, 36),    # 皮阴面
    "cord": (132, 120, 72),          # 草绳
    "ore_grey": (108, 110, 118),     # 矿石：灰铁
    "ore_rust": (152, 92, 56),       # 矿石：锈橙面
    "stitch": (62, 48, 36),          # 粗针脚
}

CX, CZ = -1.2, 5.1           # 袋身中心（x 偏右半格，z 在背面之外）
# (名, y0, y1, 半宽 x, 半深 z)
SLICES = (
    ("s0", 4.4, 5.6, 3.3, 2.7),
    ("s1", 5.6, 9.0, 3.6, 2.9),
    ("s2", 9.0, 10.4, 3.0, 2.5),
    ("s3", 10.4, 11.4, 1.6, 1.4),
)


def part_waist(rig: Rig) -> None:
    """腰后草绳三股 + 皮环 + 袋颈系绳：只走背面（z 在躯干之外），不绕进手臂体积。"""
    rig.bone("strap_waist", (CX, 13.0, 2.3))
    for i, y in enumerate((12.2, 12.85, 13.5)):
        rig.cube("strap_waist", f"rope_{i}", (-4.4, y, 2.05), (4.4, y + 0.5, 2.6), mat="cord")
    # 绳节：每股在两端各打一个小结，读作「缠了几圈」
    for i, x in enumerate((-4.5, 4.0)):
        rig.cube("strap_waist", f"rope_knot_{i}", (x, 12.1, 2.0 + 0.05), (x + 0.5, 14.0, 2.85), mat="cord")
    # 皮环：竖条，上端卡在腰绳后，下端接袋颈
    rig.cube("strap_waist", "loop_plate", (CX - 0.8, 11.2, 2.05), (CX + 0.8, 14.2, 2.7), mat="leather")
    rig.cube("strap_waist", "loop_top", (CX - 0.8, 13.6, 2.05), (CX + 0.8, 14.2, 3.2), mat="leather_dark")
    for i, y in enumerate((11.5, 12.6)):
        rig.cube("strap_waist", f"loop_st{i}", (CX - 0.5, y, 2.45), (CX + 0.5, y + 0.3, 3.0), mat="stitch")
    # 皮环下端到袋颈的系绳
    rig.shaft("strap_waist", "neck_tie", (CX, 11.4, 2.5), (CX, 11.0, 3.8), 0.3, 0.3, mat="cord")


def part_sack(rig: Rig) -> None:
    """袋身：粗布分层鼓肚（下小、腹大、肩收、颈束），加布鼓包、褶棱、颈绳和袋口褶边。"""
    rig.bone("pack_sack", (CX, 11.4, CZ - 1.0))
    for tag, y0, y1, hw, hd in SLICES:
        rig.cube("pack_sack", f"body_{tag}", (CX - hw, y0, CZ - hd), (CX + hw, y1, CZ + hd), mat="cloth")
    # 布鼓包：被矿石顶起来的几处，比主体再鼓 0.5，歪着摆读作不规则
    bumps = (
        ("l", CX - 3.6 - 0.45, 7.8, CZ - 0.6, 0.45, 1.3, 1.1),
        ("r", CX + 3.6, 6.4, CZ + 0.8, 0.45, 1.2, 1.0),
        ("b", CX + 0.9, 8.2, CZ + 2.9, 1.2, 1.3, 0.45),
        ("c", CX - 2.9, 5.9, CZ + 2.9, 1.2, 1.1, 0.45),
    )
    for tag, x, y, z, sx, sy, sz in bumps:
        rig.cube("pack_sack", f"bump_{tag}", (x, y, z), (x + sx, y + sy, z + sz), mat="cloth")
    # 褶棱：袋肩往下的竖向暗褶（背面和两侧各几道）
    for i, dx in enumerate((-2.2, -0.5, 1.3)):
        rig.cube("pack_sack", f"fold_b{i}", (CX + dx - 0.18, 7.2 + (i % 2) * 0.8, CZ + 2.9),
                 (CX + dx + 0.18, 10.1, CZ + 2.9 + 0.25), mat="cloth_dark")
    for i, dz in enumerate((-1.6, 0.6)):
        for tag, sx in (("l", -1), ("r", 1)):
            x = CX + sx * 3.0
            rig.cube("pack_sack", f"fold_{tag}{i}", (x, 8.3, CZ + dz - 0.18), (x + sx * 0.3, 10.2, CZ + dz + 0.18), mat="cloth_dark")
    # 颈绳：两圈草绳压在束口下，绳头垂下
    rig.cube("pack_sack", "neck_a", (CX - 1.9, 10.5, CZ - 1.7), (CX + 1.9, 10.95, CZ + 1.7), mat="cord")
    rig.cube("pack_sack", "neck_b", (CX - 1.85, 11.0, CZ - 1.65), (CX + 1.85, 11.4, CZ + 1.65), mat="cord")
    rig.cube("pack_sack", "neck_knot", (CX - 0.5, 10.3, CZ - 2.15), (CX + 0.5, 11.2, CZ - 1.65), mat="cord")
    rig.shaft("pack_sack", "tail_l", (CX - 0.2, 10.4, CZ - 2.0), (CX - 0.6, 8.9, CZ - 2.3), 0.22, 0.22, mat="cord")
    rig.shaft("pack_sack", "tail_r", (CX + 0.2, 10.4, CZ - 2.0), (CX + 0.8, 9.2, CZ - 2.2), 0.22, 0.22, mat="cord")
    # 袋口褶边：八个高低错落的褶，围成一圈，里面一块暗色内底
    rig.cube("pack_sack", "mouth_floor", (CX - 1.5, 11.4, CZ - 1.3), (CX + 1.5, 11.8, CZ + 1.3), mat="cloth_dark")
    for i in range(8):
        a = i * math.tau / 8
        x = CX + math.cos(a) * 1.75
        z = CZ + math.sin(a) * 1.55
        h = (1.0, 1.6, 0.9, 1.4, 1.1, 1.7, 0.8, 1.3)[i]
        rig.cube("pack_sack", f"ruffle_{i}", (x - 0.55, 11.3, z - 0.5), (x + 0.55, 11.3 + h, z + 0.5), mat="cloth")


def part_bottom(rig: Rig) -> None:
    """底皮杯：包住袋底的锈褐皮，上缘一圈粗针脚缝在布上。"""
    rig.bone("pack_bottom", (CX, 3.0, CZ))
    rig.cube("pack_bottom", "cup_floor", (CX - 2.7, 3.0, CZ - 2.3), (CX + 2.7, 3.5, CZ + 2.3), mat="leather_dark")
    rig.cube("pack_bottom", "cup_wall", (CX - 3.1, 3.5, CZ - 2.6), (CX + 3.1, 4.4, CZ + 2.6), mat="leather")
    rig.cube("pack_bottom", "cup_lip", (CX - 3.4, 4.2, CZ - 2.8), (CX + 3.4, 4.9, CZ + 2.8), mat="leather")
    # 上缘针脚：前后各 5、左右各 4
    for i in range(5):
        x = CX - 2.6 + i * 1.3
        rig.cube("pack_bottom", f"st_f{i}", (x, 4.5, CZ - 2.95), (x + 0.5, 5.4, CZ - 2.55), mat="stitch")
        rig.cube("pack_bottom", f"st_b{i}", (x, 4.5, CZ + 2.55), (x + 0.5, 5.4, CZ + 2.95), mat="stitch")
    for i in range(4):
        z = CZ - 1.9 + i * 1.3
        rig.cube("pack_bottom", f"st_l{i}", (CX - 3.55, 4.5, z), (CX - 3.15, 5.4, z + 0.5), mat="stitch")
        rig.cube("pack_bottom", f"st_r{i}", (CX + 3.15, 4.5, z), (CX + 3.55, 5.4, z + 0.5), mat="stitch")
    # 底面补丁疤：一块深色皮补
    rig.cube("pack_bottom", "cup_patch", (CX - 1.4, 2.8, CZ - 1.2), (CX + 0.6, 3.1, CZ + 1.4), mat="leather")


def _rock(rig: Rig, bone: str, name: str, c, s, rot) -> None:
    """一块棱角矿石：主块 + 一个错开的小块，读作断面多角。c 中心，s 边长，rot 欧拉角（度）。"""
    half = [v / 2 for v in s]
    lo = tuple(c[k] - half[k] for k in range(3))
    hi = tuple(c[k] + half[k] for k in range(3))
    rig.cube(bone, f"{name}_a", lo, hi, rot=rot, org=c, mat="ore_grey")
    sub = [v * 0.62 for v in s]
    off = (s[0] * 0.28, s[1] * 0.22, s[2] * 0.18)
    c2 = tuple(c[k] + off[k] for k in range(3))
    rig.cube(bone, f"{name}_b", tuple(c2[k] - sub[k] / 2 for k in range(3)), tuple(c2[k] + sub[k] / 2 for k in range(3)),
             rot=(rot[0] + 25, rot[1] - 20, rot[2] + 15), org=c2, mat="ore_rust")


def part_ore(rig: Rig) -> None:
    """矿石：袋腹四块顶出布面（左 / 右 / 背 / 前下），袋口三块探出。"""
    rig.bone("hang_ore", (CX, 9.0, CZ))
    _rock(rig, "hang_ore", "belly_r", (CX + 3.5, 8.6, CZ + 0.3), (1.9, 1.9, 1.9), (20, 30, 10))
    _rock(rig, "hang_ore", "belly_l", (CX - 3.6, 6.8, CZ + 0.2), (1.8, 1.8, 1.8), (15, 40, 20))
    _rock(rig, "hang_ore", "belly_b", (CX + 1.4, 7.2, CZ + 2.9), (2.0, 1.9, 1.7), (30, 15, 5))
    _rock(rig, "hang_ore", "belly_c", (CX - 2.2, 6.4, CZ + 2.95), (1.7, 1.6, 1.5), (10, 25, 35))
    _rock(rig, "hang_ore", "mouth_a", (CX - 0.6, 12.7, CZ + 0.2), (2.0, 1.8, 1.9), (25, 20, 10))
    _rock(rig, "hang_ore", "mouth_b", (CX + 1.0, 12.5, CZ - 0.4), (1.6, 1.6, 1.5), (10, 45, 20))
    _rock(rig, "hang_ore", "mouth_c", (CX - 0.1, 13.2, CZ + 0.9), (1.3, 1.3, 1.2), (35, 10, 25))


PARTS = {
    "waist": part_waist,
    "sack": part_sack,
    "bottom": part_bottom,
    "ore": part_ore,
}
ORDER = ["waist", "sack", "bottom", "ore"]


def build(parts: list[str] | None = None, rig: Rig | None = None) -> Rig:
    rig = rig or Rig(MATS)
    for key in (parts or ORDER):
        PARTS[key](rig)
    return rig


SOFT_OVER = frozenset({
    frozenset(("cloth", "cloth_dark")), frozenset(("cloth", "cord")), frozenset(("cloth", "stitch")),
    frozenset(("cloth", "leather")), frozenset(("cloth", "leather_dark")),
    frozenset(("cloth", "ore_grey")), frozenset(("cloth", "ore_rust")),
    frozenset(("cloth_dark", "ore_grey")), frozenset(("cloth_dark", "ore_rust")),
    frozenset(("cloth_dark", "cord")),
    frozenset(("leather", "leather_dark")), frozenset(("leather", "stitch")), frozenset(("leather", "cord")),
    frozenset(("leather_dark", "stitch")), frozenset(("leather_dark", "cord")),
    frozenset(("cord", "stitch")), frozenset(("cord", "ore_grey")), frozenset(("cord", "ore_rust")),
    frozenset(("ore_grey", "ore_rust")),
})

GATES = gc.GearGates(
    "矿石袋 / ore_sack",
    MATS,
    # 袋身偏右半格、腰绳两端结位置不对称是刻意的；袋子本体仍是中线近对称，但矿石块/结/鼓包不对称
    symmetric=False,
    soft_over=SOFT_OVER,
)

NOTE = "注：比例/挂点是否读得出，自检量不出 —— 看 check.png 的灰模三面。"


def check(rig: Rig) -> int:
    return GATES.report(rig, px=gc.PX, note=NOTE)


def main() -> int:
    ap = argparse.ArgumentParser(description="矿石袋穿戴件生成器")
    ap.add_argument("--part", help="只生成单件（调试用）")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()

    if args.list:
        for k in ORDER:
            print(f"  {k:8s} {(PARTS[k].__doc__ or '').strip().splitlines()[0]}")
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

    gc.write_bbmodel(rig, "OreSack", BBMODEL_OUT)
    shutil.copyfile(BBMODEL_OUT, BBMODEL_COPY)
    notes = [f"cubes {len(rig.elements)} / bones {len(rig.bones)} / "
             + ("gates 0 violations" if not bad else f"! gates: {bad} violations")]
    parts_png = gc.render_parts(build, ORDER, MATS, GEAR_ID, SCRATCH)
    check_png = gc.render_check(build, MATS, GEAR_ID,
                                "ore_sack (round 1/3) - back-hip ore sack worn on grey mannequin",
                                SCRATCH, notes=notes, close_pad=4.0, close_bias=(0.0, 0.0, 2.0))
    print(f"  → {BBMODEL_OUT.relative_to(REPO)}\n  → {parts_png}\n  → {check_png}")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
