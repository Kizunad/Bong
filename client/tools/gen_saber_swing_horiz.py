#!/usr/bin/env python3
"""saber_swing_horiz — 青铜单刀专属大角度平抹横斩。"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

POSE = {
    0: dict(  # 起手：持刀架于右胸前
        easing="OUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-10),
        head=dict(pitch=-2, yaw=-4, roll=0),
        torso=dict(pitch=+1, yaw=+14),
        rightArm=dict(pitch=-25.0, yaw=-20.0, roll=+15.0, bend=25.0, axis=180),
        leftArm=dict(pitch=+15.0, yaw=+18.0, roll=-12.0, bend=18.0, axis=180),
        rightLeg=dict(pitch=-10.0, yaw=+2, bend=8.0, axis=0),
        leftLeg=dict(pitch=+5.0, yaw=+2, bend=6.0, axis=0),
    ),
    3: dict(  # 蓄势引刀向右后
        easing="INQUAD",
        body=dict(x=+0.04, y=-0.02, z=-0.04, yaw=-22),
        head=dict(pitch=-4, yaw=-2),
        torso=dict(pitch=-2, yaw=+25),
        rightArm=dict(pitch=-15.0, yaw=-45.0, roll=+25.0, bend=35.0, axis=180),
        leftArm=dict(pitch=+20.0, yaw=+22.0, roll=-18.0, bend=22.0, axis=180),
        rightLeg=dict(pitch=-14.0, yaw=+3, bend=10.0, axis=0),
        leftLeg=dict(pitch=+8.0, yaw=+3, bend=12.0, axis=0),
    ),
    7: dict(  # 横扫平抹 (Impact Frame)
        easing="OUTQUAD",
        body=dict(x=-0.03, y=-0.02, z=+0.12, yaw=+12),
        head=dict(pitch=+6, yaw=+10),
        torso=dict(pitch=+12, yaw=-20),
        rightArm=dict(pitch=+15.0, yaw=+45.0, roll=-15.0, bend=12.0, axis=180),
        leftArm=dict(pitch=-15.0, yaw=-22.0, roll=+22.0, bend=25.0, axis=180),
        rightLeg=dict(pitch=-6.0, yaw=+2, bend=18.0, axis=0),
        leftLeg=dict(pitch=+15.0, yaw=+2, bend=6.0, axis=0),
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
        name="saber_swing_horiz",
        description="青铜单刀专属大范围平抹横斩",
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
