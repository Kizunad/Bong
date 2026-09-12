#!/usr/bin/env python3
"""活血丹（HuoXueDan）Blockbench .bbmodel 生成器。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/huo_xue_dan.png`
  `server/assets/items/pills.toml` (id = "huo_xue_dan", name = "活血丹")

原画构图深度还原：
  1. 后景粗麻布药囊（Linen Sack）：
     - 位于后景偏左（cx=-1.6, cz=-3.2），袋底平稳坐地，袋腹饱满圆鼓。
     - 束颈紧系朱红草绳与绳结流苏，袋口荷叶褶皱自然向上绽开。
  2. 前景侧倾圆木药盒（Tumbled Round Wooden Bowl）：
     - 位于前景偏左（cx=-1.8, cz=1.2），圆木药盒呈侧卧倾倒姿态，盒口向右前方开敞，中空展现深色内壁。
  3. 滚落而出的朱砂活血丹（Rolling Red Blood Pills）：
     - 盒口内 2 颗丹药半露，盒外地面呈自然弧线滚出散落 5 颗鲜艳夺目的朱红丹丸（向阳面朱赤高光）。
  4. 翻倒在一旁的圆木盖（Wooden Bowl Lid）：
     - 位于右侧偏前（cx=3.2, cz=1.4），圆盘木盖斜靠在地面上，带微凸圆顶与盖纽。

用法：
  python3 modelScript/generators/gen_huo_xue_dan.py
  bbmodel-render modelScript/models/HuoXueDan.bbmodel
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

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

# ── 材质色板 ──────────────────────────────────────────────────────────
# 严格提取自 huo_xue_dan.png 原画像素：
MATS = {
    "sack_lit":       (222, 210, 192),  # 麻布袋受光米灰
    "sack_mid":       (176, 160, 138),  # 麻布袋中阴影
    "sack_dark":      (132, 118, 98),   # 麻布袋深褶皱与背阴
    "cord_red":       (180, 50, 40),    # 朱红扎绳
    "cord_dark":      (124, 30, 24),    # 扎绳暗部
    "wood_base":      (134, 78, 46),    # 深木盒主体红褐色
    "wood_lit":       (168, 106, 66),   # 木盒向光弧面
    "wood_dark":      (84, 44, 26),     # 木盒内腔与暗部
    "pill_red":       (202, 58, 42),    # 活血丹鲜艳朱砂红
    "pill_lit":       (240, 102, 74),   # 活血丹向光高光红
    "pill_shadow":    (130, 32, 22),    # 活血丹阴影暗红
}


def add_round_box(rig: Rig, bone: str, prefix: str,
                  y0: float, y1: float, r: float, chamfer: float,
                  mat_m: str = "sack_lit", mat_s: str = "sack_mid",
                  cx: float = 0.0, cz: float = 0.0,
                  rot: tuple[float, float, float] | None = None,
                  org: tuple[float, float, float] | None = None):
    """十字平滑圆角块。支持整体旋转。"""
    rig.cube(bone, f"{prefix}_x",
             (cx - r, y0, cz - (r - chamfer)),
             (cx + r, y1, cz + (r - chamfer)),
             rot=rot, org=org, mat=mat_m)
    rig.cube(bone, f"{prefix}_z",
             (cx - (r - chamfer), y0, cz - r),
             (cx + (r - chamfer), y1, cz + r),
             rot=rot, org=org, mat=mat_s)


def add_smooth_pill(rig: Rig, bone: str, prefix: str,
                    cx: float, cy: float, cz: float, r: float):
    """精致圆润的朱红小活血丹。"""
    rc = r * 0.76
    rig.cube(bone, f"{prefix}_mx",
             (cx - r, cy - rc * 0.42, cz - rc),
             (cx + r, cy + rc * 0.42, cz + rc),
             mat="pill_red")
    rig.cube(bone, f"{prefix}_mz",
             (cx - rc, cy - rc * 0.42, cz - r),
             (cx + rc, cy + rc * 0.42, cz + r),
             mat="pill_red")
    rt = r * 0.62
    rig.cube(bone, f"{prefix}_top",
             (cx - rt, cy + rc * 0.42, cz - rt),
             (cx + rt, cy + r, cz + rt),
             mat="pill_lit")
    rig.cube(bone, f"{prefix}_bot",
             (cx - rt, cy - r, cz - rt),
             (cx + rt, cy - rc * 0.42, cz + rt),
             mat="pill_shadow")


def part_sack(rig: Rig) -> None:
    """背景粗麻布药袋（后景左后方：cx = -1.6, cz = -3.2）。"""
    rig.bone("sack", (-1.6, 0.0, -3.2))
    cx = -1.6
    cz = -3.2

    # 1. 袋底贴地平座 (y: 0.0 ~ 0.8)
    add_round_box(rig, "sack", "sack_bottom", 0.0, 0.8, r=3.2, chamfer=0.8,
                  mat_m="sack_dark", mat_s="sack_dark", cx=cx, cz=cz)

    # 2. 腹部下段渐起 (y: 0.8 ~ 2.4)
    add_round_box(rig, "sack", "sack_belly_low", 0.8, 2.4, r=4.0, chamfer=1.0,
                  mat_m="sack_lit", mat_s="sack_mid", cx=cx, cz=cz)

    # 3. 腹部最鼓处 (y: 2.4 ~ 4.8)
    add_round_box(rig, "sack", "sack_belly_mid", 2.4, 4.8, r=4.4, chamfer=1.1,
                  mat_m="sack_lit", mat_s="sack_mid", cx=cx, cz=cz)

    # 4. 腹部上段渐收 (y: 4.8 ~ 6.0)
    add_round_box(rig, "sack", "sack_belly_up", 4.8, 6.0, r=3.5, chamfer=0.8,
                  mat_m="sack_mid", mat_s="sack_dark", cx=cx, cz=cz)

    # 5. 束颈凹槽 (y: 6.0 ~ 6.7)
    add_round_box(rig, "sack", "sack_neck", 6.0, 6.7, r=2.4, chamfer=0.6,
                  mat_m="sack_dark", mat_s="sack_dark", cx=cx, cz=cz)

    # 6. 朱红扎绳 (y: 6.4 ~ 7.0)
    add_round_box(rig, "sack", "sack_cord", 6.4, 7.0, r=2.65, chamfer=0.6,
                  mat_m="cord_red", mat_s="cord_red", cx=cx, cz=cz)

    # 扎绳结扣与垂落绳须 (+X 方向垂挂)
    rig.cube("sack", "cord_knot",
             (cx + 2.1, 6.3, cz + 0.3), (cx + 3.0, 7.2, cz + 1.2),
             mat="cord_red")
    rig.cube("sack", "cord_tail1",
             (cx + 2.4, 4.5, cz + 0.6), (cx + 2.9, 6.4, cz + 1.0),
             rot=(0.0, 0.0, -15.0), org=(cx + 2.4, 6.4, cz + 0.8),
             mat="cord_red")
    rig.cube("sack", "cord_tail2",
             (cx + 2.7, 3.8, cz + 0.8), (cx + 3.2, 5.2, cz + 1.2),
             rot=(0.0, 0.0, -10.0), org=(cx + 2.7, 5.2, cz + 1.0),
             mat="cord_dark")

    # 7. 袋口折褶荷叶边 (y: 6.7 ~ 8.6)
    add_round_box(rig, "sack", "sack_frill_base", 6.7, 7.6, r=2.8, chamfer=0.7,
                  mat_m="sack_lit", mat_s="sack_mid", cx=cx, cz=cz)
    add_round_box(rig, "sack", "sack_frill_top", 7.6, 8.6, r=3.3, chamfer=0.8,
                  mat_m="sack_lit", mat_s="sack_mid", cx=cx, cz=cz)


def part_bowl(rig: Rig) -> None:
    """倾倒在前的深色圆木药盅与木盖。"""
    rig.bone("bowl", (-1.6, 1.2, 1.2))

    bx, bz = -1.6, 1.2
    org_b = (bx, 0.6, bz)
    rot_b = (14.0, -18.0, -42.0)

    # 1. 盅底厚足 (y: 0.0 ~ 0.8, r=2.2)
    add_round_box(rig, "bowl", "b_foot", 0.0, 0.8, r=2.2, chamfer=0.5,
                  mat_m="wood_dark", mat_s="wood_dark",
                  cx=bx, cz=bz, rot=rot_b, org=org_b)

    # 2. 盅腹圆弧膨胀 (y: 0.8 ~ 2.4, r=3.0)
    add_round_box(rig, "bowl", "b_belly", 0.8, 2.4, r=3.0, chamfer=0.7,
                  mat_m="wood_base", mat_s="wood_lit",
                  cx=bx, cz=bz, rot=rot_b, org=org_b)

    # 3. 盅口开敞厚卷沿 (y: 2.4 ~ 3.8, r_out=3.2, r_in=2.3)
    r_out = 3.2
    r_in = 2.3
    hw = r_out * 0.72
    rig.cube("bowl", "b_rim_f",
             (bx - hw, 2.4, bz + r_in), (bx + hw, 3.8, bz + r_out),
             rot=rot_b, org=org_b, mat="wood_lit")
    rig.cube("bowl", "b_rim_b",
             (bx - hw, 2.4, bz - r_out), (bx + hw, 3.8, bz - r_in),
             rot=rot_b, org=org_b, mat="wood_dark")
    rig.cube("bowl", "b_rim_l",
             (bx - r_out, 2.4, bz - hw), (bx - r_in, 3.8, bz + hw),
             rot=rot_b, org=org_b, mat="wood_base")
    rig.cube("bowl", "b_rim_r",
             (bx + r_in, 2.4, bz - hw), (bx + r_out, 3.8, bz + hw),
             rot=rot_b, org=org_b, mat="wood_base")

    # 4. 盅内腔底衬 (托住内部丹药)
    rig.cube("bowl", "b_cavity_bed",
             (bx - 2.1, 1.0, bz - 2.1), (bx + 2.1, 1.6, bz + 2.1),
             rot=rot_b, org=org_b, mat="wood_dark")

    # 5. 斜靠在右侧的木盒盖 (cx ≈ 3.2, cz ≈ 1.2)
    lx, lz = 3.2, 1.2
    org_l = (lx, 0.4, lz)
    rot_l = (12.0, -15.0, -45.0)

    # 盖沿圆盘 (r=2.4, 厚 0.5)
    add_round_box(rig, "bowl", "lid_rim", 0.0, 0.5, r=2.4, chamfer=0.6,
                  mat_m="wood_base", mat_s="wood_base",
                  cx=lx, cz=lz, rot=rot_l, org=org_l)
    # 盖面微拱穹顶 (r=1.8, 厚 0.5)
    add_round_box(rig, "bowl", "lid_dome", 0.5, 1.0, r=1.8, chamfer=0.4,
                  mat_m="wood_lit", mat_s="wood_base",
                  cx=lx, cz=lz, rot=rot_l, org=org_l)
    # 盖纽圆提手 (r=0.6, 高 0.6)
    add_round_box(rig, "bowl", "lid_knob", 1.0, 1.6, r=0.6, chamfer=0.2,
                  mat_m="wood_dark", mat_s="wood_dark",
                  cx=lx, cz=lz, rot=rot_l, org=org_l)


def part_pills(rig: Rig) -> None:
    """鲜红散落的朱砂活血丹丸。
    盅口内 2 颗半露，盅外地面自然散落 4 颗，颗颗鲜红夺目！
    """
    rig.bone("pills", (0.0, 0.0, 0.0))

    r = 1.10

    # 1. 盅口内露出的一颗丹药
    add_smooth_pill(rig, "pills", "p_in_1", cx=-1.0, cy=2.2, cz=1.3, r=r)

    # 2. 盅口下唇正欲滚出的一颗丹药
    add_smooth_pill(rig, "pills", "p_in_2", cx=-0.2, cy=1.5, cz=1.9, r=r)

    # 3. 滚落在盅口正前方地面的主丹（最前、最亮、第一视觉焦点！）
    add_smooth_pill(rig, "pills", "p_front_main", cx=0.7, cy=1.12, cz=3.2, r=1.18)

    # 4. 滚落在地面前排偏右的一颗
    add_smooth_pill(rig, "pills", "p_out_r1", cx=1.7, cy=1.08, cz=2.3, r=1.10)

    # 5. 滚落在最右前侧的一颗
    add_smooth_pill(rig, "pills", "p_out_r2", cx=2.4, cy=1.05, cz=3.3, r=1.05)

    # 6. 中间深处的一颗（形成深浅多层次散落感）
    add_smooth_pill(rig, "pills", "p_out_mid", cx=0.8, cy=1.08, cz=1.5, r=1.08)


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_sack(rig)
    part_bowl(rig)
    part_pills(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="活血丹 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "HuoXueDan.bbmodel", help="输出路径")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = build_rig()
    bb_json = rig.bbmodel("HuoXueDan")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
