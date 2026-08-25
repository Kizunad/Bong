#!/usr/bin/env python3
"""dagger_stab — 匕首低位直刺（WoundKind::Pierce 的匕首版）。

## 为什么不能沿用 sword_stab

同 `gen_dagger_slash.py` 的理由：普攻动画只按 `WoundKind` 选，匕首刺人播的是
剑的直刺。`sword_stab` 的动作核心是 **body.z +0.28 的大前扑 + 手臂完全打直
（bend=3）**——那是一米长的剑够人的方式。匕首照搬就成了"拿着看不见的剑扑刺"。

三条硬区别：

1. **前扑距离减半。** body.z 峰值 +0.18（剑刺 +0.28~+0.32）。匕首本来就是贴身
   兵器，大前扑意味着把自己送进对方的攻击距离。
2. **肘留 22°。** 剑刺打直到 3；这里最深只到 22，收势 86。
3. **副手在前面工作，不是配重。** 刀斗里空手负责探距、格挡、抓控。这里 leftArm
   guard 就前探（pitch=-64），LOAD 再探远（反相 bend 72 → 56），IMPACT 猛收回
   护胸（bend 112）——读成"左手先够到、右手跟着捅进去"。

## 8 tick 分段（docs/player-animation-conventions.md §1）

    tick 0  guard    低位持刀、刀尖朝前，副手前探（FPV 可见，§3）
    tick 2  腿先动    后腿蹬地 bend 16→38
    tick 3  LOAD     刀回胯 bend 102，身体后坐 z −0.06，副手探到最远（反相）
    tick 5  IMPACT   body.z +0.18 前送，刀直出 bend 22，副手猛收（OUTQUAD）
    tick 6  overshoot 再送 0.04 + 腕 roll +10（末端滞后 1 tick）
    tick 8  == tick 0

峰值错开：腿 t2 → 腰 t3 → 肩 t5 → 肘/腕 t6。

## 姿态被推翻重调过一次（round 3/3）

同 `gen_dagger_slash.py` 同名小节：round 1/2 是照着挂点算错的预览调的。修对之后
量出来 guard/LOAD 的刃仰角是 +31~+40°，刀举在肩上——不是"低位持刀"。

修法只动竖直面：`rightArm.pitch` t0/t2/t8 +20、t3 +24，**t5/t6 一格不动**
（撞击帧的刃仰角本来就是 +1.7°/+4.4°，已经是水平直刺，加 pitch 反而会把刺变成
往下戳）；`leftArm.pitch` 前段 +32、撞击段 +26。yaw / roll / z / bend / easing 全部
保持原值。修完 guard 刃仰角 +11°、撞击 +1.7°，手从肩高回到上胸。
`DaggerBladeReadTest.test_the_thrust_actually_levels_the_blade` 钉住撞击帧 |仰角|<15°。

## 站架：由 `body.yaw` 给，不是 `torso.yaw`

`torso.*` 只作用于躯干 ModelPart，头/臂/腿各自独立（conventions §L243）。所以
round 1~3 那个"侧身"其实是 **24° 的躯干扭转**，胯和腿全程正对前方——量出来右脚
只比左脚靠后 1.4px。症状很隐蔽：眼睛把**胸口**读成朝向，于是 3/4 机位看起来才
像正面（用户 2026-08-25 一眼指出）。

`body.yaw` 是唯一能转整个人（含头、腿、手持物）的通道，这里取 **-34° 恒定**：
34° = FRONT 机位(180) 与 3/4 机位(146) 的夹角，符号取负因为**转机位 +θ ≡ 转身体
-θ**（`_bodyyaw_sweep.png` 逐格比对确认）。于是原先只有在 3/4 机位才看得到的那个
姿态，现在正对敌人就是这样。

两条配套约束：

- **恒定**，不逐帧变。站架是站架，挥砍的转体由 `torso.yaw` 给（它本来就在走
  +24 → +42 → -20 那条 62° 曲线）。body 跟着动的话脚会在地上打滑。
- **头反向补 +34°**，保持世界朝向不变（本条动画原本是 -12°，补偿后仍是 -12°）。
  不补的话角色是"侧着身、还把脸扭开"，看不到敌人。

纯 yaw 不动高度，所以 round 3/3 那批刃向锁（仰角 / 刀尖高度）数值一格未变，
仍然绿。`DaggerStanceTest` 另外钉住"恒定 + 头朝向 + body 不许有 pitch/roll"。

## easing 的管辖方向（conventions §15）

每帧的 easing 管「本帧 → 下一帧」，不是「怎么到达本帧」，所以按段写在起始侧：
t0/t2 蓄势 OUT 族，**t3 发力 INCUBIC**（单调加速到撞击），t5 余势 OUTQUAD，
t6 收势 INOUTSINE。详见 `gen_dagger_slash.py` 同名小节记的那次踩坑。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # guard —— 低位持刀（实测刃仰 +11°），副手前探控距
        easing="OUTSINE",
        body=dict(x=+0.03, y=0.0, z=0.0, yaw=-34),
        head=dict(pitch=+2, yaw=+26),
        torso=dict(pitch=+4, yaw=+18),
        rightArm=dict(pitch=-4, yaw=-10, roll=-6, bend=86, axis=180),
        leftArm=dict(pitch=-32, yaw=+26, roll=-6, bend=72, axis=180),
        rightLeg=dict(pitch=+8, yaw=+4, bend=16, z=+0.04),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.05),
    ),
    2: dict(  # 腿先动 —— 后腿蹬地
        easing="OUTQUAD",
        body=dict(x=+0.05, y=+0.01, z=-0.03, yaw=-34),
        head=dict(pitch=+2, yaw=+24),
        torso=dict(pitch=+5, yaw=+26),
        rightArm=dict(pitch=+0, yaw=-16, roll=-9, bend=94, axis=180),
        leftArm=dict(pitch=-38, yaw=+28, roll=-8, bend=60, axis=180),  # 探得更远
        rightLeg=dict(pitch=+18, yaw=+4, bend=38, z=+0.06),
        leftLeg=dict(pitch=-8, yaw=+4, bend=16, z=-0.05),
    ),
    3: dict(  # LOAD —— 刀回胯，后坐；副手探到最远（与右手反相）
        easing="INCUBIC",
        body=dict(x=+0.06, y=+0.02, z=-0.06, yaw=-34),
        head=dict(pitch=+1, yaw=+22),
        torso=dict(pitch=+6, yaw=+34),
        rightArm=dict(pitch=+10, yaw=-22, roll=-12, bend=102, axis=180),
        leftArm=dict(pitch=-40, yaw=+30, roll=-10, bend=56, axis=180),
        rightLeg=dict(pitch=+20, yaw=+4, bend=44, z=+0.06),
        leftLeg=dict(pitch=-6, yaw=+4, bend=14, z=-0.04),
    ),
    5: dict(  # IMPACT —— 前送 0.18（剑刺是 0.28），刀直出但肘留 22°
        easing="OUTQUAD",
        body=dict(x=-0.02, y=-0.02, z=+0.18, yaw=-34),
        head=dict(pitch=+5, yaw=+36),
        torso=dict(pitch=+7, yaw=-10),
        rightArm=dict(pitch=-58, yaw=+10, roll=+10, bend=22, axis=180),
        leftArm=dict(pitch=-24, yaw=+6, roll=-22, bend=112, axis=180),  # 猛收护胸
        rightLeg=dict(pitch=+2, yaw=+8, bend=10, z=+0.02),
        leftLeg=dict(pitch=-26, yaw=+2, bend=40, z=-0.09),
    ),
    6: dict(  # overshoot —— 再送一寸 + 腕翻
        easing="INOUTSINE",
        body=dict(x=-0.01, y=-0.02, z=+0.22, yaw=-34),
        head=dict(pitch=+6, yaw=+38),
        torso=dict(pitch=+7, yaw=-12),
        rightArm=dict(pitch=-64, yaw=+13, roll=+20, bend=16, axis=180),
        leftArm=dict(pitch=-22, yaw=+4, roll=-24, bend=108, axis=180),
        rightLeg=dict(pitch=+1, yaw=+8, bend=9, z=+0.02),
        leftLeg=dict(pitch=-28, yaw=+2, bend=42, z=-0.10),
    ),
    8: dict(  # 回 guard（与 tick 0 完全一致，连击友好）
        easing="INOUTSINE",
        body=dict(x=+0.03, y=0.0, z=0.0, yaw=-34),
        head=dict(pitch=+2, yaw=+26),
        torso=dict(pitch=+4, yaw=+18),
        rightArm=dict(pitch=-4, yaw=-10, roll=-6, bend=86, axis=180),
        leftArm=dict(pitch=-32, yaw=+26, roll=-6, bend=72, axis=180),
        rightLeg=dict(pitch=+8, yaw=+4, bend=16, z=+0.04),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.05),
    ),
}

DESCRIPTION = (
    "v1 匕首低位直刺: 前扑 body.z +0.18（剑刺 +0.28，贴身兵器不过度前送），"
    "肘留 bend=22（剑刺打直到 3），副手先探后收 72 → 56 → 112 读成"
    "「左手够到、右手跟进」，腿 t2 → 腰 t3 → 肩 t5 → 腕 t6 错峰。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="dagger_stab",
        description=DESCRIPTION,
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
