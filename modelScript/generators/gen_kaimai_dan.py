#!/usr/bin/env python3
"""开脉丹（KaimaiDan）Blockbench .bbmodel 生成器。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/kaimai_dan.png`
  `server/assets/items/pills.toml` (id = "kaimai_dan", name = "开脉丹")

原画特征还原：
  1. 药用折纸花托（Kraft Paper Cup）：
     - 四方微敞包拢的油纸托杯，下部紧贴丹丸，上部自然向外舒展。
     - 四角带向上翻折高挑的折角纸尖。
  2. 极品圆硕开脉大丹（Spherical Grand Dan）：
     - 采用分层十字平滑体素逼近真圆球，顶峰柔和收敛，消除方块平台。
     - 药皮为古朴温润的琥珀红褐色调。

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

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.rig.rigkit import Rig
from bbmodel_maker import workspace

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

PX = 16.0

# ── 材质色板 ──────────────────────────────────────────────────────────
# 严格提取自 kaimai_dan.png 图标像素：
MATS = {
    "paper_lit":      (232, 208, 166),  # 油纸高光边缘与翻角
    "paper_mid":      (196, 168, 126),  # 油纸中过渡面
    "paper_dark":     (156, 124, 88),   # 油纸折痕暗部
    "paper_shadow":   (116, 88, 58),    # 油纸深底与内凹夹缝
    "pill_base":      (142, 80, 46),    # 丹丸主体琥珀褐
    "pill_lit":       (196, 126, 78),   # 丹丸向阳高光面
    "pill_shadow":    (88, 44, 24),     # 丹丸深沉底色
}


def add_round_box(rig: Rig, bone: str, prefix: str,
                  y0: float, y1: float, r: float, chamfer: float,
                  mat_m: str = "pill_base", mat_s: str = "pill_base",
                  cx: float = 0.0, cz: float = 0.0):
    """构建带倒角切角的体素圆柱层（十字平滑块）。"""
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
    """9 层精细渐变球体逼近算法。"""
    cuts = [
        ("p0_bot", -r * 1.00, -r * 0.82, r * 0.46, r * 0.12, "pill_shadow", "pill_shadow"),
        ("p1_d2",  -r * 0.82, -r * 0.60, r * 0.72, r * 0.18, "pill_shadow", "pill_shadow"),
        ("p2_d1",  -r * 0.60, -r * 0.35, r * 0.88, r * 0.24, "pill_shadow", "pill_base"),
        ("p3_md",  -r * 0.35, -r * 0.10, r * 0.98, r * 0.28, "pill_base",   "pill_base"),
        ("p4_mu",  -r * 0.10,  r * 0.20, r * 1.00, r * 0.28, "pill_lit",    "pill_base"),
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
    """高耸贴身包裹的折纸花托：
    碗状纸基 + 四方陡峭向上收拢的花瓣纸壁 + 四角高耸出挑的大褶角。
    """
    rig.bone("paper", (0.0, 0.0, 0.0))

    # 1. 贴地纸座 (y: 0.0 ~ 0.8)
    add_round_box(rig, "paper", "base_pad", 0.0, 0.8, r=2.6, chamfer=0.6,
                  mat_m="paper_shadow", mat_s="paper_shadow")

    # 2. 下层环形托壁 (y: 0.8 ~ 2.4, 贴住丹丸下部)
    add_round_box(rig, "paper", "cup_low", 0.8, 2.4, r=3.4, chamfer=0.8,
                  mat_m="paper_dark", mat_s="paper_shadow")

    # 3. 四面陡峭向上挺立的侧壁花瓣 (紧贴丹药，向上挺拔)
    rig.cube("paper", "petal_south",
             (-2.6, 2.2, 3.1), (2.6, 5.0, 3.7),
             rot=(-20.0, 0.0, 0.0), org=(0.0, 2.2, 3.1),
             mat="paper_lit")
    rig.cube("paper", "petal_north",
             (-2.6, 2.2, -3.7), (2.6, 5.2, -3.1),
             rot=(22.0, 0.0, 0.0), org=(0.0, 2.2, -3.1),
             mat="paper_dark")
    rig.cube("paper", "petal_west",
             (-3.7, 2.2, -2.6), (-3.1, 5.0, 2.6),
             rot=(0.0, 0.0, 20.0), org=(-3.1, 2.2, 0.0),
             mat="paper_mid")
    rig.cube("paper", "petal_east",
             (3.1, 2.2, -2.6), (3.7, 5.0, 2.6),
             rot=(0.0, 0.0, -20.0), org=(3.1, 2.2, 0.0),
             mat="paper_lit")

    # 4. 原画最关键的四个高挑纸角褶皱
    rig.cube("paper", "corner_se_main",
             (2.2, 2.2, 2.2), (3.8, 5.8, 3.8),
             rot=(-15.0, 45.0, -18.0), org=(2.2, 2.2, 2.2),
             mat="paper_lit")
    rig.cube("paper", "corner_se_tip",
             (2.8, 5.5, 2.8), (4.2, 6.4, 4.2),
             rot=(-20.0, 45.0, -25.0), org=(2.8, 5.5, 2.8),
             mat="paper_lit")

    rig.cube("paper", "corner_ne_main",
             (2.2, 2.2, -3.8), (3.8, 5.8, -2.2),
             rot=(15.0, -45.0, -18.0), org=(2.2, 2.2, -2.2),
             mat="paper_mid")
    rig.cube("paper", "corner_ne_tip",
             (2.8, 5.5, -4.2), (4.2, 6.4, -2.8),
             rot=(20.0, -45.0, -25.0), org=(2.8, 5.5, -2.8),
             mat="paper_mid")

    rig.cube("paper", "corner_sw_main",
             (-3.8, 2.2, 2.2), (-2.2, 5.8, 3.8),
             rot=(-15.0, -45.0, 18.0), org=(-2.2, 2.2, 2.2),
             mat="paper_mid")
    rig.cube("paper", "corner_sw_tip",
             (-4.2, 5.5, 2.8), (-2.8, 6.4, 4.2),
             rot=(-20.0, -45.0, 25.0), org=(-2.8, 5.5, 2.8),
             mat="paper_mid")

    rig.cube("paper", "corner_nw_main",
             (-3.8, 2.2, -3.8), (-2.2, 6.2, -2.2),
             rot=(18.0, 45.0, 20.0), org=(-2.2, 2.2, -2.2),
             mat="paper_dark")
    rig.cube("paper", "corner_nw_tip",
             (-4.4, 6.0, -4.4), (-2.6, 7.0, -2.6),
             rot=(24.0, 45.0, 28.0), org=(-2.6, 6.0, -2.6),
             mat="paper_dark")


def part_pill(rig: Rig) -> None:
    """正中央开脉大丹：饱满浑圆的宝丹。"""
    rig.bone("pill", (0.0, 3.6, 0.0))
    add_true_sphere(rig, "pill", "grand_dan", cx=0.0, cy=3.6, cz=0.0, r=3.2)


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
