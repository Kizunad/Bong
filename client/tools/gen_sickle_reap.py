#!/usr/bin/env python3
"""sickle_reap — 采药刀「割根一刀」：一次性的收获定音，不是循环作业。

## 和 harvest_crouch 的分工

`harvest_crouch` 是**在割**（循环，20 tick，慢，刃尖行程 ≈6px，割不完就一直割）；
本条是**割断了**（一次性，10 tick，快，刃尖横过身体中线走 16px），用在采集完成的
那一下。两条共用同一套站位与握法，差别全在**幅度和速度**——这是刻意的：同一个人
同一把刀，收势那一刀比作业中的每一刀都更狠。

## 骨架硬几何（三条，详细推导见 `gen_harvest_crouch.py` 的同名小节）

1. 手最低离地 10px（肩枢轴 y=2 + 臂长 12），够到地的是**刀**不是手。
2. 髋不下沉，`leg.bend` 一大脚就漂离地面 ⇒ 腿只做「错步站稳」，判据是脚不许离地。
3. `torso` 枢轴在**颈**，`torso.pitch` 会撕开腰缝（`gap ≈ 12·sin(pitch)`，躯干才厚
   4px）⇒ pitch 压在 10° 内；**`torso.yaw` 免费**（胯点在旋转轴上），身体参与度全走 yaw。

同理，`body.*` 一个都不写：它的单位（格 vs 像素）与 +Y 方向在预览和运行时是相反的，
未经真机定案（见 `gen_harvest_crouch.py`）。部件的 `y` 也不写（绝对枢轴 vs 增量，
手臂差 2px、腿差 12px）；`leg.z` 安全，照 vanilla 蹲伏的用法给到几 px。

## 8 tick 分段的 10 tick 版（conventions §1）

    t0  guard      刃尖探在药茎根部（离地 7.5px），与 harvest_crouch 的 REACH 同姿
    t2  引         刃往右外侧带一点，腰 yaw 装载到 +26°（腿先动的位置）
    t3  LOAD       腰到极限 +34°，刃收到最外侧；发力段从这里起加速（INCUBIC）
    t6  IMPACT     刃横过身体中线拉切到底（刃尖仍压在离地 10.8px 的**低位**——
                   这是它和 sickle_defend 的可辨识差异，那条 impact 在 13.2px），
                   腰猛转到 -20°（54° 转矩）
    t7  overshoot  末端滞后 1 tick：腕再翻、肘再收
    t10 == t0      收回起手，可以接下一株

拉长到 10 tick（而不是照抄匕首的 8）是因为这是**工具的活不是搏杀**：收势要够长才
读得出"把割下来的药材拎起来"，急停会读成挥空。

## easing 的管辖方向（§15.1：`isEasingBefore` 默认 false，写在本帧管的是本帧→下一帧）

发力段 `t3→t6` 的加速写在 **t3**（INCUBIC）。写到顶点 t6 上就会跑去管收势段——
`anqi_single_snipe` 正是栽在这个 off-by-one 上（docstring 写着 easeIn，实测出手即泄力）。

用法:
    python3 client/tools/gen_sickle_reap.py
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

# 站位：与 harvest_crouch 同一套（右脚身前 2.6px / 左脚身后 4.4px，双脚落地）
# 角度用度数；腿 z 的单位是 ModelPart 枢轴 px
POSE = {
    # ═══ t0 GUARD —— 刃尖已经探在药茎根部（不是 vanilla 垂手，§2.1）═══
    0: dict(
        easing="OUTSINE",
        head=dict(pitch=+22, yaw=+8, roll=0),
        torso=dict(pitch=+10, yaw=+16, roll=0),
        rightArm=dict(pitch=-8, yaw=+10, roll=-8, bend=30, axis=180),
        leftArm=dict(pitch=-30, yaw=+28, roll=0, bend=60, axis=180),
        rightLeg=dict(pitch=-14, yaw=+4, bend=22, z=-2.0),
        leftLeg=dict(pitch=+6, yaw=+4, bend=16, z=+1.5),
    ),

    # ═══ t2 引 —— 腿先动（kinetic chain 起点），刃往右外侧带 ═══
    2: dict(
        easing="OUTQUAD",
        head=dict(pitch=+23, yaw=+10, roll=0),
        torso=dict(pitch=+10, yaw=+26, roll=0),
        rightArm=dict(pitch=-6, yaw=+24, roll=-4, bend=34, axis=180),
        # 左手微展（辅助肢 load 相，反相位）
        leftArm=dict(pitch=-27, yaw=+30, roll=0, bend=54, axis=180),
        rightLeg=dict(pitch=-16, yaw=+4, bend=24, z=-2.0),
        leftLeg=dict(pitch=+8, yaw=+4, bend=18, z=+1.5),
    ),

    # ═══ t3 LOAD —— 腰到极限 +34°，刃在最外侧；发力段从这里起加速（§15.2）═══
    3: dict(
        easing="INCUBIC",
        head=dict(pitch=+24, yaw=+12, roll=0),
        torso=dict(pitch=+10, yaw=+34, roll=0),
        rightArm=dict(pitch=-4, yaw=+34, roll=0, bend=36, axis=180),
        leftArm=dict(pitch=-25, yaw=+32, roll=0, bend=50, axis=180),
        rightLeg=dict(pitch=-17, yaw=+4, bend=25, z=-2.0),
        leftLeg=dict(pitch=+9, yaw=+4, bend=19, z=+1.5),
    ),

    # ═══ t6 IMPACT —— 刃横过身体中线拉切到底；腰 +34 → -20 共 54° 转矩 ═══
    # 肘仍留 45°：采药刀是工具，打直没有意义（够不到也没必要）
    6: dict(
        easing="OUTQUAD",
        head=dict(pitch=+25, yaw=-14, roll=0),
        torso=dict(pitch=+10, yaw=-20, roll=0),
        rightArm=dict(pitch=+14, yaw=-55, roll=-25, bend=45, axis=180),
        # 左手 counter-pull：茎断的一瞬把整株药材拽住（snap 相）
        leftArm=dict(pitch=-24, yaw=+18, roll=0, bend=80, axis=180),
        rightLeg=dict(pitch=-12, yaw=+2, bend=20, z=-2.0),
        leftLeg=dict(pitch=+4, yaw=+2, bend=14, z=+1.5),
    ),

    # ═══ t7 OVERSHOOT —— 末端关节滞后 1 tick：腕再翻 8°、肘再收 4°（§2.6）═══
    7: dict(
        easing="INOUTSINE",
        head=dict(pitch=+25, yaw=-16, roll=0),
        torso=dict(pitch=+10, yaw=-24, roll=0),
        rightArm=dict(pitch=+17, yaw=-62, roll=-33, bend=41, axis=180),
        leftArm=dict(pitch=-22, yaw=+16, roll=0, bend=76, axis=180),
        rightLeg=dict(pitch=-11, yaw=+2, bend=19, z=-2.0),
        leftLeg=dict(pitch=+3, yaw=+2, bend=13, z=+1.5),
    ),

    # ═══ t10 —— 收回起手（== t0，可以直接接下一株）═══
    10: dict(
        easing="INOUTSINE",
        head=dict(pitch=+22, yaw=+8, roll=0),
        torso=dict(pitch=+10, yaw=+16, roll=0),
        rightArm=dict(pitch=-8, yaw=+10, roll=-8, bend=30, axis=180),
        leftArm=dict(pitch=-30, yaw=+28, roll=0, bend=60, axis=180),
        rightLeg=dict(pitch=-14, yaw=+4, bend=22, z=-2.0),
        leftLeg=dict(pitch=+6, yaw=+4, bend=16, z=+1.5),
    ),
}

DESCRIPTION = (
    "采药刀割根一刀（一次性收获定音）：与 harvest_crouch 同站位同握法，"
    "但幅度和速度都拉开——刃尖横过身体中线走约 16px（作业循环只走 6px），"
    "腰 yaw +34° → -20° 共 54° 转矩（torso.pitch 全程 10°，枢轴在颈会撕腰缝）；"
    "肘 impact 仍留 45° 不打直（工具不是兵器）；刃全程压在低位（impact 刃尖离地 10.8px，割的是地上的药株）；"
    "左手 counter-pull 在茎断瞬间把药材拽住（bend 60 → 50 微展 → 80 猛收，反相）；"
    "t7 overshoot 腕再翻 8°、肘再收 4°；"
    "10 tick 而非 8——收势要够长才读得出把药材拎起来；"
    "发力加速写在 t3（§15.2）；tick 0 == tick 10 可连采。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sickle_reap",
        description=DESCRIPTION,
        end_tick=10,
        stop_tick=12,
        is_loop=False,
    )
