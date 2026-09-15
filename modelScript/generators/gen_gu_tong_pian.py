#!/usr/bin/env python3
"""古铜片（GuTongPian）—— 上古礼器饕餮衔环残片（纯材料/残件级）Blockbench .bbmodel 生成器。

【世界观与原画深度契合】：
- 物品来源：`gu_tong_pian.png`
  “坍缩渊深处上古礼器碎块，饕餮兽纹犹存，灵力已散。散修借壳不借力，二次激活。”
- 形态语言（彻底抛弃石头底座插金属！）：
  这是一块真实的、从上古重器上崩裂剥落的青铜礼器残片！
  1. plate_body: 带有微弧度与不规则崩裂断茬的青铜残片主体（上厚下薄微弧甲片，顶端高低参差崩裂）。
  2. taotie_relief: 正面立体高浮雕饕餮兽面（卷翘雷纹角、凸起巨目、宽隆兽鼻、阔口下颚）。
  3. loose_ring: 兽口衔着的下垂倾斜青铜重环（闭环圆角结构）。
  4. patina_accents: 阴刻凹陷与断裂口处沉积的古青绿薄锈与暗铜磨损高光。

用法：
  python3 modelScript/generators/gen_gu_tong_pian.py
  bbmodel-render modelScript/models/GuTongPian.bbmodel
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
from palette_gutongpian import (
    BRONZE_VOID, BRONZE_DARK, BRONZE_MID, BRONZE_LIT, BRONZE_HIGH,
    FRACTURE_GRAIN, PATINA_MOSS
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

MATS = {
    "bronze_void":     BRONZE_VOID,
    "bronze_dark":     BRONZE_DARK,
    "bronze_mid":      BRONZE_MID,
    "bronze_lit":      BRONZE_LIT,
    "bronze_high":     BRONZE_HIGH,
    "fracture_grain":  FRACTURE_GRAIN,
    "patina_moss":     PATINA_MOSS,
}


def part_plate_body(rig: Rig):
    """构建微带弧度且顶端不规则崩裂的青铜残片基板。"""
    bone = "plate_body"
    rig.bone(bone, (0.0, 1.0, 0.0))

    # 稍微向后仰 8 度，呈现斜倚摆放的金属厚片姿态
    org = (0.0, 1.0, 0.0)
    rot = (-8.0, 0.0, 0.0)

    # 1. 残片下端收尖（盾形收底，y=0.4 ~ 3.4）
    rig.cube(bone, "plate_bottom_tip",
             (-1.6, 0.4, -0.45), (1.6, 2.0, 0.45),
             rot=rot, org=org, mat="bronze_mid")
    rig.cube(bone, "plate_bottom_flange",
             (-2.6, 2.0, -0.5), (2.6, 3.6, 0.5),
             rot=rot, org=org, mat="bronze_mid")

    # 2. 残片中段宽阔腹板（饕餮浮雕承载面，y=3.6 ~ 8.6，宽 6.4 格）
    rig.cube(bone, "plate_mid_center",
             (-3.2, 3.6, -0.55), (3.2, 8.6, 0.55),
             rot=rot, org=org, mat="bronze_dark")
    # 两侧微向前包卷的古铜器边框（厚棱）
    rig.cube(bone, "plate_edge_left",
             (-3.5, 3.2, -0.4), (-3.0, 9.0, 0.75),
             rot=rot, org=org, mat="bronze_lit")
    rig.cube(bone, "plate_edge_right",
             (3.0, 3.2, -0.4), (3.5, 9.0, 0.75),
             rot=rot, org=org, mat="bronze_lit")

    # 3. 上端断裂金属茬口（不规则崩裂参差阶梯，y=8.6 ~ 11.4）
    # 左侧崩角（较低断口）
    rig.cube(bone, "fracture_left_low",
             (-3.2, 8.6, -0.45), (-1.4, 9.8, 0.45),
             rot=rot, org=org, mat="fracture_grain")
    # 中间主断茬（高突断尖）
    rig.cube(bone, "fracture_mid_crest",
             (-1.4, 8.6, -0.48), (1.2, 11.4, 0.48),
             rot=rot, org=org, mat="fracture_grain")
    rig.cube(bone, "fracture_mid_tip",
             (-0.6, 11.4, -0.35), (0.6, 12.2, 0.35),
             rot=rot, org=org, mat="fracture_grain")
    # 右侧斜向崩落缺口（露金属粗糙截面）
    rig.cube(bone, "fracture_right_drop",
             (1.2, 8.6, -0.42), (2.8, 10.2, 0.42),
             rot=rot, org=org, mat="fracture_grain")
    rig.cube(bone, "fracture_far_right_low",
             (2.8, 8.6, -0.4), (3.3, 9.2, 0.4),
             rot=rot, org=org, mat="bronze_void")

    # 4. 背面古青铜氧化厚背
    rig.cube(bone, "plate_back_skin",
             (-2.8, 2.4, -0.75), (2.8, 9.2, -0.5),
             rot=rot, org=org, mat="bronze_void")


def part_taotie_relief(rig: Rig):
    """构建残片正面立体高浮雕饕餮兽面（雷纹角、兽眼、蒜头鼻、阔口）。"""
    bone = "taotie_relief"
    rig.bone(bone, (0.0, 6.0, 0.0))

    org = (0.0, 1.0, 0.0)
    rot = (-8.0, 0.0, 0.0)

    # 浮雕自基板前面 z=0.55 向外凸出到 z=1.2 ~ 1.8
    # 1. 左右双卷外撇雷纹兽角（y=7.4 ~ 9.6）
    rig.cube(bone, "taotie_horn_left",
             (-2.8, 7.6, 0.55), (-1.2, 9.4, 1.15),
             rot=rot, org=org, mat="bronze_lit")
    rig.cube(bone, "taotie_horn_right",
             (1.2, 7.6, 0.55), (2.8, 9.4, 1.15),
             rot=rot, org=org, mat="bronze_lit")

    # 2. 粗硕威严的眉脊与菱形巨目（y=6.0 ~ 7.6）
    rig.cube(bone, "taotie_brow_ridge",
             (-2.4, 6.8, 0.6), (2.4, 7.8, 1.35),
             rot=rot, org=org, mat="bronze_high")
    # 凸起双目（磨损亮金）
    rig.cube(bone, "taotie_eye_left",
             (-2.2, 6.2, 0.7), (-0.8, 7.0, 1.45),
             rot=rot, org=org, mat="bronze_high")
    rig.cube(bone, "taotie_eye_right",
             (0.8, 6.2, 0.7), (2.2, 7.0, 1.45),
             rot=rot, org=org, mat="bronze_high")
    # 目中深暗阴线（瞳孔阴槽）
    rig.cube(bone, "taotie_pupil_l",
             (-1.6, 6.4, 1.35), (-1.3, 6.8, 1.5),
             rot=rot, org=org, mat="bronze_void")
    rig.cube(bone, "taotie_pupil_r",
             (1.3, 6.4, 1.35), (1.6, 6.8, 1.5),
             rot=rot, org=org, mat="bronze_void")

    # 3. 正中凸起的蒜头大兽鼻（y=5.0 ~ 6.4）
    rig.cube(bone, "taotie_nose_bulb",
             (-0.9, 5.0, 0.7), (0.9, 6.4, 1.65),
             rot=rot, org=org, mat="bronze_high")
    rig.cube(bone, "taotie_nose_bridge",
             (-0.5, 6.2, 0.8), (0.5, 7.2, 1.5),
             rot=rot, org=org, mat="bronze_lit")

    # 4. 咧开的阔口与衔环底座（y=3.8 ~ 5.2）
    rig.cube(bone, "taotie_jaw_lip",
             (-1.8, 4.0, 0.6), (1.8, 5.0, 1.4),
             rot=rot, org=org, mat="bronze_lit")
    # 衔环中轴固定扣（鼻息下的圆扣，连接活动铜环）
    rig.cube(bone, "taotie_ring_socket",
             (-0.6, 4.2, 1.3), (0.6, 5.2, 1.85),
             rot=rot, org=org, mat="bronze_high")


def part_loose_ring(rig: Rig):
    """构建兽口中衔着的下垂微倾青铜圆环（衔环）。"""
    bone = "loose_ring"
    rig.bone(bone, (0.0, 3.6, 1.8))

    # 青铜环向前微翘垂挂（rot=(12, 0, 0)）
    org_r = (0.0, 4.5, 1.6)
    rot_r = (12.0, 0.0, 0.0)

    # 外径宽 3.4 格，高 3.4 格的青铜闭环
    # 上横梁（穿过兽口扣）
    rig.cube(bone, "ring_top",
             (-1.2, 4.4, 1.45), (1.2, 5.0, 1.95),
             rot=rot_r, org=org_r, mat="bronze_high")
    # 下横梁（下垂最低点）
    rig.cube(bone, "ring_bottom",
             (-1.1, 2.0, 1.45), (1.1, 2.6, 1.95),
             rot=rot_r, org=org_r, mat="bronze_high")
    # 左右两弧侧柱
    rig.cube(bone, "ring_left",
             (-1.6, 2.4, 1.45), (-1.0, 4.6, 1.95),
             rot=rot_r, org=org_r, mat="bronze_lit")
    rig.cube(bone, "ring_right",
             (1.0, 2.4, 1.45), (1.6, 4.6, 1.95),
             rot=rot_r, org=org_r, mat="bronze_lit")


def part_patina_scars(rig: Rig):
    """构建阴刻凹槽处渗出的古青铜绿斑与锈蚀刻痕。"""
    bone = "patina_scars"
    rig.bone(bone, (0.0, 5.0, 0.0))

    org = (0.0, 1.0, 0.0)
    rot = (-8.0, 0.0, 0.0)

    # 1. 眉心与兽角凹陷处的铜绿（青绿暗斑）
    rig.cube(bone, "patina_horn_l",
             (-2.2, 8.2, 0.6), (-1.5, 9.2, 1.0),
             rot=rot, org=org, mat="patina_moss")
    rig.cube(bone, "patina_horn_r",
             (1.5, 8.2, 0.6), (2.2, 9.2, 1.0),
             rot=rot, org=org, mat="patina_moss")

    # 2. 阔口缝隙间的积锈
    rig.cube(bone, "patina_jaw_crevice",
             (-1.6, 3.8, 0.7), (1.6, 4.1, 1.2),
             rot=rot, org=org, mat="patina_moss")

    # 3. 侧边磨损缝隙绿斑
    rig.cube(bone, "patina_edge_fl",
             (-3.1, 2.6, -0.3), (-2.7, 4.2, 0.4),
             rot=rot, org=org, mat="patina_moss")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_plate_body(rig)
    part_taotie_relief(rig)
    part_loose_ring(rig)
    part_patina_scars(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="古铜片饕餮衔环残件生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "GuTongPian.bbmodel", help="输出路径")
    parser.add_argument("--part", choices=["plate", "relief", "ring", "patina"],
                        help="仅单独导出指定部件预览")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = Rig(MATS, swatch=8)

    if args.part == "plate":
        part_plate_body(rig)
    elif args.part == "relief":
        part_taotie_relief(rig)
    elif args.part == "ring":
        part_loose_ring(rig)
    elif args.part == "patina":
        part_patina_scars(rig)
    else:
        part_plate_body(rig)
        part_taotie_relief(rig)
        part_loose_ring(rig)
        part_patina_scars(rig)

    bb_json = rig.bbmodel("GuTongPian")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
