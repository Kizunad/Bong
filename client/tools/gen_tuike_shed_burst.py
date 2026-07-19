#!/usr/bin/env python3
"""tuike_shed_burst —— 蜕壳：裹身紧缩→炸开甩壳→抖落（P3 批次三精修重制）。

通道核验（P3 第一性原理，2026-07-19）：`cast_shed`
（server/src/combat/tuike_v2/skills.rs:131）resolver 立即结算——`spend_qi`
（:153）+ `shed_outer_layer`（:156）+ set_cooldown（:160），零 Casting/零引导
窗，cast_ticks=8 为元数据——短 cast 走**三段式**标准域，endTick = cast(8) +
recovery 5 = 13 ∈ [12,16]（原 12t/56KF 密度可精修，附录 A B 级）。id 不变
原地重制（emit_anim :174 与 events.rs 双源同 id，双源治理归 bugfix plan）。

母题「炸壳」：双臂裹身紧缩（壳内蓄劲）→ 猛然炸开甩壳（双臂外后甩 + 挺胸
后仰 + 微跳）→ 左右抖落残壳 → 立定。与着壳（俯身上提披壳）方向相反成对。

时序（精度标准 #1/#2/#3）：
  anticipation 0→4   裹身紧缩（双臂交叉抱肩 bend 95 / torso.pitch +12 含胸）
  strike       4→8   炸开甩壳（双臂 yaw ∓55 外后甩 / torso.pitch -8 挺胸 /
                     body.y +0.03 微跳），顶点 = tick 8（cast 完成 = 壳离体）
  recovery     8→13  左右抖落（torso.yaw +6→-5）归中立（INOUTSINE）
endTick=13，stopTick=15，非循环。主打击轴：rightArm.yaw / leftArm.yaw /
torso.pitch。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 起手。
    0: dict(
        easing="OUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+4, yaw=0, roll=0),
        rightArm=dict(pitch=-25, yaw=+10, roll=-4, bend=50, axis=180),
        leftArm=dict(pitch=-22, yaw=-10, roll=+4, bend=52, axis=180),
        leftLeg=dict(pitch=-5, bend=8, z=-0.02),
        rightLeg=dict(pitch=+4, bend=7, z=+0.02),
    ),
    # 裹身中段：臂收拢过胸。
    2: dict(
        easing="OUTSINE",
        body=dict(y=-0.03, z=-0.01),
        head=dict(pitch=+6, yaw=0),
        torso=dict(pitch=+9, yaw=0, roll=0),
        rightArm=dict(pitch=-35, yaw=+24, roll=-8, bend=78, axis=180),
        leftArm=dict(pitch=-32, yaw=-22, roll=+8, bend=80, axis=180),
        leftLeg=dict(pitch=-6, bend=11, z=-0.03),
        rightLeg=dict(pitch=+5, bend=10, z=+0.02),
    ),
    # 紧缩顶点：抱肩裹死、含胸闭气。
    4: dict(
        easing="OUTSINE",
        body=dict(y=-0.05, z=-0.02),
        head=dict(pitch=+9, yaw=0),
        torso=dict(pitch=+12, yaw=0, roll=0),
        rightArm=dict(pitch=-40, yaw=+35, roll=-10, bend=95, axis=180),
        leftArm=dict(pitch=-37, yaw=-33, roll=+10, bend=97, axis=180),
        leftLeg=dict(pitch=-7, bend=13, z=-0.03),
        rightLeg=dict(pitch=+6, bend=12, z=+0.03),
    ),
    # 炸开中段：臂拆开加速外甩。
    6: dict(
        easing="INQUAD",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+2, yaw=0, roll=0),
        rightArm=dict(pitch=-30, yaw=-20, roll=+8, bend=40, axis=180),
        leftArm=dict(pitch=-27, yaw=+22, roll=-8, bend=42, axis=180),
        leftLeg=dict(pitch=-8, bend=10, z=-0.04),
        rightLeg=dict(pitch=+7, bend=9, z=+0.03),
    ),
    # 炸壳顶点（tick 8 = cast 完成 = 壳离体）：双臂外后甩满、挺胸微跳。
    8: dict(
        easing="INQUAD",
        body=dict(y=+0.03, z=+0.01),
        head=dict(pitch=-8, yaw=0),
        torso=dict(pitch=-8, yaw=0, roll=0),
        rightArm=dict(pitch=-18, yaw=-55, roll=+18, bend=14, axis=180),
        leftArm=dict(pitch=-15, yaw=+55, roll=-18, bend=16, axis=180),
        leftLeg=dict(pitch=-10, bend=8, z=-0.05),
        rightLeg=dict(pitch=+9, bend=7, z=+0.04),
    ),
    # 抖落 A：右抖。
    10: dict(
        easing="INOUTSINE",
        body=dict(y=+0.01, z=0.0),
        head=dict(pitch=-3, yaw=+3),
        torso=dict(pitch=-3, yaw=+6, roll=+2),
        rightArm=dict(pitch=-12, yaw=-30, roll=+10, bend=12, axis=180),
        leftArm=dict(pitch=-10, yaw=+32, roll=-10, bend=14, axis=180),
        leftLeg=dict(pitch=-6, bend=7, z=-0.03),
        rightLeg=dict(pitch=+5, bend=6, z=+0.03),
    ),
    # 抖落 B：左抖回。
    11: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=-1, yaw=-2),
        torso=dict(pitch=-1, yaw=-5, roll=-2),
        rightArm=dict(pitch=-8, yaw=-16, roll=+5, bend=8, axis=180),
        leftArm=dict(pitch=-6, yaw=+18, roll=-5, bend=10, axis=180),
        leftLeg=dict(pitch=-4, bend=5, z=-0.02),
        rightLeg=dict(pitch=+3, bend=4, z=+0.02),
    ),
    # 归中立。
    13: dict(
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
        name="tuike_shed_burst",
        description=(
            "P3 蜕壳重制（13t 三段式，原 12t 密度补齐）：裹身紧缩（抱肩 bend 95 "
            "含胸）→ 炸开甩壳（yaw ∓55 外后甩 / 挺胸 -8 / body.y +0.03 微跳，"
            "顶点=t8 cast 完成=壳离体）→ 左右抖落归中立。"
        ),
        end_tick=13,
        stop_tick=15,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
