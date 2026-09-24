#!/usr/bin/env python3
"""dagger_reverse_slash —— 匕首反握斩击。

## v4：反握不再是"手臂拧过去"，而是刀自己在手里翻了过来

v3 把「反握」理解成手臂 roll≈±180 的一个姿势，这是错的。手持物被 display 变换焊死
在前臂上，**光靠手臂根本表达不出正握 / 反握的区别** —— 拧手臂只会把整条小臂一起拧
过去，读出来是"胳膊别着"，不是"刀口朝下"。真正管这件事的是 PlayerAnimator 的
`rightItem` 骨头（`HeldItemMixin.changeItemLocation`），本仓此前整条链（出料 / 预览 /
门禁 / 回读）都不认它，于是这个区别在数据里根本不存在。

2026-09-01 用户在 Blockbench 里把 `dagger_right_pitch` 打了一帧 ≈180°，把「反握」这件事
明确写了出来，同时重摆了右臂两端。本版按这两件事重做：

    rightItem   全程绕**刃口轴**（刀身局部 X，刃宽 1.6px 的那一轴）转 180°
                → 刃口朝向不变、刀身整个倒转 = 反握。换算见 `anim_common.ITEM_PARTS`。
    rightArm    t0 pitch −137.4 / bend 14.5（臂近乎伸直举到肩上）
                t8 pitch  −38.4 / bend 47.0（收回胸前）

## 这两端本身就把招式定死了：不是垂直劈，是斜耙

实测两端（Bedrock px，x 负 = 玩家右侧）：

    t0  握把 (−6.9, 28.3, −5.7)  刃仰角 −76.2°（几乎垂直下垂）
    t8  握把 (−1.6, 20.6, −8.0)  刃仰角 −26.2°

拳从右上走到左前下（行程 9.7px），刃在下面像雨刷一样扫过一个 50° 的扇面。**刀尖几乎
不动**（纯插值下全程只走 8.3px），动作的可读性来自拳的行程 + 刃的角度扫掠，不是刀尖
的位移 —— 想把刀尖也甩起来就得离开这两端，那是改招不是补过渡。

所以中间要补的只有三件事：反向蓄势、撞击峰速、过冲回收。

## 8 tick 分段：脊线就是两端的插值，补的是节奏

拳的轨迹由两端唯一决定，中间**不另起一条弧** —— 试过把撞击帧推到身前横过胸口，
结果刀整个被躯干挡住（接触表 t5 四个视角都看不见刃）。可达域也说明了原因：拳前伸到
z ≤ −9 且低于 y=23 时，刃最陡只能到 −46°，正好落在两端连线上。所以中间帧写成脊线
参数 u 的取值，u<0 = 往回蓄势、u>1 = 过冲：

    tick 0  u=0      用户手摆：臂近乎伸直举到肩上，刃自拳下垂（−76.2°）
    tick 2  u=−0.08  反向蓄势 —— 沿脊线倒退，拳再举高、肘更直，腰往右拧
    tick 3  u=−0.14  LOAD 顶点（拳 y=29.0），easing 切 INCUBIC 起爆
    tick 4  u=+0.42  斩到一半 —— 拳砸到 y=25.1 / z=−8.8，刃 −56.5°，腰甩正
    tick 5  u=+0.90  IMPACT —— 拳 y=21.3，刃耙到 −31°，峰速落在这一格
    tick 6  u=+1.12  过冲 1.1px 后开始回收

t5 / t6 的 yaw、roll 离脊线各偏了 6~8°，是**自穿模修正**不是手感：反握时刃自拳向后
下垂，收招段拳走到胸前时刃正好扫过自己的胸口 —— 实测原脊线值在 t5 有 0.16px 没入
（缩过 1.2px 的躯干盒）。把前臂 roll 收回 8°、yaw 往外 6°，整条动画的最小间隙抬到
+0.71px，正好等于用户手摆的末帧自己的间隙 —— 也就是说全程不比他画的那一帧更贴身。
    tick 8  u=+1.00  用户手摆的收势

## 肘的门禁换了判据

`gate_elbow`（全程 bend ≥ 15°）在这条上是错的门 —— 用户手摆的起手式 bend=14.5°，
蓄势还要更直（9.9°）。**直臂抡下来本来就是这条招的样子**，真正该抓的缺陷是"肘全程
锁死不动"。所以本条改用 `gate_elbow_range`：肘的开合幅度必须够（实测 9.9→50.9 = 41°），
锁死的假动作会被它抓住。这不是放宽，是换成一条对直臂起手也成立的判据。
"""

from anim_common import emit_json, item_spin

# 匕首 `display.thirdperson_righthand.rotation` 与「刃口轴 = 刀身局部 X」。
DAGGER_DISPLAY_ROT = (0.0, -90.0, 55.0)
BLADE_EDGE_AXIS = (1.0, 0.0, 0.0)

# 反握 = 绕刃口轴 180°。两端都打帧，不靠"只打一帧自动保持"——那条在循环动画上会被
# `Axis.findAfter` 合成的虚拟末帧拉回默认值（约定文档 §2 rule 8）。
REVERSE_GRIP = item_spin(DAGGER_DISPLAY_ROT, BLADE_EDGE_AXIS, -180.0)

POSE = {
    0: dict(  # 用户手摆：臂近乎伸直举到肩上，刃自拳下垂（仰角 −76.2°）
        easing="OUTSINE",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=-137.4, yaw=+28.7, roll=-15.8, bend=+14.5, axis=180),
        leftArm=dict(pitch=-65.1, yaw=+26.1, roll=-6.6, bend=+20.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
        rightItem=dict(**REVERSE_GRIP),
    ),
    2: dict(  # 反向蓄势：沿脊线倒退，拳再举高、肘更直，腰往右拧
        easing="OUTQUAD",
        body=dict(x=+0.03, y=-0.01, z=+0.02, yaw=-30.0),
        head=dict(pitch=-1.0, yaw=+24.0),
        torso=dict(pitch=+1.0, yaw=+22.0),
        rightArm=dict(pitch=-145.3, yaw=+30.9, roll=-14.8, bend=+11.9, axis=180),
        leftArm=dict(pitch=-58.0, yaw=+27.0, roll=-6.0, bend=+24.0, axis=180),
        rightLeg=dict(pitch=-9.0, yaw=+4.0, bend=+18.0, axis=0),
        leftLeg=dict(pitch=+15.0, yaw=+2.0, bend=+26.0, axis=0),
    ),
    3: dict(  # LOAD 顶点：拳举到最高（y=29.0），easing 切 IN 起爆
        easing="INCUBIC",
        body=dict(x=+0.04, y=-0.02, z=+0.03, yaw=-30.0),
        head=dict(pitch=-3.0, yaw=+22.0),
        torso=dict(pitch=-1.0, yaw=+28.0),
        rightArm=dict(pitch=-151.3, yaw=+32.6, roll=-14.1, bend=+9.9, axis=180),
        leftArm=dict(pitch=-54.0, yaw=+28.0, roll=-6.0, bend=+26.0, axis=180),
        rightLeg=dict(pitch=-8.0, yaw=+4.0, bend=+16.0, axis=0),
        leftLeg=dict(pitch=+16.0, yaw=+2.0, bend=+28.0, axis=0),
    ),
    4: dict(  # 斩到一半：拳砸到 y=25.1 / z=−8.8，刃 −56.5°
        easing="INCUBIC",
        body=dict(x=+0.02, y=+0.02, z=-0.03, yaw=-30.0),
        head=dict(pitch=+4.0, yaw=+26.0),
        torso=dict(pitch=+7.0, yaw=+10.0),
        rightArm=dict(pitch=-95.8, yaw=+16.9, roll=-20.9, bend=+28.1, axis=180),
        leftArm=dict(pitch=-40.0, yaw=+26.0, roll=-8.0, bend=+22.0, axis=180),
        rightLeg=dict(pitch=-14.0, yaw=+5.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+18.0, yaw=+1.0, bend=+24.0, axis=0),
    ),
    5: dict(  # IMPACT：拳砸到 y=21.3、刃耙到 −31°，峰速落在这一格
        easing="OUTQUAD",
        body=dict(x=+0.01, y=+0.04, z=-0.05, yaw=-30.0),
        head=dict(pitch=+10.0, yaw=+28.0),
        torso=dict(pitch=+12.0, yaw=-4.0),
        rightArm=dict(pitch=-48.3, yaw=-2.5, roll=-18.7, bend=+43.8, axis=180),
        leftArm=dict(pitch=-16.0, yaw=+22.0, roll=-10.0, bend=+26.0, axis=180),
        rightLeg=dict(pitch=-18.0, yaw=+7.0, bend=+30.0, axis=0),
        leftLeg=dict(pitch=+19.0, yaw=+0.0, bend=+20.0, axis=0),
    ),
    6: dict(  # 过冲：沿脊线再走 1.1px，然后回收
        easing="INOUTSINE",
        body=dict(x=+0.00, y=+0.03, z=-0.04, yaw=-30.0),
        head=dict(pitch=+11.0, yaw=+30.0),
        torso=dict(pitch=+13.0, yaw=-9.0),
        rightArm=dict(pitch=-26.5, yaw=-2.7, roll=-21.4, bend=+50.9, axis=180),
        leftArm=dict(pitch=-6.0, yaw=+20.0, roll=-11.0, bend=+24.0, axis=180),
        rightLeg=dict(pitch=-20.0, yaw=+7.0, bend=+32.0, axis=0),
        leftLeg=dict(pitch=+20.0, yaw=+0.0, bend=+19.0, axis=0),
    ),
    8: dict(  # 用户手摆的收势：拳回收胸前，刃仍下垂
        easing="INOUTSINE",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=-38.4, yaw=+0.7, roll=-27.9, bend=+47.0, axis=180),
        leftArm=dict(pitch=+5.8, yaw=+17.0, roll=-11.0, bend=+20.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
        rightItem=dict(**REVERSE_GRIP),
    ),
}

DESCRIPTION = (
    "v4 匕首反握斩击: 全程反握（rightItem 绕刃口轴 180°，刃自拳下垂）; 首末两帧人手摆, "
    "拳自右上耙到左前下 9.7px、刃扫过 50° 扇面; 中间帧沿两端脊线取 u=−0.14 蓄势 / "
    "u=0.90 撞击 / u=1.12 过冲, 不另起弧。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="dagger_reverse_slash",
        description=DESCRIPTION,
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
