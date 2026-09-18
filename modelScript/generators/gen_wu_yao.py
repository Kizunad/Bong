#!/usr/bin/env python3
"""乌曜石（WuYao）—— 天然聚阴黑曜石原矿（黑莲贝壳鳞甲与内生金芒）Blockbench .bbmodel 生成器。

【深度对齐物品原画 wu_yao.png】：
1. 整体构型：
   - 极其饱满的椭球形天然黑曜石矿核（宽 8.8格，高 11.2格，深 8.4格）。
   - 周围层叠舒展出数圈斜向张开的锐利贝壳断口鳞片（向外绽开的黑莲/黑曜松果形态，犬牙差互，极其自然！）。
2. 中心十字裂谷与炽金聚阴核：
   - 原画最具灵魂的特征：正中心向内自然凹陷崩裂的裂隙深谷。
   - 十字纵横的裂隙深处透出炽亮内敛的金光（GOLD_CORE / GOLD_MID / GOLD_DEEP），
     与深黑贝壳质原矿形成极度震撼的明暗对比！
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
from palette_wuyao import (
    OBS_VOID, OBS_DARK, OBS_MID, OBS_EDGE, OBS_SPEC,
    GOLD_CORE, GOLD_MID, GOLD_DEEP
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

MATS = {
    "obs_void":  OBS_VOID,
    "obs_dark":  OBS_DARK,
    "obs_mid":   OBS_MID,
    "obs_edge":  OBS_EDGE,
    "obs_spec":  OBS_SPEC,
    "gold_core": GOLD_CORE,
    "gold_mid":  GOLD_MID,
    "gold_deep": GOLD_DEEP,
}


def part_obsidian_body(rig: Rig):
    """构建多层椭圆形黑曜石原矿母体（前侧留出纵横深凹十字槽谷）。"""
    bone = "obsidian_body"
    rig.bone(bone, (0.0, 5.0, 0.0))

    # 1. 底部收紧基底
    rig.cube(bone, "body_base",
             (-3.2, 0.0, -3.0), (3.2, 1.8, 3.0),
             mat="obs_void")

    # 2. 后背深沉浑厚圆体（形成饱满后背）
    rig.cube(bone, "body_back_lower",
             (-3.6, 1.6, -3.8), (3.6, 6.4, -0.4),
             mat="obs_dark")
    rig.cube(bone, "body_back_upper",
             (-3.2, 6.4, -3.4), (3.2, 9.6, -0.6),
             mat="obs_mid")
    rig.cube(bone, "body_crest_top",
             (-2.2, 9.6, -2.8), (2.2, 11.2, -0.8),
             mat="obs_edge")

    # 3. 前身左右两侧饱满岩壁（夹出正中纵向凹裂谷，左壁与右壁）
    # 左前凸起厚壁
    rig.cube(bone, "body_front_left_lower",
             (-3.8, 1.6, -0.6), (-1.2, 6.2, 3.2),
             mat="obs_dark")
    rig.cube(bone, "body_front_left_upper",
             (-3.4, 6.2, -0.6), (-1.4, 9.4, 2.6),
             mat="obs_mid")

    # 右前凸起厚壁
    rig.cube(bone, "body_front_right_lower",
             (1.2, 1.6, -0.6), (3.8, 6.2, 3.2),
             mat="obs_dark")
    rig.cube(bone, "body_front_right_upper",
             (1.4, 6.2, -0.6), (3.4, 9.4, 2.6),
             mat="obs_mid")

    # 顶部咬合小桥（使裂隙在顶部收尖）
    rig.cube(bone, "body_top_bridge",
             (-1.6, 9.4, 0.0), (1.6, 11.0, 2.0),
             mat="obs_edge")
    rig.cube(bone, "body_apex_fin",
             (-0.8, 11.0, 0.2), (0.8, 12.0, 1.6),
             mat="obs_spec")


def part_outer_facets(rig: Rig):
    """构建原画中外围层层舒展翘起的贝壳断口锐利鳞瓣。"""
    bone = "outer_facets"
    rig.bone(bone, (0.0, 5.0, 0.0))

    # 外层向四周斜展出的 8 块贝壳鳞片（形成标志性的多角黑莲轮廓）
    # 左下外翻鳞片
    rig.cube(bone, "facet_low_l",
             (-5.2, 0.8, 0.4), (-3.4, 4.4, 3.4),
             rot=(12.0, 25.0, -18.0), org=(-4.2, 1.0, 1.8),
             mat="obs_mid")
    # 右下外翻鳞片
    rig.cube(bone, "facet_low_r",
             (3.4, 0.8, 0.4), (5.2, 4.4, 3.4),
             rot=(12.0, -25.0, 18.0), org=(4.2, 1.0, 1.8),
             mat="obs_mid")

    # 正左腰侧锋利小刀片
    rig.cube(bone, "facet_mid_left",
             (-5.6, 4.2, -1.2), (-3.8, 7.6, 1.6),
             rot=(0.0, 0.0, 20.0), org=(-4.6, 4.4, 0.2),
             mat="obs_edge")
    # 正右腰侧锋利小刀片
    rig.cube(bone, "facet_mid_right",
             (3.8, 4.2, -1.2), (5.6, 7.6, 1.6),
             rot=(0.0, 0.0, -20.0), org=(4.6, 4.4, 0.2),
             mat="obs_edge")

    # 左上外翘尖瓣
    rig.cube(bone, "facet_top_left",
             (-4.4, 7.2, -0.4), (-2.6, 10.4, 2.0),
             rot=(-10.0, 15.0, -15.0), org=(-3.4, 7.4, 0.8),
             mat="obs_spec")
    # 右上外翘尖瓣
    rig.cube(bone, "facet_top_right",
             (2.6, 7.2, -0.4), (4.4, 10.4, 2.0),
             rot=(-10.0, -15.0, 15.0), org=(3.4, 7.4, 0.8),
             mat="obs_spec")

    # 后方外展两瓣
    rig.cube(bone, "facet_back_l",
             (-4.2, 3.4, -4.6), (-2.2, 7.2, -2.6),
             rot=(-15.0, -25.0, -10.0), org=(-3.2, 3.6, -3.6),
             mat="obs_void")
    rig.cube(bone, "facet_back_r",
             (2.2, 3.4, -4.6), (4.2, 7.2, -2.6),
             rot=(-15.0, 25.0, 10.0), org=(3.2, 3.6, -3.6),
             mat="obs_void")


def part_gold_core_crevice(rig: Rig):
    """构建正面深凹十字裂隙中透出的欺天聚阴金芒（与原画 1:1 对齐！）。"""
    bone = "gold_core_crevice"
    rig.bone(bone, (0.0, 5.0, 0.0))

    # 金核深陷在 z=0.6~1.4 凹槽内（绝不浮在表面！）
    # 1. 十字裂缝底层暗金辐射槽（铺在凹槽最深处）
    rig.cube(bone, "gold_trough_vertical",
             (-1.1, 2.6, 0.4), (1.1, 8.8, 1.2),
             mat="gold_deep")
    rig.cube(bone, "gold_trough_horizontal",
             (-3.0, 4.8, 0.4), (3.0, 6.8, 1.2),
             mat="gold_deep")

    # 2. 中段金芒（向前微透，形成立体光晕阶梯）
    rig.cube(bone, "gold_glow_vert",
             (-0.65, 3.4, 0.9), (0.65, 8.0, 1.8),
             mat="gold_mid")
    rig.cube(bone, "gold_glow_horiz",
             (-2.2, 5.2, 0.9), (2.2, 6.4, 1.8),
             mat="gold_mid")

    # 3. 核心聚阴纯金瞳点（十字交汇处的极亮金核）
    rig.cube(bone, "gold_eye_center",
             (-0.45, 5.3, 1.4), (0.45, 6.3, 2.3),
             mat="gold_core")
    rig.cube(bone, "gold_eye_flare_v",
             (-0.25, 4.6, 1.3), (0.25, 7.0, 2.1),
             mat="gold_core")
    rig.cube(bone, "gold_eye_flare_h",
             (-1.2, 5.5, 1.3), (1.2, 6.1, 2.1),
             mat="gold_core")

    # 4. 裂口两侧的深黑阴影内壁（夹住金光，使深陷感更加强烈）
    rig.cube(bone, "crevice_wall_l",
             (-1.5, 3.2, 1.8), (-0.8, 8.2, 2.9),
             mat="obs_void")
    rig.cube(bone, "crevice_wall_r",
             (0.8, 3.2, 1.8), (1.5, 8.2, 2.9),
             mat="obs_void")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_obsidian_body(rig)
    part_outer_facets(rig)
    part_gold_core_crevice(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="乌曜石天然聚阴原矿生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "WuYao.bbmodel", help="输出路径")
    parser.add_argument("--part", choices=["body", "facets", "crevice"],
                        help="仅单独导出指定部件预览")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = Rig(MATS, swatch=8)

    if args.part == "body":
        part_obsidian_body(rig)
    elif args.part == "facets":
        part_outer_facets(rig)
    elif args.part == "crevice":
        part_gold_core_crevice(rig)
    else:
        part_obsidian_body(rig)
        part_outer_facets(rig)
        part_gold_core_crevice(rig)

    bb_json = rig.bbmodel("WuYao")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
