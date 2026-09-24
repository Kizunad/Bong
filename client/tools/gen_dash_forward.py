#!/usr/bin/env python3
"""dash_forward —— 疾冲步：压身摆臂→前窜→刹步（P3 批次三精修重制）。

通道核验（P3 第一性原理，2026-07-19）：movement.dash 走专用移动动作通道
`MovementActionIntent`（C2S "movement_action" → `handle_movement_action_intents`
→ `MovementAction::Dashing` 分支，server/src/movement/mod.rs:422-448，服务器
权威位移驱动 + `emit_action_feedback` 即时发 PlayAnim `bong:dash_forward`
priority 1450）。无 Casting/无 resolver，cast_ticks=0 → **瞬发域**（[6,12]），
endTick=8。id 不变原地重制。

原资产 4t/13 moves 快闪（附录 A C 级：密度低、无三段结构）。重制母题「疾冲
步」：快速压身 + 双臂反摆（右后左前跑姿）→ 蹬地前窜（臂位互换）→ 刹步回正。
与 xue_beng_bu（双臂同后摆拖尾的爆发突进）刻意区分：dash 是跑姿摆臂、幅度轻。

时序（精度标准 #1/#2/#3）：
  anticipation 0→2   压身摆臂（body.y -0.06 / torso.pitch +12 / 右臂后左臂前）
  strike       2→4   蹬地前窜（body.z +0.30 / torso.pitch +20 / 臂位互换跑姿），
                     顶点 = tick 4
  recovery     4→8   刹步回正归中立（INOUTSINE）
endTick=8，stopTick=10，非循环。主打击轴：body.z / torso.pitch / rightArm.pitch。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手：站姿。
    0: dict(
        easing="OUTQUAD",
        body=dict(y=-0.01, z=0.0),
        head=dict(pitch=+2),
        torso=dict(pitch=+4, yaw=0),
        rightArm=dict(pitch=+8, yaw=-3, roll=+2, bend=20, axis=180),
        leftArm=dict(pitch=-10, yaw=+3, roll=-2, bend=22, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 压身摆臂：右臂后摆、左臂前抬（跑姿准备）。
    2: dict(
        easing="OUTQUAD",
        body=dict(y=-0.06, z=-0.04),
        head=dict(pitch=+4),
        torso=dict(pitch=+12, yaw=+5),
        rightArm=dict(pitch=+30, yaw=-8, roll=+4, bend=32, axis=180),
        leftArm=dict(pitch=-35, yaw=+8, roll=-4, bend=45, axis=180),
        leftLeg=dict(pitch=-10, bend=18, z=-0.05),
        rightLeg=dict(pitch=+9, bend=16, z=+0.04),
    ),
    # 前窜中段：蹬地离位、臂位开始互换。
    3: dict(
        easing="INQUAD",
        body=dict(y=-0.04, z=+0.16),
        head=dict(pitch=+3),
        torso=dict(pitch=+17, yaw=-2),
        rightArm=dict(pitch=-12, yaw=-6, roll=+3, bend=40, axis=180),
        leftArm=dict(pitch=+6, yaw=+6, roll=-3, bend=36, axis=180),
        leftLeg=dict(pitch=-22, bend=14, z=-0.09),
        rightLeg=dict(pitch=+24, bend=24, z=+0.06),
    ),
    # 前窜顶点（tick 4）：身体前射、跑姿满臂（右前左后）。
    4: dict(
        easing="INQUAD",
        body=dict(y=-0.03, z=+0.30),
        head=dict(pitch=+3),
        torso=dict(pitch=+20, yaw=-5),
        rightArm=dict(pitch=-45, yaw=-8, roll=+4, bend=48, axis=180),
        leftArm=dict(pitch=+35, yaw=+8, roll=-4, bend=30, axis=180),
        leftLeg=dict(pitch=-28, bend=14, z=-0.11),
        rightLeg=dict(pitch=+30, bend=26, z=+0.07),
    ),
    # 刹步：接住重心、臂回落。
    6: dict(
        easing="INOUTSINE",
        body=dict(y=-0.03, z=+0.10),
        head=dict(pitch=+2),
        torso=dict(pitch=+9, yaw=-2),
        rightArm=dict(pitch=-16, yaw=-5, roll=+3, bend=26, axis=180),
        leftArm=dict(pitch=+12, yaw=+5, roll=-3, bend=22, axis=180),
        leftLeg=dict(pitch=-14, bend=18, z=-0.06),
        rightLeg=dict(pitch=+13, bend=16, z=+0.04),
    ),
    # 归中立。
    8: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0),
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
        name="dash_forward",
        description=(
            "P3 疾冲步重制（8t 瞬发，原 4t 快闪密度补齐）：压身摆臂（body.y -0.06 "
            "/ 右后左前跑姿）→ 蹬地前窜（body.z +0.30 / 臂位互换，顶点=t4）→ "
            "刹步回正归中立。"
        ),
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
