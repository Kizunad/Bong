#!/usr/bin/env python3
"""凡铁矿（FanTie）—— 阶梯立方断层与对角金色火裂原矿 Blockbench .bbmodel 生成器。

【对齐 LingTie 架构标准，彻底破除盒式嵌套】：
原画特征深度还原（`fan_tie.png`）：
1. stepped_iron_lobes: 左右两大扇互锁咬合的深灰凡铁金属矿体，带阶梯立方断层，斜立耸起。
2. golden_chasm_veins: 沿中央咬合深裂谷纵贯穿行的曲折金色地火脉网（暗金深底 + 灿金主脉 + 炽金火核）。
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
from palette_fantie import (
    IRON_VOID, IRON_DARK, IRON_MID, IRON_LIT, IRON_SPEC,
    GOLD_DEEP, GOLD_MID, GOLD_HIGH
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

MATS = {
    "iron_void": IRON_VOID,
    "iron_dark": IRON_DARK,
    "iron_mid":  IRON_MID,
    "iron_lit":  IRON_LIT,
    "iron_spec": IRON_SPEC,
    "gold_deep": GOLD_DEEP,
    "gold_mid":  GOLD_MID,
    "gold_high": GOLD_HIGH,
}


def part_stepped_iron_lobes(rig: Rig):
    """构建凡铁原矿的左右两大咬合铁体（左瓣下阶断层，右瓣高耸断层）。"""
    bone = "stepped_lobes"
    rig.bone(bone, (0.0, 5.0, 0.0))

    org = (0.0, 5.0, 0.0)
    rot = (6.0, 14.0, -8.0) # 保持与 LingTie 一致的自然微倾

    # --- 左侧铁瓣（较低阶梯断层，平整断口） ---
    # 左底
    rig.cube(bone, "lobe_left_bottom",
             (-2.6, 0.8, -1.8), (-0.4, 3.2, 1.8),
             rot=rot, org=org, mat="iron_void")
    # 左腹主阶梯
    rig.cube(bone, "lobe_left_belly_main",
             (-3.8, 3.2, -2.4), (-0.6, 7.4, 2.2),
             rot=rot, org=org, mat="iron_dark")
    rig.cube(bone, "lobe_left_belly_flange",
             (-4.4, 3.8, -1.4), (-3.2, 6.8, 1.6),
             rot=rot, org=org, mat="iron_mid")
    # 左肩部平阶
    rig.cube(bone, "lobe_left_shoulder",
             (-3.2, 7.4, -1.6), (-0.8, 10.2, 1.4),
             rot=rot, org=org, mat="iron_mid")
    rig.cube(bone, "lobe_left_peak",
             (-2.4, 10.2, -1.0), (-1.0, 11.6, 0.8),
             rot=rot, org=org, mat="iron_lit")

    # --- 右侧铁瓣（高位阶梯断崖，直通顶峰） ---
    # 右底
    rig.cube(bone, "lobe_right_bottom",
             (0.4, 1.2, -1.6), (2.4, 3.4, 1.6),
             rot=rot, org=org, mat="iron_void")
    # 右腹（金属断层漫射面）
    rig.cube(bone, "lobe_right_belly_main",
             (0.6, 3.4, -2.2), (3.6, 7.8, 2.0),
             rot=rot, org=org, mat="iron_dark")
    rig.cube(bone, "lobe_right_facet_front",
             (0.8, 4.0, 1.4), (3.2, 7.2, 2.6),
             rot=rot, org=org, mat="iron_lit")
    # 右侧金属反光棱
    rig.cube(bone, "lobe_right_glint_ridge",
             (3.2, 4.4, -0.4), (3.7, 7.6, 1.4),
             rot=rot, org=org, mat="iron_spec")
    # 右上主峰（高耸断层，最高达 y=12.8）
    rig.cube(bone, "lobe_right_apex_tower",
             (0.2, 7.8, -1.4), (2.6, 11.2, 1.4),
             rot=rot, org=org, mat="iron_lit")
    rig.cube(bone, "lobe_right_apex_tip",
             (0.6, 11.2, -0.7), (1.8, 12.8, 0.7),
             rot=rot, org=org, mat="iron_spec")

    # --- 底部收尖与后背支撑 ---
    rig.cube(bone, "nodule_lowest_point",
             (-1.2, 0.0, -1.2), (1.0, 1.0, 1.0),
             rot=rot, org=org, mat="iron_void")
    rig.cube(bone, "back_reinforce_spine",
             (-1.6, 2.6, -3.2), (1.8, 8.8, -1.8),
             rot=rot, org=org, mat="iron_void")


def part_golden_chasm_veins(rig: Rig):
    """构建贯穿左右两瓣之间裂谷深处的金色地火裂缝网。"""
    bone = "golden_chasm_veins"
    rig.bone(bone, (0.0, 5.0, 0.0))

    org = (0.0, 5.0, 0.0)
    rot = (6.0, 14.0, -8.0)

    # 1. 裂谷底部的暗金火槽（深陷在 z=0.8~1.6 内凹深缝中）
    rig.cube(bone, "chasm_bed_lower",
             (-0.9, 1.6, 0.4), (0.9, 4.6, 1.5),
             rot=rot, org=org, mat="gold_deep")
    rig.cube(bone, "chasm_bed_mid",
             (-1.1, 4.4, 0.6), (1.1, 8.4, 1.7),
             rot=rot, org=org, mat="gold_deep")
    rig.cube(bone, "chasm_bed_upper",
             (-0.6, 8.2, 0.2), (1.2, 11.6, 1.3),
             rot=rot, org=org, mat="gold_deep")

    # 2. 纵贯对角线的曲折金火主脉（灿金地气火光）
    rig.cube(bone, "gold_line_0",
             (-0.35, 1.8, 1.45), (0.35, 3.4, 1.85),
             rot=rot, org=org, mat="gold_mid")
    rig.cube(bone, "gold_line_1",
             (-0.5, 3.2, 1.75), (0.2, 4.8, 2.15),
             rot=rot, org=org, mat="gold_mid")
    # 核心交汇火核（极亮灿金芒）
    rig.cube(bone, "gold_nexus_bright",
             (-0.3, 4.6, 2.1), (0.4, 6.2, 2.5),
             rot=rot, org=org, mat="gold_high")
    rig.cube(bone, "gold_nexus_flare",
             (-0.15, 5.0, 2.3), (0.25, 5.8, 2.62),
             rot=rot, org=org, mat="gold_high")
    # 上段冲向主峰的折裂火纹
    rig.cube(bone, "gold_line_2",
             (0.1, 6.0, 1.85), (0.75, 7.8, 2.25),
             rot=rot, org=org, mat="gold_mid")
    rig.cube(bone, "gold_line_3",
             (0.4, 7.6, 1.45), (1.1, 9.8, 1.85),
             rot=rot, org=org, mat="gold_high")
    rig.cube(bone, "gold_line_apex",
             (0.7, 9.6, 0.95), (1.3, 11.8, 1.35),
             rot=rot, org=org, mat="gold_high")

    # 3. 左右发散的金火分支微裂
    # 向左腹撕开的斜火纹
    rig.cube(bone, "gold_spur_left_low",
             (-2.2, 3.8, 1.6), (-0.4, 4.5, 2.05),
             rot=rot, org=org, mat="gold_mid")
    rig.cube(bone, "gold_spur_left_high",
             (-2.6, 5.8, 1.1), (-0.8, 6.5, 1.75),
             rot=rot, org=org, mat="gold_high")
    rig.cube(bone, "gold_spur_left_tip",
             (-3.4, 6.2, 0.6), (-2.4, 6.8, 1.2),
             rot=rot, org=org, mat="gold_deep")

    # 向右腹分叉的金色细火
    rig.cube(bone, "gold_spur_right_low",
             (0.3, 3.2, 1.6), (1.8, 3.9, 2.15),
             rot=rot, org=org, mat="gold_deep")
    rig.cube(bone, "gold_spur_right_high",
             (0.6, 6.6, 1.8), (2.4, 7.4, 2.35),
             rot=rot, org=org, mat="gold_mid")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_stepped_iron_lobes(rig)
    part_golden_chasm_veins(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="凡铁矿阶梯立方断层原矿生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "FanTie.bbmodel", help="输出路径")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = build_rig()
    bb_json = rig.bbmodel("FanTie")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
