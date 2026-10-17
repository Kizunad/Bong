#!/usr/bin/env python3
"""青铜漏壶（qing_tong_lou_hu）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：旧时铁器 / 青铜器 #12（地上摆件，放桌上 / 架上）。凡物青铜计时器：水往下漏，计量夜里过去多久。
末法体现：天文残破、节律紊乱时它仍可靠，但壶里的水是稀缺品——上壶里只有盘底一小截水，下壶里的浮箭只露出一小段。

造型与比例以参考图（``qing_tong_lou_hu_ref.png``，正视 + 侧视）为准：
方座（四足 + 裙板回纹）→ 下壶（盖 + 盖孔里一支带刻度的浮箭）→ 中壶（兽首流嘴）→ 上壶（最宽的受水壶，
细长壶嘴 + 两只龙形环耳）；三壶之间靠背后两根立框和托臂承住。整件高 22px，对应参考图 1057px，
所以 1 参考 px = 22/1057 = 0.0208 模型 px；最宽处是龙耳外跨，约 10px（和任务卡的 10×22 一致）。

坐标（px，1 方块 = 16px）：足底 y=0，壶轴在 x=z=0，正面（兽首流嘴、壶嘴）朝 -z，立框在 +z 一侧，
+x = 玩家右侧；整件超出一格方块，摆件按 geo 坐标，不受一格限制。

用法：
    python3 modelScript/metalware/qing_tong_lou_hu/gen_qing_tong_lou_hu.py --table | --gates | (全量)
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "metalware" / "_kit"))
import metal_review  # noqa: E402
from gear_kit import Gear, soft_pairs  # noqa: E402
from metal_review import OCT, MetalSpec, octagon_prism, octagon_ring  # noqa: E402

MATS = {
    "bz":        (150, 92, 58),     # 红褐铜底
    "bz_relief": (168, 112, 74),    # 浮雕带 / 腰带线
    "bz_pat":    (84, 130, 110),    # 青绿锈
    "bz_worn":   (206, 152, 98),    # 手磨亮铜（壶嘴、环耳、浮箭）
    "bz_dk":     (76, 54, 42),      # 凹处暗铜
    "water":     (84, 108, 118),    # 上壶底那一小截水
    "void":      (18, 16, 14),      # 流嘴口
}
SOFT_OVER = soft_pairs(("water",), tuple(MATS))

# 参考图 1057px 高 ↔ 模型 22px
REF_H = 1057.0
REF_BOTTOM = 1115.0
S = 22.0 / REF_H


def Y(ref_y: float) -> float:
    """参考图 y(px) → 模型 y(px)，足底 = 0。"""
    return (REF_BOTTOM - ref_y) * S


def R(ref_px: float) -> float:
    """参考图水平长度 px → 模型 px。"""
    return ref_px * S


# 各壶的剖面：(参考图 y, 参考图半宽 px)，自下而上；每相邻两点出一级八棱柱（半宽取平均）
PROFILE_LOW = ((945, 68), (930, 72), (912, 92), (890, 112), (865, 125), (835, 131), (800, 133), (770, 131),
               (745, 128), (728, 130), (714, 126), (700, 104), (688, 72), (678, 36))
PROFILE_MID = ((580, 62), (565, 66), (548, 84), (530, 100), (508, 114), (480, 122), (450, 125), (420, 123),
               (392, 118), (368, 108), (350, 90), (335, 78), (318, 77))
PROFILE_TOP_SOLID = ((315, 45), (298, 60), (282, 84), (265, 112), (248, 135), (230, 154), (212, 168), (200, 172))
PROFILE_TOP_RING = ((200, 172), (172, 174), (145, 173), (118, 172), (96, 171), (78, 174), (60, 178))

MIN_TIER_REF = 17          # 一级八棱柱至少这么高（参考图 px，约 0.36 模型 px），否则错层后过薄
STAND_TOP_Y = Y(925)       # 3.95
LID_TOP_Y = Y(678)
ROD_TOP_Y = 11.0           # 浮箭顶（让开中壶底）
SPOUT_Y = Y(175)

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根，足底落在 y=0"),
    ("stand", "root", (0.0, 2.0, 0.0), "方座：四足 + 裙板回纹 + 台面"),
    ("frame", "stand", (0.0, 9.0, 1.0), "背后两根立框 + 托臂（左右成对）"),
    ("pot_low", "stand", (0.0, 7.0, 0.0), "下壶：圈足 + 鼓腹 + 腰带 + 盖 + 兽首流嘴"),
    ("rod", "pot_low", (0.0, 9.2, 0.0), "浮箭：从盖孔里立起的带刻度铜杆"),
    ("pot_mid", "frame", (0.0, 13.5, 0.0), "中壶：圈足 + 鼓腹 + 腰带 + 兽首流嘴 + 直口"),
    ("pot_top", "pot_mid", (0.0, 19.0, 0.0), "上壶：最宽的受水壶（口外撇），腰带，盘底一小截水"),
    ("handle", "pot_top", (0.0, 19.5, 0.0), "龙形环耳（左右成对）+ 垂环"),
    ("spout_top", "pot_top", (0.0, 19.6, -4.5), "上壶细长壶嘴（朝 -z，尖端下折）"),
]


# ───────────────────────────────────────────────────────────── 辅助
def af_at(profile, y_lo: float, y_hi: float) -> float:
    """剖面里 y 区间（模型 px）内最大的对边宽（模型 px）。"""
    best = 0.0
    for (ya, ha), (yb, hb) in zip(profile, profile[1:]):
        lo, hi = Y(ya), Y(yb)
        if hi > y_lo and lo < y_hi:
            best = max(best, 2 * R(max(ha, hb)))
    return best


def tiers(g: Gear, bone: str, name: str, profile, mat: str, ring_t: float | None = None) -> None:
    """按剖面逐级叠八棱柱（ring_t 给出则叠八边形环，用于敞口的壶）。"""
    # 相邻剖面点离得太近（< MIN_TIER_REF px）会叠出过薄的一级，先并掉
    pts = [profile[0]]
    for pt in profile[1:-1]:
        if pts[-1][0] - pt[0] >= MIN_TIER_REF:
            pts.append(pt)
    pts.append(profile[-1])
    for k, ((ya, ha), (yb, hb)) in enumerate(zip(pts, pts[1:])):
        af = 2 * R((ha + hb) / 2)
        y0, y1 = Y(ya), Y(yb)
        if ring_t is None:
            octagon_prism(g, bone, f"{name}{k}", af, y0, y1, mat)
        else:
            octagon_ring(g, bone, f"{name}{k}", af, ring_t, y0, y1, mat)


def band(g: Gear, bone: str, name: str, profile, ref_y0: float, ref_y1: float) -> None:
    """壶腹的腰带：上下各一圈细线（八边形环）+ 带中一圈回纹方块（每个面一块，共 8 块）。"""
    y0, y1 = Y(ref_y1), Y(ref_y0)           # ref_y1 > ref_y0，模型 y 反过来
    af = af_at(profile, y0, y1)
    line = 0.24
    out = af + 0.44 - 0.12                 # 内径略小于壶腹，咬进壶面 0.06，不留缝
    octagon_ring(g, bone, f"{name}_lo", out, 0.22, y0, y0 + line, "bz_relief")
    octagon_ring(g, bone, f"{name}_hi", out, 0.22, y1 - line, y1, "bz_relief")
    # 回纹方块：沿 8 个面各一块（前后居中、左右各一对、四条斜面各一对），0.22 凸
    ym0, ym1 = y0 + line + 0.1, y1 - line - 0.1
    half = af / 2 - 0.06
    w = 0.6
    g.box(bone, f"{name}_m_f", (-w / 2, ym0, -half - 0.22), (w / 2, ym1, -half), "bz_relief")
    g.box(bone, f"{name}_m_b", (-w / 2, ym0, half), (w / 2, ym1, half + 0.22), "bz_relief")
    g.pair(bone, f"{name}_m_s", (half, ym0, -w / 2), (half + 0.22, ym1, w / 2), "bz_relief")
    c = (half + 0.11) * 0.70710678
    ymid = (ym0 + ym1) / 2
    g.pair(bone, f"{name}_m_df", (c - w / 2, ym0, -c - 0.11), (c + w / 2, ym1, -c + 0.11), "bz_relief",
           rot=(0, -45, 0), org=(c, ymid, -c))
    g.pair(bone, f"{name}_m_db", (c - w / 2, ym0, c - 0.11), (c + w / 2, ym1, c + 0.11), "bz_relief",
           rot=(0, 45, 0), org=(c, ymid, c))


def beast_spout(g: Gear, bone: str, name: str, yc: float, surface_z: float) -> None:
    """兽首流嘴：贴在壶腹正面（z=-surface_z）的兽面板 + 眉 + 双眼 + 双角 + 前伸的流嘴管（口朝前下）。"""
    zs = surface_z
    g.box(bone, f"{name}_plate", (-0.75, yc - 0.7, -zs - 0.35), (0.75, yc + 0.7, -zs + 0.1), "bz_relief")
    g.box(bone, f"{name}_brow", (-0.8, yc + 0.42, -zs - 0.58), (0.8, yc + 0.7, -zs - 0.35), "bz_relief")
    g.pair(bone, f"{name}_eye", (0.28, yc + 0.05, -zs - 0.6), (0.62, yc + 0.4, -zs - 0.35), "bz_worn")
    g.pair(bone, f"{name}_horn", (0.45, yc + 0.7, -zs - 0.3), (0.78, yc + 1.05, -zs + 0.05), "bz_worn")
    g.pair(bone, f"{name}_ear", (0.75, yc - 0.1, -zs - 0.3), (1.0, yc + 0.35, -zs + 0.05), "bz_worn")
    g.box(bone, f"{name}_snout", (-0.3, yc - 0.55, -zs - 1.35), (0.3, yc + 0.05, -zs - 0.35), "bz_worn")
    g.box(bone, f"{name}_mouth", (-0.18, yc - 0.45, -zs - 1.58), (0.18, yc - 0.1, -zs - 1.35), "void")
    g.pair(bone, f"{name}_fang", (0.3, yc - 0.5, -zs - 0.9), (0.52, yc - 0.15, -zs - 0.4), "bz_relief")


# ───────────────────────────────────────────────────────────── 零件
def part_stand(g: Gear) -> None:
    """方座：台面（带唇）+ 四面裙板（左右两端各一块回纹方块）+ 四足（方足带宽脚）。"""
    b = "stand"
    hx, hz = 3.3, 2.78
    top0, top1 = Y(950), STAND_TOP_Y
    g.box(b, "stand_top", (-hx, top0, -hz), (hx, top1, hz), "bz")
    g.box(b, "stand_top_lip", (-hx + 0.25, top1, -hz + 0.25), (hx - 0.25, top1 + 0.22, hz - 0.25), "bz_relief")
    # 裙板：台面下方一圈（y 2.39–3.43），前后通长，左右夹在前后之间
    ap0 = Y(1000)
    g.box(b, "stand_apron_f", (-(hx - 0.4), ap0, -(hz - 0.45)), (hx - 0.4, top0, -(hz - 0.05)), "bz")
    g.box(b, "stand_apron_b", (-(hx - 0.4), ap0, hz - 0.05), (hx - 0.4, top0, hz - 0.45), "bz")
    g.pair(b, "stand_apron_s", (hx - 0.45, ap0, -(hz - 0.5)), (hx - 0.05, top0, hz - 0.5), "bz")
    # 裙板回纹：每块 = 亮铜方框 + 暗铜内芯，前后各左右两块，侧面各一块
    for fz, z0, z1 in (("f", -(hz - 0.05) - 0.22, -(hz - 0.05)), ("b", hz - 0.05, hz - 0.05 + 0.22)):
        g.pair(b, f"stand_pat_{fz}", (1.7, ap0 + 0.15, z0), (2.45, top0 - 0.15, z1), "bz_relief")
        g.pair(b, f"stand_patc_{fz}", (1.95, ap0 + 0.35, z0 - (0.2 if fz == "f" else -0.05)),
               (2.2, top0 - 0.35, z1 + (0.2 if fz == "b" else -0.05)), "bz_dk")
    g.pair(b, "stand_pat_s", (hx - 0.05, ap0 + 0.15, -0.38), (hx - 0.05 + 0.22, top0 - 0.15, 0.38), "bz_relief")
    # 四足：方足 + 宽脚块
    leg_h = ap0
    for tag, (z0, z1) in (("f", (-hz + 0.1, -hz + 1.05)), ("b", (hz - 1.05, hz - 0.1))):
        g.pair(b, f"stand_leg_{tag}", (hx - 0.95, 0.3, z0), (hx - 0.05, leg_h, z1), "bz")
        g.pair(b, f"stand_foot_{tag}", (hx - 1.2, 0.0, z0 - 0.15), (hx + 0.1, 0.3, z1 + 0.15), "bz_relief")


def part_frame(g: Gear) -> None:
    """背后两根立框（+z 一侧，x=±2.9）：扁立柱 + 三道托臂（连下壶盖沿、中壶肩、上壶底）。"""
    b = "frame"
    x0, x1 = 2.62, 3.18
    z0, z1 = 0.2, 1.25
    top = Y(300)
    g.pair(b, "frame_post", (x0, STAND_TOP_Y - 0.05, z0), (x1, top, z1), "bz")
    g.pair(b, "frame_cap", (x0 - 0.1, top, z0 - 0.1), (x1 + 0.1, top + 0.3, z1 + 0.1), "bz_relief")
    # 托臂：post → 壶身（下壶盖沿 / 中壶肩）
    g.pair(b, "frame_arm_low", (2.25, Y(748), z0 + 0.15), (x0, Y(715), z1 - 0.15), "bz_worn")
    g.pair(b, "frame_arm_mid", (2.1, Y(405), z0 + 0.15), (x0, Y(372), z1 - 0.15), "bz_worn")
    g.pair(b, "frame_arm_top", (1.7, Y(318), z0 + 0.15), (x0, Y(300) - 0.02, z1 - 0.15), "bz_worn")


def part_pot_low(g: Gear) -> None:
    """下壶：圈足 + 鼓腹 + 腰带 + 盖（拱形，盖孔里立浮箭）+ 盖孔护圈 + 兽首流嘴。"""
    b = "pot_low"
    tiers(g, b, "pot_low_t", PROFILE_LOW, "bz")
    band(g, b, "pot_low_band", PROFILE_LOW, 760, 805)
    octagon_ring(g, b, "pot_low_lidrim", af_at(PROFILE_LOW, Y(730), Y(714)) + 0.3, 0.3, Y(730), Y(714) + 0.05, "bz_worn")
    octagon_ring(g, b, "pot_low_hole", 1.6, 0.3, LID_TOP_Y, LID_TOP_Y + 0.3, "bz_worn")      # 盖孔护圈
    beast_spout(g, b, "pot_low_beast", Y(812), af_at(PROFILE_LOW, Y(830), Y(795)) / 2 - 0.05)


def part_rod(g: Gear) -> None:
    """浮箭：方杆 + 一排等距刻线（前面，一长一短交错）。只露出一小段（水少）。"""
    b = "rod"
    g.box(b, "rod_shaft", (-0.21, LID_TOP_Y - 0.1, -0.21), (0.21, ROD_TOP_Y, 0.21), "bz_worn")
    y = LID_TOP_Y + 0.12
    k = 0
    while y + 0.22 <= ROD_TOP_Y - 0.05:
        w = 0.21 if k % 2 == 0 else 0.13
        g.box(b, f"rod_tick{k}", (-w, y, -0.21 - 0.22), (w, y + 0.22, -0.21), "bz_dk")
        y += 0.26
        k += 1
    g.box(b, "rod_cap", (-0.3, ROD_TOP_Y, -0.3), (0.3, ROD_TOP_Y + 0.25, 0.3), "bz_relief")


def part_pot_mid(g: Gear) -> None:
    """中壶：圈足 + 鼓腹 + 腰带 + 直口颈环 + 兽首流嘴。"""
    b = "pot_mid"
    tiers(g, b, "pot_mid_t", PROFILE_MID, "bz")
    band(g, b, "pot_mid_band", PROFILE_MID, 405, 448)
    octagon_ring(g, b, "pot_mid_neck", af_at(PROFILE_MID, Y(336), Y(318)) + 0.3, 0.3, Y(336), Y(318) + 0.06, "bz_worn")
    beast_spout(g, b, "pot_mid_beast", Y(462), af_at(PROFILE_MID, Y(480), Y(445)) / 2 - 0.05)


def part_pot_top(g: Gear) -> None:
    """上壶：下半实心收底、上半敞口薄壁（口外撇），腰带，盘底一小截水。"""
    b = "pot_top"
    tiers(g, b, "pot_top_s", PROFILE_TOP_SOLID, "bz")
    tiers(g, b, "pot_top_r", PROFILE_TOP_RING, "bz", ring_t=0.5)
    band(g, b, "pot_top_band", PROFILE_TOP_RING, 100, 160)
    octagon_prism(g, b, "pot_top_water", 5.8, Y(200), Y(200) + 0.3, "water")      # 盘底一小截水
    octagon_ring(g, b, "pot_top_lip", af_at(PROFILE_TOP_RING, Y(80), Y(60)) + 0.1, 0.3, Y(74), Y(60) + 0.22, "bz_worn")


def part_handle(g: Gear) -> None:
    """龙形环耳（左右成对）：壶腹上的托块 → 龙颈（两段，向外上方）→ 龙首 + 角；托块下垂一只圆环（环面朝侧，yz 平面）。"""
    b = "handle"
    wall = af_at(PROFILE_TOP_RING, Y(210), Y(150)) / 2
    g.pair(b, "hdl_mount", (wall - 0.15, Y(188), -0.45), (wall + 1.0, Y(150), 0.45), "bz_worn")
    g.pair(b, "hdl_neck1", (wall + 0.35, Y(150), -0.38), (wall + 0.85, Y(108), 0.38), "bz_worn")
    g.pair(b, "hdl_neck2", (wall + 0.7, Y(108), -0.35), (wall + 1.2, Y(84), 0.35), "bz_worn")
    g.pair(b, "hdl_head", (wall + 0.72, Y(112), -0.5), (wall + 1.25, Y(70), 0.5), "bz_worn")
    g.pair(b, "hdl_snout", (wall + 1.25, Y(100), -0.35), (wall + 1.5, Y(76), 0.35), "bz_relief")
    g.pair(b, "hdl_horn", (wall + 0.85, Y(70), -0.3), (wall + 1.1, Y(60), 0.3), "bz_relief")
    g.pair(b, "hdl_eye", (wall + 0.7, Y(100), -0.72), (wall + 1.0, Y(88), -0.5), "bz_dk")
    # 垂环：环心在托块下方，环顶咬进托块 0.15
    cy = Y(188) - 0.78 + 0.15
    # 环 ring() 的 axis="x" 是 yz 平面的圆；左右各一只
    g.ring(b, "hdl_ring_r", (wall + 0.5, cy, 0.0), 0.78, 0.3, 0.3, "bz_worn", axis="x", n=8)
    g.ring(b, "hdl_ring_l", (-(wall + 0.5), cy, 0.0), 0.78, 0.3, 0.3, "bz_worn", axis="x", n=8)


def part_spout_top(g: Gear) -> None:
    """上壶细长壶嘴：壶腹上的兽面座 + 一根朝 -z 的细管 + 尖端向下折的口。"""
    b = "spout_top"
    front = af_at(PROFILE_TOP_RING, Y(190), Y(160)) / 2
    g.box(b, "spout_boss", (-0.55, SPOUT_Y - 0.55, -front - 0.35), (0.55, SPOUT_Y + 0.55, -front + 0.15), "bz_relief")
    g.box(b, "spout_tube", (-0.25, SPOUT_Y - 0.25, -front - 5.05), (0.25, SPOUT_Y + 0.25, -front - 0.35), "bz_worn")
    g.box(b, "spout_tip", (-0.25, SPOUT_Y - 0.95, -front - 5.05), (0.25, SPOUT_Y - 0.25, -front - 4.55), "bz_worn")
    g.box(b, "spout_mouth", (-0.17, SPOUT_Y - 1.17, -front - 4.97), (0.17, SPOUT_Y - 0.95, -front - 4.63), "void")
    for k, z in enumerate((-front - 1.2, -front - 2.5, -front - 3.8)):                # 管上三道箍
        g.box(b, f"spout_band{k}", (-0.34, SPOUT_Y - 0.34, z - 0.12), (0.34, SPOUT_Y + 0.34, z + 0.12), "bz_relief")


PARTS = {
    "stand": part_stand,
    "frame": part_frame,
    "pot_low": part_pot_low,
    "rod": part_rod,
    "pot_mid": part_pot_mid,
    "pot_top": part_pot_top,
    "handle": part_handle,
    "spout_top": part_spout_top,
}

# 参考图 qing_tong_lou_hu_ref.png（1338×1176）里的整件外框与分段（像素）；侧视相机在 +x，正面朝右
SPEC = MetalSpec(
    gear_id="qing_tong_lou_hu",
    model_name="QingTongLouHu",
    title="青铜漏壶 / qing_tong_lou_hu",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=HERE,
    asym=frozenset(),
    soft_over=SOFT_OVER,
    min_bones=8,
    round_label="round 1/3",
    ref_front_box=(92, 58, 582, 1115),
    ref_side_box=(752, 58, 1300, 1115),
    height_parts=(
        ("上壶", 58, 305, ("pot_top",)),
        ("中壶", 315, 580, ("pot_mid",)),
        ("下壶含盖", 678, 945, ("pot_low",)),
        ("方座", 925, 1115, ("stand_",)),
        ("立框", 300, 925, ("frame_",)),
    ),
    width_parts=(
        ("front", "龙耳外跨", 92, 582, ("hdl_",)),
        ("front", "上壶口", 160, 510, ("pot_top",)),
        ("front", "中壶", 207, 462, ("pot_mid",)),
        ("front", "下壶", 205, 470, ("pot_low",)),
        ("front", "方座", 180, 495, ("stand_",)),
        ("side", "上壶", 752, 1058, ("pot_top",)),
        ("side", "含壶嘴整深", 752, 1300, ("pot_top", "spout_")),
        ("side", "方座", 765, 1030, ("stand_",)),
    ),
)

if __name__ == "__main__":
    metal_review.main(SPEC)
