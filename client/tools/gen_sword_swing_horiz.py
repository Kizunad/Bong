#!/usr/bin/env python3
"""sword_swing_horiz — 从左肩斜斩下来的反手大斜斩（异兽脊骨剑口径重做）。

## 动作

**剑横举于胸前、剑尖指左上（起手）→ 上抬拉满 → 转刃过头顶 → 越过中线斜劈向右前下 →
棘突倒钩咬住顿一下回抽 → 收低架。** 是 `sword_spine_slash`（右肩→左前下）的**反向对角**，
两招从远处一眼能分辨走的是哪条对角线。

tick 0 是用户在 Blockbench 里手摆的起手帧，在反解里**钉死**不动。

| tick | 阶段 | 肩向 d（+X=角色左 / −Z=前） | 半径 r |
|------|------|---------------------------|--------|
| 0  | 起手 GUARD  | (+0.91, +0.41, −0.01) 左上 | 26.0 |
| 2  | LOAD 拉满   | (+0.80, +0.47, +0.37) 左后上 | 22.2 |
| 5  | 转刃过顶    | (+0.36, +0.92, −0.15) 头顶偏左 | 23.0 |
| 8  | 斩入        | (−0.32, +0.34, −0.88) 越中线向右前 | 23.6 |
| 10 | IMPACT      | (−0.50, −0.53, −0.69) 右前下 | 22.0 |
| 13 | RIP PULL    | (−0.41, −0.30, −0.86) 提一点顿一下 | 21.15 |
| 16 | 收势        | (−0.26, −0.08, −0.96) 低架指前 | 22.5 |

## 为什么轨迹写成「肩向 + 半径」而不是绝对坐标

握姿固定后（`gen_beast_spine_sword_player_anim` 的 `sword_right_pitch/roll` 静态角）
**剑身恒垂直于小臂**，于是剑尖离肩最远只有 `sqrt(臂长² + 刃长²) = sqrt(8.06² + 24.4²)
≈ 25.7px`——直觉上"剑尖甩到身前 30px"根本不在工作空间里。第一版按绝对坐标写的轨迹有
四帧半径落在 30~32，逐帧独立反解都还剩 6~8px 残差，怎么调权重都收不进去。

半径同时是**手臂伸展度**的旋钮（实测：r=20 → 肘弯 75°，23 → 40°，25 → 16°，26 → 6°
基本笔直）。所以伸展曲线就是发力曲线：起手伸 → 蓄力收拢 → 斩入伸到峰值 → 触靶略收 →
收势折回，照抄 `sword_spine_slash` 量出来的同一条曲线。

## 顿挫感是标定过的，不是拍脑袋

t13 相对 t10 把剑尖挪 3.65px——和 `sword_spine_slash` 的 3.6px 对齐（用户验收 round 2
时点名这个量"刚刚好"，别加大也别抹平；`test_the_hitch_stays_the_size_the_user_signed_off`
就是拿 spine_slash 当基准焊死的）。整条的关节角二阶差分 max 14.8°/t²，介于
`sword_spine_slash`（12.7，用户认可）与 `sword_spine_cleave`（36.3）之间；改动前的旧版
是 49.7 —— 那个数字就是"播起来手臂抽一下"的来源。

右臂四轴是按上表整条一次性反解的（大圆插值目标 + 二阶差分罚项），不是逐帧手调：
逐帧独立解会在冗余自由度上跳（实测相邻帧 yaw −3°→−41°→−8°）。

## 时序

endTick=18（含 2 tick 混出），stopTick=20，非循环。发力顶点在 tick 10。旧版是 10t，
放长到 18t 才装得下 RIP 那一拍；本动画没有 server 技能消费（`sword_basics` 用的是
cleave/thrust/parry/infuse 四条），也没有 anim_spec_manifest 钉时长，改时长安全。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # 起手：剑横在胸前、剑尖指左上（用户手摆帧，反解回静态握姿后钉死）
        easing="OUTSINE",
        body=dict(x=+0.04, y=0.0, z=0.0, yaw=0),
        head=dict(pitch=-6.5, yaw=+10),
        torso=dict(pitch=+3, yaw=+12),
        rightArm=dict(pitch=-94.4, yaw=+10.8, roll=+67.6, bend=11.9, axis=180),
        leftArm=dict(pitch=+12.5, yaw=+15, roll=-18, bend=27.5, axis=180),
        rightLeg=dict(pitch=-8, yaw=+4, bend=12, axis=0),
        leftLeg=dict(pitch=+6, yaw=+4, bend=10, axis=0),
    ),
    2: dict(  # LOAD：肘收拢（bend 12→46）把剑再往左后上拉满，下盘同时坐低
        easing="OUTSINE",
        body=dict(x=+0.07, y=-0.02, z=-0.05, yaw=+16),
        head=dict(pitch=-4, yaw=+14),
        torso=dict(pitch=+2, yaw=+24),
        rightArm=dict(pitch=-90.5, yaw=+11.4, roll=+51.1, bend=46.6, axis=180),
        leftArm=dict(pitch=+20, yaw=+22, roll=-24, bend=34, axis=180),
        rightLeg=dict(pitch=-13, yaw=+4, bend=18, axis=0),
        leftLeg=dict(pitch=+11, yaw=+4, bend=16, axis=0),
    ),
    5: dict(  # 转刃过顶：剑立在头顶偏左，腰开始解拧
        easing="INQUAD",
        body=dict(x=+0.05, y=+0.02, z=-0.02, yaw=+11),
        head=dict(pitch=-10, yaw=+6),
        torso=dict(pitch=-2, yaw=+16),
        rightArm=dict(pitch=-46.4, yaw=+17.5, roll=+27.0, bend=54.5, axis=180),
        leftArm=dict(pitch=+14, yaw=+26, roll=-22, bend=30, axis=180),
        rightLeg=dict(pitch=-11, yaw=+4, bend=16, axis=0),
        leftLeg=dict(pitch=+9, yaw=+4, bend=14, axis=0),
    ),
    8: dict(  # 斩入：剑越过中线砍向右前，手臂伸到峰值（r 23.6）
        easing="INQUAD",
        body=dict(x=-0.03, y=-0.01, z=+0.10, yaw=-8),
        head=dict(pitch=+5, yaw=-8),
        torso=dict(pitch=+9, yaw=-8),
        rightArm=dict(pitch=+11.8, yaw=+31.3, roll=+5.9, bend=45.1, axis=180),
        leftArm=dict(pitch=-4, yaw=+30, roll=-14, bend=40, axis=180),
        rightLeg=dict(pitch=-6, yaw=+3, bend=20, axis=0),
        leftLeg=dict(pitch=+16, yaw=+3, bend=11, axis=0),
    ),
    10: dict(  # IMPACT：躯干前折 + 弓步前压，剑尖压到右前下
        easing="OUTQUAD",
        body=dict(x=-0.08, y=-0.05, z=+0.18, yaw=-17),
        head=dict(pitch=+14, yaw=-14),
        torso=dict(pitch=+17, yaw=-22),
        rightArm=dict(pitch=+45.0, yaw=+46.8, roll=-8.3, bend=41.8, axis=180),
        leftArm=dict(pitch=-18, yaw=+34, roll=-8, bend=48, axis=180),
        rightLeg=dict(pitch=-2, yaw=+2, bend=25, axis=0),
        leftLeg=dict(pitch=+22, yaw=+2, bend=8, axis=0),
    ),
    13: dict(  # RIP PULL：倒钩咬住肉，剑提一点顿一下再往回撕（量级同 spine_slash）
        easing="INOUTSINE",
        body=dict(x=-0.05, y=-0.03, z=+0.11, yaw=-12),
        head=dict(pitch=+9, yaw=-10),
        torso=dict(pitch=+12, yaw=-15),
        rightArm=dict(pitch=+59.5, yaw=+38.1, roll=-15.5, bend=64.9, axis=180),
        leftArm=dict(pitch=-10, yaw=+28, roll=-12, bend=40, axis=180),
        rightLeg=dict(pitch=-5, yaw=+3, bend=20, axis=0),
        leftLeg=dict(pitch=+16, yaw=+3, bend=10, axis=0),
    ),
    16: dict(  # 收势：低架 guard，剑指前方略下
        easing="INOUTSINE",
        body=dict(x=+0.01, y=0.0, z=+0.02, yaw=-4),
        head=dict(pitch=-2, yaw=-2),
        torso=dict(pitch=+4, yaw=+2),
        rightArm=dict(pitch=+70.0, yaw=+17.6, roll=-23.0, bend=84.9, axis=180),
        leftArm=dict(pitch=+6, yaw=+20, roll=-18, bend=30, axis=180),
        rightLeg=dict(pitch=-8, yaw=+4, bend=14, axis=0),
        leftLeg=dict(pitch=+8, yaw=+4, bend=11, axis=0),
    ),
}

DESCRIPTION = (
    "反手大斜斩 (sword_swing_horiz): 18-tick，剑横举胸前指左上 -> 上抬拉满 -> "
    "转刃过顶 -> 越中线斜劈右前下 -> 棘突倒钩顿挫回抽 -> 收低架。"
    "与 sword_spine_slash 走相反的对角线。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sword_swing_horiz",
        description=DESCRIPTION,
        end_tick=18,
        stop_tick=20,
        is_loop=False,
    )
