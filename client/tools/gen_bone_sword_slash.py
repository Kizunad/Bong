#!/usr/bin/env python3
"""bone_sword_slash —— 骨剑攻击（横切）。

只落资产，不接线：产出 player_animation/bone_sword_slash.json。

骨剑特征：骨制，轻而脆，比铁剑短约 20%，刃薄锋利但怕磕碰。
攻击类型：**水平横切**——剑从右后侧拉回，再绕身体竖轴横扫到身前左侧，刃口朝前。
与 iron_sword_v2_use 的「过顶直劈」区分：骨剑是横着扫过去，铁剑是竖着劈下来。

设计要点（按 docs/player-animation-conventions.md 的反僵硬规则）：
  - **横切走的是 yaw，不是 pitch**：手臂先在身前平举（pitch≈-80°），横切靠 yaw 从右（+）
    扫到左（-）。上一版用 pitch 从下往上抬，在三视图里看起来是竖挥，不是横切。
  - **峰值错开**：躯干（torso）先转，手臂的 yaw 比躯干晚一拍到峰，腕部（rightItem）最后翻。
  - **辅助肢有动作**：左手在蓄势时拉回、横切时猛收（反相，load-snap），不全程静止。
  - **过冲与回收**：impact 之后 overshoot 再多扫 ~10°，回 guard 用 INOUTSINE 慢收。
  - **单手**：右手持剑，左手只做平衡。

节奏：
  tick 0  guard       剑在右前侧，刃朝前上，左手前伸平衡
  tick 2  windup      右臂拉回右后侧，刃朝后，躯干右转蓄势
  tick 5  sweep       手臂回到身前中线，躯干开始左转
  tick 7  impact      横切到身前左侧，刃口朝前，躯干左转到位
  tick 8  overshoot   再扫 ~10°，腕部翻过
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

# 绕剑刃轴的腕部翻转（度）。0 = 守势中立；横切时刃口要朝前，需要在关键帧上调。
_SPIN_GUARD = item_spin(_DISPLAY_ROTATION, _BLADE_AXIS, 0)
_SPIN_WINDUP = item_spin(_DISPLAY_ROTATION, _BLADE_AXIS, 30)
_SPIN_SWEEP = item_spin(_DISPLAY_ROTATION, _BLADE_AXIS, -20)
_SPIN_IMPACT = item_spin(_DISPLAY_ROTATION, _BLADE_AXIS, -60)
_SPIN_OVER = item_spin(_DISPLAY_ROTATION, _BLADE_AXIS, -70)

_GUARD_VALS = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=+0.02),
    head=dict(pitch=-4, yaw=+6),
    torso=dict(pitch=+4, yaw=-10),
    rightArm=dict(pitch=-55, yaw=+28, roll=+10, bend=70, axis=180),
    leftArm=dict(pitch=-58, yaw=+14, roll=-16, bend=74, axis=180),
    rightLeg=dict(pitch=+8, yaw=-3, bend=12, z=+0.04),
    leftLeg=dict(pitch=-14, yaw=+4, bend=18, z=-0.08),
    rightItem=_SPIN_GUARD,
)

POSE = {
    0: _GUARD_VALS,

    2: dict(  # windup：右臂拉回右后侧，躯干右转蓄势，左手拉回平衡
        easing="INOUTSINE",
        body=dict(x=+0.04, y=0.0, z=-0.02),
        head=dict(pitch=-3, yaw=+2),
        torso=dict(pitch=+6, yaw=-24),
        rightArm=dict(pitch=-15, yaw=+75, roll=+15, bend=95, axis=180),
        leftArm=dict(pitch=-60, yaw=+16, roll=-18, bend=70, axis=180),
        rightLeg=dict(pitch=+10, yaw=-3, bend=14, z=+0.04),
        leftLeg=dict(pitch=-16, yaw=+4, bend=22, z=-0.09),
        rightItem=_SPIN_WINDUP,
    ),

    5: dict(  # sweep：手臂回到身前中线，躯干开始左转（躯干先动）
        easing="INOUTSINE",
        body=dict(x=+0.02, y=0.0, z=+0.04),
        head=dict(pitch=-2, yaw=-2),
        torso=dict(pitch=+4, yaw=-8),
        rightArm=dict(pitch=-82, yaw=+20, roll=+5, bend=50, axis=180),
        leftArm=dict(pitch=-62, yaw=+12, roll=-16, bend=76, axis=180),
        rightLeg=dict(pitch=+10, yaw=-2, bend=16, z=+0.04),
        leftLeg=dict(pitch=-18, yaw=+4, bend=24, z=-0.10),
        rightItem=_SPIN_SWEEP,
    ),

    7: dict(  # impact：横切到身前左侧，左手猛收（反相）
        easing="OUTQUAD",
        body=dict(x=-0.03, y=+0.04, z=+0.10),
        head=dict(pitch=-1, yaw=-8),
        torso=dict(pitch=+8, yaw=+18),
        rightArm=dict(pitch=-86, yaw=-62, roll=-5, bend=25, axis=180),
        leftArm=dict(pitch=-74, yaw=+8, roll=-14, bend=84, axis=180),
        rightLeg=dict(pitch=+12, yaw=-2, bend=20, z=+0.05),
        leftLeg=dict(pitch=-20, yaw=+3, bend=30, z=-0.11),
        rightItem=_SPIN_IMPACT,
    ),

    8: dict(  # overshoot：再扫 ~10°，腕部翻过
        easing="OUTQUAD",
        body=dict(x=-0.03, y=+0.04, z=+0.10),
        head=dict(pitch=-1, yaw=-9),
        torso=dict(pitch=+8, yaw=+22),
        rightArm=dict(pitch=-88, yaw=-72, roll=-6, bend=18, axis=180),
        leftArm=dict(pitch=-70, yaw=+8, roll=-14, bend=80, axis=180),
        rightLeg=dict(pitch=+12, yaw=-2, bend=22, z=+0.05),
        leftLeg=dict(pitch=-20, yaw=+3, bend=32, z=-0.11),
        rightItem=_SPIN_OVER,
    ),

    9: _GUARD_VALS,
}

DESCRIPTION = (
    "骨剑水平横切：9 tick，单手持剑。"
    "guard 剑在右前侧、刃朝前上 → windup 右臂拉回右后侧、躯干右转 -24°、左手拉回 → "
    "sweep 手臂回到身前中线、躯干开始左转 → impact 剑横切到身前左侧（右臂 yaw -62°）、"
    "躯干反转总量 42°、左手猛收 → overshoot 再扫 ~10° → 收势。"
    "与 iron_sword_v2_use 区分：骨剑横着扫（yaw 为主），铁剑竖着劈（pitch 为主）。"
)

if __name__ == "__main__":
    emit_json(POSE, name="bone_sword_slash", description=DESCRIPTION,
              end_tick=9, stop_tick=11, is_loop=False)
