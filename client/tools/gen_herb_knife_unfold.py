#!/usr/bin/env python3
"""herb_knife_unfold —— 凡铁折叠采药刀甩腕开刃。

## 动作

手探到右胯外侧摸刀（腕内扣、刀贴着腿）→ 肘再折深一格蓄住 → 前臂沿弧线甩上来、腕
猛地外翻，靠惯性把刃甩开锁定 → 落进持刀架势。10 tick。

这一条**首帧不等于末帧**，而且是故意的：它是"从没拿刀到拿着刀"的过渡，起手在胯边、
收势在架势上。末帧逐轴等于持刀架势，所以 `unfold → harvest` / `unfold → slash`
接得上，中间不会跳一格。

## 分段（easing 写在段首帧）

| tick | 段 | 干什么 |
|------|----|--------|
| 0  | 摸刀   | 手在右胯外侧、腕内扣（roll −12），刀几乎贴着大腿外面 |
| 2  | 沉腕   | 肘折深到 74°、微屈膝——甩之前的"压弹簧" |
| 5  | 抡臂   | 前臂沿弧线抡上来，腕**还没翻**（roll 只走到 0） |
| 6  | 翻腕   | 腕在**一个 tick 内**从 0 翻到 +18 —— "开刃"就是这一下，链条最后一环 |
| 10 | 架势   | 落进持刀架势 |

腕留到最后一格才翻，是为了让链条末端单独占一个峰（腿 → 腰 → 肩 → 肘 → 腕）。腕跟着
肘一起翻的版本，五条链路的峰全压在同一 tick，读作"咔一下全到位"（§2.2）。

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
    # 手臂 pitch 取小正值 = 略往身后，正是"摸胯上的刀"该在的位置。
    0: dict(
        easing="INOUTSINE",
        **stance(
            0.12, 18.0, leg_depth=0.05,
            head=dict(pitch=16.0, yaw=-14.0),      # 低头看自己的胯，找刀
            right_arm=dict(pitch=10.0, yaw=22.0, roll=-6.0, bend=20.0),
            left_arm=dict(pitch=-6.0, yaw=8.0, roll=-4.0, bend=16.0),
            leg_split=4.0,
        ),
    ),

    # ── 沉腕：压弹簧。肘折到 68° ──────────────────────────────────────
    # §2.3 的反向蓄势只给躯干/头，但这里"沉"的是**手腕自己**——甩腕这个动作的发力
    # 方向就是"先沉后甩"，沉的那一下和甩在同一条轨道上，不是绕远路的 V 形。
    2: dict(
        easing="INQUAD",
        **stance(
            0.24, 24.0, leg_depth=0.62,
            head=dict(pitch=20.0, yaw=-16.0),
            right_arm=dict(pitch=14.0, yaw=24.0, roll=-10.0, bend=68.0),
            left_arm=dict(pitch=-2.0, yaw=10.0, roll=-6.0, bend=22.0),
            leg_split=5.0,
        ),
    ),

    # ── 抡臂：前臂沿弧线上来，腕**先不翻** ────────────────────────────
    5: dict(
        easing="OUTQUAD",
        **stance(
            0.10, 4.0, leg_depth=0.20,
            head=dict(pitch=4.0, yaw=-4.0),        # 头抬起来看刃
            right_arm=dict(pitch=-44.0, yaw=-10.0, roll=0.0, bend=24.0),
            left_arm=dict(pitch=-10.0, yaw=6.0, roll=-4.0, bend=26.0),
            leg_split=3.0,
        ),
    ),

    # ── 翻腕开刃（SNAP）：一格之内翻 18°，肘同时回折 ──────────────────
    6: dict(
        easing="INOUTSINE",
        **stance(
            0.06, 8.0, leg_depth=0.10,
            head=dict(pitch=6.0, yaw=-6.0),
            right_arm=dict(pitch=-32.0, yaw=-14.0, roll=18.0, bend=46.0),
            left_arm=dict(pitch=-8.0, yaw=6.0, roll=-4.0, bend=24.0),
            leg_split=4.0,
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
