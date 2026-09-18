#!/usr/bin/env python3
"""雄黄（XiongHuang）矿石与蜜蜡晶簇 Blockbench .bbmodel 生成器。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/xiong_huang.png`
  `server/assets/items/minerals.toml` (id = "xiong_huang", name = "雄黄")
  配套方块贴图：`client/src/main/resources/assets/bong/textures/block/ore_xiong_huang.png`

视觉特征与解剖结构（严格呼应方块贴图与物品原画）：
  1. 焦黑灰黑洞穴围岩基座（part_soot_rock）：
     - 暗黑焦石残块底座，粗粝破碎，与明艳的金黄晶体构成极高反差。
     - 包含：底层开阔碎岩床、左后粗糙焦黑岩壁、右侧斜切围岩台。
  2. 粗壮六棱蜜蜡金黄主晶柱（part_amber_crystal_core）：
     - 正中高耸拔起的雄黄主晶，带粗粝多段倒角体素切面，高 10.8 格。
     - 包含深琥珀橙晶根（AMBER_DEEP）、饱满蜜蜡晶身（AMBER_BODY）与晶尖向阳极亮切面（AMBER_HIGH）。
  3. 环绕簇拥的伴生次级金晶群（part_cluster_prisms）：
     - 前倾直刺的伴生小晶、右侧高耸侧晶、左侧斜出细晶棱柱。
  4. 缝隙硫磺渗晕（part_sulfur_crust）：
     - 晶根与围岩接壤缝隙处积聚的硫磺黄粉与微光暗脉（SULFUR_SEAM）。

用法：
  python3 modelScript/generators/gen_xiong_huang.py
  bbmodel-render modelScript/models/XiongHuang.bbmodel
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
from palette_xionghuang import (
    ROCK_DARK, ROCK_BASE, ROCK_MID,
    AMBER_DEEP, AMBER_BODY, AMBER_LIT, AMBER_HIGH,
    SULFUR_SEAM
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

MATS = {
    "rock_dark":   ROCK_DARK,
    "rock_base":   ROCK_BASE,
    "rock_mid":    ROCK_MID,
    "amber_deep":  AMBER_DEEP,
    "amber_body":  AMBER_BODY,
    "amber_lit":   AMBER_LIT,
    "amber_high":  AMBER_HIGH,
    "sulfur_seam": SULFUR_SEAM,
}


def add_amber_prism(rig: Rig, bone: str, prefix: str,
                    cx: float, cz: float, y0: float, y1: float,
                    r: float, chamfer: float,
                    mat_deep: str = "amber_deep", mat_body: str = "amber_body",
                    mat_lit: str = "amber_lit", mat_high: str = "amber_high"):
    """构建粗短稳固的六棱蜜蜡金黄晶柱与斜切晶尖。"""
    rx = r
    rz = r * 0.88
    c = chamfer
    
    # 底部暗根
    y_root = y0 + 1.2
    rig.cube(bone, f"{prefix}_root",
             (cx - (rx - c * 0.5), y0, cz - (rz - c * 0.5)),
             (cx + (rx - c * 0.5), y_root, cz + (rz - c * 0.5)),
             mat=mat_deep)

    # 晶身主十字段
    h_top = y1
    h_mid = y1 - 1.2
    rig.cube(bone, f"{prefix}_body_core",
             (cx - (rx - c), y_root, cz - rz),
             (cx + (rx - c), h_mid, cz + rz),
             mat=mat_body)
    rig.cube(bone, f"{prefix}_body_cross",
             (cx - rx, y_root, cz - (rz - c)),
             (cx + rx, h_mid, cz + (rz - c)),
             mat=mat_lit)

    # 晶顶斜切锥尖
    rig.cube(bone, f"{prefix}_top_shelf",
             (cx - (rx - c * 1.4), h_mid, cz - (rz - c * 1.4)),
             (cx + (rx - c * 1.4), h_top - 0.5, cz + (rz - c * 1.4)),
             mat=mat_lit)
    rig.cube(bone, f"{prefix}_tip_facet",
             (cx - (rx * 0.45), h_top - 0.5, cz - (rz * 0.45)),
             (cx + (rx * 0.45), h_top, cz + (rz * 0.45)),
             mat=mat_high)


def part_soot_rock(rig: Rig):
    """构建洞穴深层焦黑围岩基座。"""
    bone = "soot_rock"
    rig.bone(bone, (0.0, 0.0, 0.0))

    # 底层焦石床
    rig.cube(bone, "rock_bed_main",
             (-5.6, 0.0, -5.2), (5.4, 1.6, 5.0),
             mat="rock_base")
    rig.cube(bone, "rock_bed_fl",
             (-6.2, 0.0, 0.8), (-2.6, 2.4, 5.6),
             mat="rock_dark")
    rig.cube(bone, "rock_bed_br",
             (1.2, 0.0, -5.8), (6.0, 2.0, -1.0),
             mat="rock_mid")

    # 左侧焦黑围岩壁
    rig.cube(bone, "rock_wall_l_low",
             (-5.8, 1.6, -3.4), (-2.8, 4.6, 2.8),
             mat="rock_dark")
    rig.cube(bone, "rock_wall_l_top",
             (-5.2, 4.6, -2.2), (-3.4, 7.2, 1.4),
             mat="rock_mid")

    # 后方沉实焦岩脊
    rig.cube(bone, "rock_back_bulk",
             (-2.6, 1.4, -5.4), (3.8, 5.6, -2.4),
             mat="rock_dark")
    rig.cube(bone, "rock_back_ridge",
             (-1.0, 5.6, -4.8), (2.6, 7.6, -2.8),
             mat="rock_base")

    # 前右低矮裂阶
    rig.cube(bone, "rock_fr_step",
             (2.4, 1.2, 1.4), (5.4, 3.4, 4.6),
             mat="rock_mid")


def part_amber_crystal_core(rig: Rig):
    """构建中央粗壮六棱蜜蜡主晶柱。"""
    bone = "amber_core"
    rig.bone(bone, (0.0, 1.4, 0.0))

    # 主晶：傲然挺拔，位于原点略偏后，粗达 2.6px
    add_amber_prism(
        rig, bone, "amber_main",
        cx=-0.2, cz=-0.2, y0=1.4, y1=11.2,
        r=2.6, chamfer=0.85,
        mat_deep="amber_deep", mat_body="amber_body",
        mat_lit="amber_lit", mat_high="amber_high"
    )


def part_cluster_prisms(rig: Rig):
    """构建环绕伴生的次级金晶簇。"""
    bone = "cluster_prisms"
    rig.bone(bone, (0.0, 1.0, 0.0))

    # 伴生晶 1：右侧挺拔次主晶（立于右岩台上）
    add_amber_prism(
        rig, bone, "amber_sub_r",
        cx=2.6, cz=1.4, y0=2.2, y1=7.8,
        r=1.5, chamfer=0.45,
        mat_deep="amber_deep", mat_body="amber_body",
        mat_lit="amber_lit", mat_high="amber_high"
    )

    # 伴生晶 2：前倾突出小晶角（直刺正前方）
    add_amber_prism(
        rig, bone, "amber_sub_f",
        cx=-0.8, cz=2.8, y0=1.2, y1=5.8,
        r=1.3, chamfer=0.4,
        mat_deep="amber_deep", mat_body="amber_lit",
        mat_lit="amber_lit", mat_high="amber_high"
    )

    # 伴生晶 3：后方伴生晶
    add_amber_prism(
        rig, bone, "amber_sub_back",
        cx=1.8, cz=-2.6, y0=2.8, y1=8.6,
        r=1.4, chamfer=0.45,
        mat_deep="amber_deep", mat_body="amber_body",
        mat_lit="amber_lit", mat_high="amber_lit"
    )

    # 伴生晶 4：左岩缝斜生晶
    add_amber_prism(
        rig, bone, "amber_sub_left",
        cx=-3.0, cz=1.0, y0=3.2, y1=6.8,
        r=1.2, chamfer=0.35,
        mat_deep="amber_deep", mat_body="amber_body",
        mat_lit="amber_lit", mat_high="amber_high"
    )


def part_sulfur_crust(rig: Rig):
    """构建晶根与岩壳缝隙间的硫磺黄晕与细脉。"""
    bone = "sulfur_crust"
    rig.bone(bone, (0.0, 1.2, 0.0))

    # 前方开阔岩阶上的硫磺黄脉
    rig.cube(bone, "sulfur_front_a",
             (-1.6, 1.62, 1.8), (1.2, 2.05, 3.2),
             mat="sulfur_seam")
    rig.cube(bone, "sulfur_front_b",
             (-0.2, 1.55, 3.0), (2.2, 1.95, 4.4),
             mat="sulfur_seam")
    # 右侧伴生晶底部的积黄
    rig.cube(bone, "sulfur_right",
             (1.2, 1.75, 0.2), (2.6, 2.15, 1.6),
             mat="sulfur_seam")
    # 左岩缝黄晕
    rig.cube(bone, "sulfur_left_crevice",
             (-3.6, 2.15, -0.6), (-2.0, 2.55, 1.4),
             mat="sulfur_seam")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_soot_rock(rig)
    part_amber_crystal_core(rig)
    part_cluster_prisms(rig)
    part_sulfur_crust(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="雄黄 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "XiongHuang.bbmodel", help="输出路径")
    parser.add_argument("--part", choices=["rock", "core", "prisms", "sulfur"],
                        help="仅单独导出指定部件预览")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = Rig(MATS, swatch=8)

    if args.part == "rock":
        part_soot_rock(rig)
    elif args.part == "core":
        part_amber_crystal_core(rig)
    elif args.part == "prisms":
        part_cluster_prisms(rig)
    elif args.part == "sulfur":
        part_sulfur_crust(rig)
    else:
        part_soot_rock(rig)
        part_amber_crystal_core(rig)
        part_cluster_prisms(rig)
        part_sulfur_crust(rig)

    bb_json = rig.bbmodel("XiongHuang")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
