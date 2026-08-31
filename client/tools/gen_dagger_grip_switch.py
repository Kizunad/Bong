#!/usr/bin/env python3
"""dagger_grip_switch — 匕首正握 ↔ 反握顺滑转刀过渡动画。

动作要领：
    - 正手转反手（0 -> 8 tick）：
      - tick 0: 正手前探低架持刀
      - tick 2: 略微上挑松指，手腕向内收缩起跳
      - tick 4: 掌心放空/刀柄抛指翻转，小臂横展（roll 从负翻正，刀尖完成 180° 自转倒把）
      - tick 6: 反手握紧，指节合拢卡位，微震制动
      - tick 8: 稳妥进入反手防守斜架
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from anim_common import emit_json

POSE = {
    0: dict(  # 正手持匕低架 (对齐 dagger_stab / dagger_slash 的 guard)
        easing="OUTSINE",
        body=dict(x=+0.03, y=0.0, z=0.0, yaw=-34),
        head=dict(pitch=+2, yaw=+26),
        torso=dict(pitch=+4, yaw=+18),
        rightArm=dict(pitch=-4, yaw=-10, roll=-6, bend=86, axis=180),
        leftArm=dict(pitch=-32, yaw=+26, roll=-6, bend=72, axis=180),
        rightLeg=dict(pitch=+8, yaw=+4, bend=16, z=+0.04),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.05),
    ),
    2: dict(  # 起转 —— 手腕上提挑指，刀尖上扬
        easing="INOUTSINE",
        body=dict(x=+0.02, y=+0.01, z=0.0, yaw=-32),
        head=dict(pitch=+1, yaw=+24),
        torso=dict(pitch=+4, yaw=+20),
        rightArm=dict(pitch=-24, yaw=-14, roll=+15, bend=80, axis=180),
        leftArm=dict(pitch=-30, yaw=+25, roll=-8, bend=76, axis=180),
        rightLeg=dict(pitch=+8, yaw=+4, bend=16, z=+0.04),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.05),
    ),
    4: dict(  # 空中翻转 —— 掌心松弛换把，roll 与 yaw 迅速交替过渡
        easing="INOUTQUAD",
        body=dict(x=+0.01, y=+0.02, z=0.0, yaw=-30),
        head=dict(pitch=0, yaw=+22),
        torso=dict(pitch=+3, yaw=+22),
        rightArm=dict(pitch=-38, yaw=-20, roll=+60, bend=65, axis=180),
        leftArm=dict(pitch=-28, yaw=+24, roll=-10, bend=82, axis=180),
        rightLeg=dict(pitch=+8, yaw=+4, bend=15, z=+0.03),
        leftLeg=dict(pitch=-10, yaw=+4, bend=17, z=-0.04),
    ),
    6: dict(  # 反手合指抓握 —— 刀尖朝下，掌心收拢制动
        easing="OUTQUAD",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-30),
        head=dict(pitch=-1, yaw=+20),
        torso=dict(pitch=+3, yaw=+22),
        rightArm=dict(pitch=-32, yaw=-16, roll=+48, bend=72, axis=180),
        leftArm=dict(pitch=-25, yaw=+22, roll=-12, bend=88, axis=180),
        rightLeg=dict(pitch=+8, yaw=+4, bend=14, z=+0.03),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.04),
    ),
    8: dict(  # 稳妥落入反手斜架 (对齐 dagger_reverse_slash 的 guard)
        easing="INOUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-30),
        head=dict(pitch=-2, yaw=+20),
        torso=dict(pitch=+3, yaw=+22),
        rightArm=dict(pitch=-30, yaw=-18, roll=+45, bend=70, axis=180),
        leftArm=dict(pitch=-24, yaw=+22, roll=-12, bend=90, axis=180),
        rightLeg=dict(pitch=+8, yaw=+4, bend=14, z=+0.03),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.04),
    ),
}

if __name__ == "__main__":
    emit_json(
        POSE,
        name="dagger_grip_switch",
        description="匕首正握到反握转刀过渡：挑指翻转倒把，顺滑切换握姿",
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
