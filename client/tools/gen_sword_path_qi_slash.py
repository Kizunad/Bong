#!/usr/bin/env python3
"""sword_path_qi_slash —— 剑气斩：大开大合远程挥斩，斩出后剑随气送远（P2 批次二前半）。

cast_ticks=20，去复用（原借 sword_thrust）。对拍区间 endTick ∈ [24,28]，取 26。

母题：远程剑气。比基础 thrust 更舒展——高位大回环蓄势（剑扬过肩后、深拧腰），
一记大斩挥出后手臂随剑气**送远定格在前伸位**（目送剑气远去），再收势。

时序（精度标准 #1/#2/#3）：
  anticipation 0→10  高位回环蓄势：右臂扬至过肩后（pitch -60→-148）、拧腰
                     -27°、重心后坐（easeOut 族 OUTSINE）
  strike       10→20 大斩挥出+送远：剑过身前挥至全伸展（bend 40→4）、躯干
                     开至 +16、body.z 前冲 +0.30（easeIn 族 INQUAD），
                     顶点 = tick 20（cast 完成瞬间，剑随气送远的全伸展位）
  recovery     20→26 由送远位收剑回 guard（INOUTSINE，t23 中段帧）
endTick=26，stopTick=28，非循环。主打击轴：rightArm.pitch / torso.yaw / body.z。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

# 单手剑 guard：右手剑前下位、左臂平衡护中线。
GUARD = dict(
    easing="INOUTSINE",
    body=dict(y=0.0, z=0.0),
    head=dict(pitch=-3, yaw=0),
    torso=dict(pitch=+4, yaw=-6),
    rightArm=dict(pitch=-60, yaw=-10, roll=+18, bend=40, axis=180),
    leftArm=dict(pitch=-40, yaw=+15, roll=-15, bend=50, axis=180),
    leftLeg=dict(pitch=-10, bend=12, z=-0.05),
    rightLeg=dict(pitch=+8, bend=10, z=+0.04),
)

# 送远顶点（tick 20 = cast 完成瞬间）：右臂全伸展前送、躯干前压、弓步前冲。
APEX = dict(
    easing="INQUAD",
    body=dict(y=+0.02, z=+0.30),
    head=dict(pitch=-2, yaw=0),
    torso=dict(pitch=+12, yaw=+16),
    rightArm=dict(pitch=-78, yaw=+6, roll=+2, bend=4, axis=180),
    leftArm=dict(pitch=-10, yaw=+4, roll=-26, bend=58, axis=180),
    leftLeg=dict(pitch=-30, bend=28, z=-0.13),
    rightLeg=dict(pitch=+22, bend=30, z=+0.09),
)

POSE = {
    0: GUARD,
    # 回环起手：剑向肩后扬起，拧腰启动。
    4: dict(
        easing="OUTSINE",
        body=dict(y=-0.03, z=-0.06),
        head=dict(pitch=-5, yaw=-8),
        torso=dict(pitch=-2, yaw=-16),
        rightArm=dict(pitch=-95, yaw=-22, roll=+24, bend=48, axis=180),
        leftArm=dict(pitch=-30, yaw=+26, roll=-18, bend=42, axis=180),
        leftLeg=dict(pitch=-8, bend=14, z=-0.05),
        rightLeg=dict(pitch=+11, bend=18, z=+0.06),
    ),
    # 回环近顶：剑过肩后、深拧腰。
    8: dict(
        easing="OUTSINE",
        body=dict(y=-0.06, z=-0.10),
        head=dict(pitch=-9, yaw=-10),
        torso=dict(pitch=-7, yaw=-24),
        rightArm=dict(pitch=-135, yaw=-30, roll=+18, bend=42, axis=180),
        leftArm=dict(pitch=-22, yaw=+30, roll=-20, bend=36, axis=180),
        leftLeg=dict(pitch=-6, bend=16, z=-0.04),
        rightLeg=dict(pitch=+13, bend=22, z=+0.07),
    ),
    # 蓄势顶点：剑扬到极限、拧腰 -27°、重心后坐最深。
    10: dict(
        easing="OUTSINE",
        body=dict(y=-0.07, z=-0.12),
        head=dict(pitch=-10, yaw=-11),
        torso=dict(pitch=-9, yaw=-27),
        rightArm=dict(pitch=-148, yaw=-32, roll=+14, bend=40, axis=180),
        leftArm=dict(pitch=-20, yaw=+32, roll=-22, bend=34, axis=180),
        leftLeg=dict(pitch=-5, bend=16, z=-0.04),
        rightLeg=dict(pitch=+14, bend=24, z=+0.08),
    ),
    # 大斩加速过身。
    14: dict(
        easing="INQUAD",
        body=dict(y=-0.02, z=+0.04),
        head=dict(pitch=0, yaw=-4),
        torso=dict(pitch=+4, yaw=-8),
        rightArm=dict(pitch=-80, yaw=-10, roll=+10, bend=22, axis=180),
        leftArm=dict(pitch=-30, yaw=+18, roll=-20, bend=40, axis=180),
        leftLeg=dict(pitch=-16, bend=18, z=-0.08),
        rightLeg=dict(pitch=+12, bend=18, z=+0.06),
    ),
    # 斩出近顶：剑已过中线向前送，左臂后摆抵消。
    17: dict(
        easing="INQUAD",
        body=dict(y=0.0, z=+0.18),
        head=dict(pitch=+2, yaw=-2),
        torso=dict(pitch=+9, yaw=+8),
        rightArm=dict(pitch=-70, yaw=-2, roll=+6, bend=12, axis=180),
        leftArm=dict(pitch=-14, yaw=+8, roll=-24, bend=52, axis=180),
        leftLeg=dict(pitch=-24, bend=24, z=-0.11),
        rightLeg=dict(pitch=+18, bend=26, z=+0.08),
    ),
    # 送远顶点 = cast 完成（tick 20）。
    20: APEX,
    # 收剑中段：由全伸展拉回一半。
    23: dict(
        easing="INOUTSINE",
        body=dict(y=+0.01, z=+0.15),
        head=dict(pitch=-2, yaw=0),
        torso=dict(pitch=+8, yaw=+6),
        rightArm=dict(pitch=-68, yaw=-4, roll=+10, bend=24, axis=180),
        leftArm=dict(pitch=-25, yaw=+10, roll=-20, bend=52, axis=180),
        leftLeg=dict(pitch=-20, bend=20, z=-0.09),
        rightLeg=dict(pitch=+14, bend=20, z=+0.07),
    ),
    # 收势回 guard。
    26: inherit(GUARD),
}


def main() -> int:
    emit_json(
        POSE,
        name="sword_path_qi_slash",
        description=(
            "P2 剑气斩专属：anticipation 0→10 高位回环蓄势（rightArm.pitch "
            "-60→-148 / torso.yaw -27 / body.z -0.12），strike 10→20 大斩挥出"
            "+送远（bend 40→4 全伸展 / torso 开至 +16 / body.z +0.30 前冲），"
            "recovery 20→26 经 t23 中段帧收剑回 guard。"
        ),
        end_tick=26,
        stop_tick=28,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
