#!/usr/bin/env python3
"""给生物的作者 bbmodel（rigkit 生成，带骨骼层级）上像素贴图，并出 check_tex.png。

和手持物那条线（handheld_retexture）共用同一个画笔库 ``generators/handheld_texture.py``：
每个 texel 按它在模型里的 3D 位置取画笔颜色，所以拼接的立方体之间纹理连续。
区别只有一处：手持物是「一堆平铺的立方体」，生物是「带骨骼树的 rig」，不能把 outliner 拍平，
所以这里不走 ``write_textured_bbmodel``，而是读入已有的 bbmodel，只改每个立方体 6 个面的 UV 与整张图集，
骨骼层级、枢轴、旋转原样保留（动画和 GeckoLib 导出都靠它们）。

用法（生成器里）::

    from creature_tex import texture_bbmodel, write_check_tex
    texture_bbmodel(flat_path, tex_path, mats=tuple(MATS), library_names=LIBRARY_MATS)
    write_check_tex(flat_path, tex_path, check_path, views, ref_path)

``mats`` 是生成器里 MATS 字典的键序：rigkit 把立方体的 color 写成「材质序号 % 8」，靠它还原材质名；
``library_names`` 把生成器自己的材质名映射到画笔库里的材质名。
"""

from __future__ import annotations

import base64
import io
import json
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Sequence, Tuple

from PIL import Image, ImageDraw

TOOLS = Path(__file__).resolve().parent
sys.path.insert(0, str(TOOLS))
sys.path.insert(0, str(TOOLS.parent / "generators"))
sys.path.insert(0, str(TOOLS.parent / "generators" / "meridian_factory"))

from cjk_font import load_font  # noqa: E402
from handheld_texture import handheld_override  # noqa: E402
import pixel_texture  # noqa: E402
from pixel_texture import FACE_NAMES, build_atlas  # noqa: E402

# 生物比手持物精细（0.5px 级的骨刺、碎皮），2 texel/px 时细件整面只有 1~2 个 texel，画笔没地方画；
# 生物进程里提到 4。pixel_texture 的 texel_noise 和图集尺寸都在调用时读这个全局，手持物的进程不受影响。
CREATURE_TEXELS_PER_PX = 4
pixel_texture.TEXELS_PER_PX = CREATURE_TEXELS_PER_PX

View = Tuple[str, float, float, Tuple[Tuple[float, float, float], float]]


def texture_bbmodel(src: Path, out: Path, mats: Sequence[str], library_names: Dict[str, str]) -> Image.Image:
    """读 src（平色 bbmodel），写 out（贴图版），返回图集。立方体没登记到画笔库的材质直接报错，不静默退成默认色。"""
    doc = json.loads(src.read_text(encoding="utf-8"))
    cubes: List[dict] = []
    for element in doc["elements"]:
        mat = mats[int(element["color"]) % len(mats)]
        if mat not in library_names:
            raise KeyError(f"{element['name']}: 材质 {mat} 没有映射到画笔库")
        cubes.append({
            "name": element["name"],
            "from": element["from"],
            "to": element["to"],
            "material": library_names[mat],
        })
    atlas, uvs = build_atlas(cubes, override=handheld_override)

    for element, face_uvs in zip(doc["elements"], uvs):
        element["box_uv"] = False
        element["faces"] = {face: {"uv": face_uvs[face], "texture": 0} for face in FACE_NAMES}
    buf = io.BytesIO()
    atlas.save(buf, format="PNG")
    doc["resolution"] = {"width": atlas.width, "height": atlas.height}
    doc["textures"] = [{
        "path": "", "name": f"{doc['name']}.png", "folder": "", "namespace": "bong", "id": "0",
        "particle": True, "render_mode": "default", "visible": True, "mode": "bitmap", "saved": False,
        "uuid": str(uuid.uuid4()),
        "source": "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii"),
    }]
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(doc, ensure_ascii=False, indent=2), encoding="utf-8")
    return atlas


def write_check_tex(flat: Path, tex: Path, out: Path, views: Sequence[View], ref: Path | None,
                    cell: int = 500) -> Path:
    """check_tex.png：第一行 = 参考图 | 平色版（第一个视角）| 贴图版（全部视角）；
    视角 = (标签, yaw, pitch, (焦点, 取景跨度))。"""
    from bbmodel_maker.render.render_bbmodel import render

    bg = (119, 119, 119)
    font_t, font_l = load_font(18), load_font(13)
    label, yaw, pitch, focus = views[0]
    flat_im, _ = render(flat, yaw=yaw, pitch=pitch, size=cell, bg=bg, focus=focus)
    tiles: List[Tuple[str, Image.Image]] = [(f"平色 FLAT：{label}", flat_im)]
    for label, yaw, pitch, focus in views:
        im, _ = render(tex, yaw=yaw, pitch=pitch, size=cell, bg=bg, focus=focus)
        tiles.append((f"贴图 TEX：{label}", im))

    width = cell * len(tiles) + 10 * (len(tiles) + 1)
    ref_im = None
    if ref is not None and ref.exists():
        ref_im = Image.open(ref).convert("RGB")
        ref_im = ref_im.resize((int(ref_im.width * cell / ref_im.height), cell), Image.Resampling.LANCZOS)
        width += ref_im.width + 10
    canvas = Image.new("RGB", (width, cell + 44), (25, 26, 29))
    draw = ImageDraw.Draw(canvas)
    x = 10
    if ref_im is not None:
        canvas.paste(ref_im, (x, 34))
        draw.text((x + 4, 8), "爆炸参考图 REF", fill=(215, 205, 180), font=font_t)
        x += ref_im.width + 10
    for title, im in tiles:
        canvas.paste(im, (x, 34))
        draw.text((x + 4, 10), title, fill=(215, 205, 180), font=font_l)
        x += cell + 10
    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out)
    return out
