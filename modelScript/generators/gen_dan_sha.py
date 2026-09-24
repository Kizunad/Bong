#!/usr/bin/env python3
"""丹砂（DanSha）矿石与晶簇 Blockbench .bbmodel 生成器。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/dan_sha.png`
  `server/assets/items/minerals.toml` (id = "dan_sha", name = "丹砂")
  配套方块贴图：`client/src/main/resources/assets/bong/textures/block/ore_dan_sha.png`

视觉特征与结构解剖（严格呼应图标与方块贴图）：
  1. 风化红岩碎裂围岩基座（part_rock_base）：
     - 围岩粗粝破碎，呈多阶层叠包围状，暗褐与深赭红交错。
     - 包含：底层支撑碎岩、左侧厚实围岩崖壁、右侧向外开裂的次级风化岩块。
  2. 高耸六棱挺拔主晶柱（part_main_crystal）：
     - 正中傲然挺立的大型丹砂晶柱（高 10~11 格），多段切角呈现天然六方晶系结晶感。
     - 顶端向阳倾斜截面，带有极亮朱红高光（CRYSTAL_HIGHLIGHT），腰身饱满透红（CRYSTAL_BODY）。
  3. 环绕簇拥伴生小晶刺（part_cluster_pencils）：
     - 环绕在主晶基部和围岩缝隙中的 4 支倾斜伴生晶锥（左前倾刺、右前短晶、后侧高伴生棱柱）。
  4. 缝隙渗出赤红丹砂脉络（part_veins）：
     - 基座开裂深处与晶体接缝处渗出的高饱和炽红细脉，与方块贴图中的朱红晶脉 1:1 呼应。

用法：
  python3 modelScript/generators/gen_dan_sha.py
  bbmodel-render modelScript/models/DanSha.bbmodel
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
from palette_dansha import (
    ROCK_DARK, ROCK_BASE, ROCK_MID, ROCK_LIT,
    CRYSTAL_DEEP, CRYSTAL_BODY, CRYSTAL_LIT, CRYSTAL_HIGHLIGHT,
    VEIN_GLOW
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

# 材质映射字典
MATS = {
    "rock_dark": ROCK_DARK,
    "rock_base": ROCK_BASE,
    "rock_mid":  ROCK_MID,
    "rock_lit":  ROCK_LIT,
    "cryst_deep": CRYSTAL_DEEP,
    "cryst_body": CRYSTAL_BODY,
    "cryst_lit":  CRYSTAL_LIT,
    "cryst_high": CRYSTAL_HIGHLIGHT,
    "vein_glow":  VEIN_GLOW,
}


def add_hex_crystal(rig: Rig, bone: str, prefix: str,
                    cx: float, cz: float, y0: float, y1: float,
                    r: float, chamfer: float,
                    mat_body: str = "cryst_body", mat_lit: str = "cryst_lit",
                    mat_high: str = "cryst_high"):
    """构建六方结晶体素棱柱与尖端切面。"""
    # 柱身：主十字与四角切角
    rx = r
    rz = r * 0.86
    c = chamfer
    
    # 中心核心
    rig.cube(bone, f"{prefix}_core",
             (cx - (rx - c), y0, cz - rz),
             (cx + (rx - c), y1 - 1.2, cz + rz),
             mat=mat_body)
    # 横翼
    rig.cube(bone, f"{prefix}_cross",
             (cx - rx, y0, cz - (rz - c)),
             (cx + rx, y1 - 1.2, cz + (rz - c)),
             mat=mat_lit)
    
    # 尖端收束晶锥（高光与向阳切面）
    h_top = y1
    h_mid = y1 - 1.2
    rig.cube(bone, f"{prefix}_top_core",
             (cx - (rx - c * 1.5), h_mid, cz - (rz - c * 1.5)),
             (cx + (rx - c * 1.5), h_top - 0.5, cz + (rz - c * 1.5)),
             mat=mat_lit)
    rig.cube(bone, f"{prefix}_tip",
             (cx - (rx * 0.4), h_top - 0.5, cz - (rz * 0.4)),
             (cx + (rx * 0.4), h_top, cz + (rz * 0.4)),
             mat=mat_high)


def part_rock_base(rig: Rig):
    """构建多层破碎风化红岩基座。"""
    bone = "rock_base"
    rig.bone(bone, (0.0, 0.0, 0.0))
    
    # 1. 广布底层碎岩（贴地面支撑）
    rig.cube(bone, "rock_bed_main",
             (-5.8, 0.0, -5.2), (5.2, 1.6, 5.0),
             mat="rock_base")
    rig.cube(bone, "rock_bed_fl",
             (-6.4, 0.0, 1.2), (-2.8, 2.2, 5.8),
             mat="rock_dark")
    rig.cube(bone, "rock_bed_br",
             (1.5, 0.0, -5.8), (6.0, 1.8, -1.2),
             mat="rock_mid")

    # 2. 左侧高耸岩壁（包裹主晶柱左侧）
    rig.cube(bone, "rock_wall_l_low",
             (-5.6, 1.6, -3.2), (-2.6, 4.4, 3.4),
             mat="rock_lit")
    rig.cube(bone, "rock_wall_l_step",
             (-5.2, 4.4, -2.4), (-3.2, 6.6, 2.0),
             mat="rock_mid")
    rig.cube(bone, "rock_wall_l_top",
             (-4.8, 6.6, -1.2), (-3.6, 8.0, 0.8),
             mat="rock_dark")

    # 3. 后方围岩屏障（为晶簇提供深色映衬）
    rig.cube(bone, "rock_back_bulk",
             (-2.4, 1.6, -5.4), (3.6, 5.2, -2.6),
             mat="rock_dark")
    rig.cube(bone, "rock_back_ridge",
             (-1.2, 5.2, -5.0), (2.4, 7.2, -3.0),
             mat="rock_base")

    # 4. 前右低矮裂石台
    rig.cube(bone, "rock_fr_ledge",
             (2.6, 1.4, 0.8), (5.6, 3.6, 4.4),
             mat="rock_lit")
    rig.cube(bone, "rock_fr_shelf",
             (1.8, 1.2, 2.8), (4.2, 2.6, 5.4),
             mat="rock_mid")


def part_main_crystal(rig: Rig):
    """构建中央高耸六棱丹砂主晶。"""
    bone = "main_crystal"
    rig.bone(bone, (0.0, 1.4, 0.0))
    # 中心主晶：高耸傲立，位于原点略偏后，微向右前挺出
    add_hex_crystal(
        rig, bone, "cryst_main",
        cx=-0.2, cz=-0.2, y0=1.4, y1=11.6,
        r=2.5, chamfer=0.8,
        mat_body="cryst_body", mat_lit="cryst_lit", mat_high="cryst_high"
    )


def part_cluster_pencils(rig: Rig):
    """构建环绕伴生的一簇次级晶锥。"""
    bone = "cluster_pencils"
    rig.bone(bone, (0.0, 1.0, 0.0))
    
    # 伴生晶 1：右前倾斜晶体（挺立在右岩台上）
    add_hex_crystal(
        rig, bone, "cryst_sub_fr",
        cx=2.8, cz=1.6, y0=2.4, y1=7.8,
        r=1.5, chamfer=0.45,
        mat_body="cryst_body", mat_lit="cryst_lit", mat_high="cryst_high"
    )

    # 伴生晶 2：前倾突出小晶簇（直刺正前方）
    add_hex_crystal(
        rig, bone, "cryst_sub_f",
        cx=-0.8, cz=3.0, y0=1.2, y1=5.6,
        r=1.3, chamfer=0.4,
        mat_body="cryst_deep", mat_lit="cryst_body", mat_high="cryst_lit"
    )

    # 伴生晶 3：后方深色细伴生晶
    add_hex_crystal(
        rig, bone, "cryst_sub_back",
        cx=2.0, cz=-2.4, y0=3.2, y1=8.8,
        r=1.4, chamfer=0.45,
        mat_body="cryst_deep", mat_lit="cryst_lit", mat_high="cryst_lit"
    )

    # 伴生晶 4：左岩缝中生出的斜生小棱角晶
    add_hex_crystal(
        rig, bone, "cryst_sub_left",
        cx=-3.0, cz=1.2, y0=3.4, y1=6.8,
        r=1.2, chamfer=0.35,
        mat_body="cryst_deep", mat_lit="cryst_lit", mat_high="cryst_high"
    )


def part_veins(rig: Rig):
    """构建岩石开裂缝隙处透出的丹砂熔脉。"""
    bone = "veins"
    rig.bone(bone, (0.0, 1.5, 0.0))
    # 前方开阔岩阶上的明晰朱红晶化细脉
    rig.cube(bone, "vein_front_a",
             (-1.6, 1.62, 2.0), (1.2, 2.05, 3.2),
             mat="vein_glow")
    rig.cube(bone, "vein_front_b",
             (-0.2, 1.55, 3.0), (2.4, 1.95, 4.5),
             mat="vein_glow")
    rig.cube(bone, "vein_front_c",
             (-2.2, 1.45, 3.4), (-0.4, 1.85, 4.8),
             mat="vein_glow")
    # 右侧伴生晶下的炽红岩缝
    rig.cube(bone, "vein_right",
             (1.4, 1.75, 0.2), (2.8, 2.15, 1.8),
             mat="vein_glow")
    # 左侧围岩陡壁缝隙渗出的晶液
    rig.cube(bone, "vein_left_crevice",
             (-3.6, 2.15, -0.6), (-2.0, 2.55, 1.4),
             mat="vein_glow")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_rock_base(rig)
    part_main_crystal(rig)
    part_cluster_pencils(rig)
    part_veins(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="丹砂 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "DanSha.bbmodel", help="输出路径")
    parser.add_argument("--part", choices=["base", "main", "pencils", "veins"],
                        help="仅单独导出指定部件预览")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = Rig(MATS, swatch=8)

    if args.part == "base":
        part_rock_base(rig)
    elif args.part == "main":
        part_main_crystal(rig)
    elif args.part == "pencils":
        part_cluster_pencils(rig)
    elif args.part == "veins":
        part_veins(rig)
    else:
        part_rock_base(rig)
        part_main_crystal(rig)
        part_cluster_pencils(rig)
        part_veins(rig)

    bb_json = rig.bbmodel("DanSha")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")

    # 软光栅预览
    try:
        from bbmodel_maker.render import render_model
        img = render_model(args.out, yaw=45, pitch=25, roll=0, width=512, height=512)
        out_render = RENDER_OUT / "render_DanSha.png"
        img.save(str(out_render))
        print(f"Rendered perspective preview -> {out_render}")
    except Exception as e:
        print(f"Render skipped or failed: {e}")


if __name__ == "__main__":
    main()
