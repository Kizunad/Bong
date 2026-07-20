#!/usr/bin/env python3
"""yidao_life_extension_loop —— 续命术蓄力段：左手喂丹+右手对天接引定格循环（P4）。

通道核验（P4 第一性原理，2026-07-19）：`resolve_yidao_skill`
（server/src/combat/yidao.rs）`insert_casting` 真实引导窗——life_extension
cast_ticks_base=600t（30s 咏唱，NearDeath 窗内他救），经 `yidao_cast_ticks`
缩放（窗长可变）→ 蓄力段 isLoop 覆盖任意窗长；release 段见
gen_yidao_life_extension_release.py。停止路径 = cast_emit 三打断分支 + 自然
完成分支表驱动 StopAnim（§13 #6）。

母题（plan-yidao-v1 §5 ④）：续命咏唱。左手托续命丹低探向患者口边喂入，
右臂高举对天接引业力（天人两线同时拉着）——「定格循环」：大形定住不走位，
只有接引手随咏唱脉动微沉浮/捻诀（roll 往复）、托丹手随患者气息微调、头目
在天与患者之间往复（仰望→俯看）。躯干微后仰（对天）与所有俯身治疗招区分：
本段是唯一「一手向天一手向人」的纵向拉开构图。

循环红线（§13 #5 / 库坑 #1）：BASE 帧枚举全部轴，中间帧 inherit(BASE) 派生，
首尾帧 = BASE 本体，机械保证每轴 0/26 同值闭环。

时序（26t 咏唱脉动周期）：
  0   BASE：定格位（左手托丹前探低位，右臂高举向天，头仰望）
  4   接引脉动 I：右臂再抬半分、捻诀 roll 外翻，业力下引
  8   目光下移：头由仰转俯看患者，托丹手随气息微送
  12  喂丹轻送：左手向患者口边送半寸（pitch 再低），右臂微沉
  16  目光回仰：头重新望天，接引手回举
  20  接引脉动 II：捻诀 roll 内合，躯干随咏唱微幅后仰加深
  23  回落：向定格位回归途中
  26  = BASE（闭环）
endTick=26，stopTick=28，isLoop=true。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 定格基位：左手托丹前探低位、右臂高举对天，躯干微后仰，头仰望。
BASE = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=-0.02, z=-0.01),
    head=dict(pitch=-14, yaw=+4),
    torso=dict(pitch=-5, yaw=+3),
    rightArm=dict(pitch=-152, yaw=-8, roll=-6, bend=14, axis=180),
    leftArm=dict(pitch=-34, yaw=+12, roll=+6, bend=26, axis=180),
    leftLeg=dict(pitch=-5, bend=8, z=-0.03),
    rightLeg=dict(pitch=+6, bend=7, z=+0.03),
)

POSE = {
    0: BASE,
    # 接引脉动 I：右臂再抬、捻诀外翻。
    4: inherit(
        BASE,
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.013, z=-0.012),
        head=dict(pitch=-16, yaw=+5),
        rightArm=dict(pitch=-158, yaw=-10, roll=+7, bend=11, axis=180),
        leftArm=dict(pitch=-35, yaw=+12, roll=+6, bend=27, axis=180),
    ),
    # 目光下移：仰转俯看患者，托丹手微送。
    8: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.025, z=0.0),
        head=dict(pitch=+6, yaw=+7),
        torso=dict(pitch=-3, yaw=+4),
        rightArm=dict(pitch=-154, yaw=-9, roll=+2, bend=13, axis=180),
        leftArm=dict(pitch=-37, yaw=+14, roll=+7, bend=22, axis=180),
    ),
    # 喂丹轻送：左手向患者口边送半寸，右臂微沉。
    12: inherit(
        BASE,
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.035, z=+0.02),
        head=dict(pitch=+10, yaw=+8),
        torso=dict(pitch=-1, yaw=+5),
        rightArm=dict(pitch=-147, yaw=-7, roll=-2, bend=16, axis=180),
        leftArm=dict(pitch=-41, yaw=+16, roll=+8, bend=16, axis=180),
    ),
    # 目光回仰：头重新望天，接引手回举。
    16: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.025, z=0.0),
        head=dict(pitch=-8, yaw=+5),
        torso=dict(pitch=-4, yaw=+4),
        rightArm=dict(pitch=-153, yaw=-9, roll=-4, bend=13, axis=180),
        leftArm=dict(pitch=-37, yaw=+13, roll=+7, bend=23, axis=180),
    ),
    # 接引脉动 II：捻诀内合，后仰微幅加深。
    20: inherit(
        BASE,
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.016, z=-0.018),
        head=dict(pitch=-17, yaw=+3),
        torso=dict(pitch=-7, yaw=+2),
        rightArm=dict(pitch=-156, yaw=-6, roll=-12, bend=12, axis=180),
        leftArm=dict(pitch=-33, yaw=+11, roll=+5, bend=28, axis=180),
    ),
    # 回落：向定格位回归途中。
    23: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.018, z=-0.014),
        head=dict(pitch=-15, yaw=+4),
        torso=dict(pitch=-6, yaw=+3),
        rightArm=dict(pitch=-154, yaw=-7, roll=-9, bend=13, axis=180),
    ),
    # endTick = BASE 本体：每轴与 tick 0 同值闭环（库坑 #1 机械保证）。
    26: inherit(BASE),
}


def main() -> int:
    emit_json(
        POSE,
        name="yidao_life_extension_loop",
        description=(
            "P4 续命术蓄力段（isLoop 26t）：左手托丹低探喂丹、右臂高举对天"
            "接引（pitch -147↔-158 / roll -12↔+7 捻诀脉动），头目天人往复"
            "（pitch -17↔+10），躯干微后仰。全轴 0/26 同值闭环。release 段见"
            " yidao_life_extension_release。"
        ),
        end_tick=26,
        stop_tick=28,
        is_loop=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
