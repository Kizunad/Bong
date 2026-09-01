#!/usr/bin/env python3
"""herb_knife_slash —— 凡铁采药刀反手横掠割。

## 动作

持刀架势 → 躯干右拧、刀带到右腰外侧蓄势 → 反手自右向左横掠过身前，刃口朝内**拉着割**
→ 腕再翻一格过冲 → 收回架势。

10 tick。首帧 = 末帧 = `herb_knife_stance.GUARD`，连着挥第二刀不必经过立正。

## 为什么是"掠割"不是"劈砍"

刀身连柄一共 13px，刃是鹰嘴内弧——这种刀吃的是**拉**不是**砸**。所以撞击帧要的不是
把手臂抡圆，而是把刃送出身前、再横着拖过去：门禁 `reach` 卡的就是"刃有没有真的离开
身体"（上一版刃尖最远只到 z=-2.3，躯干前脸就在 -2，等于在自己肚子上蹭）。

肘全程留 ≥20°（`DaggerAnimationTest.test_elbow_never_straightens` 的同一条理由）：
短刃打直手臂够不到更远，只会把手腕送到对方跟前。

## 分段（easing 写在段首帧）

| tick | 段 | 干什么 |
|------|----|--------|
| 0  | guard    | 持刀架势 |
| 2  | 引刀     | 躯干右拧到 30°、刀带到右腰外侧；手臂自己只回拉 8°（§2.3：反向蓄势归躯干） |
| 5  | 掠割     | 躯干反拧到 -12°，刃扫过身前 z≈-10、掠到身体左侧 |
| 6  | 过冲     | 腕再外翻 12°、刃继续往左走一格 —— 打碎"到位即冻结" |
| 10 | guard    | 逐轴等于 tick 0 |
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

    # ── 引刀：躯干右拧蓄势，手臂只小幅回拉 ────────────────────────────
    # §2.3：发力肢不做反向 anticipation，反向那一下交给躯干和头。手臂 yaw 只从
    # +24 回到 +32（8°，仍在架势范畴），躯干却从 +14 拧到 +30 —— 视觉上的"拉满"
    # 是躯干给的，手臂全程朝撞击方向单调走。
    2: dict(
        easing="INQUAD",
        **stance(
            10.0, 30.0,
            head=dict(pitch=4.0, yaw=-22.0),   # 头反向补偿，脸仍朝着目标
            right_arm=dict(pitch=2.0, yaw=32.0, roll=8.0, bend=62.0),
            left_arm=dict(pitch=14.0, yaw=20.0, roll=-14.0, bend=30.0),
            right_leg=dict(pitch=-14.0, yaw=16.0, bend=20.0),
            left_leg=dict(pitch=10.0, yaw=14.0, bend=14.0),
        ),
    ),

    # ── 掠割（IMPACT）：刃送出身前、横掠到左 ──────────────────────────
    # 这组是扫格子解出来的：刃最前伸到肩前 10.3px、最低 y 17.5、扫到身体左侧 4.5px，
    # 刃仰角 -3°（水平），读作"横着拉过去"。
    5: dict(
        easing="OUTQUAD",
        **stance(
            14.0, -12.0,
            head=dict(pitch=8.0, yaw=10.0),
            right_arm=dict(pitch=-60.0, yaw=-35.0, roll=20.0, bend=25.0),
            left_arm=dict(pitch=-6.0, yaw=-18.0, roll=6.0, bend=38.0),
            right_leg=dict(pitch=-10.0, yaw=4.0, bend=18.0),
            left_leg=dict(pitch=12.0, yaw=2.0, bend=10.0),
        ),
    ),

    # ── 过冲：腕再翻一格，刃继续往左走 ────────────────────────────────
    6: dict(
        easing="INOUTSINE",
        **stance(
            16.0, -18.0,
            head=dict(pitch=10.0, yaw=14.0),
            right_arm=dict(pitch=-52.0, yaw=-46.0, roll=32.0, bend=34.0),
            left_arm=dict(pitch=-2.0, yaw=-22.0, roll=8.0, bend=42.0),
            right_leg=dict(pitch=-9.0, yaw=2.0, bend=16.0),
            left_leg=dict(pitch=12.0, yaw=0.0, bend=9.0),
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
