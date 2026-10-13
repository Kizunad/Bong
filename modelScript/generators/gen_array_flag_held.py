#!/usr/bin/env python3
"""阵旗 array_flag（手持短旗）：握柄 + 短旗杆 + 褪色旗幡 + 杆头小骨饰。bbmodel + SML OBJ 双出。

物品依据：`server/assets/items/zhenfa.toml` 的 `array_flag`（tool / uncommon / 1×2）——
「地师用来牵引阵眼气机的短旗。可用于布雷、警戒与主动引爆。」所以是**手里握着**的短旗，
不是插地的那种；旗幡上有一道弱的青白阵纹，杆头一小截骨饰，其余都是旧物。

和 `gen_array_flag.py` 的区别：那份产 `ArrayFlagBasic.bbmodel`，对应 `array_flag_basic`
（阵旗·凡，插地旗，木桩底座 + 骨钉压脚）。本件不复用它的几何，只借它的**风格**：旧木杆、
褪色粗布、青白弱光阵纹、骨饰。

参考图：`model-review/img/handheld/exploded/array_flag.png`（本机 CPA 出的爆炸图）。

## 分件（从握把末端到尖端）

    part_pommel   柄尾骨节          y 0 ~ 0.04
    part_grip     缠革握柄 + 两道箍   y 0.04 ~ 0.27
    part_collar   骨环              y 0.27 ~ 0.31，握柄与旗杆之间的一圈
    part_pole     旗杆 两段          y 0.31 ~ 0.76，往上略收
    part_banner   旗幡 + 两道皮绑带   y 0.43 ~ 0.72，从杆头附近往 -X 伸出，边缘参差
    part_rune     阵纹 一竖两横 + 方环   穿过旗幡、两面都鼓出一点，贴图是青白弱光
    part_finial   杆头骨饰           y 0.76 ~ 0.918，骨节 + 三节往 +X 勾回的骨刺

## 方向约定

握把末端 y=0、尖端 +Y（`assert_conventions`）；旗幡伸向 **-X** 侧，和 `bao_chu` 的锄板、
两把 v2 斧的刃同向，以后挥旗 / 举旗动画按同一侧调。旗面是竖的薄板（厚在 z），不是平放的。

## 贴图未经真机标定

同 `gen_wooden_club`：走 SML item 光照，本 harness 跑不了 `render_held_item.py`，
色值照参考图取，系数是经验值。阵纹是「弱光」：贴图里只是比旗布亮很多的青白，没有发光层。
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
from handheld_check import review_available, write_check_sheet  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
BBMODEL_DIR = Path(__file__).resolve().parents[1] / "models"

ITEM_ID = "array_flag"
LENGTH = 0.918          # 全长（授权系，方块）：柄尾到骨刺钩尖
GRIP = 0.15             # 拳心对准缠革握柄的中段
SCALE = 0.90            # 显示缩放 → 手里约 0.78 方块


# ── 贴图 ──────────────────────────────────────────────────────────────────
# 每张 16²，OBJ 链每个面整张铺满，所以画通用材质样本（同 gen_wooden_club）。


def tex_flag_wood() -> Image.Image:
    """旗杆：发黑的旧木，深褐，纵向纤维两条。"""
    rng = random.Random(0xAF01)
    img = Image.new("RGBA", (16, 16), (84, 60, 42, 255))
    noise_fill(img, rng, (84, 60, 42), 7, warm=3)
    blotch(img, rng, 3, (62, 42, 28), (2.0, 4.0))
    blotch(img, rng, 3, (108, 80, 56), (1.6, 3.2))
    pixels = img.load()
    for x in (4, 11):
        for y in range(16):
            if (x * 5 + y * 3) % 4:
                pixels[x, y] = (68, 48, 33, 255)
    return img


def tex_flag_cloth() -> Image.Image:
    """旗布：褪色的赭褐粗布，经纬格纹很淡，带几块泛白的磨损。"""
    rng = random.Random(0xAF02)
    img = Image.new("RGBA", (16, 16), (150, 114, 70, 255))
    noise_fill(img, rng, (150, 114, 70), 8, warm=3)
    pixels = img.load()
    for step in range(0, 16, 4):                       # 经纬：很淡的格
        for k in range(16):
            for x, y in ((step, k), (k, step)):
                r, g, b, _ = pixels[x, y]
                pixels[x, y] = (r - 9, g - 8, b - 6, 255)
    blotch(img, rng, 4, (178, 148, 104), (1.8, 3.6))   # 磨白
    blotch(img, rng, 3, (112, 78, 46), (1.6, 3.0))     # 污渍
    return img


def tex_flag_bone() -> Image.Image:
    """骨饰：泛黄的老骨，米白带暗黄斑。"""
    rng = random.Random(0xAF03)
    img = Image.new("RGBA", (16, 16), (214, 200, 168, 255))
    noise_fill(img, rng, (214, 200, 168), 7, warm=2)
    blotch(img, rng, 4, (178, 160, 124), (1.6, 3.2))
    blotch(img, rng, 2, (236, 226, 198), (1.4, 2.6))
    return img


def tex_flag_rune() -> Image.Image:
    """阵纹：青白弱光，比旗布亮很多、偏冷；中心更亮，没有发光层所以靠对比。"""
    rng = random.Random(0xAF04)
    img = Image.new("RGBA", (16, 16), (150, 214, 224, 255))
    noise_fill(img, rng, (150, 214, 224), 6)
    blotch(img, rng, 3, (210, 244, 248), (2.0, 4.0))
    return img


def tex_flag_leather() -> Image.Image:
    """缠革 / 绑带：深褐旧皮，斜向缠纹。"""
    rng = random.Random(0xAF05)
    img = Image.new("RGBA", (16, 16), (92, 66, 46, 255))
    noise_fill(img, rng, (92, 66, 46), 7, warm=3)
    pixels = img.load()
    for start in range(-8, 16, 3):
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (68, 48, 33, 255)
    for start in range(-6, 16, 6):
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (122, 92, 64, 255)
    return img


# ── 分件 ──────────────────────────────────────────────────────────────────


def part_pommel() -> list[Box]:
    """柄尾骨节：比握柄粗一圈的一块骨，端面落在 y=0。"""
    return [Box("pommel", "flag_bone", (0.0, 0.0200, 0.0), (0.0480, 0.0200, 0.0460))]


def part_grip() -> list[Box]:
    """缠革握柄：主体一根木芯 + 外面三段错位的缠革，上下各一道深色箍。"""
    return [
        Box("grip_core", "flag_wood", (0.0, 0.1550, 0.0), (0.0380, 0.1190, 0.0360)),
        Box("grip_wrap_a", "flag_leather", (0.0020, 0.0850, -0.0010), (0.0440, 0.0350, 0.0420)),
        Box("grip_wrap_b", "flag_leather", (-0.0020, 0.1550, 0.0020), (0.0430, 0.0350, 0.0410)),
        Box("grip_wrap_c", "flag_leather", (0.0010, 0.2250, -0.0020), (0.0445, 0.0350, 0.0425)),
        Box("grip_band_low", "flag_wood", (0.0, 0.0500, 0.0), (0.0475, 0.0100, 0.0452)),
        Box("grip_band_high", "flag_wood", (0.0, 0.2650, 0.0), (0.0470, 0.0100, 0.0448)),
    ]


def part_collar() -> list[Box]:
    """骨环：握柄与旗杆之间的一圈骨节。"""
    return [Box("collar", "flag_bone", (0.0, 0.2900, 0.0), (0.0500, 0.0200, 0.0480))]


def part_pole() -> list[Box]:
    """旗杆两段，往上略收；段间半宽错开，避免相邻段侧面共面。"""
    return [
        Box("pole_low", "flag_wood", (0.0, 0.4650, 0.0), (0.0360, 0.1550, 0.0340)),
        Box("pole_high", "flag_wood", (0.0, 0.6900, 0.0), (0.0340, 0.0700, 0.0320)),
    ]


def part_banner() -> list[Box]:
    """旗幡：四列竖板自杆边往 -X 伸出，越往外越矮，尾缘再挂两块参差的碎边。

    两道皮绑带箍在杆上、压住旗幡根部，读成「旗是绑在杆上的」。板厚在 z（竖旗），
    各列厚度差 0.001~0.002，避免相邻列的 z 面共面。
    """
    return [
        Box("banner_0", "flag_cloth", (-0.0750, 0.5850, 0.0), (0.0450, 0.1350, 0.0156)),
        Box("banner_1", "flag_cloth", (-0.1700, 0.6000, 0.0), (0.0500, 0.1200, 0.0140)),
        Box("banner_2", "flag_cloth", (-0.2600, 0.6100, 0.0), (0.0400, 0.0900, 0.0165)),
        Box("banner_3", "flag_cloth", (-0.3300, 0.6200, 0.0), (0.0300, 0.0600, 0.0145)),
        Box("tatter_low", "flag_cloth", (-0.2350, 0.4900, 0.0), (0.0250, 0.0300, 0.0150)),
        Box("tatter_tail", "flag_cloth", (-0.3650, 0.6000, 0.0), (0.0200, 0.0180, 0.0140)),
        Box("strap_low", "flag_leather", (0.0, 0.4600, 0.0), (0.0440, 0.0150, 0.0420)),
        Box("strap_high", "flag_leather", (0.0, 0.7080, 0.0), (0.0440, 0.0145, 0.0420)),
    ]


def part_rune() -> list[Box]:
    """阵纹：一竖两横 + 套在十字心上的一圈方环，穿透旗幡、两面各鼓出 0.003，所以两面都看得见。

    方环是参考图里那圈圆环的体素化（四条细边围成方框）；各条的 z 半宽互不相同
    （0.0191~0.0199），相邻条的侧面不共面。
    """
    ring_x, ring_y, ring_half = -0.1450, 0.5850, 0.0300
    return [
        Box("rune_stem", "flag_rune", (-0.1450, 0.5900, 0.0), (0.0070, 0.0900, 0.0195)),
        Box("rune_arm_high", "flag_rune", (-0.1450, 0.6450, 0.0), (0.0650, 0.0070, 0.0192)),
        Box("rune_arm_low", "flag_rune", (-0.1450, 0.5500, 0.0), (0.0400, 0.0070, 0.0198)),
        Box("rune_ring_top", "flag_rune", (ring_x, ring_y + ring_half, 0.0), (ring_half, 0.0055, 0.0191)),
        Box("rune_ring_bottom", "flag_rune", (ring_x, ring_y - ring_half, 0.0), (ring_half, 0.0055, 0.0193)),
        Box("rune_ring_left", "flag_rune", (ring_x - ring_half, ring_y, 0.0), (0.0055, ring_half, 0.0197)),
        Box("rune_ring_right", "flag_rune", (ring_x + ring_half, ring_y, 0.0), (0.0055, ring_half, 0.0199)),
    ]


def part_finial() -> list[Box]:
    """杆头骨饰：一节骨 + 一枚向上、尖端向 +X 勾回的骨刺（参考图里那枚弯骨刺的方块化）。

    刺分三节，每节往 +X 偏一点、越往上越细，顶上再多一小块往 +X 探出——侧着看是个弯钩。
    """
    return [
        Box("finial_knob", "flag_bone", (0.0, 0.7850, 0.0), (0.0460, 0.0250, 0.0440)),
        Box("finial_spike_low", "flag_bone", (0.0, 0.8350, 0.0), (0.0240, 0.0250, 0.0230)),
        Box("finial_spike_mid", "flag_bone", (0.0060, 0.8750, 0.0), (0.0180, 0.0150, 0.0170)),
        Box("finial_spike_tip", "flag_bone", (0.0140, 0.9000, 0.0), (0.0110, 0.0100, 0.0120)),
        Box("finial_spike_hook", "flag_bone", (0.0230, 0.9120, 0.0), (0.0090, 0.0060, 0.0100)),
    ]


def all_boxes() -> tuple[Box, ...]:
    return tuple(
        part_pommel() + part_grip() + part_collar() + part_pole()
        + part_banner() + part_rune() + part_finial()
    )


ARRAY_FLAG = HeldItem(
    key=ITEM_ID,
    display_name="阵旗",
    host_item="stick",       # 登记意图；本次只落建模源，不接线，宿主见 plan-held-item-registration-v1
    boxes=all_boxes(),
    materials=(
        Material("flag_wood", (0.33, 0.24, 0.17), tex_flag_wood()),
        Material("flag_cloth", (0.59, 0.45, 0.27), tex_flag_cloth()),
        Material("flag_bone", (0.84, 0.78, 0.66), tex_flag_bone()),
        Material("flag_rune", (0.59, 0.84, 0.88), tex_flag_rune()),
        Material("flag_leather", (0.36, 0.26, 0.18), tex_flag_leather()),
    ),
    grip=GRIP,
    display=hand_display(SCALE, GRIP, LENGTH),
)


def items() -> tuple[HeldItem, ...]:
    return (ARRAY_FLAG,)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--no-check", action="store_true", help="只写 bbmodel，不出 check.png")
    parser.add_argument("--dump-obj", action="store_true", help="OBJ/MTL 打到 stdout")
    parser.add_argument("--dump-display", action="store_true", help="model JSON（含 display）打到 stdout")
    args = parser.parse_args()

    if args.dump_display:
        print(build_model_json(ARRAY_FLAG), end="")
        return
    if args.dump_obj:
        print(build_obj(ARRAY_FLAG))
        return

    outputs = write_assets(items(), bbmodel_dir=BBMODEL_DIR, client_resources=None, render_previews=False)
    bbmodel = outputs[f"bbmodel:{ITEM_ID}"]
    print(f"[bbmodel] {bbmodel.relative_to(REPO)}")
    if not args.no_check and review_available():
        print(f"[check] {write_check_sheet(bbmodel, ITEM_ID)}")


if __name__ == "__main__":
    main()
