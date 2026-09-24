#!/usr/bin/env python3
"""真元诡雷（zhenyuan_mine）Blockbench .bbmodel 生成器。Round 3/3 (PROMISE 担保轮)。

设定与背景：
- worldview §五·六 / docs/library/peoples/地师手记 / 绝地草木拾遗 / materials.toml:
  地师将真元封入环境方块或地脉节点制成诡雷。踩中瞬间穿腿爆发。
- 视觉参考体系：
  1. 概念设计图: modelScript/assets/refs/ref_zhenyuan_mine_concept.png
  2. 物品图标: modelScript/assets/refs/ref_zhenyuan_mine_icon.png
  3. MC 正交三视图: modelScript/assets/refs/ref_zhenyuan_mine_three_view.png
  4. MC 爆炸分解图: modelScript/assets/refs/ref_zhenyuan_mine_exploded.png
- 结构特征：
  1. Base Frame & Bed (埋地暗底承压板与泥石咬合层)
  2. 4 Reinforced Stone Plates (西北/东北/西南/东南 4 块碎裂玄武岩/暗银岩板，立体断裂台阶)
  3. 4 Ancient Brass Clasps (四边仿古铁箍/铜扣搭扣锁边)
  4. 4 Corner Bone Posts (四角锁灵骨钉/骨桩，带关节头与锁链铁环)
  5. Cursed Energy Core (中心真元凝聚环形阵眼 + 炽烈橙红爆发突刺 + 十字贯穿真元渠 + 幽蓝电弧)
  6. Inset Runes (嵌入石板表面的拓印符片)

用法：
    python3 modelScript/generators/gen_zhenyuan_mine.py
    python3 modelScript/generators/gen_zhenyuan_mine.py --preview-only
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import uuid
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
MODEL_DIR = Path(__file__).resolve().parents[1] / "models"
OUT_DIR = Path(__file__).resolve().parents[1] / "out"
BBMODEL_OUT = MODEL_DIR / "ZhenyuanMine.bbmodel"
PREVIEW_OUT = OUT_DIR / "zhenyuan_mine_preview.png"

PX = 16.0
RES = 64

# 骨骼分类 (对应爆炸图分解模块)
BONE_ORDER = ["base_frame", "stone_plates", "energy_core", "bone_posts", "metal_clasps"]
BONE_PIVOTS = {name: [0.0, 0.0, 0.0] for name in BONE_ORDER}
BONE_COLORS = {
    "base_frame": (35, 33, 32),
    "stone_plates": (75, 80, 86),
    "energy_core": (255, 135, 20),
    "bone_posts": (218, 204, 178),
    "metal_clasps": (148, 115, 62),
}

# 材质划分区域 (64x64)
MAT_ZONE = {
    "stone": (0, 0, RES, 24),        # 暗银岩/玄武岩石板
    "fissure": (0, 24, RES, 32),     # 裂隙幽暗深底
    "energy_hot": (0, 32, RES, 42),  # 炽烈橙红/金白爆发核心与十字真元槽
    "energy_cold": (0, 42, RES, 50), # 幽蓝电弧真元流光
    "bone": (0, 50, RES, 58),        # 锁灵骨柱/骨头关节
    "brass_iron": (0, 58, RES, RES), # 仿古铜箍/蚀铁锁链与扣具
}


def part_base():
    """1. 底部承压框与裂隙暗底 (Base Frame & Fissure Bed)"""
    return [
        # 主底座暗底板
        ("base_frame", "fissure", "base_slab", [-6.6, 0.0, -6.6], [6.6, 0.6, 6.6]),
        # 边缘四边埋地突起土石咬合层
        ("base_frame", "stone", "bed_edge_n", [-4.0, 0.2, -7.0], [4.0, 0.8, -6.4]),
        ("base_frame", "stone", "bed_edge_s", [-4.0, 0.2, 6.4], [4.0, 0.8, 7.0]),
        ("base_frame", "stone", "bed_edge_w", [-7.0, 0.2, -4.0], [-6.4, 0.8, 4.0]),
        ("base_frame", "stone", "bed_edge_e", [6.4, 0.2, -4.0], [7.0, 0.8, 4.0]),
    ]


def part_plates():
    """2. 破碎岩板组 (4 Reinforced Stone Plates & Fragments)
    四大主碎块 + 边缘阶梯断崖 + 嵌刻符片
    """
    return [
        # ── 西北板块 (NW) ──
        ("stone_plates", "stone", "plate_nw_main", [-6.2, 0.6, -6.2], [-1.0, 2.2, -1.0]),
        ("stone_plates", "stone", "plate_nw_step", [-5.8, 2.2, -5.8], [-1.6, 2.6, -1.6]),
        ("stone_plates", "stone", "plate_nw_rune_inset", [-4.4, 2.6, -4.4], [-2.8, 2.8, -2.8]),

        # ── 东北板块 (NE) ──
        ("stone_plates", "stone", "plate_ne_main", [1.0, 0.6, -6.2], [6.2, 2.2, -1.0]),
        ("stone_plates", "stone", "plate_ne_step", [1.6, 2.2, -5.8], [5.8, 2.6, -1.6]),
        ("stone_plates", "stone", "plate_ne_rune_inset", [2.8, 2.6, -4.4], [4.4, 2.8, -2.8]),

        # ── 西南板块 (SW) ──
        ("stone_plates", "stone", "plate_sw_main", [-6.2, 0.6, 1.0], [-1.0, 2.2, 6.2]),
        ("stone_plates", "stone", "plate_sw_step", [-5.8, 2.2, 1.6], [-1.6, 2.6, 5.8]),
        ("stone_plates", "stone", "plate_sw_rune_inset", [-4.4, 2.6, 2.8], [-2.8, 2.8, 4.4]),

        # ── 东南板块 (SE，受真元顶起最高) ──
        ("stone_plates", "stone", "plate_se_main", [1.0, 0.6, 1.0], [6.2, 2.4, 6.2]),
        ("stone_plates", "stone", "plate_se_step", [1.6, 2.4, 1.6], [5.8, 2.8, 5.8]),
        ("stone_plates", "stone", "plate_se_rune_inset", [2.8, 2.8, 2.8], [4.4, 3.0, 4.4]),

        # ── 边缘崩角碎块 (4 Chips) ──
        ("stone_plates", "stone", "chip_nw", [-6.5, 0.6, -2.5], [-6.0, 1.6, -1.0]),
        ("stone_plates", "stone", "chip_ne", [1.0, 0.6, -6.5], [2.5, 1.6, -6.0]),
        ("stone_plates", "stone", "chip_se", [6.0, 0.6, 1.0], [6.5, 1.6, 2.5]),
        ("stone_plates", "stone", "chip_sw", [-2.5, 0.6, 6.0], [-1.0, 1.6, 6.5]),
    ]


def part_core():
    """3. 真元能量核心与十字贯穿裂缝 (Cursed Energy Circuit & Core)"""
    return [
        # 中心真元阵眼环圈 (Disc ring)
        ("energy_core", "brass_iron", "core_disc_ring", [-1.6, 0.8, -1.6], [1.6, 2.2, 1.6]),
        # 中心炽烈爆发尖核
        ("energy_core", "energy_hot", "core_burst_center", [-1.1, 1.2, -1.1], [1.1, 2.6, 1.1]),
        ("energy_core", "energy_hot", "core_spike_mid", [-0.6, 2.6, -0.6], [0.6, 3.6, 0.6]),
        ("energy_core", "energy_hot", "core_spike_tip", [-0.25, 3.6, -0.25], [0.25, 4.3, 0.25]),

        # 贯穿十字炽烈真元渠 (连接中心至四边)
        ("energy_core", "energy_hot", "channel_hot_n", [-0.5, 0.8, -6.2], [0.5, 1.8, -1.2]),
        ("energy_core", "energy_hot", "channel_hot_s", [-0.5, 0.8, 1.2], [0.5, 1.8, 6.2]),
        ("energy_core", "energy_hot", "channel_hot_w", [-6.2, 0.8, -0.5], [-1.2, 1.8, 0.5]),
        ("energy_core", "energy_hot", "channel_hot_e", [1.2, 0.8, -0.5], [6.2, 1.8, 0.5]),

        # 对角线分支幽蓝电弧真元纹 (Arc channels)
        ("energy_core", "energy_cold", "arc_nw", [-4.6, 0.8, -4.6], [-2.6, 1.4, -2.6]),
        ("energy_core", "energy_cold", "arc_ne", [2.6, 0.8, -4.6], [4.6, 1.4, -2.6]),
        ("energy_core", "energy_cold", "arc_sw", [-4.6, 0.8, 2.6], [-2.6, 1.4, 4.6]),
        ("energy_core", "energy_cold", "arc_se", [2.6, 0.8, 2.6], [4.6, 1.4, 4.6]),
    ]


def part_bone_posts():
    """4. 四角锁灵骨桩 (4 Corner Bone Posts)
    长骨桩身 + 顶端粗壮关节头 + 绑扎铁链/铁环
    """
    cubes = []
    corners = [
        ("nw", -5.8, -5.8),
        ("ne", 4.6, -5.8),
        ("sw", -5.8, 4.6),
        ("se", 4.6, 4.6),
    ]
    for tag, x0, z0 in corners:
        x1, z1 = x0 + 1.2, z0 + 1.2
        # 骨干立柱
        cubes.append(("bone_posts", "bone", f"bone_post_shaft_{tag}", [x0, 0.4, z0], [x1, 3.8, z1]))
        # 顶端骨节头 (关节凸起)
        cubes.append(("bone_posts", "bone", f"bone_post_cap_{tag}", [x0 - 0.25, 3.8, z0 - 0.25], [x1 + 0.25, 4.6, z1 + 0.25]))
        # 骨柱中段铁链/锁扣
        cubes.append(("bone_posts", "brass_iron", f"bone_post_chain_{tag}", [x0 - 0.15, 1.6, z0 - 0.15], [x1 + 0.15, 2.2, z1 + 0.15]))
    return cubes


def part_metal_clasps():
    """5. 四边仿古铜箍/搭扣锁具 (4 Metal Straps / Clasps)
    包裹在四边十字出槽处的防裂锁扣
    """
    return [
        # 北边锁扣
        ("metal_clasps", "brass_iron", "clasp_n", [-1.1, 0.5, -6.7], [1.1, 2.3, -5.9]),
        ("metal_clasps", "brass_iron", "clasp_n_pin", [-0.4, 0.8, -6.9], [0.4, 2.0, -6.6]),

        # 南边锁扣
        ("metal_clasps", "brass_iron", "clasp_s", [-1.1, 0.5, 5.9], [1.1, 2.3, 6.7]),
        ("metal_clasps", "brass_iron", "clasp_s_pin", [-0.4, 0.8, 6.6], [0.4, 2.0, 6.9]),

        # 西边锁扣
        ("metal_clasps", "brass_iron", "clasp_w", [-6.7, 0.5, -1.1], [-5.9, 2.3, 1.1]),
        ("metal_clasps", "brass_iron", "clasp_w_pin", [-6.9, 0.8, -0.4], [-6.6, 2.0, 0.4]),

        # 东边锁扣
        ("metal_clasps", "brass_iron", "clasp_e", [5.9, 0.5, -1.1], [6.7, 2.3, 1.1]),
        ("metal_clasps", "brass_iron", "clasp_e_pin", [6.6, 0.8, -0.4], [6.9, 2.0, 0.4]),
    ]


def all_cubes():
    return part_base() + part_plates() + part_core() + part_bone_posts() + part_metal_clasps()


def make_texture(res=RES, seed=103):
    """高质量末法残土风格材质贴图：
    - 暗银岩/玄武岩 (0..24): 深黑冷灰、银白裂纹高光
    - 裂隙阴影 (24..32): 极深炭灰
    - 炽热真元 (32..42): 金白(255,248,190) -> 橙金(255,135,25) -> 赤褐(170,30,10)
    - 幽蓝电纹 (42..50): 荧光青蓝(100,225,255) -> 湛蓝电火花(220,250,255)
    - 锁灵骨柱 (50..58): 泛黄陈年骨白(215,202,176)、骨裂暗纹
    - 仿古青铜/蚀铁 (58..64): 黄铜暗金(150,118,65)、铜绿与铁锈
    """
    rng = np.random.default_rng(seed)
    y, x = np.mgrid[0:res, 0:res]
    img = np.zeros((res, res, 4), np.uint8)
    img[..., 3] = 255

    # 1. 暗银岩/玄武岩 (y: 0..24)
    stone_m = y < 24
    base_stone = np.array([45, 48, 54], float)[None, None, :]
    noise = (rng.random((res, res, 1)) - 0.5) * 18
    scol = base_stone + noise
    silver = rng.random((res, res, 1)) > 0.92
    scol += silver * 48
    cracks = ((x * 3 + y * 2) % 13 == 0) | ((x * 2 - y * 3) % 17 == 0)
    scol -= cracks[..., None] * 22
    img[stone_m, :3] = np.clip(scol, 16, 180)[stone_m].astype(np.uint8)

    # 2. 裂隙深底 (y: 24..32)
    fissure_m = (y >= 24) & (y < 32)
    fcol = np.array([16, 15, 18], float)[None, None, :] + (rng.random((res, res, 1)) - 0.5) * 8
    img[fissure_m, :3] = np.clip(fcol, 8, 35)[fissure_m].astype(np.uint8)

    # 3. 炽烈真元能量 (y: 32..42)
    hot_m = (y >= 32) & (y < 42)
    orange_gold = np.array([255, 135, 25], float)
    hot_white = np.array([255, 248, 190], float)
    deep_crimson = np.array([170, 30, 10], float)
    
    wave = 0.5 + 0.5 * np.sin(x * 0.75 + y * 1.1)
    hcol = orange_gold[None, None, :] + (wave[..., None] - 0.5) * 45
    core_flares = rng.random((res, res, 1)) > 0.84
    hcol = np.where(core_flares, hot_white[None, None, :], hcol)
    edges = (y == 32) | (y == 41) | (x % 6 == 0)
    hcol = np.where(edges[..., None], deep_crimson[None, None, :], hcol)
    img[hot_m, :3] = np.clip(hcol, 15, 255)[hot_m].astype(np.uint8)

    # 4. 幽蓝电弧 (y: 42..50)
    cold_m = (y >= 42) & (y < 50)
    blue_deep = np.array([20, 80, 170], float)
    cyan_bright = np.array([100, 225, 255], float)
    white_spark = np.array([220, 250, 255], float)
    
    arc_wave = 0.5 + 0.5 * np.sin(x * 1.3 - y * 0.9)
    ccol = blue_deep[None, None, :] + arc_wave[..., None] * (cyan_bright - blue_deep)
    sparks = rng.random((res, res, 1)) > 0.88
    ccol = np.where(sparks, white_spark[None, None, :], ccol)
    img[cold_m, :3] = np.clip(ccol, 15, 255)[cold_m].astype(np.uint8)

    # 5. 锁灵骨柱 (y: 50..58)
    bone_m = (y >= 50) & (y < 58)
    bone_base = np.array([215, 202, 176], float)[None, None, :]
    bcol = bone_base + (rng.random((res, res, 1)) - 0.5) * 16
    bone_cracks = rng.random((res, res, 1)) > 0.90
    bcol -= bone_cracks * 35
    img[bone_m, :3] = np.clip(bcol, 25, 240)[bone_m].astype(np.uint8)

    # 6. 仿古青铜/蚀铁扣 (y: 58..64)
    brass_m = y >= 58
    brass_base = np.array([150, 118, 65], float)[None, None, :]
    brass_col = brass_base + (rng.random((res, res, 1)) - 0.5) * 18
    # 铜绿 / 铁锈
    patina = rng.random((res, res, 1)) > 0.85
    patina_col = np.array([55, 125, 95], float)
    brass_col = np.where(patina, patina_col[None, None, :], brass_col)
    img[brass_m, :3] = np.clip(brass_col, 20, 220)[brass_m].astype(np.uint8)

    return Image.fromarray(img, "RGBA")


def png_data_url(img: Image.Image) -> str:
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


class Packer:
    def __init__(self, x0, y0, x1, y1):
        self.x0, self.y0, self.x1, self.y1 = x0, y0, x1, y1
        self.x, self.y, self.rowh = x0, y0, 0.0

    def place(self, w, h):
        w = min(w, self.x1 - self.x0)
        h = min(h, self.y1 - self.y0)
        if self.x + w > self.x1:
            self.x = self.x0
            self.y += self.rowh
            self.rowh = 0.0
        if self.y + h > self.y1:
            self.y = self.y0
        ox, oy = self.x, self.y
        self.x += w
        self.rowh = max(self.rowh, h)
        return ox, oy


def cube_faces_uv(frm, to, packer):
    dx, dy, dz = to[0] - frm[0], to[1] - frm[1], to[2] - frm[2]
    dims = {
        "north": (dx, dy), "south": (dx, dy),
        "east": (dz, dy), "west": (dz, dy),
        "up": (dx, dz), "down": (dx, dz)
    }
    faces = {}
    for name, (w, h) in dims.items():
        ox, oy = packer.place(abs(w), abs(h))
        faces[name] = {
            "uv": [round(ox, 2), round(oy, 2), round(ox + abs(w), 2), round(oy + abs(h), 2)],
            "texture": 0
        }
    return faces


def build_bbmodel():
    cubes = all_cubes()
    packers = {name: Packer(*zone) for name, zone in MAT_ZONE.items()}
    elements = []
    bone_children = {bone: [] for bone in BONE_ORDER}

    for bone, material, name, frm, to in cubes:
        euid = str(uuid.uuid4())
        elements.append({
            "name": name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "render_order": "default",
            "allow_mirror_modeling": True,
            "type": "cube",
            "uuid": euid,
            "from": [round(v, 3) for v in frm],
            "to": [round(v, 3) for v in to],
            "autouv": 0,
            "color": BONE_ORDER.index(bone),
            "origin": list(BONE_PIVOTS[bone]),
            "faces": cube_faces_uv(frm, to, packers[material]),
        })
        bone_children[bone].append(euid)

    outliner = []
    for bone in BONE_ORDER:
        outliner.append({
            "name": bone,
            "origin": list(BONE_PIVOTS[bone]),
            "color": BONE_ORDER.index(bone),
            "uuid": str(uuid.uuid4()),
            "export": True,
            "mirror_uv": False,
            "isOpen": True,
            "locked": False,
            "visibility": True,
            "autouv": 0,
            "children": bone_children[bone],
        })

    tex = make_texture()
    model = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "zhenyuan_mine",
        "model_identifier": "geometry.bong.zhenyuan_mine",
        "visible_box": [1, 0.5, 1],
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [{
            "path": "",
            "name": "zhenyuan_mine.png",
            "folder": "entity",
            "namespace": "bong",
            "id": "0",
            "width": RES,
            "height": RES,
            "uv_width": RES,
            "uv_height": RES,
            "particle": False,
            "render_mode": "default",
            "visible": True,
            "mode": "bitmap",
            "saved": False,
            "uuid": str(uuid.uuid4()),
            "source": png_data_url(tex),
        }],
    }
    return model, cubes, tex


def render_preview(cubes, tex, out=PREVIEW_OUT):
    out.parent.mkdir(parents=True, exist_ok=True)
    scale, pad, gap = 14, 20, 24

    def lit(color, k):
        return tuple(int(np.clip(c * k, 0, 255)) for c in color)

    all_from = [c[3] for c in cubes]
    all_to = [c[4] for c in cubes]
    min_x = min(f[0] for f in all_from)
    max_x = max(t[0] for t in all_to)
    min_y = min(f[1] for f in all_from)
    max_y = max(t[1] for t in all_to)
    min_z = min(f[2] for f in all_from)
    max_z = max(t[2] for t in all_to)

    w_px = int(math.ceil((max_x - min_x) * scale))
    h_px = int(math.ceil((max_y - min_y) * scale))
    d_px = int(math.ceil((max_z - min_z) * scale))

    iso_w = int((w_px + d_px) * 0.9 + 20)
    iso_h = int((w_px + d_px) * 0.5 + h_px + 20)

    total_w = pad * 2 + w_px + gap + d_px + gap + w_px + gap + iso_w
    total_h = pad * 2 + max(h_px, d_px, iso_h) + 26
    canvas = Image.new("RGBA", (total_w, total_h), (24, 26, 30, 255))
    draw = ImageDraw.Draw(canvas)

    # 1. Front (XY)
    ox, oy = pad, pad + 20 + max(0, (total_h - pad * 2 - 26 - h_px) // 2)
    draw.text((ox, pad), "FRONT (X/Y)", fill=(200, 200, 200, 255))
    for bone, mat, name, frm, to in cubes:
        x0 = ox + int((frm[0] - min_x) * scale)
        x1 = ox + int((to[0] - min_x) * scale)
        y0 = oy + int((max_y - to[1]) * scale)
        y1 = oy + int((max_y - frm[1]) * scale)
        col = lit(BONE_COLORS[bone], 1.0)
        draw.rectangle([x0, y0, x1, y1], fill=col, outline=(15, 15, 15, 255))

    # 2. Side (ZY)
    ox += w_px + gap
    draw.text((ox, pad), "SIDE (Z/Y)", fill=(200, 200, 200, 255))
    for bone, mat, name, frm, to in cubes:
        x0 = ox + int((frm[2] - min_z) * scale)
        x1 = ox + int((to[2] - min_z) * scale)
        y0 = oy + int((max_y - to[1]) * scale)
        y1 = oy + int((max_y - frm[1]) * scale)
        col = lit(BONE_COLORS[bone], 0.85)
        draw.rectangle([x0, y0, x1, y1], fill=col, outline=(15, 15, 15, 255))

    # 3. Top (XZ)
    ox += d_px + gap
    oy_top = pad + 20 + max(0, (total_h - pad * 2 - 26 - d_px) // 2)
    draw.text((ox, pad), "TOP (X/Z)", fill=(200, 200, 200, 255))
    for bone, mat, name, frm, to in cubes:
        x0 = ox + int((frm[0] - min_x) * scale)
        x1 = ox + int((to[0] - min_x) * scale)
        y0 = oy_top + int((to[2] - min_z) * scale)
        y1 = oy_top + int((frm[2] - min_z) * scale)
        y0, y1 = min(y0, y1), max(y0, y1)
        col = lit(BONE_COLORS[bone], 1.15)
        draw.rectangle([x0, y0, x1, y1], fill=col, outline=(15, 15, 15, 255))

    # 4. Isometric 3D View
    ox += w_px + gap
    oy_iso = oy_top + iso_h // 2
    draw.text((ox, pad), "ISOMETRIC (3D)", fill=(255, 200, 100, 255))
    
    sorted_cubes = sorted(cubes, key=lambda c: (c[3][0] + c[3][2] - c[3][1]))
    for bone, mat, name, frm, to in sorted_cubes:
        col_top = lit(BONE_COLORS[bone], 1.25)
        p_top_s = (ox + iso_w // 2 + int((to[0] - frm[2]) * scale * 0.7), oy_iso + int((to[0] + frm[2]) * scale * 0.35) - int(to[1] * scale * 0.8))
        p_top_e = (ox + iso_w // 2 + int((to[0] - to[2]) * scale * 0.7), oy_iso + int((to[0] + to[2]) * scale * 0.35) - int(to[1] * scale * 0.8))
        p_top_n = (ox + iso_w // 2 + int((frm[0] - to[2]) * scale * 0.7), oy_iso + int((frm[0] + to[2]) * scale * 0.35) - int(to[1] * scale * 0.8))
        p_top_w = (ox + iso_w // 2 + int((frm[0] - frm[2]) * scale * 0.7), oy_iso + int((frm[0] + frm[2]) * scale * 0.35) - int(to[1] * scale * 0.8))
        draw.polygon([p_top_w, p_top_s, p_top_e, p_top_n], fill=col_top, outline=(20, 20, 20, 255))

    canvas.save(out)
    return out


def main():
    parser = argparse.ArgumentParser(description="真元诡雷 .bbmodel 生成器")
    parser.add_argument("--preview-only", action="store_true", help="仅渲染预览图")
    parser.add_argument("--out", type=Path, default=BBMODEL_OUT, help="输出 .bbmodel 路径")
    parser.add_argument("--preview-out", type=Path, default=PREVIEW_OUT, help="输出预览 PNG 路径")
    args = parser.parse_args()

    model, cubes, tex = build_bbmodel()

    if not args.preview_only:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        with open(args.out, "w", encoding="utf-8") as f:
            json.dump(model, f, indent=2)
        print(f"→ bbmodel: {args.out} ({args.out.stat().st_size} B)")

    render_out = render_preview(cubes, tex, out=args.preview_out)
    print(f"→ preview: {render_out}")

    # 调用底座的标准 bbmodel 渲染器做软渲染三视图
    try:
        from bbmodel_maker.render.render_bbmodel import render_three_view
        im, _ = render_three_view(args.out)
        three_view_out = OUT_DIR / "render_ZhenyuanMine_three_view.png"
        im.save(three_view_out)
        print(f"→ three-view: {three_view_out}")
    except Exception as e:
        print(f"[WARN] 软渲染三视图跳过/失败: {e}")


if __name__ == "__main__":
    main()
