#!/usr/bin/env python3
"""sword_spine_cleave — 异兽脊骨剑 (BeastSpineSword) 专属双手竖斩。

## 为什么不改通用的 sword_cleave

`sword_cleave` 是**共享**基础剑招（`server/src/combat/sword_basics.rs` 在用，还有
`client/src/test/resources/bong/anim_spec_manifests/sword_cleave.json` 钉着 endTick），
按脊骨剑的握距重摆会连带改掉竹剑等所有剑的竖斩。本剑专属的那份另起一个名字，和
`sword_spine_slash` 成对。

## 动作

双手握柄举剑过面门 → 后仰蓄力 → 剑立正上 → 沿身体中线整条劈下 → 沉到最低 → 提回。
剑尖 x 全程 ≈ -3～0 px，是**正中竖斩**不是斜劈。

| tick | 阶段 | 剑尖（px） | 左手离柄尾 |
|------|------|-----------|-----------|
| 0  | 双手 GUARD | (-1, 46, +8)  | 2.1px |
| 4  | LOAD 后仰  | (-5, 43, +12) | 2.6px |
| 9  | 剑立正上   | (-4, 45, -8)  | 4.1px |
| 13 | IMPACT     | (-2, 7, -16)  | 15px（脱手，见下）|
| 15 | 最低点     | (-2, 3, -12)  | 16px |
| 17 | RIP PULL   | (-3, 12, -20) | 15px |
| 20 | 收回 GUARD | = tick 0      | 2.1px |

## 双手握的骨架硬限制（这条不是"摆得不够好"）

MC 玩家骨架两肩相距 10px、单臂长 8.06px，所以**两只手只能在 (0,22,0) 附近半径 ~6px
的盘里会合**——胸前中线、齐胸高。举过头顶或劈到身侧时左手在几何上就够不到剑柄。

于是这条动画的做法是：guard / 蓄力 / 收势帧把左手**解算**到剑柄柄尾（残差 2~4px，一个
手宽以内，看着就是双手握），劈砍段让左手顺势脱开、随身体甩出（手摆），收势再合回柄上。
参照组：现网通用 `sword_cleave` 的左手全程离柄 8~16px，从来没真握上过。

右臂四轴同样是按剑尖弧线整条反解的，理由见 `gen_sword_spine_slash` 的同名小节。

## 时序

endTick=20，stopTick=22，非循环。发力顶点 tick 13，与通用 `sword_cleave` 同拍。
"""

from anim_common import emit_json, inherit

GUARD = dict(  # 双手握柄举于面前，剑身朝上略后
    easing="OUTSINE",
    body=dict(x=0.0, y=0.0, z=0.0, yaw=0),
    head=dict(pitch=-1, yaw=0),
    torso=dict(pitch=+4, yaw=-6),
    rightArm=dict(pitch=-115.2, yaw=-28.5, roll=+22.6, bend=14.5, axis=180),
    leftArm=dict(pitch=-101.8, yaw=+41.1, roll=-27.3, bend=13.7, axis=180),
    rightLeg=dict(pitch=+8, bend=10, axis=0),
    leftLeg=dict(pitch=-10, bend=12, axis=0),
)

POSE = {
    0: GUARD,
    4: dict(  # LOAD：整体后仰坐低，剑往身后压
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.02, z=-0.06, yaw=-4),
        head=dict(pitch=-6, yaw=+2),
        torso=dict(pitch=+2, yaw=-10),
        rightArm=dict(pitch=-114.0, yaw=-22.3, roll=+22.1, bend=30.4, axis=180),
        leftArm=dict(pitch=-108.2, yaw=+41.6, roll=-24.0, bend=12.3, axis=180),
        rightLeg=dict(pitch=+11, bend=14, axis=0),
        leftLeg=dict(pitch=-13, bend=16, axis=0),
    ),
    9: dict(  # 剑立正上：过渡帧，重剑到最高点
        easing="INQUAD",
        body=dict(x=0.0, y=+0.03, z=-0.03, yaw=-2),
        head=dict(pitch=-11, yaw=+1),
        torso=dict(pitch=-4, yaw=-4),
        rightArm=dict(pitch=-62.8, yaw=-18.5, roll=-0.5, bend=27.0, axis=180),
        leftArm=dict(pitch=-91.2, yaw=+58.4, roll=-36.1, bend=6.0, axis=180),
        rightLeg=dict(pitch=+9, bend=12, axis=0),
        leftLeg=dict(pitch=-11, bend=14, axis=0),
    ),
    13: dict(  # IMPACT：躯干前折 20° + body.z 前送，剑沿中线劈到胸腹高度
        easing="INQUAD",
        body=dict(x=0.0, y=-0.04, z=+0.22, yaw=+2),
        head=dict(pitch=+14, yaw=-2),
        torso=dict(pitch=+20, yaw=+6),
        rightArm=dict(pitch=+26.7, yaw=-1.6, roll=-15.0, bend=6.0, axis=180),
        leftArm=dict(pitch=-18, yaw=+26, roll=-30, bend=44, axis=180),
        rightLeg=dict(pitch=-6, bend=24, axis=0),
        leftLeg=dict(pitch=+22, bend=8, axis=0),
    ),
    15: dict(  # 最低点：重剑惯性沉到膝下，弓步压到底
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.05, z=+0.24, yaw=+3),
        head=dict(pitch=+16, yaw=-2),
        torso=dict(pitch=+24, yaw=+8),
        rightArm=dict(pitch=+38.9, yaw=+0.6, roll=-15.5, bend=6.0, axis=180),
        leftArm=dict(pitch=-8, yaw=+30, roll=-34, bend=50, axis=180),
        rightLeg=dict(pitch=-4, bend=22, axis=0),
        leftLeg=dict(pitch=+24, bend=6, axis=0),
    ),
    17: dict(  # RIP PULL：倒钩挂住，往回提半格
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.02, z=+0.14, yaw=+2),
        head=dict(pitch=+9, yaw=-1),
        torso=dict(pitch=+15, yaw=+5),
        rightArm=dict(pitch=+20.8, yaw=-4.8, roll=-10.4, bend=13.4, axis=180),
        leftArm=dict(pitch=-28, yaw=+28, roll=-30, bend=46, axis=180),
        rightLeg=dict(pitch=-8, bend=18, axis=0),
        leftLeg=dict(pitch=+17, bend=9, axis=0),
    ),
    # 收势：逐轴回到 guard（左手重新合回柄上），只换缓动
    20: inherit(GUARD, easing="INOUTSINE"),
}

DESCRIPTION = (
    "异兽脊骨剑双手竖斩 (sword_spine_cleave): 20-tick 重剑正中竖劈，"
    "双手握柄举剑 -> 后仰蓄力 -> 沿中线整条劈下 -> 倒钩挂住提回 -> 合手收势。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sword_spine_cleave",
        description=DESCRIPTION,
        end_tick=20,
        stop_tick=22,
        is_loop=False,
    )
