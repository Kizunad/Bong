#!/usr/bin/env python3
"""vortex_palm_open —— 持涡：绕臂托举→撑伞开掌定势（P3 批次三精修重制）。

通道核验（P3 第一性原理，2026-07-19）：`cast_hold`
（server/src/combat/woliu_v2/skills.rs:166）→ `resolve_woliu_v2_skill`（:305）
resolver 同步一次性结算（零 Casting/零引导窗），cast_ticks=1 → **瞬发域**
（[6,12]），endTick=10（原 12t/40KF 在域内但密度不足，附录 A B 级精修）。
id 不变原地重制（woliu.hold 专属，schema 样例 fixture 同 id 不受影响）。

母题「维持伞」：右掌自腰间绕一小圈托举而上 → 头顶前上方撑开掌心（伞面）→
定势微颤（涡流持续悬顶）→ 收半步留姿。单掌上撑的纵向托举与开涡（双臂横撒）
/ 涡心（双臂重压）区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→3   绕臂托举（右掌腰间绕圈 roll -10→+8 / 身微沉）
  strike       3→6   撑伞开掌（右臂 -105 上撑 / yaw -25 外旋 / body.y +0.02），
                     顶点 = tick 6；6→8 伞面微颤 hold（归 strike 段）
  recovery     8→10  松腕落半、留持涡姿归中立（INOUTSINE）
endTick=10，stopTick=12，非循环。主打击轴：rightArm.pitch / rightArm.yaw /
rightArm.roll。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手：右掌垂于腰侧。
    0: dict(
        easing="OUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+2, yaw=+3),
        rightArm=dict(pitch=-15, yaw=+8, roll=-10, bend=55, axis=180),
        leftArm=dict(pitch=-12, yaw=-4, roll=0, bend=30, axis=180),
        leftLeg=dict(pitch=-5, bend=7, z=-0.02),
        rightLeg=dict(pitch=+4, bend=6, z=+0.02),
    ),
    # 绕圈托举：掌心翻转向上、抬至胸前。
    3: dict(
        easing="OUTSINE",
        body=dict(y=-0.02, z=-0.01),
        head=dict(pitch=+3, yaw=-2),
        torso=dict(pitch=+4, yaw=+6),
        rightArm=dict(pitch=-60, yaw=+20, roll=+8, bend=70, axis=180),
        leftArm=dict(pitch=-18, yaw=-8, roll=-4, bend=34, axis=180),
        leftLeg=dict(pitch=-6, bend=9, z=-0.03),
        rightLeg=dict(pitch=+5, bend=8, z=+0.02),
    ),
    # 撑伞中段：掌离胸上行。
    5: dict(
        easing="INQUAD",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=-3, yaw=-3),
        torso=dict(pitch=-1, yaw=+2),
        rightArm=dict(pitch=-88, yaw=-8, roll=+12, bend=32, axis=180),
        leftArm=dict(pitch=-22, yaw=-10, roll=-5, bend=36, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 撑伞顶点（tick 6）：掌开头顶前上方、仰视伞面。
    6: dict(
        easing="INQUAD",
        body=dict(y=+0.02, z=0.0),
        head=dict(pitch=-6, yaw=-4),
        torso=dict(pitch=-3, yaw=0),
        rightArm=dict(pitch=-105, yaw=-25, roll=+15, bend=12, axis=180),
        leftArm=dict(pitch=-26, yaw=-12, roll=-6, bend=38, axis=180),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03),
        rightLeg=dict(pitch=+6, bend=8, z=+0.02),
    ),
    # 伞面微颤（涡流悬顶，hold 归 strike）。
    8: dict(
        easing="INOUTSINE",
        body=dict(y=+0.015, z=0.0),
        head=dict(pitch=-5, yaw=-3),
        torso=dict(pitch=-2, yaw=+1),
        rightArm=dict(pitch=-102, yaw=-22, roll=+10, bend=15, axis=180),
        leftArm=dict(pitch=-24, yaw=-11, roll=-5, bend=36, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 归中立（持涡场由粒子延续）。
    10: dict(
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
        name="vortex_palm_open",
        description=(
            "P3 持涡重制（10t 瞬发，原 12t 低密度精修）：右掌腰间绕圈托举"
            "（roll -10→+8 翻掌）→ 头顶前上方撑伞开掌（pitch -105 / yaw -25，"
            "顶点=t6，6→8 伞面微颤）→ 松腕归中立。"
        ),
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
