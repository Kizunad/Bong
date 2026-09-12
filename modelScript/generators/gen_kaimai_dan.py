#!/usr/bin/env python3
"""开脉丹（KaimaiDan）Blockbench .bbmodel 生成器。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/kaimai_dan.png`
  `server/assets/items/pills.toml` (id = "kaimai_dan", name = "开脉丹")

原画构图特征深度还原：
  1. 紧裹丹丸的古朴药用油纸兜（Kraft Paper Pouch）：
     - 结构呈明显的【前低后高】漏斗花苞状！
     - 前方（Front）：纸沿低垂（兜住丹球下部），露出丹球的大半个饱满浑圆身躯。
     - 后方（Back）：原画最灵魂的大特征——**高高耸立而起的巨大后纸尖（Back Peak）**，像椅背一样直刺上方，高度甚至超越丹药顶峰！
     - 左右（Left/Right）：各有揉捏折出的起伏纸瓣与侧向尖角，自然拢抱。
  2. 极品圆硕开脉大丹（Grand Spherical Pill）：
     - 硕大浑圆的琥珀深棕色丹丸，体量饱满（直径约 6.5px），安坐于纸窝中，受光面温润圆滑。

用法：
  python3 modelScript/generators/gen_kaimai_dan.py
  bbmodel-render modelScript/models/KaimaiDan.bbmodel
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.rig.rigkit import Rig
from bbmodel_maker import workspace

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

# ── 材质色板 ──────────────────────────────────────────────────────────
# 严格提取自 kaimai_dan.png 原画：
MATS = {
    "paper_lit":      (232, 206, 162),  # 油纸向阳高光面与翻角边缘
    "paper_mid":      (196, 166, 124),  # 油纸受光中过渡面
    "paper_dark":     (152, 120, 82),   # 油纸折痕深处与背光
    "paper_shadow":   (110, 82, 52),    # 油纸最底层窝底阴影
    "pill_base":      (140, 76, 42),    # 开脉丹琥珀深棕主皮
    "pill_lit":       (192, 122, 74),   # 丹丸顶部与向阳高光
    "pill_shadow":    (84, 40, 20),     # 丹丸底部与深背光面
}


def add_round_box(rig: Rig, bone: str, prefix: str,
                  y0: float, y1: float, r: float, chamfer: float,
                  mat_m: str = "pill_base", mat_s: str = "pill_base",
                  cx: float = 0.0, cz: float = 0.0):
    """十字平滑圆柱层。"""
    rig.cube(bone, f"{prefix}_x",
             (cx - r, y0, cz - (r - chamfer)),
             (cx + r, y1, cz + (r - chamfer)),
             mat=mat_m)
    rig.cube(bone, f"{prefix}_z",
             (cx - (r - chamfer), y0, cz - r),
             (cx + (r - chamfer), y1, cz + r),
             mat=mat_s)


def add_true_sphere(rig: Rig, bone: str, prefix: str,
                    cx: float, cy: float, cz: float, r: float):
    """9 层精细渐变球体逼近算法，打造极致饱满圆润的丹丸。"""
    cuts = [
        ("p0_bot", -r * 1.00, -r * 0.82, r * 0.48, r * 0.12, "pill_shadow", "pill_shadow"),
        ("p1_d2",  -r * 0.82, -r * 0.58, r * 0.74, r * 0.18, "pill_shadow", "pill_shadow"),
        ("p2_d1",  -r * 0.58, -r * 0.32, r * 0.90, r * 0.24, "pill_shadow", "pill_base"),
        ("p3_md",  -r * 0.32, -r * 0.08, r * 0.99, r * 0.28, "pill_base",   "pill_base"),
        ("p4_mu",  -r * 0.08,  r * 0.20, r * 1.00, r * 0.28, "pill_lit",    "pill_base"),
        ("p5_u1",   r * 0.20,  r * 0.50, r * 0.94, r * 0.26, "pill_lit",    "pill_lit"),
        ("p6_u2",   r * 0.50,  r * 0.75, r * 0.80, r * 0.22, "pill_lit",    "pill_lit"),
        ("p7_u3",   r * 0.75,  r * 0.92, r * 0.58, r * 0.16, "pill_lit",    "pill_lit"),
        ("p8_top",  r * 0.92,  r * 1.00, r * 0.32, r * 0.08, "pill_lit",    "pill_lit"),
    ]

    for tag, dy0, dy1, rad, c, mm, ms in cuts:
        y0 = cy + dy0
        y1 = cy + dy1
        add_round_box(rig, bone, f"{prefix}_{tag}", y0, y1, rad, c, mm, ms, cx, cz)


def part_paper(rig: Rig) -> None:
    """油纸漏斗花苞兜（严格呈现原画【前低后高、背后高耸大尖角】的特征）：
    - 贴地小基底 (y: 0~0.8)
    - 紧裹丹腹的下兜壁 (y: 0.8~2.5)
    - 前领低伏 (y: 2.5~3.6，前倾微外翻，露出饱满丹腹)
    - 左右舒展侧瓣 (y: 2.5~6.0，斜向上翻折)
    - **后方极高耸立的大背角纸尖**（y 直插 9.2，高耸背屏，原画核心标志！）
    """
    rig.bone("paper", (0.0, 0.0, 0.0))

    # 1. 贴地纸座基 (y: 0.0 ~ 0.8)
    add_round_box(rig, "paper", "base_bottom", 0.0, 0.8, r=2.6, chamfer=0.6,
                  mat_m="paper_shadow", mat_s="paper_shadow")

    # 2. 紧裹丹丸底部的下兜身 (y: 0.8 ~ 2.4, 半径 3.4)
    add_round_box(rig, "paper", "cup_low", 0.8, 2.4, r=3.5, chamfer=0.9,
                  mat_m="paper_dark", mat_s="paper_shadow")

    # 3. 前方低伏卷领（Front Collar，前倾外翻，兜住下腹露出丹丸）
    # 正前下唇
    rig.cube("paper", "front_lip",
             (-2.6, 2.2, 2.8), (2.6, 3.4, 3.8),
             rot=(-24.0, 0.0, 0.0), org=(0.0, 2.2, 2.8),
             mat="paper_lit")
    # 前左微翘小纸褶
    rig.cube("paper", "front_sw_fold",
             (-3.4, 2.4, 2.4), (-2.0, 3.8, 3.6),
             rot=(-18.0, -30.0, 15.0), org=(-2.4, 2.4, 2.6),
             mat="paper_mid")
    # 前右微翘小纸褶
    rig.cube("paper", "front_se_fold",
             (2.0, 2.4, 2.4), (3.4, 3.8, 3.6),
             rot=(-18.0, 30.0, -15.0), org=(2.4, 2.4, 2.6),
             mat="paper_lit")

    # 4. 左右侧面向上挺立的花瓣纸折
    # 左侧向外斜上方舒展 (倾斜约 35°，高至 y≈6.0)
    rig.cube("paper", "side_left_main",
             (-4.4, 2.2, -1.8), (-3.2, 5.2, 2.0),
             rot=(0.0, 0.0, 26.0), org=(-3.2, 2.2, 0.0),
             mat="paper_mid")
    rig.cube("paper", "side_left_tip",
             (-4.8, 5.0, -1.0), (-3.6, 6.8, 1.4),
             rot=(0.0, 0.0, 32.0), org=(-3.6, 5.0, 0.0),
             mat="paper_mid")

    # 右侧向外斜上方舒展（受光亮面）
    rig.cube("paper", "side_right_main",
             (3.2, 2.2, -1.8), (4.4, 5.2, 2.0),
             rot=(0.0, 0.0, -26.0), org=(3.2, 2.2, 0.0),
             mat="paper_lit")
    rig.cube("paper", "side_right_tip",
             (3.6, 5.0, -1.0), (4.8, 6.8, 1.4),
             rot=(0.0, 0.0, -32.0), org=(3.6, 5.0, 0.0),
             mat="paper_lit")

    # 5. 原画最高辨识度灵魂：【后方高耸大背屏纸尖】（Back Peak）
    # 呈高挺折角从背后斜刺冲天，高度从 y=2.4 一路耸立到 y=9.2（比丹顶还高！）
    # 后背主纸身 (下段，贴背挺立)
    rig.cube("paper", "back_wall_main",
             (-3.2, 2.2, -4.0), (3.2, 5.8, -3.0),
             rot=(14.0, 0.0, 0.0), org=(0.0, 2.2, -3.0),
             mat="paper_dark")

    # 后背中段向后仰的大纸折面
    rig.cube("paper", "back_wall_mid",
             (-2.6, 5.5, -4.6), (2.6, 7.8, -3.4),
             rot=(20.0, 0.0, 0.0), org=(0.0, 5.5, -3.4),
             mat="paper_dark")

    # 后背正中央直冲云霄的大纸尖（原画正后方高高翘立的大折角尖顶！）
    rig.cube("paper", "back_peak_crown",
             (-1.6, 7.5, -5.2), (1.6, 9.4, -3.8),
             rot=(25.0, 0.0, 0.0), org=(0.0, 7.5, -3.8),
             mat="paper_dark")

    # 后左斜向高尖角
    rig.cube("paper", "back_nw_peak",
             (-3.4, 5.0, -4.4), (-1.8, 8.0, -3.0),
             rot=(20.0, 30.0, 18.0), org=(-2.2, 5.0, -3.2),
             mat="paper_dark")
    # 后右斜向高尖角
    rig.cube("paper", "back_ne_peak",
             (1.8, 5.0, -4.4), (3.4, 8.0, -3.0),
             rot=(20.0, -30.0, -18.0), org=(2.2, 5.0, -3.2),
             mat="paper_mid")


def part_pill(rig: Rig) -> None:
    """正中央开脉大丹：
    大体量浑圆宝丹（半径 3.3px，直径 6.6px），圆润饱满地坐于纸窝正中。
    中心位于 (0, 3.8, 0.2)，前视饱满露出，后视被高耸背屏完美托衬！
    """
    rig.bone("pill", (0.0, 3.8, 0.2))
    add_true_sphere(rig, "pill", "grand_dan", cx=0.0, cy=3.8, cz=0.2, r=3.3)


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_paper(rig)
    part_pill(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="开脉丹 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "KaimaiDan.bbmodel", help="输出路径")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = build_rig()
    bb_json = rig.bbmodel("KaimaiDan")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
