#!/usr/bin/env python3
"""凡铁匕首（Iron Dagger / 凡铁短匕）Blockbench .bbmodel 生成器。

依据三视图与概念设计复刻形制，自柄尾向刀尖分 4 大 Group（Bone 骨骼节点）：
    1. pommel      - 八角琢面锻铁配重刀首（带中心凹槽与沉头铆钉）
    2. grip        - 深褐粗麻绳密缠木柄（带螺旋防滑起伏与指位收腰）
    3. guard       - 矩形锻打生铁横护手（略带下凹防滑，保护手掌）
    4. blade       - 锻打黑铁双刃刀身（中心起脊棱线、根部较宽、中段渐收、锋利尖头）

尺寸规范（MC px，16px = 1 格）：
    全长约 16.0px = 1.0 格（短刃，刀身 ~10.5px，护手 ~1.0px，握把+刀首 ~4.5px）。
    握把半宽约 0.65px（Ø1.3px），适合单手贴合正握/反握。

贴图规范（64×64 四象限 Atlas）：
    - 锻铁刀身 (iron_blade): 深灰黑铁底色、手工锻打微凹坑、刃口冷光高光与微弱磨损
    - 麻绳缠柄 (cord_grip): 深褐麻绳纤维、斜向编织绞线、指握磨光高光
    - 铸铁五金 (iron_fitting): 粗粝铸铁质感、边缘撞击倒角高光、微弱氧化暗斑
    - 木质内芯 (wood_core): 坚韧枯木芯、深色木纹（仅露于缝隙与末端）

用法:
    python3 modelScript/generators/gen_iron_dagger.py
    python3 modelScript/generators/gen_iron_dagger.py --preview-only
    bbmodel-render modelScript/models/IronDagger.bbmodel --three-view
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
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "IronDagger.bbmodel"
PREVIEW_OUT = Path(__file__).resolve().parents[1] / "out" / "iron_dagger_preview.png"

PX = 16.0
RES = 64

# ── 纵向分段与尺寸（y: 柄尾=0.0，尖端朝 +Y）─────────────────────────────
POMMEL_Y = (0.00, 1.20)      # 八角锻铁配重刀首
GRIP_Y = (1.20, 4.60)        # 麻绳缠握把（长 3.4px）
GUARD_Y = (4.60, 5.60)       # 矩形铁护手（厚 1.0px）
BLADE_Y0 = 5.60              # 刀刃起点
BLADE_LEN = 10.40            # 刀刃总长（总长 5.6 + 10.4 = 16.0px = 1.0格）

# ── 授权系 → 出料系 ───────────────────────────────────────────────────────
# MC 的 display 变换绕方块中心转 (8, 8, 8)，因此 emit_offset = (8, 8 - grip_center, 8)
GRIP_PX = (GRIP_Y[0] + GRIP_Y[1]) / 2.0   # 2.9 —— 拳心对准处 = 握把中点
BLOCK_CENTRE_PX = 8.0
EMIT_OFFSET = (BLOCK_CENTRE_PX, BLOCK_CENTRE_PX - GRIP_PX, BLOCK_CENTRE_PX)

# Group 定义
BONE_ORDER = ["pommel", "grip", "guard", "blade"]
BONE_COLORS = {
    "pommel": (120, 122, 125),     # 锻铁
    "grip": (96, 68, 48),          # 褐色麻绳
    "guard": (110, 112, 115),      # 铸铁护手
    "blade": (150, 155, 160),      # 钢刃
}

# 贴图象限规划 (64x64)
# Q1 (0..32, 0..32): 锻铁刀身 (blade)
# Q2 (32..64, 0..32): 麻绳缠柄 (cord)
# Q3 (0..32, 32..64): 五金与护手 (fitting)
# Q4 (32..64, 32..64): 木柄内芯 (wood)
MAT_ZONE = {
    "blade": (0, 0, 32, 32),
    "cord": (32, 0, 64, 32),
    "fitting": (0, 32, 32, 64),
    "wood": (32, 32, 64, 64),
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
        """用两个重叠的 0° / 45° 盒拟合八角圆柱"""
        hz = hw if hz is None else hz
        block(bone, mat, f"{name}_0", -hw, hw, y0, y1, -hz, hz, rot=(0.0, 0.0, 0.0))
        block(bone, mat, f"{name}_45", -hw, hw, y0, y1, -hz, hz, rot=(0.0, 45.0, 0.0))

    # ═════════════════════════════════════════════════════════════════════════
    # 1. POMMEL（八角配重刀首）
    # ═════════════════════════════════════════════════════════════════════════
    # 底部八角琢面端帽
    octagon("pommel", "fitting", "pommel_cap", 0.90, POMMEL_Y[0], POMMEL_Y[0] + 0.50)
    octagon("pommel", "fitting", "pommel_ring", 0.75, POMMEL_Y[0] + 0.50, POMMEL_Y[1])
    # 尾端沉头铆钉（中心小凸台）
    block("pommel", "fitting", "pommel_rivet", -0.30, 0.30, POMMEL_Y[0] - 0.12, POMMEL_Y[0] + 0.10, -0.30, 0.30)

    # ═════════════════════════════════════════════════════════════════════════
    # 2. GRIP（麻绳密缠木柄）
    # ═════════════════════════════════════════════════════════════════════════
    # 握把内芯（带微弱的人体工学腰身变化）
    # 下段略宽、中段微收、上段渐扩至护手
    block("grip", "wood", "grip_core_low", -0.55, 0.55, GRIP_Y[0], GRIP_Y[0] + 1.20, -0.45, 0.45)
    block("grip", "wood", "grip_core_mid", -0.50, 0.50, GRIP_Y[0] + 1.20, GRIP_Y[0] + 2.30, -0.42, 0.42)
    block("grip", "wood", "grip_core_up", -0.58, 0.58, GRIP_Y[0] + 2.30, GRIP_Y[1], -0.48, 0.48)

    # 螺旋外缠粗麻绳圈（4圈起伏环，增强握持防滑质感）
    wrap_n = 5
    grip_span = GRIP_Y[1] - GRIP_Y[0]
    for i in range(wrap_n):
        wy0 = GRIP_Y[0] + (i * grip_span / wrap_n) + 0.06
        wy1 = wy0 + (grip_span / wrap_n) - 0.12
        # 按腰身微调各圈外径
        w_scale = 0.68 if (i == 2) else (0.72 if (i in (1, 3)) else 0.75)
        h_scale = w_scale * 0.82
        block("grip", "cord", f"grip_wrap_{i}", -w_scale, w_scale, wy0, wy1, -h_scale, h_scale)

    # ═════════════════════════════════════════════════════════════════════════
    # 3. GUARD（平直铸铁横护手）
    # ═════════════════════════════════════════════════════════════════════════
    # 护手基座衬套
    block("guard", "fitting", "guard_collar", -0.75, 0.75, GUARD_Y[0], GUARD_Y[0] + 0.25, -0.55, 0.55)
    # 横向主护手条（宽 4.0px，高 0.6px，厚 1.0px）
    block("guard", "fitting", "guard_cross", -2.00, 2.00, GUARD_Y[0] + 0.20, GUARD_Y[0] + 0.80, -0.50, 0.50)
    # 左右两端略微收口的端头加强块
    block("guard", "fitting", "guard_tip_l", -2.15, -1.85, GUARD_Y[0] + 0.15, GUARD_Y[0] + 0.85, -0.55, 0.55)
    block("guard", "fitting", "guard_tip_r", 1.85, 2.15, GUARD_Y[0] + 0.15, GUARD_Y[0] + 0.85, -0.55, 0.55)
    # 刀刃出鞘颈部垫圈
    block("guard", "fitting", "guard_blade_ferrule", -0.70, 0.70, GUARD_Y[0] + 0.75, GUARD_Y[1], -0.35, 0.35)

    # ═════════════════════════════════════════════════════════════════════════
    # 4. BLADE（双刃锻打黑铁刀身，四段渐收破甲尖）
    # ═════════════════════════════════════════════════════════════════════════
    # 刀身分段构建：
    # 1. 刃根 (Root): 衔接护手，较厚较平
    # 2. 刃身主体 (Belly): 最宽处，带有中心起脊棱线
    # 3. 破甲渐收段 (Taper): 快速向中心收窄
    # 4. 刀尖破甲尖 (Tip): 锐利尖头
    
    # 段 1: 刃根
    block("blade", "blade", "blade_root_core", -0.60, 0.60, BLADE_Y0, BLADE_Y0 + 1.80, -0.22, 0.22)
    # 刃根两侧斜切开刃
    block("blade", "blade", "blade_root_edge_l", -0.85, -0.55, BLADE_Y0, BLADE_Y0 + 1.80, -0.12, 0.12)
    block("blade", "blade", "blade_root_edge_r", 0.55, 0.85, BLADE_Y0, BLADE_Y0 + 1.80, -0.12, 0.12)

    # 段 2: 刀身主体（起脊凸棱）
    y_b0 = BLADE_Y0 + 1.80
    y_b1 = y_b0 + 4.20
    # 中心厚脊
    block("blade", "blade", "blade_spine_core", -0.40, 0.40, y_b0 - 0.05, y_b1, -0.26, 0.26)
    # 中间主刃片
    block("blade", "blade", "blade_belly_main", -0.80, 0.80, y_b0 - 0.05, y_b1, -0.16, 0.16)
    # 两侧外延薄刃
    block("blade", "blade", "blade_belly_edge_l", -1.05, -0.75, y_b0 - 0.05, y_b1, -0.08, 0.08)
    block("blade", "blade", "blade_belly_edge_r", 0.75, 1.05, y_b0 - 0.05, y_b1, -0.08, 0.08)

    # 段 3: 渐收破甲段
    y_t0 = y_b1
    y_t1 = y_t0 + 2.80
    block("blade", "blade", "blade_taper_core", -0.30, 0.30, y_t0 - 0.05, y_t1, -0.20, 0.20)
    block("blade", "blade", "blade_taper_main", -0.55, 0.55, y_t0 - 0.05, y_t1, -0.12, 0.12)
    block("blade", "blade", "blade_taper_edge_l", -0.75, -0.50, y_t0 - 0.05, y_t1, -0.06, 0.06)
    block("blade", "blade", "blade_taper_edge_r", 0.50, 0.75, y_t0 - 0.05, y_t1, -0.06, 0.06)

    # 段 4: 锐利刀尖收口
    y_tip0 = y_t1
    y_tip1 = y_tip0 + 1.60
    block("blade", "blade", "blade_tip_core", -0.18, 0.18, y_tip0 - 0.05, y_tip1 - 0.40, -0.14, 0.14)
    block("blade", "blade", "blade_tip_mid", -0.32, 0.32, y_tip0 - 0.05, y_tip1 - 0.40, -0.08, 0.08)
    # 极细最尖端
    block("blade", "blade", "blade_point_sharp", -0.12, 0.12, y_tip1 - 0.45, y_tip1, -0.08, 0.08)

    return cubes


def generate_texture_atlas() -> Image.Image:
    """生成 64x64 四象限手绘/程序化材质 Atlas。"""
    atlas = Image.new("RGBA", (RES, RES), (0, 0, 0, 0))
    draw = ImageDraw.Draw(atlas)
    rng = np.random.default_rng(0x1DA66E4)

    # 1. 锻铁刀身 (Q1: 0..32, 0..32)
    # 深铁基底 + 锤打斑驳 + 亮银开刃高光
    for y in range(0, 32):
        for x in range(0, 32):
            base_v = rng.integers(120, 145)
            # 增加锻打微噪点
            noise = rng.integers(-12, 13)
            val = int(np.clip(base_v + noise, 80, 200))
            # 铁色带轻微冷蓝调
            atlas.putpixel((x, y), (val - 4, val, val + 6, 255))

    # 刀身亮银开刃条纹 (x: 4..12, 20..28)
    for x in range(4, 12):
        for y in range(0, 32):
            r, g, b, _ = atlas.getpixel((x, y))
            atlas.putpixel((x, y), (min(255, r + 45), min(255, g + 48), min(255, b + 55), 255))
    for x in range(20, 28):
        for y in range(0, 32):
            r, g, b, _ = atlas.getpixel((x, y))
            atlas.putpixel((x, y), (min(255, r + 45), min(255, g + 48), min(255, b + 55), 255))

    # 2. 麻绳缠柄 (Q2: 32..64, 0..32)
    # 深褐麻绳绞线纹理
    for y in range(0, 32):
        for x in range(32, 64):
            # 45 度对角绞线
            stripe = (x + y * 2) % 6
            if stripe in (0, 1):
                c = (75 + rng.integers(-6, 7), 52 + rng.integers(-5, 6), 36 + rng.integers(-4, 5))
            elif stripe in (2, 3):
                c = (115 + rng.integers(-8, 9), 84 + rng.integers(-7, 8), 58 + rng.integers(-5, 6))
            else:
                c = (92 + rng.integers(-6, 7), 66 + rng.integers(-5, 6), 46 + rng.integers(-4, 5))
            atlas.putpixel((x, y), (c[0], c[1], c[2], 255))

    # 3. 铸铁五金/护手 (Q3: 0..32, 32..64)
    # 粗粝深灰铸铁，带边缘倒角高光
    for y in range(32, 64):
        for x in range(0, 32):
            base_v = rng.integers(90, 115)
            noise = rng.integers(-10, 11)
            val = int(np.clip(base_v + noise, 60, 160))
            atlas.putpixel((x, y), (val, val + 2, val + 4, 255))

    # 护手边缘高光与铁锈暗斑
    for i in range(16):
        rx, ry = rng.integers(2, 30), rng.integers(34, 62)
        atlas.putpixel((rx, ry), (160, 165, 170, 255))
        bx, by = rng.integers(2, 30), rng.integers(34, 62)
        atlas.putpixel((bx, by), (65, 52, 42, 255))

    # 4. 木柄内芯 (Q4: 32..64, 32..64)
    # 枯木纵向深色木纹
    for y in range(32, 64):
        for x in range(32, 64):
            grain = (x * 7 + y * 3) % 8
            if grain in (0, 1):
                c = (105, 82, 60)
            elif grain in (2, 3, 4):
                c = (128, 102, 75)
            else:
                c = (118, 92, 68)
            noise = rng.integers(-8, 9)
            atlas.putpixel((x, y), (c[0] + noise, c[1] + noise, c[2] + noise, 255))

    return atlas


def build_bbmodel_dict(cubes: list[tuple], atlas: Image.Image) -> dict:
    """组装符合 Blockbench 4.10 规范的 JSON 数据字典。"""
    tex_bytes = io.BytesIO()
    atlas.save(tex_bytes, format="PNG")
    b64_tex = "data:image/png;base64," + base64.b64encode(tex_bytes.getvalue()).decode("ascii")

    tex_uuid = str(uuid.uuid4())
    elements = []
    group_map: dict[str, list[str]] = {b: [] for b in BONE_ORDER}

    for idx, (bone_name, mat_key, cname, fx_fy_fz, tx_ty_tz, rot) in enumerate(cubes):
        el_uuid = str(uuid.uuid4())
        group_map[bone_name].append(el_uuid)

        # 授权系 -> 出料系坐标转换（握把对齐方块中心 8, 8, 8）
        f_xyz = [
            round(fx_fy_fz[0] + EMIT_OFFSET[0], 4),
            round(fx_fy_fz[1] + EMIT_OFFSET[1], 4),
            round(fx_fy_fz[2] + EMIT_OFFSET[2], 4),
        ]
        t_xyz = [
            round(tx_ty_tz[0] + EMIT_OFFSET[0], 4),
            round(tx_ty_tz[1] + EMIT_OFFSET[1], 4),
            round(tx_ty_tz[2] + EMIT_OFFSET[2], 4),
        ]

        # 计算 UV 映射区域
        u0, v0, u1, v1 = MAT_ZONE[mat_key]
        # 给每个 cube 在其材质象限内均匀分配一个小 UV 块
        col = (idx % 4) * 7
        row = ((idx // 4) % 4) * 7
        cu0 = u0 + col
        cv0 = v0 + row
        cu1 = cu0 + 6
        cv1 = cv0 + 6

        faces = {}
        for face_name in ("north", "south", "east", "west", "up", "down"):
            faces[face_name] = {
                "uv": [cu0, cv0, cu1, cv1],
                "texture": 0,
            }

        origin = [
            round((f_xyz[0] + t_xyz[0]) / 2.0, 4),
            round((f_xyz[1] + t_xyz[1]) / 2.0, 4),
            round((f_xyz[2] + t_xyz[2]) / 2.0, 4),
        ]

        el_dict = {
            "name": cname,
            "box_uv": False,
            "from": f_xyz,
            "to": t_xyz,
            "origin": origin,
            "faces": faces,
            "uuid": el_uuid,
        }
        if any(abs(r) > 1e-4 for r in rot):
            el_dict["rotation"] = [rot[0], rot[1], rot[2]]

        elements.append(el_dict)

    # 组装 groups 骨骼树
    out_groups = []
    for bname in BONE_ORDER:
        out_groups.append({
            "name": bname,
            "origin": [8.0, 8.0, 8.0],
            "color": 0,
            "isOpen": True,
            "children": group_map[bname],
        })

    # Display 变换配置（MC 第一人称/第三人称/GUI 手持显示规范）
    display = {
        "thirdperson_righthand": {
            "rotation": [0, -90, 55],
            "translation": [0, 2.5, -0.5],
            "scale": [0.85, 0.85, 0.85],
        },
        "thirdperson_lefthand": {
            "rotation": [0, 90, -55],
            "translation": [0, 2.5, -0.5],
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
            "scale": [0.95, 0.95, 0.95],
        },
        "fixed": {
            "rotation": [0, 180, 0],
            "translation": [0, 0, 0],
            "scale": [0.8, 0.8, 0.8],
        },
    }

    doc = {
        "meta": {
            "format_version": "4.10",
            "model_format": "free",
            "box_uv": False,
        },
        "name": "IronDagger",
        "geometry_name": "iron_dagger",
        "visible_box": [1, 1, 1],
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": out_groups,
        "textures": [
            {
                "name": "iron_dagger_atlas",
                "folder": "item",
                "id": "0",
                "particle": False,
                "use_as_default": True,
                "width": RES,
                "height": RES,
                "source": b64_tex,
                "uuid": tex_uuid,
            }
        ],
        "display": display,
    }
    return doc


def render_preview(doc: dict, out_path: Path) -> None:
    """使用 bbmodel-render 进行无头三视图软渲染并保存预览。"""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(suffix=".bbmodel", delete=False, mode="w", encoding="utf-8") as f:
        json.dump(doc, f, indent=2)
        tmp_name = f.name

    try:
        from bbmodel_maker.render.render_bbmodel import render
        # 正面、侧面、3/4 视角合成三视图 (render 返回 (PIL.Image, ascii_str))
        img_front, _ = render(tmp_name, yaw=180.0, pitch=0.0, size=512)
        img_side, _ = render(tmp_name, yaw=90.0, pitch=0.0, size=512)
        img_persp, _ = render(tmp_name, yaw=145.0, pitch=15.0, size=512)

        composite = Image.new("RGB", (1536, 512), (22, 23, 26))
        composite.paste(img_front, (0, 0))
        composite.paste(img_side, (512, 0))
        composite.paste(img_persp, (1024, 0))
        composite.save(out_path)
        print(f"  三视图预览保存至 → {out_path.relative_to(REPO)}")
    except Exception as e:
        print(f"  渲染三视图时出错: {e}")
    finally:
        Path(tmp_name).unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=BBMODEL_OUT, help="输出 .bbmodel 路径")
    parser.add_argument("--preview-out", type=Path, default=PREVIEW_OUT, help="输出渲染预览图路径")
    parser.add_argument("--preview-only", action="store_true", help="只渲染预览图，不写 .bbmodel")
    args = parser.parse_args()

    cubes = build_cubes()
    atlas = generate_texture_atlas()
    doc = build_bbmodel_dict(cubes, atlas)

    if not args.preview_only:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        with open(args.out, "w", encoding="utf-8") as f:
            json.dump(doc, f, indent=2)
        print(f"[IronDagger] 模型生成成功 ({len(cubes)} 个 Cubes) → {args.out.relative_to(REPO)}")

    render_preview(doc, args.preview_out)


if __name__ == "__main__":
    main()
