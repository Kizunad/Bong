#!/usr/bin/env python3
"""anqi_multi_shot —— 多发齐射：胸前拢镖蓄势→扇形撒出（双臂开扇）（P2 批次二前半）。

cast_ticks=30，去复用（原借 release_burst）。对拍区间 endTick ∈ [34,38]，取 36。

母题：群镖齐射。双手在胸前拢镖蓄势（掬捧收紧、含胸沉桩，t12→t15 有一次
"再攥紧"的 load-snap 呼吸），然后双臂向外上方同时开扇撒出——左右对称的
大开扇是本招与单射/其它暗器招最强的远距辨识特征。

时序（精度标准 #1/#2/#3）：
  anticipation 0→18  胸前拢镖：双手掬捧至胸口（bend 60→115）、含胸 +15、
                     重心下沉 -0.07（easeOut 族 OUTSINE，t15 微松 load-snap）
  strike       18→30 扇形撒出：双臂对称外开上扬（yaw ±26→∓45 / bend 115→5）、
                     挺胸展背 -6、重心升起前送（easeIn 族 INQUAD），
                     顶点 = tick 30（cast 完成瞬间，双臂全开扇位）
  recovery     30→36 开扇位收落回 guard（INOUTSINE，t33 中段帧）
endTick=36，stopTick=38，非循环。主打击轴：rightArm.yaw / leftArm.yaw /
rightArm.bend / body.z。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 对称持镖 guard：双手中位持镖、正身浅站架。
GUARD = dict(
    easing="INOUTSINE",
    body=dict(y=0.0, z=0.0),
    head=dict(pitch=-3),
    torso=dict(pitch=+4, yaw=0),
    rightArm=dict(pitch=-50, yaw=+5, roll=+10, bend=60, axis=180),
    leftArm=dict(pitch=-50, yaw=-5, roll=-10, bend=60, axis=180),
    leftLeg=dict(pitch=-10, bend=12, z=-0.05),
    rightLeg=dict(pitch=+8, bend=10, z=+0.04),
)

# 开扇顶点（tick 30 = cast 完成瞬间）：双臂对称全开扇、挺胸展背、重心前送。
APEX = dict(
    easing="INQUAD",
    body=dict(y=+0.04, z=+0.12),
    head=dict(pitch=-8),
    torso=dict(pitch=-6, yaw=0),
    rightArm=dict(pitch=-95, yaw=-45, roll=-5, bend=5, axis=180),
    leftArm=dict(pitch=-95, yaw=+45, roll=+5, bend=5, axis=180),
    leftLeg=dict(pitch=-20, bend=22, z=-0.09),
    rightLeg=dict(pitch=+16, bend=20, z=+0.07),
)

POSE = {
    0: GUARD,
    # 拢镖起手：双手向胸口中线并拢。
    4: dict(
        easing="OUTSINE",
        body=dict(y=-0.02, z=-0.03),
        head=dict(pitch=+2),
        torso=dict(pitch=+7, yaw=0),
        rightArm=dict(pitch=-58, yaw=+14, roll=+18, bend=78, axis=180),
        leftArm=dict(pitch=-58, yaw=-14, roll=-18, bend=78, axis=180),
        leftLeg=dict(pitch=-11, bend=14, z=-0.05),
        rightLeg=dict(pitch=+9, bend=13, z=+0.04),
    ),
    # 掬捧至胸口：含胸、低头看镖。
    8: dict(
        easing="OUTSINE",
        body=dict(y=-0.04, z=-0.05),
        head=dict(pitch=+6),
        torso=dict(pitch=+10, yaw=0),
        rightArm=dict(pitch=-64, yaw=+20, roll=+24, bend=95, axis=180),
        leftArm=dict(pitch=-64, yaw=-20, roll=-24, bend=95, axis=180),
        leftLeg=dict(pitch=-12, bend=18, z=-0.06),
        rightLeg=dict(pitch=+10, bend=16, z=+0.05),
    ),
    # 攥紧第一波：拢得更深、沉桩。
    12: dict(
        easing="OUTSINE",
        body=dict(y=-0.06, z=-0.07),
        head=dict(pitch=+9),
        torso=dict(pitch=+13, yaw=0),
        rightArm=dict(pitch=-68, yaw=+24, roll=+28, bend=108, axis=180),
        leftArm=dict(pitch=-68, yaw=-24, roll=-28, bend=108, axis=180),
        leftLeg=dict(pitch=-14, bend=22, z=-0.07),
        rightLeg=dict(pitch=+12, bend=20, z=+0.06),
    ),
    # load-snap 微松：呼吸感（反相位，防"慢慢收紧"的单调）。
    15: dict(
        easing="OUTSINE",
        body=dict(y=-0.055, z=-0.065),
        head=dict(pitch=+8),
        torso=dict(pitch=+11, yaw=0),
        rightArm=dict(pitch=-66, yaw=+22, roll=+26, bend=100, axis=180),
        leftArm=dict(pitch=-66, yaw=-22, roll=-26, bend=100, axis=180),
        leftLeg=dict(pitch=-13, bend=20, z=-0.07),
        rightLeg=dict(pitch=+11, bend=19, z=+0.06),
    ),
    # 蓄势顶点：极限攥紧、含胸最深、腿弓最沉。
    18: dict(
        easing="OUTSINE",
        body=dict(y=-0.07, z=-0.09),
        head=dict(pitch=+11),
        torso=dict(pitch=+15, yaw=0),
        rightArm=dict(pitch=-72, yaw=+26, roll=+30, bend=115, axis=180),
        leftArm=dict(pitch=-72, yaw=-26, roll=-30, bend=115, axis=180),
        leftLeg=dict(pitch=-15, bend=26, z=-0.08),
        rightLeg=dict(pitch=+13, bend=24, z=+0.07),
    ),
    # 开扇启动：双臂开始外开上扬、躯干直起。
    22: dict(
        easing="INQUAD",
        body=dict(y=-0.03, z=0.0),
        head=dict(pitch=+2),
        torso=dict(pitch=+8, yaw=0),
        rightArm=dict(pitch=-78, yaw=0, roll=+18, bend=70, axis=180),
        leftArm=dict(pitch=-78, yaw=0, roll=-18, bend=70, axis=180),
        leftLeg=dict(pitch=-13, bend=18, z=-0.07),
        rightLeg=dict(pitch=+11, bend=16, z=+0.05),
    ),
    # 开扇中段：双臂过肩线向外撒。
    26: dict(
        easing="INQUAD",
        body=dict(y=+0.01, z=+0.06),
        head=dict(pitch=-4),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=-85, yaw=-25, roll=+8, bend=30, axis=180),
        leftArm=dict(pitch=-85, yaw=+25, roll=-8, bend=30, axis=180),
        leftLeg=dict(pitch=-16, bend=18, z=-0.08),
        rightLeg=dict(pitch=+13, bend=16, z=+0.06),
    ),
    # 开扇顶点 = cast 完成（tick 30）。
    30: APEX,
    # 收落中段：双臂由开扇位落回。
    33: dict(
        easing="INOUTSINE",
        body=dict(y=+0.01, z=+0.06),
        head=dict(pitch=-5),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=-70, yaw=-18, roll=+4, bend=35, axis=180),
        leftArm=dict(pitch=-70, yaw=+18, roll=-4, bend=35, axis=180),
        leftLeg=dict(pitch=-15, bend=16, z=-0.07),
        rightLeg=dict(pitch=+12, bend=14, z=+0.05),
    ),
    # 收势回 guard。
    36: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="anqi_multi_shot",
        description=(
            "P2 多发齐射专属：anticipation 0→18 胸前拢镖蓄势（双手掬捧 bend "
            "60→115 / 含胸 torso +15 / 沉桩 body.y -0.07，t15 load-snap 微松），"
            "strike 18→30 扇形撒出（双臂对称外开 yaw ±26→∓45 / bend→5 / 挺胸 "
            "-6 / body.z +0.12），recovery 30→36 经 t33 中段帧收落回 guard。"
        ),
        end_tick=36,
        stop_tick=38,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
