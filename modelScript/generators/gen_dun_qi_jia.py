#!/usr/bin/env python3
"""钝气夹 dun_qi_jia：两片厚铁夹合皮垫，夹毒草和锐刺用。bbmodel + SML OBJ 双出（同一份 box 表）。

物品依据：`server/assets/items/tools.toml` 的 `dun_qi_jia`——「两片厚铁夹合皮垫，用来夹取毒草和
锐刺；挡的是皮肉伤，不挡真元，夹口磨平就等于空手。」所以识别特征是：
1. **两片厚铁夹**：又厚又平的两条铁板，上端微张成 V；
2. **皮垫**：夹口内侧各一块缝过线的深色皮垫（「夹口磨平」说的就是这里磨没了）；
3. **钝**：夹口尖端是磨平的钝头，不是尖的——所以叫「钝」气夹，也不带锯齿。

参考图：`model-review/img/handheld/exploded/dun_qi_jia.png`（本机 CPA 出的爆炸图）。

## 分件（从握把末端到尖端）

    part_hinge    铰块 + 轴销        y 0 ~ 0.06，两片夹子在底下合成一个整体
    part_plates   两片厚铁夹，三段，往上由内收渐张     y 0.06 ~ 0.40
    part_wrap     麻绳缠三道         y 0.155 ~ 0.24，手握处，同时把两片夹子勒在一起
    part_pads     夹口皮垫 ×2 + 缝线   y 0.31 ~ 0.38，贴在两片夹子的内侧面，每块三道缝线
    part_tips     磨平的钝头 ×2      y 0.40 ~ 0.445，用亮铁（磨出来的）

## 方向约定

握把末端 y=0、尖端 +Y（`assert_conventions`）；两片夹子左右（±X）对开，夹板厚在 x、宽在 z。

## 贴图未经真机标定

同 `gen_wooden_club`：走 SML item 光照，本 harness 跑不了 `render_held_item.py`。
贴图取自 `handheld_textures`（凡铁 / 亮边铁 / 麻绳 / 皮垫）。
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

ITEM_ID = "dun_qi_jia"
LENGTH = 0.445          # 全长（授权系，方块）：铰块底到钝头顶
GRIP = 0.20             # 拳心对准麻绳缠段
SCALE = 0.90            # 显示缩放 → 手里约 0.40 方块（小工具）


def _mirror(boxes: list[Box]) -> list[Box]:
    """把 -X 一爿镜像成 +X 一爿（名字 `_l` → `_r`）。"""
    return [Box(box.name.replace("_l", "_r"), box.material,
                (-box.center[0], box.center[1], box.center[2]), box.half) for box in boxes]


def _left_plate() -> list[Box]:
    """-X 一片厚铁夹：三段，越往上离中线越远（V 形张开）；段间 x 位置和 z 半宽都错开，侧面不共面。"""
    return [
        Box("plate_low_l", "clamp_iron", (-0.0300, 0.1300, 0.0), (0.0190, 0.0700, 0.0340)),
        Box("plate_mid_l", "clamp_iron", (-0.0360, 0.2500, 0.0), (0.0190, 0.0500, 0.0335)),
        Box("plate_jaw_l", "clamp_iron", (-0.0460, 0.3500, 0.0), (0.0190, 0.0500, 0.0330)),
        # 夹口皮垫：贴在夹口段内侧面（x = -0.021）上，朝中线鼓出 0.0125
        Box("pad_l", "clamp_pad", (-0.0207, 0.3450, 0.0), (0.0063, 0.0350, 0.0260)),
        # 皮垫缝线：三道细麻线横过垫面，露在垫的内侧面外 0.003
        Box("stitch_a_l", "clamp_cord", (-0.0135, 0.3250, 0.0), (0.0030, 0.0035, 0.0200)),
        Box("stitch_b_l", "clamp_cord", (-0.0132, 0.3450, 0.0), (0.0030, 0.0030, 0.0205)),
        Box("stitch_c_l", "clamp_cord", (-0.0138, 0.3650, 0.0), (0.0030, 0.0032, 0.0198)),
        # 钝头：夹口磨平的那一截，亮铁；顶上再压一小级，读成倒了角的钝面
        Box("tip_l", "clamp_edge", (-0.0460, 0.4150, 0.0), (0.0200, 0.0150, 0.0310)),
        Box("tip_top_l", "clamp_edge", (-0.0480, 0.4375, 0.0), (0.0170, 0.0075, 0.0280)),
    ]


def part_hinge() -> list[Box]:
    """铰块 + 轴销：参考图里底下那块方铁和穿过它的销子，销头两侧各露一截。"""
    return [
        Box("hinge_block", "clamp_iron", (0.0, 0.0300, 0.0), (0.0500, 0.0300, 0.0400)),
        Box("axle_pin", "clamp_edge", (0.0, 0.0300, 0.0), (0.0140, 0.0140, 0.0520)),
    ]


def part_plates() -> list[Box]:
    plates = _left_plate()
    return [box for box in plates + _mirror(plates) if box.name.startswith("plate")]


def part_wrap() -> list[Box]:
    """麻绳三道，厚薄、前后偏位参差——等厚等宽读成车出来的凹槽，参差才读成绕上去的绳。"""
    return [
        Box("wrap_a", "clamp_cord", (0.0010, 0.1675, 0.0), (0.0560, 0.0125, 0.0405)),
        Box("wrap_b", "clamp_cord", (-0.0010, 0.1975, 0.0), (0.0575, 0.0125, 0.0415)),
        Box("wrap_c", "clamp_cord", (0.0010, 0.2275, 0.0), (0.0565, 0.0125, 0.0400)),
    ]


def part_pads() -> list[Box]:
    plates = _left_plate()
    return [box for box in plates + _mirror(plates) if box.name.startswith(("pad", "stitch"))]


def part_tips() -> list[Box]:
    plates = _left_plate()
    return [box for box in plates + _mirror(plates) if box.name.startswith("tip")]


def all_boxes() -> tuple[Box, ...]:
    return tuple(part_hinge() + part_plates() + part_wrap() + part_pads() + part_tips())


DUN_QI_JIA = HeldItem(
    key=ITEM_ID,
    display_name="钝气夹",
    host_item="iron_ingot",   # 登记意图；本次只落建模源，不接线，宿主见 plan-held-item-registration-v1
    boxes=all_boxes(),
    materials=(
        Material("clamp_iron", (0.36, 0.34, 0.33), textures.pitted_iron(0x6F01)),
        Material("clamp_edge", (0.54, 0.52, 0.49), textures.worn_edge(0x6F02, rust_blotches=2)),
        Material("clamp_pad", (0.38, 0.27, 0.19), textures.hide_pad(0x6F03)),
        Material("clamp_cord", (0.57, 0.50, 0.37), textures.hemp_cord(0x6F04)),
    ),
    grip=GRIP,
    display=hand_display(SCALE, GRIP, LENGTH),
)


def items() -> tuple[HeldItem, ...]:
    return (DUN_QI_JIA,)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-check", action="store_true", help="只写 bbmodel，不出 check.png")
    parser.add_argument("--dump-obj", action="store_true", help="OBJ/MTL 打到 stdout")
    parser.add_argument("--dump-display", action="store_true", help="model JSON（含 display）打到 stdout")
    args = parser.parse_args()

    if args.dump_display:
        print(build_model_json(DUN_QI_JIA), end="")
        return
    if args.dump_obj:
        print(build_obj(DUN_QI_JIA))
        return

    outputs = write_assets(items(), bbmodel_dir=BBMODEL_DIR, client_resources=None, render_previews=False)
    bbmodel = outputs[f"bbmodel:{ITEM_ID}"]
    print(f"[bbmodel] {bbmodel.relative_to(REPO)}")
    if not args.no_check and review_available():
        print(f"[check] {write_check_sheet(bbmodel, ITEM_ID)}")


if __name__ == "__main__":
    main()
