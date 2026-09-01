#!/usr/bin/env python3
"""凡铁采药刀（herb_knife_iron / 采药小折刀）Blockbench .bbmodel 生成器。Round 3/3 (PROMISE 担保轮)。

设计理念与世界观：
- worldview & materials.toml:
  凡铁打造的传统中式采药小折刀。刀刃呈微弯的鹰嘴/草药镰形（便于勾割灵草茎叶、剔剥根须），
  通过黄铜/凡铁转轴铆接在一截质地坚硬的深色老木折叠刀柄上。
- 刀尾带有防脱手的粗麻绳/皮绳尾绳结。
- 折刀展开呈经典的采药微弯握姿，整体质朴厚重、暗哑耐用，充满末法残土采集者的风霜痕迹。

结构组成（5 大 Bone Group）：
    1. pivot       - 黄铜/凡铁旋转转轴（外凸转轴帽与垫圈铆钉）
    2. handle      - 深色硬木雕花折叠刀柄（两侧护木、夹层内槽、凹槽与防滑抓握倒角）
    3. blade_spine - 凡铁厚实刀背与折刀卡笋转轴根部
    4. blade_edge  - 弧形锋刃（带有鹰嘴内弧与开刃斜面，草药汁与磨损痕迹）
    5. tail_cord   - 刀尾穿绳孔与垂落的防滑皮绳结

贴图规范（64x64 贴图）：
    - weathered_iron (0, 0, 32, 32): 凡铁刀身、暗哑斑驳、铁锈、磨损刃线
    - dark_wood (32, 0, 64, 32): 深褐老木柄、天然风干木纹、握痕暗光
    - brass_rivet (0, 32, 32, 64): 黄铜转轴垫片、铆钉五金、做旧铜色
    - hemp_cord (32, 32, 64, 64): 麻绳/皮绳尾绳、绳结、纤维暗调

用法：
    python3 modelScript/generators/gen_herb_knife_iron.py
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
BBMODEL_OUT = MODEL_DIR / "HerbKnifeIron.bbmodel"
PREVIEW_OUT = OUT_DIR / "herb_knife_iron_preview.png"

PX = 16.0
RES = 64

# 骨骼分类
BONE_ORDER = ["pivot", "handle", "blade_spine", "blade_edge", "tail_cord"]
BONE_PIVOTS = {name: [8.0, 8.0, 8.0] for name in BONE_ORDER}
BONE_COLORS = {
    "pivot": (180, 150, 70),       # 黄铜转轴
    "handle": (90, 65, 45),        # 老木刀柄
    "blade_spine": (85, 90, 95),   # 凡铁刀背
    "blade_edge": (160, 165, 170), # 磨损开刃
    "tail_cord": (140, 105, 60),   # 尾绳
}

# 材质划分区域 (64x64)
MAT_ZONE = {
    "weathered_iron": (0, 0, 32, 32),
    "blade_edge": (0, 0, 32, 32),
    "dark_wood": (32, 0, 64, 32),
    "brass_rivet": (0, 32, 32, 64),
    "hemp_cord": (32, 32, 64, 64),
}


def _assert_no_coplanar_faces(cubes: list[tuple]):
    """校验同一方向的面没有完全共面重叠，防止 Z-Fighting。"""
    faces = {"+x": [], "-x": [], "+y": [], "-y": [], "+z": [], "-z": []}
    for bone, mat, name, from_xyz, to_xyz in cubes:
        x0, y0, z0 = from_xyz
        x1, y1, z1 = to_xyz
        # 记录 6 个外表面的 (coord, min_u, max_u, min_v, max_v, name)
        faces["-x"].append((round(x0, 4), round(y0, 4), round(y1, 4), round(z0, 4), round(z1, 4), name))
        faces["+x"].append((round(x1, 4), round(y0, 4), round(y1, 4), round(z0, 4), round(z1, 4), name))
        faces["-y"].append((round(y0, 4), round(x0, 4), round(x1, 4), round(z0, 4), round(z1, 4), name))
        faces["+y"].append((round(y1, 4), round(x0, 4), round(x1, 4), round(z0, 4), round(z1, 4), name))
        faces["-z"].append((round(z0, 4), round(x0, 4), round(x1, 4), round(y0, 4), round(y1, 4), name))
        faces["+z"].append((round(z1, 4), round(x0, 4), round(x1, 4), round(y0, 4), round(y1, 4), name))

    for dir_name, face_list in faces.items():
        for i in range(len(face_list)):
            c1, u0_1, u1_1, v0_1, v1_1, n1 = face_list[i]
            for j in range(i + 1, len(face_list)):
                c2, u0_2, u1_2, v0_2, v1_2, n2 = face_list[j]
                if abs(c1 - c2) < 1e-4:
                    # 检查重叠区间
                    overlap_u = max(0.0, min(u1_1, u1_2) - max(u0_1, u0_2))
                    overlap_v = max(0.0, min(v1_1, v1_2) - max(v0_1, v0_2))
                    if overlap_u > 0.05 and overlap_v > 0.05:
                        raise ValueError(f"Z-Fighting 共面检测失败: {n1} 与 {n2} 在方向 {dir_name} 重叠 (coord={c1})")


def part_pivot():
    """1. 黄铜转轴 (Brass Pivot & Washer Pin)
    折刀顶部的枢轴，X 轴贯穿刀柄两侧，外侧有带倒角的厚垫片与中心铆钉头。
    基准枢轴中心设在 (8.0, 12.0, 8.0)。
    """
    return [
        # 左侧黄铜主垫圈 (凸出于左木柄)
        ("pivot", "brass_rivet", "pivot_washer_left", [6.95, 11.2, 7.2], [7.28, 12.8, 8.8]),
        # 左侧中心铆钉凸起
        ("pivot", "brass_rivet", "pivot_pin_left", [6.75, 11.6, 7.6], [6.95, 12.4, 8.4]),
        # 内部贯穿转轴钢芯 (略微内缩，避免共面)
        ("pivot", "weathered_iron", "pivot_internal_axle", [7.35, 11.52, 7.52], [8.65, 12.48, 8.48]),
        # 右侧黄铜主垫圈 (凸出于右木柄)
        ("pivot", "brass_rivet", "pivot_washer_right", [8.72, 11.2, 7.2], [9.05, 12.8, 8.8]),
        # 右侧中心铆钉凸起
        ("pivot", "brass_rivet", "pivot_pin_right", [9.05, 11.6, 7.6], [9.25, 12.4, 8.4]),
    ]


def part_handle():
    """2. 老木折叠刀柄 (Carved Wooden Handle)
    刀柄自枢轴顶部 y=12.5 延伸至刀尾 y=4.5 (长约 8px)。
    呈经典的折刀槽状结构：左木侧板、右木侧板、背部止刀铁衬/垫条、刀尾穿孔与铜箍。
    中心在 (8.0, y, 8.0)。
    """
    return [
        # ── 顶部弧形包边 ──
        ("handle", "dark_wood", "handle_top_crown_left", [7.30, 12.5, 7.3], [7.78, 13.2, 8.7]),
        ("handle", "dark_wood", "handle_top_crown_right", [8.22, 12.5, 7.3], [8.70, 13.2, 8.7]),
        # ── 左侧木柄护板 (分三段微弧过渡) ──
        ("handle", "dark_wood", "handle_left_upper", [7.28, 9.5, 7.2], [7.82, 12.5, 8.8]),
        ("handle", "dark_wood", "handle_left_mid", [7.24, 6.5, 7.15], [7.82, 9.5, 8.85]),
        ("handle", "dark_wood", "handle_left_lower", [7.28, 4.2, 7.25], [7.82, 6.5, 8.75]),
        # ── 右侧木柄护板 (分三段微弧过渡) ──
        ("handle", "dark_wood", "handle_right_upper", [8.18, 9.5, 7.2], [8.72, 12.5, 8.8]),
        ("handle", "dark_wood", "handle_right_mid", [8.18, 6.5, 7.15], [8.76, 9.5, 8.85]),
        ("handle", "dark_wood", "handle_right_lower", [8.18, 4.2, 7.25], [8.72, 6.5, 8.75]),
        # ── 刀柄背部内衬与止刀垫块 (位于 +Z 面) ──
        ("handle", "weathered_iron", "handle_back_spring_top", [7.82, 10.2, 8.42], [8.18, 12.6, 8.82]),
        ("handle", "weathered_iron", "handle_back_spring_mid", [7.82, 6.8, 8.45], [8.18, 10.2, 8.85]),
        ("handle", "weathered_iron", "handle_back_spring_bot", [7.82, 4.2, 8.35], [8.18, 6.8, 8.75]),
        # ── 刀柄前腹内槽垫条 (刀腹 -Z 面开口，仅底部有封口衬块) ──
        ("handle", "weathered_iron", "handle_front_bottom_spacer", [7.82, 4.2, 7.25], [8.18, 5.2, 7.65]),
        # ── 柄部加固铜铆钉 (两枚位于握把中下部) ──
        ("handle", "brass_rivet", "handle_rivet_mid_left", [7.18, 7.8, 7.8], [7.24, 8.4, 8.4]),
        ("handle", "brass_rivet", "handle_rivet_mid_right", [8.76, 7.8, 7.8], [8.82, 8.4, 8.4]),
        ("handle", "brass_rivet", "handle_rivet_bot_left", [7.18, 5.2, 7.7], [7.24, 5.8, 8.3]),
        ("handle", "brass_rivet", "handle_rivet_bot_right", [8.76, 5.2, 7.7], [8.82, 5.8, 8.3]),
        # ── 尾部黄铜穿绳衬圈 ──
        ("handle", "brass_rivet", "handle_tail_ferrule_left", [7.20, 3.8, 7.4], [7.28, 4.2, 8.6]),
        ("handle", "brass_rivet", "handle_tail_ferrule_right", [8.72, 3.8, 7.4], [8.80, 4.2, 8.6]),
    ]


def part_blade():
    """3 & 4. 凡铁折刀刀刃 (Sickle Blade: Spine & Edge)
    刀刃自枢轴 (8.0, 12.0, 8.0) 展开，向 -Z 方向伸展并呈优雅的向下弯曲鹰嘴草药镰弧度。
    刀身分为凡铁厚背 (blade_spine) 与薄刃开锋斜面 (blade_edge)。
    折刀展开状态：刃部向 -Z 和 -Y 延伸，刀尖落在约 (8.0, 6.2, 4.2)。
    """
    cubes = []

    # ── 刀根转轴衔接座 (Pivot Tang) ──
    cubes.append(("blade_spine", "weathered_iron", "blade_tang_hub", [7.68, 11.2, 7.22], [8.32, 12.6, 8.38]))
    cubes.append(("blade_spine", "weathered_iron", "blade_tang_neck", [7.70, 10.4, 6.72], [8.30, 11.8, 7.48]))

    # ── 刀背厚实主体 (Blade Spine: 4段弧形折弯向下) ──
    # 段 1: 近根部延伸 (y: 9.6~11.0, z: 6.04~7.02)
    cubes.append(("blade_spine", "weathered_iron", "blade_spine_1", [7.72, 9.6, 6.04], [8.28, 11.0, 7.02]))
    # 段 2: 中段前倾 (y: 8.2~9.8, z: 5.24~6.24)
    cubes.append(("blade_spine", "weathered_iron", "blade_spine_2", [7.74, 8.2, 5.24], [8.26, 9.8, 6.24]))
    # 段 3: 下弯鹰嘴前段 (y: 6.8~8.4, z: 4.64~5.44)
    cubes.append(("blade_spine", "weathered_iron", "blade_spine_3", [7.75, 6.8, 4.64], [8.25, 8.4, 5.44]))
    # 段 4: 刀背收尖 (y: 5.6~7.0, z: 4.24~4.84)
    cubes.append(("blade_spine", "weathered_iron", "blade_spine_4", [7.76, 5.6, 4.24], [8.24, 7.0, 4.84]))

    # ── 刀刃内弧与锋刃斜面 (Blade Edge: 极薄厚度，内凹割草药弧) ──
    # 刃段 1: 根部微刃
    cubes.append(("blade_edge", "blade_edge", "blade_edge_1", [7.88, 9.4, 5.42], [8.12, 10.8, 6.16]))
    # 刃段 2: 主切割弧 (中部草药割取内弧)
    cubes.append(("blade_edge", "blade_edge", "blade_edge_2", [7.89, 7.8, 4.58], [8.11, 9.5, 5.36]))
    # 刃段 3: 鹰嘴尖前内弧
    cubes.append(("blade_edge", "blade_edge", "blade_edge_3", [7.90, 6.4, 3.98], [8.10, 8.0, 4.74]))
    # 刃段 4: 极细鹰嘴尖刺 (Sickle Hook Tip)
    cubes.append(("blade_edge", "blade_edge", "blade_edge_hook_tip", [7.91, 5.2, 3.78], [8.09, 6.6, 4.22]))

    # ── 刀面锻打起棱脊线 (Central Bevel Ridge) ──
    cubes.append(("blade_spine", "weathered_iron", "blade_bevel_ridge_upper", [7.78, 8.8, 5.46], [8.22, 10.4, 5.92]))
    cubes.append(("blade_spine", "weathered_iron", "blade_bevel_ridge_lower", [7.80, 7.2, 4.76], [8.20, 8.8, 5.22]))

    return cubes


def part_tail_cord():
    """5. 尾部防滑皮绳结 (Hemp/Leather Tail Cord & Tassel Knot)
    穿出刀柄尾端 (y=3.8)，下垂形成一束双股绳结与末端流苏绳头。
    中心位于 (8.0, y, 8.0)。
    """
    return [
        # 尾部穿孔引出绳圈
        ("tail_cord", "hemp_cord", "cord_loop_collar", [7.65, 3.2, 7.5], [8.35, 3.8, 8.3]),
        # 主绳身下垂双股
        ("tail_cord", "hemp_cord", "cord_strand_left", [7.55, 1.8, 7.6], [7.95, 3.2, 8.2]),
        ("tail_cord", "hemp_cord", "cord_strand_right", [8.05, 1.8, 7.7], [8.45, 3.2, 8.3]),
        # 粗大绳结 (Double Knotted Ball)
        ("tail_cord", "hemp_cord", "cord_knot_main", [7.45, 0.8, 7.4], [8.55, 1.8, 8.5]),
        # 绳结下散开的短绳须
        ("tail_cord", "hemp_cord", "cord_fringe_1", [7.60, 0.0, 7.5], [7.95, 0.8, 8.0]),
        ("tail_cord", "hemp_cord", "cord_fringe_2", [8.05, 0.0, 7.8], [8.40, 0.8, 8.3]),
    ]


def build_cubes():
    """组装所有分件"""
    cubes = []
    cubes.extend(part_pivot())
    cubes.extend(part_handle())
    cubes.extend(part_blade())
    cubes.extend(part_tail_cord())
    return cubes


def generate_texture() -> Image.Image:
    """生成 64x64 四象限纹理贴图：
    Q1 (0..32, 0..32): weathered_iron - 凡铁暗哑、铁锈、磨痕
    Q2 (32..64, 0..32): dark_wood - 深褐风干老木、天然纵向木纹
    Q3 (0..32, 32..64): brass_rivet - 古旧黄铜、金属反光、氧化斑
    Q4 (32..64, 32..64): hemp_cord - 粗麻绳/熟皮绳交错编织纹
    """
    img = Image.new("RGBA", (RES, RES), (0, 0, 0, 255))
    rng = np.random.default_rng(42)

    # ── Q1: 凡铁刀身 (weathered_iron: 0..32, 0..32) ──
    iron_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    base_iron = np.array([68, 72, 78], dtype=np.float32)
    noise_iron = rng.normal(0, 8, (32, 32, 1))
    iron_rgb = np.clip(base_iron + noise_iron, 40, 110).astype(np.uint8)
    iron_arr[:, :, :3] = iron_rgb
    iron_arr[:, :, 3] = 255
    q1 = Image.fromarray(iron_arr, "RGBA")
    q1_draw = ImageDraw.Draw(q1)

    # 铁锈斑与暗痕
    for _ in range(12):
        rx, ry = rng.integers(2, 30, 2)
        rw, rh = rng.integers(2, 5, 2)
        rust_col = (rng.integers(75, 95), rng.integers(50, 65), rng.integers(35, 45), 255)
        q1_draw.ellipse([rx, ry, rx + rw, ry + rh], fill=rust_col)

    # 研磨亮色开刃线
    for y in range(0, 32):
        bright = int(170 + rng.integers(0, 40))
        q1_draw.line([(0, y), (4, y)], fill=(bright, bright + 5, bright + 10, 255))
        q1_draw.line([(5, y), (7, y)], fill=(bright - 40, bright - 35, bright - 30, 255))

    img.paste(q1, (0, 0))

    # ── Q2: 深褐老木刀柄 (dark_wood: 32..64, 0..32) ──
    wood_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    base_wood = np.array([82, 58, 42], dtype=np.float32)
    noise_wood = rng.normal(0, 6, (32, 32, 1))
    wood_rgb = np.clip(base_wood + noise_wood, 50, 120).astype(np.uint8)
    wood_arr[:, :, :3] = wood_rgb
    wood_arr[:, :, 3] = 255
    q2 = Image.fromarray(wood_arr, "RGBA")
    q2_draw = ImageDraw.Draw(q2)

    # 纵向木纹纹理
    for x in range(0, 32, 3):
        col_w = int(60 + rng.integers(0, 25))
        q2_draw.line([(x, 0), (x, 31)], fill=(col_w + 15, col_w, col_w - 15, 255))
        # 局部深色木结
        if rng.random() > 0.6:
            kn_y = rng.integers(4, 28)
            q2_draw.ellipse([x - 1, kn_y - 2, x + 2, kn_y + 2], fill=(45, 30, 20, 255))

    img.paste(q2, (32, 0))

    # ── Q3: 黄铜转轴与铆钉 (brass_rivet: 0..32, 32..64) ──
    brass_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    base_brass = np.array([165, 135, 68], dtype=np.float32)
    noise_brass = rng.normal(0, 7, (32, 32, 1))
    brass_rgb = np.clip(base_brass + noise_brass, 110, 210).astype(np.uint8)
    brass_arr[:, :, :3] = brass_rgb
    brass_arr[:, :, 3] = 255
    q3 = Image.fromarray(brass_arr, "RGBA")
    q3_draw = ImageDraw.Draw(q3)

    # 黄铜同心圆高光与铜绿暗斑
    q3_draw.ellipse([6, 6, 26, 26], outline=(210, 185, 110, 255), width=2)
    q3_draw.ellipse([11, 11, 21, 21], fill=(135, 105, 48, 255))
    q3_draw.ellipse([14, 14, 18, 18], fill=(230, 205, 130, 255))
    # 局部铜绿
    for _ in range(4):
        px = rng.integers(2, 30)
        py = rng.integers(2, 30)
        q3_draw.point((px, py), fill=(75, 125, 95, 255))

    img.paste(q3, (0, 32))

    # ── Q4: 麻绳与皮绳结 (hemp_cord: 32..64, 32..64) ──
    cord_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    base_cord = np.array([135, 105, 68], dtype=np.float32)
    noise_cord = rng.normal(0, 8, (32, 32, 1))
    cord_rgb = np.clip(base_cord + noise_cord, 85, 175).astype(np.uint8)
    cord_arr[:, :, :3] = cord_rgb
    cord_arr[:, :, 3] = 255
    q4 = Image.fromarray(cord_arr, "RGBA")
    q4_draw = ImageDraw.Draw(q4)

    # 斜向麻绳编织交错纹
    for i in range(-32, 64, 4):
        q4_draw.line([(i, 0), (i + 32, 32)], fill=(160, 130, 85, 255), width=1)
        q4_draw.line([(i + 2, 0), (i + 34, 32)], fill=(95, 70, 45, 255), width=1)

    img.paste(q4, (32, 32))

    return img


def get_uv_for_mat(mat_key: str, from_xyz: list[float], to_xyz: list[float]) -> dict:
    """按象限映射 Blockbench 盒状 UV"""
    u0, v0, u1, v1 = MAT_ZONE[mat_key]
    dx = abs(to_xyz[0] - from_xyz[0])
    dy = abs(to_xyz[1] - from_xyz[1])
    dz = abs(to_xyz[2] - from_xyz[2])

    # 缩放因子让材质在小部件上展现细节
    scale = 3.0
    u_len = min(30.0, max(2.0, (dz + dx) * scale))
    v_len = min(30.0, max(2.0, (dz + dy) * scale))

    base_u = u0 + 1.0
    base_v = v0 + 1.0

    return {
        "north": {"uv": [base_u, base_v, base_u + dx * scale, base_v + dy * scale], "texture": 0},
        "south": {"uv": [base_u, base_v, base_u + dx * scale, base_v + dy * scale], "texture": 0},
        "east": {"uv": [base_u, base_v, base_u + dz * scale, base_v + dy * scale], "texture": 0},
        "west": {"uv": [base_u, base_v, base_u + dz * scale, base_v + dy * scale], "texture": 0},
        "up": {"uv": [base_u, base_v, base_u + dx * scale, base_v + dz * scale], "texture": 0},
        "down": {"uv": [base_u, base_v, base_u + dx * scale, base_v + dz * scale], "texture": 0},
    }


def build_bbmodel(cubes: list[tuple] | None = None, texture_img: Image.Image | None = None) -> tuple[dict, list[tuple], Image.Image]:
    """构建标准 Blockbench .bbmodel JSON"""
    if cubes is None:
        cubes = build_cubes()
        _assert_no_coplanar_faces(cubes)
    if texture_img is None:
        texture_img = generate_texture()

    tex_buf = io.BytesIO()
    texture_img.save(tex_buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(tex_buf.getvalue()).decode("ascii")

    elements = []
    group_elements: dict[str, list[str]] = {name: [] for name in BONE_ORDER}

    for bone, mat, name, from_xyz, to_xyz in cubes:
        elem_uuid = str(uuid.uuid4())
        uv_faces = get_uv_for_mat(mat, from_xyz, to_xyz)
        elem = {
            "name": name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": [round(from_xyz[0], 3), round(from_xyz[1], 3), round(from_xyz[2], 3)],
            "to": [round(to_xyz[0], 3), round(to_xyz[1], 3), round(to_xyz[2], 3)],
            "faces": uv_faces,
            "uuid": elem_uuid,
        }
        elements.append(elem)
        if bone in group_elements:
            group_elements[bone].append(elem_uuid)

    groups = []
    for bone_name in BONE_ORDER:
        groups.append({
            "name": bone_name,
            "origin": BONE_PIVOTS.get(bone_name, [8.0, 8.0, 8.0]),
            "color": 0,
            "children": group_elements[bone_name],
        })

    bbmodel = {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "HerbKnifeIron",
        "model_identifier": "herb_knife_iron",
        "visible_box": [-16.0, -16.0, -16.0, 16.0, 16.0, 16.0],
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": groups,
        "textures": [
            {
                "path": "",
                "name": "herb_knife_iron_texture",
                "folder": "item",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "visible": True,
                "mode": "bitmap",
                "saved": True,
                "uuid": str(uuid.uuid4()),
                "source": tex_b64,
            }
        ],
        "display": {
            "thirdperson_righthand": {
                "rotation": [0, 90, -55],
                "translation": [0, 4.0, 1.5],
                "scale": [0.85, 0.85, 0.85]
            },
            "thirdperson_lefthand": {
                "rotation": [0, -90, 55],
                "translation": [0, 4.0, 1.5],
                "scale": [0.85, 0.85, 0.85]
            },
            "firstperson_righthand": {
                "rotation": [0, -90, 25],
                "translation": [1.13, 3.2, 1.13],
                "scale": [0.68, 0.68, 0.68]
            },
            "firstperson_lefthand": {
                "rotation": [0, 90, -25],
                "translation": [1.13, 3.2, 1.13],
                "scale": [0.68, 0.68, 0.68]
            },
            "ground": {
                "rotation": [0, 0, 0],
                "translation": [0, 2, 0],
                "scale": [0.7, 0.7, 0.7]
            },
            "gui": {
                "rotation": [0, 90, -45],
                "translation": [0, 0, 0],
                "scale": [1.0, 1.0, 1.0]
            },
            "head": {
                "rotation": [0, 0, 0],
                "translation": [0, 0, 0],
                "scale": [1.0, 1.0, 1.0]
            },
            "fixed": {
                "rotation": [0, 180, 0],
                "translation": [0, 0, 0],
                "scale": [0.8, 0.8, 0.8]
            }
        }
    }
    return bbmodel, cubes, texture_img


def main():
    parser = argparse.ArgumentParser(description="生成凡铁采药刀 Blockbench 模型")
    parser.add_argument("--out", type=Path, default=BBMODEL_OUT, help="输出 .bbmodel 路径")
    parser.add_argument("--round", type=int, default=3, help="迭代轮次 (1/2/3)")
    args = parser.parse_args()

    # 兼容相对路径 / 沙箱根目录
    out_path = args.out
    if not out_path.is_absolute():
        out_path = Path.cwd() / out_path

    out_path.parent.mkdir(parents=True, exist_ok=True)
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    bbmodel_json, cubes, texture_img = build_bbmodel()

    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(bbmodel_json, f, indent=2)

    print(f"✓ 成功生成 Blockbench 模型: {out_path} (包含 {len(cubes)} 个立方体)")


if __name__ == "__main__":
    main()
