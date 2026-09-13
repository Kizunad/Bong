#!/usr/bin/env python3
"""固元丹（GuYuanPill）Blockbench .bbmodel 生成器。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/guyuan_pill.png`
  `server/assets/items/pills.toml` (id = "guyuan_pill", name = "固元丹")

原画特征分析（极具视觉张力的阵纹星核宝丹）：
  1. 赤炎玄黑丹壳（Magma Crust）：
     - 丹体如地火淬炼的星核，外层为深黑褐色碳化药皮，内蕴金红真元地火。
  2. 核心同心圆金光丹纹（Concentric Rune Rings & Orbits）：
     - 丹体正前方刻制着精密的同心圆灵力固元法阵（金光璀璨的圆环、轨道线与灵力节点）。
  3. 裂隙地火光芒（Glowing Veins）：
     - 丹皮表面纵横交错着金红色熔岩裂纹。
  4. 三才稳固堆叠布局（Triadic Base & Crown Dan）：
     - 结合用户打磨反馈（“做几个叠在一起”）：
       - 底层左右两颗玄赤伴生丹沉稳托底，形成“三才固元”之势；
       - 中央顶峰高耸主丹（直径 6.8px），正对正面视线，完整展示璀璨金光阵纹与裂痕！

结构设计：
  - part_base_pills: 底层稳固托底的两颗玄黑地火伴生丹
  - part_main_pill: 顶峰主丹身（多层圆球体素逼近真圆）
  - part_runes: 主丹正前方的同心金光阵纹环、环形轨道与灵力节点

用法：
  python3 modelScript/generators/gen_guyuan_pill.py
  bbmodel-render modelScript/models/GuYuanPill.bbmodel
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
# 严格提炼自 guyuan_pill.png 图标像素：
# 玄黑药皮：墨黑焦褐 (36, 16, 12)、深暗熔岩壳 (58, 24, 16)、中红褐 (108, 42, 22)
# 熔岩金红裂纹：赤红炽火 (214, 78, 28)、高亮金焰 (254, 186, 42)、纯金灵阵光 (255, 228, 88)
# 蜕变白斑/壳裂：骨白药蜕 (236, 230, 218)
MATS = {
    "crust_dark":     (36, 16, 12),     # 玄黑深焦药皮
    "crust_mid":      (68, 28, 18),     # 药皮深红褐过渡
    "crust_red":      (118, 48, 24),    # 药皮暗红受光
    "vein_fire":      (214, 78, 28),    # 裂隙赤红炽火
    "rune_gold":      (254, 186, 42),   # 阵纹灿金
    "rune_bright":    (255, 232, 96),   # 阵核极亮纯金光
    "crust_flake":    (228, 218, 204),  # 丹皮剥落露出的骨白药晕
}


def add_round_box(rig: Rig, bone: str, prefix: str,
                  y0: float, y1: float, r: float, chamfer: float,
                  mat_m: str = "crust_dark", mat_s: str = "crust_mid",
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
                    cx: float, cy: float, cz: float, r: float,
                    mat_body: str = "crust_dark", mat_lit: str = "crust_red"):
    """构建平滑体素圆球丹体。"""
    rc = r * 0.76
    rig.cube(bone, f"{prefix}_mx",
             (cx - r, cy - rc * 0.42, cz - rc),
             (cx + r, cy + rc * 0.42, cz + rc),
             mat=mat_body)
    rig.cube(bone, f"{prefix}_mz",
             (cx - rc, cy - rc * 0.42, cz - r),
             (cx + rc, cy + rc * 0.42, cz + r),
             mat=mat_body)
    rt = r * 0.62
    rig.cube(bone, f"{prefix}_top",
             (cx - rt, cy + rc * 0.42, cz - rt),
             (cx + rt, cy + r, cz + rt),
             mat=mat_lit)
    rig.cube(bone, f"{prefix}_bot",
             (cx - rt, cy - r, cz - rt),
             (cx + rt, cy - rc * 0.42, cz + rt),
             mat="crust_dark")


def add_true_sphere(rig: Rig, bone: str, prefix: str,
                    cx: float, cy: float, cz: float, r: float):
    """9 层精细渐变球体逼近算法（用于核心主丹）。"""
    cuts = [
        ("p0_bot", -r * 1.00, -r * 0.82, r * 0.48, r * 0.12, "crust_dark", "crust_dark"),
        ("p1_d2",  -r * 0.82, -r * 0.58, r * 0.74, r * 0.18, "crust_dark", "crust_mid"),
        ("p2_d1",  -r * 0.58, -r * 0.32, r * 0.90, r * 0.24, "crust_dark", "crust_mid"),
        ("p3_md",  -r * 0.32, -r * 0.08, r * 0.99, r * 0.28, "crust_mid",  "crust_mid"),
        ("p4_mu",  -r * 0.08,  r * 0.20, r * 1.00, r * 0.28, "crust_red",  "crust_mid"),
        ("p5_u1",   r * 0.20,  r * 0.50, r * 0.94, r * 0.26, "crust_red",  "crust_red"),
        ("p6_u2",   r * 0.50,  r * 0.75, r * 0.80, r * 0.22, "crust_red",  "crust_mid"),
        ("p7_u3",   r * 0.75,  r * 0.92, r * 0.58, r * 0.16, "crust_red",  "crust_red"),
        ("p8_top",  r * 0.92,  r * 1.00, r * 0.32, r * 0.08, "crust_red",  "crust_red"),
    ]

    for tag, dy0, dy1, rad, c, mm, ms in cuts:
        y0 = cy + dy0
        y1 = cy + dy1
        add_round_box(rig, bone, f"{prefix}_{tag}", y0, y1, rad, c, mm, ms, cx, cz)


def part_base_pills(rig: Rig) -> None:
    """底层稳固托底的两颗玄黑地火伴生丹（左前与右前下沉）。"""
    rig.bone("base_pills", (0.0, 0.0, 0.0))

    r_sub = 1.95
    # 左侧托底伴生丹 (cx=-2.0, cy=1.9, cz=-0.4)
    add_smooth_pill(rig, "base_pills", "sub_left", cx=-2.2, cy=1.95, cz=-0.4, r=r_sub,
                    mat_body="crust_dark", mat_lit="crust_mid")
    # 右侧托底伴生丹 (cx=2.0, cy=1.9, cz=-0.4)
    add_smooth_pill(rig, "base_pills", "sub_right", cx=2.2, cy=1.95, cz=-0.4, r=r_sub,
                    mat_body="crust_dark", mat_lit="crust_mid")


def part_main_pill(rig: Rig) -> None:
    """顶峰核心大丹：
    位于 (0.0, 4.4, 0.6)，半径 3.3px（直径 6.6px），稳坐在双伴生丹之上，
    正面对准正面视线，展示最完整的圆润球形与地火色泽。
    并在左下方带一块原画独特的剥落骨白药蜕斑（crust_flake）！
    """
    rig.bone("main_pill", (0.0, 4.4, 0.6))
    cx, cy, cz = 0.0, 4.4, 0.6
    r = 3.3

    # 1. 浑圆主丹球体
    add_true_sphere(rig, "main_pill", "grand_guyuan", cx=cx, cy=cy, cz=cz, r=r)

    # 2. 原画左下角鲜明的剥落骨白药蜕斑 (Flake)
    rig.cube("main_pill", "flake_patch",
             (cx - 2.5, cy - 2.8, cz + 1.2), (cx - 1.2, cy - 1.5, cz + 2.6),
             mat="crust_flake")


def part_runes(rig: Rig) -> None:
    """主丹正面极具辨识度的【同心金光丹纹与裂隙灵阵】（Concentric Rune Array）：
    紧密贴附在主丹前向表面 (+Z 侧，z ≈ 3.7~4.0)，正对正交前方视角：
    - 中心灵光金核 (Rune Center)
    - 内同心圆金环 (Inner Ring)
    - 外同心圆轨道与节点小球 (Outer Ring & Orbit Nodes)
    - 左右放射延伸的灵脉金纹 (Radial Meridian Lines)
    - 纵横交织的赤红炽火裂纹 (Fire Veins)
    """
    rig.bone("runes", (0.0, 4.4, 0.6))
    cx, cy, cz = 0.0, 4.4, 0.6
    zf = cz + 3.32  # 贴在主丹球体前表面

    # 1. 中心金光阵核 (高度约 4.4，正中)
    rig.cube("runes", "rune_core",
             (cx - 0.45, cy - 0.45, zf), (cx + 0.45, cy + 0.45, zf + 0.18),
             mat="rune_bright")

    # 2. 内圈同心圆金环 (半径约 1.2px)
    # 上下左右 4 段弧块拼成内同心环
    rig.cube("runes", "ring_in_t",
             (cx - 0.9, cy + 0.9, zf), (cx + 0.9, cy + 1.25, zf + 0.15),
             mat="rune_gold")
    rig.cube("runes", "ring_in_b",
             (cx - 0.9, cy - 1.25, zf), (cx + 0.9, cy - 0.9, zf + 0.15),
             mat="rune_gold")
    rig.cube("runes", "ring_in_l",
             (cx - 1.25, cy - 0.9, zf), (cx - 0.9, cy + 0.9, zf + 0.15),
             mat="rune_gold")
    rig.cube("runes", "ring_in_r",
             (cx + 0.9, cy - 0.9, zf), (cx + 1.25, cy + 0.9, zf + 0.15),
             mat="rune_gold")

    # 3. 外圈同心圆轨道 (半径约 2.1px)
    rig.cube("runes", "ring_out_t",
             (cx - 1.6, cy + 1.8, zf - 0.08), (cx + 1.6, cy + 2.15, zf + 0.08),
             mat="rune_gold")
    rig.cube("runes", "ring_out_b",
             (cx - 1.6, cy - 2.15, zf - 0.08), (cx + 1.6, cy - 1.8, zf + 0.08),
             mat="rune_gold")
    rig.cube("runes", "ring_out_l",
             (cx - 2.15, cy - 1.6, zf - 0.08), (cx - 1.8, cy + 1.6, zf + 0.08),
             mat="rune_gold")
    rig.cube("runes", "ring_out_r",
             (cx + 1.8, cy - 1.6, zf - 0.08), (cx + 2.15, cy + 1.6, zf + 0.08),
             mat="rune_gold")

    # 4. 轨道上的灵力节点小金珠 (Nodes)
    rig.cube("runes", "node_top",
             (cx - 0.3, cy + 2.1, zf - 0.05), (cx + 0.3, cy + 2.7, zf + 0.15),
             mat="rune_bright")
    rig.cube("runes", "node_bot",
             (cx - 0.3, cy - 2.7, zf - 0.05), (cx + 0.3, cy - 2.1, zf + 0.15),
             mat="rune_bright")
    rig.cube("runes", "node_left",
             (cx - 2.7, cy - 0.3, zf - 0.05), (cx - 2.1, cy + 0.3, zf + 0.15),
             mat="rune_bright")
    rig.cube("runes", "node_right",
             (cx + 2.1, cy - 0.3, zf - 0.05), (cx + 2.7, cy + 0.3, zf + 0.15),
             mat="rune_bright")

    # 5. 横贯左右的纬线灵力轨道（原画左右弧形轨道）
    rig.cube("runes", "orbit_lat_up",
             (cx - 2.9, cy + 0.7, zf - 0.15), (cx + 2.9, cy + 0.95, zf + 0.05),
             mat="rune_gold")
    rig.cube("runes", "orbit_lat_dn",
             (cx - 2.9, cy - 0.95, zf - 0.15), (cx + 2.9, cy - 0.7, zf + 0.05),
             mat="rune_gold")

    # 6. 原画特有的斜向赤火熔岩裂纹（Fire Veins）
    rig.cube("runes", "fire_vein_ne",
             (cx + 0.5, cy + 0.8, zf), (cx + 2.4, cy + 2.6, zf + 0.12),
             rot=(0.0, 0.0, 42.0), org=(cx + 1.4, cy + 1.7, zf),
             mat="vein_fire")
    rig.cube("runes", "fire_vein_sw",
             (cx - 2.4, cy - 2.6, zf), (cx - 0.5, cy - 0.8, zf + 0.12),
             rot=(0.0, 0.0, 45.0), org=(cx - 1.4, cy - 1.7, zf),
             mat="vein_fire")
    rig.cube("runes", "fire_vein_nw",
             (cx - 2.2, cy + 0.6, zf), (cx - 0.6, cy + 2.4, zf + 0.12),
             rot=(0.0, 0.0, -42.0), org=(cx - 1.4, cy + 1.5, zf),
             mat="vein_fire")


def build_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    part_base_pills(rig)
    part_main_pill(rig)
    part_runes(rig)
    return rig


def main():
    parser = argparse.ArgumentParser(description="固元丹 bbmodel 生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "GuYuanPill.bbmodel", help="输出路径")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = build_rig()
    bb_json = rig.bbmodel("GuYuanPill")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
