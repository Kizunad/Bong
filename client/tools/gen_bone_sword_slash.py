#!/usr/bin/env python3
"""bone_sword_slash —— 骨剑攻击（弧切）。

只落资产，不接线：产出 player_animation/bone_sword_slash.json。

骨剑特征：骨制，轻而脆，比铁剑短约 20%，刃薄锋利但怕磕碰。
攻击类型：**侧弧切**（右下角起，向左前上弧切），与
  iron_sword_v2_use 的「过顶直劈」区分——铁剑从头顶劈，骨剑从腰侧弧出。

设计要点：
  - **单手侧握**：右手持剑，左手辅助平衡（不搭手）。
  - **弧切路径**：剑身从右腰后侧向左前上方画弧，而非竖向（过顶）。
    torso.yaw 从右转开始，随弧切旋转到左转完成。
  - **骨剑轻盈**：全程 9 tick（铁剑典型 12+），动作快但弧度长。
  - **使用 item_spin 旋转剑骨**：windup 时 rightItem.y=+30（刃朝后下），
    impact 时 rightItem.y=-60（刃翻到上前）。

节奏：
  tick 0  guard       剑尖在前方，右腰侧持剑，left arm 平衡前展
  tick 2  windup      右臂拉回右后腰，剑刃朝后下，躯干右转蓄势
  tick 5  sweep_start 右臂开始由后向前上弧出
  tick 7  impact      剑刃到达左前方（弧切顶点），躯干完成反转
  tick 8  overshoot   剑头再弧过 12°
  tick 9  guard       收势
"""

from pathlib import Path
import json as _json
from anim_common import emit_json, item_spin

# 骨剑模型的 thirdperson_righthand.rotation，局部 +Y 轴为剑刃朝向
_MODEL_JSON = (Path(__file__).resolve().parents[1]
               / "src/main/resources/assets/bong/models/item/bone_sword_v2/bone_sword_v2.json")
_DISPLAY_ROTATION = _json.loads(_MODEL_JSON.read_text(encoding="utf-8"))["display"]["thirdperson_righthand"]["rotation"]
_BLADE_AXIS = (0.0, 1.0, 0.0)  # 局部 +Y = 剑刃朝向

# 各阶段绕剑刃轴的旋转量（弧切动作中，手腕带动剑刃翻面）
_SPIN_GUARD = item_spin(_DISPLAY_ROTATION, _BLADE_AXIS, 0)     # 守势中立
_SPIN_WINDUP = item_spin(_DISPLAY_ROTATION, _BLADE_AXIS, 30)   # 刃朝后下
_SPIN_MID = item_spin(_DISPLAY_ROTATION, _BLADE_AXIS, -20)     # 翻转中途
_SPIN_IMPACT = item_spin(_DISPLAY_ROTATION, _BLADE_AXIS, -60)  # 刃朝前上
_SPIN_OVER = item_spin(_DISPLAY_ROTATION, _BLADE_AXIS, -70)    # 过冲

_GUARD_VALS = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=+0.02),
    head=dict(pitch=-4, yaw=+6),
    torso=dict(pitch=+4, yaw=-10),
    rightArm=dict(pitch=-70, yaw=+8, roll=+12, bend=60, axis=180),
    leftArm=dict(pitch=-58, yaw=+14, roll=-16, bend=74, axis=180),
    rightLeg=dict(pitch=+8, yaw=-3, bend=12, z=+0.04),
    leftLeg=dict(pitch=-14, yaw=+4, bend=18, z=-0.08),
    rightItem=_SPIN_GUARD,
)

POSE = {
    0: _GUARD_VALS,

    2: dict(  # windup：右臂拉回右后腰，剑刃朝后下，躯干右转
        easing="INOUTSINE",
        body=dict(x=+0.04, y=0.0, z=-0.02),
        head=dict(pitch=-3, yaw=+2),
        torso=dict(pitch=+6, yaw=-24),
        rightArm=dict(pitch=-30, yaw=-14, roll=+8, bend=88, axis=180),
        leftArm=dict(pitch=-60, yaw=+16, roll=-18, bend=70, axis=180),
        rightLeg=dict(pitch=+10, yaw=-3, bend=14, z=+0.04),
        leftLeg=dict(pitch=-16, yaw=+4, bend=22, z=-0.09),
        rightItem=_SPIN_WINDUP,
    ),

    5: dict(  # sweep_start：臂开始弧出，躯干开始反转
        easing="INQUAD",
        body=dict(x=+0.02, y=0.0, z=+0.04),
        head=dict(pitch=-2, yaw=-2),
        torso=dict(pitch=+4, yaw=-8),     # 反转中间态
        rightArm=dict(pitch=-80, yaw=-2, roll=+6, bend=46, axis=180),
        leftArm=dict(pitch=-62, yaw=+12, roll=-16, bend=76, axis=180),
        rightLeg=dict(pitch=+10, yaw=-2, bend=16, z=+0.04),
        leftLeg=dict(pitch=-18, yaw=+4, bend=24, z=-0.10),
        rightItem=_SPIN_MID,
    ),

    7: dict(  # impact：剑弧切到左前方顶点，躯干反转完成
        easing="OUTQUAD",
        body=dict(x=-0.03, y=+0.04, z=+0.10),
        head=dict(pitch=-1, yaw=-8),
        torso=dict(pitch=+8, yaw=+18),    # 总量 42°
        # 右臂向左前上方展出（弧切到顶），臂几乎平展
        rightArm=dict(pitch=-124, yaw=-18, roll=-4, bend=22, axis=180),
        leftArm=dict(pitch=-66, yaw=+8, roll=-14, bend=80, axis=180),
        rightLeg=dict(pitch=+12, yaw=-2, bend=20, z=+0.05),
        leftLeg=dict(pitch=-20, yaw=+3, bend=30, z=-0.11),
        rightItem=_SPIN_IMPACT,
    ),

    8: dict(  # overshoot：剑头再弧过 12°
        easing="OUTQUAD",
        body=dict(x=-0.03, y=+0.04, z=+0.10),
        head=dict(pitch=-1, yaw=-9),
        torso=dict(pitch=+8, yaw=+22),
        rightArm=dict(pitch=-136, yaw=-20, roll=-6, bend=20, axis=180),
        leftArm=dict(pitch=-66, yaw=+8, roll=-14, bend=80, axis=180),
        rightLeg=dict(pitch=+12, yaw=-2, bend=22, z=+0.05),
        leftLeg=dict(pitch=-20, yaw=+3, bend=32, z=-0.11),
        rightItem=_SPIN_OVER,
    ),

    9: _GUARD_VALS,
}

DESCRIPTION = (
    "骨剑侧弧切：9 tick，单手持剑。"
    "guard 剑尖朝前守势 → windup 右臂拉回右后腰、剑刃翻后下(+30°)、躯干右转 -24° → "
    "sweep 臂由后下弧向前上 → impact 剑弧切到左前方、刃翻 -60°（前上朝向）、躯干反转总量 42° → "
    "overshoot 再弧 12° → 收势。"
    "与 iron_sword_v2_use 区分：骨剑走腰侧弧切路径（横弧），铁剑为过顶直劈（纵弧）；"
    "骨剑 9 tick 更快，扭矩 42° 小于铁剑。"
)

if __name__ == "__main__":
    emit_json(POSE, name="bone_sword_slash", description=DESCRIPTION,
              end_tick=9, stop_tick=11, is_loop=False)
