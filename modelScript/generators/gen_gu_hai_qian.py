#!/usr/bin/env python3
"""骨骸钳 gu_hai_qian：钳口厚重的铁钳，撬骨屠宰用。bbmodel + SML OBJ 双出（同一份 box 表）。

物品依据：`server/assets/items/tools.toml` 的 `gu_hai_qian`——「钳口厚重，柄端预留异兽骨包覆位；
撬骨屠宰会磨钳齿，fauna-v1 未落地前以占位材料制作。」所以两个识别特征：
1. **钳口厚重**：新月形的粗铁钳口，内侧一排被磨平的锯齿；
2. **柄端预留骨包覆位**：每只柄的末端是一个铁杯套 + 一截**占位骨套**（泛白的老骨色）——
   fauna-v1 落地后换成真正的异兽骨，几何不用动，只换那一个 material。

参考图：`model-review/img/handheld/exploded/gu_hai_qian.png`（本机 CPA 出的爆炸图）。

## 分件（从握把末端到尖端）

    part_sleeves  占位骨套 ×2         y 0 ~ 0.09，两只柄的末端
    part_handles  铁杯套 + 麻绳缠 + 弯臂 ×2   y 0.09 ~ 0.33，自下而上由外八字收向枢轴
    part_pivot    枢轴铁板 + 两颗铆钉头     y 0.31 ~ 0.39
    part_jaws     交叉后的两只新月钳口 + 内侧锯齿   y 0.39 ~ 0.60，齿 L/R 错开，合上不顶死

## 方向约定

握把末端 y=0、尖端 +Y（`assert_conventions`）；钳是左右（±X）对开的两爿，板厚在 z。
手柄在下、钳口在上，两爿在枢轴处交叉（左柄接右钳口、右柄接左钳口）——但 box 表里把交叉
折进了「柄往里收、钳口往外张」的几何，不单独建交叉的两根斜杆（斜杆在体素里只会是楼梯）。

## 贴图未经真机标定

同 `gen_wooden_club`：走 SML item 光照，本 harness 跑不了 `render_held_item.py`。
贴图取自 `handheld_textures`（凡铁 / 亮边铁 / 麻绳 / 老骨）。
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

ITEM_ID = "gu_hai_qian"
LENGTH = 0.625          # 全长（授权系，方块）：柄尾到钳口尖
GRIP = 0.17             # 拳心对准麻绳缠段
SCALE = 0.85            # 显示缩放 → 手里约 0.51 方块


def _mirror(boxes: list[Box]) -> list[Box]:
    """把 -X 一爿镜像成 +X 一爿（名字 `_l` → `_r`）。"""
    return [Box(box.name.replace("_l", "_r"), box.material,
                (-box.center[0], box.center[1], box.center[2]), box.half) for box in boxes]


def _left_handle() -> list[Box]:
    """-X 一爿的柄：骨套 → 铁杯套 → 麻绳三道 → 铁芯弯臂，自下而上往枢轴收。

    麻绳 y 范围与铁芯错开 0.005（避免底面共面）；两段弯臂的 x 位置、半宽各不相同。
    """
    return [
        Box("sleeve_l", "tong_bone", (-0.0720, 0.0450, 0.0), (0.0260, 0.0450, 0.0260)),
        Box("cup_l", "tong_iron", (-0.0680, 0.1075, 0.0), (0.0300, 0.0175, 0.0300)),
        Box("arm_low_l", "tong_iron", (-0.0620, 0.1750, 0.0), (0.0200, 0.0550, 0.0200)),
        Box("wrap_a_l", "tong_cord", (-0.0620, 0.1400, 0.0), (0.0255, 0.0150, 0.0255)),
        Box("wrap_b_l", "tong_cord", (-0.0600, 0.1730, 0.0), (0.0260, 0.0150, 0.0260)),
        Box("wrap_c_l", "tong_cord", (-0.0630, 0.2030, 0.0), (0.0252, 0.0125, 0.0252)),
        Box("arm_bend_l", "tong_iron", (-0.0460, 0.2700, 0.0), (0.0200, 0.0600, 0.0192)),
        Box("arm_neck_l", "tong_iron", (-0.0320, 0.3250, 0.0), (0.0180, 0.0150, 0.0210)),
    ]


def _left_jaw() -> list[Box]:
    """-X 一爿的钳口：新月形三段，越往外越厚、尖端往里收，内侧（+X 面）一排锯齿。

    三段的 z 半宽各不相同（0.028 / 0.030 / 0.026）——体素里的新月只能靠各段 x 位移 + 厚薄变化读出来。
    """
    return [
        # 钳口微张成 V：越往尖端离中线越远，内侧锯齿才看得见；内侧面 x = -0.010 / -0.024 / -0.036
        Box("jaw_a_l", "tong_iron", (-0.0340, 0.4250, 0.0), (0.0240, 0.0350, 0.0280)),
        Box("jaw_b_l", "tong_iron", (-0.0500, 0.4950, 0.0), (0.0260, 0.0350, 0.0300)),
        Box("jaw_c_l", "tong_iron", (-0.0580, 0.5650, 0.0), (0.0220, 0.0350, 0.0260)),
        # 钳口尖端再收一级：新月的钩尖，比 jaw_c 窄、向中线偏一点
        Box("jaw_tip_l", "tong_iron", (-0.0520, 0.6125, 0.0), (0.0160, 0.0125, 0.0210)),
        # 齿：贴在各段内侧面上朝中线突出 ~0.011，一段一枚、高度错开；右爿镜像后整体再抬 0.012，
        # 左右齿不在同一高度——参考图里两爿的齿也是错开咬合的
        Box("tooth_a_l", "tong_edge", (-0.0075, 0.4400, 0.0), (0.0065, 0.0100, 0.0220)),
        Box("tooth_b_l", "tong_edge", (-0.0215, 0.4850, 0.0), (0.0085, 0.0100, 0.0210)),
        Box("tooth_b2_l", "tong_edge", (-0.0220, 0.5200, 0.0), (0.0090, 0.0090, 0.0200)),
        Box("tooth_c_l", "tong_edge", (-0.0335, 0.5800, 0.0), (0.0085, 0.0100, 0.0190)),
    ]


def part_sleeves() -> list[Box]:
    handles = _left_handle()
    return [box for box in handles + _mirror(handles) if box.name.startswith("sleeve")]


def part_handles() -> list[Box]:
    handles = _left_handle()
    return [box for box in handles + _mirror(handles) if not box.name.startswith("sleeve")]


def part_pivot() -> list[Box]:
    """枢轴铁板 + 两面的铆钉头（参考图里那颗大铆钉带两片垫圈）。"""
    return [
        Box("pivot_plate", "tong_iron", (0.0, 0.3550, 0.0), (0.0400, 0.0400, 0.0300)),
        Box("pivot_rivet_front", "tong_edge", (0.0, 0.3550, -0.0360), (0.0200, 0.0200, 0.0060)),
        Box("pivot_rivet_back", "tong_edge", (0.0, 0.3550, 0.0360), (0.0200, 0.0200, 0.0060)),
        # 垫圈：铆钉头下面各一片更大的薄方片（参考图里铆钉两侧的垫圈），z 面与铆钉头错开
        Box("pivot_washer_front", "tong_iron", (0.0, 0.3550, -0.0340), (0.0280, 0.0280, 0.0035)),
        Box("pivot_washer_back", "tong_iron", (0.0, 0.3550, 0.0340), (0.0280, 0.0280, 0.0035)),
    ]


def part_jaws() -> list[Box]:
    left = _left_jaw()
    right = [Box(box.name.replace("_l", "_r"), box.material,
                 (-box.center[0], box.center[1] + (0.012 if box.name.startswith("tooth") else 0.0), box.center[2]),
                 box.half) for box in left]
    return left + right


def all_boxes() -> tuple[Box, ...]:
    return tuple(part_sleeves() + part_handles() + part_pivot() + part_jaws())


GU_HAI_QIAN = HeldItem(
    key=ITEM_ID,
    display_name="骨骸钳",
    host_item="iron_ingot",   # 登记意图；本次只落建模源，不接线，宿主见 plan-held-item-registration-v1
    boxes=all_boxes(),
    materials=(
        Material("tong_iron", (0.36, 0.34, 0.33), textures.pitted_iron(0x6E01, base=(86, 82, 80))),
        Material("tong_edge", (0.54, 0.52, 0.49), textures.worn_edge(0x6E02, rust_blotches=4)),
        Material("tong_cord", (0.57, 0.50, 0.37), textures.hemp_cord(0x6E03)),
        Material("tong_bone", (0.84, 0.78, 0.66), textures.pale_bone(0x6E04)),
    ),
    grip=GRIP,
    display=hand_display(SCALE, GRIP, LENGTH),
)


def items() -> tuple[HeldItem, ...]:
    return (GU_HAI_QIAN,)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-check", action="store_true", help="只写 bbmodel，不出 check.png")
    parser.add_argument("--dump-obj", action="store_true", help="OBJ/MTL 打到 stdout")
    parser.add_argument("--dump-display", action="store_true", help="model JSON（含 display）打到 stdout")
    args = parser.parse_args()

    if args.dump_display:
        print(build_model_json(GU_HAI_QIAN), end="")
        return
    if args.dump_obj:
        print(build_obj(GU_HAI_QIAN))
        return

    outputs = write_assets(items(), bbmodel_dir=BBMODEL_DIR, client_resources=None, render_previews=False)
    bbmodel = outputs[f"bbmodel:{ITEM_ID}"]
    print(f"[bbmodel] {bbmodel.relative_to(REPO)}")
    if not args.no_check and review_available():
        print(f"[check] {write_check_sheet(bbmodel, ITEM_ID)}")


if __name__ == "__main__":
    main()
