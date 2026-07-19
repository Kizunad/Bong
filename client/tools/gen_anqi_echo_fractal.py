#!/usr/bin/env python3
"""anqi_echo_fractal —— 诱饵分形：t0 爆撒命中→织网余韵（P2 后半，review r2 定形）。

cast_ticks=60 是**元数据**：`resolve_anqi_skill` 在 cast 起始 tick 立即结算
（与 armor_pierce 同一 resolver 通道）。r1 返工版仍留 2t anticipation（顶点
t4），r2 review 裁定违反「瞬发结算型 strike 顶点=tick 0」跨端时序契约。本版
**tick 0 即爆撒顶点**：开帧即双臂外撒仰开（诱饵已离手），其后只承担织网
余韵与收势。契约由 instant spec manifest（strike_peak_tick=0）+
AnimCastTicksAlignmentTest INSTANT_RESOLVER_SKILLS 分类 pin 机械锁定，不再驻
CAST_ALIGNMENT_ALLOWLIST。

母题：爆撒织网。开帧即双臂高位外张仰开（撒饵爆发、重心上提），随后双臂在
高位反相波动「编织回响之网」（t4→t13 交替相位、幅度渐衰——分形回响意象），
收臂归中立。与单射（侧身鞭甩）/ 齐射（拢镖开扇平撒后即收）动向完全区分：
本招开帧爆撒后有长织网余韵。

时序（instant 契约 + 精度标准 #3）：
  strike    0→2   t0 爆撒顶点（双臂 pitch -118/-112 外张、torso.pitch -8 后仰、
                  body.y +0.04）→ t2 撒满落定
  recovery  2→20  t4/t7/t10/t13 双臂反相波动（幅度渐衰）→ t16 收半 → t20
                  归中立（INOUTSINE）
endTick=20，stopTick=22，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
torso.pitch / body.y（全程 ≤4t 帧距，t0 全轴落帧）。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # t0 = 爆撒顶点（与 resolver 结算同帧）：双臂外撒仰开、重心上提。
    0: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.04, z=+0.02),
        head=dict(pitch=-14, yaw=0),
        torso=dict(pitch=-8, yaw=+2),
        rightArm=dict(pitch=-118, yaw=-52, roll=+14, bend=8, axis=180),
        leftArm=dict(pitch=-112, yaw=+56, roll=-14, bend=10, axis=180),
        leftLeg=dict(pitch=-4, bend=5, z=-0.02),
        rightLeg=dict(pitch=+4, bend=4, z=+0.02),
    ),
    # 撒满落定：臂稍落、仰姿保持。
    2: dict(
        easing="OUTSINE",
        body=dict(x=0.0, y=+0.02, z=+0.01),
        head=dict(pitch=-10, yaw=0),
        torso=dict(pitch=-6, yaw=0),
        rightArm=dict(pitch=-106, yaw=-44, roll=+10, bend=16, axis=180),
        leftArm=dict(pitch=-102, yaw=+48, roll=-10, bend=18, axis=180),
        leftLeg=dict(pitch=-4, bend=5, z=-0.02),
        rightLeg=dict(pitch=+4, bend=4, z=+0.02),
    ),
    # 织网余韵 A：右臂沉左臂扬（反相波动，幅度最大）。
    4: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=+0.01, z=0.0),
        head=dict(pitch=-6, yaw=-5),
        torso=dict(pitch=-4, yaw=-4),
        rightArm=dict(pitch=-88, yaw=-30, roll=+6, bend=30, axis=180),
        leftArm=dict(pitch=-110, yaw=+42, roll=-8, bend=14, axis=180),
        leftLeg=dict(pitch=-3, bend=4, z=-0.02),
        rightLeg=dict(pitch=+3, bend=3, z=+0.01),
    ),
    # 织网余韵 B：反相（右扬左沉）。
    7: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=+0.01, z=0.0),
        head=dict(pitch=-6, yaw=+5),
        torso=dict(pitch=-4, yaw=+4),
        rightArm=dict(pitch=-104, yaw=-38, roll=+8, bend=16, axis=180),
        leftArm=dict(pitch=-84, yaw=+28, roll=-6, bend=32, axis=180),
        leftLeg=dict(pitch=-3, bend=4, z=-0.02),
        rightLeg=dict(pitch=+3, bend=3, z=+0.01),
    ),
    # 织网余韵 A'（幅度衰减）。
    10: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-4, yaw=-3),
        torso=dict(pitch=-3, yaw=-2),
        rightArm=dict(pitch=-86, yaw=-26, roll=+4, bend=28, axis=180),
        leftArm=dict(pitch=-98, yaw=+34, roll=-5, bend=18, axis=180),
        leftLeg=dict(pitch=-2, bend=3, z=-0.01),
        rightLeg=dict(pitch=+2, bend=2, z=+0.01),
    ),
    # 织网余韵 B'（再衰减）。
    13: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-3, yaw=+2),
        torso=dict(pitch=-2, yaw=+2),
        rightArm=dict(pitch=-92, yaw=-30, roll=+4, bend=20, axis=180),
        leftArm=dict(pitch=-82, yaw=+26, roll=-4, bend=28, axis=180),
        leftLeg=dict(pitch=-2, bend=3, z=-0.01),
        rightLeg=dict(pitch=+2, bend=2, z=+0.01),
    ),
    # 收臂半程。
    16: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-1, yaw=0),
        torso=dict(pitch=-1, yaw=0),
        rightArm=dict(pitch=-44, yaw=-14, roll=+2, bend=14, axis=180),
        leftArm=dict(pitch=-40, yaw=+12, roll=-2, bend=14, axis=180),
        leftLeg=dict(pitch=-1, bend=1, z=-0.01),
        rightLeg=dict(pitch=+1, bend=1, z=0.0),
    ),
    # 归中立。
    20: dict(
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
        name="anqi_echo_fractal",
        description=(
            "P2 诱饵分形专属（20t 非循环，instant 契约：strike 顶点=tick 0 与 "
            "resolver 结算同帧，解除 release_burst 借用；cast_ticks=60 为元数据）："
            "t0 爆撒顶点（双臂 -118/-112 外张 / torso.pitch -8 / body.y +0.04），"
            "recovery 2→20 织网余韵（t4/t7/t10/t13 双臂反相波动渐衰）→收臂归中立。"
        ),
        end_tick=20,
        stop_tick=22,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
