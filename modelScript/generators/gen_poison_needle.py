#!/usr/bin/env python3
"""毒蛊飞针 (poison_needle / PoisonNeedle) Blockbench .bbmodel 生成器。

严格依据 three_view.png 右侧 ITEM DETAIL 与 exploded.png 右上 BONE DART BREAKDOWN：
- 结构顺序（自顶向下）：
    1. tip   - 细长锯齿叶状矿物箭尖（venom_tip / 亮绿 #7fb03a 带暗绿 #2e4a1c 斑驳，边缘 2~3 级错开小齿，无白色块，直接接深色针身）
    2. shaft - 深黑灰暗化细长针身（needle_shaft / 贯穿主干，极细硬质针体）
    3. tail  - 针身下段细红丝缠绕与垂须（red_thread_wrap / 几圈 0.35px 细红线圈，圈间露出深色针身，垂下 2~3 根长短不一细红丝须）
    4. grip  - 木质握柄与底端收口（wooden_grip / 比针身粗一圈，深棕与浅棕相间的横向环纹缠绳/刻槽，底端深色小收口）

尺寸规范（MC px，16px = 1 格）：
    全长约 15.5px ≈ 0.97 格。
    底端收口与木柄 (grip): y: 0.00 -> 3.20 (长 3.2px, hw=0.48)
    红丝缠绕区 (tail): y: 3.20 -> 5.20 (长 2.0px, 细线圈 hw=0.38，垂须下垂至 y=2.20)
    深色针身 (shaft): y: 5.20 -> 11.50 (长 6.3px, hw=0.28)
    锯齿矿物绿尖 (tip): y: 11.50 -> 16.50 (长 5.0px, 宽 0.85px, 带错落齿尖)

贴图规范（64×64 四象限 Atlas）：
    - 矿物绿尖 (venom_tip): 亮绿 #7fb03a (RGB 127,176,58) 与暗绿 #2e4a1c (RGB 46,74,28) 斑驳
    - 细长针身 (needle_shaft): 深黑灰阴炼骨质 (RGB 48..65)
    - 细红丝线 (red_thread): 红色 #b3261e (RGB 179,38,30) 与暗部 #6e1512 (RGB 110,21,18)
    - 木质握柄 (wooden_grip): 深棕 #3b2a1e (RGB 59,42,30) 与浅棕 #6b4a30 (RGB 107,74,48) 相间环纹

支持逐部件单独导出与单件渲染（--export-parts）。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import os
import subprocess
import sys
import uuid
from pathlib import Path

import numpy as np
from PIL import Image

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "PoisonNeedle.bbmodel"
PREVIEW_OUT = Path(__file__).resolve().parents[1] / "out" / "poison_needle_preview.png"
REVIEW_PARTS_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/poison_needle/parts")

PX = 16.0
RES = 64

# ── 纵向与结构坐标规划（自底向上 y: 0.00 -> 16.50）────────
GRIP_Y0 = 0.00
GRIP_Y1 = 3.20

THREAD_Y0 = 3.20
THREAD_Y1 = 5.20

SHAFT_Y0 = 5.20
SHAFT_Y1 = 11.50

TIP_Y0 = 11.50
TIP_Y1 = 16.50

# 手持居中对齐偏移（以木质握柄 y=1.60 处为持握中心）
GRIP_CENTER_Y = 1.60
BLOCK_CENTRE_PX = 8.0
EMIT_OFFSET = (BLOCK_CENTRE_PX, BLOCK_CENTRE_PX - GRIP_CENTER_Y, BLOCK_CENTRE_PX)

# 贴图象限规划 (64x64)
MAT_ZONE = {
    "venom_tip": (0, 0, 32, 32),
    "needle_shaft": (32, 0, 64, 32),
    "red_thread": (0, 32, 32, 64),
    "wooden_grip": (32, 32, 64, 64),
}


def block(bone, mat, name, x0, x1, y0, y1, z0, z1, rot=(0.0, 0.0, 0.0)):
    fx, tx = min(x0, x1), max(x0, x1)
    fy, ty = min(y0, y1), max(y0, y1)
    fz, tz = min(z0, z1), max(z0, z1)
    return (bone, mat, name, [fx, fy, fz], [tx, ty, tz], tuple(rot))


def octagon(bone, mat, name, hw, y0, y1, hz=None):
    hz = hw if hz is None else hz
    return [
        block(bone, mat, f"{name}_0", -hw, hw, y0, y1, -hz, hz, rot=(0.0, 0.0, 0.0)),
        block(bone, mat, f"{name}_45", -hw + 0.02, hw - 0.02, y0 + 0.01, y1 - 0.01, -hz + 0.02, hz - 0.02, rot=(0.0, 45.0, 0.0)),
    ]


def part_tip() -> list[tuple]:
    """生成锯齿状矿物绿箭尖 (tip)。

    像一片细长叶子，边缘有 2~3 级错开的小齿，亮绿带暗绿斑驳；无白色块，直接接深色针身。
    """
    cubes = []
    # 1. 绿尖基部插芯（与针身顶端顺畅衔接，y: 11.50 -> 12.20, hw: 0.28）
    cubes.append(block("tip", "venom_tip", "venom_base_socket", -0.28, 0.28, 11.50, 12.20, -0.16, 0.16))

    # 2. 细长叶状矿物箭尖主叶身 (扁平菱形叶身，y: 12.20 -> 15.60, 宽 0.70px, 厚 0.18px)
    cubes.append(block("tip", "venom_tip", "venom_blade_core", -0.35, 0.35, 12.20, 15.60, -0.09, 0.09))

    # 3. 边缘 3 级交错凸出的小锯齿 (serrated mineral teeth)
    # (a) 左侧齿 1 (低位齿, y: 12.50 -> 13.20, 凸向 -X)
    cubes.append(block("tip", "venom_tip", "venom_tooth_l1", -0.62, -0.35, 12.50, 13.20, -0.07, 0.07, rot=(0.0, 0.0, -18.0)))
    # (b) 右侧齿 1 (中低位错开齿, y: 13.20 -> 13.90, 凸向 +X)
    cubes.append(block("tip", "venom_tip", "venom_tooth_r1", 0.35, 0.65, 13.20, 13.90, -0.07, 0.07, rot=(0.0, 0.0, 18.0)))
    # (c) 左侧齿 2 (高位齿, y: 13.90 -> 14.60, 凸向 -X)
    cubes.append(block("tip", "venom_tip", "venom_tooth_l2", -0.55, -0.35, 13.90, 14.60, -0.06, 0.06, rot=(0.0, 0.0, -15.0)))
    # (d) 右侧齿 2 (高位微齿, y: 14.50 -> 15.10, 凸向 +X)
    cubes.append(block("tip", "venom_tip", "venom_tooth_r2", 0.35, 0.52, 14.50, 15.10, -0.06, 0.06, rot=(0.0, 0.0, 15.0)))

    # 4. 渐收顶尖锐锋 (y: 15.60 -> 16.50, 平滑收至 0.08px 单针尖)
    cubes.append(block("tip", "venom_tip", "venom_taper_upper", -0.18, 0.18, 15.60, 16.15, -0.06, 0.06))
    cubes.append(block("tip", "venom_tip", "venom_needle_point", -0.05, 0.05, 16.15, 16.50, -0.04, 0.04))

    return cubes


def part_shaft() -> list[tuple]:
    """生成深黑灰暗化细长针身 (shaft)。

    深色贯穿硬质针杆，接在 tip 与缠丝之间。
    """
    cubes = []
    # 细长直针杆 (y: 5.20 -> 11.50, 长 6.3px, hw: 0.26)
    cubes.extend(octagon("shaft", "needle_shaft", "shaft_main", 0.26, SHAFT_Y0, SHAFT_Y1, hz=0.26))
    return cubes


def part_tail() -> list[tuple]:
    """生成几圈细红丝线缠绕与下垂细丝 (tail)。

    每圈 0.32~0.38px 粗，圈与圈之间露出深色针身；外加 2 根细红丝从缠绕处垂下来，长短不一。
    """
    cubes = []
    # 1. 穿过红丝缠绕内部的针身内芯（露出部在圈与圈之间，y: 3.20 -> 5.20, hw: 0.25）
    cubes.extend(octagon("tail", "needle_shaft", "tail_shaft_core", 0.25, THREAD_Y0, THREAD_Y1, hz=0.25))

    # 2. 3 圈独立缠绕的细红线圈 (每圈高 0.40px, 间隙 0.25px 露出深色骨芯)
    ring_ys = [(3.30, 3.75), (4.00, 4.45), (4.70, 5.15)]
    for i, (y0, y1) in enumerate(ring_ys):
        cubes.extend(octagon("tail", "red_thread", f"thread_ring_{i}", 0.36, y0, y1, hz=0.36))

    # 3. 2 根从缠绕处下垂的长短不一细红丝须
    # 丝须 1 (长丝，垂至 y=1.75，微调起始高度避开木柄环槽共面)
    cubes.append(block("tail", "red_thread", "thread_dangle_long", 0.34, 0.46, 1.75, 3.80, -0.06, 0.06, rot=(0.0, 0.0, -10.0)))
    # 丝须 2 (短丝，垂至 y=2.45，微调起始高度避开木柄环槽 2.40 共面)
    cubes.append(block("tail", "red_thread", "thread_dangle_short", 0.32, 0.44, 2.45, 4.20, 0.15, 0.27, rot=(12.0, 0.0, -6.0)))

    return cubes


def part_grip() -> list[tuple]:
    """生成木质握柄与底端小收口 (grip)。

    位于红丝线下方，比针身粗一圈 (hw: 0.46)，深棕与浅棕相间的横向环纹（缠绳/刻槽），底端一个深色小收口。
    """
    cubes = []
    # 1. 内部木柄轴芯 (y: 0.35 -> 3.20, hw: 0.42)
    cubes.extend(octagon("grip", "wooden_grip", "grip_wood_core", 0.42, 0.35, GRIP_Y1, hz=0.42))

    # 2. 3 圈凸起的横向刻槽环纹 (深棕与浅棕相间, hw: 0.50)
    groove_ys = [(0.50, 1.15), (1.45, 2.10), (2.40, 3.05)]
    for i, (y0, y1) in enumerate(groove_ys):
        cubes.extend(octagon("grip", "wooden_grip", f"grip_band_{i}", 0.50, y0, y1, hz=0.50))

    # 3. 底端深色小收口端头 (y: 0.00 -> 0.35, hw: 0.38)
    cubes.extend(octagon("grip", "wooden_grip", "grip_butt_cap", 0.38, 0.00, 0.35, hz=0.38))

    return cubes


def all_cubes() -> list[tuple]:
    """汇总飞针所有部件。"""
    cubes = []
    cubes.extend(part_tip())
    cubes.extend(part_shaft())
    cubes.extend(part_tail())
    cubes.extend(part_grip())
    return cubes


def _assert_no_coplanar_faces(cubes: list[tuple]) -> None:
    """门禁：校验各立方体之间是否存在重叠或共面 Z-Fighting。"""
    n = len(cubes)
    tol = 1e-4
    for i in range(n):
        c1 = cubes[i]
        b1_min = c1[3]
        b1_max = c1[4]
        for j in range(i + 1, n):
            c2 = cubes[j]
            b2_min = c2[3]
            b2_max = c2[4]

            overlap_x = min(b1_max[0], b2_max[0]) - max(b1_min[0], b2_min[0])
            overlap_y = min(b1_max[1], b2_max[1]) - max(b1_min[1], b2_min[1])
            overlap_z = min(b1_max[2], b2_max[2]) - max(b1_min[2], b2_min[2])

            if overlap_x > tol and overlap_y > tol and overlap_z > tol:
                for axis, name in [(0, "X"), (1, "Y"), (2, "Z")]:
                    if abs(b1_min[axis] - b2_min[axis]) < tol:
                        raise ValueError(f"共面冲突: {c1[2]} 与 {c2[2]} 在 -{name} 面共面 ({b1_min[axis]:.4f})")
                    if abs(b1_max[axis] - b2_max[axis]) < tol:
                        raise ValueError(f"共面冲突: {c1[2]} 与 {c2[2]} 在 +{name} 面共面 ({b1_max[axis]:.4f})")


def make_texture_atlas() -> Image.Image:
    """生成 64x64 四象限毒蛊飞针 Texture Atlas。"""
    img = Image.new("RGBA", (RES, RES), (0, 0, 0, 0))
    rng = np.random.default_rng(42)

    # 1. 锯齿矿物绿尖 (Q1: 0..32, 0..32)
    # 亮绿 #7fb03a (RGB 127,176,58) 与暗绿 #2e4a1c (RGB 46,74,28) 斑驳，无白块
    tip_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            t = (x * 3 + y * 5) % 17 / 17.0
            r = int(46 + (127 - 46) * t + rng.integers(-6, 7))
            g = int(74 + (176 - 74) * t + rng.integers(-8, 9))
            b = int(28 + (58 - 28) * t + rng.integers(-5, 6))
            tip_arr[y, x] = [int(np.clip(r, 0, 255)), int(np.clip(g, 0, 255)), int(np.clip(b, 0, 255)), 255]
    img.paste(Image.fromarray(tip_arr, "RGBA"), (0, 0))

    # 2. 细长针身 (Q2: 32..64, 0..32) - 深黑灰暗化骨质
    shaft_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            base_val = rng.integers(48, 65)
            if (x + y) % 4 == 0:
                base_val -= 10
            shaft_arr[y, x] = [base_val, int(np.clip(base_val - 2, 0, 255)), int(np.clip(base_val - 4, 0, 255)), 255]
    img.paste(Image.fromarray(shaft_arr, "RGBA"), (32, 0))

    # 3. 细红丝线 (Q3: 0..32, 32..64)
    # 红色 #b3261e (RGB 179,38,30) 与暗部 #6e1512 (RGB 110,21,18)
    thread_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            t = (y % 4) / 4.0
            r = int(110 + (179 - 110) * t + rng.integers(-8, 9))
            g = int(21 + (38 - 21) * t + rng.integers(-4, 5))
            b = int(18 + (30 - 18) * t + rng.integers(-3, 4))
            thread_arr[y, x] = [int(np.clip(r, 0, 255)), int(np.clip(g, 0, 255)), int(np.clip(b, 0, 255)), 255]
    img.paste(Image.fromarray(thread_arr, "RGBA"), (0, 32))

    # 4. 木质握柄 (Q4: 32..64, 32..64)
    # 深棕 #3b2a1e (RGB 59,42,30) 与浅棕 #6b4a30 (RGB 107,74,48) 相间环纹
    grip_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            if (y % 6) < 3:
                # 浅棕色环纹
                r = rng.integers(100, 115)
                g = rng.integers(68, 80)
                b = rng.integers(42, 54)
            else:
                # 深棕色沟槽
                r = rng.integers(54, 66)
                g = rng.integers(38, 48)
                b = rng.integers(26, 36)
            grip_arr[y, x] = [r, g, b, 255]
    img.paste(Image.fromarray(grip_arr, "RGBA"), (32, 32))

    return img


def build_bbmodel(cubes: list[tuple], tex_img: Image.Image, model_name: str = "PoisonNeedle") -> dict:
    """组装符合 Blockbench 格式的 .bbmodel JSON 数据。"""
    buf = io.BytesIO()
    tex_img.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    tex_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": "poison_needle",
        "folder": "item",
        "namespace": "bong",
        "id": "0",
        "particle": False,
        "render_mode": "default",
        "visible": True,
        "mode": "bitmap",
        "saved": True,
        "uuid": tex_uuid,
        "source": tex_b64,
        "width": 64,
        "height": 64,
    }

    elements = []
    groups_map: dict[str, list[str]] = {}

    for idx, (bone_name, mat_key, cube_name, f_pos, t_pos, rot) in enumerate(cubes):
        elem_uuid = str(uuid.uuid4())
        groups_map.setdefault(bone_name, []).append(elem_uuid)

        from_coord = [
            round(f_pos[0] + EMIT_OFFSET[0], 4),
            round(f_pos[1] + EMIT_OFFSET[1], 4),
            round(f_pos[2] + EMIT_OFFSET[2], 4),
        ]
        to_coord = [
            round(t_pos[0] + EMIT_OFFSET[0], 4),
            round(t_pos[1] + EMIT_OFFSET[1], 4),
            round(t_pos[2] + EMIT_OFFSET[2], 4),
        ]

        zx0, zy0, zx1, zy1 = MAT_ZONE[mat_key]
        span_x = max(1, zx1 - zx0 - 4)
        span_y = max(1, zy1 - zy0 - 4)
        uv_u0 = zx0 + (idx * 2) % span_x
        uv_v0 = zy0 + (idx * 3) % span_y
        uv_box = [float(uv_u0), float(uv_v0), float(uv_u0 + 4), float(uv_v0 + 4)]

        faces = {
            face_name: {
                "uv": uv_box,
                "texture": 0,
            }
            for face_name in ("north", "east", "south", "west", "up", "down")
        }

        element = {
            "name": cube_name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": from_coord,
            "to": to_coord,
            "autouv": 0,
            "color": 0,
            "origin": [BLOCK_CENTRE_PX, BLOCK_CENTRE_PX, BLOCK_CENTRE_PX],
            "faces": faces,
            "type": "cube",
            "uuid": elem_uuid,
        }
        if any(abs(r) > 1e-4 for r in rot):
            origin = [
                (from_coord[0] + to_coord[0]) / 2.0,
                (from_coord[1] + to_coord[1]) / 2.0,
                (from_coord[2] + to_coord[2]) / 2.0,
            ]
            element["origin"] = origin
            element["rotation"] = list(rot)

        elements.append(element)

    out_groups = []
    for g_name in ["tip", "shaft", "tail", "grip"]:
        if g_name in groups_map:
            out_groups.append({
                "name": g_name,
                "origin": [BLOCK_CENTRE_PX, BLOCK_CENTRE_PX, BLOCK_CENTRE_PX],
                "color": 0,
                "uuid": str(uuid.uuid4()),
                "isOpen": True,
                "children": groups_map[g_name],
            })

    bb_data = {
        "meta": {
            "format_version": "4.8",
            "model_format": "free",
            "box_uv": False,
        },
        "name": model_name,
        "model_identifier": "poison_needle",
        "visible_box": [1, 1, 0],
        "geometry_name": "poison_needle",
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": out_groups,
        "textures": [texture_entry],
    }
    return bb_data


def generate() -> Path:
    """执行标准生成流程。"""
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)

    BBMODEL_OUT.parent.mkdir(parents=True, exist_ok=True)
    PREVIEW_OUT.parent.mkdir(parents=True, exist_ok=True)

    tex = make_texture_atlas()
    bb_json = build_bbmodel(cubes, tex)

    BBMODEL_OUT.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"✓ 已输出 Blockbench 模型: {BBMODEL_OUT}")
    return BBMODEL_OUT


def export_parts() -> None:
    """逐部件打磨导出并单件渲染，存入 model-review/img/poison_needle/parts/<name>.png"""
    REVIEW_PARTS_DIR.mkdir(parents=True, exist_ok=True)
    tex = make_texture_atlas()

    parts = [
        ("tip", part_tip()),
        ("shaft", part_shaft()),
        ("tail", part_tail()),
        ("grip", part_grip()),
    ]

    for part_name, part_cubes in parts:
        _assert_no_coplanar_faces(part_cubes)
        tmp_bb = REPO / "modelScript" / "out" / f"tmp_PoisonNeedle_part_{part_name}.bbmodel"
        bb_json = build_bbmodel(part_cubes, tex, model_name=f"PoisonNeedle_part_{part_name}")
        tmp_bb.write_text(json.dumps(bb_json, indent=2, ensure_ascii=False), encoding="utf-8")

        render_cmd = ["bbmodel-render", str(tmp_bb), "--three-view"]
        subprocess.run(render_cmd, check=True)

        render_out = REPO / "modelScript" / "out" / f"render_tmp_PoisonNeedle_part_{part_name}_three_view.png"
        if not render_out.exists():
            render_out = Path.home() / "modelScript" / "out" / f"render_tmp_PoisonNeedle_part_{part_name}_three_view.png"
        target_png = REVIEW_PARTS_DIR / f"{part_name}.png"
        if render_out.exists():
            target_png.write_bytes(render_out.read_bytes())
            print(f"✓ 部件单件三视图已落盘: {target_png}")
        else:
            print(f"⚠ 未找到渲染产物: {render_out}")


def self_test() -> None:
    """门禁差分自证：注入共面冲突，验证 _assert_no_coplanar_faces 能准确拦截。"""
    print("运行 gen_poison_needle.py 差分自证...")
    clean_cubes = all_cubes()
    _assert_no_coplanar_faces(clean_cubes)
    print("  [OK] 正常立方体集无共面冲突")

    bad_cubes = list(clean_cubes)
    c0 = bad_cubes[0]
    bad_cube = ("tip", "venom_tip", "inject_coplanar", (c0[3][0], c0[3][1], c0[3][2]), (c0[4][0], c0[4][1], c0[4][2]), (0, 0, 0))
    bad_cubes.append(bad_cube)

    caught = False
    try:
        _assert_no_coplanar_faces(bad_cubes)
    except ValueError as e:
        if "共面冲突" in str(e):
            caught = True
            print(f"  [OK] 成功捕获注入缺陷: {e}")

    if not caught:
        raise AssertionError("差分自证失败：未能拦截注入的共面缺陷！")
    print("✓ gen_poison_needle.py 差分自证全绿")


def main() -> None:
    parser = argparse.ArgumentParser(description="生成 PoisonNeedle.bbmodel 并支持逐部件导出")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--export-parts", action="store_true", help="逐部件打磨导出并单件渲染")
    args = parser.parse_args()

    if args.self_test:
        self_test()
    elif args.export_parts:
        export_parts()
    else:
        generate()


if __name__ == "__main__":
    main()
