#!/usr/bin/env python3
"""anqi_armor_pierce —— 破甲注射：旋钻蓄力→螺旋贯刺（P2 批次二后半）。

cast_ticks=40，**瞬发结算型长 cast**（`resolve_anqi_skill` 立即结算、无引导窗，
plan 附录 A 决策 (b)，与 echo_fractal 同型）：46t 非循环单段专属演出——0→40
覆盖 cast 元数据期，贯刺顶点 = tick 40，recovery 40→46。时长对拍 allowlist
条目保留（cast≥40 机械断言要求 isLoop，通道真实化后再落两段式）。去复用：
原借通用施法 cast_invoke，本批解除。

母题：旋钻贯甲。双手在右腰侧合握骨镖做「旋钻」蓄力（roll 往复拧转、躯干
反向盘紧 yaw→-24，钻头意象），蓄满后一记带螺旋的平直贯刺（右臂前送 roll
+30→-25 翻拧、torso.yaw -24→+18 甩转、弓步 body.z +0.22），命中定格后收。
与凝魂注射（面前举镖下压）/ 单射（侧身鞭甩）动向完全区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→32  腰侧旋钻蓄力：4t 步进 roll 往复（±12 渐涨）、躯干渐盘紧
                     （yaw 0→-24）、重心渐沉（OUTSINE 族）
  strike       32→40 螺旋贯刺：35 拉镖引弓 → 38 前送半程 → 40 贯刺顶点
                     （rightArm pitch -85 / roll -25 / torso.yaw +18 / body.z +0.22，
                     INQUAD），顶点 = cast 完成瞬间
  recovery     40→46 由贯刺位撤臂直身回中立（INOUTSINE，t43 中段帧）
endTick=46，stopTick=48，非循环。主打击轴：rightArm.pitch / rightArm.roll /
torso.yaw / body.z（全程 ≤4t 帧距）。
"""

from __future__ import annotations

import math

from anim_common import emit_json


def drill_frame(t: int) -> dict:
    """旋钻蓄力期（0→32）某 tick 的拧转姿态。

    双手右腰侧合握，roll 以 8t 周期往复拧转（幅度随 t 渐涨），躯干反向
    盘紧（yaw 线性 0→-24）、重心渐沉。
    """
    ramp = 0.5 + 0.5 * (t / 32.0)
    coil = t / 32.0
    s = math.sin(2.0 * math.pi * (t % 8) / 8.0)
    return dict(
        easing="OUTSINE",
        body=dict(x=+0.02 * coil, y=-0.02 - 0.035 * coil, z=-0.02 - 0.03 * coil),
        head=dict(pitch=+6 + 4 * coil, yaw=-8 * coil),
        torso=dict(pitch=+4 + 4 * coil, yaw=-24 * coil),
        rightArm=dict(
            pitch=-48 - 6 * coil,
            yaw=-26 - 6 * coil,
            roll=+10 + 12 * s * ramp,
            bend=70 + 10 * coil,
            axis=180,
        ),
        leftArm=dict(
            pitch=-42 - 5 * coil,
            yaw=+8 - 10 * coil,
            roll=-8 - 10 * s * ramp,
            bend=62 + 12 * coil,
            axis=180,
        ),
        leftLeg=dict(pitch=-6 - 6 * coil, bend=8 + 8 * coil, z=-0.03 - 0.03 * coil),
        rightLeg=dict(pitch=+5 + 5 * coil, bend=7 + 7 * coil, z=+0.03 + 0.02 * coil),
    )


# 旋钻蓄力：0→32 每 4t 一帧（主轴密度 ≤4t 机械保证）。
POSE = {t: drill_frame(t) for t in range(0, 33, 4)}

POSE.update(
    {
        # 拉镖引弓：镖手后引到腰后、盘至最紧（贯刺前的反向拉满）。
        35: dict(
            easing="INQUAD",
            body=dict(x=+0.03, y=-0.06, z=-0.06),
            head=dict(pitch=+8, yaw=-10),
            torso=dict(pitch=+7, yaw=-28),
            rightArm=dict(pitch=-40, yaw=-38, roll=+26, bend=86, axis=180),
            leftArm=dict(pitch=-52, yaw=+2, roll=-20, bend=78, axis=180),
            leftLeg=dict(pitch=-13, bend=17, z=-0.06),
            rightLeg=dict(pitch=+11, bend=15, z=+0.05),
        ),
        # 前送半程：螺旋展开、躯干开始甩转。
        38: dict(
            easing="INQUAD",
            body=dict(x=0.0, y=-0.03, z=+0.10),
            head=dict(pitch=+4, yaw=+2),
            torso=dict(pitch=+9, yaw=-2),
            rightArm=dict(pitch=-70, yaw=-14, roll=+2, bend=34, axis=180),
            leftArm=dict(pitch=-30, yaw=+18, roll=-14, bend=46, axis=180),
            leftLeg=dict(pitch=-18, bend=20, z=-0.08),
            rightLeg=dict(pitch=+14, bend=17, z=+0.06),
        ),
        # 贯刺顶点 = cast 完成瞬间（tick 40）：右臂平直全伸、roll 反拧到底、弓步前压。
        40: dict(
            easing="INQUAD",
            body=dict(x=-0.02, y=-0.02, z=+0.22),
            head=dict(pitch=+2, yaw=+6),
            torso=dict(pitch=+12, yaw=+18),
            rightArm=dict(pitch=-85, yaw=-8, roll=-25, bend=4, axis=180),
            leftArm=dict(pitch=-18, yaw=+22, roll=-10, bend=52, axis=180),
            leftLeg=dict(pitch=-24, bend=24, z=-0.11),
            rightLeg=dict(pitch=+18, bend=22, z=+0.07),
        ),
        # 收势中段：撤臂、直身。
        43: dict(
            easing="INOUTSINE",
            body=dict(x=-0.01, y=-0.01, z=+0.10),
            head=dict(pitch=+2, yaw=+2),
            torso=dict(pitch=+6, yaw=+8),
            rightArm=dict(pitch=-45, yaw=-10, roll=-8, bend=24, axis=180),
            leftArm=dict(pitch=-12, yaw=+14, roll=-6, bend=30, axis=180),
            leftLeg=dict(pitch=-14, bend=15, z=-0.07),
            rightLeg=dict(pitch=+11, bend=13, z=+0.05),
        ),
        # 归中立。
        46: dict(
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
)


def main() -> int:
    emit_json(
        POSE,
        name="anqi_armor_pierce",
        description=(
            "P2 破甲注射专属（46t 非循环，瞬发结算型长 cast 决策 (b)，解除 "
            "cast_invoke 借用）：anticipation 0→32 腰侧旋钻蓄力（roll 8t 往复拧转 "
            "±12 渐涨 / torso.yaw 盘紧 0→-24），strike 32→40 拉镖引弓→螺旋贯刺"
            "（pitch -85 / roll +26→-25 翻拧 / torso.yaw -28→+18 / body.z +0.22），"
            "recovery 40→46 撤臂回中立。"
        ),
        end_tick=46,
        stop_tick=48,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
