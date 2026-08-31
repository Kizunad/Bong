#!/usr/bin/env python3
"""普通素面青铜单刀 / 环首刀 (Bronze Saber / bronze_saber) Blockbench .bbmodel 生成器。

严格依据 AI 参考图（概念图、物品图标、三视图与爆炸分解图）形制建模：
- 末法残土素朴写实画风：去华丽化，素面厚刃，实用废土单刀。

分 5 大 Group（Bone 骨骼节点）：
    1. pommel      - 环首（经典青铜大圆环配重刀首）
    2. tassel      - 环首垂绳（单股粗麻绳节与下垂纤维流苏）
    3. grip        - 缠绳握把（粗麻绳密缠木柄，带抓握起伏）
    4. guard       - 青铜盘镡（极简扁平椭圆/八角青铜小刀镡与刀夹）
    5. blade       - 厚背直刃单刀（包含厚刀背、斜削锋刃、斜切刀尖与刀根铜夹）

尺寸规范（MC px，16px = 1 格）：
    总长约 26.5px ≈ 1.65 格（刀刃 ~19.0px，刀柄+环首 ~7.5px）。
    握把半宽约 0.85px（Ø1.7px），符合玩家单手握姿。
    刀身宽度宽厚突出（宽度 2.7px，厚度 0.65px），具有沉重朴实的青铜厚砍刀剪影。

贴图规范（64×64 四象限 Atlas）：
    - 刀身青铜 (bronze_blade): 暗黑古铜、暗绿色铜绿斑驳、冷色研磨刃线
    - 刀首刀镡 (bronze_fittings): 铸造粗糙青铜、微凹凸氧化颗粒
    - 握把麻绳 (cord_wrap): 枯黄/深褐麻绳编织交错纹、高对比阴影
    - 刀柄内木与木芯 (wood_core): 深色风干老木、磨损木纹

用法:
    python3 modelScript/generators/gen_bronze_saber.py
    bbmodel-render modelScript/models/BronzeSaber.bbmodel --three-view
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import tempfile
import uuid
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "BronzeSaber.bbmodel"
PREVIEW_OUT = Path(__file__).resolve().parents[1] / "out" / "bronze_saber_preview.png"

PX = 16.0
RES = 64

# ── 纵向分段与尺寸（y: 环首底=0.0，刀尖朝 +Y）───────────────────────────
RING_Y = (0.00, 2.80)        # 青铜环首（直径约 2.8px）
RING_NECK_Y = (2.80, 3.30)   # 环首与刀柄过渡颈
GRIP_Y = (3.30, 7.80)        # 缠绳握把（长 4.5px，适合单手握持）
GUARD_Y = (7.80, 8.45)       # 扁平青铜刀镡（厚 0.65px）
COLLAR_Y = (8.45, 9.30)      # 刀根铜夹（Habaki，长 0.85px）
BLADE_Y0 = 9.30              # 刀刃主体起点
BLADE_MAIN_LEN = 13.20       # 直刃主体长度
BLADE_TIP_LEN = 4.00         # 斜削刀尖长度（总长 9.3 + 13.2 + 4.0 = 26.5px ≈ 1.65格）

# 握把中点（用于 MC display 居中握持偏移）
GRIP_PX = (GRIP_Y[0] + GRIP_Y[1]) / 2.0  # 5.55
BLOCK_CENTRE_PX = 8.0
EMIT_OFFSET = (BLOCK_CENTRE_PX, BLOCK_CENTRE_PX - GRIP_PX, BLOCK_CENTRE_PX)

# 刀身断面尺寸（X: 宽度/刃宽，Z: 厚度）
# 背厚（+X侧）：厚度 0.66px；刃口（-X侧）：厚度 0.16px
BLADE_HW_ROOT = 1.35         # 根部宽度 2.7px (从 -1.35 到 +1.35)
BLADE_HW_TIP = 1.15          # 尖前半宽 2.3px
BLADE_THICK_SPINE = 0.33     # 刀背半厚（全厚 0.66px）
BLADE_THICK_EDGE = 0.08      # 刀刃半厚（全厚 0.16px）

BONE_ORDER = ["pommel", "tassel", "grip", "guard", "blade"]
BONE_COLORS = {
    "pommel": (158, 140, 96),       # 青铜环首
    "tassel": (130, 110, 75),       # 麻绳流苏
    "grip": (105, 80, 52),          # 缠绳木柄
    "guard": (165, 148, 102),       # 青铜镡
    "blade": (140, 155, 138),       # 绿锈青铜刃
}

# 贴图象限规划 (64x64)
# Q1 (0..32, 0..32): 青铜刀身 (bronze_blade)
# Q2 (32..64, 0..32): 青铜五金件 (bronze_fittings: 环首、镡、铜夹)
# Q3 (0..32, 32..64): 麻绳握把与流苏 (cord_wrap)
# Q4 (32..64, 32..64): 刀柄老木 (wood_core)
MAT_ZONE = {
    "bronze_blade": (0, 0, 32, 32),
    "bronze_fittings": (32, 0, 64, 32),
    "cord_wrap": (0, 32, 32, 64),
    "wood_core": (32, 32, 64, 64),
}


def build_cubes():
    """构建所有立方体元素，组织进对应 Group。
    返回列表: [(bone_name, mat_key, cube_name, from_xyz, to_xyz, rot_xyz)]
    """
    cubes: list[tuple] = []

    def block(bone, mat, name, x0, x1, y0, y1, z0, z1, rot=(0.0, 0.0, 0.0)):
        fx, tx = min(x0, x1), max(x0, x1)
        fy, ty = min(y0, y1), max(y0, y1)
        fz, tz = min(z0, z1), max(z0, z1)
        cubes.append((bone, mat, name, [fx, fy, fz], [tx, ty, tz], tuple(rot)))

    def octagon(bone, mat, name, hw, y0, y1, hz=None):
        hz = hw if hz is None else hz
        block(bone, mat, f"{name}_0", -hw, hw, y0, y1, -hz, hz, rot=(0.0, 0.0, 0.0))
        block(bone, mat, f"{name}_45", -hw, hw, y0, y1, -hz, hz, rot=(0.0, 45.0, 0.0))

    # ═════════════════════════════════════════════════════════════════════════
    # 1. POMMEL（经典素面青铜环首）
    # ═════════════════════════════════════════════════════════════════════════
    # 经典大环首：外径 2.8px，内径 1.6px，厚 0.60px
    # 底部横梁
    block("pommel", "bronze_fittings", "ring_bottom", -1.25, 1.25, RING_Y[0], RING_Y[0] + 0.65, -0.30, 0.30)
    # 顶部横梁
    block("pommel", "bronze_fittings", "ring_top", -1.25, 1.25, RING_Y[1] - 0.65, RING_Y[1], -0.30, 0.30)
    # 左右立柱
    block("pommel", "bronze_fittings", "ring_left", -1.45, -0.75, RING_Y[0] + 0.50, RING_Y[1] - 0.50, -0.30, 0.30)
    block("pommel", "bronze_fittings", "ring_right", 0.75, 1.45, RING_Y[0] + 0.50, RING_Y[1] - 0.50, -0.30, 0.30)
    # 环首斜角切面拟合圆角（四个内/外角微块）
    block("pommel", "bronze_fittings", "ring_corner_bl", -1.35, -0.70, RING_Y[0] + 0.20, RING_Y[0] + 0.75, -0.28, 0.28, rot=(0.0, 0.0, 45.0))
    block("pommel", "bronze_fittings", "ring_corner_br", 0.70, 1.35, RING_Y[0] + 0.20, RING_Y[0] + 0.75, -0.28, 0.28, rot=(0.0, 0.0, -45.0))
    # 环首过渡颈（八角铜柱）
    octagon("pommel", "bronze_fittings", "ring_neck", 0.80, RING_NECK_Y[0], RING_NECK_Y[1], hz=0.60)

    # ═════════════════════════════════════════════════════════════════════════
    # 2. TASSEL（环首系带与流苏）
    # ═════════════════════════════════════════════════════════════════════════
    # 穿过环孔的粗麻绳圈
    block("tassel", "cord_wrap", "tassel_loop", -0.35, 0.35, RING_Y[0] + 0.40, RING_Y[0] + 1.05, -0.18, 0.18)
    # 下垂绳段
    block("tassel", "cord_wrap", "tassel_cord1", -0.22, 0.22, -1.60, RING_Y[0] + 0.40, -0.16, 0.16, rot=(0.0, 0.0, 4.0))
    # 粗结
    block("tassel", "cord_wrap", "tassel_knot", -0.45, 0.45, -2.40, -1.60, -0.32, 0.32)
    # 散开的流苏穗子
    block("tassel", "cord_wrap", "tassel_fringe1", -0.48, 0.48, -4.00, -2.40, -0.28, 0.28, rot=(0.0, 0.0, 6.0))
    block("tassel", "cord_wrap", "tassel_fringe2", -0.35, 0.35, -5.40, -4.00, -0.20, 0.20, rot=(0.0, 0.0, -4.0))

    # ═════════════════════════════════════════════════════════════════════════
    # 3. GRIP（缠绳握把）
    # ═════════════════════════════════════════════════════════════════════════
    # 木柄内芯（扁圆八角木芯，长 4.5px，宽 1.5px，厚 1.1px）
    octagon("grip", "wood_core", "grip_core", 0.75, GRIP_Y[0], GRIP_Y[1], hz=0.55)
    # 5 圈紧密交错缠绕的粗麻绳环（模拟手握防滑起伏）
    wrap_n = 5
    for i in range(wrap_n):
        y0 = GRIP_Y[0] + i * (GRIP_Y[1] - GRIP_Y[0]) / wrap_n
        y1 = y0 + 0.62
        octagon("grip", "cord_wrap", f"grip_wrap_{i}", 0.92, y0, y1, hz=0.68)

    # ═════════════════════════════════════════════════════════════════════════
    # 4. GUARD & COLLAR（青铜盘镡与刀夹）
    # ═════════════════════════════════════════════════════════════════════════
    # 扁平椭圆盘镡（宽 3.4px，厚 2.2px，高 0.65px）
    octagon("guard", "bronze_fittings", "guard_plate", 1.70, GUARD_Y[0], GUARD_Y[1], hz=1.10)
    # 刀根青铜夹 (Habaki/刀夹)
    octagon("guard", "bronze_fittings", "blade_collar", 1.15, COLLAR_Y[0], COLLAR_Y[1], hz=0.68)

    # ═════════════════════════════════════════════════════════════════════════
    # 5. BLADE（青铜单刀刀身与斜削刀尖）
    # ═════════════════════════════════════════════════════════════════════════
    # 坐标系约定：
    # +X 侧为【刀背】（Spine，厚实厚重）
    # -X 侧为【刀刃】（Edge，薄且锋锐，朝向切割方向）
    # Z 轴为刀身厚度面
    
    blade_segs = 6
    seg_h = BLADE_MAIN_LEN / blade_segs
    for i in range(blade_segs):
        y0 = BLADE_Y0 + i * seg_h
        y1 = y0 + seg_h
        t = i / blade_segs
        hw = BLADE_HW_ROOT + (BLADE_HW_TIP - BLADE_HW_ROOT) * t
        th_spine = BLADE_THICK_SPINE * (1.0 - 0.08 * t)

        # (1) 刀背（厚方脊，+X侧：从 0 到 +hw）
        block("blade", "bronze_blade", f"blade_spine_{i}", 0.00, hw, y0, y1, -th_spine, th_spine)
        # (2) 刀身中斜面（过渡斜面）
        block("blade", "bronze_blade", f"blade_mid_{i}", -hw * 0.65, 0.10, y0, y1, -th_spine * 0.70, th_spine * 0.70)
        # (3) 刀刃（极薄锋线，-X侧：从 -hw 到 -hw*0.55）
        block("blade", "bronze_blade", f"blade_edge_{i}", -hw, -hw * 0.55, y0, y1, -BLADE_THICK_EDGE, BLADE_THICK_EDGE)

    # 刀尖部分 (Blade Tip) - 经典斜切单刃尖 (Clip Point)
    y_tip0 = BLADE_Y0 + BLADE_MAIN_LEN
    y_tip1 = y_tip0 + BLADE_TIP_LEN * 0.40
    y_tip2 = y_tip0 + BLADE_TIP_LEN * 0.75
    y_tip3 = y_tip0 + BLADE_TIP_LEN

    # 刀尖基段
    block("blade", "bronze_blade", "tip_base_spine", 0.00, BLADE_HW_TIP * 0.90, y_tip0, y_tip1, -BLADE_THICK_SPINE * 0.80, BLADE_THICK_SPINE * 0.80)
    block("blade", "bronze_blade", "tip_base_edge", -BLADE_HW_TIP * 0.90, 0.00, y_tip0, y_tip1, -BLADE_THICK_EDGE * 1.2, BLADE_THICK_EDGE * 1.2)

    # 刀尖斜削段（背部斜切向刃尖收缩）
    block("blade", "bronze_blade", "tip_mid_spine", -0.15, BLADE_HW_TIP * 0.60, y_tip1, y_tip2, -BLADE_THICK_SPINE * 0.60, BLADE_THICK_SPINE * 0.60, rot=(0.0, 0.0, -12.0))
    block("blade", "bronze_blade", "tip_mid_edge", -BLADE_HW_TIP * 0.80, -0.05, y_tip1, y_tip2, -BLADE_THICK_EDGE, BLADE_THICK_EDGE)

    # 最终尖端刺角
    block("blade", "bronze_blade", "tip_point", -BLADE_HW_TIP * 0.70, 0.10, y_tip2, y_tip3, -BLADE_THICK_EDGE * 0.9, BLADE_THICK_EDGE * 0.9, rot=(0.0, 0.0, -18.0))

    return cubes


def make_texture_atlas() -> Image.Image:
    """生成 64x64 四象限青铜单刀 Texture Atlas"""
    img = Image.new("RGBA", (RES, RES), (0, 0, 0, 0))
    rng = np.random.default_rng(42)

    # 1. Q1: 青铜刀身 (bronze_blade, 0..32, 0..32)
    # 基底暗铜绿 + 灰黑氧化 + 刃口研磨高光
    for y in range(0, 32):
        for x in range(0, 32):
            noise = rng.integers(-14, 15)
            r = int(np.clip(82 + noise * 0.8, 48, 125))
            g = int(np.clip(112 + noise * 1.2, 72, 155))
            b = int(np.clip(96 + noise * 0.9, 62, 138))

            # 铜绿氧化斑点 (Verdigris patch)
            if (x * 7 + y * 13) % 17 < 4:
                r = int(np.clip(r - 25, 30, 90))
                g = int(np.clip(g + 35, 95, 185))
                b = int(np.clip(b + 30, 85, 165))

            # 刀刃锋线高光区（左侧 x<10）
            if x < 10:
                r = int(np.clip(r + 40, 95, 175))
                g = int(np.clip(g + 40, 115, 195))
                b = int(np.clip(b + 35, 105, 185))

            img.putpixel((x, y), (r, g, b, 255))

    # 2. Q2: 青铜五金件 (bronze_fittings, 32..64, 0..32)
    # 铸造质感更重、颜色偏暗金古铜
    for y in range(0, 32):
        for x in range(32, 64):
            noise = rng.integers(-15, 16)
            r = int(np.clip(140 + noise, 85, 175))
            g = int(np.clip(120 + noise * 0.9, 75, 155))
            b = int(np.clip(78 + noise * 0.7, 48, 115))
            if (x + y) % 5 == 0:
                r = int(r * 0.78)
                g = int(g * 0.78)
                b = int(b * 0.78)
            img.putpixel((x, y), (r, g, b, 255))

    # 3. Q3: 麻绳缠把与流苏 (cord_wrap, 0..32, 32..64)
    # 枯草褐、风干粗麻纤维
    for y in range(32, 64):
        for x in range(0, 32):
            noise = rng.integers(-12, 13)
            r = int(np.clip(120 + noise * 1.1, 78, 160))
            g = int(np.clip(95 + noise * 0.9, 58, 135))
            b = int(np.clip(62 + noise * 0.8, 38, 98))
            if (x * 2 + y) % 4 == 0:
                r = int(r * 0.68)
                g = int(g * 0.68)
                b = int(b * 0.68)
            img.putpixel((x, y), (r, g, b, 255))

    # 4. Q4: 握柄老木 (wood_core, 32..64, 32..64)
    for y in range(32, 64):
        for x in range(32, 64):
            noise = rng.integers(-8, 9)
            r = int(np.clip(76 + noise, 46, 112))
            g = int(np.clip(54 + noise * 0.8, 32, 88))
            b = int(np.clip(38 + noise * 0.7, 22, 62))
            if x % 7 == 0:
                r = int(r * 0.82)
                g = int(g * 0.82)
                b = int(b * 0.82)
            img.putpixel((x, y), (r, g, b, 255))

    return img


def build_bbmodel(cubes: list[tuple], texture_img: Image.Image) -> dict:
    """组装符合 Blockbench 4.10 规范的 .bbmodel JSON"""
    tex_bytes = io.BytesIO()
    texture_img.save(tex_bytes, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(tex_bytes.getvalue()).decode("ascii")
    tex_uuid = str(uuid.uuid4())

    elements = []
    element_map_by_bone: dict[str, list[str]] = {b: [] for b in BONE_ORDER}

    for idx, (bone, mat, name, from_xyz, to_xyz, rot_xyz) in enumerate(cubes):
        el_uuid = str(uuid.uuid4())
        element_map_by_bone[bone].append(el_uuid)

        # 换算到方块中心出料系
        f_out = [
            round(from_xyz[0] + EMIT_OFFSET[0], 4),
            round(from_xyz[1] + EMIT_OFFSET[1], 4),
            round(from_xyz[2] + EMIT_OFFSET[2], 4),
        ]
        t_out = [
            round(to_xyz[0] + EMIT_OFFSET[0], 4),
            round(to_xyz[1] + EMIT_OFFSET[1], 4),
            round(to_xyz[2] + EMIT_OFFSET[2], 4),
        ]

        # UV 分配在对应材质象限内
        zx0, zy0, zx1, zy1 = MAT_ZONE[mat]
        uv_u0 = zx0 + (idx * 3) % (zx1 - zx0 - 6)
        uv_v0 = zy0 + (idx * 3) % (zy1 - zy0 - 6)
        uv_box = [uv_u0, uv_v0, uv_u0 + 4, uv_v0 + 4]

        faces = {
            face: {
                "uv": uv_box,
                "texture": 0,
            }
            for face in ["north", "east", "south", "west", "up", "down"]
        }

        elem = {
            "name": name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": f_out,
            "to": t_out,
            "faces": faces,
            "uuid": el_uuid,
        }

        # 旋转（若有）
        if any(abs(r) > 1e-4 for r in rot_xyz):
            origin = [
                (f_out[0] + t_out[0]) / 2.0,
                (f_out[1] + t_out[1]) / 2.0,
                (f_out[2] + t_out[2]) / 2.0,
            ]
            elem["origin"] = origin
            elem["rotation"] = list(rot_xyz)

        elements.append(elem)

    # 组装骨骼树 (Outliner)
    outliner = []
    for bone in BONE_ORDER:
        bone_children = element_map_by_bone[bone]
        outliner.append({
            "name": bone,
            "origin": [8.0, 8.0, 8.0],
            "color": 0,
            "uuid": str(uuid.uuid4()),
            "export": True,
            "isOpen": True,
            "locked": False,
            "visibility": True,
            "autouv": 0,
            "children": bone_children,
        })

    bbmodel = {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "BronzeSaber",
        "model_identifier": "bronze_saber",
        "visible_box": [1, 1, 0],
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": outliner,
        "textures": [
            {
                "path": "",
                "name": "bronze_saber_atlas.png",
                "folder": "item",
                "namespace": "bong",
                "id": "0",
                "particle": False,
                "render_mode": "default",
                "saved": True,
                "uuid": tex_uuid,
                "source": tex_b64,
            }
        ],
        "display": {
            "thirdperson_righthand": {
                "rotation": [0, 90, -35],
                "translation": [0, 1.25, -3.5],
                "scale": [0.85, 0.85, 0.85],
            },
            "thirdperson_lefthand": {
                "rotation": [0, -90, 35],
                "translation": [0, 1.25, -3.5],
                "scale": [0.85, 0.85, 0.85],
            },
            "firstperson_righthand": {
                "rotation": [0, -90, 25],
                "translation": [1.13, 3.2, 1.13],
                "scale": [0.68, 0.68, 0.68],
            },
            "firstperson_lefthand": {
                "rotation": [0, 90, -25],
                "translation": [1.13, 3.2, 1.13],
                "scale": [0.68, 0.68, 0.68],
            },
            "ground": {
                "rotation": [0, 0, 0],
                "translation": [0, 2, 0],
                "scale": [0.5, 0.5, 0.5],
            },
            "gui": {
                "rotation": [0, 0, -45],
                "translation": [0, 0, 0],
                "scale": [0.75, 0.75, 0.75],
            },
            "head": {
                "rotation": [0, 180, 0],
                "translation": [0, 13, 7],
                "scale": [1, 1, 1],
            },
            "fixed": {
                "rotation": [0, 180, 0],
                "translation": [0, 0, 0],
                "scale": [1, 1, 1],
            },
        },
    }
    return bbmodel


def main():
    parser = argparse.ArgumentParser(description="生成青铜单刀 .bbmodel 模型")
    parser.add_argument("--out", default=str(BBMODEL_OUT), help="输出 .bbmodel 路径")
    parser.add_argument("--self-test", action="store_true", help="运行几何与共面自检")
    args = parser.parse_args()

    out_path = Path(args.out)
    cubes = build_cubes()
    print(f"✓ 构建完成: 共 {len(cubes)} 个 Cubes，分布于 {len(BONE_ORDER)} 个骨骼分组。")

    tex = make_texture_atlas()
    print(f"✓ 贴图完成: 64x64 四象限青铜刀 Atlas。")

    bbmodel_dict = build_bbmodel(cubes, tex)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(bbmodel_dict, f, indent=2, ensure_ascii=False)
    print(f"✓ 成功落盘 .bbmodel: {out_path}")


if __name__ == "__main__":
    main()
