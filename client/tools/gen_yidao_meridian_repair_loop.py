#!/usr/bin/env python3
"""yidao_meridian_repair_loop —— 接经术蓄力段：俯身双手持针逐点施针循环（P4）。

通道核验（P4 第一性原理，2026-07-19）：`resolve_yidao_skill`
（server/src/combat/yidao.rs）对 5 招统一 `insert_casting` 真实长引导窗——
meridian_repair cast_ticks_base=1200t（60s），运行时经 `yidao_cast_ticks` 按
mastery/平和色缩放（窗长可变）→ 长引导两段式：本蓄力段 isLoop 覆盖任意窗长，
release 段见 gen_yidao_meridian_repair_release.py。完成链 cast_emit.rs
`tick_casts_or_interrupt` 自然完成分支发 `YidaoCastCompleteEvent` →
`complete_yidao_casts` 有效结算接力 release；三打断分支表驱动 StopAnim
（`looping_cast_anim_id`，§13 #6 红线）。

母题（plan-yidao-v1 §5 ①）：针灸接经。医者俯身于患者上方（弯腰走 bow_salute
补偿先例：torso + / legs pitch 小负 + bend / body.z 前移），右手捏针悬于经脉
上方，左手指腹按循下一穴位引路——「30 穴位序」意象化为一个循环内针尖走过
四个落点（内→中→外→回），每个落点一次下针轻顿（rightArm pitch 落 8-10°
+ roll 捻针），左手始终先针一步探穴，头目随针点游走。与灸火对掌（contam
purge）/ CPR 按压（emergency）的双手对称动向区分：本段是「右针左探」的
不对称精细手上工作。

循环红线（§13 #5 / 库坑 #1）：BASE 帧枚举全部轴，中间帧 inherit(BASE) 派生，
首尾帧 = BASE 本体，机械保证每轴 0/28 同值闭环。

时序（28t 施针周期，四落点）：
  0   BASE：悬针位 A（内侧近柄），左手按 A 前方
  4   落针 A：右腕下顿 + 捻针 roll，左手移向 B
  7   提针移位 B：右臂随左手外移
  11  落针 B（中位）
  14  提针移位 C：至最外落点，身体随探微沉
  18  落针 C（外侧最深一顿）
  21  提针回撤 D（半程回位）
  25  落针 D（回程轻顿）
  28  = BASE（回到悬针位 A，闭环）
endTick=28，stopTick=30，isLoop=true。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 俯身持针基位：torso 前倾 + 腿部 bow 补偿，右手捏针悬于胸前下方，左手探穴。
BASE = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=-0.10, z=+0.07),
    head=dict(pitch=+16, yaw=-3),
    torso=dict(pitch=+22, yaw=-4),
    rightArm=dict(pitch=-38, yaw=-10, roll=-6, bend=52, axis=180),
    leftArm=dict(pitch=-44, yaw=+16, roll=+4, bend=38, axis=180),
    leftLeg=dict(pitch=-8, bend=16, z=-0.03),
    rightLeg=dict(pitch=-7, bend=15, z=+0.03),
)

POSE = {
    0: BASE,
    # 落针 A：右腕下顿 + 捻针（roll 翻转），左手先针一步移向 B。
    4: inherit(
        BASE,
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.115, z=+0.075),
        head=dict(pitch=+17.5, yaw=-4),
        rightArm=dict(pitch=-30, yaw=-9, roll=+7, bend=46, axis=180),
        leftArm=dict(pitch=-46, yaw=+10, roll=+5, bend=34, axis=180),
    ),
    # 提针移位 B：针随左手外移半步。
    7: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.105, z=+0.072),
        head=dict(pitch=+16.5, yaw=-6),
        rightArm=dict(pitch=-40, yaw=-13, roll=-6, bend=50, axis=180),
        leftArm=dict(pitch=-48, yaw=+6, roll=+6, bend=31, axis=180),
    ),
    # 落针 B（中位落点）。
    11: inherit(
        BASE,
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.12, z=+0.078),
        head=dict(pitch=+18, yaw=-7),
        torso=dict(pitch=+23, yaw=-6),
        rightArm=dict(pitch=-31, yaw=-14, roll=+8, bend=44, axis=180),
        leftArm=dict(pitch=-50, yaw=0, roll=+7, bend=28, axis=180),
    ),
    # 提针移位 C：至最外侧落点，身体随探更沉半分。
    14: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.11, z=+0.075),
        head=dict(pitch=+17, yaw=-10),
        torso=dict(pitch=+23, yaw=-7),
        rightArm=dict(pitch=-42, yaw=-18, roll=-7, bend=48, axis=180),
        leftArm=dict(pitch=-52, yaw=-5, roll=+8, bend=25, axis=180),
    ),
    # 落针 C：外侧最深一顿（本循环发力最重的一针）。
    18: inherit(
        BASE,
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.125, z=+0.08),
        head=dict(pitch=+18.5, yaw=-11),
        torso=dict(pitch=+24, yaw=-8),
        rightArm=dict(pitch=-29, yaw=-19, roll=+9, bend=42, axis=180),
        leftArm=dict(pitch=-53, yaw=-8, roll=+8, bend=24, axis=180),
    ),
    # 提针回撤 D：半程回位，左手引回。
    21: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.11, z=+0.074),
        head=dict(pitch=+17, yaw=-7),
        torso=dict(pitch=+23, yaw=-6),
        rightArm=dict(pitch=-40, yaw=-14, roll=-6, bend=49, axis=180),
        leftArm=dict(pitch=-49, yaw=+4, roll=+6, bend=31, axis=180),
    ),
    # 落针 D：回程轻顿（收循环的一针）。
    25: inherit(
        BASE,
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.112, z=+0.072),
        head=dict(pitch=+17, yaw=-5),
        rightArm=dict(pitch=-33, yaw=-11, roll=+6, bend=47, axis=180),
        leftArm=dict(pitch=-46, yaw=+11, roll=+5, bend=35, axis=180),
    ),
    # endTick = BASE 本体：每轴与 tick 0 同值闭环（库坑 #1 机械保证）。
    28: inherit(BASE),
}


def main() -> int:
    emit_json(
        POSE,
        name="yidao_meridian_repair_loop",
        description=(
            "P4 接经术蓄力段（isLoop 28t）：俯身持针四落点施针循环——右手捏针"
            "逐穴下顿捻针（pitch -38↔-29 / roll -6↔+9），左手先针一步探穴引路"
            "（yaw +16→-8→+16），头目随针点游走，bow 补偿俯身。全轴 0/28 同值"
            "闭环。release 段见 yidao_meridian_repair_release。"
        ),
        end_tick=28,
        stop_tick=30,
        is_loop=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
