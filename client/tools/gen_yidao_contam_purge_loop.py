#!/usr/bin/env python3
"""yidao_contam_purge_loop —— 排异加速蓄力段：双手对掌灸火推送循环（P4）。

通道核验（P4 第一性原理，2026-07-19）：`resolve_yidao_skill`
（server/src/combat/yidao.rs）`insert_casting` 真实引导窗——contam_purge
cast_ticks_base=600t（30s），经 `yidao_cast_ticks` 缩放（窗长可变）→ 蓄力段
isLoop 覆盖任意窗长；release 段见 gen_yidao_contam_purge_release.py。停止
路径 = cast_emit 三打断分支 + 自然完成分支表驱动 StopAnim（§13 #6）。

母题（plan-yidao-v1 §5 ②）：灸火排异。医者站定，双掌于胸前对拢聚灸火，
随呼吸把火气一波波推送进患者体表排异点——「聚火（双臂收拢 bend 高）→
推送（双掌前伸下压、身体前送）→ 回抽再聚」的对称推拉循环。与接经术的
不对称针工（右针左探）、CPR 的垂直按压区分：本段是双手对称的水平推送。

循环红线（§13 #5 / 库坑 #1）：BASE 帧枚举全部轴，中间帧 inherit(BASE) 派生，
首尾帧 = BASE 本体，机械保证每轴 0/24 同值闭环。

时序（24t 灸火推送周期）：
  0   BASE：聚火位（双掌对拢胸前，bend 高）
  4   火成：双掌微开蓄热，身体吸气微抬
  8   推送启动：双掌前伸，身体前送
  12  推送顶点：双臂近展直、掌根下压，body.z 最前（灸火按进排异点）
  16  收劲：掌回撤半程
  20  回抽：双臂收拢回聚火位途中，身体回正吸气
  24  = BASE（闭环）
endTick=24，stopTick=26，isLoop=true。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 聚火基位：双掌对拢胸前（yaw 内收、bend 高），站桩微沉。
BASE = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=-0.03, z=0.0),
    head=dict(pitch=+10, yaw=0),
    torso=dict(pitch=+7, yaw=0),
    rightArm=dict(pitch=-58, yaw=-14, roll=-8, bend=62, axis=180),
    leftArm=dict(pitch=-58, yaw=+14, roll=+8, bend=62, axis=180),
    leftLeg=dict(pitch=-6, bend=10, z=-0.03),
    rightLeg=dict(pitch=+5, bend=9, z=+0.03),
)

POSE = {
    0: BASE,
    # 火成：双掌微开蓄热，吸气微抬。
    4: inherit(
        BASE,
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.02, z=-0.01),
        head=dict(pitch=+9, yaw=0),
        torso=dict(pitch=+6, yaw=0),
        rightArm=dict(pitch=-60, yaw=-18, roll=-9, bend=58, axis=180),
        leftArm=dict(pitch=-60, yaw=+18, roll=+9, bend=58, axis=180),
    ),
    # 推送启动：双掌前伸，身体开始前送。
    8: inherit(
        BASE,
        easing="INQUAD",
        body=dict(x=0.0, y=-0.045, z=+0.04),
        head=dict(pitch=+12, yaw=0),
        torso=dict(pitch=+9, yaw=0),
        rightArm=dict(pitch=-54, yaw=-11, roll=-6, bend=38, axis=180),
        leftArm=dict(pitch=-54, yaw=+11, roll=+6, bend=38, axis=180),
    ),
    # 推送顶点：双臂近展直、掌根下压，灸火按进排异点。
    12: inherit(
        BASE,
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.06, z=+0.07),
        head=dict(pitch=+14, yaw=0),
        torso=dict(pitch=+11, yaw=0),
        rightArm=dict(pitch=-48, yaw=-8, roll=-4, bend=16, axis=180),
        leftArm=dict(pitch=-48, yaw=+8, roll=+4, bend=16, axis=180),
        leftLeg=dict(pitch=-8, bend=13, z=-0.03),
        rightLeg=dict(pitch=+6, bend=11, z=+0.03),
    ),
    # 收劲：掌回撤半程。
    16: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.05, z=+0.04),
        head=dict(pitch=+12, yaw=0),
        torso=dict(pitch=+9, yaw=0),
        rightArm=dict(pitch=-53, yaw=-11, roll=-6, bend=36, axis=180),
        leftArm=dict(pitch=-53, yaw=+11, roll=+6, bend=36, axis=180),
    ),
    # 回抽：双臂收拢回聚火位途中，回正吸气。
    20: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.038, z=+0.015),
        head=dict(pitch=+11, yaw=0),
        torso=dict(pitch=+8, yaw=0),
        rightArm=dict(pitch=-56, yaw=-13, roll=-7, bend=52, axis=180),
        leftArm=dict(pitch=-56, yaw=+13, roll=+7, bend=52, axis=180),
    ),
    # endTick = BASE 本体：每轴与 tick 0 同值闭环（库坑 #1 机械保证）。
    24: inherit(BASE),
}


def main() -> int:
    emit_json(
        POSE,
        name="yidao_contam_purge_loop",
        description=(
            "P4 排异加速蓄力段（isLoop 24t）：双掌胸前聚灸火→前伸下压推送"
            "（bend 62→16 / body.z 0→+0.07）→ 回抽再聚的对称推拉循环，随呼吸"
            "沉浮。全轴 0/24 同值闭环。release 段见 yidao_contam_purge_release。"
        ),
        end_tick=24,
        stop_tick=26,
        is_loop=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
