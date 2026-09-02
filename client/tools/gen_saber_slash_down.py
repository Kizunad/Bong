#!/usr/bin/env python3
"""saber_slash_down — 青铜单刀专属单手顺步下劈斩。

设计：右手持刀，orthodox 正架（左脚前）。过肩蓄势 → 沉身劈斩。

三段式结构：
  - anticipation (0→1): 反向蓄势，torso 微后仰
  - windup (1→3): 举刀过肩，右臂高举，torso 扭转蓄力
  - strike (3→6): kinetic chain 爆发，沉身重劈
  - overshoot (6→7): 刀身过冲，略低于目标线
  - recovery (7→10): 收刀回 guard

帧点：
  tick 0 = guard（持刀在右侧中段）
  tick 1 = anticipation（torso 微后仰，刀未动）
  tick 3 = windup peak（刀举过肩，torso 扭转到极限）
  tick 5 = strike（沉身劈下，impact 前一帧）
  tick 6 = impact（刀锋到位）
  tick 7 = overshoot（刀身过冲 +10°）
  tick 10 = guard return（回到 tick 0）
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

# 单位：角度用度数（emit 时转弧度），xyz 用米
POSE_V2 = {
    # ═══ GUARD: 持刀在右侧中段，左手前引护胸 ═══
    0: dict(
        easing="INOUTSINE",
        body=dict(x=+0.03, y=-0.02, z=+0.00),
        head=dict(pitch=-4, yaw=-6),
        torso=dict(pitch=+3, yaw=+12),
        # 右臂：持刀在身侧，肘微弯
        rightArm=dict(pitch=-50, yaw=-18, roll=+12, bend=30, axis=180),
        # 左臂：前引护胸，load-snap 基准姿态
        leftArm=dict(pitch=-45, yaw=+18, roll=-15, bend=90, axis=180),
        rightLeg=dict(pitch=-10, yaw=+3, bend=10, z=+0.04),
        leftLeg=dict(pitch=+8, yaw=+3, bend=12, z=-0.12),
    ),

    # ═══ ANTICIPATION: 反向蓄势，只有 torso/body/head 做反向 ═══
    1: dict(
        easing="OUTSINE",
        body=dict(x=+0.05, y=+0.01, z=-0.03),
        head=dict(pitch=-6, yaw=-8),
        torso=dict(pitch=-2, yaw=+18),  # torso 微后仰、右肩略后
        # 发力肢（右臂）保持 guard 姿态，不反向
        rightArm=dict(pitch=-50, yaw=-18, roll=+12, bend=30, axis=180),
        # 左臂：load-snap 微放松（反相位）
        leftArm=dict(pitch=-42, yaw=+20, roll=-12, bend=85, axis=180),
        rightLeg=dict(pitch=-12, yaw=+3, bend=12, z=+0.05),
        leftLeg=dict(pitch=+10, yaw=+3, bend=14, z=-0.13),
    ),

    # ═══ WINDUP: 举刀过肩，torso 扭转到极限 ═══
    3: dict(
        easing="INQUAD",
        body=dict(x=+0.06, y=-0.03, z=-0.06),
        head=dict(pitch=-8, yaw=-4),
        torso=dict(pitch=-5, yaw=+28),  # torso 扭转到蓄力顶峰
        # 右臂：举刀过肩（pitch -90°），肘展开
        rightArm=dict(pitch=-90, yaw=-25, roll=+18, bend=45, axis=180),
        # 左臂：load-snap 微放松持续
        leftArm=dict(pitch=-40, yaw=+22, roll=-10, bend=80, axis=180),
        rightLeg=dict(pitch=-15, yaw=+4, bend=15, z=+0.06),
        leftLeg=dict(pitch=+12, yaw=+4, bend=18, z=-0.14),
    ),

    # ═══ STRIKE: kinetic chain 爆发，沉身下劈（impact 前一帧）═══
    5: dict(
        easing="INQUAD",
        body=dict(x=-0.02, y=-0.04, z=+0.14),  # 前冲 + 沉身
        head=dict(pitch=+8, yaw=+6),
        torso=dict(pitch=+15, yaw=-18),  # torso 从 +28 扭到 -18（46° 爆发扭转）
        # 右臂：劈下中途，pitch 从 -90 到 +35（125° 弧）
        rightArm=dict(pitch=+35, yaw=+8, roll=-12, bend=18, axis=180),
        # 左臂：load-snap 猛收紧（反相位，counter-pull）
        leftArm=dict(pitch=-52, yaw=-15, roll=+18, bend=110, axis=180),
        rightLeg=dict(pitch=-6, yaw=+2, bend=22, z=+0.03),
        leftLeg=dict(pitch=+18, yaw=+2, bend=8, z=-0.16),
    ),

    # ═══ IMPACT: 刀锋到位，重劈命中线 ═══
    6: dict(
        easing="OUTQUAD",
        body=dict(x=-0.05, y=-0.03, z=+0.18),
        head=dict(pitch=+10, yaw=+8),
        torso=dict(pitch=+18, yaw=-22),
        # 右臂：impact 姿态，pitch +48°（比 strike 多 13°）
        rightArm=dict(pitch=+48, yaw=+12, roll=-15, bend=12, axis=180),
        # 左臂：收紧持续
        leftArm=dict(pitch=-55, yaw=-18, roll=+20, bend=115, axis=180),
        rightLeg=dict(pitch=-4, yaw=+2, bend=24, z=+0.02),
        leftLeg=dict(pitch=+20, yaw=+2, bend=6, z=-0.18),
    ),

    # ═══ OVERSHOOT: 刀身过冲 +10°，弹性物理 ═══
    7: dict(
        easing="OUTQUAD",
        body=dict(x=-0.06, y=-0.02, z=+0.16),
        head=dict(pitch=+11, yaw=+9),
        torso=dict(pitch=+20, yaw=-24),
        # 右臂：overshoot，pitch +58°（比 impact 多 10°）
        rightArm=dict(pitch=+58, yaw=+14, roll=-18, bend=8, axis=180),
        # 左臂：微反弹
        leftArm=dict(pitch=-53, yaw=-16, roll=+22, bend=120, axis=180),
        rightLeg=dict(pitch=-3, yaw=+2, bend=25, z=+0.02),
        leftLeg=dict(pitch=+22, yaw=+2, bend=5, z=-0.19),
    ),

    # ═══ RECOVERY: 收刀回 guard ═══
    10: dict(
        easing="INOUTSINE",
        # 回到 tick 0 值（guard-pose 框架）
        body=dict(x=+0.03, y=-0.02, z=+0.00),
        head=dict(pitch=-4, yaw=-6),
        torso=dict(pitch=+3, yaw=+12),
        rightArm=dict(pitch=-50, yaw=-18, roll=+12, bend=30, axis=180),
        leftArm=dict(pitch=-45, yaw=+18, roll=-15, bend=90, axis=180),
        rightLeg=dict(pitch=-10, yaw=+3, bend=10, z=+0.04),
        leftLeg=dict(pitch=+8, yaw=+3, bend=12, z=-0.12),
    ),
}

DESCRIPTION_V2 = (
    "青铜单刀顺步重劈：guard-pose 框架（tick 0==10），"
    "三段式（anticipation 0→1 torso 反向 / windup 1→3 举刀过肩 / strike 3→6 沉身劈下），"
    "kinetic chain 错峰（torso +28°→-22° = 50° 扭矩 / rightArm -90°→+58° = 148° 弧），"
    "左臂 load-snap 反相（LOAD 放松 bend 80° / IMPACT 收紧 bend 115°），"
    "overshoot 6→7 刀身过冲 +10°。"
)

if __name__ == "__main__":
    emit_json(
        POSE_V2,
        name="saber_slash_down",
        description=DESCRIPTION_V2,
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
