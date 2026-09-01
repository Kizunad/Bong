#!/usr/bin/env python3
"""saber_swing_horiz — 青铜单刀专属大角度破风平抹横斩。

设计：右手持刀，orthodox 正架（左脚前）。引刀向右后 → 横扫平抹。

三段式结构：
  - anticipation (0→1): 反向蓄势，torso 微前倾
  - windup (1→3): 引刀向右后，torso 扭转蓄力
  - strike (3→6): kinetic chain 爆发，横扫平抹
  - overshoot (6→7): 刀身过冲，略超目标线
  - recovery (7→10): 收刀回 guard

帧点：
  tick 0 = guard（持刀架于右胸前）
  tick 1 = anticipation（torso 微前倾，刀未动）
  tick 3 = windup peak（引刀向右后，torso 扭转到极限）
  tick 5 = strike（横扫中途，impact 前一帧）
  tick 6 = impact（刀锋到位，平抹扫过）
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
    # ═══ GUARD: 持刀架于右胸前，左手前引护胸 ═══
    0: dict(
        easing="INOUTSINE",
        body=dict(x=+0.03, y=-0.02, z=+0.00),
        head=dict(pitch=-3, yaw=-5),
        torso=dict(pitch=+2, yaw=+14),
        # 右臂：持刀架在胸前，肘弯
        rightArm=dict(pitch=-30, yaw=-22, roll=+16, bend=28, axis=180),
        # 左臂：前引护胸，load-snap 基准姿态
        leftArm=dict(pitch=-42, yaw=+16, roll=-14, bend=88, axis=180),
        rightLeg=dict(pitch=-9, yaw=+2, bend=9, z=+0.04),
        leftLeg=dict(pitch=+7, yaw=+2, bend=11, z=-0.12),
    ),

    # ═══ ANTICIPATION: 反向蓄势，只有 torso/body/head 做反向 ═══
    1: dict(
        easing="OUTSINE",
        body=dict(x=+0.05, y=+0.01, z=-0.02),
        head=dict(pitch=-5, yaw=-7),
        torso=dict(pitch=+5, yaw=+18),  # torso 微前倾、右肩略前
        # 发力肢（右臂）保持 guard 姿态，不反向
        rightArm=dict(pitch=-30, yaw=-22, roll=+16, bend=28, axis=180),
        # 左臂：load-snap 微放松（反相位）
        leftArm=dict(pitch=-40, yaw=+18, roll=-12, bend=83, axis=180),
        rightLeg=dict(pitch=-11, yaw=+2, bend=11, z=+0.05),
        leftLeg=dict(pitch=+9, yaw=+2, bend=13, z=-0.13),
    ),

    # ═══ WINDUP: 引刀向右后，torso 扭转到极限 ═══
    3: dict(
        easing="INQUAD",
        body=dict(x=+0.06, y=-0.02, z=-0.05),
        head=dict(pitch=-6, yaw=-3),
        torso=dict(pitch=-3, yaw=+32),  # torso 扭转到蓄力顶峰
        # 右臂：引刀向右后（yaw -50°），肘展开
        rightArm=dict(pitch=-18, yaw=-50, roll=+28, bend=38, axis=180),
        # 左臂：load-snap 微放松持续
        leftArm=dict(pitch=-38, yaw=+20, roll=-10, bend=78, axis=180),
        rightLeg=dict(pitch=-14, yaw=+3, bend=14, z=+0.06),
        leftLeg=dict(pitch=+11, yaw=+3, bend=16, z=-0.14),
    ),

    # ═══ STRIKE: kinetic chain 爆发，横扫平抹（impact 前一帧）═══
    5: dict(
        easing="INQUAD",
        body=dict(x=-0.02, y=-0.03, z=+0.12),  # 前冲 + 略沉身
        head=dict(pitch=+5, yaw=+8),
        torso=dict(pitch=+10, yaw=-24),  # torso 从 +32 扭到 -24（56° 爆发扭转）
        # 右臂：横扫中途，yaw 从 -50 到 +42（92° 弧）
        rightArm=dict(pitch=+12, yaw=+42, roll=-14, bend=16, axis=180),
        # 左臂：load-snap 猛收紧（反相位，counter-pull）
        leftArm=dict(pitch=-50, yaw=-20, roll=+20, bend=108, axis=180),
        rightLeg=dict(pitch=-7, yaw=+2, bend=20, z=+0.03),
        leftLeg=dict(pitch=+16, yaw=+2, bend=8, z=-0.16),
    ),

    # ═══ IMPACT: 刀锋到位，平抹扫过目标线 ═══
    6: dict(
        easing="OUTQUAD",
        body=dict(x=-0.04, y=-0.02, z=+0.15),
        head=dict(pitch=+7, yaw=+10),
        torso=dict(pitch=+12, yaw=-28),
        # 右臂：impact 姿态，yaw +50°（比 strike 多 8°）
        rightArm=dict(pitch=+16, yaw=+50, roll=-16, bend=12, axis=180),
        # 左臂：收紧持续
        leftArm=dict(pitch=-53, yaw=-24, roll=+22, bend=113, axis=180),
        rightLeg=dict(pitch=-5, yaw=+2, bend=22, z=+0.02),
        leftLeg=dict(pitch=+18, yaw=+2, bend=6, z=-0.18),
    ),

    # ═══ OVERSHOOT: 刀身过冲 +10°，弹性物理 ═══
    7: dict(
        easing="OUTQUAD",
        body=dict(x=-0.05, y=-0.01, z=+0.14),
        head=dict(pitch=+8, yaw=+11),
        torso=dict(pitch=+14, yaw=-30),
        # 右臂：overshoot，yaw +60°（比 impact 多 10°）
        rightArm=dict(pitch=+18, yaw=+60, roll=-18, bend=8, axis=180),
        # 左臂：微反弹
        leftArm=dict(pitch=-51, yaw=-22, roll=+24, bend=118, axis=180),
        rightLeg=dict(pitch=-4, yaw=+2, bend=23, z=+0.02),
        leftLeg=dict(pitch=+20, yaw=+2, bend=5, z=-0.19),
    ),

    # ═══ RECOVERY: 收刀回 guard ═══
    10: dict(
        easing="INOUTSINE",
        # 回到 tick 0 值（guard-pose 框架）
        body=dict(x=+0.03, y=-0.02, z=+0.00),
        head=dict(pitch=-3, yaw=-5),
        torso=dict(pitch=+2, yaw=+14),
        rightArm=dict(pitch=-30, yaw=-22, roll=+16, bend=28, axis=180),
        leftArm=dict(pitch=-42, yaw=+16, roll=-14, bend=88, axis=180),
        rightLeg=dict(pitch=-9, yaw=+2, bend=9, z=+0.04),
        leftLeg=dict(pitch=+7, yaw=+2, bend=11, z=-0.12),
    ),
}

DESCRIPTION_V2 = (
    "青铜单刀大角度横斩：guard-pose 框架（tick 0==10），"
    "三段式（anticipation 0→1 torso 反向 / windup 1→3 引刀右后 / strike 3→6 横扫平抹），"
    "kinetic chain 错峰（torso +32°→-28° = 60° 扭矩 / rightArm yaw -50°→+60° = 110° 弧），"
    "左臂 load-snap 反相（LOAD 放松 bend 78° / IMPACT 收紧 bend 113°），"
    "overshoot 6→7 刀身过冲 +10°。"
)

if __name__ == "__main__":
    emit_json(
        POSE_V2,
        name="saber_swing_horiz",
        description=DESCRIPTION_V2,
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
