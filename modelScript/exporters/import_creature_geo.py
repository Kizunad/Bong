"""将已有 Bedrock cuboid 模型导入可编辑 bbmodel；坐标/UV 是离线 codec 的逆变换。"""
from __future__ import annotations

import argparse
import base64
import json
from pathlib import Path
from uuid import NAMESPACE_URL, uuid5


def import_geo(geometry: dict, texture: bytes, name: str) -> dict:
    model = geometry["minecraft:geometry"][0]
    description = model["description"]
    elements = []
    groups = {}
    roots = []
    box_uv = all(isinstance(cube["uv"], list) for bone in model["bones"] for cube in bone.get("cubes", []))

    def uid(value):
        return str(uuid5(NAMESPACE_URL, f"bong/fauna/{name}/{value}"))

    def pivot(value):
        return [-value[0], value[1], value[2]]

    def rotation(value):
        return [-value[0], -value[1], value[2]]

    for bone in model["bones"]:
        group = {"uuid": uid(bone["name"]), "name": bone["name"],
                 "origin": pivot(bone.get("pivot", [0, 0, 0])),
                 "rotation": rotation(bone.get("rotation", [0, 0, 0])), "children": []}
        groups[bone["name"]] = group
        for i, cube in enumerate(bone.get("cubes", [])):
            origin, size = cube["origin"], cube["size"]
            start = [-origin[0] - size[0], origin[1], origin[2]]
            element = {"uuid": uid(f"{bone['name']}/{i}"), "name": f"{bone['name']}_{i}", "type": "cube",
                       "from": start, "to": [a + b for a, b in zip(start, size)],
                       "origin": pivot(cube.get("pivot", bone.get("pivot", [0, 0, 0]))),
                       "rotation": rotation(cube.get("rotation", [0, 0, 0])),
                       "inflate": cube.get("inflate", 0), "box_uv": box_uv,
                       "mirror_uv": cube.get("mirror", bone.get("mirror", False))}
            if box_uv:
                element["uv_offset"] = cube["uv"]
                # bbmodel 保留实际面 UV，离线预览器也能读取，不能只填占位 [0,0,1,1]。
                u, v = cube["uv"]
                w, h, depth = size
                faces = {
                    "east": [u, v + depth, u + depth, v + depth + h],
                    "north": [u + depth, v + depth, u + depth + w, v + depth + h],
                    "west": [u + depth + w, v + depth, u + 2 * depth + w, v + depth + h],
                    "south": [u + 2 * depth + w, v + depth, u + 2 * depth + 2 * w, v + depth + h],
                    "up": [u + depth + w, v + depth, u + depth, v],
                    "down": [u + depth + 2 * w, v, u + depth + w, v + depth],
                }
                if element["mirror_uv"]:
                    faces["east"], faces["west"] = faces["west"], faces["east"]
                    faces = {face: [uv[2], uv[1], uv[0], uv[3]] for face, uv in faces.items()}
                element["faces"] = {face: {"uv": uv, "texture": 0} for face, uv in faces.items()}
            else:
                if not isinstance(cube["uv"], dict):
                    raise ValueError("混合 box UV/per-face UV 请用 Blockbench 导入")
                element["faces"] = {}
                for face, uv in cube["uv"].items():
                    u, v = uv["uv"]
                    w, h = uv["uv_size"]
                    if face in ("up", "down"):
                        u, v, w, h = u + w, v + h, -w, -h
                    element["faces"][face] = {"uv": [u, v, u + w, v + h], "texture": 0}
            elements.append(element)
            group["children"].append(element["uuid"])
    for bone in model["bones"]:
        group = groups[bone["name"]]
        if bone.get("parent"):
            groups[bone["parent"]]["children"].append(group)
        else:
            roots.append(group)
    return {"meta": {"format_version": "4.10", "model_format": "free", "box_uv": box_uv},
            "name": name, "resolution": {"width": description["texture_width"], "height": description["texture_height"]},
            "elements": elements, "outliner": roots, "animations": [],
            "textures": [{"uuid": uid("texture"), "name": name, "id": "0",
                          "width": description["texture_width"], "height": description["texture_height"],
                          "source": "data:image/png;base64," + base64.b64encode(texture).decode()}]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("geo", type=Path)
    parser.add_argument("texture", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(import_geo(json.loads(args.geo.read_text()), args.texture.read_bytes(),
                                               args.output.stem), indent=2) + "\n")
