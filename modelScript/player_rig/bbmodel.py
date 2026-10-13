"""由切分表生成 Blockbench 工程 JSON（.bbmodel）并导出 Bedrock 几何。"""

from __future__ import annotations

import base64
import io
import json
import subprocess
import uuid
from pathlib import Path

from . import rig, skin

_NS = uuid.UUID("6f1b6a52-7f42-4a6e-9a55-6b0a1a2b3c4d")
HERE = Path(__file__).resolve().parent


def _id(*parts) -> str:
    return str(uuid.uuid5(_NS, "/".join(parts)))


def _texture_source() -> str:
    buffer = io.BytesIO()
    skin.build().save(buffer, "PNG")
    return "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode()


def build(variant: str, texture: bool = True) -> dict:
    slim = variant == "slim"
    bones = rig.build(slim)
    elements, groups = [], {}
    for bone in bones:
        child_ids = []
        for cube in bone.cubes:
            uid = _id(variant, bone.name, "outer" if cube.outer else "inner")
            child_ids.append(uid)
            faces = {}
            for face in ("north", "east", "south", "west", "up", "down"):
                if face in cube.faces:
                    faces[face] = {"uv": cube.faces[face], "texture": 0}
                else:
                    faces[face] = {"uv": [0, 0, 0, 0], "texture": None}
            element = {
                "name": f"{bone.name}_{'outer' if cube.outer else 'inner'}",
                "box_uv": False, "rescale": False, "locked": False, "type": "cube", "uuid": uid,
                "from": list(cube.low), "to": list(cube.high), "autouv": 0,
                "color": 1 if cube.outer else 0, "origin": list(bone.pivot), "rotation": [0, 0, 0],
                "faces": faces,
            }
            if cube.outer:
                element["inflate"] = 0.5 if bone.name == "head" else 0.25
            elements.append(element)
        groups[bone.name] = {
            "name": bone.name, "origin": list(bone.pivot), "rotation": [0, 0, 0], "color": 0,
            "uuid": _id(variant, bone.name, "group"), "export": True, "mirror_uv": False,
            "isOpen": True, "locked": False, "visibility": True, "autouv": 0, "children": child_ids,
        }
    outliner = []
    for bone in bones:
        group = groups[bone.name]
        if bone.parent is None:
            outliner.append(group)
        else:
            groups[bone.parent]["children"].append(group)
    model = {
        "meta": {"format_version": "4.10", "model_format": "bedrock", "box_uv": False},
        "name": f"player_skeleton_{variant}",
        "model_identifier": f"player.{variant}",
        "resolution": {"width": 64, "height": 64},
        "visible_box": [1, 2, 0],
        "elements": elements,
        "outliner": outliner,
        "textures": [],
    }
    if texture:
        model["textures"].append({
            "path": "", "name": "player_debug_skin.png", "folder": "", "namespace": "", "id": "0",
            "particle": False, "render_mode": "default", "render_sides": "auto", "frame_time": 1,
            "frame_order_type": "loop", "frame_order": "", "frame_interpolate": False, "visible": True,
            "mode": "bitmap", "saved": False, "uuid": _id(variant, "texture"), "source": _texture_source(),
        })
    return model


def export_geometry(classic_path: Path, slim_path: Path) -> dict:
    out = subprocess.run(["node", str(HERE / "export_geo.cjs"), str(classic_path), str(slim_path)],
                         check=True, capture_output=True, text=True).stdout
    return json.loads(out)
