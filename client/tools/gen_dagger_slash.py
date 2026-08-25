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
2. **腰给的是读感，不是刀弧。** torso yaw 走 +24 → +42 → −20 共 62° 转矩，符合
   §2.5「躯干扭转幅度 > 手臂自身 yaw」——但要清楚它**带不动刀**：`torso.*` 只作用
   于躯干 ModelPart 本体，头/臂/腿各自独立（conventions §L243）。刀的位移全部来自
   右臂自身的 pitch/yaw/roll/bend。想让腰真的甩动手臂只能用 `body.yaw`，代价是头
   也跟着转、读成「整个人转身」（§L246），本招不采用。
   实测刀尖 LOAD→IMPACT 位移 16.9，剑横劈 13.4，够用。
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

## 姿态被推翻重调过一次（round 3/3）

round 1/2 的姿态是照着一个**手持物挂点算错的预览**调出来的：那版预览缺
`R_ATTACH`、把 display translation 当成旋转后再加、还漏了方块中心重定心，刀根本
没在手上。等挂点修对了再量，才发现这套姿态每一 tick 的**刃仰角都在 +33~+43°**，
而修挂点之前用原版剑那种⊥握法量出来是 +63~+78°、刃尖越过肩往身后指——渲出来
读成"举着火把"，和下面写的"低架、刀在右胸前"完全是两回事。

修法只动**竖直面**这一路：`rightArm.pitch` 全帧 +20、`leftArm.pitch` 全帧 +30，
把手从肩高压回上胸；yaw / roll / z / bend / easing **一格没动**——横向编排（腰转
62°、counter-pull、错峰）是这条动画的骨架，和读错刃向无关。修完刃仰角落在
+14~+24°、刀尖从"过头顶"回到胸口高度。由 `DaggerBladeReadTest` 钉死（<45°、
且前向分量恒 >0）。

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

每帧的 easing 管的是「**本帧 → 下一帧**」这一段，不是「怎么到达本帧」——
PlayerAnimator 的 `isEasingBefore` 默认 false，取的是 `before.ease`。所以按段写在
起始侧：t0/t2 蓄势用 OUT 族（松出去、到 LOAD 时几乎静止），**t3 发力用 INCUBIC**
（从静止单调加速，最快点落在撞击帧），t5 余势 OUTQUAD 卸力，t6 收势 INOUTSINE。

这条一开始写错过：OUTQUAD 写在 t5/t6 上，本意是「到撞击时减速」，实际管的是撞击
**之后**那两段——量出来峰速落在 t6.0 的收招段而不是撞击，且收势起手有 0.10 → 1.53
的 15 倍速度断层。改完峰速回到 t4.8（撞击前最后一帧）并提高 70%。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # guard —— 侧身低架，刀在右胸前（实测刃仰 +14°、刀尖在胸口高度）
        easing="OUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-34),
        head=dict(pitch=-3, yaw=+22),
        torso=dict(pitch=+2, yaw=+24),
        rightArm=dict(pitch=-16, yaw=-14, roll=-8, bend=76, axis=180),
        leftArm=dict(pitch=-22, yaw=+24, roll=-10, bend=98, axis=180),
        rightLeg=dict(pitch=+8, yaw=+6, bend=14, z=+0.04),
        leftLeg=dict(pitch=-12, yaw=+4, bend=20, z=-0.05),
    ),
    2: dict(  # 腿先动 —— 后腿蹬地，链条从下往上启动
        easing="OUTQUAD",
        body=dict(x=+0.04, y=+0.01, z=-0.02, yaw=-34),
        head=dict(pitch=-4, yaw=+20),
        torso=dict(pitch=+3, yaw=+32),
        rightArm=dict(pitch=-13, yaw=-20, roll=-11, bend=84, axis=180),
        leftArm=dict(pitch=-19, yaw=+27, roll=-9, bend=88, axis=180),  # load 微展
        rightLeg=dict(pitch=+16, yaw=+6, bend=34, z=+0.06),
        leftLeg=dict(pitch=-10, yaw=+4, bend=18, z=-0.05),
    ),
    3: dict(  # LOAD —— 腰到极限，刀收内侧（仍在 guard 范畴，不越到反侧）
        easing="INCUBIC",
        body=dict(x=+0.05, y=+0.02, z=-0.05, yaw=-34),
        head=dict(pitch=-5, yaw=+18),
        torso=dict(pitch=+4, yaw=+42),
        rightArm=dict(pitch=-10, yaw=-26, roll=-14, bend=88, axis=180),
        leftArm=dict(pitch=-16, yaw=+30, roll=-8, bend=84, axis=180),
        rightLeg=dict(pitch=+18, yaw=+6, bend=40, z=+0.06),
        leftLeg=dict(pitch=-8, yaw=+4, bend=16, z=-0.04),
    ),
    5: dict(  # IMPACT —— 腰猛转正，刀横扫过身前；肘仍留 58°
        easing="OUTQUAD",
        body=dict(x=-0.03, y=-0.01, z=+0.12, yaw=-34),
        head=dict(pitch=+2, yaw=+40),
        torso=dict(pitch=+5, yaw=-20),
        rightArm=dict(pitch=-30, yaw=+30, roll=+14, bend=58, axis=180),
        leftArm=dict(pitch=-28, yaw=+10, roll=-24, bend=116, axis=180),  # counter-pull
        rightLeg=dict(pitch=+4, yaw=+10, bend=12, z=+0.02),
        leftLeg=dict(pitch=-24, yaw=+2, bend=38, z=-0.08),
    ),
    6: dict(  # overshoot —— 末端关节滞后 1 tick：腕再翻、肘再收
        easing="INOUTSINE",
        body=dict(x=-0.02, y=-0.01, z=+0.13, yaw=-34),
        head=dict(pitch=+3, yaw=+42),
        torso=dict(pitch=+5, yaw=-26),
        rightArm=dict(pitch=-32, yaw=+38, roll=+24, bend=52, axis=180),
        leftArm=dict(pitch=-26, yaw=+8, roll=-26, bend=112, axis=180),
        rightLeg=dict(pitch=+3, yaw=+10, bend=11, z=+0.02),
        leftLeg=dict(pitch=-26, yaw=+2, bend=40, z=-0.09),
    ),
    8: dict(  # 回 guard（与 tick 0 完全一致，连击友好）
        easing="INOUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0, yaw=-34),
        head=dict(pitch=-3, yaw=+22),
        torso=dict(pitch=+2, yaw=+24),
        rightArm=dict(pitch=-16, yaw=-14, roll=-8, bend=76, axis=180),
        leftArm=dict(pitch=-22, yaw=+24, roll=-10, bend=98, axis=180),
        rightLeg=dict(pitch=+8, yaw=+6, bend=14, z=+0.04),
        leftLeg=dict(pitch=-12, yaw=+4, bend=20, z=-0.05),
    ),
}

DESCRIPTION = (
    "v1 匕首横划: 肘全程不伸直（impact 仍 bend=58，对比剑刺 bend=3），"
    "腰扭 62° 只给读感（torso 带不动刀，刀弧全由右臂自身给），"
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
