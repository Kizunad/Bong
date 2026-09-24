#!/usr/bin/env python3
"""粗铁矿（CuTie）—— 泥褐风化结核与生铁金属原矿（纯材料级）。

【对齐 LingTie 架构标准，彻底消灭孤立箱体与平贴面板】：
复用 LingTie 经过 Round 2 人工闸门验证通过的优秀地质结构：
1. iron_lobes: 水滴梭形浑厚矿体，由左瓣泥岩与右瓣金属紧密嵌合，形成正面深凹纵向裂谷。
2. thermal_chasm_veins: 沿裂谷深陷纵贯的曲折地热熔火裂隙（暗红深底 + 金橙主脉 + 炽金火心）。
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
from palette_cutie import (
    ORE_VOID, ORE_DARK, ORE_MUD_MID, ORE_MUD_LIT,
    ORE_IRON_MID, ORE_IRON_LIT, ORE_IRON_SPEC,
    FIRE_DEEP, FIRE_MID, FIRE_HIGH
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

MATS = {
    "iron_void": ORE_VOID,
    "iron_dark": ORE_DARK,
    "iron_mid":  ORE_IRON_MID,
    "iron_lit":  ORE_IRON_LIT,
    "iron_spec": ORE_IRON_SPEC,
    "mud_mid":   ORE_MUD_MID,
    "mud_lit":   ORE_MUD_LIT,
    "fire_deep": FIRE_DEEP,
    "fire_mid":  FIRE_MID,
    "fire_high": FIRE_HIGH,
}


def part_mineral_lobes(rig: Rig):
    """构建水滴卵形原矿的左右两扇互锁咬合矿体（左泥岩，右生铁）。"""
    bone = "mineral_lobes"
    rig.bone(bone, (0.0, 5.0, 0.0))

    org = (0.0, 5.0, 0.0)
    rot = (6.0, 12.0, -8.0) # 保持与 LingTie 一致的自然微倾姿态

    # --- 左侧主瓣：风化泥岩（形成左翼与左上肩峰） ---
    # 左底
    rig.cube(bone, "lobe_left_bottom",
             (-2.6, 0.8, -1.8), (-0.4, 3.2, 1.8),
             rot=rot, org=org, mat="iron_void")
    # 左腹（向外膨出，粗糙泥岩）
    rig.cube(bone, "lobe_left_belly_main",
             (-3.8, 3.2, -2.4), (-0.6, 7.4, 2.2),
             rot=rot, org=org, mat="mud_mid")
    rig.cube(bone, "lobe_left_belly_flange",
             (-4.4, 3.8, -1.4), (-3.2, 6.8, 1.6),
             rot=rot, org=org, mat="mud_lit")
    # 左上肩峰
    rig.cube(bone, "lobe_left_shoulder",
             (-3.2, 7.4, -1.6), (-0.8, 10.2, 1.4),
             rot=rot, org=org, mat="mud_mid")
    rig.cube(bone, "lobe_left_peak",
             (-2.4, 10.2, -1.0), (-1.0, 11.6, 0.8),
             rot=rot, org=org, mat="mud_lit")

    # --- 右侧主瓣：生铁金属（向右上扬起，形成最高主峰） ---
    # 右底
    rig.cube(bone, "lobe_right_bottom",
             (0.4, 1.2, -1.6), (2.4, 3.4, 1.6),
             rot=rot, org=org, mat="iron_void")
    # 右腹（正面受光金属漫射面）
    rig.cube(bone, "lobe_right_belly_main",
             (0.6, 3.4, -2.2), (3.6, 7.8, 2.0),
             rot=rot, org=org, mat="iron_dark")
    rig.cube(bone, "lobe_right_facet_front",
             (0.8, 4.0, 1.4), (3.2, 7.2, 2.6),
             rot=rot, org=org, mat="iron_lit")
    # 右侧受光金属冷亮棱线
    rig.cube(bone, "lobe_right_glint_ridge",
             (3.2, 4.4, -0.4), (3.7, 7.6, 1.4),
             rot=rot, org=org, mat="iron_spec")
    # 右上主峰（高耸扬起，最高达 y=12.8）
    rig.cube(bone, "lobe_right_apex_tower",
             (0.2, 7.8, -1.4), (2.6, 11.2, 1.4),
             rot=rot, org=org, mat="iron_lit")
    rig.cube(bone, "lobe_right_apex_tip",
             (0.6, 11.2, -0.7), (1.8, 12.8, 0.7),
             rot=rot, org=org, mat="iron_spec")

    # --- 底极点与背部支撑脊 ---
    # 底部最下方收尖点
    rig.cube(bone, "nodule_lowest_point",
             (-1.2, 0.0, -1.2), (1.0, 1.0, 1.0),
             rot=rot, org=org, mat="iron_void")
    # 后背深邃浑厚脊背
    rig.cube(bone, "back_reinforce_spine",
             (-1.6, 2.6, -3.2), (1.8, 8.8, -1.8),
             rot=rot, org=org, mat="iron_void")


def part_thermal_chasm_veins(rig: Rig):
    """构建嵌入左右两瓣之间裂谷深处的金橙地热火脉网。"""
    bone = "thermal_chasm_veins"
    rig.bone(bone, (0.0, 5.0, 0.0))

    org = (0.0, 5.0, 0.0)
    rot = (6.0, 12.0, -8.0)

    # 1. 裂谷底部的暗红熔火凹槽（深陷在 z=0.8~1.6 之间形成内凹深缝）
    rig.cube(bone, "chasm_bed_lower",
             (-0.9, 1.6, 0.4), (0.9, 4.6, 1.5),
             rot=rot, org=org, mat="fire_deep")
    rig.cube(bone, "chasm_bed_mid",
             (-1.1, 4.4, 0.6), (1.1, 8.4, 1.7),
             rot=rot, org=org, mat="fire_deep")
    rig.cube(bone, "chasm_bed_upper",
             (-0.6, 8.2, 0.2), (1.2, 11.6, 1.3),
             rot=rot, org=org, mat="fire_deep")

    # 2. 纵贯对角线的曲折金橙主火脉
    rig.cube(bone, "vein_line_0",
             (-0.35, 1.8, 1.45), (0.35, 3.4, 1.85),
             rot=rot, org=org, mat="fire_mid")
    rig.cube(bone, "vein_line_1",
             (-0.5, 3.2, 1.75), (0.2, 4.8, 2.15),
             rot=rot, org=org, mat="fire_mid")
    # 中段核心交汇火核（极亮金黄交汇点）
    rig.cube(bone, "vein_nexus_bright",
             (-0.3, 4.6, 2.1), (0.4, 6.2, 2.5),
             rot=rot, org=org, mat="fire_high")
    rig.cube(bone, "vein_nexus_flare",
             (-0.15, 5.0, 2.3), (0.25, 5.8, 2.62),
             rot=rot, org=org, mat="fire_high")
    # 上段冲向主峰的折裂脉
    rig.cube(bone, "vein_line_2",
             (0.1, 6.0, 1.85), (0.75, 7.8, 2.25),
             rot=rot, org=org, mat="fire_mid")
    rig.cube(bone, "vein_line_3",
             (0.4, 7.6, 1.45), (1.1, 9.8, 1.85),
             rot=rot, org=org, mat="fire_high")
    rig.cube(bone, "vein_line_apex",
             (0.7, 9.6, 0.95), (1.3, 11.8, 1.35),
             rot=rot, org=org, mat="fire_high")

    # 3. 左右发散的毛细地热枝脉
    # 向左腹泥岩撕开的横向裂痕
    rig.cube(bone, "vein_spur_left_low",
             (-2.2, 3.8, 1.6), (-0.4, 4.5, 2.05),
             rot=rot, org=org, mat="fire_mid")
    rig.cube(bone, "vein_spur_left_high",
             (-2.6, 5.8, 1.1), (-0.8, 6.5, 1.75),
             rot=rot, org=org, mat="fire_high")
    rig.cube(bone, "vein_spur_left_tip",
             (-3.4, 6.2, 0.6), (-2.4, 6.8, 1.2),
             rot=rot, org=org, mat="fire_deep")

    # 向右腹生铁分叉的微火脉
    rig.cube(bone, "vein_spur_right_low",
             (0.3, 3.2, 1.6), (1.8, 3.9, 2.15),
             rot=rot, org=org, mat="fire_deep")
    rig.cube(bone, "vein_spur_right_high",
             (0.6, 6.6, 1.8), (2.4, 7.4, 2.35),
             rot=rot, org=org, mat="fire_mid")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_mineral_lobes(rig)
    part_thermal_chasm_veins(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="粗铁矿原矿生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "CuTie.bbmodel", help="输出路径")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = build_rig()
    bb_json = rig.bbmodel("CuTie")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
