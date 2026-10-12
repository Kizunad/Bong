#!/usr/bin/env python3
"""矛族三件：粗石矛 crude_stone_spear / 骨尖矛 bone_tip_spear / 锈铁矛 rusted_iron_spear。

依据 `docs/plans-skeleton/plan-craft-chain-items-v1.md`「族 A：矛族」：三档矛填 `WeaponKind::Spear` 整系空档，
**三矛必须远观可辨**——石矛灰白粗石头、骨矛微弯骨尖 + 暗色缠扎、铁矛锈红宽刃，差异在部件层做出来，不靠贴图微调。
模板 id 用 plan 草案里的 `crude_stone_spear` / `bone_tip_spear` / `rusted_iron_spear`。

**三件共用矛杆 / 绑缚 / 矛尾，只换矛头**（以及绑缚材质：石矛是麻绳，骨矛 / 铁矛是暗色蛛丝绳）：

    part_butt()      矛尾木帽
    part_shaft()     矛杆：三段略有粗细和偏移的木杆 + 几个木疖
    part_grip()      下三分之一的握把缠绳
    part_socket()    杆头劈开的两瓣木槽（夹住矛头）
    part_lashing()   木槽外的缠扎绳圈
    part_head_*()    矛头（每件各一个）

授权系（见 `held_item_common`）：握把末端在 y=0、尖端朝 +Y，单位 = 方块（1.0 = 16px）。全长 1.19，
落在手持物合理区间 0.3~1.2 之内；握把中点 y=0.32（缠绳段中点），拳心对准这里。
双手持长兵，手里显示缩放比单手剑略小一点，免得枪头顶出画面。

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

LENGTH = 1.19           # 全长（授权系，方块）
GRIP = 0.32             # 拳心对准的高度：握把缠绳段中点
SCALE = 0.90            # 手里显示缩放

SHAFT_HALF = 0.034      # 矛杆半宽（≈2px 粗，参考图里杆细、头宽）


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
    """粗石矛头：灰白燧石，崩口和打片留下的明暗大斑，边缘更暗的崩疤。"""
    rng = random.Random(0x5E03)
    img = Image.new("RGBA", (16, 16), (150, 148, 142, 255))
    noise_fill(img, rng, (150, 148, 142), 12, warm=1)
    blotch(img, rng, 6, (186, 184, 176), (1.8, 3.6))       # 打片的亮面
    blotch(img, rng, 5, (112, 110, 106), (1.4, 3.0))       # 崩疤的暗面
    blotch(img, rng, 2, (96, 92, 90), (0.9, 1.6))
    return img


# ── 共用部件 ─────────────────────────────────────────────────────────────


def _box(name: str, material: str, y0: float, y1: float, x: float, z: float, hx: float, hz: float) -> Box:
    return Box(name, material, (round(x, 5), round((y0 + y1) / 2.0, 5), round(z, 5)),
               (round(hx, 5), round((y1 - y0) / 2.0, 5), round(hz, 5)))


def part_butt() -> list[Box]:
    """矛尾木帽：比矛杆略粗的一截钝木，防劈裂也是握把的尾端止滑。"""
    return [_box("butt", "wood", 0.0, 0.030, 0.0, 0.0, SHAFT_HALF * 1.14, SHAFT_HALF * 1.14)]


def part_shaft() -> list[Box]:
    """矛杆：三段略有粗细和偏移的木杆（天然木杆不是车出来的直棍），加几个木疖。"""
    segments = (
        # y0     y1     x 偏     z 偏     半宽系数
        (0.030, 0.300, +0.002, -0.001, 1.00),
        (0.300, 0.600, -0.003, +0.002, 0.94),
        (0.600, 0.860, +0.002, -0.002, 0.88),
    )
    boxes = [_box(f"shaft_{i}", "wood", y0, y1, x, z, SHAFT_HALF * k, SHAFT_HALF * k)
             for i, (y0, y1, x, z, k) in enumerate(segments)]
    # 木疖：杆上鼓出的几小块，错开方向和高度
    boxes.append(_box("knot_a", "wood", 0.470, 0.505, SHAFT_HALF * 0.94 + 0.006, 0.004, 0.012, 0.016))
    boxes.append(_box("knot_b", "wood", 0.690, 0.722, -0.002, SHAFT_HALF * 0.88 + 0.006, 0.014, 0.011))
    boxes.append(_box("knot_c", "wood", 0.120, 0.150, -SHAFT_HALF - 0.005, -0.003, 0.011, 0.014))
    return boxes


def part_grip(cord: str) -> list[Box]:
    """握把缠绳：下三分之一四道，厚薄与偏向参差（等厚等宽读成车出来的凹槽）。"""
    coils = (
        # y0     y1     proud   x 偏    z 偏
        (0.200, 0.246, 1.30, -0.003, +0.002),
        (0.252, 0.296, 1.22, +0.004, -0.003),
        (0.302, 0.348, 1.32, -0.003, +0.003),
        (0.354, 0.398, 1.20, +0.003, -0.002),
    )
    return [_box(f"grip_{i}", cord, y0, y1, x, z, SHAFT_HALF * k, SHAFT_HALF * k)
            for i, (y0, y1, k, x, z) in enumerate(coils)]


def part_socket() -> list[Box]:
    """杆头劈开的两瓣木槽：夹住矛头尾部。左右两瓣各一块，中间留缝给矛头舌。"""
    gap = 0.010
    half = SHAFT_HALF * 0.86
    boxes = []
    for side, sx in (("l", -1.0), ("r", 1.0)):
        boxes.append(_box(f"socket_{side}", "wood", 0.860, 0.950, sx * (gap + half * 0.5), 0.0, half * 0.5, half))
    return boxes


def part_lashing(cord: str) -> list[Box]:
    """缠扎绳圈：三道裹在木槽外，压住矛头舌，错开厚薄。"""
    coils = (
        (0.866, 0.890, 1.55, +0.002, -0.002),
        (0.896, 0.920, 1.45, -0.003, +0.003),
        (0.926, 0.948, 1.55, +0.002, +0.002),
    )
    half = SHAFT_HALF * 0.86
    return [_box(f"lash_{i}", cord, y0, y1, x, z, (0.014 + half) * k, half * k)
            for i, (y0, y1, k, x, z) in enumerate(coils)]


# ── 矛头 ─────────────────────────────────────────────────────────────────


def part_head_stone() -> list[Box]:
    """粗石矛头：长叶形的打制石片——**宽而厚、边缘崩口**，轮廓参差，一眼是石头不是铁。

    自下而上：舌（插进木槽）→ 肩 → 最宽的叶身（s3）→ 一路收到尖。每段左右中心略错、厚度不同，
    再加几小片崩出来的碎片贴在边缘，轮廓才碎。
    """
    segments = (
        # 名       y0     y1     x 偏    半宽    半厚
        ("tang", 0.925, 0.975, 0.000, 0.018, 0.014),
        ("s1", 0.975, 1.000, -0.002, 0.050, 0.024),
        ("s2", 1.000, 1.030, +0.002, 0.072, 0.030),
        ("s3", 1.030, 1.065, -0.003, 0.086, 0.034),
        ("s4", 1.065, 1.095, +0.003, 0.078, 0.032),
        ("s5", 1.095, 1.125, -0.002, 0.062, 0.028),
        ("s6", 1.125, 1.150, +0.002, 0.042, 0.021),
        ("s7", 1.150, 1.172, 0.000, 0.024, 0.014),
        ("s8", 1.172, 1.190, 0.000, 0.010, 0.008),
    )
    boxes = [_box(f"stone_{name}", "stone", y0, y1, x, 0.0, hw, ht) for name, y0, y1, x, hw, ht in segments]
    # 崩口 / 碎片：贴在叶身两侧边缘的小块，高度和凸出量各不相同
    boxes.append(_box("stone_chip_l", "stone", 1.034, 1.056, -0.092, 0.004, 0.014, 0.018))
    boxes.append(_box("stone_chip_r", "stone", 1.050, 1.078, +0.096, -0.003, 0.014, 0.016))
    boxes.append(_box("stone_chip_l2", "stone", 1.088, 1.108, -0.074, -0.004, 0.010, 0.014))
    return boxes


# ── 整件 ─────────────────────────────────────────────────────────────────


def all_boxes_for(key: str) -> tuple[Box, ...]:
    cord = "hemp"
    head = {"crude_stone_spear": part_head_stone}[key]
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
    stone = Material("stone", (0.59, 0.58, 0.56), tex_spear_stone())
    return (_item("crude_stone_spear", "粗石矛", (wood, hemp, stone)),)


# ── 审阅图 ───────────────────────────────────────────────────────────────


def render_check(item: HeldItem, bbmodel: Path) -> Path:
    """model-review/img/handheld/<id>/check.png：左爆炸参考图，右渲染 3/4 + 侧视。"""
    from bbmodel_maker.render.render_bbmodel import render

    out_dir = REVIEW_ROOT / item.key
    out_dir.mkdir(parents=True, exist_ok=True)
    bg = (119, 119, 119)
    iso, _ = render(bbmodel, yaw=-35.0, pitch=25.0, size=640, bg=bg)
    side, _ = render(bbmodel, yaw=90.0, pitch=0.0, size=640, bg=bg)

    ref = Image.open(REVIEW_ROOT / "exploded" / f"{item.key}.png").convert("RGB")
    h = 640
    ref = ref.resize((int(ref.width * h / ref.height), h), Image.Resampling.LANCZOS)
    canvas = Image.new("RGB", (ref.width + 2 * 640 + 48, h + 36), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)
    canvas.paste(ref, (12, 30))
    canvas.paste(iso, (ref.width + 24, 30))
    canvas.paste(side, (ref.width + 640 + 36, 30))
    draw.text((16, 8), f"EXPLODED REFERENCE ({item.key}.png)", fill=(210, 200, 180))
    draw.text((ref.width + 28, 8), "NOW RENDER (3/4 VIEW)", fill=(180, 220, 210))
    draw.text((ref.width + 640 + 40, 8), "SIDE VIEW", fill=(180, 220, 210))
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
