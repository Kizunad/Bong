#!/usr/bin/env python3
"""玉髓（YuSui）矿石与冷翠玉核 Blockbench .bbmodel 生成器。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/yu_sui.png`
  `server/assets/items/minerals.toml` (id = "yu_sui", name = "玉髓")
  配套方块贴图：`client/src/main/resources/assets/bong/textures/block/ore_yu_sui.png`

视觉特征与解剖结构：
  1. 深渊寒页岩外壳基座（part_shale_crust）：
     - 墨黑深邃、层状片理断口的深渊寒岩，如同天然地质壳层紧密包覆。
     - 包含：底层寒石岩台、左右两侧高耸页岩护壁、后方沉稳岩背。
  2. 十字破裂中心温润玉髓核（part_jade_core）：
     - 核心深陷在黑岩裂缝正中，呈现青白温润、冷灵内敛的玉质光泽。
     - 包含中心深层玉核（JADE_DEEP）、饱满青翠玉身（JADE_BODY）与冰白向阳极亮冷高光（JADE_HIGH）。
  3. 四方裂隙延展玉脉（part_jade_cross_arms）：
     - 仿照原画中极具辨识度的十字放射状裂隙，玉脉自核心向左上、右上、前下方斜向劈开黑岩。
  4. 缝隙冷灵光暗流（part_glow_crevices）：
     - 寒岩与玉石交界面透出的幽翠冷芒（GLOW_SEAM）。

用法：
  python3 modelScript/generators/gen_yu_sui.py
  bbmodel-render modelScript/models/YuSui.bbmodel
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
from palette_yusui import (
    SHALE_DARK, SHALE_BASE, SHALE_MID,
    JADE_DEEP, JADE_BODY, JADE_LIT, JADE_HIGH,
    GLOW_SEAM
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

MATS = {
    "shale_dark": SHALE_DARK,
    "shale_base": SHALE_BASE,
    "shale_mid":  SHALE_MID,
    "jade_deep":  JADE_DEEP,
    "jade_body":  JADE_BODY,
    "jade_lit":   JADE_LIT,
    "jade_high":  JADE_HIGH,
    "glow_seam":  GLOW_SEAM,
}


def part_shale_crust(rig: Rig):
    """构建深渊墨黑寒页岩外壳基座。"""
    bone = "shale_crust"
    rig.bone(bone, (0.0, 0.0, 0.0))

    # 底层寒岩床
    rig.cube(bone, "shale_bed_main",
             (-5.6, 0.0, -5.2), (5.2, 1.6, 4.8),
             mat="shale_base")
    rig.cube(bone, "shale_bed_fl",
             (-6.2, 0.0, 0.8), (-2.6, 2.4, 5.6),
             mat="shale_dark")
    rig.cube(bone, "shale_bed_br",
             (1.4, 0.0, -5.8), (6.0, 2.0, -1.2),
             mat="shale_mid")

    # 左侧页岩壁（层叠页岩片理）
    rig.cube(bone, "shale_wall_l_low",
             (-5.8, 1.6, -3.2), (-2.8, 5.2, 2.4),
             mat="shale_dark")
    rig.cube(bone, "shale_wall_l_mid",
             (-5.4, 5.2, -2.0), (-3.2, 8.2, 1.4),
             mat="shale_mid")
    rig.cube(bone, "shale_wall_l_top",
             (-4.8, 8.2, -1.0), (-3.6, 9.8, 0.8),
             mat="shale_dark")

    # 右侧厚重寒岩台（降低高度并向后收缩，不遮挡右视角的玉核与右玉脉）
    rig.cube(bone, "shale_wall_r_low",
             (2.8, 1.4, -2.4), (5.6, 3.8, 2.8),
             mat="shale_dark")
    rig.cube(bone, "shale_wall_r_top",
             (3.4, 3.8, -1.8), (4.8, 5.8, 1.6),
             mat="shale_mid")

    # 后背深沉岩脊
    rig.cube(bone, "shale_back_ridge",
             (-2.4, 1.4, -5.4), (3.6, 6.8, -2.6),
             mat="shale_dark")
    rig.cube(bone, "shale_back_peak",
             (-1.2, 6.8, -5.0), (2.4, 9.2, -3.2),
             mat="shale_base")


def part_jade_core(rig: Rig):
    """构建中央十字交错处的温润冷翠玉髓核心。"""
    bone = "jade_core"
    rig.bone(bone, (0.0, 1.4, 0.0))

    # 核心深层暗玉
    rig.cube(bone, "jade_core_deep",
             (-2.4, 1.8, -2.0), (2.2, 4.8, 2.0),
             mat="jade_deep")

    # 中心隆起受光玉身
    rig.cube(bone, "jade_core_bulk",
             (-1.8, 4.4, -1.4), (1.6, 7.8, 1.6),
             mat="jade_body")

    # 核心向阳极亮冰白高光晶台
    rig.cube(bone, "jade_core_lit",
             (-1.2, 6.8, -0.8), (1.0, 9.4, 1.2),
             mat="jade_lit")
    rig.cube(bone, "jade_core_high",
             (-0.6, 9.2, -0.4), (0.6, 10.6, 0.8),
             mat="jade_high")


def part_jade_cross_arms(rig: Rig):
    """构建从核心向四方放射劈裂的温润玉脉晶棱。"""
    bone = "jade_cross_arms"
    rig.bone(bone, (0.0, 1.2, 0.0))

    # 1. 前下方倾斜突出玉柱（直指前景，正前方透光）
    rig.cube(bone, "arm_front_base",
             (-1.4, 1.6, 1.6), (1.2, 3.8, 3.6),
             mat="jade_body")
    rig.cube(bone, "arm_front_tip",
             (-0.8, 3.2, 2.4), (0.8, 5.2, 4.4),
             mat="jade_lit")
    rig.cube(bone, "arm_front_high",
             (-0.4, 4.8, 3.2), (0.4, 5.8, 4.2),
             mat="jade_high")

    # 2. 右上方斜插裂隙晶棱（向右上劈出黑岩，体积更饱满）
    rig.cube(bone, "arm_right_branch",
             (1.2, 3.8, -1.2), (3.6, 7.6, 1.6),
             mat="jade_body")
    rig.cube(bone, "arm_right_edge",
             (1.8, 6.8, -0.6), (3.4, 8.8, 1.2),
             mat="jade_lit")
    rig.cube(bone, "arm_right_facet",
             (2.2, 8.2, -0.2), (3.2, 9.6, 0.8),
             mat="jade_high")

    # 3. 左上方伴生微细玉脉（嵌入左侧页岩壁缝）
    rig.cube(bone, "arm_left_crevice",
             (-3.2, 3.6, -0.6), (-1.4, 6.4, 1.2),
             mat="jade_deep")
    rig.cube(bone, "arm_left_lip",
             (-2.8, 5.6, -0.2), (-1.6, 7.6, 0.8),
             mat="jade_lit")


def part_glow_crevices(rig: Rig):
    """构建寒岩与玉核接缝处的冷翠微光暗纹。"""
    bone = "glow_crevices"
    rig.bone(bone, (0.0, 1.2, 0.0))

    # 前方主开裂缝隙处的冷灵光（清晰露出正面）
    rig.cube(bone, "glow_front_seam",
             (-1.6, 2.0, 2.2), (1.4, 2.45, 3.4),
             mat="glow_seam")
    rig.cube(bone, "glow_front_diagonal",
             (0.4, 3.6, 1.8), (1.6, 5.4, 2.5),
             mat="glow_seam")

    # 右侧开裂石槽微光
    rig.cube(bone, "glow_right_trough",
             (1.8, 2.8, 0.2), (2.6, 4.6, 1.6),
             mat="glow_seam")

    # 左壁缝隙冷光
    rig.cube(bone, "glow_left_slit",
             (-3.0, 2.4, -0.4), (-2.2, 4.4, 1.0),
             mat="glow_seam")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_shale_crust(rig)
    part_jade_core(rig)
    part_jade_cross_arms(rig)
    part_glow_crevices(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="玉髓 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "YuSui.bbmodel", help="输出路径")
    parser.add_argument("--part", choices=["shale", "core", "arms", "glow"],
                        help="仅单独导出指定部件预览")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = Rig(MATS, swatch=8)

    if args.part == "shale":
        part_shale_crust(rig)
    elif args.part == "core":
        part_jade_core(rig)
    elif args.part == "arms":
        part_jade_cross_arms(rig)
    elif args.part == "glow":
        part_glow_crevices(rig)
    else:
        part_shale_crust(rig)
        part_jade_core(rig)
        part_jade_cross_arms(rig)
        part_glow_crevices(rig)

    bb_json = rig.bbmodel("YuSui")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
