#!/usr/bin/env python3
"""yidao_emergency_resuscitate_release —— 急救复苏俯听收势（P4）。

两段式 release 段（蓄力段见 gen_yidao_emergency_resuscitate_loop.py，
isLoop）。引导自然完成且结算有效（患者拉回 NearDeath 窗）时由 server
StopAnim(蓄力段)+PlayAnim(本段) 接力；打断/无效完成只 StopAnim。

母题：复苏俯听。最后一压压定，双手离胸，医者侧头贴向患者胸口听心息
（俯听：torso 再前压 + 头侧转贴胸），听得一息复跳，撑膝直起长出一口气。
「压定→侧耳俯听→直身舒气」的三拍与其余收势全部区分（唯一以头部侧转
俯贴为顶点的收势）。

时序（精度标准 #1/#2/#3）：
  anticipation 0→3   末压压定：最深一压定住（INQUAD 蓄）
  strike       3→6   俯听：双手离胸让位、头侧转贴向患者胸口
                     （head.yaw +26 / torso +34 最俯点），t6 = 俯听顶点
  recovery     6→12  撑膝直起长出气归中立（INOUTSINE，t9 中段帧）
endTick=12，stopTick=14，非循环。主打击轴：torso.pitch / head.yaw / body.y。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 承接蓄力段撑顶位。
    0: dict(
        easing="INQUAD",
        body=dict(y=-0.30, z=+0.10),
        head=dict(pitch=+18, yaw=0),
        torso=dict(pitch=+26, yaw=0),
        rightArm=dict(pitch=-46, yaw=-13, roll=-4, bend=5, axis=180),
        leftArm=dict(pitch=-49, yaw=+13, roll=+4, bend=4, axis=180),
        leftLeg=dict(pitch=-12, bend=26, z=-0.03),
        rightLeg=dict(pitch=-11, bend=25, z=+0.03),
    ),
    # 末压压定：最深一压定住。
    3: dict(
        easing="INQUAD",
        body=dict(y=-0.41, z=+0.12),
        head=dict(pitch=+21, yaw=0),
        torso=dict(pitch=+31, yaw=0),
        rightArm=dict(pitch=-40, yaw=-13, roll=-4, bend=3, axis=180),
        leftArm=dict(pitch=-43, yaw=+13, roll=+4, bend=2, axis=180),
        leftLeg=dict(pitch=-13, bend=29, z=-0.03),
        rightLeg=dict(pitch=-12, bend=28, z=+0.03),
    ),
    # 俯听中段：双手离胸让位、头开始侧转。
    5: dict(
        easing="OUTQUAD",
        body=dict(y=-0.42, z=+0.13),
        head=dict(pitch=+16, yaw=+18),
        torso=dict(pitch=+33, yaw=+3),
        rightArm=dict(pitch=-20, yaw=-20, roll=-6, bend=12, axis=180),
        leftArm=dict(pitch=-24, yaw=+22, roll=+6, bend=10, axis=180),
        leftLeg=dict(pitch=-13, bend=29, z=-0.03),
        rightLeg=dict(pitch=-12, bend=28, z=+0.03),
    ),
    # 俯听顶点：侧耳贴胸听心息。
    6: dict(
        easing="OUTSINE",
        body=dict(y=-0.43, z=+0.135),
        head=dict(pitch=+14, yaw=+26),
        torso=dict(pitch=+34, yaw=+4),
        rightArm=dict(pitch=-14, yaw=-22, roll=-6, bend=14, axis=180),
        leftArm=dict(pitch=-18, yaw=+24, roll=+6, bend=12, axis=180),
        leftLeg=dict(pitch=-13, bend=30, z=-0.03),
        rightLeg=dict(pitch=-12, bend=29, z=+0.03),
    ),
    # 直起中段：撑膝起身、头回正。
    9: dict(
        easing="INOUTSINE",
        body=dict(y=-0.14, z=+0.05),
        head=dict(pitch=+6, yaw=+8),
        torso=dict(pitch=+14, yaw=+2),
        rightArm=dict(pitch=-10, yaw=-12, roll=-3, bend=20, axis=180),
        leftArm=dict(pitch=-12, yaw=+12, roll=+3, bend=18, axis=180),
        leftLeg=dict(pitch=-7, bend=14, z=-0.02),
        rightLeg=dict(pitch=-6, bend=13, z=+0.02),
    ),
    # 归中立（长出一口气收尾）。
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
        name="yidao_emergency_resuscitate_release",
        description=(
            "P4 急救复苏俯听收势（12t 非循环）：anticipation 0→3 末压压定"
            "（body.y -0.41），strike 3→6 双手离胸侧耳俯听（head.yaw 0→+26 / "
            "torso +31→+34），recovery 6→12 撑膝直起归中立。"
        ),
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
