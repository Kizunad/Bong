#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "numpy",
#     "pillow",
#     "pyrender",
#     "trimesh",
#     "pyopengl>=3.1.7",
# ]
#
# [tool.uv]
# override-dependencies = ["pyopengl>=3.1.7"]
# ///
"""Render a Bong weapon model in MC handheld pose for headless display tuning.

pair 关系:
  - 读取 tripo_to_sml.py 产出的资产 (OBJ + MTL + 贴图) 和 asset_configs/<id>.json 覆盖
  - 复用同一份 HANDHELD_DISPLAY 默认（镜像，保持两边一致 —— 改了一边请同步另一边）
  - 用同一套 JOML rotationXYZ + T @ R @ S 数学还原 MC ItemRenderer 的 display transform

输出:
  默认 /tmp/display_preview/<asset_id>/{mode}.png + grid.png
  Claude 可以 Read 出来肉眼比对，iter 调 display 无需 runClient。

保真度说明:
  - rotation / translation / scale 的数学和 MC 一致（JOML rotationXYZ + MatrixStack
    顺序 T·R·S），改了 asset config PNG 就会变
  - 光照、FOV、阴影不和 MC 一样，所以颜色/高光观感会差
  - 一人称 swaybob 动画那套 animation-time 变换不在 display transform 作用范围，不还原

用法:
    uv run client/tools/render_held_item.py placeholder_sword
    # → client/tools/renders/placeholder_sword/{tp,fp,gui}.png + grid.png
    uv run client/tools/render_held_item.py cracked_heart --out /tmp/foo
    uv run client/tools/render_held_item.py crystal_shard_dagger --modes gui thirdperson_righthand

陷阱:
  - 和 tripo_to_sml.py 里 HANDHELD_DISPLAY 的那套"校准陷阱注释"完全适用（rotation Z
    方向反直觉 / translation 轴映射随 rotation 漂移 等）。本工具只如实渲染，不纠正。
"""

from __future__ import annotations

import argparse
import json
import math
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

# 必须在 import pyrender 之前设，否则 GL 平台决议就走 X11 了
os.environ.setdefault("PYOPENGL_PLATFORM", "egl")

import numpy as np
import pyrender
import trimesh
from PIL import Image

CLIENT_ROOT = Path(__file__).resolve().parents[1]
RESOURCES_ROOT = CLIENT_ROOT / "src" / "main" / "resources"
ASSET_CONFIG_DIR = Path(__file__).resolve().parent / "asset_configs"
MODELS_DIR = RESOURCES_ROOT / "assets" / "bong" / "models" / "item"
TEXTURES_ROOT = RESOURCES_ROOT / "assets" / "bong" / "textures"


# ━━━━━ 与 tripo_to_sml.py HANDHELD_DISPLAY 镜像 ━━━━━
# 改了一边请同步另一边。这个重复是有意的：tripo_to_sml.py 被 uv run 跑，不想让它
# import 本文件以免把 pyrender 拽进 pipeline 依赖。
HANDHELD_DISPLAY: dict[str, dict[str, list[float]]] = {
    "thirdperson_righthand": {"rotation": [0, -90, 55], "translation": [0, 0, 0], "scale": [1.0, 1.0, 1.0]},
    "thirdperson_lefthand":  {"rotation": [0,  90, -55], "translation": [0, 0, 0], "scale": [1.0, 1.0, 1.0]},
    "firstperson_righthand": {"rotation": [0, -90, 25], "translation": [0, 0, 0], "scale": [1.0, 1.0, 1.0]},
    "firstperson_lefthand":  {"rotation": [0,  90, -25], "translation": [0, 0, 0], "scale": [1.0, 1.0, 1.0]},
    "ground": {"rotation": [0, 0, 0], "translation": [0, 2, 0], "scale": [0.5, 0.5, 0.5]},
    "gui":    {"rotation": [0, 0, 0], "translation": [0, 0, 0], "scale": [1.0, 1.0, 1.0]},
    "fixed":  {"rotation": [0, 0, 0], "translation": [0, 0, 0], "scale": [1.0, 1.0, 1.0]},
}

DISPLAY_MODE_KEYS = set(HANDHELD_DISPLAY.keys())
DEFAULT_MODES = ("thirdperson_righthand", "firstperson_righthand", "gui")


# ━━━━━ config IO ━━━━━

def load_asset_config(asset_id: str) -> dict:
    path = ASSET_CONFIG_DIR / f"{asset_id}.json"
    if not path.exists():
        return {}
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except Exception as e:
        print(f"WARN: asset config 读取失败 {path}: {e}", file=sys.stderr)
        return {}
    return {k: v for k, v in raw.items() if not k.startswith("_")}


def load_display_override(asset_id: str) -> dict:
    cfg = load_asset_config(asset_id)
    return {k: v for k, v in cfg.items() if k in DISPLAY_MODE_KEYS}


def merge_display(default: dict, override: dict) -> dict:
    out = {k: dict(v) for k, v in default.items()}
    for mode, fields in override.items():
        if mode not in out or not isinstance(fields, dict):
            continue
        for field in ("rotation", "translation", "scale"):
            if field in fields:
                out[mode][field] = list(fields[field])
    return out


# ━━━━━ transform math ━━━━━

def rot_matrix_xyz(rx: float, ry: float, rz: float) -> np.ndarray:
    """JOML Quaternionf.rotationXYZ(x, y, z) as 3x3: Rx @ Ry @ Rz.

    MC ItemRenderer: matrices.multiply(new Quaternionf().rotationXYZ(x, y, z));
    vector transform: v' = (Rx·Ry·Rz) @ v  (先 Z, 再 Y, 再 X).
    """
    cx, sx = math.cos(rx), math.sin(rx)
    cy, sy = math.cos(ry), math.sin(ry)
    cz, sz = math.cos(rz), math.sin(rz)
    Rx = np.array([[1, 0, 0], [0, cx, -sx], [0, sx, cx]], dtype=np.float64)
    Ry = np.array([[cy, 0, sy], [0, 1, 0], [-sy, 0, cy]], dtype=np.float64)
    Rz = np.array([[cz, -sz, 0], [sz, cz, 0], [0, 0, 1]], dtype=np.float64)
    return Rx @ Ry @ Rz


def display_transform_matrix(cfg: dict) -> np.ndarray:
    """4x4 matrix: v_world = T · R · S · v_local, 单位 = pixel (1 MC unit)."""
    tx, ty, tz = cfg["translation"]
    rx, ry, rz = [math.radians(v) for v in cfg["rotation"]]
    sx, sy, sz = cfg["scale"]
    R = rot_matrix_xyz(rx, ry, rz)
    S = np.diag([sx, sy, sz])
    M = np.eye(4, dtype=np.float64)
    M[:3, :3] = R @ S
    M[:3, 3] = [tx, ty, tz]
    return M


# ━━━━━ OBJ + MTL + texture 加载 ━━━━━

@dataclass
class Material:
    name: str
    kd: tuple[float, float, float]
    texture: Optional[Image.Image]  # PIL RGB image, 已解析

    @property
    def has_texture(self) -> bool:
        return self.texture is not None


def _resolve_texture_reference(ref: str) -> Optional[Path]:
    """MTL map_Kd 解析:
      bong:item/foo/bar   → RESOURCES_ROOT/assets/bong/textures/item/foo/bar.png
      相对路径 (e.g. texture.png) → 调用方自己拼
    """
    if ":" in ref:
        namespace, rel = ref.split(":", 1)
        candidate = RESOURCES_ROOT / "assets" / namespace / "textures" / (rel + ".png")
        return candidate if candidate.exists() else None
    return None


def parse_mtl(mtl_path: Path) -> dict[str, Material]:
    """MTL 解析 —— 只抓 Kd 和 map_Kd，够做 display 渲染了。"""
    if not mtl_path.exists():
        return {}
    materials: dict[str, Material] = {}
    current: Optional[str] = None
    kd: dict[str, tuple[float, float, float]] = {}
    texref: dict[str, str] = {}

    for line in mtl_path.read_text(encoding="utf-8", errors="replace").splitlines():
        parts = line.strip().split()
        if not parts:
            continue
        head = parts[0].lower()
        if head == "newmtl":
            current = parts[1]
            kd[current] = (0.8, 0.8, 0.8)
        elif head == "kd" and current is not None and len(parts) >= 4:
            kd[current] = (float(parts[1]), float(parts[2]), float(parts[3]))
        elif head == "map_kd" and current is not None and len(parts) >= 2:
            texref[current] = parts[1]

    for name, color in kd.items():
        tex_img = None
        if name in texref:
            p = _resolve_texture_reference(texref[name])
            if p is not None:
                try:
                    tex_img = Image.open(p).convert("RGBA")
                except Exception as e:
                    print(f"WARN: 贴图读失败 {p}: {e}", file=sys.stderr)
            else:
                print(f"WARN: 贴图解析不到 {texref[name]}（material {name}）", file=sys.stderr)
        materials[name] = Material(name=name, kd=color, texture=tex_img)
    return materials


def parse_obj(obj_path: Path) -> tuple[np.ndarray, np.ndarray, np.ndarray, list[dict]]:
    """OBJ 解析 —— 按 usemtl 分组返回每个子 mesh 的 face list。

    返回:
      verts: (N, 3)
      uvs:   (M, 2)  空则 shape (0, 2)
      normals: (K, 3)  空则 shape (0, 3)
      groups: [{"material": str, "faces": [(v_idx, vt_idx, vn_idx), ...]}], 0-indexed
    """
    verts: list[tuple[float, float, float]] = []
    uvs: list[tuple[float, float]] = []
    normals: list[tuple[float, float, float]] = []
    groups: list[dict] = []
    current_mat = ""
    current_faces: list[tuple] = []

    def flush_group() -> None:
        nonlocal current_faces
        if current_faces:
            groups.append({"material": current_mat, "faces": current_faces})
            current_faces = []

    for line in obj_path.read_text(encoding="utf-8", errors="replace").splitlines():
        parts = line.strip().split()
        if not parts:
            continue
        head = parts[0]
        if head == "v" and len(parts) >= 4:
            verts.append((float(parts[1]), float(parts[2]), float(parts[3])))
        elif head == "vt" and len(parts) >= 3:
            uvs.append((float(parts[1]), float(parts[2])))
        elif head == "vn" and len(parts) >= 4:
            normals.append((float(parts[1]), float(parts[2]), float(parts[3])))
        elif head == "usemtl" and len(parts) >= 2:
            flush_group()
            current_mat = parts[1]
        elif head == "f":
            # face 每个顶点可能是 v 或 v/vt 或 v/vt/vn 或 v//vn，1-indexed
            indices = []
            for token in parts[1:]:
                sub = token.split("/")
                vi = int(sub[0]) - 1 if sub[0] else -1
                vti = int(sub[1]) - 1 if len(sub) >= 2 and sub[1] else -1
                vni = int(sub[2]) - 1 if len(sub) >= 3 and sub[2] else -1
                indices.append((vi, vti, vni))
            # 三角化（fan from v0）
            for k in range(1, len(indices) - 1):
                current_faces.append((indices[0], indices[k], indices[k + 1]))
    flush_group()

    return (
        np.array(verts, dtype=np.float64),
        np.array(uvs, dtype=np.float64) if uvs else np.zeros((0, 2), dtype=np.float64),
        np.array(normals, dtype=np.float64) if normals else np.zeros((0, 3), dtype=np.float64),
        groups,
    )


def build_submesh(
    verts: np.ndarray,
    uvs: np.ndarray,
    group: dict,
    material: Optional[Material],
) -> trimesh.Trimesh:
    """把一个 usemtl group 构造成 trimesh，UV + 贴图 / 漫反射色处理在这。

    OBJ 的 wedge 语义：同一 vertex 在不同 face 可能有不同 UV。trimesh 要求 per-vertex UV，
    所以我们重建顶点数组——每个 face 顶点各自出一份，避免索引错位。
    """
    new_verts = []
    new_uvs = []
    faces: list[list[int]] = []
    has_uv = uvs.shape[0] > 0 and material is not None and material.has_texture

    for (a, b, c) in group["faces"]:
        tri = [a, b, c]
        face_indices = []
        for (vi, vti, _) in tri:
            new_verts.append(verts[vi])
            if has_uv and vti >= 0 and vti < uvs.shape[0]:
                u, v = uvs[vti]
                # OBJ UV origin 在左下，PIL/trimesh 要左下（trimesh 内部自己翻）
                new_uvs.append((u, v))
            face_indices.append(len(new_verts) - 1)
        faces.append(face_indices)

    mesh = trimesh.Trimesh(
        vertices=np.asarray(new_verts, dtype=np.float64),
        faces=np.asarray(faces, dtype=np.int64),
        process=False,
    )
    if has_uv:
        mesh.visual = trimesh.visual.TextureVisuals(
            uv=np.asarray(new_uvs, dtype=np.float64),
            image=material.texture,
        )
    elif material is not None:
        color = tuple(int(c * 255) for c in material.kd) + (255,)
        mesh.visual.face_colors = color
    return mesh


def load_item_meshes(asset_id: str) -> list[pyrender.Mesh]:
    """加载 asset_id 对应的 OBJ + MTL + 贴图 → pyrender.Mesh 列表（每个 material 一项）。"""
    obj_path = MODELS_DIR / asset_id / f"{asset_id}.obj"
    if not obj_path.exists():
        raise FileNotFoundError(f"OBJ 找不到: {obj_path}")

    # tripo_to_sml.py 两种风格：foo.mtl 或 foo.obj.mtl
    mtl_candidates = [obj_path.with_suffix(".mtl"), obj_path.with_name(f"{asset_id}.obj.mtl")]
    mtl_path = next((p for p in mtl_candidates if p.exists()), obj_path.with_suffix(".mtl"))
    materials = parse_mtl(mtl_path)

    verts, uvs, _normals, groups = parse_obj(obj_path)
    if verts.shape[0] == 0:
        raise SystemExit(f"OBJ 里没顶点: {obj_path}")

    meshes: list[pyrender.Mesh] = []
    for g in groups:
        mat = materials.get(g["material"])
        tm = build_submesh(verts, uvs, g, mat)
        meshes.append(pyrender.Mesh.from_trimesh(tm, smooth=False))
    if not meshes:
        raise SystemExit(f"OBJ 没 face: {obj_path}")
    return meshes


# ━━━━━ scene composition + render ━━━━━

# MC model space 约定: +X = player 左, +Y = 下, +Z = 后 (player 朝 -Z)
# 右手 rest 位置 = rightArm pivot (-5,2,0) + hand end local (-1,10,0) = (-6,12,0)
# 我们把 display frame 的原点放在场景 (0,0,0)，手腕参考臂绘制在 +Y 方向

def build_reference_arm() -> trimesh.Trimesh:
    """右前臂 cuboid 参考: 手腕在 (0,0,0)，手臂沿 -Y 方向延伸 (MC 中 -Y = 上)。

    尺寸 4×12×4 (vanilla arm cuboid)；绘制的是"手以上的手臂部分"。
    """
    arm = trimesh.creation.box(extents=(4.0, 12.0, 4.0))
    arm.apply_translation([0.0, -6.0, 0.0])  # 顶端（肩侧）在 y=-12，底端（手腕）在 y=0
    arm.visual.face_colors = [210, 190, 160, 255]
    return arm


def _look_at_pose(eye: np.ndarray, target: np.ndarray, up_world: np.ndarray) -> np.ndarray:
    """pyrender camera pose: camera frame +X right / +Y up / looks -Z.
    返回 4x4: camera-to-world。
    """
    forward = target - eye
    forward = forward / np.linalg.norm(forward)
    right = np.cross(forward, up_world)
    right = right / np.linalg.norm(right)
    up = np.cross(right, forward)
    pose = np.eye(4, dtype=np.float64)
    pose[:3, 0] = right
    pose[:3, 1] = up
    pose[:3, 2] = -forward  # camera looks along -Z of its own frame
    pose[:3, 3] = eye
    return pose


# MC +Y = 下，所以 world up = (0, -1, 0)
MC_UP = np.array([0.0, -1.0, 0.0])


def scene_bbox(item_bounds: np.ndarray, show_arm: bool) -> tuple[np.ndarray, np.ndarray]:
    """item_bounds: [[xmin,ymin,zmin],[xmax,ymax,zmax]] （item 在 display transform 之后的 AABB）。
    合并 arm 参考 box 得到场景总 bbox。返回 (center, extents)。"""
    boxes = [item_bounds]
    if show_arm:
        arm_center = np.array([0.0, -6.0, 0.0])
        arm_half = np.array([2.0, 6.0, 2.0])
        boxes.append(np.stack([arm_center - arm_half, arm_center + arm_half]))
    lo = np.min(np.stack([b[0] for b in boxes]), axis=0)
    hi = np.max(np.stack([b[1] for b in boxes]), axis=0)
    return (lo + hi) / 2.0, (hi - lo)


def _frame_distance(extents: np.ndarray, fov_rad: float, fit_ratio: float = 0.8) -> float:
    """给定场景 extents，返回能让 bbox 填 fit_ratio 纵向 FOV 的相机距离。"""
    max_dim = float(np.max(extents))
    return max_dim / (2.0 * math.tan(fov_rad / 2.0) * fit_ratio)


def make_camera_for_mode(
    mode: str,
    item_bounds: np.ndarray,
    show_arm: bool,
) -> tuple[pyrender.Camera, np.ndarray]:
    """每个 display mode 的相机放置 + auto-frame。

    scene 约定:
      - 原点 (0,0,0) = 右手腕（display transform 作用点）
      - 手臂参考 box 在 (0, -6, 0)，extents (4, 12, 4)  —— +Y = 下 in MC 所以 -Y 方向 = 上
      - Item 在 display transform 后的 AABB 由调用方算好传入
    """
    center, extents = scene_bbox(item_bounds, show_arm)

    if mode.startswith("thirdperson"):
        # 固定聚焦"手腕周边"——不 auto-frame 到 arm 完整 bbox，否则 arm 12 高会把剑压成 2px
        fov = math.radians(35.0)
        # 以 item center + hand(0,0,0) 中点为 target，保证剑居中
        item_center = (item_bounds[0] + item_bounds[1]) / 2.0
        target = (item_center + np.zeros(3)) / 2.0
        # 框一个固定半径（item 大半径 + 3 最少）
        item_radius = float(np.max(item_bounds[1] - item_bounds[0])) / 2.0
        view_radius = max(item_radius + 3.0, 6.0)
        dist = view_radius / math.tan(fov / 2.0) / 0.7
        # 玩家右后上 45° 角
        eye_dir = np.array([-0.45, -0.45, 1.0])
        eye_dir = eye_dir / np.linalg.norm(eye_dir)
        eye = target + eye_dir * dist
        pose = _look_at_pose(eye, target, MC_UP)
        return pyrender.PerspectiveCamera(yfov=fov, znear=0.5, zfar=500.0), pose

    if mode.startswith("firstperson"):
        # FP 模拟玩家 eye 看过自己的手。真 MC 的 first-person 会把 arm 额外前推 9/-8/-12
        # px 到屏幕右下固定位置，这里不还原那个 arm 前推——只渲染 item 本身相对 hand
        # 的 display transform 结果。相机从 eye 方向靠近 item + hand 中点，距离按 item
        # 大小自动调整，保证能看清。
        fov = math.radians(55.0)
        item_center = (item_bounds[0] + item_bounds[1]) / 2.0
        target = (item_center + np.zeros(3)) / 2.0
        # 眼方向: body (+6, -16, 0) 朝 hand (0, 0, 0) 的单位向量的反方向 （"退眼位置"）
        eye_dir = np.array([0.35, -0.85, 0.4])  # 略右、上、后
        eye_dir = eye_dir / np.linalg.norm(eye_dir)
        item_radius = float(np.max(item_bounds[1] - item_bounds[0])) / 2.0
        view_radius = max(item_radius + 2.0, 4.0)
        dist = view_radius / math.tan(fov / 2.0) / 0.7
        eye = target + eye_dir * dist
        pose = _look_at_pose(eye, target, MC_UP)
        return pyrender.PerspectiveCamera(yfov=fov, znear=0.1, zfar=500.0), pose

    if mode == "gui":
        # 正交，从 +Z 朝 origin 看；取 item bounds 决定 ortho 宽度
        half = max(float(np.max(extents)) / 2.0, 1.0) * 1.2
        eye = center + np.array([0.0, 0.0, 40.0])
        target = center.copy()
        pose = _look_at_pose(eye, target, MC_UP)
        cam = pyrender.OrthographicCamera(xmag=half, ymag=half, znear=0.1, zfar=200.0)
        return cam, pose

    # fallback (ground / fixed etc.)
    fov = math.radians(50.0)
    dist = max(_frame_distance(extents, fov, fit_ratio=0.7), 15.0)
    eye = center + np.array([0.0, 0.0, 1.0]) * dist
    pose = _look_at_pose(eye, center, MC_UP)
    return pyrender.PerspectiveCamera(yfov=fov, znear=0.5, zfar=500.0), pose


def _item_bounds_after_transform(meshes: list[pyrender.Mesh], M: np.ndarray) -> np.ndarray:
    """Item AABB after applying 4x4 display transform M."""
    pts = []
    for m in meshes:
        for prim in m.primitives:
            pts.append(prim.positions)
    if not pts:
        return np.array([[0, 0, 0], [0, 0, 0]], dtype=np.float64)
    verts = np.concatenate(pts, axis=0)
    homo = np.concatenate([verts, np.ones((verts.shape[0], 1))], axis=1)
    transformed = (M @ homo.T).T[:, :3]
    return np.stack([transformed.min(axis=0), transformed.max(axis=0)])


def render_mode(
    asset_id: str,
    mode: str,
    display_cfg: dict,
    item_meshes: list[pyrender.Mesh],
    out_path: Path,
    width: int,
    height: int,
    show_arm: bool,
    bg_color: tuple[float, float, float],
) -> None:
    scene = pyrender.Scene(
        bg_color=[*bg_color, 1.0],
        ambient_light=np.array([0.35, 0.35, 0.35]),
    )

    # item: 应用 display transform
    M = display_transform_matrix(display_cfg)
    for mesh in item_meshes:
        scene.add(mesh, pose=M)

    # 参考几何（按模式）：
    #   - thirdperson: arm + 0.4 cube hand marker  —— 能看出 "剑穿不穿手臂 / item 握点"
    #   - firstperson: 只渲 item —— 真 MC FPV 手臂是单独 transform，参考 arm 会挡 item
    #   - gui / fixed: 只渲 item —— 展示单体
    draw_arm = show_arm and mode.startswith("thirdperson")
    if draw_arm:
        arm = build_reference_arm()
        scene.add(pyrender.Mesh.from_trimesh(arm, smooth=False))

    if mode.startswith("thirdperson"):
        # 小 marker：0.4 cube，不会挡到剑基本上都在 item AABB 之外
        marker = trimesh.creation.box(extents=(0.4, 0.4, 0.4))
        marker.visual.face_colors = [220, 90, 90, 255]
        scene.add(pyrender.Mesh.from_trimesh(marker, smooth=False))

    # 基于 item transformed bounds auto-frame
    item_bounds = _item_bounds_after_transform(item_meshes, M)
    cam, cam_pose = make_camera_for_mode(mode, item_bounds, draw_arm)
    scene.add(cam, pose=cam_pose)

    # 两个方向光 —— MC 风格 key + fill
    light_key = pyrender.DirectionalLight(color=np.ones(3), intensity=3.5)
    key_pose = _look_at_pose(
        np.array([-20.0, -30.0, 20.0]), np.array([0.0, 0.0, 0.0]), MC_UP
    )
    scene.add(light_key, pose=key_pose)
    light_fill = pyrender.DirectionalLight(color=np.ones(3), intensity=1.2)
    fill_pose = _look_at_pose(
        np.array([20.0, 10.0, 20.0]), np.array([0.0, 0.0, 0.0]), MC_UP
    )
    scene.add(light_fill, pose=fill_pose)

    r = pyrender.OffscreenRenderer(width, height)
    try:
        color, _ = r.render(scene)
    finally:
        r.delete()

    # 叠一行文字 label
    img = Image.fromarray(color)
    from PIL import ImageDraw, ImageFont
    draw = ImageDraw.Draw(img)
    try:
        font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 13)
    except OSError:
        font = ImageFont.load_default()
    rot = display_cfg["rotation"]
    trs = display_cfg["translation"]
    scl = display_cfg["scale"]
    label = (
        f"{asset_id}  [{mode}]\n"
        f"rot={rot}  trs={trs}  scl={scl}"
    )
    # 文字背景条
    draw.rectangle([0, 0, width, 40], fill=(0, 0, 0, 180))
    draw.text((6, 4), label, fill=(230, 230, 230), font=font)
    img.save(out_path)


def stitch_grid(paths: list[Path], out_path: Path) -> None:
    imgs = [Image.open(p) for p in paths]
    if not imgs:
        return
    w = max(i.width for i in imgs)
    total_h = sum(i.height for i in imgs) + 4 * (len(imgs) - 1)
    grid = Image.new("RGB", (w, total_h), (255, 255, 255))
    y = 0
    for img in imgs:
        grid.paste(img, (0, y))
        y += img.height + 4
    grid.save(out_path)


# ━━━━━ CLI ━━━━━

def parse_args(argv: list[str]) -> argparse.Namespace:
    ap = argparse.ArgumentParser(description="render Bong OBJ weapon in MC handheld pose (headless)")
    ap.add_argument("asset_id", help="asset id (对应 client/tools/asset_configs/<id>.json 和 models/item/<id>/)")
    ap.add_argument("--out", type=Path, default=None, help="输出目录 (默认 client/tools/renders/<asset_id>/)")
    ap.add_argument(
        "--modes",
        nargs="+",
        default=list(DEFAULT_MODES),
        choices=list(DISPLAY_MODE_KEYS),
        help=f"要渲染的 display 模式 (默认 {list(DEFAULT_MODES)})",
    )
    ap.add_argument("--width", type=int, default=512)
    ap.add_argument("--height", type=int, default=512)
    ap.add_argument("--no-arm", action="store_true", help="不画手臂参考 cuboid")
    ap.add_argument("--bg", default="0.16,0.16,0.18", help="背景色 R,G,B (0..1)")
    return ap.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    asset_id = args.asset_id
    out_dir = args.out or (Path(__file__).resolve().parent / "renders" / asset_id)
    out_dir.mkdir(parents=True, exist_ok=True)

    bg = tuple(float(x) for x in args.bg.split(","))
    if len(bg) != 3:
        raise SystemExit(f"--bg 必须是 R,G,B，收到 {args.bg}")

    override = load_display_override(asset_id)
    display_final = merge_display(HANDHELD_DISPLAY, override)

    override_str = ", ".join(sorted(override.keys())) if override else "(无 asset config)"
    print(f"[render_held_item] asset={asset_id}  override: {override_str}")
    print(f"[render_held_item] 输出目录: {out_dir}")

    meshes = load_item_meshes(asset_id)
    print(f"[render_held_item] 加载 {len(meshes)} 个 material 子 mesh")

    rendered: list[Path] = []
    for mode in args.modes:
        cfg = display_final[mode]
        out_png = out_dir / f"{mode}.png"
        print(f"  → 渲染 {mode}  rot={cfg['rotation']}  trs={cfg['translation']}  scl={cfg['scale']}")
        render_mode(
            asset_id=asset_id,
            mode=mode,
            display_cfg=cfg,
            item_meshes=meshes,
            out_path=out_png,
            width=args.width,
            height=args.height,
            show_arm=not args.no_arm,
            bg_color=bg,
        )
        rendered.append(out_png)

    grid_path = out_dir / "grid.png"
    stitch_grid(rendered, grid_path)
    print(f"[render_held_item] grid → {grid_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
