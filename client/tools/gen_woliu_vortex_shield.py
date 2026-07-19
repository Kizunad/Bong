#!/usr/bin/env python3
"""woliu_vortex_shield —— 涡盾：环抱屏障公转循环（P3 批次三精修重制）。

通道核验（P3 第一性原理，2026-07-19）：`cast_vortex_shield`
（server/src/combat/woliu_v2/skills.rs:220）→ `resolve_woliu_v2_skill`（:305）
resolver 同步结算 + 插 `VortexV2State{active_until=now+5s}`（:410-421，减伤
DamageReduction 0.6/5s）。**持续维持型循环**：唯一退出路径 = active 窗自然
到期，由 `emit_woliu_v2_visual_stop_triggers`（network/vfx_animation_trigger.rs
:554-570，was_active→!active 边沿）发 StopAnim(fade 4t)——无提前破盾/主动取消
机制，停止路径完整（§13 #6）。入对拍测试 SUSTAINED_LOOP_EXCEPTIONS + segment
loop manifest，出 CAST_ALIGNMENT_ALLOWLIST。id 不变原地重制（原 18t/33KF
三帧点稀疏，且首尾闭合不完备）。

母题「环抱屏障」：双臂胸前环抱成圆（抱涡成盾），双手沿盾面缓慢公转一周
（右手引、左手随），身体随涡息轻浮沉。全程低位环抱，与持涡（单掌上撑）/
真空锁（开合下压）区分。

循环红线（§13 #5/#6，库坑 #1）：BASE 帧枚举全部轴，中间帧 inherit(BASE) 派生，
首尾帧 = BASE 同值闭环（loopSeamViolations 机械为空）。

时序（20t 公转周期，4t 步进满足主轴密度 ≤4t 红线）：
  0→4    公转东位：双手右移（yaw 偏 +6 / roll +8/-4）、身微浮
  4→8    公转南位：双手沉底（pitch -48 / body.y -0.05）
  8→12   公转西位：双手左移（yaw 偏 -6 / roll -4/+8）、身回浮
  12→16  公转北位：双手上抬回顶（pitch -63 / 身近浮位）
  16→20  收回 BASE（endTick 同值闭环）
endTick=20，stopTick=22，isLoop=true。主轴：rightArm.yaw / rightArm.pitch /
body.y。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 环抱基位：双臂胸前抱圆（掌心向内环涡）、微沉桩。BASE 枚举全部 part.axis。
BASE = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=-0.03, z=-0.01),
    head=dict(pitch=+5, yaw=0),
    torso=dict(pitch=+4, yaw=0, roll=0),
    rightArm=dict(pitch=-58, yaw=-20, roll=+10, bend=82, axis=180),
    leftArm=dict(pitch=-55, yaw=+22, roll=-12, bend=85, axis=180),
    leftLeg=dict(pitch=-7, bend=12, z=-0.04),
    rightLeg=dict(pitch=+6, bend=11, z=+0.03),
)

POSE = {
    0: BASE,
    # 公转东位：双手沿盾面右移、身微浮。
    4: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=+0.015, y=-0.015, z=-0.01),
        head=dict(pitch=+4, yaw=+2),
        torso=dict(pitch=+3, yaw=+3, roll=+1),
        rightArm=dict(pitch=-62, yaw=-14, roll=+18, bend=78, axis=180),
        leftArm=dict(pitch=-50, yaw=+28, roll=-8, bend=88, axis=180),
    ),
    # 公转南位：双手沉到盾底、身沉。
    8: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.05, z=-0.014),
        head=dict(pitch=+7, yaw=0),
        torso=dict(pitch=+6, yaw=0, roll=0),
        rightArm=dict(pitch=-48, yaw=-22, roll=+6, bend=90, axis=180),
        leftArm=dict(pitch=-45, yaw=+24, roll=-8, bend=92, axis=180),
    ),
    # 公转西位：双手左移、身回浮。
    12: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=-0.015, y=-0.02, z=-0.01),
        head=dict(pitch=+4, yaw=-2),
        torso=dict(pitch=+3, yaw=-3, roll=-1),
        rightArm=dict(pitch=-54, yaw=-26, roll=+4, bend=86, axis=180),
        leftArm=dict(pitch=-60, yaw=+16, roll=-18, bend=80, axis=180),
    ),
    # 公转北位：双手回顶、身近浮位。
    16: inherit(
        BASE,
        easing="INOUTSINE",
        body=dict(x=-0.005, y=-0.024, z=-0.01),
        head=dict(pitch=+4.5, yaw=-1),
        torso=dict(pitch=+3.5, yaw=-1, roll=0),
        rightArm=dict(pitch=-63, yaw=-22, roll=+8, bend=80, axis=180),
        leftArm=dict(pitch=-60, yaw=+20, roll=-14, bend=82, axis=180),
    ),
    # endTick = BASE 本体：每轴与 tick 0 同值闭环（库坑 #1 机械保证）。
    20: inherit(BASE),
}


def main() -> int:
    emit_json(
        POSE,
        name="woliu_vortex_shield",
        description=(
            "P3 涡盾重制（isLoop 20t 闭环，原 18t 稀疏）：双臂胸前环抱成盾，"
            "双手沿盾面公转一周（东 yaw+roll 偏移 → 南沉底 body.y -0.05 → 西反向 "
            "→ 北回位），身随涡息浮沉。全轴 0/20 同值闭环；StopAnim=窗到期边沿"
            "（vfx_animation_trigger.rs:554）。"
        ),
        end_tick=20,
        stop_tick=22,
        is_loop=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
