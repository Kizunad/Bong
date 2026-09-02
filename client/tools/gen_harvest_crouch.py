#!/usr/bin/env python3
"""harvest_crouch — 采药刀割药的循环作业动作（server `GatheringTargetKind::Herb` 的 tick 动画）。

## 这条动画做不到什么 —— 先把 MC 骨架的三条硬几何摆出来

动画 id 里带着 "crouch"（server 的 `bong:harvest_crouch` 已在用，不改），但**真蹲不
下去**。这不是姿态没调好，是骨架就这样，逐条量过：

1. **手够不到地。** 肩枢轴在 `(±5, 2, 0)`、上臂+前臂共 12px，手心最低到 y=14；地面
   在 y=24（腿枢轴 12 + 腿长 12）。手最低**离地 10px**，膝盖高度。"蹲下去用手薅草"
   在这套骨架上不成立。
   → 够到地的是**刀**：握把到刃尖 11.4px、`hand_display` 再乘 0.85，刃尖能压到离地
   4~7px。所以构图是「人躬身、手在膝高、**刀尖探到药茎根部**」——真采药人也是割茎，
   不是把手插进土里。
2. **髋不下沉。** 扫过 `leg.pitch × bend` 的整个网格：膝高恒在离地 6~6.8px，`bend`
   只是把小腿往后折、**把脚抬离地面**。bend 44 那档右脚悬空 2.5px（上一版就是这个
   数），SIDE 机位读成大跨步而不是作业站姿。
   → 所以腿只能给「错步站稳」：右腿 (pitch -14, bend 22, z -2.0) / 左腿 (pitch +6,
   bend 16, z +1.5)，实测右脚身前 2.6px 离地 0.2、左脚身后 4.4px 离地 0.5，错步 7.0px。
   判据只有一条：**脚不许离地**。
3. **`torso` 的枢轴在脖子不在腰。** torso cuboid 从 y=0（颈）往下长到 y=12（胯），
   `torso.pitch` 是绕**颈**转——肩不动、胯往后甩，越"弯腰"腰缝越大：
   `gap = 12·sin(pitch)`，pitch 12° 就 2.51px，而躯干才厚 4px。
   → 本文件把 `torso.pitch` 压在 **10°** 以内（实测腰缝 2.34px）。俯身的读感改由 `head.pitch`（头枢轴
   就在颈上，绕它转不产生缝）承担。
   → **`torso.yaw` 是免费的**：胯点 (0,12,0) 落在 yaw 的旋转轴上，缝恒为 0。所以身体
   的参与度全部走 yaw，这也是全仓的既有做法（`dagger_slash` 的腰转到 42°）。
   佐证：152 条用了 `torso.pitch` 的已上线动画，中位数 **9°**、70% ≤12°；越界的
   `loot_bend` 45° / `death_collapse` 90° 全是首批粗制资产。

## 为什么一个 `body.*` 都没写

`body.y` 是唯一能整体降低玩家的通道，但它现在**不能用**：

- 运行时它走 MatrixStack（`PlayerRendererMixin:83` 的 `translate(x, y+0.7, z)`），注入
  点在 `LivingEntityRenderer` 的 `scale(-1,-1,1)` **之前**，所以那里 **+Y 朝上、单位
  是格**；同一处的 `0.7` 也只有解释成「臀高、以格为单位」才说得通。
- 预览（`render_animation.solve_skeleton`）却把它当**像素**加进 ModelPart 空间，而
  ModelPart 是 +Y 朝下。同一个 `body.y=+0.32`：预览里往下 0.32px（等于没动），进游戏
  是**往上 0.32 格**（把人抬起来）。

上一版正是栽在这里——写着"身体压低 +0.32m 蹲伏"，预览看不出蹲，真机则是浮起来。
已上线资产对这个符号的用法本身自相矛盾（`levitate` +0.18 上浮 vs `stealth_crouch`
+0.28 下蹲），说明它从来没被定过。**在真机实证定案前，本文件只用无歧义的轴。**

同源的第二条坑：**部件的 `x/y/z` 是绝对枢轴 px，不是增量**（`KeyframeAnimationPlayer
.Axis.getValueAtCurrentTick` 只在该轴无关键帧时才回落到 vanilla 值），而预览算的是
`PIVOTS + offset`。torso/head 的 rest 枢轴是 0，两者等价；**手臂（rest y=2）差 2px、
腿（rest y=12）差 12px**（写 `leg.y` 腿会飞到头顶）。所以下面一个 `y` 都没写。
`leg.z` 反而是安全的——预览 rest 记的是 0.0、真机是 0.1，只差 0.1px，而 vanilla 自己
的蹲伏用的就是 `leg.z = 4.0`。（顺带：仓库里那些 `leg.z=±0.05` 是 0.05px 的空操作。）

## 循环编排：伸手 → 勾住 → 拉切 → 松开

镰刀割茎是**拉切**不是砍：刃勾住茎往回带，靠刃弧把茎割断。所以主运动是刀尖在身前
画一段由外往内的弧，不是上下挥。腰用 yaw 跟着转，给行程加厚度（§2.5：手臂幅度 ≠
视觉幅度）。

## 库坑对照（conventions §7 / §13 #5）

- **循环单帧衰减**：每个用到的轴都在 `endTick` 补同值帧，`_check_loop_closure` 逐轴核。
- **`leg.pitch ≤ 40°`**：这里最大 16°。
- **肘往身前折**：手臂一律 `axis=180`；膝往身后折：腿 `axis` 取默认 0。出料时
  `assert_joint_fold_is_anatomical` 逐帧拦。
- **easing 管「本帧 → 下一帧」**（§15.1，`isEasingBefore` 默认 false）：拉切段的加速
  写在段的**起始**帧 t5 上（INQUAD），写到顶点 t11 会跑去管松开段。

用法:
    python3 client/tools/gen_harvest_crouch.py
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

# 角度用度数（emit 时转弧度）；腿 z 的单位是 ModelPart 枢轴 px（不是米）
POSE = {
    # ═══ t0 REACH —— 伸出去，刃尖探到药茎根部 ═══
    0: dict(
        easing="INOUTSINE",
        # 俯身的读感给头（头枢轴就在颈上，绕它转不产生腰缝），但 pitch 顶到 25°：
        # 全仓 138 条用 head.pitch 的动画 98% ≤25°、中位 6°，再大头就从脖子上探出去
        head=dict(pitch=+24, yaw=+8, roll=0),
        torso=dict(pitch=+10, yaw=+14, roll=0),
        # 右臂：低位前伸，肘留 28°（作业姿，不打直）
        rightArm=dict(pitch=-8, yaw=+8, roll=-8, bend=28, axis=180),
        # 左臂：前伸按住药株，把茎拉紧好下刀
        leftArm=dict(pitch=-30, yaw=+28, roll=0, bend=60, axis=180),
        # 错步站稳：脚不许离地（实测右 0.2 / 左 0.5）
        rightLeg=dict(pitch=-14, yaw=+4, bend=22, z=-2.0),
        leftLeg=dict(pitch=+6, yaw=+4, bend=16, z=+1.5),
    ),

    # ═══ t5 HOOK —— 刃勾住茎、压下去咬住；拉切段从这里起加速（§15.2）═══
    5: dict(
        easing="INQUAD",
        head=dict(pitch=+25, yaw=+4, roll=0),
        torso=dict(pitch=+10, yaw=+18, roll=0),
        rightArm=dict(pitch=-2, yaw=0, roll=-10, bend=34, axis=180),
        # 左手把茎攥紧一点（辅助肢 load 相，反相于主动肢）
        leftArm=dict(pitch=-32, yaw=+26, roll=0, bend=56, axis=180),
        rightLeg=dict(pitch=-15, yaw=+4, bend=23, z=-2.0),
        leftLeg=dict(pitch=+7, yaw=+4, bend=17, z=+1.5),
    ),

    # ═══ t11 DRAW —— 拉切顶点：刃勾着茎横过身体中线带回来，腰同向转 32° ═══
    11: dict(
        easing="OUTQUAD",
        head=dict(pitch=+25, yaw=-12, roll=0),
        torso=dict(pitch=+10, yaw=-14, roll=0),
        rightArm=dict(pitch=+15, yaw=-35, roll=-10, bend=50, axis=180),
        # 左手 counter-pull：茎断的瞬间把它拽住（snap 相，收紧）
        leftArm=dict(pitch=-36, yaw=+22, roll=0, bend=68, axis=180),
        rightLeg=dict(pitch=-16, yaw=+2, bend=23, z=-2.0),
        leftLeg=dict(pitch=+8, yaw=+2, bend=17, z=+1.5),
    ),

    # ═══ t15 RELEASE —— 割断了，刃松开抬起一点，手往回收准备下一株 ═══
    15: dict(
        easing="INOUTSINE",
        head=dict(pitch=+24, yaw=-4, roll=0),
        torso=dict(pitch=+10, yaw=0, roll=0),
        rightArm=dict(pitch=+6, yaw=-16, roll=-9, bend=42, axis=180),
        leftArm=dict(pitch=-33, yaw=+25, roll=0, bend=62, axis=180),
        rightLeg=dict(pitch=-15, yaw=+3, bend=23, z=-2.0),
        leftLeg=dict(pitch=+7, yaw=+3, bend=17, z=+1.5),
    ),

    # ═══ t20 —— 回到 t0（循环点，逐轴必须完全相等，§7.1）═══
    20: dict(
        easing="INOUTSINE",
        head=dict(pitch=+24, yaw=+8, roll=0),
        torso=dict(pitch=+10, yaw=+14, roll=0),
        rightArm=dict(pitch=-8, yaw=+8, roll=-8, bend=28, axis=180),
        leftArm=dict(pitch=-30, yaw=+28, roll=0, bend=60, axis=180),
        rightLeg=dict(pitch=-14, yaw=+4, bend=22, z=-2.0),
        leftLeg=dict(pitch=+6, yaw=+4, bend=16, z=+1.5),
    ),
}

DESCRIPTION = (
    "采药刀割药作业（循环）：错步站稳（右脚身前 2.6px、左脚身后 4.4px，双脚落地），"
    "头俯 24~25° 盯住手下的活（全仓 head.pitch 的 98% 分位就在 25°），刃尖探到离地 7.5px 的药茎根部，"
    "勾住后横过身体中线拉切回来（刃尖行程 ≈6px，腰用 yaw 同向转 32° 加厚度），"
    "割断后松刃收手再伸出去；"
    "手够不到地是骨架硬几何（肩枢轴 y=2 + 臂长 12 ⇒ 手最低离地 10px），够到地的是刀；"
    "torso.pitch 压在 10° 内（枢轴在颈，gap≈12·sin(pitch)，躯干才厚 4px；yaw 实测免费）；"
    "不用 body.*（预览按 px、运行时按格且 +Y 朝上，符号未定案）；"
    "拉切加速写在 t5（§15.2 easing 管本帧→下一帧）；tick 0 == tick 20（§7.1）。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="harvest_crouch",
        description=DESCRIPTION,
        end_tick=20,
        stop_tick=22,
        is_loop=True,
    )
