#!/usr/bin/env python3
"""金钟丹（JinZhongDan）Blockbench .bbmodel 生成器 - 叠放金丹群版。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/jin_zhong_dan.png`
  `server/assets/items/pills.toml` (id = "jin_zhong_dan", name = "金钟丹")

用户反馈需求：
  “做几个叠在一起吧”
  - 经典四面体紧密堆积的金丹塔群（Tetrahedral Golden Pill Cluster）：
    - 底部 3 颗饱满金钟大丹呈正三角形稳稳坐落于地面。
    - 顶端 1 颗主丹稳坐于三丹正中央的凹窝中，顶峰高耸，纯金高光耀眼。
    - 侧前方滚落 1 颗饱满伴生金丹，形成自然生动、满盅金光四溢的宝丹堆叠场景！

结构设计：
  - part_pills: 叠放在一起的 5 颗浑圆金钟宝丹

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
# 纯金宝丹渐变色系：
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


def add_smooth_golden_dan(rig: Rig, bone: str, prefix: str,
                          cx: float, cy: float, cz: float, r: float):
    """构建一颗多层高精度体素圆润金丹。"""
    rc = r * 0.76
    # 核心赤道层
    rig.cube(bone, f"{prefix}_mx",
             (cx - r, cy - rc * 0.42, cz - rc),
             (cx + r, cy + rc * 0.42, cz + rc),
             mat="gold_base")
    rig.cube(bone, f"{prefix}_mz",
             (cx - rc, cy - rc * 0.42, cz - r),
             (cx + rc, cy + rc * 0.42, cz + r),
             mat="gold_base")

    # 顶层向阳亮面与纯金高光
    rt = r * 0.62
    rig.cube(bone, f"{prefix}_top_mid",
             (cx - rt, cy + rc * 0.42, cz - rt),
             (cx + rt, cy + r * 0.85, cz + rt),
             mat="gold_lit")
    # 极顶高光点
    rs = r * 0.38
    rig.cube(bone, f"{prefix}_top_spec",
             (cx - rs, cy + r * 0.85, cz - rs),
             (cx + rs, cy + r, cz + rs),
             mat="gold_spec")

    # 底层阴影面
    rig.cube(bone, f"{prefix}_bot",
             (cx - rt, cy - r, cz - rt),
             (cx + rt, cy - rc * 0.42, cz + rt),
             mat="gold_shadow")


def part_pills(rig: Rig) -> None:
    """叠放的金钟宝丹群：
    - 底层 3 颗大金丹呈正三角环抱基底 (r = 1.9)
    - 顶端 1 颗核心金丹耸立在正中央金字塔顶端 (y = 4.8)
    - 前侧滚落 1 颗伴生金丹 (r = 1.6)，形成自然的生动散落感！
    """
    rig.bone("pills", (0.0, 0.0, 0.0))

    r = 1.95
    y_base = 1.95

    # 底部三角形顶点坐标 (外接圆半径 R = 2.3)
    # 丹 1: 正前方偏左 (-1.9, y_base, 1.1)
    add_smooth_golden_dan(rig, "pills", "p_base_fl", cx=-1.7, cy=y_base, cz=1.0, r=r)

    # 丹 2: 正前方偏右 (1.9, y_base, 1.1)
    add_smooth_golden_dan(rig, "pills", "p_base_fr", cx=1.7, cy=y_base, cz=1.0, r=r)

    # 丹 3: 正后方 (0.0, y_base, -1.9)
    add_smooth_golden_dan(rig, "pills", "p_base_back", cx=0.0, cy=y_base, cz=-1.9, r=r)

    # 顶层核心主丹：安稳压在三丹正中央金字塔顶，顶峰高达 y=7.0！
    add_smooth_golden_dan(rig, "pills", "p_top_main", cx=0.0, cy=4.75, cz=0.0, r=2.05)

    # 前方散落滚出的第 5 颗金丹（打破僵硬对称，原画活泼感）
    add_smooth_golden_dan(rig, "pills", "p_out_front", cx=0.5, cy=1.5, cz=3.4, r=1.50)


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_pills(rig)
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
