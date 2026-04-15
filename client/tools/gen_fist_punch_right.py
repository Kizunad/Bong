#!/usr/bin/env python3
"""Generate fist_punch_right.json from a compact pose-per-tick design table.

Each pose is a dict of (part, axis) -> value (radians for angles, meters for xyz).
Omitted axes default to rest value (0 for most; the library's own defaults fire
for leg z etc.).

v10 goals per user feedback:
  - 双手需要向内收 (pull hands inward toward centerline)
  - 出拳幅度需要大 (bigger swing — load deeper, impact farther)
  - 出拳手臂完全伸直 (right arm near-straight at impact, bend ≈ 0)
  - 侧身 (sideways stance for longer reach — lean into punch with body.x/body.z
    and torso.yaw rotation)

Design: orthodox stance. Left foot forward, right foot back. Right cross punch.
Frames:
    tick 0 = ready guard (standing, hands at face, inward)
    tick 3 = load / chamber (right fist pulled to chest, torso wound back)
    tick 5 = impact (right arm fully extended across centerline)
    tick 7 = mid-recovery (right retracting toward guard)
    tick 10 = guard return (same silhouette as tick 0)
"""

from __future__ import annotations
import json
import math
from pathlib import Path

d = math.radians  # shorthand: degrees → radians

# Part fields that hold angles (values in radians after the `d()` conversion)
ANGLE_AXES = {"pitch", "yaw", "roll", "bend", "axis"}
# Part fields that hold linear xyz (no conversion)
LINEAR_AXES = {"x", "y", "z"}


# --------- v10 pose table ---------------------------------------------------
# Compact encoding. Only list changes; missing axes default to 0.
# Units: angles in DEGREES (converted at emit time since emote.degrees=false → radians);
# xyz in MC scaling (already in "meters"-ish units, NOT scaled).

POSE_V10 = {
    # --- GUARD: ready position, hands at face inward, slight orthodox lean ---
    0: dict(
        easing="INOUTSINE",
        body=dict(x=+0.05, y=-0.05, z=+0.00),
        head=dict(pitch=-5, yaw=-8),
        torso=dict(pitch=+5, yaw=+15),
        # right arm: stronger roll + negative yaw to pull hand to centerline
        # (roll +35 + yaw -10 → hand ~(-2, -4, -5) = centerline at face)
        rightArm=dict(pitch=-88, yaw=-10, roll=+35, bend=100, axis=180),
        # left arm: mirror
        leftArm=dict(pitch=-88, yaw=+10, roll=-35, bend=100, axis=180),
        # front leg forward (orthodox: left foot lead)
        leftLeg=dict(pitch=-18, yaw=+6, bend=25, z=-0.15),
        rightLeg=dict(pitch=+10, yaw=+5, bend=15, z=+0.05),
    ),
    # --- LOAD: wind-up ~tick 3. Right fist chambered at chest, torso coiled. ---
    3: dict(
        easing="INOUTSINE",
        body=dict(x=+0.10, y=+0.03, z=-0.05),
        head=dict(pitch=-5, yaw=-18),  # keep gaze on target
        torso=dict(pitch=+10, yaw=+30),  # coil further back
        # right arm: pitch lower (elbow dropped), bend very tight to chamber fist
        rightArm=dict(pitch=-55, yaw=-10, roll=+28, bend=145, axis=180),
        leftArm=dict(pitch=-90, yaw=+8, roll=-25, bend=85, axis=180),
        leftLeg=dict(pitch=-12, yaw=+6, bend=22, z=-0.14),
        rightLeg=dict(pitch=+15, yaw=+6, bend=20, z=+0.07),
    ),
    # --- IMPACT: tick 5. Right arm fully straight, crosses centerline, body lunges in. ---
    5: dict(
        easing="OUTQUAD",
        # big body lunge forward, slight rise on extension
        body=dict(x=-0.10, y=-0.02, z=+0.22),
        head=dict(pitch=-6, yaw=+12),
        # torso rotates FORWARD (right shoulder comes through)
        torso=dict(pitch=+4, yaw=-32),
        # right arm: fully extended, fist rises to chin/face level (pitch -100°),
        # yaw=-22° pulls fist across body centerline, bend=3° fully straight.
        rightArm=dict(pitch=-100, yaw=-22, roll=+10, bend=3, axis=180),
        # left arm retracts toward chin (standard boxing: rear hand covers face on extension)
        leftArm=dict(pitch=-88, yaw=+10, roll=-35, bend=100, axis=180),
        # legs: push off back, load front
        leftLeg=dict(pitch=-22, yaw=+6, bend=30, z=-0.18),
        rightLeg=dict(pitch=+8, yaw=+8, bend=10, z=+0.04),
    ),
    # --- RECOVERY: tick 7. Retract right, torso untwists. ---
    7: dict(
        easing="OUTQUAD",
        body=dict(x=-0.02, y=+0.02, z=+0.10),
        head=dict(pitch=-6, yaw=+2),
        torso=dict(pitch=+6, yaw=+5),
        rightArm=dict(pitch=-88, yaw=-10, roll=+30, bend=60, axis=180),
        leftArm=dict(pitch=-88, yaw=+10, roll=-32, bend=95, axis=180),
        leftLeg=dict(pitch=-16, yaw=+6, bend=25, z=-0.16),
        rightLeg=dict(pitch=+12, yaw=+6, bend=15, z=+0.06),
    ),
    # --- GUARD RETURN: tick 10 matches tick 0 ---
    10: dict(
        easing="INOUTSINE",
        body=dict(x=+0.05, y=-0.05, z=+0.00),
        head=dict(pitch=-5, yaw=-8),
        torso=dict(pitch=+5, yaw=+15),
        rightArm=dict(pitch=-88, yaw=-10, roll=+35, bend=100, axis=180),
        leftArm=dict(pitch=-88, yaw=+10, roll=-35, bend=100, axis=180),
        leftLeg=dict(pitch=-18, yaw=+6, bend=25, z=-0.15),
        rightLeg=dict(pitch=+10, yaw=+5, bend=15, z=+0.05),
    ),
}


DESCRIPTION_V10 = (
    "v10 完整侧身 cross: 双手内收至中线 (guard roll±25° + yaw±5° 把手臂往胸前拉), "
    "出拳幅度加大 (LOAD pitch-55°/bend145° 贴肋, IMPACT pitch-92°/bend3° 手臂完全伸直 yaw-22° 穿中线), "
    "身体大幅侧转 (torso 右肩后+15°→+30°→左转-32° = 62° 扭矩), "
    "body.z 前冲 0.22m 加 body.x 重心切换 (+0.05→+0.10→-0.10)。"
)


def emit(pose_table: dict, description: str) -> dict:
    """Convert pose table to Emotecraft v3 JSON dict (ready to json.dump)."""
    moves = []
    # Sort ticks
    for tick in sorted(pose_table.keys()):
        pose = pose_table[tick]
        easing = pose.get("easing", "linear")
        # Emit ONE entry per (part, axis) so each move has a single axis value.
        # This matches the existing JSON format style.
        for part_name, part_axes in pose.items():
            if part_name == "easing":
                continue
            for axis_name, value in part_axes.items():
                entry = {"tick": tick, "easing": easing}
                # convert degrees -> radians for angle axes
                val_out = d(value) if axis_name in ANGLE_AXES else float(value)
                entry[part_name] = {axis_name: round(val_out, 7)}
                moves.append(entry)

    doc = {
        "version": 3,
        "author": "Bong",
        "name": "fist_punch_right",
        "description": description,
        "emote": {
            "beginTick": 0,
            "endTick": 10,
            "stopTick": 12,
            "isLoop": False,
            "returnTick": 0,
            "nsfw": False,
            "degrees": False,
            "moves": moves,
        },
    }
    return doc


def main() -> int:
    out_path = Path(__file__).resolve().parent.parent / "src/main/resources/assets/bong/player_animation/fist_punch_right.json"
    doc = emit(POSE_V10, DESCRIPTION_V10)
    out_path.write_text(json.dumps(doc, ensure_ascii=False, indent=2))
    print(f"wrote {out_path}  ({len(doc['emote']['moves'])} move entries)")
    return 0


if __name__ == "__main__":
    import sys
    sys.exit(main())
