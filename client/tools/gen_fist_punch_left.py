#!/usr/bin/env python3
"""fist_punch_left — 南爪姿左直拳。

直接 import fist_punch_right v10 的 POSE 表，逐帧 mirror_pose (左右翻)。
v10 细节：双手内收 roll±35°, IMPACT bend=3° 完全伸直, torso 62° 扭矩, body.z +0.22 前冲。

Mirror 规则 (anim_common.mirror_pose)：
  - rightArm ↔ leftArm 交换
  - rightLeg ↔ leftLeg 交换
  - yaw / roll 符号翻转
  - body.x / body.yaw / head.yaw / torso.yaw / torso.roll 符号翻转
  - bend axis 做 360°-axis 翻转 (180° → 180° 不变, 170° → 190°)
  - pitch / bend / body.y / body.z / torso.pitch / head.pitch 保持不变

也就是说：南爪姿 (右脚前 / 左脚后) 从正架的右 cross 变成左 cross。
"""
from anim_common import emit_json, mirror_pose

from gen_fist_punch_right import POSE_V10 as _SRC

POSE = {tick: mirror_pose(frame) for tick, frame in _SRC.items()}

DESCRIPTION = (
    "v10 南爪姿 left cross: 双手内收至中线, LOAD pitch-55°/bend145° 贴肋, "
    "IMPACT pitch-92°/bend3° 左臂完全伸直 yaw+22° 穿中线, torso 62° 反扭, body.z 0.22m 前冲。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="fist_punch_left",
        description=DESCRIPTION,
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
