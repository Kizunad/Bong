#!/usr/bin/env python3
"""dagger_slash — 匕首横划（WoundKind::Cut 的匕首版）。

## 为什么不能沿用 sword_slash_down

普攻动画此前只按 `WoundKind` 选（`vfx_animation_trigger.rs::attack_anim_for_wound_kind`），
匕首砍人播的是**剑的过头劈**。握一把 0.7 格长的石刃做双手大剑的下劈，
读起来是"这人拿着看不见的剑"。

匕首和剑在身法上的硬区别，本动画围绕这三条建：

1. **肘全程不伸直。** 剑刺 impact 时 `bend=3`（整条手臂打直够远）；匕首够不着，
   伸直只是把自己的手腕送到对方面前。这里 impact 仍留 `bend=58`，收势 76。
   —— 这条由 `test_gen_dagger_anims.py` 钉死，改小了会退回成"短剑"。
2. **刀弧由腰给，不由肩给。** torso yaw 走 +24 → +42 → −20，**62° 转矩**；
   右臂自身 yaw 只走 44°。符合 §2.5「躯干扭转幅度 > 手臂自身 yaw 幅度」。
3. **副手是活的。** 刀斗里副手抬在胸前挡/抓，不是配重。这里 leftArm 走完整的
   load-snap 反相：guard 98 → LOAD 微展 84 → IMPACT 猛收 116。

## 8 tick 分段（docs/player-animation-conventions.md §1）

    tick 0  guard    侧身，刀在右胸前（FPV 可见，§3）
    tick 2  腿先动    后腿蹬地 bend 14→34（kinetic chain 起点）
    tick 3  LOAD     腰扭到极限 +42°，刀收内侧，副手微展（反相）
    tick 5  IMPACT   腰猛转正到 −20°，刀横扫过身前，副手猛收（OUTQUAD）
    tick 6  overshoot 腕再翻 10°、肘再收 6°（末端关节滞后 1 tick）
    tick 8  == tick 0

峰值错开：腿 t2 → 腰 t3 → 肩 t5 → 肘/腕 t6。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # guard —— 侧身低架，刀在右胸前
        easing="INOUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0),
        head=dict(pitch=-3, yaw=-12),
        torso=dict(pitch=+2, yaw=+24),
        rightArm=dict(pitch=-36, yaw=-14, roll=-8, bend=76, axis=180),
        leftArm=dict(pitch=-52, yaw=+24, roll=-10, bend=98, axis=180),
        rightLeg=dict(pitch=+8, yaw=+6, bend=14, z=+0.04),
        leftLeg=dict(pitch=-12, yaw=+4, bend=20, z=-0.05),
    ),
    2: dict(  # 腿先动 —— 后腿蹬地，链条从下往上启动
        easing="INOUTSINE",
        body=dict(x=+0.04, y=+0.01, z=-0.02),
        head=dict(pitch=-4, yaw=-14),
        torso=dict(pitch=+3, yaw=+32),
        rightArm=dict(pitch=-33, yaw=-20, roll=-11, bend=84, axis=180),
        leftArm=dict(pitch=-49, yaw=+27, roll=-9, bend=88, axis=180),  # load 微展
        rightLeg=dict(pitch=+16, yaw=+6, bend=34, z=+0.06),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.05),
    ),
    3: dict(  # LOAD —— 腰到极限，刀收内侧（仍在 guard 范畴，不越到反侧）
        easing="INOUTSINE",
        body=dict(x=+0.05, y=+0.02, z=-0.05),
        head=dict(pitch=-5, yaw=-16),
        torso=dict(pitch=+4, yaw=+42),
        rightArm=dict(pitch=-30, yaw=-26, roll=-14, bend=88, axis=180),
        leftArm=dict(pitch=-46, yaw=+30, roll=-8, bend=84, axis=180),
        rightLeg=dict(pitch=+18, yaw=+6, bend=40, z=+0.06),
        leftLeg=dict(pitch=-8, yaw=+4, bend=16, z=-0.04),
    ),
    5: dict(  # IMPACT —— 腰猛转正，刀横扫过身前；肘仍留 58°
        easing="OUTQUAD",
        body=dict(x=-0.03, y=-0.01, z=+0.12),
        head=dict(pitch=+2, yaw=+6),
        torso=dict(pitch=+5, yaw=-20),
        rightArm=dict(pitch=-50, yaw=+30, roll=+14, bend=58, axis=180),
        leftArm=dict(pitch=-58, yaw=+10, roll=-24, bend=116, axis=180),  # counter-pull
        rightLeg=dict(pitch=+4, yaw=+10, bend=12, z=+0.02),
        leftLeg=dict(pitch=-24, yaw=+2, bend=38, z=-0.08),
    ),
    6: dict(  # overshoot —— 末端关节滞后 1 tick：腕再翻、肘再收
        easing="OUTQUAD",
        body=dict(x=-0.02, y=-0.01, z=+0.13),
        head=dict(pitch=+3, yaw=+8),
        torso=dict(pitch=+5, yaw=-26),
        rightArm=dict(pitch=-52, yaw=+38, roll=+24, bend=52, axis=180),
        leftArm=dict(pitch=-56, yaw=+8, roll=-26, bend=112, axis=180),
        rightLeg=dict(pitch=+3, yaw=+10, bend=11, z=+0.02),
        leftLeg=dict(pitch=-26, yaw=+2, bend=40, z=-0.09),
    ),
    8: dict(  # 回 guard（与 tick 0 完全一致，连击友好）
        easing="INOUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0),
        head=dict(pitch=-3, yaw=-12),
        torso=dict(pitch=+2, yaw=+24),
        rightArm=dict(pitch=-36, yaw=-14, roll=-8, bend=76, axis=180),
        leftArm=dict(pitch=-52, yaw=+24, roll=-10, bend=98, axis=180),
        rightLeg=dict(pitch=+8, yaw=+6, bend=14, z=+0.04),
        leftLeg=dict(pitch=-12, yaw=+4, bend=20, z=-0.05),
    ),
}

DESCRIPTION = (
    "v1 匕首横划: 肘全程不伸直（impact 仍 bend=58，对比剑刺 bend=3），"
    "刀弧由腰给（torso yaw +42° → -20°，62° 转矩 > 右臂自身 44°），"
    "副手 load-snap 反相 98 → 84 → 116，腿 t2 → 腰 t3 → 肩 t5 → 腕 t6 错峰。"
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
