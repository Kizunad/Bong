#!/usr/bin/env python3
"""杂皮行囊（za_pi_xing_nang）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #8（.task-wasteland-gear.md）。穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

外形：几种兽皮（灰 / 褐 / 浅 / 斑）拼缝成的卷筒行囊，横着背在肩后（x∈±9、y∈[17.6,24]，顶面正好顶在头底下，
z∈[2.6,8.6]，高出的部分都在头后面）；两头收口，扎草绳，封一块卷纹圆盖；背面用骨白色的粗针脚缝着大小不一的皮块；
左右各一道布带箍住，带上有锈铁扣片和骨栓；底下垂着几件骨坠 / 布签。肩带 + 胸带 + 腰带走 _kit/harness.py
（骑过手臂上沿，不进躯干）。

分部件：part_*() 逐件写，--part 单件预览；骨骼树在 BONES 里一处定义，按挂点分骨。
用法：
    python3 modelScript/gear/za_pi_xing_nang/gen_za_pi_xing_nang.py --self-test   # 门禁差分自证
    python3 modelScript/gear/za_pi_xing_nang/gen_za_pi_xing_nang.py --part patch  # 单件预览
    python3 modelScript/gear/za_pi_xing_nang/gen_za_pi_xing_nang.py --worn        # 只出穿戴三面
    python3 modelScript/gear/za_pi_xing_nang/gen_za_pi_xing_nang.py               # 全量：模型 + check.png
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
import harness  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质（RGB，每种占贴图一个 8×8 色块）
MATS = {
    "hide":       (126, 86, 62),     # 褐皮（囊身底色）
    "hide_dk":    (92, 66, 50),      # 深褐
    "hide_lt":    (156, 114, 82),
    "hide_grey":  (126, 120, 110),   # 灰皮
    "hide_pale":  (184, 158, 120),   # 浅皮
    "hide_spot":  (70, 54, 44),      # 斑皮的暗斑
    "hide_stain": (102, 78, 58),
    "cloth":      (166, 146, 112),   # 布带
    "cloth_dk":   (124, 106, 80),
    "cord":       (154, 130, 86),
    "cord_dk":    (108, 88, 56),
    "bone_lt":    (228, 218, 190),   # 骨针脚 / 骨栓
    "bone_dk":    (164, 150, 120),
    "bone_stain": (126, 106, 78),
    "iron":       (88, 80, 76),
    "rust":       (150, 84, 48),
    "rust_dk":    (104, 56, 34),
    "void":       (28, 24, 22),
}
SOFT = ("hide", "hide_dk", "hide_lt", "hide_grey", "hide_pale", "hide_spot", "hide_stain", "cloth", "cloth_dk", "cord", "cord_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("iron", "rust", "hide_dk", "cloth", "hide"))

HARNESS = harness.HarnessMats(
    strap="cloth", strap_dk="hide_stain", pad="hide", stitch="cord_dk", hard="iron", hard_pin="rust",
    hard_rust="rust_dk", toggle="bone_lt", toggle_pin="bone_stain", hole="void")

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("roll_body", "root", (0.0, 21.0, 5.6), "皮囊身：中段 + 上下削角 + 两头收口的颈"),
    ("roll_cap_r", "roll_body", (8.6, 20.8, 5.6), "右端封口盖：卷纹圆盖 + 草绳扎口 + 交叉绑绳"),
    ("roll_cap_l", "roll_body", (-8.6, 20.8, 5.6), "左端封口盖（镜像）"),
    ("roll_patch", "roll_body", (0.0, 21.0, 8.6), "背面大小不一的拼皮块（左右不对称）"),
    ("roll_stitch", "roll_body", (0.0, 21.0, 8.8), "骨白粗针脚（沿拼皮边缘，不对称）"),
    ("roll_band_r", "roll_body", (3.6, 21.0, 5.6), "右侧箍带 + 锈铁扣片 + 骨栓"),
    ("roll_band_l", "roll_body", (-3.6, 21.0, 5.6), "左侧箍带（镜像）"),
    ("roll_dangle", "roll_body", (0.0, 17.4, 5.6), "囊底垂的骨坠 / 布签（会晃，不对称）"),
    *harness.HARNESS_BONES,
]
ASYM_BONES = frozenset({"roll_patch", "roll_stitch", "roll_dangle"}) | harness.HARNESS_ASYM

# ───────────────────────────────────────────────────────────── 常量
RX = 8.0                                  # 囊身半长（收口前）
Y_LO, Y_HI = 17.6, 24.0                   # 囊身 y 范围：顶面顶在头底下
Z_LO, Z_HI = 2.6, 8.6                     # 囊身 z 范围：贴背（躯干背面在 z=2）
CORE_Y = (18.4, 23.2)                     # 中段（含正面 / 背面）y 范围
SLAB_Z = (3.4, 7.8)                       # 上下削角的 z 范围
NECK_X = (RX, 8.6)                        # 收口颈 x 范围
NECK_Y, NECK_Z = (18.9, 22.7), (3.2, 8.0)  # 收口颈的截面


# ───────────────────────────────────────────────────────────── 分部件：囊身
def part_body(g: Gear) -> None:
    """皮囊身：中段方筒 + 上下各一层削角 + 两头收窄的颈，合起来读成圆筒。"""
    b = "roll_body"
    g.box(b, "core", (-RX, CORE_Y[0], Z_LO), (RX, CORE_Y[1], Z_HI), "hide")
    g.box(b, "slab_top", (-RX, CORE_Y[1], SLAB_Z[0]), (RX, Y_HI, SLAB_Z[1]), "hide")
    g.box(b, "slab_bot", (-RX, Y_LO, SLAB_Z[0]), (RX, CORE_Y[0], SLAB_Z[1]), "hide_dk")
    # 收口：囊身两头缩成颈，扎口处一道褶
    g.pair(b, "neck", (NECK_X[0], NECK_Y[0], NECK_Z[0]), (NECK_X[1], NECK_Y[1], NECK_Z[1]), "hide_stain")
    g.pair(b, "neck_fold", (RX - 0.9, 18.2, Z_LO - 0.1), (RX - 0.3, 23.4, Z_HI + 0.1), "hide_stain")
    # 顶面 / 前面各一条缝线凸起（沿 x 方向的接缝），顶面那条嵌进囊身 0.05px 避免与箍带共面
    g.box(b, "seam_top", (-6.4, Y_HI - 0.05, 5.4), (6.4, Y_HI + 0.2, 6.0), "hide_stain")
    g.box(b, "seam_front", (-6.4, 20.8, Z_LO - 0.2), (6.4, 21.4, Z_LO), "hide_stain")
    g.box(b, "seam_bot", (-6.4, Y_LO - 0.2, 4.85), (6.4, Y_LO + 0.05, 5.5), "hide_stain")


def part_caps(g: Gear) -> None:
    """两头封口盖：一块底板 + 两圈逐层缩进的卷纹 + 草绳扎一圈 + 交叉的绑绳。"""
    y0, y1 = NECK_Y
    z0, z1 = NECK_Z
    for sx, tag in ((1, "r"), (-1, "l")):
        b = f"roll_cap_{tag}"
        x0 = sx * NECK_X[1]

        def box(name: str, xa: float, xb: float, f_yz: tuple, t_yz: tuple, mat: str) -> None:
            g.rig.cube(b, f"{name}_{tag}", (min(xa, xb), f_yz[0], f_yz[1]), (max(xa, xb), t_yz[0], t_yz[1]), mat=mat)

        box("cap_plate", x0, x0 + sx * 0.3, (y0 - 0.2, z0 - 0.2), (y1 + 0.2, z1 + 0.2), "hide_dk")
        for r, (inset, mat) in enumerate(((0.5, "hide_stain"), (1.2, "hide_lt"))):
            xa = x0 + sx * (0.3 + 0.2 * r)
            xb = xa + sx * 0.25
            ya, yb, za, zb, t = y0 + inset - 0.2, y1 - inset + 0.2, z0 + inset - 0.2, z1 - inset + 0.2, 0.4
            box(f"cap_r{r}_top", xa, xb, (yb - t, za), (yb, zb), mat)
            box(f"cap_r{r}_bot", xa, xb, (ya, za), (ya + t, zb), mat)
            box(f"cap_r{r}_front", xa, xb, (ya + t, za), (yb - t, za + t), mat)
            box(f"cap_r{r}_back", xa, xb, (ya + t, zb - t), (yb - t, zb), mat)
        box("cap_core", x0 + sx * 0.3, x0 + sx * 1.0, (20.2, 5.0), (21.4, 6.2), "void")
        # 扎口草绳：绕颈一圈（四片）+ 一个结 + 绳头
        xl, xh = (RX - 0.1, RX + 0.7) if sx > 0 else (-RX - 0.7, -RX + 0.1)
        box("lash_top", xl, xh, (y1, z0 - 0.1), (y1 + 0.35, z1 + 0.1), "cord")
        box("lash_bot", xl, xh, (y0 - 0.35, z0 - 0.1), (y0, z1 + 0.1), "cord_dk")
        box("lash_front", xl, xh, (y0, z0 - 0.4), (y1, z0), "cord")
        box("lash_back", xl, xh, (y0, z1), (y1, z1 + 0.4), "cord_dk")
        box("lash_knot", xl + 0.1, xh - 0.1, (20.6, z1 + 0.4), (21.6, z1 + 0.9), "cord_dk")
        box("lash_end", xl + 0.2, xh - 0.2, (18.8, z1 + 0.4), (20.6, z1 + 0.7), "cord")
        # 盖面交叉绑绳：两根斜杆（x 在盖面外再凸出一点点）
        xc = x0 + sx * 0.9
        g.rod(b, f"cross_a_{tag}", (xc, y0 + 0.3, z0 + 0.2), (xc, y1 - 0.3, z1 - 0.2), 0.2, "cord", rz=0.2)
        g.rod(b, f"cross_b_{tag}", (xc, y1 - 0.3, z0 + 0.2), (xc, y0 + 0.3, z1 - 0.2), 0.2, "cord_dk", rz=0.2)


# ───────────────────────────────────────────────────────────── 分部件：拼皮 / 针脚
PATCHES = (   # (x0, x1, y0, y1, mat)  背面 z=Z_HI，避开 x=±(2.4..4.8) 的箍带和 |x|>7.1 的收口
    (-7.0, -4.9, 18.4, 20.6, "hide_grey"), (-7.0, -5.6, 20.6, 23.2, "hide_pale"), (-5.6, -4.9, 20.6, 23.2, "hide_lt"),
    (-2.2, 0.2, 18.4, 20.8, "hide_spot"), (-2.2, -0.6, 20.8, 23.2, "hide_grey"), (-0.6, 2.2, 20.8, 23.2, "hide_lt"),
    (0.2, 2.2, 18.4, 20.8, "hide_dk"),
    (5.0, 7.0, 18.4, 20.8, "hide_pale"), (5.0, 6.2, 20.8, 23.2, "hide_spot"), (6.2, 7.0, 20.8, 23.2, "hide_grey"),
)


def part_patch(g: Gear) -> None:
    """背面拼皮：十块大小 / 颜色不一的皮块贴在囊身背面（z=8.6），块与块之间留一道缝露出底皮。"""
    b = "roll_patch"
    for k, (x0, x1, y0, y1, mat) in enumerate(PATCHES):
        g.box(b, f"patch_{k}", (x0 + 0.1, y0 + 0.1, Z_HI), (x1 - 0.1, y1 - 0.1, Z_HI + 0.25), mat)
    # 顶面也拼了两块（z≥4.1：头在 z≤4）
    g.box(b, "top_patch_a", (-6.8, Y_HI - 0.05, 4.1), (-3.0, Y_HI + 0.2, 5.2), "hide_grey")
    g.box(b, "top_patch_b", (1.0, Y_HI - 0.05, 6.2), (6.4, Y_HI + 0.2, 7.5), "hide_spot")
    # 斑皮上几块更暗的斑点（凸起 0.25）
    for k, (x, y) in enumerate(((-1.2, 19.0), (-0.95, 19.7), (5.4, 22.0), (5.7, 22.6))):
        g.box(b, f"spot_{k}", (x, y, Z_HI + 0.25), (x + 0.8, y + 0.6, Z_HI + 0.5), "hide_spot")


def part_stitch(g: Gear) -> None:
    """骨白粗针脚：沿大皮块的上 / 下 / 左边各缝一排，每针是一小截骨条，隔着缝斜跨过去。"""
    b = "roll_stitch"
    z0 = Z_HI + 0.25
    for k, (x0, x1, y0, y1, mat) in enumerate(PATCHES):
        if (x1 - x0) < 2.0 or (y1 - y0) < 1.6:
            continue
        runs = [(f"p{k}_bot", "x", y0, x0 + 0.5, x1 - 0.5), (f"p{k}_left", "y", x0 + 0.1, y0 + 0.4, y1 - 0.4)]
        if y1 > CORE_Y[1] - 0.5:                      # 上下两排共用中间那条缝，只在上排的底边缝一次
            runs.append((f"p{k}_top", "x", y1, x0 + 0.5, x1 - 0.5))
        for name, axis, fixed, a, c in runs:
            n = max(1, int((c - a) / 0.9))
            for i in range(n):
                t = a + (c - a) * (i + 0.5) / n
                mat_s = "bone_lt" if i % 2 == 0 else "bone_dk"
                if axis == "x":
                    g.box(b, f"st_{name}_{i}", (t - 0.25, fixed - 0.3, z0), (t + 0.25, fixed + 0.3, z0 + 0.25), mat_s)
                else:
                    g.box(b, f"st_{name}_{i}", (fixed - 0.3, t - 0.25, z0), (fixed + 0.3, t + 0.25, z0 + 0.25), mat_s)


# ───────────────────────────────────────────────────────────── 分部件：箍带 / 坠饰
def part_bands(g: Gear) -> None:
    """两道布带箍住囊身（背 / 下 / 前三面 + 顶面避开头的 z≥4.05 部分），背面中间一块锈铁扣片 + 骨栓。"""
    b = "roll_band"
    x0, x1 = 2.4, 4.8
    cx = (x0 + x1) / 2
    g.pair(b, "band_back", (x0, 18.2, Z_HI), (x1, 23.4, Z_HI + 0.3), "cloth", sided=True)
    g.pair(b, "band_top", (x0, Y_HI, 4.05), (x1, Y_HI + 0.3, 8.0), "cloth_dk", sided=True)
    g.pair(b, "band_bot", (x0, Y_LO - 0.3, 3.3), (x1, Y_LO + 0.1, 8.0), "cloth_dk", sided=True)
    g.pair(b, "band_front", (x0, 18.2, Z_LO - 0.3), (x1, 23.4, Z_LO + 0.1), "cloth", sided=True)
    # 带上的缝线（两侧边各一排短线，避开扣片和骨栓）
    for j, y in enumerate((18.6, 19.8, 22.0, 22.7)):
        g.pair(b, f"band_stitch_{j}_o", (x0 + 0.1, y, Z_HI + 0.25), (x0 + 0.4, y + 0.5, Z_HI + 0.5), "cord_dk", sided=True)
        g.pair(b, f"band_stitch_{j}_i", (x1 - 0.4, y, Z_HI + 0.25), (x1 - 0.1, y + 0.5, Z_HI + 0.5), "cord_dk", sided=True)
    # 锈铁扣片：带面中间一块方铁，四角铆钉；骨栓挂在扣片下
    g.pair(b, "buckle_plate", (cx - 0.7, 20.6, Z_HI + 0.3), (cx + 0.7, 22.0, Z_HI + 0.6), "iron", sided=True)
    g.pair(b, "buckle_rust", (cx - 0.5, 21.05, Z_HI + 0.6), (cx + 0.1, 21.5, Z_HI + 0.8), "rust", sided=True)
    for j, (dx, dy) in enumerate(((-0.65, 20.65), (0.25, 20.65), (-0.65, 21.6), (0.25, 21.6))):
        g.pair(b, f"buckle_rivet_{j}", (cx + dx, dy, Z_HI + 0.6), (cx + dx + 0.4, dy + 0.35, Z_HI + 0.85), "rust_dk", sided=True)
    g.pair(b, "toggle", (cx - 0.5, 19.0, Z_HI + 0.3), (cx + 0.5, 20.4, Z_HI + 0.9), "bone_lt", sided=True)
    g.pair(b, "toggle_pin", (cx - 0.15, 19.3, Z_HI + 0.9), (cx + 0.15, 20.0, Z_HI + 1.15), "bone_stain", sided=True)


def part_dangle(g: Gear) -> None:
    """囊底垂的零碎：一件骨坠、一条布签、一撮草绳（各挂各的，位置不对称）。"""
    b = "roll_dangle"
    # 骨坠：草绳从囊底垂下，末端挂一截骨头（带关节头）
    g.box(b, "bone_cord", (-5.2, 15.8, 4.9), (-4.7, 17.6, 5.4), "cord")
    g.box(b, "bone_shaft", (-5.5, 13.8, 4.8), (-4.4, 15.8, 5.5), "bone_lt")
    g.box(b, "bone_knob_a", (-5.8, 15.4, 4.6), (-4.1, 16.0, 5.7), "bone_dk")
    g.box(b, "bone_knob_b", (-5.8, 13.4, 4.6), (-4.1, 14.0, 5.7), "bone_dk")
    # 布签：一条窄布挂着一块锈铁牌
    g.box(b, "tag_cord", (1.2, 16.2, 4.6), (1.6, 17.6, 5.0), "cord_dk")
    g.box(b, "tag_cloth", (0.8, 13.8, 4.5), (2.0, 16.2, 5.1), "cloth_dk")
    g.box(b, "tag_cloth_tooth", (0.8, 13.0, 4.5), (1.4, 13.8, 5.1), "cloth")
    g.box(b, "tag_plate", (1.0, 14.6, 5.1), (1.8, 15.6, 5.4), "iron")
    g.box(b, "tag_plate_rust", (1.15, 14.9, 5.4), (1.65, 15.4, 5.65), "rust")
    # 草绳一撮：三根长短不一，系在一个结上
    for k, (x, h) in enumerate(((5.2, 2.0), (5.8, 2.8), (6.4, 1.6))):
        g.box(b, f"fringe_{k}", (x, Y_LO - 0.3 - h, 5.0), (x + 0.4, Y_LO - 0.3, 5.4), "cord" if k % 2 == 0 else "cord_dk")
    g.box(b, "fringe_knot", (5.0, Y_LO - 0.3, 4.8), (6.8, Y_LO, 5.6), "cord_dk")


PART_FUNCS = {
    "body": part_body,
    "caps": part_caps,
    "patch": part_patch,
    "stitch": part_stitch,
    "bands": part_bands,
    "dangle": part_dangle,
    **harness.harness_parts(HARNESS),
}

SPEC = GearSpec(
    gear_id="za_pi_xing_nang",
    model_name="ZaPiXingNang",
    title="杂皮行囊 / za_pi_xing_nang",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：背 / 肩（横卷 y∈[17.6,24]，z∈[2.6,8.6]）+ 双肩带 + 胸带 + 腰带",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
