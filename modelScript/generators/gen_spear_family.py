#!/usr/bin/env python3
"""矛族三件：粗石矛 crude_stone_spear / 骨尖矛 bone_tip_spear / 锈铁矛 rusted_iron_spear。

依据 `docs/plans-skeleton/plan-craft-chain-items-v1.md`「族 A：矛族」：三档矛填 `WeaponKind::Spear` 整系空档，
**三矛必须远观可辨**——石矛灰白粗石头、骨矛微弯骨尖 + 暗色缠扎、铁矛锈红宽刃，差异在部件层做出来，不靠贴图微调。
模板 id 用 plan 草案里的 `crude_stone_spear` / `bone_tip_spear` / `rusted_iron_spear`。

**三件共用矛杆 / 绑缚 / 矛尾，只换矛头**（以及绑缚材质：石矛是麻绳，骨矛 / 铁矛是暗色蛛丝绳）：

    part_butt()      矛尾木帽
    part_shaft()     矛杆：三段略有粗细和偏移的木杆（2×2 削四角的近圆）+ 几个木疖
    part_grip()      下三分之一的握把缠绳
    part_socket()    杆头劈开的两瓣（各 1px 厚），夹住中间 1px 厚的矛头
    part_lashing()   杆头外的十字交叉缠绳（两道斜向绳带交叉）+ 上下各一圈收口
    part_head_*()    矛头（每件各一个）

单位：授权系（见 `held_item_common`）——握把末端在 y=0、尖端朝 +Y，单位 = 方块（1.0 = 16px）。下面凡写 `px`
的尺寸都用 `PX = 1/16` 换算。全长 1.1925（手持物合理区间上限 1.2），握把中点 y=0.32。

## 第 1 次审的修改（2026-10-12）

- 矛头：由胖菱形改成**宽 3px、长 7px、厚 1px 的柳叶形**，最宽处在下 1/3，两侧边缘各打 2~3 个 1px 缺口；
- 矛头**插进劈开的杆头**：杆头两瓣各 1px 夹住矛头；
- 绑法：由平行绳圈改成**十字交叉缠绳**（前后两面各两道斜向绳带交叉）；
- 矛杆截面：2×2 方改成 **2×2 削掉四角的近圆**（每角削 0.5px，用「宽 2 × 厚 1」+「宽 1 × 厚 2」两块十字相叠）。

用法：
    python gen_spear_family.py                 # 出 bbmodel + 单件三视图预览
    python gen_spear_family.py --check         # 另出审阅图 model-review/img/handheld/<id>/check.png
"""

from __future__ import annotations

import argparse
import random
import sys
from pathlib import Path

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))

from bbmodel_maker.render.held_item_common import (  # noqa: E402
    Box,
    HeldItem,
    Material,
    blotch,
    build_model_json,
    build_mtl,
    build_obj,
    hand_display,
    noise_fill,
    write_assets,
)

REPO = Path(__file__).resolve().parents[2]
BBMODEL_DIR = Path(__file__).resolve().parents[1] / "models"
PREVIEW_DIR = Path(__file__).resolve().parents[1] / "out"
REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld")

PX = 1.0 / 16.0
LENGTH = 1.1925         # 全长（授权系，方块）：矛头顶端
GRIP = 0.32             # 拳心对准的高度：握把缠绳段中点
SCALE = 0.88            # 手里显示缩放（双手长兵，比单手剑略小）

SHAFT_Y = (0.03, 0.74)  # 矛杆段（尾帽之上、杆头劈口之下）
SOCKET_Y = (0.74, 0.86)  # 杆头劈开的两瓣
HEAD_Y0 = 0.755         # 矛头最下端（插进杆头 0.105）
ROW = 0.5 * PX          # 矛头逐行高度（0.5px，共 14 行 = 7px）


# ── 贴图（16²，通用材质样本；OBJ 链每个面整张铺满） ───────────────────────────


def tex_spear_wood() -> Image.Image:
    """矛杆木：去皮硬木，暖褐、纵向纤维沟。纵纹只画两条（柄面只有十来像素，多了读成条纹布）。"""
    rng = random.Random(0x5E01)
    img = Image.new("RGBA", (16, 16), (128, 92, 58, 255))
    noise_fill(img, rng, (128, 92, 58), 8, warm=3)
    blotch(img, rng, 4, (104, 74, 46), (2.0, 4.0))
    blotch(img, rng, 3, (150, 110, 72), (1.6, 3.2))
    pixels = img.load()
    for x in (4, 11):
        for y in range(16):
            if (x * 5 + y * 3) % 4:
                pixels[x, y] = (110, 79, 49, 255)
    return img


def tex_spear_hemp() -> Image.Image:
    """麻绳：搓过的粗麻，浅黄褐、斜向拧纹。"""
    rng = random.Random(0x5E02)
    img = Image.new("RGBA", (16, 16), (176, 146, 94, 255))
    noise_fill(img, rng, (176, 146, 94), 8, warm=3)
    pixels = img.load()
    for start in range(-8, 16, 3):
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (143, 116, 72, 255)
    for start in range(-6, 16, 6):
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (201, 172, 120, 255)
    return img


def tex_spear_stone() -> Image.Image:
    """粗石矛头：灰白燧石，打片留下的明暗大斑，边缘更暗的崩疤。"""
    rng = random.Random(0x5E03)
    img = Image.new("RGBA", (16, 16), (150, 148, 142, 255))
    noise_fill(img, rng, (150, 148, 142), 12, warm=1)
    blotch(img, rng, 6, (186, 184, 176), (1.8, 3.6))       # 打片的亮面
    blotch(img, rng, 5, (112, 110, 106), (1.4, 3.0))       # 崩疤的暗面
    blotch(img, rng, 2, (96, 92, 90), (0.9, 1.6))
    return img


def tex_spear_silk() -> Image.Image:
    """蛛丝绳：暗褐灰、紧拧的细绳，斜纹比麻绳更密更细，整体压暗（骨矛 / 铁矛的绑缚色）。"""
    rng = random.Random(0x5E04)
    img = Image.new("RGBA", (16, 16), (74, 58, 50, 255))
    noise_fill(img, rng, (74, 58, 50), 6, warm=2)
    pixels = img.load()
    for start in range(-8, 16, 2):
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (54, 41, 36, 255)
    for start in range(-5, 16, 5):
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (101, 82, 70, 255)
    return img


def tex_spear_bone() -> Image.Image:
    """骨尖：象牙白微黄，纵向骨纤维，少量暗孔（骨髓腔的小凹）。"""
    rng = random.Random(0x5E05)
    img = Image.new("RGBA", (16, 16), (222, 211, 184, 255))
    noise_fill(img, rng, (222, 211, 184), 7, warm=2)
    blotch(img, rng, 4, (240, 232, 208), (1.6, 3.2))
    blotch(img, rng, 3, (196, 182, 152), (1.4, 2.8))
    pixels = img.load()
    for x in (5, 10):                                      # 纵向纤维
        for y in range(16):
            if (x * 3 + y * 5) % 3:
                pixels[x, y] = (206, 193, 164, 255)
    for _ in range(3):                                     # 暗孔
        pixels[rng.randint(1, 14), rng.randint(1, 14)] = (150, 135, 108, 255)
    return img


def tex_spear_bone_dark() -> Image.Image:
    """骨尖上的血槽 / 缺口暗面：烟熏过的深褐骨。"""
    rng = random.Random(0x5E06)
    img = Image.new("RGBA", (16, 16), (126, 108, 84, 255))
    noise_fill(img, rng, (126, 108, 84), 8, warm=2)
    blotch(img, rng, 3, (98, 82, 62), (1.2, 2.4))
    return img


# ── 共用部件 ─────────────────────────────────────────────────────────────


def _box(name: str, material: str, y0: float, y1: float, x: float, z: float, hx: float, hz: float) -> Box:
    return Box(name, material, (round(x, 5), round((y0 + y1) / 2.0, 5), round(z, 5)),
               (round(hx, 5), round((y1 - y0) / 2.0, 5), round(hz, 5)))


def part_butt() -> list[Box]:
    """矛尾木帽：比矛杆略粗的一截钝木，也是握把的尾端止滑。"""
    return [_box("butt", "wood", 0.0, 0.030, 0.0, 0.0, 1.15 * PX, 1.15 * PX)]


def _octagon(name: str, material: str, y0: float, y1: float, x: float, z: float, k: float = 1.0) -> list[Box]:
    """2×2 削掉四角（每角 0.5px）的近圆截面：「宽 2 × 厚 1」+「宽 1 × 厚 2」两块十字相叠，高度错开一点点，
    免得两块的上下面共面。`k` 是整体粗细系数。"""
    return [
        _box(f"{name}_a", material, y0, y1, x, z, 1.0 * PX * k, 0.5 * PX * k),
        _box(f"{name}_b", material, y0 + 0.0033, y1 - 0.0033, x, z, 0.5 * PX * k, 1.0 * PX * k),
    ]


def part_shaft() -> list[Box]:
    """矛杆：三段略有粗细和偏移的木杆（天然木杆不是车出来的直棍），加几个木疖。"""
    segments = (
        # y0     y1     x 偏     z 偏     粗细系数
        (0.030, 0.300, +0.002, -0.001, 1.00),
        (0.300, 0.540, -0.003, +0.002, 0.95),
        (0.540, 0.740, +0.002, -0.002, 0.90),
    )
    boxes: list[Box] = []
    for i, (y0, y1, x, z, k) in enumerate(segments):
        boxes += _octagon(f"shaft_{i}", "wood", y0, y1, x, z, k)
    # 木疖：杆上鼓出的几小块，错开方向和高度
    boxes.append(_box("knot_a", "wood", 0.455, 0.490, 1.0 * PX * 0.95 + 0.006, 0.004, 0.011, 0.016))
    boxes.append(_box("knot_b", "wood", 0.620, 0.652, -0.002, 1.0 * PX * 0.90 + 0.006, 0.014, 0.011))
    boxes.append(_box("knot_c", "wood", 0.110, 0.140, -1.0 * PX - 0.005, -0.003, 0.011, 0.014))
    return boxes


def part_grip(cord: str) -> list[Box]:
    """握把缠绳：下三分之一四道，厚薄与偏向参差（等厚等宽读成车出来的凹槽）。"""
    coils = (
        # y0     y1     半宽 px  x 偏    z 偏
        (0.200, 0.246, 1.28, -0.003, +0.002),
        (0.252, 0.296, 1.20, +0.004, -0.003),
        (0.302, 0.348, 1.30, -0.003, +0.003),
        (0.354, 0.398, 1.18, +0.003, -0.002),
    )
    return [_box(f"grip_{i}", cord, y0, y1, x, z, hw * PX, hw * PX)
            for i, (y0, y1, hw, x, z) in enumerate(coils)]


def part_socket() -> list[Box]:
    """杆头劈开的两瓣：各 1px 厚（z 向）、2px 宽，夹住中间 1px 厚的矛头。

    两瓣的外侧面是 z = ±1.5px，矛头占 z ±0.5px，两瓣内侧面正好贴在矛头两面上。
    """
    return [_box(f"socket_{side}", "wood", SOCKET_Y[0], SOCKET_Y[1], 0.0, sz * 1.0 * PX, 1.05 * PX, 0.5 * PX)
            for side, sz in (("f", +1.0), ("b", -1.0))]


def part_lashing(cord: str) -> list[Box]:
    """十字交叉缠绳：前后两面各两道斜向绳带交叉（一道左下→右上，一道右下→左上），
    上下各一圈收口。两道斜带凸出量不同，交叉处才不共面。

    每道斜带 4 级台阶，每级 `0.667px` 横移、`0.5px` 高：socket 高 0.12≈1.9px，正好走满。
    """
    y_lo, y_hi = SOCKET_Y
    boxes: list[Box] = []
    step_h = (y_hi - y_lo - 0.016) / 4.0
    for face, sz in (("f", +1.0), ("b", -1.0)):
        for band, (direction, proud) in enumerate(((+1.0, 1.62), (-1.0, 1.78))):
            for i in range(4):
                y0 = y_lo + 0.008 + i * step_h + band * 0.004   # 两道带错开一点，免得上下面共面
                x = direction * (-1.0 + i * 0.667) * PX
                boxes.append(_box(f"lash_{face}{band}_{i}", cord, y0, y0 + step_h, x, sz * proud * PX, 0.55 * PX, 0.18 * PX))
    # 上下收口圈：整圈裹住两瓣 + 矛头，只比杆头外沿凸一点
    for name, (y0, y1) in (("lash_ring_lo", (SOCKET_Y[0] - 0.003, SOCKET_Y[0] + 0.009)),
                           ("lash_ring_hi", (SOCKET_Y[1] - 0.009, SOCKET_Y[1] + 0.003))):
        boxes.append(_box(name, cord, y0, y1, 0.0, 0.0, 1.22 * PX, 1.66 * PX))
    return boxes


# ── 矛头 ─────────────────────────────────────────────────────────────────


# 柳叶形：14 行 × 0.5px = 7px 长，最宽 3px 出现在第 4~5 行（下 1/3），厚 1px（z ±0.5px），尖端逐渐变薄。
_LEAF_WIDTH_PX = (1.0, 1.6, 2.3, 2.8, 3.0, 3.0, 2.9, 2.7, 2.4, 2.0, 1.6, 1.2, 0.8, 0.4)
_LEAF_HALF_THICK_PX = (0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.45, 0.4, 0.35, 0.3)
# 缺口：每个缺口占 2 行（= 1px 高），向内啃 0.5px；左侧 3 个、右侧 2 个，错开
_NOTCH_LEFT = ((2, 3), (6, 7), (10, 11))
_NOTCH_RIGHT = ((4, 5), (8, 9))


def part_head_stone() -> list[Box]:
    """粗石矛头：**细长柳叶形的打制石片**——宽 3px、长 7px、厚 1px，最宽处在下 1/3，
    两侧边缘打出 2~3 个 1px 缺口（左 3 右 2，错开），轮廓参差，一眼是打制石片。

    逐行 0.5px 叠成；缺口那两行，对应一侧边缘向内收 0.5px（行中心偏 0.25px、半宽减 0.25px）。
    """
    boxes: list[Box] = []
    for i, (w, ht) in enumerate(zip(_LEAF_WIDTH_PX, _LEAF_HALF_THICK_PX)):
        half = w / 2.0
        centre = 0.0
        if any(i in pair for pair in _NOTCH_LEFT):
            half -= 0.25
            centre += 0.25
        if any(i in pair for pair in _NOTCH_RIGHT):
            half -= 0.25
            centre -= 0.25
        y0 = HEAD_Y0 + i * ROW
        boxes.append(_box(f"stone_row_{i:02d}", "stone", y0, y0 + ROW, centre * PX, 0.0, half * PX, ht * PX))
    return boxes


# 骨尖：比石头细、**向一侧微弯**、右侧边缘有锯齿状的骨刺。同样 14 行 × 0.5px = 7px 长、厚 1px（z ±0.5px），
# 才能插进同一副两瓣杆头。最宽 2.2px 在下 1/3，往尖端收；弯：行中心按 (i/13)^1.6 向 -x 偏，到尖端偏 1.1px。
_BONE_WIDTH_PX = (1.2, 1.7, 2.0, 2.2, 2.2, 2.1, 2.0, 1.8, 1.6, 1.4, 1.2, 0.9, 0.6, 0.3)
_BONE_CURVE_PX = 1.1
_BONE_TEETH = (3, 5, 7, 9)          # 右缘锯齿所在行：向外多凸 0.3px


def part_head_bone() -> list[Box]:
    """骨尖矛头：**微弯的骨尖**——比石矛头窄（最宽 2.2px）、向 -x 微弯 1.1px，右缘四个锯齿状骨刺，
    中线一道暗色血槽。厚度仍是 1px，插进同一副两瓣杆头；弯度让它和直刃的石 / 铁矛头远观可辨。
    """
    boxes: list[Box] = []
    for i, w in enumerate(_BONE_WIDTH_PX):
        half = w / 2.0
        centre = -_BONE_CURVE_PX * (i / 13.0) ** 1.6
        if i in _BONE_TEETH:
            half += 0.15
            centre += 0.15
        y0 = HEAD_Y0 + i * ROW
        boxes.append(_box(f"bone_row_{i:02d}", "bone", y0, y0 + ROW, centre * PX, 0.0, half * PX, 0.5 * PX))
    # 血槽：中线一条暗色细带，正反两面各一，只画中段（行 2~11），微凸 0.1px（半埋进矛头 0.02px 以保持连通）
    for i in range(2, 12):
        centre = -_BONE_CURVE_PX * (i / 13.0) ** 1.6
        y0 = HEAD_Y0 + i * ROW
        for face, sz in (("f", +1.0), ("b", -1.0)):
            boxes.append(_box(f"bone_groove_{face}{i:02d}", "bone_dark", y0, y0 + ROW, centre * PX, sz * 0.54 * PX, 0.2 * PX, 0.06 * PX))
    return boxes


# ── 整件 ─────────────────────────────────────────────────────────────────


def all_boxes_for(key: str) -> tuple[Box, ...]:
    """每件：(绑缚材质, 矛头部件)。石矛是麻绳，骨矛 / 铁矛是暗色蛛丝绳。"""
    cord, head = {
        "crude_stone_spear": ("hemp", part_head_stone),
        "bone_tip_spear": ("silk", part_head_bone),
    }[key]
    return tuple(part_butt() + part_shaft() + part_grip(cord) + part_socket() + part_lashing(cord) + head())


def _item(key: str, name: str, materials: tuple[Material, ...]) -> HeldItem:
    return HeldItem(
        key=key,
        display_name=name,
        host_item="trident",        # 宿主只是登记意图：接线另做（plan §P2 要查 vanillaModelPaths 防撞）
        boxes=all_boxes_for(key),
        materials=materials,
        grip=GRIP,
        display=hand_display(SCALE, GRIP, LENGTH, gui_spin=45.0),
    )


def items() -> tuple[HeldItem, ...]:
    wood = Material("wood", (0.50, 0.36, 0.23), tex_spear_wood())
    hemp = Material("hemp", (0.69, 0.57, 0.37), tex_spear_hemp())
    silk = Material("silk", (0.29, 0.23, 0.20), tex_spear_silk())
    stone = Material("stone", (0.59, 0.58, 0.56), tex_spear_stone())
    bone = Material("bone", (0.87, 0.83, 0.72), tex_spear_bone())
    bone_dark = Material("bone_dark", (0.49, 0.42, 0.33), tex_spear_bone_dark())
    return (
        _item("crude_stone_spear", "粗石矛", (wood, hemp, stone)),
        _item("bone_tip_spear", "骨尖矛", (wood, silk, bone, bone_dark)),
    )


# ── 审阅图 ───────────────────────────────────────────────────────────────


def render_check(item: HeldItem, bbmodel: Path) -> Path:
    """model-review/img/handheld/<id>/check.png：左爆炸参考图，右渲染 3/4 + 侧视 + 矛头特写。"""
    from bbmodel_maker.render.render_bbmodel import render

    out_dir = REVIEW_ROOT / item.key
    out_dir.mkdir(parents=True, exist_ok=True)
    bg = (119, 119, 119)
    iso, _ = render(bbmodel, yaw=-35.0, pitch=25.0, size=640, bg=bg)
    side, _ = render(bbmodel, yaw=90.0, pitch=0.0, size=640, bg=bg)
    front, _ = render(bbmodel, yaw=0.0, pitch=0.0, size=640, bg=bg)

    ref = Image.open(REVIEW_ROOT / "exploded" / f"{item.key}.png").convert("RGB")
    h = 640
    ref = ref.resize((int(ref.width * h / ref.height), h), Image.Resampling.LANCZOS)
    canvas = Image.new("RGB", (ref.width + 3 * 640 + 60, h + 36), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)
    canvas.paste(ref, (12, 30))
    draw.text((16, 8), f"EXPLODED REFERENCE ({item.key}.png)", fill=(210, 200, 180))
    for i, (title, im) in enumerate((("NOW RENDER (3/4 VIEW)", iso), ("SIDE VIEW", side), ("FRONT VIEW", front))):
        x = ref.width + 24 + i * 652
        canvas.paste(im, (x, 30))
        draw.text((x + 4, 8), title, fill=(180, 220, 210))
    path = out_dir / "check.png"
    canvas.save(path)
    return path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-preview", action="store_true", help="只写 bbmodel，不出图")
    parser.add_argument("--check", action="store_true", help="另出 model-review 审阅图 check.png")
    parser.add_argument("--dump-obj", action="store_true", help="OBJ/MTL 打到 stdout")
    parser.add_argument("--dump-display", action="store_true", help="model JSON（含 display 变换）打到 stdout")
    args = parser.parse_args()

    if args.dump_display:
        for item in items():
            print(build_model_json(item), end="")
        return
    if args.dump_obj:
        for item in items():
            print(build_obj(item))
            print(build_mtl(item))
        return

    outputs = write_assets(items(), bbmodel_dir=BBMODEL_DIR, client_resources=None,
                           preview_dir=PREVIEW_DIR, render_previews=not args.no_preview)
    for key, path in outputs.items():
        print(f"[{key}] {path.relative_to(REPO)}")
    if args.check:
        for item in items():
            print(f"[check:{item.key}] {render_check(item, outputs[f'bbmodel:{item.key}'])}")


if __name__ == "__main__":
    main()
