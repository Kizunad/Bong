#!/usr/bin/env python3
"""morph_cast —— 易形：t0 塌形收拢→痉挛重组→立形（P3，瞬发结算型）。

通道核验（P3 第一性原理，2026-07-19）：`cast_morph_yixing`
（server/src/body_plan/morph.rs:119）**resolver 双分支立即结算**：已有
`MorphState` → `release_morph_state` 当场解除（:125-131 兽→人）；否则扣 qi 后
当场 `insert(MorphState)`（:134-181 人→兽）。零 `Casting`、零 timer、零打断窗
——`YIXING_CAST_TICKS=60`（morph.rs:96）仅作 `CastResult::Started` 元数据回填
（:130/:180），known_techniques cast_ticks=60 同为元数据。**无引导窗可挂循环
段** → 按 conventions §13 #2 例外 ③ 入**瞬发结算型分类契约**
（INSTANT_RESOLVER_SKILLS + instant manifest，strike 顶点=tick 0 与变形结算
同帧），出 CAST_ALIGNMENT_ALLOWLIST。伴随粒子 lifetime 随本动画 endTick=20
对齐（morph.rs `emit_yixing_av` duration_ticks，§8.1 #1 表现层参数）。
双向共用一条动画（Morph/Release 两分支同发，与现状一致）。id 不变原地重制
（原 30t 仅 2 帧点、30↔cast60 错配的附录 A C 级项）。

母题「塌形重组」：**开帧即塌**——深蹲抱头收拢成一团（血肉折叠瞬间，弯腰走
torso+legs 补偿），痉挛两拍（roll 交替，形骸重排）→ 缓缓立起（新形态起身）
→ 摆正。t0 与 MorphState 插入/移除同帧，玩家看到的正是「形变的那一下」。

时序（instant 契约 + 精度标准 #3）：
  strike    0→4   t0 塌形顶点（body.y -0.30 / torso.pitch +38+legs 补偿 /
                  双臂抱头 bend 110）→ t2/t4 痉挛（torso.roll +7/-6）
  recovery  4→20  t7 起身 A → t10 起身 B → t13 摆正微晃（roll +3/yaw +4）→
                  t16 近立 → t20 归中立（INOUTSINE）
endTick=20，stopTick=22，非循环。主打击轴：body.y / torso.pitch /
rightArm.pitch（全程 ≤4t 帧距、t0 落帧）。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # t0 = 塌形顶点（与 resolver 结算同帧）：深蹲抱头折叠。
    0: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.30, z=+0.08),
        head=dict(pitch=+24, yaw=0),
        torso=dict(pitch=+38, yaw=0, roll=0),
        rightArm=dict(pitch=-130, yaw=+30, roll=-10, bend=110, axis=180),
        leftArm=dict(pitch=-126, yaw=-28, roll=+10, bend=112, axis=180),
        leftLeg=dict(pitch=-20, bend=32, z=-0.06),
        rightLeg=dict(pitch=-18, bend=30, z=+0.04),
    ),
    # 痉挛 A：形骸重排第一拍。
    2: dict(
        easing="OUTSINE",
        body=dict(x=+0.01, y=-0.27, z=+0.08),
        head=dict(pitch=+22, yaw=+3),
        torso=dict(pitch=+36, yaw=+4, roll=+7),
        rightArm=dict(pitch=-122, yaw=+28, roll=-14, bend=106, axis=180),
        leftArm=dict(pitch=-118, yaw=-26, roll=+6, bend=108, axis=180),
        leftLeg=dict(pitch=-19, bend=30, z=-0.06),
        rightLeg=dict(pitch=-17, bend=28, z=+0.04),
    ),
    # 痉挛 B：反向抖。
    4: dict(
        easing="INOUTSINE",
        body=dict(x=-0.01, y=-0.25, z=+0.07),
        head=dict(pitch=+20, yaw=-3),
        torso=dict(pitch=+33, yaw=-4, roll=-6),
        rightArm=dict(pitch=-100, yaw=+24, roll=-6, bend=92, axis=180),
        leftArm=dict(pitch=-96, yaw=-22, roll=+12, bend=94, axis=180),
        leftLeg=dict(pitch=-18, bend=28, z=-0.05),
        rightLeg=dict(pitch=-16, bend=26, z=+0.04),
    ),
    # 起身 A：新形态开始展开。
    7: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.16, z=+0.05),
        head=dict(pitch=+12, yaw=0),
        torso=dict(pitch=+20, yaw=0, roll=+2),
        rightArm=dict(pitch=-60, yaw=+16, roll=-4, bend=60, axis=180),
        leftArm=dict(pitch=-56, yaw=-14, roll=+4, bend=62, axis=180),
        leftLeg=dict(pitch=-12, bend=20, z=-0.04),
        rightLeg=dict(pitch=-10, bend=18, z=+0.03),
    ),
    # 起身 B：躯干近直。
    10: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.08, z=+0.03),
        head=dict(pitch=+6, yaw=0),
        torso=dict(pitch=+10, yaw=0, roll=-1),
        rightArm=dict(pitch=-30, yaw=+8, roll=-2, bend=30, axis=180),
        leftArm=dict(pitch=-27, yaw=-8, roll=+2, bend=32, axis=180),
        leftLeg=dict(pitch=-7, bend=12, z=-0.03),
        rightLeg=dict(pitch=-5, bend=11, z=+0.02),
    ),
    # 摆正微晃：新形骸落位。
    13: dict(
        easing="INOUTSINE",
        body=dict(x=+0.005, y=-0.04, z=+0.01),
        head=dict(pitch=+3, yaw=+2),
        torso=dict(pitch=+5, yaw=+4, roll=+3),
        rightArm=dict(pitch=-14, yaw=+4, roll=-1, bend=14, axis=180),
        leftArm=dict(pitch=-12, yaw=-4, roll=+1, bend=16, axis=180),
        leftLeg=dict(pitch=-4, bend=7, z=-0.02),
        rightLeg=dict(pitch=-2, bend=6, z=+0.01),
    ),
    # 近立。
    16: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.01, z=0.0),
        head=dict(pitch=+1, yaw=0),
        torso=dict(pitch=+2, yaw=-1, roll=0),
        rightArm=dict(pitch=-5, yaw=+2, roll=0, bend=5, axis=180),
        leftArm=dict(pitch=-4, yaw=-2, roll=0, bend=6, axis=180),
        leftLeg=dict(pitch=-1, bend=3, z=-0.01),
        rightLeg=dict(pitch=-1, bend=2, z=+0.01),
    ),
    # 归中立。
    20: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=0, roll=0),
        rightArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftArm=dict(pitch=0, yaw=0, roll=0, bend=0, axis=180),
        leftLeg=dict(pitch=0, bend=0, z=0.0),
        rightLeg=dict(pitch=0, bend=0, z=0.0),
    ),
}


def main() -> int:
    emit_json(
        POSE,
        name="morph_cast",
        description=(
            "P3 易形重制（20t 非循环，instant 契约：strike 顶点=tick 0 与变形"
            "结算同帧；cast_ticks=60 为元数据）：t0 塌形抱头折叠（body.y -0.30 / "
            "torso.pitch +38+legs 补偿）→ 痉挛两拍（roll +7/-6）→ 缓起立形摆正"
            "归中立。Morph/Release 双向共用。"
        ),
        end_tick=20,
        stop_tick=22,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
