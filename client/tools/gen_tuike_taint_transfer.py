#!/usr/bin/env python3
"""tuike_taint_transfer —— 移秽：按胸引秽→抽出推入壳层→抚平（P3 批次三精修重制）。

通道核验（P3 第一性原理，2026-07-19）：`cast_transfer_taint`
（server/src/combat/tuike_v2/skills.rs:183）resolver 立即结算——
`transfer_taint_to_outer_skin`（:208）+ drain/backflow/set_cooldown（:229-275），
零 Casting/零引导窗，cast_ticks=10 为元数据——短 cast 走**三段式**标准域，
endTick = cast(10) + recovery 5 = 15 ∈ [14,18]（原 14t/55KF 密度可精修，
附录 A B 级）。id 不变原地重制（emit_anim :291 与 events.rs 双源同 id，双源
治理归 bugfix plan）。

母题「移秽入壳」：右掌按住心口（引秽聚拢）→ 缓缓抽离胸口（把污染拉出体外，
臂带轻颤）→ 顶点猛然前推按进伪皮壳层 → 沿壳面向下抚平。按胸-抽出-前推的
胸口起点动线与着壳（腿侧上提）/ 蜕壳（裹身外甩）区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→5   按胸引秽（右掌贴心口 bend 100 / 俯首 +10 / 左臂外展平衡）
  strike       5→10  抽出前推（t7 抽离胸口带颤 roll -12 → t10 前推按入壳层
                     pitch -80 / body.z +0.10），顶点 = tick 10（cast 完成）
  recovery     10→15 沿壳面下抚、落臂归中立（INOUTSINE）
endTick=15，stopTick=17，非循环。主打击轴：rightArm.pitch / body.z /
torso.pitch。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手。
    0: dict(
        easing="OUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=+3, yaw=0),
        torso=dict(pitch=+2, yaw=0),
        rightArm=dict(pitch=-30, yaw=+8, roll=0, bend=60, axis=180),
        leftArm=dict(pitch=-12, yaw=-8, roll=0, bend=24, axis=180),
        leftLeg=dict(pitch=-4, bend=6, z=-0.02),
        rightLeg=dict(pitch=+3, bend=5, z=+0.02),
    ),
    # 按胸中段。
    2: dict(
        easing="OUTSINE",
        body=dict(y=-0.01, z=0.0),
        head=dict(pitch=+6, yaw=0),
        torso=dict(pitch=+4, yaw=+2),
        rightArm=dict(pitch=-45, yaw=+14, roll=-4, bend=85, axis=180),
        leftArm=dict(pitch=-20, yaw=-14, roll=+4, bend=28, axis=180),
        leftLeg=dict(pitch=-5, bend=7, z=-0.02),
        rightLeg=dict(pitch=+4, bend=6, z=+0.02),
    ),
    # 按胸顶点：掌根贴死心口、俯首内视。
    5: dict(
        easing="OUTSINE",
        body=dict(y=-0.02, z=-0.01),
        head=dict(pitch=+10, yaw=0),
        torso=dict(pitch=+6, yaw=+3),
        rightArm=dict(pitch=-55, yaw=+20, roll=-6, bend=100, axis=180),
        leftArm=dict(pitch=-28, yaw=-18, roll=+6, bend=30, axis=180),
        leftLeg=dict(pitch=-6, bend=9, z=-0.03),
        rightLeg=dict(pitch=+5, bend=8, z=+0.02),
    ),
    # 抽秽：掌缓缓离胸、臂带轻颤（秽物拉出体外）。
    7: dict(
        easing="INQUAD",
        body=dict(y=-0.02, z=-0.03),
        head=dict(pitch=+8, yaw=+2),
        torso=dict(pitch=+4, yaw=+5),
        rightArm=dict(pitch=-62, yaw=+12, roll=-12, bend=62, axis=180),
        leftArm=dict(pitch=-24, yaw=-16, roll=+5, bend=28, axis=180),
        leftLeg=dict(pitch=-6, bend=9, z=-0.03),
        rightLeg=dict(pitch=+5, bend=8, z=+0.02),
    ),
    # 抽出续段：秽丝将断未断。
    9: dict(
        easing="INQUAD",
        body=dict(y=-0.01, z=+0.03),
        head=dict(pitch=+5, yaw=+1),
        torso=dict(pitch=+7, yaw=+1),
        rightArm=dict(pitch=-72, yaw=+4, roll=-8, bend=35, axis=180),
        leftArm=dict(pitch=-16, yaw=-12, roll=+3, bend=24, axis=180),
        leftLeg=dict(pitch=-8, bend=10, z=-0.04),
        rightLeg=dict(pitch=+7, bend=9, z=+0.03),
    ),
    # 推入顶点（tick 10 = cast 完成）：前推按进壳层、身前送。
    10: dict(
        easing="INQUAD",
        body=dict(y=-0.01, z=+0.10),
        head=dict(pitch=+4, yaw=0),
        torso=dict(pitch=+10, yaw=-4),
        rightArm=dict(pitch=-80, yaw=-2, roll=-4, bend=12, axis=180),
        leftArm=dict(pitch=-10, yaw=-10, roll=+2, bend=20, axis=180),
        leftLeg=dict(pitch=-10, bend=12, z=-0.05),
        rightLeg=dict(pitch=+9, bend=11, z=+0.04),
    ),
    # 下抚：沿壳面向下抹平。
    12: dict(
        easing="INOUTSINE",
        body=dict(y=-0.02, z=+0.05),
        head=dict(pitch=+3, yaw=0),
        torso=dict(pitch=+6, yaw=-2),
        rightArm=dict(pitch=-40, yaw=+2, roll=-2, bend=30, axis=180),
        leftArm=dict(pitch=-6, yaw=-6, roll=+1, bend=14, axis=180),
        leftLeg=dict(pitch=-6, bend=9, z=-0.03),
        rightLeg=dict(pitch=+5, bend=8, z=+0.02),
    ),
    # 归中立。
    15: dict(
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
        name="tuike_taint_transfer",
        description=(
            "P3 移秽重制（15t 三段式，原 14t 密度补齐）：右掌按住心口引秽"
            "（bend 100 / 俯首 +10）→ 抽离胸口带颤（roll -12）→ 前推按进壳层"
            "（pitch -80 / body.z +0.10，顶点=t10 cast 完成）→ 沿壳面下抚归中立。"
        ),
        end_tick=15,
        stop_tick=17,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
