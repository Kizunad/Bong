#!/usr/bin/env python3
"""刮刀 gua_dao：细刃刮刀，刮树皮、取脂、剥薄矿膜。bbmodel + SML OBJ 双出（同一份 box 表）。

物品依据：`server/assets/items/tools.toml` 的 `gua_dao`——「细刃刮刀，可刮树皮、取脂，也能
剥薄矿膜；用于近斗只算临时短刃。」所以是**工具不是兵器**：短、薄、素，刃是宽扁的刮板，
带一道单面开刃的斜边，不是尖利的刀尖。

现状：`BongWeaponModelRegistry` 里 `gua_dao` 借原版 `item/stone_sword` 当宿主（石剑被 6 个
模板共用，见 `gen_knife_trio.py` 的宿主说明），没有专属模型。这份是专属模型，只落建模源，
不改 registry、不接线。

参考图：`model-review/img/handheld/exploded/gua_dao.png`（本机 CPA 出的爆炸图）。

## 分件（从握把末端到尖端，与参考图自下而上一一对应）

    part_pommel   铁柄帽              y 0 ~ 0.035
    part_handle   木柄两段 + 麻绳三道 + 绳结   y 0.035 ~ 0.29，上段略粗，绳缠在中段
    part_ferrule  铁箍               y 0.29 ~ 0.33
    part_blade    扁榫 + 窄颈 + 肩 + 3px 刮板 + 带缺口的开刃斜边 + 向后勾的弯钩   y 0.33 ~ 0.84

## 方向约定

握把末端 y=0、尖端 +Y（`assert_conventions`）。刃面朝 ±Z（刃板厚在 z、宽在 x），开刃斜边在
**+X** 侧，弯钩尖端向 -X（刀背）折回——和 `gen_knife_trio` 的小刀同一姿态（刃平面是 x–y），握在手里刃口朝外。

## 贴图未经真机标定

同 `gen_wooden_club`：走 SML item 光照，本 harness 跑不了 `render_held_item.py`。
贴图取自 `handheld_textures`（旧木 / 凡铁 / 亮边铁 / 麻绳）。
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from bbmodel_maker.render.held_item_common import (
    Box,
    HeldItem,
    Material,
    build_model_json,
    build_obj,
    hand_display,
    write_assets,
)

import handheld_textures as textures

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from handheld_check import review_available, write_check_sheet  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
BBMODEL_DIR = Path(__file__).resolve().parents[1] / "models"

ITEM_ID = "gua_dao"
LENGTH = 0.84           # 全长（授权系，方块）：柄帽到钩尖
GRIP = 0.16             # 拳心对准麻绳缠段
SCALE = 0.78            # 显示缩放 → 手里约 0.66 方块


def part_pommel() -> list[Box]:
    """铁柄帽：比木柄粗一圈的一块锈铁，端面落在 y=0。"""
    return [Box("pommel", "scr_iron", (0.0, 0.0175, 0.0), (0.0460, 0.0175, 0.0420))]


def part_handle() -> list[Box]:
    """木柄两段（上段略粗）+ 三道麻绳 + 一个绳结。

    绳道的 y 范围刻意避开两段木柄的接缝（0.17）——否则绳道底面与上段底面共面。
    绳结挂在 +X 侧，半个埋进绳里（参考图里绳尾打的那个结）。
    """
    return [
        Box("handle_low", "scr_wood", (0.0, 0.1025, 0.0), (0.0380, 0.0675, 0.0360)),
        Box("handle_high", "scr_wood", (0.0, 0.2300, 0.0), (0.0405, 0.0600, 0.0385)),
        Box("wrap_a", "scr_cord", (0.0, 0.1125, 0.0), (0.0435, 0.0125, 0.0415)),
        Box("wrap_b", "scr_cord", (0.0020, 0.1475, 0.0), (0.0445, 0.0125, 0.0425)),
        Box("wrap_c", "scr_cord", (-0.0020, 0.1875, 0.0), (0.0440, 0.0125, 0.0420)),
        Box("wrap_knot", "scr_cord", (0.0460, 0.1500, 0.0), (0.0100, 0.0140, 0.0120)),
    ]


def part_ferrule() -> list[Box]:
    """铁箍：握柄与刃之间的一圈方铁，参考图里那截宽扁的套。"""
    return [Box("ferrule", "scr_iron", (0.0, 0.3100, 0.0), (0.0470, 0.0200, 0.0430))]


def part_blade() -> list[Box]:
    """刮板：扁榫 → 窄颈 → 肩 → 3px 宽的刮板，+X 侧是开刃斜边（带三个缺口），顶端一枚向后勾的弯钩。

    尺寸（px，1px = 1/16 方块）：
    - 刀身总宽 3px（-X 的刀背到 +X 的刃口）：主板 2px + 开刃斜边 1px；
    - 刃口一侧三个缺口：斜边按 y 切成四块，块间三道 1px 深（斜边宽度）、约半 px 高的缺口，
      高低不等，等距读成梳齿；
    - 弯钩：尖端 2px 高，先直着立 1px，再在顶上 1px 向刀背（-X）折回 1px——参考图里那个
      「向后勾」的钩尖。

    刃板厚（z 半宽）逐段错开 0.0005~0.001：相邻段侧面不共面；斜边四块的 y 面各不相同。
    """
    bevel_pieces = (
        # 名      y0      y1
        ("a", 0.485, 0.520),
        ("b", 0.545, 0.580),
        ("c", 0.605, 0.640),
        ("d", 0.665, 0.685),
    )
    boxes = [
        Box("tang", "scr_iron", (0.0, 0.3575, 0.0), (0.0200, 0.0275, 0.0110)),
        Box("neck", "scr_iron", (0.0, 0.4150, 0.0), (0.0245, 0.0300, 0.0125)),
        Box("blade_shoulder", "scr_iron", (-0.0050, 0.4625, 0.0), (0.0530, 0.0175, 0.0130)),
        Box("blade_main", "scr_iron", (-0.0312, 0.5850, 0.0), (0.0627, 0.1050, 0.0135)),
        Box("blade_top", "scr_iron", (-0.0312, 0.7025, 0.0), (0.0627, 0.0125, 0.0128)),
        Box("hook_stem", "scr_iron", (0.0312, 0.7463, 0.0), (0.0312, 0.0313, 0.0126)),
        Box("hook_curl", "scr_iron", (0.0, 0.8088, 0.0), (0.0625, 0.0313, 0.0122)),
        # 钩尖折回后往下一拽：折回的 1px 末端再垂下 1px，读成真正的「勾」而不是顶上一根横条
        Box("hook_drop", "scr_iron", (-0.0550, 0.7613, 0.0), (0.0075, 0.0163, 0.0120)),
        # 刃根两级台阶（参考图里刃与榫之间那几级）：刀背侧一块宽的、刃口侧一块窄的，高低不同
        Box("root_step_spine", "scr_iron", (-0.0760, 0.4675, 0.0), (0.0180, 0.0125, 0.0131)),
        Box("root_step_edge", "scr_iron", (0.0565, 0.4650, 0.0), (0.0085, 0.0150, 0.0128)),
    ]
    for name, y0, y1 in bevel_pieces:
        boxes.append(Box(f"blade_bevel_{name}", "scr_edge", (0.0628, (y0 + y1) / 2.0, 0.0),
                         (0.0313, (y1 - y0) / 2.0, 0.0125)))
    return boxes


def all_boxes() -> tuple[Box, ...]:
    return tuple(part_pommel() + part_handle() + part_ferrule() + part_blade())


GUA_DAO = HeldItem(
    key=ITEM_ID,
    display_name="刮刀",
    host_item="stone_sword",   # 现状借的宿主（登记意图）；本次只落建模源，不接线
    boxes=all_boxes(),
    materials=(
        Material("scr_wood", (0.53, 0.39, 0.24), textures.old_wood(0x6D01)),
        Material("scr_iron", (0.36, 0.34, 0.33), textures.pitted_iron(0x6D02)),
        Material("scr_edge", (0.54, 0.52, 0.49), textures.worn_edge(0x6D03)),
        Material("scr_cord", (0.57, 0.50, 0.37), textures.hemp_cord(0x6D04)),
    ),
    grip=GRIP,
    display=hand_display(SCALE, GRIP, LENGTH),
)


def items() -> tuple[HeldItem, ...]:
    return (GUA_DAO,)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-check", action="store_true", help="只写 bbmodel，不出 check.png")
    parser.add_argument("--dump-obj", action="store_true", help="OBJ/MTL 打到 stdout")
    parser.add_argument("--dump-display", action="store_true", help="model JSON（含 display）打到 stdout")
    args = parser.parse_args()

    if args.dump_display:
        print(build_model_json(GUA_DAO), end="")
        return
    if args.dump_obj:
        print(build_obj(GUA_DAO))
        return

    outputs = write_assets(items(), bbmodel_dir=BBMODEL_DIR, client_resources=None, render_previews=False)
    bbmodel = outputs[f"bbmodel:{ITEM_ID}"]
    print(f"[bbmodel] {bbmodel.relative_to(REPO)}")
    if not args.no_check and review_available():
        print(f"[check] {write_check_sheet(bbmodel, ITEM_ID)}")


if __name__ == "__main__":
    main()
