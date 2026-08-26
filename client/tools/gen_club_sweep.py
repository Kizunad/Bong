#!/usr/bin/env python3
"""club_sweep — 木棍**双手**横抡扫击（钝器的快那一招，与 `club_smash` 成对）。

## 为什么一件钝器要两条动画

`club_smash` 是重的那一招：12 tick、棍头竖直落差 24px、收势占 5 tick。全靠它一条的话，
玩家每次普攻都看到同一记过顶抡砸，读感很快就磨平了。横抡补的正是另一档：8 tick、贴着
胸腹高扫过身前，快、平。

两条的差别做成了**可量的**，不是"数值微调一下"。全程棍头包围盒实测见 §量测，一条纵、
一条横，正交。玩家在远处只看剪影也能分出对面在用哪一招——这是招式差异化的底线要求
（CLAUDE.md「招式/技能 A/V 差异化」）。`ClubAnimationTest` 把这两个比值钉死。

## v2：改成双手握，方向反过来（由用户手摆的 tick 0 定调）

v1 是单手右→左横抡。v2 的 tick 0 **是用户在 Blockbench 里亲手摆的**：棍横在身体**左**
侧、双手都握在棍身上、右肩前倾左侧后缩。这一帧定死了两件事：

1. **抡击方向反过来**：棍从左侧起手 → 扫向右侧。躯干转矩随之整条反号
   （v1 是 `+22 → +40 → −30`，v2 是 `−22 → −40 → +30`），站架 `body.yaw` 与腿的
   蹬/承分工同步镜像。
2. **双手握是硬约束，不是装饰**。手持物挂在右手上，左手是自由的——想让它"也握着棍"，
   就得让左腕真的落在棍身线段上。这件事**眼睛摆不准**：左腕差 3px 就是"手悬在棍旁边"，
   而 3px 在正视图里只有一个多像素，截图上看不出来，进游戏一转视角就露馅。v1 的副手
   全程离棍身 **16px**（`held_item_pose.py track --grip` 实测），根本没在握。

副手姿态因此是**解**出来的，不是摆的：`held_item_pose.py chain` 按帧推进，每帧在"贴住
棍身"和"跟上一帧连贯"之间加权取舍。全程（1/4 tick 取样）左腕离轴 ≤ 1.38px。

## 双手握把这一招的行程**压小了**，这是物理，不是没调好

v1 单手棍头横向行程 35.5px。双手握之后只剩 ~20px——因为两只手同时挂在同一根棍上，
右臂能伸到的地方被左臂的关节极限反过来卡住了。`solve --two-hand` 实测：撞击帧棍头最远
只能到肩右 ~7px，再远副手就搭不上（最好也差 8.4px，而一个拳头才 4px 宽）。

**这个代价是对的**：用户要的就是双手。把行程硬撑回 35px 的唯一办法是让左手脱开，那就
不是双手招式了。横/竖比仍然把它和抡砸分得干干净净（见 §量测）。

## 中段那一帧（tick 4）是为副手加的，不是为姿态

只卡 t3 / t5 时两条手臂在关节空间各走各的路：棍由右臂带着，左臂走另一条弧，关键帧上
严丝合缝，**中间照样甩脱**——实测 t4.5 副手离棍身 4.31px。整 tick 取样只报 2.05px，
根本看不见，所以 `track --grip` 和测试都改成 1/4 tick 取样。钉住中段后全程 ≤1.38px。

顺带把抡击的转角摊平了：`chain` 逐帧最大转角从 24/18/**96**/6/42/36 变成
24/18/30/42/6/12/36，没有任何一帧要副手甩过 50°。

## 分段（docs/player-animation-conventions.md §1）

POSE 表按**设计 tick**（8 tick 骨架）写，出料前整体拉长到 10 tick（见下节）：

    设计  出料
      0 →  0   guard    棍横在身体左侧、双手握持；右肩前倾（用户手摆的那一帧）
      2 →  3   腿先动    后腿蹬地 bend 15→40（kinetic chain 起点）
      3 →  4   LOAD     腰扭到 −40°，棍沉进抡击平面（棍头肩左 17.8px、肩下 2.4px、放平）
      4 →  5   中段      棍过身体中线（棍头肩左 9.6px）—— 见上节，这一帧是为副手加的
      5 →  6   IMPACT   腰猛转到 +30°（总转矩 70°），棍扫到肩右 6.4px、身前 11.8px
      6 →  7   overshoot 棍再扫右 + 腕再拧（末端关节滞后 1 tick）
      7 →  9   收势      棍**贴着低位**抽回左侧，过了头的轮廓才抬起来
      8 → 10   == tick 0

峰值错开：腿 → 腰 → 肩 → 肘/腕。impact 落在 6/10 = 60%（标配 60%）。

## 拉长 1.2×：整数网格上只能给到 1.25×

tick 是**整数**——PlayerAnimator 读 JSON 时 `getAsInt()`（AnimationJson.java:123），存储层
也是 `findAtTick(int)`。8 × 1.2 = 9.6 落不到网格上，写小数会被截断、和相邻帧撞成一帧，
静默丢关键帧。最近的整数是 10，也就是 **1.25×**（比要的多 4%，20ms，肉眼无从分辨）。

拉长走 `anim_common.retime`——**搬帧，不重采样**。姿态一个数都不改，所以贴棍距离、
挡不挡脸、棍头包围盒这些几何判据逐字成立，变的只有速度。重采样（在新网格上按原曲线
取值）会把 LOAD / IMPACT 削掉：1.25 倍下设计的极值帧落在两个新整数 tick 之间，插值
过去峰值就没了，而峰值恰恰是这条动画最要紧的东西。

多出来的 2 tick 落在哪儿是**解出来的**，不是挑的：`integer_retime` 对累计位置取整，
保证任何一帧的时间误差 ≤ 0.5 tick。唯一的人工约束是 `keep_gap={6}`——overshoot 必须
贴着 impact 后一 tick（conventions §2.6），被拉成 2 tick 就不再是弹性过冲，而是"到位
之后又慢慢挪了一下"。解出来是 guard→腿 3 tick（原 2）、overshoot→收势 2 tick（原 1）。

**抡击段本身没变速**（LOAD→中段→IMPACT 仍是 1+1 tick）。这不是偷懒：1 tick 的段在整数
网格上乘 1.2 只能落回 1 或 2，也就是 ×1.0 或 ×2.0——把它拉成 2 tick 是把这一下**减半速**，
远超要求的 1.2×，而且会抹掉「smash 是重的、sweep 是快的」这条差异化的立身之本。多出来
的时间因此摊在蓄势和收势上，这也正是累计取整自己挑出来的位置。

## 转矩全靠 `torso.yaw`，站架靠 `body.yaw`

横抡是**胯带肩、肩带手**的动作，躯干转矩比抡砸更重要：`torso.yaw` 走 −22 → −40 → +30，
总转矩 70°（`club_smash` 只有 54°）。但要清楚 `torso.*` **带不动棍**——它只作用于躯干
ModelPart，头/臂/腿各自独立（conventions §L243）。棍的位移全部来自两条手臂自身的
pitch/yaw/roll/bend，torso 给的是"这一下是从腰上抡出来的"这个读感。

`body.yaw = +24` 恒定（比 `club_smash` 的 −16 更侧身，且**反向**：v2 从左侧起手）。
头反向补 −26 保持世界朝向。**恒定**是硬要求——站架跟着逐帧转的话脚会在地上打滑。

## easing：IN 接 IN 会在接缝上撞出一个静止点

每帧的 easing 管「本帧 → 下一帧」。t0/t2 蓄势用 OUT 族，**t3 发力用 IN 族**（从静止
加速），t5 余势 OUTQUAD 卸力。把 OUTQUAD 写在撞击帧 t5 上是最容易犯的错——那管的是
撞击**之后**。

加了 tick 4 之后多出一条：**IN 族结束时是快的，开始时是慢的**，所以 t3 和 t4 都用 IN
族会在 t4 这个接缝上把速度掐回近乎 0（实测 t4.06 只剩 0.5px/tick，棍在抡击中途卡住
一下）。t4 因此用 LINEAR 接住 t3 末端的速度。

收势两帧（t6/t7）也都用 LINEAR，理由不同：收势要走的 3D 距离和抡击本来就接近（回程
21px / 抡击 24px，都是 2 tick），任何带峰的 easing 都会让**回程峰速反超打击峰速**，
读成"抽回来比打出去还快"。实测 INOUTSINE 回程峰 24.4 vs 打击峰 17.2，LINEAR 之后是
回程 16.2 / 打击 17.2。

## 量测（`held_item_pose.py track --item wooden_club --anim club_sweep --grip --dump`）

    club_smash   横向 12.1  竖直 33.8  前后 13.5   —— 竖直是横向的 2.79 倍
    club_sweep   横向 25.4  竖直 10.9  前后  8.6   —— 横向是竖直的 2.33 倍
    副手离棍身轴线 ≤ 1.38px（1/4 tick 取样；v1 是 16px）
    峰速 17.2px/tick @ t5.50（落在抡击段内），回程峰 16.4 —— 低于打击峰
    开场「什么都没发生」的窗口 0.44 → 1.03 tick（拉长的代价，全部落在蓄势前）
    全程无「棍从脸正前方横过」
"""

from anim_common import emit_json, integer_retime, retime

# **键是设计 tick（8 tick 骨架），不是出料 tick**——出料前整体拉长到 10 tick，见 §拉长。
# 下面注释里提到的 tick 一律指设计 tick，换算查 §分段 那张两列表。
#
# tick 0 的 rightArm 是**用户在 Blockbench 里亲手摆的**，小数一位不许凑整——凑了就是
# 偷偷改他的姿态。其余各帧的右臂由 `held_item_pose.py solve --two-hand` 解出，副手由
# `chain` 按帧链式解出。
POSE = {
    0: dict(  # guard —— 棍横在身体左侧、双手握（棍头肩左 13.4px、身前 11.6px、仰角 +29°）
        easing="OUTSINE",
        body=dict(x=-0.03, y=0.0, z=0.0, yaw=+24),
        head=dict(pitch=+1, yaw=-26),
        torso=dict(pitch=+3, yaw=-22),          # 负 = 右肩前倾（用户指定）
        rightArm=dict(pitch=-39.9, yaw=-53.37, roll=-37.83, bend=40, axis=180),
        leftArm=dict(pitch=-32, yaw=+30, roll=-6, bend=86, axis=180),
        rightLeg=dict(pitch=-12, yaw=-4, bend=20, z=-0.05),
        leftLeg=dict(pitch=+8, yaw=-6, bend=15, z=+0.04),
    ),
    2: dict(  # 腿先动 —— 后腿蹬地，链条从下往上启动；棍同时**沉进抡击平面**
        easing="OUTQUAD",
        body=dict(x=-0.05, y=+0.01, z=-0.02, yaw=+24),
        head=dict(pitch=0, yaw=-24),
        torso=dict(pitch=+4, yaw=-32),
        rightArm=dict(pitch=-34, yaw=-58, roll=-39, bend=26, axis=180),
        leftArm=dict(pitch=-8, yaw=+6, roll=+18, bend=98, axis=180),
        rightLeg=dict(pitch=-10, yaw=-4, bend=17, z=-0.05),
        leftLeg=dict(pitch=+18, yaw=-6, bend=40, z=+0.06),
    ),
    3: dict(  # LOAD —— 腰到极限，棍横在身体左侧、**齐胸放平**
        # （棍头肩左 17.8px、肩下 2.4px、身前 7.2px、仰角 +0.7°）
        easing="INSINE",           # 缓加速起步；猛加速留给 t4→撞击那一 tick
        body=dict(x=-0.06, y=+0.02, z=-0.05, yaw=+24),
        head=dict(pitch=-1, yaw=-22),
        torso=dict(pitch=+5, yaw=-40),
        rightArm=dict(pitch=-28, yaw=-63, roll=-41, bend=15, axis=180),
        leftArm=dict(pitch=+10, yaw=-12, roll=+6, bend=98, axis=180),
        rightLeg=dict(pitch=-8, yaw=-4, bend=15, z=-0.04),
        leftLeg=dict(pitch=+20, yaw=-6, bend=44, z=+0.06),
    ),
    4: dict(  # 抡击中段 —— 棍正过身体中线，**这一帧是为副手加的**
        # 两条手臂各自在关节空间插值：棍由右臂带着走，左臂走的是另一条路。t3/t5 上都
        # 严丝合缝，中间照样甩脱——实测 t4.5 副手离棍身 4.31px（整 tick 取样只报 2.05px，
        # 完全看不见）。钉住中段之后全程 ≤0.6px。
        easing="LINEAR",           # 见 §easing：IN 接 IN 会在 t4 上撞出一个静止点
        body=dict(x=0.0, y=0.0, z=+0.04, yaw=+24),
        head=dict(pitch=+1, yaw=-34),
        torso=dict(pitch=+6, yaw=-8),
        rightArm=dict(pitch=+30, yaw=-14, roll=-56, bend=87, axis=180),
        leftArm=dict(pitch=+10, yaw=+18, roll=+36, bend=68, axis=180),
        rightLeg=dict(pitch=-17, yaw=-3, bend=27, z=-0.06),
        leftLeg=dict(pitch=+12, yaw=-8, bend=28, z=+0.04),
    ),
    5: dict(  # IMPACT —— 腰猛转 70°，棍横扫过身前（棍头肩右 6.4px、身前 11.8px）
        easing="OUTQUAD",
        body=dict(x=+0.05, y=-0.01, z=+0.11, yaw=+24),
        head=dict(pitch=+3, yaw=-44),
        torso=dict(pitch=+6, yaw=+30),
        rightArm=dict(pitch=+5, yaw=+22, roll=-56, bend=118, axis=180),
        leftArm=dict(pitch=+16, yaw=+18, roll=+72, bend=26, axis=180),
        rightLeg=dict(pitch=-24, yaw=-2, bend=38, z=-0.08),
        leftLeg=dict(pitch=+4, yaw=-10, bend=12, z=+0.02),
    ),
    6: dict(  # overshoot —— 末端关节滞后 1 tick：棍再扫右、腕再拧
        easing="LINEAR",
        body=dict(x=+0.04, y=-0.01, z=+0.12, yaw=+24),
        head=dict(pitch=+4, yaw=-46),
        torso=dict(pitch=+6, yaw=+36),
        rightArm=dict(pitch=+16, yaw=+36, roll=-60, bend=130, axis=180),
        leftArm=dict(pitch=+16, yaw=+24, roll=+72, bend=20, axis=180),
        rightLeg=dict(pitch=-26, yaw=-2, bend=40, z=-0.09),
        leftLeg=dict(pitch=+3, yaw=-10, bend=11, z=+0.02),
    ),
    7: dict(  # 收势 —— 棍**贴着低位**抽回左侧，过了头的轮廓才抬起来
        # 没有这一帧，t6 → t8 的直线插值会让棍在 t7.25 从脸正前方 15px 处横过去
        # （`track --dump` 报 -1.4px「挡脸」）。棍高在 t8 才回到 guard 的肩上高度。
        easing="LINEAR",
        body=dict(x=0.0, y=0.0, z=+0.05, yaw=+24),
        head=dict(pitch=+2, yaw=-34),
        torso=dict(pitch=+4, yaw=+10),
        rightArm=dict(pitch=-5, yaw=-8, roll=-26, bend=68, axis=180),
        leftArm=dict(pitch=+4, yaw=+12, roll=+60, bend=26, axis=180),
        rightLeg=dict(pitch=-19, yaw=-3, bend=30, z=-0.07),
        leftLeg=dict(pitch=+6, yaw=-8, bend=13, z=+0.03),
    ),
    8: dict(  # 回 guard（与 tick 0 完全一致，连击友好）
        easing="INOUTSINE",
        body=dict(x=-0.03, y=0.0, z=0.0, yaw=+24),
        head=dict(pitch=+1, yaw=-26),
        torso=dict(pitch=+3, yaw=-22),
        rightArm=dict(pitch=-39.9, yaw=-53.37, roll=-37.83, bend=40, axis=180),
        leftArm=dict(pitch=-32, yaw=+30, roll=-6, bend=86, axis=180),
        rightLeg=dict(pitch=-12, yaw=-4, bend=20, z=-0.05),
        leftLeg=dict(pitch=+8, yaw=-6, bend=15, z=+0.04),
    ),
}

DESCRIPTION = (
    "v2 木棍双手横抡扫击: 棍从身体左侧扫向右侧（tick 0 由用户手摆），双手全程握在棍身上"
    "（左腕离轴 ≤1.4px，v1 是 16px）；与 club_smash 正交——横向行程 25.4px、竖直 10.9px"
    "（抡砸恰好反过来：竖 33.8 / 横 12.1），10 tick 快、平、贴胸腹高扫过身前；"
    "腰转矩 70°（−22 → −40 → +30，抡砸 54°），腿 t3 → 腰 t4 → 肩 t6 → 腕 t7 错峰。"
)

# 整体拉长。见 §拉长——8 tick 的设计骨架搬到 10 tick 网格上（1.25×，整数网格上离
# 1.2× 最近的一档）。`keep_gap={6}` 把 overshoot 钉在 impact 后一 tick。
TIME_SCALE = 1.25
TIMING = integer_retime(POSE, TIME_SCALE, keep_gap={6})
END_TICK = TIMING[max(POSE)]

if __name__ == "__main__":
    emit_json(
        retime(POSE, TIMING),
        name="club_sweep",
        description=DESCRIPTION,
        end_tick=END_TICK,
        stop_tick=END_TICK + 2,
        is_loop=False,
    )
