#!/usr/bin/env python3
"""anqi_charge_carrier_release —— 封骨充能完成收势（P2 批次二后半）。

两段式 release 段（蓄力段见 gen_anqi_charge_carrier_loop.py）。充能完成
（`finish_charge(full_charge=true)`）时由 server StopAnim(loop)+PlayAnim(本段)
接力；移动打断（full_charge=false）只 StopAnim，不播本段（打断不奖励收势）。

母题：封印落定。结印双手做最后一记合拢紧压（封口），随即向外下方分掌把
充能完毕的骨器"呈出"——胸口打开、抬首，收势归中立。整体是短促干脆的
「压—展—定」，与蓄力段的绵长呼吸循环形成节奏对比。

时序（精度标准 #1/#2/#3）：
  anticipation 0→4   封口紧压：双臂再收拢下压（pitch -60→-67 / bend→100/102）、
                     身体下沉 y -0.05→-0.068（OUTQUAD 蓄势）
  strike       4→9   分掌呈出：双臂向外下方展开（pitch→-35/-32、yaw ±30+、
                     bend→20/22）、胸口打开 torso -6、抬首 -8、身体回浮（INQUAD）
  recovery     9→14  由呈出位落回中立（INOUTSINE，t11 中段帧）
endTick=14，stopTick=16，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
torso.pitch / body.y。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 承接蓄力段结印位（略深压）。
    0: dict(
        easing="OUTQUAD",
        body=dict(y=-0.05, z=-0.025),
        head=dict(pitch=+12, yaw=0),
        torso=dict(pitch=+8, yaw=0),
        rightArm=dict(pitch=-60, yaw=-19, roll=+10, bend=94, axis=180),
        leftArm=dict(pitch=-57, yaw=+21, roll=-12, bend=98, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.03),
    ),
    # 封口紧压顶点：最后一记下压封定。
    2: dict(
        easing="OUTQUAD",
        body=dict(y=-0.068, z=-0.03),
        head=dict(pitch=+14, yaw=0),
        torso=dict(pitch=+10, yaw=0),
        rightArm=dict(pitch=-67, yaw=-21, roll=+12, bend=100, axis=180),
        leftArm=dict(pitch=-64, yaw=+23, roll=-14, bend=102, axis=180),
        leftLeg=dict(pitch=-7, bend=10, z=-0.03),
        rightLeg=dict(pitch=+6, bend=9, z=+0.03),
    ),
    # anticipation 末帧 / strike 起点：压定微滞。
    4: dict(
        easing="INQUAD",
        body=dict(y=-0.065, z=-0.028),
        head=dict(pitch=+13, yaw=0),
        torso=dict(pitch=+9, yaw=0),
        rightArm=dict(pitch=-66, yaw=-20, roll=+11, bend=99, axis=180),
        leftArm=dict(pitch=-63, yaw=+22, roll=-13, bend=101, axis=180),
        leftLeg=dict(pitch=-7, bend=10, z=-0.03),
        rightLeg=dict(pitch=+6, bend=9, z=+0.03),
    ),
    # 分掌启动：双臂向外掰开、胸口开始打开。
    6: dict(
        easing="INQUAD",
        body=dict(y=-0.03, z=-0.01),
        head=dict(pitch=+2, yaw=0),
        torso=dict(pitch=+1, yaw=0),
        rightArm=dict(pitch=-58, yaw=-30, roll=+14, bend=55, axis=180),
        leftArm=dict(pitch=-55, yaw=+33, roll=-13, bend=58, axis=180),
        leftLeg=dict(pitch=-7, bend=9, z=-0.03),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03),
    ),
    # 呈出定势（strike 顶点）：双掌外下方摊开呈骨、抬首开胸。
    9: dict(
        easing="OUTSINE",
        body=dict(y=0.0, z=+0.02),
        head=dict(pitch=-8, yaw=0),
        torso=dict(pitch=-6, yaw=0),
        rightArm=dict(pitch=-35, yaw=-32, roll=+6, bend=20, axis=180),
        leftArm=dict(pitch=-32, yaw=+35, roll=-6, bend=22, axis=180),
        leftLeg=dict(pitch=-5, bend=6, z=-0.02),
        rightLeg=dict(pitch=+4, bend=5, z=+0.02),
    ),
    # 收势中段：落臂、直身。
    11: dict(
        easing="INOUTSINE",
        body=dict(y=-0.008, z=+0.008),
        head=dict(pitch=-3, yaw=0),
        torso=dict(pitch=-2, yaw=0),
        rightArm=dict(pitch=-16, yaw=-14, roll=+3, bend=10, axis=180),
        leftArm=dict(pitch=-14, yaw=+16, roll=-3, bend=11, axis=180),
        leftLeg=dict(pitch=-2, bend=3, z=-0.01),
        rightLeg=dict(pitch=+2, bend=2, z=+0.01),
    ),
    # 归中立。
    14: dict(
        easing="INOUTSINE",
        body=dict(y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftLeg=dict(pitch=0, bend=0, z=0.0),
        rightLeg=dict(pitch=0, bend=0, z=0.0),
    ),
}


def main() -> int:
    emit_json(
        POSE,
        name="anqi_charge_carrier_release",
        description=(
            "P2 封骨充能完成收势（14t 非循环）：anticipation 0→4 封口紧压（bend→"
            "100/102 / body.y -0.068），strike 4→9 分掌呈出（pitch→-35/-32 外展 / "
            "torso -6 开胸 / 抬首 -8），recovery 9→14 经 t11 落回中立。"
        ),
        end_tick=14,
        stop_tick=16,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
