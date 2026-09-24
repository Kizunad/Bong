#!/usr/bin/env python3
"""woliu_heart —— 涡心：举天聚涡→千钧压落沉桩（P3 批次三，去共用）。

通道核验（P3 第一性原理，2026-07-19）：`cast_heart`
（server/src/combat/woliu_v2/skills.rs:202）→ `resolve_woliu_v2_skill`（:305）
resolver 同步一次性结算，恒插 `VortexV2State`（:411 `skill == Heart` 恒真，
持续场表达归粒子；窗到期 StopAnim 由 `emit_woliu_v2_visual_stop_triggers`
对非循环动画无实际效果，无碍）。零 Casting/零引导窗，cast_ticks=10 为元数据——
短 cast 走**三段式**标准域，endTick = cast(10) + recovery 6 = 16 ∈ [14,18]。

去共用：原 visual_for `"bong:vortex_spiral_stance"`（与 v1 站桩共用）→ 专属
`bong:woliu_heart`。母题「山谷级强压」：双臂缓举过顶（把整片场域的灵机拢起）
→ 千钧压落到心口高度、深沉马步（谷底之压）→ 缓慢起身。**过顶大举-重压下落**
的纵向大行程是本招签名，与开涡（横撒）/ 涡口（前探）/ 涡引（前探后拽）区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→6   举天聚涡（双臂 -150/-145 过顶 / 仰面 / 身微浮）
  strike       6→10  千钧压落（双臂压到心口 -35 / body.y -0.12 深沉 /
                     torso.pitch +10），顶点 = tick 10（cast 完成）
  recovery     10→16 谷底起身归中立（INOUTSINE，重招缓收）
endTick=16，stopTick=18，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
body.y。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手：静立聚气。
    0: dict(
        easing="OUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+2, yaw=0, roll=0),
        rightArm=dict(pitch=-30, yaw=-8, roll=+4, bend=36, axis=180),
        leftArm=dict(pitch=-28, yaw=+8, roll=-4, bend=38, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 举涡中段：双臂过肩。
    3: dict(
        easing="OUTSINE",
        body=dict(y=+0.01, z=-0.01),
        head=dict(pitch=-4, yaw=0),
        torso=dict(pitch=-2, yaw=0, roll=0),
        rightArm=dict(pitch=-95, yaw=-14, roll=+8, bend=32, axis=180),
        leftArm=dict(pitch=-90, yaw=+14, roll=-8, bend=34, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 举天顶点：双臂过顶合拢、仰面身浮。
    6: dict(
        easing="OUTSINE",
        body=dict(y=+0.02, z=-0.02),
        head=dict(pitch=-10, yaw=0),
        torso=dict(pitch=-5, yaw=0, roll=0),
        rightArm=dict(pitch=-150, yaw=-18, roll=+10, bend=30, axis=180),
        leftArm=dict(pitch=-145, yaw=+18, roll=-10, bend=32, axis=180),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03),
        rightLeg=dict(pitch=+6, bend=8, z=+0.02),
    ),
    # 压落中段：臂过面门、身开始下沉。
    8: dict(
        easing="INQUAD",
        body=dict(y=-0.05, z=0.0),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+4, yaw=0, roll=0),
        rightArm=dict(pitch=-90, yaw=-10, roll=+6, bend=50, axis=180),
        leftArm=dict(pitch=-85, yaw=+10, roll=-6, bend=52, axis=180),
        leftLeg=dict(pitch=-10, bend=18, z=-0.05),
        rightLeg=dict(pitch=+9, bend=16, z=+0.04),
    ),
    # 压落顶点（tick 10 = cast 完成）：掌压心口高度、深沉马步。
    10: dict(
        easing="INQUAD",
        body=dict(y=-0.12, z=0.0),
        head=dict(pitch=+10, yaw=0),
        torso=dict(pitch=+10, yaw=0, roll=0),
        rightArm=dict(pitch=-35, yaw=-6, roll=+14, bend=70, axis=180),
        leftArm=dict(pitch=-31, yaw=+6, roll=-14, bend=72, axis=180),
        leftLeg=dict(pitch=-12, bend=26, z=-0.06),
        rightLeg=dict(pitch=+11, bend=24, z=+0.05),
    ),
    # 谷底定势：压意驻留半拍。
    12: dict(
        easing="INOUTSINE",
        body=dict(y=-0.10, z=0.0),
        head=dict(pitch=+8, yaw=0),
        torso=dict(pitch=+8, yaw=0, roll=0),
        rightArm=dict(pitch=-30, yaw=-5, roll=+10, bend=62, axis=180),
        leftArm=dict(pitch=-26, yaw=+5, roll=-10, bend=64, axis=180),
        leftLeg=dict(pitch=-11, bend=22, z=-0.05),
        rightLeg=dict(pitch=+10, bend=20, z=+0.04),
    ),
    # 起身中段。
    14: dict(
        easing="INOUTSINE",
        body=dict(y=-0.05, z=0.0),
        head=dict(pitch=+4, yaw=0),
        torso=dict(pitch=+4, yaw=0, roll=0),
        rightArm=dict(pitch=-14, yaw=-3, roll=+5, bend=28, axis=180),
        leftArm=dict(pitch=-12, yaw=+3, roll=-5, bend=30, axis=180),
        leftLeg=dict(pitch=-7, bend=12, z=-0.03),
        rightLeg=dict(pitch=+6, bend=10, z=+0.02),
    ),
    # 归中立。
    16: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0, roll=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftLeg=dict(pitch=0, bend=0, z=0.0),
        rightLeg=dict(pitch=0, bend=0, z=0.0),
    ),
}


def main() -> int:
    emit_json(
        POSE,
        name="woliu_heart",
        description=(
            "P3 涡心专属（16t 三段式，与 v1 站桩解除共用）：举天聚涡（双臂 "
            "-150/-145 过顶仰面）→ 千钧压落心口+深沉马步（pitch→-35 / body.y "
            "-0.12，顶点=t10 cast 完成）→ 谷底定势缓起归中立。"
        ),
        end_tick=16,
        stop_tick=18,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
