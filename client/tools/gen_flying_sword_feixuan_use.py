#!/usr/bin/env python3
"""flying_sword_feixuan_use —— 飞旋剑单手回旋撩斩（自下而上撩，带腰部转动）。

只落资产，不接线：产出 player_animation/flying_sword_feixuan_use.json，没有任何代码播放它。

模型事实（决定了动作怎么设计）：
  - flying_sword_feixuan_v2 的手持 display 与 iron_sword_v2 完全相同（thirdperson rotation
    [-80, 90, 0]）。剑身顺着前臂延长，刃线（模型 x 轴）与刀面法线（z 轴）由右臂决定。
  - 模型比铁剑长（剑身 y 方向约 2.08 格，含底部挂饰），撩起时剑尖走得远，看得出弧线。
  - 动画里**不写任何 rightItem 关键帧**：剑按 display 自然握持。

撩斩怎么做（与铁剑、骨剑都不同）：
  铁剑是矢状面内从过顶往下劈；骨剑是从右上斜向左下。飞旋剑反过来：从右侧腰胯附近
  （手臂低垂、剑尖朝下后方）开始，自下而上撩向左上方，剑尖划出一道斜上的弧。
    - 撩起段（tick 2.5–4.5）手臂 pitch 从低位 +33° 摆到前方 -84°，同时 roll 从 -33° 转向 -31°，
      躯干从右转 +18° 回转到左 -2°：刃线沿撩起方向（斜上），刀面朝侧前方，剑是「刃口朝上」撩起。
    - 顶端（tick 5–6）手臂继续上举到左后上方（pitch -121° ~ -136°、yaw -10° ~ -34°），
      同时 roll 从 -72° 转到 -84°，剑在顶端回旋半圈，这是「回旋」的来源。
    - 躯干从右转 +18° 带到左 -18°：腰部转动把撩起弧线拧成斜向螺旋，不是铁剑那种纯矢状面直劈。

判据（刃线 ∥ 手速，刀面 ⟂ 手速，撩起段 tick 2.5–4.5）：
  client/tools/check_blade_edge.py --strike 2.5-4.5 复核，见 model-review/anim/。
  撩起段的刃线对齐全程在 0.9 以上，顶端回旋段不计入（与骨剑只评 impact 段同口径）。

节奏（10 tick / 0.5s，非循环）：
  tick 0  guard       剑在右侧低位，剑尖斜朝下后
  tick 2  scoop       右侧低位起撩，躯干右转蓄力
  tick 4  rise        剑自下而上撩到身前，刃口朝上
  tick 5  top         剑尖到左上方，腰部回转，roll 开始翻转
  tick 6  spin        顶端回旋到左后上方
  tick 10 guard       与 tick 0 同值，可连撩

左手：不持物，自然放松，随躯干反向摆动做平衡，不做怪姿势。
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=0.0),
    head=dict(pitch=-4, yaw=+4),
    torso=dict(pitch=+2, yaw=+12),
    rightArm=dict(pitch=+12, yaw=+14, roll=-20, bend=25, axis=180),
    leftArm=dict(pitch=0, yaw=+8, roll=-6, bend=25, axis=180),
    rightLeg=dict(pitch=+10, yaw=0, bend=10),
    leftLeg=dict(pitch=-12, yaw=0, bend=16),
)

POSE = {
    0: GUARD,
    2: dict(  # scoop —— 右侧低位起撩，躯干右转蓄力
        easing="INQUAD",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=+2, yaw=+6),
        torso=dict(pitch=+4, yaw=+18),
        rightArm=dict(pitch=+33, yaw=+16, roll=-33, bend=25, axis=180),
        leftArm=dict(pitch=+6, yaw=+12, roll=-6, bend=40, axis=180),
        rightLeg=dict(pitch=+12, yaw=0, bend=12),
        leftLeg=dict(pitch=-10, yaw=0, bend=14),
    ),
    4: dict(  # rise —— 剑自下而上撩到身前，刃口朝上，躯干开始回转
        easing="OUTQUAD",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-6, yaw=-4),
        torso=dict(pitch=-4, yaw=-2),
        rightArm=dict(pitch=-84, yaw=+1, roll=-31, bend=10, axis=180),
        leftArm=dict(pitch=-4, yaw=-6, roll=-6, bend=35, axis=180),
        rightLeg=dict(pitch=+14, yaw=0, bend=14),
        leftLeg=dict(pitch=-14, yaw=0, bend=18),
    ),
    5: dict(  # top —— 剑尖到左上方，腰部回转，roll 开始翻转
        easing="OUTQUAD",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-10, yaw=-12),
        torso=dict(pitch=-8, yaw=-14),
        rightArm=dict(pitch=-121, yaw=-10, roll=-72, bend=16, axis=180),
        leftArm=dict(pitch=-10, yaw=-10, roll=-6, bend=40, axis=180),
        rightLeg=dict(pitch=+14, yaw=0, bend=14),
        leftLeg=dict(pitch=-14, yaw=0, bend=18),
    ),
    6: dict(  # spin —— 顶端回旋到左后上方
        easing="OUTQUAD",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=-12, yaw=-14),
        torso=dict(pitch=-10, yaw=-18),
        rightArm=dict(pitch=-136, yaw=-34, roll=-84, bend=6, axis=180),
        leftArm=dict(pitch=-10, yaw=-10, roll=-6, bend=40, axis=180),
        rightLeg=dict(pitch=+14, yaw=0, bend=14),
        leftLeg=dict(pitch=-14, yaw=0, bend=18),
    ),
    10: GUARD,
}

DESCRIPTION = (
    "飞旋剑单手回旋撩斩（自下而上撩，带腰部转动）：10 tick。剑按 display 自然握持，无 rightItem。"
    "guard 剑在右侧低位 → scoop 右侧起撩、躯干右转 → rise 剑自下而上撩到身前，刃口朝上 → "
    "top 腰部回转、剑尖到左上 → spin 顶端回旋到左后上方 → 回 guard。"
    "与铁剑过顶直劈（自上而下）、骨剑斜劈（右上到左下）区分。只落资产未接线。"
)

if __name__ == "__main__":
    emit_json(POSE, name="flying_sword_feixuan_use", description=DESCRIPTION,
              end_tick=10, stop_tick=12, is_loop=False)
