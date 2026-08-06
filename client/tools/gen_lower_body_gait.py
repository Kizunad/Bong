#!/usr/bin/env python3
"""下半身步态动画四件套：lower_walk / lower_jog / lower_sprint / lower_dash。

上下半身分离的下半层。**只写 leftLeg / rightLeg / body**，绝不碰 arm/torso/head——
PlayerAnimator 的透传粒度是 axis 级（KeyframeAnimationPlayer.Axis.getValueAtCurrentTick:
该 axis 无关键帧就 `return currentValue`），所以本层不写的部位原样交给上层的招式动画
或 vanilla。反过来上半身动画也不许写 leg，否则会把步态踩掉（现网 141 个动画全是
七部位全写的全身动画，直接复用会互相打架）。

为什么 body 归下半层：body.* 走 MatrixStack、影响整个玩家（含头发/盔甲/手持物），
跑动的重心起伏与前倾本来就该带上半身。上层只写 torso 不写 body，两者叠加不冲突。

符号（MC 模型空间 y 向下、+Z 向后，实测确认）：
    leg.pitch  > 0 → 脚向后        （所以"腿在前"写负值）
    body.pitch > 0 → 整体前倾
    body.y     > 0 → 整体下沉
库坑：循环动画每个用到的 axis 必须在 tick 0 与 endTick 同值收口，否则 findAfter 会
fabricate 一个 (endTick+1, defaultValue) 虚拟帧、整条循环被拖回 0——anim_common
的 _check_loop_closure 会挡住。另外每个 axis 都要在 tick 0 有帧：首帧晚于 0 时
findBefore 的 pos==-1 分支返回 defaultValue，会把下层值踩成 0。

腿部幅度纪律：leg.pitch ≤ 40°（60° 腿根脱胯，见 render_bend_matrix.png），跑动的
视觉强度靠 bend（膝）堆，不靠加大 pitch。

用法:
    python3 client/tools/gen_lower_body_gait.py
    python3 scripts/models/render_player_pose.py --anim <json>   # 三视图预览
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import anim_common as AC  # noqa: E402

# 每档：周期 tick、腿摆幅、膝弯（支撑/过渡/摆动峰值）、重心起伏、前倾
GAITS = {
    "lower_walk": dict(
        period=20, swing=22.0, knee=(6.0, 12.0, 34.0), bob=0.035, lean=0.0,
        desc="下半身·行走循环（普通移动）。只写双腿与 body，上半身交给上层动画/vanilla。",
    ),
    "lower_jog": dict(
        period=16, swing=32.0, knee=(9.0, 20.0, 52.0), bob=0.055, lean=7.0,
        desc="下半身·慢跑循环（vanilla sprint 档）。步频提高、膝弯加深、body 微前倾。",
    ),
    "lower_sprint": dict(
        period=12, swing=40.0, knee=(12.0, 30.0, 72.0), bob=0.075, lean=13.0,
        desc="下半身·冲刺循环（速度倍率超阈值）。pitch 顶到 40° 约定上限，强度靠膝 bend 堆。",
    ),
}

DASH_DESC = "下半身·瞬步（MovementAction.DASHING）。蹬地→滞空收腿→落地缓冲，末帧归零交还下层。"


def gait_pose(period: int, swing: float, knee, bob: float, lean: float):
    """四相位循环：右腿前 → 交错 → 右腿后 → 交错 → 回位。

    knee = (stance, cross, swing_peak)：支撑腿、交错腿、摆动腿峰值的膝弯。
    """
    k_stance, k_cross, k_swing = knee
    # 四相位必须等距，否则半个周期快、半个慢——period 不被 4 整除时整数 tick 会挤歪
    if period % 4 != 0:
        raise ValueError(f"步态周期必须被 4 整除（四相位等距），得到 {period}")
    q = period // 4

    def frame(r_pitch, l_pitch, r_bend, l_bend, sink, easing="INOUTSINE"):
        pose = {
            "easing": easing,
            "rightLeg": dict(pitch=r_pitch, bend=r_bend),
            "leftLeg": dict(pitch=l_pitch, bend=l_bend),
            "body": dict(y=sink, pitch=lean),
        }
        return pose

    return {
        # 右腿在前触地、左腿在后蹬离
        0: frame(-swing, +swing, k_stance, k_cross, +bob),
        # 交错：左腿摆动经过身体下方（膝弯峰值），重心抬起
        q: frame(-swing * 0.1, -swing * 0.1, k_cross, k_swing, -bob),
        # 左腿在前触地、右腿在后
        2 * q: frame(+swing, -swing, k_cross, k_stance, +bob),
        # 交错：右腿摆动
        3 * q: frame(+swing * 0.1, +swing * 0.1, k_swing, k_cross, -bob),
        # 收口（必须逐轴等于 tick 0）
        period: frame(-swing, +swing, k_stance, k_cross, +bob),
    }


def dash_pose():
    """4 tick 一次性：0 起势 → 1 蹬地爆发 → 2 滞空收腿 → 3 落地缓冲 → 4 归零。"""
    return {
        0: {
            "easing": "INOUTSINE",
            "rightLeg": dict(pitch=+18.0, bend=22.0),
            "leftLeg": dict(pitch=-12.0, bend=30.0),
            "body": dict(y=+0.04, z=0.0, pitch=16.0),
        },
        1: {
            "easing": "OUTQUAD",
            "rightLeg": dict(pitch=+38.0, bend=8.0),    # 后腿蹬直
            "leftLeg": dict(pitch=-30.0, bend=58.0),    # 前腿高收
            "body": dict(y=+0.01, z=+0.16, pitch=23.0),
        },
        2: {
            "easing": "OUTQUAD",
            "rightLeg": dict(pitch=-10.0, bend=48.0),   # 滞空双腿收拢
            "leftLeg": dict(pitch=+14.0, bend=34.0),
            "body": dict(y=-0.05, z=+0.30, pitch=19.0),
        },
        3: {
            "easing": "INOUTSINE",
            "rightLeg": dict(pitch=+10.0, bend=32.0),   # 落地缓冲，膝吃掉冲量
            "leftLeg": dict(pitch=-8.0, bend=26.0),
            "body": dict(y=+0.06, z=+0.12, pitch=9.0),
        },
        4: {
            "easing": "INOUTSINE",
            "rightLeg": dict(pitch=0.0, bend=0.0),
            "leftLeg": dict(pitch=0.0, bend=0.0),
            "body": dict(y=0.0, z=0.0, pitch=0.0),
        },
    }


DASH_DURATION_TICKS = 4


LOWER_PARTS = {"leftLeg", "rightLeg", "body"}


def assert_lower_only(pose_table, name: str) -> None:
    """分身契约：下半层写到 arm/torso/head 就会踩掉上层招式动画。"""
    bad = set()
    for pose in pose_table.values():
        bad |= {k for k in pose if k not in AC.RESERVED_KEYS and k not in LOWER_PARTS}
    if bad:
        raise AssertionError(f"{name}: 下半身动画不得写 {sorted(bad)}（只允许 {sorted(LOWER_PARTS)}）")


def main():
    for name, spec in GAITS.items():
        pose = gait_pose(spec["period"], spec["swing"], spec["knee"], spec["bob"], spec["lean"])
        assert_lower_only(pose, name)
        AC.emit_json(
            pose,
            name=name,
            description=spec["desc"],
            end_tick=spec["period"],
            stop_tick=spec["period"] + 3,
            is_loop=True,
            return_tick=0,
        )
    pose = dash_pose()
    assert_lower_only(pose, "lower_dash")
    AC.emit_json(
        pose,
        name="lower_dash",
        description=DASH_DESC,
        end_tick=DASH_DURATION_TICKS,
        stop_tick=DASH_DURATION_TICKS,
        is_loop=False,
    )


if __name__ == "__main__":
    main()
