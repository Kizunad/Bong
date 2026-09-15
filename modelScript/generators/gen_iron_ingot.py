#!/usr/bin/env python3
"""粗铁锭（IronIngot）—— 横卧锻打生铁锭坯（纯工料级）Blockbench .bbmodel 生成器。

【世界观与材料第一性原理】：
- 物品来源：`iron_ingot.png`
  “铁矿粗炼得的铁坯。杂质多，将就做凡器够用。”
- 彻底摒弃立柱与底座！
  这是一块真实的、横卧放置在工坊地面的梯形粗炼生铁锭（Cast Pig Iron Ingot）！
- 结构解剖（严格呼应方块贴图 raw_iron_block.png）：
  1. ingot_body: 横向卧置的梯形生铁锭身（底宽 6.0格，顶收窄至 4.4格，长 12.0格，高 3.6格）。
     具有真实的重锤锻打倒角与冶炼拔模斜面。
  2. hammer_dimples: 顶面与侧边经重锤拍平后留下的锻打凹陷与氧化皮斑。
  3. cooling_embers: 铁锭底部与侧隙深处隐隐透出的未冷透暗红余烬火痕（与方块贴图完全同构）。

用法：
  python3 modelScript/generators/gen_iron_ingot.py
  bbmodel-render modelScript/models/IronIngot.bbmodel
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
from palette_ironingot import (
    SCALE_VOID, SCALE_DARK, SCALE_MID, SCALE_LIT, SCALE_HIGH,
    EMBER_DARK, EMBER_GLOW
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

MATS = {
    "scale_void": SCALE_VOID,
    "scale_dark": SCALE_DARK,
    "scale_mid":  SCALE_MID,
    "scale_lit":  SCALE_LIT,
    "scale_high": SCALE_HIGH,
    "ember_dark": EMBER_DARK,
    "ember_glow": EMBER_GLOW,
}


def part_ingot_body(rig: Rig):
    """构建横卧的梯形粗铁锭身（下大上小，带有拔模斜面与棱角倒角）。"""
    bone = "ingot_body"
    rig.bone(bone, (0.0, 1.8, 0.0))

    # 铁锭横跨 X 轴：长度 x = -5.8 ~ 5.8 (长 11.6格)
    # 宽度 Z 轴：底面宽 z = -2.8 ~ 2.8 (宽 5.6格)，顶面收至 z = -2.1 ~ 2.1 (宽 4.2格)
    # 高度 Y 轴：y = 0.0 ~ 3.6格

    # 1. 铁锭底层宽基（y=0.0 ~ 1.4）
    rig.cube(bone, "ingot_tier0_core",
             (-5.6, 0.0, -2.6), (5.6, 1.4, 2.6),
             mat="scale_dark")
    # 底面端头稍微内收
    rig.cube(bone, "ingot_tier0_flange",
             (-5.2, 0.0, -2.8), (5.2, 1.4, 2.8),
             mat="scale_dark")

    # 2. 铁锭中段拔模斜身（y=1.4 ~ 2.6）
    rig.cube(bone, "ingot_tier1_core",
             (-5.3, 1.4, -2.4), (5.3, 2.6, 2.4),
             mat="scale_mid")
    rig.cube(bone, "ingot_tier1_face",
             (-5.0, 1.4, 2.35), (5.0, 2.6, 2.55),
             mat="scale_lit")

    # 3. 铁锭顶面锻打台（y=2.6 ~ 3.6，向阳受光面）
    rig.cube(bone, "ingot_tier2_top",
             (-4.8, 2.6, -2.0), (4.8, 3.6, 2.0),
             mat="scale_lit")

    # 4. 顶面四周倒角受光棱线（重锤敲平后反光的金属棱角）
    rig.cube(bone, "ingot_edge_front",
             (-4.6, 3.45, 1.85), (4.6, 3.68, 2.05),
             mat="scale_high")
    rig.cube(bone, "ingot_edge_back",
             (-4.6, 3.45, -2.05), (4.6, 3.68, -1.85),
             mat="scale_mid")
    rig.cube(bone, "ingot_edge_left",
             (-4.85, 3.45, -1.9), (-4.65, 3.68, 1.9),
             mat="scale_high")
    rig.cube(bone, "ingot_edge_right",
             (4.65, 3.45, -1.9), (4.85, 3.68, 1.9),
             mat="scale_mid")

    # 5. 铁锭两端下斜端面（梯形两头）
    rig.cube(bone, "ingot_cap_left",
             (-5.8, 0.4, -2.2), (-5.4, 2.2, 2.2),
             mat="scale_void")
    rig.cube(bone, "ingot_cap_right",
             (5.4, 0.4, -2.2), (5.8, 2.2, 2.2),
             mat="scale_dark")


def part_hammer_dimples(rig: Rig):
    """构建生铁表面重锤拍打的凹痕与局部剥落的氧化皮坑。"""
    bone = "hammer_dimples"
    rig.bone(bone, (0.0, 3.0, 0.0))

    # 顶面锤击凹陷坑
    rig.cube(bone, "dimple_top_left",
             (-2.6, 3.35, -0.6), (-1.2, 3.62, 0.8),
             mat="scale_dark")
    rig.cube(bone, "dimple_top_right",
             (1.4, 3.40, -1.1), (2.8, 3.65, 0.4),
             mat="scale_dark")
    rig.cube(bone, "dimple_center_pip",
             (-0.4, 3.42, 0.2), (0.6, 3.65, 1.2),
             mat="scale_void")

    # 前壁氧化皮开裂残斑
    rig.cube(bone, "scale_patch_fl",
             (-3.8, 1.2, 2.45), (-2.2, 2.4, 2.65),
             mat="scale_void")
    rig.cube(bone, "scale_patch_fr",
             (1.8, 1.6, 2.35), (3.4, 2.6, 2.58),
             mat="scale_dark")

    # 右端面锻打粗斑（使 SIDE_R 视角充分可见）
    rig.cube(bone, "scale_patch_right",
             (5.62, 0.8, -1.4), (5.85, 2.4, 1.4),
             mat="scale_void")


def part_cooling_embers(rig: Rig):
    """构建铁锭底角与裂缝处隐隐透出的暗红余火（与方块贴图完全呼应）。"""
    bone = "cooling_embers"
    rig.bone(bone, (0.0, 0.8, 0.0))

    # 铁锭底部与地面接触缝隙中未冷透的余烬细缝（向外露出一道暗红微火）
    rig.cube(bone, "ember_seam_front_glow",
             (-3.2, 0.08, 2.70), (0.8, 0.45, 2.92),
             mat="ember_glow")
    rig.cube(bone, "ember_seam_front_dark",
             (0.8, 0.08, 2.68), (3.6, 0.45, 2.90),
             mat="ember_dark")

    # 左端铸造收缩缝隙火痕
    rig.cube(bone, "ember_crack_left",
             (-5.85, 0.3, -0.6), (-5.45, 1.2, 0.8),
             mat="ember_glow")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_ingot_body(rig)
    part_hammer_dimples(rig)
    part_cooling_embers(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="粗铁锭 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "IronIngot.bbmodel", help="输出路径")
    parser.add_argument("--part", choices=["body", "dimples", "embers"],
                        help="仅单独导出指定部件预览")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = Rig(MATS, swatch=8)

    if args.part == "body":
        part_ingot_body(rig)
    elif args.part == "dimples":
        part_hammer_dimples(rig)
    elif args.part == "embers":
        part_cooling_embers(rig)
    else:
        part_ingot_body(rig)
        part_hammer_dimples(rig)
        part_cooling_embers(rig)

    bb_json = rig.bbmodel("IronIngot")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
