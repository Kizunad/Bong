#!/usr/bin/env python3
"""灵铁（LingTie）—— 水滴梭形冷铁原矿（多面体裂隙与贯穿冰裂储灵网）Blockbench .bbmodel 生成器。

【世界观与原画深度还原】：
- 物品来源：`ling_tie.png`（高 108px，宽 79px 的斜向水滴梭形冷矿）
- 核心形态突破：
  1. 彻底打破圆柱堆叠与方形贴片！
  2. 原矿由左右两扇巨型冷铁断块在中心咬合构成，形成一条贯穿底顶的天然 V 形深凹破裂峡谷。
  3. 峡谷与断裂缝隙深处，喷薄出如蛛网/闪电般曲折盘旋的青碧储灵冷光网（QI_HIGH 极亮核心 + QI_BODY 鲜明青碧）。
  4. 矿体下部为斜向收尖的圆锥底，上部为向右上翘起的冷铁尖峰，侧翼布满贝壳状金属切面。
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.rig.rigkit import Rig
from bbmodel_maker import workspace
from palette_lingtie import (
    IRON_VOID, IRON_DARK, IRON_MID, IRON_LIT, IRON_SPEC,
    QI_DEEP, QI_BODY, QI_HIGH
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
    "qi_deep":   QI_DEEP,
    "qi_body":   QI_BODY,
    "qi_high":   QI_HIGH,
}


def part_iron_lobes(rig: Rig):
    """构建构成水滴梭形外轮廓的左右两扇咬合冷铁主块体。"""
    bone = "iron_lobes"
    rig.bone(bone, (0.0, 5.0, 0.0))

    org = (0.0, 5.0, 0.0)
    rot = (6.0, 12.0, -8.0) # 整体自然斜立，呼应原画姿态

    # --- 左侧主铁瓣（较厚重，形成左翼与左上肩峰） ---
    # 左底（向底尖收缩）
    rig.cube(bone, "lobe_left_bottom",
             (-2.6, 0.8, -1.8), (-0.4, 3.2, 1.8),
             rot=rot, org=org, mat="iron_void")
    # 左腹（向外膨出，带有贝壳棱角）
    rig.cube(bone, "lobe_left_belly_main",
             (-3.8, 3.2, -2.4), (-0.6, 7.4, 2.2),
             rot=rot, org=org, mat="iron_dark")
    rig.cube(bone, "lobe_left_belly_flange",
             (-4.4, 3.8, -1.4), (-3.2, 6.8, 1.6),
             rot=rot, org=org, mat="iron_mid")
    # 左上肩峰（向上收束）
    rig.cube(bone, "lobe_left_shoulder",
             (-3.2, 7.4, -1.6), (-0.8, 10.2, 1.4),
             rot=rot, org=org, mat="iron_mid")
    rig.cube(bone, "lobe_left_peak",
             (-2.4, 10.2, -1.0), (-1.0, 11.6, 0.8),
             rot=rot, org=org, mat="iron_lit")

    # --- 右侧主铁瓣（向右上扬起，形成最高主峰） ---
    # 右底
    rig.cube(bone, "lobe_right_bottom",
             (0.4, 1.2, -1.6), (2.4, 3.4, 1.6),
             rot=rot, org=org, mat="iron_void")
    # 右腹（正面受光漫射面）
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
    # 右上主峰（高耸扬起，最高达 y=13.0）
    rig.cube(bone, "lobe_right_apex_tower",
             (0.2, 7.8, -1.4), (2.6, 11.2, 1.4),
             rot=rot, org=org, mat="iron_lit")
    rig.cube(bone, "lobe_right_apex_tip",
             (0.6, 11.2, -0.7), (1.8, 12.8, 0.7),
             rot=rot, org=org, mat="iron_spec")

    # --- 底极点与背部支撑脊 ---
    # 底部最下方水滴收尖点（y=0.0 ~ 0.8）
    rig.cube(bone, "nodule_lowest_point",
             (-1.2, 0.0, -1.2), (1.0, 1.0, 1.0),
             rot=rot, org=org, mat="iron_void")
    # 后背深邃浑厚脊背
    rig.cube(bone, "back_reinforce_spine",
             (-1.6, 2.6, -3.2), (1.8, 8.8, -1.8),
             rot=rot, org=org, mat="iron_void")


def part_teal_chasm_veins(rig: Rig):
    """构建嵌入左右铁瓣之间裂谷深处、如闪电裂变般的青碧储灵冷光网。"""
    bone = "teal_chasm_veins"
    rig.bone(bone, (0.0, 5.0, 0.0))

    org = (0.0, 5.0, 0.0)
    rot = (6.0, 12.0, -8.0)

    # 1. 裂谷底部的暗青储灵凹槽（深陷在 z=0.8~1.6 之间，真实下陷形成内凹深缝！）
    rig.cube(bone, "chasm_bed_lower",
             (-0.9, 1.6, 0.4), (0.9, 4.6, 1.5),
             rot=rot, org=org, mat="qi_deep")
    rig.cube(bone, "chasm_bed_mid",
             (-1.1, 4.4, 0.6), (1.1, 8.4, 1.7),
             rot=rot, org=org, mat="qi_deep")
    rig.cube(bone, "chasm_bed_upper",
             (-0.6, 8.2, 0.2), (1.2, 11.6, 1.3),
             rot=rot, org=org, mat="qi_deep")

    # 2. 纵贯对角线的闪电主脉（由曲折错落的细窄晶核拼合，闪耀青碧冷辉）
    # 下段斜上折线
    rig.cube(bone, "vein_line_0",
             (-0.35, 1.8, 1.45), (0.35, 3.4, 1.85),
             rot=rot, org=org, mat="qi_body")
    rig.cube(bone, "vein_line_1",
             (-0.5, 3.2, 1.75), (0.2, 4.8, 2.15),
             rot=rot, org=org, mat="qi_body")
    # 中段核心交汇闪电结（极亮纯白青冷光）
    rig.cube(bone, "vein_nexus_bright",
             (-0.3, 4.6, 2.1), (0.4, 6.2, 2.5),
             rot=rot, org=org, mat="qi_high")
    rig.cube(bone, "vein_nexus_flare",
             (-0.15, 5.0, 2.3), (0.25, 5.8, 2.62),
             rot=rot, org=org, mat="qi_high")
    # 上段冲向主峰的折裂脉
    rig.cube(bone, "vein_line_2",
             (0.1, 6.0, 1.85), (0.75, 7.8, 2.25),
             rot=rot, org=org, mat="qi_body")
    rig.cube(bone, "vein_line_3",
             (0.4, 7.6, 1.45), (1.1, 9.8, 1.85),
             rot=rot, org=org, mat="qi_high")
    rig.cube(bone, "vein_line_apex",
             (0.7, 9.6, 0.95), (1.3, 11.8, 1.35),
             rot=rot, org=org, mat="qi_high")

    # 3. 左右发散的毛细储灵枝脉（蛛网微裂纹）
    # 向左腹撕开的横向裂痕
    rig.cube(bone, "vein_spur_left_low",
             (-2.2, 3.8, 1.6), (-0.4, 4.5, 2.05),
             rot=rot, org=org, mat="qi_body")
    rig.cube(bone, "vein_spur_left_high",
             (-2.6, 5.8, 1.1), (-0.8, 6.5, 1.75),
             rot=rot, org=org, mat="qi_high")
    rig.cube(bone, "vein_spur_left_tip",
             (-3.4, 6.2, 0.6), (-2.4, 6.8, 1.2),
             rot=rot, org=org, mat="qi_deep")

    # 向右腹与前壁分叉的微脉
    rig.cube(bone, "vein_spur_right_low",
             (0.3, 3.2, 1.6), (1.8, 3.9, 2.15),
             rot=rot, org=org, mat="qi_deep")
    rig.cube(bone, "vein_spur_right_high",
             (0.6, 6.6, 1.8), (2.4, 7.4, 2.35),
             rot=rot, org=org, mat="qi_body")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_iron_lobes(rig)
    part_teal_chasm_veins(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="灵铁水滴原矿 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "LingTie.bbmodel", help="输出路径")
    parser.add_argument("--part", choices=["lobes", "veins"],
                        help="仅单独导出指定部件预览")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = Rig(MATS, swatch=8)

    if args.part == "lobes":
        part_iron_lobes(rig)
    elif args.part == "veins":
        part_teal_chasm_veins(rig)
    else:
        part_iron_lobes(rig)
        part_teal_chasm_veins(rig)

    bb_json = rig.bbmodel("LingTie")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
