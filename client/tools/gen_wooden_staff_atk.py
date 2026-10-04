#!/usr/bin/env python3
"""wooden_staff_atk —— 木杖攻击（截击）。

只落资产，不接线：产出 player_animation/wooden_staff_atk.json。

木杖特征：双手持、杖身约 1.6m（比玩家稍高），重量介于轻兵器和重兵器之间。
攻击类型：**斜向截击**（从右肩高斜扫到左腰低位），借杖长制造扫击弧线，
而非垂直挑刺（那会和骨剑过于相似）。

设计要点：
  - **双手持杖**：右手在近握把端，左手贴在杖身中段偏上（离右手约 6px，即 0.375 格），
    每个关键帧左手都由 `tools/render_block_figure.py` 的手持链路反解过：左手前臂末端中心
    与杖身轴线的距离在关键帧上 ≤ 1.5px。左臂 pitch 不与右臂同步，读作「滑握」动作。
  - **借杖长**：身体微侧转、手臂展开幅度大，展现杖身长度带来的宽扫弧。
  - **中速**：11 tick，比兵甲重拳(13)快、比缠手拳(8/9)慢，居中定位。
  - 使用 item_spin 辅助骨：由外部调用方决定，JSON 里只驱动躯干和手臂。

节奏：
  tick 0  guard       双手持杖，杖头在右肩上方（蓄势姿态）
  tick 2  windup      右臂上举、左臂拉伸，躯干右转蓄势
  tick 5  apex        杖头到达最高点（右上角），躯干右转顶点
  tick 8  impact      杖从右上扫到左下，躯干完成反转
  tick 9  overshoot   继续扫过左腰位
  tick 11 guard       收势，杖回到肩上守势
"""

from anim_common import emit_json

_GUARD_VALS = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=+0.02),
    head=dict(pitch=-4, yaw=+8),
    torso=dict(pitch=+4, yaw=-16),            # 侧身站，右肩朝前
    # 右手近握把端，高位守着（杖头在右肩上方）
    rightArm=dict(pitch=-130, yaw=+10, roll=+14, bend=56, axis=180),
    # 左手贴杖身中段偏上（见文件头「双手持杖」），由杖轴反解
    leftArm=dict(pitch=-165.0, yaw=+55.5, roll=-80.0, bend=20.0, axis=180),
    rightLeg=dict(pitch=+8, yaw=-4, bend=12, z=+0.04),
    leftLeg=dict(pitch=-16, yaw=+5, bend=22, z=-0.10),
)

POSE = {
    0: _GUARD_VALS,

    2: dict(  # windup：右臂上举，左臂拉伸，躯干右转
        easing="INOUTSINE",
        body=dict(x=+0.04, y=0.0, z=-0.02),
        head=dict(pitch=-2, yaw=+4),
        torso=dict(pitch=+2, yaw=-28),            # 右转加深
        rightArm=dict(pitch=-152, yaw=+14, roll=+18, bend=42, axis=180),
        leftArm=dict(pitch=-173.4, yaw=+49.2, roll=-82.0, bend=20.0, axis=180),
        rightLeg=dict(pitch=+10, yaw=-4, bend=14, z=+0.04),
        leftLeg=dict(pitch=-18, yaw=+4, bend=24, z=-0.11),
    ),

    5: dict(  # apex：杖头到达最高点，躯干右转顶点
        easing="INOUTSINE",
        body=dict(x=+0.06, y=-0.02, z=-0.04),
        head=dict(pitch=+2, yaw=-2),
        torso=dict(pitch=-2, yaw=-36),            # 右转顶点
        rightArm=dict(pitch=-166, yaw=+16, roll=+22, bend=34, axis=180),
        leftArm=dict(pitch=-180.0, yaw=+47.0, roll=-84.4, bend=20.0, axis=180),
        rightLeg=dict(pitch=+10, yaw=-3, bend=12, z=+0.04),
        leftLeg=dict(pitch=-16, yaw=+4, bend=20, z=-0.10),
    ),

    8: dict(  # impact：杖从右上扫到左下，双臂展开压下
        easing="OUTQUAD",
        body=dict(x=-0.04, y=+0.08, z=+0.12),
        head=dict(pitch=+4, yaw=-10),
        torso=dict(pitch=+14, yaw=+22),           # 反转总量 58°
        # 右臂随杖头扫到左下，pitch 大幅降低
        rightArm=dict(pitch=-58, yaw=-6, roll=+6, bend=24, axis=180),
        # 左手随杖身一起扫向左前上，仍在杖身中段偏上
        leftArm=dict(pitch=-135.2, yaw=+85.5, roll=-71.2, bend=20.0, axis=180),
        rightLeg=dict(pitch=+14, yaw=-3, bend=28, z=+0.06),
        leftLeg=dict(pitch=-22, yaw=+4, bend=36, z=-0.12),
    ),

    9: dict(  # overshoot：继续扫过，杖头过左腰
        easing="OUTQUAD",
        body=dict(x=-0.04, y=+0.09, z=+0.12),
        head=dict(pitch=+4, yaw=-11),
        torso=dict(pitch=+15, yaw=+26),
        rightArm=dict(pitch=-42, yaw=-8, roll=+4, bend=28, axis=180),
        leftArm=dict(pitch=-120.5, yaw=+90.0, roll=-60.0, bend=20.0, axis=180),
        rightLeg=dict(pitch=+14, yaw=-3, bend=30, z=+0.06),
        leftLeg=dict(pitch=-22, yaw=+4, bend=38, z=-0.12),
    ),

    11: _GUARD_VALS,
}

DESCRIPTION = (
    "木杖斜向截击：11 tick。双手持杖（右手近握把端、左手贴杖身中段偏上，离右手约 0.375 格）。"
    "guard 杖头在右肩上方守势、躯干右侧身 -16° → "
    "windup 右臂上举(-152°)、躯干右转 -28° → apex 杖头到最高点(-166°)、右转顶点 -36° → "
    "impact 杖从右上扫到左下（右臂 -58°/左臂 -135°，两手都贴杖）、躯干反转总量 58° → "
    "overshoot 过扫 → 收回守势。"
    "识别特征：双手宽开持杖、大弧度扫击路径、两臂高低差异。"
)

if __name__ == "__main__":
    emit_json(POSE, name="wooden_staff_atk", description=DESCRIPTION,
              end_tick=11, stop_tick=13, is_loop=False)
