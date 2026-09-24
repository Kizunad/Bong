#!/usr/bin/env python3
"""woliu_vortex_cast —— 绝灵涡流·双臂开涡（P3 批次三，借用解除 + 缺口修正）。

通道核验（P3 第一性原理，2026-07-19）：`resolve_woliu_vortex_skill`
（server/src/combat/woliu.rs:174，注册 :165）resolver 立即结算——同 tick 幂等
toggle `VortexField` 持续领域组件（:206），无 Casting/无引导窗，cast_ticks=1 →
**瞬发域**（[6,12]），endTick=10。

缺口修正：plan 附录 A 原判「combat/woliu.rs 零 PlayAnim」——实地核验发现动画走
**field lifecycle 驱动**：`emit_woliu_v1_vortex_visual_triggers`
（network/vfx_animation_trigger.rs，field 出现分支）已借播 v2 涡旋站桩
`vortex_spiral_stance`（20t，瞬发域不符 + 与 woliu.heart 撞形）。本批改指专属
`bong:woliu_vortex_cast`（常量 ANIM_WOLIU_V1_STANCE 改值），并删
MISSING_ANIM_ALLOWLIST 条目。非循环一次性起手式，field 存续由粒子环表达，
无需 StopAnim。

母题「双臂开涡」：双手胸前聚气 → 双臂横撒画圆开涡（外扬 + 仰面）→ 落定。
与 heart（举天下压）/ hold（单掌维持伞）/ burst（交叉弹开）全部区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→3   胸前聚气（双臂收拢 bend 90 / 微俯）
  strike       3→6   横撒开涡（双臂外扬 yaw ∓55 / 仰面 -8 / body.y +0.03），
                     顶点 = tick 6
  recovery     6→10  臂落身定归中立（INOUTSINE）
endTick=10，stopTick=12，非循环。主打击轴：rightArm.yaw / leftArm.yaw /
rightArm.pitch。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手：自然位微含。
    0: dict(
        easing="OUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=+2),
        torso=dict(pitch=+2, yaw=0),
        rightArm=dict(pitch=-30, yaw=+6, roll=0, bend=50, axis=180),
        leftArm=dict(pitch=-28, yaw=-6, roll=0, bend=52, axis=180),
        leftLeg=dict(pitch=-5, bend=7, z=-0.03),
        rightLeg=dict(pitch=+4, bend=6, z=+0.02),
    ),
    # 聚气：双手收到胸前、微俯闭气。
    3: dict(
        easing="OUTSINE",
        body=dict(y=-0.02, z=-0.02),
        head=dict(pitch=+7),
        torso=dict(pitch=+6, yaw=0),
        rightArm=dict(pitch=-52, yaw=+22, roll=-8, bend=90, axis=180),
        leftArm=dict(pitch=-50, yaw=-20, roll=+8, bend=92, axis=180),
        leftLeg=dict(pitch=-6, bend=9, z=-0.03),
        rightLeg=dict(pitch=+5, bend=8, z=+0.02),
    ),
    # 开涡中段：双臂拆开外扬过半。
    5: dict(
        easing="INQUAD",
        body=dict(y=+0.01, z=0.0),
        head=dict(pitch=-3),
        torso=dict(pitch=-2, yaw=0),
        rightArm=dict(pitch=-68, yaw=-30, roll=+16, bend=36, axis=180),
        leftArm=dict(pitch=-64, yaw=+32, roll=-16, bend=38, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 开涡顶点（tick 6）：双臂横撒到极限、仰面身浮。
    6: dict(
        easing="INQUAD",
        body=dict(y=+0.03, z=0.0),
        head=dict(pitch=-8),
        torso=dict(pitch=-6, yaw=0),
        rightArm=dict(pitch=-80, yaw=-55, roll=+25, bend=10, axis=180),
        leftArm=dict(pitch=-76, yaw=+55, roll=-25, bend=12, axis=180),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03),
        rightLeg=dict(pitch=+6, bend=8, z=+0.02),
    ),
    # 落定中段：臂缓缓落半、身回沉。
    8: dict(
        easing="INOUTSINE",
        body=dict(y=+0.01, z=0.0),
        head=dict(pitch=-3),
        torso=dict(pitch=-2, yaw=0),
        rightArm=dict(pitch=-40, yaw=-26, roll=+12, bend=22, axis=180),
        leftArm=dict(pitch=-38, yaw=+26, roll=-12, bend=24, axis=180),
        leftLeg=dict(pitch=-5, bend=7, z=-0.02),
        rightLeg=dict(pitch=+4, bend=6, z=+0.02),
    ),
    # 归中立。
    10: dict(
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
        name="woliu_vortex_cast",
        description=(
            "P3 绝灵涡流开涡专属（10t 瞬发，解除 vortex_spiral_stance 借用）：胸前"
            "聚气（双臂 bend 90 收拢）→ 横撒开涡（yaw ∓55 外扬 / 仰面 -8 / body.y "
            "+0.03，顶点=t6）→ 臂落归中立。field 存续由粒子环表达。"
        ),
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
