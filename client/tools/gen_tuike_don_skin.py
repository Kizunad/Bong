#!/usr/bin/env python3
"""tuike_don_skin —— 着壳：俯身探底→沿身上提披壳→定壳（P3 批次三精修重制）。

通道核验（P3 第一性原理，2026-07-19）：`cast_don`
（server/src/combat/tuike_v2/skills.rs:53）resolver 立即结算——当场 push_outer
伪皮层（:81）+ set_cooldown（:98），**零 Casting/零引导窗**，cast_ticks=12 为
元数据——短 cast 按 P1 短招惯例走**三段式**标准域（instant 分类仅收 cast≥40
无窗招），endTick = cast(12) + recovery 6 = 18 ∈ [16,20]（原 16t/48KF 密度
不足，附录 A B 级精修）。id 不变原地重制（emit_anim 内联 :122 与
events.rs TuikeSkillVisual 双源同 id，双源治理归 bugfix plan 不动）。

母题「披壳」：俯身双手探到腿侧拾壳（弯腰走 torso+legs 补偿先例 bow_salute：
torso + / legs pitch 小负 + bend / body.z 前移）→ 双手沿腿-腹-胸一路上提，
把伪皮拉上肩 → 抖身定壳（roll 微震让壳贴合）→ 立正。

时序（精度标准 #1/#2/#3）：
  anticipation 0→6   俯身探底（torso.pitch +30 / body.y -0.14 / 双臂下探）
  strike       6→12  沿身披壳上提（t8 提至腿侧 → t10 提至胸口 → t12 拉上肩
                     定壳 + torso.roll +4 抖壳），顶点 = tick 12（cast 完成）
  recovery     12→18 反向抖壳（roll -3）→ 立正归中立（INOUTSINE）
endTick=18，stopTick=20，非循环。主打击轴：rightArm.pitch / body.y /
torso.pitch。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手。
    0: dict(
        easing="OUTSINE",
        body=dict(y=-0.01, z=0.0),
        head=dict(pitch=+3, yaw=0),
        torso=dict(pitch=+4, yaw=0, roll=0),
        rightArm=dict(pitch=-10, yaw=-4, roll=0, bend=18, axis=180),
        leftArm=dict(pitch=-8, yaw=+4, roll=0, bend=20, axis=180),
        leftLeg=dict(pitch=-4, bend=8, z=-0.02),
        rightLeg=dict(pitch=+3, bend=7, z=+0.02),
    ),
    # 俯身中段。
    3: dict(
        easing="OUTSINE",
        body=dict(y=-0.08, z=+0.05),
        head=dict(pitch=+10, yaw=0),
        torso=dict(pitch=+18, yaw=0, roll=0),
        rightArm=dict(pitch=+16, yaw=-6, roll=0, bend=14, axis=180),
        leftArm=dict(pitch=+18, yaw=+6, roll=0, bend=15, axis=180),
        leftLeg=dict(pitch=-7, bend=14, z=-0.03),
        rightLeg=dict(pitch=-6, bend=13, z=+0.02),
    ),
    # 探底顶点：弯腰到底、双手探至腿侧壳缘。
    6: dict(
        easing="OUTSINE",
        body=dict(y=-0.14, z=+0.10),
        head=dict(pitch=+14, yaw=0),
        torso=dict(pitch=+30, yaw=0, roll=0),
        rightArm=dict(pitch=+35, yaw=-8, roll=+3, bend=12, axis=180),
        leftArm=dict(pitch=+38, yaw=+8, roll=-3, bend=13, axis=180),
        leftLeg=dict(pitch=-10, bend=20, z=-0.04),
        rightLeg=dict(pitch=-9, bend=19, z=+0.03),
    ),
    # 披壳一段：提至腿侧、身开始直。
    8: dict(
        easing="INQUAD",
        body=dict(y=-0.10, z=+0.07),
        head=dict(pitch=+10, yaw=0),
        torso=dict(pitch=+20, yaw=0, roll=0),
        rightArm=dict(pitch=+8, yaw=-10, roll=+4, bend=35, axis=180),
        leftArm=dict(pitch=+10, yaw=+10, roll=-4, bend=37, axis=180),
        leftLeg=dict(pitch=-8, bend=16, z=-0.03),
        rightLeg=dict(pitch=-7, bend=15, z=+0.02),
    ),
    # 披壳二段：提至胸口。
    10: dict(
        easing="INQUAD",
        body=dict(y=-0.05, z=+0.03),
        head=dict(pitch=+5, yaw=0),
        torso=dict(pitch=+9, yaw=0, roll=0),
        rightArm=dict(pitch=-42, yaw=-16, roll=+6, bend=70, axis=180),
        leftArm=dict(pitch=-40, yaw=+16, roll=-6, bend=72, axis=180),
        leftLeg=dict(pitch=-5, bend=10, z=-0.02),
        rightLeg=dict(pitch=-4, bend=9, z=+0.02),
    ),
    # 定壳顶点（tick 12 = cast 完成）：拉上肩领、抖身贴合。
    12: dict(
        easing="INQUAD",
        body=dict(y=-0.02, z=0.0),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+4, yaw=0, roll=+4),
        rightArm=dict(pitch=-70, yaw=-24, roll=+10, bend=85, axis=180),
        leftArm=dict(pitch=-68, yaw=+24, roll=-10, bend=87, axis=180),
        leftLeg=dict(pitch=-3, bend=8, z=-0.02),
        rightLeg=dict(pitch=+2, bend=7, z=+0.01),
    ),
    # 反向抖壳：壳面落定。
    14: dict(
        easing="INOUTSINE",
        body=dict(y=-0.01, z=0.0),
        head=dict(pitch=+1, yaw=0),
        torso=dict(pitch=+2, yaw=0, roll=-3),
        rightArm=dict(pitch=-40, yaw=-14, roll=+5, bend=50, axis=180),
        leftArm=dict(pitch=-38, yaw=+14, roll=-5, bend=52, axis=180),
        leftLeg=dict(pitch=-2, bend=6, z=-0.01),
        rightLeg=dict(pitch=+2, bend=5, z=+0.01),
    ),
    # 立正过渡。
    16: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=+1, yaw=0, roll=+1),
        rightArm=dict(pitch=-14, yaw=-5, roll=+2, bend=18, axis=180),
        leftArm=dict(pitch=-13, yaw=+5, roll=-2, bend=19, axis=180),
        leftLeg=dict(pitch=-1, bend=3, z=-0.01),
        rightLeg=dict(pitch=+1, bend=2, z=+0.01),
    ),
    # 归中立。
    18: dict(
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
        name="tuike_don_skin",
        description=(
            "P3 着壳重制（18t 三段式，原 16t 密度补齐）：俯身探底（torso.pitch "
            "+30 / body.y -0.14，bow 补偿）→ 双手沿腿-胸-肩披壳上提（顶点=t12 "
            "cast 完成 + roll +4 抖壳）→ 反向抖壳立正归中立。"
        ),
        end_tick=18,
        stop_tick=20,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
