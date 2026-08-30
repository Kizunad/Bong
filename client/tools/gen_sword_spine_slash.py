#!/usr/bin/env python3
"""sword_spine_slash — 异兽脊骨剑 (BeastSpineSword) 专属单手斜斩。

## 动作

单手持剑：**起手把剑扛过右肩（剑尖朝后上）→ 过顶 → 斜劈向左前下 → 棘突咬住撕扯回抽 →
收低架**。正面看是一条从左上到右下的斜线。tick 0 是用户在 Blockbench 里手摆的起手帧，
原样保留（见下）。

| tick | 阶段 | 剑尖去处（模型坐标 px） |
|------|------|------------------------|
| 0  | 起手 GUARD  | (-14, 34, +16) 右肩后上 |
| 2  | LOAD 收肘蓄力 | (-15, 30, +15) 再沉半分 |
| 5  | 过顶        | (-3, 45, +2) 头顶正上 |
| 8  | 斩入        | (+11, 27, -17) 越过中线向左前 |
| 10 | IMPACT      | (+6, 14, -17) 左前下，腰腹发力顶点 |
| 13 | RIP PULL    | (+4, 17, -19) 倒钩咬住，提半格顿一下 |
| 18 | 收势        | (-5, 26, -22) 低架，剑指前方 |

## 为什么角度是解出来的不是手调的

握姿固定后（`gen_beast_spine_sword_player_anim` 的 `sword_right_pitch/roll` 静态角），
**剑身恒垂直于小臂**，剑尖去哪主要由 `rightArm.roll` 决定而不是 `pitch`——凭直觉调
pitch 只会让整把剑绕着拳头打转。这里的 rightArm 四轴是按上表的剑尖弧线（大圆插值）
整条一次性反解出来的，并罚了关节角的二阶差分，所以是一条连贯的弧而不是逐帧各解各的
（逐帧独立解实测会在冗余自由度上跳，相邻帧 yaw −3°→−41°→−8°，播起来手臂抽一下）。

tick 0 在反解里是**钉死**的：那是用户手摆的帧，不许被优化器改掉。

## 时序

endTick=20（含 2 tick 混出），stopTick=22，非循环。发力顶点在 tick 10。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # 起手：剑扛过右肩，剑尖朝后上（用户手摆帧，原样保留）
        easing="OUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-12),
        head=dict(pitch=-2, yaw=-6, roll=+0.7),
        torso=dict(pitch=+2, yaw=+14),
        rightArm=dict(pitch=-117.9, yaw=-3.3, roll=-17.4, bend=44.5, axis=180),
        leftArm=dict(pitch=+23.5, yaw=+28, roll=-24, bend=29.5, axis=180),
        rightLeg=dict(pitch=-18.5, yaw=+4, bend=9, axis=0),
        leftLeg=dict(pitch=+9, yaw=+4, bend=12, axis=0),
    ),
    2: dict(  # LOAD：收肘（bend 44→57）把重剑再往后沉半分，下盘同时坐低
        easing="OUTSINE",
        body=dict(x=+0.05, y=-0.02, z=-0.07, yaw=-20),
        head=dict(pitch=-5, yaw=-2),
        torso=dict(pitch=+3, yaw=+26),
        rightArm=dict(pitch=-117.0, yaw=-7.2, roll=-7.6, bend=57.2, axis=180),
        leftArm=dict(pitch=+32, yaw=+34, roll=-28, bend=36, axis=180),
        rightLeg=dict(pitch=-25, yaw=+4, bend=14, axis=0),
        leftLeg=dict(pitch=+15, yaw=+4, bend=18, axis=0),
    ),
    5: dict(  # 过顶：剑立在头顶正上，腰开始解拧
        easing="INQUAD",
        body=dict(x=+0.02, y=+0.02, z=-0.01, yaw=-14),
        head=dict(pitch=-9, yaw=+1),
        torso=dict(pitch=-3, yaw=+14),
        rightArm=dict(pitch=-77.6, yaw=-18.3, roll=+10.6, bend=39.2, axis=180),
        leftArm=dict(pitch=+22, yaw=+30, roll=-22, bend=28, axis=180),
        rightLeg=dict(pitch=-21, yaw=+4, bend=16, axis=0),
        leftLeg=dict(pitch=+12, yaw=+4, bend=15, axis=0),
    ),
    8: dict(  # 斩入：剑越过中线砍向左前，手臂展开（bend 39→16）
        easing="INQUAD",
        body=dict(x=-0.02, y=-0.01, z=+0.12, yaw=-4),
        head=dict(pitch=+6, yaw=+8),
        torso=dict(pitch=+10, yaw=-6),
        rightArm=dict(pitch=-27.6, yaw=-31.8, roll=+26.4, bend=16.1, axis=180),
        leftArm=dict(pitch=+2, yaw=+22, roll=-16, bend=24, axis=180),
        rightLeg=dict(pitch=-13, yaw=+3, bend=20, axis=0),
        leftLeg=dict(pitch=+19, yaw=+3, bend=10, axis=0),
    ),
    10: dict(  # IMPACT：躯干前折 + 弓步前压，剑尖压到左前下
        easing="OUTQUAD",
        body=dict(x=-0.05, y=-0.04, z=+0.20, yaw=+2),
        head=dict(pitch=+13, yaw=+12),
        torso=dict(pitch=+19, yaw=-18),
        rightArm=dict(pitch=-2.5, yaw=-39.6, roll=+32.0, bend=14.8, axis=180),
        leftArm=dict(pitch=-20, yaw=+16, roll=-10, bend=32, axis=180),
        rightLeg=dict(pitch=-7, yaw=+2, bend=24, axis=0),
        leftLeg=dict(pitch=+24, yaw=+2, bend=8, axis=0),
    ),
    13: dict(  # RIP PULL：倒钩咬住肉，剑提半格顿一下再往回撕
        easing="INOUTSINE",
        body=dict(x=-0.02, y=-0.02, z=+0.11, yaw=0),
        head=dict(pitch=+8, yaw=+8),
        torso=dict(pitch=+13, yaw=-12),
        rightArm=dict(pitch=+20.0, yaw=-30.6, roll=+30.8, bend=37.4, axis=180),
        leftArm=dict(pitch=-12, yaw=+18, roll=-13, bend=30, axis=180),
        rightLeg=dict(pitch=-10, yaw=+3, bend=19, axis=0),
        leftLeg=dict(pitch=+17, yaw=+3, bend=10, axis=0),
    ),
    18: dict(  # 收势：低架 guard，剑指前方略上
        easing="INOUTSINE",
        body=dict(x=+0.01, y=0.0, z=+0.01, yaw=-8),
        head=dict(pitch=-3, yaw=+4),
        torso=dict(pitch=+3, yaw=+8),
        rightArm=dict(pitch=+53.1, yaw=+9.9, roll=+23.3, bend=74.0, axis=180),
        leftArm=dict(pitch=+10, yaw=+24, roll=-20, bend=28, axis=180),
        rightLeg=dict(pitch=-11, yaw=+4, bend=13, axis=0),
        leftLeg=dict(pitch=+9, yaw=+4, bend=11, axis=0),
    ),
}

DESCRIPTION = (
    "异兽脊骨剑单手斜斩 (sword_spine_slash): 20-tick 重剑斜劈，"
    "扛剑过右肩蓄力 -> 过顶斜劈向左前下 -> 棘突倒钩撕扯回抽 -> 收低架。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sword_spine_slash",
        description=DESCRIPTION,
        end_tick=20,
        stop_tick=22,
        is_loop=False,
    )
