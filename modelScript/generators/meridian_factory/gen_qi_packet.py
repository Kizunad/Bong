#!/usr/bin/env python3
"""经脉工厂内景载荷生成器 —— p01: qi_packet (真元载荷包)

风格：A 有机型 (活体血肉、半透明筋管/经脉、旧损暗淡配色)
规范出处：
- /home/serverkizuna/Code/Bong/.agent-worktrees/.task-meridian-models.md
- /home/serverkizuna/Code/Bong/.agent-worktrees/model-review/meridian_factory.md

调度审第 1 次更正说明落实：
1. 实心 4×4×4 方块，不开口：
   - 基础外包严格为 4×4×4 px (x in [-2, 2], y in [0, 4], z in [-2, 2])，
     原点位于底面中心 (0.0, 0.0, 0.0)；
   - 内部完全实心，绝非开口空心杯。
2. 四条竖棱各切 0.5px 的八棱柱特征完整保留：
   - 四个竖直边缘削去 0.5×0.5px，形成精致八角截面微晶体。
3. 六面中央 2×2 面心色方片（营造强烈内发光感）：
   - 在立方体的全部 6 个外表面（上、下、前、后、左、右）中央，各贴一块 2×2 的内芯色方片（浮出 0.05px）；
   - 外层使用浅色，面心方片使用深一档颜色，产生深邃的光核向外漫射的真实内发光晶块质感。
4. 6 个变体（外层浅色 / 面心深色）：
   - qi_packet_neutral: #f2f0ea / #d8d4c8 (中性真元)
   - qi_packet_metal:   #c8ccd0 / #9aa0a8 (金真元)
   - qi_packet_wood:    #5aa060 / #3a7a40 (木真元)
   - qi_packet_water:   #4a78c0 / #2a5090 (水真元)
   - qi_packet_fire:    #c84a3a / #902a20 (火真元)
   - qi_packet_earth:   #b08a40 / #806020 (土真元)
5. 门禁要求：通过 _assert_no_coplanar_faces 严格自检（0 共面冲突），带 --self-test 差分缺陷拦截验证。
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Tuple

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
MODEL_DIR = REPO / "modelScript" / "models" / "meridian_factory"
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/qi_packet")
REVIEW_DIR_ALIAS = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/p01_qi_packet")
REF_IMAGE = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/meridian_factory/refs/p01_qi_packets.png")

# 6 个变体的外层浅色与面心深色配置 (RGBA)
VARIANTS = {
    "neutral": {
        "title": "Neutral (中性)",
        "shell": (242, 240, 234, 255), # #f2f0ea
        "core":  (216, 212, 200, 255), # #d8d4c8
    },
    "metal": {
        "title": "Metal (金)",
        "shell": (200, 204, 208, 255), # #c8ccd0
        "core":  (154, 160, 168, 255), # #9aa0a8
    },
    "wood": {
        "title": "Wood (木)",
        "shell": (90,  160, 96,  255), # #5aa060
        "core":  (58,  122, 64,  255), # #3a7a40
    },
    "water": {
        "title": "Water (水)",
        "shell": (74,  120, 192, 255), # #4a78c0
        "core":  (42,  80,  144, 255), # #2a5090
    },
    "fire": {
        "title": "Fire (火)",
        "shell": (200, 74,  58,  255), # #c84a3a
        "core":  (144, 42,  32,  255), # #902a20
    },
    "earth": {
        "title": "Earth (土)",
        "shell": (176, 138, 64,  255), # #b08a40
        "core":  (128, 96,  32,  255), # #806020
    },
}

MAT_UV = {
    "shell": [0,  0, 32, 32],
    "core":  [32, 0, 64, 32],
}

RES = 64


# =============================================================================
# 各部件几何定义 (part_* 拆分)
# =============================================================================

def part_01_solid_shell() -> List[dict]:
    """1. 实心 4×4×4 八棱柱主体（四条竖棱各切 0.5px，材质 shell）。

    完全实心，不开口。由中央 3×3 柱体与四侧凸块严密契合构成。
    """
    cubes = []
    # 中央核心主体 (截面 3x3, 高 4, x in [-1.5, 1.5], z in [-1.5, 1.5], y in [0, 4])
    cubes.append({"name": "shell_main_core", "from": [-1.5, 0.0, -1.5], "to": [1.5, 4.0, 1.5], "group": "crystal_shell", "material": "shell"})

    # 四面凸出 (长 3, 宽 0.5, 高 4, 使主轴外廓达到 4x4x4，四角削去 0.5x0.5px)
    cubes.append({"name": "shell_lobe_s", "from": [-1.5, 0.0,  1.5], "to": [1.5, 4.0,  2.0], "group": "crystal_shell", "material": "shell"})
    cubes.append({"name": "shell_lobe_n", "from": [-1.5, 0.0, -2.0], "to": [1.5, 4.0, -1.5], "group": "crystal_shell", "material": "shell"})
    cubes.append({"name": "shell_lobe_w", "from": [-2.0, 0.0, -1.5], "to": [-1.5, 4.0, 1.5], "group": "crystal_shell", "material": "shell"})
    cubes.append({"name": "shell_lobe_e", "from": [ 1.5, 0.0, -1.5], "to": [ 2.0, 4.0, 1.5], "group": "crystal_shell", "material": "shell"})

    return cubes


def part_02_face_patches() -> List[dict]:
    """2. 六面中央 2×2 面心色方片（浮出 0.05px，材质 core）。

    在六个表面中央各贴一块 2×2 px 的深一档内芯色方片，浮出 0.05px，
    呈现强烈内发光与精细微浮雕质感。
    """
    cubes = []
    # 顶面 (+Y 面心)
    cubes.append({"name": "patch_top",    "from": [-1.0, 4.0, -1.0], "to": [1.0, 4.05, 1.0], "group": "qi_core", "material": "core"})
    # 底面 (-Y 面心)
    cubes.append({"name": "patch_bottom", "from": [-1.0, -0.05, -1.0], "to": [1.0, 0.0, 1.0], "group": "qi_core", "material": "core"})
    # 前面 (+Z 面心)
    cubes.append({"name": "patch_front",  "from": [-1.0, 1.0, 2.0], "to": [1.0, 3.0, 2.05], "group": "qi_core", "material": "core"})
    # 后面 (-Z 面心)
    cubes.append({"name": "patch_back",   "from": [-1.0, 1.0, -2.05], "to": [1.0, 3.0, -2.0], "group": "qi_core", "material": "core"})
    # 左面 (-X 面心)
    cubes.append({"name": "patch_left",   "from": [-2.05, 1.0, -1.0], "to": [-2.0, 3.0, 1.0], "group": "qi_core", "material": "core"})
    # 右面 (+X 面心)
    cubes.append({"name": "patch_right",  "from": [2.0, 1.0, -1.0], "to": [2.05, 3.0, 1.0], "group": "qi_core", "material": "core"})

    return cubes


def all_cubes() -> List[dict]:
    """汇总真元载荷包全部立方体。"""
    return part_01_solid_shell() + part_02_face_patches()


# =============================================================================
# 门禁与共面冲突自检
# =============================================================================

def _assert_no_coplanar_faces(cubes: List[dict]):
    """严格检查立方体集是否存在同向同坐标且投影相交的共面冲突 (Z-fighting)。"""
    faces: Dict[Tuple[str, float], List[dict]] = {}
    for c in cubes:
        f = c["from"]
        t = c["to"]
        for side, axis, val in [
            ("-X", 0, f[0]), ("+X", 0, t[0]),
            ("-Y", 1, f[1]), ("+Y", 1, t[1]),
            ("-Z", 2, f[2]), ("+Z", 2, t[2]),
        ]:
            if axis == 0:
                rect = (min(f[1], t[1]), min(f[2], t[2]), max(f[1], t[1]), max(f[2], t[2]))
            elif axis == 1:
                rect = (min(f[0], t[0]), min(f[2], t[2]), max(f[0], t[0]), max(f[2], t[2]))
            else:
                rect = (min(f[0], t[0]), min(f[1], t[1]), max(f[0], t[0]), max(f[1], t[1]))
            key = (side, round(val, 4))
            faces.setdefault(key, []).append((c["name"], rect))

    conflicts = []
    for (side, val), entries in faces.items():
        if len(entries) < 2:
            continue
        for i in range(len(entries)):
            for j in range(i + 1, len(entries)):
                n1, r1 = entries[i]
                n2, r2 = entries[j]
                u0 = max(r1[0], r2[0])
                u1 = min(r1[2], r2[2])
                v0 = max(r1[1], r2[1])
                v1 = min(r1[3], r2[3])
                if u1 - u0 > 0.001 and v1 - v0 > 0.001:
                    conflicts.append(
                        f"共面冲突: {n1} 与 {n2} 在 {side} 面共面 ({val}), 重叠区域 ({(u1-u0):.3f}x{(v1-v0):.3f})"
                    )
    if conflicts:
        raise AssertionError("\n".join(conflicts))


# =============================================================================
# 贴图与 Blockbench 序列化
# =============================================================================

def build_texture(variant_key: str, res: int = RES) -> Image.Image:
    """生成特定变体的 64×64 RGBA 贴图。"""
    v_info = VARIANTS[variant_key]
    shell_c = v_info["shell"]
    core_c = v_info["core"]

    rng = np.random.default_rng(20261009)
    arr = np.zeros((res, res, 4), dtype=np.uint8)

    # 绘制 shell 区域 (0..32, 0..32)
    for y in range(0, 32):
        for x in range(0, 32):
            noise = rng.integers(-4, 5)
            r = int(np.clip(shell_c[0] + noise, 0, 255))
            g = int(np.clip(shell_c[1] + noise, 0, 255))
            b = int(np.clip(shell_c[2] + noise, 0, 255))
            if (x + y * 2) % 7 in (0, 1):
                r = int(np.clip(r + 6, 0, 255))
                g = int(np.clip(g + 6, 0, 255))
                b = int(np.clip(b + 6, 0, 255))
            arr[y, x] = [r, g, b, shell_c[3]]

    # 绘制 core 区域 (32..64, 0..32)
    for y in range(0, 32):
        for x in range(32, 64):
            noise = rng.integers(-3, 4)
            r = int(np.clip(core_c[0] + noise, 0, 255))
            g = int(np.clip(core_c[1] + noise, 0, 255))
            b = int(np.clip(core_c[2] + noise, 0, 255))
            if (x * 3 + y * 5) % 11 in (0, 1):
                r = int(np.clip(r + 10, 0, 255))
                g = int(np.clip(g + 10, 0, 255))
                b = int(np.clip(b + 8, 0, 255))
            arr[y, x] = [r, g, b, core_c[3]]

    return Image.fromarray(arr, "RGBA")


def build_bbmodel_doc(cubes: List[dict], tex: Image.Image, model_id: str) -> dict:
    """组装符合 Blockbench 4.10 格式的 JSON 字典。"""
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    texture_uuid = str(uuid.uuid4())
    texture_entry = {
        "name": model_id,
        "folder": "meridian_factory",
        "namespace": "bong",
        "id": "0",
        "particle": False,
        "render_mode": "default",
        "visible": True,
        "mode": "bitmap",
        "saved": True,
        "uuid": texture_uuid,
        "source": tex_b64,
        "width": 64,
        "height": 64,
    }

    elements = []
    groups_map: Dict[str, List[str]] = {}

    for idx, c in enumerate(cubes):
        elem_uuid = str(uuid.uuid4())
        g_name = c.get("group", "qi_packet")
        groups_map.setdefault(g_name, []).append(elem_uuid)

        mat_key = c.get("material", "shell")
        u0, v0, u1, v1 = MAT_UV.get(mat_key, [0, 0, 32, 32])
        span_x = max(1, u1 - u0 - 4)
        span_y = max(1, v1 - v0 - 4)
        uv_u = u0 + (idx * 3) % span_x
        uv_v = v0 + (idx * 5) % span_y
        uv_box = [float(uv_u), float(uv_v), float(uv_u + 4), float(uv_v + 4)]

        faces = {
            face_name: {"uv": uv_box, "texture": 0}
            for face_name in ("north", "east", "south", "west", "up", "down")
        }

        element = {
            "name": c["name"],
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "from": [round(float(v), 4) for v in c["from"]],
            "to":   [round(float(v), 4) for v in c["to"]],
            "autouv": 0,
            "color": 0,
            "origin": [0.0, 0.0, 0.0],
            "faces": faces,
            "type": "cube",
            "uuid": elem_uuid,
        }
        elements.append(element)

    outliner = []
    for g_name in ["crystal_shell", "qi_core"]:
        if g_name in groups_map:
            outliner.append({
                "name": g_name,
                "origin": [0.0, 0.0, 0.0],
                "color": 0,
                "uuid": str(uuid.uuid4()),
                "isOpen": True,
                "children": groups_map[g_name],
            })

    return {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": f"QiPacket_{model_id}",
        "model_identifier": model_id,
        "visible_box": [1, 1, 1],
        "geometry_name": model_id,
        "resolution": {"width": 64, "height": 64},
        "elements": elements,
        "outliner": outliner,
        "textures": [texture_entry],
    }


def generate_all_bbmodels() -> Dict[str, Path]:
    """生成 6 个变体及默认的 bbmodel 文件。"""
    MODEL_DIR.mkdir(parents=True, exist_ok=True)
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)

    result_paths = {}
    for var_key in VARIANTS.keys():
        model_id = f"qi_packet_{var_key}"
        out_path = MODEL_DIR / f"{model_id}.bbmodel"
        tex = build_texture(var_key)
        doc = build_bbmodel_doc(cubes, tex, model_id)
        out_path.write_text(json.dumps(doc, indent=2, ensure_ascii=False), encoding="utf-8")
        result_paths[var_key] = out_path

    # 生成默认主文件 qi_packet.bbmodel (等同于 neutral)
    default_out = MODEL_DIR / "qi_packet.bbmodel"
    tex_default = build_texture("neutral")
    doc_default = build_bbmodel_doc(cubes, tex_default, "qi_packet")
    default_out.write_text(json.dumps(doc_default, indent=2, ensure_ascii=False), encoding="utf-8")
    result_paths["default"] = default_out

    print(f"✓ 6 个真元包变体 + 1 个主 bbmodel 生成完成，输出目录: {MODEL_DIR}")
    return result_paths


# =============================================================================
# 审阅图像渲染 (render.png 与 check.png)
# =============================================================================

def render_views(model_paths: Dict[str, Path]):
    """输出 6 个变体横排并排放大图 render.png 与左右并排对照卡 check.png。"""
    from bbmodel_maker.render.render_bbmodel import render

    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    REVIEW_DIR_ALIAS.mkdir(parents=True, exist_ok=True)
    bg_color = (119, 119, 119)  # 严格对齐参考图中性灰

    # 1. 为每个变体渲染 3/4 等轴透视 (yaw=-35, pitch=25) 和 俯视图 (yaw=0, pitch=89.9)
    variant_order = ["neutral", "metal", "wood", "water", "fire", "earth"]
    rendered_iso = {}
    rendered_top = {}

    for k in variant_order:
        bb_p = model_paths[k]
        im_i, _ = render(bb_p, yaw=-35.0, pitch=25.0, size=380, bg=bg_color)
        im_t, _ = render(bb_p, yaw=0.0, pitch=89.9, size=380, bg=bg_color)
        rendered_iso[k] = im_i
        rendered_top[k] = im_t

    # 2. 拼装 render.png (2 行 6 列：上行 3/4 视，下行俯视)
    col_w = 380
    row_h = 380
    pad = 12
    margin_x = 24
    margin_y = 40

    total_w = margin_x * 2 + 6 * col_w + 5 * pad
    total_h = margin_y * 2 + 2 * row_h + 30 + pad

    canvas = Image.new("RGB", (total_w, total_h), (35, 36, 40))
    draw = ImageDraw.Draw(canvas)

    # 标题栏
    draw.text((margin_x, 12), "p01 qi_packet 6 Variants (Solid 4x4x4 px, 2x2 Core Patch on 6 Faces, Beveled Vertices)", fill=(230, 230, 230))

    for idx, k in enumerate(variant_order):
        v_title = VARIANTS[k]["title"]
        px = margin_x + idx * (col_w + pad)
        py_iso = margin_y + 24
        py_top = py_iso + row_h + pad

        # 贴 3/4 视
        canvas.paste(rendered_iso[k], (px, py_iso))
        draw.rectangle([px, py_iso, px + col_w, py_iso + 24], fill=(24, 25, 28))
        draw.text((px + 8, py_iso + 5), f"{v_title} 3/4", fill=(220, 220, 220))

        # 贴俯视
        canvas.paste(rendered_top[k], (px, py_top))
        draw.rectangle([px, py_top, px + col_w, py_top + 24], fill=(24, 25, 28))
        draw.text((px + 8, py_top + 5), f"{v_title} TOP", fill=(180, 200, 220))

    for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
        r_path = target_dir / "render.png"
        canvas.save(r_path)
        print(f"✓ render.png 已输出: {r_path}")

    # 3. 拼装 check.png (左参考图右 6 个变体并排渲染图)
    if REF_IMAGE.exists():
        ref_im = Image.open(REF_IMAGE).convert("RGB")
        target_h = 560
        ref_w = int(ref_im.width * target_h / ref_im.height)
        ref_scaled = ref_im.resize((ref_w, target_h), Image.Resampling.LANCZOS)

        # 右侧图：将 6 个变体的 3/4 视横排拼成一行 (各 240x240)
        pack_w = int(target_h * 0.72)
        right_sub_w = pack_w * 6 + 5 * 8
        right_cv = Image.new("RGB", (right_sub_w, target_h), (119, 119, 119))
        for idx, k in enumerate(variant_order):
            scaled_v = rendered_iso[k].resize((pack_w, pack_w), Image.Resampling.LANCZOS)
            rx = idx * (pack_w + 8)
            ry = (target_h - pack_w) // 2
            right_cv.paste(scaled_v, (rx, ry))

        # 左右并排
        total_w_check = ref_w + right_sub_w + 32
        check_cv = Image.new("RGB", (total_w_check, target_h + 36), (28, 29, 33))
        c_draw = ImageDraw.Draw(check_cv)

        check_cv.paste(ref_scaled, (12, 28))
        c_draw.text((16, 6), "REFERENCE (p01_qi_packets.png: Left Tray / Right Tray 3/4)", fill=(210, 200, 180))

        check_cv.paste(right_cv, (ref_w + 20, 28))
        c_draw.text((ref_w + 20, 6), "NOW RENDER (6 Variants Parallel: Neutral, Metal, Wood, Water, Fire, Earth)", fill=(180, 220, 210))

        for target_dir in [REVIEW_DIR, REVIEW_DIR_ALIAS]:
            c_path = target_dir / "check.png"
            check_cv.save(c_path)
            print(f"✓ check.png 并排对照图已输出: {c_path}")


def self_test():
    """运行门禁差分自证：正常立方体无共面冲突，故意注入共面冲突能准确拦截。"""
    print("运行 gen_qi_packet.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] 正常立方体集无共面冲突")

    # 注入测试缺陷
    defect_cubes = list(cubes) + [{
        "name": "inject_coplanar_fail",
        "from": [-1.5, 0.0, -1.5],
        "to":   [ 1.5, 4.0,  1.5],  # 与 shell_main_core 完全重叠
        "material": "shell",
    }]
    caught = False
    try:
        _assert_no_coplanar_faces(defect_cubes)
    except AssertionError as e:
        caught = True
        print(f"  [OK] 成功捕获注入共面缺陷: {e.args[0].splitlines()[0]}")

    if not caught:
        raise RuntimeError("门禁失效: 注入共面冲突未被拦截!")
    print("✓ gen_qi_packet.py 差分自证全绿")


def main():
    parser = argparse.ArgumentParser(description="经脉工厂内景载荷 p01 qi_packet 生成器")
    parser.add_argument("--self-test", action="store_true", help="运行门禁差分自证")
    parser.add_argument("--render", action="store_true", help="渲染并输出 render.png 与 check.png")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    # 默认流程：自测 -> 导出所有变体 bbmodel -> 渲染图片
    self_test()
    model_paths = generate_all_bbmodels()
    render_views(model_paths)


if __name__ == "__main__":
    main()
