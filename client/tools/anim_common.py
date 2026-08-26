"""Shared emitter for Bong player_animation JSON (Emotecraft v3, degrees=false).

Usage inside a per-animation generator (`gen_<name>.py`):

    from anim_common import emit_json, render

    POSE = {
        0: dict(easing="INOUTSINE",
            body=dict(x=0.0, y=0.0, z=0.0),
            rightArm=dict(pitch=-30, bend=15, axis=180),
            ...),
        5: dict(easing="OUTQUAD", ...),
        ...
    }

    emit_json(POSE,
        name="meditate_sit",
        description="...",
        end_tick=40, stop_tick=43, is_loop=True)

Conventions:
  - Angles in DEGREES (converted to radians at emit time since emote.degrees=false).
  - Linear xyz in MC "meters" (model pixels × 1/16 for body, raw for part offsets).
  - "axis" is the JSON key for bendDirection (NOT "bendDirection" — see
    docs/player-animation-conventions.md §7.4).
  - For looped animations: tick 0 and tick end_tick MUST have matching values on
    every axis used, or KeyframeAnimationPlayer.Axis.findAfter fabricates a
    virtual endTick+1 frame pointing to defaultValue and you get "fade to T-pose"
    mid-loop. We assert this.
  - Default bend axis for folding forearm toward player FRONT (punching /
    holding / meditating): axis=180°. Default (axis=0°) folds toward back.
"""

from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Dict, Iterable, Optional

d = math.radians

ANGLE_AXES = frozenset({"pitch", "yaw", "roll", "bend", "axis"})
LINEAR_AXES = frozenset({"x", "y", "z"})
VALID_PARTS = frozenset(
    {"body", "head", "torso", "leftArm", "rightArm", "leftLeg", "rightLeg"}
)
RESERVED_KEYS = frozenset({"easing"})


# ── 关节解剖朝向 ────────────────────────────────────────────────────────────
# 肘只能往身前折（手够向脸/胸），膝只能往身后折（脚跟够向臀）。这不是风格偏好，
# 是骨骼约束——折反了就是断肢，而渲染出来往往只让人觉得"姿势有点别扭"，很难一眼
# 认定是 bug。所以在源码里硬拦，不靠肉眼审图。
#
# 判据：bendy-lib 把弯折轴取为 (cos(axis), 0, sin(axis))，绕它转 bend 后，末端相对
# 静止位的**前后**位移是
#
#     Δz = L · sin(bend) · cos(axis)          （MC 空间，+Z 是身后）
#
# 于是只需看 sin(bend)·cos(axis) 的符号，负 bend 和任意 axis 都自动处理：
#     肘 → 必须 < 0（往前）
#     膝 → 必须 > 0（往后）
# sin(bend) ≈ 0 时肢体本就是直的，axis 无意义，跳过。

_ARM_PARTS = frozenset({"leftArm", "rightArm"})
_LEG_PARTS = frozenset({"leftLeg", "rightLeg"})
# 低于这个折角就认为肢体是直的（axis 此时纯属声明，不产生朝向）
_FOLD_EPSILON_DEG = 1.0


def joint_fold_z(bend_deg: float, axis_deg: float) -> float:
    """末端前后位移的符号量（正=往身后，负=往身前）。单位无关，只看符号与大小。"""
    return math.sin(math.radians(bend_deg)) * math.cos(math.radians(axis_deg))


def assert_joint_fold_is_anatomical(part: str, bend_deg: float, axis_deg: float,
                                    where: str = "") -> None:
    """折向不合解剖就抛 ValueError。非可弯部位与近乎伸直的肢体直接放行。"""
    if part not in _ARM_PARTS and part not in _LEG_PARTS:
        return
    if abs(math.sin(math.radians(bend_deg))) < math.sin(math.radians(_FOLD_EPSILON_DEG)):
        return
    z = joint_fold_z(bend_deg, axis_deg)
    is_arm = part in _ARM_PARTS
    ok = z < 0 if is_arm else z > 0
    if ok:
        return
    joint, want, got = ("肘", "身前", "身后") if is_arm else ("膝", "身后", "身前")
    loc = f"{where}: " if where else ""
    raise ValueError(
        f"{loc}{part} 的{joint}折反了——bend={bend_deg:g}° axis={axis_deg:g}° 会让末端往"
        f"{got}折，{joint}只能往{want}折。\n"
        f"    修法：axis 加减 180°（本仓约定 手臂 axis=180、腿 axis=0），"
        f"或把 bend 取反。\n"
        f"    判据 sin(bend)·cos(axis) = {z:+.3f}，"
        f"手臂要求 < 0、腿要求 > 0。"
    )


def _validate_pose_table(pose_table: Dict[int, dict]) -> None:
    for tick, pose in pose_table.items():
        if not isinstance(tick, int):
            raise TypeError(f"pose tick must be int, got {type(tick).__name__}={tick!r}")
        for key, value in pose.items():
            if key in RESERVED_KEYS:
                continue
            if key not in VALID_PARTS:
                raise ValueError(f"tick {tick}: unknown part '{key}' (valid: {sorted(VALID_PARTS)})")
            if not isinstance(value, dict):
                raise TypeError(f"tick {tick}: part '{key}' must be dict, got {type(value).__name__}")
            for axis in value:
                if axis not in ANGLE_AXES and axis not in LINEAR_AXES:
                    raise ValueError(
                        f"tick {tick}, part {key}: unknown axis '{axis}' "
                        f"(valid angles {sorted(ANGLE_AXES)}, linear {sorted(LINEAR_AXES)})"
                    )
            if "bend" in value:
                assert_joint_fold_is_anatomical(
                    key, float(value["bend"]), float(value.get("axis", 0.0)),
                    where=f"tick {tick}")


def _check_loop_closure(pose_table: Dict[int, dict], end_tick: int) -> None:
    """For looped animations, tick 0 and end_tick must match on every axis mentioned.

    Why: PlayerAnimator's Axis.findAfter synthesizes a virtual (endTick+1,
    defaultValue) frame when looping, so an axis keyed only at tick 0 linearly
    fades to 0 over the loop. See conventions doc §2 rule 8.
    """
    if 0 not in pose_table or end_tick not in pose_table:
        raise ValueError(f"looped anim must define both tick 0 and tick {end_tick}")
    pose0 = pose_table[0]
    pose_end = pose_table[end_tick]
    # Union of axes mentioned on either boundary.
    parts = (set(pose0.keys()) | set(pose_end.keys())) - RESERVED_KEYS
    problems = []
    for part in parts:
        axes0 = pose0.get(part, {})
        axesE = pose_end.get(part, {})
        all_axes = set(axes0.keys()) | set(axesE.keys())
        for axis in all_axes:
            v0 = axes0.get(axis)
            vE = axesE.get(axis)
            if v0 is None or vE is None or abs(float(v0) - float(vE)) > 1e-6:
                problems.append(f"  {part}.{axis}: tick 0 = {v0}, tick {end_tick} = {vE}")
    if problems:
        raise AssertionError(
            "loop boundary mismatch (tick 0 must equal tick {}):\n".format(end_tick)
            + "\n".join(problems)
        )


def build_doc(
    pose_table: Dict[int, dict],
    *,
    name: str,
    description: str,
    end_tick: int,
    stop_tick: int,
    is_loop: bool = False,
    return_tick: int = 0,
) -> dict:
    """Convert a POSE dict to an Emotecraft v3 JSON dict."""
    _validate_pose_table(pose_table)
    if is_loop:
        _check_loop_closure(pose_table, end_tick)
    if stop_tick < end_tick:
        raise ValueError(f"stop_tick ({stop_tick}) must be >= end_tick ({end_tick})")

    moves = []
    for tick in sorted(pose_table.keys()):
        pose = pose_table[tick]
        easing = pose.get("easing", "linear")
        for part_name, axes in pose.items():
            if part_name in RESERVED_KEYS:
                continue
            for axis_name, value in axes.items():
                val_out = d(float(value)) if axis_name in ANGLE_AXES else float(value)
                moves.append(
                    {
                        "tick": tick,
                        "easing": easing,
                        part_name: {axis_name: round(val_out, 7)},
                    }
                )

    return {
        "version": 3,
        "author": "Bong",
        "name": name,
        "description": description,
        "emote": {
            "beginTick": 0,
            "endTick": int(end_tick),
            "stopTick": int(stop_tick),
            "isLoop": bool(is_loop),
            "returnTick": int(return_tick),
            "nsfw": False,
            "degrees": False,
            "moves": moves,
        },
    }


def resolve_output_path(name: str) -> Path:
    """Write into the Fabric resource tree regardless of CWD."""
    here = Path(__file__).resolve().parent
    return here.parent / "src/main/resources/assets/bong/player_animation" / f"{name}.json"


def write_json(doc: dict, out_path: Optional[Path] = None) -> Path:
    if out_path is None:
        out_path = resolve_output_path(doc["name"])
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(doc, ensure_ascii=False, indent=2))
    return out_path


def emit_json(
    pose_table: Dict[int, dict],
    *,
    name: str,
    description: str,
    end_tick: int,
    stop_tick: int,
    is_loop: bool = False,
    return_tick: int = 0,
) -> Path:
    """One-shot: build + write + print a small summary."""
    doc = build_doc(
        pose_table,
        name=name,
        description=description,
        end_tick=end_tick,
        stop_tick=stop_tick,
        is_loop=is_loop,
        return_tick=return_tick,
    )
    path = write_json(doc)
    print(
        f"wrote {path.name}  "
        f"ticks={sorted(pose_table.keys())}  "
        f"moves={len(doc['emote']['moves'])}  "
        f"loop={is_loop}"
    )
    return path


# ---------------------------------------------------------------------------
# Pose inheritance helpers
# ---------------------------------------------------------------------------


def inherit(base_pose: dict, **overrides: dict) -> dict:
    """Shallow-merge a base pose with per-part overrides.

    Useful for guard-return frames (copy the guard pose) or mirror poses.
    Each override value is merged INTO the corresponding base part dict, so
    you can tweak just `rightArm.pitch` without retyping the other axes.
    """
    out: dict = {}
    for k, v in base_pose.items():
        if isinstance(v, dict):
            out[k] = dict(v)
        else:
            out[k] = v
    for part, axes in overrides.items():
        if part in RESERVED_KEYS or not isinstance(axes, dict):
            out[part] = axes
            continue
        merged = dict(out.get(part, {}))
        merged.update(axes)
        out[part] = merged
    return out


def mirror_pose(pose: dict, *, exclude_parts: Iterable[str] = ()) -> dict:
    """Mirror left/right of a pose in place — swap arms/legs and flip signs on
    symmetric axes (yaw, roll, body.x, body.yaw, head.yaw, torso.yaw, bend axis).

    Bend MAGNITUDE is preserved (pitch is preserved for the corresponding arm).
    Call for left-handed cross from a right-handed cross, etc.
    """
    exclude = set(exclude_parts)
    out: dict = {}
    for k, v in pose.items():
        if k in exclude:
            out[k] = v
            continue
        if isinstance(v, dict):
            out[k] = dict(v)
        else:
            out[k] = v

    # swap arms
    if "rightArm" in out or "leftArm" in out:
        ra = out.pop("rightArm", {})
        la = out.pop("leftArm", {})
        out["rightArm"] = _flip_axes(la)
        out["leftArm"] = _flip_axes(ra)
    # swap legs
    if "rightLeg" in out or "leftLeg" in out:
        rl = out.pop("rightLeg", {})
        ll = out.pop("leftLeg", {})
        out["rightLeg"] = _flip_axes(ll)
        out["leftLeg"] = _flip_axes(rl)

    # flip symmetric axes on central parts
    for part in ("body", "head", "torso"):
        axes = out.get(part)
        if not axes:
            continue
        flipped = dict(axes)
        if "x" in flipped:
            flipped["x"] = -flipped["x"]
        if "yaw" in flipped:
            flipped["yaw"] = -flipped["yaw"]
        if "roll" in flipped:
            flipped["roll"] = -flipped["roll"]
        out[part] = flipped
    return out


def _flip_axes(axes: dict) -> dict:
    """Flip yaw / roll signs. Flip bend axis around π (axis → 360°-axis).

    pitch and bend MAGNITUDE are preserved.
    """
    out = dict(axes)
    if "yaw" in out:
        out["yaw"] = -out["yaw"]
    if "roll" in out:
        out["roll"] = -out["roll"]
    if "axis" in out:
        # bend axis in degrees; mirror around vertical plane = 360 - axis (mod 360)
        out["axis"] = (360.0 - out["axis"]) % 360.0
    return out
