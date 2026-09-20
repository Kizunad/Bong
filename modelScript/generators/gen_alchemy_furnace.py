#!/usr/bin/env python3
"""便携行脚炼丹炉（AlchemyFurnace）—— 分部件程序化生成与渐进审查管线。

严格依照 `ref_alchemy_furnace_exploded.png` 爆炸分解图中的独立部件（Group）分步实现：
Part 1: group_leg_assembly   (四角斜撑机械铜足 + 铆钉轴承关节 + 底环托架)
Part 2: group_lower_bowl     (底部下收玄石托盆)
Part 3: group_hearth_core    (内部中空炉膛 + 阶梯炽火 + 八角青铜观察窗)
Part 4: group_outer_shell    (八角鼓腹玄石护甲壳 + 后背青铜阻灵脊梁)
Part 5: group_brass_collar   (上围铆钉黄铜锁箍 + 炉颈收缩台)
Part 6: group_side_handles   (双侧横挑黄铜吊耳与圆环)
Part 7: group_top_cap        (覆斗顶冠排烟阀 + 格栅气孔 + 铰链 + 宝珠提纽)

支持生成全装配或指定单部件，并自动渲染多重视角。
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.rig.rigkit import Rig
from bbmodel_maker import workspace
from palette_alchemy_furnace import (
    STONE_VOID, STONE_DARK, STONE_MID, STONE_LIT,
    BRASS_DARK, BRASS_BASE, BRASS_MID, BRASS_LIT, BRASS_HIGH,
    FIRE_DEEP, FIRE_MID, FIRE_LIT, FIRE_CORE
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

MATS = {
    "stone_void": STONE_VOID,
    "stone_dark": STONE_DARK,
    "stone_mid":  STONE_MID,
    "stone_lit":  STONE_LIT,
    "brass_dark": BRASS_DARK,
    "brass_base": BRASS_BASE,
    "brass_mid":  BRASS_MID,
    "brass_lit":  BRASS_LIT,
    "brass_high": BRASS_HIGH,
    "fire_deep":  FIRE_DEEP,
    "fire_mid":   FIRE_MID,
    "fire_lit":   FIRE_LIT,
    "fire_core":  FIRE_CORE,
}


def add_single_leg(rig: Rig, bone: str, prefix: str,
                   cx: float, cz: float,
                   rot_x: float, rot_z: float,
                   rivet_dx: float, rivet_dz: float):
    """构建单只带轴承方体关节、外撇斜支柱与接地蹄足的黄铜鼎足。"""
    org = (cx, 4.4, cz)
    rot = (rot_x, 0.0, rot_z)

    # 1. 关节立方体 (Joint Box, 2.0W x 1.8H x 2.0D)
    rig.cube(bone, f"{prefix}_joint_box",
             (cx - 1.0, 3.6, cz - 1.0), (cx + 1.0, 5.2, cz + 1.0),
             rot=rot, org=org, mat="brass_mid")
    # 2. 突出圆形/方形铆钉扣 (Rivet)
    rig.cube(bone, f"{prefix}_joint_rivet",
             (cx + rivet_dx - 0.4, 4.0, cz + rivet_dz - 0.4),
             (cx + rivet_dx + 0.4, 4.8, cz + rivet_dz + 0.4),
             rot=rot, org=org, mat="brass_high")

    # 3. 外撇斜支柱 (Leg Strut, 1.4W x 3.6H x 1.4D)
    rig.cube(bone, f"{prefix}_strut",
             (cx - 0.7, 0.6, cz - 0.7), (cx + 0.7, 3.8, cz + 0.7),
             rot=rot, org=org, mat="brass_base")
    # 支柱前侧加固金属棱条
    rig.cube(bone, f"{prefix}_strut_rib",
             (cx + rivet_dx * 0.6 - 0.35, 1.0, cz + rivet_dz * 0.6 - 0.35),
             (cx + rivet_dx * 0.6 + 0.35, 3.4, cz + rivet_dz * 0.6 + 0.35),
             rot=rot, org=org, mat="brass_lit")

    # 4. 接地平稳蹄足 (Foot Pad, 2.0W x 0.8H x 2.0D)
    rig.cube(bone, f"{prefix}_foot_pad",
             (cx - 1.0, 0.0, cz - 1.0), (cx + 1.0, 0.8, cz + 1.0),
             rot=rot, org=org, mat="brass_lit")


def part_leg_assembly(rig: Rig):
    """Part 1: 四角斜撑机械铜足底座（Group: group_leg_assembly）。"""
    bone = "group_leg_assembly"
    rig.bone(bone, (0.0, 0.0, 0.0))

    # 1. 底部托承环（八角形连接中空环，连接四足关节，中心悬空透气）
    rig.cube(bone, "leg_ring_x",
             (-4.0, 4.2, -3.2), (4.0, 5.0, 3.2),
             mat="brass_dark")
    rig.cube(bone, "leg_ring_z",
             (-3.2, 4.2, -4.0), (3.2, 5.0, 4.0),
             mat="brass_dark")

    # 2. 四支对称向外撇出的鼎足（FL, FR, BL, BR）
    d = 3.6
    # 左前足 (FL: X-, Z+)
    add_single_leg(rig, bone, "leg_fl", -d, d, rot_x=18.0, rot_z=-18.0, rivet_dx=-0.9, rivet_dz=0.9)
    # 右前足 (FR: X+, Z+)
    add_single_leg(rig, bone, "leg_fr", d, d, rot_x=18.0, rot_z=18.0, rivet_dx=0.9, rivet_dz=0.9)
    # 左后足 (BL: X-, Z-)
    add_single_leg(rig, bone, "leg_bl", -d, -d, rot_x=-18.0, rot_z=-18.0, rivet_dx=-0.9, rivet_dz=-0.9)
    # 右后足 (BR: X+, Z-)
    add_single_leg(rig, bone, "leg_br", d, -d, rot_x=-18.0, rot_z=18.0, rivet_dx=0.9, rivet_dz=-0.9)


def part_lower_bowl(rig: Rig):
    """Part 2: 下部玄石托盆（Group: group_lower_bowl）。

    对应爆炸图 'Lower Armor Plate (Stability and protection)'：
    向上逐渐外扩的八角弧面阶梯托盆，紧密咬合在支撑四足之上，承托内炉膛与上壳。
    """
    bone = "group_lower_bowl"
    rig.bone(bone, (0.0, 5.5, 0.0))

    # 1. 托盆底部收口座 (Y: 4.2 ~ 5.2，嵌入四足中央环)
    rig.cube(bone, "bowl_base_core",
             (-3.4, 4.2, -3.4), (3.4, 5.2, 3.4),
             mat="stone_void")
    rig.cube(bone, "bowl_base_oct_x",
             (-4.0, 4.2, -2.6), (4.0, 5.2, 2.6),
             mat="stone_dark")
    rig.cube(bone, "bowl_base_oct_z",
             (-2.6, 4.2, -4.0), (2.6, 5.2, 4.0),
             mat="stone_dark")

    # 2. 托盆中层外展弧段 (Y: 5.2 ~ 6.4，向上展开至宽 9.2 格)
    rig.cube(bone, "bowl_mid_cross_x",
             (-4.6, 5.2, -3.4), (4.6, 6.4, 3.4),
             mat="stone_dark")
    rig.cube(bone, "bowl_mid_cross_z",
             (-3.4, 5.2, -4.6), (3.4, 6.4, 4.6),
             mat="stone_dark")
    # 中层四角倒角斜面
    d_mid = 3.6
    for i, (cx, cz, ry) in enumerate([
        (d_mid, d_mid, 45.0),
        (-d_mid, d_mid, -45.0),
        (-d_mid, -d_mid, 45.0),
        (d_mid, -d_mid, -45.0)
    ]):
        rig.cube(bone, f"bowl_mid_chamfer_{i}",
                 (cx - 1.2, 5.2, cz - 1.2), (cx + 1.2, 6.4, cz + 1.2),
                 rot=(0.0, ry, 0.0), org=(cx, 5.8, cz), mat="stone_mid")

    # 3. 托盆上沿外扩厚实盆口 (Y: 6.4 ~ 7.4，宽达 10.4 格)
    rig.cube(bone, "bowl_rim_cross_x",
             (-5.2, 6.4, -4.0), (5.2, 7.4, 4.0),
             mat="stone_dark")
    rig.cube(bone, "bowl_rim_cross_z",
             (-4.0, 6.4, -5.2), (4.0, 7.4, 5.2),
             mat="stone_dark")
    # 上沿四角倒角与外凸青铜加固托爪
    d_rim = 4.2
    for i, (cx, cz, ry) in enumerate([
        (d_rim, d_rim, 45.0),
        (-d_rim, d_rim, -45.0),
        (-d_rim, -d_rim, 45.0),
        (d_rim, -d_rim, -45.0)
    ]):
        rig.cube(bone, f"bowl_rim_chamfer_{i}",
                 (cx - 1.3, 6.4, cz - 1.3), (cx + 1.3, 7.4, cz + 1.3),
                 rot=(0.0, ry, 0.0), org=(cx, 6.9, cz), mat="stone_mid")
        # 四角青铜扣爪 (Bracket)
        rig.cube(bone, f"bowl_bracket_{i}",
                 (cx - 0.6, 6.2, cz + 0.9), (cx + 0.6, 7.2, cz + 1.35),
                 rot=(0.0, ry, 0.0), org=(cx, 6.7, cz), mat="brass_lit")

    # 4. 内部中空凹槽底（承托内层炉膛）
    rig.cube(bone, "bowl_cavity_floor",
             (-3.0, 6.8, -3.0), (3.0, 7.4, 3.0),
             mat="stone_void")


def part_hearth_core(rig: Rig):
    """Part 3: 内部炉膛地火与八角观察窗（Group: group_hearth_core）。

    对应爆炸图 'Furnace Hearth (Core chamber, fire, observation window)'：
    - 炉膛内部耐火暗红炭床与阶梯炽火（FIRE_DEEP, FIRE_MID, FIRE_LIT, FIRE_CORE）
    - 炉膛八角内胆壁（STONE_VOID / STONE_DARK）
    - 前部外凸的八角青铜边框投药/观察窗（BRASS_MID / BRASS_LIT / BRASS_HIGH）
    - 窗内透过观察窗可见的炽热火核
    """
    bone = "group_hearth_core"
    rig.bone(bone, (0.0, 9.0, 0.0))

    # 1. 炉膛内胆底与炭床 (Y: 7.2 ~ 8.4)
    # 底层炭床暗红火核
    rig.cube(bone, "hearth_coal_bed",
             (-2.8, 7.2, -2.8), (2.8, 8.0, 2.8),
             mat="fire_deep")
    # 中层阶梯炽火核心
    rig.cube(bone, "hearth_fire_mid",
             (-2.0, 8.0, -2.0), (2.0, 9.0, 2.0),
             mat="fire_mid")
    # 核心金黄火苗与极亮白黄火苗
    rig.cube(bone, "hearth_fire_lit",
             (-1.2, 9.0, -1.2), (1.2, 10.2, 1.2),
             mat="fire_lit")
    rig.cube(bone, "hearth_fire_core",
             (-0.6, 9.4, -0.6), (0.6, 11.0, 0.6),
             mat="fire_core")

    # 2. 内胆侧壁与隔热层 (Y: 7.4 ~ 11.8)
    # 内胆十字壁 (X/Z 方向耐火壁)
    rig.cube(bone, "hearth_wall_back",
             (-3.0, 7.4, -3.4), (3.0, 11.6, -2.8),
             mat="stone_void")
    rig.cube(bone, "hearth_wall_left",
             (-3.4, 7.4, -3.0), (-2.8, 11.6, 3.0),
             mat="stone_void")
    rig.cube(bone, "hearth_wall_right",
             (2.8, 7.4, -3.0), (3.4, 11.6, 3.0),
             mat="stone_void")

    # 3. 前部观察/投药窗 (Front Observation & Feeding Window, 面向 +Z)
    # 窗框位置: Y: 8.2 ~ 11.4, Z: 3.4 ~ 5.6
    # 3.1 观察窗外凸喉管/通道 (连接内部火核与外部窗框)
    rig.cube(bone, "hearth_window_throat",
             (-2.2, 8.4, 2.8), (2.2, 11.2, 4.4),
             mat="stone_dark")
    # 通道内发光火光映射面
    rig.cube(bone, "hearth_window_fire_glow",
             (-1.4, 8.8, 4.3), (1.4, 10.8, 4.6),
             mat="fire_lit")
    rig.cube(bone, "hearth_window_fire_bright",
             (-0.8, 9.2, 4.5), (0.8, 10.4, 4.8),
             mat="fire_core")

    # 3.2 八角青铜窗框 (Octagonal Brass Window Frame, 凸出在 Z: 4.6 ~ 5.4)
    # 上下横框
    rig.cube(bone, "window_frame_top",
             (-1.8, 11.2, 4.6), (1.8, 12.0, 5.4),
             mat="brass_lit")
    rig.cube(bone, "window_frame_bottom",
             (-1.8, 7.8, 4.6), (1.8, 8.6, 5.4),
             mat="brass_base")
    # 左右竖框
    rig.cube(bone, "window_frame_left",
             (-2.6, 8.4, 4.6), (-1.8, 11.4, 5.4),
             mat="brass_mid")
    rig.cube(bone, "window_frame_right",
             (1.8, 8.4, 4.6), (2.6, 11.4, 5.4),
             mat="brass_mid")
    # 四角倒角斜框 (Chamfered Corner Brackets at 45 deg)
    w_d = 1.8
    for i, (cx, cy, rz) in enumerate([
        (w_d, 11.0, -45.0),
        (-w_d, 11.0, 45.0),
        (-w_d, 8.8, -45.0),
        (w_d, 8.8, 45.0),
    ]):
        rig.cube(bone, f"window_frame_chamfer_{i}",
                 (cx - 0.5, cy - 0.5, 4.6), (cx + 0.5, cy + 0.5, 5.4),
                 rot=(0.0, 0.0, rz), org=(cx, cy, 5.0), mat="brass_lit")

    # 3.3 观察窗青铜铆钉铰链与锁扣 (Hinge & Latch)
    rig.cube(bone, "window_hinge_top",
             (-0.6, 11.8, 5.3), (0.6, 12.4, 5.7),
             mat="brass_high")
    rig.cube(bone, "window_latch_bottom",
             (-0.4, 7.4, 5.3), (0.4, 8.0, 5.7),
             mat="brass_high")


def part_outer_shell(rig: Rig):
    """Part 4: 八角鼓腹玄石护甲外壳与青铜阻灵背梁（Group: group_outer_shell）。

    对应爆炸图 'Outer Armor Plates (Heavy thermal protection and reinforcement)'：
    - 左右及后部围合的八角鼓腹厚实玄石外壳（STONE_DARK, STONE_MID, STONE_LIT）
    - 前部开出观察窗矩形/八角让位槽，使观察窗紧凑外凸咬合
    - 后部中央贯穿一根垂直青铜阻灵脊梁（BRASS_MID, BRASS_LIT），强化机械与仙道构件感
    - 上部内收至炉颈收缩台，准备承接上围黄铜锁箍
    """
    bone = "group_outer_shell"
    rig.bone(bone, (0.0, 10.0, 0.0))

    # 外壳高度跨度: Y: 7.2 ~ 12.8，分下收、中鼓、上收三层
    # 1. 壳体下层下收弧段 (Y: 7.2 ~ 8.8)
    # 后壁与左右侧壁
    rig.cube(bone, "shell_low_back",
             (-4.8, 7.2, -5.2), (4.8, 8.8, -3.6),
             mat="stone_dark")
    rig.cube(bone, "shell_low_left",
             (-5.4, 7.2, -3.8), (-3.8, 8.8, 3.8),
             mat="stone_dark")
    rig.cube(bone, "shell_low_right",
             (3.8, 7.2, -3.8), (5.4, 8.8, 3.8),
             mat="stone_dark")
    # 前壁两侧（给观察窗正中留孔）
    rig.cube(bone, "shell_low_front_l",
             (-5.0, 7.2, 3.6), (-2.4, 8.8, 5.0),
             mat="stone_dark")
    rig.cube(bone, "shell_low_front_r",
             (2.4, 7.2, 3.6), (5.0, 8.8, 5.0),
             mat="stone_dark")
    # 下层四角 45° 倒角斜板
    d_l = 4.2
    for i, (cx, cz, ry) in enumerate([
        (d_l, d_l, 45.0),
        (-d_l, d_l, -45.0),
        (-d_l, -d_l, 45.0),
        (d_l, -d_l, -45.0)
    ]):
        rig.cube(bone, f"shell_low_chamfer_{i}",
                 (cx - 1.2, 7.2, cz - 1.2), (cx + 1.2, 8.8, cz + 1.2),
                 rot=(0.0, ry, 0.0), org=(cx, 8.0, cz), mat="stone_mid")

    # 2. 壳体中层鼓腹最宽处 (Y: 8.8 ~ 11.4, 外宽扩展至 11.4 ~ 11.8)
    # 后壁厚板
    rig.cube(bone, "shell_mid_back",
             (-5.2, 8.8, -5.6), (5.2, 11.4, -3.8),
             mat="stone_dark")
    # 左右侧壁鼓腹板（为侧吊耳提供基座）
    rig.cube(bone, "shell_mid_left",
             (-5.8, 8.8, -4.0), (-4.0, 11.4, 4.0),
             mat="stone_dark")
    rig.cube(bone, "shell_mid_right",
             (4.0, 8.8, -4.0), (5.8, 11.4, 4.0),
             mat="stone_dark")
    # 前壁两侧
    rig.cube(bone, "shell_mid_front_l",
             (-5.4, 8.8, 3.8), (-2.5, 11.4, 5.2),
             mat="stone_dark")
    rig.cube(bone, "shell_mid_front_r",
             (2.5, 8.8, 3.8), (5.4, 11.4, 5.2),
             mat="stone_dark")
    # 中层四角倒角受光斜面
    d_m = 4.6
    for i, (cx, cz, ry) in enumerate([
        (d_m, d_m, 45.0),
        (-d_m, d_m, -45.0),
        (-d_m, -d_m, 45.0),
        (d_m, -d_m, -45.0)
    ]):
        rig.cube(bone, f"shell_mid_chamfer_{i}",
                 (cx - 1.3, 8.8, cz - 1.3), (cx + 1.3, 11.4, cz + 1.3),
                 rot=(0.0, ry, 0.0), org=(cx, 10.1, cz), mat="stone_mid")

    # 3. 壳体上层向内收拢至炉颈 (Y: 11.4 ~ 12.8, 宽度收缩至 9.6)
    # 后壁与前壁门楣
    rig.cube(bone, "shell_top_back",
             (-4.6, 11.4, -4.8), (4.6, 12.8, -3.4),
             mat="stone_dark")
    rig.cube(bone, "shell_top_front_lintel",
             (-4.6, 12.0, 3.4), (4.6, 12.8, 4.8),
             mat="stone_mid")
    # 左右侧壁
    rig.cube(bone, "shell_top_left",
             (-4.8, 11.4, -3.6), (-3.4, 12.8, 3.6),
             mat="stone_dark")
    rig.cube(bone, "shell_top_right",
             (3.4, 11.4, -3.6), (4.8, 12.8, 3.6),
             mat="stone_dark")
    # 上层四角倒角收口
    d_t = 3.8
    for i, (cx, cz, ry) in enumerate([
        (d_t, d_t, 45.0),
        (-d_t, d_t, -45.0),
        (-d_t, -d_t, 45.0),
        (d_t, -d_t, -45.0)
    ]):
        rig.cube(bone, f"shell_top_chamfer_{i}",
                 (cx - 1.1, 11.4, cz - 1.1), (cx + 1.1, 12.8, cz + 1.1),
                 rot=(0.0, ry, 0.0), org=(cx, 12.1, cz), mat="stone_lit")

    # 4. 后背青铜阻灵加固脊梁 (Reinforced Spine on -Z)
    rig.cube(bone, "shell_spine_main",
             (-1.0, 7.6, -5.9), (1.0, 12.6, -5.3),
             mat="brass_mid")
    rig.cube(bone, "shell_spine_rib_top",
             (-1.4, 12.0, -6.1), (1.4, 12.6, -5.2),
             mat="brass_lit")
    rig.cube(bone, "shell_spine_rib_mid",
             (-1.6, 9.6, -6.2), (1.6, 10.4, -5.2),
             mat="brass_lit")
    rig.cube(bone, "shell_spine_rib_low",
             (-1.4, 7.6, -6.1), (1.4, 8.2, -5.2),
             mat="brass_lit")


def part_brass_collar(rig: Rig):
    """Part 5: 上围黄铜锁箍与炉颈收缩台（Group: group_brass_collar）。

    对应爆炸图 'Upper Head / Middle Collar (Locking ring, rivets, neck housing)'：
    - 炉颈下部过渡玄石环（neck_stone_*）
    - 外凸八角厚实黄铜锁箍带（collar_ring_*）
    - 锁箍外围 8 枚凸出方形黄铜铆钉/锁栓（collar_rivet_*）
    - 锁箍上部收缩齿轮套管与玄石炉颈（neck_housing_*）
    """
    bone = "group_brass_collar"
    rig.bone(bone, (0.0, 14.0, 0.0))

    # 1. 炉颈下层过渡基座 (Y: 12.4 ~ 13.2, 承接外壳顶部)
    rig.cube(bone, "neck_base_core",
             (-4.4, 12.4, -4.4), (4.4, 13.2, 4.4),
             mat="stone_dark")
    rig.cube(bone, "neck_base_oct_x",
             (-4.8, 12.4, -3.2), (4.8, 13.2, 3.2),
             mat="stone_mid")
    rig.cube(bone, "neck_base_oct_z",
             (-3.2, 12.4, -4.8), (3.2, 13.2, 4.8),
             mat="stone_mid")

    # 2. 外凸八角厚实黄铜锁箍主带 (Y: 13.0 ~ 14.4, 外宽达到 10.4)
    # 十字主箍环
    rig.cube(bone, "collar_main_cross_x",
             (-5.2, 13.0, -3.8), (5.2, 14.4, 3.8),
             mat="brass_mid")
    rig.cube(bone, "collar_main_cross_z",
             (-3.8, 13.0, -5.2), (3.8, 14.4, 5.2),
             mat="brass_mid")
    # 锁箍上下两道加固压条 (Upper & Lower Molded Trim)
    rig.cube(bone, "collar_trim_top_x",
             (-5.4, 14.1, -3.9), (5.4, 14.5, 3.9),
             mat="brass_lit")
    rig.cube(bone, "collar_trim_top_z",
             (-3.9, 14.1, -5.4), (3.9, 14.5, 5.4),
             mat="brass_lit")
    rig.cube(bone, "collar_trim_bot_x",
             (-5.4, 12.9, -3.9), (5.4, 13.3, 3.9),
             mat="brass_dark")
    rig.cube(bone, "collar_trim_bot_z",
             (-3.9, 12.9, -5.4), (3.9, 13.3, 5.4),
             mat="brass_dark")

    # 八角 45° 倒角斜板
    d_c = 4.2
    for i, (cx, cz, ry) in enumerate([
        (d_c, d_c, 45.0),
        (-d_c, d_c, -45.0),
        (-d_c, -d_c, 45.0),
        (d_c, -d_c, -45.0)
    ]):
        rig.cube(bone, f"collar_chamfer_{i}",
                 (cx - 1.4, 13.0, cz - 1.4), (cx + 1.4, 14.4, cz + 1.4),
                 rot=(0.0, ry, 0.0), org=(cx, 13.7, cz), mat="brass_base")
        # 斜面上的压条
        rig.cube(bone, f"collar_chamfer_trim_{i}",
                 (cx - 1.45, 14.1, cz - 1.45), (cx + 1.45, 14.5, cz + 1.45),
                 rot=(0.0, ry, 0.0), org=(cx, 13.7, cz), mat="brass_lit")

    # 3. 锁箍四周凸出铆钉/方栓 (Rivets & Studs)
    # 前后左右 4 枚正向铆钉
    rig.cube(bone, "collar_rivet_front",
             (-0.5, 13.4, 5.1), (0.5, 14.0, 5.6),
             mat="brass_high")
    rig.cube(bone, "collar_rivet_back",
             (-0.5, 13.4, -5.6), (0.5, 14.0, -5.1),
             mat="brass_high")
    rig.cube(bone, "collar_rivet_left",
             (-5.6, 13.4, -0.5), (-5.1, 14.0, 0.5),
             mat="brass_high")
    rig.cube(bone, "collar_rivet_right",
             (5.1, 13.4, -0.5), (5.6, 14.0, 0.5),
             mat="brass_high")
    # 四角 4 枚对角铆钉
    d_r = 4.8
    for i, (cx, cz, ry) in enumerate([
        (d_r, d_r, 45.0),
        (-d_r, d_r, -45.0),
        (-d_r, -d_r, 45.0),
        (d_r, -d_r, -45.0)
    ]):
        rig.cube(bone, f"collar_rivet_diag_{i}",
                 (cx - 0.4, 13.4, cz + 0.4), (cx + 0.4, 14.0, cz + 0.9),
                 rot=(0.0, ry, 0.0), org=(cx, 13.7, cz), mat="brass_high")

    # 4. 锁箍上方收口玄石炉颈 (Y: 14.4 ~ 15.8, 宽度收至 8.2)
    rig.cube(bone, "neck_housing_core",
             (-3.8, 14.4, -3.8), (3.8, 15.8, 3.8),
             mat="stone_dark")
    rig.cube(bone, "neck_housing_cross_x",
             (-4.2, 14.4, -2.8), (4.2, 15.8, 2.8),
             mat="stone_mid")
    rig.cube(bone, "neck_housing_cross_z",
             (-2.8, 14.4, -4.2), (2.8, 15.8, 4.2),
             mat="stone_mid")
    # 炉颈顶端青铜承托圈 (准备咬合顶盖)
    rig.cube(bone, "neck_lip_brass",
             (-4.0, 15.5, -4.0), (4.0, 16.0, 4.0),
             mat="brass_lit")


def part_side_handles(rig: Rig):
    """Part 6: 双侧横挑黄铜吊耳与圆环提手（Group: group_side_handles）。

    对应爆炸图 'Side Handle (Brass handle for maintenance and carrying)'：
    - 位于左右侧壁鼓腹最宽处（X = -5.8 / +5.8, Y: 9.4 ~ 11.2）
    - 紧贴玄石外壳的加固基座（BRASS_DARK, BRASS_BASE）
    - 横向向外挑出的青铜 U 形吊环座（BRASS_MID, BRASS_LIT）
    - 下垂的中空圆角黄铜提手圆环（BRASS_LIT, BRASS_HIGH）
    """
    bone = "group_side_handles"
    rig.bone(bone, (0.0, 10.0, 0.0))

    for side, sign in [("left", -1.0), ("right", 1.0)]:
        # 1. 紧贴外壳的加固铜基座 (Base Plate on Shell, X: sign*5.2 ~ sign*5.9)
        x_in = sign * 5.2
        x_out = sign * 5.9
        x_min, x_max = min(x_in, x_out), max(x_in, x_out)
        rig.cube(bone, f"handle_base_{side}",
                 (x_min, 9.4, -1.2), (x_max, 11.2, 1.2),
                 mat="brass_base")
        # 基座上下铆钉
        rig.cube(bone, f"handle_rivet_top_{side}",
                 (sign * 5.7 - 0.25, 10.7, -0.4), (sign * 5.7 + 0.25, 11.1, 0.4),
                 mat="brass_high")
        rig.cube(bone, f"handle_rivet_bot_{side}",
                 (sign * 5.7 - 0.25, 9.5, -0.4), (sign * 5.7 + 0.25, 9.9, 0.4),
                 mat="brass_high")

        # 2. 横向向外探出的吊耳固定鼻 (Lobe / Ear Bracket, X: sign*5.9 ~ sign*6.7)
        x_lobe_in = sign * 5.9
        x_lobe_out = sign * 6.7
        x_lmin, x_lmax = min(x_lobe_in, x_lobe_out), max(x_lobe_in, x_lobe_out)
        rig.cube(bone, f"handle_bracket_{side}",
                 (x_lmin, 9.8, -0.8), (x_lmax, 10.8, 0.8),
                 mat="brass_lit")
        # 贯通吊耳的铜转轴销栓 (Pivot Pin, 沿 Z 轴贯通)
        rig.cube(bone, f"handle_pin_{side}",
                 (sign * 6.3 - 0.35, 10.0, -1.1), (sign * 6.3 + 0.35, 10.6, 1.1),
                 mat="brass_high")

        # 3. 悬挂垂下的中空中圆角提环 (Hanging Loop Handle)
        # 提环主体位于 X: sign*6.6 ~ sign*7.3, Y: 8.0 ~ 10.4, Z: -1.0 ~ 1.0
        x_ring_in = sign * 6.6
        x_ring_out = sign * 7.3
        xr_min, xr_max = min(x_ring_in, x_ring_out), max(x_ring_in, x_ring_out)
        # 上横梁 (套在销栓下方)
        rig.cube(bone, f"handle_ring_top_{side}",
                 (xr_min, 9.9, -0.8), (xr_max, 10.4, 0.8),
                 mat="brass_lit")
        # 下横梁
        rig.cube(bone, f"handle_ring_bot_{side}",
                 (xr_min, 8.0, -0.7), (xr_max, 8.5, 0.7),
                 mat="brass_lit")
        # 前后竖边 (形成中空环)
        rig.cube(bone, f"handle_ring_front_{side}",
                 (xr_min, 8.4, 0.5), (xr_max, 10.0, 0.9),
                 mat="brass_mid")
        rig.cube(bone, f"handle_ring_back_{side}",
                 (xr_min, 8.4, -0.9), (xr_max, 10.0, -0.5),
                 mat="brass_mid")


def part_top_cap(rig: Rig):
    """Part 7: 覆斗顶冠排烟阀与宝珠提纽（Group: group_top_cap）。

    对应爆炸图 'Top Cap / Decorative Crown (Ventilation slots, lid, hinge, finial)'：
    - 下层阶梯覆斗飞檐顶盖（cap_lid_*）
    - 中层环形排烟通风气孔格栅（cap_vent_*）
    - 排烟阀单侧青铜连接铰链与排气阀嘴（cap_hinge_*）
    - 上层收拢飞檐与顶端宝珠提纽（cap_finial_*）
    """
    bone = "group_top_cap"
    rig.bone(bone, (0.0, 16.0, 0.0))

    # 1. 顶盖下沿覆斗大飞檐（Eaves Lid, Y: 15.6 ~ 16.6, 罩住炉颈上圈，宽达 9.0）
    # 大十字覆斗底
    rig.cube(bone, "cap_eaves_cross_x",
             (-4.5, 15.6, -3.2), (4.5, 16.4, 3.2),
             mat="brass_base")
    rig.cube(bone, "cap_eaves_cross_z",
             (-3.2, 15.6, -4.5), (3.2, 16.4, 4.5),
             mat="brass_base")
    # 八角 45° 倒角飞檐
    d_e = 3.6
    for i, (cx, cz, ry) in enumerate([
        (d_e, d_e, 45.0),
        (-d_e, d_e, -45.0),
        (-d_e, -d_e, 45.0),
        (d_e, -d_e, -45.0)
    ]):
        rig.cube(bone, f"cap_eaves_chamfer_{i}",
                 (cx - 1.2, 15.6, cz - 1.2), (cx + 1.2, 16.4, cz + 1.2),
                 rot=(0.0, ry, 0.0), org=(cx, 16.0, cz), mat="brass_lit")

    # 飞檐上层小阶梯收拢台 (Y: 16.4 ~ 17.0, 宽 7.6)
    rig.cube(bone, "cap_eaves_tier2_core",
             (-3.6, 16.4, -3.6), (3.6, 17.0, 3.6),
             mat="brass_mid")

    # 2. 中层排烟通风格栅楼（Ventilation Grille Tower, Y: 17.0 ~ 18.2, 宽度收至 6.4）
    # 格栅内部深邃排烟暗腔 (Void Chamber)
    rig.cube(bone, "cap_vent_chamber_core",
             (-2.6, 17.0, -2.6), (2.6, 18.2, 2.6),
             mat="stone_void")
    # 四面排烟格栅柱 (Vertical Brass Slits / Pillars)
    # 前面 2 根格栅柱
    rig.cube(bone, "cap_vent_slit_fl",
             (-1.8, 17.0, 2.4), (-0.8, 18.2, 2.9),
             mat="brass_lit")
    rig.cube(bone, "cap_vent_slit_fr",
             (0.8, 17.0, 2.4), (1.8, 18.2, 2.9),
             mat="brass_lit")
    # 后面 2 根格栅柱
    rig.cube(bone, "cap_vent_slit_bl",
             (-1.8, 17.0, -2.9), (-0.8, 18.2, -2.4),
             mat="brass_lit")
    rig.cube(bone, "cap_vent_slit_br",
             (0.8, 17.0, -2.9), (1.8, 18.2, -2.4),
             mat="brass_lit")
    # 左面 2 根格栅柱
    rig.cube(bone, "cap_vent_slit_lf",
             (-2.9, 17.0, 0.8), (-2.4, 18.2, 1.8),
             mat="brass_lit")
    rig.cube(bone, "cap_vent_slit_lb",
             (-2.9, 17.0, -1.8), (-2.4, 18.2, -0.8),
             mat="brass_lit")
    # 右面 2 根格栅柱
    rig.cube(bone, "cap_vent_slit_rf",
             (2.4, 17.0, 0.8), (2.9, 18.2, 1.8),
             mat="brass_lit")
    rig.cube(bone, "cap_vent_slit_rb",
             (2.4, 17.0, -1.8), (2.9, 18.2, -0.8),
             mat="brass_lit")

    # 单侧外凸排气安全阀嘴/铰链扣 (Valve / Hinge on +X / -Z corner)
    rig.cube(bone, "cap_valve_nozzle",
             (2.6, 17.2, -1.0), (3.6, 18.0, 0.0),
             mat="brass_high")

    # 3. 排烟楼上部屋顶盖板 (Y: 18.2 ~ 19.0, 外挑至宽 7.2)
    rig.cube(bone, "cap_roof_lower",
             (-3.6, 18.2, -3.6), (3.6, 18.8, 3.6),
             mat="brass_mid")
    rig.cube(bone, "cap_roof_trim_x",
             (-3.8, 18.2, -2.6), (3.8, 18.8, 2.6),
             mat="brass_lit")
    rig.cube(bone, "cap_roof_trim_z",
             (-2.6, 18.2, -3.8), (2.6, 18.8, 3.8),
             mat="brass_lit")

    # 4. 顶端宝珠提纽（Finial Crown & Knob, Y: 19.0 ~ 22.0）
    # 宝珠底座八角托盘 (Y: 18.8 ~ 19.4)
    rig.cube(bone, "cap_finial_base",
             (-1.8, 18.8, -1.8), (1.8, 19.4, 1.8),
             mat="brass_dark")
    # 细脖颈 (Neck, Y: 19.4 ~ 20.2)
    rig.cube(bone, "cap_finial_neck",
             (-0.8, 19.4, -0.8), (0.8, 20.2, 0.8),
             mat="brass_lit")
    # 中间膨大宝珠体 (Sphere Core, Y: 20.2 ~ 21.4)
    rig.cube(bone, "cap_finial_ball_core",
             (-1.4, 20.2, -1.4), (1.4, 21.4, 1.4),
             mat="brass_lit")
    rig.cube(bone, "cap_finial_ball_crest_x",
             (-1.7, 20.5, -0.9), (1.7, 21.1, 0.9),
             mat="brass_high")
    rig.cube(bone, "cap_finial_ball_crest_z",
             (-0.9, 20.5, -1.7), (0.9, 21.1, 1.7),
             mat="brass_high")
    # 顶端尖纽 (Top Crown Tip, Y: 21.4 ~ 22.2)
    rig.cube(bone, "cap_finial_tip",
             (-0.5, 21.4, -0.5), (0.5, 22.2, 0.5),
             mat="brass_high")


def build_furnace_rig(part: str | None = None) -> Rig:
    rig = Rig(MATS, swatch=8)
    if part == "leg":
        part_leg_assembly(rig)
    elif part == "bowl_only":
        part_lower_bowl(rig)
    elif part == "hearth_only":
        part_hearth_core(rig)
    elif part == "shell_only":
        part_outer_shell(rig)
    elif part == "collar_only":
        part_brass_collar(rig)
    elif part == "handles_only":
        part_side_handles(rig)
    elif part == "cap_only":
        part_top_cap(rig)
    elif part == "step3" or part == "hearth":
        part_leg_assembly(rig)
        part_lower_bowl(rig)
        part_hearth_core(rig)
    elif part == "step4" or part == "shell":
        part_leg_assembly(rig)
        part_lower_bowl(rig)
        part_hearth_core(rig)
        part_outer_shell(rig)
    elif part == "step5" or part == "collar":
        part_leg_assembly(rig)
        part_lower_bowl(rig)
        part_hearth_core(rig)
        part_outer_shell(rig)
        part_brass_collar(rig)
    elif part == "step6" or part == "handles":
        part_leg_assembly(rig)
        part_lower_bowl(rig)
        part_hearth_core(rig)
        part_outer_shell(rig)
        part_brass_collar(rig)
        part_side_handles(rig)
    elif part == "step7" or part == "all" or part is None:
        part_leg_assembly(rig)
        part_lower_bowl(rig)
        part_hearth_core(rig)
        part_outer_shell(rig)
        part_brass_collar(rig)
        part_side_handles(rig)
        part_top_cap(rig)
    elif part == "bowl" or part == "step2":
        part_leg_assembly(rig)
        part_lower_bowl(rig)
    else:
        part_leg_assembly(rig)
        part_lower_bowl(rig)
        part_hearth_core(rig)
        part_outer_shell(rig)
        part_brass_collar(rig)
        part_side_handles(rig)
        part_top_cap(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="炼丹炉程序化模型生成与分步审查")
    parser.add_argument("--part", choices=["leg", "bowl_only", "hearth_only", "shell_only", "collar_only", "handles_only", "cap_only",
                                           "bowl", "step2", "step3", "step4", "step5", "step6", "step7",
                                           "hearth", "shell", "collar", "handles", "cap", "all"],
                        default="all", help="指定审查的部件")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "AlchemyFurnace.bbmodel", help="输出路径")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = build_furnace_rig(args.part if args.part != "all" else None)
    bb_json = rig.bbmodel("AlchemyFurnace")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out} (部件: {args.part})")


if __name__ == "__main__":
    main()
