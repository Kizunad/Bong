#!/usr/bin/env python3
"""灵晶（LingJing）—— 悬浮双锥八面体天道灵核 Blockbench .bbmodel 生成器。

【世界观与纯材料第一性原理】：
- 物品来源：`ling_jing.png`
  “青云/血谷产，晶莹透亮法宝阵眼核心。”
- 彻底摒弃厚重石底座！
  灵晶是高纯度天地真元在极端灵压下凝华析出的“双锥悬浮灵核”（Double-Terminated Crystal Core）。
  两端皆为收尖晶锥，周身自然悬浮于空，伴生数枚微细碎砾，晶身内蕴天然天道灵符。

结构部件：
  1. main_bipyramid_core: 正中悬浮的双锥八面棱晶（腰部宽厚、两极向上下对称收尖，高 1.8 ~ 14.2格）
  2. celestial_runes:     晶面正反天然生成的苍白天道灵符刻痕
  3. floating_shards:     环绕主晶四周悬浮漂浮的 4 枚伴生微晶砾
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
from palette_lingjing import (
    CRYSTAL_VOID, CRYSTAL_DEEP, CRYSTAL_MID, CRYSTAL_LIT, CRYSTAL_HIGH
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

MATS = {
    "cryst_void": CRYSTAL_VOID,
    "cryst_deep": CRYSTAL_DEEP,
    "cryst_mid":  CRYSTAL_MID,
    "cryst_lit":  CRYSTAL_LIT,
    "cryst_high": CRYSTAL_HIGH,
}


def part_main_bipyramid_core(rig: Rig):
    """构建悬浮在空中、上下两头收尖的双锥棱柱主核。"""
    bone = "bipyramid_core"
    rig.bone(bone, (0.0, 8.0, 0.0))

    # 主晶微向右前倾斜旋转，呈现天然悬浮灵动感（rot=(8, -12, 10)）
    org = (0.0, 8.0, 0.0)
    rot = (8.0, -12.0, 10.0)

    # 1. 腰部主晶柱（最宽厚段，y=6.0 ~ 10.0，宽 4.8格，八面体素倒角逼近）
    # 十字主核
    rig.cube(bone, "waist_cross_x",
             (-1.6, 6.0, -2.4), (1.6, 10.0, 2.4),
             rot=rot, org=org, mat="cryst_mid")
    rig.cube(bone, "waist_cross_z",
             (-2.4, 6.0, -1.6), (2.4, 10.0, 1.6),
             rot=rot, org=org, mat="cryst_deep")
    # 四侧切角面（营造八面柱镜面折光）
    rig.cube(bone, "waist_facet_fl",
             (-2.1, 6.2, 1.2), (-1.2, 9.8, 2.1),
             rot=rot, org=org, mat="cryst_lit")
    rig.cube(bone, "waist_facet_fr",
             (1.2, 6.2, 1.2), (2.1, 9.8, 2.1),
             rot=rot, org=org, mat="cryst_lit")
    rig.cube(bone, "waist_glint_front",
             (-0.6, 6.6, 2.35), (0.6, 9.4, 2.48),
             rot=rot, org=org, mat="cryst_high")

    # 2. 上部向上收尖晶锥（y=10.0 ~ 14.4）
    # 上锥第一段
    rig.cube(bone, "upper_taper_sec1",
             (-1.8, 10.0, -1.8), (1.8, 11.6, 1.8),
             rot=rot, org=org, mat="cryst_lit")
    # 上锥第二段
    rig.cube(bone, "upper_taper_sec2",
             (-1.2, 11.6, -1.2), (1.2, 13.0, 1.2),
             rot=rot, org=org, mat="cryst_lit")
    # 上极点尖针
    rig.cube(bone, "upper_tip_apex",
             (-0.5, 13.0, -0.5), (0.5, 14.4, 0.5),
             rot=rot, org=org, mat="cryst_high")

    # 3. 下部倒置收尖晶锥（y=1.8 ~ 6.0，悬浮离地）
    # 下锥第一段
    rig.cube(bone, "lower_taper_sec1",
             (-1.8, 4.4, -1.8), (1.8, 6.0, 1.8),
             rot=rot, org=org, mat="cryst_deep")
    # 下锥第二段
    rig.cube(bone, "lower_taper_sec2",
             (-1.2, 3.0, -1.2), (1.2, 4.4, 1.2),
             rot=rot, org=org, mat="cryst_deep")
    # 下极点尖针
    rig.cube(bone, "lower_tip_apex",
             (-0.5, 1.8, -0.5), (0.5, 3.0, 0.5),
             rot=rot, org=org, mat="cryst_lit")


def part_celestial_runes(rig: Rig):
    """构建主晶正反晶面阳刻的天道灵符微光阵痕（与原画中心灵符完全同构）。"""
    bone = "celestial_runes"
    rig.bone(bone, (0.0, 8.0, 0.0))

    org = (0.0, 8.0, 0.0)
    rot = (8.0, -12.0, 10.0)

    # 1. 正面中心纵向天道符主干线（贴在 z=2.4 处）
    rig.cube(bone, "rune_central_spine",
             (-0.18, 5.2, 2.45), (0.18, 10.8, 2.55),
             rot=rot, org=org, mat="cryst_high")

    # 2. 交叉菱形天道灵纹（回形纹与尖角支线）
    rig.cube(bone, "rune_diamond_top",
             (-1.0, 8.8, 2.38), (1.0, 9.6, 2.48),
             rot=rot, org=org, mat="cryst_high")
    rig.cube(bone, "rune_diamond_bot",
             (-1.0, 6.4, 2.38), (1.0, 7.2, 2.48),
             rot=rot, org=org, mat="cryst_high")
    rig.cube(bone, "rune_wing_left",
             (-1.4, 7.4, 2.15), (-0.8, 8.6, 2.35),
             rot=rot, org=org, mat="cryst_lit")
    rig.cube(bone, "rune_wing_right",
             (0.8, 7.4, 2.15), (1.4, 8.6, 2.35),
             rot=rot, org=org, mat="cryst_lit")

    # 3. 核心凝灵光核点
    rig.cube(bone, "rune_core_pip",
             (-0.4, 7.6, 2.48), (0.4, 8.4, 2.62),
             rot=rot, org=org, mat="cryst_high")


def part_floating_shards(rig: Rig):
    """构建环绕主晶四周悬浮翻飞的微细伴生晶砾（无接触、真悬浮）。"""
    bone = "floating_shards"
    rig.bone(bone, (0.0, 8.0, 0.0))

    # 碎砾 1：右前上方翻转小晶片（高光）
    rig.cube(bone, "shard_fr_high",
             (3.0, 9.2, 1.8), (4.2, 11.0, 2.8),
             rot=(18.0, 35.0, -25.0), org=(3.6, 10.1, 2.3),
             mat="cryst_high")
    rig.cube(bone, "shard_fr_tip",
             (3.3, 11.0, 2.0), (3.9, 11.8, 2.6),
             rot=(18.0, 35.0, -25.0), org=(3.6, 10.1, 2.3),
             mat="cryst_lit")

    # 碎砾 2：左侧腰部浮动薄片
    rig.cube(bone, "shard_left_mid",
             (-4.4, 6.8, -0.6), (-3.2, 8.6, 0.6),
             rot=(-15.0, -42.0, 20.0), org=(-3.8, 7.7, 0.0),
             mat="cryst_lit")

    # 碎砾 3：后方下沉小晶砾
    rig.cube(bone, "shard_back_low",
             (1.2, 3.4, -3.8), (2.4, 4.8, -2.6),
             rot=(24.0, -18.0, 15.0), org=(1.8, 4.1, -3.2),
             mat="cryst_deep")

    # 碎砾 4：左前方下方悬浮伴生晶芽
    rig.cube(bone, "shard_fl_ground",
             (-2.8, 2.6, 2.4), (-1.6, 4.2, 3.4),
             rot=(-10.0, 28.0, -16.0), org=(-2.2, 3.4, 2.9),
             mat="cryst_lit")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_main_bipyramid_core(rig)
    part_celestial_runes(rig)
    part_floating_shards(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="灵晶双锥悬浮灵核 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "LingJing.bbmodel", help="输出路径")
    parser.add_argument("--part", choices=["core", "runes", "shards"],
                        help="仅单独导出指定部件预览")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = Rig(MATS, swatch=8)

    if args.part == "core":
        part_main_bipyramid_core(rig)
    elif args.part == "runes":
        part_celestial_runes(rig)
    elif args.part == "shards":
        part_floating_shards(rig)
    else:
        part_main_bipyramid_core(rig)
        part_celestial_runes(rig)
        part_floating_shards(rig)

    bb_json = rig.bbmodel("LingJing")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
