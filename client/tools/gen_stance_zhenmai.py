#!/usr/bin/env python3
"""stance_zhenmai —— 针脉功法习得亮相（P6，§8.1 #2 第 4 条遗留清偿）。

**不是循环站桩**。原资产更糟：`isLoop:true`、20t、三个帧点（0/10/20）**逐字节
完全相同**——是一张静止的持守姿态图在空转，没有任何动作。与
`emit_technique_learned_stance_triggers` 的单发语义错配，且无 StopAnim 停止
路径（conventions §13 #6 红线违例）。P6 按决议改成一次性亮相：`isLoop:false`
+ 真实动作 + 收势回中立。

母题：针脉 = 以指代针，点封经脉。左手虚扶（定位取穴），右手二指并拢自腰侧
提起、向前一记短促点出（下针），点定后收指归中立。发力由 torso 拧转送肩
承担（§13 #4），不是单纯甩胳膊。

时序（精度标准 #1/#2/#3）：
  anticipation 0→8    提指取穴：右臂自体侧抬起（pitch 0→-46）、torso.yaw
                      拧到 +14 蓄劲、body.y -0.045 微沉，OUTQUAD
  strike       8→20   下针：右臂前点（pitch -46→-104 / bend 收到 14 近伸直）、
                      **yaw -14→-28 带横向分量**、torso.yaw +14→+28 送肩、
                      body.z +0.11 前送，INQUAD 起 → t20 点定顶点
                      （打击轴禁 linear，§13 #3）
  recovery     20→28  收指归中立：全轴回零，INOUTSINE
endTick=28，stopTick=30，非循环。主运动轴：rightArm.pitch / rightArm.bend /
torso.yaw / body.z。帧点 0,4,8,12,16,20,24,28 —— 全程间隔 ≤4t。

左臂全程虚扶在身前偏内（yaw +22 / bend 58），与右手的「点」形成主辅分工，
不做镜像对称——避免看成双手同动的通用架势。

**点出方向带横向分量**（round 2 修正）：round 1 是纯正前方直点（rightArm.yaw
仅 -14、torso.yaw +34），三视图审下来正面几乎完全**透视缩短成一个点**——而
「远距离能分辨对面在用哪招」正是本 plan 的验收判据，正前方直刺是最差的剪影。
改法是把手臂外分到 yaw -28、同时把 torso.yaw 从 +34 收到 +28：世界方向仍近乎
正前（+28-28≈0，即「拧身而直点」的传统身法），但手臂相对躯干张开，正面剪影
成一条清晰斜线。head.pitch 同步 +8 低头看穴位，交代「点的是一个具体位置」。
leg.pitch 全程 ≤ 10°；head/torso.roll 全程不写。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 中立起手。
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
    # 提指起：右臂离体侧，torso 开始拧（躯干先于末端，§2.2）。
    4: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.025, z=-0.01),
        head=dict(pitch=+4, yaw=+5),
        torso=dict(pitch=+4, yaw=+7),
        rightArm=dict(pitch=-22, yaw=-8, bend=34, axis=180),
        leftArm=dict(pitch=-26, yaw=+14, bend=38, axis=180),
        leftLeg=dict(pitch=-3, bend=5, z=-0.012),
        rightLeg=dict(pitch=+3, bend=4, z=+0.012),
    ),
    # anticipation 末帧 / strike 起点：取穴定位，劲蓄满。
    8: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=-0.045, z=-0.02),
        head=dict(pitch=+7, yaw=+9),
        torso=dict(pitch=+6, yaw=+14),
        rightArm=dict(pitch=-46, yaw=-14, bend=62, axis=180),
        leftArm=dict(pitch=-40, yaw=+22, bend=58, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.02),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 下针中段：肘先伸、指尖跟上（末端滞后，§2.2）。
    12: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=-0.03, z=+0.03),
        head=dict(pitch=+5, yaw=+11),
        torso=dict(pitch=+4, yaw=+20),
        rightArm=dict(pitch=-70, yaw=-20, bend=44, axis=180),
        leftArm=dict(pitch=-40, yaw=+22, bend=58, axis=180),
        leftLeg=dict(pitch=-5, bend=7, z=-0.016),
        rightLeg=dict(pitch=+4, bend=6, z=+0.016),
    ),
    # 近顶点：手臂接近伸直，送肩到位。
    16: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=-0.015, z=+0.08),
        head=dict(pitch=+6, yaw=+10),
        torso=dict(pitch=+2, yaw=+24),
        rightArm=dict(pitch=-94, yaw=-24, bend=24, axis=180),
        leftArm=dict(pitch=-38, yaw=+23, bend=56, axis=180),
        leftLeg=dict(pitch=-3, bend=5, z=-0.01),
        rightLeg=dict(pitch=+3, bend=4, z=+0.01),
    ),
    # strike 顶点：点定（overshoot 到 -104 后不再前推，§2.6）。
    20: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.005, z=+0.11),
        head=dict(pitch=+8, yaw=+9),
        torso=dict(pitch=+1, yaw=+28),
        rightArm=dict(pitch=-104, yaw=-28, bend=14, axis=180),
        leftArm=dict(pitch=-36, yaw=+24, bend=54, axis=180),
        leftLeg=dict(pitch=-2, bend=4, z=-0.008),
        rightLeg=dict(pitch=+2, bend=3, z=+0.008),
    ),
    # recovery 中段：收指、解拧。
    24: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.01, z=+0.04),
        head=dict(pitch=+2, yaw=+7),
        torso=dict(pitch=+2, yaw=+15),
        rightArm=dict(pitch=-48, yaw=-8, bend=30, axis=180),
        leftArm=dict(pitch=-20, yaw=+12, bend=28, axis=180),
        leftLeg=dict(pitch=-1, bend=2, z=-0.004),
        rightLeg=dict(pitch=+1, bend=2, z=+0.004),
    ),
    # 归中立。
    28: dict(
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
        name="stance_zhenmai",
        description=(
            "针脉功法习得亮相（28t 非循环）：anticipation 0→8 提指取穴"
            "（右臂 pitch 0→-46 / torso.yaw +14 蓄拧 / body.y -0.045），strike "
            "8→20 以指代针前点下针（rightArm pitch -46→-104 / yaw -28 外分 / "
            "bend 62→14 近伸直 / torso.yaw +28 送肩 / body.z +0.11 / head.pitch "
            "+8 看穴），recovery 20→28 收指归中立。"
        ),
        end_tick=28,
        stop_tick=30,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
