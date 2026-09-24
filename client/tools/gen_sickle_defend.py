#!/usr/bin/env python3
"""sickle_defend — 采药刀应急防身：采药人被扑上来了，拿手里的刀乱划一记。

## 这条为什么不能复用 `dagger_slash`

`dagger_slash` 是**刀三件套**（石刃 / 凡铁匕首 / 骨刺）的普攻，按 `WoundKind` 选，
它的主体是一个会打架的人：站架 `body.yaw=-34°` 侧身对敌、腰转 62° 送刀、副手在胸前
探距格挡、impact 时刀扫过身前。那是**兵器**的身法。

采药刀是 `category=tool`：「凡铁小刀，刃薄而短，只够割根须和药茎；急了也能划人，刃口
很快卷」（`server/assets/items/tools.toml`）。挥它的人是采药人不是武人。所以本条围绕
三条**反过来**的特征建，每条都可量：

1. **人是往后躲的，不是压上去的。** `dagger_slash` 的 impact 帧腰压到 -20° 送肩前送；
   这里 impact 帧上身反而**留在后侧**、头继续偏开——读作"一边后仰一边乱划"。
2. **肘折得更深、全程不打直。** 匕首那条 impact 最浅 bend=58、overshoot 52；这里最浅
   **45** 也只出现一帧，guard 深到 62。手臂始终缩着，够不远。
3. **副手是挡脸的，不是探距的。** 匕首那条副手在胸前做 load-snap 帮着发力；这里左臂
   全程高抬护住头脸（bend 95~110），是"护"不是"用"。

站架也刻意不同：`body.yaw = -20°`（匕首是 -34°），半侧身而不是完整格斗架——因为这人
根本没摆过架子，是被逼着转过来的。

## 骨架硬几何（推导见 `gen_harvest_crouch.py` 的同名小节）

- `torso.pitch` 撕腰缝（枢轴在**颈**，`gap ≈ 12·sin(pitch)`，躯干厚 4px）⇒ 压在 10° 内；
  **`torso.yaw` 免费**，挥砍的转体全走 yaw。
- 腿只做「错步站稳」，脚不许离地。
- `body.yaw` 是唯一能转**整个人**（含头、腿、手持物）的通道，取恒定值；`body` 的
  平移轴（x/y/z）不写——单位与 +Y 方向在预览和运行时相反，未经真机定案。

## 8 tick 分段（conventions §1，与 `dagger_slash` 同骨架便于对比）

    t0  guard      刀横在腹前偏右，肘深折 62；左臂已经抬起来护脸
    t2  腿先动      后腿蹬地（kinetic chain 起点），刀往右外带
    t3  LOAD       刀收到最外侧，腰装载 +30°；发力段从这里起加速（INCUBIC，§15.2）
    t5  IMPACT     横划扫过身前，腰转到 -14°；**上身仍偏后**，肘只开到 45
    t6  overshoot  腕再翻、肘再收 3°
    t8  == t0      缩回护身姿

用法:
    python3 client/tools/gen_sickle_defend.py
"""

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "client" / "tools"))
from anim_common import emit_json  # noqa: E402

# 站架：半侧身 -20°（匕首那条是 -34° 的完整格斗架），全程恒定——站架是站架，
# 挥砍的转体由 torso.yaw 给；body 跟着逐帧动会让脚在地上打滑。
STANCE_YAW = -20

# 角度用度数；腿 z 的单位是 ModelPart 枢轴 px
POSE = {
    # ═══ t0 GUARD —— 刀横在腹前偏右，肘深折；左臂护脸（§2.1 不是 vanilla 垂手）═══
    0: dict(
        easing="OUTSINE",
        body=dict(yaw=STANCE_YAW),
        # 头反向补偿回来，保持世界朝向朝敌（世界朝向 = body.yaw + head.yaw）
        head=dict(pitch=+6, yaw=+14, roll=0),
        torso=dict(pitch=+8, yaw=+14, roll=0),
        rightArm=dict(pitch=-10, yaw=+15, roll=0, bend=62, axis=180),
        # 左臂高抬护住头脸——"护"不是"用"
        leftArm=dict(pitch=-58, yaw=+14, roll=0, bend=98, axis=180),
        rightLeg=dict(pitch=-14, yaw=+4, bend=22, z=-2.0),
        leftLeg=dict(pitch=+6, yaw=+4, bend=16, z=+1.5),
    ),

    # ═══ t2 腿先动 —— 后腿蹬地，刀往右外带（kinetic chain 起点）═══
    2: dict(
        easing="OUTQUAD",
        body=dict(yaw=STANCE_YAW),
        head=dict(pitch=+4, yaw=+16, roll=0),
        torso=dict(pitch=+9, yaw=+22, roll=0),
        rightArm=dict(pitch=-14, yaw=+28, roll=-6, bend=58, axis=180),
        # 左臂微展（辅助肢 load 相，反相位）——护得松一点点
        leftArm=dict(pitch=-55, yaw=+16, roll=0, bend=92, axis=180),
        rightLeg=dict(pitch=-16, yaw=+4, bend=24, z=-2.0),
        leftLeg=dict(pitch=+8, yaw=+4, bend=18, z=+1.5),
    ),

    # ═══ t3 LOAD —— 刀收到最外侧，腰装载 +30°；发力从这里起加速（§15.2）═══
    3: dict(
        easing="INCUBIC",
        body=dict(yaw=STANCE_YAW),
        head=dict(pitch=+3, yaw=+18, roll=0),
        torso=dict(pitch=+9, yaw=+30, roll=0),
        rightArm=dict(pitch=-18, yaw=+38, roll=-12, bend=54, axis=180),
        leftArm=dict(pitch=-52, yaw=+18, roll=0, bend=88, axis=180),
        rightLeg=dict(pitch=-17, yaw=+4, bend=25, z=-2.0),
        leftLeg=dict(pitch=+9, yaw=+4, bend=19, z=+1.5),
    ),

    # ═══ t5 IMPACT —— 横划扫过身前；肘只开到 45（工具，够不远也不该够远）═══
    # 关键差异：上身**没有**压上去。头继续偏开、torso 只转到 -14（匕首那条压到 -20
    # 且是送肩前进），读作"一边后仰一边乱划"而不是"沉肩送刀"。
    5: dict(
        easing="OUTQUAD",
        body=dict(yaw=STANCE_YAW),
        head=dict(pitch=+2, yaw=+26, roll=0),
        torso=dict(pitch=+8, yaw=-14, roll=0),
        rightArm=dict(pitch=-10, yaw=-42, roll=-24, bend=45, axis=180),
        # 左臂继续护脸并收得更紧（counter-pull，但方向是"缩"不是"发力"）
        leftArm=dict(pitch=-64, yaw=+10, roll=0, bend=110, axis=180),
        rightLeg=dict(pitch=-12, yaw=+2, bend=20, z=-2.0),
        leftLeg=dict(pitch=+4, yaw=+2, bend=14, z=+1.5),
    ),

    # ═══ t6 OVERSHOOT —— 末端滞后 1 tick：腕再翻 8°、肘再收 3°（§2.6）═══
    6: dict(
        easing="INOUTSINE",
        body=dict(yaw=STANCE_YAW),
        head=dict(pitch=+2, yaw=+28, roll=0),
        torso=dict(pitch=+8, yaw=-18, roll=0),
        rightArm=dict(pitch=-8, yaw=-49, roll=-32, bend=42, axis=180),
        leftArm=dict(pitch=-62, yaw=+9, roll=0, bend=106, axis=180),
        rightLeg=dict(pitch=-11, yaw=+2, bend=19, z=-2.0),
        leftLeg=dict(pitch=+3, yaw=+2, bend=13, z=+1.5),
    ),

    # ═══ t8 —— 缩回护身姿（== t0）═══
    8: dict(
        easing="INOUTSINE",
        body=dict(yaw=STANCE_YAW),
        head=dict(pitch=+6, yaw=+14, roll=0),
        torso=dict(pitch=+8, yaw=+14, roll=0),
        rightArm=dict(pitch=-10, yaw=+15, roll=0, bend=62, axis=180),
        leftArm=dict(pitch=-58, yaw=+14, roll=0, bend=98, axis=180),
        rightLeg=dict(pitch=-14, yaw=+4, bend=22, z=-2.0),
        leftLeg=dict(pitch=+6, yaw=+4, bend=16, z=+1.5),
    ),
}

DESCRIPTION = (
    "采药刀应急防身横划：采药人不是武人，三条特征都和刀三件套的 dagger_slash 反着来——"
    "① 人往后躲不往前压（impact 帧 torso 只到 -14° 且头继续偏开 +26°，"
    "匕首那条是压到 -20° 送肩前进）；"
    "② 肘折更深、全程不打直（最浅 45° 只出现一帧，guard 深到 62°；匕首最浅 58°）；"
    "③ 副手全程高抬护脸（bend 98 → 110），是'护'不是探距格挡；"
    "站架 body.yaw=-20° 半侧身（匕首 -34° 完整格斗架），恒定不逐帧动免得脚打滑，"
    "头反向补 +14° 保持世界朝向朝敌；"
    "torso.pitch 压在 9° 内（枢轴在颈会撕腰缝），转体全走免费的 torso.yaw（+30° → -18°）；"
    "发力加速写在 t3（§15.2）；t6 overshoot 腕再翻 8°、肘再收 3°；tick 0 == tick 8。"
)

if __name__ == "__main__":
    emit_json(
        POSE,
        name="sickle_defend",
        description=DESCRIPTION,
        end_tick=8,
        stop_tick=10,
        is_loop=False,
    )
