#!/usr/bin/env python3
"""器物补模型（守家傀儡 / 阵符 / 灵龛诡雷）共用的小工具：立方体写法、共面门禁、带贴图导出、check.png 拼图。

贴图沿用经脉工厂的统一贴图入口 `meridian_factory/pixel_texture.write_textured_bbmodel`（每面独立 UV，
每个 texel 按 3D 位置上色），这里只补一份器物用的调色板（石、暗石、符纸、朱砂……）。
"""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

from PIL import Image, ImageDraw

GENERATORS = Path(__file__).resolve().parent
sys.path.insert(0, str(GENERATORS / "meridian_factory"))
sys.path.insert(0, str(GENERATORS.parent))

from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from common_parts import _assert_no_coplanar_faces  # noqa: E402
from pixel_texture import (  # noqa: E402
    Color, Override, Painter, Vec, hex_color, mix, shade, texel_noise, value_noise, write_textured_bbmodel,
)

REPO = GENERATORS.parents[1]
MODEL_DIR = GENERATORS.parent / "models"
REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld")

BG = (119, 119, 119)
LABEL_BG = (24, 25, 28)
CANVAS_BG = (28, 29, 33)
TILE = 480

Cube = dict
View = Tuple[str, float, float]      # (标签, yaw, pitch)，标签写出实际照到的面

# 器物调色板：骨 / 石走各自画笔，这里只给没有专用画笔的材质一个底色
PALETTE: Dict[str, Tuple[int, int, int]] = {
    "base_stone": (104, 102, 96),
    "stone_dark": (58, 55, 50),
    "rust": (122, 74, 48),
    "talisman_paper": (214, 196, 158),
    "talisman_dark": (92, 70, 48),
    "cinnabar": (150, 38, 30),
    "cinnabar_dark": (92, 22, 20),
    "ash": (74, 72, 70),
    "void": (24, 20, 28),
    "rope": (122, 80, 52),
    "paper_ochre": (176, 150, 100),
    "ink_ochre": (168, 116, 28),
    "paper_frost": (128, 142, 152),
    "ink_blue": (50, 84, 112),
    "paper_scorch": (58, 50, 48),
    "ink_cinnabar": (176, 42, 32),
}


# 斑驳材质：两个底色按噪声混拼，再随机压暗几块。石与木混拼出粗糙的傀儡感，不出现整块纯色
MOTTLED_PAIRS: Dict[str, Tuple[Color, Color]] = {
    "stone": (hex_color("#8a8478"), hex_color("#a39d90")),
    "wood": (hex_color("#6b4a2f"), hex_color("#7d5a3a")),
}
MOTTLE_SCALE = 2.2          # 两个底色之间的混拼块大小（px）
PATCH_SCALE = 1.1           # 压暗斑块的大小（px）
PATCH_THRESHOLD = 0.72      # 斑块噪声超过它就压暗
PATCH_SHADE = -0.16
RENDER_SHADE = -0.14        # 渲染器 mc 光照会把贴图提亮一档，整体先压暗一点，实际看到的才接近指定色


def _mottled_painter(pair: Tuple[Color, Color], seed: int) -> Painter:
    def paint(p: Vec, axis: int) -> Color:
        blend = 0.75 * value_noise(p, MOTTLE_SCALE, seed) + 0.25 * texel_noise(p, seed + 1)
        color = shade(mix(pair[0], pair[1], blend * 1.3 - 0.15), RENDER_SHADE)
        if value_noise(p, PATCH_SCALE, seed + 2) > PATCH_THRESHOLD:
            color = shade(color, PATCH_SHADE)
        return color

    return paint


def mottled_override(cube: dict, default: Painter) -> Optional[Painter]:
    pair = MOTTLED_PAIRS.get(cube.get("material", ""))
    if pair is None:
        return None
    return _mottled_painter(pair, 301 if cube["material"] == "stone" else 311)


def cube(name: str, lo: Sequence[float], hi: Sequence[float], material: str, group: str) -> Cube:
    return {"name": name, "from": [float(v) for v in lo], "to": [float(v) for v in hi],
            "material": material, "group": group}


def mirror_x(cubes: List[Cube], suffix_from: str = "_l", suffix_to: str = "_r") -> List[Cube]:
    """把一组立方体沿 x 镜像（名字里的 _l 换成 _r），用来由左半边出右半边。"""
    out = []
    for c in cubes:
        lo, hi = c["from"], c["to"]
        mirrored = dict(c)
        mirrored["from"] = [-hi[0], lo[1], lo[2]]
        mirrored["to"] = [-lo[0], hi[1], hi[2]]
        mirrored["name"] = c["name"].replace(suffix_from, suffix_to)
        out.append(mirrored)
    return out


def check_geometry(cubes: List[Cube]) -> None:
    """共面门禁 + 材质必须有调色板或内置画笔。"""
    _assert_no_coplanar_faces(cubes)


def write_model(cubes: List[Cube], model_id: str, override: Optional[Override] = None) -> Path:
    check_geometry(cubes)
    out = MODEL_DIR / f"{model_id}.bbmodel"
    write_textured_bbmodel(cubes, out, model_id, PALETTE, override or mottled_override)
    return out


def _labeled(img: Image.Image, text: str) -> Image.Image:
    out = Image.new("RGB", (img.width, img.height + 26), LABEL_BG)
    out.paste(img, (0, 26))
    ImageDraw.Draw(out).text((8, 8), text, fill=(230, 230, 230))
    return out


def write_check(model_path: Path, model_id: str, views: Sequence[View], ref_name: Optional[str] = None) -> Path:
    """check.png：左边是爆炸参考图，右边是本次模型的各个视角（标签写出实际照到的面）。"""
    out_dir = REVIEW_ROOT / model_id
    out_dir.mkdir(parents=True, exist_ok=True)
    tiles = []
    ref_path = REVIEW_ROOT / "exploded" / (ref_name or f"{model_id}.png")
    if ref_path.exists():
        ref = Image.open(ref_path).convert("RGB")
        ref = ref.resize((int(ref.width * TILE / ref.height), TILE), Image.Resampling.LANCZOS)
        tiles.append(_labeled(ref, f"REFERENCE ({ref_path.name})"))
    for label, yaw, pitch in views:
        image, _ = render(model_path, yaw=yaw, pitch=pitch, size=TILE, bg=BG, shading="mc")
        tiles.append(_labeled(image, label))
    canvas = Image.new("RGB", (sum(t.width for t in tiles) + 8 * (len(tiles) - 1), tiles[0].height), CANVAS_BG)
    x = 0
    for tile in tiles:
        canvas.paste(tile, (x, 0))
        x += tile.width + 8
    target = out_dir / "check.png"
    canvas.save(target)
    return target
