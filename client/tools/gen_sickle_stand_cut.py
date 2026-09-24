#!/usr/bin/env python3
"""sickle_stand_cut — 采药刀站立割茎：胸腹高的药株，不用弯下去。

## 为什么要单独一条

`harvest_crouch` 割的是**地面药株**（刃尖压到离地 7.5px），而末法残土的药材不全长在
地上——藤蔓、树挂、齐胸的灌木都得站着割。用同一条低位动画去割齐胸的东西，读起来是
"这人在对着空气锄地"。

两条的可辨识差异是**刃的高度和姿态**，实测：

    harvest_crouch   刃尖离地  7.5px（膝下）  刃仰 -45°（斜插向下）
    sickle_stand_cut 刃尖离地 15.9px（腰胸）  刃仰 -10°（近水平）

也就是说刃尖差了整整半个身高、刃的姿态差 35°——远处一眼能分出在割地上的还是齐胸的。

## 骨架硬几何（推导见 `gen_harvest_crouch.py` 的同名小节，这里只列结论）

- `torso.pitch` 撕腰缝（枢轴在颈，`gap ≈ 12·sin(pitch)`，躯干厚 4px）⇒ 压在 10° 内；
  **`torso.yaw` 免费**，身体参与度全走 yaw。
- `head.pitch` 顶到 25°（全仓 138 条的 98% 分位）。站立割茎本来也不用低头，这里只
  给 10~14°。
- 腿只做「错步站稳」，判据是**脚不许离地**（`leg.bend` 一大脚就漂）。
- `body.*` 与部件 `y` 一律不写（单位/符号在预览与运行时不一致，未经真机定案）。

## 循环编排（24 tick，比蹲身那条慢一拍）

站着割省力，节奏比蹲着慢：伸→勾→拉→松，一个来回 24 tick。刃在腰胸高度横向走，
主要行程来自 `rightArm.yaw` 与 `torso.yaw` 同向叠加。

    t0  REACH   刃平伸到药株前（刃尖离地 15.9 / 前 16.5，近水平）
    t7  HOOK    勾住茎，肘开始折；拉切段从这里起加速（INQUAD，§15.2）
    t14 DRAW    横过身前拉切到底，腰同向转
    t19 RELEASE 松刃、手回收
    t24 == t0

用法:
    python3 client/tools/gen_sickle_stand_cut.py
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

# 角度用度数；腿 z 的单位是 ModelPart 枢轴 px
POSE = {
    # ═══ t0 REACH —— 刃平伸到齐胸的药株前 ═══
    # 实测：刃尖离地 15.9px / 身前 16.5px，刃仰 -10°（近水平），前向分量 +0.97
    0: dict(
        easing="INOUTSINE",
        head=dict(pitch=+12, yaw=+6, roll=0),
        torso=dict(pitch=+6, yaw=+12, roll=0),
        # 右臂：平举前伸，肘留 25°
        rightArm=dict(pitch=-45, yaw=-10, roll=0, bend=25, axis=180),
        # 左臂：抬起来把枝条拉过来固定
        leftArm=dict(pitch=-52, yaw=+20, roll=0, bend=72, axis=180),
        rightLeg=dict(pitch=-12, yaw=+4, bend=20, z=-2.0),
        leftLeg=dict(pitch=+5, yaw=+4, bend=14, z=+1.5),
    ),

    # ═══ t7 HOOK —— 刃勾住茎；拉切段从这里起加速（§15.2）═══
    7: dict(
        easing="INQUAD",
        head=dict(pitch=+14, yaw=+3, roll=0),
        torso=dict(pitch=+7, yaw=+18, roll=0),
        rightArm=dict(pitch=-42, yaw=-2, roll=-6, bend=32, axis=180),
        # 左手把枝条攥紧（load 相，反相于主动肢）
        leftArm=dict(pitch=-54, yaw=+18, roll=0, bend=66, axis=180),
        rightLeg=dict(pitch=-13, yaw=+4, bend=21, z=-2.0),
        leftLeg=dict(pitch=+6, yaw=+4, bend=15, z=+1.5),
    ),

    # ═══ t14 DRAW —— 横过身前拉切到底，腰同向转（+18 → -12 共 30°）═══
    14: dict(
        easing="OUTQUAD",
        head=dict(pitch=+13, yaw=-10, roll=0),
        torso=dict(pitch=+7, yaw=-12, roll=0),
        rightArm=dict(pitch=-36, yaw=-36, roll=-14, bend=48, axis=180),
        # 左手 counter-pull：枝条断的一瞬拽住
        leftArm=dict(pitch=-58, yaw=+12, roll=0, bend=84, axis=180),
        rightLeg=dict(pitch=-14, yaw=+2, bend=22, z=-2.0),
        leftLeg=dict(pitch=+7, yaw=+2, bend=16, z=+1.5),
    ),

    # ═══ t19 RELEASE —— 松刃，手回收准备下一枝 ═══
    19: dict(
        easing="INOUTSINE",
        head=dict(pitch=+12, yaw=-3, roll=0),
        torso=dict(pitch=+6, yaw=+2, roll=0),
        rightArm=dict(pitch=-42, yaw=-22, roll=-7, bend=38, axis=180),
        leftArm=dict(pitch=-55, yaw=+16, roll=0, bend=76, axis=180),
        rightLeg=dict(pitch=-13, yaw=+3, bend=21, z=-2.0),
        leftLeg=dict(pitch=+6, yaw=+3, bend=15, z=+1.5),
    ),

    # ═══ t24 —— 回到 t0（循环点，逐轴必须完全相等，§7.1）═══
    24: dict(
        easing="INOUTSINE",
        head=dict(pitch=+12, yaw=+6, roll=0),
        torso=dict(pitch=+6, yaw=+12, roll=0),
        rightArm=dict(pitch=-45, yaw=-10, roll=0, bend=25, axis=180),
        leftArm=dict(pitch=-52, yaw=+20, roll=0, bend=72, axis=180),
        rightLeg=dict(pitch=-12, yaw=+4, bend=20, z=-2.0),
        leftLeg=dict(pitch=+5, yaw=+4, bend=14, z=+1.5),
    ),
}

DESCRIPTION = (
    "采药刀站立割茎（循环）：割齐胸的藤蔓/灌木，不用弯下去；"
    "与 harvest_crouch 的可辨识差异是刃的高度与姿态——"
    "刃尖离地 15.9px（对 7.5px）、刃仰 -10° 近水平（对 -45° 斜插向下），差半个身高、35°；"
    "节奏比蹲身那条慢（24 tick 对 20），伸→勾→拉→松一个来回；"
    "左手抬起固定枝条并在断的一瞬 counter-pull（bend 72 → 66 微展 → 84 猛收，反相）；"
    "torso.pitch 压在 7° 内、身体参与度全走免费的 torso.yaw（+18° → -12°）；"
    "不用 body.*；拉切加速写在 t7（§15.2）；tick 0 == tick 24（§7.1）。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sickle_stand_cut",
        description=DESCRIPTION,
        end_tick=24,
        stop_tick=26,
        is_loop=True,
    )
