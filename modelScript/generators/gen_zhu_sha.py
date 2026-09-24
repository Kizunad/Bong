#!/usr/bin/env python3
"""朱砂（ZhuSha）矿石与血髓原石 Blockbench .bbmodel 生成器。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/zhu_sha.png`
  `server/assets/items/minerals.toml` (id = "zhu_sha", name = "朱砂")
  配套方块贴图：`client/src/main/resources/assets/bong/textures/block/ore_zhu_sha.png`

视觉特征与解剖结构：
  1. 火山熔岩玄武黑壳（part_volcanic_crust）：
     - 零散包裹在多面体原矿背侧与底部的深黑黑褐碎岩残皮。
     - 质地粗糙，与炽红血髓晶体形成极其强烈的明暗对比。
  2. 多面体多阶凝固血髓主石（part_blood_marrow_core）：
     - 区别于丹砂的六棱挺拔晶柱，朱砂是一整块巨型多棱角、阶梯折面切割的血髓原矿！
     - 包含主晶核、向阳斜向切面、晶面粉白冷高光（BLOOD_HIGH）与浓郁血髓艳红（BLOOD_BODY）。
  3. 伴生错落多棱角小原石（part_cluster_facets）：
     - 环绕主原矿两侧与前侧的参差错落小切面岩块，使整块原矿具有地质碎裂自然感。
  4. 熔岩火脉裂隙（part_magma_seams）：
     - 晶石贯穿断口中露出的明亮橙红火痕（MAGMA_GLOW）。

用法：
  python3 modelScript/generators/gen_zhu_sha.py
  bbmodel-render modelScript/models/ZhuSha.bbmodel
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
from palette_zhusha import (
    CRUST_DARK, CRUST_BASE, CRUST_MID,
    BLOOD_DEEP, BLOOD_BODY, BLOOD_LIT, BLOOD_HIGH,
    MAGMA_GLOW
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

MATS = {
    "crust_dark": CRUST_DARK,
    "crust_base": CRUST_BASE,
    "crust_mid":  CRUST_MID,
    "blood_deep": BLOOD_DEEP,
    "blood_body": BLOOD_BODY,
    "blood_lit":  BLOOD_LIT,
    "blood_high": BLOOD_HIGH,
    "magma_glow": MAGMA_GLOW,
}


def part_volcanic_crust(rig: Rig):
    """构建残存的火山玄武岩外壳基座。"""
    bone = "volcanic_crust"
    rig.bone(bone, (0.0, 0.0, 0.0))

    # 底层黑岩垫基
    rig.cube(bone, "crust_floor",
             (-5.6, 0.0, -5.2), (5.2, 1.4, 4.8),
             mat="crust_base")
    # 左侧厚玄武黑岩壁
    rig.cube(bone, "crust_wall_left",
             (-6.2, 0.8, -3.2), (-3.4, 5.2, 2.6),
             mat="crust_dark")
    rig.cube(bone, "crust_wall_l_shelf",
             (-5.4, 5.2, -1.8), (-3.8, 7.8, 1.4),
             mat="crust_mid")

    # 后背黑岩脊背
    rig.cube(bone, "crust_ridge_back",
             (-2.8, 1.2, -5.8), (4.4, 6.4, -2.8),
             mat="crust_dark")
    rig.cube(bone, "crust_ridge_b_top",
             (-1.4, 6.4, -5.2), (2.8, 8.6, -3.4),
             mat="crust_base")

    # 右前散落风化熔岩块
    rig.cube(bone, "crust_fr_chunk",
             (2.6, 0.0, 2.2), (5.8, 2.4, 5.4),
             mat="crust_mid")


def part_blood_marrow_core(rig: Rig):
    """构建整块多棱多面体凝固血髓主石。"""
    bone = "blood_core"
    rig.bone(bone, (0.0, 1.4, 0.0))

    # 1. 核心大石体（下宽上窄，带四角大斜切面逼近多面体）
    # 底层宽台
    rig.cube(bone, "core_tier1_main",
             (-3.8, 1.2, -3.4), (3.6, 4.2, 3.2),
             mat="blood_deep")
    rig.cube(bone, "core_tier1_flank",
             (-4.4, 1.6, -2.4), (4.2, 3.8, 2.2),
             mat="blood_body")

    # 中段隆起层（血髓艳红受光部）
    rig.cube(bone, "core_tier2_bulk",
             (-3.2, 4.2, -2.8), (2.8, 7.6, 2.6),
             mat="blood_body")
    rig.cube(bone, "core_tier2_front_shelf",
             (-2.6, 3.8, 1.8), (2.2, 6.4, 3.4),
             mat="blood_lit")
    rig.cube(bone, "core_tier2_left_facet",
             (-3.8, 4.6, -1.6), (-2.8, 7.2, 2.0),
             mat="blood_body")

    # 上部斜切尖峰（向阳切面）
    rig.cube(bone, "core_tier3_apex",
             (-2.2, 7.6, -2.2), (1.8, 10.4, 1.6),
             mat="blood_lit")
    rig.cube(bone, "core_tier3_front_slope",
             (-1.6, 6.8, 1.0), (1.2, 9.6, 2.2),
             mat="blood_lit")

    # 顶峰棱角截面与冷光晶面
    rig.cube(bone, "core_peak_facet",
             (-1.2, 10.4, -1.4), (0.8, 11.8, 0.8),
             mat="blood_high")
    rig.cube(bone, "core_peak_high",
             (-0.6, 11.8, -0.8), (0.4, 12.4, 0.2),
             mat="blood_high")


def part_cluster_facets(rig: Rig):
    """构建伴生错落的小多面体晶石。"""
    bone = "cluster_facets"
    rig.bone(bone, (0.0, 1.0, 0.0))

    # 右侧伴生血髓块（凸向右侧）
    rig.cube(bone, "facet_right_base",
             (2.4, 1.6, -1.4), (4.8, 4.4, 2.2),
             mat="blood_body")
    rig.cube(bone, "facet_right_top",
             (2.8, 4.4, -0.8), (4.2, 6.8, 1.6),
             mat="blood_lit")
    rig.cube(bone, "facet_right_high",
             (3.0, 6.8, -0.4), (3.8, 7.4, 1.0),
             mat="blood_high")

    # 前左低矮突出晶角
    rig.cube(bone, "facet_fl_low",
             (-2.8, 1.2, 2.4), (-0.6, 3.6, 4.4),
             mat="blood_body")
    rig.cube(bone, "facet_fl_tip",
             (-2.2, 3.6, 2.8), (-1.2, 4.8, 4.0),
             mat="blood_lit")

    # 后方次生斜柱
    rig.cube(bone, "facet_back_col",
             (0.8, 2.4, -4.2), (3.2, 7.2, -2.2),
             mat="blood_deep")
    rig.cube(bone, "facet_back_tip",
             (1.2, 7.2, -3.8), (2.6, 8.4, -2.6),
             mat="blood_lit")


def part_magma_seams(rig: Rig):
    """构建晶石断口与岩壳间露出的炽热火痕。"""
    bone = "magma_seams"
    rig.bone(bone, (0.0, 1.2, 0.0))

    # 正中主石前壁裂开的横贯火脉（向外凸出使正面清晰可见）
    rig.cube(bone, "magma_seam_front",
             (-1.8, 3.8, 2.65), (1.4, 4.4, 3.48),
             mat="magma_glow")
    rig.cube(bone, "magma_seam_diagonal",
             (0.2, 4.4, 2.05), (1.6, 6.6, 2.75),
             mat="magma_glow")
    rig.cube(bone, "magma_seam_lower",
             (-2.4, 2.2, 2.85), (-0.8, 3.2, 3.85),
             mat="magma_glow")

    # 右侧矿缝中的熔岩火光
    rig.cube(bone, "magma_seam_right",
             (2.2, 2.2, 0.4), (2.85, 4.8, 2.0),
             mat="magma_glow")

    # 左侧黑岩接壤处火脉
    rig.cube(bone, "magma_seam_left",
             (-3.45, 2.6, -0.6), (-2.65, 4.4, 1.6),
             mat="magma_glow")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_volcanic_crust(rig)
    part_blood_marrow_core(rig)
    part_cluster_facets(rig)
    part_magma_seams(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="朱砂 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "ZhuSha.bbmodel", help="输出路径")
    parser.add_argument("--part", choices=["crust", "core", "facets", "seams"],
                        help="仅单独导出指定部件预览")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = Rig(MATS, swatch=8)

    if args.part == "crust":
        part_volcanic_crust(rig)
    elif args.part == "core":
        part_blood_marrow_core(rig)
    elif args.part == "facets":
        part_cluster_facets(rig)
    elif args.part == "seams":
        part_magma_seams(rig)
    else:
        part_volcanic_crust(rig)
        part_blood_marrow_core(rig)
        part_cluster_facets(rig)
        part_magma_seams(rig)

    bb_json = rig.bbmodel("ZhuSha")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
