#!/usr/bin/env python3
"""防潮包（moisture_guard）穿戴件生成器 —— Round 1/3。

server/assets/items/workbench_materials.toml：「灰烬苔裹布包，吸湿防霉。」
任务卡 #4：腰侧挂件，灰烬苔裹的布包，外层苔藓、捆扎绳。参考图见
``model-review/img/gear/moisture_guard/{ref,exploded}.png``。

穿戴（Bedrock geo 系，脚 y=0，+z 背面；穿戴者左腰 = +x，与右腰的灵草囊分边）：
  · 布包悬在左胯外侧：核心 x∈[4.6,7.0]、y∈[4.2,10.6]、z∈[-1.9,1.9]，苔壳再外包 0.45~0.55，
    整个在手臂静止体积（y≥12）之下、腿（x<4）之外
  · 腰上两道草绳只走躯干前、背面外侧（|z|∈[2.05,2.6]），不绕进手臂体积；前面的绳垂一个绳圈挂住布包
  · 苔壳以顶面和外侧为主，前后两端留布面（折边 + 补丁），底缘垂下长短不一的苔须
  · 两道横绳 + 前后竖绳十字捆扎，绳结在前面

bone：
  strap_waist  腰绳前后两段 + 绳圈（挂点）
  hang_bundle  布包核心 + 端部折边 + 补丁 + 针脚
  hang_moss    灰烬苔壳（顶 / 外侧 / 内侧 / 端部碎苔）+ 垂下的苔须
  hang_lash    捆扎绳（横两道 + 前后竖绳 + 绳结）

用法：
  python3 modelScript/gear/moisture_guard/gen_moisture_guard.py              # 全件 + parts.png + check.png
  python3 modelScript/gear/moisture_guard/gen_moisture_guard.py --self-test  # 门禁差分自证
  python3 modelScript/gear/moisture_guard/gen_moisture_guard.py --check
  python3 modelScript/gear/moisture_guard/gen_moisture_guard.py --part moss
"""

from __future__ import annotations

import argparse
import random
import shutil
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "gear"))
import gear_common as gc  # noqa: E402
from bbmodel_maker.rig.rigkit import Rig  # noqa: E402

GEAR_ID = "moisture_guard"
HERE = Path(__file__).resolve().parent
BBMODEL_OUT = HERE / "MoistureGuard.bbmodel"
BBMODEL_COPY = gc.MODELS_DIR / "MoistureGuard.bbmodel"
SCRATCH = REPO / "modelScript" / "out" / "gear" / GEAR_ID

# 材质 ≤ 8 种：rigkit 把 color 写成「材质序号 % 8」
MATS = {
    "cloth": (170, 156, 128),        # 旧布：灰米，沾潮渍
    "cloth_dark": (118, 96, 74),     # 布补丁 / 折边阴面
    "moss": (112, 124, 78),          # 灰烬苔：灰绿
    "moss_dark": (74, 88, 58),       # 苔暗处
    "ash": (150, 150, 136),          # 苔尖的灰白（灰烬）
    "cord": (140, 114, 74),          # 草绳
    "stitch": (62, 48, 36),          # 粗针脚
    "patch": (124, 78, 58),          # 锈褐皮补
}

# 布包核心（穿戴者左胯外侧）
X0, X1 = 4.6, 7.0
Y0, Y1 = 4.2, 10.6
Z0, Z1 = -1.9, 1.9
SHELL = 0.5                # 苔壳厚度（外包）


def _moss_mat(rng: random.Random) -> str:
    r = rng.random()
    if r < 0.5:
        return "moss"
    if r < 0.8:
        return "moss_dark"
    return "ash"


def part_waist(rig: Rig) -> None:
    """腰绳前后两段 + 绳圈：只走躯干前/背面外侧（z 在 ±2.05 之外），不进手臂体积。"""
    rig.bone("strap_waist", (4.8, 12.6, -2.35))
    for tag, z0, z1 in (("f", -2.6, -2.05), ("b", 2.05, 2.6)):
        for i, y in enumerate((12.25, 12.8)):
            rig.cube("strap_waist", f"rope_{tag}{i}", (-0.5, y, z0), (5.0, y + 0.5, z1), mat="cord")
    # 前面绳结：腰绳尽头打个结，下接绳圈
    rig.cube("strap_waist", "knot", (4.5, 12.1, -2.75), (5.5, 13.4, -2.05), mat="cord")
    # 绳圈：两条竖绳 + 顶横，垂到布包前上角
    rig.cube("strap_waist", "loop_l", (4.7, 11.0, -2.6), (5.2, 12.4, -2.1), mat="cord")
    rig.cube("strap_waist", "loop_r", (5.7, 11.0, -2.6), (6.2, 12.4, -2.1), mat="cord")
    rig.cube("strap_waist", "loop_top", (4.7, 11.95, -2.6), (6.2, 12.45, -2.1), mat="cord")


def part_bundle(rig: Rig) -> None:
    """布包核心：粗布长方包，两端折边外翻，一块皮补丁，针脚压边。"""
    rig.bone("hang_bundle", (5.8, 10.6, 0.0))
    rig.cube("hang_bundle", "core", (X0, Y0, Z0), (X1, Y1, Z1), mat="cloth")
    # 端部折边（前后各一）：布向外翻出的两道折，比核心略宽略凸
    for tag, z0, z1 in (("f", Z0 - 0.45, Z0 + 0.1), ("b", Z1 - 0.1, Z1 + 0.45)):
        rig.cube("hang_bundle", f"fold_{tag}0", (X0 + 0.1, Y0 + 0.5, z0), (X1 - 0.1, Y1 - 0.6, z1), mat="cloth_dark")
        rig.cube("hang_bundle", f"fold_{tag}1", (X0 + 0.5, Y0 + 1.0, z0 - (0.15 if tag == "f" else -0.15)),
                 (X1 - 0.5, Y1 - 1.4, z1 + (0.15 if tag == "b" else -0.15)), mat="cloth")
    # 端面补丁：前端面一块皮补，四边针脚
    pz = Z0 - 0.45
    px0, px1, py0, py1 = X0 + 0.5, X1 - 0.5, 6.0, 8.6
    rig.cube("hang_bundle", "patch_f", (px0, py0, pz - 0.28), (px1, py1, pz), mat="patch")
    for i in range(4):
        x = px0 + 0.1 + i * 0.55
        rig.cube("hang_bundle", f"pst_t{i}", (x, py1 - 0.25, pz - 0.42), (x + 0.3, py1 + 0.25, pz - 0.08), mat="stitch")
        rig.cube("hang_bundle", f"pst_b{i}", (x, py0 - 0.25, pz - 0.42), (x + 0.3, py0 + 0.25, pz - 0.08), mat="stitch")
    # 侧缝针脚：包身外侧一道竖缝
    for i in range(5):
        y = 5.0 + i * 1.0
        rig.cube("hang_bundle", f"seam_{i}", (X1 - 0.1, y, -0.2), (X1 + 0.28, y + 0.5, 0.2), mat="stitch")


def part_moss(rig: Rig) -> None:
    """灰烬苔壳：顶面 / 外侧 / 内侧成片，前后端碎苔，底缘垂下苔须（确定性随机，固定种子）。"""
    rig.bone("hang_moss", (5.8, 10.6, 0.0))
    rng = random.Random(0x4A5B)

    def tile(name: str, lo, hi) -> None:
        rig.cube("hang_moss", name, lo, hi, mat=_moss_mat(rng))

    # 顶面：4×3 方格，高度参差
    nx, nz = 4, 3
    sx = (X1 - X0 + 2 * SHELL) / nx
    sz = (Z1 - Z0 + 2 * SHELL) / nz
    for i in range(nx):
        for j in range(nz):
            h = 0.5 + rng.random() * 0.6
            x = X0 - SHELL + i * sx
            z = Z0 - SHELL + j * sz
            tile(f"top_{i}{j}", (x, Y1 - 0.3, z), (x + sx + 0.15, Y1 + h, z + sz + 0.15))
    # 外侧（+x）：3 列（沿 z）× 4 行（沿 y）
    ny = 4
    sy = (Y1 - Y0) / ny
    for j in range(3):
        for k in range(ny):
            t = 0.35 + rng.random() * 0.35
            z = Z0 - SHELL + j * sz
            y = Y0 + k * sy
            tile(f"out_{j}{k}", (X1 - 0.25, y, z), (X1 + t, y + sy + 0.15, z + sz + 0.15))
    # 内侧（-x）：只上半截，且不进腿体积（x>=4.05）
    for j in range(3):
        for k in range(2, ny):
            z = Z0 - SHELL + j * sz
            y = Y0 + k * sy
            tile(f"in_{j}{k}", (4.05, y, z), (X0 + 0.25, y + sy + 0.15, z + sz + 0.15))
    # 端部碎苔：前后端面各三小块，不全盖，露出布面
    for tag, zc in (("f", Z0 - 0.45), ("b", Z1 + 0.45)):
        for i, (dx, dy) in enumerate(((0.3, 8.8), (1.7, 5.4), (0.9, 9.6))):
            x = X0 + dx
            z0 = zc - 0.3 if tag == "f" else zc - 0.35
            tile(f"end_{tag}{i}", (x, dy, z0 - (0.2 if tag == "f" else 0.0)), (x + 0.9, dy + 0.9, z0 + 0.65 + (0.0 if tag == "f" else 0.2)))
    # 苔须：从底缘和两侧缘垂下，长短不一
    for i in range(14):
        x = X0 - 0.3 + rng.random() * (X1 - X0 + 0.9)
        x = max(x, 4.05)
        z = Z0 - SHELL + rng.random() * (Z1 - Z0 + 2 * SHELL - 0.5)
        length = 0.9 + rng.random() * 1.5
        top = Y0 + 0.5
        rig.cube("hang_moss", f"hang_{i}", (x, top - length, z), (x + 0.5, top, z + 0.5),
                 mat="moss" if i % 3 else "moss_dark")


def part_lash(rig: Rig) -> None:
    """捆扎绳：横两道绕整包，前后各一条竖绳，十字处打结。"""
    rig.bone("hang_lash", (5.8, 7.5, 0.0))
    ox0, ox1 = X0 - SHELL + 0.1, X1 + SHELL - 0.1     # 绳在苔壳外
    oz0, oz1 = Z0 - SHELL - 0.15, Z1 + SHELL + 0.15
    for i, y in enumerate((6.7, 7.4)):
        rig.cube("hang_lash", f"ring_f{i}", (ox0, y, oz0 - 0.4), (ox1, y + 0.5, oz0 + 0.2), mat="cord")
        rig.cube("hang_lash", f"ring_b{i}", (ox0, y, oz1 - 0.2), (ox1, y + 0.5, oz1 + 0.4), mat="cord")
        rig.cube("hang_lash", f"ring_l{i}", (ox0, y, oz0 - 0.4), (ox0 + 0.5, y + 0.5, oz1 + 0.4), mat="cord")
        rig.cube("hang_lash", f"ring_r{i}", (ox1 - 0.5, y, oz0 - 0.4), (ox1, y + 0.5, oz1 + 0.4), mat="cord")
    # 竖绳：前后各一条，从底缘到顶面
    for tag, z0, z1 in (("f", oz0 - 0.4, oz0 + 0.2), ("b", oz1 - 0.2, oz1 + 0.4)):
        rig.cube("hang_lash", f"vert_{tag}", (5.45, 4.5, z0), (6.05, 11.2, z1), mat="cord")
    # 顶面竖绳连接前后
    rig.cube("hang_lash", "vert_top", (5.45, 11.0, oz0 - 0.4), (6.05, 11.55, oz1 + 0.4), mat="cord")
    # 前面绳结
    rig.cube("hang_lash", "knot", (5.2, 6.6, oz0 - 0.65), (6.3, 7.7, oz0 - 0.05), mat="cord")


PARTS = {
    "waist": part_waist,
    "bundle": part_bundle,
    "moss": part_moss,
    "lash": part_lash,
}
ORDER = ["waist", "bundle", "moss", "lash"]


def build(parts: list[str] | None = None, rig: Rig | None = None) -> Rig:
    rig = rig or Rig(MATS)
    for key in (parts or ORDER):
        PARTS[key](rig)
    return rig


SOFT_OVER = frozenset({
    frozenset(("cloth", "cloth_dark")), frozenset(("cloth", "patch")), frozenset(("cloth", "stitch")),
    # 注意：cloth×moss 故意不放行 —— 苔壳只许薄薄包在布面外（咬入 <0.55），苔块埋进布核心才是缺陷；
    # 放行它会让穿模门在本件上没有任何「厚实的硬件对」可注入，差分自证造不出缺陷
    frozenset(("cloth_dark", "moss")), frozenset(("cloth_dark", "moss_dark")), frozenset(("cloth_dark", "ash")),
    frozenset(("cloth_dark", "patch")), frozenset(("cloth_dark", "stitch")),
    frozenset(("patch", "stitch")), frozenset(("patch", "moss")), frozenset(("patch", "moss_dark")),
    frozenset(("cord", "moss")), frozenset(("cord", "moss_dark")), frozenset(("cord", "ash")),
    frozenset(("cord", "cloth")), frozenset(("cord", "cloth_dark")), frozenset(("cord", "patch")),
    frozenset(("cord", "stitch")),
    frozenset(("moss", "moss_dark")), frozenset(("moss", "ash")), frozenset(("moss_dark", "ash")),
    frozenset(("stitch", "moss")), frozenset(("stitch", "moss_dark")), frozenset(("stitch", "ash")),
})

GATES = gc.GearGates(
    "防潮包 / moisture_guard",
    MATS,
    symmetric=False,
    soft_over=SOFT_OVER,
)

NOTE = "注：比例/挂点是否读得出，自检量不出 —— 看 check.png 的灰模三面。"


def check(rig: Rig) -> int:
    return GATES.report(rig, px=gc.PX, note=NOTE)


def main() -> int:
    ap = argparse.ArgumentParser(description="防潮包穿戴件生成器")
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

    gc.write_bbmodel(rig, "MoistureGuard", BBMODEL_OUT)
    shutil.copyfile(BBMODEL_OUT, BBMODEL_COPY)
    notes = [f"cubes {len(rig.elements)} / bones {len(rig.bones)} / "
             + ("gates 0 violations" if not bad else f"! gates: {bad} violations")]
    parts_png = gc.render_parts(build, ORDER, MATS, GEAR_ID, SCRATCH)
    check_png = gc.render_check(build, MATS, GEAR_ID,
                                "moisture_guard (round 1/3) - mossy waist-side bundle worn on grey mannequin",
                                SCRATCH, notes=notes, close_at=((5.0, 9.0, 0.0), 15.0))
    print(f"  → {BBMODEL_OUT.relative_to(REPO)}\n  → {parts_png}\n  → {check_png}")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
