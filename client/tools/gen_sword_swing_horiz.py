#!/usr/bin/env python3
"""sword_swing_horiz — 从左肩斜斩下来的反手大斜斩（异兽脊骨剑口径重做）。

## 动作

**剑横举于胸前、剑尖指左上（起手）→ 往身后拧（不抬高）→ 沉肘转刃 → 越过中线斜劈向
右前下 → 棘突倒钩咬住顿一下回抽 → 收低架。** 是 `sword_spine_slash`（右肩→左前下）的
**反向对角**，两招从远处一眼能分辨走的是哪条对角线。

tick 0 是用户在 Blockbench 里手摆的起手帧，在反解里**钉死**不动。

## 剑尖高度必须是 ∪，不能是 ∩

第一版在 t5 插了一拍"转刃过顶"，剑尖高度走成 32.7 → 33.8 → **42.3** → 29.8 → 12.3，
先拱起一个高于起手的峰再落下——那是个 ∩ 形。用户看图直接指出来了：应该"先向下凸，
也就是凹曲线"。**「从肩部斜斩下来」的意思就是剑已经在肩上，直接砍，不再多举一次。**
`sword_spine_slash` 反过来：用户对它的要求原话是"手臂抬起然后……斜斩"，所以那条该有
过顶峰。两条的形状差异是有意的，`test_swing_horiz_tip_height_is_a_valley` 焊死这条。

现在的高度曲线：32.7 → 29.4 → 25.3 → 12.3 → 8.7 → 11.4 → 17.7（单调下沉到谷底再抬回）。

| tick | 阶段 | 肩向 d（+X=角色左 / −Z=前） | 半径 r | 剑尖高度 |
|------|------|---------------------------|--------|---------|
| 0  | 起手 GUARD  | (+0.91, +0.41, −0.03) 左上 | 25.9 | 32.7 |
| 2  | LOAD 往身后拧 | (+0.90, +0.30, +0.31) 左后 | 23.0 | 29.4 |
| 5  | 沉肘转刃    | (+0.85, +0.12, −0.51) 左前 | 23.5 | 25.3 |
| 8  | 斩入        | (+0.35, −0.35, −0.87) 越中线 | 24.0 | 12.3 |
| 10 | IMPACT 谷底 | (−0.42, −0.62, −0.66) 右前下 | 22.5 | 8.7 |
| 13 | RIP PULL    | (−0.47, −0.52, −0.72) 提一点顿一下 | 21.5 | 11.4 |
| 16 | 收势        | (−0.30, −0.18, −0.94) 低架指前 | 22.5 | 17.7 |

## 为什么轨迹写成「肩向 + 半径」而不是绝对坐标

握姿固定后（`gen_beast_spine_sword_player_anim` 的 `sword_right_pitch/roll` 静态角）
**剑身恒垂直于小臂**，于是剑尖离肩最远只有 `sqrt(臂长² + 刃长²) = sqrt(8.06² + 24.4²)
≈ 25.7px`——直觉上"剑尖甩到身前 30px"根本不在工作空间里。最早一版按绝对坐标写的轨迹
有四帧半径落在 30~32，逐帧独立反解都还剩 6~8px 残差，怎么调权重都收不进去。

半径同时是**手臂伸展度**的旋钮（实测：r=20 → 肘弯 75°，23 → 40°，25 → 16°，26 → 6°
基本笔直）。所以伸展曲线就是发力曲线。

## 顿挫感是标定过的，不是拍脑袋

t13 相对 t10 把剑尖挪 3.59px——和 `sword_spine_slash` 的 3.6px 对齐（用户验收 round 2
时点名这个量"刚刚好"，别加大也别抹平；`test_the_hitch_stays_the_size_the_user_signed_off`
就是拿 spine_slash 当基准焊死的）。整条的关节角二阶差分 max 15.4°/t²，介于
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
    2: dict(  # LOAD：肘收拢（bend 12→43）往身后拧，**不抬高**——抬高就又成 ∩ 了
        easing="OUTSINE",
        body=dict(x=+0.07, y=-0.02, z=-0.05, yaw=+16),
        head=dict(pitch=-3, yaw=+14),
        torso=dict(pitch=+2, yaw=+22),
        rightArm=dict(pitch=-91.0, yaw=+2.9, roll=+71.2, bend=43.3, axis=180),
        leftArm=dict(pitch=+20, yaw=+22, roll=-24, bend=34, axis=180),
        rightLeg=dict(pitch=-13, yaw=+4, bend=18, axis=0),
        leftLeg=dict(pitch=+11, yaw=+4, bend=16, axis=0),
    ),
    5: dict(  # 沉肘转刃：刃口翻正，剑尖已经在往下走
        easing="INQUAD",
        body=dict(x=+0.05, y=-0.01, z=+0.02, yaw=+10),
        head=dict(pitch=+2, yaw=+8),
        torso=dict(pitch=+5, yaw=+12),
        rightArm=dict(pitch=-43.8, yaw=-10.1, roll=+80.7, bend=44.4, axis=180),
        leftArm=dict(pitch=+8, yaw=+26, roll=-22, bend=32, axis=180),
        rightLeg=dict(pitch=-11, yaw=+4, bend=16, axis=0),
        leftLeg=dict(pitch=+9, yaw=+4, bend=14, axis=0),
    ),
    8: dict(  # 斩入：越过中线，剑尖已沉到胸腹高度
        easing="INQUAD",
        body=dict(x=-0.03, y=-0.02, z=+0.11, yaw=-6),
        head=dict(pitch=+11, yaw=-4),
        torso=dict(pitch=+12, yaw=-6),
        rightArm=dict(pitch=+8.7, yaw=-28.8, roll=+89.2, bend=39.2, axis=180),
        leftArm=dict(pitch=-4, yaw=+30, roll=-14, bend=40, axis=180),
        rightLeg=dict(pitch=-6, yaw=+3, bend=20, axis=0),
        leftLeg=dict(pitch=+16, yaw=+3, bend=11, axis=0),
    ),
    10: dict(  # IMPACT：谷底。躯干前折 + 弓步前压，剑尖压到右前下
        easing="OUTQUAD",
        body=dict(x=-0.08, y=-0.05, z=+0.18, yaw=-16),
        head=dict(pitch=+17, yaw=-12),
        torso=dict(pitch=+18, yaw=-20),
        rightArm=dict(pitch=+41.5, yaw=-50.7, roll=+100.2, bend=35.5, axis=180),
        leftArm=dict(pitch=-18, yaw=+34, roll=-8, bend=48, axis=180),
        rightLeg=dict(pitch=-2, yaw=+2, bend=25, axis=0),
        leftLeg=dict(pitch=+22, yaw=+2, bend=8, axis=0),
    ),
    13: dict(  # RIP PULL：倒钩咬住肉，剑提一点顿一下再往回撕（量级同 spine_slash）
        easing="INOUTSINE",
        body=dict(x=-0.05, y=-0.03, z=+0.11, yaw=-12),
        head=dict(pitch=+11, yaw=-9),
        torso=dict(pitch=+13, yaw=-14),
        rightArm=dict(pitch=+59.9, yaw=-50.7, roll=+112.0, bend=53.8, axis=180),
        leftArm=dict(pitch=-10, yaw=+28, roll=-12, bend=40, axis=180),
        rightLeg=dict(pitch=-5, yaw=+3, bend=20, axis=0),
        leftLeg=dict(pitch=+16, yaw=+3, bend=10, axis=0),
    ),
    16: dict(  # 收势：剑尖抬回，低架指前
        easing="INOUTSINE",
        body=dict(x=+0.01, y=0.0, z=+0.02, yaw=-4),
        head=dict(pitch=+1, yaw=-2),
        torso=dict(pitch=+5, yaw=+2),
        rightArm=dict(pitch=+70.0, yaw=-26.1, roll=+127.5, bend=73.2, axis=180),
        leftArm=dict(pitch=+6, yaw=+20, roll=-18, bend=30, axis=180),
        rightLeg=dict(pitch=-8, yaw=+4, bend=14, axis=0),
        leftLeg=dict(pitch=+8, yaw=+4, bend=11, axis=0),
    ),
}

DESCRIPTION = (
    "反手大斜斩 (sword_swing_horiz): 18-tick，剑横举胸前指左上 -> 往身后拧 -> "
    "沉肘转刃 -> 越中线斜劈右前下 -> 棘突倒钩顿挫回抽 -> 收低架。"
    "剑尖高度是先降后升的 ∪，全程不再举过头顶；与 sword_spine_slash 走相反的对角线。"
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
