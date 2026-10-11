#!/usr/bin/env python3
"""经脉工厂：给已过审的几何统一重贴图（只改贴图和 UV，几何不动）。

所有模型都走同一个入口 pixel_texture.write_textured_bbmodel；这里只负责
「编号 → 立方体 / 调色板 / 参考图 / 输出目录」的登记，以及出审阅图：

    model-review/img/meridian_factory/<目录>/render_tex.png   多视角（或多变体）渲染
    model-review/img/meridian_factory/<目录>/check_tex.png    参考图 | 现有平色版 | 贴图版（同一 3/4 取景）

旧的 render.png / check.png 不覆盖。贴图版 bbmodel 写到 modelScript/models/meridian_factory/<名>_tex.bbmodel（被 gitignore）。

用法（在本目录下，不要设 PYTHONPATH=modelScript，否则本地存根会盖住真正的 bbmodel_maker）：
    python retexture.py b01 b02
    python retexture.py --list
"""

from __future__ import annotations

import argparse
import shutil
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Dict, List, Optional

from PIL import Image, ImageDraw

from bbmodel_maker.render.render_bbmodel import render
from piece_overrides import (
    arm_override,
    buffer_override,
    coupling_override,
    floor_override,
    lung_override,
    port_override,
    stomach_override,
    tank_override,
)
from pixel_texture import Override, palette_to_colors, write_textured_bbmodel

REPO = Path(__file__).resolve().parents[3]
MODEL_DIR = REPO / "modelScript" / "models" / "meridian_factory"
REVIEW_ROOT = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory")

BG = (119, 119, 119)
TILE = 480
ISO_YAW, ISO_PITCH = 35.0, 25.0
LABEL_BG = (24, 25, 28)
CANVAS_BG = (28, 29, 33)


@dataclass
class Variant:
    name: str                               # 变体名（单变体件为模型名）
    cubes: Callable[[], List[dict]]
    flat_stem: str                          # 现有平色 bbmodel 的文件名（不含扩展名）
    palette: Dict[str, tuple]
    override: Optional[Override] = None     # 个别立方体的专属画笔，见 piece_overrides.py


@dataclass
class Piece:
    review_dir: Path
    ref_name: Optional[str]
    variants: List[Variant]
    alias_dir: Optional[Path] = None


def _palette(module) -> Dict[str, tuple]:
    return palette_to_colors(module.PALETTE)


def build_registry() -> Dict[str, Piece]:
    import gen_arm_muscle
    import gen_belt
    import gen_coupling
    import gen_floor_wall
    import gen_leg_muscle
    import gen_lung
    import gen_meridian_bend
    import gen_meridian_straight
    import gen_outer_wall_tank
    import gen_port
    import gen_prep_buffer
    import gen_qi_packet
    import gen_splitter
    import gen_stomach
    import gen_texture_proto

    def single(module, cubes_fn, flat_stem, override: Optional[Override] = None) -> List[Variant]:
        return [Variant(flat_stem, cubes_fn, flat_stem, _palette(module), override)]

    def belt_cubes():
        up = gen_belt.build_slope_up_cubes()
        return {
            "belt_straight": gen_belt.build_straight_cubes(),
            "belt_corner": gen_belt.build_corner_cubes(),
            "belt_slope_up": up,
            "belt_slope_down": gen_belt.build_slope_down_cubes(up),
        }

    belt_palette = _palette(gen_belt)
    belt_variants = [
        Variant(name, (lambda n=name: belt_cubes()[n]), name, belt_palette)
        for name in ("belt_straight", "belt_corner", "belt_slope_up", "belt_slope_down")
    ]

    qi_variants = []
    for key, info in gen_qi_packet.VARIANTS.items():
        colors = {"shell": tuple(info["shell"][:3]), "core": tuple(info["core"][:3])}
        qi_variants.append(Variant(f"qi_packet_{key}", gen_qi_packet.all_cubes, f"qi_packet_{key}", colors))

    def folder(name: str) -> Path:
        return REVIEW_ROOT / name

    return {
        "b01": Piece(gen_meridian_straight.REVIEW_DIR, "b01_meridian_straight.png",
                     [Variant("meridian_straight", gen_texture_proto.textured_cubes, "meridian_straight",
                              _palette(gen_meridian_straight))]),
        "b02": Piece(gen_meridian_bend.REVIEW_DIR, "b02_meridian_bend.png",
                     single(gen_meridian_bend, gen_meridian_bend.all_cubes, "meridian_bend")),
        "b03": Piece(gen_port.REVIEW_DIR, "b03_port.png", [
            Variant("port_in", lambda: gen_port.all_cubes(True), "port_in", _palette(gen_port), port_override),
            Variant("port_out", lambda: gen_port.all_cubes(False), "port_out", _palette(gen_port), port_override),
        ]),
        "b08": Piece(gen_coupling.REVIEW_DIR, "b08_coupling.png", [
            Variant("coupling", gen_coupling.all_cubes, "coupling", _palette(gen_coupling), coupling_override),
        ]),
        "b04": Piece(gen_outer_wall_tank.REVIEW_DIR, "b04_outer_wall_tank.png", [
            Variant(f"outer_wall_tank_{v}", (lambda v=v: gen_outer_wall_tank.all_cubes(v)),
                    f"outer_wall_tank_{v}", _palette(gen_outer_wall_tank), tank_override)
            for v in ("empty", "half", "full")
        ]),
        "b05": Piece(gen_splitter.REVIEW_DIR, "b05_splitter.png",
                     single(gen_splitter, gen_splitter.all_cubes, "splitter")),
        "b06": Piece(gen_prep_buffer.REVIEW_DIR, "b06_prep_buffer.png", [
            Variant(f"prep_buffer_{v}", (lambda v=v: gen_prep_buffer.all_cubes(v)),
                    f"prep_buffer_{v}", _palette(gen_prep_buffer), buffer_override)
            for v in ("empty", "full")
        ]),
        "b07": Piece(gen_belt.REVIEW_DIR, "b07_belt.png", belt_variants, gen_belt.REVIEW_DIR_ALIAS),
        "b09": Piece(gen_floor_wall.REVIEW_DIR, "b09_floor_wall.png", [
            Variant("floor_flesh", gen_floor_wall.build_floor_flesh_cubes, "floor_flesh", _palette(gen_floor_wall), floor_override),
            Variant("wall_bone", gen_floor_wall.build_wall_bone_cubes, "wall_bone", _palette(gen_floor_wall)),
        ], gen_floor_wall.REVIEW_DIR_ALIAS),
        "o01": Piece(gen_lung.REVIEW_DIR, "o01_lung.png",
                     single(gen_lung, gen_lung.all_cubes, "lung", lung_override), gen_lung.REVIEW_DIR_ALIAS),
        "o02": Piece(gen_stomach.REVIEW_DIR, "o02_stomach.png",
                     single(gen_stomach, gen_stomach.all_cubes, "stomach", stomach_override), gen_stomach.REVIEW_DIR_ALIAS),
        "o03": Piece(gen_arm_muscle.REVIEW_DIR, "o03_arm_muscle.png",
                     single(gen_arm_muscle, gen_arm_muscle.all_cubes, "arm_muscle", arm_override), gen_arm_muscle.REVIEW_DIR_ALIAS),
        "o04": Piece(gen_leg_muscle.REVIEW_DIR, "o04_leg_muscle.png",
                     single(gen_leg_muscle, gen_leg_muscle.all_cubes, "leg_muscle"), gen_leg_muscle.REVIEW_DIR_ALIAS),
        "p01": Piece(gen_qi_packet.REVIEW_DIR, "p01_qi_packets.png", qi_variants, gen_qi_packet.REVIEW_DIR_ALIAS),
    }


# =============================================================================
# 渲染与出图
# =============================================================================

def labeled(img: Image.Image, text: str) -> Image.Image:
    out = Image.new("RGB", (img.width, img.height + 26), LABEL_BG)
    out.paste(img, (0, 26))
    ImageDraw.Draw(out).text((8, 8), text, fill=(230, 230, 230))
    return out


def hstack(tiles: List[Image.Image], gap: int = 8) -> Image.Image:
    canvas = Image.new("RGB", (sum(t.width for t in tiles) + gap * (len(tiles) - 1), max(t.height for t in tiles)), CANVAS_BG)
    x = 0
    for tile in tiles:
        canvas.paste(tile, (x, 0))
        x += tile.width + gap
    return canvas


def vstack(rows: List[Image.Image], gap: int = 8) -> Image.Image:
    canvas = Image.new("RGB", (max(r.width for r in rows), sum(r.height for r in rows) + gap * (len(rows) - 1)), CANVAS_BG)
    y = 0
    for row in rows:
        canvas.paste(row, (0, y))
        y += row.height + gap
    return canvas


def shot(path: Path, yaw: float, pitch: float, size: int = TILE) -> Image.Image:
    image, _ = render(path, yaw=yaw, pitch=pitch, size=size, bg=BG, shading="mc")
    return image


def render_sheet(tex_paths: Dict[str, Path]) -> Image.Image:
    """render_tex.png：单变体 = 3/4 + 正视 + 侧视 + 俯视（2×2）；多变体 = 每个变体一张 3/4，每行最多 3 个。"""
    if len(tex_paths) == 1:
        path = next(iter(tex_paths.values()))
        views = (
            (f"3/4 (yaw {ISO_YAW:g}, pitch {ISO_PITCH:g})", ISO_YAW, ISO_PITCH),
            ("front (yaw 0, pitch 0)", 0.0, 0.0),
            ("side (yaw 90, pitch 0)", 90.0, 0.0),
            ("top (yaw 0, pitch 89.9)", 0.0, 89.9),
        )
        tiles = [labeled(shot(path, yaw, pitch), title) for title, yaw, pitch in views]
        return vstack([hstack(tiles[:2]), hstack(tiles[2:])])

    tiles = [labeled(shot(path, ISO_YAW, ISO_PITCH), f"{name} (yaw {ISO_YAW:g}, pitch {ISO_PITCH:g})")
             for name, path in tex_paths.items()]
    rows = [hstack(tiles[i:i + 3]) for i in range(0, len(tiles), 3)]
    return vstack(rows)


def check_sheet(piece: Piece, first: Variant, tex_path: Path) -> Image.Image:
    """check_tex.png：参考图 | 现有平色版 | 贴图版（后两者同一 3/4 取景）。"""
    tiles = []
    ref_path = REVIEW_ROOT / "refs" / piece.ref_name if piece.ref_name else None
    if ref_path and ref_path.exists():
        ref = Image.open(ref_path).convert("RGB")
        ref = ref.resize((int(ref.width * TILE / ref.height), TILE), Image.Resampling.LANCZOS)
        tiles.append(labeled(ref, f"REFERENCE ({ref_path.name})"))
    flat_path = MODEL_DIR / f"{first.flat_stem}.bbmodel"
    if flat_path.exists():
        tiles.append(labeled(shot(flat_path, ISO_YAW, ISO_PITCH), f"FLAT (existing {flat_path.name})"))
    tiles.append(labeled(shot(tex_path, ISO_YAW, ISO_PITCH), f"TEXTURED ({tex_path.name})"))
    return hstack(tiles)


def retexture(piece_id: str, piece: Piece):
    tex_paths: Dict[str, Path] = {}
    for variant in piece.variants:
        out = MODEL_DIR / f"{variant.name}_tex.bbmodel"
        write_textured_bbmodel(variant.cubes(), out, f"{variant.name}_tex", variant.palette, variant.override)
        tex_paths[variant.name] = out
        print(f"  {piece_id}: {out.name}")

    out_dirs = [piece.review_dir] + ([piece.alias_dir] if piece.alias_dir else [])
    for out_dir in out_dirs:
        out_dir.mkdir(parents=True, exist_ok=True)

    first = piece.variants[0]
    sheet = render_sheet(tex_paths)
    check = check_sheet(piece, first, tex_paths[first.name])
    sheet.save(piece.review_dir / "render_tex.png")
    check.save(piece.review_dir / "check_tex.png")
    if piece.alias_dir:
        shutil.copy(piece.review_dir / "render_tex.png", piece.alias_dir / "render_tex.png")
        shutil.copy(piece.review_dir / "check_tex.png", piece.alias_dir / "check_tex.png")
    print(f"✓ {piece_id}: {piece.review_dir / 'render_tex.png'}")


def main():
    registry = build_registry()
    parser = argparse.ArgumentParser(description="经脉工厂统一重贴图（几何不动）")
    parser.add_argument("ids", nargs="*", help="模型编号，如 b01 b02")
    parser.add_argument("--list", action="store_true", help="列出可用编号")
    args = parser.parse_args()

    if args.list or not args.ids:
        print("可用编号:", " ".join(registry))
        return
    for piece_id in args.ids:
        retexture(piece_id, registry[piece_id])


if __name__ == "__main__":
    main()
