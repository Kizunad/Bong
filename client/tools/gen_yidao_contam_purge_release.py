#!/usr/bin/env python3
"""yidao_contam_purge_release —— 排异加速散烟收势（P4）。

两段式 release 段（蓄力段见 gen_yidao_contam_purge_loop.py，isLoop）。引导
自然完成且结算有效时由 server StopAnim(蓄力段)+PlayAnim(本段) 接力；打断/
无效完成只 StopAnim（收势只奖励有效结算）。

母题：散烟。最后一波灸火按定后，双掌交叠收拢一合（合烟），随即向左右
对称外拂把余烟残秽扫散（双臂 yaw 大开 + 掌心外翻 roll），身体微直起吐气，
双臂自然落回。左右对称大开的横向拂扫是本收势辨识核心（接经收针是右臂
单侧上挑，续命是右臂纵向引落）。

时序（精度标准 #1/#2/#3）：
  anticipation 0→3   合烟：双掌交叠收拢（bend 收满，OUTQUAD 蓄）
  strike       3→7   外拂：双臂对称向外横扫（yaw ∓14→±34 / bend 64→14 /
                     掌心外翻 roll），身体直起吐气，t7 = 拂散顶点
  recovery     7→12  双臂落回归中立（INOUTSINE，t9 中段帧）
endTick=12，stopTick=14，非循环。主打击轴：rightArm.yaw / leftArm.yaw /
rightArm.pitch。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 承接蓄力段聚火位。
    0: dict(
        easing="OUTQUAD",
        body=dict(y=-0.03, z=0.0),
        head=dict(pitch=+10, yaw=0),
        torso=dict(pitch=+7, yaw=0),
        rightArm=dict(pitch=-58, yaw=-14, roll=-8, bend=62, axis=180),
        leftArm=dict(pitch=-58, yaw=+14, roll=+8, bend=62, axis=180),
        leftLeg=dict(pitch=-6, bend=10, z=-0.03),
        rightLeg=dict(pitch=+5, bend=9, z=+0.03),
    ),
    # 合烟：双掌交叠收拢到底。
    3: dict(
        easing="OUTQUAD",
        body=dict(y=-0.045, z=+0.01),
        head=dict(pitch=+12, yaw=0),
        torso=dict(pitch=+9, yaw=0),
        rightArm=dict(pitch=-60, yaw=-18, roll=-10, bend=64, axis=180),
        leftArm=dict(pitch=-60, yaw=+18, roll=+10, bend=64, axis=180),
        leftLeg=dict(pitch=-7, bend=11, z=-0.03),
        rightLeg=dict(pitch=+6, bend=10, z=+0.03),
    ),
    # 外拂中段：双臂开扇过半、掌心外翻。
    5: dict(
        easing="INQUAD",
        body=dict(y=-0.02, z=+0.01),
        head=dict(pitch=+6, yaw=0),
        torso=dict(pitch=+4, yaw=0),
        rightArm=dict(pitch=-52, yaw=-26, roll=+10, bend=36, axis=180),
        leftArm=dict(pitch=-52, yaw=+26, roll=-10, bend=36, axis=180),
        leftLeg=dict(pitch=-5, bend=8, z=-0.02),
        rightLeg=dict(pitch=+4, bend=7, z=+0.02),
    ),
    # 拂散顶点：双臂对称大开、身体直起吐气。
    7: dict(
        easing="OUTSINE",
        body=dict(y=+0.005, z=0.0),
        head=dict(pitch=-2, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=-46, yaw=-34, roll=+18, bend=14, axis=180),
        leftArm=dict(pitch=-46, yaw=+34, roll=-18, bend=14, axis=180),
        leftLeg=dict(pitch=-3, bend=5, z=-0.02),
        rightLeg=dict(pitch=+3, bend=4, z=+0.02),
    ),
    # 落臂中段。
    9: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=-22, yaw=-16, roll=+8, bend=8, axis=180),
        leftArm=dict(pitch=-22, yaw=+16, roll=-8, bend=8, axis=180),
        leftLeg=dict(pitch=-2, bend=2, z=-0.01),
        rightLeg=dict(pitch=+2, bend=2, z=+0.01),
    ),
    # 归中立。
    12: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftLeg=dict(pitch=0, bend=0, z=0.0),
        rightLeg=dict(pitch=0, bend=0, z=0.0),
    ),
}


def main() -> int:
    emit_json(
        POSE,
        name="yidao_contam_purge_release",
        description=(
            "P4 排异加速散烟收势（12t 非循环）：anticipation 0→3 双掌合烟收拢，"
            "strike 3→7 双臂对称外拂扫散（yaw ∓18→±34 / bend 64→14 / roll 外翻），"
            "recovery 7→12 落臂归中立。"
        ),
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
