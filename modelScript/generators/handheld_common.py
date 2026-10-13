#!/usr/bin/env python3
"""器物补模型（守家傀儡 / 阵符 / 灵龛诡雷）共用的小工具：立方体写法、共面门禁、带贴图导出、check.png 拼图。

贴图沿用经脉工厂的统一贴图入口 `meridian_factory/pixel_texture.write_textured_bbmodel`（每面独立 UV，
每个 texel 按 3D 位置上色）。平色版写 <id>.bbmodel，贴图版写 <id>_tex.bbmodel，画笔库见 handheld_texture.py。
"""

from __future__ import annotations

import sys
from pathlib import Path
from typing import List, Optional, Sequence, Tuple

from PIL import Image, ImageDraw

GENERATORS = Path(__file__).resolve().parent
sys.path.insert(0, str(GENERATORS / "meridian_factory"))
sys.path.insert(0, str(GENERATORS.parent))

from bbmodel_maker.render.render_bbmodel import render  # noqa: E402
from common_parts import _assert_no_coplanar_faces  # noqa: E402
from handheld_texture import flat_override, handheld_override  # noqa: E402
from pixel_texture import (  # noqa: E402
    Override, write_textured_bbmodel,
)

REPO = GENERATORS.parents[1]
MODEL_DIR = GENERATORS.parent / "models"
REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/handheld")


def review_available() -> bool:
    """审阅图目录所在的 model-review 是否存在。CI 与 golden 沙箱里没有它，出图要跳过而不是在无权限路径上 mkdir 崩掉。"""
    return REVIEW_ROOT.parents[1].is_dir()


BG = (119, 119, 119)
LABEL_BG = (24, 25, 28)
CANVAS_BG = (28, 29, 33)
TILE = 480

Cube = dict
View = tuple      # (标签, yaw, pitch[, focus])，标签写出实际照到的面；focus = (中心 xyz px, 取景跨度 px)

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


def write_model(cubes: List[Cube], model_id: str, override: Optional[Override] = None, check: bool = True) -> Path:
    """平色版：<id>.bbmodel，每个材质一个纯色（handheld_texture.flat_override）。"""
    if check:
        check_geometry(cubes)
    out = MODEL_DIR / f"{model_id}.bbmodel"
    write_textured_bbmodel(cubes, out, model_id, None, override or flat_override)
    return out


def write_model_tex(cubes: List[Cube], model_id: str, override: Optional[Override] = None) -> Path:
    """贴图版：<id>_tex.bbmodel，每个材质一套画笔（handheld_texture.handheld_override）。"""
    out = MODEL_DIR / f"{model_id}_tex.bbmodel"
    write_textured_bbmodel(cubes, out, model_id, None, override or handheld_override)
    return out


def write_check_tex(flat_path: Path, tex_path: Path, model_id: str, views: "Sequence[View] | View",
                    ref_name: Optional[str] = None) -> Path:
    """check_tex.png：左参考 / 中平色 / 右贴图；给多个视角时，每个视角各加一对（平色、贴图）。
    视角可以带第 4 项 focus=(中心 xyz px, 取景跨度 px)，用来拉近看剑头 / 矛头。"""
    out_dir = REVIEW_ROOT / model_id
    out_dir.mkdir(parents=True, exist_ok=True)
    if views and isinstance(views[0], str):
        views = [views]                          # 传进来的是单个视角
    tiles = []
    ref_path = REVIEW_ROOT / "exploded" / (ref_name or f"{model_id}.png")
    if ref_path.exists():
        ref = Image.open(ref_path).convert("RGB")
        ref = ref.resize((int(ref.width * TILE / ref.height), TILE), Image.Resampling.LANCZOS)
        tiles.append(_labeled(ref, f"REFERENCE ({ref_path.name})"))
    for view in views:
        label, yaw, pitch = view[:3]
        focus = view[3] if len(view) > 3 else None
        for title, path in (("FLAT", flat_path), ("TEXTURED", tex_path)):
            image, _ = render(path, yaw=yaw, pitch=pitch, size=TILE, bg=BG, shading="mc", focus=focus)
            tiles.append(_labeled(image, f"{title}: {label}"))
    canvas = Image.new("RGB", (sum(t.width for t in tiles) + 8 * (len(tiles) - 1), tiles[0].height), CANVAS_BG)
    x = 0
    for tile in tiles:
        canvas.paste(tile, (x, 0))
        x += tile.width + 8
    target = out_dir / "check_tex.png"
    canvas.save(target)
    return target


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
