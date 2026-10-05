#!/usr/bin/env python3
"""bing_jia_heavy_left —— 兵甲手套（铁甲覆手）左重拳（重拳，要「沉」）。

只落资产，不接线：产出 player_animation/bing_jia_heavy_left.json。

左式不单独手调，直接由 gen_bing_jia_heavy_right.py 的右式镜像得到（anim_common.mirror_pose：
左右臂腿对调，yaw / roll / body x 取反，axis 取 2π−axis）。这样左右两式的「沉」、节奏、
余震、收势完全一致，只差左右手。设计要点见右式文件头。
"""

from anim_common import emit_json, mirror_pose
from gen_bing_jia_heavy_right import POSE as RIGHT_POSE

POSE = {tick: mirror_pose(pose) for tick, pose in RIGHT_POSE.items()}

DESCRIPTION = (
    "兵甲手套铁甲左重拳（重拳，要「沉」）：16 tick。"
    "与右式镜像对称：先坐住（躯干拧、身体下沉、重心后移、后腿屈）→ 拳收到耳后、重心压后腿 → "
    "腰胯带动、躯干先反拧，手臂晚一拍到位 → overshoot 过冲、rebound 回弹 → 慢收回 guard。"
    "拳到位后有 3 tick 余震，远处看和空手拳不同。"
)

if __name__ == "__main__":
    emit_json(POSE, name="bing_jia_heavy_left", description=DESCRIPTION,
              end_tick=16, stop_tick=18, is_loop=False)
