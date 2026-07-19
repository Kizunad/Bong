#!/usr/bin/env python3
"""sword_infuse —— 注剑灌注：横剑抚刃循环蓄力段（P2 批次二后半重制）。

cast_ticks=40，真实引导窗（`cast_sword_infuse` 插 `Casting` + `PendingSwordInfuse`
两段式通道）。旧 sword_infuse.json 为 40t 一次性快闪（29 moves 无 recovery），
本批重制为两段式**蓄力段**：isLoop 28t（id 沿用 `sword_infuse`——v1 资产清单
pin 该文件名，语义改为循环蓄力段）；release 段见 gen_sword_infuse_release.py。
打断路径：cast_emit 三打断分支表驱动 StopAnim（§8.1 #3）。

母题：横剑抚刃。右手举剑横持于胸前（刃身横陈），左掌自剑柄沿刃身向剑尖
缓缓推抚灌注真元（去程 0→14），至剑尖后收掌回柄（回程 14→28），随呼吸
微沉浮，头部目光随左掌走刃。全程低位持守，与所有挥斩类剑招动向区分。

循环红线（§13 #5 / 库坑 #1）：BASE 帧枚举全部轴，中间帧 inherit(BASE) 派生，
首尾帧 = BASE 本体，机械保证每轴 0/28 同值闭环。循环段不写 spec manifest。
不用 head/torso.roll（v1 资产清单边界复位断言域）。

时序（28t 抚刃周期）：
  0→14  推抚去程：左掌沿刃身向外推（yaw +30→-8、pitch -65→-80、bend 55→22），
        身体随呼吸下沉 y -0.02→-0.045，头目随掌 yaw +4→-7
  14→28 收掌回程：左掌沿刃收回剑柄（对称回落），身体回浮，目光随回
endTick=28，stopTick=30，isLoop=true。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 横剑持守基位：右手举剑横持胸前、左掌搭于剑柄处。
# BASE 枚举本动画用到的全部 part.axis —— 循环闭环的机械保证。
BASE = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=-0.02, z=-0.015),
    head=dict(pitch=+9, yaw=+4),
    torso=dict(pitch=+5, yaw=-6),
    rightArm=dict(pitch=-72, yaw=-22, roll=-12, bend=30, axis=180),
    leftArm=dict(pitch=-65, yaw=+30, roll=-5, bend=55, axis=180),
    leftLeg=dict(pitch=-7, bend=9, z=-0.03),
    rightLeg=dict(pitch=+6, bend=8, z=+0.03),
)

POSE = {
    0: BASE,
    # 推抚启动：左掌离柄、沿刃身外推一分。
    4: inherit(
        BASE,
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.028, z=-0.017),
        head=dict(pitch=+9.5, yaw=+1),
        torso=dict(pitch=+5.5, yaw=-5),
        rightArm=dict(pitch=-72.5, yaw=-22, roll=-13, bend=30, axis=180),
        leftArm=dict(pitch=-70, yaw=+18, roll=-6, bend=44, axis=180),
    ),
    # 推抚中段：掌至刃身中部，身体渐沉。
    7: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.036, z=-0.019),
        head=dict(pitch=+10, yaw=-2),
        torso=dict(pitch=+6, yaw=-4),
        rightArm=dict(pitch=-73, yaw=-21.5, roll=-14, bend=29, axis=180),
        leftArm=dict(pitch=-74, yaw=+8, roll=-7, bend=35, axis=180),
    ),
    # 推抚远段：掌近剑尖、臂近展直。
    10: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.042, z=-0.021),
        head=dict(pitch=+10.5, yaw=-5),
        torso=dict(pitch=+6.5, yaw=-3),
        rightArm=dict(pitch=-73.5, yaw=-21, roll=-15, bend=28, axis=180),
        leftArm=dict(pitch=-78, yaw=-2, roll=-8, bend=26, axis=180),
    ),
    # 去程顶点：掌至剑尖（灌注最远点），身体最沉、目光在剑尖。
    14: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.045, z=-0.022),
        head=dict(pitch=+11, yaw=-7),
        torso=dict(pitch=+7, yaw=-2),
        rightArm=dict(pitch=-74, yaw=-21, roll=-16, bend=27, axis=180),
        leftArm=dict(pitch=-80, yaw=-8, roll=-9, bend=22, axis=180),
    ),
    # 回程启动：掌离尖回撤。
    18: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.04, z=-0.02),
        head=dict(pitch=+10.5, yaw=-4),
        torso=dict(pitch=+6.5, yaw=-3),
        rightArm=dict(pitch=-73.5, yaw=-21.5, roll=-15, bend=28, axis=180),
        leftArm=dict(pitch=-76, yaw=+2, roll=-8, bend=32, axis=180),
    ),
    # 回程中段：掌回刃身中部，身体回浮。
    21: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.033, z=-0.018),
        head=dict(pitch=+10, yaw=-1),
        torso=dict(pitch=+6, yaw=-4),
        rightArm=dict(pitch=-73, yaw=-22, roll=-14, bend=29, axis=180),
        leftArm=dict(pitch=-72, yaw=+12, roll=-7, bend=42, axis=180),
    ),
    # 回程近柄：掌将落回剑柄。
    24: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.026, z=-0.016),
        head=dict(pitch=+9.5, yaw=+2),
        torso=dict(pitch=+5.5, yaw=-5),
        rightArm=dict(pitch=-72.5, yaw=-22, roll=-13, bend=30, axis=180),
        leftArm=dict(pitch=-68, yaw=+22, roll=-6, bend=50, axis=180),
    ),
    # endTick = BASE 本体：每轴与 tick 0 同值闭环（库坑 #1 机械保证）。
    28: inherit(BASE),
}


def main() -> int:
    emit_json(
        POSE,
        name="sword_infuse",
        description=(
            "P2 注剑灌注蓄力段（isLoop 28t，两段式重制）：横剑持守，左掌沿刃身"
            "柄→尖→柄往复推抚灌注（leftArm yaw +30→-8→+30 / pitch -65→-80→-65 / "
            "bend 55→22→55），身体呼吸沉浮 y -0.02→-0.045→-0.02，头目随掌走刃。"
            "全轴 0/28 同值闭环。release 段见 sword_infuse_release。"
        ),
        end_tick=28,
        stop_tick=30,
        is_loop=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
