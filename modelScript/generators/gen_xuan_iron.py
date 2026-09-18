#!/usr/bin/env python3
"""玄铁（XuanIron）—— 致密深黑多面体陨铁碎块 Blockbench .bbmodel 生成器。

【设计原则与第一性原理】：
- 物品来源：`client/src/main/resources/assets/bong-client/textures/gui/items/xuan_iron.png`
  世界观：纯度极高、极其致密沉重的深黑陨铁原石，炼飞剑重兵之极品胎料。
- 视觉形态：
  1. 纯粹纯金属多面体：绝无泥壳、绝无岩底、绝无熔岩火光。
  2. 锋利不规则多面折角（Cleavage Facets）：
     - 类似天然重晶石与陨铁断口的大角度平整解理面。
     - 左侧高耸锋锐的主晶峰（陡峭斜坡受光），向右下方倾斜断裂。
     - 前脸大面积斜向倾切的向阳平整金属受光面（XUAN_LIT）。
     - 脊线上极具辨识度的冷银刀锋反光线（XUAN_SPEC）。
     - 右侧与后方粗粝错落的金属梯级断层（XUAN_MID, XUAN_DARK）。
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.rig.rigkit import Rig
from bbmodel_maker import workspace
from palette_xuaniron import (
    XUAN_VOID, XUAN_DARK, XUAN_MID, XUAN_LIT, XUAN_SPEC, XUAN_RUST
)

_WS = workspace.Workspace.discover(start=Path(__file__))
OUT_DIR = _WS.models
RENDER_OUT = _WS.out

MATS = {
    "xuan_void": XUAN_VOID,
    "xuan_dark": XUAN_DARK,
    "xuan_mid":  XUAN_MID,
    "xuan_lit":  XUAN_LIT,
    "xuan_spec": XUAN_SPEC,
    "xuan_rust": XUAN_RUST,
}


def build_xuan_iron_rig() -> Rig:
    rig = Rig(MATS, swatch=8)
    bone = "xuan_iron"
    rig.bone(bone, (0.0, 5.0, 0.0))

    # =========================================================================
    # 1. 底盘与实心母核（沉重压地，宽实致密）
    # =========================================================================
    # 底座支撑（致密重金属暗底）
    rig.cube(bone, "base_core",
             (-3.8, 0.0, -3.4), (3.6, 2.4, 3.2),
             mat="xuan_void")
    # 下部外扩断角
    rig.cube(bone, "base_flange_l",
             (-4.4, 0.6, -1.8), (-3.4, 2.8, 2.2),
             mat="xuan_dark")
    rig.cube(bone, "base_flange_r",
             (3.2, 0.4, -2.4), (4.4, 2.6, 1.8),
             mat="xuan_dark")
    rig.cube(bone, "base_front_step",
             (-2.2, 0.4, 2.6), (2.4, 2.2, 3.8),
             mat="xuan_mid")

    # =========================================================================
    # 2. 中躯实心本体（厚重深黑多面体）
    # =========================================================================
    rig.cube(bone, "body_core_bulk",
             (-3.6, 2.4, -3.2), (3.4, 7.2, 2.8),
             mat="xuan_dark")
    rig.cube(bone, "body_core_back",
             (-2.8, 2.4, -3.8), (2.6, 6.8, -2.8),
             mat="xuan_void")

    # =========================================================================
    # 3. 前倾平整金属解理大斜面（朝向前方的核心受光折面，对应原画大亮面）
    # =========================================================================
    # 采用向内倾斜的主受光平切断层
    org_front = (0.2, 5.2, 2.0)
    rot_front = (-16.0, 18.0, -8.0)

    # 主断裂面底壳
    rig.cube(bone, "facet_front_body",
             (-2.4, 2.8, 1.6), (2.6, 7.6, 3.4),
             rot=rot_front, org=org_front, mat="xuan_mid")
    # 主断裂面大面积平整受光金属层
    rig.cube(bone, "facet_front_lit_plane",
             (-1.8, 3.4, 2.4), (2.2, 7.2, 3.7),
             rot=rot_front, org=org_front, mat="xuan_lit")
    # 折面右侧锋利冷银反光边棱（原画正中的锐利反光线）
    rig.cube(bone, "facet_front_spec_edge",
             (1.4, 3.8, 2.8), (2.3, 7.4, 3.8),
             rot=rot_front, org=org_front, mat="xuan_spec")

    # =========================================================================
    # 4. 左侧高耸主脊峰（不对称断层主峰，高达 12.2 格）
    # =========================================================================
    org_peak = (-1.6, 8.0, 0.4)
    rot_peak = (12.0, 15.0, -18.0)

    # 峰柱母体
    rig.cube(bone, "peak_bulk",
             (-3.4, 6.6, -1.8), (-0.2, 10.8, 2.0),
             rot=rot_peak, org=org_peak, mat="xuan_dark")
    # 峰顶向阳削面
    rig.cube(bone, "peak_facet_lit",
             (-3.0, 8.2, -0.8), (-0.6, 11.4, 1.8),
             rot=rot_peak, org=org_peak, mat="xuan_lit")
    # 峰顶极锐利刀刃尖棱（冷银最高光）
    rig.cube(bone, "peak_blade_spec",
             (-2.4, 10.4, 0.0), (-1.0, 12.2, 1.2),
             rot=rot_peak, org=org_peak, mat="xuan_spec")

    # =========================================================================
    # 5. 右侧次生错落断层与阶梯解理块（向右下倾斜的次生断崖）
    # =========================================================================
    org_r = (2.2, 5.0, 0.2)
    rot_r = (-8.0, -26.0, 14.0)

    # 右侧低阶金属断块
    rig.cube(bone, "flank_r_bulk",
             (0.8, 2.6, -1.6), (3.8, 6.6, 2.2),
             rot=rot_r, org=org_r, mat="xuan_dark")
    # 右侧向阳受光斜面
    rig.cube(bone, "flank_r_lit",
             (1.4, 3.2, 0.4), (3.6, 6.2, 2.4),
             rot=rot_r, org=org_r, mat="xuan_lit")
    # 右侧中层高光短棱
    rig.cube(bone, "flank_r_spec",
             (2.6, 4.4, 1.2), (3.5, 5.8, 2.5),
             rot=rot_r, org=org_r, mat="xuan_spec")

    # 右后方阶梯碎块
    rig.cube(bone, "flank_br_step",
             (1.2, 6.0, -2.2), (3.2, 8.8, 0.6),
             rot=(10.0, -15.0, 8.0), org=(2.0, 6.5, -1.0), mat="xuan_mid")

    # =========================================================================
    # 6. 顶脊横断金属骨线（连接主峰与右断层的坚硬脊背）
    # =========================================================================
    rig.cube(bone, "ridge_bridge_dark",
             (-1.2, 7.2, -1.6), (1.6, 9.6, 1.2),
             mat="xuan_dark")
    rig.cube(bone, "ridge_crest_spec",
             (-0.6, 8.8, -0.4), (1.2, 10.2, 0.8),
             mat="xuan_spec")

    return rig


def main():
    parser = argparse.ArgumentParser(description="致密玄铁多面体原矿生成器")
    parser.add_argument("--out", type=Path, default=OUT_DIR / "XuanIron.bbmodel", help="输出路径")
    args = parser.parse_args()

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    RENDER_OUT.mkdir(parents=True, exist_ok=True)

    rig = build_xuan_iron_rig()
    bb_json = rig.bbmodel("XuanIron")
    args.out.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 写入模型: {args.out}")


if __name__ == "__main__":
    main()
