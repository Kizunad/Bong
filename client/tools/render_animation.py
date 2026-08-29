#!/usr/bin/env python3
"""Render a PlayerAnimator Emotecraft v3 keyframe JSON as stick-figure views.

Replicates the KosmX PlayerAnimator + bendy-lib transform pipeline closely enough
to visually validate pose design without running Minecraft. Output is one PNG
per test tick, plus a combined grid, so Claude can `Read` the PNGs and iterate
on pose design autonomously.

Coordinate convention (MC ModelPart local space):
    +X = player's LEFT (yes, inverted)
    +Y = DOWN (MC model space is y-inverted vs world)
    +Z = BACK (player faces -Z — face texture on -Z side of head cube)

Biped rest pose pivots (vanilla BipedEntityModel):
    head         (  0,  0, 0)
    torso        (  0,  0, 0)    (called "body" in code pre-v3)
    leftArm      (  5,  2, 0)
    rightArm     ( -5,  2, 0)
    leftLeg      (  1.9, 12, 0)
    rightLeg     ( -1.9, 12, 0)

Bend geometry (from BendableCuboid.Builder.build + IBendable.applyBend):
    arm cuboid: offset (-3,-2,-2) for right / (1,-2,-2) for left, size (4,12,4)
        → bend_center = (bendX, bendY, bendZ) = (-1, 4, 0) right / (3, 4, 0) left
        → hand rest (in local) = (bendX, 10, bendZ)
    leg cuboid: offset (-2, 0, -2) / (0, 0, -2), size (4,12,4)
        → bend_center = (0, 6, 0) for both (by geometry)
        → foot rest = (bendX, 12, bendZ)

Bend math (for direction=UP, which both arms and both legs use per
BipedEntityModelMixin.java:39-42):
    axis vector = (cos(bendAxis), 0, sin(bendAxis))      # in cuboid local
    Lower half (closer to basePlane=hand/foot end) rotated by bendValue
    around that axis, centered at (bendX,bendY,bendZ).
    isBendInverted=True for UP, so the rotation direction is effectively
    the reported axis but signed.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple

import numpy as np
from PIL import Image, ImageDraw, ImageFont


# ----- biped rest geometry --------------------------------------------------

PIVOTS: Dict[str, Tuple[float, float, float]] = {
    "head":     ( 0.0,  0.0, 0.0),
    "torso":    ( 0.0,  0.0, 0.0),
    "leftArm":  ( 5.0,  2.0, 0.0),
    "rightArm": (-5.0,  2.0, 0.0),
    "leftLeg":  ( 1.9, 12.0, 0.0),
    "rightLeg": (-1.9, 12.0, 0.0),
}

# cuboid offset + size for bend-center computation
CUBOIDS = {
    "leftArm":  dict(offset=( 1.0, -2.0, -2.0), size=(4, 12, 4)),
    "rightArm": dict(offset=(-3.0, -2.0, -2.0), size=(4, 12, 4)),
    "leftLeg":  dict(offset=( 0.0,  0.0, -2.0), size=(4, 12, 4)),
    "rightLeg": dict(offset=(-2.0,  0.0, -2.0), size=(4, 12, 4)),
}

# For non-bendable segments, just draw a line pivot → "end offset in local"
SEG_END_LOCAL = {
    "head":  (0.0, -8.0, 0.0),   # head cuboid extends upward from pivot (-Y)
    "torso": (0.0, 12.0, 0.0),   # torso goes DOWN from pivot
}


def bend_center(part: str) -> np.ndarray:
    c = CUBOIDS[part]
    ox, oy, oz = c["offset"]
    sx, sy, sz = c["size"]
    return np.array([ox + sx / 2, oy + sy / 2, oz + sz / 2], dtype=np.float64)


def limb_end_local(part: str) -> np.ndarray:
    """Rest position of hand/foot end, in ModelPart local space (relative to pivot)."""
    c = CUBOIDS[part]
    ox, oy, oz = c["offset"]
    sx, sy, sz = c["size"]
    # centered in X/Z, at bottom of cuboid in Y
    return np.array([ox + sx / 2, oy + sy, oz + sz / 2], dtype=np.float64)


# ----- linear-algebra helpers ----------------------------------------------

def rot_x(a: float) -> np.ndarray:
    c, s = math.cos(a), math.sin(a)
    return np.array([[1, 0, 0], [0, c, -s], [0, s, c]], dtype=np.float64)


def rot_y(a: float) -> np.ndarray:
    c, s = math.cos(a), math.sin(a)
    return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]], dtype=np.float64)


def rot_z(a: float) -> np.ndarray:
    c, s = math.cos(a), math.sin(a)
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]], dtype=np.float64)


def part_rotation_matrix(pitch: float, yaw: float, roll: float) -> np.ndarray:
    """Replicates Quaternionf.rotationZYX(roll, yaw, pitch) as a 3x3 matrix.

    JOML's rotationZYX(angleZ, angleY, angleX) is equivalent to extrinsic
    Rz·Ry·Rx (i.e., apply Rx first to vector, then Ry, then Rz).
    So the final matrix is M = Rz(roll) · Ry(yaw) · Rx(pitch).
    """
    return rot_z(roll) @ rot_y(yaw) @ rot_x(pitch)


def rotate_about_axis(axis: np.ndarray, angle: float) -> np.ndarray:
    """Rodrigues rotation matrix for rotating by `angle` around unit `axis`."""
    ax = axis / (np.linalg.norm(axis) + 1e-12)
    c, s = math.cos(angle), math.sin(angle)
    x, y, z = ax
    K = np.array([[0, -z, y], [z, 0, -x], [-y, x, 0]], dtype=np.float64)
    return np.eye(3) + s * K + (1 - c) * (K @ K)


# ----- bend ----------------------------------------------------------------

def bent_end_local(part: str, bend_axis_rad: float, bend_value_rad: float) -> np.ndarray:
    """Compute the hand/foot position in ModelPart local space after bend.

    Replicates the "hand end moves under the bend rotation" half of the
    IBendable.applyBend logic. Shoulder/hip end is approximately unmoved
    (it's on the "other" side of basePlane), so for stick-figure purposes
    we only need the moved end.
    """
    end_local = limb_end_local(part)
    center = bend_center(part)

    # axis vector per IBendable.applyBend: (cos(bendAxis), 0, sin(bendAxis))
    # then rotated by Direction.UP's rotation quaternion (identity for UP).
    axis_vec = np.array(
        [math.cos(bend_axis_rad), 0.0, math.sin(bend_axis_rad)],
        dtype=np.float64,
    )

    # isBendInverted=True for UP (field_11036) — the rotation direction flips.
    # Empirically we want: with pitch=-85°, axis=π, bend=80°, forearm folds UP
    # toward face (v8 ground truth confirmed in-game).
    # Try positive rotation first; if the mirror is wrong we'll revisit.
    effective_angle = bend_value_rad  # inverted? experimentally determine
    R = rotate_about_axis(axis_vec, effective_angle)

    # translate to bend center, rotate, translate back
    p = end_local - center
    p_rot = R @ p
    return p_rot + center


# ----- keyframe sampling ---------------------------------------------------
# 这一段已提进 modelScript/core/emote_anim.py：它处理的是通用 MC 动画格式
# （关键帧收集 / easing 曲线 / 按 tick 采样），跟 Bong 无关，而 modelScript 的
# 渲染底座也要用。留在这里的话就是渲染底座反过来依赖客户端工具目录。
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "modelScript" / "core"))
from emote_anim import (  # noqa: E402
    AXIS_NAMES,
    BODY_PART_NAMES,
    apply_easing,
    collect_keyframes,
    default_axis_value,
    sample_axis,
    sample_part,
)

__all__ = [
    "AXIS_NAMES", "BODY_PART_NAMES", "apply_easing", "collect_keyframes",
    "default_axis_value", "sample_axis", "sample_part",
]
# ----- skeleton solve ------------------------------------------------------

def solve_skeleton(
    kfs, tick: float, body_disp_scale: float = 1.0
) -> Dict[str, Dict[str, np.ndarray]]:
    """Compute world-space joint positions at a given tick.

    Returns a dict:
      {part: {"start": ndarray, "end": ndarray, "elbow": ndarray or None}}
    All positions are in MC model space (+X=left, +Y=down, +Z=back).

    body_disp_scale scales the whole-body translation (body.x/y/z). 1.0 = raw
    animation; 0.0 previews an FPV variant that zeroes body displacement (the
    anti-camera-swim rule, see docs/player-animation-conventions.md §16.1);
    0.5 halves it. Body rotation is left intact.
    """
    body = sample_part(kfs, "body", tick)
    body_pos = (
        np.array([body["x"], body["y"], body["z"]], dtype=np.float64) * body_disp_scale
    )
    body_rot = part_rotation_matrix(body["pitch"], body["yaw"], body["roll"])

    out: Dict[str, Dict[str, np.ndarray]] = {}

    # head & torso (non-bendable segments, just pivot + rotation + local end)
    for part in ("head", "torso"):
        pivot = np.array(PIVOTS[part], dtype=np.float64)
        p = sample_part(kfs, part, tick)
        # x/y/z are additive offsets to the pivot
        pivot_offset = np.array([p["x"], p["y"], p["z"]], dtype=np.float64)
        pivot_local = pivot + pivot_offset
        R_part = part_rotation_matrix(p["pitch"], p["yaw"], p["roll"])
        end_local = np.array(SEG_END_LOCAL[part], dtype=np.float64)
        end_part_frame = R_part @ end_local
        start_world = body_rot @ pivot_local + body_pos
        end_world = body_rot @ (pivot_local + end_part_frame) + body_pos
        out[part] = {"start": start_world, "end": end_world, "elbow": None}

    # bendable limbs
    for part in ("leftArm", "rightArm", "leftLeg", "rightLeg"):
        pivot = np.array(PIVOTS[part], dtype=np.float64)
        p = sample_part(kfs, part, tick)
        pivot_offset = np.array([p["x"], p["y"], p["z"]], dtype=np.float64)
        pivot_local = pivot + pivot_offset
        R_part = part_rotation_matrix(p["pitch"], p["yaw"], p["roll"])
        # rest end
        hand_rest_local = limb_end_local(part)
        # after bend
        hand_bent_local = bent_end_local(part, p["axis"], p["bend"])
        elbow_local = bend_center(part)  # roughly the elbow stays at bend center

        # apply ModelPart rotation (bent local vertex → part frame)
        hand_part_frame = R_part @ hand_bent_local
        elbow_part_frame = R_part @ elbow_local

        start_world = body_rot @ pivot_local + body_pos
        elbow_world = body_rot @ (pivot_local + elbow_part_frame) + body_pos
        end_world = body_rot @ (pivot_local + hand_part_frame) + body_pos
        out[part] = {"start": start_world, "end": end_world, "elbow": elbow_world}

    return out


# ----- rendering -----------------------------------------------------------

VIEW_PROJECTIONS = {
    # (label, screen_x_fn, screen_y_fn, depth_fn)
    # MC coords: +X=left, +Y=down, +Z=back.  Screen: +x=right, +y=down.
    "front": {
        "label": "FRONT  (looking at face)",
        "x": lambda p: -p[0],  # player's right → screen right
        "y": lambda p: p[1],   # MC +Y down matches screen
        "d": lambda p: -p[2],  # -Z forward = closer to camera
    },
    "side": {
        "label": "SIDE  (player's right)",
        "x": lambda p: -p[2],  # -Z forward → screen right
        "y": lambda p: p[1],
        "d": lambda p: -p[0],  # player's right side visible
    },
    "top": {
        "label": "TOP  (bird's eye)",
        "x": lambda p: -p[0],
        "y": lambda p: -p[2],  # -Z forward → screen UP
        "d": lambda p: p[1],
    },
}

# Drawing colors (RGB tuples)
COLORS = {
    "head":     (200, 150, 100),
    "torso":    (160, 160, 200),
    "leftArm":  ( 80, 180,  80),  # left = green
    "rightArm": (220,  80,  80),  # right = red (the punching arm)
    "leftLeg":  ( 80, 140,  60),
    "rightLeg": (180,  60,  60),
}


def project(pos: np.ndarray, view: str, scale: float, origin_screen: Tuple[int, int]) -> Tuple[int, int]:
    proj = VIEW_PROJECTIONS[view]
    sx = proj["x"](pos)
    sy = proj["y"](pos)
    cx, cy = origin_screen
    return (int(cx + sx * scale), int(cy + sy * scale))


def draw_skeleton_view(
    draw: ImageDraw.ImageDraw,
    skel: Dict[str, Dict[str, np.ndarray]],
    view: str,
    bbox: Tuple[int, int, int, int],  # x0, y0, x1, y1
    scale: float,
    label: str,
    font: ImageFont.ImageFont,
) -> None:
    x0, y0, x1, y1 = bbox
    # background + border
    draw.rectangle(bbox, fill=(248, 248, 250), outline=(40, 40, 60), width=1)
    draw.text((x0 + 4, y0 + 2), label, fill=(40, 40, 60), font=font)
    # grid: vertical center line + ground line
    cx = (x0 + x1) // 2
    ground_y = y0 + int(0.85 * (y1 - y0))
    draw.line([(cx, y0 + 12), (cx, y1 - 2)], fill=(220, 220, 230), width=1)
    draw.line([(x0 + 2, ground_y), (x1 - 2, ground_y)], fill=(220, 220, 230), width=1)

    # origin for projection: player head (world 0,0,0) projected to (cx, y0 + 30)
    origin = (cx, y0 + 30)

    # head: circle at end (top of head)
    head_start = project(skel["head"]["start"], view, scale, origin)
    head_end = project(skel["head"]["end"], view, scale, origin)
    # end is the top of head (y = -8); represent head as circle at midpoint
    mid = ((head_start[0] + head_end[0]) // 2, (head_start[1] + head_end[1]) // 2)
    radius = max(int(4 * scale), 3)
    draw.ellipse(
        [mid[0] - radius, mid[1] - radius, mid[0] + radius, mid[1] + radius],
        outline=COLORS["head"], width=2,
    )

    # torso
    p0 = project(skel["torso"]["start"], view, scale, origin)
    p1 = project(skel["torso"]["end"], view, scale, origin)
    draw.line([p0, p1], fill=COLORS["torso"], width=3)

    # limbs with elbow/knee bend
    for part in ("leftArm", "rightArm", "leftLeg", "rightLeg"):
        seg = skel[part]
        start = project(seg["start"], view, scale, origin)
        elbow = project(seg["elbow"], view, scale, origin)
        end = project(seg["end"], view, scale, origin)
        color = COLORS[part]
        draw.line([start, elbow], fill=color, width=3)
        draw.line([elbow, end], fill=color, width=3)
        # joint dots
        r = max(int(1.5 * scale), 2)
        draw.ellipse([start[0] - r, start[1] - r, start[0] + r, start[1] + r], fill=color)
        draw.ellipse([elbow[0] - r, elbow[1] - r, elbow[0] + r, elbow[1] + r], fill=(60, 60, 60))
        draw.ellipse([end[0] - r, end[1] - r, end[0] + r, end[1] + r], fill=color)


def render_tick(
    kfs,
    tick: float,
    out_path: Path,
    title: str,
    scale: float = 13.0,
    font: Optional[ImageFont.ImageFont] = None,
) -> None:
    # 3 views side by side
    view_w, view_h = 380, 500
    total_w = view_w * 3 + 20
    total_h = view_h + 48
    img = Image.new("RGB", (total_w, total_h), (255, 255, 255))
    draw = ImageDraw.Draw(img)
    if font is None:
        try:
            font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 11)
        except OSError:
            font = ImageFont.load_default()
    big_font = font
    try:
        big_font = ImageFont.truetype("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 13)
    except OSError:
        pass

    draw.text((10, 6), title, fill=(20, 20, 30), font=big_font)

    skel = solve_skeleton(kfs, tick)
    # numeric summary on right side of title row
    body = sample_part(kfs, "body", tick)
    torso = sample_part(kfs, "torso", tick)
    rArm = sample_part(kfs, "rightArm", tick)
    lArm = sample_part(kfs, "leftArm", tick)
    rLeg = sample_part(kfs, "rightLeg", tick)
    lLeg = sample_part(kfs, "leftLeg", tick)
    r = math.degrees
    summary = (
        f"body xyz=({body['x']:+.2f},{body['y']:+.2f},{body['z']:+.2f}) "
        f"yaw={r(body['yaw']):+.0f}° | "
        f"torso yaw={r(torso['yaw']):+.0f}° pitch={r(torso['pitch']):+.0f}° | "
        f"rArm p={r(rArm['pitch']):+.0f} y={r(rArm['yaw']):+.0f} bend={r(rArm['bend']):+.0f}@ax{r(rArm['axis']):+.0f} | "
        f"lArm p={r(lArm['pitch']):+.0f} y={r(lArm['yaw']):+.0f} bend={r(lArm['bend']):+.0f}@ax{r(lArm['axis']):+.0f}"
    )
    draw.text((10, 26), summary, fill=(60, 60, 80), font=font)

    for i, view in enumerate(("front", "side", "top")):
        bbox = (10 + i * (view_w + 5), 44, 10 + (i + 1) * view_w + i * 5, 44 + view_h)
        draw_skeleton_view(draw, skel, view, bbox, scale, VIEW_PROJECTIONS[view]["label"], font)

    img.save(out_path)


def render_grid(json_path: Path, out_dir: Path, ticks: Optional[List[float]] = None) -> Path:
    data = json.loads(json_path.read_text())
    emote = data["emote"]
    degrees_flag = emote.get("degrees", True)
    if degrees_flag:
        print(
            "WARNING: emote.degrees=true (or absent) — values are in degrees. "
            "This tool assumes radians. Convert before re-running.",
            file=sys.stderr,
        )
    kfs = collect_keyframes(emote)

    if ticks is None:
        # use actual keyframe ticks, skip duplicates
        all_ticks = set()
        for part in kfs.values():
            for axis_list in part.values():
                for t, _, _ in axis_list:
                    all_ticks.add(int(t))
        ticks = sorted(all_ticks)

    out_dir.mkdir(parents=True, exist_ok=True)
    per_tick: List[Path] = []
    name = json_path.stem
    description = emote.get("description", "")

    for tick in ticks:
        out_path = out_dir / f"{name}_t{int(tick):02d}.png"
        title = f"{name}   tick={tick}   {description[:120]}"
        render_tick(kfs, tick, out_path, title)
        per_tick.append(out_path)

    # grid combine vertically
    imgs = [Image.open(p) for p in per_tick]
    w = max(i.width for i in imgs)
    total_h = sum(i.height for i in imgs)
    grid = Image.new("RGB", (w, total_h), (255, 255, 255))
    y = 0
    for i in imgs:
        grid.paste(i, (0, y))
        y += i.height
    grid_path = out_dir / f"{name}_grid.png"
    grid.save(grid_path)
    return grid_path


# ----- first-person (FPV) rendering ----------------------------------------
#
# Anchors a perspective camera at the player's eye in the *body* frame, looking
# down -Z (player-forward), and renders ONLY the arms + a schematic held-item
# blade. Purpose: headless iteration of FIRST-PERSON arm poses
# (plan-fpv-cast-av-v1 P0/P2) — is the arm actually in frame, how much of the
# swing is visible, and does body.* displacement swim the camera?
#
# IMPORTANT: this does NOT replicate vanilla HeldItemRenderer. The DECISIVE
# held-item OCCLUSION judgment that picks route A/B/C (plan §8.1 #1) can only be
# seen in real runClient. Here the blade is a proxy line to gauge sweep, not
# occlusion. Read the "world-ref" dot's drift off the crosshair as camera swim.

# Eye position in body-local model space: mid-head height (y=-4). Placed at the
# back of the head cube (z=+2) rather than the face plane so the close, splayed
# arms stay within the frustum — MC's real FPV uses a separate screen-anchored
# arm model we can't replicate headless, so this vantage is tuned for readable
# pose framing (arm angles + blade sweep + body-swim), not pixel fidelity.
# +X=left / +Y=down / +Z=back.
FPV_EYE_LOCAL = np.array([0.0, -4.0, 2.0], dtype=np.float64)

# 近平面深度：project_fpv 与 project_seg_fpv 共用同一边界（防两处魔数 0.5 漂移）。
# 语义 = "d < FPV_NEAR 才算在相机后/太近而剔除"，d == FPV_NEAR 可正常投影——故
# 近平面裁剪可精确裁到 FPV_NEAR 而不被 project_fpv 判否（裁剪端点 t∈[0,1] 无外推）。
FPV_NEAR = 0.5


def compute_fpv_camera(kfs, tick: float, body_disp_scale: float = 1.0) -> dict:
    """Eye world position + orthonormal camera basis, in MC model space.

    The camera rides the animated body transform (rotation always; translation
    scaled by body_disp_scale) — exactly how FirstPersonMode.ENABLED lets body.*
    move the first-person view. body_disp_scale=0 previews an FPV variant that
    zeroes body displacement (the anti-swim rule), 0.5 halves it.
    """
    body = sample_part(kfs, "body", tick)
    body_pos = (
        np.array([body["x"], body["y"], body["z"]], dtype=np.float64) * body_disp_scale
    )
    body_rot = part_rotation_matrix(body["pitch"], body["yaw"], body["roll"])
    eye = body_rot @ FPV_EYE_LOCAL + body_pos
    fwd = body_rot @ np.array([0.0, 0.0, -1.0])  # player faces -Z
    up = body_rot @ np.array([0.0, -1.0, 0.0])  # MC up = -Y
    right = body_rot @ np.array([-1.0, 0.0, 0.0])  # player's right = -X
    return {"eye": eye, "fwd": fwd, "up": up, "right": right, "body_pos": body_pos}


def project_fpv(
    pos: np.ndarray, cam: dict, focal: float, origin: Tuple[int, int]
) -> Optional[Tuple[int, int]]:
    """Perspective-project a world point; None if strictly behind the near plane.

    Accepts a point exactly AT the near plane (d == FPV_NEAR) so that near-plane
    clipping in project_seg_fpv can land its clipped endpoint precisely on the
    plane and still render.
    """
    rel = pos - cam["eye"]
    d = float(np.dot(rel, cam["fwd"]))
    if d < FPV_NEAR:
        return None
    u = float(np.dot(rel, cam["right"]))
    v = float(np.dot(rel, cam["up"]))
    cx, cy = origin
    return (int(cx + focal * (u / d)), int(cy - focal * (v / d)))


def _fpv_line(draw, a, b, color, width) -> None:
    if a is not None and b is not None:
        draw.line([a, b], fill=color, width=width)


def project_seg_fpv(
    a_world: np.ndarray,
    b_world: np.ndarray,
    cam: dict,
    focal: float,
    origin: Tuple[int, int],
    near: float = FPV_NEAR,
) -> Optional[Tuple[Tuple[int, int], Tuple[int, int]]]:
    """Project a world segment, clipping against the near plane so a segment
    with ONE endpoint behind the camera still renders its visible part.

    Arm shoulders sit behind the eye, so without clipping the shoulder→elbow
    segment is dropped whole and the arm looks amputated. This keeps the in-view
    part.
    """
    da = float(np.dot(a_world - cam["eye"], cam["fwd"]))
    db = float(np.dot(b_world - cam["eye"], cam["fwd"]))
    a, b = a_world, b_world
    if da < near and db < near:
        return None  # entirely behind the near plane
    # Clip the behind-plane endpoint exactly ONTO the near plane. project_fpv
    # accepts d == near, so the clipped endpoint renders; and because the moved
    # endpoint has da < near <= db (or symmetric), t = (near-da)/(db-da) stays in
    # [0, 1] — no extrapolation past the segment's far endpoint.
    if da < near:
        t = (near - da) / (db - da)
        a = a_world + (b_world - a_world) * t
    elif db < near:
        t = (near - db) / (da - db)
        b = b_world + (a_world - b_world) * t
    pa = project_fpv(a, cam, focal, origin)
    pb = project_fpv(b, cam, focal, origin)
    if pa is None or pb is None:
        return None
    return (pa, pb)


def draw_fpv_view(
    draw: ImageDraw.ImageDraw,
    skel: Dict[str, Dict[str, np.ndarray]],
    cam: dict,
    bbox: Tuple[int, int, int, int],
    focal: float,
    label: str,
    font: ImageFont.ImageFont,
    draw_item: bool = True,
) -> None:
    x0, y0, x1, y1 = bbox
    draw.rectangle(bbox, fill=(18, 20, 28), outline=(40, 40, 60), width=1)
    draw.text((x0 + 4, y0 + 2), label, fill=(200, 200, 220), font=font)
    cx = (x0 + x1) // 2
    cy = (y0 + y1) // 2
    origin = (cx, cy)

    # crosshair = where the camera aims (screen center)
    draw.line([(cx - 8, cy), (cx + 8, cy)], fill=(90, 90, 110), width=1)
    draw.line([(cx, cy - 8), (cx, cy + 8)], fill=(90, 90, 110), width=1)

    # world-fixed reference dot at (0,0,-40): NOT tied to the body, so body.*
    # swim makes it drift off the crosshair — read the offset as camera shake.
    ref = project_fpv(np.array([0.0, 0.0, -40.0]), cam, focal, origin)
    if ref is not None and x0 <= ref[0] <= x1 and y0 <= ref[1] <= y1:
        draw.ellipse(
            [ref[0] - 3, ref[1] - 3, ref[0] + 3, ref[1] + 3],
            outline=(230, 200, 90),
            width=1,
        )
        draw.text((ref[0] + 5, ref[1] - 6), "world-ref", fill=(230, 200, 90), font=font)

    # arms only (FPV never shows legs/torso from the eye). Near-plane clipping
    # keeps the visible part of segments whose shoulder is behind the eye.
    for part in ("leftArm", "rightArm"):
        seg = skel[part]
        color = COLORS[part]
        upper = project_seg_fpv(seg["start"], seg["elbow"], cam, focal, origin)
        fore = project_seg_fpv(seg["elbow"], seg["end"], cam, focal, origin)
        if upper is not None:
            draw.line(list(upper), fill=color, width=5)
        if fore is not None:
            draw.line(list(fore), fill=color, width=5)
        # joint dots only for joints actually in front of the eye
        for joint in ("elbow", "end"):
            pt = project_fpv(seg[joint], cam, focal, origin)
            if pt is not None and x0 <= pt[0] <= x1 and y0 <= pt[1] <= y1:
                draw.ellipse([pt[0] - 3, pt[1] - 3, pt[0] + 3, pt[1] + 3], fill=color)

    # held-item proxy: schematic blade from the right hand along the forearm
    # direction. NOT occlusion-accurate — real check = runClient.
    if draw_item:
        rseg = skel["rightArm"]
        dirv = rseg["end"] - rseg["elbow"]
        n = float(np.linalg.norm(dirv))
        if n > 1e-6:
            blade_tip = rseg["end"] + (dirv / n) * 22.0  # ~1.4 blocks of blade
            blade = project_seg_fpv(rseg["end"], blade_tip, cam, focal, origin)
            if blade is not None:
                draw.line(list(blade), fill=(170, 175, 185), width=3)
                draw.text(
                    (blade[1][0] + 3, blade[1][1] - 4),
                    "item(proxy)",
                    fill=(150, 155, 165),
                    font=font,
                )


def render_tick_fpv(
    kfs,
    tick: float,
    out_path: Path,
    title: str,
    body_disp_scale: float = 1.0,
    fov_deg: float = 70.0,
    draw_item: bool = True,
    font: Optional[ImageFont.ImageFont] = None,
) -> None:
    panel_w, panel_h = 560, 460
    total_w, total_h = panel_w + 20, panel_h + 52
    img = Image.new("RGB", (total_w, total_h), (255, 255, 255))
    draw = ImageDraw.Draw(img)
    if font is None:
        try:
            font = ImageFont.truetype(
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 11
            )
        except OSError:
            font = ImageFont.load_default()
    big_font = font
    try:
        big_font = ImageFont.truetype(
            "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", 13
        )
    except OSError:
        pass
    draw.text((10, 6), title, fill=(20, 20, 30), font=big_font)

    skel = solve_skeleton(kfs, tick, body_disp_scale=body_disp_scale)
    cam = compute_fpv_camera(kfs, tick, body_disp_scale=body_disp_scale)
    focal = (panel_w / 2) / math.tan(math.radians(fov_deg) / 2)

    body = sample_part(kfs, "body", tick)
    r = math.degrees
    ex, ey, ez = cam["eye"]
    summary = (
        f"FPV  fov={fov_deg:.0f}deg  body-scale={body_disp_scale:.2f}  "
        f"eye=({ex:+.2f},{ey:+.2f},{ez:+.2f})  "
        f"body xyz=({body['x']:+.2f},{body['y']:+.2f},{body['z']:+.2f}) "
        f"pitch={r(body['pitch']):+.0f} yaw={r(body['yaw']):+.0f}"
    )
    draw.text((10, 26), summary, fill=(60, 60, 80), font=font)

    bbox = (10, 46, 10 + panel_w, 46 + panel_h)
    label = (
        "FIRST-PERSON (eye @ body frame, look -Z) — arm pose + blade sweep; "
        "occlusion NOT modeled (use runClient)"
    )
    draw_fpv_view(draw, skel, cam, bbox, focal, label, font, draw_item=draw_item)
    img.save(out_path)


def render_grid_fpv(
    json_path: Path,
    out_dir: Path,
    ticks: Optional[List[float]] = None,
    body_disp_scale: float = 1.0,
    fov_deg: float = 70.0,
    draw_item: bool = True,
) -> Path:
    data = json.loads(json_path.read_text())
    emote = data["emote"]
    if emote.get("degrees", True):
        print(
            "WARNING: emote.degrees=true/absent — values assumed radians.",
            file=sys.stderr,
        )
    kfs = collect_keyframes(emote)
    if ticks is None:
        all_ticks = set()
        for part in kfs.values():
            for axis_list in part.values():
                for t, _, _ in axis_list:
                    all_ticks.add(int(t))
        ticks = sorted(all_ticks)
    out_dir.mkdir(parents=True, exist_ok=True)
    name = json_path.stem
    description = emote.get("description", "")
    scale_tag = f"_b{int(round(body_disp_scale * 100)):03d}"
    per_tick: List[Path] = []
    for tick in ticks:
        out_path = out_dir / f"{name}_fpv{scale_tag}_t{int(tick):02d}.png"
        title = f"{name} FPV  tick={tick}  {description[:90]}"
        render_tick_fpv(
            kfs,
            tick,
            out_path,
            title,
            body_disp_scale=body_disp_scale,
            fov_deg=fov_deg,
            draw_item=draw_item,
        )
        per_tick.append(out_path)
    imgs = [Image.open(p) for p in per_tick]
    w = max(i.width for i in imgs)
    total_h = sum(i.height for i in imgs)
    grid = Image.new("RGB", (w, total_h), (255, 255, 255))
    y = 0
    for i in imgs:
        grid.paste(i, (0, y))
        y += i.height
    grid_path = out_dir / f"{name}_fpv{scale_tag}_grid.png"
    grid.save(grid_path)
    return grid_path


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("json", type=Path, help="path to a player_animation JSON")
    ap.add_argument("-o", "--out", type=Path, default=Path("/tmp/anim_render"))
    ap.add_argument(
        "--ticks",
        type=str,
        default="",
        help="comma-separated ticks (default: use all keyframe ticks)",
    )
    ap.add_argument(
        "--fpv",
        action="store_true",
        help="render a first-person (eye) view instead of the 3 ortho views",
    )
    ap.add_argument(
        "--body-scale",
        type=float,
        default=1.0,
        help="[fpv] scale body.* displacement: 1=raw TPV (shows camera swim), "
        "0=FPV-variant (no swim), 0.5=halved",
    )
    ap.add_argument("--fov", type=float, default=70.0, help="[fpv] vertical FOV degrees")
    ap.add_argument(
        "--no-item",
        action="store_true",
        help="[fpv] hide the held-item proxy blade",
    )
    args = ap.parse_args()
    ticks = None
    if args.ticks:
        ticks = [float(t) for t in args.ticks.split(",")]
    if args.fpv:
        grid_path = render_grid_fpv(
            args.json,
            args.out,
            ticks=ticks,
            body_disp_scale=args.body_scale,
            fov_deg=args.fov,
            draw_item=not args.no_item,
        )
    else:
        grid_path = render_grid(args.json, args.out, ticks=ticks)
    print(f"wrote grid: {grid_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
