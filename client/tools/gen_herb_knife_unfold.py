#!/usr/bin/env python3
"""herb_knife_unfold — 凡铁折叠采药刀甩腕展开出刀动作。"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

POSE = {
    0: dict(  # 起手：右手自腰侧取出折刀横置胸前
        easing="OUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0, yaw=-5),
        head=dict(pitch=+10, yaw=-6, roll=0),
        torso=dict(pitch=+4, yaw=+8),
        rightArm=dict(pitch=-45.0, yaw=+15.0, roll=-30.0, bend=65.0, axis=180),
        leftArm=dict(pitch=+10.0, yaw=-10.0, roll=+10.0, bend=20.0, axis=180),
        rightLeg=dict(pitch=-5.0, yaw=0, bend=5.0, axis=0),
        leftLeg=dict(pitch=+5.0, yaw=0, bend=5.0, axis=0),
    ),
    4: dict(  # 甩腕亮刃：手腕轻抖顺势甩出刀刃锁定
        easing="OUTQUAD",
        body=dict(x=+0.01, y=0.0, z=0.02, yaw=+4),
        head=dict(pitch=+5, yaw=+2, roll=0),
        torso=dict(pitch=+2, yaw=-5),
        rightArm=dict(pitch=-15.0, yaw=-20.0, roll=+25.0, bend=20.0, axis=180),
        leftArm=dict(pitch=+15.0, yaw=-12.0, roll=+12.0, bend=25.0, axis=180),
        rightLeg=dict(pitch=-4.0, yaw=0, bend=4.0, axis=0),
        leftLeg=dict(pitch=+4.0, yaw=0, bend=4.0, axis=0),
    ),
    8: dict(  # 定式持刀架势
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0, yaw=0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=0.0, yaw=0.0, roll=0.0, bend=0.0, axis=180),
        leftArm=dict(pitch=0.0, yaw=0.0, roll=0.0, bend=0.0, axis=180),
        rightLeg=dict(pitch=0.0, yaw=0.0, roll=0.0, bend=0.0, axis=0),
        leftLeg=dict(pitch=0.0, yaw=0.0, roll=0.0, bend=0.0, axis=0),
    ),
}

if __name__ == "__main__":
    emit_json(
        POSE,
        name="herb_knife_unfold",
        description="凡铁采药刀甩腕亮刃展开",
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
