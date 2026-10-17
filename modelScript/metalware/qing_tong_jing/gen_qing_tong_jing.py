#!/usr/bin/env python3
"""青铜镜（qing_tong_jing）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：旧时铁器 / 青铜器 #4（手持 / 佩挂，独立小件）。凡物青铜镜：原本能「照灵」，末法后只当普通镜子用——
背面的灵纹（外圈锯齿 / 连弧）暗掉，镜面能照脸、照不出灵气。

造型与比例以参考图（``qing_tong_jing_ref.png``：左 = 背面正视，右 = 侧面）为准，整件对整件：
盘径 8px、厚约 1px（参考图盘厚 / 盘径 = 13%）；背面自外向内：宽素边（占半径 17%）→ 锯齿带 → 连弧带 →
四只对称的小兽 → 纽座环 → 半球纽（凸出 0.74px，纽上穿一根丝绳）。正面（-z）是磨亮的镜面。

坐标（px，1 方块 = 16px）：镜面朝 -z、背面和纽朝 +z，盘心 (0, 4, 0)，盘底落在 y=0（站在地上看比例）；
+x = 玩家右侧。手持时右臂前抬，纽握在拳里，镜面朝外（见 check.png 的「持握姿势」栏）。

圆盘用 16 边形近似：8 条宽 2a、厚 2a·tan(11.25°) 的条各转 22.5° 叠出来（a = 内切半径），
环带用 n 条沿切向的短条首尾相接，齿 / 弧 / 兽按方位角逐个摆。

用法：
    python3 modelScript/metalware/qing_tong_jing/gen_qing_tong_jing.py --table | --gates | (全量)
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "metalware" / "_kit"))
import metal_review  # noqa: E402
from gear_kit import Gear  # noqa: E402
from metal_review import MetalSpec, octagon_prism  # noqa: E402

MATS = {
    "bz":        (150, 92, 58),     # 红褐铜底
    "bz_relief": (168, 112, 74),    # 浮雕纹
    "bz_pat":    (84, 130, 110),    # 青绿锈（边缘蚕食）
    "bz_worn":   (206, 152, 98),    # 手磨亮铜（素边、纽）
    "bz_dk":     (76, 54, 42),      # 凹处暗铜
    "dead":      (54, 52, 50),      # 暗掉的灵纹：更暗更灰，不发光
    "face":      (196, 200, 196),   # 磨得发白的镜面
    "cord":      (110, 92, 70),     # 丝绳
}

# ───────────────────────────────────────────────────────────── 尺寸（px）
CX, CY = 0.0, 4.0           # 盘心（盘底落在 y=0）
R_OUT = 4.0                 # 盘半径（内切）
RIM_W = 0.65                # 外缘宽素边宽度（参考图：占半径 17%）
Z_FACE = (-0.52, -0.22)     # 镜面层（磨亮的银白）
Z_FIELD = (-0.22, 0.18)     # 背面纹区底板
Z_RIM = (-0.22, 0.52)       # 素边高出纹区的厚度
Z_REL = (0.18, 0.40)        # 浅浮雕层（0.22 凸）
SEAT_TOP = 0.42             # 纽座顶面
KNOB_TOP = SEAT_TOP + 0.74  # 纽顶（参考图纽凸出 0.74px）

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根；盘底落在 y=0"),
    ("face", "root", (CX, CY, -0.4), "镜面：磨亮的银白圆盘（16 边形近似）"),
    ("plate", "face", (CX, CY, 0.0), "盘身：纹区底板 + 外缘宽素边"),
    ("relief", "plate", (CX, CY, 0.3), "背纹：锯齿带 + 连弧带 + 四兽 + 乳钉（灵纹，暗掉不亮）"),
    ("seat", "plate", (CX, CY, 0.3), "纽座：座盘 + 座环"),
    ("knob", "seat", (CX, CY, 0.8), "半球纽（三级收顶）"),
    ("cord", "knob", (CX, CY - 1.0, 1.3), "穿纽的丝绳：双股垂下 + 绳结"),
]


# ───────────────────────────────────────────────────────────── 圆形近似
def disc16(g: Gear, bone: str, name: str, a: float, z0: float, z1: float, mat: str, cy: float = CY) -> None:
    """居中 16 边形盘（内切半径 a，厚 z0..z1）：8 条 2a × 2a·tan(11.25°) 的条各转 22.5°。

    φ=0 与 φ=90 两块不旋转的条错开 0.015px，防共面闪烁。
    """
    short = 2 * a * math.tan(math.radians(11.25))
    for k in range(8):
        phi = 22.5 * k
        if k == 4:
            g.box(bone, f"{name}_{k}", (CX - short / 2, cy - a, z0 + 0.015), (CX + short / 2, cy + a, z1 - 0.015), mat)
        elif k == 0:
            g.box(bone, f"{name}_{k}", (CX - a, cy - short / 2, z0), (CX + a, cy + short / 2, z1), mat)
        else:
            g.box(bone, f"{name}_{k}", (CX - a, cy - short / 2, z0), (CX + a, cy + short / 2, z1), mat,
                  rot=(0, 0, phi), org=(CX, cy, (z0 + z1) / 2))


def _angles(n: int):
    """n 等分方位角（度，从 +x 起逆时针）→ ('pair', α) 右半各一对、('t', 90) / ('u', 270) 正上 / 正下各一个。"""
    for k in range(n):
        alpha = 360.0 * k / n
        c = math.cos(math.radians(alpha))
        if c > 1e-6:
            yield "pair", alpha
        elif abs(c) <= 1e-6:
            yield ("t" if math.sin(math.radians(alpha)) > 0 else "u"), alpha


def ring_bars(g: Gear, bone: str, name: str, n: int, a: float, w: float, z0: float, z1: float, mat: str,
              overlap: float = 0.05) -> None:
    """n 边形环（内切半径 a，壁宽 w）：n 条沿切向的短条首尾相接（条长 = 外沿边长 + overlap）。"""
    length = 2 * a * math.tan(math.pi / n) + overlap
    rho = a - w / 2
    for kind, alpha in _angles(n):
        ca, sa = math.cos(math.radians(alpha)), math.sin(math.radians(alpha))
        cx, cy = CX + rho * ca, CY + rho * sa
        f = (cx - length / 2, cy - w / 2, z0)
        t = (cx + length / 2, cy + w / 2, z1)
        rot, org = (0, 0, alpha + 90.0), (cx, cy, (z0 + z1) / 2)
        if kind == "pair":
            g.pair(bone, f"{name}_{int(round(alpha))}", f, t, mat, rot=rot, org=org)
        else:
            g.box(bone, f"{name}_{kind}", f, t, mat, rot=rot, org=org)


def radial_marks(g: Gear, bone: str, name: str, n: int, rho: float, width: float, length: float,
                 z0: float, z1: float, mat: str) -> None:
    """n 个沿半径方向的短条（锯齿 / 乳钉）：中心在半径 rho 上，长轴朝外。"""
    for kind, alpha in _angles(n):
        ca, sa = math.cos(math.radians(alpha)), math.sin(math.radians(alpha))
        cx, cy = CX + rho * ca, CY + rho * sa
        f = (cx - width / 2, cy - length / 2, z0)
        t = (cx + width / 2, cy + length / 2, z1)
        rot, org = (0, 0, alpha - 90.0), (cx, cy, (z0 + z1) / 2)
        if kind == "pair":
            g.pair(bone, f"{name}_{int(round(alpha))}", f, t, mat, rot=rot, org=org)
        else:
            g.box(bone, f"{name}_{kind}", f, t, mat, rot=rot, org=org)


# ───────────────────────────────────────────────────────────── 零件
def part_face(g: Gear) -> None:
    """镜面：整盘一层磨亮的银白（参考图：盘面发白，边缘有绿锈蚕食）。"""
    disc16(g, "face", "face_disc", R_OUT, *Z_FACE, "face")


def part_plate(g: Gear) -> None:
    """盘身：纹区底板（r ≤ 3.35）+ 外缘宽素边一圈（r 3.35–4.0，比纹区高出 0.34）。"""
    disc16(g, "plate", "plate_field", R_OUT - RIM_W, *Z_FIELD, "bz")
    ring_bars(g, "plate", "rim", 16, R_OUT, RIM_W, Z_RIM[0], Z_RIM[1], "bz_worn")


def part_relief(g: Gear) -> None:
    """背纹（暗掉的灵纹）：外圈锯齿带（24 齿）+ 内外各一圈分隔线 + 连弧带（12 段）+ 四只对称的小兽 + 四颗乳钉。"""
    b = "relief"
    z0, z1 = Z_REL
    ring_bars(g, b, "arc_sep_o", 16, R_OUT - RIM_W - 0.02, 0.22, z0, z1, "dead")       # 锯齿带外缘线
    radial_marks(g, b, "tooth", 24, 2.8, 0.4, 0.5, z0, z1, "dead")                     # 锯齿
    ring_bars(g, b, "arc_sep_i", 16, 2.45, 0.22, z0, z1, "dead")                       # 锯齿带内缘线
    ring_bars(g, b, "arc", 12, 2.2, 0.22, z0, z1, "dead")                              # 连弧带
    # 乳钉：正右 / 正左一对，正上 / 正下各一颗
    radial_marks(g, b, "stud", 4, 1.95, 0.32, 0.32, z0, z1 + 0.1, "bz_relief")
    # 四兽：右上与右下各摆一只，左半由 pair 镜像；兽 = 躯干条 + 头块 + 尾卷（两条）
    for sy, tag in ((1, "u"), (-1, "d")):
        bx, by = 1.15, sy * 1.15
        g.pair(b, f"beast_{tag}_body", (CX + bx - 0.3, CY + by - 0.17, z0), (CX + bx + 0.65, CY + by + 0.17, z1), "dead")
        def span(d0: float, d1: float) -> tuple[float, float]:
            """相对兽躯干中线 by 的竖向区间 [d0, d1]（上兽向上、下兽整体上下翻转）。"""
            y0, y1 = by + sy * d0, by + sy * d1
            return min(y0, y1), max(y0, y1)

        hy = span(-0.02, 0.34)
        g.pair(b, f"beast_{tag}_head", (CX + bx + 0.65, CY + hy[0], z0), (CX + bx + 1.0, CY + hy[1], z1 + 0.12), "dead")
        ta, tb = span(-0.17, 0.5), span(0.5, 0.8)
        g.pair(b, f"beast_{tag}_tail_a", (CX + bx - 0.62, CY + ta[0], z0), (CX + bx - 0.3, CY + ta[1], z1), "dead")
        g.pair(b, f"beast_{tag}_tail_b", (CX + bx - 0.62, CY + tb[0], z0), (CX + bx + 0.05, CY + tb[1], z1), "dead")


def part_seat(g: Gear) -> None:
    """纽座：座盘（r 0.95，凸 0.24）+ 外沿一圈细座环。"""
    disc16(g, "seat", "seat_disc", 0.95, Z_FIELD[1], SEAT_TOP, "bz_relief")
    ring_bars(g, "seat", "seat_ring", 16, 1.3, 0.22, Z_REL[0], Z_REL[1], "bz_relief")


def part_knob(g: Gear) -> None:
    """半球纽：三级八棱收顶（沿 z 轴叠），凸出 0.74px；顶面一个暗孔穿绳。"""
    b = "knob"
    for k, (af, z0, z1) in enumerate(((1.1, SEAT_TOP, 0.66), (0.95, 0.66, 0.9), (0.65, 0.9, KNOB_TOP))):
        h, e = af / 2, af * 0.41421356 / 2
        org = (CX, CY, (z0 + z1) / 2)
        g.box(b, f"knob_{k}_a", (CX - h, CY - e, z0), (CX + h, CY + e, z1), "bz_worn")
        g.box(b, f"knob_{k}_b", (CX - e, CY - h, z0 + 0.015), (CX + e, CY + h, z1 - 0.015), "bz_worn")
        g.box(b, f"knob_{k}_c", (CX - h, CY - e, z0 + 0.01), (CX + h, CY + e, z1 - 0.01), "bz_worn", rot=(0, 0, 45), org=org)
        g.box(b, f"knob_{k}_d", (CX - h, CY - e, z0 + 0.01), (CX + h, CY + e, z1 - 0.01), "bz_worn", rot=(0, 0, -45), org=org)


def part_cord(g: Gear) -> None:
    """丝绳：从纽顶垂下的双股（三段，逐段错开读作拧股）+ 一个绳结。"""
    b = "cord"
    zc0, zc1 = KNOB_TOP, KNOB_TOP + 0.22
    for k, (y0, y1, x0, x1) in enumerate(((CY - 0.45, CY + 0.3, 0.05, 0.27), (CY - 1.1, CY - 0.45, 0.02, 0.24),
                                          (CY - 1.5, CY - 1.1, 0.06, 0.28))):
        g.pair(b, f"cord_strand{k}", (x0, y0, zc0), (x1, y1, zc1), "cord")
    g.box(b, "cord_knot", (-0.3, CY - 1.85, zc0 - 0.04), (0.3, CY - 1.45, zc1 + 0.2), "cord")


PARTS = {
    "face": part_face,
    "plate": part_plate,
    "relief": part_relief,
    "seat": part_seat,
    "knob": part_knob,
    "cord": part_cord,
}

# 参考图 qing_tong_jing_ref.png（1536×1024）：左 = 背面正视（相机在 +z），右 = 侧视（相机在 +x，镜面在右）
SPEC = MetalSpec(
    gear_id="qing_tong_jing",
    model_name="QingTongJing",
    title="青铜镜 / qing_tong_jing",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=HERE,
    min_bones=6,
    interpen_bite=0.3,
    round_label="round 1/3",
    front_yaw=0.0,
    ref_front_box=(105, 30, 975, 898),
    ref_side_box=(1108, 30, 1327, 898),
    height_parts=(
        ("外缘素边（顶）", 30, 103, ("rim_t",)),
        ("锯齿带（顶）", 103, 165, ("tooth_t",)),
        ("连弧带（顶）", 165, 212, ("arc_t",)),
    ),
    width_parts=(
        ("front", "纽座", 440, 640, ("seat_disc",)),
        ("front", "纽", 485, 595, ("knob_",)),
        ("side", "盘身厚", 1212, 1327, ("face_", "plate_", "rim_")),
        ("side", "纽凸出", 1130, 1212, ("knob_",)),
    ),
    # 手持：右臂整体前抬 90°，握点 = 纽心，放在拳心沿臂轴往前 1.25px 处（纽埋在拳前端，盘背贴着拳尖）
    hold={"arm_deg": 90.0, "anchor": (CX, CY, 0.9), "lead": 1.25, "skip_arm": ("knob_", "cord_")},
)


if __name__ == "__main__":
    metal_review.main(SPEC)
