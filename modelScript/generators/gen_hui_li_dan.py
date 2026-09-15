#!/usr/bin/env python3
"""回力丹（HuiLiDan）Blockbench .bbmodel 生成器。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/hui_li_dan.png`
  `server/assets/items/pills.toml` (id = "hui_li_dan", name = "回力丹")

原画构图特征还原：
  1. 深褐色圆木药钵（Dark Wooden Herb Bowl）：
     - 位于中后方，圆腹厚沿，中空内腔。
  2. 满盛的金黄色回力丹丸（Golden Energy Pills）：
     - 钵中堆满圆润饱满、充满阳刚劲力的金黄色丹药。
     - 顶峰 1 颗主丹耸立，周围簇拥 5 颗金丹。
  3. 左侧倚靠的百年老参（Ginseng Root）：
     - 位于药钵左前方，粗壮纺锤形主参体、参头芦头、向下向左前蜿蜒盘曲的细长参须。
  4. 右侧伴生舒展的人参复叶（Ginseng Leaves）：
     - 位于右侧地面，青翠复叶向外舒展。

用法：
  python3 modelScript/generators/gen_hui_li_dan.py
  bbmodel-render modelScript/models/HuiLiDan.bbmodel
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
# 严格提取自 hui_li_dan.png：
# 陶钵：深陶棕褐 (108, 68, 48)、受光木色 (142, 94, 68)、内壁暗部 (66, 42, 30)
# 回力丹：金黄丹皮 (228, 178, 54)、向光亮金 (252, 214, 88)、阴面暗金褐 (168, 122, 36)
# 人参：土黄泛白参肉 (222, 192, 134)、节环暗纹 (174, 142, 94)、参须 (198, 168, 114)
# 绿叶：鲜草绿 (102, 144, 68)、阴影绿 (62, 98, 44)
MATS = {
    "bowl_base":      (108, 68, 48),    # 陶钵主体深棕褐
    "bowl_lit":       (142, 94, 68),    # 陶钵受光面
    "bowl_dark":      (66, 42, 30),     # 陶钵暗底与内腔
    "pill_base":      (228, 178, 54),   # 回力丹金黄药皮
    "pill_lit":       (252, 214, 88),   # 回力丹向光亮金
    "pill_shadow":    (168, 122, 36),   # 回力丹阴影暗金褐
    "ginseng_body":   (222, 192, 134),  # 人参主根肉色
    "ginseng_dark":   (174, 142, 94),   # 人参节环暗纹
    "ginseng_root":   (198, 168, 114),  # 细长参须
    "leaf_lit":       (102, 144, 68),   # 人参叶面鲜绿
    "leaf_dark":      (62, 98, 44),     # 人参叶背暗绿
}


def add_round_box(rig: Rig, bone: str, prefix: str,
                  y0: float, y1: float, r: float, chamfer: float,
                  mat_m: str = "bowl_base", mat_s: str = "bowl_lit",
                  cx: float = 0.0, cz: float = 0.0):
    """构建平滑圆角层。"""
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
    """构建饱满圆润的金黄回力丹丸。"""
    rc = r * 0.76
    rig.cube(bone, f"{prefix}_mx",
             (cx - r, cy - rc * 0.42, cz - rc),
             (cx + r, cy + rc * 0.42, cz + rc),
             mat="pill_base")
    rig.cube(bone, f"{prefix}_mz",
             (cx - rc, cy - rc * 0.42, cz - r),
             (cx + rc, cy + rc * 0.42, cz + r),
             mat="pill_base")
    rt = r * 0.62
    rig.cube(bone, f"{prefix}_top",
             (cx - rt, cy + rc * 0.42, cz - rt),
             (cx + rt, cy + r, cz + rt),
             mat="pill_lit")
    rig.cube(bone, f"{prefix}_bot",
             (cx - rt, cy - r, cz - rt),
             (cx + rt, cy - rc * 0.42, cz + rt),
             mat="pill_shadow")


def part_bowl(rig: Rig) -> None:
    """中后部的深褐厚木/粗陶药钵（中心在 cx = 0.4, cz = -1.2）。"""
    rig.bone("bowl", (0.4, 0.0, -1.2))
    cx, cz = 0.4, -1.2

    # 1. 钵底圈足 (y: 0.0 ~ 0.8)
    add_round_box(rig, "bowl", "bowl_foot", 0.0, 0.8, r=2.8, chamfer=0.7,
                  mat_m="bowl_dark", mat_s="bowl_dark", cx=cx, cz=cz)

    # 2. 钵身下腹 (y: 0.8 ~ 2.4)
    add_round_box(rig, "bowl", "bowl_belly_low", 0.8, 2.4, r=3.9, chamfer=1.0,
                  mat_m="bowl_base", mat_s="bowl_lit", cx=cx, cz=cz)

    # 3. 钵身中腹最大鼓腹 (y: 2.4 ~ 4.6)
    add_round_box(rig, "bowl", "bowl_belly_mid", 2.4, 4.6, r=4.5, chamfer=1.2,
                  mat_m="bowl_base", mat_s="bowl_lit", cx=cx, cz=cz)

    # 4. 钵内衬底 (y: 4.0 ~ 4.6)
    rig.cube("bowl", "inner_bed",
             (cx - 3.0, 4.0, cz - 3.0), (cx + 3.0, 4.6, cz + 3.0),
             mat="bowl_dark")

    # 5. 厚实中空钵口沿 (y: 4.6 ~ 5.6, r_out=4.3, r_in=3.1)
    r_out, r_in = 4.3, 3.1
    hw = r_out * 0.72
    rig.cube("bowl", "rim_f",
             (cx - hw, 4.6, cz + r_in), (cx + hw, 5.6, cz + r_out),
             mat="bowl_lit")
    rig.cube("bowl", "rim_b",
             (cx - hw, 4.6, cz - r_out), (cx + hw, 5.6, cz - r_in),
             mat="bowl_dark")
    rig.cube("bowl", "rim_l",
             (cx - r_out, 4.6, cz - hw), (cx - r_in, 5.6, cz + hw),
             mat="bowl_base")
    rig.cube("bowl", "rim_r",
             (cx + r_in, 4.6, cz - hw), (cx + r_out, 5.6, cz + hw),
             mat="bowl_base")


def part_pills(rig: Rig) -> None:
    """满盛的金黄回力丹丸。
    中央顶峰高耸主丹，周围 4 颗丹丸紧凑环抱，粒粒金黄夺目！
    """
    rig.bone("pills", (0.4, 5.0, -1.2))
    cx, cz = 0.4, -1.2

    r_main = 1.35
    r_sub = 1.20

    # 1. 核心高耸主丹（耀眼金光，第一视觉中心）
    add_smooth_pill(rig, "pills", "p_main", cx=cx, cy=6.6, cz=cz, r=r_main)

    # 2. 前排偏左丹丸
    add_smooth_pill(rig, "pills", "p_fl", cx=cx - 1.15, cy=5.6, cz=cz + 1.25, r=r_sub)

    # 3. 前排偏右丹丸
    add_smooth_pill(rig, "pills", "p_fr", cx=cx + 1.20, cy=5.7, cz=cz + 1.15, r=r_sub)

    # 4. 后排偏左丹丸
    add_smooth_pill(rig, "pills", "p_bl", cx=cx - 1.20, cy=5.8, cz=cz - 1.15, r=r_sub)

    # 5. 后排偏右丹丸
    add_smooth_pill(rig, "pills", "p_br", cx=cx + 1.15, cy=5.9, cz=cz - 1.25, r=r_sub)


def part_ginseng(rig: Rig) -> None:
    """左侧倚靠的百年老参（芦头、粗壮主根、向下向前盘曲的细密参须）。
    位于 x ∈ [-5.8, -1.8], z ∈ [-1.0, 3.8]，完全与药钵分开。
    """
    rig.bone("ginseng", (-3.6, 0.0, 1.4))

    # 1. 人参主根（纺锤形斜卧，y: 1.0 ~ 4.4）
    # 参身下段
    rig.cube("ginseng", "gin_body_low",
             (-4.4, 1.0, 0.6), (-2.8, 2.4, 2.2),
             rot=(14.0, -12.0, -20.0), org=(-3.6, 1.7, 1.4),
             mat="ginseng_body")
    # 参身最粗中段
    rig.cube("ginseng", "gin_body_mid",
             (-4.6, 2.4, 0.4), (-2.6, 4.0, 2.4),
             rot=(14.0, -12.0, -20.0), org=(-3.6, 3.2, 1.4),
             mat="ginseng_body")
    # 参身环形横纹节（增加写实度）
    rig.cube("ginseng", "gin_ring",
             (-4.4, 2.8, 0.6), (-2.8, 3.3, 2.2),
             rot=(14.0, -12.0, -20.0), org=(-3.6, 3.0, 1.4),
             mat="ginseng_dark")
    # 参头芦头（向上渐细并带芦碗）
    rig.cube("ginseng", "gin_head",
             (-4.1, 4.0, 0.8), (-3.1, 5.0, 1.8),
             rot=(14.0, -12.0, -20.0), org=(-3.6, 4.5, 1.3),
             mat="ginseng_dark")

    # 2. 向前向下蜿蜒的大量细密参须
    # 主参须 A（向前下方蜿蜒延伸）
    rig.cube("ginseng", "root_a1",
             (-4.4, 0.2, 1.8), (-3.2, 0.8, 2.6),
             mat="ginseng_root")
    rig.cube("ginseng", "root_a2",
             (-4.8, 0.0, 2.6), (-3.4, 0.4, 3.8),
             mat="ginseng_root")
    # 侧参须 B（向左下方盘曲）
    rig.cube("ginseng", "root_b1",
             (-5.2, 0.2, 0.8), (-4.2, 0.8, 1.6),
             mat="ginseng_root")
    rig.cube("ginseng", "root_b2",
             (-6.0, 0.0, 1.2), (-5.0, 0.4, 2.2),
             mat="ginseng_root")


def part_leaves(rig: Rig) -> None:
    """右侧衬托的翠绿人参复叶枝条。
    位于 x ∈ [2.4, 6.0], z ∈ [0.8, 3.8]，完全与药钵分开。
    """
    rig.bone("leaves", (3.6, 0.0, 2.0))

    # 1. 细斜叶柄 (y: 0.2 ~ 1.8)
    rig.cube("leaves", "leaf_stem",
             (3.0, 0.2, 1.8), (3.6, 1.6, 2.4),
             rot=(-10.0, 20.0, 25.0), org=(3.3, 0.9, 2.1),
             mat="leaf_dark")

    # 2. 向上舒展的羽状绿叶 (向外向右前方舒展)
    rig.cube("leaves", "leaf_blade_1",
             (3.4, 1.4, 1.2), (5.6, 1.9, 2.8),
             rot=(-10.0, 25.0, 18.0), org=(4.5, 1.6, 2.0),
             mat="leaf_lit")
    rig.cube("leaves", "leaf_blade_2",
             (3.2, 1.2, 2.6), (5.2, 1.7, 4.0),
             rot=(-15.0, -10.0, 15.0), org=(4.2, 1.4, 3.3),
             mat="leaf_dark")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_bowl(rig)
    part_pills(rig)
    part_ginseng(rig)
    part_leaves(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="回力丹 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "HuiLiDan.bbmodel", help="输出路径")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = build_rig()
    bb_json = rig.bbmodel("HuiLiDan")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
