#!/usr/bin/env python3
"""herb_harvest — 凡铁采药刀俯身专注采割灵草动作。"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

POSE = {
    0: dict(  # 准备：俯身下蹲，左手探向地面按住灵草基底，右手持刀抬起
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.22, z=0.08, yaw=+5),
        head=dict(pitch=+35, yaw=-5, roll=0),
        torso=dict(pitch=+30, yaw=+8),
        rightArm=dict(pitch=-40.0, yaw=+10.0, roll=-25.0, bend=55.0, axis=180),
        leftArm=dict(pitch=+45.0, yaw=-15.0, roll=+20.0, bend=40.0, axis=180),
        rightLeg=dict(pitch=-35.0, yaw=0, bend=38.0, axis=0),
        leftLeg=dict(pitch=+15.0, yaw=0, bend=35.0, axis=0),
    ),
    4: dict(  # 下刀割取：右手鹰嘴刀刃贴地斜切入土剥离根茎
        easing="INOUTQUAD",
        body=dict(x=+0.01, y=-0.25, z=0.10, yaw=+8),
        head=dict(pitch=+40, yaw=-3, roll=0),
        torso=dict(pitch=+34, yaw=+12),
        rightArm=dict(pitch=+25.0, yaw=+5.0, roll=-30.0, bend=35.0, axis=180),
        leftArm=dict(pitch=+50.0, yaw=-12.0, roll=+25.0, bend=45.0, axis=180),
        rightLeg=dict(pitch=-38.0, yaw=0, bend=40.0, axis=0),
        leftLeg=dict(pitch=+18.0, yaw=0, bend=38.0, axis=0),
    ),
    8: dict(  # 弧线勾割提刃：刀尖向后顺势拉割灵草根须
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.23, z=0.09, yaw=+4),
        head=dict(pitch=+36, yaw=-4, roll=0),
        torso=dict(pitch=+32, yaw=+6),
        rightArm=dict(pitch=+10.0, yaw=-15.0, roll=-15.0, bend=60.0, axis=180),
        leftArm=dict(pitch=+42.0, yaw=-18.0, roll=+18.0, bend=38.0, axis=180),
        rightLeg=dict(pitch=-36.0, yaw=0, bend=38.0, axis=0),
        leftLeg=dict(pitch=+16.0, yaw=0, bend=36.0, axis=0),
    ),
    12: dict(  # 完成采收复位
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
        name="herb_harvest",
        description="凡铁采药刀俯身专注采割灵草",
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
