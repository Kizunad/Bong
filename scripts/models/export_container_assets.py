#!/usr/bin/env python3
"""导出三种可放置容器 bbmodel → GeckoLib 运行时资源。"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

from export_coffin_assets import _load, build_geo, extract_texture

REPO = Path(__file__).resolve().parents[2]
MODELS = REPO / "local_models"
ASSETS = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong"
GEO_DIR = ASSETS / "geo"
TEXTURE_DIR = ASSETS / "textures" / "entity"
ANIMATION_DIR = ASSETS / "animations"


@dataclass(frozen=True)
class ContainerAsset:
    entity_id: str
    bbmodel_file: str


CONTAINERS = (
    ContainerAsset("trade_crate", "TradeCrate.bbmodel"),
    ContainerAsset("herb_crate_placed", "HerbCrate.bbmodel"),
    ContainerAsset("dead_drop_box", "DeadDropBox.bbmodel"),
)


def _wrap_body_bone(geo: dict) -> None:
    bones = geo["minecraft:geometry"][0]["bones"]
    if any(bone["name"] == "Body" for bone in bones):
        return
    body = {"name": "Body", "parent": "root", "pivot": [0, 8, 0]}
    for bone in bones:
        if bone["name"] != "root":
            bone["parent"] = "Body"
    bones.insert(1, body)


def _build_animation(entity_id: str) -> dict:
    return {
        "format_version": "1.8.0",
        "animations": {
            f"animation.bong.{entity_id}.idle": {
                "loop": True,
                "animation_length": 2.0,
                "bones": {
                    "Body": {
                        "rotation": {
                            "0.0": [0, 0, 0],
                            "1.0": [0, 0.15, 0],
                            "2.0": [0, 0, 0],
                        }
                    }
                },
            }
        },
    }


def export_container(spec: ContainerAsset) -> None:
    bbmodel = _load(MODELS / spec.bbmodel_file)
    geo = build_geo(bbmodel, f"geometry.bong.{spec.entity_id}")
    texture, width, height = extract_texture(bbmodel)
    geo["minecraft:geometry"][0]["description"]["texture_width"] = width
    geo["minecraft:geometry"][0]["description"]["texture_height"] = height
    _wrap_body_bone(geo)

    GEO_DIR.mkdir(parents=True, exist_ok=True)
    TEXTURE_DIR.mkdir(parents=True, exist_ok=True)
    ANIMATION_DIR.mkdir(parents=True, exist_ok=True)

    (GEO_DIR / f"{spec.entity_id}.geo.json").write_text(
        json.dumps(geo, indent=2, ensure_ascii=False) + "\n"
    )
    (TEXTURE_DIR / f"{spec.entity_id}_intact.png").write_bytes(texture)
    (ANIMATION_DIR / f"{spec.entity_id}.animation.json").write_text(
        json.dumps(_build_animation(spec.entity_id), indent=2, ensure_ascii=False) + "\n"
    )
    cube_count = sum(len(bone.get("cubes", [])) for bone in geo["minecraft:geometry"][0]["bones"])
    print(f"{spec.entity_id}: {cube_count} cube, texture {width}x{height}")


def main() -> None:
    for spec in CONTAINERS:
        export_container(spec)


if __name__ == "__main__":
    main()
