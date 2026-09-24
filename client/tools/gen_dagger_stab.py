#!/usr/bin/env python3
"""dagger_stab —— 匕首低位直刺（WoundKind::Pierce 的匕首版）。

## v3：首末两帧是人手摆的，这里只做过渡

2026-08-31 用户在 Blockbench 里重摆了 t0 与 t8，并**把中间帧删了**——存盘里
`arm_*_pitch` / `arm_*_bend` / `head_pitch` 只剩首末两个关键帧。意思很直白：起手式和
收势按他给的来，中间自己填。两端逐轴回读走
`bbmodel_maker.workbench.bbmodel_to_pose`（生成器 `4.10` / Blockbench 存盘 `5.0`
符号相反，那个脚本按 `format_version` 自动分辨），读回来原样钉死；t2/t3/t5/t6 按
「握把在哪 / 刃朝哪 / 副手在哪」最小二乘反解。

两端读回来长这样（Bedrock 世界 px：+x 玩家左 / +y 上 / −z 前）：

| | 握把 | 刃向仰角 | 肘 bend |
|---|---|---|---|
| t0 起手 | (−4.4, 16.6, −9.6) | −10°（平指前方略压） | 55.5°（收肘蓄势） |
| t8 收势 | (−2.5, 20.8, −13.6) | +6.6°（送出后微抬） | 6.9°（几乎打直） |

## 这条动画不再回到起手式

v2 的末帧与首帧逐轴相等（连击不跳帧）。用户把末帧改成了**完全伸展的定格**，回程只
靠 `endTick 8 → stopTick 10` 那两 tick 混出带回站架。相应地，门禁那边：

- 「收势闭合」收窄到下盘（`guard_parts`）—— 他一根没动 body/torso/head/两腿，那边
  首末仍逐轴相同，这条约束在那儿仍然成立、仍有 teeth；
- 手臂那一头改由 `gate_decel`（末格速度不许还是峰速，实测占峰速 0%）和
  `gate_blendout`（必须留出混出段）接手；
- 「肘不打直」收在 `t≤6`（`elbow_until`），窗口内实测最小 20°。

## 反解里压着的两条约束（v2 就有，v3 继续）

- **头当障碍物**。只约束手和刃时，求解器会大方地把**肘**举进脑袋（实测某版
  grip_switch 的肘到了 y=25.7，正插在头方块侧面）。
- **插值路径也算进残差**。tick 必须是整数（`AnimationJson` 读的是 `getAsInt()`），
  相邻整数帧之间插不进东西，所以「中间那半 tick 会不会把刀甩过头顶」只能在解**这一
  帧**的时候就算进去。

v3 另外补了一条：**端点欧拉重绕**。PlayerAnimator 是逐轴插值的，走哪条路完全由欧拉
数值决定。用户存盘里反握上撕的起手式是 `pitch=+175.1`、收势是 `−70.9`，逐轴插值会让
手臂正着转 246°；同一个姿态写成 `−184.9` 就只需反着转 114°。两种写法渲出来一模一样，
差别全在中间那七格 —— 所以端点在解完之后要挑「离邻帧最近的那个等价写法」。

## 8 tick 分段（docs/player-animation-conventions.md §1）

    tick 0  起手    用户手摆：刀在右胯前、刃平指对手略下压
    tick 2  腿先动   后腿蹬地，刀开始回收、点下沉
    tick 3  LOAD    刀收到右肋，刃压向腹线；重心后坐
    tick 5  IMPACT  握把沿直线送出（发力段 8.5px），刃摆平
    tick 6  overshoot 再送一寸 + 腕翻（末端关节滞后 1 tick）
    tick 8  用户手摆的伸展定格

峰值错开：腿 t2 → 腰 t3 → 肩 t5 → 肘/腕 t6。实测峰速落在 t5.00。

## easing 的管辖方向（conventions §15）

每帧的 easing 管「本帧 → 下一帧」，不是「怎么到达本帧」，所以按段写在起始侧：
t0/t2 蓄势 OUT 族，**t3 发力 INCUBIC**（单调加速到撞击），t5 余势 OUTQUAD，
t6 收势 INOUTSINE。`gate_easing` 从源头侧、`gate_peak` 从结果侧各锁一道。

## 站架

`body.yaw = -30°` 恒定（右前架，右肩在前 —— 持刀手离对手最近），头反向补 +28°
保持世界朝向不变。`torso.*` 只作用于躯干 ModelPart，头/臂/腿各自独立
（conventions §L243），所以出手的转体由 `torso.yaw` 给，站架由 `body.yaw` 给；
两者分工不能混，body 跟着动的话脚会在地上打滑。
"""

from anim_common import emit_json

POSE = {
    0: dict(  # 用户手摆的起手式 —— 刀在右胯前、刃平指对手略下压（实测 −10°）
        easing="OUTSINE",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=+6.2, yaw=+28.6, roll=-9.2, bend=+55.5, axis=180),
        leftArm=dict(pitch=-22.9, yaw=+21.8, roll=+2.3, bend=+27.2, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
    ),
    2: dict(  # 腿先动 —— 后腿蹬地，刀开始回收、点下沉
        easing="OUTQUAD",
        body=dict(x=+0.04, y=+0.01, z=-0.03, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+26.0),
        torso=dict(pitch=+5.0, yaw=+24.0),
        rightArm=dict(pitch=+48.8, yaw=+35.0, roll=-4.9, bend=+86.4, axis=180),
        leftArm=dict(pitch=-41.9, yaw=+23.0, roll=+3.8, bend=+15.0, axis=180),
        rightLeg=dict(pitch=-10.0, yaw=+4.0, bend=+18.0, axis=0),
        leftLeg=dict(pitch=+16.0, yaw=+2.0, bend=+30.0, axis=0),
    ),
    3: dict(  # LOAD —— 刀收到右肋，刃压向腹线；重心后坐
        easing="INCUBIC",
        body=dict(x=+0.05, y=+0.02, z=-0.05, yaw=-30.0),
        head=dict(pitch=+0.0, yaw=+24.0),
        torso=dict(pitch=+6.0, yaw=+32.0),
        rightArm=dict(pitch=+69.1, yaw=+38.2, roll=-3.0, bend=+97.9, axis=180),
        leftArm=dict(pitch=-46.1, yaw=+23.7, roll=+4.5, bend=+15.0, axis=180),
        rightLeg=dict(pitch=-8.0, yaw=+4.0, bend=+16.0, axis=0),
        leftLeg=dict(pitch=+18.0, yaw=+2.0, bend=+34.0, axis=0),
    ),
    5: dict(  # IMPACT —— 握把沿直线送出（发力段 8.5px），刃摆平
        easing="OUTQUAD",
        body=dict(x=-0.02, y=-0.01, z=+0.14, yaw=-30.0),
        head=dict(pitch=+4.0, yaw=+32.0),
        torso=dict(pitch=+8.0, yaw=-18.0),
        rightArm=dict(pitch=-42.1, yaw=+14.3, roll=+4.4, bend=+20.0, axis=180),
        leftArm=dict(pitch=-16.6, yaw=+17.7, roll=+3.2, bend=+15.0, axis=180),
        rightLeg=dict(pitch=-22.0, yaw=+8.0, bend=+32.0, axis=0),
        leftLeg=dict(pitch=+22.0, yaw=+0.0, bend=+12.0, axis=0),
    ),
    6: dict(  # overshoot —— 再送一寸 + 腕翻（末端关节滞后 1 tick）
        easing="INOUTSINE",
        body=dict(x=-0.01, y=-0.01, z=+0.16, yaw=-30.0),
        head=dict(pitch=+5.0, yaw=+34.0),
        torso=dict(pitch=+8.0, yaw=-22.0),
        rightArm=dict(pitch=-42.3, yaw=+13.7, roll=+17.3, bend=+20.0, axis=180),
        leftArm=dict(pitch=-9.1, yaw=+16.5, roll=+4.2, bend=+20.8, axis=180),
        rightLeg=dict(pitch=-24.0, yaw=+8.0, bend=+34.0, axis=0),
        leftLeg=dict(pitch=+23.0, yaw=+0.0, bend=+10.0, axis=0),
    ),
    8: dict(  # 用户手摆的伸展定格（肘几乎打直，靠 stopTick 两 tick 混出回站架）
        easing="INOUTSINE",
        body=dict(x=+0.02, y=+0.00, z=+0.00, yaw=-30.0),
        head=dict(pitch=+1.0, yaw=+28.0, roll=-0.0),
        torso=dict(pitch=+4.0, yaw=+14.0, roll=-0.0),
        rightArm=dict(pitch=-52.0, yaw=+14.0, roll=+11.7, bend=+6.9, axis=180),
        leftArm=dict(pitch=-22.9, yaw=+21.8, roll=+2.3, bend=+15.0, axis=180),
        rightLeg=dict(pitch=-12.0, yaw=+4.0, roll=-0.0, bend=+22.0, axis=0),
        leftLeg=dict(pitch=+12.0, yaw=+2.0, roll=-0.0, bend=+20.0, axis=0),
    ),
}

DESCRIPTION = (
    "v3 匕首低位直刺: 首末两帧人手摆（右胯起手 → 伸展定格），中间反解; 刀收右肋、"
    "刃压向腹线 → 沿直线送出并在撞击帧摆平，发力段握把 8.5px; 肘在 t≤6 不打直，"
    "刀尖全程在下巴线以下，腿 t2 → 腰 t3 → 肩 t5 → 腕 t6 错峰。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="dagger_stab",
        description=DESCRIPTION,
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
