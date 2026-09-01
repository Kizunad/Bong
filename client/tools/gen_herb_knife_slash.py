#!/usr/bin/env python3
"""herb_knife_slash —— 凡铁采药刀反手横掠割。

## 动作

持刀架势 → 躯干右拧、刀带到右腰外侧蓄势 → 反手自右向左横掠过身前，刃口朝内**拉着割**
→ 腕再翻一格过冲 → 收回架势。10 tick，首帧 = 末帧 = 持刀架势。

## 为什么是"掠割"不是"劈砍"

刀身连柄一共 13px，刃是鹰嘴内弧——这种刀吃的是**拉**不是**砸**。撞击帧要的不是把
手臂抡圆，而是把刃送出身前、再横着拖过去。

## 转体的分工（§2.5 + DaggerStanceTest）

- `body.yaw` = 站架，**恒定**，整个人（含胯/腿）侧着；
- `torso.yaw` = 这一挥的转体，从 +24° 拧到 −18°，共 42° 的躯干扭矩；
- `body.z` = 前冲（撞击帧 +0.20 格），身体承担视觉位移，不指望手臂自己走出幅度。

§2.3：反向蓄势**只给躯干和头**，手臂从起手到撞击单调朝目标走——手臂先反向抬一下
再挥，观众看到的就是"画三角形"。

## 分段（easing 写在段首帧）

| tick | 段 | 干什么 |
|------|----|--------|
| 0  | guard  | 持刀架势 |
| 2  | 引刀   | 躯干右拧到 +24°、微沉重心；手臂只小幅回拉，仍在架势范畴 |
| 5  | 掠割   | 躯干反拧到 −18°、前冲，刃扫过身前掠到身体左侧 |
| 6  | 过冲   | 腕再外翻、刃继续往左走一格 —— 打碎"到位即冻结" |
| 10 | guard  | 逐轴等于 tick 0 |
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402
from herb_knife_stance import guard_pose, stance  # noqa: E402

POSE = {
    # ── guard ─────────────────────────────────────────────────────────
    0: guard_pose("INOUTSINE"),

    # ── 引刀：躯干右拧蓄势（胯跟着转 55%），手臂只小幅回拉 ────────────
    2: dict(
        easing="INQUAD",
        **stance(
            0.14, 46.0, leg_depth=0.44,        # 腿先蹬地：链条第一环
            head=dict(pitch=8.0, yaw=-24.0),   # 头反向补偿，脸仍朝着目标
            right_arm=dict(pitch=-6.0, yaw=16.0, roll=0.0, bend=74.0),
            left_arm=dict(pitch=-14.0, yaw=12.0, roll=-8.0, bend=30.0),
            leg_split=6.0,
        ),
    ),

    # ── 掠割（IMPACT）：刃送出身前、横掠到左 ──────────────────────────
    5: dict(
        easing="OUTQUAD",
        **stance(
            0.28, -32.0, leg_depth=0.10,
            head=dict(pitch=6.0, yaw=8.0),
            right_arm=dict(pitch=-44.0, yaw=-36.0, roll=10.0, bend=18.0),
            left_arm=dict(pitch=-20.0, yaw=-12.0, roll=2.0, bend=40.0),
            leg_split=2.0,
        ),
    ),

    # ── 过冲：腕再翻一格，刃继续往左走 ────────────────────────────────
    6: dict(
        easing="INOUTSINE",
        **stance(
            0.32, -42.0, leg_depth=0.16,
            head=dict(pitch=8.0, yaw=12.0),
            right_arm=dict(pitch=-34.0, yaw=-52.0, roll=20.0, bend=40.0),
            left_arm=dict(pitch=-16.0, yaw=-16.0, roll=4.0, bend=44.0),
            leg_split=1.0,
        ),
    ),

    # ── 回 guard ──────────────────────────────────────────────────────
    10: guard_pose("INOUTSINE"),
}

if __name__ == "__main__":
    emit_json(
        POSE,
        name="herb_knife_slash",
        description="凡铁采药刀反手横掠割：右拧引刀 → 刃出身前自右向左拉割 → 腕翻过冲",
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
