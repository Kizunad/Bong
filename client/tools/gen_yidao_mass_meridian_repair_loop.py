#!/usr/bin/env python3
"""yidao_mass_meridian_repair_loop —— 群体接经蓄力段：捧法器高举环阵环视循环（P4）。

通道核验（P4 第一性原理，2026-07-19）：`resolve_yidao_skill`
（server/src/combat/yidao.rs）`insert_casting` 真实引导窗——
mass_meridian_repair cast_ticks_base=1200t（60s，化虚境群疗），经
`yidao_cast_ticks` 缩放（窗长可变）→ 蓄力段 isLoop 覆盖任意窗长；release
段见 gen_yidao_mass_meridian_repair_release.py。停止路径 = cast_emit 三打断
分支 + 自然完成分支表驱动 StopAnim（§13 #6）。

母题（plan-yidao-v1 §5 ⑤）：环阵共振。医者立于患者环阵中心，双手捧化虚
平和色法器高举过顶，随共振嗡鸣缓缓左右环视扫过每一名患者（torso.yaw 大幅
横扫 + head.yaw 领先半拍），法器随扫势轻微倾摆（双臂 roll 同步），身体随
共振低频沉浮。与续命术「一手天一手人」区分：本段双手同举对称；与接经/CPR
的俯身区分：本段直立昂扬、动向在横轴（yaw 扫）而非纵轴。

循环红线（§13 #5 / 库坑 #1）：BASE 帧枚举全部轴，中间帧 inherit(BASE) 派生，
首尾帧 = BASE 本体，机械保证每轴 0/32 同值闭环。

时序（32t 环视扫描周期：左→中→右→中→左）：
  0   BASE：环视左极（torso.yaw -16，头领先 -22）
  4   左中途：向中扫，法器随摆
  8   正中：面向正前，法器最正，身体共振沉底
  12  右中途：继续向右扫
  16  右极：torso.yaw +16（镜像左极），头 +22
  20  回扫右中途
  24  回正中（第二次沉底）
  28  回左中途
  32  = BASE（回左极，闭环）
endTick=32，stopTick=34，isLoop=true。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 高举法器基位（环视左极）：双臂对称高举过顶捧器，直立微沉。
BASE = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=-0.02, z=0.0),
    head=dict(pitch=-8, yaw=-22),
    torso=dict(pitch=-3, yaw=-16),
    rightArm=dict(pitch=-148, yaw=-13, roll=-8, bend=12, axis=180),
    leftArm=dict(pitch=-148, yaw=+13, roll=+8, bend=12, axis=180),
    leftLeg=dict(pitch=-4, bend=7, z=-0.03),
    rightLeg=dict(pitch=+4, bend=6, z=+0.03),
)

POSE = {
    0: BASE,
    # 左中途：向中扫，法器随摆（roll 同步倾）。
    4: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.028, z=0.0),
        head=dict(pitch=-8, yaw=-11),
        torso=dict(pitch=-3, yaw=-8),
        rightArm=dict(pitch=-149, yaw=-13, roll=-4, bend=12, axis=180),
        leftArm=dict(pitch=-149, yaw=+13, roll=+12, bend=12, axis=180),
    ),
    # 正中：面向正前，法器最正，共振沉底。
    8: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.045, z=0.0),
        head=dict(pitch=-6, yaw=0),
        torso=dict(pitch=-2, yaw=0),
        rightArm=dict(pitch=-151, yaw=-13, roll=0, bend=11, axis=180),
        leftArm=dict(pitch=-151, yaw=+13, roll=0, bend=11, axis=180),
        leftLeg=dict(pitch=-5, bend=9, z=-0.03),
        rightLeg=dict(pitch=+5, bend=8, z=+0.03),
    ),
    # 右中途：继续向右扫。
    12: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.028, z=0.0),
        head=dict(pitch=-8, yaw=+11),
        torso=dict(pitch=-3, yaw=+8),
        rightArm=dict(pitch=-149, yaw=-13, roll=+4, bend=12, axis=180),
        leftArm=dict(pitch=-149, yaw=+13, roll=+4, bend=12, axis=180),
    ),
    # 右极：镜像左极。
    16: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.02, z=0.0),
        head=dict(pitch=-8, yaw=+22),
        torso=dict(pitch=-3, yaw=+16),
        rightArm=dict(pitch=-148, yaw=-13, roll=+8, bend=12, axis=180),
        leftArm=dict(pitch=-148, yaw=+13, roll=-8, bend=12, axis=180),
    ),
    # 回扫右中途。
    20: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.028, z=0.0),
        head=dict(pitch=-8, yaw=+11),
        torso=dict(pitch=-3, yaw=+8),
        rightArm=dict(pitch=-149, yaw=-13, roll=+4, bend=12, axis=180),
        leftArm=dict(pitch=-149, yaw=+13, roll=+4, bend=12, axis=180),
    ),
    # 回正中：第二次共振沉底。
    24: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.045, z=0.0),
        head=dict(pitch=-6, yaw=0),
        torso=dict(pitch=-2, yaw=0),
        rightArm=dict(pitch=-151, yaw=-13, roll=0, bend=11, axis=180),
        leftArm=dict(pitch=-151, yaw=+13, roll=0, bend=11, axis=180),
        leftLeg=dict(pitch=-5, bend=9, z=-0.03),
        rightLeg=dict(pitch=+5, bend=8, z=+0.03),
    ),
    # 回左中途。
    28: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.028, z=0.0),
        head=dict(pitch=-8, yaw=-11),
        torso=dict(pitch=-3, yaw=-8),
        rightArm=dict(pitch=-149, yaw=-13, roll=-4, bend=12, axis=180),
        leftArm=dict(pitch=-149, yaw=+13, roll=+12, bend=12, axis=180),
    ),
    # endTick = BASE 本体：每轴与 tick 0 同值闭环（库坑 #1 机械保证）。
    32: inherit(BASE),
}


def main() -> int:
    emit_json(
        POSE,
        name="yidao_mass_meridian_repair_loop",
        description=(
            "P4 群体接经蓄力段（isLoop 32t）：双手捧法器高举过顶（双臂 -148"
            "↔-151 对称），torso.yaw -16↔+16 环阵环视横扫（head.yaw ±22 领先），"
            "法器随扫 roll 倾摆，共振低频沉浮。全轴 0/32 同值闭环。release 段见"
            " yidao_mass_meridian_repair_release。"
        ),
        end_tick=32,
        stop_tick=34,
        is_loop=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
