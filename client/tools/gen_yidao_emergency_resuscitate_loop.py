#!/usr/bin/env python3
"""yidao_emergency_resuscitate_loop —— 急救蓄力段：CPR 双手叠压节律循环（P4）。

通道核验（P4 第一性原理，2026-07-19）：`resolve_yidao_skill`
（server/src/combat/yidao.rs）`insert_casting` 真实引导窗——
emergency_resuscitate cast_ticks_base=100t（5s，NearDeath 窗内急救），经
`yidao_cast_ticks` 缩放（窗长可变）→ 蓄力段 isLoop 覆盖任意窗长；release
段见 gen_yidao_emergency_resuscitate_release.py。停止路径 = cast_emit 三打断
分支 + 自然完成分支表驱动 StopAnim（§13 #6）。

母题（plan-yidao-v1 §5 ③）：CPR 胸外按压。医者深俯于倒地患者上方（深弯腰
走 bow_salute 补偿：torso 大 + / legs pitch 小负 + bend 深 / body.z 前移
body.y 深沉），双臂伸直叠掌（右下左上，yaw 内收叠于中线），以躯干起伏带动
垂直按压——一个循环两次按压且深浅不一（「按压深度起伏」：第一压深、第二压
稍浅），压间有一拍短喘。与灸火推送的水平对称推拉、接经的精细针工区分：
本段是全身重量灌注的垂直节律按压。

循环红线（§13 #5 / 库坑 #1）：BASE 帧枚举全部轴，中间帧 inherit(BASE) 派生，
首尾帧 = BASE 本体，机械保证每轴 0/20 同值闭环。

时序（20t 双按压周期，~2 压/秒对齐 CPR 节律）：
  0   BASE：撑顶位（臂直叠掌、肘锁死）
  3   第一压到底（最深：body.y -0.40 / torso +30）
  6   回弹撑顶
  10  短喘：微抬肩换气（比撑顶再松半分）
  13  第二压到底（稍浅：body.y -0.375，深度起伏）
  16  回弹撑顶
  20  = BASE（闭环）
endTick=20，stopTick=22，isLoop=true。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 深俯撑顶基位：双臂伸直叠掌于中线（右下左上），躯干深前倾。
BASE = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=-0.30, z=+0.10),
    head=dict(pitch=+18, yaw=0),
    torso=dict(pitch=+26, yaw=0),
    rightArm=dict(pitch=-46, yaw=-13, roll=-4, bend=5, axis=180),
    leftArm=dict(pitch=-49, yaw=+13, roll=+4, bend=4, axis=180),
    leftLeg=dict(pitch=-12, bend=26, z=-0.03),
    rightLeg=dict(pitch=-11, bend=25, z=+0.03),
)

POSE = {
    0: BASE,
    # 第一压到底：最深一压，躯干重量全灌（body.y 最低、torso 最前）。
    3: inherit(
        BASE,
        easing="INQUAD",
        body=dict(x=0.0, y=-0.40, z=+0.12),
        head=dict(pitch=+21, yaw=0),
        torso=dict(pitch=+30, yaw=0),
        rightArm=dict(pitch=-40, yaw=-13, roll=-4, bend=3, axis=180),
        leftArm=dict(pitch=-43, yaw=+13, roll=+4, bend=2, axis=180),
        leftLeg=dict(pitch=-13, bend=29, z=-0.03),
        rightLeg=dict(pitch=-12, bend=28, z=+0.03),
    ),
    # 回弹撑顶（让胸廓回弹，臂不离位）。
    6: inherit(
        BASE,
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.295, z=+0.10),
        head=dict(pitch=+17.5, yaw=0),
    ),
    # 短喘：微抬肩换气，头稍抬看患者面色。
    10: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.28, z=+0.095),
        head=dict(pitch=+14, yaw=+3),
        torso=dict(pitch=+24, yaw=+1),
        rightArm=dict(pitch=-48, yaw=-13, roll=-4, bend=6, axis=180),
        leftArm=dict(pitch=-51, yaw=+13, roll=+4, bend=5, axis=180),
    ),
    # 第二压到底：稍浅（深度起伏），头回正盯压点。
    13: inherit(
        BASE,
        easing="INQUAD",
        body=dict(x=0.0, y=-0.375, z=+0.115),
        head=dict(pitch=+20, yaw=0),
        torso=dict(pitch=+29, yaw=0),
        rightArm=dict(pitch=-41, yaw=-13, roll=-4, bend=3, axis=180),
        leftArm=dict(pitch=-44, yaw=+13, roll=+4, bend=2, axis=180),
        leftLeg=dict(pitch=-13, bend=28, z=-0.03),
        rightLeg=dict(pitch=-12, bend=27, z=+0.03),
    ),
    # 回弹撑顶。
    16: inherit(
        BASE,
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.298, z=+0.10),
        head=dict(pitch=+18.5, yaw=0),
    ),
    # endTick = BASE 本体：每轴与 tick 0 同值闭环（库坑 #1 机械保证）。
    20: inherit(BASE),
}


def main() -> int:
    emit_json(
        POSE,
        name="yidao_emergency_resuscitate_loop",
        description=(
            "P4 急救蓄力段（isLoop 20t）：深俯双臂伸直叠掌 CPR 垂直按压——"
            "一循环两压深浅起伏（body.y -0.30→-0.40→-0.375）+ 压间短喘抬头，"
            "躯干起伏带动（torso.pitch +24↔+30，bow 补偿）。全轴 0/20 同值"
            "闭环。release 段见 yidao_emergency_resuscitate_release。"
        ),
        end_tick=20,
        stop_tick=22,
        is_loop=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
