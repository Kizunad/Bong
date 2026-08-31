#!/usr/bin/env python3
"""dagger_reverse_slash —— 匕首反握撕斩（反握 = 刃自拳下垂）。

## v3：首末两帧是人手摆的，招式路线因此改了

2026-08-31 用户在 Blockbench 里重摆了 t0 / t8 并删掉中间帧。两端逐轴回读
（`bbmodel_maker.workbench.bbmodel_to_pose`）：

| | 握把 | 刃向 | 肘 bend |
|---|---|---|---|
| t0 起手 | (−4.5, 18.5, −5.6) | 自拳下垂略朝前，−64.5° | 169.5°（深折） |
| t8 收势 | (−6.8, 21.0, +0.8) | 朝右后、基本水平，+4.4° | 159.5° |

两端只差 `rightArm.pitch` 一个 114° 的摆动（−184.9 → −70.9），其余轴几乎不动 ——
所以这条是**刃自胸前垂下 → 扫过正下方 → 向右后撕开**的反手撕，不是 v2 那种
「自右上向左下斜斩」。招式换了，编排跟着换。

## 端点欧拉必须先重绕

用户存盘里 t0 写的是 `pitch=+175.1`。PlayerAnimator 逐轴插值，从 +175.1 走到 −70.9
是**正着转 246°**（手臂绕过头顶再回来）；同一个姿态写成 `−184.9` 就只需反着转 114°。
两种写法渲出来一模一样，差别全在中间那七格。`design3.rewrap` 负责挑离邻帧最近的
等价写法，并断言旋转矩阵不变。

## 为什么这条不反解，直接写欧拉角

反握要求前臂 `roll` 落在 ±180 附近 —— 那正是 ZYX 欧拉的退化带。逐帧独立反解时相邻
两帧会落到「朝向几乎一样、欧拉差一百多度」的两支上：实测 t2→t3 的 pitch 从 +134.7
跳到 −47.6、t4→t5 的 yaw 从 −38.3 跳到 +95.0（顶死在边界上），中间七格于是乱窜 ——
刀尖单格瞬移 15.7px、刃向绕远、前臂穿头 1/65 帧。用户给的两端本来就在同一支上，
顺着它逐帧写就没有这个问题（同一套门下：瞬移 5.7px、刃向偏离弧 4°、穿头 0 帧）。

## 8 tick 分段

    tick 0  起手     用户手摆：反握低架，刃自拳下垂（刀尖 y=8.4）
    tick 2  探刃      刃向前下探（刀尖 y=10.9，体坐标 z=−14.1）
    tick 3  LOAD     探到最前（pitch −206，刀尖 y=13.3、z=−16.7）；重心后坐
    tick 4  过弧底    刃扫过正下方（仰角 −84.0°，刀尖 y=6.5）—— 这一帧钉在弧底上，
                     不然 4→5 段的仰角会先掉下去再爬上来
    tick 5  IMPACT   撕开（刀尖 y=15.1，已越到体坐标 z=+10.8）；峰速实测落在 t5.00
    tick 6  overshoot 甩到最高最后（刀尖 y=24.4、z=+16.1）
    tick 8  用户手摆的右后收势

刀尖整段行程包围盒对角 34.7px —— 四条里最长的一条。

## 门禁跟着招式一起改（`knife_anim_gates.SUITE`）

- **绕背收在弧底之前**（`behind_until` = 4.0）。刃越过背面**是这一招本身**（用户手摆
  的末帧刀尖就在体坐标 z=+13.2）。这条门当初要抓的是「蓄势时刀尖绕到后脑勺」
  （返工前实测 z=+8.9），那一段落在 t≤4 里，窗口内实测最深 −3.2，teeth 没丢。
- **肘收在 t≤6**：与另外三条同口径（用户把末帧摆成跟随动作，不再回起手式）。
- **收势闭合收窄到下盘**：他没动 body/torso/head/两腿。

## 「刀尖在 t4 掉下去」不是败笔，是弧底

刃绕肩关节画一个圆，t4 正好在圆的最低点（仰角 −84°）。把它当成"抖了一下"去修，只会
把 4→5 段的摆幅塞回 t3→t4，反而让撕的那一下没了力量。判断标准是**单调**：t4 之后
仰角一路 −84 → −20.7 → +14 不回头。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # 用户手摆的反握低架 —— 刃自拳下垂（实测 −64.5°）
        easing="OUTSINE",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=-184.9, yaw=+28.7, roll=-15.8, bend=+169.5, axis=180),
        leftArm=dict(pitch=-65.1, yaw=+26.1, roll=-6.6, bend=+20.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
    ),
    2: dict(  # 探刃 —— 刃向前下探（刀尖 y=10.9）
        easing="OUTQUAD",
        body=dict(x=+0.04, y=+0.01, z=-0.03, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+26.0),
        torso=dict(pitch=+5.0, yaw=+24.0),
        rightArm=dict(pitch=-198.0, yaw=+31.0, roll=-11.0, bend=+171.0, axis=180),
        leftArm=dict(pitch=-68.0, yaw=+27.0, roll=-6.0, bend=+18.0, axis=180),
        rightLeg=dict(pitch=-10.0, yaw=+4.0, bend=+18.0, axis=0),
        leftLeg=dict(pitch=+16.0, yaw=+2.0, bend=+30.0, axis=0),
    ),
    3: dict(  # LOAD —— 探到最前（刀尖体坐标 z=−16.7）；重心后坐
        easing="INCUBIC",
        body=dict(x=+0.05, y=+0.02, z=-0.05, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+24.0),
        torso=dict(pitch=+6.0, yaw=+32.0),
        rightArm=dict(pitch=-206.0, yaw=+33.0, roll=-8.0, bend=+173.0, axis=180),
        leftArm=dict(pitch=-70.0, yaw=+28.0, roll=-6.0, bend=+17.0, axis=180),
        rightLeg=dict(pitch=-8.0, yaw=+4.0, bend=+16.0, axis=0),
        leftLeg=dict(pitch=+18.0, yaw=+2.0, bend=+34.0, axis=0),
    ),
    4: dict(  # 过弧底 —— 刃扫过正下方（仰角 −84.0°），此后仰角单调爬升
        easing="INCUBIC",
        body=dict(x=+0.02, y=+0.01, z=+0.05, yaw=-30.0),
        head=dict(pitch=+2.0, yaw=+28.0),
        torso=dict(pitch=+7.0, yaw=+8.0),
        rightArm=dict(pitch=-152.0, yaw=+26.0, roll=-4.0, bend=+170.0, axis=180),
        leftArm=dict(pitch=-52.0, yaw=+25.0, roll=-7.0, bend=+22.0, axis=180),
        rightLeg=dict(pitch=-15.0, yaw=+6.0, bend=+24.0, axis=0),
        leftLeg=dict(pitch=+20.0, yaw=+1.0, bend=+22.0, axis=0),
    ),
    5: dict(  # IMPACT —— 撕开（刀尖 y=15.1，已越到体坐标 z=+10.8）
        easing="OUTQUAD",
        body=dict(x=-0.02, y=+0.03, z=+0.12, yaw=-30.0),
        head=dict(pitch=+6.0, yaw=+30.0),
        torso=dict(pitch=+10.0, yaw=-14.0),
        rightArm=dict(pitch=-95.0, yaw=+16.0, roll=-16.0, bend=+166.0, axis=180),
        leftArm=dict(pitch=-22.0, yaw=+21.0, roll=-9.0, bend=+30.0, axis=180),
        rightLeg=dict(pitch=-20.0, yaw=+8.0, bend=+36.0, axis=0),
        leftLeg=dict(pitch=+20.0, yaw=+0.0, bend=+20.0, axis=0),
    ),
    6: dict(  # overshoot —— 甩到最高最后（刀尖 y=24.4）
        easing="INOUTSINE",
        body=dict(x=-0.01, y=+0.03, z=+0.13, yaw=-30.0),
        head=dict(pitch=+7.0, yaw=+32.0),
        torso=dict(pitch=+11.0, yaw=-18.0),
        rightArm=dict(pitch=-62.0, yaw=+2.0, roll=-29.0, bend=+158.0, axis=180),
        leftArm=dict(pitch=+0.0, yaw=+18.0, roll=-10.0, bend=+24.0, axis=180),
        rightLeg=dict(pitch=-22.0, yaw=+8.0, bend=+38.0, axis=0),
        leftLeg=dict(pitch=+21.0, yaw=+0.0, bend=+18.0, axis=0),
    ),
    8: dict(  # 用户手摆的右后收势
        easing="INOUTSINE",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=-70.9, yaw=+0.7, roll=-27.9, bend=+159.5, axis=180),
        leftArm=dict(pitch=+5.8, yaw=+17.0, roll=-11.0, bend=+20.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
    ),
}

DESCRIPTION = (
    "v3 匕首反握撕斩: 首末两帧人手摆（反握低架 → 右后收势），中间按欧拉直线补 5 帧; "
    "刃自胸前下垂 → 前探蓄势 → 扫过正下方（弧底 −84°）→ 向右后撕开，刀尖行程 34.7px; "
    "峰速落在 t5，肘全程深折 158~174° 不打直。"
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
