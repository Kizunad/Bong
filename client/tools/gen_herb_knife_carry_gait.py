#!/usr/bin/env python3
"""herb_knife_carry_walk / _sprint —— 手持凡铁采药刀时的专属携行步态。

## 为什么另立一条，而不是往全局步态里加手臂

`lower_walk` / `lower_sprint` 是 `GaitSelector` 驱动的**全局**步态，所有武器和空手
共用。把持刀的手型写进去，拿剑拿棍空着手的人也会摆出握采药刀的姿势。所以这里另出
一份「携行」变体，只在手里确实拿着采药刀时由 `GaitVariants` 顶替。

## 脚下逐帧复用全局步态，一个数都不改

腿与 body 直接调 `gen_lower_body_gait.gait_pose(**GAITS[base])` 生成——和全局那条
**同一个函数、同一批参数**，所以步幅、膝弯、重心起伏完全一致。换武器只换手臂，
脚下不会"换了把刀就变了走法"。

## 手臂来自人工闸门（Blockbench 手搓）

Round 2 之后用户在 Blockbench 里手搓了这两条的手臂摆动，用
`bbmodel_to_pose` 按 5.0 口径读回。**这里只是把那份手稿固化成生成器**，数值没有
重新设计过；我只改了"什么时候到"（见下）。

改过的两件事（都是硬缺陷，不是审美）：

1. **同手同脚**。手稿里右腿在 t0 是 `pitch -22`（脚在身前），右臂 t0 也是 `-35`
   （手在身前）——同侧手脚同时前摆。相位对调后：右腿前 → 右臂在身后，右腿后 →
   右臂甩到身前。这个相位是从旧 `STANCE_POSES` 的静态架势继承的，不是手搓引入的。
2. **循环不闭合**。`isLoop` 要求每个用到的 axis 首末同值（否则 `findAfter` 会
   fabricate 一个 `(endTick+1, defaultValue)` 虚拟帧把整条循环拖回 0，
   conventions §7.1）。手稿四条肘轨道首末不等，最大差 37.5°；这里首末取同值。

## 只写腿 / body / 双臂

不写 `torso` 和 `head`：那两处要留给"边走边看四周"和招式的躯干拧转。手臂虽然写了，
但本层是 `LOWER_BODY`(500)，招式在 `UPPER_BODY`(1000)，施法时手臂由上层接管，
不会打架。

## 为什么只有 walk，没有 sprint

手稿的冲刺那一条**只动了肘**（bend 在 42.5↔50 / 62.5↔47.5 之间泵），前后摆幅
`pitch` 全程恒定 −30（右臂一直在身前）。这样的变体比不做还差：本层会把手臂**钉死**在
一个静止姿态上，等于用"不摆的手"替换掉 vanilla 跑动时本来就有的摆臂；而且右腿摆到
身前那半个周期就是**同手同脚**（`test_the_knife_arm_swings_opposite_its_own_leg`
实测报红）。

回落到全局 `lower_sprint` 反而是对的：手臂不写，交还上层/vanilla。所以冲刺档**不出
变体**，手稿的数值留档在 `PENDING_SPRINT_ARMS` 里，等补上前后摆幅再启用。
`lower_jog` / `lower_dash` 同理没有变体。

用法:
    python3 client/tools/gen_herb_knife_carry_gait.py
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import anim_common as AC  # noqa: E402
from gen_lower_body_gait import GAITS, gait_pose  # noqa: E402

#: 携行变体 → (全局基准步态, 手臂轨道)。手臂帧点必须落在基准步态已有的 tick 上
#: （walk 0/5/10/15/20），否则会插出一个只有手臂动的孤帧。
CARRY = {
    "herb_knife_carry_walk": dict(
        base="lower_walk",
        desc="携行·行走（持凡铁采药刀）。脚下逐帧复用 lower_walk，只叠持刀手臂摆动。",
        arms={
            # 右腿在前 → 持刀的右臂在身后（手稿的中段姿态）
            0: dict(
                rightArm=dict(pitch=2.396, yaw=-8.744, roll=8.305, bend=27.5, axis=180.0),
                leftArm=dict(pitch=10.92, yaw=13.95, roll=0.974, bend=32.5, axis=180.0),
            ),
            # 右腿到身后 → 右臂甩到身前（手稿的首帧姿态）
            10: dict(
                rightArm=dict(pitch=-35.0, yaw=-15.0, roll=20.0, bend=25.0, axis=180.0),
                leftArm=dict(pitch=15.0, yaw=15.0, roll=-10.0, bend=35.0, axis=180.0),
            ),
            20: dict(
                rightArm=dict(pitch=2.396, yaw=-8.744, roll=8.305, bend=27.5, axis=180.0),
                leftArm=dict(pitch=10.92, yaw=13.95, roll=0.974, bend=32.5, axis=180.0),
            ),
        },
    ),
}

#: 手稿里冲刺那条的手臂值，**尚未启用**（缺前后摆幅，见模块文档）。留档以免手搓的
#: 数值丢掉：补上 pitch 摆幅后把它挪回 `CARRY` 即可，基准步态是 `lower_sprint`
#: （帧点 0/3/6/9/12）。
PENDING_SPRINT_ARMS = {
    0: dict(
        rightArm=dict(pitch=-30.0, yaw=-20.0, roll=25.0, bend=42.5, axis=180.0),
        leftArm=dict(pitch=20.0, yaw=20.0, roll=-15.0, bend=62.5, axis=180.0),
    ),
    6: dict(
        rightArm=dict(pitch=-30.0, yaw=-20.0, roll=25.0, bend=50.0, axis=180.0),
        leftArm=dict(pitch=20.0, yaw=20.0, roll=-15.0, bend=47.5, axis=180.0),
    ),
    12: dict(
        rightArm=dict(pitch=-30.0, yaw=-20.0, roll=25.0, bend=42.5, axis=180.0),
        leftArm=dict(pitch=20.0, yaw=20.0, roll=-15.0, bend=62.5, axis=180.0),
    ),
}

#: 携行层允许写的部位。比全局步态多两条手臂，仍然**不含 torso / head**。
CARRY_PARTS = {"leftLeg", "rightLeg", "body", "leftArm", "rightArm"}


def assert_carry_only(pose_table, name: str) -> None:
    """写到 torso/head 就会踩掉"边走边看四周"和招式的躯干拧转。"""
    bad = set()
    for pose in pose_table.values():
        bad |= {k for k in pose if k not in AC.RESERVED_KEYS and k not in CARRY_PARTS}
    if bad:
        raise AssertionError(
            f"{name}: 携行步态不得写 {sorted(bad)}（只允许 {sorted(CARRY_PARTS)}）")


def carry_pose(base: str, arms: dict) -> dict:
    """全局步态的腿/body 表 + 手臂轨道。手臂帧点必须落在已有 tick 上。"""
    spec = GAITS[base]
    pose = gait_pose(spec["period"], spec["swing"], spec["knee"], spec["bob"], spec["lean"])
    missing = sorted(set(arms) - set(pose))
    if missing:
        raise AssertionError(
            f"{base}: 手臂帧点 {missing} 不在基准步态的 tick {sorted(pose)} 上——"
            f"会插出一个只有手臂动的孤帧")
    for tick, axes in arms.items():
        pose[tick] = dict(pose[tick], **axes)
    return pose


def main() -> None:
    for name, spec in CARRY.items():
        period = GAITS[spec["base"]]["period"]
        pose = carry_pose(spec["base"], spec["arms"])
        assert_carry_only(pose, name)
        AC.emit_json(
            pose,
            name=name,
            description=spec["desc"],
            end_tick=period,
            stop_tick=period + 3,
            is_loop=True,
            return_tick=0,
        )


if __name__ == "__main__":
    main()
