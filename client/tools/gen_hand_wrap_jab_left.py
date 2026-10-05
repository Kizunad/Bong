#!/usr/bin/env python3
"""hand_wrap_jab_left —— 缠手（布缠）左直拳（轻拳，快、碎）。

只落资产，不接线：产出 player_animation/hand_wrap_jab_left.json。

左式不单独手调，直接由 gen_hand_wrap_jab_right.py 的右式镜像得到（anim_common.mirror_pose：
左右臂腿对调，yaw / roll / body x 取反，axis 取 2π−axis）。这样左右两式的节奏、幅度、
间隔完全一致，只差左右手。设计要点见右式文件头。
"""

from anim_common import emit_json, mirror_pose
from gen_hand_wrap_jab_right import POSE as RIGHT_POSE

POSE = {tick: mirror_pose(pose) for tick, pose in RIGHT_POSE.items()}

DESCRIPTION = (
    "缠手布缠左直拳（轻拳）：8 tick，快、碎。"
    "与右式镜像对称：两拳贴脸 → 左腕外旋 → 左拳出、右拳同步收紧 → "
    "impact 短距离打出、腕翻约 58°、躯干只拧约 8° → overshoot → recover → 回 guard。"
    "左右两式间隔短、身体几乎不晃。"
)

if __name__ == "__main__":
    emit_json(POSE, name="hand_wrap_jab_left", description=DESCRIPTION,
              end_tick=8, stop_tick=10, is_loop=False)
