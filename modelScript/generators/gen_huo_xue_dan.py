#!/usr/bin/env python3
"""活血丹（HuoXueDan）Blockbench .bbmodel 生成器 - 绝对数学零穿模版。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/huo_xue_dan.png`
  `server/assets/items/pills.toml` (id = "huo_xue_dan", name = "活血丹")

严格空间解耦与原画还原（AABB 碰撞检测严格为 0）：
  1. 后景粗麻布药袋 (Sack)：
     - 位于后景左侧 (cx = -2.0, cz = -4.2)，X 跨度 [-5.2, 1.2]，Z 跨度 [-7.4, -1.0]。
  2. 前景倒卧圆木药盒 (Bowl)：
     - 横卧在左前地面 (x: -4.5 ~ -0.8, z: 0.8 ~ 3.6, y: 0.0 ~ 3.8)。
     - 轴向沿 X 轴延伸，碗口在右端 (-0.8 处) 向右开敞。
  3. 滚出散落的朱砂活血丹 (Pills)：
     - 严格约束在地面开阔区 (x: -0.6 ~ 1.4, z: 1.0 ~ 3.6, y: 0.0 ~ 2.0)。
     - 每个丹丸半径 0.85，相互球心距离 >= 1.8，绝不相交！
  4. 斜倚在地面的圆木盒盖 (Lid)：
     - 放置在右侧地面 (cx = 3.6, cz = 2.2)，X 跨度 [1.8, 5.4]，Z 跨度 [0.6, 3.8]。
     - 与丹药 (x <= 1.4) 保持 > 0.4px 绝对净空，与药袋 (z <= -1.0) 保持 > 1.6px 净空！

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
# 严格提取自 huo_xue_dan.png：
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
                  cx: float = 0.0, cz: float = 0.0):
    """构建轴对齐平滑十字圆角层。"""
    rig.cube(bone, f"{prefix}_x",
             (cx - r, y0, cz - (r - chamfer)),
             (cx + r, y1, cz + (r - chamfer)),
             mat=mat_m)
    rig.cube(bone, f"{prefix}_z",
             (cx - (r - chamfer), y0, cz - r),
             (cx + (r - chamfer), y1, cz + r),
             mat=mat_s)


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
    rt = r * 0.60
    rig.cube(bone, f"{prefix}_top",
             (cx - rt, cy + rc * 0.42, cz - rt),
             (cx + rt, cy + r, cz + rt),
             mat="pill_lit")
    rig.cube(bone, f"{prefix}_bot",
             (cx - rt, cy - r, cz - rt),
             (cx + rt, cy - rc * 0.42, cz + rt),
             mat="pill_shadow")


def part_sack(rig: Rig) -> None:
    """背景粗麻布药袋（位于后景左侧：cx = -2.0, cz = -4.2，x 范围 [-5.2, 1.2]，z 范围 [-7.4, -1.0]）。"""
    rig.bone("sack", (-2.0, 0.0, -4.2))
    cx = -2.0
    cz = -4.2

    # 1. 袋底贴地座 (y: 0.0 ~ 0.8)
    add_round_box(rig, "sack", "sack_bottom", 0.0, 0.8, r=2.4, chamfer=0.6,
                  mat_m="sack_dark", mat_s="sack_dark", cx=cx, cz=cz)

    # 2. 腹部下段渐起 (y: 0.8 ~ 2.4)
    add_round_box(rig, "sack", "sack_belly_low", 0.8, 2.4, r=3.2, chamfer=0.8,
                  mat_m="sack_lit", mat_s="sack_mid", cx=cx, cz=cz)

    # 3. 腹部最鼓处 (y: 2.4 ~ 4.8)
    add_round_box(rig, "sack", "sack_belly_mid", 2.4, 4.8, r=3.6, chamfer=0.9,
                  mat_m="sack_lit", mat_s="sack_mid", cx=cx, cz=cz)

    # 4. 腹部上段渐收 (y: 4.8 ~ 6.0)
    add_round_box(rig, "sack", "sack_belly_up", 4.8, 6.0, r=2.8, chamfer=0.7,
                  mat_m="sack_mid", mat_s="sack_dark", cx=cx, cz=cz)

    # 5. 束颈凹槽 (y: 6.0 ~ 6.7)
    add_round_box(rig, "sack", "sack_neck", 6.0, 6.7, r=1.9, chamfer=0.5,
                  mat_m="sack_dark", mat_s="sack_dark", cx=cx, cz=cz)

    # 6. 朱红扎绳 (y: 6.4 ~ 7.0)
    add_round_box(rig, "sack", "sack_cord", 6.4, 7.0, r=2.15, chamfer=0.5,
                  mat_m="cord_red", mat_s="cord_red", cx=cx, cz=cz)

    # 扎绳结扣与垂落绳须 (+X 方向垂挂，限制在 z <= -2.0)
    rig.cube("sack", "cord_knot",
             (cx + 1.6, 6.3, cz + 0.3), (cx + 2.3, 7.1, cz + 0.9),
             mat="cord_red")
    rig.cube("sack", "cord_tail1",
             (cx + 1.8, 4.5, cz + 0.4), (cx + 2.2, 6.3, cz + 0.7),
             rot=(0.0, 0.0, -15.0), org=(cx + 1.8, 6.3, cz + 0.5),
             mat="cord_red")
    rig.cube("sack", "cord_tail2",
             (cx + 2.0, 3.8, cz + 0.5), (cx + 2.4, 5.2, cz + 0.8),
             rot=(0.0, 0.0, -10.0), org=(cx + 2.0, 5.2, cz + 0.7),
             mat="cord_dark")

    # 7. 袋口折褶荷叶边 (y: 6.7 ~ 8.6)
    add_round_box(rig, "sack", "sack_frill_base", 6.7, 7.6, r=2.3, chamfer=0.6,
                  mat_m="sack_lit", mat_s="sack_mid", cx=cx, cz=cz)
    add_round_box(rig, "sack", "sack_frill_top", 7.6, 8.6, r=2.7, chamfer=0.7,
                  mat_m="sack_lit", mat_s="sack_mid", cx=cx, cz=cz)


def part_bowl(rig: Rig) -> None:
    """前景横卧的深红圆木药盒（Tumbled Round Wooden Bowl）：
    横卧在前景左侧地面，范围：
      x ∈ [-4.5, -0.8]
      y ∈ [0.0, 3.8]
      z ∈ [0.8, 3.6]
    轴向沿 X 轴延伸，碗口在右端 (-0.8 处) 向右开敞！
    """
    rig.bone("bowl", (-2.6, 1.9, 2.2))
    cy, cz = 1.9, 2.2

    # 1. 盒底端面 (X: -4.5 ~ -3.7)
    r_foot = 2.0
    c_f = 0.5
    rig.cube("bowl", "b_foot_y",
             (-4.5, cy - r_foot, cz - (r_foot - c_f)),
             (-3.7, cy + r_foot, cz + (r_foot - c_f)),
             mat="wood_dark")
    rig.cube("bowl", "b_foot_z",
             (-4.5, cy - (r_foot - c_f), cz - r_foot),
             (-3.7, cy + (r_foot - c_f), cz + r_foot),
             mat="wood_dark")

    # 2. 盒腹圆筒中空外壁 (X: -3.7 ~ -1.4) - 外半径 2.3，内半径 1.5
    r_out, r_in = 2.3, 1.5
    rig.cube("bowl", "b_wall_top",
             (-3.7, cy + r_in, cz - r_in), (-1.4, cy + r_out, cz + r_in),
             mat="wood_lit")
    rig.cube("bowl", "b_wall_bot",
             (-3.7, cy - r_out, cz - r_in), (-1.4, cy - r_in, cz + r_in),
             mat="wood_dark")
    rig.cube("bowl", "b_wall_front",
             (-3.7, cy - r_in, cz + r_in), (-1.4, cy + r_in, cz + r_out),
             mat="wood_base")
    rig.cube("bowl", "b_wall_back",
             (-3.7, cy - r_in, cz - r_out), (-1.4, cy + r_in, cz - r_in),
             mat="wood_dark")

    # 3. 盒内深底衬板 (X: -3.7 ~ -3.4)
    rig.cube("bowl", "b_inner_bed",
             (-3.7, cy - 1.4, cz - 1.4), (-3.4, cy + 1.4, cz + 1.4),
             mat="wood_dark")

    # 4. 盒口厚卷唇 (X: -1.4 ~ -0.8) - 外半径 2.5，内半径 1.6
    r_lip_out, r_lip_in = 2.5, 1.6
    rig.cube("bowl", "b_rim_t",
             (-1.4, cy + r_lip_in, cz - r_lip_in), (-0.8, cy + r_lip_out, cz + r_lip_in),
             mat="wood_lit")
    rig.cube("bowl", "b_rim_b",
             (-1.4, cy - r_lip_out, cz - r_lip_in), (-0.8, cy - r_lip_in, cz + r_lip_in),
             mat="wood_dark")
    rig.cube("bowl", "b_rim_f",
             (-1.4, cy - r_lip_in, cz + r_lip_in), (-0.8, cy + r_lip_in, cz + r_lip_out),
             mat="wood_lit")
    rig.cube("bowl", "b_rim_bk",
             (-1.4, cy - r_lip_in, cz - r_lip_out), (-0.8, cy + r_lip_in, cz - r_lip_in),
             mat="wood_dark")


def part_lid(rig: Rig) -> None:
    """翻倒在右侧地面的圆木盒盖（Wooden Bowl Lid）：
    严格放置在 cx = 3.6, cz = 2.2 (x 范围 [1.8, 5.4], z 范围 [0.6, 3.8])。
    单轴绕 Z 轴倾斜 30°。
    """
    rig.bone("lid", (3.6, 0.4, 2.2))
    lx, ly, lz = 3.6, 0.4, 2.2
    org_l = (lx, ly, lz)
    rot_l = (0.0, 0.0, -30.0)

    # 1. 盖沿薄圆盘 (半径 1.8, 厚 0.4)
    rig.cube("lid", "lid_rim_x",
             (lx - 1.8, ly - 0.2, lz - 1.3), (lx + 1.8, ly + 0.2, lz + 1.3),
             rot=rot_l, org=org_l, mat="wood_base")
    rig.cube("lid", "lid_rim_z",
             (lx - 1.3, ly - 0.2, lz - 1.8), (lx + 1.3, ly + 0.2, lz + 1.8),
             rot=rot_l, org=org_l, mat="wood_base")

    # 2. 盖面微拱薄圆顶 (半径 1.3, 厚 0.3)
    rig.cube("lid", "lid_dome_x",
             (lx - 1.3, ly + 0.2, lz - 0.9), (lx + 1.3, ly + 0.5, lz + 0.9),
             rot=rot_l, org=org_l, mat="wood_lit")
    rig.cube("lid", "lid_dome_z",
             (lx - 0.9, ly + 0.2, lz - 1.3), (lx + 0.9, ly + 0.5, lz + 1.3),
             rot=rot_l, org=org_l, mat="wood_lit")

    # 3. 盖纽小提手
    rig.cube("lid", "lid_knob",
             (lx - 0.3, ly + 0.5, lz - 0.3), (lx + 0.3, ly + 0.9, lz + 0.3),
             rot=rot_l, org=org_l, mat="wood_dark")


def part_pills(rig: Rig) -> None:
    """鲜红散落的朱砂活血丹丸。
    严格安放在地面开阔区 (x: -0.6 ~ 1.4, z: 1.0 ~ 3.6)：
    与左侧木盒 (x <= -0.8)、右侧木盖 (x >= 1.8) 完全净空，零穿模！
    """
    rig.bone("pills", (0.0, 0.0, 0.0))

    r_main = 0.92
    r_sub = 0.82

    # 1. 刚刚滚出盒口的一颗丹药 (x = -0.3, y = 0.85, z = 2.2)
    add_smooth_pill(rig, "pills", "p_exit", cx=-0.3, cy=0.85, cz=2.2, r=r_sub)

    # 2. 滚落在正前方地面的主丹（最前、最亮、第一视觉焦点！x = 0.6, y = 0.92, z = 3.3）
    add_smooth_pill(rig, "pills", "p_front_main", cx=0.6, cy=0.92, cz=3.3, r=r_main)

    # 3. 散落在右前方的一颗 (x = 1.0, y = 0.82, z = 2.0)
    add_smooth_pill(rig, "pills", "p_out_r1", cx=1.0, cy=0.82, cz=2.0, r=r_sub)

    # 4. 中间偏后的一颗 (x = 0.3, y = 0.82, z = 1.1)
    add_smooth_pill(rig, "pills", "p_out_mid", cx=0.3, cy=0.82, cz=1.1, r=r_sub)


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_sack(rig)
    part_bowl(rig)
    part_lid(rig)
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
