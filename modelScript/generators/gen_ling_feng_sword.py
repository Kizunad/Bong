#!/usr/bin/env python3
"""灵锋剑 ling_feng_sword：碎铁锻成的稀有单手剑，刃口走一线灵光，剑格是异兽骨。bbmodel + SML OBJ 双出。

依据（server/assets/items/forge.toml、server/assets/forge/blueprints/ling_feng_v0.json）：
- 描述「灵锋入骨，真元过刃时可听见细碎风声」→ 剑格用**异兽骨**（blueprint 的可选载体之一 `yi_shou_gu`，「入骨」），
  刃口两线灵光（真元过刃）；
- blueprint 铭文步 `slots: 2` → 剑身两面各嵌**两格铭文槽**（发青白光的小方块）；
- 稀有、tier 2、base_attack 21：比青锋（剑脊一线青光，uncommon）更精致，所以刃宽一些、有剑格翼和骨环剑首。

目前客户端灵锋剑借 `iron_sword.obj`（`BongWeaponModelRegistry.java:209-210`），这件是它的专属模型；**只做模型，不接线**。

参考爆炸图：`model-review/img/handheld/exploded/ling_feng_sword.png`。

部件（授权系：握把末端 y=0、尖端 +Y，单位 = 方块，下面写 `px` 的用 `PX = 1/16` 换算）：

    part_pommel()   骨环剑首：骨白小环（孔穿 z 向），在 y 0..2px
    part_grip()     深色木柄，外面缠浅色绳（螺旋：每道错开前后左右）
    part_guard()    骨质新月护手：中块 + 向两侧上翘的翼（阶梯上扬）
    part_blade()    剑身：冷灰钢，12 行逐行收窄的直刃，尖端收成针
    part_edge()     刃口灵光：两侧边缘各一线青白发光带
    part_slots()    铭文槽：正反两面各两格发光小方块

用法：
    python gen_ling_feng_sword.py            # 出 bbmodel + 三视图预览
    python gen_ling_feng_sword.py --check    # 另出审阅图 model-review/img/handheld/ling_feng_sword/check.png
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
KEY = "ling_feng_sword"
LENGTH = 0.935          # 全长：剑尖
GRIP = 0.215            # 拳心对准的高度：木柄中段
SCALE = 1.0

POMMEL_Y = (0.0, 0.100)
GRIP_Y = (0.100, 0.300)
GUARD_Y = (0.300, 0.345)
BLADE_Y0 = 0.345
# 剑身逐行：(y1, 半宽 px)；12 行，前 9 行近乎等宽（直刃），最后三行收成针
_BLADE_ROWS = (
    (0.400, 0.95), (0.455, 1.00), (0.510, 1.00), (0.565, 1.00), (0.620, 1.00), (0.675, 0.98),
    (0.730, 0.94), (0.785, 0.88), (0.835, 0.78), (0.880, 0.60), (0.912, 0.35), (0.935, 0.15),
)
BLADE_HALF_THICK_PX = 0.35


# ── 贴图（16²，通用材质样本；OBJ 链每个面整张铺满） ───────────────────────────


def tex_steel() -> Image.Image:
    """冷灰钢：淡灰蓝，细密纵向打磨纹，亮暗两档斑。"""
    rng = random.Random(0x6F01)
    img = Image.new("RGBA", (16, 16), (170, 180, 190, 255))
    noise_fill(img, rng, (170, 180, 190), 7, warm=0)
    blotch(img, rng, 4, (196, 206, 214), (1.6, 3.4))
    blotch(img, rng, 3, (140, 150, 162), (1.4, 2.8))
    pixels = img.load()
    for x in (3, 8, 12):
        for y in range(16):
            if (x + y * 3) % 5:
                pixels[x, y] = (156, 166, 178, 255)
    return img


def tex_glow() -> Image.Image:
    """灵光：青白发光，中心近白、边缘偏青。不做发光层，靠贴图自身亮度对比。"""
    rng = random.Random(0x6F02)
    img = Image.new("RGBA", (16, 16), (150, 232, 238, 255))
    noise_fill(img, rng, (150, 232, 238), 8, warm=0)
    blotch(img, rng, 4, (214, 250, 252), (1.6, 3.2))
    blotch(img, rng, 2, (104, 196, 214), (1.2, 2.2))
    return img


def tex_bone() -> Image.Image:
    """异兽骨：象牙白微黄，纵向纤维，几处暗孔。"""
    rng = random.Random(0x6F03)
    img = Image.new("RGBA", (16, 16), (226, 215, 190, 255))
    noise_fill(img, rng, (226, 215, 190), 7, warm=2)
    blotch(img, rng, 4, (243, 235, 212), (1.6, 3.2))
    blotch(img, rng, 3, (200, 187, 158), (1.4, 2.8))
    pixels = img.load()
    for _ in range(3):
        pixels[rng.randint(1, 14), rng.randint(1, 14)] = (156, 141, 112, 255)
    return img


def tex_grip_wood() -> Image.Image:
    """柄木：深褐，纵向木纹。"""
    rng = random.Random(0x6F04)
    img = Image.new("RGBA", (16, 16), (84, 58, 42, 255))
    noise_fill(img, rng, (84, 58, 42), 7, warm=2)
    blotch(img, rng, 3, (64, 43, 31), (1.6, 3.0))
    pixels = img.load()
    for x in (4, 11):
        for y in range(16):
            if (x * 5 + y * 3) % 4:
                pixels[x, y] = (70, 48, 35, 255)
    return img


def tex_cord() -> Image.Image:
    """缠绳：浅米色，斜向拧纹。"""
    rng = random.Random(0x6F05)
    img = Image.new("RGBA", (16, 16), (216, 202, 170, 255))
    noise_fill(img, rng, (216, 202, 170), 7, warm=2)
    pixels = img.load()
    for start in range(-8, 16, 3):
        for step in range(16):
            x, y = start + step, 15 - step
            if 0 <= x < 16 and 0 <= y < 16:
                pixels[x, y] = (182, 168, 138, 255)
    return img


# ── 部件 ─────────────────────────────────────────────────────────────────


def _box(name: str, material: str, y0: float, y1: float, x: float, z: float, hx: float, hz: float) -> Box:
    return Box(name, material, (round(x, 5), round((y0 + y1) / 2.0, 5), round(z, 5)),
               (round(hx, 5), round((y1 - y0) / 2.0, 5), round(hz, 5)))


def part_pommel() -> list[Box]:
    """骨环剑首：外 2.6px 宽 × 2px 高、厚 1px，孔 0.8px×1px（穿 z 向）；四条边。"""
    y0, y1 = POMMEL_Y
    t = 0.5 * PX
    return [
        _box("pommel_l", "bone", y0, y1, -0.85 * PX, 0.0, 0.45 * PX, 0.5 * PX),
        _box("pommel_r", "bone", y0, y1, +0.85 * PX, 0.0, 0.45 * PX, 0.5 * PX),
        _box("pommel_lo", "bone", y0, y0 + t, 0.0, 0.0, 0.4 * PX, 0.5 * PX),
        _box("pommel_hi", "bone", y1 - t, y1, 0.0, 0.0, 0.4 * PX, 0.5 * PX),
    ]


def part_grip() -> list[Box]:
    """深色木柄 + 螺旋缠绳（每道错开前后左右，读成绕上去的绳而不是一摞圆盘）。"""
    y0, y1 = GRIP_Y
    boxes = [_box("grip_core", "grip", y0, y1, 0.0, 0.0, 0.75 * PX, 0.75 * PX)]
    wraps = (
        # y0     y1     x 偏     z 偏     半宽 px
        (0.140, 0.160, +0.20, -0.10, 0.98),
        (0.170, 0.190, -0.20, +0.12, 0.96),
        (0.200, 0.220, +0.18, +0.10, 1.00),
        (0.230, 0.250, -0.18, -0.12, 0.96),
        (0.260, 0.280, +0.20, +0.08, 0.98),
    )
    for i, (a, b, dx, dz, hw) in enumerate(wraps):
        boxes.append(_box(f"wrap_{i}", "cord", a, b, dx * PX, dz * PX, hw * PX, hw * PX))
    return boxes


def part_guard() -> list[Box]:
    """骨质新月护手：中块 + 两侧阶梯上翘的翼（左右对称，每级向外 0.8px、向上 0.35px）。"""
    y0, y1 = GUARD_Y
    boxes = [_box("guard_core", "bone", y0, y1, 0.0, 0.0, 1.0 * PX, 0.85 * PX)]
    for side, sx in (("l", -1.0), ("r", 1.0)):
        for i in range(3):
            x = sx * (1.0 + 0.6 * (i + 0.5)) * PX
            lift = 0.018 * (i + 1) + (0.003 if side == "r" else 0.0)
            boxes.append(_box(f"guard_{side}{i}", "bone", y0 + lift, y1 + lift + 0.003 * i, x, 0.0,
                              0.3 * PX, (0.7 - 0.1 * i) * PX))
    return boxes


def part_blade() -> list[Box]:
    """剑身：冷灰钢，12 行逐行收窄的直刃（前 9 行近乎等宽），尖端收成针；中线一条更亮的脊。"""
    boxes = []
    y0 = BLADE_Y0
    for i, (y1, hw) in enumerate(_BLADE_ROWS):
        boxes.append(_box(f"blade_{i:02d}", "steel", y0, y1, 0.0, 0.0, hw * PX, BLADE_HALF_THICK_PX * PX))
        y0 = y1
    return boxes


def part_edge() -> list[Box]:
    """刃口灵光：两侧边缘各一线青白发光带（比剑身略厚 0.05px、宽 0.3px），只走前 9 行直刃。"""
    boxes = []
    y0 = BLADE_Y0
    for i, (y1, hw) in enumerate(_BLADE_ROWS[:9]):
        for side, sx in (("l", -1.0), ("r", 1.0)):
            boxes.append(_box(f"edge_{side}{i:02d}", "glow", y0 + 0.002, y1 - 0.002, sx * (hw - 0.05) * PX, 0.0,
                              0.09 * PX, (BLADE_HALF_THICK_PX + 0.06) * PX))
        y0 = y1
    return boxes


def part_slots() -> list[Box]:
    """铭文槽：blueprint `inscription.slots = 2`——正反两面各嵌两格发光小方块（0.6px × 1.2px）。"""
    boxes = []
    for i, (a, b) in enumerate(((0.470, 0.545), (0.650, 0.725))):
        for face, sz in (("f", +1.0), ("b", -1.0)):
            boxes.append(_box(f"slot_{face}{i}", "glow", a, b, 0.0, sz * (BLADE_HALF_THICK_PX + 0.03) * PX, 0.22 * PX, 0.10 * PX))
    return boxes


def all_boxes() -> tuple[Box, ...]:
    return tuple(part_pommel() + part_grip() + part_guard() + part_blade() + part_edge() + part_slots())


def item() -> HeldItem:
    return HeldItem(
        key=KEY,
        display_name="灵锋剑",
        host_item="iron_sword",     # 现在借铁剑 OBJ 的宿主；专属宿主接线时再定（要查 vanillaModelPaths 防撞）
        boxes=all_boxes(),
        materials=(
            Material("steel", (0.66, 0.70, 0.74), tex_steel()),
            Material("glow", (0.60, 0.91, 0.93), tex_glow()),
            Material("bone", (0.89, 0.84, 0.74), tex_bone()),
            Material("grip", (0.33, 0.23, 0.16), tex_grip_wood()),
            Material("cord", (0.85, 0.79, 0.67), tex_cord()),
        ),
        grip=GRIP,
        display=hand_display(SCALE, GRIP, LENGTH, gui_spin=45.0),
    )


# ── 审阅图 ───────────────────────────────────────────────────────────────


def render_check(bbmodel: Path) -> Path:
    """model-review/img/handheld/ling_feng_sword/check.png：左爆炸参考图，右渲染 3/4 + 侧视 + 正视。"""
    from bbmodel_maker.render.render_bbmodel import render

    out_dir = REVIEW_ROOT / KEY
    out_dir.mkdir(parents=True, exist_ok=True)
    bg = (119, 119, 119)
    iso, _ = render(bbmodel, yaw=-35.0, pitch=25.0, size=640, bg=bg)
    side, _ = render(bbmodel, yaw=90.0, pitch=0.0, size=640, bg=bg)
    front, _ = render(bbmodel, yaw=0.0, pitch=0.0, size=640, bg=bg)

    ref = Image.open(REVIEW_ROOT / "exploded" / f"{KEY}.png").convert("RGB")
    h = 640
    ref = ref.resize((int(ref.width * h / ref.height), h), Image.Resampling.LANCZOS)
    canvas = Image.new("RGB", (ref.width + 3 * 640 + 60, h + 36), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)
    canvas.paste(ref, (12, 30))
    draw.text((16, 8), f"EXPLODED REFERENCE ({KEY}.png)", fill=(210, 200, 180))
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

    sword = item()
    if args.dump_display:
        print(build_model_json(sword), end="")
        return
    if args.dump_obj:
        print(build_obj(sword))
        print(build_mtl(sword))
        return

    outputs = write_assets((sword,), bbmodel_dir=BBMODEL_DIR, client_resources=None,
                           preview_dir=PREVIEW_DIR, render_previews=not args.no_preview)
    for key, path in outputs.items():
        print(f"[{key}] {path.relative_to(REPO)}")
    if args.check:
        print(f"[check:{KEY}] {render_check(outputs[f'bbmodel:{KEY}'])}")


if __name__ == "__main__":
    main()
