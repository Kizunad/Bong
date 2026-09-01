#!/usr/bin/env python3
"""herb_knife_slash — 凡铁采药刀小幅度迅速反手割划攻击动作。"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

POSE = {
    0: dict(  # 起手：反手微拉，重心微沉，准备前刺/反割
        easing="OUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-8),
        head=dict(pitch=-2, yaw=-3, roll=0),
        torso=dict(pitch=+4, yaw=+10),
        rightArm=dict(pitch=-35.0, yaw=-20.0, roll=+25.0, bend=35.0, axis=180),
        leftArm=dict(pitch=+15.0, yaw=+15.0, roll=-10.0, bend=15.0, axis=180),
        rightLeg=dict(pitch=-8.0, yaw=+2, bend=8.0, axis=0),
        leftLeg=dict(pitch=+5.0, yaw=+2, bend=6.0, axis=0),
    ),
    2: dict(  # 蓄势拉至侧腰
        easing="INQUAD",
        body=dict(x=+0.03, y=-0.01, z=-0.03, yaw=-12),
        head=dict(pitch=-4, yaw=-2),
        torso=dict(pitch=+2, yaw=+16),
        rightArm=dict(pitch=-55.0, yaw=-25.0, roll=+30.0, bend=50.0, axis=180),
        leftArm=dict(pitch=+20.0, yaw=+20.0, roll=-15.0, bend=20.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+2, bend=10.0, axis=0),
        leftLeg=dict(pitch=+8.0, yaw=+2, bend=12.0, axis=0),
    ),
    5: dict(  # 迅捷弧线割击 (Impact Frame: 刀尖向前方斜向划出)
        easing="OUTQUAD",
        body=dict(x=-0.03, y=-0.02, z=+0.12, yaw=+6),
        head=dict(pitch=+6, yaw=+6),
        torso=dict(pitch=+12, yaw=-14),
        rightArm=dict(pitch=+30.0, yaw=+15.0, roll=-20.0, bend=15.0, axis=180),
        leftArm=dict(pitch=-15.0, yaw=-12.0, roll=+15.0, bend=25.0, axis=180),
        rightLeg=dict(pitch=-4.0, yaw=+2, bend=18.0, axis=0),
        leftLeg=dict(pitch=+16.0, yaw=+2, bend=5.0, axis=0),
    ),
    9: dict(  # 收势复位
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
        name="herb_knife_slash",
        description="凡铁采药刀短快反手割击",
        end_tick=9,
        stop_tick=11,
        is_loop=False,
    )
