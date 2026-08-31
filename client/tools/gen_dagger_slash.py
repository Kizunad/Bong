#!/usr/bin/env python3
"""dagger_slash —— 匕首过顶下劈（WoundKind::Slash 的匕首版）。

## v3：招式本身被用户改掉了

v2 是横划（刀拉到右外侧、横扫过身前）。2026-08-31 用户在 Blockbench 里重摆的两端把它
改成了**过顶下劈**：

| | 握把 | 刃向仰角 | 肘 bend |
|---|---|---|---|
| t0 起手 | (−2.8, 30.7, −9.2) | **+68°**（刃朝天，刀尖 y=41.1） | 12°（几乎打直） |
| t8 收势 | (+0.2, 16.4, −11.4) | −16°（前下） | 1.6°（打直） |

两端只差 `rightArm.pitch` 72°（−112.1 → −40.2），其余轴几乎不动 —— 整条就是**肩关节
绕 X 轴的一次摆动**。

## 为什么这条不反解，直接写欧拉角

反解在这里帮倒忙。目标点落在手臂够得着的**边缘**上（握把要到 y≈31，肩在 y≈22，
臂展约 12px），最小二乘在两个等价分支之间跳，插值路径随之乱窜 —— 实测刀尖单格瞬移
16.7px、刃向偏离 slerp 116°、17/65 帧前臂穿头。改成沿用户那条欧拉直线逐帧写，路径
按定义就是设计好的那条：瞬移降到 5.8px、刃向偏离 18°、穿头 0 帧。

**这不是"放弃反解"，是选对工具**：单关节单轴的摆动，欧拉空间就是它的自然参数化；
反解适合的是「手要到某个点、刃要指某个方向」这种被 display 变换耦合住的多轴问题
（直刺、反握上撕、转刀仍在用）。

## 门禁跟着招式一起改（`knife_anim_gates.SUITE`）

- **刀尖高度 / 刃仰角改成从撞击帧起算**（`tip_since` / `elev_since` = 5.0）。蓄势段
  举刀过顶是设计（用户手摆的起手式刀尖就在 y=41.1）；一刀切只能把门限抬到 45，那就
  成了纯棘轮什么都锁不住。真正的败笔是**劈完了刀还举在脸前**，所以窗口从 t5 起算、
  门限维持在下巴线 26 —— 实测撞击帧刀尖 25.1，余量 0.9px（很紧，这是设计：刃正好在
  撞击帧扫过下巴高度）。蓄势段由 `gate_head` / `gate_selfclip` 兜着。
- **肘不打直收在蓄势段**（`elbow_until` = 3.0、下限 10）。用户把整条摆成直臂挥
  （末帧 bend 1.6°），这条对收势段不再成立；蓄势段仍有意义 —— 实测 12~21°，一个
  从头锁死直臂的版本会被它抓住。
- **收势闭合收窄到下盘**：用户没动 body/torso/head/两腿，那边首末仍逐轴相同；手臂
  那一头交给 `gate_decel` + `gate_blendout`。

## 8 tick 分段

    tick 0  起手     用户手摆：刀举过头顶、刃朝天
    tick 2  再仰一寸  向后蓄（pitch −121），肘微收
    tick 3  LOAD     蓄势极点（pitch −127，bend 21）；腰扭到极限
    tick 4  过中线    刃扫过水平前一格（这一帧钉住弧线中点）
    tick 5  IMPACT   刃扫到下巴高度（刀尖 y=25.1，仰角 +16°）；峰速实测落在 t5.00
    tick 6  overshoot 继续劈到最低（tip y=12.5），腕滞后
    tick 8  用户手摆的低位伸展定格

## easing

t0/t2 蓄势 OUT 族，**t3/t4 发力 INCUBIC**（连着两格单调加速到撞击），t5 余势
OUTQUAD，t6 收势 INOUTSINE。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # 用户手摆的过顶高架 —— 刃朝天，刀尖 y=41.1
        easing="OUTSINE",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=-112.1, yaw=+1.3, roll=-5.6, bend=+12.0, axis=180),
        leftArm=dict(pitch=-41.5, yaw=+60.0, roll=-49.0, bend=+15.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
    ),
    2: dict(  # 再仰一寸 —— 向后蓄（pitch −121），肘微收
        easing="OUTQUAD",
        body=dict(x=+0.04, y=+0.01, z=-0.03, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+26.0),
        torso=dict(pitch=+5.0, yaw=+24.0),
        rightArm=dict(pitch=-121.0, yaw=+3.0, roll=-9.0, bend=+17.0, axis=180),
        leftArm=dict(pitch=-39.0, yaw=+60.0, roll=-49.0, bend=+16.0, axis=180),
        rightLeg=dict(pitch=-10.0, yaw=+4.0, bend=+18.0, axis=0),
        leftLeg=dict(pitch=+16.0, yaw=+2.0, bend=+30.0, axis=0),
    ),
    3: dict(  # LOAD —— 蓄势极点（pitch −127、bend 21）；腰扭到极限
        easing="INCUBIC",
        body=dict(x=+0.05, y=+0.02, z=-0.05, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+24.0),
        torso=dict(pitch=+6.0, yaw=+32.0),
        rightArm=dict(pitch=-127.0, yaw=+4.0, roll=-12.0, bend=+21.0, axis=180),
        leftArm=dict(pitch=-37.0, yaw=+60.0, roll=-49.0, bend=+17.0, axis=180),
        rightLeg=dict(pitch=-8.0, yaw=+4.0, bend=+16.0, axis=0),
        leftLeg=dict(pitch=+18.0, yaw=+2.0, bend=+34.0, axis=0),
    ),
    4: dict(  # 过中线 —— 刃扫过水平前一格，钉住弧线中点
        easing="INCUBIC",
        body=dict(x=+0.02, y=+0.01, z=+0.05, yaw=-30.0),
        head=dict(pitch=+2.0, yaw=+28.0),
        torso=dict(pitch=+7.0, yaw=+8.0),
        rightArm=dict(pitch=-104.0, yaw=+5.5, roll=-15.0, bend=+16.0, axis=180),
        leftArm=dict(pitch=-34.0, yaw=+60.0, roll=-49.0, bend=+16.0, axis=180),
        rightLeg=dict(pitch=-15.0, yaw=+6.0, bend=+24.0, axis=0),
        leftLeg=dict(pitch=+20.0, yaw=+1.0, bend=+22.0, axis=0),
    ),
    5: dict(  # IMPACT —— 刃扫到下巴高度（刀尖 y=25.1，仰角 +16°）
        easing="OUTQUAD",
        body=dict(x=-0.02, y=-0.01, z=+0.14, yaw=-30.0),
        head=dict(pitch=+4.0, yaw=+32.0),
        torso=dict(pitch=+8.0, yaw=-18.0),
        rightArm=dict(pitch=-66.0, yaw=+7.5, roll=-19.0, bend=+8.0, axis=180),
        leftArm=dict(pitch=-30.0, yaw=+60.0, roll=-49.0, bend=+15.0, axis=180),
        rightLeg=dict(pitch=-22.0, yaw=+8.0, bend=+32.0, axis=0),
        leftLeg=dict(pitch=+22.0, yaw=+0.0, bend=+12.0, axis=0),
    ),
    6: dict(  # overshoot —— 继续劈到最低（刀尖 y=12.5），腕滞后
        easing="INOUTSINE",
        body=dict(x=-0.01, y=-0.01, z=+0.16, yaw=-30.0),
        head=dict(pitch=+5.0, yaw=+34.0),
        torso=dict(pitch=+8.0, yaw=-22.0),
        rightArm=dict(pitch=-36.0, yaw=+9.0, roll=-23.0, bend=+3.0, axis=180),
        leftArm=dict(pitch=-27.5, yaw=+60.0, roll=-49.0, bend=+15.0, axis=180),
        rightLeg=dict(pitch=-24.0, yaw=+8.0, bend=+34.0, axis=0),
        leftLeg=dict(pitch=+23.0, yaw=+0.0, bend=+10.0, axis=0),
    ),
    8: dict(  # 用户手摆的低位伸展定格
        easing="INOUTSINE",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=-40.1, yaw=+9.4, roll=-23.1, bend=+1.6, axis=180),
        leftArm=dict(pitch=-26.5, yaw=+60.0, roll=-49.0, bend=+15.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
    ),
}

DESCRIPTION = (
    "v3 匕首过顶下劈: 首末两帧人手摆（刃朝天的过顶高架 → 低位伸展定格），中间按欧拉"
    "直线补 5 帧; 刀尖行程 30px，撞击帧刃扫到下巴高度(y=25.1)，峰速落在 t5; 蓄势段"
    "肘收到 21°，收势靠 endTick→stopTick 两 tick 混出带回站架。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="dagger_slash",
        description=DESCRIPTION,
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
