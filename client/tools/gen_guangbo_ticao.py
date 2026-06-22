#!/usr/bin/env python3
"""guangbo_ticao —— 广播体操完整套路（#4 各招专属动画补全）。

此前 ANIM_GUANGBO_TICAO 复用 bong:guard_raise（4 tick 举臂格挡），真机表现"只动了一下"。
本动画给广播体操一套**完整的多节套路**（150 tick / 7.5s, 非循环），收录 5 节经典动作：

  第一节 伸展运动 (8-36)   ：双臂自体侧上举过头 + 踮脚(body.y) → 落
  第二节 扩胸运动 (36-64)  ：双臂前平举 → 后振扩胸(yaw 外展 + torso 挺胸) → 收
  第三节 体转运动 (64-96)  ：双臂侧平举, torso 左右拧转(yaw ±30)
  第四节 体侧运动 (96-124) ：torso 左右侧屈(roll ±26), 异侧臂上举过头
  第五节 下蹲运动 (124-150)：屈膝下蹲(body.y↓ + legs bend) 双臂前平举 → 起立收势

反僵硬 / PlayerAnimator 陷阱规避（见 memory feedback_playeranimator_gotchas）：
  - 无 IK：腿只能整条 bend，下蹲用温和 bend≤38° + body.y 下沉造"蹲"感，不强求屈膝
  - 不过头绕轴：手臂上举用 pitch≈-104°(经验阈值: <-130° 会绕过头顶朝身后下方)
  - 非循环：is_loop=False，末帧回中性，靠 stop_tick 缓收到 defaultValue（T-pose 化）
  - 节奏变速：每节起 INOUTSINE 进、动作峰 OUTQUAD/INOUTSINE，避免匀速死板
  - 对称臂用显式 L/R（体侧节是异侧不对称，显式更清晰）

预览：uv run client/tools/render_animation.py \
        client/src/main/resources/assets/bong/player_animation/guangbo_ticao.json
"""
from anim_common import emit_json

# 中性预备姿（每节回到此处过渡）
NEUTRAL = dict(
    body=dict(y=0.0),
    head=dict(pitch=0, yaw=0),
    torso=dict(pitch=0, yaw=0, roll=0),
    rightArm=dict(pitch=0, yaw=0, bend=0, axis=180),
    leftArm=dict(pitch=0, yaw=0, bend=0, axis=180),
    rightLeg=dict(bend=0),
    leftLeg=dict(bend=0),
)


def neutral(easing="INOUTSINE"):
    p = {k: dict(v) for k, v in NEUTRAL.items()}
    p["easing"] = easing
    return p


POSE = {
    0: neutral("INOUTSINE"),
    # ── 第一节 伸展运动：双臂自体侧上举过头 + 踮脚 ──────────────────────────
    8: dict(  # 预备：双臂略外开
        easing="INOUTSINE",
        body=dict(y=0.0), head=dict(pitch=0, yaw=0), torso=dict(pitch=0, yaw=0, roll=0),
        rightArm=dict(pitch=-12, yaw=-8, bend=0, axis=180),
        leftArm=dict(pitch=-12, yaw=+8, bend=0, axis=180),
        rightLeg=dict(bend=0), leftLeg=dict(bend=0),
    ),
    20: dict(  # 上举过头 + 踮脚 + 略仰头
        easing="OUTQUAD",
        body=dict(y=+0.06), head=dict(pitch=-16, yaw=0), torso=dict(pitch=-6, yaw=0, roll=0),
        rightArm=dict(pitch=-104, yaw=-6, bend=8, axis=180),
        leftArm=dict(pitch=-104, yaw=+6, bend=8, axis=180),
        rightLeg=dict(bend=0), leftLeg=dict(bend=0),
    ),
    28: dict(  # 保持伸展
        easing="INOUTSINE",
        body=dict(y=+0.07), head=dict(pitch=-16, yaw=0), torso=dict(pitch=-6, yaw=0, roll=0),
        rightArm=dict(pitch=-105, yaw=-5, bend=6, axis=180),
        leftArm=dict(pitch=-105, yaw=+5, bend=6, axis=180),
        rightLeg=dict(bend=0), leftLeg=dict(bend=0),
    ),
    36: neutral("INOUTSINE"),  # 落臂回中
    # ── 第二节 扩胸运动：前平举 → 后振扩胸 → 收 ───────────────────────────
    44: dict(  # 双臂前平举（水平向前）
        easing="INOUTSINE",
        body=dict(y=0.0), head=dict(pitch=0, yaw=0), torso=dict(pitch=0, yaw=0, roll=0),
        rightArm=dict(pitch=-88, yaw=-4, bend=0, axis=180),
        leftArm=dict(pitch=-88, yaw=+4, bend=0, axis=180),
        rightLeg=dict(bend=0), leftLeg=dict(bend=0),
    ),
    52: dict(  # 后振扩胸：双臂外展到侧后 + 挺胸 + 略抬头
        easing="OUTQUAD",
        body=dict(y=0.0), head=dict(pitch=-8, yaw=0), torso=dict(pitch=-10, yaw=0, roll=0),
        rightArm=dict(pitch=-90, yaw=-52, bend=6, axis=180),
        leftArm=dict(pitch=-90, yaw=+52, bend=6, axis=180),
        rightLeg=dict(bend=0), leftLeg=dict(bend=0),
    ),
    60: dict(  # 再前平举（第二拍）
        easing="INOUTSINE",
        body=dict(y=0.0), head=dict(pitch=0, yaw=0), torso=dict(pitch=0, yaw=0, roll=0),
        rightArm=dict(pitch=-88, yaw=-4, bend=0, axis=180),
        leftArm=dict(pitch=-88, yaw=+4, bend=0, axis=180),
        rightLeg=dict(bend=0), leftLeg=dict(bend=0),
    ),
    64: neutral("INOUTSINE"),
    # ── 第三节 体转运动：双臂侧平举, 躯干左右拧转 ─────────────────────────
    72: dict(  # 侧平举 + 右转
        easing="INOUTSINE",
        body=dict(y=0.0), head=dict(pitch=0, yaw=+18), torso=dict(pitch=0, yaw=+30, roll=0),
        rightArm=dict(pitch=-90, yaw=-78, bend=4, axis=180),
        leftArm=dict(pitch=-90, yaw=+78, bend=4, axis=180),
        rightLeg=dict(bend=0), leftLeg=dict(bend=0),
    ),
    84: dict(  # 左转
        easing="INOUTSINE",
        body=dict(y=0.0), head=dict(pitch=0, yaw=-18), torso=dict(pitch=0, yaw=-30, roll=0),
        rightArm=dict(pitch=-90, yaw=-78, bend=4, axis=180),
        leftArm=dict(pitch=-90, yaw=+78, bend=4, axis=180),
        rightLeg=dict(bend=0), leftLeg=dict(bend=0),
    ),
    96: neutral("INOUTSINE"),
    # ── 第四节 体侧运动：躯干侧屈, 异侧臂上举过头 ─────────────────────────
    106: dict(  # 向右侧屈：左臂上举, 右臂叉腰
        easing="INOUTSINE",
        body=dict(y=0.0), head=dict(pitch=0, yaw=0), torso=dict(pitch=0, yaw=0, roll=+26),
        rightArm=dict(pitch=-28, yaw=-6, bend=96, axis=180),
        leftArm=dict(pitch=-104, yaw=+10, bend=8, axis=180),
        rightLeg=dict(bend=0), leftLeg=dict(bend=0),
    ),
    116: dict(  # 向左侧屈：右臂上举, 左臂叉腰
        easing="INOUTSINE",
        body=dict(y=0.0), head=dict(pitch=0, yaw=0), torso=dict(pitch=0, yaw=0, roll=-26),
        rightArm=dict(pitch=-104, yaw=-10, bend=8, axis=180),
        leftArm=dict(pitch=-28, yaw=+6, bend=96, axis=180),
        rightLeg=dict(bend=0), leftLeg=dict(bend=0),
    ),
    124: neutral("INOUTSINE"),
    # ── 第五节 下蹲运动：屈膝下蹲 + 双臂前平举 → 起立收势 ─────────────────
    134: dict(  # 下蹲（body.y 下沉 + 温和屈膝 + 双臂前举平衡）
        easing="OUTQUAD",
        body=dict(y=-0.20), head=dict(pitch=+6, yaw=0), torso=dict(pitch=+12, yaw=0, roll=0),
        rightArm=dict(pitch=-86, yaw=-6, bend=10, axis=180),
        leftArm=dict(pitch=-86, yaw=+6, bend=10, axis=180),
        rightLeg=dict(bend=38), leftLeg=dict(bend=38),
    ),
    142: dict(  # 起立
        easing="INOUTSINE",
        body=dict(y=0.0), head=dict(pitch=0, yaw=0), torso=dict(pitch=0, yaw=0, roll=0),
        rightArm=dict(pitch=-30, yaw=-6, bend=0, axis=180),
        leftArm=dict(pitch=-30, yaw=+6, bend=0, axis=180),
        rightLeg=dict(bend=0), leftLeg=dict(bend=0),
    ),
    150: neutral("INOUTSINE"),  # 收势回中性
}

DESCRIPTION = (
    "广播体操完整套路 150tick/7.5s 非循环 5 节：伸展(双臂上举踮脚)/扩胸(前平举后振)/"
    "体转(侧平举躯干拧转±30°)/体侧(侧屈roll±26°异侧臂上举)/下蹲(body.y-0.20+屈膝38°)。"
    "替代此前复用 guard_raise 的'只动一下'。无 IK 用温和 bend+body.y 造蹲, 上举 pitch-104°避绕轴(>-130°会绕过头顶朝身后)。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="guangbo_ticao",
        description=DESCRIPTION,
        end_tick=150,
        stop_tick=158,
        is_loop=False,
    )
