#!/usr/bin/env python3
"""金钟丹（JinZhongDan）Blockbench .bbmodel 生成器。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/jin_zhong_dan.png`
  `server/assets/items/pills.toml` (id = "jin_zhong_dan", name = "金钟丹")

原画特征分析：
  1. 金钟宝丹（Golden Bell Sphere）：
     - 纯净无暇、灿烂耀眼的浑圆真金宝丹！
     - 原画中无外罩容器或纸托，独此一枚浑圆硕大的金色大丹，光芒内敛温润，通体金光流动。
     - 顶部有强烈的暖白高光斑点与极度丝滑的金铜过渡，底部沉稳。
  2. 极致纯正的体素圆球解算（True Voxel Sphere）：
     - 直径 7.2px（体量丰硕浑厚）。
     - 采用多层渐变正交切片 + 向光受光层分色，打造具有真金质感的纯丹模型。

用法：
  python3 modelScript/generators/gen_jin_zhong_dan.py
  bbmodel-render modelScript/models/JinZhongDan.bbmodel
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
# 严格提取自 jin_zhong_dan.png 图标像素：
# 纯金宝丹：主体耀金 (224, 172, 42)、向阳亮金 (252, 218, 76)、极顶纯金高光 (255, 246, 154)、
#          侧阴金褐 (184, 134, 30)、底阴深铜金 (138, 96, 22)
MATS = {
    "gold_spec":      (255, 246, 154),  # 极顶受光纯金白高光
    "gold_lit":       (252, 218, 76),   # 向上受光亮金
    "gold_base":      (224, 172, 42),   # 金钟丹主体纯金
    "gold_mid":       (184, 134, 30),   # 侧阴金褐色
    "gold_shadow":    (138, 96, 22),    # 底部沉稳深铜金
}


def add_round_box(rig: Rig, bone: str, prefix: str,
                  y0: float, y1: float, r: float, chamfer: float,
                  mat_m: str = "gold_base", mat_s: str = "gold_mid",
                  cx: float = 0.0, cz: float = 0.0):
    """构建带倒角切角的体素圆层。"""
    rig.cube(bone, f"{prefix}_x",
             (cx - r, y0, cz - (r - chamfer)),
             (cx + r, y1, cz + (r - chamfer)),
             mat=mat_m)
    rig.cube(bone, f"{prefix}_z",
             (cx - (r - chamfer), y0, cz - r),
             (cx + (r - chamfer), y1, cz + r),
             mat=mat_s)


def part_pill(rig: Rig) -> None:
    """金钟丹浑圆金身：
    采用 9 层细致微切分层逼近真球体（半径 3.6px，直径 7.2px），
    坐落于 (0, 3.8, 0)，顶端达 y=7.4，底端落于 y=0.2。
    """
    rig.bone("pill", (0.0, 3.8, 0.0))
    cx, cy, cz = 0.0, 3.8, 0.0
    r = 3.6

    cuts = [
        # (tag, dy0, dy1, rad, c, mat_m, mat_s)
        ("p0_bot",  -r * 1.00, -r * 0.82, r * 0.48, r * 0.12, "gold_shadow", "gold_shadow"),
        ("p1_d2",   -r * 0.82, -r * 0.58, r * 0.74, r * 0.18, "gold_shadow", "gold_mid"),
        ("p2_d1",   -r * 0.58, -r * 0.32, r * 0.90, r * 0.24, "gold_mid",    "gold_base"),
        ("p3_md",   -r * 0.32, -r * 0.06, r * 0.99, r * 0.28, "gold_base",   "gold_base"),
        ("p4_mu",   -r * 0.06,  r * 0.22, r * 1.00, r * 0.28, "gold_lit",    "gold_base"),
        ("p5_u1",    r * 0.22,  r * 0.52, r * 0.94, r * 0.26, "gold_lit",    "gold_lit"),
        ("p6_u2",    r * 0.52,  r * 0.76, r * 0.80, r * 0.22, "gold_lit",    "gold_spec"),
        ("p7_u3",    r * 0.76,  r * 0.92, r * 0.58, r * 0.16, "gold_spec",   "gold_spec"),
        ("p8_top",   r * 0.92,  r * 1.00, r * 0.32, r * 0.08, "gold_spec",   "gold_spec"),
    ]

    for tag, dy0, dy1, rad, c, mm, ms in cuts:
        y0 = cy + dy0
        y1 = cy + dy1
        add_round_box(rig, "pill", f"golden_bell_{tag}", y0, y1, rad, c, mm, ms, cx, cz)


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_pill(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="金钟丹 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "JinZhongDan.bbmodel", help="输出路径")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = build_rig()
    bb_json = rig.bbmodel("JinZhongDan")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
