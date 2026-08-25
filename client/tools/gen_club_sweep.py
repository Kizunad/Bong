#!/usr/bin/env python3
"""club_sweep — 木棍横抡扫击（钝器的**快**那一招，与 `club_smash` 成对）。

## 为什么一件钝器要两条动画

`club_smash` 是重的那一招：12 tick、棍头竖直落差 24px、收势占 5 tick。全靠它一条的话，
玩家每次普攻都看到同一记过顶抡砸，读感很快就磨平了。横抡补的正是另一档：8 tick、贴着
胸腹高扫过身前，快、平。

两条的差别做成了**可量的**，不是"数值微调一下"。全程棍头包围盒实测：

    club_smash   横向  7.0  竖直 26.9  前后 17.1   —— 竖直是横向的 3.9 倍
    club_sweep   横向 35.5  竖直 14.6  前后 12.9   —— 横向是竖直的 2.4 倍

一条纵、一条横，正交。玩家在远处只看剪影也能分出对面在用哪一招——这是招式差异化的
底线要求（CLAUDE.md「招式/技能 A/V 差异化」）。`ClubAnimationTest` 把这两个比值钉死。

## guard 就是握棒待击，不是垂手

同 `gen_club_smash.py` 那节的理由：横抡的打击方向是**从右扫向左**，tick 0 就把棍横伸在
右前方（`pitch=+5 / yaw=+50 / roll=+56 / bend=40`，实测棍头在肩右 17.5px、身前 5.7px、
仰角 +3°）。于是棍的横向位置全程单调右→左，一格反向都没有。

棍在前后向上确实走了个 V（身前 +5.7 → 抽回 +0.9 → 扫出 +13.8）——那是**圆弧本身**：
横抡的轨迹是一个绕人转的圆，前后分量必然先退后进。§2.3 禁的是让末端轨迹变 V 形的
**抽搐**，不是禁止圆弧运动；这里横向（主轴）单调，前后是圆弧的副产品。

## 高度是解出来的，不是摆出来的：第一版从下巴前面扫过去

第一版把整条弧摆在**肩高**（撞击帧棍头高于肩 1.3px）。数字上很漂亮——竖直行程只有
0.7px，比现在还平——但渲出来棍是贴着**下巴**扫过去的，正视图里整根棍横在脸前面。
棍在人体前方、并没有真的穿模，可"挡住脸"本身就是缺陷。

这轮把整条弧压到胸腹高（棍头恒在肩下 2~5px），代价是竖直行程从 0.7 涨到 14.6px（弧线
压低之后两端会翘）。**这是对的取舍**：横/竖比 2.4 仍然把它和抡砸的 3.9 分得干干净净，
而"看得见脸"是不能拿来换的。

## 8 tick 分段（docs/player-animation-conventions.md §1）

    tick 0  guard    棍横伸在右前、胸腹高；副手收在胸前
    tick 2  腿先动    后腿蹬地 bend 15→40（kinetic chain 起点）
    tick 3  LOAD     腰扭到 +40°，棍抽回身侧（肘由 40 折到 70），副手前探（反相）
    tick 5  IMPACT   腰猛转到 −30°（总转矩 70°），棍扫到肩左 13px、身前 13.8px，
                     副手猛收（counter-pull）
    tick 6  overshoot 棍再扫左 5px + 腕再拧（末端关节滞后 1 tick）
    tick 8  == tick 0

峰值错开：腿 t2 → 腰 t3 → 肩 t5 → 肘/腕 t6。impact 落在 5/8 = 62%（标配 60%）。

## 转矩全靠 `torso.yaw`，站架靠 `body.yaw`

横抡是**胯带肩、肩带手**的动作，躯干转矩比抡砸更重要：`torso.yaw` 走 +22 → +40 → −30，
总转矩 70°（`club_smash` 只有 54°）。但要清楚 `torso.*` **带不动棍**——它只作用于躯干
ModelPart，头/臂/腿各自独立（conventions §L243）。棍的位移全部来自右臂自身的
pitch/yaw/roll/bend，torso 给的是"这一下是从腰上抡出来的"这个读感。

`body.yaw = -24` 恒定（比 `club_smash` 的 −16 更侧身：横抡本来就是侧身对敌的架势，正面
站着抡会把弧线甩到自己胸前）。头反向补 +24 保持世界朝向。**恒定**是硬要求——站架跟着
逐帧转的话脚会在地上打滑。

## easing 的管辖方向（conventions §15）

每帧的 easing 管「本帧 → 下一帧」。所以 t0/t2 蓄势用 OUT 族，**t3 发力用 INCUBIC**
（从静止单调加速，最快点落在撞击帧），t5 余势 OUTQUAD 卸力，t6 收势 INOUTSINE。
把 OUTQUAD 写在撞击帧 t5 上是最容易犯的错——那管的是撞击**之后**。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # guard —— 棍横伸在右前（棍头肩右 17.5px、身前 5.7px、仰角 +3°）
        easing="OUTSINE",
        body=dict(x=+0.03, y=0.0, z=0.0, yaw=-24),
        head=dict(pitch=+1, yaw=+26),
        torso=dict(pitch=+3, yaw=+22),
        rightArm=dict(pitch=+5, yaw=+50, roll=+56, bend=40, axis=180),
        leftArm=dict(pitch=-36, yaw=+10, roll=-28, bend=104, axis=180),
        rightLeg=dict(pitch=+8, yaw=+6, bend=15, z=+0.04),
        leftLeg=dict(pitch=-12, yaw=+4, bend=20, z=-0.05),
    ),
    2: dict(  # 腿先动 —— 后腿蹬地，链条从下往上启动
        easing="OUTQUAD",
        body=dict(x=+0.05, y=+0.01, z=-0.02, yaw=-24),
        head=dict(pitch=0, yaw=+24),
        torso=dict(pitch=+4, yaw=+32),
        rightArm=dict(pitch=+30, yaw=+50, roll=+60, bend=55, axis=180),
        leftArm=dict(pitch=-42, yaw=+14, roll=-24, bend=92, axis=180),  # 前探（反相）
        rightLeg=dict(pitch=+18, yaw=+6, bend=40, z=+0.06),
        leftLeg=dict(pitch=-10, yaw=+4, bend=17, z=-0.05),
    ),
    3: dict(  # LOAD —— 腰到极限，棍抽回身侧（棍头肩右 16.1px、身前 0.9px，仍近水平）
        easing="INCUBIC",
        body=dict(x=+0.06, y=+0.02, z=-0.05, yaw=-24),
        head=dict(pitch=-1, yaw=+22),
        torso=dict(pitch=+5, yaw=+40),
        rightArm=dict(pitch=+55, yaw=+49, roll=+63, bend=70, axis=180),
        leftArm=dict(pitch=-46, yaw=+18, roll=-20, bend=84, axis=180),
        rightLeg=dict(pitch=+20, yaw=+6, bend=44, z=+0.06),
        leftLeg=dict(pitch=-8, yaw=+4, bend=15, z=-0.04),
    ),
    5: dict(  # IMPACT —— 腰猛转 70°，棍横扫过身前（棍头肩左 13.0px、身前 13.8px）
        easing="OUTQUAD",
        body=dict(x=-0.05, y=-0.01, z=+0.11, yaw=-24),
        head=dict(pitch=+3, yaw=+44),
        torso=dict(pitch=+6, yaw=-30),
        rightArm=dict(pitch=-30, yaw=-21, roll=-56, bend=16, axis=180),
        leftArm=dict(pitch=-26, yaw=+2, roll=-44, bend=122, axis=180),  # counter-pull
        rightLeg=dict(pitch=+4, yaw=+10, bend=12, z=+0.02),
        leftLeg=dict(pitch=-24, yaw=+2, bend=38, z=-0.08),
    ),
    6: dict(  # overshoot —— 末端关节滞后 1 tick：棍再扫左、腕再拧
        easing="INOUTSINE",
        body=dict(x=-0.04, y=-0.01, z=+0.12, yaw=-24),
        head=dict(pitch=+4, yaw=+46),
        torso=dict(pitch=+6, yaw=-36),
        rightArm=dict(pitch=-10, yaw=-56, roll=-56, bend=16, axis=180),
        leftArm=dict(pitch=-24, yaw=0, roll=-46, bend=118, axis=180),
        rightLeg=dict(pitch=+3, yaw=+10, bend=11, z=+0.02),
        leftLeg=dict(pitch=-26, yaw=+2, bend=40, z=-0.09),
    ),
    8: dict(  # 回 guard（与 tick 0 完全一致，连击友好）
        easing="INOUTSINE",
        body=dict(x=+0.03, y=0.0, z=0.0, yaw=-24),
        head=dict(pitch=+1, yaw=+26),
        torso=dict(pitch=+3, yaw=+22),
        rightArm=dict(pitch=+5, yaw=+50, roll=+56, bend=40, axis=180),
        leftArm=dict(pitch=-36, yaw=+10, roll=-28, bend=104, axis=180),
        rightLeg=dict(pitch=+8, yaw=+6, bend=15, z=+0.04),
        leftLeg=dict(pitch=-12, yaw=+4, bend=20, z=-0.05),
    ),
}

DESCRIPTION = (
    "v1 木棍横抡扫击: 与 club_smash 正交的那一招——棍头横向行程 35.5px、竖直 14.6px"
    "（抡砸恰好反过来：竖 26.9 / 横 7.0），8 tick 快、平、贴胸腹高扫过身前；"
    "腰转矩 70°（抡砸 54°），副手先探后收 104 → 84 → 122，"
    "腿 t2 → 腰 t3 → 肩 t5 → 腕 t6 错峰。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="club_sweep",
        description=DESCRIPTION,
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
