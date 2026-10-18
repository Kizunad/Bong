#!/usr/bin/env python3
"""青铜雁足灯（qing_tong_yan_zu_deng）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何（熄灭版）。

任务卡：旧时铁器 / 青铜器 #3（地上摆件，柱上有提环也可提着走）。凡物青铜灯：缺燃料是这件的主题——
盘里只剩薄薄一层灯油，灯芯烧得很短，盘沿结焦黑渍；罩片上的聚光阵纹暗掉，不再亮（更暗更灰，不发光）。

造型与比例以参考图（``qing_tong_yan_zu_deng_ref.png``，正视 + 侧视）为准：
鹅蹼三趾的足 → 一根细腿 → 颈箍（回纹带）+ 提环 → 立鹅的身 / 颈 / 头（喙朝 -z）→ 头顶托着浅圆灯盘 → 盘沿一块半圆挡风罩。
整件高 16px（任务卡写「约 14」，参考图里盘径占总高 41%，14px 时腿细到 0.9px，取 16px 更能读出腿与颈）；
灯盘外径 6.5px。参考图各部件占总高：罩 16%、盘 8%、颈头 24%、身 14%、箍 3%、腿 22%、足 14%。

坐标（px，1 方块 = 16px）：足底 y=0，鹅脸朝 -z（参考图正视的鹅脸朝镜头），+x = 玩家右侧；
挡风罩在 +x / +z 一侧，提环在 -x 一侧（这两处刻意不对称，各占一根 asym 骨）。

用法：
    python3 modelScript/metalware/qing_tong_yan_zu_deng/gen_qing_tong_yan_zu_deng.py --table
    python3 modelScript/metalware/qing_tong_yan_zu_deng/gen_qing_tong_yan_zu_deng.py --gates
    python3 modelScript/metalware/qing_tong_yan_zu_deng/gen_qing_tong_yan_zu_deng.py
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "metalware" / "_kit"))
import metal_review  # noqa: E402
from gear_kit import Gear, soft_pairs  # noqa: E402
from metal_review import MetalSpec, octagon_prism, octagon_ring  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质
MATS = {
    "bz":        (150, 92, 58),     # 红褐铜底
    "bz_relief": (168, 112, 74),    # 羽线 / 回纹等浮雕（比底色亮一档）
    "bz_pat":    (84, 130, 110),    # 青绿锈
    "bz_worn":   (206, 152, 98),    # 手磨亮铜（颈、提环、喙、足尖）
    "bz_dk":     (76, 54, 42),      # 凹处暗铜
    "dead":      (54, 52, 50),      # 暗掉的聚光阵纹：更暗更灰，不发光
    "soot":      (34, 30, 28),      # 结焦黑渍 / 烧短的灯芯
    "oil":       (92, 66, 34),      # 盘底薄薄一层灯油（暗琥珀）
    "flame":     (240, 150, 50),    # 点亮版的火舌（高对比，不做真发光；熄灭版不用）
    "flame_core": (255, 226, 130),  # 点亮版的焰心
    "void":      (18, 16, 14),
}
SOFT_OVER = soft_pairs(("soot", "oil"), tuple(MATS))

# 生成器材质名 → 画笔库（handheld_texture.py 的 metal_bz_* 一组，六件青铜器共用；火舌用库里现成的 gear_glow_*）
BZ_TEXTURES = {
    "bz": "metal_bz", "bz_relief": "metal_bz_relief", "bz_pat": "metal_bz_pat", "bz_worn": "metal_bz_worn",
    "bz_dk": "metal_bz_dk", "dead": "metal_bz_dead", "soot": "metal_bz_soot", "oil": "metal_bz_oil",
    "flame": "gear_glow_flame", "flame_core": "gear_glow_core",
}

# ───────────────────────────────────────────────────────────── 尺寸（px）
# 纵向分段：参考图 y(px) → 模型 y = (1140 - y) * 16 / 1112
ANKLE_Y = 2.16           # 腿底 / 趾根
LEG_TOP = 5.61           # 腿顶 = 颈箍底
COLLAR_TOP = 6.1
BODY_BOT, BODY_TOP = 6.19, 8.35
NECK_TOP = 12.16         # 头顶 = 灯盘底
DISH_BOT = 12.16
FLOOR_Y = 13.1           # 盘内底
RIM_TOP = 13.45
OIL_TOP = 13.36          # 灯油面（只有薄薄 0.26px）
SHIELD_TOP = 16.0
DISH_AF = 6.52           # 灯盘外对边宽
TOE_LEN = 2.77           # 趾尖到腿心的水平距离（三趾 120° 张开，前趾朝 -z）

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根，足底落在 y=0"),
    ("toe", "root", (0.0, ANKLE_Y, 0.0), "鹅蹼三趾（前趾朝 -z，后两趾左右成对）"),
    ("leg", "root", (0.0, 3.9, 0.0), "细腿 + 颈箍（回纹带）"),
    ("body", "leg", (0.0, 7.2, 0.0), "立鹅的身：卵形躯干 + 两翼（羽线）+ 尾"),
    ("neck", "body", (0.0, 9.5, -0.4), "颈（略前倾的 S 形）+ 头 + 喙（朝 -z）"),
    ("dish", "neck", (0.0, 12.8, 0.0), "灯盘：圆（八边形近似）浅盘、外撇盘沿、灯油、烧短的灯芯"),
    ("shield", "dish", (1.8, 14.4, 1.9), "半圆挡风罩（+x/+z 一侧，面上暗掉的聚光阵纹）"),
    ("ring_trinket", "leg", (-1.15, 4.85, 0.0), "提环 + 挂钩（-x 一侧）"),
    ("soot", "dish", (0.0, 13.3, 0.0), "盘沿结焦黑渍（不对称堆积）"),
]
ASYM_BONES = frozenset({"shield", "ring_trinket", "soot"})


def toe_dirs():
    """三趾水平方向（单位向量 x, z）：前趾 -z，后两趾朝 ±x / +z。"""
    return {"f": (0.0, -1.0), "r": (0.8660254, 0.5)}


# ───────────────────────────────────────────────────────────── 零件
def part_toes(g: Gear) -> None:
    """鹅蹼三趾：每趾 = 粗筋（趾根 → 趾节）+ 趾节 + 扁平蹼（贴地）+ 趾尖亮铜爪；趾间再铺一片蹼膜。"""
    b = "toe"
    root = (0.0, 2.5, 0.0)
    for tag, (dx, dz) in toe_dirs().items():
        knuckle = (dx * TOE_LEN * 0.35, 1.05, dz * TOE_LEN * 0.35)
        mid = (dx * TOE_LEN * 0.62, 0.5, dz * TOE_LEN * 0.62)
        tip = (dx * TOE_LEN, 0.14, dz * TOE_LEN)
        sides = (("", 1.0),) if tag == "f" else (("_r", 1.0), ("_l", -1.0))
        for suffix, sx in sides:
            def pt(p):
                return (p[0] * sx, p[1], p[2])

            g.rod(b, f"toe_{tag}_bar{suffix}", pt(root), pt(knuckle), 0.3, "bz", rz=0.26)
            g.rod(b, f"toe_{tag}_joint{suffix}", pt(knuckle), pt(mid), 0.34, "bz_relief", rz=0.2)
            g.rod(b, f"toe_{tag}_web{suffix}", pt(mid), pt(tip), 0.72, "bz", rz=0.13)
            nail_a = ((tip[0] - 0.3 * dx) * sx, 0.3, tip[2] - 0.3 * dz)
            nail_b = ((tip[0] + 0.15 * dx) * sx, 0.12, tip[2] + 0.15 * dz)
            g.rod(b, f"toe_{tag}_nail{suffix}", nail_a, nail_b, 0.18, "bz_worn", rz=0.12)
    # 趾间蹼膜：前趾与后趾之间贴地的一片扁板（左右成对）
    g.pair(b, "toe_membrane_a", (0.5, 0.0, -1.45), (1.35, 0.24, -0.4), "bz_dk")
    g.pair(b, "toe_membrane_b", (0.75, 0.0, -0.4), (1.7, 0.24, 0.3), "bz_dk")
    g.box(b, "toe_membrane_back", (-0.9, 0.0, 0.55), (0.9, 0.24, 1.2), "bz_dk")


def part_leg(g: Gear) -> None:
    """细腿（八棱柱）+ 踝节 + 腿上两圈弦纹 + 颈箍（八棱柱 + 四面回纹浮雕）。"""
    b = "leg"
    octagon_prism(g, b, "leg_ankle", 1.3, ANKLE_Y, 2.6, "bz_relief")
    octagon_prism(g, b, "leg_shaft", 1.0, 2.6, LEG_TOP, "bz")
    octagon_prism(g, b, "leg_cord_a", 1.18, 3.3, 3.55, "bz_relief")
    octagon_prism(g, b, "leg_cord_b", 1.18, 4.4, 4.65, "bz_relief")
    octagon_prism(g, b, "collar_band", 1.4, LEG_TOP, COLLAR_TOP, "bz_worn")
    # 颈箍回纹：每面三格（中间一格暗铜、两侧亮铜），0.22 凸；前后面写右半，左半由 pair 镜像
    g.box(b, "collar_rel_fm", (-0.15, 5.74, -0.7 - 0.22), (0.15, 5.97, -0.7), "bz_dk")
    g.box(b, "collar_rel_bm", (-0.15, 5.74, 0.7), (0.15, 5.97, 0.7 + 0.22), "bz_dk")
    g.pair(b, "collar_rel_f", (0.2, 5.74, -0.7 - 0.22), (0.55, 5.97, -0.7), "bz_relief")
    g.pair(b, "collar_rel_b", (0.2, 5.74, 0.7), (0.55, 5.97, 0.7 + 0.22), "bz_relief")
    g.pair(b, "collar_rel_sm", (0.7, 5.74, -0.15), (0.7 + 0.22, 5.97, 0.15), "bz_dk")
    g.pair(b, "collar_rel_sa", (0.7, 5.74, 0.2), (0.7 + 0.22, 5.97, 0.55), "bz_relief")
    g.pair(b, "collar_rel_sb", (0.7, 5.74, -0.55), (0.7 + 0.22, 5.97, -0.2), "bz_relief")


def part_body(g: Gear) -> None:
    """立鹅的身：四层卵形躯干 + 两翼（翼板 + 五道错落羽线）+ 尾（两段斜条）+ 胸前三排羽鳞。"""
    b = "body"
    # 卵形躯干：(y0, y1, 半宽 x, z0, z1)
    layers = ((BODY_BOT, 6.85, 0.55, -0.95, 0.95), (6.85, 7.6, 0.78, -1.25, 1.2),
              (7.6, 8.0, 0.66, -1.1, 0.9), (8.0, BODY_TOP, 0.45, -0.8, 0.5))
    for k, (y0, y1, hx, z0, z1) in enumerate(layers):
        g.box(b, f"body_egg{k}", (-hx, y0, z0), (hx, y1, z1), "bz")
    # 两翼：贴在躯干两侧的翼板 + 五道横向羽线（顺鹅身长向，逐道收短，0.22 凸）
    g.pair(b, "body_wing", (0.78, 6.5, -0.15), (1.0, 7.7, 1.3), "bz")
    for k, (y, z1) in enumerate(((6.55, 0.95), (6.8, 1.15), (7.05, 1.2), (7.3, 1.05), (7.52, 0.8))):
        g.pair(b, f"body_wing_line{k}", (1.0, y, 0.0), (1.0 + 0.22, y + 0.2, z1), "bz_relief")
    # 翼尖覆羽：翼板后缘再叠三片短羽
    for k, (y, z0, z1) in enumerate(((6.62, 1.3, 1.62), (6.95, 1.3, 1.72), (7.28, 1.3, 1.6))):
        g.pair(b, f"body_wing_tip{k}", (0.78, y, z0), (1.0, y + 0.24, z1), "bz_relief")
    # 尾：从躯干后下方斜出两段，尖端翘起；尾上两道羽线
    g.rod(b, "body_tail_a", (0.0, 6.75, 1.05), (0.0, 6.45, 1.65), 0.42, "bz", rz=0.22)
    g.rod(b, "body_tail_b", (0.0, 6.45, 1.65), (0.0, 6.55, 2.06), 0.28, "bz_worn", rz=0.2)
    # 胸前羽鳞：三排错落的小凸条（前面 z=-1.25 / -0.95 两个层面）
    g.pair(b, "body_scale_a0", (0.2, 6.95, -1.25 - 0.22), (0.62, 7.2, -1.25), "bz_relief")
    g.pair(b, "body_scale_a1", (0.56, 7.25, -1.25 - 0.22), (0.78, 7.5, -1.25), "bz_relief")
    g.box(b, "body_scale_b0", (-0.2, 7.25, -1.25 - 0.22), (0.2, 7.5, -1.25), "bz_relief")
    g.pair(b, "body_scale_c0", (0.2, 6.5, -0.95 - 0.22), (0.55, 6.75, -0.95), "bz_relief")
    g.box(b, "body_scale_c1", (-0.2, 6.55, -0.95 - 0.22), (0.2, 6.8, -0.95), "bz_relief")
    g.pair(b, "body_scale_d0", (0.2, 7.65, -1.1 - 0.22), (0.62, 7.9, -1.1), "bz_relief")
    g.box(b, "body_scale_d1", (-0.2, 7.7, -1.1 - 0.22), (0.2, 7.95, -1.1), "bz_relief")
    # 腹下近腿处的一圈收口环（躯干接颈箍处）
    octagon_ring(g, b, "body_skirt", 1.7, 0.3, BODY_BOT - 0.09, BODY_BOT + 0.2, "bz_worn")


def part_neck(g: Gear) -> None:
    """颈（三段，逐段前倾成 S 形）+ 头 + 眼 + 喙（上喙 + 尖）+ 颈上羽鳞与羽线。"""
    b = "neck"
    hw = 0.47
    g.box(b, "neck_n1", (-hw, BODY_TOP, -0.7), (hw, 9.35, 0.3), "bz_worn")
    g.box(b, "neck_n2", (-hw, 9.35, -0.95), (hw, 10.3, 0.05), "bz_worn")
    g.box(b, "neck_n3", (-hw, 10.3, -1.2), (hw, 11.15, -0.2), "bz_worn")
    # 头：两侧眼（亮铜眼眶 + 暗铜瞳）
    g.box(b, "neck_head", (-0.6, 11.15, -1.9), (0.6, NECK_TOP, -0.65), "bz_worn")
    g.pair(b, "neck_eye", (0.6, 11.55, -1.75), (0.6 + 0.22, 11.95, -1.4), "bz_relief")
    g.pair(b, "neck_pupil", (0.6 + 0.22, 11.65, -1.66), (0.6 + 0.44, 11.87, -1.44), "bz_dk")
    # 喙：粗段 + 尖，向前下方；喙根一道亮铜箍
    g.box(b, "neck_beak_a", (-0.28, 11.35, -2.65), (0.28, 11.75, -1.9), "bz_worn")
    g.box(b, "neck_beak_b", (-0.2, 11.15, -3.3), (0.2, 11.5, -2.65), "bz_worn")
    g.box(b, "neck_beak_band", (-0.34, 11.3, -1.92 - 0.22), (0.34, 11.8, -1.92), "bz_relief")
    # 颈上羽鳞：每段正面一道小凸条，侧面两道
    for k, (y, z) in enumerate(((8.7, -0.7), (9.6, -0.95), (10.5, -1.2))):
        g.box(b, f"neck_scale{k}", (-0.22, y, z - 0.22), (0.22, y + 0.25, z), "bz_relief")
        g.pair(b, f"neck_scale_s{k}", (hw, y + 0.1, z + 0.2), (hw + 0.22, y + 0.38, z + 0.75), "bz_relief")
        g.pair(b, f"neck_scale_t{k}", (hw, y + 0.5, z + 0.1), (hw + 0.22, y + 0.72, z + 0.5), "bz_relief")


def part_dish(g: Gear) -> None:
    """灯盘：圆形（八边形近似）浅盘，三级收腹 + 外撇盘沿 + 盘沿亮铜棱；盘里一层薄油、一根烧短的灯芯，外壁两道弦纹。"""
    b = "dish"
    octagon_prism(g, b, "dish_foot", 2.2, DISH_BOT, 12.42, "bz")
    octagon_prism(g, b, "dish_bowl1", 3.8, 12.42, 12.7, "bz")
    octagon_prism(g, b, "dish_bowl2", 5.2, 12.7, FLOOR_Y, "bz")
    octagon_ring(g, b, "dish_rim", DISH_AF, 0.76, FLOOR_Y, RIM_TOP, "bz")
    octagon_ring(g, b, "dish_lip", DISH_AF, 0.3, RIM_TOP, RIM_TOP + 0.22, "bz_worn")   # 盘沿磨亮的一圈棱
    octagon_ring(g, b, "dish_cord", 5.6, 0.4, 12.6, 12.85, "bz_relief")               # 盘腹弦纹
    octagon_ring(g, b, "dish_cord2", 4.0, 0.3, 12.45, 12.68, "bz_relief")               # 盘底弦纹
    octagon_prism(g, b, "dish_oil", 4.0, FLOOR_Y, OIL_TOP, "oil")                       # 盘底薄油
    g.box(b, "dish_wick", (-0.22, OIL_TOP, -0.22), (0.22, OIL_TOP + 0.5, 0.22), "soot")  # 烧得很短的灯芯


def part_shield(g: Gear) -> None:
    """半圆挡风罩（四分之一圆盘，直边在内、弧边在外）：逐行收窄的薄板 + 弧边亮铜棱 + 面上暗掉的聚光阵纹。"""
    b = "shield"
    z0, z1 = 1.75, 2.1
    # 弧边逐行外缘 x（圆弧：半径 ≈ 1.5，圆心在 (1.0, FLOOR_Y)，上行收窄）
    ys = [FLOOR_Y + k * (SHIELD_TOP - FLOOR_Y) / 9 for k in range(10)]
    xs = (2.5, 2.5, 2.46, 2.4, 2.3, 2.15, 1.95, 1.7, 1.42)
    for k in range(9):
        g.box(b, f"shield_row{k}", (1.0, ys[k], z0), (xs[k], ys[k + 1], z1), "bz")
        g.box(b, f"shield_arc{k}", (xs[k] - 0.0, ys[k], z0 + 0.05), (xs[k] + 0.22, ys[k + 1], z1 - 0.05), "bz_worn")
    # 聚光阵纹（死纹）：贴在朝灯芯的一面，三道弧线 + 一道竖线，不发光
    g.box(b, "shield_dead_v", (1.15, 13.8, z0 - 0.22), (1.37, 15.22, z0), "dead")
    g.box(b, "shield_dead_a", (1.37, 13.8, z0 - 0.22), (2.3, 14.02, z0), "dead")
    g.box(b, "shield_dead_b", (1.37, 14.4, z0 - 0.22), (1.95, 14.62, z0), "dead")
    g.box(b, "shield_dead_c", (1.37, 15.0, z0 - 0.22), (1.62, 15.22, z0), "dead")
    g.box(b, "shield_edge", (1.0, 13.5, z1), (1.22, 15.85, z1 + 0.22), "bz_worn")      # 直边磨亮


def part_ring(g: Gear) -> None:
    """提环（-x 一侧）：挂钩 + 八段短杆围成的圆环，环面朝前（xy 平面）。"""
    b = "ring_trinket"
    g.box(b, "ring_hook", (-0.95, 5.25, -0.18), (-0.5, 5.6, 0.18), "bz_worn")
    g.ring(b, "ring_loop", (-1.15, 4.8, 0.0), 0.5, 0.22, 0.22, "bz_worn", axis="z", n=8)


def part_soot(g: Gear) -> None:
    """盘沿结焦黑渍（不对称）：贴在盘沿内壁的几块焦渣 + 灯芯根部一圈黑垢。"""
    b = "soot"
    g.box(b, "crust_a", (2.15, FLOOR_Y, -0.9), (2.5, 13.42, 0.7), "soot")
    g.box(b, "crust_b", (-2.5, FLOOR_Y, -1.6), (-2.15, 13.38, -0.2), "soot")
    g.box(b, "crust_c", (-0.9, FLOOR_Y, 2.15), (0.8, 13.4, 2.5), "soot")
    # 盘沿使用磕碰的几处暗痕（贴在沿顶）
    g.box(b, "nick_a", (2.7, RIM_TOP - 0.02, -0.6), (3.0, RIM_TOP + 0.25, -0.1), "soot")
    g.box(b, "nick_b", (-2.4, RIM_TOP - 0.02, 1.9), (-1.9, RIM_TOP + 0.25, 2.25), "soot")
    g.box(b, "nick_c", (-0.3, RIM_TOP - 0.02, -3.0), (0.3, RIM_TOP + 0.25, -2.6), "soot")
    g.box(b, "crust_d", (-0.5, OIL_TOP - 0.04, -0.5), (0.5, OIL_TOP + 0.2, 0.5), "soot")


FLAME_BONE = ("flame", "dish", (0.0, OIL_TOP + 0.5, 0.0), "点亮版的火舌（只在 Lit 模型里：焰心 + 两级火舌，高对比，不做真发光）")


def part_flame(g: Gear) -> None:
    """点亮版的火舌：坐在烧短的灯芯顶上，焰心（亮黄）+ 两级收窄的火舌（橙）。靠对比出光感，不是发光层。"""
    b = "flame"
    y0 = OIL_TOP + 0.5
    g.box(b, "flame_core", (-0.26, y0, -0.26), (0.26, y0 + 0.32, 0.26), "flame_core")
    g.box(b, "flame_mid", (-0.22, y0 + 0.32, -0.22), (0.22, y0 + 0.72, 0.22), "flame")
    g.box(b, "flame_tip", (-0.14, y0 + 0.72, -0.14), (0.14, y0 + 1.08, 0.14), "flame")


PARTS = {
    "toes": part_toes,
    "leg": part_leg,
    "body": part_body,
    "neck": part_neck,
    "dish": part_dish,
    "shield": part_shield,
    "ring": part_ring,
    "soot": part_soot,
}

# 参考图 qing_tong_yan_zu_deng_ref.png（1312×1199）里的整件外框与分段，像素坐标；侧视是从 -x 看（鹅脸朝左）
SPEC = MetalSpec(
    gear_id="qing_tong_yan_zu_deng",
    model_name="QingTongYanZuDeng",
    title="青铜雁足灯 / qing_tong_yan_zu_deng",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=HERE,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=8,
    round_label="round 3/3",
    texture_overrides=BZ_TEXTURES,
    side_yaw=90.0,
    ref_front_box=(117, 28, 574, 1140),
    ref_side_box=(775, 30, 1200, 1140),
    height_parts=(
        ("罩", 28, 205, ("shield_",)),
        ("灯盘", 205, 295, ("dish_",)),
        ("颈 + 头", 295, 560, ("neck_",)),
        ("身", 560, 710, ("body_",)),
        ("颈箍", 710, 750, ("collar_",)),
        ("腿", 750, 990, ("leg_",)),
        ("足", 990, 1140, ("toe_",)),
    ),
    width_parts=(
        ("front", "灯盘", 121, 574, ("dish_",)),
        ("front", "足跨", 140, 540, ("toe_",)),
        ("front", "身", 274, 411, ("body_",)),
        ("front", "腿", 306, 379, ("leg_",)),
        ("side", "灯盘", 778, 1192, ("dish_",)),
        ("side", "身（含尾）", 915, 1145, ("body_",)),
    ),
)


def write_lit() -> None:
    """点亮的高对比版：同一套几何 + 灯芯上一簇火舌，另存 QingTongYanZuDengLit.bbmodel / _tex.bbmodel，附 check_lit.png。"""
    import dataclasses
    import json

    import gear_kit
    from PIL import Image

    lit = dataclasses.replace(SPEC, model_name="QingTongYanZuDengLit", bones=BONES + [FLAME_BONE],
                              parts={**PARTS, "flame": part_flame}, min_bones=9)
    g = lit.build()
    total = metal_review.run_gates(lit, g)
    if total:
        raise SystemExit(f"点亮版门禁仍有 {total} 处违例")
    gear_kit.write_model(g, HERE / "QingTongYanZuDengLit.bbmodel", lit.model_name)
    tex = gear_kit.write_textured(lit, g, HERE / "QingTongYanZuDengLit_tex.bbmodel")
    lo, hi = metal_review.model_bounds(g)
    center = tuple((lo[i] + hi[i]) / 2 for i in range(3))
    span = max(hi[i] - lo[i] for i in range(3)) * 1.1
    cell = 480
    views = (("FRONT 正面", 180.0, 0.0, span), ("3/4 右前上方", -150.0, 18.0, span), ("盘口特写", -150.0, 22.0, span * 0.38))
    sheet = Image.new("RGB", (cell * len(views) + 4 * (len(views) + 1), cell + 40), (28, 29, 33))
    from PIL import ImageDraw
    d = ImageDraw.Draw(sheet)
    d.text((8, 6), f"{lit.title} 点亮版（高对比，不做真发光）· {g.total()} cubes", fill=(235, 235, 235), font=gear_kit.load_font(16))
    magenta = 0
    for k, (label, yaw, pitch, sp) in enumerate(views):
        c = center if sp == span else (0.0, 13.7, 0.0)
        img = metal_review.render_view(tex, yaw, pitch, cell, c, sp, shading="mc")
        magenta += metal_review.count_magenta(img)
        sheet.paste(img, (4 + k * (cell + 4), 34))
        d.text((10 + k * (cell + 4), 38), label, fill=(235, 235, 235), font=gear_kit.load_font(13))
    out = lit.review_dir / "check_lit.png"
    sheet.save(out)
    print(f"✓ 点亮版 {g.total()} cubes → check_lit.png（洋红像素 {magenta}）")


if __name__ == "__main__":
    metal_review.main(SPEC)
    if len(sys.argv) == 1:
        write_lit()
