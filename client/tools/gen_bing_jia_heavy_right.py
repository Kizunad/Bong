#!/usr/bin/env python3
"""bing_jia_heavy_right —— 兵甲手套（铁甲覆手）右重拳（重拳，要「沉」）。

只落资产，不接线：产出 player_animation/bing_jia_heavy_right.json。
左式见 gen_bing_jia_heavy_left.py：由本文件的右式镜像得到，左右两式完全对称。

「沉」的设计（远处看要和空手拳完全不同）：
  - **起手先坐**：load 时躯干先向左拧（+20°）、重心后移（body.z 负，后腿屈 bend 34°），
    身体整体下沉（body.y 正）。坐住了才出拳。
  - **蓄力到顶**：coil 时躯干拧到 +38°，拳收到右耳后（pitch -118°，bend 124°），
    重心完全压在后腿（后腿 bend 40°）。
  - **腰胯带动，手臂晚一拍**：drive（tick 8）躯干先转到 -22°、身体前送、下沉；
    手臂的峰值在 extension（tick 9）才到。躯干先动、手臂后动，是反僵硬规则 §2.2 的 kinetic chain。
  - **拳到位后有余震**：extension 之后 overshoot 再过冲 ~6°（tick 10），rebound 回弹（tick 12），
    共 3 tick 消化冲击，不是到位即冻结。
  - **收势慢**：settle 与 guard 之间拉 6 tick（tick 10→16），比空手拳的收势长一倍。
  - **反相**：右拳出时左拳（守势手）同步收紧（bend 122°），与右臂反相（load-snap）。

节奏（16 tick，约 0.8 秒）：
  tick 0   guard       右拳贴下颌，左拳前伸护架
  tick 3   load        躯干左拧 +20°，身体下沉、重心后移，后腿屈 34°
  tick 6   coil        躯干左拧到顶 +38°，右拳收到右耳后，重心压后腿
  tick 8   drive       腰胯带动，躯干反拧到 -22°，身体前送下沉，手臂还在半路
  tick 9   extension   右拳到位（pitch -100°，几乎伸直），躯干 -22°
  tick 10  overshoot   手臂再过冲约 6°
  tick 12  rebound     手臂回弹约 10°，躯干回收
  tick 14  settle      慢收，手臂回到守势附近
  tick 16  guard       回守，与 tick 0 同值
"""

from anim_common import emit_json

GUARD = dict(
    easing="INOUTSINE",
    body=dict(x=-0.02, y=0.0, z=0.0),
    head=dict(pitch=-3, yaw=-4),
    torso=dict(pitch=+6, yaw=-6),
    # 右拳贴下颌，铁甲内扣
    rightArm=dict(pitch=-60, yaw=-18, roll=+26, bend=112, axis=180),
    # 左拳前伸护架
    leftArm=dict(pitch=-74, yaw=+10, roll=-24, bend=88, axis=180),
    rightLeg=dict(pitch=+10, yaw=-4, bend=14, z=+0.04),
    leftLeg=dict(pitch=-16, yaw=+5, bend=20, z=-0.10),
)

POSE = {
    0: GUARD,

    3: dict(  # load：先坐住——躯干左拧，身体下沉，重心后移，后腿屈
        easing="INOUTSINE",
        body=dict(x=-0.03, y=+0.06, z=-0.04),
        head=dict(pitch=-2, yaw=+4),
        torso=dict(pitch=+8, yaw=+20),
        rightArm=dict(pitch=-70, yaw=-20, roll=+30, bend=118, axis=180),
        leftArm=dict(pitch=-82, yaw=+12, roll=-26, bend=96, axis=180),
        rightLeg=dict(pitch=+16, yaw=-4, bend=34, z=+0.05),
        leftLeg=dict(pitch=-20, yaw=+5, bend=26, z=-0.12),
    ),

    6: dict(  # coil：躯干拧到顶，拳收到右耳后，重心压后腿
        easing="INOUTSINE",
        body=dict(x=-0.04, y=+0.08, z=-0.06),
        head=dict(pitch=-4, yaw=+8),
        torso=dict(pitch=+6, yaw=+38),
        rightArm=dict(pitch=-118, yaw=-34, roll=+34, bend=124, axis=180),
        leftArm=dict(pitch=-84, yaw=+14, roll=-28, bend=100, axis=180),
        rightLeg=dict(pitch=+18, yaw=-4, bend=40, z=+0.06),
        leftLeg=dict(pitch=-22, yaw=+4, bend=26, z=-0.14),
    ),

    8: dict(  # drive：腰胯带动，躯干先反拧，身体前送下沉；手臂还在半路
        easing="OUTQUAD",
        body=dict(x=+0.02, y=+0.12, z=+0.12),
        head=dict(pitch=+4, yaw=-4),
        torso=dict(pitch=+14, yaw=-22),
        rightArm=dict(pitch=-86, yaw=-14, roll=+14, bend=34, axis=180),
        leftArm=dict(pitch=-62, yaw=+8, roll=-24, bend=122, axis=180),
        rightLeg=dict(pitch=+12, yaw=-3, bend=24, z=+0.06),
        leftLeg=dict(pitch=-26, yaw=+4, bend=42, z=-0.14),
    ),

    9: dict(  # extension：拳到位，几乎伸直；手臂峰值比躯干晚一拍
        easing="OUTQUAD",
        body=dict(x=+0.04, y=+0.12, z=+0.16),
        head=dict(pitch=+6, yaw=-6),
        torso=dict(pitch=+16, yaw=-22),
        rightArm=dict(pitch=-100, yaw=-6, roll=+6, bend=4, axis=180),
        leftArm=dict(pitch=-62, yaw=+8, roll=-24, bend=122, axis=180),
        rightLeg=dict(pitch=+12, yaw=-3, bend=26, z=+0.06),
        leftLeg=dict(pitch=-26, yaw=+4, bend=42, z=-0.14),
    ),

    10: dict(  # overshoot：手臂再过冲约 6°
        easing="OUTQUAD",
        body=dict(x=+0.04, y=+0.12, z=+0.17),
        head=dict(pitch=+7, yaw=-7),
        torso=dict(pitch=+17, yaw=-24),
        rightArm=dict(pitch=-106, yaw=-8, roll=+10, bend=2, axis=180),
        leftArm=dict(pitch=-64, yaw=+8, roll=-24, bend=124, axis=180),
        rightLeg=dict(pitch=+13, yaw=-3, bend=28, z=+0.06),
        leftLeg=dict(pitch=-26, yaw=+4, bend=42, z=-0.14),
    ),

    12: dict(  # rebound：手臂回弹约 10°，躯干回收
        easing="INOUTSINE",
        body=dict(x=+0.03, y=+0.09, z=+0.10),
        head=dict(pitch=+4, yaw=-5),
        torso=dict(pitch=+12, yaw=-20),
        rightArm=dict(pitch=-90, yaw=-10, roll=+12, bend=26, axis=180),
        leftArm=dict(pitch=-70, yaw=+8, roll=-24, bend=112, axis=180),
        rightLeg=dict(pitch=+12, yaw=-3, bend=26, z=+0.05),
        leftLeg=dict(pitch=-22, yaw=+4, bend=34, z=-0.12),
    ),

    14: dict(  # settle：慢收，手臂回到守势附近
        easing="INOUTSINE",
        body=dict(x=0.0, y=+0.03, z=+0.03),
        head=dict(pitch=-1, yaw=-3),
        torso=dict(pitch=+8, yaw=-12),
        rightArm=dict(pitch=-72, yaw=-16, roll=+20, bend=104, axis=180),
        leftArm=dict(pitch=-78, yaw=+10, roll=-24, bend=96, axis=180),
        rightLeg=dict(pitch=+12, yaw=-4, bend=18, z=+0.04),
        leftLeg=dict(pitch=-18, yaw=+5, bend=24, z=-0.10),
    ),

    16: GUARD,
}

DESCRIPTION = (
    "兵甲手套铁甲右重拳（重拳，要「沉」）：16 tick。"
    "guard 右拳贴下颌 → load 先坐住：躯干左拧 +20°、身体下沉、重心后移、后腿屈 34° → "
    "coil 躯干拧到顶 +38°、右拳收到右耳后、重心压后腿 → "
    "drive 腰胯带动，躯干反拧到 -22°、身体前送下沉，手臂晚一拍 → "
    "extension 拳到位几乎伸直 → overshoot 过冲约 6° → rebound 回弹约 10° → "
    "settle 慢收 → 回 guard。"
    "拳到位后有 3 tick 余震，收势用 6 tick，远处看和空手拳不同。左式由本式镜像。"
)

if __name__ == "__main__":
    emit_json(POSE, name="bing_jia_heavy_right", description=DESCRIPTION,
              end_tick=16, stop_tick=18, is_loop=False)
