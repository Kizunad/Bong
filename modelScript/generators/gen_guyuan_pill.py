#!/usr/bin/env python3
"""固元丹（GuYuanPill）Blockbench .bbmodel 生成器 - 曲面贴合真元阵纹版。

物品来源：
  `client/src/main/resources/assets/bong-client/textures/gui/items/guyuan_pill.png`
  `server/assets/items/pills.toml` (id = "guyuan_pill", name = "固元丹")

原画特征深度还原：
  1. 赤炎玄黑丹壳（Magma Crust）：
     - 丹体如地火淬炼的星核，外层为深黑褐色碳化药皮，内蕴金红真元地火。
     - 左下角带有原画鲜明的剥落骨白药蜕斑（crust_flake）。
  2. 曲面贴合的同心圆固元金光丹纹（Curved Concentric Rune Array）：
     - 阵纹严格按照丹丸球面曲率 z = cz + sqrt(R^2 - r^2) 向球体前后顺滑收束贴合，
       杜绝侧视悬空与薄板穿插感，从任何角度看都如精雕在星核表面一般！
     - 包含中心灵核、内金环、外轨道金环、轨道灵力节点小珠、横向纬线轨道与赤炎地火裂隙。
  3. 三才稳固堆叠布局（Triadic Base）：
     - 底层左右双伴生丹沉稳托底，中央高耸浑圆大主丹（直径 6.8px）。

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
MATS = {
    "crust_dark":     (36, 16, 12),     # 玄黑深焦药皮
    "crust_mid":      (68, 28, 18),     # 药皮深红褐过渡
    "crust_red":      (118, 48, 24),    # 药皮暗红受光
    "vein_fire":      (214, 78, 28),    # 裂隙赤红炽火
    "rune_gold":      (254, 186, 42),   # 阵纹灿金
    "rune_bright":    (255, 232, 96),   # 阵核极亮纯金光
    "crust_flake":    (228, 218, 204),  # 丹皮剥落露出的骨白药蜕
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
    """底层稳固托底的两颗玄黑地火伴生丹。"""
    rig.bone("base_pills", (0.0, 0.0, 0.0))

    r_sub = 1.95
    # 左侧托底伴生丹
    add_smooth_pill(rig, "base_pills", "sub_left", cx=-2.2, cy=1.95, cz=-0.4, r=r_sub,
                    mat_body="crust_dark", mat_lit="crust_mid")
    # 右侧托底伴生丹
    add_smooth_pill(rig, "base_pills", "sub_right", cx=2.2, cy=1.95, cz=-0.4, r=r_sub,
                    mat_body="crust_dark", mat_lit="crust_mid")


def part_main_pill(rig: Rig) -> None:
    """顶峰核心大丹（直径 6.8px，坐落于中心偏上）。"""
    rig.bone("main_pill", (0.0, 4.4, 0.4))
    cx, cy, cz = 0.0, 4.4, 0.4
    r = 3.4

    # 1. 浑圆主丹球体
    add_true_sphere(rig, "main_pill", "grand_guyuan", cx=cx, cy=cy, cz=cz, r=r)

    # 2. 原画左下角剥落骨白药蜕斑 (Flake) - 贴合球面
    rig.cube("main_pill", "flake_patch",
             (cx - 2.5, cy - 2.7, cz + 1.6), (cx - 1.2, cy - 1.4, cz + 2.8),
             mat="crust_flake")


def part_runes(rig: Rig) -> None:
    """按球面曲率贴合的同心金光丹纹与裂隙灵阵：
    球心位于 (0, 4.4, 0.4)，半径 R=3.4。
    随着距中心距离增加，各同心圆环和节点顺滑内缩，紧贴球面！
    """
    rig.bone("runes", (0.0, 4.4, 0.4))
    cx, cy, cz = 0.0, 4.4, 0.4

    # 1. 中心金光阵核 (r ≈ 0, z_surf ≈ cz + 3.42 = 3.82)
    z_core = cz + 3.42
    rig.cube("runes", "rune_core",
             (cx - 0.45, cy - 0.45, z_core - 0.05), (cx + 0.45, cy + 0.45, z_core + 0.15),
             mat="rune_bright")

    # 2. 内圈同心圆金环 (环半径约 1.15, z_surf ≈ cz + 3.20 = 3.60)
    z_in = cz + 3.22
    rig.cube("runes", "ring_in_t",
             (cx - 0.85, cy + 0.85, z_in), (cx + 0.85, cy + 1.25, z_in + 0.15),
             mat="rune_gold")
    rig.cube("runes", "ring_in_b",
             (cx - 0.85, cy - 1.25, z_in), (cx + 0.85, cy - 0.85, z_in + 0.15),
             mat="rune_gold")
    rig.cube("runes", "ring_in_l",
             (cx - 1.25, cy - 0.85, z_in), (cx - 0.85, cy + 0.85, z_in + 0.15),
             mat="rune_gold")
    rig.cube("runes", "ring_in_r",
             (cx + 0.85, cy - 0.85, z_in), (cx + 1.25, cy + 0.85, z_in + 0.15),
             mat="rune_gold")

    # 3. 外圈同心圆轨道 (环半径约 2.05, z_surf ≈ cz + 2.71 = 3.11)
    z_out = cz + 2.75
    rig.cube("runes", "ring_out_t",
             (cx - 1.5, cy + 1.75, z_out), (cx + 1.5, cy + 2.10, z_out + 0.15),
             mat="rune_gold")
    rig.cube("runes", "ring_out_b",
             (cx - 1.5, cy - 2.10, z_out), (cx + 1.5, cy - 1.75, z_out + 0.15),
             mat="rune_gold")
    rig.cube("runes", "ring_out_l",
             (cx - 2.10, cy - 1.5, z_out), (cx - 1.75, cy + 1.5, z_out + 0.15),
             mat="rune_gold")
    rig.cube("runes", "ring_out_r",
             (cx + 1.75, cy - 1.5, z_out), (cx + 2.10, cy + 1.5, z_out + 0.15),
             mat="rune_gold")

    # 4. 轨道上的灵力节点小金珠 (Nodes, 半径约 2.3, z_surf ≈ cz + 2.5 = 2.9)
    z_node = cz + 2.55
    rig.cube("runes", "node_top",
             (cx - 0.3, cy + 2.1, z_node), (cx + 0.3, cy + 2.7, z_node + 0.18),
             mat="rune_bright")
    rig.cube("runes", "node_bot",
             (cx - 0.3, cy - 2.7, z_node), (cx + 0.3, cy - 2.1, z_node + 0.18),
             mat="rune_bright")
    rig.cube("runes", "node_left",
             (cx - 2.7, cy - 0.3, z_node), (cx - 2.1, cy + 0.3, z_node + 0.18),
             mat="rune_bright")
    rig.cube("runes", "node_right",
             (cx + 2.1, cy - 0.3, z_node), (cx + 2.7, cy + 0.3, z_node + 0.18),
             mat="rune_bright")

    # 5. 横贯左右的纬线灵力轨道 (外延到两侧，向后贴紧球面)
    # 中间段 (x: -1.8 ~ 1.8, z ≈ cz + 2.85)
    rig.cube("runes", "orbit_lat_up_mid",
             (cx - 1.8, cy + 0.72, cz + 2.95), (cx + 1.8, cy + 0.95, cz + 3.10),
             mat="rune_gold")
    rig.cube("runes", "orbit_lat_dn_mid",
             (cx - 1.8, cy - 0.95, cz + 2.95), (cx + 1.8, cy - 0.72, cz + 3.10),
             mat="rune_gold")
    # 两侧延伸段 (向后收束弯曲贴合球面，z 降至 cz + 2.0)
    rig.cube("runes", "orbit_lat_up_l",
             (cx - 2.8, cy + 0.72, cz + 2.10), (cx - 1.8, cy + 0.95, cz + 2.95),
             mat="rune_gold")
    rig.cube("runes", "orbit_lat_up_r",
             (cx + 1.8, cy + 0.72, cz + 2.10), (cx + 2.8, cy + 0.95, cz + 2.95),
             mat="rune_gold")
    rig.cube("runes", "orbit_lat_dn_l",
             (cx - 2.8, cy - 0.95, cz + 2.10), (cx - 1.8, cy - 0.72, cz + 2.95),
             mat="rune_gold")
    rig.cube("runes", "orbit_lat_dn_r",
             (cx + 1.8, cy - 0.95, cz + 2.10), (cx + 2.8, cy - 0.72, cz + 2.95),
             mat="rune_gold")

    # 6. 斜向赤火熔岩裂纹 (同样按球面深度分段贴合)
    rig.cube("runes", "fire_vein_ne",
             (cx + 0.4, cy + 0.7, cz + 2.85), (cx + 2.2, cy + 2.4, cz + 3.15),
             rot=(0.0, 0.0, 42.0), org=(cx + 1.3, cy + 1.6, cz + 3.0),
             mat="vein_fire")
    rig.cube("runes", "fire_vein_sw",
             (cx - 2.2, cy - 2.4, cz + 2.85), (cx - 0.4, cy - 0.7, cz + 3.15),
             rot=(0.0, 0.0, 45.0), org=(cx - 1.3, cy - 1.6, cz + 3.0),
             mat="vein_fire")
    rig.cube("runes", "fire_vein_nw",
             (cx - 2.1, cy + 0.5, cz + 2.85), (cx - 0.5, cy + 2.2, cz + 3.15),
             rot=(0.0, 0.0, -42.0), org=(cx - 1.3, cy + 1.4, cz + 3.0),
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
