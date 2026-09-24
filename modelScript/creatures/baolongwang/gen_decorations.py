#!/usr/bin/env python3
"""原版暴龙王的附加炉体与融合人臂；原几何、骨树与动画保留。"""

from __future__ import annotations

import argparse
import base64
import copy
import io
import json
import math
import uuid
from pathlib import Path

import numpy as np
from PIL import Image, ImageOps

from bbmodel_maker.rig.rigkit import shaft_box

ROOT = Path(__file__).resolve().parents[3]
MODELS = ROOT / "modelScript/models/baolongwang"
BASE = MODELS / "BaolongwangBase.bbmodel"
MATERIALS = ROOT / "modelScript/assets/refs/baolongwang/decorations_materials.png"
FACES = ("north", "east", "south", "west", "up", "down")
MATERIAL_NAMES = ("iron", "bronze", "soot", "skin", "scar", "ember")


def stable_id(name: str) -> str:
    return str(uuid.uuid5(uuid.NAMESPACE_URL, "bong:baolongwang:decorations:" + name))


def load_base() -> dict:
    return json.loads(BASE.read_text())


def texture_image(texture: dict) -> Image.Image:
    return Image.open(io.BytesIO(base64.b64decode(texture["source"].split(",", 1)[1]))).convert("RGBA")


def aged_furnace_patches(patches: dict[str, Image.Image]) -> dict[str, Image.Image]:
    """沿用已有 AI 材质的斑块结构，离线调色、叠加腐蚀与烟灰。"""
    def relief(name):
        small = patches[name].resize((32, 32), Image.Resampling.BOX).convert("L")
        return ImageOps.autocontrast(small, cutoff=4)

    iron, bronze, soot = (relief(name) for name in ("iron", "bronze", "soot"))
    rust_mask = iron.point(lambda p: max(0, min(255, (p - 125) * 4)))
    copper_mask = bronze.point(lambda p: max(0, min(255, (75 - p) * 5)))
    smoke_mask = soot.transpose(Image.Transpose.ROTATE_90).point(lambda p: int(p * .72))
    old_iron = ImageOps.colorize(iron, (28, 32, 33), (91, 96, 94))
    rust = ImageOps.colorize(iron, (52, 31, 25), (150, 82, 47))
    old_iron = Image.composite(rust, old_iron, rust_mask)
    verdigris = ImageOps.colorize(bronze, (31, 51, 43), (103, 149, 122))
    copper = ImageOps.colorize(bronze, (81, 60, 36), (140, 107, 67))
    verdigris = Image.composite(copper, verdigris, copper_mask)
    carbon = ImageOps.colorize(soot, (17, 19, 20), (54, 57, 56))
    smoked = Image.composite(carbon, verdigris, smoke_mask)
    ash = ImageOps.colorize(soot, (44, 42, 39), (133, 136, 127))
    materials = dict(iron=old_iron, bronze=verdigris, soot=carbon,
                     rust=rust, smoked_bronze=smoked, ash=ash)
    return {name: patch.convert("RGBA").resize((128, 128), Image.Resampling.NEAREST)
            for name, patch in materials.items()}


def build_atlas(base: dict, round_number: int) -> tuple[Image.Image, dict[str, tuple[float, float]]]:
    """原图区不缩放；画布横向翻倍，同时 UV 宽度翻倍。"""
    original = texture_image(base["textures"][0])
    atlas = Image.new("RGBA", (original.width * 2, original.height))
    atlas.paste(original, (0, 0))
    factor = base["resolution"]["width"] / original.width
    origins = {}
    patches = {}
    with Image.open(MATERIALS) as source:
        tw, th = source.width / 3, source.height / 2
        for i, name in enumerate(MATERIAL_NAMES):
            col, row = i % 3, i // 3
            patch = source.crop((int((col + .15) * tw), int((row + .15) * th),
                                 int((col + .85) * tw), int((row + .85) * th)))
            patch = patch.convert("RGBA").resize((32, 32), Image.Resampling.BOX)
            patch = patch.resize((128, 128), Image.Resampling.NEAREST)
            patches[name] = patch
            x, y = original.width + col * 128, row * 128
            atlas.paste(patch, (x, y))
            origins[name] = ((x + 12) * factor, (y + 12) * factor)
    if round_number >= 3:
        # 人臂与融合组织仍取原材质；做旧专用块放进原图集的空白行。
        for i, (name, patch) in enumerate(aged_furnace_patches(patches).items()):
            col, row = i % 3, i // 3
            x, y = original.width + col * 128, row * 256
            atlas.paste(patch, (x, y))
            origins[name] = ((x + 12) * factor, (y + 12) * factor)
    return atlas, origins


class Decorations:
    def __init__(self, base: dict, round_number: int):
        self.base = base
        self.doc = copy.deepcopy(base)
        self.round = round_number
        self.nodes = {}

        def walk(node):
            if isinstance(node, dict):
                self.nodes[node["uuid"]] = node
                for child in node.get("children", []):
                    walk(child)

        for node in self.doc["outliner"]:
            walk(node)
        self.groups = {g["name"]: g for g in self.doc["groups"]}
        self.atlas, self.uv = build_atlas(base, round_number)
        stream = io.BytesIO()
        self.atlas.save(stream, format="PNG")
        texture = self.doc["textures"][0]
        texture.update(name="baolongwang_decorated.png", width=self.atlas.width,
                       height=self.atlas.height, uv_width=base["resolution"]["width"] * 2,
                       source="data:image/png;base64," + base64.b64encode(stream.getvalue()).decode())
        self.doc["resolution"]["width"] *= 2
        self.doc["name"] = f"BaolongwangDecorated_round{round_number}"

    def group(self, name, origin, parent="bdk_body"):
        group = {"name": name, "uuid": stable_id(name), "origin": list(origin),
                 "rotation": [0, 0, 0], "visibility": True, "export": True,
                 "children": [], "shade": True, "color": 3}
        node = {"uuid": group["uuid"], "children": []}
        self.nodes[self.groups[parent]["uuid"]]["children"].append(node)
        self.nodes[group["uuid"]] = node
        self.groups[name] = group
        self.doc["groups"].append(group)

    def cube(self, bone, name, lower, upper, material, rotation=(0, 0, 0), origin=None):
        lower, upper = np.minimum(lower, upper), np.maximum(lower, upper)
        dx, dy, dz = upper - lower
        u, v = self.uv[material]
        # 各面从生成的材质块取样，保持所有面落在同一材质区内。
        dims = ((dx, dy), (dz, dy), (dx, dy), (dz, dy), (dx, dz), (dx, dz))
        faces = {face: {"uv": [u, v, u + max(2, w * 2), v + max(2, h * 2)], "texture": 0}
                 for face, (w, h) in zip(FACES, dims)}
        if self.round >= 3 and bone == "organ_furnace" and material != "ember":
            # 小炉沿也需读出锈斑；逐面错开采样，避免所有金属面重复同一角像素。
            factor = self.base["resolution"]["width"] / (self.atlas.width / 2)
            seed = uuid.UUID(stable_id(name)).int
            for i, (face, (w, h)) in enumerate(zip(FACES, dims)):
                scale = 88 / max(w, h)
                pw, ph = max(12, w * scale), max(12, h * scale)
                du, dv = ((seed >> (i * 4)) % 9) * factor, ((seed >> (i * 4 + 2)) % 9) * factor
                faces[face]["uv"] = [u + du, v + dv, u + du + pw * factor, v + dv + ph * factor]
        element = {"name": name, "uuid": stable_id(name), "type": "cube", "box_uv": False,
                   "from": lower.tolist(), "to": upper.tolist(), "origin": list(origin or (lower + upper) / 2),
                   "rotation": list(rotation), "faces": faces, "color": 3, "autouv": 0}
        self.doc["elements"].append(element)
        self.nodes[self.groups[bone]["uuid"]]["children"].append(element["uuid"])
        return element

    def shaft(self, bone, name, a, b, width, depth, material):
        lower, upper, rotation, origin = shaft_box(tuple(a), tuple(b), width / 2, depth / 2, extend=.25)
        return self.cube(bone, name, lower, upper, material, rotation, origin)


def part_furnace(builder: Decorations) -> None:
    center = (0, 86, -32)
    builder.group("organ_furnace", center)
    # 炉膛朝 -Z。后半体积位于原胸腹内，前缘以组织包边接回宿主。
    builder.cube("organ_furnace", "furnace_buried_shell", (-7, 79, -31), (7, 93, -18), "iron")
    builder.cube("organ_furnace", "furnace_chamber", (-5.9, 80.4, -35.8), (5.9, 91.6, -34.5), "soot")
    for i in range(8):
        a, b = i * math.tau / 8, (i + 1) * math.tau / 8
        p = (7.4 * math.cos(a), 86 + 7.4 * math.sin(a), -35.5)
        q = (7.4 * math.cos(b), 86 + 7.4 * math.sin(b), -35.5)
        material = "smoked_bronze" if builder.round >= 3 and i < 4 else "bronze"
        builder.shaft("organ_furnace", f"furnace_rim_{i}", p, q, 2.2, 3.6, material)
    for i, (x, y, size) in enumerate(((-3.6, 82.2, 2.2), (-.8, 82, 2.8), (2.2, 82.8, 2.3),
                                      (3.9, 85.3, 1.3), (-2.6, 85.1, 1.4))):
        builder.cube("organ_furnace", f"furnace_ember_{i}", (x, y, -36.2),
                     (x + size, y + size * .7, -35.7), "ember")
    for i, x in enumerate((-3.7, 0, 3.7)):
        material = "rust" if builder.round >= 3 and i != 1 else "iron"
        builder.shaft("organ_furnace", f"furnace_grate_{i}", (x, 80.5, -37.1),
                      (x + .4, 89.5 if i != 1 else 85.8, -37.1), .7, 1.0, material)
    material = "ash" if builder.round >= 3 else "iron"
    builder.cube("organ_furnace", "furnace_lower_lip", (-5.4, 77.7, -36.2), (5.4, 79.2, -31.4), material)


def part_fusion(builder: Decorations) -> None:
    builder.group("organ_fusion", (0, 86, -30))
    for i, (a, b, width) in enumerate((
        ((-9.1, 93, -30), (-7.3, 91.8, -36), 2.8),
        ((-10.6, 85.4, -27), (-8.2, 85, -35.2), 2.5),
        ((-8.5, 78.5, -27), (-6.7, 79.8, -35.8), 2.5),
        ((7.5, 93.2, -31), (5.1, 92.2, -35.7), 2.8),
        ((10.5, 87.5, -28), (8, 86.5, -35.7), 2.7),
        ((8.3, 78.8, -27), (6.4, 79.6, -34.9), 2.6),
    )):
        builder.shaft("organ_fusion", f"fusion_fold_{i}", a, b, width, 2, "scar")
    # 鳞片直接复用原体表的 UV，过渡颜色与原资产保持一致。
    scale_faces = copy.deepcopy(builder.base["elements"][76]["faces"])
    for i, (x, y, z, rot) in enumerate(((-8.8, 92.8, -33, -22), (7.7, 93, -33.4, 24),
                                       (-10.2, 84, -30.7, -18), (9.8, 82.5, -30.7, 18))):
        e = builder.cube("organ_fusion", f"fusion_scale_{i}", (x-1.6, y-2.4, z-1),
                         (x+1.6, y+2.4, z+1), "iron", (0, rot, rot / 2))
        e["faces"] = copy.deepcopy(scale_faces)


def part_human_arm(builder: Decorations, side: str) -> None:
    bent = side == "a"
    shoulder = (-13, 98, -25.5) if bent else (12, 93, -24)
    elbow = (-18, 87, -33) if bent else (17, 79, -30)
    wrist = (-9.9, 84.8, -39) if bent else (16, 67, -33)
    palm = (-8, 83.5, -39.5) if bent else (15.5, 64.7, -33.6)
    if bent and builder.round >= 2:
        elbow = (-19, 86.2, -42)
        wrist = (-11.5, 84.2, -44)
        palm = (-9.7, 82.9, -44.5)
    upper, forearm, hand = (f"human_{side}_{part}" for part in ("upper_arm", "forearm", "hand"))
    builder.group(upper, shoulder)
    builder.group(forearm, elbow, upper)
    builder.group(hand, wrist, forearm)
    builder.shaft(upper, f"human_{side}_upper_skin", shoulder, elbow, 3.7, 3.3, "skin")
    builder.shaft(forearm, f"human_{side}_forearm_skin", elbow, wrist, 2.6, 2.5, "skin")
    builder.cube(forearm, f"human_{side}_elbow", np.array(elbow)-1.5, np.array(elbow)+1.5, "skin")
    builder.shaft(hand, f"human_{side}_wrist", wrist, palm, 2.2, 2, "skin")
    px, py, pz = palm
    builder.cube(hand, f"human_{side}_palm", (px-1.85, py-1.8, pz-1), (px+1.85, py+1.3, pz+1), "skin")
    for i, (offset, length) in enumerate(((-1.35, 3), (-.45, 3.7), (.45, 3.5), (1.35, 2.6))):
        root = (px+offset, py-1.4, pz-.05)
        joint = (px+offset, py-1.4-length*.6, pz-.45)
        tip = (px+offset+.12, py-1.4-length, pz+.25)
        builder.shaft(hand, f"human_{side}_finger_{i}_proximal", root, joint, .65, .72, "skin")
        builder.shaft(hand, f"human_{side}_finger_{i}_distal", joint, tip, .57, .65, "skin")
    inward = 1 if bent else -1
    thumb_root = (px+inward*1.6, py+.1, pz-.2)
    thumb_joint = (px+inward*2.6, py-.9, pz-.6)
    thumb_tip = (px+inward*2.3, py-2, pz-.7)
    builder.shaft(hand, f"human_{side}_thumb_proximal", thumb_root, thumb_joint, .8, .85, "skin")
    builder.shaft(hand, f"human_{side}_thumb_distal", thumb_joint, thumb_tip, .7, .7, "skin")
    # 肩根的鳞肉套与胸廓同骨绑定，前臂保留独立铰链。
    sx, sy, sz = shoulder
    builder.shaft("organ_fusion", f"fusion_arm_{side}_root", (sx*.78, sy+2, sz+3),
                  (sx, sy-3.8, sz-2), 5.6, 4.3, "scar")
    for i, dy in enumerate((1, -2.5)):
        e = builder.cube("organ_fusion", f"fusion_arm_{side}_scale_{i}",
                         (sx-2.8, sy+dy-1.7, sz-3.2), (sx+2.8, sy+dy+1.7, sz-.4),
                         "iron", (15, 0, -18 if bent else 18))
        e["faces"] = copy.deepcopy(builder.base["elements"][76]["faces"])


def build(round_number=1) -> Decorations:
    builder = Decorations(load_base(), round_number)
    part_furnace(builder)
    part_fusion(builder)
    part_human_arm(builder, "a")
    part_human_arm(builder, "b")
    return builder


def write_model(builder: Decorations, destination: Path) -> None:
    texture_path = destination.with_suffix(".png")
    stream = io.BytesIO()
    builder.atlas.save(stream, format="PNG")
    texture_bytes = stream.getvalue()
    # 两个文件均先核对，避免发现手改贴图前已经写出模型。
    if destination.exists():
        old = json.loads(destination.read_text())
        # 与本次确定性产物不同即视为手改稿，禁止重跑覆盖。
        if old != builder.doc:
            raise FileExistsError(f"已有不同内容的作者稿，请换输出名：{destination}")
    if texture_path.exists() and texture_path.read_bytes() != texture_bytes:
        raise FileExistsError(f"已有不同内容的贴图，请换输出名：{texture_path}")
    destination.parent.mkdir(parents=True, exist_ok=True)
    if not destination.exists():
        destination.write_text(json.dumps(builder.doc, ensure_ascii=False, indent=2) + "\n")
    if not texture_path.exists():
        texture_path.write_bytes(texture_bytes)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--round", type=int, choices=(1, 2, 3), default=1)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    result = build(args.round)
    destination = args.out or MODELS / f"BaolongwangDecorated_round{args.round}.bbmodel"
    write_model(result, destination)
    print(f"{destination}: {len(result.doc['elements'])} cubes, {len(result.doc['groups'])} bones, {len(result.doc['animations'])} animations")
