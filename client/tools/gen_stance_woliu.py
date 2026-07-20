#!/usr/bin/env python3
"""stance_woliu —— 涡流功法习得亮相（P6，§8.1 #2 第 4 条遗留清偿）。

**不是循环站桩**。原资产是 `isLoop:true` 的 40t 双掌开合 ping-pong，与梯队一
接线的实际语义错配：`emit_technique_learned_stance_triggers` 在
`TechniqueLearnedEvent` 上**单发一次** PlayAnim，全仓没有任何持续架势状态能驱动
循环，也没有任何 StopAnim 停止路径——即 conventions §13 #6「循环动画必须同批
交付停止路径」的红线违例。P6 按决议改成一次性亮相：`isLoop:false` + 收势回中立。

原资产的另外两笔欠账一并清偿：① 只有 3 个帧点（0/20/40，间隔 20t）远超精度
标准 #3 的「主轴 ≤4t 一帧」；② 除双臂外 legs/torso/head 全程不动，违反标准 #4
「发力招必须有 torso 拧转 + body 位移，不许只挥手臂」。

母题：涡流 = 吞噬 / 漩涡 / 真空。沉身塌腰把气收进丹田（吸），随即双掌自胸前
向外旋开托举，掌心相对撑出一个看不见的涡场（吐），最后卸力归中立。开合的
「合」在前、「开」在后，与吞噬意象同向。

时序（精度标准 #1/#2/#3）：
  anticipation 0→8    沉身收拢：body.y -0.06 下坐、torso.pitch +9 塌腰、
                      双臂收到胸前（pitch -38 / bend 74），OUTQUAD 收得快
  strike       8→20   外旋托举：双臂 pitch -38→-96、yaw 外分 ±34、bend 74→52
                      展开，torso.pitch +9→-5 挺身、body.y -0.06→+0.03 拔起，
                      INOUTSINE；t20 = 托举顶点（撑住 2 帧成定格）
  recovery     20→32  卸力归中立：全轴回零，INOUTSINE
endTick=32，stopTick=34，非循环。主运动轴：rightArm.pitch / leftArm.pitch /
torso.pitch / body.y。帧点 0,4,8,12,16,20,24,28,32 —— 全程间隔 ≤4t。

leg.pitch 全程 ≤ 12°（远在 §13 #5 的 40° 上限内），下盘只做微沉，重心表达
交给 body.y + torso.pitch（§13 #4）。head/torso.roll 全程不写，规避
BongAnimationAssetManifestTest 的 roll 归零约束。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 中立起手（亮相从 vanilla 冷起手接入，server 侧 fade_in=3 铺垫）。
    0: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=0, yaw=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, bend=0, axis=180),
        leftLeg=dict(pitch=0, bend=0, z=0.0),
        rightLeg=dict(pitch=0, bend=0, z=0.0),
    ),
    # 起吸：肩先沉，手臂开始抬向胸前（kinetic chain 由躯干先动，§2.2）。
    4: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.035, z=-0.01),
        head=dict(pitch=+5, yaw=0),
        torso=dict(pitch=+6, yaw=0),
        rightArm=dict(pitch=-20, yaw=-6, bend=40, axis=180),
        leftArm=dict(pitch=-20, yaw=+6, bend=40, axis=180),
        leftLeg=dict(pitch=-4, bend=6, z=-0.015),
        rightLeg=dict(pitch=+4, bend=5, z=+0.015),
    ),
    # anticipation 末帧 / strike 起点：气收到底，双掌收拢在胸前。
    8: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.06, z=-0.02),
        head=dict(pitch=+8, yaw=0),
        torso=dict(pitch=+9, yaw=0),
        rightArm=dict(pitch=-38, yaw=-10, bend=74, axis=180),
        leftArm=dict(pitch=-38, yaw=+10, bend=74, axis=180),
        leftLeg=dict(pitch=-7, bend=10, z=-0.025),
        rightLeg=dict(pitch=+6, bend=9, z=+0.025),
    ),
    # 外旋起：掌根先向外翻，肘还没展开（load-snap，§2.4）。
    12: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.04, z=-0.01),
        head=dict(pitch=+4, yaw=0),
        torso=dict(pitch=+5, yaw=0),
        rightArm=dict(pitch=-58, yaw=-20, bend=70, axis=180),
        leftArm=dict(pitch=-58, yaw=+20, bend=70, axis=180),
        leftLeg=dict(pitch=-6, bend=9, z=-0.02),
        rightLeg=dict(pitch=+5, bend=8, z=+0.02),
    ),
    # 撑开中段：肘展、掌心相对，涡场被撑出来。
    16: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.01, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=-82, yaw=-30, bend=60, axis=180),
        leftArm=dict(pitch=-82, yaw=+30, bend=60, axis=180),
        leftLeg=dict(pitch=-4, bend=6, z=-0.012),
        rightLeg=dict(pitch=+3, bend=5, z=+0.012),
    ),
    # strike 顶点：托举定格（挺身拔起，涡场最大）。
    20: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=+0.03, z=+0.01),
        head=dict(pitch=-4, yaw=0),
        torso=dict(pitch=-5, yaw=0),
        rightArm=dict(pitch=-96, yaw=-34, bend=52, axis=180),
        leftArm=dict(pitch=-96, yaw=+34, bend=52, axis=180),
        leftLeg=dict(pitch=-2, bend=4, z=-0.008),
        rightLeg=dict(pitch=+2, bend=3, z=+0.008),
    ),
    # recovery 起：撑住一拍后开始卸力（顶点后 4t 才落，避免一到顶就掉）。
    24: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=+0.015, z=+0.005),
        head=dict(pitch=-2, yaw=0),
        torso=dict(pitch=-3, yaw=0),
        rightArm=dict(pitch=-74, yaw=-24, bend=44, axis=180),
        leftArm=dict(pitch=-74, yaw=+24, bend=44, axis=180),
        leftLeg=dict(pitch=-1, bend=3, z=-0.005),
        rightLeg=dict(pitch=+1, bend=2, z=+0.005),
    ),
    # 落臂中段。
    28: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.005, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=+1, yaw=0),
        rightArm=dict(pitch=-32, yaw=-10, bend=20, axis=180),
        leftArm=dict(pitch=-32, yaw=+10, bend=20, axis=180),
        leftLeg=dict(pitch=0, bend=1, z=0.0),
        rightLeg=dict(pitch=0, bend=1, z=0.0),
    ),
    # 归中立（一次性亮相的收势终点，非循环故不需回绕补帧）。
    32: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0),
        rightArm=dict(pitch=0, yaw=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, bend=0, axis=180),
        leftLeg=dict(pitch=0, bend=0, z=0.0),
        rightLeg=dict(pitch=0, bend=0, z=0.0),
    ),
}


def main() -> int:
    emit_json(
        POSE,
        name="stance_woliu",
        description=(
            "P6 涡流功法习得亮相（32t 非循环）：anticipation 0→8 沉身收拢"
            "（body.y -0.06 / torso.pitch +9 / 双臂收胸前 bend 74），strike "
            "8→20 双掌外旋托举撑开涡场（双臂 pitch -38→-96 / yaw 外分 ±34 / "
            "挺身 body.y +0.03），recovery 20→32 卸力归中立。取代原 40t "
            "isLoop 站桩（无停止路径、3 帧点、只动手臂）。"
        ),
        end_tick=32,
        stop_tick=34,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
