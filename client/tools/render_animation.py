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

BODY_PART_NAMES = {"body", "head", "torso", "leftArm", "rightArm", "leftLeg", "rightLeg"}
AXIS_NAMES = {"x", "y", "z", "pitch", "yaw", "roll", "bend", "axis"}


def default_axis_value(axis_name: str) -> float:
    # MC rightLeg rest z = 0.1, leftLeg = 0.1? In vanilla, both legs have z=0.1?
    # For the bare-bones stick figure this default doesn't matter much; 0 is fine.
    return 0.0


def collect_keyframes(emote: dict) -> Dict[str, Dict[str, List[Tuple[int, float, str]]]]:
    """{part_name: {axis_name: [(tick, value, easing), ...]}} sorted by tick."""
    kfs: Dict[str, Dict[str, List[Tuple[int, float, str]]]] = {}
    for move in emote["moves"]:
        tick = int(move["tick"])
        easing = move.get("easing", "linear")
        for k, v in move.items():
            if k in ("tick", "comment", "easing", "turn"):
                continue
            if k not in BODY_PART_NAMES or not isinstance(v, dict):
                continue
            for axis, value in v.items():
                if axis not in AXIS_NAMES:
                    continue
                kfs.setdefault(k, {}).setdefault(axis, []).append((tick, float(value), easing))
    for part_kfs in kfs.values():
        for axis_list in part_kfs.values():
            axis_list.sort(key=lambda t: t[0])
    return kfs


def sample_axis(
    kfs: Dict[str, Dict[str, List[Tuple[int, float, str]]]],
    part: str,
    axis: str,
    tick: float,
) -> float:
    axis_list = kfs.get(part, {}).get(axis)
    if not axis_list:
        return default_axis_value(axis)
    # linear interpolation
    if tick <= axis_list[0][0]:
        return axis_list[0][1]
    if tick >= axis_list[-1][0]:
        return axis_list[-1][1]
    for i in range(len(axis_list) - 1):
        t0, v0, _ = axis_list[i]
        t1, v1, _ = axis_list[i + 1]
        if t0 <= tick <= t1:
            if t1 == t0:
                return v1
            alpha = (tick - t0) / (t1 - t0)
            return v0 + (v1 - v0) * alpha
    return axis_list[-1][1]


def sample_part(kfs, part: str, tick: float) -> Dict[str, float]:
    return {axis: sample_axis(kfs, part, axis, tick) for axis in AXIS_NAMES}


# ----- skeleton solve ------------------------------------------------------

def solve_skeleton(kfs, tick: float) -> Dict[str, Dict[str, np.ndarray]]:
    """Compute world-space joint positions at a given tick.

    Returns a dict:
      {part: {"start": ndarray, "end": ndarray, "elbow": ndarray or None}}
    All positions are in MC model space (+X=left, +Y=down, +Z=back).
    """
    body = sample_part(kfs, "body", tick)
    body_pos = np.array([body["x"], body["y"], body["z"]], dtype=np.float64)
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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("json", type=Path, help="path to fist_punch_right.json")
    ap.add_argument("-o", "--out", type=Path, default=Path("/tmp/anim_render"))
    ap.add_argument("--ticks", type=str, default="", help="comma-separated ticks (default: use all keyframe ticks)")
    args = ap.parse_args()
    ticks = None
    if args.ticks:
        ticks = [float(t) for t in args.ticks.split(",")]
    grid_path = render_grid(args.json, args.out, ticks=ticks)
    print(f"wrote grid: {grid_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
