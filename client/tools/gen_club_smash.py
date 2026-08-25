#!/usr/bin/env python3
"""club_smash — 木棍过顶抡砸（`WoundKind::Blunt` 的钝器版）。

## 为什么不能沿用 fist_punch_right

普攻动画只按 `WoundKind` 选（`vfx_animation_trigger.rs::attack_anim_for_wound_kind`），
而 `WeaponKind::Staff | Fist → WoundKind::Blunt`（`npc/equipment.rs:156`）。于是**握着
一根 0.84 格长的木棍打人，播的是空手直拳**——手臂做着出拳的直线，棍子跟着手横在半空，
读起来是"这人拿着根棍子在打空拳"。同 `gen_dagger_slash.py` 那次的理由。

## 钝器和刀 / 拳的三条硬区别

1. **打击点在长弧的末端，不在手上。** 拳和匕首的杀伤在拳面 / 刃口，手到哪里打到哪里；
   棍的杀伤在**头部**，靠的是半径 × 角速度。所以这条动画的主角不是手的位移，是**棍头
   的行程**：LOAD → IMPACT 之间棍头从高于肩 11.0px 落到低于肩 16.5px，竖直落差 27.5px，
   是同段手位移的 3 倍还多。
2. **肘要打开，但不锁死。** 匕首 impact 仍留 `bend=58`（够不着，伸直只是把手腕送出去），
   剑刺打直到 3。棍取中间：impact `bend=22`。真挥棍不会锁肘——锁了自己受力，但也不会
   像匕首那样全程蜷着，那样等于放弃了棍长带来的力矩。

   **这里有个反直觉的点，靠猜必然做错**：display 的 `Rx(-80)` 让棍沿前臂出虎口，于是
   `pitch` 和 `bend` 在同一个旋向上**相加**决定棍的朝向。第一版按"手臂往前伸就是砸"写
   了 `pitch=-58 / bend=34`，量出来棍仰角是 **+15.7°——还朝上**，整条动画读成"举着棍往前
   捅"。真正砸下去的那一档是 `pitch≈+2 / bend≈22`（手臂近乎垂下、肘几乎打开），棍才顺着
   手臂继续朝前下方指。数值是扫格子解出来的，不是调出来的。
3. **收不住。** 重心在前的兵器一旦抡出去就得让它走完，回程是**人把它拽回来**，不是它
   自己弹回来。这条动画因此是 **12 tick 而不是 8**，收势占了 5 tick（详见分段那节）。

## guard 就是扛棍，不是垂手

标配 §2.3「发力肢禁止反向 anticipation」要求发力肢从 tick 0 到 impact **单调朝打击方向**
运动。抡砸的打击方向是**向下**，那"把棍举起来"岂不是天然违规？

解法是**把举起来的状态放进 guard**：tick 0 就是棍已经举在右肩前上方
（`rightArm.pitch=-14 / bend=132`，前臂折上去、棍顺着前臂朝上，实测棍头高于肩 9.8px、
仰角 +64°）。t0 → t4 只再抬 1.2px 到竖直，随后一路落到肩下 16.5px：**举起来那段占整段
行程的 4%，落下那段占 96%**，读感上完全不构成"先反向甩一下"。

顺带说清楚：`fist_punch_right` v10 这条基线本身也有 chamber（右臂 pitch −88 → −55 →
−100，回拉 33° 再前伸 45°），§2.3 真正禁的是**幅度与主行程同量级的反向**（那会让手的
空间轨迹变 V 形），不是禁止一切蓄势。本条 4% 的抬手远在那条线以下。

guard 同时满足 §2.1（静帧就能认出这是要抡）和 §3（手在肩前 3.6px，FPV 视野里看得见）。

## 12 tick 分段（docs/player-animation-conventions.md §1）

    tick 0  guard     棍斜扛在右肩外侧，副手收在胸前
    tick 3  腿先动     后腿蹬地 bend 16→38（kinetic chain 起点），棍继续往上竖
    tick 5  LOAD      腰扭到极限 +36°，棍竖到最高，副手微展（反相）
    tick 7  IMPACT    腰猛转正到 −18° 并前折 +14°，棍抡到身前下方，副手猛收（counter-pull）
    tick 8  overshoot 肘再开 6°、腕再翻 10°，棍沉过腿侧（末端关节滞后 1 tick）
    tick 9  低位滞留   棍几乎不动一拍 —— 这一拍就是"收不住"
    tick 12 == tick 0（3 tick 把棍拖回肩上）

峰值错开：腿 t3 → 腰 t5 → 肩 t7 → 肘/腕 t8。impact 落在 7/12 = 58%，标配是 60%。

**为什么是 12 tick 而不是标配的 8**：标配那条 8 tick 模板给收势留 2~3 tick，按在棍上会
读成"打完瞬间刹住"，那是轻兵器才做得到的事。这里把收势拉到 5 tick——overshoot 1 +
**低位滞留 1** + 拖回 3。

关键的是中间那一拍滞留，不是总时长。第一版 10 tick 没有滞留帧，棍头速度从打击峰值一路
连续地拐进回程，读起来是"砸下去顺势弹回来"；加了 t8→t9 这一拍近乎静止（实测 1.1 →
0.7 px/tick）之后，才读成"砸到底了、停在那儿、然后人把它拖回来"。

回程峰速 21.7 px/tick，是打击峰速（58.1）的 37%——这个比值两版一样，也不该靠它去调：
重量感来自**节奏**（慢起 / 爆发 / 硬停 / 滞留 / 拖回），不是来自把回程调慢。

## 站架用 `body.yaw`，前折要给腿补偿

- **`body.yaw = -16` 恒定**。`torso.*` 只作用于躯干 ModelPart，头/臂/腿各自独立
  （conventions §L243）——只用 torso 的话胯和腿全程正对前方，那是"扭了下腰"不是站架。
  取 -16 而不是匕首那条的 -34：抡棍是**双脚更开、更正对目标**的架势，侧身太多会把棍的
  弧线甩到身侧去。头反向补 +16 保持世界朝向。
- **`torso.pitch` 前折配 `body.z` 前移**。腿不是 torso 的子节点（memory:
  torso/legs 不共祖），单给 torso 前折会在腰上撕开一道缝。impact 的 +14° 配 `body.z
  +0.16`，前腿同时承重；再大就必须连腿一起同向 pitch，而 `leg.pitch ≤ 40°` 的库坑
  （§7.2）又不允许——所以前折封顶在 +14。

## easing 的管辖方向（conventions §15）

每帧的 easing 管「**本帧 → 下一帧**」，不是「怎么到达本帧」。所以按段写在起始侧：
t0/t2 蓄势 OUT 族（松出去、到 LOAD 时几乎静止），**t4 发力 INCUBIC**（从静止单调加速，
最快点落在撞击帧），t6 余势 OUTQUAD 卸力，t7 收势 INOUTSINE。
把 OUTQUAD 写在撞击帧 t6 上是最容易犯的错——那管的是撞击**之后**。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # guard —— 棍斜扛在右肩外侧（实测棍头高于肩 7.1px、偏右 10.8px、仰角 +38°）
        easing="OUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-16),
        head=dict(pitch=-2, yaw=+18),
        torso=dict(pitch=+2, yaw=+18),
        rightArm=dict(pitch=-52, yaw=+4, roll=-52, bend=130, axis=180),
        leftArm=dict(pitch=-34, yaw=+8, roll=-30, bend=100, axis=180),
        rightLeg=dict(pitch=+8, yaw=+6, bend=16, z=+0.04),
        leftLeg=dict(pitch=-12, yaw=+4, bend=20, z=-0.05),
    ),
    3: dict(  # 腿先动 —— 后腿蹬地，链条从下往上启动；棍继续往上竖
        easing="OUTQUAD",
        body=dict(x=+0.04, y=+0.01, z=-0.03, yaw=-16),
        head=dict(pitch=-3, yaw=+16),
        torso=dict(pitch=+3, yaw=+26),
        rightArm=dict(pitch=-56, yaw=+12, roll=-47, bend=129, axis=180),
        leftArm=dict(pitch=-30, yaw=+12, roll=-26, bend=90, axis=180),  # load 微展
        rightLeg=dict(pitch=+18, yaw=+6, bend=38, z=+0.06),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.05),
    ),
    5: dict(  # LOAD —— 腰到极限，棍竖到最高（棍头高于肩 9.2px，仰角 +51°）
        easing="INCUBIC",
        body=dict(x=+0.05, y=+0.02, z=-0.06, yaw=-16),
        head=dict(pitch=-5, yaw=+14),
        torso=dict(pitch=+4, yaw=+36),
        rightArm=dict(pitch=-60, yaw=+20, roll=-42, bend=128, axis=180),
        leftArm=dict(pitch=-26, yaw=+16, roll=-22, bend=82, axis=180),
        rightLeg=dict(pitch=+20, yaw=+6, bend=44, z=+0.06),
        leftLeg=dict(pitch=-8, yaw=+4, bend=15, z=-0.04),
    ),
    7: dict(  # IMPACT —— 腰猛转正 + 前折，棍抡到身前下方（棍头低于肩 15.1px、朝前 10.7px）
        easing="OUTQUAD",
        body=dict(x=-0.04, y=-0.02, z=+0.16, yaw=-16),
        head=dict(pitch=+14, yaw=+26),
        torso=dict(pitch=+14, yaw=-18),
        rightArm=dict(pitch=-14, yaw=+18, roll=+2, bend=22, axis=180),
        leftArm=dict(pitch=-24, yaw=+4, roll=-40, bend=120, axis=180),  # counter-pull
        rightLeg=dict(pitch=+6, yaw=+10, bend=14, z=+0.02),
        leftLeg=dict(pitch=-26, yaw=+2, bend=40, z=-0.09),
    ),
    8: dict(  # overshoot —— 末端关节滞后 1 tick：肘再开 6°、腕再翻 10°，棍沉过腿侧
        easing="OUTSINE",
        body=dict(x=-0.03, y=-0.03, z=+0.19, yaw=-16),
        head=dict(pitch=+16, yaw=+28),
        torso=dict(pitch=+13, yaw=-24),
        rightArm=dict(pitch=+2, yaw=+22, roll=+12, bend=16, axis=180),
        leftArm=dict(pitch=-22, yaw=+2, roll=-42, bend=116, axis=180),
        rightLeg=dict(pitch=+4, yaw=+10, bend=12, z=+0.02),
        leftLeg=dict(pitch=-28, yaw=+2, bend=42, z=-0.10),
    ),
    9: dict(  # 低位滞留 —— 棍几乎不动一拍。这一拍就是"收不住"：重心在前的兵器抡到底
        #        之后要靠人去把它拽回来，不是弹回来
        easing="INOUTSINE",
        body=dict(x=-0.02, y=-0.02, z=+0.17, yaw=-16),
        head=dict(pitch=+13, yaw=+26),
        torso=dict(pitch=+11, yaw=-20),
        rightArm=dict(pitch=+4, yaw=+21, roll=+10, bend=19, axis=180),
        leftArm=dict(pitch=-24, yaw=+4, roll=-38, bend=112, axis=180),
        rightLeg=dict(pitch=+5, yaw=+9, bend=13, z=+0.02),
        leftLeg=dict(pitch=-26, yaw=+2, bend=40, z=-0.09),
    ),
    12: dict(  # 回 guard（与 tick 0 完全一致，连击友好）—— 3 tick 把棍拖回肩上
        easing="INOUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-16),
        head=dict(pitch=-2, yaw=+18),
        torso=dict(pitch=+2, yaw=+18),
        rightArm=dict(pitch=-52, yaw=+4, roll=-52, bend=130, axis=180),
        leftArm=dict(pitch=-34, yaw=+8, roll=-30, bend=100, axis=180),
        rightLeg=dict(pitch=+8, yaw=+6, bend=16, z=+0.04),
        leftLeg=dict(pitch=-12, yaw=+4, bend=20, z=-0.05),
    ),
}


DESCRIPTION = (
    "v1 木棍过顶抡砸: guard 即扛棍（棍在右肩，全程单调落下，无反向蓄势），"
    "打击点在棍头不在手上（LOAD→IMPACT 棍头竖直落差 27.5px，是手位移的 3 倍），"
    "肘开到 bend=22（匕首 58 / 剑刺 3 之间），10 tick 留 4 tick 随势收不住，"
    "腿 t2 → 腰 t4 → 肩 t6 → 腕 t7 错峰。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="club_smash",
        description=DESCRIPTION,
        end_tick=12,
        stop_tick=14,
        is_loop=False,
    )
