#!/usr/bin/env python3
"""盐蓬晶（SaltCrystal）—— 斜插深蓝六方长晶与放射晶簇原矿 Blockbench .bbmodel 生成器。

【彻底消除人造方底座，100% 还原原画灵动天然晶体簇】：
1. 绝无平整底座底板！
   底盘由 6 块向四方不规则放射刺出的小晶锥和碎晶尖自然支撑，纯粹是结晶簇聚合体。
2. 修长傲然的斜插六方主晶（Main Oblique Crystal Spike）：
   修长、尖锐、大倾角（向右前斜刺 ~34°），高耸拔起，顶端带有晶莹的单斜切面晶尖。
3. 纵贯晶体的天然白霜闪电解理（Frost Lightning Fracture）：
   沿主晶中轴曲折蔓延的极细冰白霜裂，通透深邃。
4. 侧后方簇生的多角度次生晶锥（Satellite Crystal Pencils）：
   紧贴主晶根部向不同方向翘起，形成星芒放射状天然簇生体。
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
from palette_saltcrystal import (
    SALT_VOID, SALT_DARK, SALT_INDIGO,
    CRYST_MID, CRYST_LIT, CRYST_HIGH,
    FROST_DEEP, FROST_WHITE
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

MATS = {
    "salt_void":   SALT_VOID,
    "salt_dark":   SALT_DARK,
    "salt_indigo": SALT_INDIGO,
    "cryst_mid":   CRYST_MID,
    "cryst_lit":   CRYST_LIT,
    "cryst_high":  CRYST_HIGH,
    "frost_deep":  FROST_DEEP,
    "frost_white": FROST_WHITE,
}


def part_main_oblique_pillar(rig: Rig):
    """构建修长锋利、大角度向右前上方斜刺拔起的主晶柱。"""
    bone = "main_oblique_pillar"
    rig.bone(bone, (0.0, 3.0, 0.0))

    # 旋转中心在左下方 (-0.8, 1.2, 0.0)
    org = (-0.8, 1.2, 0.0)
    # rot 角度：Z 向右倾 -32 度，Y 偏转 -15 度，X 仰角 18 度
    rot = (18.0, -15.0, -32.0)

    # 1. 晶柱中下段长身（修长六方棱柱，宽 2.6格，厚 2.4格，长 8.5格）
    rig.cube(bone, "pillar_shaft_x",
             (-1.3, 1.0, -1.0), (1.3, 9.5, 1.0),
             rot=rot, org=org, mat="cryst_mid")
    rig.cube(bone, "pillar_shaft_z",
             (-1.0, 1.0, -1.2), (1.0, 9.5, 1.2),
             rot=rot, org=org, mat="cryst_mid")
    # 正面受光通透斜切棱
    rig.cube(bone, "pillar_facet_front",
             (-0.9, 1.6, 0.95), (0.9, 9.0, 1.35),
             rot=rot, org=org, mat="cryst_lit")
    # 侧棱高光线
    rig.cube(bone, "pillar_glint_ridge",
             (-1.35, 2.0, 0.2), (-0.95, 9.2, 1.1),
             rot=rot, org=org, mat="cryst_high")

    # 2. 晶柱顶端单斜锋利晶尖（向右上尖锐收束，y=9.5 ~ 14.2格）
    rig.cube(bone, "pillar_taper_neck",
             (-0.9, 9.5, -0.85), (0.9, 12.0, 0.85),
             rot=rot, org=org, mat="cryst_lit")
    rig.cube(bone, "pillar_taper_facet",
             (-0.6, 9.8, 0.75), (0.6, 12.4, 1.1),
             rot=rot, org=org, mat="cryst_high")
    # 极尖斜刀刃点
    rig.cube(bone, "pillar_apex_spike",
             (-0.45, 12.0, -0.45), (0.45, 14.2, 0.45),
             rot=rot, org=org, mat="frost_white")

    # 3. 晶柱深层暗蓝根核
    rig.cube(bone, "pillar_root_plug",
             (-1.5, 0.2, -1.3), (1.5, 1.6, 1.3),
             rot=rot, org=org, mat="salt_indigo")


def part_frost_lightning_core(rig: Rig):
    """构建顺着主晶长轴曲折贯穿的细密白霜闪电裂隙。"""
    bone = "frost_lightning"
    rig.bone(bone, (0.0, 3.0, 0.0))

    org = (-0.8, 1.2, 0.0)
    rot = (18.0, -15.0, -32.0)

    # 贴在主晶正面 z=1.28 ~ 1.40 处（纤细曲折电裂）
    # 下段电裂
    rig.cube(bone, "lightning_s0",
             (-0.25, 2.0, 1.26), (0.25, 3.8, 1.36),
             rot=rot, org=org, mat="frost_deep")
    rig.cube(bone, "lightning_s1",
             (-0.45, 3.6, 1.28), (0.1, 5.4, 1.38),
             rot=rot, org=org, mat="frost_white")

    # 中段核心折点（闪烁冰白强光）
    rig.cube(bone, "lightning_nexus",
             (-0.15, 5.2, 1.30), (0.5, 7.2, 1.42),
             rot=rot, org=org, mat="frost_white")
    rig.cube(bone, "lightning_branch_w",
             (-0.65, 5.8, 1.28), (-0.1, 6.4, 1.37),
             rot=rot, org=org, mat="frost_deep")

    # 上段冲尖细裂
    rig.cube(bone, "lightning_s2",
             (0.05, 7.0, 1.05), (0.45, 9.4, 1.18),
             rot=rot, org=org, mat="frost_white")
    rig.cube(bone, "lightning_s3",
             (-0.2, 9.2, 0.82), (0.25, 11.8, 0.95),
             rot=rot, org=org, mat="frost_white")


def part_base_cluster(rig: Rig):
    """构建底部犬牙差互、向外斜刺的次生小晶簇群（彻底抛弃平整方块底板！）。"""
    bone = "base_cluster"
    rig.bone(bone, (0.0, 0.0, 0.0))

    # 1. 前左侧向左前方斜刺出的小晶尖（接触地面作为支点）
    org_fl = (-2.2, 0.2, 1.8)
    rot_fl = (10.0, 32.0, -22.0)
    rig.cube(bone, "cluster_fl_body",
             (-3.2, 0.0, 1.0), (-1.4, 3.6, 2.4),
             rot=rot_fl, org=org_fl, mat="salt_indigo")
    rig.cube(bone, "cluster_fl_tip",
             (-2.8, 3.4, 1.3), (-1.8, 5.2, 2.1),
             rot=rot_fl, org=org_fl, mat="cryst_lit")
    rig.cube(bone, "cluster_fl_spec",
             (-2.5, 5.0, 1.5), (-2.1, 6.0, 1.9),
             rot=rot_fl, org=org_fl, mat="frost_white")

    # 2. 前右侧贴近主晶根部向右前外翻的短粗晶角
    org_fr = (1.8, 0.2, 1.2)
    rot_fr = (15.0, -28.0, 24.0)
    rig.cube(bone, "cluster_fr_body",
             (1.0, 0.0, 0.4), (2.8, 3.2, 2.0),
             rot=rot_fr, org=org_fr, mat="salt_indigo")
    rig.cube(bone, "cluster_fr_tip",
             (1.4, 3.0, 0.7), (2.4, 4.6, 1.7),
             rot=rot_fr, org=org_fr, mat="cryst_lit")

    # 3. 后左侧向后仰出的一簇暗蓝次生小晶尖
    org_bl = (-2.0, 0.2, -2.4)
    rot_bl = (-18.0, -20.0, -12.0)
    rig.cube(bone, "cluster_bl_body",
             (-2.8, 0.0, -3.2), (-1.2, 3.8, -1.6),
             rot=rot_bl, org=org_bl, mat="salt_dark")
    rig.cube(bone, "cluster_bl_tip",
             (-2.4, 3.6, -2.8), (-1.6, 5.0, -2.0),
             rot=rot_bl, org=org_bl, mat="cryst_mid")

    # 4. 后右侧高耸暗蓝伴生柱（作为后方稳固支撑，高至 y=7.2）
    org_br = (1.2, 0.2, -2.2)
    rot_br = (-14.0, 18.0, 10.0)
    rig.cube(bone, "cluster_br_body",
             (0.4, 0.0, -3.0), (2.4, 5.2, -1.2),
             rot=rot_br, org=org_br, mat="salt_dark")
    rig.cube(bone, "cluster_br_tip",
             (0.8, 5.0, -2.6), (2.0, 7.2, -1.6),
             rot=rot_br, org=org_br, mat="salt_indigo")

    # 5. 核心底部紧凑相咬合的深渊极暗晶底（隐藏在晶体脚底，无外露底台）
    rig.cube(bone, "cluster_root_node",
             (-1.8, 0.0, -1.6), (1.6, 1.4, 1.4),
             mat="salt_void")


def part_satellite_spikes(rig: Rig):
    """构建主晶中段两侧向外挑出的 2 枚锐利伴生微晶尖。"""
    bone = "satellite_spikes"
    rig.bone(bone, (0.0, 5.0, 0.0))

    # 右侧中段向上刺出的次生晶锥（位于 y=5~9.2 格）
    org_r = (2.2, 4.0, -0.6)
    rot_r = (-5.0, -38.0, 20.0)
    rig.cube(bone, "sat_spike_r_body",
             (1.6, 3.8, -1.2), (3.0, 7.4, 0.2),
             rot=rot_r, org=org_r, mat="cryst_mid")
    rig.cube(bone, "sat_spike_r_tip",
             (1.9, 7.2, -0.9), (2.7, 9.0, -0.1),
             rot=rot_r, org=org_r, mat="cryst_high")

    # 左侧腰部向上挑出的小晶芒
    org_l = (-2.6, 4.4, -0.2)
    rot_l = (12.0, 42.0, -16.0)
    rig.cube(bone, "sat_spike_l_body",
             (-3.4, 4.2, -0.8), (-2.0, 7.2, 0.4),
             rot=rot_l, org=org_l, mat="cryst_mid")
    rig.cube(bone, "sat_spike_l_tip",
             (-3.1, 7.0, -0.5), (-2.3, 8.4, 0.1),
             rot=rot_l, org=org_l, mat="cryst_high")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_main_oblique_pillar(rig)
    part_frost_lightning_core(rig)
    part_base_cluster(rig)
    part_satellite_spikes(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="盐蓬晶斜插六方晶原矿 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "SaltCrystal.bbmodel", help="输出路径")
    parser.add_argument("--part", choices=["pillar", "lightning", "base", "satellite"],
                        help="仅单独导出指定部件预览")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = Rig(MATS, swatch=8)

    if args.part == "pillar":
        part_main_oblique_pillar(rig)
    elif args.part == "lightning":
        part_frost_lightning_core(rig)
    elif args.part == "base":
        part_base_cluster(rig)
    elif args.part == "satellite":
        part_satellite_spikes(rig)
    else:
        part_main_oblique_pillar(rig)
        part_frost_lightning_core(rig)
        part_base_cluster(rig)
        part_satellite_spikes(rig)

    bb_json = rig.bbmodel("SaltCrystal")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
