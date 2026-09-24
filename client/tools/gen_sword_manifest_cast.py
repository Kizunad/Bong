#!/usr/bin/env python3
"""sword_manifest_cast —— 剑意化形：t0 送出命中→目送余韵（P2 后半，review r2 定形）。

cast_ticks=40 是**元数据**：`cast_manifest`（skill_register.rs）在 cast 起始
tick 立即 spawn SwordIntentEntity 追击实体。r1 返工版仍留 2t anticipation
（顶点 t6），r2 review 裁定违反「瞬发结算型 strike 顶点=tick 0」跨端时序契约
——化形剑 tick 0 已在场上飞，动画开帧就必须是送出姿态。本版 **tick 0 即翻腕
送出顶点**，其后只承担目送余韵与收势。契约由 instant spec manifest
（strike_peak_tick=0）+ AnimCastTicksAlignmentTest INSTANT_RESOLVER_SKILLS
分类 pin 机械锁定，不再驻 CAST_ALIGNMENT_ALLOWLIST。

母题：送出目送。开帧即右手翻腕虚握前送（化形剑离手、躯干甩转 +14、弓步
body.z +0.16），随后长目送余韵（头随剑意抬望远方 head.pitch -12、双臂缓落，
t5→t11）收势归中立。与 heaven_gate（高举过顶蓄力）/ 基础剑招（持实剑挥斩）
动向完全区分。

时序（instant 契约 + 精度标准 #3）：
  strike    0→2   t0 翻腕送出顶点（rightArm pitch -92 前指 / torso.yaw +14 /
                  body.z +0.16）→ t2 送出定格
  recovery  2→14  t5 目送 A（head.pitch -12 随剑意望远）→ t8 双臂缓落 →
                  t11 直身 → t14 归中立（INOUTSINE）
endTick=14，stopTick=16，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
torso.yaw / body.z（全程 ≤3t 帧距，t0 全轴落帧）。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # t0 = 翻腕送出顶点（与 spawn SwordIntentEntity 同帧）：化形剑离手飞出。
    0: dict(
        easing="OUTQUAD",
        body=dict(x=-0.01, y=-0.02, z=+0.16),
        head=dict(pitch=-2, yaw=+4),
        torso=dict(pitch=+8, yaw=+14),
        rightArm=dict(pitch=-92, yaw=-6, roll=+18, bend=6, axis=180),
        leftArm=dict(pitch=-20, yaw=+18, roll=+6, bend=40, axis=180),
        leftLeg=dict(pitch=-18, bend=19, z=-0.09),
        rightLeg=dict(pitch=+14, bend=16, z=+0.06),
    ),
    # 送出定格：臂保持前指、剑意远去。
    2: dict(
        easing="OUTSINE",
        body=dict(x=-0.01, y=-0.02, z=+0.14),
        head=dict(pitch=-6, yaw=+3),
        torso=dict(pitch=+7, yaw=+11),
        rightArm=dict(pitch=-90, yaw=-7, roll=+14, bend=8, axis=180),
        leftArm=dict(pitch=-18, yaw=+16, roll=+5, bend=38, axis=180),
        leftLeg=dict(pitch=-17, bend=18, z=-0.08),
        rightLeg=dict(pitch=+13, bend=15, z=+0.06),
    ),
    # 目送 A：头随剑意抬望远方、前指臂微落。
    5: dict(
        easing="INOUTSINE",
        body=dict(x=-0.01, y=-0.01, z=+0.10),
        head=dict(pitch=-12, yaw=+2),
        torso=dict(pitch=+5, yaw=+8),
        rightArm=dict(pitch=-72, yaw=-8, roll=+8, bend=14, axis=180),
        leftArm=dict(pitch=-14, yaw=+12, roll=+4, bend=28, axis=180),
        leftLeg=dict(pitch=-13, bend=14, z=-0.06),
        rightLeg=dict(pitch=+10, bend=12, z=+0.04),
    ),
    # 目送 B：双臂缓落。
    8: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.01, z=+0.06),
        head=dict(pitch=-8, yaw=+1),
        torso=dict(pitch=+3, yaw=+5),
        rightArm=dict(pitch=-40, yaw=-8, roll=+4, bend=16, axis=180),
        leftArm=dict(pitch=-10, yaw=+8, roll=+3, bend=18, axis=180),
        leftLeg=dict(pitch=-9, bend=10, z=-0.04),
        rightLeg=dict(pitch=+7, bend=8, z=+0.03),
    ),
    # 直身。
    11: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=+0.02),
        head=dict(pitch=-3, yaw=0),
        torso=dict(pitch=+1, yaw=+2),
        rightArm=dict(pitch=-16, yaw=-4, roll=+2, bend=8, axis=180),
        leftArm=dict(pitch=-5, yaw=+4, roll=+1, bend=8, axis=180),
        leftLeg=dict(pitch=-4, bend=5, z=-0.02),
        rightLeg=dict(pitch=+3, bend=4, z=+0.01),
    ),
    # 归中立。
    14: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
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
        name="sword_manifest_cast",
        description=(
            "P2 剑意化形专属（14t 非循环，instant 契约：strike 顶点=tick 0 与 "
            "cast_manifest spawn SwordIntentEntity 同帧；cast_ticks=40 为元数据）："
            "t0 翻腕送出顶点（pitch -92 / torso.yaw +14 / body.z +0.16），"
            "recovery 2→14 目送余韵（head.pitch -12 随剑意望远）→收势归中立。"
        ),
        end_tick=14,
        stop_tick=16,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
