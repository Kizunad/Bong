#!/usr/bin/env python3
"""毒蛊飞针 (poison_needle / PoisonNeedle) Blockbench .bbmodel 生成器。

严格依据 AI 参考图（概念图、物品图标、三视图与爆炸分解图）形制建模：
- 末法残土幽微暗器写实画风：细长暗化淬毒硬骨长针体，针尖浸渍墨绿/深黑侵蚀性毒斑，针尾收口处紧密缠绕一圈微小暗赤色丝线与微凸线结。
- 暗器捏持比例：纯灰模特右手两指轻捏，体素尺寸紧凑纤细，全长 14.5px。

分 3 大 Group（Bone 骨骼节点）：
    1. shaft  - 细长暗化骨针身（needle_shaft / 深灰暗化兽骨直身，带有细微骨髓纹与微糙质感）
    2. tip    - 针尖淬蚀浸渍区（venom_tip / 前端渐收锐针与墨绿黑腐蚀性浸渍毒芒）
    3. tail   - 赤丝尾缠与线结（red_thread_tail / 针尾收口处缠绕的暗赤丝线与微突线结）

尺寸规范（MC px，16px = 1 格）：
    全长约 14.5px ≈ 0.91 格（暗器飞针尺度）。
    针身直径 Ø0.48~0.60px (hw=0.24..0.30)。
    针尾赤丝缠裹 Ø0.72px (hw=0.36)。
    针尖逐渐锐化收细至 0.08px。

贴图规范（64×64 四象限 Atlas）：
    - 细长针身 (needle_bone): 深黑灰阴炼骨质 (RGB 50..70)
    - 淬毒针尖 (venom_green): 墨绿暗黑侵蚀毒斑与尖端惨白冷光 (RGB 25..55, 绿 60..90)
    - 针尾赤丝 (red_thread): 暗赤红色粗丝线 (RGB 145..175, 30..45, 25..40)

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

# ── 纵向与结构坐标规划（y: 针尾=0.0，针尖位于 y=14.50）────────
TAIL_Y0 = 0.00
TAIL_Y1 = 2.20               # 尾部赤丝缠线区 (长 2.2px)

SHAFT_Y0 = 2.20
SHAFT_Y1 = 9.50              # 针身主段 (长 7.3px)

TIP_Y0 = 9.50
TIP_Y1 = 14.50               # 淬毒收尖区 (长 5.0px)

# 手持居中对齐偏移（以两指捏持针身中下段 y=4.50 处为捏持中心）
PINCH_CENTER_Y = 4.50
BLOCK_CENTRE_PX = 8.0
EMIT_OFFSET = (BLOCK_CENTRE_PX, BLOCK_CENTRE_PX - PINCH_CENTER_Y, BLOCK_CENTRE_PX)

# 贴图象限规划 (64x64)
MAT_ZONE = {
    "needle_bone": (0, 0, 32, 32),
    "venom_green": (32, 0, 64, 32),
    "red_thread": (0, 32, 32, 64),
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


def part_tail() -> list[tuple]:
    """生成赤丝尾缠与线结 (tail)。

    针尾收口处紧密缠绕一圈微小暗赤色丝线与两指捏持微突线结。
    """
    cubes = []
    # 1. 针尾内部微缩骨核 (y: 0.10 -> 2.20, hw: 0.22)
    cubes.extend(octagon("tail", "needle_bone", "tail_core", 0.22, 0.10, TAIL_Y1, hz=0.22))

    # 2. 针尾平整外露骨底端 (y: 0.00 -> 0.10, hw: 0.25)
    cubes.extend(octagon("tail", "needle_bone", "tail_base", 0.25, TAIL_Y0, 0.10, hz=0.25))

    # 3. 针尾紧密密缠的暗赤丝线带 (y: 0.40 -> 2.00, hw: 0.36)
    cubes.extend(octagon("tail", "red_thread", "thread_wrap_main", 0.36, 0.40, 2.00, hz=0.36))

    # 4. 微小下垂赤丝线结与须尾 (y: 0.20 -> 0.90, 微偏向一侧)
    cubes.append(block("tail", "red_thread", "thread_knot_bead", 0.25, 0.52, 0.50, 0.95, -0.15, 0.15))
    cubes.append(block("tail", "red_thread", "thread_tail_fringe", 0.32, 0.48, 0.15, 0.50, -0.08, 0.08, rot=(0.0, 0.0, -12.0)))

    return cubes


def part_shaft() -> list[tuple]:
    """生成细长暗化针身 (shaft)。

    深灰暗化硬兽骨直身，带有细微骨髓纹与微糙微光。
    """
    cubes = []
    # 细长直针体分 3 段平滑圆柱微过渡 (y: 2.20 -> 9.50)
    # (a) 下段微粗承力段 (y: 2.20 -> 4.80, hw: 0.30)
    cubes.extend(octagon("shaft", "needle_bone", "shaft_lower", 0.30, TAIL_Y1, 4.80, hz=0.30))

    # (b) 中段轻巧捏持段 (y: 4.80 -> 7.20, hw: 0.28)
    cubes.extend(octagon("shaft", "needle_bone", "shaft_mid", 0.28, 4.80, 7.20, hz=0.28))

    # (c) 上段平滑渐收段 (y: 7.20 -> 9.50, hw: 0.25)
    cubes.extend(octagon("shaft", "needle_bone", "shaft_upper", 0.25, 7.20, TIP_Y0, hz=0.25))

    return cubes


def part_tip() -> list[tuple]:
    """生成针尖淬蚀浸渍区 (tip)。

    针尖前端约 1/3 (y: 9.50 -> 14.50) 浸渍墨绿/深黑渐变色泽，4 级平滑收拢至极锐利飞针尖芒。
    """
    cubes = []
    # 1. 淬毒浸渍过渡基段 (y: 9.50 -> 11.20, hw: 0.22)
    cubes.extend(octagon("tip", "venom_green", "venom_tier_1", 0.22, TIP_Y0, 11.20, hz=0.22))

    # 2. 淬蚀渐收段 (y: 11.20 -> 12.60, hw: 0.17)
    cubes.extend(octagon("tip", "venom_green", "venom_tier_2", 0.17, 11.20, 12.60, hz=0.17))

    # 3. 极细锐化段 (y: 12.60 -> 13.70, hw: 0.12)
    cubes.extend(octagon("tip", "venom_green", "venom_tier_3", 0.12, 12.60, 13.70, hz=0.12))

    # 4. 极致破甲穿刺单针尖芒 (y: 13.70 -> 14.50, 宽 0.08px, 极锐利冷光)
    cubes.append(block("tip", "venom_green", "venom_needle_point", -0.04, 0.04, 13.70, 14.50, -0.04, 0.04))

    return cubes


def all_cubes() -> list[tuple]:
    """汇总飞针所有部件。"""
    cubes = []
    cubes.extend(part_tail())
    cubes.extend(part_shaft())
    cubes.extend(part_tip())
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

    # 1. 细长针身 (Q1: 0..32, 0..32) - 深黑灰暗化骨质
    shaft_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            base_r = rng.integers(50, 70)
            base_g = rng.integers(48, 66)
            base_b = rng.integers(46, 62)
            if (x + y) % 4 == 0:
                base_r -= 10
                base_g -= 10
                base_b -= 10
            shaft_arr[y, x] = [int(np.clip(base_r, 0, 255)), int(np.clip(base_g, 0, 255)), int(np.clip(base_b, 0, 255)), 255]
    img.paste(Image.fromarray(shaft_arr, "RGBA"), (0, 0))

    # 2. 淬毒针尖 (Q2: 32..64, 0..32) - 墨绿暗黑侵蚀毒斑与冷光尖芒
    tip_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            if y <= 6:
                # 极细锋芒尖刃 (冷白惨白高亮 RGB 225..248)
                base_r = rng.integers(215, 235)
                base_g = rng.integers(235, 252)
                base_b = rng.integers(220, 240)
            else:
                # 墨绿与深黑渐变腐蚀毒液浸渍
                t = (y - 6) / 26.0
                base_r = int(25 + 30 * t + rng.integers(-4, 5))
                base_g = int(78 - 30 * t + rng.integers(-5, 6))
                base_b = int(35 + 20 * t + rng.integers(-4, 5))
            tip_arr[y, x] = [int(np.clip(base_r, 0, 255)), int(np.clip(base_g, 0, 255)), int(np.clip(base_b, 0, 255)), 255]
    img.paste(Image.fromarray(tip_arr, "RGBA"), (32, 0))

    # 3. 针尾赤丝 (Q3: 0..32, 32..64) - 暗赤红粗丝线
    thread_arr = np.zeros((32, 32, 4), dtype=np.uint8)
    for y in range(32):
        for x in range(32):
            stripe = (y % 3) * 8
            base_r = rng.integers(150, 175) - stripe
            base_g = rng.integers(32, 48) - (stripe // 2)
            base_b = rng.integers(26, 40) - (stripe // 2)
            thread_arr[y, x] = [int(np.clip(base_r, 50, 255)), int(np.clip(base_g, 10, 200)), int(np.clip(base_b, 10, 180)), 255]
    img.paste(Image.fromarray(thread_arr, "RGBA"), (0, 32))

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
    for g_name in ["shaft", "tip", "tail"]:
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
        ("shaft", part_shaft()),
        ("tip", part_tip()),
        ("tail", part_tail()),
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
    bad_cube = ("tail", "needle_bone", "inject_coplanar", (c0[3][0], c0[3][1], c0[3][2]), (c0[4][0], c0[4][1], c0[4][2]), (0, 0, 0))
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
