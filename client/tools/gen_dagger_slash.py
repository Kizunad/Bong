#!/usr/bin/env python3
"""dagger_slash — 采药刀应急防身短促挥割。

设计：右手持采药刀，短促急速横划防身。
采药刀不是战斗武器，所以：
  - 肘全程不伸直（impact 时 bend 仍 ≈60°，对比战刀 bend ≈10°）
  - 刀弧主要由手臂给，腰扭只为读感（torso 扭转幅度适中）
  - 动作短促（8 tick），强调快速应急

三段式结构：
  - anticipation (0→1): 反向蓄势，torso 微后仰
  - windup (1→3): 引刀向右后
  - strike (3→6): 横划挥出
  - overshoot (6→7): 刀身过冲
  - recovery (7→8): 快速回 guard

帧点：
  tick 0 = guard（持刀在身前下方）
  tick 1 = anticipation（torso 反向）
  tick 3 = windup（引刀右后）
  tick 5 = strike（横划中途）
  tick 6 = impact（刀锋到位）
  tick 7 = overshoot（过冲 +8°）
  tick 8 = guard return（回 tick 0）
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

# 单位：角度用度数（emit 时转弧度），xyz 用米
POSE_V2 = {
    # ═══ GUARD: 持刀在身前下方，防御姿态 ═══
    0: dict(
        easing="INOUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0),
        head=dict(pitch=-3, yaw=-6),
        torso=dict(pitch=+2, yaw=+14),
        # 右臂：持刀在身前偏下（pitch -16°，不是高举）
        rightArm=dict(pitch=-16, yaw=-14, roll=-8, bend=76, axis=180),
        # 左臂：前引护胸，load-snap 基准
        leftArm=dict(pitch=-22, yaw=+24, roll=-10, bend=98, axis=180),
        rightLeg=dict(pitch=-8, yaw=+2, bend=8, z=+0.04),
        leftLeg=dict(pitch=+6, yaw=+2, bend=10, z=-0.12),
    ),

    # ═══ ANTICIPATION: 反向蓄势，只有 torso/body/head ═══
    1: dict(
        easing="OUTSINE",
        body=dict(x=+0.04, y=+0.01, z=-0.02),
        head=dict(pitch=-5, yaw=-8),
        torso=dict(pitch=+4, yaw=+18),  # torso 微后仰
        # 右臂：保持 guard，不反向
        rightArm=dict(pitch=-16, yaw=-14, roll=-8, bend=76, axis=180),
        # 左臂：load-snap 微放松
        leftArm=dict(pitch=-20, yaw=+26, roll=-8, bend=84, axis=180),
        rightLeg=dict(pitch=-10, yaw=+2, bend=10, z=+0.05),
        leftLeg=dict(pitch=+8, yaw=+2, bend=12, z=-0.13),
    ),

    # ═══ WINDUP: 引刀向右后 ═══
    3: dict(
        easing="INQUAD",
        body=dict(x=+0.05, y=-0.01, z=-0.04),
        head=dict(pitch=-6, yaw=-4),
        torso=dict(pitch=-2, yaw=+28),  # torso 扭转蓄力
        # 右臂：引刀向右后（yaw -38°）
        rightArm=dict(pitch=-12, yaw=-38, roll=+12, bend=68, axis=180),
        # 左臂：load-snap 微放松持续
        leftArm=dict(pitch=-18, yaw=+28, roll=-6, bend=80, axis=180),
        rightLeg=dict(pitch=-12, yaw=+3, bend=12, z=+0.06),
        leftLeg=dict(pitch=+10, yaw=+3, bend=14, z=-0.14),
    ),

    # ═══ STRIKE: 横划中途（impact 前一帧）═══
    5: dict(
        easing="INQUAD",
        body=dict(x=-0.02, y=-0.02, z=+0.10),
        head=dict(pitch=+4, yaw=+6),
        torso=dict(pitch=+8, yaw=-18),  # torso 扭转爆发（+28 → -18 = 46°）
        # 右臂：横划中途（yaw 从 -38 到 +28，66° 弧）
        rightArm=dict(pitch=+8, yaw=+28, roll=-10, bend=62, axis=180),
        # 左臂：load-snap 猛收紧（反相位）
        leftArm=dict(pitch=-42, yaw=-16, roll=+16, bend=116, axis=180),
        rightLeg=dict(pitch=-6, yaw=+2, bend=18, z=+0.03),
        leftLeg=dict(pitch=+14, yaw=+2, bend=8, z=-0.16),
    ),

    # ═══ IMPACT: 刀锋到位 ═══
    6: dict(
        easing="OUTQUAD",
        body=dict(x=-0.04, y=-0.01, z=+0.12),
        head=dict(pitch=+6, yaw=+8),
        torso=dict(pitch=+10, yaw=-22),
        # 右臂：impact 姿态，yaw +36°（比 strike 多 8°）
        rightArm=dict(pitch=+10, yaw=+36, roll=-12, bend=58, axis=180),
        # 左臂：收紧持续
        leftArm=dict(pitch=-45, yaw=-18, roll=+18, bend=120, axis=180),
        rightLeg=dict(pitch=-4, yaw=+2, bend=20, z=+0.02),
        leftLeg=dict(pitch=+16, yaw=+2, bend=6, z=-0.18),
    ),

    # ═══ OVERSHOOT: 刀身过冲 +8° ═══
    7: dict(
        easing="OUTQUAD",
        body=dict(x=-0.05, y=0.0, z=+0.11),
        head=dict(pitch=+7, yaw=+9),
        torso=dict(pitch=+11, yaw=-24),
        # 右臂：overshoot，yaw +44°（比 impact 多 8°）
        rightArm=dict(pitch=+12, yaw=+44, roll=-14, bend=54, axis=180),
        # 左臂：微反弹
        leftArm=dict(pitch=-43, yaw=-16, roll=+20, bend=124, axis=180),
        rightLeg=dict(pitch=-3, yaw=+2, bend=21, z=+0.02),
        leftLeg=dict(pitch=+18, yaw=+2, bend=5, z=-0.19),
    ),

    # ═══ RECOVERY: 快速回 guard ═══
    8: dict(
        easing="INOUTSINE",
        # 回到 tick 0 值（guard-pose 框架）
        body=dict(x=+0.02, y=0.0, z=0.0),
        head=dict(pitch=-3, yaw=-6),
        torso=dict(pitch=+2, yaw=+14),
        rightArm=dict(pitch=-16, yaw=-14, roll=-8, bend=76, axis=180),
        leftArm=dict(pitch=-22, yaw=+24, roll=-10, bend=98, axis=180),
        rightLeg=dict(pitch=-8, yaw=+2, bend=8, z=+0.04),
        leftLeg=dict(pitch=+6, yaw=+2, bend=10, z=-0.12),
    ),
}

DESCRIPTION_V2 = (
    "采药刀应急防身横划：guard-pose 框架（tick 0==8），"
    "三段式短促挥击（anticipation 0→1 / windup 1→3 / strike 3→6），"
    "kinetic chain 错峰（torso +28°→-22° = 50° 扭矩 / rightArm yaw -38°→+44° = 82° 弧），"
    "肘全程不伸直（impact bend=58°，非战斗武器特征），"
    "左臂 load-snap 反相（LOAD 放松 80° / IMPACT 收紧 120°），"
    "overshoot 6→7 刀身过冲 +8°。"
)

if __name__ == "__main__":
    emit_json(
        POSE_V2,
        name="dagger_slash",
        description=DESCRIPTION_V2,
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
