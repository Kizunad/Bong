#!/usr/bin/env python3
"""saber_slash_down — 青铜单刀专属单手顺步下劈斩。"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

POSE = {
    0: dict(  # 起手：持刀在右侧中段，左臂前引平衡
        easing="OUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-10),
        head=dict(pitch=-3, yaw=-4, roll=0),
        torso=dict(pitch=+2, yaw=+12),
        rightArm=dict(pitch=-45.0, yaw=-15.0, roll=+10.0, bend=25.0, axis=180),
        leftArm=dict(pitch=+20.0, yaw=+20.0, roll=-15.0, bend=20.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+3, bend=10.0, axis=0),
        leftLeg=dict(pitch=+6.0, yaw=+3, bend=8.0, axis=0),
    ),
    3: dict(  # 蓄势举刀过肩
        easing="INQUAD",
        body=dict(x=+0.04, y=-0.02, z=-0.05, yaw=-18),
        head=dict(pitch=-6, yaw=-2),
        torso=dict(pitch=-3, yaw=+20),
        rightArm=dict(pitch=-85.0, yaw=-20.0, roll=+15.0, bend=40.0, axis=180),
        leftArm=dict(pitch=+25.0, yaw=+25.0, roll=-20.0, bend=25.0, axis=180),
        rightLeg=dict(pitch=-16.0, yaw=+3, bend=12.0, axis=0),
        leftLeg=dict(pitch=+10.0, yaw=+3, bend=14.0, axis=0),
    ),
    7: dict(  # 沉身重劈 (Impact Frame)
        easing="OUTQUAD",
        body=dict(x=-0.04, y=-0.03, z=+0.16, yaw=+5),
        head=dict(pitch=+10, yaw=+8),
        torso=dict(pitch=+18, yaw=-15),
        rightArm=dict(pitch=+45.0, yaw=+10.0, roll=-15.0, bend=12.0, axis=180),
        leftArm=dict(pitch=-20.0, yaw=-18.0, roll=+20.0, bend=30.0, axis=180),
        rightLeg=dict(pitch=-5.0, yaw=+2, bend=22.0, axis=0),
        leftLeg=dict(pitch=+20.0, yaw=+2, bend=6.0, axis=0),
    ),
    12: dict(  # 收势复位
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
        name="saber_slash_down",
        description="青铜单刀专属顺步重劈斩",
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
