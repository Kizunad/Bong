#!/usr/bin/env python3
"""末法残土渊空畸变体 (Void Distorted / void_distorted) Blockbench .bbmodel 程序化生成器。

严格按 AI 参考图与用户审定特征清单建模，分部件与骨骼层级规划：
- asymmetric_carapace: 非对称畸变开裂的异化兽躯背甲（左侧骨棘增生，右侧撕裂肌理）
- warped_split_head: 扭曲开裂的半骨半肉异化头颅与外翻咬合獠牙
- dominant_claw_arm: 左侧粗长强壮、深扎地面的异变主支撑巨爪前肢
- atrophied_bone_limb: 右侧萎缩折叠、带有尖锐短棘的残缺异化副肢
- ragged_hind_paws: 低伏发力、紧扣地面的粗壮兽足后肢

门禁与自检：
  - _assert_no_coplanar_faces 检查共面冲突
  - --self-test 注入缺陷自证门禁有效性
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import sys
import uuid
from pathlib import Path
from typing import Dict, List, Tuple

import numpy as np
from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[2]
BBMODEL_OUT = Path(__file__).resolve().parents[1] / "models" / "VoidDistorted.bbmodel"
PREVIEW_OUT = Path(__file__).resolve().parents[1] / "out" / "void_distorted_preview.png"

RES = 64

# ── 调色板 ──
VOID_FLESH  = [54, 46, 62]
BONE_CREST  = [180, 172, 160]
CHITIN_DARK = [36, 34, 40]
CLAW_IRON   = [24, 22, 28]


def part_carapace() -> List[dict]:
    """非对称异变兽躯主甲。"""
    cubes = []
    # 主躯干 (x: -5.0 到 5.0, y: 5.0 到 13.0, z: -7.0 到 5.0)
    cubes.append({
        "name": "carapace_asym_main_torso",
        "from": [-5.0, 5.0, -7.0],
        "to": [5.0, 13.0, 5.0],
        "group": "body",
        "material": "void_flesh",
    })
    # 左侧突增硬化骨板 (高突起)
    cubes.append({
        "name": "carapace_asym_left_bone_hump",
        "from": [-5.8, 11.5, -4.5],
        "to": [-1.04, 16.5, 3.5],
        "group": "body",
        "material": "bone_crest",
    })
    # 右侧撕裂肌理低平背
    cubes.append({
        "name": "carapace_asym_right_flesh_shelf",
        "from": [1.04, 9.5, -6.0],
        "to": [5.6, 14.0, 2.0],
        "group": "body",
        "material": "chitin_dark",
    })
    return cubes


def part_head() -> List[dict]:
    """半颅骨半虚空裂口的畸变头颅。"""
    cubes = []
    # 头部主脑颅 (x: -3.5 到 3.5, y: 6.0 到 13.0, z: 5.04 到 11.0)
    cubes.append({
        "name": "head_split_cranium",
        "from": [-3.5, 6.0, 5.04],
        "to": [3.5, 13.0, 11.0],
        "group": "head",
        "material": "void_flesh",
    })
    # 左侧外突白骨面罩残壳
    cubes.append({
        "name": "head_split_left_crest",
        "from": [-4.2, 8.5, 7.5],
        "to": [-1.04, 14.5, 12.0],
        "group": "head",
        "material": "bone_crest",
    })
    # 右侧下斜撕裂外翻獠牙
    cubes.append({
        "name": "head_split_right_tusks",
        "from": [1.04, 4.2, 8.5],
        "to": [3.8, 8.0, 13.0],
        "group": "head",
        "material": "claw_iron",
    })
    return cubes


def part_arms() -> List[dict]:
    """非对称双前肢：左侧主支撑巨爪，右侧萎缩骨肢。"""
    cubes = []
    # 左侧粗长支撑主臂 (上臂、粗小臂、尖锐黑铁长爪)
    cubes.append({
        "name": "arm_dominant_left_shoulder",
        "from": [-8.2, 8.0, 1.0],
        "to": [-5.04, 14.0, 5.5],
        "group": "left_arm",
        "material": "chitin_dark",
    })
    cubes.append({
        "name": "arm_dominant_left_forearm",
        "from": [-8.0, 0.0, 3.5],
        "to": [-5.2, 8.04, 7.5],
        "group": "left_arm",
        "material": "void_flesh",
    })
    cubes.append({
        "name": "arm_dominant_left_talons",
        "from": [-8.4, 0.0, 7.54],
        "to": [-4.8, 3.2, 11.0],
        "group": "left_arm",
        "material": "claw_iron",
    })

    # 右侧萎缩副肢 (悬空短小，带有骨刺)
    cubes.append({
        "name": "limb_atrophied_right_upper",
        "from": [5.04, 7.5, 1.5],
        "to": [7.4, 12.0, 4.5],
        "group": "right_arm",
        "material": "void_flesh",
    })
    cubes.append({
        "name": "limb_atrophied_right_hook",
        "from": [5.4, 5.0, 4.0],
        "to": [7.2, 8.04, 6.5],
        "group": "right_arm",
        "material": "bone_crest",
    })
    return cubes


def part_hind_legs() -> List[dict]:
    """低伏发力的粗壮兽足后肢。"""
    cubes = []
    # 左后腿
    cubes.append({
        "name": "hind_paw_left_thigh",
        "from": [-7.5, 4.5, -6.5],
        "to": [-5.04, 10.5, -1.5],
        "group": "left_leg",
        "material": "chitin_dark",
    })
    cubes.append({
        "name": "hind_paw_left_foot",
        "from": [-7.2, 0.0, -5.5],
        "to": [-5.2, 4.54, -0.5],
        "group": "left_leg",
        "material": "void_flesh",
    })

    # 右后腿
    cubes.append({
        "name": "hind_paw_right_thigh",
        "from": [5.04, 4.5, -6.5],
        "to": [7.5, 10.5, -1.5],
        "group": "right_leg",
        "material": "chitin_dark",
    })
    cubes.append({
        "name": "hind_paw_right_foot",
        "from": [5.2, 0.0, -5.5],
        "to": [7.2, 4.54, -0.5],
        "group": "right_leg",
        "material": "void_flesh",
    })
    return cubes


def all_cubes() -> List[dict]:
    cubes = []
    cubes.extend(part_carapace())
    cubes.extend(part_head())
    cubes.extend(part_arms())
    cubes.extend(part_hind_legs())
    return cubes


def _assert_no_coplanar_faces(cubes: List[dict]) -> None:
    """检查立方体集是否存在共面 Z-Fighting 冲突。"""
    faces = {"+X": [], "-X": [], "+Y": [], "-Y": [], "+Z": [], "-Z": []}
    for c in cubes:
        f = c["from"]
        t = c["to"]
        name = c["name"]
        faces["-X"].append((f[0], (f[1], t[1], f[2], t[2]), name))
        faces["+X"].append((t[0], (f[1], t[1], f[2], t[2]), name))
        faces["-Y"].append((f[1], (f[0], t[0], f[2], t[2]), name))
        faces["+Y"].append((t[1], (f[0], t[0], f[2], t[2]), name))
        faces["-Z"].append((f[2], (f[0], t[0], f[1], t[1]), name))
        faces["+Z"].append((t[2], (f[0], t[0], f[1], t[1]), name))

    for axis, plane_list in faces.items():
        for i in range(len(plane_list)):
            pos_i, (u0_i, u1_i, v0_i, v1_i), name_i = plane_list[i]
            for j in range(i + 1, len(plane_list)):
                pos_j, (u0_j, u1_j, v0_j, v1_j), name_j = plane_list[j]
                if abs(pos_i - pos_j) < 1e-4:
                    overlap_u = max(0.0, min(u1_i, u1_j) - max(u0_i, u0_j))
                    overlap_v = max(0.0, min(v1_i, v1_j) - max(v0_i, v0_j))
                    if overlap_u > 1e-3 and overlap_v > 1e-3:
                        raise ValueError(
                            f"共面冲突: {name_i} 与 {name_j} 在 {axis} 面共面 ({pos_i:.4f}), "
                            f"重叠区域 ({overlap_u:.3f}x{overlap_v:.3f})"
                        )


def build_texture(res: int = 64) -> Image.Image:
    """合成异化紫黑肉、增生白骨与暗黑坚爪的 64x64 贴图。"""
    im = Image.new("RGBA", (res, res), tuple(VOID_FLESH + [255]))
    rng = np.random.default_rng(808)

    for y in range(res):
        for x in range(res):
            noise = rng.uniform(-14, 14)
            col = [int(np.clip(c + noise, 0, 255)) for c in VOID_FLESH]
            im.putpixel((x, y), tuple(col + [255]))

    # [0:32, 32:64] 增生骨刺苍白色
    for y in range(32, 64):
        for x in range(0, 32):
            noise = rng.uniform(-10, 10)
            col = [int(np.clip(c + noise, 0, 255)) for c in BONE_CREST]
            im.putpixel((x, y), tuple(col + [255]))

    # [32:64, 32:64] 尖爪黑铁深色
    for y in range(32, 64):
        for x in range(32, 64):
            noise = rng.uniform(-6, 6)
            col = [int(np.clip(c + noise, 0, 255)) for c in CLAW_IRON]
            im.putpixel((x, y), tuple(col + [255]))

    # [32:64, 0:32] 深黑死皮几丁色
    for y in range(0, 32):
        for x in range(32, 64):
            noise = rng.uniform(-12, 12)
            col = [int(np.clip(c + noise, 0, 255)) for c in CHITIN_DARK]
            im.putpixel((x, y), tuple(col + [255]))

    return im


def generate_bbmodel(out_path: Path = BBMODEL_OUT) -> Path:
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)

    tex = build_texture(RES)
    buf = io.BytesIO()
    tex.save(buf, format="PNG")
    tex_base64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")

    elements = []
    for c in cubes:
        f = c["from"]
        t = c["to"]
        mat = c.get("material", "void_flesh")
        if mat == "bone_crest":
            uv_base = [0, 32, 16, 48]
        elif mat == "claw_iron":
            uv_base = [32, 32, 48, 48]
        elif mat == "chitin_dark":
            uv_base = [32, 0, 48, 16]
        else:
            uv_base = [0, 0, 16, 16]

        faces = {}
        for side in ["north", "south", "east", "west", "up", "down"]:
            faces[side] = {"uv": uv_base, "texture": 0}

        elements.append({
            "name": c["name"],
            "box_uv": False,
            "from": f,
            "to": t,
            "faces": faces,
            "uuid": str(uuid.uuid4()),
        })

    head_uuids = [e["uuid"] for i, e in enumerate(elements) if cubes[i].get("group") == "head"]
    body_uuids = [e["uuid"] for i, e in enumerate(elements) if cubes[i].get("group") == "body"]
    larm_uuids = [e["uuid"] for i, e in enumerate(elements) if cubes[i].get("group") == "left_arm"]
    rarm_uuids = [e["uuid"] for i, e in enumerate(elements) if cubes[i].get("group") == "right_arm"]
    lleg_uuids = [e["uuid"] for i, e in enumerate(elements) if cubes[i].get("group") == "left_leg"]
    rleg_uuids = [e["uuid"] for i, e in enumerate(elements) if cubes[i].get("group") == "right_leg"]

    bbmodel = {
        "meta": {"format_version": "4.10", "model_format": "free"},
        "name": "void_distorted",
        "resolution": {"width": RES, "height": RES},
        "elements": elements,
        "outliner": [
            {
                "name": "body",
                "origin": [0.0, 8.0, 0.0],
                "children": body_uuids,
            },
            {
                "name": "head",
                "origin": [0.0, 9.0, 5.0],
                "children": head_uuids,
            },
            {
                "name": "left_arm",
                "origin": [-6.0, 11.0, 3.0],
                "children": larm_uuids,
            },
            {
                "name": "right_arm",
                "origin": [6.0, 10.0, 3.0],
                "children": rarm_uuids,
            },
            {
                "name": "left_leg",
                "origin": [-6.0, 7.0, -4.0],
                "children": lleg_uuids,
            },
            {
                "name": "right_leg",
                "origin": [6.0, 7.0, -4.0],
                "children": rleg_uuids,
            },
        ],
        "textures": [
            {
                "name": "void_distorted",
                "folder": "entity",
                "namespace": "bong",
                "id": 0,
                "source": tex_base64,
            }
        ],
    }

    out_path.write_text(json.dumps(bbmodel, indent=2), encoding="utf-8")
    print(f"✓ VoidDistorted bbmodel 写入成功: {out_path.relative_to(REPO)}")
    return out_path


def self_test() -> None:
    print("运行 gen_void_distorted.py 差分自证...")
    cubes = all_cubes()
    _assert_no_coplanar_faces(cubes)
    print("  [OK] 正常立方体集无共面冲突")

    # 缺陷注入：构造与主躯干顶面 (+Y=13.0) 精确共面的假部件
    bad_cubes = list(cubes)
    bad_cubes.append({
        "name": "inject_coplanar_fail",
        "from": [-2.0, 10.0, -2.0],
        "to": [2.0, 13.0, 2.0],  # +Y 落在 13.0，与 carapace_asym_main_torso +Y 面共面
        "group": "body",
    })
    try:
        _assert_no_coplanar_faces(bad_cubes)
        raise RuntimeError("FAIL: 缺陷注入未被共面检查拦截！")
    except ValueError as e:
        print(f"  [OK] 成功捕获注入缺陷: {e}")

    print("✓ gen_void_distorted.py 差分自证全绿")


def main() -> None:
    parser = argparse.ArgumentParser(description="生成 void_distorted.bbmodel 资产")
    parser.add_argument("--self-test", action="store_true", help="执行门禁缺陷注入自测")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return

    generate_bbmodel()


if __name__ == "__main__":
    main()
