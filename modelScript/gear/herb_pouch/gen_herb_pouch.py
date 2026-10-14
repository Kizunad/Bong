#!/usr/bin/env python3
"""灵草囊（herb_pouch）穿戴件生成器 —— Round 1/3。

server/assets/items/workbench_materials.toml：「粗布缝成的草药袋，采集时直接塞入减少磨损。」
任务卡 #1：腰侧挂件，粗布袋口露几根干草药，草绳系在腰带上。参考图见
``model-review/img/gear/herb_pouch/{ref,exploded}.png``。

穿戴（Bedrock geo 系，脚 y=0，+z 背面；穿戴者右腰 = -x）：
  · 袋身挂在右胯外侧：x∈[-7.5,-4.1]、y∈[3.6,10.5]，整个在手臂静止体积（y≥12）之下、
    腿（|x|<4）之外，不穿身体
  · 一段皮带片 + 草绳套扣在右胯前侧（y≈12.2..13.9，z 在躯干前面 -2.05 以外），草绳从带片垂到袋颈
  · 草药从袋口斜着往前探：高过 y=12 的那一段全部在 z<-2.1（躯干/手臂之前），不进手臂体积

bone：
  hang_loop   皮带片 + 挂绳（挂点，pivot 在带片下沿）
  hang_pouch  袋身 + 袋口褶 + 补丁 + 颈绳 + 骨栓（整只袋子，以后绕 hang_loop 晃）
  hang_herbs  袋口探出的三根草药（单独一骨，晃得比袋子大）

用法：
  python3 modelScript/gear/herb_pouch/gen_herb_pouch.py              # 全件 + parts.png + check.png
  python3 modelScript/gear/herb_pouch/gen_herb_pouch.py --self-test  # 门禁差分自证
  python3 modelScript/gear/herb_pouch/gen_herb_pouch.py --check      # 只跑自检
  python3 modelScript/gear/herb_pouch/gen_herb_pouch.py --part pouch # 只出单件
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

GEAR_ID = "herb_pouch"
HERE = Path(__file__).resolve().parent
BBMODEL_OUT = HERE / "HerbPouch.bbmodel"
BBMODEL_COPY = gc.MODELS_DIR / "HerbPouch.bbmodel"
SCRATCH = REPO / "modelScript" / "out" / "gear" / GEAR_ID

# 材质 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」，超出会让门禁把两种材质当成一种。
MATS = {
    "cloth": (152, 136, 108),       # 粗麻布：灰米
    "cloth_dark": (108, 94, 74),    # 褶皱 / 袋口内侧
    "patch": (118, 66, 52),         # 锈褐皮补丁、带片
    "cord": (126, 118, 70),         # 草绳：枯橄榄
    "bone": (214, 204, 176),        # 骨栓、铆钉
    "herb_green": (112, 128, 78),   # 青干草药
    "herb_dry": (192, 170, 120),    # 枯黄草药 / 籽穗
    "stitch": (62, 48, 36),         # 粗针脚：深褐，比 patch 暗才读得出「缝」
}

# 袋身中心与尺寸（px，穿戴者右胯外侧）
CX, CZ = -5.8, -1.3
# (y0, y1, 半宽 x, 半深 z)：自下而上的收放，读作鼓肚束口的布袋
SLICES = (
    ("base", 3.6, 4.3, 1.0, 1.0),
    ("low", 4.3, 5.3, 1.4, 1.4),
    ("belly", 5.3, 7.6, 1.7, 1.7),
    ("shoulder", 7.6, 8.6, 1.5, 1.5),
    ("neck", 8.6, 9.4, 1.15, 1.15),
)
MOUTH_Y0, MOUTH_Y1 = 9.4, 10.5


def stalk(rig: Rig, bone: str, name: str, pts, r: float, mat: str) -> None:
    """沿折线铺草药茎：逐段短柱，避免斜长柱的轴对齐包围盒胖到蹭进手臂体积。"""
    for i, (a, b) in enumerate(zip(pts, pts[1:])):
        rig.shaft(bone, f"{name}_{i}", a, b, r, r, mat=mat)


def part_loop(rig: Rig) -> None:
    """皮带片 + 挂绳：贴在右胯前侧躯干之外，z 全在 -2.05 之前。"""
    rig.bone("hang_loop", (-4.6, 12.2, -2.4))
    # 带片：横跨右胯前，上缘 13.9 下缘 12.2；z 贴躯干前面（-2）外侧 0.55
    rig.cube("hang_loop", "belt_strip", (-4.9, 12.2, -2.62), (-0.4, 13.9, -2.05), mat="patch")
    # 上下针脚行
    for i in range(5):
        x = -4.5 + i * 0.95
        rig.cube("hang_loop", f"belt_st_t{i}", (x, 13.5, -2.82), (x + 0.4, 13.8, -2.42), mat="stitch")
        rig.cube("hang_loop", f"belt_st_b{i}", (x, 12.3, -2.82), (x + 0.4, 12.6, -2.42), mat="stitch")
    # 骨铆钉
    for i, x in enumerate((-4.1, -0.9)):
        rig.cube("hang_loop", f"rivet_{i}", (x, 12.7, -2.9), (x + 0.5, 13.2, -2.62), mat="bone")
    # 挂绳：从带片左端下沿垂到袋颈（两段，中间一个绳结）
    rig.shaft("hang_loop", "cord_a", (-4.55, 12.35, -2.5), (-4.85, 11.2, -2.55), 0.26, 0.26, mat="cord")
    rig.cube("hang_loop", "cord_knot", (-5.15, 10.7, -2.8), (-4.55, 11.3, -2.25), mat="cord")
    rig.shaft("hang_loop", "cord_b", (-4.85, 10.8, -2.55), (-5.35, 9.9, -2.6), 0.26, 0.26, mat="cord")


def _cloth_ring(rig: Rig, bone: str, tag: str, y0: float, y1: float, hw: float, hd: float, mat: str) -> None:
    """一层布：实心方块，外加四条角棱让轮廓不是死方盒。"""
    rig.cube(bone, f"{tag}_core", (CX - hw, y0, CZ - hd), (CX + hw, y1, CZ + hd), mat=mat)


def part_pouch(rig: Rig) -> None:
    """袋身：鼓肚束口布袋，自下而上逐层收放，加褶棱、颈绳和袋口褶。"""
    rig.bone("hang_pouch", (CX, 10.2, CZ - 1.0))
    for tag, y0, y1, hw, hd in SLICES:
        _cloth_ring(rig, "hang_pouch", tag, y0, y1, hw, hd, "cloth")
    # 袋底：深色底边，防磨的缝边
    rig.cube("hang_pouch", "bottom_hem", (CX - 1.05, 3.45, CZ - 1.05), (CX + 1.05, 3.7, CZ + 1.05), mat="cloth_dark")
    # 褶棱：前面三道、后面两道竖向暗褶，把大面分成布的起伏
    for i, x in enumerate((-6.7, -5.8, -4.9)):
        rig.cube("hang_pouch", f"fold_f{i}", (x - 0.17, 5.5, CZ - 1.7 - 0.2),
                 (x + 0.17, 8.3 - (i % 2) * 0.6, CZ - 1.7), mat="cloth_dark")
    for i, x in enumerate((-6.4, -5.2)):
        rig.cube("hang_pouch", f"fold_b{i}", (x - 0.17, 5.5, CZ + 1.7),
                 (x + 0.17, 7.9, CZ + 1.7 + 0.25), mat="cloth_dark")
    # 颈绳：袋颈绕三圈草绳，打结后拖出两条绳头
    rig.cube("hang_pouch", "neck_wrap_a", (CX - 1.35, 8.75, CZ - 1.35), (CX + 1.35, 9.15, CZ + 1.35), mat="cord")
    rig.cube("hang_pouch", "neck_wrap_b", (CX - 1.3, 9.2, CZ - 1.3), (CX + 1.3, 9.5, CZ + 1.3), mat="cord")
    rig.cube("hang_pouch", "neck_knot", (CX - 0.45, 8.6, CZ - 1.85), (CX + 0.45, 9.4, CZ - 1.35), mat="cord")
    rig.shaft("hang_pouch", "cord_tail_l", (CX - 0.2, 8.7, CZ - 1.75), (CX - 0.55, 7.4, CZ - 1.9), 0.2, 0.2, mat="cord")
    rig.shaft("hang_pouch", "cord_tail_r", (CX + 0.2, 8.7, CZ - 1.75), (CX + 0.7, 7.6, CZ - 1.85), 0.2, 0.2, mat="cord")
    # 骨栓：别在绳头上的小骨（横着，两头带疙瘩）
    rig.cube("hang_pouch", "toggle_bone", (CX - 0.1, 7.1, CZ - 2.2), (CX + 1.5, 7.55, CZ - 1.75), mat="bone")
    rig.cube("hang_pouch", "toggle_end_a", (CX - 0.35, 7.0, CZ - 2.25), (CX + 0.1, 7.65, CZ - 1.75), mat="bone")
    rig.cube("hang_pouch", "toggle_end_b", (CX + 1.3, 7.0, CZ - 2.25), (CX + 1.8, 7.65, CZ - 1.75), mat="bone")
    # 袋口：四面褶边，高低参差读作「束不紧」；里面一块暗色内底托住草药
    rig.cube("hang_pouch", "mouth_floor", (CX - 1.0, MOUTH_Y0, CZ - 1.0), (CX + 1.0, MOUTH_Y0 + 0.4, CZ + 1.0), mat="cloth_dark")
    rig.cube("hang_pouch", "mouth_f", (CX - 1.35, MOUTH_Y0, CZ - 1.35), (CX + 1.35, MOUTH_Y1 - 0.2, CZ - 0.85), mat="cloth")
    rig.cube("hang_pouch", "mouth_b", (CX - 1.35, MOUTH_Y0, CZ + 0.85), (CX + 1.35, MOUTH_Y1, CZ + 1.35), mat="cloth")
    rig.cube("hang_pouch", "mouth_l", (CX - 1.35, MOUTH_Y0, CZ - 0.85), (CX - 0.85, MOUTH_Y1 - 0.45, CZ + 0.85), mat="cloth")
    rig.cube("hang_pouch", "mouth_r", (CX + 0.85, MOUTH_Y0, CZ - 0.85), (CX + 1.35, MOUTH_Y1 - 0.1, CZ + 0.85), mat="cloth")
    # 褶尖：袋口边上几个翘起来的小褶角
    for i, (dx, dz, h) in enumerate(((-1.1, -1.1, 0.5), (0.2, -1.1, 0.35), (1.0, 0.9, 0.5), (-1.1, 0.7, 0.4))):
        rig.cube("hang_pouch", f"mouth_tip{i}", (CX + dx - 0.25, MOUTH_Y1 - 0.2, CZ + dz - 0.25),
                 (CX + dx + 0.25, MOUTH_Y1 - 0.2 + h, CZ + dz + 0.25), mat="cloth")


def part_patches(rig: Rig) -> None:
    """补丁与针脚：前下一块、外侧一块、袋底一块，四边粗针脚。"""
    # 前面补丁
    x0, x1, y0, y1 = -6.85, -5.05, 5.4, 7.2
    zf = CZ - 1.7
    rig.cube("hang_pouch", "patch_front", (x0, y0, zf - 0.28), (x1, y1, zf), mat="patch")
    for i in range(3):
        x = x0 + 0.25 + i * 0.6
        rig.cube("hang_pouch", f"pf_st_t{i}", (x, y1 - 0.25, zf - 0.42), (x + 0.3, y1 + 0.25, zf - 0.08), mat="stitch")
        rig.cube("hang_pouch", f"pf_st_b{i}", (x, y0 - 0.25, zf - 0.42), (x + 0.3, y0 + 0.25, zf - 0.08), mat="stitch")
    for i in range(2):
        y = y0 + 0.4 + i * 0.7
        rig.cube("hang_pouch", f"pf_st_l{i}", (x0 - 0.15, y, zf - 0.42), (x0 + 0.3, y + 0.3, zf - 0.08), mat="stitch")
        rig.cube("hang_pouch", f"pf_st_r{i}", (x1 - 0.3, y, zf - 0.42), (x1 + 0.15, y + 0.3, zf - 0.08), mat="stitch")
    # 外侧补丁（朝 -x，袋子外面）
    xo = CX - 1.7
    rig.cube("hang_pouch", "patch_side", (xo - 0.28, 6.5, CZ - 1.0), (xo, 8.3, CZ + 0.9), mat="patch")
    for i in range(3):
        z = CZ - 0.8 + i * 0.65
        rig.cube("hang_pouch", f"ps_st_t{i}", (xo - 0.42, 8.05, z), (xo - 0.08, 8.55, z + 0.3), mat="stitch")
        rig.cube("hang_pouch", f"ps_st_b{i}", (xo - 0.42, 6.25, z), (xo - 0.08, 6.75, z + 0.3), mat="stitch")
    # 袋底皮补：垫在底下
    rig.cube("hang_pouch", "patch_bottom", (CX - 1.25, 3.3, CZ - 1.25), (CX + 1.25, 3.5, CZ + 1.25), mat="patch")


def part_herbs(rig: Rig) -> None:
    """三根干草药从袋口探出：一根青叶草斜着往前探（高过 y=12 的部分在躯干之前），两根枯穗草短，不进手臂体积。"""
    rig.bone("hang_herbs", (CX, MOUTH_Y0 + 0.4, CZ))
    # 青叶草：往前（-z）斜探，叶片成对挂在上半截
    stalk(rig, "hang_herbs", "herb_a",
          [(CX - 0.3, 9.6, CZ - 0.1), (CX - 0.2, 10.8, CZ - 0.9), (CX, 11.9, CZ - 1.9),
           (CX + 0.25, 12.9, CZ - 2.6), (CX + 0.4, 13.7, CZ - 3.2)],
          0.28, "herb_green")
    for i, (dy, dz) in enumerate(((12.0, -1.9), (12.9, -2.6))):
        rig.cube("hang_herbs", f"leaf_a_l{i}", (CX - 0.75, dy, CZ + dz - 0.5), (CX + 0.15, dy + 0.4, CZ + dz + 0.1), mat="herb_green")
        rig.cube("hang_herbs", f"leaf_a_r{i}", (CX + 0.45, dy + 0.3, CZ + dz - 0.5), (CX + 1.35, dy + 0.7, CZ + dz + 0.1), mat="herb_green")
    rig.cube("hang_herbs", "leaf_a_top", (CX + 0.15, 13.5, CZ - 3.6), (CX + 0.65, 14.3, CZ - 3.0), mat="herb_green")
    # 枯穗草 B：往外（-x）斜，籽穗不越过 y=11.9
    stalk(rig, "hang_herbs", "herb_b",
          [(CX - 0.9, 9.6, CZ + 0.3), (CX - 1.4, 10.7, CZ + 0.3), (CX - 1.8, 11.4, CZ + 0.3)],
          0.24, "herb_dry")
    rig.cube("hang_herbs", "seed_b", (CX - 2.1, 11.3, CZ + 0.05), (CX - 1.5, 11.9, CZ + 0.55), mat="herb_dry")
    # 枯穗草 C：往后（+z）斜
    stalk(rig, "hang_herbs", "herb_c",
          [(CX + 0.7, 9.6, CZ + 0.5), (CX + 1.0, 10.7, CZ + 0.9), (CX + 1.2, 11.4, CZ + 1.3)],
          0.24, "herb_dry")
    rig.cube("hang_herbs", "seed_c", (CX + 0.95, 11.3, CZ + 1.05), (CX + 1.5, 11.9, CZ + 1.6), mat="herb_dry")


PARTS = {
    "loop": part_loop,
    "pouch": part_pouch,
    "patches": part_patches,
    "herbs": part_herbs,
}
ORDER = ["loop", "pouch", "patches", "herbs"]


def build(parts: list[str] | None = None, rig: Rig | None = None) -> Rig:
    rig = rig or Rig(MATS)
    for key in (parts or ORDER):
        if key == "patches" and "pouch" not in (parts or ORDER):
            rig.bone("hang_pouch", (CX, 10.2, CZ - 1.0))
        PARTS[key](rig)
    return rig


# 软覆盖：软件盖硬件 / 同件内贴合是设计意图；硬对硬互穿才是缺陷
SOFT_OVER = frozenset({
    frozenset(("cloth", "cloth_dark")), frozenset(("cloth", "patch")), frozenset(("cloth", "stitch")),
    frozenset(("cloth", "cord")), frozenset(("cloth", "bone")),
    frozenset(("cloth_dark", "cord")), frozenset(("cloth_dark", "herb_green")),
    frozenset(("cloth_dark", "herb_dry")), frozenset(("cloth", "herb_green")), frozenset(("cloth", "herb_dry")),
    frozenset(("patch", "stitch")), frozenset(("patch", "cord")), frozenset(("patch", "bone")),
    frozenset(("cord", "bone")), frozenset(("cord", "stitch")),
})

GATES = gc.GearGates(
    "灵草囊 / herb_pouch",
    MATS,
    symmetric=False,
    soft_over=SOFT_OVER,
)

NOTE = "注：比例/挂点是否读得出，自检量不出 —— 看 check.png 的灰模三面。"


def check(rig: Rig) -> int:
    return GATES.report(rig, px=gc.PX, note=NOTE)


def main() -> int:
    ap = argparse.ArgumentParser(description="灵草囊穿戴件生成器")
    ap.add_argument("--part", help="只生成单件（调试用）")
    ap.add_argument("--check", action="store_true", help="只跑自检，不写盘")
    ap.add_argument("--self-test", action="store_true", help="差分自证：每道门注入它该抓的缺陷")
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

    gc.write_bbmodel(rig, "HerbPouch", BBMODEL_OUT)
    shutil.copyfile(BBMODEL_OUT, BBMODEL_COPY)
    notes = [f"cubes {len(rig.elements)} / bones {len(rig.bones)} / gates 0 violations"
             if not bad else f"! gates: {bad} violations"]
    parts_png = gc.render_parts(build, ORDER, MATS, GEAR_ID, SCRATCH)
    check_png = gc.render_check(build, MATS, GEAR_ID,
                                "herb_pouch (round 1/3) - waist-side herb pouch worn on grey mannequin",
                                SCRATCH, notes=notes, close_pad=3.5,
                                close_bias=(-1.5, 0.0, -1.0))
    print(f"  → {BBMODEL_OUT.relative_to(REPO)}\n  → {parts_png}\n  → {check_png}")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
