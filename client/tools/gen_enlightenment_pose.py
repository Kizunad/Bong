#!/usr/bin/env python3
"""enlightenment_pose — 顿悟 (双手胸前合十 + 低头静立)。

身法节奏（40 tick / 2s, 非循环）：
  tick 0   guard
  tick 8   合十  双手合并胸前 (pitch=-70° bend=130° yaw 内收 ±22°)
  tick 15  低头
  tick 25  定格
  tick 32  微抬
  tick 40  回 guard

**全程 INOUTSINE 不用 OUTQUAD**: conventions §5 "安静类" 模板。

**v2 参考 KosmX/Emotecraft-emotes/hearthands.json**: hearthands 在胸前合心形手的
关键数据是 rightArm pitch=-46° yaw=-30° roll=+10°, leftArm pitch=-57° yaw=+13°。
v1 用 pitch=-75° bend=135° 把双手举到接近锁骨高度+前臂折得极深, 看起来像高举致敬
而非合十祈祷。v2 下调到 pitch=-55° bend=95° 让掌停在胸前水平, 更接近真实合掌位置。

反僵硬要点：
  - 合十 tick 8 → 定 25: 不是死定值 —— body.y -0.03 → -0.05 微沉 (气沉丹田),
    head 15° → 18° → 17° 极轻微点头 (仿真入定时的浮想)
  - 双臂 roll 内翻: right roll=-18°, left roll=+18° (掌心完全相对 贴合)
  - torso pitch -3° 挺直 + 微收下颌
  - 双腿 bend=4° 极轻屈 (承重姿, 不是笔直站)
  - axis=180° 所有 bend 朝前
  - tick 32→40 抬头比 cast_invoke 更缓 (INOUTSINE 8 tick)
"""
from anim_common import emit_json

POSE = {
    0: dict(
        easing="INOUTSINE",
        body=dict(y=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        rightLeg=dict(bend=0),
        leftLeg=dict(bend=0),
    ),
    8: dict(  # 合十 —— 双掌贴合
        easing="INOUTSINE",
        body=dict(y=-0.03),
        head=dict(pitch=+10),
        torso=dict(pitch=-2),
        rightArm=dict(pitch=-55, yaw=-22, roll=-14, bend=95, axis=180),
        leftArm=dict(pitch=-55, yaw=+22, roll=+14, bend=95, axis=180),
        rightLeg=dict(bend=4),
        leftLeg=dict(bend=4),
    ),
    15: dict(  # 低头
        easing="INOUTSINE",
        body=dict(y=-0.04),
        head=dict(pitch=+18),
        torso=dict(pitch=-3),
        rightArm=dict(pitch=-55, yaw=-22, roll=-14, bend=95, axis=180),
        leftArm=dict(pitch=-55, yaw=+22, roll=+14, bend=95, axis=180),
        rightLeg=dict(bend=4),
        leftLeg=dict(bend=4),
    ),
    25: dict(  # 定格 —— 气沉最深
        easing="INOUTSINE",
        body=dict(y=-0.05),
        head=dict(pitch=+17),
        torso=dict(pitch=-3),
        rightArm=dict(pitch=-55, yaw=-22, roll=-14, bend=95, axis=180),
        leftArm=dict(pitch=-55, yaw=+22, roll=+14, bend=95, axis=180),
        rightLeg=dict(bend=4),
        leftLeg=dict(bend=4),
    ),
    32: dict(  # 微抬头 (开悟出定前的预收)
        easing="INOUTSINE",
        body=dict(y=-0.03),
        head=dict(pitch=+10),
        torso=dict(pitch=-2),
        rightArm=dict(pitch=-50, yaw=-18, roll=-10, bend=85, axis=180),
        leftArm=dict(pitch=-50, yaw=+18, roll=+10, bend=85, axis=180),
        rightLeg=dict(bend=3),
        leftLeg=dict(bend=3),
    ),
    40: dict(  # 回 guard
        easing="INOUTSINE",
        body=dict(y=0.0),
        head=dict(pitch=0),
        torso=dict(pitch=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        rightLeg=dict(bend=0),
        leftLeg=dict(bend=0),
    ),
}

DESCRIPTION = (
    "v2 JSON 顿悟 (参考 hearthands.json): 40 tick 全 INOUTSINE, 双掌合十 (pitch-55° "
    "bend95° yaw±22° roll±14° axis=180°) 胸前水平高度, body.y -0.03→-0.05 气沉, "
    "head +10→+18° 入定微颤, torso-3° 挺直, 腿 bend4° 极轻屈。v1 pitch=-75° 过高"
    "像致敬, v2 按 hearthands 下调到真正合掌位置。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="enlightenment_pose",
        description=DESCRIPTION,
        end_tick=40,
        stop_tick=43,
        is_loop=False,
    )
