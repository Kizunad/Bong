#!/usr/bin/env python3
"""pickaxe_iron_v2_use —— 铁镐（v2 模型）双手抡镐刨地 + 撬。

只落资产，不接线：产出 player_animation/pickaxe_iron_v2_use.json，没有任何代码播放它。

镐和斧的区别在**落点**和**收尾**：
  - 镐尖要刨进脚前的地面 / 矿壁下沿，所以落点比斧低得多：手臂 pitch 落到 -22°，
    镐头朝下前方；躯干弯腰 +26°（torso 与腿各自 pitch，body.z 前移补偿，§库坑）；
  - 刨进去之后有一段**撬**：手臂回抬 12°、躯干后仰一点，像把矿石撬松；
  - 14 tick，双手握柄（左手在下，yaw 内收）。

display 把镐柄摆成顺着前臂延长；镐头宽 17px，横在柄顶端。

节奏：
  tick 0  guard       镐斜提在身前
  tick 4  windup      双手举镐过头
  tick 7  impact      镐尖刨进脚前地面，弯腰下沉
  tick 8  overshoot   镐尖再下 5°
  tick 10 pry         撬：手臂回抬、躯干微起
  tick 14 guard
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=0.0, y=0.0, z=0.0),
    head=dict(pitch=+2, yaw=-4),
    torso=dict(pitch=+6, yaw=+6),
    rightArm=dict(pitch=-40, yaw=-14, roll=+6, bend=28, axis=180),
    leftArm=dict(pitch=-44, yaw=+30, roll=-10, bend=34, axis=180),
    rightLeg=dict(pitch=+10, yaw=+4, bend=12, z=+0.04),
    leftLeg=dict(pitch=-12, yaw=+4, bend=16, z=-0.10),
)

POSE = {
    0: GUARD,
    4: dict(  # windup
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.04, z=-0.04),
        head=dict(pitch=-10, yaw=-2),
        torso=dict(pitch=-8, yaw=+4),
        rightArm=dict(pitch=-168, yaw=-8, roll=+2, bend=22, axis=180),
        leftArm=dict(pitch=-155, yaw=+20, roll=-4, bend=28, axis=180),
        rightLeg=dict(pitch=+10, yaw=+4, bend=8, z=+0.04),
        leftLeg=dict(pitch=-12, yaw=+4, bend=10, z=-0.10),
    ),
    7: dict(  # impact —— 刨进脚前地面
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.18, z=+0.10),
        head=dict(pitch=+22, yaw=-2),
        torso=dict(pitch=+26, yaw=+2),
        rightArm=dict(pitch=-22, yaw=-10, roll=+4, bend=6, axis=180),
        leftArm=dict(pitch=-26, yaw=+24, roll=-8, bend=12, axis=180),
        rightLeg=dict(pitch=+16, yaw=+4, bend=40, z=+0.05),
        leftLeg=dict(pitch=-22, yaw=+4, bend=46, z=-0.12),
    ),
    8: dict(  # overshoot
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.19, z=+0.10),
        head=dict(pitch=+23, yaw=-2),
        torso=dict(pitch=+28, yaw=+2),
        rightArm=dict(pitch=-17, yaw=-10, roll=+4, bend=8, axis=180),
        leftArm=dict(pitch=-21, yaw=+24, roll=-8, bend=14, axis=180),
        rightLeg=dict(pitch=+16, yaw=+4, bend=42, z=+0.05),
        leftLeg=dict(pitch=-22, yaw=+4, bend=48, z=-0.12),
    ),
    10: dict(  # pry —— 撬
        easing="INOUTSINE",
        body=dict(x=0.0, y=+0.14, z=+0.07),
        head=dict(pitch=+16, yaw=-2),
        torso=dict(pitch=+20, yaw=+4),
        rightArm=dict(pitch=-34, yaw=-12, roll=+4, bend=20, axis=180),
        leftArm=dict(pitch=-36, yaw=+26, roll=-8, bend=26, axis=180),
        rightLeg=dict(pitch=+14, yaw=+4, bend=34, z=+0.05),
        leftLeg=dict(pitch=-20, yaw=+4, bend=38, z=-0.11),
    ),
    14: GUARD,
}

DESCRIPTION = (
    "铁镐 v2 双手刨地：guard 镐斜提身前 → windup 双手举镐过头 → "
    "impact 镐尖刨进脚前地面(-22°)、弯腰 +26°、下沉 0.18 → overshoot → pry 撬松回抬 12° → 回 guard。"
    "落点比斧低，多一段撬。只落资产未接线。"
)

if __name__ == "__main__":
    emit_json(POSE, name="pickaxe_iron_v2_use", description=DESCRIPTION,
              end_tick=14, stop_tick=16, is_loop=False)
