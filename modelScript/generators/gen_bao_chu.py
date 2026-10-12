#!/usr/bin/env python3
"""刨锄 bao_chu：凡铁锄头配木柄，翻土挖根用。bbmodel + SML OBJ 双出（同一份 box 表）。

物品依据：`server/assets/items/tools.toml` 的 `bao_chu`——「凡铁锄头配木柄，用来翻土、
挖根，柄上没有封灵纹；当钝器抡只占一点力臂便宜」。所以这件的识别特征是：**素**。
没有刻纹、没有灵光、没有骨饰，只有一块钝而发锈的铁锄头、一根光木柄、一段缠手的麻绳。

参考图：`model-review/img/handheld/exploded/bao_chu.png`（本机 CPA 出的爆炸图）。

## 分件（从握把末端到尖端，与参考图自下而上一一对应）

    part_butt      柄尾木盖          y 0 ~ 0.04
    part_wrap      缠手麻绳 五道      y 0.05 ~ 0.24（参考图里绳缠在柄尾上方一段）
    part_haft      光木柄 三段        y 0.04 ~ 0.62，向上略收
    part_socket    2×2px 方銎 + 两颗铆钉    y 0.60 ~ 0.74
    part_head      锄板 小舌 + 过渡 + 主板 + 带缺口的前缘   銎顶往 -X 折成直角，宽 6px、伸出 5px、厚 1px

## 方向约定

握把末端 y=0、尖端 +Y（`assert_conventions`）。锄头的刃建在 **-X** 一侧，和 `axe_bone_v2`
/ `axe_iron_v2` 同向——两把斧的使用动画就是按「刃在建模稿 -X 侧」调的，锄头跟着走，
以后做刨地动画不用再翻一次。刃面是水平的板（宽在 z，厚在 y），逐段往下垂，刃口最低最宽：
读起来是「一块往下刮的铁板」，不是剑也不是斧。

## 贴图未经真机标定

同 `gen_wooden_club`：走 SML item 光照，本 harness 跑不了 `render_held_item.py`，
色值照参考图取、系数是经验值。
"""

from __future__ import annotations

import argparse
import random
import sys
from pathlib import Path

from bbmodel_maker.render.held_item_common import (
    Box,
    HeldItem,
    Material,
    blotch,
    build_model_json,
    build_obj,
    hand_display,
    noise_fill,
    write_assets,
)
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from handheld_check import write_check_sheet  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
BBMODEL_DIR = Path(__file__).resolve().parents[1] / "models"

ITEM_ID = "bao_chu"
LENGTH = 0.8338         # 全长（授权系，方块）：握把末端 y=0 到翻边上沿
GRIP = 0.14             # 拳心对准缠手麻绳的中段
SCALE = 0.85            # 显示缩放 → 手里约 0.66 方块

PX = 1.0 / 16.0         # 1 像素 = 1/16 方块；锄板尺寸按像素给
EYE_PX = 2.0            # 方銎 2×2 px
SOCKET_TOP = 0.74       # 方銎顶（y）
PLATE_WIDTH_PX = 6.0    # 锄板宽（z）
PLATE_REACH_PX = 5.0    # 锄板伸出方銎之外的长度（-X）
PLATE_THICK_PX = 1.0    # 锄板厚（y）
LIP_PX = 1.5            # 锄板两侧翻边高度
LIP_THICK = 0.0150      # 翻边厚（z）


# ── 贴图 ──────────────────────────────────────────────────────────────────
# 每张 16²，OBJ 链每个面整张铺满，所以画通用材质样本（同 gen_wooden_club）。


def tex_hoe_wood() -> Image.Image:
    """光木柄：干燥的浅褐老木，纵向纤维只画两条（柄面在屏幕上只有十几像素，多了读成条纹布）。"""
    rng = random.Random(0xB0C1)
    img = Image.new("RGBA", (16, 16), (136, 99, 60, 255))
    noise_fill(img, rng, (136, 99, 60), 7, warm=3)
    blotch(img, rng, 3, (112, 78, 44), (2.0, 4.0))          # 汗渍压暗
    blotch(img, rng, 3, (158, 118, 74), (1.8, 3.4))         # 磨亮
    pixels = img.load()
    for x in (4, 11):
        for y in range(16):
            if (x * 5 + y * 3) % 4:
                pixels[x, y] = (116, 82, 48, 255)
    return img


def tex_hoe_iron() -> Image.Image:
    """凡铁：暗哑的灰铁，满是锈斑和凹坑，没有任何反光——这是凡铁不是灵铁。"""
    rng = random.Random(0xB0C2)
    img = Image.new("RGBA", (16, 16), (92, 88, 84, 255))
    noise_fill(img, rng, (92, 88, 84), 9)
    blotch(img, rng, 5, (128, 78, 46), (1.6, 3.4))          # 锈
    blotch(img, rng, 4, (62, 58, 56), (1.4, 2.8))           # 坑
    blotch(img, rng, 2, (118, 114, 108), (1.6, 3.0))        # 磨亮的一点
    return img


def tex_hoe_edge() -> Image.Image:
    """前缘：翻土磨出来的亮铁，比铁身亮一档，但缺口边上锈斑成片——磨亮与生锈并存。"""
    rng = random.Random(0xB0C3)
    img = Image.new("RGBA", (16, 16), (138, 134, 126, 255))
    noise_fill(img, rng, (138, 134, 126), 8)
    blotch(img, rng, 3, (166, 162, 154), (1.8, 3.2))
    blotch(img, rng, 5, (132, 74, 40), (1.6, 3.2))          # 前缘锈斑：缺口边上尤其重
    return img


def tex_hoe_cord() -> Image.Image:
    """缠手麻绳：搓过的黄麻，斜向拧纹（斜纹的方向和密度就是"绳"的全部识别特征）。"""
    rng = random.Random(0xB0C4)
    img = Image.new("RGBA", (16, 16), (146, 128, 94, 255))
    noise_fill(img, rng, (146, 128, 94), 7, warm=3)
    pixels = img.load()
    for start in range(-8, 16, 3):
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (116, 100, 72, 255)
    for start in range(-6, 16, 6):
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (170, 152, 116, 255)
    return img


# ── 分件 ──────────────────────────────────────────────────────────────────


def part_butt() -> list[Box]:
    """柄尾木盖：比柄粗一圈的一块圆木，端面落在 y=0。"""
    return [Box("butt_cap", "hoe_wood", (0.0, 0.0200, 0.0), (0.0500, 0.0200, 0.0470))]


def part_wrap() -> list[Box]:
    """缠手麻绳五道，厚薄与前后偏位参差——等厚等宽读成车出来的凹槽，参差才读成绕上去的绳。"""
    coils = (
        # y0     y1     半宽 x  半宽 z  x 偏    z 偏
        (0.050, 0.082, 0.0525, 0.0495, -0.002, +0.002),
        (0.088, 0.120, 0.0515, 0.0505, +0.003, -0.002),
        (0.126, 0.160, 0.0530, 0.0490, -0.003, +0.003),
        (0.166, 0.198, 0.0518, 0.0502, +0.002, -0.003),
        (0.204, 0.238, 0.0528, 0.0494, -0.002, +0.002),
    )
    return [
        Box(f"wrap_{index}", "hoe_cord", (dx, (y0 + y1) / 2.0, dz), (hx, (y1 - y0) / 2.0, hz))
        for index, (y0, y1, hx, hz, dx, dz) in enumerate(coils)
    ]


def part_haft() -> list[Box]:
    """光木柄三段，往上略收；段间 x/z 半宽错开 0.002~0.003，避免相邻段侧面共面。"""
    segments = (
        # 名          y0     y1     半宽 x  半宽 z
        ("haft_low", 0.040, 0.250, 0.0440, 0.0410),
        ("haft_mid", 0.250, 0.450, 0.0420, 0.0392),
        ("haft_top", 0.450, 0.620, 0.0398, 0.0372),
    )
    return [
        Box(name, "hoe_wood", (0.0, (y0 + y1) / 2.0, 0.0), (hx, (y1 - y0) / 2.0, hz))
        for name, y0, y1, hx, hz in segments
    ]


def part_socket() -> list[Box]:
    """方銎：2×2px 的方铁筒套住柄头（参考图里那截带铆钉的方筒），两颗铆钉在 ±Z 两面各一。"""
    half = EYE_PX / 2.0 * PX
    rivet = 0.0070
    return [
        Box("socket", "hoe_iron", (0.0, 0.6700, 0.0), (half, 0.0700, half)),
        Box("rivet_front", "hoe_iron", (0.0, 0.6350, -(half + rivet)), (0.0140, 0.0140, rivet)),
        Box("rivet_back", "hoe_iron", (0.0, 0.7000, half + rivet), (0.0140, 0.0140, rivet)),
    ]


def part_head() -> list[Box]:
    """锄板：一块又宽又薄、从方銎顶向 -X 折成直角的铁板（参考图那块折过来的铁板）。

    尺寸按调度审定的（px，1px = 1/16 方块）：板宽 6、伸出方銎之外 5、厚 1。
    从銎口往外三级：銎顶的 2px 小舌 → 4px 宽的过渡 → 6px 宽的主板，所以是**越往前越宽**的
    一块铁板，不是一个小方块。主板最前沿 1px 切成三小块、中间留两道缺口；
    前缘那三块用带锈斑的 `hoe_edge`，缺口与锈斑就是「磨了很多年的刨地口」。
    相邻几块的 x 头尾相接、y 完全齐平——侧面共面但投影不相交，不会 z-fighting。
    """
    top_y = SOCKET_TOP + PLATE_THICK_PX * PX
    mid_y = (SOCKET_TOP + top_y) / 2.0
    half_t = PLATE_THICK_PX * PX / 2.0
    eye = EYE_PX / 2.0 * PX
    reach = PLATE_REACH_PX * PX
    half_w = PLATE_WIDTH_PX / 2.0 * PX
    edge = PX                                   # 前缘 1px
    main_x1 = -eye - reach + edge               # 主板的前沿（缺口前缘之后）
    mid_x0 = -eye - 2.0 * PX                    # 过渡段占銎外侧前 2px
    boxes = [
        Box("plate_tab", "hoe_iron", (0.0, mid_y, 0.0), (eye, half_t, eye)),
        Box("plate_neck", "hoe_iron", ((mid_x0 - eye) / 2.0, mid_y, 0.0),
            ((-eye - mid_x0) / 2.0, half_t, 2.0 * PX)),
        Box("plate_main", "hoe_iron", ((mid_x0 + main_x1) / 2.0, mid_y, 0.0),
            ((mid_x0 - main_x1) / 2.0, half_t, half_w)),
    ]
    # 前缘 1px 切成三块，块间两道缺口，宽度不等：等宽缺口读成梳齿
    spans = (
        (-half_w, -half_w + 1.5 * PX),
        (-half_w + 2.5 * PX, -half_w + 4.25 * PX),
        (-half_w + 5.25 * PX, half_w),
    )
    for index, (z0, z1) in enumerate(spans):
        boxes.append(Box(f"plate_edge_{index}", "hoe_edge", (main_x1 - edge / 2.0, mid_y, (z0 + z1) / 2.0),
                         (edge / 2.0, half_t, (z1 - z0) / 2.0)))
    # 主板两侧各立一道 1.5px 高的翻边：整块板读成浅浅的一块「畚」（参考图里内凹的锄板），
    # 不再是一块平放的铁片。翻边只在主板范围内，不盖前缘的缺口。
    lip_h = LIP_PX * PX
    for name, side in (("plate_lip_front", -1.0), ("plate_lip_back", 1.0)):
        boxes.append(Box(name, "hoe_iron", ((mid_x0 + main_x1) / 2.0, top_y + lip_h / 2.0, side * (half_w - LIP_THICK / 2.0)),
                         ((mid_x0 - main_x1) / 2.0 - 0.004, lip_h / 2.0, LIP_THICK / 2.0)))
    return boxes


def all_boxes() -> tuple[Box, ...]:
    return tuple(part_butt() + part_wrap() + part_haft() + part_socket() + part_head())


BAO_CHU = HeldItem(
    key=ITEM_ID,
    display_name="刨锄",
    host_item="iron_hoe",     # 登记意图；本次只落建模源，不接线，宿主见 plan-held-item-registration-v1
    boxes=all_boxes(),
    materials=(
        Material("hoe_wood", (0.53, 0.39, 0.24), tex_hoe_wood()),
        Material("hoe_iron", (0.36, 0.34, 0.33), tex_hoe_iron()),
        Material("hoe_edge", (0.54, 0.52, 0.49), tex_hoe_edge()),
        Material("hoe_cord", (0.57, 0.50, 0.37), tex_hoe_cord()),
    ),
    grip=GRIP,
    display=hand_display(SCALE, GRIP, LENGTH),
)


def items() -> tuple[HeldItem, ...]:
    return (BAO_CHU,)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-check", action="store_true", help="只写 bbmodel，不出 check.png")
    parser.add_argument("--dump-obj", action="store_true", help="OBJ/MTL 打到 stdout")
    parser.add_argument("--dump-display", action="store_true", help="model JSON（含 display）打到 stdout")
    args = parser.parse_args()

    if args.dump_display:
        print(build_model_json(BAO_CHU), end="")
        return
    if args.dump_obj:
        print(build_obj(BAO_CHU))
        return

    outputs = write_assets(items(), bbmodel_dir=BBMODEL_DIR, client_resources=None, render_previews=False)
    bbmodel = outputs[f"bbmodel:{ITEM_ID}"]
    print(f"[bbmodel] {bbmodel.relative_to(REPO)}")
    if not args.no_check:
        print(f"[check] {write_check_sheet(bbmodel, ITEM_ID)}")


if __name__ == "__main__":
    main()
