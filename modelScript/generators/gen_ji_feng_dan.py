#!/usr/bin/env python3
"""疾风丹（JiFengDan）Blockbench .bbmodel 生成器。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/ji_feng_dan.png`
  `server/assets/items/pills.toml` (id = "ji_feng_dan", name = "疾风丹")

原画特征深度还原：
  1. 青翠灵草叶托（Herb Leaf Pouch/Wrapper）：
     - 由数片宽大青翠灵叶折裹而成漏斗状花苞包裹。
     - 下部收束，紧扎一道草青色绳箍，右下方带着打结绳结与自然飘散的绳尾。
     - 叶片向上向外舒展绽放：前叶低伏露出丹丸，后叶高耸如屏障，左右舒展翻卷，叶缘与叶尖层次错落。
  2. 满盛疾风灵丹（Breeze Green Pills）：
     - 叶兜中饱满堆叠 5~6 颗青绿/碧玉色泽的疾风丹丸。
     - 顶峰主丹高耸饱满，周围环绕簇拥丹药，尽显风系灵动轻盈之感。

用法：
  python3 modelScript/generators/gen_ji_feng_dan.py
  bbmodel-render modelScript/models/JiFengDan.bbmodel
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
# 提取自 ji_feng_dan.png：
# 灵叶：受光嫩绿 (154, 186, 92)、叶面中绿 (106, 142, 62)、叶背暗绿 (62, 94, 40)、深褶痕 (42, 66, 28)
# 草绳：淡青黄绳 (166, 178, 102)
# 疾风丹：青绿药皮 (148, 172, 114)、向阳亮碧 (188, 208, 146)、阴面墨绿 (84, 112, 68)
MATS = {
    "leaf_lit":       (154, 186, 92),   # 灵叶向光嫩绿
    "leaf_mid":       (106, 142, 62),   # 灵叶主身翠绿
    "leaf_dark":      (62, 94, 40),     # 灵叶背光与内侧暗绿
    "leaf_shadow":    (42, 66, 28),     # 叶兜底部深折痕
    "cord_grass":     (166, 178, 102),  # 扎紧叶托的草青扎绳
    "pill_base":      (148, 172, 114),  # 疾风丹主体青碧色
    "pill_lit":       (192, 212, 150),  # 疾风丹受光亮绿色
    "pill_shadow":    (84, 112, 68),    # 疾风丹背阴沉绿色
}


def add_round_box(rig: Rig, bone: str, prefix: str,
                  y0: float, y1: float, r: float, chamfer: float,
                  mat_m: str = "leaf_mid", mat_s: str = "leaf_lit",
                  cx: float = 0.0, cz: float = 0.0):
    """构建带倒角切角的平滑圆角层。"""
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
    """构建饱满圆润的青碧疾风丹丸。"""
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


def part_leaves(rig: Rig) -> None:
    """青翠灵草叶托兜：
    底柄束 -> 草绳箍与打结绳须 -> 展开的叶身碗筒 -> 错落耸立的灵叶尖瓣。
    """
    rig.bone("leaves", (0.0, 0.0, 0.0))

    # 1. 最底部的叶柄聚束底座 (y: 0.0 ~ 1.0)
    add_round_box(rig, "leaves", "stem_base", 0.0, 1.0, r=2.2, chamfer=0.6,
                  mat_m="leaf_shadow", mat_s="leaf_dark")

    # 2. 草绳束箍 (y: 0.9 ~ 1.6) - 勒紧叶兜的青黄色扎绳
    add_round_box(rig, "leaves", "cord_ring", 0.9, 1.6, r=2.5, chamfer=0.6,
                  mat_m="cord_grass", mat_s="cord_grass")

    # 扎绳绳结与微翘的绳尾 (朝向右下方 +X, +Z)
    rig.cube("leaves", "cord_knot",
             (1.9, 0.8, 0.0), (2.8, 1.8, 0.9),
             mat="cord_grass")
    rig.cube("leaves", "cord_tail",
             (2.2, 0.2, 0.4), (2.7, 1.2, 1.0),
             rot=(0.0, 0.0, -20.0), org=(2.2, 1.2, 0.7),
             mat="cord_grass")

    # 3. 绳上叶片向上绽开的圆筒下段 (y: 1.6 ~ 3.4)
    add_round_box(rig, "leaves", "leaf_tube_low", 1.6, 3.4, r=3.4, chamfer=0.8,
                  mat_m="leaf_dark", mat_s="leaf_mid")

    # 4. 叶筒中段鼓腹与承丹底 (y: 3.4 ~ 5.0, r=4.0)
    add_round_box(rig, "leaves", "leaf_tube_mid", 3.4, 5.0, r=4.0, chamfer=1.0,
                  mat_m="leaf_mid", mat_s="leaf_lit")

    # 5. 原画精髓：错落耸立展开的四方灵叶大尖瓣
    # 前叶瓣 (+Z)：前领低伏外翻，露出饱满丹丸
    rig.cube("leaves", "leaf_front_main",
             (-2.6, 4.6, 3.4), (2.6, 7.2, 4.0),
             rot=(-20.0, 0.0, 0.0), org=(0.0, 4.6, 3.4),
             mat="leaf_lit")
    rig.cube("leaves", "leaf_front_tip",
             (-1.4, 6.8, 3.6), (1.4, 8.4, 4.1),
             rot=(-26.0, 0.0, 0.0), org=(0.0, 6.8, 3.6),
             mat="leaf_lit")

    # 后叶瓣 (-Z)：后方高高耸立的拱形大叶屏障（直达 y≈10.2！）
    rig.cube("leaves", "leaf_back_main",
             (-2.8, 4.6, -4.0), (2.8, 7.8, -3.4),
             rot=(16.0, 0.0, 0.0), org=(0.0, 4.6, -3.4),
             mat="leaf_dark")
    rig.cube("leaves", "leaf_back_tip",
             (-1.6, 7.6, -4.1), (1.6, 10.0, -3.5),
             rot=(22.0, 0.0, 0.0), org=(0.0, 7.6, -3.5),
             mat="leaf_dark")

    # 左叶瓣 (-X)：向左上方翻展
    rig.cube("leaves", "leaf_left_main",
             (-4.0, 4.6, -2.6), (-3.4, 7.4, 2.6),
             rot=(0.0, 0.0, 18.0), org=(-3.4, 4.6, 0.0),
             mat="leaf_mid")
    rig.cube("leaves", "leaf_left_tip",
             (-4.1, 7.1, -1.4), (-3.5, 8.8, 1.4),
             rot=(0.0, 0.0, 24.0), org=(-3.5, 7.1, 0.0),
             mat="leaf_mid")

    # 右叶瓣 (+X)：向右上方翻展
    rig.cube("leaves", "leaf_right_main",
             (3.4, 4.6, -2.6), (4.0, 7.4, 2.6),
             rot=(0.0, 0.0, -18.0), org=(3.4, 4.6, 0.0),
             mat="leaf_lit")
    rig.cube("leaves", "leaf_right_tip",
             (3.5, 7.1, -1.4), (4.1, 8.8, 1.4),
             rot=(0.0, 0.0, -24.0), org=(3.5, 7.1, 0.0),
             mat="leaf_lit")


def part_pills(rig: Rig) -> None:
    """叶兜中盛满的青碧疾风丹丸。
    中央顶峰主丹耸立，周围 4 颗丹丸团聚倚叶。
    """
    rig.bone("pills", (0.0, 5.0, 0.0))

    r_main = 1.35
    r_sub = 1.20

    # 1. 顶峰核心主丹（青翠饱满，向阳受光亮丽）
    add_smooth_pill(rig, "pills", "p_main", cx=0.0, cy=6.8, cz=0.1, r=r_main)

    # 2. 前排左侧丹丸
    add_smooth_pill(rig, "pills", "p_fl", cx=-1.1, cy=5.8, cz=1.25, r=r_sub)

    # 3. 前排右侧丹丸
    add_smooth_pill(rig, "pills", "p_fr", cx=1.2, cy=5.9, cz=1.1, r=r_sub)

    # 4. 后排左侧丹丸
    add_smooth_pill(rig, "pills", "p_bl", cx=-1.2, cy=6.0, cz=-1.1, r=r_sub)

    # 5. 后排右侧丹丸
    add_smooth_pill(rig, "pills", "p_br", cx=1.1, cy=6.1, cz=-1.2, r=r_sub)


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_leaves(rig)
    part_pills(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="疾风丹 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "JiFengDan.bbmodel", help="输出路径")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = build_rig()
    bb_json = rig.bbmodel("JiFengDan")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
