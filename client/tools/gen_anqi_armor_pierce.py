#!/usr/bin/env python3
"""anqi_armor_pierce —— 破甲注射：t0 贯刺命中→钻拧余韵（P2 后半，review r2 定形）。

cast_ticks=40 是**元数据**：`resolve_anqi_skill` 在 cast 起始 tick 立即结算
（anqi_v2.rs 无 `Casting`、无 timer、无打断窗）。r1 返工版仍留 2t anticipation
（顶点 t6），r2 review 裁定违反「瞬发结算型 strike 顶点=tick 0」跨端时序契约
——伤害结算与视觉命中必须同帧。本版 **tick 0 即贯刺命中顶点**：动画开帧就是
全伸贯刺姿态（server PlayAnim fade_in 1-2t 是引擎平滑下限），其后只承担钻拧
余震、撤臂与收势。契约由 instant spec manifest（strike_peak_tick=0）+
AnimCastTicksAlignmentTest INSTANT_RESOLVER_SKILLS 分类 pin 机械锁定，不再驻
CAST_ALIGNMENT_ALLOWLIST。

母题：贯甲钻拧。开帧即弓步全伸贯刺（roll 反拧到底、躯干甩转 +18、body.z
+0.22），钻头在创口内往复拧转（t2 roll +8 / t4 roll -18 极值直落采样帧，
CodeRabbit 欠采样修复语义保持），撤臂直身回中立。与凝魂注射（面前举镖下压）
/ 单射（侧身鞭甩）动向完全区分。

时序（instant 契约 + 精度标准 #3）：
  strike    0→2   t0 贯刺命中顶点（rightArm pitch -85 / roll -25 / torso.yaw
                  +18 / body.z +0.22）→ t2 钻拧 A（roll +8，保持全伸）
  recovery  2→12  t4 钻拧 B（roll -18 极值帧）→ t6 撤臂半程 → t9 直身 →
                  t12 归中立（INOUTSINE）
endTick=12，stopTick=14，非循环。主打击轴：rightArm.pitch / rightArm.roll /
torso.yaw / body.z（全程 ≤3t 帧距，t0 全轴落帧）。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # t0 = 贯刺命中顶点（与 resolver 结算同帧）：弓步全伸、roll 反拧到底。
    0: dict(
        easing="OUTQUAD",
        body=dict(x=-0.02, y=-0.02, z=+0.22),
        head=dict(pitch=+2, yaw=+6),
        torso=dict(pitch=+12, yaw=+18),
        rightArm=dict(pitch=-85, yaw=-8, roll=-25, bend=4, axis=180),
        leftArm=dict(pitch=-18, yaw=+22, roll=-10, bend=52, axis=180),
        leftLeg=dict(pitch=-24, bend=24, z=-0.11),
        rightLeg=dict(pitch=+18, bend=22, z=+0.07),
    ),
    # 钻拧余震 A：臂保持贯出，roll 反向拧回极值（极值直落采样帧）。
    2: dict(
        easing="OUTSINE",
        body=dict(x=-0.02, y=-0.02, z=+0.20),
        head=dict(pitch=+2, yaw=+5),
        torso=dict(pitch=+11, yaw=+14),
        rightArm=dict(pitch=-83, yaw=-9, roll=+8, bend=6, axis=180),
        leftArm=dict(pitch=-17, yaw=+21, roll=-8, bend=50, axis=180),
        leftLeg=dict(pitch=-23, bend=23, z=-0.10),
        rightLeg=dict(pitch=+17, bend=21, z=+0.07),
    ),
    # 钻拧余震 B：roll 再反拧（幅度衰减），钻头意象收尾。
    4: dict(
        easing="INOUTSINE",
        body=dict(x=-0.02, y=-0.02, z=+0.17),
        head=dict(pitch=+2, yaw=+4),
        torso=dict(pitch=+10, yaw=+12),
        rightArm=dict(pitch=-80, yaw=-10, roll=-18, bend=10, axis=180),
        leftArm=dict(pitch=-16, yaw=+19, roll=-7, bend=46, axis=180),
        leftLeg=dict(pitch=-21, bend=21, z=-0.09),
        rightLeg=dict(pitch=+16, bend=19, z=+0.06),
    ),
    # 撤臂半程。
    6: dict(
        easing="INOUTSINE",
        body=dict(x=-0.01, y=-0.01, z=+0.10),
        head=dict(pitch=+2, yaw=+2),
        torso=dict(pitch=+6, yaw=+8),
        rightArm=dict(pitch=-45, yaw=-10, roll=-8, bend=24, axis=180),
        leftArm=dict(pitch=-12, yaw=+14, roll=-6, bend=30, axis=180),
        leftLeg=dict(pitch=-14, bend=15, z=-0.07),
        rightLeg=dict(pitch=+11, bend=13, z=+0.05),
    ),
    # 直身。
    9: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.01, z=+0.04),
        head=dict(pitch=+1, yaw=+1),
        torso=dict(pitch=+3, yaw=+3),
        rightArm=dict(pitch=-20, yaw=-6, roll=-3, bend=12, axis=180),
        leftArm=dict(pitch=-6, yaw=+7, roll=-3, bend=14, axis=180),
        leftLeg=dict(pitch=-7, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=6, z=+0.02),
    ),
    # 归中立。
    12: dict(
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
        name="anqi_armor_pierce",
        description=(
            "P2 破甲注射专属（12t 非循环，instant 契约：strike 顶点=tick 0 与 "
            "resolver 结算同帧，解除 cast_invoke 借用；cast_ticks=40 为元数据）："
            "t0 贯刺命中顶点（pitch -85 / roll -25 / torso.yaw +18 / body.z "
            "+0.22），t2/t4 钻拧余震（roll +8/-18 极值帧），recovery 6→12 撤臂"
            "直身归中立。"
        ),
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
