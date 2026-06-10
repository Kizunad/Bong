#!/usr/bin/env python3
"""导出 Workbench.bbmodel → GeckoLib geo/texture/idle animation 运行时资源。"""

from __future__ import annotations

import json
from pathlib import Path

from export_coffin_assets import _load, build_geo, extract_texture

REPO = Path(__file__).resolve().parents[2]
MODEL = REPO / "local_models" / "Workbench.bbmodel"
ASSETS = REPO / "client" / "src" / "main" / "resources" / "assets" / "bong"
GEO = ASSETS / "geo" / "workbench.geo.json"
TEXTURE = ASSETS / "textures" / "entity" / "workbench_intact.png"
ANIMATION = ASSETS / "animations" / "workbench.animation.json"


def _wrap_body_bone(geo: dict) -> None:
    bones = geo["minecraft:geometry"][0]["bones"]
    if any(bone["name"] == "Body" for bone in bones):
        return
    body = {"name": "Body", "parent": "root", "pivot": [0, 8, 0]}
    for bone in bones:
        if bone["name"] != "root":
            bone["parent"] = "Body"
    bones.insert(1, body)


def _build_animation() -> dict:
    return {
        "format_version": "1.8.0",
        "animations": {
            "animation.bong.workbench.idle": {
                "loop": True,
                "animation_length": 2.0,
                "bones": {
                    "Body": {
                        "rotation": {
                            "0.0": [0, 0, 0],
                            "1.0": [0, 0.2, 0],
                            "2.0": [0, 0, 0],
                        }
                    }
                },
            }
        },
    }


def main() -> None:
    bbmodel = _load(MODEL)
    geo = build_geo(bbmodel, "geometry.bong.workbench")
    texture, width, height = extract_texture(bbmodel)
    geo["minecraft:geometry"][0]["description"]["texture_width"] = width
    geo["minecraft:geometry"][0]["description"]["texture_height"] = height
    _wrap_body_bone(geo)

    GEO.parent.mkdir(parents=True, exist_ok=True)
    TEXTURE.parent.mkdir(parents=True, exist_ok=True)
    ANIMATION.parent.mkdir(parents=True, exist_ok=True)
    GEO.write_text(json.dumps(geo, indent=2, ensure_ascii=False) + "\n")
    TEXTURE.write_bytes(texture)
    ANIMATION.write_text(json.dumps(_build_animation(), indent=2, ensure_ascii=False) + "\n")
    cube_count = sum(len(bone.get("cubes", [])) for bone in geo["minecraft:geometry"][0]["bones"])
    print(f"导出 Workbench：{cube_count} cube，贴图 {width}x{height}")


if __name__ == "__main__":
    main()
