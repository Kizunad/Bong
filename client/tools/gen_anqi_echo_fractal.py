#!/usr/bin/env python3
"""anqi_echo_fractal —— 诱饵分形：织网撒饵结印→爆发撒出（P2 批次二后半）。

cast_ticks=60，**瞬发结算型长 cast**（`resolve_anqi_skill` 立即结算、无引导窗，
plan 附录 A 决策 (b)）：cast_ticks 是元数据非真实引导窗，无法做「循环蓄力段+
StopAnim」两段式（isLoop 无停止信号会永久循环）。改做 66t 非循环单段专属长
演出——0→60 覆盖 cast 元数据期，顶点 = tick 60，recovery 60→66。时长对拍
allowlist 条目保留（cast≥40 机械断言要求 isLoop，通道真实化后再落两段式）。

母题：织网撒饵。双手在身前交替织画分形轨迹（左右手反相「编网」，幅度随
灌注渐涨），织满后双手收拢一压（聚饵），随即双臂大开扇向外上方撒出诱饵
（爆发帧），身体微仰目送。与齐射（拢镖开扇平撒）区分点：织网期的持续交替
编织 + 撒出方向为外上方仰撒。

时序（精度标准 #1/#2/#3）：
  anticipation 0→52  织网（4t 步进反相摆动，幅度 ramp 0.55→1.2）：
                     rightArm pitch -70±12·r / yaw -12±15·r，leftArm 反相，
                     torso.yaw ±6·r 随手，head.yaw 目随主织手
  strike       52→60 聚饵一压（52→55 双手拢胸 bend 90+ / 身体下沉）→爆发仰撒
                     （58→60 双臂大开扇外上方 pitch -118/-112、yaw ±48、仰身 -8）
  recovery     60→66 由仰撒位落回中立（INOUTSINE，t63 中段帧）
endTick=66，stopTick=68，非循环。主打击轴：rightArm.pitch / rightArm.yaw /
leftArm.pitch / torso.yaw（全程 ≤4t 帧距，织网步进本身即密度保证）。
"""

from __future__ import annotations

import math

from anim_common import emit_json


def weave_frame(t: int) -> dict:
    """织网期（0→48）某 tick 的反相编织姿态。

    左右手以 16t 为周期反相摆动（右手领、左手随），幅度随 t 线性渐涨
    （0.55→1.2），躯干/头部小幅跟随。所有轴每帧齐全（密度红线）。
    """
    ramp = 0.55 + (1.2 - 0.55) * (t / 48.0)
    ph = 2.0 * math.pi * (t % 16) / 16.0
    s = math.sin(ph)
    c = math.cos(ph)
    return dict(
        easing="INOUTSINE",
        body=dict(x=+0.02 * s * ramp, y=-0.02 - 0.012 * ramp * (0.5 + 0.5 * c), z=-0.02),
        head=dict(pitch=+6 + 2 * c * ramp, yaw=+5 * s * ramp),
        torso=dict(pitch=+4 + 1.5 * c * ramp, yaw=+6 * s * ramp),
        rightArm=dict(
            pitch=-70 - 12 * s * ramp,
            yaw=-12 - 15 * s * ramp,
            roll=+8 + 6 * c * ramp,
            bend=62 + 14 * c * ramp,
            axis=180,
        ),
        leftArm=dict(
            pitch=-66 + 12 * s * ramp,
            yaw=+14 - 15 * s * ramp,
            roll=-8 - 6 * c * ramp,
            bend=66 - 14 * c * ramp,
            axis=180,
        ),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03),
    )


# 织网期：0→48 每 4t 一帧（主轴密度 ≤4t 机械保证）。
POSE = {t: weave_frame(t) for t in range(0, 49, 4)}

POSE.update(
    {
        # 聚饵：双手拢至胸前收网。
        52: dict(
            easing="OUTQUAD",
            body=dict(x=0.0, y=-0.05, z=-0.035),
            head=dict(pitch=+12, yaw=0),
            torso=dict(pitch=+9, yaw=-2),
            rightArm=dict(pitch=-78, yaw=-20, roll=+10, bend=90, axis=180),
            leftArm=dict(pitch=-74, yaw=+22, roll=-10, bend=94, axis=180),
            leftLeg=dict(pitch=-9, bend=12, z=-0.04),
            rightLeg=dict(pitch=+8, bend=11, z=+0.04),
        ),
        # 压饵蓄爆：再沉一分、拢得更紧（爆发前最后压缩）。
        55: dict(
            easing="OUTQUAD",
            body=dict(x=0.0, y=-0.08, z=-0.045),
            head=dict(pitch=+15, yaw=0),
            torso=dict(pitch=+12, yaw=-3),
            rightArm=dict(pitch=-84, yaw=-24, roll=+12, bend=98, axis=180),
            leftArm=dict(pitch=-80, yaw=+26, roll=-12, bend=102, axis=180),
            leftLeg=dict(pitch=-11, bend=15, z=-0.05),
            rightLeg=dict(pitch=+10, bend=14, z=+0.05),
        ),
        # 爆发启动：双臂向外上方掰开、身体开始起立后仰。
        58: dict(
            easing="INQUAD",
            body=dict(x=0.0, y=-0.02, z=-0.02),
            head=dict(pitch=-4, yaw=0),
            torso=dict(pitch=-2, yaw=+2),
            rightArm=dict(pitch=-100, yaw=-34, roll=+16, bend=40, axis=180),
            leftArm=dict(pitch=-96, yaw=+36, roll=-16, bend=44, axis=180),
            leftLeg=dict(pitch=-8, bend=10, z=-0.04),
            rightLeg=dict(pitch=+7, bend=9, z=+0.04),
        ),
        # 仰撒顶点 = cast 完成瞬间（tick 60）：双臂大开扇外上方撒出、仰身目送。
        60: dict(
            easing="INQUAD",
            body=dict(x=0.0, y=+0.02, z=-0.05),
            head=dict(pitch=-11, yaw=0),
            torso=dict(pitch=-8, yaw=+4),
            rightArm=dict(pitch=-118, yaw=-48, roll=+22, bend=8, axis=180),
            leftArm=dict(pitch=-112, yaw=+50, roll=-22, bend=10, axis=180),
            leftLeg=dict(pitch=-12, bend=14, z=-0.05),
            rightLeg=dict(pitch=+14, bend=12, z=+0.06),
        ),
        # 收势中段：落臂直身。
        63: dict(
            easing="INOUTSINE",
            body=dict(x=0.0, y=0.0, z=-0.02),
            head=dict(pitch=-4, yaw=0),
            torso=dict(pitch=-3, yaw=+1),
            rightArm=dict(pitch=-40, yaw=-18, roll=+8, bend=14, axis=180),
            leftArm=dict(pitch=-36, yaw=+20, roll=-8, bend=16, axis=180),
            leftLeg=dict(pitch=-5, bend=6, z=-0.02),
            rightLeg=dict(pitch=+5, bend=5, z=+0.02),
        ),
        # 归中立。
        66: dict(
            easing="INOUTSINE",
            body=dict(x=0.0, y=0.0, z=0.0),
            head=dict(pitch=0, yaw=0),
            torso=dict(pitch=0, yaw=0),
            rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
            leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
            leftLeg=dict(pitch=0, bend=0, z=0.0),
            rightLeg=dict(pitch=0, bend=0, z=0.0),
        ),
    }
)


def main() -> int:
    emit_json(
        POSE,
        name="anqi_echo_fractal",
        description=(
            "P2 诱饵分形专属（66t 非循环长演出，瞬发结算型长 cast 决策 (b)）："
            "anticipation 0→52 织网撒饵（左右手 16t 反相编织，幅度 ramp 0.55→1.2），"
            "strike 52→60 聚饵一压（bend→98/102 / body.y -0.08）→爆发仰撒（pitch "
            "-118/-112 / yaw ±48+ / 仰身 -8），recovery 60→66 落回中立。"
        ),
        end_tick=66,
        stop_tick=68,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
