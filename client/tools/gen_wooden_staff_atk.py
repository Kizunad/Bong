#!/usr/bin/env python3
"""wooden_staff_atk —— 木杖攻击（横扫 + 点戳）。

只落资产，不接线：产出 player_animation/wooden_staff_atk.json。

木杖特征：双手持、杖身约 1.4 格（v2 模型按 0.8 缩放），双手横持，杖身斜过胸前。
攻击类型：**横扫接点戳**——杖身先横在胸前、向后拉回（windup），再从身体右侧横扫到
身前（apex → impact），杖尖在 impact 指向身前左侧做点戳。

设计要点：
  - **双手横持**：右手握杖下段（握点在杖身下部），左手握杖中上段（离右手沿杖轴 6px，即 0.375 格）。
    两手都在杖轴上：每个关键帧由 `tools/render_block_figure.py` 的手持链路反解过，左手前臂末端
    中心到杖身轴线的距离 ≤ 1.5px。
  - **关节舒适范围**：两臂 pitch 绝对值 ≤ 120°，yaw / roll 绝对值 ≤ 60°，不出现扭转式姿态。
    手臂与杖的朝向都由求解器算出，握点与杖尖方向的目标值见下方「关键帧」。
  - **右手朝杖**：rightItem 关键帧绕握点转杖身，让杖尖的朝向跟着横扫走，而手的位置不跳。
  - **借杖长 / 身体转**：躯干仍然大幅右转再反转（torso），手臂在躯干之上独立摆动。
  - **中速**：11 tick，比兵甲重拳(13)快、比缠手拳(8/9)慢，居中定位。

关键帧（握点 G、杖尖方向 u，单位像素与 MC 坐标，+X 玩家左、+Y 下、+Z 背后）：
  tick 0  guard     G=(-3,4,-8)  u=(1,0,0)     杖横在胸前，杖尖指向左
  tick 2  windup    G=(-5,4,-5)  u=(1,0,0)     握点收回右侧，杖仍横放，蓄势
  tick 5  apex      G=(-4,3,-5)  u=(0.96,0,-0.29) 杖尖向前左扫，躯干右转顶点
  tick 8  impact    G=(-3,3,-3)  u=(0.5,0,-0.87)  杖尖指向身前左侧，点戳
  tick 9  overshoot G=(-3,3,-3.1) u=(0.4,0,-0.92) 点戳后的余势
  tick 11 guard     同 tick 0，收势回守
"""

from anim_common import emit_json

_GUARD_VALS = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=+0.02),
    head=dict(pitch=-4, yaw=+8),
    torso=dict(pitch=+4, yaw=-16),            # 侧身站，右肩朝前
    rightArm=dict(pitch=-52.5, yaw=-25.0, roll=+60.0, bend=48.75, axis=180),
    leftArm=dict(pitch=-40.0, yaw=+38.12, roll=-25.63, bend=70.62, axis=180),
    rightItem=dict(pitch=+20.63, yaw=+58.75, roll=-12.5),
    rightLeg=dict(pitch=+8, yaw=-4, bend=12, z=+0.04),
    leftLeg=dict(pitch=-16, yaw=+5, bend=22, z=-0.10),
)

POSE = {
    0: _GUARD_VALS,

    2: dict(  # windup：握点收回，杖仍横放，躯干右转蓄势
        easing="INOUTSINE",
        body=dict(x=+0.04, y=0.0, z=-0.02),
        head=dict(pitch=-2, yaw=+4),
        torso=dict(pitch=+2, yaw=-28),            # 右转加深
        rightArm=dict(pitch=+18.75, yaw=-26.25, roll=+52.5, bend=109.37, axis=180),
        leftArm=dict(pitch=+31.88, yaw=+48.12, roll=+58.75, bend=106.25, axis=180),
        rightItem=dict(pitch=+20.0, yaw=+67.5, roll=-10.0),
        rightLeg=dict(pitch=+10, yaw=-4, bend=14, z=+0.04),
        leftLeg=dict(pitch=-18, yaw=+4, bend=24, z=-0.11),
    ),

    5: dict(  # apex：杖尖向前左扫，躯干右转顶点
        easing="INOUTSINE",
        body=dict(x=+0.06, y=-0.02, z=-0.04),
        head=dict(pitch=+2, yaw=-2),
        torso=dict(pitch=-2, yaw=-36),            # 右转顶点
        rightArm=dict(pitch=+23.75, yaw=-8.75, roll=-16.25, bend=117.5, axis=180),
        leftArm=dict(pitch=-48.75, yaw=+56.88, roll=-46.88, bend=95.0, axis=180),
        rightItem=dict(pitch=-74.38, yaw=+49.38, roll=-40.0),
        rightLeg=dict(pitch=+10, yaw=-3, bend=12, z=+0.04),
        leftLeg=dict(pitch=-16, yaw=+4, bend=20, z=-0.10),
    ),

    8: dict(  # impact：杖尖指向身前左侧，躯干反转完成，点戳
        easing="OUTQUAD",
        body=dict(x=-0.04, y=+0.08, z=+0.12),
        head=dict(pitch=+4, yaw=-10),
        torso=dict(pitch=+14, yaw=+22),           # 反转总量 58°
        rightArm=dict(pitch=+6.88, yaw=-35.0, roll=-39.38, bend=101.87, axis=180),
        leftArm=dict(pitch=-85.0, yaw=+37.5, roll=-29.38, bend=30.0, axis=180),
        rightItem=dict(pitch=-45.0, yaw=-14.38, roll=-6.87),
        rightLeg=dict(pitch=+14, yaw=-3, bend=28, z=+0.06),
        leftLeg=dict(pitch=-22, yaw=+4, bend=36, z=-0.12),
    ),

    9: dict(  # overshoot：点戳后的余势
        easing="OUTQUAD",
        body=dict(x=-0.04, y=+0.09, z=+0.12),
        head=dict(pitch=+4, yaw=-11),
        torso=dict(pitch=+15, yaw=+26),
        rightArm=dict(pitch=-3.75, yaw=-38.12, roll=-20.62, bend=101.25, axis=180),
        leftArm=dict(pitch=-85.0, yaw=+40.0, roll=-25.0, bend=30.0, axis=180),
        rightItem=dict(pitch=-44.38, yaw=-18.75, roll=-4.38),
        rightLeg=dict(pitch=+14, yaw=-3, bend=30, z=+0.06),
        leftLeg=dict(pitch=-22, yaw=+4, bend=38, z=-0.12),
    ),

    11: _GUARD_VALS,
}

DESCRIPTION = (
    "木杖横扫接点戳：11 tick。双手横持，右手握杖下段、左手握杖中上段（离右手约 0.375 格），"
    "两臂 pitch 不超过 ±120°、yaw/roll 不超过 ±60°。"
    "guard 杖横在胸前，杖尖朝左 → "
    "windup 握点收回右侧，杖仍横放蓄势，躯干右转 -28° → "
    "apex 杖尖向前左扫，躯干右转顶点 -36° → "
    "impact 杖尖指向身前左侧点戳，躯干反转总量 58° → "
    "overshoot 点戳余势 → 收回守势。"
    "识别特征：双手横握、杖身斜过胸前，先横扫后点戳。"
)

if __name__ == "__main__":
    emit_json(POSE, name="wooden_staff_atk", description=DESCRIPTION,
              end_tick=11, stop_tick=13, is_loop=False)
