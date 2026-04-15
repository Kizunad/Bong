#!/usr/bin/env python3
"""death_collapse — 道消身陨 (摇晃 → 跪地 → 前倒 → 扑地)。

**v2 修正 torso/legs hinge**: v1 从 0 → torso+90° 扑地全程腿 pitch=0, 结果扑地那帧
腿直挺挺像旗杆插地上。按 feedback_torso_legs_hinge 规则:
  - 跪 (torso+30°): 腿 pitch=-10° 髋后推 (坐骨接地)
  - 倾 (torso+55°): 腿 pitch=+30° 开始水平化
  - 扑 (torso+90°): 腿 pitch=+75° 跟进 (全身贴地水平)


身法节奏（30 tick / 1.5s, 非循环）：
  tick 0  guard    站姿
  tick 3  SWAY     摇晃 (body.x +0.06 倾歪)
  tick 5  buckle   膝盖开始屈 (bend 50°, body.y -0.20)
  tick 10 kneel    完全跪下 (腿 bend 100°, body.y -0.70, torso +30° 前倾)
  tick 18 tilt     向前倾倒 (torso +55°, body.z +0.30)
  tick 25 faceplant 完全扑地 (torso +90°, body.y -1.10, z=+0.40, 双臂前伸)
  tick 30 still    静止

**视觉主体是 body.y + torso.pitch**: y 从 0 → -1.10 (整个身体下沉 ~18px, 约 1 格),
torso.pitch 0 → 90° (从直立到水平趴地). 这两个轴承载了 80% 的信息量。其余肢体
动作只是"描边"。

反僵硬要点：
  - 3 阶段节奏断: OUTQUAD 从 swaying → buckle → kneel (重力加速感),
    INOUTSINE 25→30 static (死了不动)
  - body.x 摇晃: +0.06 → -0.04 → 0 (最后一次挣扎的反弹)
  - head pitch 缓慢低头 20° → 45° (随躯干倒下去)
  - 双臂 pitch -60° 撑地 → +60° 前伸死亡姿 (手垂在身前)
  - 腿 bend 100° 跪 → 60° 最终盘在身下
  - 没有 yaw 翻转 (身体直接前倒不侧倾)
  - axis=180° 让撑地时手肘朝前折, 不是朝身后
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, bend=0, axis=180),
        leftArm=dict(pitch=0, bend=0, axis=180),
        rightLeg=dict(pitch=0, bend=0),
        leftLeg=dict(pitch=0, bend=0),
    ),
    3: dict(  # SWAY —— 摇晃
        easing="OUTQUAD",
        body=dict(x=+0.06, y=-0.04, z=0.0),
        head=dict(pitch=+12, yaw=+6),
        torso=dict(pitch=+6, yaw=-3),
        rightArm=dict(pitch=+8, bend=10, axis=180),
        leftArm=dict(pitch=-4, bend=6, axis=180),
        rightLeg=dict(pitch=0, bend=8),
        leftLeg=dict(pitch=0, bend=4),
    ),
    5: dict(  # buckle —— 膝盖屈, 髋略后推
        easing="OUTQUAD",
        body=dict(x=+0.02, y=-0.20, z=+0.03),
        head=dict(pitch=+20, yaw=+3),
        torso=dict(pitch=+15, yaw=0),
        rightArm=dict(pitch=-10, bend=20, axis=180),
        leftArm=dict(pitch=-10, bend=20, axis=180),
        rightLeg=dict(pitch=-5, bend=50),
        leftLeg=dict(pitch=-5, bend=50),
    ),
    10: dict(  # kneel —— 跪地 (hinge: torso 前 + 腿 pitch 后)
        easing="OUTQUAD",
        body=dict(x=-0.04, y=-0.70, z=+0.10),
        head=dict(pitch=+30),
        torso=dict(pitch=+30),
        rightArm=dict(pitch=-60, bend=40, axis=180),
        leftArm=dict(pitch=-60, bend=40, axis=180),
        rightLeg=dict(pitch=-10, bend=100),
        leftLeg=dict(pitch=-10, bend=100),
    ),
    18: dict(  # tilt —— 前倾 (腿开始随身体水平化)
        easing="OUTQUAD",
        body=dict(x=-0.02, y=-0.90, z=+0.30),
        head=dict(pitch=+38),
        torso=dict(pitch=+55),
        rightArm=dict(pitch=+10, bend=20, axis=180),
        leftArm=dict(pitch=+10, bend=20, axis=180),
        rightLeg=dict(pitch=+30, bend=80),
        leftLeg=dict(pitch=+30, bend=80),
    ),
    25: dict(  # faceplant —— 扑地 (全身水平, 腿 pitch 同 torso 走向 +90°)
        easing="OUTQUAD",
        body=dict(x=0.0, y=-1.10, z=+0.40),
        head=dict(pitch=+45),
        torso=dict(pitch=+90),
        rightArm=dict(pitch=+60, bend=10, axis=180),
        leftArm=dict(pitch=+60, bend=10, axis=180),
        rightLeg=dict(pitch=+75, bend=30),
        leftLeg=dict(pitch=+75, bend=30),
    ),
    30: dict(  # still
        easing="INOUTSINE",
        body=dict(x=0.0, y=-1.10, z=+0.40),
        head=dict(pitch=+45),
        torso=dict(pitch=+90),
        rightArm=dict(pitch=+60, bend=10, axis=180),
        leftArm=dict(pitch=+60, bend=10, axis=180),
        rightLeg=dict(pitch=+75, bend=30),
        leftLeg=dict(pitch=+75, bend=30),
    ),
}

DESCRIPTION = (
    "v1 JSON 身陨: 30 tick 7 阶段, 站 → 摇 → 屈膝 → 跪 → 前倾 → 扑地 → 静, "
    "body.y 0→-1.10 下沉 + torso 0→90° 前倾 为视觉主体, body.z 0→+0.40 前扑, "
    "双臂 pitch-60° 撑地 → +60° 前伸, 腿 bend 100°→60°, axis=180°。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="death_collapse",
        description=DESCRIPTION,
        end_tick=30,
        stop_tick=33,
        is_loop=False,
    )
