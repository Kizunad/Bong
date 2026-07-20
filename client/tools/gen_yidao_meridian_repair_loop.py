#!/usr/bin/env python3
"""yidao_meridian_repair_loop —— 接经术蓄力段：双手持针 30 穴位序循环（P4）。

通道核验（P4 第一性原理，2026-07-19）：`resolve_yidao_skill`
（server/src/combat/yidao.rs）对 5 招统一 `insert_casting` 真实长引导窗——
meridian_repair cast_ticks_base=1200t（60s），运行时经 `yidao_cast_ticks` 按
mastery/平和色缩放（窗长可变）→ 长引导两段式：本蓄力段 isLoop 覆盖任意窗长，
release 段见 gen_yidao_meridian_repair_release.py。完成链 cast_emit.rs
`tick_casts_or_interrupt` 自然完成分支发 `YidaoCastCompleteEvent` →
`complete_yidao_casts` 有效结算接力 release；三打断分支表驱动 StopAnim
（`looping_cast_anim_id`，§13 #6 红线）。

母题（plan-yidao-v1 §5 ①「双手持针对患者经脉点（30 个穴位顺序）」）：医者俯身
于患者上方（弯腰走 bow_salute 补偿先例：torso + / legs pitch 小负 + bend /
body.z 前移），**两手各捏一针**，沿同一条经脉左右交替落针——右手落 15 针、
左手落 15 针，合计**一个循环 = 完整 30 穴位序**。落点沿经脉由内向外推进再回
程（三角 sweep），落针时该手腕下顿 + 捻针 roll，另一手同拍提针移向下一穴；
头目随当前针点游走，躯干随外探渐沉。与灸火对掌（contam_purge，双手对称同步
推送）/ CPR 按压（emergency，双手中线叠压）的区分：本段是**双手交替**的高频
精细手上工作，远距离可辨。

> review r2（2026-07-20，4 reviewer 一致 REQUEST_CHANGES）返工：初版实现为
> 「右手持针 + 左手探穴、一循环四落点」，与 plan 锁定的「双手持针 30 穴位序」
> 在**双手职责**与**穴位序规模**两项上均不等价。本版按原交付物重做，未走裁剪
> 决议路线。

循环红线（§13 #5 / 库坑 #1）：BASE 帧枚举全部轴，逐针帧 inherit(BASE) 派生，
首尾帧同为「右手落针 + sweep=0」相位，机械保证每轴 0/90 同值闭环。

时序（90t = 30 针 × 3t/针，密度 3t ≤ 4t 红线）：
  tick = i*3（i=0..30），i 偶数 = 右手落针 / 左手提针移位，i 奇数 = 左手落针。
  sweep s = 三角波（i≤15 外推、i>15 回程），落点 yaw 随 s 线性推移。
  i=30（tick 90）与 i=0 同相位（右手落针 + s=0）= BASE 本体，闭环。
endTick=90，stopTick=92，isLoop=true。
"""

from __future__ import annotations

from anim_common import emit_json, inherit

NEEDLE_POINTS = 30
"""plan-yidao-v1 §5 ① 锁定的穴位序规模——一个循环走完整 30 穴。"""

TICKS_PER_POINT = 3
"""每穴 3t：满足「主要运动轴相邻帧点 ≤4 tick」密度红线，且读作连续行针节律。"""

END_TICK = NEEDLE_POINTS * TICKS_PER_POINT  # 90

# 俯身双手持针基位（i=0 相位：右手落针、左手提针，sweep=0 内侧起针点）。
# 两臂同为「前伸 + 屈肘捏针」形态，只在 yaw 上左右分开——双手持针的姿态语言。
BASE = dict(
    easing="OUTQUAD",
    body=dict(x=0.0, y=-0.115, z=+0.075),
    head=dict(pitch=+17.5, yaw=-3),
    torso=dict(pitch=+22, yaw=-3),
    rightArm=dict(pitch=-28, yaw=-8, roll=+10, bend=40, axis=180),
    leftArm=dict(pitch=-48, yaw=+14, roll=-8, bend=54, axis=180),
    leftLeg=dict(pitch=-8, bend=16, z=-0.03),
    rightLeg=dict(pitch=-7, bend=15, z=+0.03),
)

# 落针手 / 提针手的两组腕部姿态（差值即「下顿 + 捻针」的可辨幅度）。
# 下顿/提针的幅度差按「远距离读招」要求拉开（pitch 20° / bend 14° / roll 18°）：
# 初版 13°/6° 在旁观距离上读不出交替节律（review r2 验收口径）。
DOWN_PITCH, DOWN_ROLL, DOWN_BEND = -28.0, +10.0, 40.0
UP_PITCH, UP_ROLL, UP_BEND = -48.0, -8.0, 54.0

# 落点沿经脉推移的 yaw 行程（右手基准 -8°，外推 14°；左手恒偏内 +22° 领半步）。
RIGHT_YAW_INNER = -8.0
LEFT_YAW_OFFSET = +22.0
YAW_SWEEP = -14.0


def sweep_at(index: int) -> float:
    """三角 sweep：0 → 1（第 15 针最外）→ 0（第 30 针回到起点，闭环）。"""
    half = NEEDLE_POINTS // 2
    return index / half if index <= half else (NEEDLE_POINTS - index) / half


def pose_at(index: int) -> dict:
    """第 index 针（0-based）的整帧姿态：右手偶数针落、左手奇数针落。"""
    s = sweep_at(index)
    right_down = index % 2 == 0
    yaw_shift = YAW_SWEEP * s
    right_yaw = RIGHT_YAW_INNER + yaw_shift
    left_yaw = RIGHT_YAW_INNER + LEFT_YAW_OFFSET + yaw_shift

    def arm(is_down: bool, yaw: float) -> dict:
        return dict(
            pitch=round(DOWN_PITCH if is_down else UP_PITCH, 2),
            yaw=round(yaw, 2),
            roll=DOWN_ROLL if is_down else UP_ROLL,
            bend=DOWN_BEND if is_down else UP_BEND,
            axis=180,
        )

    return inherit(
        BASE,
        # 落针帧 easeOut 收住下顿；提针过渡帧在下一针自然承接。
        easing="OUTQUAD",
        # 外探时躯干随之渐沉，回程复位（s 驱动，首尾同值）。
        body=dict(x=0.0, y=round(-0.115 - 0.012 * s, 4), z=round(0.075 + 0.006 * s, 4)),
        # 头目跟随当前落针手的针点。
        head=dict(
            pitch=+17.5,
            yaw=round((right_yaw if right_down else left_yaw) * 0.45, 2),
        ),
        torso=dict(pitch=round(22 + 2.0 * s, 2), yaw=round(-3 + yaw_shift * 0.35, 2)),
        rightArm=arm(right_down, right_yaw),
        leftArm=arm(not right_down, left_yaw),
    )


# i=30 与 i=0 同相位（右手落针 + s=0）→ 尾帧 = BASE 本体，每轴 0/90 同值闭环。
POSE = {i * TICKS_PER_POINT: pose_at(i) for i in range(NEEDLE_POINTS + 1)}


def main() -> int:
    assert POSE[0] == POSE[END_TICK], "循环缝合红线：首尾帧必须逐轴同值（库坑 #1）"
    right_taps = sum(1 for i in range(NEEDLE_POINTS) if i % 2 == 0)
    left_taps = NEEDLE_POINTS - right_taps
    assert right_taps == left_taps == 15, "双手持针：左右手落针数必须各半"
    emit_json(
        POSE,
        name="yidao_meridian_repair_loop",
        description=(
            f"P4 接经术蓄力段（isLoop {END_TICK}t）：俯身**双手持针**沿经脉交替"
            f"落针，一个循环走完整 {NEEDLE_POINTS} 穴位序（右手 {right_taps} 针 / "
            f"左手 {left_taps} 针，每 {TICKS_PER_POINT}t 一针）——落针手腕下顿 + "
            "捻针（pitch -48→-28 / roll -8→+10），同拍另一手提针移向下一穴；落点"
            "沿经脉外推再回程（yaw 行程 14°），躯干随外探渐沉，bow 补偿俯身。"
            f"全轴 0/{END_TICK} 同值闭环。release 段见 yidao_meridian_repair_release。"
        ),
        end_tick=END_TICK,
        stop_tick=END_TICK + 2,
        is_loop=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
