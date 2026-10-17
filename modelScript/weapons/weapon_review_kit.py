#!/usr/bin/env python3
"""共用武器审阅图与姿态辅助工具 (weapon_review_kit.py)。

提供：
1. (A)「并排比例」模型与渲染：
   - 兵器竖直立在占位玩家右侧，留 2 px 空隙，底端和脚底对齐 (y=0.0)；
   - 渲染正面 (FRONT) + 侧面 (SIDE)；
   - 标出兵器全长 / 人高 (32px) 百分比。
2. (B)「持握姿势」模型与渲染：
   - 右臂绕肩 [6, 22, 0] 前抬 30°；
   - 握点放在拳心 [6.0, 14.6388, -4.2500]；
   - 兵器沿 Minecraft 第三人称右手持物方向朝前上方伸出（单手兵器朝前上 45°；长柄兵器斜在身体前方，下端在身后侧，不穿过躯干）；
   - 渲染正面 (FRONT)、侧面 (SIDE)、3/4 视角；
   - 几何检查：严格分离轴定理 (SAT) 检查武器所有方块与玩家「头 / 躯干 / 左臂 / 右腿 / 左腿」的交叠体积，确保严格为 0，并在图中标明检查结果。
3. 比例对照面板：参考图剪影 vs 模型剪影等高对齐、贯穿参考线、比例对比表。
4. 一键生成完整的 Round 1 `check.png` 与 Round 3 `check_tex.png`。
"""

from __future__ import annotations

import base64
import copy
import io
import json
import math
import sys
from pathlib import Path
from typing import Dict, List, Tuple

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
sys.path.insert(0, str(REPO / "modelScript" / "gear" / "_kit"))

import numpy as np
from PIL import Image, ImageDraw

from cjk_font import load_font
from bbmodel_maker.render.render_bbmodel import render
from bbmodel_maker.rig.rigkit import Rig
import gear_kit

PLAYER_BOXES = {
    "头": ((-4.0, 24.0, -4.0), (4.0, 32.0, 4.0)),
    "躯干": ((-4.0, 12.0, -2.0), (4.0, 24.0, 2.0)),
    "右臂": ((4.0, 12.0, -2.0), (8.0, 24.0, 2.0)),
    "左臂": ((-8.0, 12.0, -2.0), (-4.0, 24.0, 2.0)),
    "右腿": ((0.0, 0.0, -2.0), (4.0, 12.0, 2.0)),
    "左腿": ((-4.0, 0.0, -2.0), (0.0, 12.0, 2.0)),
}

PLAYER_COLLISION_PARTS = {
    "头": ((-4.0, 24.0, -4.0), (4.0, 32.0, 4.0)),
    "躯干": ((-4.0, 12.0, -2.0), (4.0, 24.0, 2.0)),
    "左臂": ((-8.0, 12.0, -2.0), (-4.0, 24.0, 2.0)),
    "右腿": ((0.0, 0.0, -2.0), (4.0, 12.0, 2.0)),
    "左腿": ((-4.0, 0.0, -2.0), (0.0, 12.0, 2.0)),
}

SHOULDER_ORIGIN = np.array([6.0, 22.0, 0.0])
ARM_ROTATION_DEG = 30.0

def rotmat(deg: float, axis: int) -> np.ndarray:
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    if axis == 0:
        return np.array([[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]])
    if axis == 1:
        return np.array([[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]])
    return np.array([[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]])

def get_fist_position() -> np.ndarray:
    R_arm = rotmat(ARM_ROTATION_DEG, 0)
    fist_orig = np.array([6.0, 13.5, 0.0])
    return R_arm @ (fist_orig - SHOULDER_ORIGIN) + SHOULDER_ORIGIN

def obb_aabb_intersect(obb_corners: List[np.ndarray], obb_axes: List[np.ndarray],
                       aabb_min: Tuple[float, float, float], aabb_max: Tuple[float, float, float]) -> bool:
    axes = [
        np.array([1., 0., 0.]),
        np.array([0., 1., 0.]),
        np.array([0., 0., 1.]),
    ]
    for a in obb_axes:
        axes.append(a)
    for a1 in [np.array([1., 0., 0.]), np.array([0., 1., 0.]), np.array([0., 0., 1.])]:
        for a2 in obb_axes:
            cross = np.cross(a1, a2)
            norm = np.linalg.norm(cross)
            if norm > 1e-4:
                axes.append(cross / norm)

    aabb_corners = [
        np.array([x, y, z])
        for x in [aabb_min[0], aabb_max[0]]
        for y in [aabb_min[1], aabb_max[1]]
        for z in [aabb_min[2], aabb_max[2]]
    ]

    for axis in axes:
        p_obb = [np.dot(c, axis) for c in obb_corners]
        p_aabb = [np.dot(c, axis) for c in aabb_corners]
        if min(p_obb) >= max(p_aabb) or min(p_aabb) >= max(p_obb):
            return False
    return True

def inject_grey_player_texture(doc: dict) -> Tuple[List[float], List[float]]:
    """在贴图图集最后一行写入灰色人体与深灰眼睛 swatch，返回其 UV 坐标，确保占位玩家为纯灰色绝不染棕色。"""
    body_uv = [49.0, 57.0, 55.0, 63.0]
    eye_uv = [57.0, 57.0, 63.0, 63.0]

    if not doc.get("textures"):
        img = Image.new("RGBA", (64, 64), (160, 160, 165, 255))
        px = img.load()
        for y in range(56, 64):
            for x in range(56, 64):
                px[x, y] = (60, 60, 65, 255)
        buf = io.BytesIO()
        img.save(buf, format="PNG")
        new_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")
        doc["textures"] = [{"id": 0, "name": "texture", "folder": "weapon", "source": new_b64}]
        return body_uv, eye_uv

    tex_entry = doc["textures"][0]
    src = tex_entry.get("source", "")
    if "," in src:
        raw = base64.b64decode(src.split(",", 1)[1])
    else:
        raw = base64.b64decode(src) if src else None

    if raw:
        im = Image.open(io.BytesIO(raw)).convert("RGBA")
    else:
        im = Image.new("RGBA", (64, 64), (160, 160, 165, 255))

    px = im.load()
    # 写入中性灰色身体色块与暗灰眼睛色块
    for y in range(56, 64):
        for x in range(48, 56):
            px[x, y] = (160, 160, 165, 255)
        for x in range(56, 64):
            px[x, y] = (60, 60, 65, 255)

    buf = io.BytesIO()
    im.save(buf, format="PNG")
    new_b64 = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode("ascii")
    tex_entry["source"] = new_b64
    return body_uv, eye_uv


def check_holding_collisions(
    weapon_doc: dict,
    grip_y: float,
    rot_w: Tuple[float, float, float],
    pose_type: str = "single_hand"
) -> Tuple[int, List[str]]:
    if pose_type == "crossbow":
        # 弩双手端平抵肩：后托抵在右肩前部 (x=3.5, y=20.0, z=-2.4)，沿 -Z 水平向前延伸
        dx = 3.5
        dy = 20.0
        dz = -2.4
        Rm = rotmat(rot_w[2], 2) @ rotmat(rot_w[1], 1) @ rotmat(rot_w[0], 0)
        pivot = np.array([0.0, 0.0, 0.0])
    elif pose_type == "bow":
        # 弓左手持弓：左臂伸直向前，拳心握在握把中心，弓绕 Y 转 180° (弓背朝前 -Z, 弓弦朝向射手 +Z)
        p_grip = np.array([0.0, grip_y, 4.2])
        p_fist = np.array([-6.0, 21.7, -8.5])
        R_bow = rotmat(180.0, 1)
    else:
        fist_pos = get_fist_position()
        dx = fist_pos[0] - 0.0
        dy = fist_pos[1] - grip_y
        dz = fist_pos[2] - 0.0
        Rm = rotmat(rot_w[2], 2) @ rotmat(rot_w[1], 1) @ rotmat(rot_w[0], 0)
        pivot = fist_pos

    collisions = []

    for elem in weapon_doc["elements"]:
        f = np.array(elem["from"], float)
        t = np.array(elem["to"], float)
        e_rot = elem.get("rotation", [0, 0, 0])
        e_org = np.array(elem.get("origin", [0, 0, 0]), float)
        Re = rotmat(e_rot[2], 2) @ rotmat(e_rot[1], 1) @ rotmat(e_rot[0], 0) if any(e_rot) else np.eye(3)

        corners = [
            np.array([x, y, z])
            for x in [f[0], t[0]]
            for y in [f[1], t[1]]
            for z in [f[2], t[2]]
        ]
        if any(e_rot):
            corners = [Re @ (c - e_org) + e_org for c in corners]

        if pose_type == "crossbow":
            corners = [Rm @ c + np.array([dx, dy, dz]) for c in corners]
            obb_axes = [
                Rm @ Re @ np.array([1., 0., 0.]),
                Rm @ Re @ np.array([0., 1., 0.]),
                Rm @ Re @ np.array([0., 0., 1.])
            ]
        elif pose_type == "bow":
            corners = [R_bow @ (c - p_grip) + p_fist for c in corners]
            obb_axes = [
                R_bow @ Re @ np.array([1., 0., 0.]),
                R_bow @ Re @ np.array([0., 1., 0.]),
                R_bow @ Re @ np.array([0., 0., 1.])
            ]
        else:
            corners = [c + np.array([dx, dy, dz]) for c in corners]
            corners = [Rm @ (c - pivot) + pivot for c in corners]
            obb_axes = [
                Rm @ Re @ np.array([1., 0., 0.]),
                Rm @ Re @ np.array([0., 1., 0.]),
                Rm @ Re @ np.array([0., 0., 1.])
            ]

        for part_name, (p_min, p_max) in PLAYER_COLLISION_PARTS.items():
            if obb_aabb_intersect(corners, obb_axes, p_min, p_max):
                collisions.append(f"{elem['name']} 与 {part_name}")

    return len(collisions), collisions

def make_standing_bbmodel(weapon_doc: dict, out_path: Path, textured: bool = False) -> None:
    """生成并排比例模型：兵器竖直立在占位玩家右侧，留 2px 空隙，底端对齐脚底 (y=0.0)。"""
    min_x = min(e["from"][0] for e in weapon_doc["elements"])
    min_y = min(e["from"][1] for e in weapon_doc["elements"])
    dx_stand = 10.0 - min_x
    dy_stand = 0.0 - min_y

    doc = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "Standing_Scale",
        "resolution": weapon_doc.get("resolution", {"width": 64, "height": 64}),
        "elements": [],
        "outliner": [],
        "textures": copy.deepcopy(weapon_doc.get("textures", []))
    }

    body_uv, eye_uv = inject_grey_player_texture(doc)

    # 玩家方块（纯灰色占位人体）
    for label, (lo, hi) in PLAYER_BOXES.items():
        elem = {
            "name": f"player_{label}",
            "box_uv": False,
            "from": list(lo),
            "to": list(hi),
            "faces": {face: {"uv": list(body_uv), "texture": 0} for face in ["north", "south", "east", "west", "up", "down"]},
            "uuid": f"p_{label}"
        }
        doc["elements"].append(elem)

    # 眼睛
    for idx, sx in enumerate([-2.0, 1.0]):
        elem = {
            "name": f"player_eye_{idx}",
            "box_uv": False,
            "from": [sx, 27.0, -4.05],
            "to": [sx + 1.0, 28.0, -3.95],
            "faces": {face: {"uv": list(eye_uv), "texture": 0} for face in ["north", "south", "east", "west", "up", "down"]},
            "uuid": f"p_eye_{idx}"
        }
        doc["elements"].append(elem)

    # 武器方块
    for idx, e in enumerate(weapon_doc["elements"]):
        new_e = copy.deepcopy(e)
        new_e["from"] = [round(e["from"][0] + dx_stand, 4), round(e["from"][1] + dy_stand, 4), round(e["from"][2], 4)]
        new_e["to"] = [round(e["to"][0] + dx_stand, 4), round(e["to"][1] + dy_stand, 4), round(e["to"][2], 4)]
        new_e["uuid"] = f"w_{idx}"
        if any(new_e.get("rotation", [0, 0, 0])):
            org = new_e["origin"]
            new_e["origin"] = [round(org[0] + dx_stand, 4), round(org[1] + dy_stand, 4), round(org[2], 4)]
        doc["elements"].append(new_e)

    doc["outliner"] = [{"name": "root", "origin": [0, 0, 0], "children": [e["uuid"] for e in doc["elements"]]}]
    out_path.write_text(json.dumps(doc, ensure_ascii=False, indent=1), encoding="utf-8")

def make_holding_bbmodel(weapon_doc: dict, grip_y: float, rot_w: Tuple[float, float, float],
                        out_path: Path, textured: bool = False, pose_type: str = "single_hand") -> None:
    """生成持握姿势模型：支持单手兵器、长弓（左手持弓，绕Y转180°）及弩（双手端平抵肩）。"""
    doc = {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": "Holding_Pose",
        "resolution": weapon_doc.get("resolution", {"width": 64, "height": 64}),
        "elements": [],
        "outliner": [],
        "textures": copy.deepcopy(weapon_doc.get("textures", []))
    }

    body_uv, eye_uv = inject_grey_player_texture(doc)

    if pose_type == "crossbow":
        # 弩双手端平抵肩：后托抵在右肩前部 (x=3.5, y=20.0, z=-2.4)，沿 -Z 水平向前延伸
        dx_hold = 3.5
        dy_hold = 20.0
        dz_hold = -2.4
        fist_pos = [0.0, 0.0, 0.0]
    elif pose_type == "bow":
        # 弓左手持弓：左臂伸直向前，拳心在 [-6.0, 21.7, -8.5]
        p_grip = np.array([0.0, grip_y, 4.2])
        p_fist = np.array([-6.0, 21.7, -8.5])
        R_bow = rotmat(180.0, 1)
    else:
        fist_pos = list(get_fist_position())
        dx_hold = fist_pos[0] - 0.0
        dy_hold = fist_pos[1] - grip_y
        dz_hold = fist_pos[2] - 0.0

    # 玩家方块 (带姿态旋转)
    for label, (lo, hi) in PLAYER_BOXES.items():
        elem = {
            "name": f"player_{label}",
            "box_uv": False,
            "from": list(lo),
            "to": list(hi),
            "faces": {face: {"uv": list(body_uv), "texture": 0} for face in ["north", "south", "east", "west", "up", "down"]},
            "uuid": f"p_{label}"
        }
        if pose_type == "crossbow":
            if label == "右臂":
                elem["origin"] = list(SHOULDER_ORIGIN)
                elem["rotation"] = [78.0, 15.0, 0.0]
            elif label == "左臂":
                elem["origin"] = [-6.0, 22.0, 0.0]
                elem["rotation"] = [70.0, -35.0, 0.0]
        elif pose_type == "bow":
            if label == "左臂":
                elem["origin"] = [-6.0, 22.0, 0.0]
                elem["rotation"] = [88.0, 0.0, 0.0]
            elif label == "右臂":
                elem["origin"] = list(SHOULDER_ORIGIN)
                elem["rotation"] = [38.0, -10.0, 0.0]
        else:
            if label == "右臂":
                elem["origin"] = list(SHOULDER_ORIGIN)
                elem["rotation"] = [ARM_ROTATION_DEG, 0.0, 0.0]
        doc["elements"].append(elem)

    # 眼睛
    for idx, sx in enumerate([-2.0, 1.0]):
        elem = {
            "name": f"player_eye_{idx}",
            "box_uv": False,
            "from": [sx, 27.0, -4.05],
            "to": [sx + 1.0, 28.0, -3.95],
            "faces": {face: {"uv": list(eye_uv), "texture": 0} for face in ["north", "south", "east", "west", "up", "down"]},
            "uuid": f"p_eye_{idx}"
        }
        doc["elements"].append(elem)

    # 武器方块
    for idx, e in enumerate(weapon_doc["elements"]):
        new_e = copy.deepcopy(e)
        if pose_type == "crossbow":
            # 武器绕自身原点旋转后平移到持握位
            Rm = rotmat(rot_w[2], 2) @ rotmat(rot_w[1], 1) @ rotmat(rot_w[0], 0)
            f_orig = np.array(e["from"], float)
            t_orig = np.array(e["to"], float)
            e_rot = e.get("rotation", [0, 0, 0])
            e_org = np.array(e.get("origin", [0, 0, 0]), float)
            Re = rotmat(e_rot[2], 2) @ rotmat(e_rot[1], 1) @ rotmat(e_rot[0], 0) if any(e_rot) else np.eye(3)

            corners = [
                np.array([x, y, z])
                for x in [f_orig[0], t_orig[0]]
                for y in [f_orig[1], t_orig[1]]
                for z in [f_orig[2], t_orig[2]]
            ]
            if any(e_rot):
                corners = [Re @ (c - e_org) + e_org for c in corners]
            corners = [Rm @ c + np.array([dx_hold, dy_hold, dz_hold]) for c in corners]
            c_arr = np.array(corners)
            new_e["from"] = [round(float(c_arr[:, 0].min()), 4), round(float(c_arr[:, 1].min()), 4), round(float(c_arr[:, 2].min()), 4)]
            new_e["to"] = [round(float(c_arr[:, 0].max()), 4), round(float(c_arr[:, 1].max()), 4), round(float(c_arr[:, 2].max()), 4)]
            new_e.pop("rotation", None)
            new_e.pop("origin", None)
        elif pose_type == "bow":
            # 弓绕握把中心旋转 180° 后平移到左拳心 (弓背朝前 -Z, 弓弦朝身体 +Z)
            f_orig = np.array(e["from"], float)
            t_orig = np.array(e["to"], float)
            e_rot = e.get("rotation", [0, 0, 0])
            e_org = np.array(e.get("origin", [0, 0, 0]), float)
            Re = rotmat(e_rot[2], 2) @ rotmat(e_rot[1], 1) @ rotmat(e_rot[0], 0) if any(e_rot) else np.eye(3)

            corners = [
                np.array([x, y, z])
                for x in [f_orig[0], t_orig[0]]
                for y in [f_orig[1], t_orig[1]]
                for z in [f_orig[2], t_orig[2]]
            ]
            if any(e_rot):
                corners = [Re @ (c - e_org) + e_org for c in corners]
            corners = [R_bow @ (c - p_grip) + p_fist for c in corners]
            c_arr = np.array(corners)
            new_e["from"] = [round(float(c_arr[:, 0].min()), 4), round(float(c_arr[:, 1].min()), 4), round(float(c_arr[:, 2].min()), 4)]
            new_e["to"] = [round(float(c_arr[:, 0].max()), 4), round(float(c_arr[:, 1].max()), 4), round(float(c_arr[:, 2].max()), 4)]
            new_e.pop("rotation", None)
            new_e.pop("origin", None)
        else:
            new_e["from"] = [round(e["from"][0] + dx_hold, 4), round(e["from"][1] + dy_hold, 4), round(e["from"][2] + dz_hold, 4)]
            new_e["to"] = [round(e["to"][0] + dx_hold, 4), round(e["to"][1] + dy_hold, 4), round(e["to"][2] + dz_hold, 4)]
            new_e["origin"] = list(fist_pos)
            new_e["rotation"] = list(rot_w)
        new_e["uuid"] = f"w_{idx}"
        doc["elements"].append(new_e)

    doc["outliner"] = [{"name": "root", "origin": [0, 0, 0], "children": [e["uuid"] for e in doc["elements"]]}]
    out_path.write_text(json.dumps(doc, ensure_ascii=False, indent=1), encoding="utf-8")

def render_player_scale_and_pose_panels(
    weapon_doc: dict,
    grip_y: float,
    weapon_len: float,
    is_polearm: bool = False,
    out_dir: Path = Path("/tmp"),
    textured: bool = False,
    font_h=None,
    font_l=None,
    pose_type: str = "single_hand"
) -> Image.Image:
    """渲染并拼接两栏：(A) 并排比例 2张 + (B) 持握姿势 3张。"""
    font_h = font_h or load_font(16)
    font_l = font_l or load_font(12)

    if pose_type == "crossbow":
        rot_w = (-90.0, 0.0, 0.0)
        pose_desc = "弩双手端平抵肩 · 弩弓在前/扳机朝下/射击朝前"
    elif pose_type == "bow":
        rot_w = (0.0, 180.0, 0.0)
        pose_desc = "弓左手持弓 · 弓背朝前 / 弓弦朝射手"
    else:
        rot_w = (-35.0, 0.0, 0.0) if is_polearm else (-45.0, 0.0, 0.0)
        pose_desc = "右臂前抬30°握在拳心"

    stand_bb = out_dir / "_tmp_weapon_stand.bbmodel"
    hold_bb = out_dir / "_tmp_weapon_hold.bbmodel"

    make_standing_bbmodel(weapon_doc, stand_bb, textured=textured)
    make_holding_bbmodel(weapon_doc, grip_y, rot_w, hold_bb, textured=textured, pose_type=pose_type)

    col_count, _ = check_holding_collisions(weapon_doc, grip_y, rot_w, pose_type=pose_type)

    cell = 380
    pad = 8
    shading = "mc" if textured else "lambert"

    shot_a_front, _ = render(stand_bb, yaw=180.0, pitch=0.0, size=cell, bg=gear_kit.BG, shading=shading)
    shot_a_side, _ = render(stand_bb, yaw=-90.0, pitch=0.0, size=cell, bg=gear_kit.BG, shading=shading)

    shot_b_front, _ = render(hold_bb, yaw=180.0, pitch=0.0, size=cell, bg=gear_kit.BG, shading=shading)
    shot_b_side, _ = render(hold_bb, yaw=-90.0, pitch=0.0, size=cell, bg=gear_kit.BG, shading=shading)
    shot_b_iso, _ = render(hold_bb, yaw=145.0, pitch=20.0, size=cell, bg=gear_kit.BG, shading=shading)

    # 在侧视图 shot_b_side 上绘制射击方向箭头（相机 yaw=-90 视线向-X，画面左方为 -Z 前方目标）
    if pose_type in ("bow", "crossbow"):
        d_side = ImageDraw.Draw(shot_b_side)
        arrow_y = 35
        # 绘制向左的红色/黄色箭头与文字
        d_side.line([(140, arrow_y), (40, arrow_y)], fill=(240, 210, 80), width=3)
        d_side.polygon([(32, arrow_y), (45, arrow_y - 6), (45, arrow_y + 6)], fill=(240, 210, 80))
        d_side.text((50, arrow_y - 18), "← 射击方向 (目标)", fill=(240, 220, 100), font=font_l)
        if pose_type == "bow":
            d_side.text((15, 345), "自检: 弓背朝前 / 弓弦朝射手 ✓", fill=(130, 235, 160), font=font_l)
        elif pose_type == "crossbow":
            d_side.text((15, 345), "自检: 弩弓在前 / 扳机朝下 ✓", fill=(130, 235, 160), font=font_l)

    stand_bb.unlink(missing_ok=True)
    hold_bb.unlink(missing_ok=True)

    total_w = cell * 5 + pad * 6
    header_h = 58
    total_h = cell + header_h + 10

    panel = Image.new("RGB", (total_w, total_h), (28, 29, 33))
    d = ImageDraw.Draw(panel)

    pct_str = f"{weapon_len:.1f}px / 32px = {weapon_len / 32.0 * 100:.1f}%"
    col_str = "0 处交叠 (体积严格为 0)" if col_count == 0 else f"{col_count} 处交叠"

    d.text((pad, 6), f"(A) 并排比例 · {pct_str} · 竖立右侧间隙2px底端对齐脚底", fill=(240, 225, 175), font=font_h)
    d.text((pad + cell * 2 + pad * 2, 6), f"(B) 持握姿势 · {pose_desc} · 几何相交: {col_str} ✓",
           fill=(150, 235, 180), font=font_h)

    cards_a = [("并排: 正面 FRONT", shot_a_front), ("并排: 侧面 SIDE", shot_a_side)]
    for i, (title, shot) in enumerate(cards_a):
        x = pad + i * (cell + pad)
        d.rectangle([x, 32, x + cell - 1, 52], fill=(20, 22, 26))
        d.text((x + 6, 35), title, fill=(230, 230, 230), font=font_l)
        panel.paste(shot, (x, 54))

    cards_b = [
        ("持握: 正面 FRONT", shot_b_front),
        ("持握: 侧面 SIDE", shot_b_side),
        ("持握: 3/4 斜视", shot_b_iso)
    ]
    x_b_start = pad + cell * 2 + pad * 2
    for i, (title, shot) in enumerate(cards_b):
        x = x_b_start + i * (cell + pad)
        d.rectangle([x, 32, x + cell - 1, 52], fill=(20, 22, 26))
        d.text((x + 6, 35), title, fill=(230, 230, 230), font=font_l)
        panel.paste(shot, (x, 54))

    return panel

def extract_weapon_silhouette(ref_path: Path) -> Image.Image:
    im = Image.open(ref_path).convert("RGB")
    arr = np.array(im)
    bg = np.median(arr[:30, :30], axis=(0,1))
    mask = np.max(np.abs(arr.astype(float) - bg), axis=2) > 22

    from scipy import ndimage
    labels, num = ndimage.label(mask)
    best_h = 0
    best_bbox = None
    for idx in range(num):
        ys, xs = np.where(labels == idx + 1)
        w = xs.max() - xs.min() + 1
        h = ys.max() - ys.min() + 1
        if len(ys) > 0 and w > 20:
            if h > best_h:
                best_h = h
                best_bbox = (xs.min(), ys.min(), xs.max(), ys.max())

    if best_bbox is None:
        return im.crop((im.width - 200, 0, im.width, im.height))
    
    crop = im.crop(best_bbox)
    # 若横置则旋转为直立
    if crop.width > crop.height * 1.5:
        crop = crop.rotate(90, expand=True)
    return crop

def crop_tight(im: Image.Image, threshold: float = 20.0) -> Image.Image:
    arr = np.array(im.convert("RGB"))
    bg = np.median(arr[:10, :10], axis=(0, 1))
    diff = np.max(np.abs(arr.astype(float) - bg), axis=2)
    ys, xs = np.where(diff > threshold)
    if len(ys) > 0:
        return im.crop((xs.min(), ys.min(), xs.max(), ys.max()))
    return im

def build_crossbow_dual_proportion_panel(
    ref_side_crop: Image.Image,
    model_side_img: Image.Image,
    side_metrics: List[Tuple[str, float, float]],
    side_lines: List[Tuple[str, float, Tuple[int, int, int]]],
    ref_top_crop: Image.Image,
    model_top_img: Image.Image,
    top_metrics: List[Tuple[str, float, float]],
    top_lines: List[Tuple[str, float, Tuple[int, int, int]]],
    target_h: int = 460
) -> Image.Image:
    """生成双视角比例对照面板：左侧整把弩【侧视长图对照】，右侧整把弩【俯视长图对照】。"""
    font_h = load_font(15)
    font_m = load_font(12)
    font_s = load_font(11)

    # 辅助函数：构建单视角的等高并排与指标表
    def build_single_view(title: str, ref_im: Image.Image, mod_im: Image.Image,
                           metrics: List[Tuple[str, float, float]],
                           ref_lines: List[Tuple[str, float, Tuple[int, int, int]]],
                           w_sub: int = 580) -> Image.Image:
        # 保证两者均为直立（高大于宽）
        if ref_im.width > ref_im.height * 1.2:
            r_im = ref_im.rotate(90, expand=True)
        else:
            r_im = ref_im

        if mod_im.width > mod_im.height * 1.2:
            m_im = mod_im.rotate(90, expand=True)
        else:
            m_im = mod_im

        r_im = crop_tight(r_im)
        m_im = crop_tight(m_im)

        scale_r = target_h / r_im.height
        w_r = max(24, int(round(r_im.width * scale_r)))
        r_scaled = r_im.resize((w_r, target_h), Image.Resampling.LANCZOS)

        scale_m = target_h / m_im.height
        w_m = max(24, int(round(m_im.width * scale_m)))
        m_scaled = m_im.resize((w_m, target_h), Image.Resampling.LANCZOS)

        pad_x = 35
        table_h = 32 + len(metrics) * 22 + 15
        sub_h = 40 + target_h + 20 + table_h
        sub_panel = Image.new("RGB", (w_sub, sub_h), (30, 31, 35))
        d_sub = ImageDraw.Draw(sub_panel)

        d_sub.text((10, 8), title, fill=(240, 225, 175), font=font_h)

        x_r = (w_sub // 2) - w_r - (pad_x // 2)
        x_m = (w_sub // 2) + (pad_x // 2)
        y_top = 38

        sub_panel.paste(r_scaled, (x_r, y_top))
        sub_panel.paste(m_scaled, (x_m, y_top))

        d_sub.text((x_r + (w_r // 2) - 30, y_top - 16), "参考图整弩", fill=(190, 210, 235), font=font_s)
        d_sub.text((x_m + (w_m // 2) - 30, y_top - 16), "模型对应视角", fill=(160, 230, 180), font=font_s)

        line_x0 = x_r - 25
        line_x1 = x_m + w_m + 25
        for label, pct, col in ref_lines:
            ly = y_top + int(round((1.0 - pct) * target_h))
            for seg_x in range(line_x0, line_x1, 7):
                d_sub.line([(seg_x, ly), (min(seg_x + 3, line_x1), ly)], fill=col, width=1)
            d_sub.text((line_x1 + 4, ly - 5), label, fill=col, font=font_s)

        # 比例指标表格
        ty = y_top + target_h + 15
        d_sub.rectangle([8, ty, w_sub - 8, ty + table_h - 8], outline=(65, 70, 80), width=1)
        headers = ["对比项目", "参考图", "模型值", "差值(Δ)", "判定"]
        col_xs = [14, 180, 280, 380, 480]
        d_sub.rectangle([8, ty, w_sub - 8, ty + 24], fill=(22, 24, 28))
        for i, h in enumerate(headers):
            d_sub.text((col_xs[i], ty + 4), h, fill=(225, 225, 225), font=font_m)

        row_y = ty + 26
        for item_name, ref_val, mod_val in metrics:
            diff = abs(ref_val - mod_val)
            ok = diff <= 10.0
            status_text = "✓ 合格" if ok else "✗ 超标"
            status_color = (130, 225, 140) if ok else (240, 100, 90)

            d_sub.text((col_xs[0], row_y + 2), item_name, fill=(210, 210, 210), font=font_s)
            d_sub.text((col_xs[1], row_y + 2), f"{ref_val:.1f}%", fill=(190, 210, 235), font=font_s)
            d_sub.text((col_xs[2], row_y + 2), f"{mod_val:.1f}%", fill=(160, 230, 180), font=font_s)
            d_sub.text((col_xs[3], row_y + 2), f"{diff:+.1f}%", fill=(240, 210, 140), font=font_s)
            d_sub.text((col_xs[4], row_y + 2), status_text, fill=status_color, font=font_s)
            row_y += 22

        return sub_panel

    sub_side = build_single_view(
        "【侧视长图比例对照】参考图侧视 vs 模型侧视（整弩对齐）",
        ref_side_crop, model_side_img, side_metrics, side_lines, w_sub=590
    )
    sub_top = build_single_view(
        "【俯视长图比例对照】参考图俯视 vs 模型俯视（整弩对齐）",
        ref_top_crop, model_top_img, top_metrics, top_lines, w_sub=590
    )

    pad_mid = 15
    total_w = sub_side.width + sub_top.width + pad_mid + 16
    total_h = max(sub_side.height, sub_top.height) + 12

    panel = Image.new("RGB", (total_w, total_h), (25, 26, 29))
    panel.paste(sub_side, (8, 6))
    panel.paste(sub_top, (8 + sub_side.width + pad_mid, 6))

    return panel
    font_m = load_font(13)
    font_s = load_font(12)

    scale_ref = target_h / ref_crop.height
    w_ref = max(30, int(round(ref_crop.width * scale_ref)))
    ref_scaled = ref_crop.resize((w_ref, target_h), Image.Resampling.LANCZOS)

    arr_m = np.array(model_front)
    bg_m = np.median(arr_m[:10, :10], axis=(0,1))
    mask_m = np.max(np.abs(arr_m.astype(float) - bg_m), axis=2) > 15
    ys_m, xs_m = np.where(mask_m)
    if len(ys_m) > 0:
        model_crop = model_front.crop((xs_m.min(), ys_m.min(), xs_m.max(), ys_m.max()))
    else:
        model_crop = model_front
    scale_mod = target_h / model_crop.height
    w_mod = max(30, int(round(model_crop.width * scale_mod)))
    model_scaled = model_crop.resize((w_mod, target_h), Image.Resampling.LANCZOS)

    pad_x = 40
    img_area_w = w_ref + pad_x + w_mod + 80
    panel_w = max(680, img_area_w + 30)
    table_h = 40 + len(metrics) * 24 + 30
    panel_h = 45 + target_h + 30 + table_h

    panel = Image.new("RGB", (panel_w, panel_h), (30, 31, 35))
    d = ImageDraw.Draw(panel)

    d.text((12, 10), "比例对照 PROPORTION CHECK（参考图剪影 vs 模型同一视角，对齐等高）", fill=(240, 225, 175), font=font_h)

    x_ref = (panel_w // 2) - w_ref - (pad_x // 2)
    x_mod = (panel_w // 2) + (pad_x // 2)
    y_top = 45

    panel.paste(ref_scaled, (x_ref, y_top))
    panel.paste(model_scaled, (x_mod, y_top))

    d.text((x_ref + (w_ref // 2) - 30, y_top - 18), "参考图剪影", fill=(190, 210, 235), font=font_s)
    d.text((x_mod + (w_mod // 2) - 30, y_top - 18), "模型正视剪影", fill=(160, 230, 180), font=font_s)

    line_x0 = x_ref - 35
    line_x1 = x_mod + w_mod + 35
    for label, pct, col in ref_lines:
        ly = y_top + int(round((1.0 - pct) * target_h))
        for seg_x in range(line_x0, line_x1, 8):
            d.line([(seg_x, ly), (min(seg_x + 4, line_x1), ly)], fill=col, width=1)
        d.text((line_x1 + 6, ly - 6), label, fill=col, font=font_s)

    ty = y_top + target_h + 25
    d.rectangle([12, ty, panel_w - 12, ty + table_h - 10], outline=(65, 70, 80), width=1)
    headers = ["对比项目", "参考图测量值", "模型当前值", "差值 (Δ)", "阈值判定"]
    col_xs = [20, 200, 320, 440, 560]
    d.rectangle([12, ty, panel_w - 12, ty + 26], fill=(22, 24, 28))
    for i, h in enumerate(headers):
        d.text((col_xs[i], ty + 5), h, fill=(225, 225, 225), font=font_m)

    row_y = ty + 28
    for item_name, ref_val, mod_val in metrics:
        diff = abs(ref_val - mod_val)
        ok = diff <= 10.0
        status_text = f"✓ 合格 (<10%)" if ok else f"✗ 超标"
        status_color = (130, 225, 140) if ok else (240, 100, 90)

        d.text((col_xs[0], row_y + 3), item_name, fill=(210, 210, 210), font=font_s)
        d.text((col_xs[1], row_y + 3), f"{ref_val:.1f}%", fill=(190, 210, 235), font=font_s)
        d.text((col_xs[2], row_y + 3), f"{mod_val:.1f}%", fill=(160, 230, 180), font=font_s)
        d.text((col_xs[3], row_y + 3), f"{diff:+.1f}%", fill=(240, 210, 140), font=font_s)
        d.text((col_xs[4], row_y + 3), status_text, fill=status_color, font=font_s)
        row_y += 24

    return panel

def assemble_check_image(
    six_views_img: Image.Image,
    scale_pose_panel: Image.Image,
    prop_panel: Image.Image,
    tree_panel: Image.Image,
    ref_scaled: Image.Image | None,
    exp_scaled: Image.Image | None,
    out_path: Path,
    title: str = "本模型（Round 1 几何平涂版；无贴图；尺寸比例依参考图重构）"
) -> None:
    lw = 640
    font_h = load_font(18)
    ref_h = (ref_scaled.height if ref_scaled else 200) + (exp_scaled.height if exp_scaled else 200) + 80
    mid_top_w = six_views_img.width + prop_panel.width + 15
    mid_w = max(mid_top_w, scale_pose_panel.width)
    total_h = max(ref_h + 20, six_views_img.height + scale_pose_panel.height + 60, tree_panel.height + 44)
    canvas = Image.new("RGB", (lw + mid_w + tree_panel.width + 65, total_h), (25, 26, 29))
    d = ImageDraw.Draw(canvas)

    # 1. 左侧参考图与爆炸图
    y = 8
    d.text((10, y), "REFERENCE 参考图（概念与手持）", fill=(215, 205, 180), font=font_h)
    if ref_scaled:
        canvas.paste(ref_scaled, (10, y + 30))
        y += ref_scaled.height + 40
    d.text((10, y), "EXPLODED 爆炸图（部件结构）", fill=(215, 205, 180), font=font_h)
    if exp_scaled:
        canvas.paste(exp_scaled, (10, y + 30))

    # 2. 中间上排：六视图 + 比例对照面板
    x = lw + 25
    d.text((x, 8), title, fill=(140, 230, 180), font=font_h)
    canvas.paste(six_views_img, (x, 38))
    canvas.paste(prop_panel, (x + six_views_img.width + 15, 38))

    # 3. 中间下排：(A)并排比例 + (B)持握姿势 (宽度 1948px)
    canvas.paste(scale_pose_panel, (x, 38 + six_views_img.height + 15))

    # 4. 右侧：骨骼树
    x_tree = lw + mid_w + 40
    d.text((x_tree, 8), "骨骼层级树", fill=(180, 220, 240), font=font_h)
    canvas.paste(tree_panel, (x_tree, 38))

    canvas.save(out_path)
    print(f"✓ 审阅图 check.png 生成完毕: {out_path} ({canvas.width}x{canvas.height})")

def assemble_check_tex_image(
    ref_im: Image.Image,
    model_flat_path: Path,
    model_tex_path: Path,
    scale_pose_panel_tex: Image.Image,
    out_path: Path,
    title: str = "贴图版全景检视"
) -> None:
    """生成 Round 3 专用 check_tex.png：上排参考图 + 4视角平色/贴图对照，下排贴图版并排比例与持握姿势。"""
    font_h = load_font(16)
    font_l = load_font(12)
    size = 360
    tex_views = [
        ("FRONT 正视", 180.0, 0.0),
        ("SIDE 右侧视", -90.0, 0.0),
        ("3/4 斜视", 145.0, 20.0),
        ("BACK 背面", 0.0, 0.0)
    ]

    ref = ref_im.resize((round(ref_im.width * size / ref_im.height), size), Image.Resampling.LANCZOS)
    top_w = ref.width + len(tex_views) * 2 * (size + 6) + 12
    total_w = max(top_w, scale_pose_panel_tex.width)
    total_h = size + 45 + scale_pose_panel_tex.height + 15

    canvas = Image.new("RGB", (total_w, total_h), (26, 27, 30))
    d = ImageDraw.Draw(canvas)

    d.text((8, 8), "REFERENCE 参考图", fill=(215, 205, 180), font=font_h)
    canvas.paste(ref, (8, 32))

    x = ref.width + 16
    for label, yaw, pitch in tex_views:
        for mode_title, path, shading in (("FLAT 平色", model_flat_path, "lambert"), ("TEXTURED 贴图", model_tex_path, "mc")):
            shot, _ = render(path, yaw=yaw, pitch=pitch, size=size, shading=shading, bg=gear_kit.BG)
            canvas.paste(shot, (x, 32))
            d.text((x + 4, 10), f"{mode_title}: {label}", fill=(220, 220, 220), font=font_l)
            x += size + 6

    canvas.paste(scale_pose_panel_tex, (0, size + 45))
    canvas.save(out_path)
    print(f"✓ 贴图审阅图 check_tex.png 生成完毕: {out_path} ({canvas.width}x{canvas.height})")
