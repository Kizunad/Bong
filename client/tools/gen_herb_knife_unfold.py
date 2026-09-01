#!/usr/bin/env python3
"""herb_knife_unfold —— 凡铁折叠采药刀甩腕开刃。

## 动作

手探到右胯外侧摸刀（腕内扣、刀贴着腿）→ 腕再往下沉一格蓄住 → 前臂沿弧线甩上来、
腕猛地外翻，靠惯性把刃甩开锁定 → 腕过冲一格 → 落进持刀架势。

10 tick。这一条**首帧不等于末帧**，而且是故意的：它是"从没拿刀到拿着刀"的过渡，
起手在胯边、收势在架势上。末帧逐轴等于 `herb_knife_stance.GUARD`，所以
`unfold → harvest` / `unfold → slash` 接得上，中间不会跳一格。

## 分段（easing 写在段首帧）

| tick | 段 | 干什么 |
|------|----|--------|
| 0  | 摸刀   | 手在右胯外侧、腕内扣（roll -16），刀几乎贴着大腿外面 |
| 2  | 沉腕   | 手外展到胯外侧、肘折深到 68° —— 这是甩之前的"压弹簧" |
| 5  | 抡臂   | 前臂沿弧线抡上来，腕还没翻（roll 只走到 +2） |
| 6  | 翻腕   | 腕在**一个 tick 内**从 +2 翻到 +44 —— "开刃"就是这一下，也是链条最后一环 |
| 10 | 架势   | 落进 GUARD |

## 已知局限（写在这里免得下一轮又去调姿态）

刀的 3D 模型是**半开**状态建的（照概念图那张摆拍：刃与柄约 40°），而且是一整块、
没有可转的刃骨。所以"刃弹开"这一下在**几何上不存在**，只能靠腕的翻转和刃在空中划过
的弧线来暗示。真要让刃自己转开，得给 `gen_herb_knife_iron.py` 的 `blade_*` 组把枢轴
从方块中心挪到转轴 `(8, 12, 8)`，再在 PlayerAnim 工程里给它一条 `blade_deploy` 轨道
——那是模型层的改动，不在这条动画的范围里。
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402
from herb_knife_stance import guard_pose, stance  # noqa: E402

POSE = {
    # ── 摸刀：手探到右胯外侧，腕内扣 ──────────────────────────────────
    0: dict(
        easing="INOUTSINE",
        **stance(
            6.0, 18.0,
            head=dict(pitch=16.0, yaw=-18.0),      # 低头看自己的胯，找刀
            right_arm=dict(pitch=6.0, yaw=28.0, roll=-16.0, bend=30.0),
            left_arm=dict(pitch=10.0, yaw=12.0, roll=-8.0, bend=18.0),
            right_leg=dict(pitch=-4.0, yaw=10.0, bend=6.0),
            left_leg=dict(pitch=3.0, yaw=8.0, bend=4.0),
        ),
    ),

    # ── 沉腕：压弹簧。肘折到 68°，手外展到胯外侧 ──────────────────────
    # `yaw` 必须把手带出体外：第一版写 yaw=20 / roll=-34，刀扎进躯干 1.32px（自穿门
    # 报的），因为腕扣得太死、刀身贴着小腹转了进去。
    # §2.3 的反向蓄势只给躯干/头，但这里"沉"的是**手腕自己**——甩腕这个动作的发力
    # 方向就是"先沉后甩"，沉的那一下和甩在同一条轨道上，不是绕远路的 V 形。
    2: dict(
        easing="INQUAD",
        **stance(
            9.0, 22.0,
            head=dict(pitch=20.0, yaw=-20.0),
            right_arm=dict(pitch=12.0, yaw=30.0, roll=-20.0, bend=68.0),
            left_arm=dict(pitch=14.0, yaw=16.0, roll=-10.0, bend=24.0),
            right_leg=dict(pitch=-8.0, yaw=12.0, bend=12.0),
            left_leg=dict(pitch=6.0, yaw=10.0, bend=8.0),
        ),
    ),

    # ── 抡臂：前臂沿弧线上来，腕**先不翻** ────────────────────────────
    # 腕留在最后一格才翻，是为了让链条的末端单独占一个峰（腿 t1 → 腰/肩/肘 t3~5 →
    # 腕 t5）。腕跟着肘一起翻的版本，五条链路的峰全压在 t4.8，`stagger` 门直接报废。
    5: dict(
        easing="OUTQUAD",
        **stance(
            12.0, 8.0,
            head=dict(pitch=6.0, yaw=-6.0),        # 头抬起来看刃
            right_arm=dict(pitch=-30.0, yaw=8.0, roll=2.0, bend=26.0),
            left_arm=dict(pitch=2.0, yaw=8.0, roll=-12.0, bend=30.0),
            right_leg=dict(pitch=-6.0, yaw=10.0, bend=10.0),
            left_leg=dict(pitch=5.0, yaw=8.0, bend=6.0),
        ),
    ),

    # ── 翻腕开刃（SNAP）：一格之内翻 42°，肘同时回弹 ──────────────────
    6: dict(
        easing="INOUTSINE",
        **stance(
            10.0, 11.0,
            head=dict(pitch=8.0, yaw=-8.0),
            right_arm=dict(pitch=-22.0, yaw=14.0, roll=44.0, bend=38.0),
            left_arm=dict(pitch=6.0, yaw=10.0, roll=-10.0, bend=26.0),
            right_leg=dict(pitch=-5.0, yaw=10.0, bend=9.0),
            left_leg=dict(pitch=4.0, yaw=8.0, bend=5.0),
        ),
    ),

    # ── 落进架势 ──────────────────────────────────────────────────────
    10: guard_pose("INOUTSINE"),
}

if __name__ == "__main__":
    emit_json(
        POSE,
        name="herb_knife_unfold",
        description="凡铁折叠采药刀甩腕开刃：胯边摸刀 → 沉腕蓄势 → 抡臂翻腕弹开刃 → 落进持刀架势",
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
