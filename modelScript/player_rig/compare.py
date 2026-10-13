#!/usr/bin/env python3
"""零姿态逐像素对照：骨架模板 vs 按原版 UV 搭的 6 部件（+外层）对照模型，同一张皮肤、同一取景。

渲染器（bbmodel_maker）不认 inflate，也会把 texture=null 的面画成黑色，因此渲染前：
  - 对照模型：每个原版部件一个方块，外层方块的 from/to 按原版膨胀值直接外扩；
  - 模板：丢弃切口面，外层按模组读取器的规则（完整部件包围盒统一外扩）展开 from/to。
用法：python3 -m modelScript.player_rig.compare --mod-repo ~/Code/Kizuna-Players-Animation --out DIR
"""

from __future__ import annotations

import argparse
import copy
import json
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image

from bbmodel_maker.render import render_bbmodel

from . import bbmodel, rig, vanilla

VIEWS = [("front", 180, 0), ("back", 0, 0), ("right", 90, 0), ("left", 270, 0),
         ("q_front_r", 145, 22), ("q_back_l", 325, 22), ("top", 180, 60), ("bottom", 180, -60)]
# 零姿态参照里每个 skin part 的内层名 -> 渲染用名
PARTS = ("head", "body", "right_arm", "left_arm", "right_leg", "left_leg")


def _plain(face: dict) -> bool:
    return face.get("texture") is not None


def reference_model(variant: str) -> dict:
    """按原版 6 部件 + 6 外层搭的对照模型，UV 取自重建的原版 Cuboid。"""
    slim = variant == "slim"
    template = bbmodel.build(variant)
    elements = []
    for name in PARTS:
        for skin in (name, vanilla.OVERLAY[name]):
            p = vanilla.part(skin, slim)
            base = vanilla.part(name, slim)
            e = p.dilation
            faces = {}
            for face in vanilla.FACES:
                faces[face] = {"uv": rig._face_uv(p, face, base.low, base.high), "texture": 0}
            elements.append({
                "name": skin, "type": "cube", "uuid": f"ref-{variant}-{skin}",
                "from": [round(a - e, 4) for a in base.low], "to": [round(a + e, 4) for a in base.high],
                "origin": [0, 0, 0], "rotation": [0, 0, 0], "faces": faces,
            })
    ref = copy.deepcopy(template)
    ref["elements"] = elements
    ref["outliner"] = []
    return ref


def render_ready_template(variant: str) -> dict:
    """模板按读取器规则展开外层、剔除切口面。"""
    model = bbmodel.build(variant)
    bones = {b.name: b for b in rig.build(variant == "slim")}
    # 内层方块并出每个原版部件的包围盒
    bounds = {}
    for bone in bones.values():
        for cube in bone.cubes:
            if not cube.outer:
                lo, hi = bounds.get(cube.base_part, (cube.low, cube.high))
                bounds[cube.base_part] = (tuple(min(a, b) for a, b in zip(lo, cube.low)),
                                          tuple(max(a, b) for a, b in zip(hi, cube.high)))
    by_uuid = {}
    for bone in bones.values():
        for cube in bone.cubes:
            by_uuid[bbmodel._id(variant, bone.name, "outer" if cube.outer else "inner")] = cube
    for element in model["elements"]:
        cube = by_uuid[element["uuid"]]
        element["faces"] = {k: v for k, v in element["faces"].items() if _plain(v)}
        if cube.outer:
            e = element.pop("inflate")
            lo, hi = bounds[cube.base_part]
            for key in ("from", "to"):
                # 不取整：相邻分段在切口处必须得到逐位相同的坐标，否则会留下亚像素缝
                element[key] = [p + e * (2 * (p - l) / (h - l) - 1)
                                for p, l, h in zip(element[key], lo, hi)]
    # 原版部件之间有共面重叠（如袖子与夹克前面），渲染器先画者胜；按对照模型的部件顺序排列，
    # 让这类原版固有的 z 竞争在两边判定一致。
    order = [name for part in PARTS for name in (part, vanilla.OVERLAY[part])]
    model["elements"].sort(key=lambda el: order.index(by_uuid[el["uuid"]].skin_part))
    return model


def _only(model: dict, keep) -> dict:
    sub = copy.deepcopy(model)
    sub["elements"] = [e for e in sub["elements"] if keep(e)]
    return sub


def compare(variant: str, out: Path, size: int = 512) -> dict:
    """逐原版部件单独渲染对比（部件之间原版本身有共面重叠，整体叠加时先后判定属于 z 竞争噪声）。"""
    out.mkdir(parents=True, exist_ok=True)
    reference = reference_model(variant)
    template = render_ready_template(variant)
    bones = {b.name: b for b in rig.build(variant == "slim")}
    skin_of = {}
    for bone in bones.values():
        for cube in bone.cubes:
            skin_of[bbmodel._id(variant, bone.name, "outer" if cube.outer else "inner")] = cube.skin_part
    names = [name for part in PARTS for name in (part, vanilla.OVERLAY[part])]
    # 取景中心略微偏离整数，避免像素中心恰好落在纹素边界上（细臂有 0.5 偏移，边界取样由浮点噪声决定）
    focus = ((0.0037, 16.0113, 0.0021), 44)
    result = {}
    with tempfile.TemporaryDirectory() as tmp:
        for view, yaw, pitch in VIEWS:
            total, painted = 0, 0
            tiles = []
            for name in names:
                images = {}
                for label, model, keep in (("reference", reference, lambda e, n=name: e["name"] == n),
                                           ("template", template, lambda e, n=name: skin_of[e["uuid"]] == n)):
                    path = Path(tmp) / f"{label}.bbmodel"
                    path.write_text(json.dumps(_only(model, keep)))
                    image, _ = render_bbmodel.render(path, yaw=yaw, pitch=pitch, size=size, focus=focus,
                                                     shading="mc", bg=(0, 0, 0))
                    images[label] = np.asarray(image).astype(int)
                diff = np.abs(images["reference"] - images["template"]).sum(axis=2) > 0
                total += int(diff.sum())
                painted += int((images["reference"].sum(axis=2) > 0).sum())
                if len(tiles) < 1 or name in ("body", "right_arm", "right_pants"):
                    tiles.append(np.concatenate([images["reference"], images["template"],
                                                 np.repeat((diff * 255)[:, :, None], 3, axis=2)], axis=1))
            result[view] = {"diff_pixels": total, "painted_pixels": painted}
            Image.fromarray(np.concatenate(tiles, axis=0).astype(np.uint8)).resize(
                (size * 3 // 2, size * len(tiles) // 2)).save(out / f"{variant}_{view}.png")
    return result


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    worst = 0
    for variant in ("classic", "slim"):
        result = compare(variant, args.out)
        print(variant, result)
        worst = max(worst, max(v['diff_pixels'] for v in result.values()))
    print("差异像素最大值:", worst)
    raise SystemExit(1 if worst else 0)


if __name__ == "__main__":
    main()
