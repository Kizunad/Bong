#!/usr/bin/env python3
"""herb_harvest —— 凡铁采药刀俯身勾割灵草。

## 动作

持刀架势 → 屈膝沉下去、左手拨开草叶按住茎 → 刃贴着茎根切入 → 顺着鹰嘴的内弧**往回
勾带**（这才是这把刀真正的用法：钩住茎往身前拉着割，不是劈）→ 起身，左手托着割下来
的草。14 tick，首帧 = 末帧 = 持刀架势（conventions §2.1）。

## 俯身怎么做的（上一版就死在这里）

`depth` 一个旋钮同时驱动 **躯干前倾 + 双膝屈 + 胯后推 + 整体下沉前移**，四件事永远
同步。数值照 `harvest_crouch`（仓库里已有的采集姿态：torso 26° / 腿 bend 48° /
body.y 0.3）抄。上一版把这四件事拆成四处手写、还禁用了 `body.*` 改成自创的"上半身
整体平移"，结果上半身在 z 上滑而两条腿站在原地——人看到的就是上下身各干各的。
详见 `herb_knife_stance` 模块文档。

## 分段（easing 写在**段首帧**上——它管的是这一帧到下一帧那一段，§15.1）

| tick | 段 | 干什么 |
|------|----|--------|
| 0  | guard   | 持刀架势，静态一眼能认出"手里有把干活的小刀" |
| 3  | 入位     | 膝先屈到六成、左手前探拨叶——kinetic chain 第一环是腿 |
| 6  | 割入     | 蹲到底，刃探进草区贴着茎切 |
| 8  | 勾带     | 腕外翻、肘回折，刃沿鹰嘴内弧往身前上方拉（末端过冲在这里） |
| 11 | 起身     | 起到三成，左手托草，刀提回腰前 |
| 14 | guard   | 逐轴等于 tick 0 |
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402
from herb_knife_stance import guard_pose, stance  # noqa: E402

POSE = {
    # ── guard：持刀架势 ────────────────────────────────────────────────
    0: guard_pose("INOUTSINE"),

    # ── 入位：腿先动（膝屈到六成），躯干跟着折下去，左手前探拨叶 ──────
    # 左臂 pitch 取**负**才是往身前伸——上一版写成 +48，那只手实际甩到了身后。
    3: dict(
        easing="INQUAD",                       # 段首 IN 族 = 从静止加速冲向割入帧
        **stance(
            0.55, 8.0, leg_depth=0.92,             # 腿领先躯干：链条第一环
            head=dict(pitch=22.0, yaw=-10.0),
            right_arm=dict(pitch=-24.0, yaw=-6.0, roll=4.0, bend=52.0),
            left_arm=dict(pitch=-30.0, yaw=-14.0, roll=-4.0, bend=34.0),
            leg_split=5.0,
        ),
    ),

    # ── 割入（IMPACT）：俯到底，刃探进草区贴着茎切 ────────────────────
    # 这组是扫格子解出来的：刃最低落到世界 y 12.2、最前伸到身前 8.3px、刃顶不过
    # 18.9（肩在 22，不构成"举火把"）。够到的是一格高灵草的**茎中段**——不是地面：
    # 这套骨架里屈膝**不会让上半身下来**（各部件是兄弟不是链，torso 枢轴恒在 y=24），
    # 真正的下蹲只有 `body.y` 做得到，而那条通道的符号未定，见模块文档。
    6: dict(
        easing="OUTQUAD",                      # 割入后急刹，接勾带
        **stance(
            1.0, 0.0, leg_depth=1.0,
            head=dict(pitch=30.0, yaw=-4.0),
            right_arm=dict(pitch=-11.0, yaw=-14.0, roll=8.0, bend=30.0),
            left_arm=dict(pitch=-34.0, yaw=-18.0, roll=-2.0, bend=42.0),
            leg_split=6.0,
        ),
    ),

    # ── 勾带：腕外翻、肘回折，刃沿鹰嘴内弧往身前上方拉 ────────────────
    # 这把刀的招牌动作——鹰嘴是**钩住往回拉着割**的，不是劈的。腕是 kinetic chain
    # 的最后一环，峰值落在这一段，也充当末端过冲。roll 封顶 20°：roll 转的是肘的
    # 折弯平面，上一版 58° 把前臂掀到了侧面，读作"肘往外翻"。
    8: dict(
        easing="INOUTSINE",
        **stance(
            0.88, 6.0, leg_depth=0.80,
            head=dict(pitch=26.0, yaw=-6.0),
            right_arm=dict(pitch=-30.0, yaw=6.0, roll=20.0, bend=68.0),
            left_arm=dict(pitch=-30.0, yaw=-12.0, roll=-4.0, bend=46.0),
            leg_split=5.0,
        ),
    ),

    # ── 起身：左手托着割下来的草，刀提回腰前 ──────────────────────────
    11: dict(
        easing="INOUTSINE",
        **stance(
            0.28, 12.0, leg_depth=0.18,
            head=dict(pitch=14.0, yaw=-8.0),
            right_arm=dict(pitch=-18.0, yaw=-4.0, roll=10.0, bend=50.0),
            left_arm=dict(pitch=-22.0, yaw=0.0, roll=-6.0, bend=44.0),
            leg_split=4.0,
        ),
    ),

    # ── 回 guard ──────────────────────────────────────────────────────
    14: guard_pose("INOUTSINE"),
}

if __name__ == "__main__":
    emit_json(
        POSE,
        name="herb_harvest",
        description="凡铁采药刀俯身勾割灵草：屈膝探刃 → 贴茎切入 → 沿鹰嘴内弧回勾带起",
        end_tick=14,
        stop_tick=16,
        is_loop=False,
    )
