#!/usr/bin/env python3
"""护甲 bbmodel/贴图/真实三视图的确定性生成公共层。"""

from __future__ import annotations
import sys

import base64
import io
import json
import uuid
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
import workspace  # noqa: E402

TEXTURE_SIZE = 64
MODEL_NAMESPACE = uuid.UUID("cfbe55ab-8e59-4b47-8500-21f93600a1f9")

MOUNT_X = {
    "HEAD": 0.0,
    "BODY": 0.0,
    "LEFT_LEG": 1.9,
    "RIGHT_LEG": -1.9,
    "LEFT_FOOT": 1.9,
    "RIGHT_FOOT": -1.9,
}

MOUNT_PIVOT = {
    "HEAD": (0.0, 24.0, 0.0),
    "BODY": (0.0, 24.0, 0.0),
    "LEFT_LEG": (1.9, 12.0, 0.0),
    "RIGHT_LEG": (-1.9, 12.0, 0.0),
    "LEFT_FOOT": (1.9, 12.0, 0.0),
    "RIGHT_FOOT": (-1.9, 12.0, 0.0),
}


@dataclass(frozen=True)
class Cube:
    mount: str
    name: str
    origin: tuple[float, float, float]
    size: tuple[float, float, float]
    uv: tuple[int, int]

    @property
    def end(self) -> tuple[float, float, float]:
        return tuple(self.origin[i] + self.size[i] for i in range(3))


@dataclass(frozen=True)
class ArmorPart:
    key: str
    display_name: str
    cubes: tuple[Cube, ...]


def validate_part(part: ArmorPart) -> None:
    if not part.key or not part.cubes:
        raise ValueError(f"armor part must have key and cubes: {part.key!r}")
    names: set[str] = set()
    for cube in part.cubes:
        if cube.mount not in MOUNT_X:
            raise ValueError(f"{part.key}/{cube.name}: unknown mount {cube.mount}")
        if cube.name in names:
            raise ValueError(f"{part.key}: duplicate cube name {cube.name}")
        names.add(cube.name)
        if any(value <= 0.0 for value in cube.size):
            raise ValueError(f"{part.key}/{cube.name}: cube size must be positive")
        u, v = cube.uv
        sx, sy, sz = cube.size
        if u < 0 or v < 0 or u + 2 * (sx + sz) > TEXTURE_SIZE or v + sy + sz > TEXTURE_SIZE:
            raise ValueError(f"{part.key}/{cube.name}: box UV exceeds {TEXTURE_SIZE}x{TEXTURE_SIZE}")


def _box_uv_faces(cube: Cube) -> dict[str, dict[str, object]]:
    u, v = cube.uv
    sx, sy, sz = cube.size
    return {
        "west": {"uv": [u, v + sz, u + sz, v + sz + sy], "texture": 0},
        "north": {"uv": [u + sz, v + sz, u + sz + sx, v + sz + sy], "texture": 0},
        "east": {"uv": [u + sz + sx, v + sz, u + 2 * sz + sx, v + sz + sy], "texture": 0},
        "south": {"uv": [u + 2 * sz + sx, v + sz, u + 2 * (sz + sx), v + sz + sy], "texture": 0},
        "up": {"uv": [u + sz, v, u + sz + sx, v + sz], "texture": 0},
        "down": {"uv": [u + sz + sx, v, u + sz + 2 * sx, v + sz], "texture": 0},
    }


def _texture_data_url(texture: Image.Image) -> str:
    data = io.BytesIO()
    texture.save(data, format="PNG")
    return "data:image/png;base64," + base64.b64encode(data.getvalue()).decode("ascii")


def build_bbmodel(
    material: str,
    part: ArmorPart,
    texture: Image.Image,
    namespace: str | None = None,
) -> dict[str, object]:
    validate_part(part)
    if texture.size != (TEXTURE_SIZE, TEXTURE_SIZE):
        raise ValueError(f"texture must be {TEXTURE_SIZE}x{TEXTURE_SIZE}, got {texture.size}")

    elements: list[dict[str, object]] = []
    children: dict[str, list[str]] = {}
    for cube in part.cubes:
        x_offset = MOUNT_X[cube.mount]
        origin = (cube.origin[0] + x_offset, cube.origin[1], cube.origin[2])
        end = (cube.end[0] + x_offset, cube.end[1], cube.end[2])
        element_id = str(uuid.uuid5(MODEL_NAMESPACE, f"{material}/{part.key}/{cube.name}"))
        elements.append(
            {
                "name": cube.name,
                "box_uv": False,
                "rescale": False,
                "locked": False,
                "render_order": "default",
                "allow_mirror_modeling": True,
                "type": "cube",
                "uuid": element_id,
                "from": [round(value, 3) for value in origin],
                "to": [round(value, 3) for value in end],
                "autouv": 0,
                "color": list(MOUNT_X).index(cube.mount),
                "origin": list(MOUNT_PIVOT[cube.mount]),
                "faces": _box_uv_faces(cube),
            }
        )
        children.setdefault(cube.mount, []).append(element_id)

    outliner = []
    for mount, cube_ids in children.items():
        outliner.append(
            {
                "name": mount.lower(),
                "origin": list(MOUNT_PIVOT[mount]),
                "color": list(MOUNT_X).index(mount),
                "uuid": str(uuid.uuid5(MODEL_NAMESPACE, f"{material}/{part.key}/{mount}")),
                "export": True,
                "mirror_uv": False,
                "isOpen": True,
                "locked": False,
                "visibility": True,
                "autouv": 0,
                "children": cube_ids,
            }
        )

    return {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": part.key,
        "model_identifier": f"geometry.{namespace or workspace.default().namespace}.{part.key}",
        "visible_box": [2, 2, 0.5],
        "resolution": {"width": TEXTURE_SIZE, "height": TEXTURE_SIZE},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "",
                "name": f"{material}_armor.png",
                "folder": "armor",
                "namespace": namespace or workspace.default().namespace,
                "id": "0",
                "width": TEXTURE_SIZE,
                "height": TEXTURE_SIZE,
                "uv_width": TEXTURE_SIZE,
                "uv_height": TEXTURE_SIZE,
                "particle": False,
                "render_mode": "default",
                "visible": True,
                "mode": "bitmap",
                "saved": False,
                "uuid": str(uuid.uuid5(MODEL_NAMESPACE, f"{material}/texture")),
                "source": _texture_data_url(texture),
            }
        ],
    }


def write_material_assets(
    material: str,
    parts: tuple[ArmorPart, ...],
    texture: Image.Image,
    local_models: Path,
    texture_root: Path,
    preview_root: Path,
    render_previews: bool = True,
) -> dict[str, Path]:
    if len({part.key for part in parts}) != len(parts):
        raise ValueError(f"{material}: duplicate armor part key")

    model_dir = local_models / "armor" / material
    model_dir.mkdir(parents=True, exist_ok=True)
    texture_root.mkdir(parents=True, exist_ok=True)
    preview_root.mkdir(parents=True, exist_ok=True)
    outputs: dict[str, Path] = {}

    for part in parts:
        model_name = "".join(word.capitalize() for word in part.key.split("_")) + ".bbmodel"
        model_path = model_dir / model_name
        model_path.write_text(
            json.dumps(build_bbmodel(material, part, texture), ensure_ascii=False, indent=1) + "\n",
            encoding="utf-8",
        )
        texture_path = texture_root / part.key / "0.png"
        texture_path.parent.mkdir(parents=True, exist_ok=True)
        texture.save(texture_path)
        outputs[f"model:{part.key}"] = model_path
        outputs[f"texture:{part.key}"] = texture_path

    if render_previews:
        from render_bbmodel import render_three_view

        tiles: list[tuple[str, Image.Image]] = []
        for part in parts:
            model_path = outputs[f"model:{part.key}"]
            preview, _ = render_three_view(model_path, size=320)
            preview_path = preview_root / f"{part.key}_render_three_view.png"
            preview.save(preview_path)
            outputs[f"preview:{part.key}"] = preview_path
            tiles.append((part.display_name, preview))

        combined = _combine_previews(material, tiles)
        combined_path = preview_root / f"{material}_armor_render_all.png"
        combined.save(combined_path)
        outputs["preview:all"] = combined_path

    return outputs


def _combine_previews(material: str, tiles: list[tuple[str, Image.Image]]) -> Image.Image:
    gap = 16
    label_height = 20
    width = max(image.width for _, image in tiles) + gap * 2
    height = sum(image.height + label_height for _, image in tiles) + gap * (len(tiles) + 1)
    canvas = Image.new("RGB", (width, height), (14, 15, 17))
    draw = ImageDraw.Draw(canvas)
    draw.text((gap, 4), f"{material.upper()} ARMOR", fill=(225, 225, 218))
    y = gap + label_height
    for label, image in tiles:
        draw.text((gap, y - 14), label, fill=(205, 205, 198))
        canvas.paste(image, (gap, y))
        y += image.height + label_height + gap
    return canvas
