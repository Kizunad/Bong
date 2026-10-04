#!/usr/bin/env python3
"""方块人 + 手持物的三视图审图渲染（物品使用动画 Round 1 起用）。

读 emotecraft v3 动画 JSON，按原版 MC 1.20.1 玩家模型的方块尺寸画出头、躯干、
上臂 / 前臂、大腿 / 小腿，并把手持物按原版手持链路放进手里。输出三视图：
正面、侧面（玩家右侧）、3/4 斜前。

为什么不用 render_animation.py：它把四肢画成线段，看不出握的是哪一截、体积朝哪边，
也画不出手里的物品，判不了握法。本工具补的正是这两点。

姿态解算沿用 render_animation.py 的 §10 管线（ModelPart 的 ZYX 旋转、bendy-lib 折弯），
与它有两处刻意的差异，看图前要知道：
  - body / 部件的 x,y,z 按方块解释，乘 16 换算成像素（docs/player-animation-conventions.md
    §0 与 anim_common.py 的 ITEM_PARTS 注释）。render_animation.py 直接当像素用，位移偏小 16 倍。
  - rightItem / leftItem 关键帧：bbmodel_maker 的 BODY_PART_NAMES 不含它们，这里自己收集。

手持链路（1.20.1 字节码确认：HumanoidArm 的 setArmAngle = ModelPart.rotate，
ItemInHandLayer 的 rotX(-90°)、rotY(180°)、translate((±1/16, 0.125, -0.625))，
ItemRenderer 的 display = translate · rotationXYZ · scale，末尾 translate(-0.5)）。
PlayerAnimator 的 rightItem 插在 display 之后、手持偏移之前。合起来，手持物顶点在
前臂局部系里的位置是

    p = P · ( R_item · D(v) + t )
    D(v) = d + Rxyz(rot) · s · (v - 0.5)          （方块，d = translation/16）
    P    = rotX(-90°) · rotY(180°)，t = (±1, 2, -10) px

握点 P·t 落在前臂末端（约 y=10px），与手掌位置对得上，这是这条链路的旁证。

已知近似（看图时记住）：
  - 手持物随前臂折弯整体刚性转动；原版 bend 只弯前臂的网格，手持物不做网格变形。
  - 正交投影，没有光照与 FOV；物品只用面法线做明暗。
  - 握持是否自然的最终判断仍以 runClient 为准。
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

import numpy as np
from PIL import Image, ImageDraw, ImageFont

from bbmodel_maker.rig.emote_anim import AXIS_NAMES, sample_axis
from render_animation import (
    PIVOTS,
    part_rotation_matrix,
    rot_x,
    rot_y,
    rot_z,
    rotate_about_axis,
)

CLIENT_ROOT = Path(__file__).resolve().parents[1]
RESOURCES = CLIENT_ROOT / "src/main/resources/assets/bong"
ANIM_DIR = RESOURCES / "player_animation"
ITEM_MODEL_DIR = RESOURCES / "models/item"
ASSET_CONFIG_DIR = Path(__file__).resolve().parent / "asset_configs"

BLOCK_PX = 16.0
META_KEYS = frozenset({"tick", "easing", "comment", "turn"})

# 手持物顶点数上限：超出按步长抽稀，保证一张图几秒内出完。
MAX_ITEM_TRIANGLES = 1500

# 手持链路常量（见模块文档）。
HAND_PRE_ROT = rot_x(-math.pi / 2) @ rot_y(math.pi)
HAND_OFFSET_PX = {
    "right": np.array([1.0, 2.0, -10.0]),
    "left": np.array([-1.0, 2.0, -10.0]),
}
# tripo_to_sml.HANDHELD_DISPLAY 的镜像（改一边请同步另一边，见 render_held_item.py）。
HANDHELD_DEFAULT = {
    "thirdperson_righthand": {"rotation": [0.0, -90.0, 55.0], "translation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0]},
    "thirdperson_lefthand": {"rotation": [0.0, 90.0, -55.0], "translation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0]},
}

# 三视图的（屏幕右向, 朝向镜头）。正面 = 面向玩家脸；侧面 = 玩家右侧；3/4 = 两者之间。
_R_FRONT = np.array([-1.0, 0.0, 0.0])
_F_FRONT = np.array([0.0, 0.0, -1.0])
_R_SIDE = np.array([0.0, 0.0, -1.0])
_F_SIDE = np.array([-1.0, 0.0, 0.0])
VIEWS: Dict[str, Tuple[str, np.ndarray, np.ndarray]] = {
    "front": ("正面", _R_FRONT, _F_FRONT),
    "side": ("侧面（玩家右侧）", _R_SIDE, _F_SIDE),
    "34": ("3/4 斜前", (_R_FRONT + _R_SIDE) / math.sqrt(2), (_F_FRONT + _F_SIDE) / math.sqrt(2)),
}
VIEW_ORDER = ("front", "side", "34")

PANEL_W = 220
PANEL_H = 250
PX_SCALE = 5.2       # 屏幕像素 / 模型像素
PANEL_CX = PANEL_W // 2
PANEL_OY = 100       # 脚底（y=24px）落在面板偏下位置，头顶上方留给标题

# 每个 part 的方块段：(段名, 相对 pivot 的 lo, hi, 是否为折弯后段)。单位像素。
# 折弯段（前臂 / 小腿）绕 "lo.y" 那一条线（肘 / 膝）折。
PART_SEGMENTS: Dict[str, Tuple[Tuple[str, Tuple[float, float, float], Tuple[float, float, float], bool], ...]] = {
    "head": (("head", (-4, -8, -4), (4, 0, 4), False),),
    "torso": (("torso", (-4, 0, -2), (4, 12, 2), False),),
    "rightArm": (
        ("upper", (-3, -2, -2), (1, 4, 2), False),
        ("forearm", (-3, 4, -2), (1, 10, 2), True),
    ),
    "leftArm": (
        ("upper", (1, -2, -2), (5, 4, 2), False),
        ("forearm", (1, 4, -2), (5, 10, 2), True),
    ),
    "rightLeg": (
        ("thigh", (-2, 0, -2), (2, 6, 2), False),
        ("shin", (-2, 6, -2), (2, 12, 2), True),
    ),
    "leftLeg": (
        ("thigh", (0, 0, -2), (4, 6, 2), False),
        ("shin", (0, 6, -2), (4, 12, 2), True),
    ),
}

PART_COLORS = {
    "head": (222, 184, 140),
    "torso": (72, 112, 186),
    "arm": (72, 112, 186),
    "leg": (52, 52, 96),
}
EDGE_COLOR = (30, 30, 40)

# 手持物颜色刻意避开肤色（222,184,140）与袖子蓝，否则一眼分不出手里拿的是什么。
ITEM_COLORS = {
    "cloth": (200, 58, 52),
    "iron": (120, 128, 142),
    "bone": (244, 240, 226),
    "wood": (112, 64, 28),
}


@dataclass(frozen=True)
class ItemModel:
    """一件手持物的模型来源与显示参数来源。"""

    color: Tuple[int, int, int]
    obj: Optional[str] = None               # 相对 models/item 的 OBJ 路径
    box_blocks: Optional[float] = None      # 没有可用网格时，用这个边长（方块）的立方体示意
    display_json: Optional[str] = None      # 物品模型 JSON（取 display 段）
    override_json: Optional[str] = None     # asset_configs 覆盖
    note: str = ""


# 注册表（BongWeaponModelRegistry）登记的模型与 v2 模型不一致时，选 v2：
#   - 骨剑：注册表 objPath=null（借原版石剑贴图），bone_sword_slash 生成器读的是 bone_sword_v2。
#   - 木杖：注册表 wooden_staff.obj 高 10 格、无缩放配置，wooden_staff_v2 高 1.76 格，与设计的 1.6m 一致。
#   - 骨镐：注册表 pickaxe_bone.obj 只有 16 个顶点（一块薄板），pickaxe_bone_v2 才是镐形。
# 三处不一致都写进 model-review/item-anim-batch1.md，等调度定。
ITEMS: Dict[str, ItemModel] = {
    # 拳套不用 hand_wrap.obj：它是双手整套护甲网格（x 跨 3.75 格、z 跨 6 格），按现有
    # asset_configs 的 scale 0.35 仍约 2 格长，远超手掌（约 0.25 格），画出来是一团布片，
    # 判不了握法。这里用拳头尺寸的立方体示意，握点与出拳方向才是要看的东西。
    "hand_wrap": ItemModel(
        color=ITEM_COLORS["cloth"],
        box_blocks=0.3,
        note="示意拳面 0.3 格立方体：hand_wrap.obj 按现有配置约 2 格，不可用于判握法（见文档）",
    ),
    "bing_jia_shou_tao": ItemModel(
        color=ITEM_COLORS["iron"],
        box_blocks=0.34,
        note="示意铁甲拳面 0.34 格立方体（注册表 objPath=null，实际为原版皮革 2D 贴图）",
    ),
    "pickaxe_bone_v2": ItemModel(
        obj="pickaxe_bone_v2/pickaxe_bone_v2.obj",
        color=ITEM_COLORS["bone"],
        display_json="pickaxe_bone_v2/pickaxe_bone_v2.json",
        note="骨镐 v2 模型（注册表登记的是 pickaxe_bone.obj，见上）",
    ),
    "wooden_staff_v2": ItemModel(
        obj="wooden_staff_v2/wooden_staff_v2.obj",
        color=ITEM_COLORS["wood"],
        display_json="wooden_staff_v2/wooden_staff_v2.json",
        note="木杖 v2 模型（注册表登记的是 wooden_staff.obj，见上）",
    ),
    "bone_sword_v2": ItemModel(
        obj="bone_sword_v2/bone_sword_v2.obj",
        color=ITEM_COLORS["bone"],
        display_json="bone_sword_v2/bone_sword_v2.json",
        note="骨剑 v2 模型（注册表 objPath=null，见上）",
    ),
    "iron_sword_v2": ItemModel(
        obj="iron_sword_v2/iron_sword_v2.obj",
        color=ITEM_COLORS["iron"],
        display_json="iron_sword_v2/iron_sword_v2.json",
        note="对照用：铁剑 v2",
    ),
    "pickaxe_iron_v2": ItemModel(
        obj="pickaxe_iron_v2/pickaxe_iron_v2.obj",
        color=ITEM_COLORS["iron"],
        display_json="pickaxe_iron_v2/pickaxe_iron_v2.json",
        note="对照用：铁镐 v2",
    ),
}

# 本批新动画 → 手里拿的东西。(手, 物品 id)。拳套两手都戴。木杖是双手持，但原版只有
# 右手能持物品，所以这里只画右手那一端，左手的位置要人看着判断它在不在杖上。
ANIMATION_HOLDS: Dict[str, Tuple[Tuple[str, str], ...]] = {
    "hand_wrap_jab_left": (("left", "hand_wrap"), ("right", "hand_wrap")),
    "hand_wrap_jab_right": (("left", "hand_wrap"), ("right", "hand_wrap")),
    "bing_jia_heavy_left": (("left", "bing_jia_shou_tao"), ("right", "bing_jia_shou_tao")),
    "bing_jia_heavy_right": (("left", "bing_jia_shou_tao"), ("right", "bing_jia_shou_tao")),
    "pickaxe_bone_use": (("right", "pickaxe_bone_v2"),),
    "wooden_staff_atk": (("right", "wooden_staff_v2"),),
    "bone_sword_slash": (("right", "bone_sword_v2"),),
}

# 同类旧动画（对照行）：(动画 id, 手里拿的物品)。
REFERENCES: Dict[str, Tuple[str, Tuple[Tuple[str, str], ...]]] = {
    "hand_wrap_jab_left": ("fist_punch_left", ()),
    "hand_wrap_jab_right": ("fist_punch_right", ()),
    "bing_jia_heavy_left": ("fist_punch_left", ()),
    "bing_jia_heavy_right": ("fist_punch_right", ()),
    "pickaxe_bone_use": ("pickaxe_iron_v2_use", (("right", "pickaxe_iron_v2"),)),
    "wooden_staff_atk": ("sword_swing_horiz", ()),
    "bone_sword_slash": ("iron_sword_v2_use", (("right", "iron_sword_v2"),)),
}


# ━━━━━ 关键帧收集 ━━━━━


def collect_keyframes(emote: dict) -> Dict[str, Dict[str, List[Tuple[int, float, str]]]]:
    """{part: {axis: [(tick, value, easing), ...]}}，包含 rightItem / leftItem。"""
    kfs: Dict[str, Dict[str, List[Tuple[int, float, str]]]] = {}
    for move in emote["moves"]:
        tick = int(move["tick"])
        easing = move.get("easing", "linear")
        for key, val in move.items():
            if key in META_KEYS or not isinstance(val, dict):
                continue
            for axis, value in val.items():
                if axis not in AXIS_NAMES:
                    continue
                kfs.setdefault(key, {}).setdefault(axis, []).append((tick, float(value), easing))
    for part in kfs.values():
        for track in part.values():
            track.sort(key=lambda item: item[0])
    return kfs


def sample(kfs, part: str, tick: float) -> Dict[str, float]:
    return {axis: sample_axis(kfs, part, axis, tick) for axis in AXIS_NAMES}


def keyframe_ticks(kfs) -> List[int]:
    ticks = set()
    for part in kfs.values():
        for track in part.values():
            ticks.update(int(t) for t, _, _ in track)
    return sorted(ticks)


# ━━━━━ 几何 ━━━━━


def _apply(mat: np.ndarray, pts: np.ndarray) -> np.ndarray:
    return pts @ mat.T


def _bend_matrix(axis_rad: float, bend_rad: float) -> np.ndarray:
    axis_vec = np.array([math.cos(axis_rad), 0.0, math.sin(axis_rad)])
    return rotate_about_axis(axis_vec, bend_rad)


def _bend_points(pts: np.ndarray, center: np.ndarray, mat: np.ndarray) -> np.ndarray:
    return (pts - center) @ mat.T + center


def _box_corners(lo, hi) -> np.ndarray:
    """8 个角，下标 i*4 + j*2 + k，i/j/k 对应 x/y/z 的 lo(0)/hi(1)。"""
    xs, ys, zs = (lo[0], hi[0]), (lo[1], hi[1]), (lo[2], hi[2])
    return np.array([[x, y, z] for x in xs for y in ys for z in zs], dtype=np.float64)


def _box_faces() -> List[List[int]]:
    faces = []
    cycle = ((0, 0), (0, 1), (1, 1), (1, 0))
    for axis in range(3):
        others = [a for a in range(3) if a != axis]
        for side in (0, 1):
            quad = []
            for u, w in cycle:
                bits = [0, 0, 0]
                bits[axis] = side
                bits[others[0]] = u
                bits[others[1]] = w
                quad.append(bits[0] * 4 + bits[1] * 2 + bits[2])
            faces.append(quad)
    return faces


BOX_FACES = _box_faces()


def _obj_triangles(path: Path) -> np.ndarray:
    """OBJ → (T, 3, 3) 三角形，单位是方块（sml:builtin/obj 的约定）。多边形按扇形剖分。"""
    verts: List[List[float]] = []
    tris: List[List[List[float]]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "v":
            verts.append([float(parts[1]), float(parts[2]), float(parts[3])])
        elif parts[0] == "f":
            idx = [int(tok.split("/")[0]) - 1 for tok in parts[1:]]
            for k in range(1, len(idx) - 1):
                tris.append([verts[idx[0]], verts[idx[k]], verts[idx[k + 1]]])
    arr = np.array(tris, dtype=np.float64)
    if len(arr) > MAX_ITEM_TRIANGLES:
        stride = math.ceil(len(arr) / MAX_ITEM_TRIANGLES)
        arr = arr[::stride]
    return arr


def _box_triangles(size_blocks: float) -> np.ndarray:
    """中心在 (0.5, 0.5, 0.5)（方块坐标）的立方体三角形，与 OBJ 的坐标约定一致。"""
    half = size_blocks / 2
    corners = _box_corners((0.5 - half,) * 3, (0.5 + half,) * 3)
    tris = []
    for face in BOX_FACES:
        tris.append([corners[face[0]], corners[face[1]], corners[face[2]]])
        tris.append([corners[face[0]], corners[face[2]], corners[face[3]]])
    return np.array(tris, dtype=np.float64)


def _item_triangles(item: ItemModel) -> np.ndarray:
    if item.box_blocks is not None:
        return _box_triangles(item.box_blocks)
    return _obj_triangles(ITEM_MODEL_DIR / item.obj)


def _display_for(item: ItemModel, hand_key: str) -> dict:
    display = {k: list(v) for k, v in HANDHELD_DEFAULT[hand_key].items()}
    if item.display_json:
        src = json.loads((ITEM_MODEL_DIR / item.display_json).read_text(encoding="utf-8"))
        display.update(src["display"][hand_key])
    if item.override_json:
        cfg = json.loads((ASSET_CONFIG_DIR / item.override_json).read_text(encoding="utf-8"))
        display.update(cfg.get(hand_key, {}))
    return display


def _display_transform(display: dict) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
    """(Rxyz, scale, translation_px)。JOML rotationXYZ = Rx·Ry·Rz，角度为度。"""
    rx, ry, rz = (math.radians(v) for v in display["rotation"])
    rot = rot_x(rx) @ rot_y(ry) @ rot_z(rz)
    scale = np.array(display["scale"], dtype=np.float64)
    translation_px = np.array(display["translation"], dtype=np.float64)
    return rot, scale, translation_px


# ━━━━━ 姿态解算 ━━━━━


def _part_color(part: str) -> Tuple[int, int, int]:
    if part == "head":
        return PART_COLORS["head"]
    if part == "torso":
        return PART_COLORS["torso"]
    if "Arm" in part:
        return PART_COLORS["arm"]
    return PART_COLORS["leg"]


@dataclass(frozen=True)
class Scene:
    """一个 tick 的场景，全部在世界系（MC 像素，+Y 向下）。

    quads: (四角, 颜色, 所属方块的世界中心)——中心用来把面法线定向到外侧。
    tris:  (T, 3, 3) 三角形, 颜色)——手持物。
    """

    quads: List[Tuple[np.ndarray, Tuple[int, int, int], np.ndarray]]
    tris: List[Tuple[np.ndarray, Tuple[int, int, int]]]


def build_scene(kfs, tick: float, holds: Sequence[Tuple[str, str]]) -> Scene:
    body = sample(kfs, "body", tick)
    body_rot = part_rotation_matrix(body["pitch"], body["yaw"], body["roll"])
    body_pos = np.array([body["x"], body["y"], body["z"]]) * BLOCK_PX

    quads = []
    arm_frames: Dict[str, Tuple[np.ndarray, np.ndarray]] = {}
    elbow_bends: Dict[str, Tuple[np.ndarray, np.ndarray]] = {}

    for part, segments in PART_SEGMENTS.items():
        p = sample(kfs, part, tick)
        pivot = np.array(PIVOTS[part], dtype=np.float64) + np.array([p["x"], p["y"], p["z"]]) * BLOCK_PX
        r_part = part_rotation_matrix(p["pitch"], p["yaw"], p["roll"])
        if part.endswith("Arm"):
            arm_frames[part] = (pivot, r_part)
        bend = _bend_matrix(p["axis"], p["bend"])
        for _name, lo, hi, bent in segments:
            corners = _box_corners(lo, hi)
            if bent:
                # 折弯中心：段的 x/z 中心，y 取折线（肘 / 膝所在的那一行）。
                center = np.array([(lo[0] + hi[0]) / 2, lo[1], (lo[2] + hi[2]) / 2], dtype=np.float64)
                corners = _bend_points(corners, center, bend)
                if part.endswith("Arm"):
                    elbow_bends[part] = (center, bend)
            world = _apply(body_rot, _apply(r_part, corners) + pivot) + body_pos
            box_center = world.mean(axis=0)
            color = _part_color(part)
            for face in BOX_FACES:
                quads.append((world[face], color, box_center))

    tris = []
    for side, item_id in holds:
        item = ITEMS[item_id]
        hand_part = "rightArm" if side == "right" else "leftArm"
        pivot, r_part = arm_frames[hand_part]
        center, bend = elbow_bends[hand_part]
        display = _display_for(item, f"thirdperson_{'righthand' if side == 'right' else 'lefthand'}")
        rot_disp, scale, trans_px = _display_transform(display)
        verts = _item_triangles(item)
        flat = verts.reshape(-1, 3)
        # 手持物自己的旋转（rightItem 关键帧）插在 display 之后、手持偏移之前。
        spin_part = "rightItem" if side == "right" else "leftItem"
        spin = sample(kfs, spin_part, tick)
        r_item = part_rotation_matrix(spin["pitch"], spin["yaw"], spin["roll"])
        disp = trans_px + BLOCK_PX * (((flat - 0.5) * scale) @ rot_disp.T)
        hand_local = (disp @ r_item.T + HAND_OFFSET_PX[side]) @ HAND_PRE_ROT.T
        hand_local = _bend_points(hand_local, center, bend)
        world = _apply(body_rot, _apply(r_part, hand_local) + pivot) + body_pos
        tris.append((world.reshape(-1, 3, 3), item.color))

    return Scene(quads=quads, tris=tris)


# ━━━━━ 投影与绘制 ━━━━━


def _unit(v: np.ndarray) -> np.ndarray:
    norm = float(np.linalg.norm(v))
    return v / norm if norm > 1e-9 else v


def _shade(color: Tuple[int, int, int], k: float) -> Tuple[int, int, int]:
    return tuple(max(0, min(255, int(c * k))) for c in color)


def _project(pts: np.ndarray, right: np.ndarray, ox: int, oy: int) -> List[Tuple[float, float]]:
    """正交投影：屏幕 x 取 right 方向，屏幕 y 直接取 MC 的 +Y（向下）。"""
    sx = ox + PX_SCALE * (pts @ right)
    sy = oy + PX_SCALE * pts[:, 1]
    return [(float(x), float(y)) for x, y in zip(sx, sy)]


def draw_view(draw: ImageDraw.ImageDraw, scene: Scene, view: str, ox: int, oy: int) -> None:
    """画一个视角。背面剔除方块面，手持物三角形双面画；按深度从远到近画（画家算法）。"""
    _label, right, toward = VIEWS[view]
    polys = []
    for quad, color, box_center in scene.quads:
        n = _unit(np.cross(quad[1] - quad[0], quad[3] - quad[0]))
        if np.dot(n, quad.mean(axis=0) - box_center) < 0:
            n = -n
        facing = float(np.dot(n, toward))
        if facing <= 0:
            continue
        depth = float(np.dot(quad.mean(axis=0), toward))
        polys.append((depth, _project(quad, right, ox, oy), _shade(color, 0.5 + 0.5 * facing), True))
    for tri_set, color in scene.tris:
        for tri in tri_set:
            n = _unit(np.cross(tri[1] - tri[0], tri[2] - tri[0]))
            depth = float(np.dot(tri.mean(axis=0), toward))
            shade = 0.45 + 0.55 * abs(float(np.dot(n, toward)))
            polys.append((depth, _project(tri, right, ox, oy), _shade(color, shade), False))
    polys.sort(key=lambda item: item[0])
    for _depth, pts, color, outlined in polys:
        if outlined:
            draw.polygon(pts, fill=color, outline=EDGE_COLOR)
        else:
            draw.polygon(pts, fill=color)


def render_frame(kfs, tick: float, holds, title: str, font) -> Image.Image:
    """一个 tick 的三视图，横排。"""
    scene = build_scene(kfs, tick, holds)
    img = Image.new("RGB", (PANEL_W * len(VIEW_ORDER), PANEL_H), (248, 248, 250))
    for i, view in enumerate(VIEW_ORDER):
        panel = Image.new("RGB", (PANEL_W, PANEL_H), (248, 248, 250))
        pdraw = ImageDraw.Draw(panel)
        pdraw.rectangle([0, 0, PANEL_W - 1, PANEL_H - 1], outline=(190, 190, 200))
        pdraw.text((5, 4), f"{title}  {VIEWS[view][0]}", fill=(40, 40, 60), font=font)
        draw_view(pdraw, scene, view, PANEL_CX, PANEL_OY)
        img.paste(panel, (i * PANEL_W, 0))
    return img


def _load_font(size: int):
    try:
        return ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", size)
    except OSError:
        return ImageFont.load_default()


def render_strip(anim_id: str, holds, label: str) -> Image.Image:
    """一个动画的条带：每列一个 tick（三视图横排），行头写 tick 号。"""
    path = ANIM_DIR / f"{anim_id}.json"
    emote = json.loads(path.read_text(encoding="utf-8"))["emote"]
    if emote.get("degrees", True):
        raise ValueError(f"{path.name}: emote.degrees 必须为 false（本工具按弧度解释）")
    kfs = collect_keyframes(emote)
    ticks = keyframe_ticks(kfs)
    font = _load_font(11)
    head_font = _load_font(13)
    frames = [render_frame(kfs, t, holds, f"t={t}", font) for t in ticks]
    width = sum(f.width for f in frames)
    header_h = 26
    strip = Image.new("RGB", (width, header_h + frames[0].height), (255, 255, 255))
    ImageDraw.Draw(strip).text((8, 6), f"{label}   [{anim_id}]", fill=(20, 20, 30), font=head_font)
    x = 0
    for frame in frames:
        strip.paste(frame, (x, header_h))
        x += frame.width
    return strip


def render_pair(anim_id: str, holds, ref: Optional[Tuple[str, Tuple[Tuple[str, str], ...]]]) -> Image.Image:
    own = render_strip(anim_id, holds, "新动画")
    if ref is None:
        return own
    ref_id, ref_holds = ref
    old = render_strip(ref_id, ref_holds, "同类旧动画（对照）")
    width = max(own.width, old.width)
    pair = Image.new("RGB", (width, own.height + old.height + 12), (255, 255, 255))
    pair.paste(own, (0, 0))
    pair.paste(old, (0, own.height + 12))
    return pair


def stack_vertical(images: List[Image.Image]) -> Image.Image:
    width = max(im.width for im in images)
    height = sum(im.height for im in images) + 24 * len(images)
    sheet = Image.new("RGB", (width, height), (255, 255, 255))
    y = 0
    for im in images:
        sheet.paste(im, (0, y))
        y += im.height + 24
    return sheet


def main(argv: Optional[Sequence[str]] = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("anim_ids", nargs="+", help="player_animation 的 id，如 hand_wrap_jab_left")
    ap.add_argument("--out", type=Path, required=True, help="输出目录")
    ap.add_argument("--refs", action="store_true", help="每个动画下面附同类旧动画对照")
    ap.add_argument("--sheet", type=Path, default=None, help="把所有行拼成一张总接触表")
    args = ap.parse_args(argv)

    args.out.mkdir(parents=True, exist_ok=True)
    rows = []
    for anim_id in args.anim_ids:
        holds = ANIMATION_HOLDS.get(anim_id, ())
        ref = REFERENCES.get(anim_id) if args.refs else None
        row = render_pair(anim_id, holds, ref)
        out_path = args.out / f"{anim_id}.png"
        row.save(out_path)
        rows.append(row)
        print(f"wrote {out_path}")
    if args.sheet is not None:
        stack_vertical(rows).save(args.sheet)
        print(f"wrote {args.sheet}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
