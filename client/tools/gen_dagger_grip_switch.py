#!/usr/bin/env python3
"""dagger_grip_switch —— 匕首换握（正握 ↔ 反握）。

## v4：这条动画此前根本没在换握

v2/v3 都想用手臂表达换握，那是**表达不出来的**：手持物被 display 变换焊死在前臂上，
刃相对前臂的朝向按定义恒定（本仓实测：v3 全程 0.00°）。转手臂只会把整条小臂连刀
一起拧过去，观众读到的是"胳膊别了一下"。当时的 `gate_flip` 也被这件事逼歪了 ——
它只好退而量"世界刃向转过多少度"，而那个量对着一次普通挥砍也有 87°，分辨不了。

真正管这件事的是 PlayerAnimator 的 `rightItem` 骨头（`HeldItemMixin.changeItemLocation`
把它乘进手持物矩阵）。2026-09-01 用户在 Blockbench 里给 `dagger_right_pitch` 打了
t0=0° / t8≈180° 两帧，把「换握」明确写了出来。本版按它重做。

## 换握 = 绕**刃口轴**转 180°，不是绕刀面法线

匕首局部 +Y 是刃向、+X 是刃口方向（刃宽 1.6px）、+Z 是刀面法线（厚 0.32px）。

    绕 X 转 180°  刃口朝向不变、刀身整个倒转  ← 正握 ↔ 反握，人手真这么翻
    绕 Z 转 180°  刃倒转但刃口翻到另一侧      ← 另一个动作

用户打的正是 X。换算走 `anim_common.item_spin`（R_item = R_disp·R_extra·R_disp⁻¹），
不在本文件里写死三个角 —— θ 一改三个角全变。

## 旋向取负：半程刃朝向身体外侧

θ 只由两端决定不了旋向（±180 落点相同），是本版选的。实测半程 θ=−90 时刃在前臂系里
指向 (−1, 0, 0) = 玩家**右**外侧，θ=+90 则指向左、横穿身体前方，会往躯干/头上蹭。
按间隙选负向，不是按手感。

## 8 tick 分段

    tick 0  用户手摆的正握低架（θ=0）
    tick 2  反向预备 —— 刃先往回压 12°，拳微沉
    tick 3  起转（θ=−18），easing 切 IN 起爆
    tick 4  刃过横（θ=−78），此时刃指向右外侧，离身体最远
    tick 5  转过大半（θ=−140），拳同时抬到位
    tick 6  基本到位（θ=−172），拳略过冲
    tick 8  用户手摆的反握收势（θ=−180）

## 一条留给系统层的账

本条是 one-shot emote：`stopTick` 之后 `rightItem` 会随混出回到 0，也就是**刀会自己
转回正握**。要让"换完握就一直是反握"，得由持握状态层（一条保持性的 idle/hold 动画）
接手，单条 emote 表达不了。手臂姿态同理。
"""

from anim_common import emit_json, item_spin

DAGGER_DISPLAY_ROT = (0.0, -90.0, 55.0)
BLADE_EDGE_AXIS = (1.0, 0.0, 0.0)


def grip(theta_deg: float) -> dict:
    """刀在手里绕刃口轴转 theta 度。0 = 正握，−180 = 反握。"""
    return item_spin(DAGGER_DISPLAY_ROT, BLADE_EDGE_AXIS, theta_deg)


POSE = {
    0: dict(  # 用户手摆的正握低架
        easing="OUTQUAD",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=+36.7, yaw=-3.4, roll=+1.5, bend=+45.5, axis=180),
        leftArm=dict(pitch=+11.7, yaw=+27.6, roll=-17.3, bend=+15.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
        rightItem=grip(0.0),
    ),
    2: dict(  # 反向预备：刃先往回压一点，拳微沉
        easing="OUTQUAD",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+15.0),
        rightArm=dict(pitch=+40.0, yaw=-5.0, roll=+0.0, bend=+41.0, axis=180),
        leftArm=dict(pitch=+9.0, yaw=+28.5, roll=-15.9, bend=+17.7, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
        rightItem=grip(+12.0),
    ),
    3: dict(  # 起转
        easing="INCUBIC",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+15.0),
        rightArm=dict(pitch=+38.0, yaw=-2.0, roll=+3.0, bend=+44.0, axis=180),
        leftArm=dict(pitch=+7.0, yaw=+29.0, roll=-14.0, bend=+20.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
        rightItem=grip(-18.0),
    ),
    4: dict(  # 刃过横：指向右外侧，离身体最远
        easing="INCUBIC",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+16.0),
        rightArm=dict(pitch=+20.0, yaw=+6.0, roll=+10.0, bend=+52.0, axis=180),
        leftArm=dict(pitch=+4.7, yaw=+30.2, roll=-12.0, bend=+24.1, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
        rightItem=grip(-78.0),
    ),
    5: dict(  # 转过大半，拳抬到位
        easing="OUTQUAD",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+16.0),
        rightArm=dict(pitch=-2.0, yaw=+14.0, roll=+18.0, bend=+58.0, axis=180),
        leftArm=dict(pitch=+1.1, yaw=+31.3, roll=-10.0, bend=+25.8, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
        rightItem=grip(-140.0),
    ),
    6: dict(  # 基本到位，拳略过冲
        easing="INOUTSINE",
        body=dict(x=+0.02, y=+0.01, z=+0.00, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+27.0),
        torso=dict(pitch=+4.0, yaw=+16.0),
        rightArm=dict(pitch=-16.0, yaw=+21.0, roll=+27.0, bend=+59.0, axis=180),
        leftArm=dict(pitch=-2.3, yaw=+32.6, roll=-7.7, bend=+25.5, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, bend=+21.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, bend=+21.0, axis=0),
        rightItem=grip(-172.0),
    ),
    8: dict(  # 用户手摆的反握收势
        easing="INOUTSINE",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=-10.1, yaw=+19.4, roll=+24.6, bend=+57.1, axis=180),
        leftArm=dict(pitch=-7.1, yaw=+28.4, roll=-5.4, bend=+20.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
        rightItem=grip(-180.0),
    ),
}

DESCRIPTION = (
    "v4 匕首换握: 刀在手里绕刃口轴转 180°（rightItem 骨头），正握 → 反握; 首末两帧人手摆, "
    "t2 反向预备 12°、t4 刃过横指向右外侧、t6 转到 172° 略过冲; 手只抬 8.4px 前伸 5.6px, "
    "读作转刀不是挥刀。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="dagger_grip_switch",
        description=DESCRIPTION,
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
