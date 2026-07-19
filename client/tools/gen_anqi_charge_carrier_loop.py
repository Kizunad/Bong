#!/usr/bin/env python3
"""anqi_charge_carrier_loop —— 封骨充能：双手胸前结印灌注微循环（P2 批次二后半）。

cast_ticks=400（`CHARGE_DURATION_TICKS` = 真实 20s 引导通道），两段式蓄力段。
isLoop:true，周期 32t；release 段见 gen_anqi_charge_carrier_release.py。

母题：封骨结印。双手于胸前合拢结印，掌间夹着待充能的骨器，以呼吸节律把真元
一压一压封进骨中——提息上悬（胸微起）→灌压下沉（掌根下压、俯首）→底点双相
微振（真元脉冲，左右 roll 交替）→回升归位。全程低位小幅，与 windup_charge
（高举蓄力）/ 各 stance 站桩动向完全区分。

循环红线（§13 #5 / 库坑 #1）：所有用到的轴在 tick 0 与 endTick=32 同值补帧——
BASE 帧枚举全部轴，每个中间帧 inherit(BASE) 派生，首尾帧 = BASE 本体，机械保证
loopSeamViolations 为空。循环段不写 spec manifest（三段式结构不适用，见 plan
附录 A 决策注记）。

时序（32t 呼吸周期）：
  0→8   提息上悬：双臂微举（pitch -65→-74）、bend 收拢 +7、身体微浮 y -0.03→-0.015
  8→16  灌压下沉：掌根下压（pitch → -56）、bend 加深至 96、身体下沉 y -0.06、俯首 +13
  16→24 底点微振：roll 双相脉冲（右 +8→+14→+5、左 -10→-16→-6）、torso.yaw ∓3 交替
  24→32 回升归位：全轴回 BASE（endTick 同值闭环）
endTick=32，stopTick=34，isLoop=true。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 结印基位：双手胸前合拢（掌间夹骨器）、微俯首注视、沉桩。
# BASE 枚举本动画用到的全部 part.axis —— 循环闭环的机械保证。
BASE = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=-0.03, z=-0.02),
    head=dict(pitch=+10, yaw=0),
    torso=dict(pitch=+6, yaw=0),
    rightArm=dict(pitch=-65, yaw=-18, roll=+8, bend=88, axis=180),
    leftArm=dict(pitch=-62, yaw=+20, roll=-10, bend=92, axis=180),
    leftLeg=dict(pitch=-6, bend=8, z=-0.03),
    rightLeg=dict(pitch=+5, bend=7, z=+0.03),
)

POSE = {
    0: BASE,
    # 提息：双臂随吸气微举，身体微浮。
    4: inherit(
        BASE,
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.022, z=-0.02),
        head=dict(pitch=+8, yaw=0),
        torso=dict(pitch=+4, yaw=0),
        rightArm=dict(pitch=-71, yaw=-18, roll=+9, bend=92, axis=180),
        leftArm=dict(pitch=-68, yaw=+20, roll=-11, bend=96, axis=180),
    ),
    # 提息顶点：悬持一瞬（吸满）。
    8: inherit(
        BASE,
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.015, z=-0.018),
        head=dict(pitch=+7, yaw=0),
        torso=dict(pitch=+3, yaw=0),
        rightArm=dict(pitch=-74, yaw=-17, roll=+10, bend=95, axis=180),
        leftArm=dict(pitch=-71, yaw=+19, roll=-12, bend=99, axis=180),
    ),
    # 灌压：掌根下压封真元入骨，身体下沉、俯首。
    12: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.05, z=-0.025),
        head=dict(pitch=+12, yaw=0),
        torso=dict(pitch=+8, yaw=0),
        rightArm=dict(pitch=-59, yaw=-19, roll=+11, bend=94, axis=180),
        leftArm=dict(pitch=-56, yaw=+21, roll=-13, bend=98, axis=180),
    ),
    # 底点微振 A：真元脉冲外扩（roll 张、torso 左倾一分）。
    16: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.06, z=-0.028),
        head=dict(pitch=+13, yaw=-1),
        torso=dict(pitch=+9, yaw=-3),
        rightArm=dict(pitch=-56, yaw=-20, roll=+14, bend=93, axis=180),
        leftArm=dict(pitch=-53, yaw=+22, roll=-16, bend=97, axis=180),
    ),
    # 底点微振 B：脉冲回收（roll 合、torso 右摆）。
    20: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.055, z=-0.026),
        head=dict(pitch=+12.5, yaw=+1),
        torso=dict(pitch=+8.5, yaw=+3),
        rightArm=dict(pitch=-58, yaw=-18.5, roll=+5, bend=94, axis=180),
        leftArm=dict(pitch=-55, yaw=+20.5, roll=-6, bend=98, axis=180),
    ),
    # 回升：由底点缓缓抬回。
    24: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.042, z=-0.023),
        head=dict(pitch=+11, yaw=0),
        torso=dict(pitch=+7, yaw=-1),
        rightArm=dict(pitch=-61, yaw=-18, roll=+7, bend=91, axis=180),
        leftArm=dict(pitch=-58, yaw=+20, roll=-9, bend=95, axis=180),
    ),
    # 近位：几乎回到 BASE（收尾平滑）。
    28: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.034, z=-0.021),
        head=dict(pitch=+10.3, yaw=0),
        torso=dict(pitch=+6.3, yaw=0),
        rightArm=dict(pitch=-64, yaw=-18, roll=+8.3, bend=89, axis=180),
        leftArm=dict(pitch=-61, yaw=+20, roll=-10.3, bend=93, axis=180),
    ),
    # endTick = BASE 本体：每轴与 tick 0 同值闭环（库坑 #1 机械保证）。
    32: inherit(BASE),
}


def main() -> int:
    emit_json(
        POSE,
        name="anqi_charge_carrier_loop",
        description=(
            "P2 封骨充能蓄力段（isLoop 32t）：双手胸前结印灌注微循环——提息上悬"
            "（rightArm pitch -65→-74 / body.y -0.03→-0.015）→灌压下沉（pitch→-56 / "
            "bend→96 / y -0.06 / 俯首 +13）→底点 roll 双相微振（+14/-16 ↔ +5/-6，"
            "torso.yaw ∓3）→回升归位。全轴 0/32 同值闭环。"
        ),
        end_tick=32,
        stop_tick=34,
        is_loop=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
