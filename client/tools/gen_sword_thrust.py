#!/usr/bin/env python3
"""sword_thrust —— 收刃于右侧后转腕直刺（异兽脊骨剑垂直握姿口径重做）。

cast_ticks=10 → endTick ∈ [14,18]，取 16（沿用，manifest 相位不变）。

时序（`client/src/test/resources/bong/anim_spec_manifests/sword_thrust.json`）：
  anticipation 0→6   剑从平举撤向右侧、拧腰蓄势（easeOut 族 OUTSINE）
  strike       6→12  转腕把刃甩回正前 + 深弓步送出（easeIn 族 INQUAD），
                     发力顶点 = tick 10（cast 完成瞬间），hold 10→12 定格
  recovery     12→16 回平举待发（INOUTSINE）
endTick=16，stopTick=18，非循环。主打击轴：rightArm.pitch / rightArm.bend /
torso.yaw / body.z。

## 为什么"收刃于右侧"而不是"收剑于腰侧再直推"

握姿是**剑身垂直于小臂**（见 `gen_beast_spine_sword_player_anim`），于是剑尖恒在
以肩为心、半径 21~25.7px 的球面上——**把手往前推并不会把剑尖往前送**。实测：剑身
指正前方时，肘从伸直折到 88° 也只能把剑尖从离肩 25.7px 拉回 21.3px，前后只差 4px，
根本读不出"刺"。

所以行程只能靠**转向**挣：蓄势帧把刃甩到身体右侧（剑尖离肩 z 只剩 −11.7px），发力帧
再转回正前（−25.0px）。相对肩的前后行程因此有 13.4px，再叠 body.z 的弓步位移。
（更极端的"刃指身后"够不着：那要求小臂垂直于 +Z，撞 yaw ±80° 的关节上限。）

tick 0（平举待发）与 tick 10（深弓步直刺）是用户在 Blockbench 里手摆的两帧，剑骨
关键帧已按静态握姿反解回手臂四轴；用户 t10 的剑尖离肩 34.5px **超出工作空间**
（上限 25.7px），已 clamp，其余姿态照搬。

## body.z 的符号

本仓两处口径打架：`render_player_pose` 把 body.z 当模型空间 +Z（= 向后）渲染，而
现网 140 条动画（含旧 sword_thrust / sword_cleave / fist_punch）一律在发力帧写正值
并注为"前冲"。这里**跟现网走**（发力帧 +0.30），待真机确认；要翻的话是全仓一次
性 sed，不是这条动画单独的事。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # 平举待发：剑横平指向正前方、腰高（用户手摆帧，反解回静态握姿）
        easing="INOUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0),
        head=dict(pitch=0, yaw=-6),
        torso=dict(pitch=+3, yaw=+10),
        rightArm=dict(pitch=+11.3, yaw=+5.3, roll=-27.3, bend=15.2, axis=180),
        leftArm=dict(pitch=0, yaw=+14, roll=-15, bend=25, axis=180),
        rightLeg=dict(pitch=+8, bend=12, axis=0),
        leftLeg=dict(pitch=-10, bend=12, axis=0),
    ),
    3: dict(  # 撤刃：刃开始转向身体右侧，腰同时往回拧（torso.yaw +10→+18）
        easing="OUTSINE",
        body=dict(x=+0.02, y=+0.02, z=-0.05),
        head=dict(pitch=-1, yaw=-2),
        torso=dict(pitch=+1, yaw=+18),
        rightArm=dict(pitch=+10.8, yaw=+43.7, roll=-16.3, bend=18.1, axis=180),
        leftArm=dict(pitch=-14, yaw=+20, roll=-18, bend=40, axis=180),
        rightLeg=dict(pitch=+12, bend=16, axis=0),
        leftLeg=dict(pitch=-15, bend=18, axis=0),
    ),
    6: dict(  # 蓄满：刃完全甩到右侧（剑尖离肩 z 只剩 -11.7），腰拧到 +24°，重心坐低
        easing="OUTSINE",
        body=dict(x=+0.02, y=+0.035, z=-0.09),
        head=dict(pitch=-2, yaw=0),
        torso=dict(pitch=0, yaw=+24),
        rightArm=dict(pitch=+7.9, yaw=+59.3, roll=-6.7, bend=15.1, axis=180),
        leftArm=dict(pitch=-20, yaw=+24, roll=-20, bend=46, axis=180),
        rightLeg=dict(pitch=+15, bend=20, axis=0),
        leftLeg=dict(pitch=-19, bend=22, axis=0),
    ),
    10: dict(  # STRIKE：转腕把刃甩回正前 + 深弓步送出（用户手摆帧）
        easing="INQUAD",
        body=dict(x=+0.02, y=+0.02, z=+0.30),
        head=dict(pitch=+5, yaw=-6),
        torso=dict(pitch=+15.5, yaw=-14, bend=10, axis=180),
        rightArm=dict(pitch=-5.0, yaw=-5.8, roll=+3.4, bend=14.4, axis=180),
        leftArm=dict(pitch=+47.5, yaw=+14, roll=-15, bend=15, axis=180),
        rightLeg=dict(pitch=+30.5, bend=17, axis=0),
        leftLeg=dict(pitch=-40, bend=47, axis=0),
    ),
    12: dict(  # hold：刺到底定格两 tick，重剑的分量感全在这里
        easing="OUTQUAD",
        body=dict(x=+0.02, y=+0.02, z=+0.26),
        head=dict(pitch=+4, yaw=-5),
        torso=dict(pitch=+14, yaw=-12, bend=9, axis=180),
        rightArm=dict(pitch=-6.5, yaw=-10.2, roll=+7.8, bend=13.0, axis=180),
        leftArm=dict(pitch=+42, yaw=+15, roll=-15, bend=18, axis=180),
        rightLeg=dict(pitch=+28, bend=17, axis=0),
        leftLeg=dict(pitch=-37, bend=44, axis=0),
    ),
    16: dict(  # recovery：收回平举待发（body 轴显式归位，防非循环残值偏移）
        easing="INOUTSINE",
        body=dict(x=+0.02, y=0.0, z=0.0),
        head=dict(pitch=0, yaw=-6),
        torso=dict(pitch=+3, yaw=+10),
        rightArm=dict(pitch=+2.8, yaw=-7.8, roll=+17.1, bend=6.0, axis=180),
        leftArm=dict(pitch=0, yaw=+14, roll=-15, bend=25, axis=180),
        rightLeg=dict(pitch=+8, bend=12, axis=0),
        leftLeg=dict(pitch=-10, bend=12, axis=0),
    ),
}

DESCRIPTION = (
    "直刺 (sword_thrust): 16-tick，剑平举待发 -> 撤刃到右侧拧腰蓄满 -> "
    "转腕甩回正前 + 深弓步送出 -> 定格 -> 收回平举。"
    "垂直握姿下剑尖恒在以肩为心的球面上，行程靠转向挣（相对肩 13.4px）。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sword_thrust",
        description=DESCRIPTION,
        end_tick=16,
        stop_tick=18,
        is_loop=False,
    )
