#!/usr/bin/env python3
"""rune_draw — 凌空画符 (右手在身前远处运笔描绘符文轨迹)。

身法节奏（20 tick / 1s, 非循环）：
  tick 0  guard   双臂下垂
  tick 4  ready   右手伸到身前中心 (大臂前指 + 前臂近伸直 bend=25°), 左手扶符
  tick 7  撇      笔触右上 (pitch=-72° yaw=-2° bend=22° 手伸远)
  tick 10 捺      笔触左下 + 下压 (pitch=-85° yaw=-22° bend=35° 笔锋下沉手微收)
  tick 13 中段    回正 (pitch=-78° yaw=-8° bend=25°)
  tick 16 末笔    顿 (pitch=-80° yaw=-20° bend=20° OUTQUAD 手伸最远)
  tick 20 收      回 guard

**v2 修正 (axis 不该当格斗默认)**: v1 抄 Java 用 bend=90°→100° 把前臂折回胸前,
配 axis=180° 等于"前臂在胸前 90° 弯", 完全不像画符 —— 那是格斗护胸姿。画符的物理
是手腕在远处运笔, 整条胳膊近乎伸直微指, bend 应该小 (20-35°)。

**bend 在画符中的语义**: 不是"前臂朝身前折", 而是"笔的提按":
  - bend=20° 笔提起 (末笔顿/撇结束 手伸最远)
  - bend=25° 中性运笔
  - bend=35° 笔下压 (捺笔锋下沉, 手腕略收)

反僵硬要点：
  - 大臂 pitch -72°→-85° 复合: 撇上抬, 捺下压, 模拟书法笔势
  - yaw 描绘摆动 -8° → -2° → -22° → -8° → -20° 非等幅 (人手书法节奏)
  - bend 提按 25°→22°→35°→25°→20° 微变 (笔尖触感)
  - 末笔 OUTQUAD + bend 最小 = 收尾"顿住"且笔尖伸到最远
  - 左臂扶符: pitch=-60° bend=70° yaw=+22° 中等抬起 (不画但姿态稳, 整段不动)
  - head pitch +16°→+22° 眼随笔走略点头
  - axis=180° 仅取"剩余 bend 朝身前略弯"的方向, 不是主导 —— 主导是 pitch+yaw 末端轨迹
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(y=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        rightLeg=dict(bend=0),
        leftLeg=dict(bend=0),
    ),
    4: dict(  # ready —— 右手伸到身前中心 (近伸直)
        easing="INOUTSINE",
        body=dict(y=-0.02),
        head=dict(pitch=+16),
        torso=dict(pitch=-2),
        rightArm=dict(pitch=-78, yaw=-8, roll=-3, bend=25, axis=180),
        leftArm=dict(pitch=-60, yaw=+22, roll=+8, bend=70, axis=180),
        rightLeg=dict(bend=4),
        leftLeg=dict(bend=4),
    ),
    7: dict(  # 撇 —— 笔触右上, 手伸更远
        easing="INOUTSINE",
        body=dict(y=-0.03),
        head=dict(pitch=+18),
        torso=dict(pitch=-3),
        rightArm=dict(pitch=-72, yaw=-2, roll=-2, bend=22, axis=180),
        leftArm=dict(pitch=-60, yaw=+22, roll=+8, bend=70, axis=180),
        rightLeg=dict(bend=4),
        leftLeg=dict(bend=4),
    ),
    10: dict(  # 捺 —— 笔触左下 + 笔锋下压, 手微收
        easing="INOUTSINE",
        body=dict(y=-0.04),
        head=dict(pitch=+22),
        torso=dict(pitch=-4),
        rightArm=dict(pitch=-85, yaw=-22, roll=-6, bend=35, axis=180),
        leftArm=dict(pitch=-60, yaw=+22, roll=+8, bend=70, axis=180),
        rightLeg=dict(bend=6),
        leftLeg=dict(bend=6),
    ),
    13: dict(  # 中段回正
        easing="INOUTSINE",
        body=dict(y=-0.03),
        head=dict(pitch=+18),
        torso=dict(pitch=-3),
        rightArm=dict(pitch=-78, yaw=-8, roll=-3, bend=25, axis=180),
        leftArm=dict(pitch=-60, yaw=+22, roll=+8, bend=70, axis=180),
        rightLeg=dict(bend=4),
        leftLeg=dict(bend=4),
    ),
    16: dict(  # 末笔顿 —— 手伸最远 + bend 最小
        easing="OUTQUAD",
        body=dict(y=-0.03),
        head=dict(pitch=+20),
        torso=dict(pitch=-3),
        rightArm=dict(pitch=-80, yaw=-20, roll=-4, bend=20, axis=180),
        leftArm=dict(pitch=-60, yaw=+22, roll=+8, bend=70, axis=180),
        rightLeg=dict(bend=4),
        leftLeg=dict(bend=4),
    ),
    20: dict(  # 收手回 guard
        easing="INOUTSINE",
        body=dict(y=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        rightLeg=dict(bend=0),
        leftLeg=dict(bend=0),
    ),
}

DESCRIPTION = (
    "v2 JSON 画符: 20 tick 非循环, 右臂近伸直 (bend 20°-35° 提按) 远处运笔, "
    "pitch -72°→-85° 撇/捺笔势 + yaw -2°→-22° 描绘轨迹, 末笔 OUTQUAD 顿在最远, "
    "左臂扶符 pitch-60° bend70° 静止, head 眼随笔 +16°→+22°。"
    "v2 修正: v1 误用 bend=90° axis=180° (前臂折回胸前像格斗护胸) 不像画符。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="rune_draw",
        description=DESCRIPTION,
        end_tick=20,
        stop_tick=23,
        is_loop=False,
    )
