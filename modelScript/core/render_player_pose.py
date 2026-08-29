#!/usr/bin/env python3
"""真实 cuboid + bend 变形的玩家姿态渲染器（headless）。

和 client/tools/render_animation.py 的分工：那个画火柴人骨架，快、看得清关节角度，
但看不出"腿腹断连""bend 把小臂折到哪"这类**体积**问题（它自己的 §11.3 限制 3 就写着
不画 bendable cuboid 的真实变形）。本脚本按 vanilla BipedEntityModel 的真实 cuboid
渲染，并复刻 bendy-lib 的 applyBend 语义，专门用来判断体积/穿模/断连。

坐标系：y 向上、脚在 y=0、脸朝 -Z（与 render_jian_in_hand 一致）。
MC ModelPart 空间是 y 向下的，换算 world_y = 24 - mc_y；随之而来的符号规则：
    pitch(绕X) → world 绕 X 转 -pitch
    yaw  (绕Y) → world 绕 Y 转 +yaw     （y 翻转不影响 XZ 平面内的旋转）
    roll (绕Z) → world 绕 Z 转 -roll
    bend 轴 (cos a, 0, sin a) 不变，但转角取 -bendValue（y 翻转是反射，改手性）

bend 的几何（IBendable.applyBend）：cuboid 沿自身 Y 从几何中心切开，靠 basePlane
一侧（手/脚那半）绕 (cos(bendAxis), 0, sin(bendAxis)) 轴、以几何中心为原点旋转
bendValue；另一半不动。本脚本用"切两半、下半整体旋转"实现——接缝处真实 bendy-lib
会把 quad 拉伸成楔形，这里是直角断口，差异只在那一条缝。

用法:
    python3 modelScript/core/render_player_pose.py --bend-matrix   # 弯曲能力扫描图
    python3 modelScript/core/render_player_pose.py --pose stand    # 单姿态三视图
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import render_bbmodel as R  # noqa: E402
import render_jian_in_hand as H  # noqa: E402
import workspace  # noqa: E402

# emotecraft JSON 采样原先是从 client/tools/render_animation.py 借的，那条依赖是反的：
# 渲染底座不该依赖调用方仓库的目录布局。现已提进 core/emote_anim.py。
import emote_anim as RA  # noqa: E402

_WS = workspace.Workspace.discover(start=Path(__file__))
REPO = _WS.root
OUT_MATRIX = _WS.out / "render_bend_matrix.png"
OUT_POSE = _WS.out / "render_player_pose.png"
OUT_ANIM = _WS.out / "render_anim_pose.png"
MIN_RENDER_SIZE = 1
MAX_RENDER_SIZE = 1024
ANGLE_AXES = frozenset(("pitch", "yaw", "roll", "bend", "axis"))
BODY_ROOT = np.array([0.0, 24.0, 0.0])


def _validate_size(size, context="size"):
    if isinstance(size, bool) or not isinstance(size, (int, np.integer)):
        raise ValueError(
            f"{context} must be an integer with {MIN_RENDER_SIZE} <= size <= "
            f"{MAX_RENDER_SIZE}; got {size!r}"
        )
    size = int(size)
    if not MIN_RENDER_SIZE <= size <= MAX_RENDER_SIZE:
        raise ValueError(
            f"{context} must satisfy {MIN_RENDER_SIZE} <= size <= {MAX_RENDER_SIZE}; "
            f"got {size}"
        )
    return size


def _parse_size(value):
    try:
        size = int(value)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(
            f"size must be an integer with {MIN_RENDER_SIZE} <= size <= {MAX_RENDER_SIZE}; "
            f"got {value!r}"
        ) from exc
    try:
        return _validate_size(size)
    except ValueError as exc:
        raise argparse.ArgumentTypeError(str(exc)) from exc


# vanilla BipedEntityModel（已换算到 y-up）：pivot / cuboid / bend 中心 / skin box-uv。
# bendable 取自 PlayerAnimator AnimationBuilder：head 与两个 item 槽不可 bend，其余可。
PARTS = {
    "head":     dict(pivot=(0.0, 24.0, 0.0), box=((-4, 24, -4), (4, 32, 4)),
                     bend_center=(0.0, 28.0, 0.0), bendable=False, uv=(0, 0)),
    "torso":    dict(pivot=(0.0, 24.0, 0.0), box=((-4, 12, -2), (4, 24, 2)),
                     bend_center=(0.0, 18.0, 0.0), bendable=True, uv=(16, 16)),
    "rightArm": dict(pivot=(-5.0, 22.0, 0.0), box=((-8, 12, -2), (-4, 24, 2)),
                     bend_center=(-6.0, 18.0, 0.0), bendable=True, uv=(40, 16)),
    "leftArm":  dict(pivot=(5.0, 22.0, 0.0), box=((4, 12, -2), (8, 24, 2)),
                     bend_center=(6.0, 18.0, 0.0), bendable=True, uv=(32, 48)),
    "rightLeg": dict(pivot=(-1.9, 12.0, 0.0), box=((-4, 0, -2), (0, 12, 2)),
                     bend_center=(-1.9, 6.0, 0.0), bendable=True, uv=(0, 16)),
    "leftLeg":  dict(pivot=(1.9, 12.0, 0.0), box=((0, 0, -2), (4, 12, 2)),
                     bend_center=(1.9, 6.0, 0.0), bendable=True, uv=(16, 48)),
}


def _rot_axis(axis, deg):
    """绕任意轴旋转（Rodrigues）。"""
    a = np.array(axis, float)
    a = a / np.linalg.norm(a)
    t = math.radians(deg)
    c, s = math.cos(t), math.sin(t)
    K = np.array([[0, -a[2], a[1]], [a[2], 0, -a[0]], [-a[1], a[0], 0]])
    return np.eye(3) * c + K * s + np.outer(a, a) * (1 - c)


def _stable_orthogonal_axis(direction):
    direction = np.asarray(direction, dtype=float)
    basis = np.eye(3)[int(np.argmin(np.abs(direction)))]
    axis = np.cross(direction, basis)
    return axis / np.linalg.norm(axis)


def _align_up_to_direction(direction):
    """Return a proper rotation that maps +Y to a nonzero direction."""
    target = np.asarray(direction, dtype=float)
    if target.shape != (3,) or not np.isfinite(target).all():
        raise ValueError(f"direction must be a finite 3-vector; got {direction!r}")
    norm = np.linalg.norm(target)
    if norm <= 1e-12:
        raise ValueError(f"direction must be nonzero; got {direction!r}")
    target = target / norm

    up = np.array([0.0, 1.0, 0.0])
    cross = np.cross(up, target)
    dot = float(np.clip(np.dot(up, target), -1.0, 1.0))
    cross_norm = np.linalg.norm(cross)
    if cross_norm <= 1e-12:
        if dot > 0.0:
            return np.eye(3)
        return _rot_axis(_stable_orthogonal_axis(target), 180.0)
    return _rot_axis(cross, math.degrees(math.atan2(cross_norm, dot)))


def part_matrix(pitch=0.0, yaw=0.0, roll=0.0):
    """ModelPart 的 rotationZYX(roll, yaw, pitch)，换算到 y-up 世界（符号见模块 docstring）。"""
    return R._rotmat(-roll, 2) @ R._rotmat(yaw, 1) @ R._rotmat(-pitch, 0)


def _split_box(frm, to, y_split):
    """沿 y 切两半，返回 [(from, to, v_frac_lo, v_frac_hi)]，v_frac 用来切 UV。"""
    span = to[1] - frm[1]
    f = (y_split - frm[1]) / span
    lower = (list(frm), [to[0], y_split, to[2]], 0.0, f)          # 靠手/脚一端
    upper = ([frm[0], y_split, frm[2]], list(to), f, 1.0)          # 靠 pivot 一端
    return lower, upper


def part_tris(name, axes, v_off=0):
    """单个 part → 三角形。axes: pitch/yaw/roll/bend/axis(bendDirection)/x,y,z（度、px）。"""
    spec = PARTS[name]
    frm, to = [list(map(float, v)) for v in spec["box"]]
    pivot = np.array(spec["pivot"], float)
    bend_v = float(axes.get("bend", 0.0))
    bend_a = float(axes.get("axis", 0.0))
    M = part_matrix(axes.get("pitch", 0.0), axes.get("yaw", 0.0), axes.get("roll", 0.0))
    # x/y/z 是 pivot 偏移（MC 空间，y 向下 → world 取反）
    off = np.array([axes.get("x", 0.0), -axes.get("y", 0.0), axes.get("z", 0.0)], float)

    pieces = []
    if spec["bendable"] and abs(bend_v) > 1e-6:
        center = np.array(spec["bend_center"], float)
        axis = np.array([math.cos(math.radians(bend_a)), 0.0, math.sin(math.radians(bend_a))])
        B = _rot_axis(axis, -bend_v)  # y 翻转是反射 → 转角取负
        lower, upper = _split_box(frm, to, spec["bend_center"][1])
        pieces.append((lower[0], lower[1], lower[2], lower[3], B, center))
        pieces.append((upper[0], upper[1], upper[2], upper[3], None, None))
    else:
        pieces.append((frm, to, 0.0, 1.0, None, None))

    size = (to[0] - frm[0], to[1] - frm[1], to[2] - frm[2])
    uv_full = H.box_uv(spec["uv"], size)
    tris = []
    for pf, pt, v0, v1, B, center in pieces:
        pf, pt = np.array(pf, float), np.array(pt, float)
        for fname, (corner_fn, normal) in R.FACES.items():
            u1, vv1, u2, vv2 = uv_full[fname]
            # 竖直方向按切块比例取 UV（up/down 面是横截面，整块用）
            if fname in ("west", "east", "north", "south"):
                h = vv2 - vv1
                a, b = vv1 + h * (1.0 - v1), vv1 + h * (1.0 - v0)
                uv = (u1, a, u2, b)
            else:
                uv = (u1, vv1, u2, vv2)
            cs = [np.array(c, float) for c in corner_fn(pf, pt)]
            n = np.array(normal, float)
            if B is not None:                       # bend：靠手/脚一端绕几何中心折
                cs = [B @ (c - center) + center for c in cs]
                n = B @ n
            cs = [M @ (c - pivot) + pivot + off for c in cs]   # ModelPart 自身旋转
            n = M @ n
            uvs = [(uv[0], uv[1] + v_off), (uv[2], uv[1] + v_off),
                   (uv[2], uv[3] + v_off), (uv[0], uv[3] + v_off)]
            for i, j in ((1, 2), (2, 3)):
                tris.append((np.array([cs[0], cs[i], cs[j]]),
                             np.array([uvs[0], uvs[i], uvs[j]]), n))
    return tris


def part_point(name, axes, local):
    """把 part 局部空间的一个点按同一条管线变换（bend → 自身旋转 → pivot 偏移）。
    手心必须这样算：小臂 bend 之后手的位置会挪，只按欧拉旋转推会错位。"""
    spec = PARTS[name]
    p = np.array(local, float)
    bend_v = float(axes.get("bend", 0.0))
    if spec["bendable"] and abs(bend_v) > 1e-6 and p[1] < spec["bend_center"][1]:
        center = np.array(spec["bend_center"], float)
        bend_a = float(axes.get("axis", 0.0))
        axis = np.array([math.cos(math.radians(bend_a)), 0.0, math.sin(math.radians(bend_a))])
        p = _rot_axis(axis, -bend_v) @ (p - center) + center
    M = part_matrix(axes.get("pitch", 0.0), axes.get("yaw", 0.0), axes.get("roll", 0.0))
    off = np.array([axes.get("x", 0.0), -axes.get("y", 0.0), axes.get("z", 0.0)], float)
    pivot = np.array(spec["pivot"], float)
    return M @ (p - pivot) + pivot + off, M


def jian_tris(pose: dict, v_off: int):
    """把单锏挂到双手：手心随手臂 pose 走，锏沿小臂朝向（bend 之后的方向）。"""
    import json as _json
    src = H.load_model_document()
    base = []
    for e in src["elements"]:
        frm, to = np.array(e["from"], float), np.array(e["to"], float)
        rot = e.get("rotation", [0, 0, 0])
        org = np.array(e.get("origin", [0, 0, 0]), float)
        Rc = None
        if any(abs(r) > 1e-6 for r in rot):
            Rc = R._rotmat(rot[2], 2) @ R._rotmat(rot[1], 1) @ R._rotmat(rot[0], 0)
        for fname, (corner_fn, normal) in R.FACES.items():
            fd = e.get("faces", {}).get(fname)
            if not fd:
                continue
            u1, vv1, u2, vv2 = fd["uv"]
            cs = [np.array(c, float) for c in corner_fn(frm, to)]
            n = np.array(normal, float)
            if Rc is not None:
                cs = [Rc @ (c - org) + org for c in cs]
                n = Rc @ n
            uvs = [(u1, vv1 + v_off), (u2, vv1 + v_off), (u2, vv2 + v_off), (u1, vv2 + v_off)]
            for i, j in ((1, 2), (2, 3)):
                base.append((np.array([cs[0], cs[i], cs[j]]),
                             np.array([uvs[0], uvs[i], uvs[j]]), n))

    out = []
    for arm, sx in (("rightArm", -1), ("leftArm", 1)):
        axes = pose.get(arm, {})
        # 手心 = 小臂末端中心（该臂 cuboid 底面中心）
        hand_local = (PARTS[arm]["bend_center"][0], 12.5, -0.4)
        hand, M = part_point(arm, axes, hand_local)
        # 锏沿小臂轴：小臂在 bend 后指向 (0,-1,0) 绕 bend 轴转过的方向
        bend_v = float(axes.get("bend", 0.0))
        bend_a = float(axes.get("axis", 0.0))
        axis = np.array([math.cos(math.radians(bend_a)), 0.0, math.sin(math.radians(bend_a))])
        forearm_dir = _rot_axis(axis, -bend_v) @ np.array([0.0, -1.0, 0.0])
        world_dir = M @ forearm_dir
        # 把锏的 +y（柄尾→锏尖）对到小臂朝向，握把中心落在手心
        A = _align_up_to_direction(world_dir)
        for vs, uvs, n in base:
            out.append((np.array([A @ (p - H.GRIP_ANCHOR) + hand for p in vs]), uvs, A @ n))
    return out


def pose_tris(pose: dict):
    """pose: {part: {axis: value}}，缺省 part 用静止姿态。"""
    tris = []
    for name in PARTS:
        tris += part_tris(name, pose.get(name, {}))
    return tris


def skin_atlas(with_jian: bool = False) -> np.ndarray:
    atlas = Image.new("RGBA", (H.ATLAS, H.ATLAS), (0, 0, 0, 0))
    atlas.paste(H.make_skin(), (0, 0))
    if with_jian:
        import base64 as _b64, io as _io, json as _json
        src = H.load_model_document()
        tex = Image.open(_io.BytesIO(_b64.b64decode(
            src["textures"][0]["source"].split(",", 1)[1]))).convert("RGBA")
        atlas.paste(tex.resize((H.SKIN, H.SKIN), Image.NEAREST), (0, H.WEAPON_V_OFF))
    return np.asarray(atlas, float)


def render_pose(tris, tex, yaw=180.0, pitch=4.0, size=300, bg=(26, 27, 31)):
    size = _validate_size(size, "render_pose size")
    orig = R.load_bbmodel

    def load_pose(_path, xform=None, texture=None):
        if xform:
            raise ValueError(
                "render_pose uses pre-baked triangles and cannot apply "
                "element-level xform"
            )
        return tris, tex, (H.ATLAS, H.ATLAS), "pose"

    R.load_bbmodel = load_pose
    try:
        im, _ = R.render("<pose>", yaw=yaw, pitch=pitch, size=size, bg=bg)
    finally:
        R.load_bbmodel = orig
    return im


MAX_GRID_PIXELS = 32 * 1024 * 1024
MAX_ANIMATION_FRAMES = 128


def _grid_dimensions(cell_count, per_row, size, title=None):
    size = _validate_size(size, "grid size")
    if cell_count < 1:
        raise ValueError("render grid must contain at least one frame")
    if per_row < 1:
        raise ValueError("render grid must contain at least one column")
    gap, lab = 8, 17
    rows = (cell_count + per_row - 1) // per_row
    head = 22 if title else 0
    width = size * per_row + gap * (per_row + 1)
    height = (size + lab) * rows + gap * (rows + 1) + head
    pixels = width * height
    if pixels > MAX_GRID_PIXELS:
        raise ValueError(
            f"render grid area {pixels} exceeds limit {MAX_GRID_PIXELS} "
            f"for {cell_count} frames at size={size}"
        )
    return width, height


def grid(cells, per_row, size, out: Path, title=None):
    width, height = _grid_dimensions(len(cells), per_row, size, title)
    gap, lab = 8, 17
    head = 22 if title else 0
    cv = Image.new("RGB", (width, height), (14, 15, 17))

    d = ImageDraw.Draw(cv)
    f = H.label_font(13)
    if title:
        d.text((gap, 5), title, fill=(238, 232, 210), font=H.label_font(15))
    for i, (label, im) in enumerate(cells):
        cx = gap + (i % per_row) * (size + gap)
        cy = head + gap + (i // per_row) * (size + lab + gap)
        d.text((cx + 2, cy), label, fill=(214, 212, 204), font=f)
        cv.paste(im, (cx, cy + lab))
    out.parent.mkdir(parents=True, exist_ok=True)
    cv.save(out)
    return out


# ── 弯曲能力扫描 ──────────────────────────────────────────────────────────
def bend_matrix(size=250, with_jian=False):
    size = _validate_size(size, "bend_matrix size")
    _grid_dimensions(18, 6, size, title=True)
    tex = skin_atlas(with_jian)
    cells = []

    # 1) 手臂 bend 扫描（pitch=-85 前平举，看小臂折到哪）
    for bv in (0, 30, 60, 90, 120, 150):
        pose = {"rightArm": dict(pitch=-85, bend=bv, axis=0),
                "leftArm": dict(pitch=-85, bend=bv, axis=0)}
        tris = pose_tris(pose)
        if with_jian:
            tris += jian_tris(pose, H.WEAPON_V_OFF)
        cells.append((f"arm.bend {bv}° (pitch-85, axis0)",
                      render_pose(tris, tex, yaw=135.0, pitch=10.0, size=size)))

    # 2) bendAxis 扫描（固定 bend=90，看折弯方向绕主轴转）
    for ba in (0, 90, 180, 270):
        pose = {"rightArm": dict(pitch=-85, bend=90, axis=ba),
                "leftArm": dict(pitch=-85, bend=90, axis=ba)}
        tris = pose_tris(pose)
        if with_jian:
            tris += jian_tris(pose, H.WEAPON_V_OFF)
        cells.append((f"arm.axis {ba}° (bend90)",
                      render_pose(tris, tex, yaw=135.0, pitch=10.0, size=size)))

    # 3) 腿：pitch 单独加大 → 腿腹断连；同等视觉强度改用 bend
    for lp, lb, tag in ((20, 0, "轻微"), (40, 0, "约定上限"), (60, 0, "断连"), (40, 90, "pitch40+bend90")):
        pose = {"rightLeg": dict(pitch=lp, bend=lb), "leftLeg": dict(pitch=-lp * 0.4)}
        tris = pose_tris(pose)
        if with_jian:
            tris += jian_tris(pose, H.WEAPON_V_OFF)
        cells.append((f"leg.pitch {lp}° bend {lb}° — {tag}",
                      render_pose(tris, tex, yaw=100.0, pitch=6.0, size=size)))

    # 4) 躯干：pitch（整体绕腰转，胯不跟 → 腰断） vs bend（腰部折弯）
    for tp, tb, tag in ((30, 0, "torso.pitch 30"), (60, 0, "torso.pitch 60"),
                        (0, 30, "torso.bend 30"), (0, 60, "torso.bend 60")):
        pose = {"torso": dict(pitch=tp, bend=tb, axis=0)}
        tris = pose_tris(pose)
        if with_jian:
            tris += jian_tris(pose, H.WEAPON_V_OFF)
        cells.append((f"{tag}", render_pose(tris, tex, yaw=100.0, pitch=6.0, size=size)))

    return grid(cells, 6, size, OUT_MATRIX,
                title="玩家模型弯曲能力扫描 — 每肢仅 1 个 bend；head 不可 bend")


def anim_pose_table(json_path: Path):
    """Read Emotecraft v3 poses and normalize angle units to degrees."""
    import json as _json
    doc = _json.loads(Path(json_path).read_text())
    emote = doc["emote"]
    degrees_flag = emote.get("degrees", True)
    if not isinstance(degrees_flag, bool):
        raise ValueError(
            "emote.degrees must be a boolean discriminator: false means radians, "
            "true means degrees"
        )
    kfs = RA.collect_keyframes(emote)
    ticks = sorted({t for part in kfs.values() for axis in part.values() for t, *_ in axis})
    out = []
    for tick in ticks:
        pose = {}
        for part in kfs:
            axes = RA.sample_part(kfs, part, float(tick))
            # sample_part 保持输入角度单位，位移是米/px 原值
            conv = {}
            for k, v in axes.items():
                conv[k] = math.degrees(v) if not degrees_flag and k in ANGLE_AXES else v
            if part == "body":       # body 走 MatrixStack，本渲染器按整体位移近似
                pose["_body"] = conv
            elif part in PARTS:
                pose[part] = conv
        out.append((tick, pose))
    return doc.get("name", Path(json_path).stem), emote, out


def render_anim(json_path: Path, size=280, yaw=90.0, pitch=6.0, with_jian=False):
    """按关键帧 tick 逐帧渲染（步态看侧面、招式看 3/4）。"""
    size = _validate_size(size, "render_anim size")
    name, emote, table = anim_pose_table(json_path)
    if not table:
        raise ValueError(f"animation {name!r} must contain at least one keyframe")
    if len(table) > MAX_ANIMATION_FRAMES:
        raise ValueError(
            f"animation {name!r} has {len(table)} keyframes, "
            f"exceeds limit {MAX_ANIMATION_FRAMES}"
        )
    tex = skin_atlas(with_jian)
    cells, frames = [], []
    for tick, pose in table:
        body = pose.pop("_body", {})
        if not isinstance(body, dict):
            raise ValueError(f"animation {name!r} body pose must be an object")
        tris = pose_tris(pose)
        if with_jian:
            tris = tris + jian_tris(pose, H.WEAPON_V_OFF)
        if body:
            M = part_matrix(body.get("pitch", 0.0), body.get("yaw", 0.0), body.get("roll", 0.0))
            off = np.array([body.get("x", 0.0), -body.get("y", 0.0), body.get("z", 0.0)]) * 16.0
            tris = [
                (np.array([M @ (v - BODY_ROOT) + BODY_ROOT + off for v in vs]), uvs, M @ n)
                for vs, uvs, n in tris
            ]
        frames.append((tick, tris))

    # render_bbmodel 每次按自身 bbox 自适应缩放 → 逐帧尺度不一致，举锏那帧人会被缩小。
    # 塞一个退化三角形（三点共线，面积 0）钉住全局 bbox：它参与 center/scale 计算，
    # 但光栅化阶段被 `abs(area) < 1e-6` 跳过，不会画出任何像素。
    allv = np.array([v for _t, tris in frames for vs, _u, _n in tris for v in vs])
    lo, hi = allv.min(0), allv.max(0)
    anchor = (np.array([lo, lo, hi]), np.zeros((3, 2)), np.array([0.0, 0.0, 1.0]))
    for tick, tris in frames:
        cells.append((f"t{tick:g}", render_pose(tris + [anchor], tex, yaw=yaw, pitch=pitch, size=size)))
    out = OUT_ANIM.with_name(f"render_anim_{name}.png")
    loop = "loop" if emote.get("isLoop") else "once"
    return grid(cells, min(len(cells), 5), size, out,
                title=f"{name} — endTick {emote['endTick']} / {loop}（侧视，脸朝左）")


POSES = {
    "stand": {},
    "crouch": {"torso": dict(bend=35), "rightLeg": dict(pitch=30, bend=60),
               "leftLeg": dict(pitch=30, bend=60)},
    "lean": {"torso": dict(bend=55), "rightArm": dict(pitch=-40), "leftArm": dict(pitch=-40)},
}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bend-matrix", action="store_true")
    ap.add_argument("--pose", choices=sorted(POSES), default=None)
    ap.add_argument("--anim", default=None, help="emotecraft v3 JSON → 逐关键帧三视图")
    ap.add_argument("--yaw", type=float, default=90.0)
    ap.add_argument("--with-jian", action="store_true", help="双手挂上竹节锏一起渲")
    ap.add_argument(
        "--size",
        type=_parse_size,
        default=250,
        help=f"每个渲染单元的边长（{MIN_RENDER_SIZE}..{MAX_RENDER_SIZE}）",
    )
    args = ap.parse_args()

    if args.anim:
        p = render_anim(Path(args.anim), size=args.size, yaw=args.yaw, with_jian=args.with_jian)
        print(f"→ {p.relative_to(REPO)}")
        return

    if args.bend_matrix or args.pose is None:
        p = bend_matrix(size=args.size, with_jian=args.with_jian)
        print(f"→ {p.relative_to(REPO)}")
        return
    tex = skin_atlas(args.with_jian)
    tris = pose_tris(POSES[args.pose])
    if args.with_jian:
        tris += jian_tris(POSES[args.pose], H.WEAPON_V_OFF)
    pose_size = _validate_size(args.size, "pose render size")
    cells = [(lab, render_pose(tris, tex, yaw=yaw, pitch=pitch, size=pose_size))
             for lab, yaw, pitch in (("正面", 180.0, 4.0), ("侧面", 90.0, 4.0), ("3/4", 145.0, 10.0))]
    p = grid(cells, 3, pose_size, OUT_POSE, title=f"pose: {args.pose}")
    print(f"→ {p.relative_to(REPO)}")


if __name__ == "__main__":
    main()
