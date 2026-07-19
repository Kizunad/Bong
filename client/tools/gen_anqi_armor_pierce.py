#!/usr/bin/env python3
"""anqi_armor_pierce —— 破甲注射：疾拧贯刺→钻拧余韵（P2 批次二后半，review 返工重做）。

cast_ticks=40 是**元数据**：`resolve_anqi_skill` 在 cast 起始 tick 立即结算
（anqi_v2.rs 无 `Casting`、无 timer、无打断窗）。返工前的 46t 版本把贯刺顶点
放在 tick 40，造成「伤害已落、动作两秒后才到」的表演/结算脱节（PR #1240
review blocker）。本版遵循「动画 strike 顶点对齐真实结算点」：18t 非循环，
顶点 = tick 6（紧贴 tick 0 结算），旋钻母题移入贯刺后的钻拧余韵。时长对拍
allowlist 条目保留（endTick=18 与 cast=40 元数据错配如实驻表）。

母题：疾拧贯甲。双手急收右腰侧盘紧（0→2 快速 anticipation），螺旋平直贯刺
（右臂前送 roll +26→-25 翻拧、torso.yaw -26→+18 甩转、弓步 body.z +0.22，
顶点 t6），命中后钻头在创口内往复拧转（t8/t10 roll 极值直落采样帧——修
CodeRabbit 欠采样：拧转极值显式落帧，不经正弦零点），撤臂直身回中立。
与凝魂注射（面前举镖下压）/ 单射（侧身鞭甩）动向完全区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→2   急收盘紧：双手合握收右腰侧、躯干反向盘紧 yaw→-26
  strike       2→6   螺旋贯刺：t4 前送半程 → t6 贯刺顶点（rightArm pitch -85
                     / roll -25 / torso.yaw +18 / body.z +0.22，INQUAD）
  recovery     6→18  钻拧余韵（t8 roll +8 / t10 roll -18 极值帧）→ t12 撤臂
                     → t15 直身 → t18 归中立（INOUTSINE）
endTick=18，stopTick=20，非循环。主打击轴：rightArm.pitch / rightArm.roll /
torso.yaw / body.z（全程 ≤3t 帧距）。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 急收盘紧起手：双手合握收右腰侧、重心快速下沉。
    0: dict(
        easing="OUTQUAD",
        body=dict(x=+0.02, y=-0.04, z=-0.04),
        head=dict(pitch=+7, yaw=-8),
        torso=dict(pitch=+6, yaw=-18),
        rightArm=dict(pitch=-46, yaw=-30, roll=+16, bend=78, axis=180),
        leftArm=dict(pitch=-44, yaw=+4, roll=-12, bend=68, axis=180),
        leftLeg=dict(pitch=-10, bend=13, z=-0.05),
        rightLeg=dict(pitch=+9, bend=12, z=+0.04),
    ),
    # 盘至最紧（贯刺前的反向拉满）。
    2: dict(
        easing="INQUAD",
        body=dict(x=+0.03, y=-0.06, z=-0.06),
        head=dict(pitch=+8, yaw=-10),
        torso=dict(pitch=+7, yaw=-26),
        rightArm=dict(pitch=-40, yaw=-38, roll=+26, bend=86, axis=180),
        leftArm=dict(pitch=-52, yaw=+2, roll=-20, bend=78, axis=180),
        leftLeg=dict(pitch=-13, bend=17, z=-0.06),
        rightLeg=dict(pitch=+11, bend=15, z=+0.05),
    ),
    # 前送半程：螺旋展开、躯干开始甩转。
    4: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=-0.03, z=+0.10),
        head=dict(pitch=+4, yaw=+2),
        torso=dict(pitch=+9, yaw=-2),
        rightArm=dict(pitch=-70, yaw=-14, roll=+2, bend=34, axis=180),
        leftArm=dict(pitch=-30, yaw=+18, roll=-14, bend=46, axis=180),
        leftLeg=dict(pitch=-18, bend=20, z=-0.08),
        rightLeg=dict(pitch=+14, bend=17, z=+0.06),
    ),
    # 贯刺顶点（紧贴 tick 0 结算点）：右臂平直全伸、roll 反拧到底、弓步前压。
    6: dict(
        easing="INQUAD",
        body=dict(x=-0.02, y=-0.02, z=+0.22),
        head=dict(pitch=+2, yaw=+6),
        torso=dict(pitch=+12, yaw=+18),
        rightArm=dict(pitch=-85, yaw=-8, roll=-25, bend=4, axis=180),
        leftArm=dict(pitch=-18, yaw=+22, roll=-10, bend=52, axis=180),
        leftLeg=dict(pitch=-24, bend=24, z=-0.11),
        rightLeg=dict(pitch=+18, bend=22, z=+0.07),
    ),
    # 钻拧余韵 A：臂保持贯出，roll 反向拧回极值（极值直落采样帧）。
    8: dict(
        easing="OUTSINE",
        body=dict(x=-0.02, y=-0.02, z=+0.20),
        head=dict(pitch=+2, yaw=+5),
        torso=dict(pitch=+11, yaw=+14),
        rightArm=dict(pitch=-83, yaw=-9, roll=+8, bend=6, axis=180),
        leftArm=dict(pitch=-17, yaw=+21, roll=-8, bend=50, axis=180),
        leftLeg=dict(pitch=-23, bend=23, z=-0.10),
        rightLeg=dict(pitch=+17, bend=21, z=+0.07),
    ),
    # 钻拧余韵 B：roll 再反拧（幅度衰减），钻头意象收尾。
    10: dict(
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
    12: dict(
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
    15: dict(
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
    18: dict(
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
            "P2 破甲注射专属（18t 非循环，strike 顶点 t6 对齐瞬发结算点，解除 "
            "cast_invoke 借用；cast_ticks=40 为元数据、错配如实驻 allowlist）："
            "anticipation 0→2 急收盘紧（torso.yaw→-26 / roll +26 拉满），strike "
            "2→6 螺旋贯刺（pitch -85 / roll +26→-25 翻拧 / torso.yaw +18 / "
            "body.z +0.22），recovery 6→18 钻拧余韵（t8 roll +8 / t10 roll -18 "
            "极值帧）→撤臂归中立。"
        ),
        end_tick=18,
        stop_tick=20,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
