#!/usr/bin/env python3
"""抗灵压丹（AntiSpiritPressurePill）Blockbench .bbmodel 生成器。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/anti_spirit_pressure_pill.png`
  `server/assets/items/pills.toml` (id = "anti_spirit_pressure_pill", name = "抗灵压丹")

原画特征分析：
  1. 高足斑驳玄铁/古青铜药钵（Aged Bronze/Black Iron Chalice Bowl）：
     - 极具古典厚重感的宽口药钵，带高高收腰的喇叭状高足底座（Pedestal Foot）。
     - 腹部下窄上宽、大弧形外展，口沿明显向外翻卷形成平阔的外撇厚唇圈（Flared Rim）。
     - 材质呈现黑褐色古铁皮壳，受光面带暗铜褐微光。
  2. 满盛玄黑抗灵压大丹（Dense Obsidian Pills）：
     - 钵中堆满深沉如墨、致密沉重的玄黑色抗灵压丹药（带深灰青暗光与微弱高光）。
     - 呈自然金字塔堆积：中央 1 颗主丹耸立，周围簇拥 5 颗丹丸，下层深沉，气度森严沉稳。

结构分层：
  - part_chalice: 高足青铜药钵（喇叭圈足、束腰颈、圆鼓腹身、外撇厚唇、内腔暗底）
  - part_pills: 满盛簇拥堆叠的玄墨黑丹丸

用法：
  python3 modelScript/generators/gen_anti_spirit_pressure_pill.py
  bbmodel-render modelScript/models/AntiSpiritPressurePill.bbmodel
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
# 严格提取自 anti_spirit_pressure_pill.png：
# 古铜铁钵：玄铁黑褐 (54, 44, 38)、铜锈微光 (86, 70, 58)、受光古铜 (118, 96, 78)、深阴暗部 (34, 28, 24)
# 抗灵压丹：玄黑丹皮 (42, 40, 44)、向光深铅灰 (72, 70, 76)、极深黑底 (24, 22, 26)
MATS = {
    "chalice_base":   (54, 44, 38),     # 古铜钵主体深黑褐
    "chalice_lit":    (118, 96, 78),    # 钵口向光古铜面
    "chalice_mid":    (86, 70, 58),     # 钵身过渡暗铜色
    "chalice_dark":   (34, 28, 24),     # 钵底足与内壁暗部
    "pill_base":      (42, 40, 44),     # 抗灵压丹玄黑皮壳
    "pill_lit":       (72, 70, 76),     # 丹丸向光深铅灰
    "pill_shadow":    (24, 22, 26),     # 丹丸阴影墨黑色
}


def add_round_box(rig: Rig, bone: str, prefix: str,
                  y0: float, y1: float, r: float, chamfer: float,
                  mat_m: str = "chalice_base", mat_s: str = "chalice_lit",
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
    """构建饱满致密的玄黑抗灵压丹丸。"""
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


def part_chalice(rig: Rig) -> None:
    """高足古铜药钵：
    喇叭状高圈足 -> 束腰细颈 -> 优雅大弧度展开腹部 -> 外翻平撇厚唇沿。
    """
    rig.bone("chalice", (0.0, 0.0, 0.0))

    # 1. 喇叭高足底盘 (y: 0.0 ~ 0.8, r=3.0) - 稳重外撇的底盘
    add_round_box(rig, "chalice", "foot_base", 0.0, 0.8, r=3.0, chamfer=0.8,
                  mat_m="chalice_dark", mat_s="chalice_base")

    # 2. 高足收束腰圈 (y: 0.8 ~ 1.8, r=2.2) - 优雅收腰
    add_round_box(rig, "chalice", "foot_waist", 0.8, 1.8, r=2.2, chamfer=0.6,
                  mat_m="chalice_dark", mat_s="chalice_dark")

    # 3. 钵身下腹向上展开 (y: 1.8 ~ 3.2, r=3.6)
    add_round_box(rig, "chalice", "belly_low", 1.8, 3.2, r=3.6, chamfer=0.9,
                  mat_m="chalice_base", mat_s="chalice_mid")

    # 4. 钵身中腹饱满展弧 (y: 3.2 ~ 5.0, r=4.6) - 大弧度外展
    add_round_box(rig, "chalice", "belly_mid", 3.2, 5.0, r=4.6, chamfer=1.2,
                  mat_m="chalice_base", mat_s="chalice_lit")

    # 5. 钵内深底暗衬 (y: 4.2 ~ 4.8)
    rig.cube("chalice", "inner_bed",
             (-3.2, 4.2, -3.2), (3.2, 4.8, 3.2),
             mat="chalice_dark")

    # 6. 外翻平撇厚唇沿 (y: 5.0 ~ 6.0, r_out=5.1, r_in=3.6) - 原画鲜明的外撇大宽沿！
    r_out, r_in = 5.1, 3.6
    hw = r_out * 0.72
    rig.cube("chalice", "rim_f",
             (-hw, 5.0, r_in), (hw, 6.0, r_out),
             mat="chalice_lit")
    rig.cube("chalice", "rim_b",
             (-hw, 5.0, -r_out), (hw, 6.0, -r_in),
             mat="chalice_base")
    rig.cube("chalice", "rim_l",
             (-r_out, 5.0, -hw), (-r_in, 6.0, hw),
             mat="chalice_mid")
    rig.cube("chalice", "rim_r",
             (r_in, 5.0, -hw), (r_out, 6.0, hw),
             mat="chalice_lit")


def part_pills(rig: Rig) -> None:
    """满盛的玄墨色抗灵压丹丸。
    中央顶峰主丹高高隆起，周围 5 颗丹药紧凑簇拥堆满宽大钵口！
    """
    rig.bone("pills", (0.0, 5.0, 0.0))

    r_main = 1.45
    r_sub = 1.25

    # 1. 核心高耸主丹（深邃玄黑，高高耸立居中，第一视觉中心）
    add_smooth_pill(rig, "pills", "p_main", cx=0.0, cy=7.0, cz=0.1, r=r_main)

    # 2. 前侧偏左丹丸
    add_smooth_pill(rig, "pills", "p_fl", cx=-1.25, cy=5.9, cz=1.35, r=r_sub)

    # 3. 前侧偏右丹丸
    add_smooth_pill(rig, "pills", "p_fr", cx=1.25, cy=5.9, cz=1.35, r=r_sub)

    # 4. 后侧偏左丹丸
    add_smooth_pill(rig, "pills", "p_bl", cx=-1.35, cy=6.1, cz=-1.1, r=r_sub)

    # 5. 后侧偏右丹丸
    add_smooth_pill(rig, "pills", "p_br", cx=1.35, cy=6.1, cz=-1.1, r=r_sub)

    # 6. 正后侧靠垫丹丸 (形成紧凑环抱簇拥群)
    add_smooth_pill(rig, "pills", "p_bm", cx=0.0, cy=6.2, cz=-1.5, r=1.18)


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_chalice(rig)
    part_pills(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="抗灵压丹 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "AntiSpiritPressurePill.bbmodel", help="输出路径")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = build_rig()
    bb_json = rig.bbmodel("AntiSpiritPressurePill")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
