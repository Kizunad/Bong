#!/usr/bin/env python3
"""hit_recoil — 被击退缩反应。

身法节奏（6 tick / 0.3s，反应类动画）：
  tick 0 neutral  正常站姿 (非 guard pose)
  tick 2 HIT      身体被冲击后仰 + 头后甩 + 后退 + 双臂护身 + 膝微屈
  tick 3 REBOUND  小幅回弹 (真实被击会弹一下再回)
  tick 6 recover  归中立

反僵硬要点：
  - 反应类动画 **冲击在开头 tick 2**，不是攻击类的 tick 5 —— 这决定了整个节奏感
  - 没有 anticipation (被击不蓄力), 只有 tick 2 的猛然停滞 + tick 3 的小回弹 + tick 6 的缓慢复位
  - 头 pitch -22° 后甩比躯干 -16° 更剧烈 —— 符合真实被击惯性 (头更轻先动)
  - body.x +0.04 轻微偏移代表被打歪了一下，body.z -0.14 身体整体后退
  - 双臂 pitch -30° bend=65° axis=180° 自动收回护身 (本能反应)
  - 全程 OUTQUAD (第一段) / INOUTSINE (回落) —— 冲击要"猛停"而不是"柔顺"
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=0, bend=0, axis=180),
        leftArm=dict(pitch=0, bend=0, axis=180),
        rightLeg=dict(bend=0),
        leftLeg=dict(bend=0),
    ),
    2: dict(  # HIT —— 被击冲击
        easing="OUTQUAD",
        body=dict(x=+0.04, y=-0.06, z=-0.14),
        head=dict(pitch=-22, yaw=+10),  # 头后甩 + 偏一下
        torso=dict(pitch=-16, yaw=+6),
        rightArm=dict(pitch=-32, yaw=-8, bend=65, axis=180),
        leftArm=dict(pitch=-32, yaw=+8, bend=65, axis=180),
        rightLeg=dict(bend=22),
        leftLeg=dict(bend=22),
    ),
    3: dict(  # REBOUND —— 小回弹
        easing="OUTQUAD",
        body=dict(x=+0.02, y=-0.04, z=-0.10),
        head=dict(pitch=-14, yaw=+6),
        torso=dict(pitch=-10, yaw=+4),
        rightArm=dict(pitch=-26, yaw=-6, bend=55, axis=180),
        leftArm=dict(pitch=-26, yaw=+6, bend=55, axis=180),
        rightLeg=dict(bend=18),
        leftLeg=dict(bend=18),
    ),
    6: dict(  # recover —— 归零
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=0, bend=0, axis=180),
        leftArm=dict(pitch=0, bend=0, axis=180),
        rightLeg=dict(bend=0),
        leftLeg=dict(bend=0),
    ),
}

DESCRIPTION = (
    "v1 JSON 受击: 6 tick 反应类 (冲击在 tick 2), head 后甩 -22° 比躯干 -16° 更剧烈, "
    "body.z 后退 -0.14 + x 偏移 +0.04 打歪, 双臂本能护身 bend 65° axis 180°, "
    "tick 3 小回弹 + tick 6 归零。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="hit_recoil",
        description=DESCRIPTION,
        end_tick=6,
        stop_tick=8,
        is_loop=False,
    )
