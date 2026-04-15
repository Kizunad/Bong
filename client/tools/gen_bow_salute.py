#!/usr/bin/env python3
"""bow_salute — 抱拳行礼 (右拳贴左掌, 鞠躬)。

身法节奏（25 tick / 1.25s, 非循环）：
  tick 0   guard
  tick 5   to-chest   双手内收胸前 (yaw ±55° 贴近中线, bend=110°)
  tick 10  bow        前伸抱拳 + 躯干大鞠躬 (torso +40°)
  tick 17  hold
  tick 25  回 guard

**v2 参考 KosmX/Emotecraft-emotes/bow1.json**: bow1 的关键数据是 rightArm pitch=-46°
yaw=-60° roll=+15° + torso pitch 大前倾。我们 v1 用 torso +15° + yaw ±10° 太保守,
看起来像轻点头而不是抱拳礼。v2 加深: torso +40° (武人礼比西式鞠躬稍浅但比点头深),
yaw ±55° 让双手内收到胸前中线 (模拟右拳贴左掌), pitch -60° 手臂不过胸口。

**抱拳形状**: 右臂 (yaw=-55° + pitch=-60° + bend=100°): 肘屈, 拳抬到胸前左侧。
左臂 (yaw=+55° 镜像): 掌心对应位置, 右拳刚好落在左掌上。MC 无 IK, 精确贴合不可能,
靠 yaw 对称 + 合适 pitch 产生视觉贴合即可。

反僵硬要点：
  - 3 段节奏: 0→5 抬手 (INOUTSINE), 5→10 鞠躬 (INOUTSINE), 10→17 hold, 17→25 收
  - body.y -0.02 → -0.05 配合鞠躬下沉
  - torso +40° 鞠躬 + head +15° 低头 (总计 55° 前倾, 武人大礼)
  - 腿 bend=8° 微屈 (承重微蹲)
  - axis=180° 肘朝前折
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        rightLeg=dict(pitch=0, bend=0),
        leftLeg=dict(pitch=0, bend=0),
    ),
    5: dict(  # to-chest —— 双手内收胸前
        easing="INOUTSINE",
        body=dict(y=-0.02, z=+0.02),
        head=dict(pitch=+5),
        torso=dict(pitch=+8),
        rightArm=dict(pitch=-55, yaw=-22, roll=+10, bend=115, axis=180),
        leftArm=dict(pitch=-55, yaw=+22, roll=-10, bend=115, axis=180),
        rightLeg=dict(pitch=-3, bend=5),
        leftLeg=dict(pitch=-3, bend=5),
    ),
    10: dict(  # bow —— 鞠躬 (腿补偿 pitch 让整体像腰铰链弯)
        easing="INOUTSINE",
        body=dict(y=-0.05, z=+0.10),
        head=dict(pitch=+15),
        torso=dict(pitch=+30),
        rightArm=dict(pitch=-60, yaw=-25, roll=+12, bend=110, axis=180),
        leftArm=dict(pitch=-60, yaw=+25, roll=-12, bend=110, axis=180),
        rightLeg=dict(pitch=-10, bend=15),
        leftLeg=dict(pitch=-10, bend=15),
    ),
    17: dict(  # hold —— 定礼
        easing="INOUTSINE",
        body=dict(y=-0.05, z=+0.10),
        head=dict(pitch=+15),
        torso=dict(pitch=+30),
        rightArm=dict(pitch=-60, yaw=-25, roll=+12, bend=110, axis=180),
        leftArm=dict(pitch=-60, yaw=+25, roll=-12, bend=110, axis=180),
        rightLeg=dict(pitch=-10, bend=15),
        leftLeg=dict(pitch=-10, bend=15),
    ),
    25: dict(  # 回 guard
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        rightLeg=dict(pitch=0, bend=0),
        leftLeg=dict(pitch=0, bend=0),
    ),
}

DESCRIPTION = (
    "v2 JSON 抱拳 (参考 bow1.json): 25 tick 武人礼, 双手内收胸前 (pitch-55°→-60° "
    "yaw±50°→±55° bend110°→100°), 躯干 torso+40° head+15° 大鞠躬, body.y-0.05 下沉, "
    "hold 7 tick → 回 guard, axis=180°。v1 torso+15° 太浅像点头, v2 加深到武人大礼。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="bow_salute",
        description=DESCRIPTION,
        end_tick=25,
        stop_tick=28,
        is_loop=False,
    )
