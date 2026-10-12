#!/usr/bin/env python3
"""灵锋剑 ling_feng_sword：碎铁锻成的稀有单手剑，窄长的蓝色灵锋剑身，剑格是异兽骨月牙。bbmodel + SML OBJ 双出。

依据（server/assets/items/forge.toml、server/assets/forge/blueprints/ling_feng_v0.json）：
- 描述「灵锋入骨，真元过刃时可听见细碎风声」→ 剑格用**异兽骨**（blueprint 的可选载体之一 `yi_shou_gu`，「入骨」），
  剑身通体偏青蓝、刃口两线灵光（真元过刃）；
- blueprint 铭文步 `slots: 2` → 剑身两面各嵌**两格铭文槽**（发青白光的小方块）；
- 稀有、tier 2、base_attack 21。

目前客户端灵锋剑借 `iron_sword.obj`（`BongWeaponModelRegistry.java:209-210`），这件是它的专属模型；**只做模型，不接线**。

## 第 2 次修改（用户原话：「灵锋剑太短了，重做」）

先量 main 上同类长剑的实际尺寸（OBJ 顶点，单位 = 方块）：

    qing_feng_sword_v2   总长 1.680，剑身 ≈1.13（占 67%），剑身宽 0.169、厚 0.065（宽:长 ≈ 1:6.7）
    iron_sword_v2        总长 1.663，剑身 ≈1.16（占 70%），剑身宽 0.169（宽:长 ≈ 1:6.9）
    （第三人称缩放都是 0.8，手里长 ≈1.33~1.34）

上一版灵锋剑总长只有 0.935（青锋的 56%），剑身 0.59（63%），所以「太短」。这一版：

    总长 1.76（青锋 1.68 的 105%），第三人称缩放 0.8 → 手里长 1.41
    剑身 1.29，占全长 73%（≥ 2/3）
    剑身宽 2.58px = 0.161，**宽:长 = 1:8**；厚 0.7px

`held_item_common.assert_conventions` 把全长限制在 0.3~1.2（那是单手小件的区间），1.76 的长剑会被它拦下；
所以这里**不走 `write_assets`**，改成逐项调用它的其余门禁（共面 + 连通）再自己写 bbmodel，
并在 `check_dimensions()` 里加了自己的长度 / 占比 / 宽长比门禁（先注入缺陷自证）。

部件（授权系：握把末端 y=0、尖端 +Y，单位 = 方块，下面写 `px` 的用 `PX = 1/16` 换算）：

    part_pommel()   骨环剑首
    part_grip()     深色木柄，外面缠浅色绳（螺旋：每道错开前后左右）
    part_guard()    骨质新月护手：中块 + 向两侧上翘的翼（阶梯上扬）
    part_blade()    剑身：蓝色灵钢，逐行收窄的直刃，尖端收成针；中线一条亮脊
    part_edge()     刃口灵光：两侧边缘各一线青白发光带
    part_slots()    铭文槽：正反两面各两格发光小方块

用法：
    python gen_ling_feng_sword.py            # 出 bbmodel + 三视图预览
    python gen_ling_feng_sword.py --check    # 另出审阅图 model-review/img/handheld/ling_feng_sword/check.png
                                             # （左爆炸图 | 3/4 | 侧视 | 与青锋剑并排的同比例对比图）
"""

from __future__ import annotations

import argparse
import json
import random
import sys
from pathlib import Path

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))

from bbmodel_maker import workspace  # noqa: E402
from bbmodel_maker.render.held_item_common import (  # noqa: E402
    Box,
    HeldItem,
    Material,
    assert_boxes_are_connected,
    assert_no_coplanar_faces,
    blotch,
    build_bbmodel,
    build_model_json,
    build_mtl,
    build_obj,
    hand_display,
    noise_fill,
)

REPO = Path(__file__).resolve().parents[2]
BBMODEL_DIR = Path(__file__).resolve().parents[1] / "models"
PREVIEW_DIR = Path(__file__).resolve().parents[1] / "out"
REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld")

PX = 1.0 / 16.0
KEY = "ling_feng_sword"
SCALE = 0.8             # 第三人称缩放：和 qing_feng_sword_v2 / iron_sword_v2 一致

POMMEL_Y = (0.0, 0.075)
GRIP_Y = (0.075, 0.425)
GUARD_Y = (0.425, 0.475)
BLADE_Y0 = 0.475
BLADE_Y1 = 1.760        # 剑尖
GRIP = 0.250            # 拳心对准的高度：木柄中段
BLADE_HALF_WIDTH_PX = 1.29           # 宽 2.58px = 0.161，约 1 : 8
BLADE_HALF_THICK_PX = 0.35

# 参照量（main 上的 OBJ 实测，见模块 docstring）
QING_LENGTH = 1.680
MIN_BLADE_SHARE = 2.0 / 3.0
BLADE_WIDTH_TO_LENGTH = (1 / 9.0, 1 / 7.0)      # 允许区间，目标约 1:8


# ── 贴图（16²，通用材质样本；OBJ 链每个面整张铺满） ───────────────────────────


def tex_steel() -> Image.Image:
    """蓝色灵钢：冷青蓝，细密纵向打磨纹，亮暗两档斑。"""
    rng = random.Random(0x6F01)
    img = Image.new("RGBA", (16, 16), (128, 168, 206, 255))
    noise_fill(img, rng, (128, 168, 206), 7, warm=0)
    blotch(img, rng, 4, (162, 200, 232), (1.6, 3.4))
    blotch(img, rng, 3, (98, 134, 176), (1.4, 2.8))
    pixels = img.load()
    for x in (3, 8, 12):
        for y in range(16):
            if (x + y * 3) % 5:
                pixels[x, y] = (112, 150, 190, 255)
    return img


def tex_steel_hi() -> Image.Image:
    """剑脊：比剑身更亮的淡青白。"""
    rng = random.Random(0x6F06)
    img = Image.new("RGBA", (16, 16), (190, 222, 244, 255))
    noise_fill(img, rng, (190, 222, 244), 6, warm=0)
    blotch(img, rng, 3, (222, 242, 252), (1.4, 2.8))
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
    """骨环剑首：外 2.6px 宽 × 1.2px 高、厚 1px，四条边围出孔（穿 z 向）。"""
    y0, y1 = POMMEL_Y
    t = 0.45 * PX
    return [
        _box("pommel_l", "bone", y0, y1, -0.85 * PX, 0.0, 0.45 * PX, 0.5 * PX),
        _box("pommel_r", "bone", y0, y1, +0.85 * PX, 0.0, 0.45 * PX, 0.5 * PX),
        _box("pommel_lo", "bone", y0, y0 + t, 0.0, 0.0, 0.4 * PX, 0.5 * PX),
        _box("pommel_hi", "bone", y1 - t, y1, 0.0, 0.0, 0.4 * PX, 0.5 * PX),
    ]


def part_grip() -> list[Box]:
    """深色木柄（5.6px）+ 螺旋缠绳 8 道（每道错开前后左右，读成绕上去的绳而不是一摞圆盘）。"""
    y0, y1 = GRIP_Y
    boxes = [_box("grip_core", "grip", y0, y1, 0.0, 0.0, 0.75 * PX, 0.75 * PX)]
    n = 8
    step = (y1 - y0 - 0.03) / n
    offsets = ((+0.20, -0.10), (-0.20, +0.12), (+0.18, +0.10), (-0.18, -0.12))
    for i in range(n):
        a = y0 + 0.015 + i * step
        dx, dz = offsets[i % 4]
        hw = 0.98 + 0.03 * (i % 3)
        boxes.append(_box(f"wrap_{i}", "cord", a + 0.003, a + step * 0.62, dx * PX, dz * PX, hw * PX, hw * PX))
    return boxes


def part_guard() -> list[Box]:
    """骨质新月护手：中块 + 两侧阶梯上翘的翼（左右对称，每级向外 0.8px、向上 0.5px，四级）。"""
    y0, y1 = GUARD_Y
    boxes = [_box("guard_core", "bone", y0, y1, 0.0, 0.0, 1.1 * PX, 0.9 * PX)]
    for side, sx in (("l", -1.0), ("r", 1.0)):
        for i in range(4):
            x = sx * (1.1 + 0.8 * (i + 0.5)) * PX
            lift = 0.018 * (i + 1) + (0.003 if side == "r" else 0.0)
            boxes.append(_box(f"guard_{side}{i}", "bone", y0 + lift, y1 + lift + 0.003 * i, x, 0.0,
                              0.4 * PX, (0.75 - 0.1 * i) * PX))
    return boxes


def _blade_rows() -> list[tuple[float, float]]:
    """剑身逐行 (y1, 半宽 px)：前 70% 等宽（直刃），之后逐步收窄，最后收成针。共 16 行。"""
    length = BLADE_Y1 - BLADE_Y0
    cuts = [0.10, 0.20, 0.30, 0.40, 0.50, 0.60, 0.70, 0.78, 0.84, 0.89, 0.93, 0.96, 0.98, 0.99, 0.995, 1.0]
    widths = [1.00, 1.00, 1.00, 1.00, 1.00, 1.00, 0.99, 0.95, 0.88, 0.78, 0.64, 0.48, 0.34, 0.24, 0.16, 0.10]
    return [(BLADE_Y0 + c * length, w * BLADE_HALF_WIDTH_PX) for c, w in zip(cuts, widths)]


def part_blade() -> list[Box]:
    """剑身：蓝色灵钢，逐行收窄的直刃，尖端收成针；中线一条更亮的脊（下 85%）。"""
    boxes = []
    y0 = BLADE_Y0
    rows = _blade_rows()
    for i, (y1, hw) in enumerate(rows):
        boxes.append(_box(f"blade_{i:02d}", "steel", y0, y1, 0.0, 0.0, hw * PX, BLADE_HALF_THICK_PX * PX))
        y0 = y1
    ridge_top = BLADE_Y0 + 0.85 * (BLADE_Y1 - BLADE_Y0)
    y0 = BLADE_Y0
    for i, (y1, _) in enumerate(rows):
        if y1 > ridge_top:
            break
        boxes.append(_box(f"ridge_{i:02d}", "steel_hi", y0 + 0.002, y1 - 0.002, 0.0, 0.0, 0.16 * PX,
                          (BLADE_HALF_THICK_PX + 0.04) * PX))
        y0 = y1
    return boxes


def part_edge() -> list[Box]:
    """刃口灵光：两侧边缘各一线青白发光带（宽 0.18px，比剑身略厚 0.06px），走直刃那部分。"""
    boxes = []
    y0 = BLADE_Y0
    for i, (y1, hw) in enumerate(_blade_rows()[:8]):
        for side, sx in (("l", -1.0), ("r", 1.0)):
            boxes.append(_box(f"edge_{side}{i:02d}", "glow", y0 + 0.002, y1 - 0.002, sx * (hw - 0.05) * PX, 0.0,
                              0.09 * PX, (BLADE_HALF_THICK_PX + 0.06) * PX))
        y0 = y1
    return boxes


def part_slots() -> list[Box]:
    """铭文槽：blueprint `inscription.slots = 2`——正反两面各嵌两格发光小方块（0.5px × 1.6px），沿剑身长 0.3 / 0.62 处。"""
    length = BLADE_Y1 - BLADE_Y0
    boxes = []
    for i, centre in enumerate((0.30, 0.62)):
        a = BLADE_Y0 + (centre - 0.045) * length
        b = BLADE_Y0 + (centre + 0.045) * length
        for face, sz in (("f", +1.0), ("b", -1.0)):
            boxes.append(_box(f"slot_{face}{i}", "glow", a, b, 0.0, sz * (BLADE_HALF_THICK_PX + 0.03) * PX, 0.25 * PX, 0.10 * PX))
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
            Material("steel", (0.50, 0.66, 0.81), tex_steel()),
            Material("steel_hi", (0.75, 0.87, 0.96), tex_steel_hi()),
            Material("glow", (0.60, 0.91, 0.93), tex_glow()),
            Material("bone", (0.89, 0.84, 0.74), tex_bone()),
            Material("grip", (0.33, 0.23, 0.16), tex_grip_wood()),
            Material("cord", (0.85, 0.79, 0.67), tex_cord()),
        ),
        grip=GRIP,
        display=hand_display(SCALE, GRIP, BLADE_Y1, gui_spin=45.0),
    )


# ── 尺寸门禁（先注入缺陷再信它） ──────────────────────────────────────────


def check_dimensions(boxes: tuple[Box, ...]) -> dict:
    """全长不短于青锋剑；剑身占全长 ≥2/3；剑身宽:长在 1:9 ~ 1:7 之间。返回实测数。"""
    y_max = max(b.high[1] for b in boxes)
    blade = [b for b in boxes if b.name.startswith("blade_")]
    blade_len = max(b.high[1] for b in blade) - min(b.low[1] for b in blade)
    blade_width = 2.0 * max(b.half[0] for b in blade)
    assert y_max >= QING_LENGTH, f"总长 {y_max:.3f} 短于青锋剑 {QING_LENGTH}"
    assert blade_len / y_max >= MIN_BLADE_SHARE, f"剑身只占全长 {blade_len / y_max:.0%}，要 ≥ 2/3"
    ratio = blade_width / blade_len
    assert BLADE_WIDTH_TO_LENGTH[0] <= ratio <= BLADE_WIDTH_TO_LENGTH[1], f"剑身宽:长 = 1:{1 / ratio:.1f}，要在 1:7 ~ 1:9"
    return {"length": y_max, "blade_len": blade_len, "blade_share": blade_len / y_max, "width": blade_width, "ratio": 1 / ratio}


def self_test() -> None:
    boxes = all_boxes()
    m = check_dimensions(boxes)
    print(f"  [OK] 总长 {m['length']:.3f}（青锋 {QING_LENGTH}）、剑身 {m['blade_len']:.3f} 占 {m['blade_share']:.0%}、"
          f"剑身宽 {m['width']:.3f}（宽:长 1:{m['ratio']:.1f}）")

    def must_fail(name: str, bad: tuple[Box, ...]) -> None:
        try:
            check_dimensions(bad)
        except AssertionError as e:
            print(f"  [OK] 注入「{name}」被拦截: {e}")
            return
        raise RuntimeError(f"门禁失效：注入「{name}」没被拦住")

    def scale_y(b: Box, k: float) -> Box:
        return Box(b.name, b.material, (b.center[0], b.center[1] * k, b.center[2]), (b.half[0], b.half[1] * k, b.half[2]))

    must_fail("总长缩短（回到 0.935 那一版）", tuple(scale_y(b, 0.935 / BLADE_Y1) for b in boxes))
    must_fail("剑身变短变胖", tuple(
        Box(b.name, b.material, b.center, (b.half[0] * 2.2, b.half[1], b.half[2])) if b.name.startswith("blade_") else b
        for b in boxes))
    # 在尖端之上再接一截杆（总长 2.2、剑身只占 58%）
    must_fail("剑身占比 <2/3", boxes + (Box("long_pad", "grip", (0.0, 2.0, 0.0), (0.02, 0.2, 0.02)),))

# ── 审阅图 ───────────────────────────────────────────────────────────────


def _silhouette_height(img: Image.Image, bg=(119, 119, 119)) -> tuple[int, int]:
    """渲染图里非背景像素的 (最上行, 最下行)。"""
    px = img.convert("RGB").load()
    rows = [y for y in range(img.height) if any(sum(abs(px[x, y][c] - bg[c]) for c in range(3)) > 24 for x in range(0, img.width, 2))]
    return rows[0], rows[-1]


def _same_scale_compare(ling_model: Path, qing_model: Path, qing_length: float, ling_length: float) -> Image.Image:
    """青锋剑和灵锋剑并排的**同比例**正视对比（看得到剑身宽）：各自渲染后，按「模型实长 / 剪影像素高」把两张缩到同一像素/方块，底边对齐。"""
    from bbmodel_maker.render.render_bbmodel import render

    bg = (119, 119, 119)
    shots = []
    for path, length in ((qing_model, qing_length), (ling_model, ling_length)):
        im, _ = render(path, yaw=0.0, pitch=0.0, size=900, bg=bg)
        top, bottom = _silhouette_height(im, bg)
        shots.append((im.convert("RGB"), top, bottom, (bottom - top) / length))
    target = min(s[3] for s in shots)       # 像素 / 方块；取较小的，两张都只缩小不放大
    tiles = []
    for im, top, bottom, ppb in shots:
        crop = im.crop((0, max(0, top - 6), im.width, min(im.height, bottom + 6)))
        k = target / ppb
        tiles.append(crop.resize((max(1, int(crop.width * k)), max(1, int(crop.height * k))), Image.Resampling.LANCZOS))
    h = max(t.height for t in tiles) + 40
    canvas = Image.new("RGB", (sum(t.width for t in tiles) + 40, h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)
    x = 10
    for t, label in zip(tiles, (f"QING FENG V2  (L={qing_length:.2f})", f"LING FENG  (L={ling_length:.2f})")):
        canvas.paste(t, (x, h - t.height - 6))
        draw.text((x + 4, 6), label, fill=(210, 200, 180))
        x += t.width + 20
    return canvas


def render_check(bbmodel: Path) -> Path:
    """model-review/img/handheld/ling_feng_sword/check.png：左爆炸图 | 3/4 | 侧视 | 与青锋剑并排的同比例对比。"""
    from bbmodel_maker.render.render_bbmodel import render

    out_dir = REVIEW_ROOT / KEY
    out_dir.mkdir(parents=True, exist_ok=True)
    bg = (119, 119, 119)
    h = 760
    iso, _ = render(bbmodel, yaw=-35.0, pitch=25.0, size=h, bg=bg)
    front, _ = render(bbmodel, yaw=0.0, pitch=0.0, size=h, bg=bg)
    compare = _same_scale_compare(bbmodel, BBMODEL_DIR / "QingFengSwordV2.bbmodel", QING_LENGTH, BLADE_Y1)
    compare = compare.resize((int(compare.width * h / compare.height), h), Image.Resampling.LANCZOS)

    ref = Image.open(REVIEW_ROOT / "exploded" / f"{KEY}.png").convert("RGB")
    ref = ref.resize((int(ref.width * h / ref.height), h), Image.Resampling.LANCZOS)
    parts = [("EXPLODED REFERENCE", ref), ("3/4 VIEW", iso), ("FRONT VIEW", front), ("SAME-SCALE vs QING FENG", compare)]
    canvas = Image.new("RGB", (sum(p.width for _, p in parts) + 12 * (len(parts) + 1), h + 40), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)
    x = 12
    for title, im in parts:
        canvas.paste(im, (x, 32))
        draw.text((x + 4, 10), title, fill=(180, 220, 210))
        x += im.width + 12
    path = out_dir / "check.png"
    canvas.save(path)
    return path


def write_bbmodel(sword: HeldItem) -> Path:
    """逐项调用 held_item_common 的共面 + 连通门禁，再写 bbmodel（不走 `write_assets`，原因见模块 docstring）。"""
    assert_no_coplanar_faces(sword)
    assert_boxes_are_connected(sword)
    BBMODEL_DIR.mkdir(parents=True, exist_ok=True)
    path = BBMODEL_DIR / "LingFengSword.bbmodel"
    path.write_text(json.dumps(build_bbmodel(sword, workspace.default().namespace), ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    return path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
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

    print("gen_ling_feng_sword 自检…")
    self_test()
    path = write_bbmodel(sword)
    print(f"[bbmodel] {path.relative_to(REPO)}")
    if args.check:
        print(f"[check] {render_check(path)}")


if __name__ == "__main__":
    main()
