#!/usr/bin/env python3
"""给 rigkit.Rig 搭好的生物模型贴像素贴图，保留骨骼层级（outliner / pivot）。

`handheld_texture` 的 `write_textured_bbmodel` 只出平铺的分组，没有骨骼 pivot，动画用不了。
这里复用它背后的 `build_atlas`（每面独立 UV、2 texel / px、按 3D 位置上色），
只是把图集 UV 写回 Rig 的 element，outliner 仍用 `rig.outliner()`。

用法::

    cube_materials = {element_uuid: "hide", ...}      # Rig.cube 返回的 element["uuid"] -> 画笔库材质名
    doc = rigged_textured_bbmodel(rig, cube_materials, "CanBeiTuo")
    flat = rigged_textured_bbmodel(rig, cube_materials, "CanBeiTuo_flat", flat=True)
"""

from __future__ import annotations

import base64
import io
import sys
import uuid
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from handheld_texture import flat_override, handheld_override  # noqa: E402
from pixel_texture import FACE_NAMES, build_atlas  # noqa: E402


def rigged_textured_bbmodel(rig, cube_materials: dict[str, str], name: str, flat: bool = False) -> dict:
    """返回带内嵌贴图的 bbmodel dict。flat=True 出平色对照版。"""
    cubes = [
        {
            "name": el["name"],
            "from": el["from"],
            "to": el["to"],
            "origin": el["origin"],
            "rotation": el["rotation"],
            "material": cube_materials[el["uuid"]],
        }
        for el in rig.elements
    ]
    atlas, uvs = build_atlas(cubes, override=flat_override if flat else handheld_override)
    buf = io.BytesIO()
    atlas.save(buf, format="PNG")

    elements = []
    for el, face_uvs in zip(rig.elements, uvs):
        out = dict(el)
        out["faces"] = {face: {"uv": face_uvs[face], "texture": 0} for face in FACE_NAMES}
        elements.append(out)

    return {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": name,
        "model_identifier": name,
        "resolution": {"width": atlas.width, "height": atlas.height},
        "elements": elements,
        "outliner": rig.outliner(),
        "textures": [
            {
                "path": "",
                "name": f"{name}.png",
                "folder": "",
                "namespace": "bong",
                "id": "0",
                "particle": True,
                "render_mode": "default",
                "visible": True,
                "mode": "bitmap",
                "saved": False,
                "uuid": str(uuid.uuid4()),
                "source": "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii"),
            }
        ],
    }
