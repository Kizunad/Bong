#!/usr/bin/env python3
"""水囊（water_skin）穿戴件生成器 —— Round 1/3。

server/assets/items/workbench_materials.toml：「兽皮缝的水袋，远征不渴。」
任务卡 #3：斜挎，整张兽皮缝的水袋，骨塞口，皮绳斜挎。参考图见
``model-review/img/gear/water_skin/{ref,exploded}.png``。

穿戴（Bedrock geo 系，脚 y=0，+z 背面；穿戴者右肩 = -x，左胯 = +x）：
  · 皮绳从右肩顶（y=24.05，躯干/手臂顶面之上、头体积之外 x<-4.2）翻过去，前后各一条斜带，
    贴在躯干前面/背面之外（z∈[-2.6,-2.05] / [2.05,2.6]），往下斜到左胯，在手臂体积底（y<12）以下
    收进袋颈的铁环 —— 带子一路只在手臂体积的前后侧（z 在 ±2 之外），不扎进手臂
  · 水囊挂在左胯外侧：x∈[4.2,8.2]、y∈[2.2,10.0]，整个在手臂静止体积之下、腿之外
  · 骨塞插在袋颈，塞顶 y≤11.9，不进手臂体积
  · 袋身用两种兽皮拼成（左半浅色、右半褐色，上下对调），带补丁、背缝针脚、四个带蹄的皮腿

bone：
  strap_shoulder  过肩带（右肩顶，连前后两条斜带）
  strap_front     前斜带 + 铁环端头 + 绳缠
  strap_back      后斜带 + 铁环端头 + 绳缠
  hang_skin       袋身 + 皮腿 + 蹄 + 颈绳 + 补丁与针脚
  hang_stopper    骨塞 + 塞绳

用法：
  python3 modelScript/gear/water_skin/gen_water_skin.py              # 全件 + parts.png + check.png
  python3 modelScript/gear/water_skin/gen_water_skin.py --self-test  # 门禁差分自证
  python3 modelScript/gear/water_skin/gen_water_skin.py --check
  python3 modelScript/gear/water_skin/gen_water_skin.py --part skin
"""

from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "gear"))
import gear_common as gc  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig, shaft_box  # noqa: E402

GEAR_ID = "water_skin"
HERE = Path(__file__).resolve().parent
BBMODEL_OUT = HERE / "WaterSkin.bbmodel"
BBMODEL_COPY = gc.MODELS_DIR / "WaterSkin.bbmodel"
SCRATCH = REPO / "modelScript" / "out" / "gear" / GEAR_ID

# 材质 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」
MATS = {
    "hide_pale": (196, 176, 140),    # 浅色兽皮（毛面朝外，旧）
    "hide_brown": (128, 86, 60),     # 褐色兽皮
    "strap": (112, 72, 50),          # 皮绳带
    "cord": (136, 118, 74),          # 草绳缠 / 颈绳
    "bone": (220, 210, 182),         # 骨塞
    "iron": (88, 88, 90),            # 铁环
    "stitch": (62, 48, 36),          # 粗针脚
    "hoof": (58, 46, 40),            # 蹄
}

CX, CZ = 6.2, 0.0            # 水囊中心（穿戴者左胯外侧）
# (名, y0, y1, 半宽 x, 半深 z)
SLICES = (
    ("base", 3.4, 4.3, 1.3, 1.3),
    ("low", 4.3, 5.5, 1.8, 1.75),
    ("belly", 5.5, 7.6, 1.98, 1.9),
    ("shoulder", 7.6, 8.8, 1.55, 1.5),
    ("neck", 8.8, 10.0, 0.8, 0.8),
)
# 两半兽皮拼色：每层 (左半, 右半)，上下对调读作拼缝皮
HALVES = {
    "base": ("hide_brown", "hide_pale"),
    "low": ("hide_brown", "hide_pale"),
    "belly": ("hide_pale", "hide_brown"),
    "shoulder": ("hide_pale", "hide_brown"),
    "neck": ("hide_brown", "hide_brown"),
}

# 皮绳走位（前后同 x/y，z 贴躯干前/背面外侧）
SHOULDER_X = (-5.9, -4.2)
STRAP_A = (-5.05, 20.6)      # 肩下起点 (x, y)
STRAP_B = (4.95, 12.75)      # 袋端终点 (x, y)
STRAP_T = 0.56               # 带厚（z）
STRAP_HALF_W = 0.75          # 带半宽（沿斜带的横向）
Z_FRONT = -2.33              # 前带中心 z（躯干前面 -2，外侧 0.33）
Z_BACK = 2.33


def _flat_band(rig: Rig, bone: str, name: str, a, b, half_w: float, half_t: float, z_up, mat: str) -> None:
    """斜带：a→b 的平板，薄的那一维朝 z（贴着躯干前/背面）。"""
    frm, to, rot, org = shaft_box(a, b, half_w, half_t, up=z_up)
    rig.cube(bone, name, frm, to, rot=rot, org=org, mat=mat)


def _lerp(a, b, t: float):
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


def part_shoulder(rig: Rig) -> None:
    """过肩带：右肩顶一块横跨前后的平板，前后各一道下垂的立带，接两条斜带。"""
    rig.bone("strap_shoulder", (-5.0, 24.3, 0.0))
    x0, x1 = SHOULDER_X
    rig.cube("strap_shoulder", "shoulder_top", (x0, 24.05, -2.6), (x1, 24.6, 2.6), mat="strap")
    rig.cube("strap_shoulder", "drop_front", (x0, 20.4, -2.6), (x1, 24.6, -2.05), mat="strap")
    rig.cube("strap_shoulder", "drop_back", (x0, 20.4, 2.05), (x1, 24.6, 2.6), mat="strap")
    # 肩顶磨亮的一道：前后各缝一道针脚压带
    for i, z in enumerate((-2.1, 1.8)):
        rig.cube("strap_shoulder", f"shoulder_st{i}", (x0 + 0.3, 24.5, z), (x1 - 0.3, 24.8, z + 0.3), mat="stitch")


def _diagonal(rig: Rig, bone: str, tag: str, zc: float, z_up, outward: float) -> None:
    """一侧斜带：肩下 → 袋端，加两处绳缠、末端立带和通向铁环的横舌。"""
    a = (STRAP_A[0], STRAP_A[1], zc)
    b = (STRAP_B[0], STRAP_B[1], zc)
    _flat_band(rig, bone, f"band_{tag}", a, b, STRAP_HALF_W, STRAP_T / 2, z_up, "strap")
    # 绳缠：沿带取两个点，套一圈草绳（比带宽、略厚）
    for i, t in enumerate((0.3, 0.72)):
        c = _lerp(a, b, t)
        d = (b[0] - a[0], b[1] - a[1])
        n = (d[0] ** 2 + d[1] ** 2) ** 0.5
        u = (d[0] / n * 0.28, d[1] / n * 0.28)
        p0 = (c[0] - u[0], c[1] - u[1], zc)
        p1 = (c[0] + u[0], c[1] + u[1], zc)
        _flat_band(rig, bone, f"wrap_{tag}{i}", p0, p1, STRAP_HALF_W + 0.22, STRAP_T / 2 + 0.17, z_up, "cord")
    # 末端：立带下到袋颈高度，再折一条横舌向袋颈（y<12，躲开手臂体积）
    zlo, zhi = (zc - STRAP_T / 2, zc + STRAP_T / 2)
    rig.cube(bone, f"end_drop_{tag}", (4.3, 10.6, zlo), (5.6, 13.0, zhi), mat="strap")
    z_in = zc + outward * 1.4
    rig.cube(bone, f"end_tongue_{tag}", (4.3, 10.4, min(zlo, z_in)), (5.6, 11.1, max(zhi, z_in)), mat="strap")
    rig.cube(bone, f"end_st_{tag}", (4.6, 11.6, zlo - 0.15 if outward > 0 else zhi - 0.1),
             (5.3, 12.2, zlo + 0.1 if outward > 0 else zhi + 0.15), mat="stitch")


def part_front(rig: Rig) -> None:
    """前斜带：从右肩前沿贴着胸前斜到左胯，末端折舌接铁环。"""
    rig.bone("strap_front", (-5.0, 22.0, Z_FRONT))
    _diagonal(rig, "strap_front", "f", Z_FRONT, (0.0, 0.0, -1.0), outward=1.0)


def part_back(rig: Rig) -> None:
    """后斜带：与前带对称地贴背斜到左胯，在袋颈铁环汇合。"""
    rig.bone("strap_back", (-5.0, 22.0, Z_BACK))
    _diagonal(rig, "strap_back", "b", Z_BACK, (0.0, 0.0, 1.0), outward=-1.0)


def part_skin(rig: Rig) -> None:
    """袋身：两种兽皮左右拼色的鼓肚水囊，四条带蹄的皮腿，补丁、背缝针脚与铁环颈。"""
    rig.bone("hang_skin", (CX, 10.4, CZ))
    for tag, y0, y1, hw, hd in SLICES:
        left, right = HALVES[tag]
        rig.cube("hang_skin", f"{tag}_l", (CX - hw, y0, CZ - hd), (CX, y1, CZ + hd), mat=left)
        rig.cube("hang_skin", f"{tag}_r", (CX, y0, CZ - hd), (CX + hw, y1, CZ + hd), mat=right)
    # 皮腿：袋底四角各一条短腿（兽皮原来的四肢），腿底带深色蹄
    for i, (sx, sz) in enumerate(((-1, -1), (1, -1), (-1, 1), (1, 1))):
        x0 = CX + sx * 1.15 - 0.45
        z0 = CZ + sz * 1.0 - 0.45
        mat = "hide_pale" if (sx + sz) % 4 else "hide_brown"
        rig.cube("hang_skin", f"leg_{i}", (x0, 2.8, z0), (x0 + 0.9, 3.7, z0 + 0.9), mat=mat)
        rig.cube("hang_skin", f"hoof_{i}", (x0 - 0.05, 2.25, z0 - 0.05), (x0 + 0.95, 2.95, z0 + 0.95), mat="hoof")
    # 补丁：前面浅色半边上一块褐补丁，四边针脚
    px0, px1, py0, py1 = CX - 1.7, CX - 0.2, 5.8, 7.4
    zf = CZ - 1.9
    rig.cube("hang_skin", "patch", (px0, py0, zf - 0.28), (px1, py1, zf), mat="hide_brown")
    for i in range(3):
        x = px0 + 0.1 + i * 0.5
        rig.cube("hang_skin", f"pa_st_t{i}", (x, py1 - 0.25, zf - 0.42), (x + 0.3, py1 + 0.25, zf - 0.08), mat="stitch")
        rig.cube("hang_skin", f"pa_st_b{i}", (x, py0 - 0.25, zf - 0.42), (x + 0.3, py0 + 0.25, zf - 0.08), mat="stitch")
    for i in range(2):
        y = py0 + 0.35 + i * 0.6
        rig.cube("hang_skin", f"pa_st_l{i}", (px0 - 0.15, y, zf - 0.42), (px0 + 0.3, y + 0.3, zf - 0.08), mat="stitch")
        rig.cube("hang_skin", f"pa_st_r{i}", (px1 - 0.3, y, zf - 0.42), (px1 + 0.15, y + 0.3, zf - 0.08), mat="stitch")
    # 背缝：袋背中线一排粗针脚（皮缝合处）
    for i in range(6):
        y = 4.4 + i * 0.85
        # 针脚贴在该高度那一层的背面上，肩部收窄处要跟着往里走
        hd = next(h for _, y0, y1, _, h in SLICES if y0 <= y + 0.2 < y1)
        zb = CZ + hd
        rig.cube("hang_skin", f"seam_{i}", (CX - 0.2, y, zb - 0.1), (CX + 0.2, y + 0.45, zb + 0.34), mat="stitch")
    # 颈：铁环箍住袋颈，两条带舌系在环上；环下一圈绳缠
    rig.cube("hang_skin", "neck_ring", (CX - 1.15, 9.95, CZ - 1.15), (CX + 1.15, 10.5, CZ + 1.15), mat="iron")
    rig.cube("hang_skin", "neck_wrap", (CX - 0.98, 9.3, CZ - 0.98), (CX + 0.98, 9.75, CZ + 0.98), mat="cord")
    rig.cube("hang_skin", "neck_hem", (CX - 0.95, 10.45, CZ - 0.95), (CX + 0.95, 10.8, CZ + 0.95), mat="hide_brown")


def part_stopper(rig: Rig) -> None:
    """骨塞：塞进袋颈，顶上是蘑菇帽，塞颈缠绳、拖一个绳圈。"""
    rig.bone("hang_stopper", (CX, 10.8, CZ))
    rig.cube("hang_stopper", "plug", (CX - 0.55, 9.9, CZ - 0.55), (CX + 0.55, 11.4, CZ + 0.55), mat="bone")
    rig.cube("hang_stopper", "cap", (CX - 0.95, 11.2, CZ - 0.95), (CX + 0.95, 11.8, CZ + 0.95), mat="bone")
    rig.cube("hang_stopper", "cap_nub", (CX - 0.4, 11.7, CZ - 0.4), (CX + 0.4, 11.9, CZ + 0.4), mat="bone")
    rig.cube("hang_stopper", "plug_wrap", (CX - 0.72, 10.85, CZ - 0.72), (CX + 0.72, 11.2, CZ + 0.72), mat="cord")
    # 塞绳：从塞颈拖出一小截绳圈，系在铁环上
    rig.shaft("hang_stopper", "cord_a", (CX + 0.6, 11.0, CZ + 0.1), (CX + 1.5, 10.7, CZ + 0.3), 0.2, 0.2, mat="cord")
    rig.cube("hang_stopper", "cord_loop", (CX + 1.35, 10.3, CZ + 0.05), (CX + 1.85, 10.9, CZ + 0.55), mat="cord")


PARTS = {
    "shoulder": part_shoulder,
    "front": part_front,
    "back": part_back,
    "skin": part_skin,
    "stopper": part_stopper,
}
ORDER = ["shoulder", "front", "back", "skin", "stopper"]


def build(parts: list[str] | None = None, rig: Rig | None = None) -> Rig:
    rig = rig or Rig(MATS)
    for key in (parts or ORDER):
        PARTS[key](rig)
    return rig


SOFT_OVER = frozenset({
    frozenset(("hide_pale", "hide_brown")), frozenset(("hide_pale", "stitch")),
    frozenset(("hide_brown", "stitch")), frozenset(("hide_pale", "hoof")), frozenset(("hide_brown", "hoof")),
    frozenset(("hide_pale", "cord")), frozenset(("hide_brown", "cord")),
    frozenset(("hide_pale", "bone")), frozenset(("hide_brown", "bone")),
    frozenset(("hide_pale", "iron")), frozenset(("hide_brown", "iron")),
    frozenset(("strap", "cord")), frozenset(("strap", "stitch")), frozenset(("strap", "iron")),
    frozenset(("strap", "hide_brown")), frozenset(("strap", "hide_pale")),
    frozenset(("bone", "cord")), frozenset(("bone", "iron")), frozenset(("cord", "iron")),
    frozenset(("cord", "stitch")),
})

GATES = gc.GearGates(
    "水囊 / water_skin",
    MATS,
    symmetric=False,
    soft_over=SOFT_OVER,
)

NOTE = "注：比例/挂点是否读得出，自检量不出 —— 看 check.png 的灰模三面。"


def check(rig: Rig) -> int:
    return GATES.report(rig, px=gc.PX, note=NOTE)


def main() -> int:
    ap = argparse.ArgumentParser(description="水囊穿戴件生成器")
    ap.add_argument("--part", help="只生成单件（调试用）")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()

    if args.list:
        for k in ORDER:
            print(f"  {k:9s} {(PARTS[k].__doc__ or '').strip().splitlines()[0]}")
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

    gc.write_bbmodel(rig, "WaterSkin", BBMODEL_OUT)
    shutil.copyfile(BBMODEL_OUT, BBMODEL_COPY)
    notes = [f"cubes {len(rig.elements)} / bones {len(rig.bones)} / "
             + ("gates 0 violations" if not bad else f"! gates: {bad} violations")]
    parts_png = gc.render_parts(build, ORDER, MATS, GEAR_ID, SCRATCH)
    check_png = gc.render_check(build, MATS, GEAR_ID,
                                "water_skin (round 1/3) - diagonal-sling hide water skin worn on grey mannequin",
                                SCRATCH, notes=notes, close_at=((3.0, 14.0, 0.0), 22.0))
    print(f"  → {BBMODEL_OUT.relative_to(REPO)}\n  → {parts_png}\n  → {check_png}")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
