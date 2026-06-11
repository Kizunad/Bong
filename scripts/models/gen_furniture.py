#!/usr/bin/env python3
"""生成 4 个家具 Blockbench `.bbmodel`、方块模型 JSON 与预览图。

本脚本用于 `plan-furniture-buff-v1` P4 三轮视觉资产收口。模型坚持 1×1 方块
落地决议：床/防潮地基的背包格子更大，但世界里仍只占一格。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
LOCAL_MODELS = REPO / "local_models"
PREVIEW_DIR = REPO / "scripts" / "models"
BLOCK_MODEL_DIR = (
    REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "models" / "block"
)

TEXTURE_RES = 64
UUID_NAMESPACE = uuid.UUID("4f0d85f8-1874-4e29-9d75-7e44f4b4f001")


@dataclass(frozen=True)
class Cube:
    bone: str
    material: str
    name: str
    origin: tuple[float, float, float]
    target: tuple[float, float, float]


@dataclass(frozen=True)
class FurnitureSpec:
    item_id: str
    model_name: str
    display_name: str
    bones: tuple[str, ...]
    materials: dict[str, tuple[int, int, int]]
    build: Callable[[], list[Cube]]


def cube(
    bone: str,
    material: str,
    name: str,
    origin: tuple[float, float, float],
    target: tuple[float, float, float],
) -> Cube:
    return Cube(bone, material, name, origin, target)


def stable_uuid(*parts: str) -> str:
    return str(uuid.uuid5(UUID_NAMESPACE, ":".join(parts)))


def part_simple_bed_base() -> list[Cube]:
    cubes: list[Cube] = []
    for x_label, x_origin, x_target in (("head", 1.0, 3.0), ("foot", 13.0, 15.0)):
        for z_label, z_origin, z_target in (("left", 2.0, 4.0), ("right", 12.0, 14.0)):
            cubes.append(
                cube(
                    "base",
                    "wood",
                    f"bed_leg_{x_label}_{z_label}",
                    (x_origin, 0.0, z_origin),
                    (x_target, 2.2, z_target),
                )
            )
    cubes.extend(
        [
            cube("base", "wood", "side_rail_left", (1.0, 2.0, 2.0), (15.0, 3.1, 3.2)),
            cube("base", "wood", "side_rail_right", (1.0, 2.0, 12.8), (15.0, 3.1, 14.0)),
            cube("base", "wood", "foot_rail", (13.6, 2.0, 3.2), (15.0, 3.6, 12.8)),
            cube("base", "wood", "head_rail", (1.0, 2.0, 3.2), (2.4, 4.2, 12.8)),
            cube("base", "wood", "support_slats", (2.2, 2.0, 3.4), (13.8, 2.6, 12.6)),
        ]
    )
    return cubes


def part_simple_bed_bedding() -> list[Cube]:
    return [
        cube("bedding", "straw", "straw_mat", (2.2, 2.6, 3.0), (13.8, 3.7, 13.0)),
        cube("bedding", "cloth", "rough_blanket", (5.4, 3.7, 3.3), (13.5, 4.4, 12.7)),
        cube("bedding", "pillow", "cloth_pillow", (2.6, 3.7, 4.0), (5.4, 4.9, 12.0)),
        cube("bedding", "thread", "patched_band", (8.2, 4.42, 3.35), (8.7, 4.55, 12.65)),
    ]


def build_simple_bed() -> list[Cube]:
    return part_simple_bed_base() + part_simple_bed_bedding()


def part_meditation_mat_base() -> list[Cube]:
    return [
        cube("mat", "reed", "mat_core", (4.0, 0.0, 4.0), (12.0, 0.85, 12.0)),
        cube("mat", "reed", "mat_north", (5.4, 0.0, 2.4), (10.6, 0.85, 4.0)),
        cube("mat", "reed", "mat_south", (5.4, 0.0, 12.0), (10.6, 0.85, 13.6)),
        cube("mat", "reed", "mat_west", (2.4, 0.0, 5.4), (4.0, 0.85, 10.6)),
        cube("mat", "reed", "mat_east", (12.0, 0.0, 5.4), (13.6, 0.85, 10.6)),
        cube("mat", "reed", "mat_corner_nw", (3.4, 0.0, 3.4), (5.4, 0.85, 5.4)),
        cube("mat", "reed", "mat_corner_ne", (10.6, 0.0, 3.4), (12.6, 0.85, 5.4)),
        cube("mat", "reed", "mat_corner_sw", (3.4, 0.0, 10.6), (5.4, 0.85, 12.6)),
        cube("mat", "reed", "mat_corner_se", (10.6, 0.0, 10.6), (12.6, 0.85, 12.6)),
    ]


def part_meditation_mat_cushion() -> list[Cube]:
    return [
        cube("cushion", "cloth", "cushion_core", (5.0, 0.85, 5.0), (11.0, 2.15, 11.0)),
        cube("cushion", "cloth", "cushion_north", (6.0, 0.85, 3.9), (10.0, 2.15, 5.0)),
        cube("cushion", "cloth", "cushion_south", (6.0, 0.85, 11.0), (10.0, 2.15, 12.1)),
        cube("cushion", "cloth", "cushion_west", (3.9, 0.85, 6.0), (5.0, 2.15, 10.0)),
        cube("cushion", "cloth", "cushion_east", (11.0, 0.85, 6.0), (12.1, 2.15, 10.0)),
        cube("cushion", "binding", "front_binding", (5.4, 2.15, 4.7), (10.6, 2.42, 5.1)),
        cube("cushion", "binding", "back_binding", (5.4, 2.15, 10.9), (10.6, 2.42, 11.3)),
        cube("cushion", "binding", "left_binding", (4.7, 2.15, 5.4), (5.1, 2.42, 10.6)),
        cube("cushion", "binding", "right_binding", (10.9, 2.15, 5.4), (11.3, 2.42, 10.6)),
        cube("cushion", "patch", "center_patch", (6.6, 2.42, 6.6), (9.4, 2.62, 9.4)),
    ]


def build_meditation_mat() -> list[Cube]:
    return part_meditation_mat_base() + part_meditation_mat_cushion()


def part_moisture_base_stone() -> list[Cube]:
    return [
        cube("base", "stone", "stone_pad", (1.0, 0.0, 2.0), (15.0, 1.6, 14.0)),
        cube("base", "ash", "ash_moss_layer", (1.6, 1.6, 2.6), (14.4, 2.1, 13.4)),
        cube("base", "stone", "front_lip", (1.0, 2.1, 2.0), (15.0, 3.0, 3.0)),
        cube("base", "stone", "back_lip", (1.0, 2.1, 13.0), (15.0, 3.0, 14.0)),
        cube("base", "stone", "left_lip", (1.0, 2.1, 3.0), (2.0, 3.0, 13.0)),
        cube("base", "stone", "right_lip", (14.0, 2.1, 3.0), (15.0, 3.0, 13.0)),
    ]


def part_moisture_base_tray() -> list[Cube]:
    return [
        cube("tray", "wood", "tray_slats_a", (2.2, 3.0, 4.0), (13.8, 3.6, 5.0)),
        cube("tray", "wood", "tray_slats_b", (2.2, 3.0, 7.5), (13.8, 3.6, 8.5)),
        cube("tray", "wood", "tray_slats_c", (2.2, 3.0, 11.0), (13.8, 3.6, 12.0)),
        cube("tray", "wood", "tray_side_left", (2.0, 3.4, 3.6), (3.0, 4.2, 12.4)),
        cube("tray", "wood", "tray_side_right", (13.0, 3.4, 3.6), (14.0, 4.2, 12.4)),
    ]


def build_moisture_base() -> list[Cube]:
    return part_moisture_base_stone() + part_moisture_base_tray()


def part_spirit_stone_rack_frame() -> list[Cube]:
    cubes = [
        cube("frame", "wood", "rack_base", (2.2, 0.0, 3.0), (13.8, 1.4, 13.0)),
        cube("frame", "wood", "left_post", (2.6, 1.4, 3.2), (4.0, 10.5, 4.6)),
        cube("frame", "wood", "right_post", (12.0, 1.4, 3.2), (13.4, 10.5, 4.6)),
        cube("frame", "wood", "back_left_post", (2.6, 1.4, 11.4), (4.0, 8.5, 12.8)),
        cube("frame", "wood", "back_right_post", (12.0, 1.4, 11.4), (13.4, 8.5, 12.8)),
        cube("frame", "wood", "top_front_rail", (2.4, 9.2, 3.1), (13.6, 10.5, 4.5)),
        cube("frame", "wood", "mid_front_rail", (2.4, 5.1, 3.1), (13.6, 6.0, 4.3)),
        cube("frame", "wood", "back_rail", (2.4, 7.4, 11.6), (13.6, 8.5, 12.8)),
    ]
    return cubes


def part_spirit_stone_rack_slots() -> list[Cube]:
    cubes: list[Cube] = [
        cube("slots", "stone", "shelf_slab", (3.2, 3.6, 5.2), (12.8, 4.3, 10.8)),
    ]
    for slot_index, center_x in enumerate((5.0, 8.0, 11.0)):
        cubes.extend(
            [
                cube("slots", "stone", f"slot_{slot_index}_front", (center_x - 1.0, 4.3, 5.5), (center_x + 1.0, 5.1, 6.0)),
                cube("slots", "stone", f"slot_{slot_index}_back", (center_x - 1.0, 4.3, 10.0), (center_x + 1.0, 5.1, 10.5)),
                cube("slots", "stone", f"slot_{slot_index}_left", (center_x - 1.25, 4.3, 6.0), (center_x - 0.75, 5.1, 10.0)),
                cube("slots", "stone", f"slot_{slot_index}_right", (center_x + 0.75, 4.3, 6.0), (center_x + 1.25, 5.1, 10.0)),
            ]
        )
    return cubes


def build_spirit_stone_rack() -> list[Cube]:
    return part_spirit_stone_rack_frame() + part_spirit_stone_rack_slots()


SPECS = [
    FurnitureSpec(
        "simple_bed",
        "SimpleBed",
        "简易床铺",
        ("base", "bedding"),
        {
            "wood": (122, 84, 50),
            "straw": (190, 162, 93),
            "cloth": (138, 104, 78),
            "pillow": (205, 192, 162),
            "thread": (82, 60, 48),
        },
        build_simple_bed,
    ),
    FurnitureSpec(
        "meditation_mat",
        "MeditationMat",
        "蒲团",
        ("mat", "cushion"),
        {
            "reed": (172, 142, 82),
            "cloth": (116, 92, 62),
            "binding": (78, 64, 46),
            "patch": (150, 128, 90),
        },
        build_meditation_mat,
    ),
    FurnitureSpec(
        "moisture_base",
        "MoistureBase",
        "防潮地基",
        ("base", "tray"),
        {
            "stone": (110, 112, 106),
            "ash": (82, 103, 80),
            "wood": (112, 78, 48),
        },
        build_moisture_base,
    ),
    FurnitureSpec(
        "spirit_stone_rack",
        "SpiritStoneRack",
        "灵石架",
        ("frame", "slots"),
        {
            "wood": (106, 74, 48),
            "stone": (120, 118, 108),
        },
        build_spirit_stone_rack,
    ),
]


class Packer:
    def __init__(self, x_origin: float, y_origin: float, x_target: float, y_target: float) -> None:
        self.x_origin = x_origin
        self.y_origin = y_origin
        self.x_target = x_target
        self.y_target = y_target
        self.current_x = x_origin
        self.current_y = y_origin
        self.row_height = 0.0

    def place(self, width: float, height: float) -> tuple[float, float]:
        width = min(width, self.x_target - self.x_origin)
        height = min(height, self.y_target - self.y_origin)
        if self.current_x + width > self.x_target:
            self.current_x = self.x_origin
            self.current_y += self.row_height
            self.row_height = 0.0
        if self.current_y + height > self.y_target:
            self.current_x = self.x_origin
            self.current_y = self.y_origin
        placed = (self.current_x, self.current_y)
        self.current_x += width
        self.row_height = max(self.row_height, height)
        return placed


def material_zones(materials: dict[str, tuple[int, int, int]]) -> dict[str, tuple[float, float, float, float]]:
    material_names = list(materials)
    band_height = TEXTURE_RES / max(1, len(material_names))
    zones = {}
    for index, material in enumerate(material_names):
        zones[material] = (0.0, index * band_height, float(TEXTURE_RES), (index + 1) * band_height)
    return zones


def make_texture(spec: FurnitureSpec) -> Image.Image:
    stable_seed = sum((index + 1) * ord(char) for index, char in enumerate(spec.item_id))
    random = np.random.default_rng(stable_seed)
    image = np.zeros((TEXTURE_RES, TEXTURE_RES, 4), np.uint8)
    image[..., 3] = 255
    zones = material_zones(spec.materials)
    grid_y, grid_x = np.mgrid[0:TEXTURE_RES, 0:TEXTURE_RES]

    for material, color in spec.materials.items():
        _, y_origin, _, y_target = zones[material]
        mask = (grid_y >= y_origin) & (grid_y < y_target)
        base = np.array(color, float)[None, None, :]
        noise = (random.random((TEXTURE_RES, TEXTURE_RES, 1)) - 0.5) * 22
        grain = np.sin(grid_x * 0.55 + grid_y * 0.17)[..., None] * 9
        shaded = np.clip(base + noise + grain, 24, 235)
        image[mask, :3] = shaded[mask].astype(np.uint8)

        stripe = mask & ((grid_x + grid_y) % 13 == 0)
        image[stripe, :3] = np.clip(image[stripe, :3].astype(int) - 32, 0, 255).astype(np.uint8)

    return Image.fromarray(image, "RGBA")


def png_data_url(image: Image.Image) -> str:
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode()


def cube_faces_uv(cube_spec: Cube, packer: Packer) -> dict[str, dict[str, object]]:
    x_size = cube_spec.target[0] - cube_spec.origin[0]
    y_size = cube_spec.target[1] - cube_spec.origin[1]
    z_size = cube_spec.target[2] - cube_spec.origin[2]
    dimensions = {
        "north": (x_size, y_size),
        "south": (x_size, y_size),
        "east": (z_size, y_size),
        "west": (z_size, y_size),
        "up": (x_size, z_size),
        "down": (x_size, z_size),
    }
    faces: dict[str, dict[str, object]] = {}
    for face_name, (width, height) in dimensions.items():
        uv_origin_x, uv_origin_y = packer.place(abs(width), abs(height))
        faces[face_name] = {
            "uv": [
                round(uv_origin_x, 2),
                round(uv_origin_y, 2),
                round(uv_origin_x + abs(width), 2),
                round(uv_origin_y + abs(height), 2),
            ],
            "texture": 0,
        }
    return faces


def build_bbmodel(spec: FurnitureSpec) -> dict[str, object]:
    cubes = spec.build()
    zones = material_zones(spec.materials)
    packers = {
        material: Packer(*zone)
        for material, zone in zones.items()
    }
    elements: list[dict[str, object]] = []
    bone_children: dict[str, list[str]] = {bone: [] for bone in spec.bones}

    for cube_spec in cubes:
        element_uuid = stable_uuid(spec.item_id, "cube", cube_spec.name)
        elements.append(
            {
                "name": cube_spec.name,
                "box_uv": False,
                "rescale": False,
                "locked": False,
                "render_order": "default",
                "allow_mirror_modeling": True,
                "type": "cube",
                "uuid": element_uuid,
                "from": [round(value, 3) for value in cube_spec.origin],
                "to": [round(value, 3) for value in cube_spec.target],
                "autouv": 0,
                "color": spec.bones.index(cube_spec.bone),
                "origin": [0.0, 0.0, 0.0],
                "faces": cube_faces_uv(cube_spec, packers[cube_spec.material]),
            }
        )
        bone_children[cube_spec.bone].append(element_uuid)

    outliner = [
        {
            "name": bone,
            "origin": [0.0, 0.0, 0.0],
            "color": index,
            "uuid": stable_uuid(spec.item_id, "bone", bone),
            "export": True,
            "mirror_uv": False,
            "isOpen": True,
            "locked": False,
            "visibility": True,
            "autouv": 0,
            "children": bone_children[bone],
        }
        for index, bone in enumerate(spec.bones)
    ]
    texture = make_texture(spec)
    return {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": spec.model_name,
        "model_identifier": f"geometry.bong.{spec.item_id}",
        "visible_box": [1.2, 1.2, 0.5],
        "resolution": {"width": TEXTURE_RES, "height": TEXTURE_RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "",
                "name": f"{spec.item_id}.png",
                "folder": "block",
                "namespace": "bong",
                "id": "0",
                "width": TEXTURE_RES,
                "height": TEXTURE_RES,
                "uv_width": TEXTURE_RES,
                "uv_height": TEXTURE_RES,
                "particle": False,
                "render_mode": "default",
                "visible": True,
                "mode": "bitmap",
                "saved": False,
                "uuid": stable_uuid(spec.item_id, "texture"),
                "source": png_data_url(texture),
            }
        ],
    }


def block_model_faces() -> dict[str, dict[str, str]]:
    return {
        "north": {"texture": "#all"},
        "south": {"texture": "#all"},
        "east": {"texture": "#all"},
        "west": {"texture": "#all"},
        "up": {"texture": "#all"},
        "down": {"texture": "#all"},
    }


def build_block_model(spec: FurnitureSpec) -> dict[str, object]:
    return {
        "textures": {
            "particle": f"bong:block/{spec.item_id}",
            "all": f"bong:block/{spec.item_id}",
        },
        "elements": [
            {
                "name": cube_spec.name,
                "from": [round(value, 3) for value in cube_spec.origin],
                "to": [round(value, 3) for value in cube_spec.target],
                "faces": block_model_faces(),
            }
            for cube_spec in spec.build()
        ],
    }


def render_preview(spec: FurnitureSpec, out_path: Path) -> None:
    cubes = spec.build()
    scale = 8
    padding = 16
    gap = 20

    def shaded(color: tuple[int, int, int], factor: float) -> tuple[int, int, int]:
        return tuple(int(np.clip(channel * factor, 0, 255)) for channel in color)

    def project(point: tuple[float, float, float]) -> tuple[float, float]:
        x_coord, y_coord, z_coord = point
        angle_cos = math.cos(math.radians(30))
        angle_sin = math.sin(math.radians(30))
        return (x_coord - z_coord) * angle_cos, (x_coord + z_coord) * angle_sin - y_coord

    points = [
        project((x_coord, y_coord, z_coord))
        for cube_spec in cubes
        for x_coord in (cube_spec.origin[0], cube_spec.target[0])
        for y_coord in (cube_spec.origin[1], cube_spec.target[1])
        for z_coord in (cube_spec.origin[2], cube_spec.target[2])
    ]
    min_x = min(point[0] for point in points)
    max_x = max(point[0] for point in points)
    min_y = min(point[1] for point in points)
    max_y = max(point[1] for point in points)
    width = int((max_x - min_x) * scale) + padding * 2
    height = int((max_y - min_y) * scale) + padding * 2 + 22
    image = Image.new("RGBA", (width, height), (24, 24, 28, 255))
    draw = ImageDraw.Draw(image)
    draw.text((padding, 4), f"{spec.model_name} / {spec.display_name}", fill=(225, 225, 218))

    def to_pixel(point: tuple[float, float]) -> tuple[float, float]:
        return (
            padding + (point[0] - min_x) * scale,
            padding + 20 + (point[1] - min_y) * scale,
        )

    ordered = sorted(cubes, key=lambda item: item.origin[0] + item.origin[1] + item.origin[2])
    for cube_spec in ordered:
        origin = cube_spec.origin
        target = cube_spec.target
        color = spec.materials[cube_spec.material]
        faces = [
            (
                [
                    (origin[0], target[1], origin[2]),
                    (target[0], target[1], origin[2]),
                    (target[0], target[1], target[2]),
                    (origin[0], target[1], target[2]),
                ],
                1.16,
            ),
            (
                [
                    (origin[0], origin[1], target[2]),
                    (target[0], origin[1], target[2]),
                    (target[0], target[1], target[2]),
                    (origin[0], target[1], target[2]),
                ],
                0.9,
            ),
            (
                [
                    (target[0], origin[1], origin[2]),
                    (target[0], origin[1], target[2]),
                    (target[0], target[1], target[2]),
                    (target[0], target[1], origin[2]),
                ],
                0.68,
            ),
        ]
        for vertices, factor in faces:
            polygon = [to_pixel(project(vertex)) for vertex in vertices]
            draw.polygon(polygon, fill=shaded(color, factor) + (255,), outline=(18, 14, 12, 255))

    draw.text(
        (padding, height - 16),
        f"cubes={len(cubes)} bones={','.join(spec.bones)}",
        fill=(185, 185, 178),
    )
    image.save(out_path)


def write_spec(spec: FurnitureSpec) -> None:
    LOCAL_MODELS.mkdir(parents=True, exist_ok=True)
    PREVIEW_DIR.mkdir(parents=True, exist_ok=True)
    BLOCK_MODEL_DIR.mkdir(parents=True, exist_ok=True)

    bbmodel_path = LOCAL_MODELS / f"{spec.model_name}.bbmodel"
    block_model_path = BLOCK_MODEL_DIR / f"{spec.item_id}.json"
    preview_path = PREVIEW_DIR / f"{spec.item_id}_preview.png"

    bbmodel_path.write_text(json.dumps(build_bbmodel(spec), ensure_ascii=False, indent=1) + "\n")
    block_model_path.write_text(json.dumps(build_block_model(spec), ensure_ascii=False, indent=2) + "\n")
    render_preview(spec, preview_path)
    print(f"{spec.model_name}: {len(spec.build())} cubes -> {bbmodel_path.relative_to(REPO)}")
    print(f"  block model: {block_model_path.relative_to(REPO)}")
    print(f"  preview    : {preview_path.relative_to(REPO)}")


def verify_spec(spec: FurnitureSpec) -> None:
    cubes = spec.build()
    if len(cubes) < 4:
        raise ValueError(f"{spec.item_id} cube 数过少，不能退回占位方块")
    seen_names: set[str] = set()
    for cube_spec in cubes:
        if cube_spec.name in seen_names:
            raise ValueError(f"{spec.item_id} cube 名重复：{cube_spec.name}")
        seen_names.add(cube_spec.name)
        if cube_spec.bone not in spec.bones:
            raise ValueError(f"{spec.item_id} cube {cube_spec.name} 使用未知 bone={cube_spec.bone}")
        if cube_spec.material not in spec.materials:
            raise ValueError(f"{spec.item_id} cube {cube_spec.name} 使用未知 material={cube_spec.material}")
        for axis_name, origin_value, target_value in zip(
            ("x", "y", "z"),
            cube_spec.origin,
            cube_spec.target,
        ):
            if not (0.0 <= origin_value < target_value <= 16.0):
                raise ValueError(
                    f"{spec.item_id} cube {cube_spec.name} {axis_name} 坐标越界或退化："
                    f"{origin_value}..{target_value}"
                )
    unused_bones = set(spec.bones) - {cube_spec.bone for cube_spec in cubes}
    if unused_bones:
        raise ValueError(f"{spec.item_id} 存在未使用 bone：{sorted(unused_bones)}")


def verify_all_specs() -> None:
    for spec in SPECS:
        verify_spec(spec)
        print(f"{spec.item_id}: verify ok ({len(spec.build())} cubes)")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--only", choices=[spec.item_id for spec in SPECS], help="只生成一个家具")
    parser.add_argument("--verify", action="store_true", help="只校验规格，不写文件")
    args = parser.parse_args()

    if args.verify:
        verify_all_specs()
        return

    for spec in SPECS:
        if args.only and spec.item_id != args.only:
            continue
        write_spec(spec)


if __name__ == "__main__":
    main()
