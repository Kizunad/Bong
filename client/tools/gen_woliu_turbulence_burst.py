#!/usr/bin/env python3
"""woliu_turbulence_burst —— 乱流爆：t0 炸开→乱流余震→息止（P3，瞬发结算型）。

通道核验（P3 第一性原理，2026-07-19）：`cast_turbulence_burst`
（server/src/combat/woliu_v2/skills.rs:247）→ `resolve_woliu_v2_skill`（:305）
**resolver 同步一次性结算**：concussion 伤害当场落（`apply_v3_runtime_effects`
turbulence 分支），零 `Casting`、零 timer、零打断窗——cast_ticks=40 仅作
`CastResult::Started.anim_duration_ticks` 透传（:479），连 CastSync 进度条都因
无 Casting 被跳过（client_request_handler `push_skill_cast_started_sync` 直接
return）。**无引导窗可挂循环段** → 按 conventions §13 #2 例外 ③ 入
**瞬发结算型分类契约**（INSTANT_RESOLVER_SKILLS + instant manifest，
strike 顶点=tick 0 与结算同帧），出 CAST_ALIGNMENT_ALLOWLIST。通道日后真实化
（引入 Casting 引导窗）则退类改两段式。id 不变原地重制（原 40t/35KF 三帧点
稀疏错配）。

母题「乱流炸开」：**开帧即爆**——双臂甩至斜上极限、躯干后仰、身体上浮（乱流
从自身炸出的反冲），其后乱流余震（躯干左右摆、双臂不对称涡摆衰减）→ 息止。

时序（instant 契约 + 精度标准 #3）：
  strike    0→4   t0 爆开顶点（双臂 -140/-136 甩上 / torso.pitch -14 后仰 /
                  body.y +0.06）→ t2 冲击回落 → t4 余震起（torso.yaw +10）
  recovery  4→20  t7 余震 B（yaw -8 反摆 / 臂不对称涡摆）→ t10 衰减 → t14
                  近息 → t17 → t20 归中立（INOUTSINE）
endTick=20，stopTick=22，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
torso.pitch（全程 ≤4t 帧距、t0 落帧）。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # t0 = 爆开顶点（与 resolver 结算同帧）：双臂甩上极限、后仰上浮。
    0: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=+0.06, z=-0.04),
        head=dict(pitch=-16, yaw=0),
        torso=dict(pitch=-14, yaw=0, roll=0),
        rightArm=dict(pitch=-140, yaw=-40, roll=+20, bend=8, axis=180),
        leftArm=dict(pitch=-136, yaw=+40, roll=-20, bend=10, axis=180),
        leftLeg=dict(pitch=-12, bend=10, z=-0.06),
        rightLeg=dict(pitch=+10, bend=9, z=+0.05),
    ),
    # 冲击回落：臂降一段、后仰缓。
    2: dict(
        easing="OUTSINE",
        body=dict(x=0.0, y=+0.04, z=-0.03),
        head=dict(pitch=-11, yaw=0),
        torso=dict(pitch=-10, yaw=0, roll=0),
        rightArm=dict(pitch=-120, yaw=-34, roll=+16, bend=14, axis=180),
        leftArm=dict(pitch=-116, yaw=+34, roll=-16, bend=16, axis=180),
        leftLeg=dict(pitch=-10, bend=10, z=-0.05),
        rightLeg=dict(pitch=+9, bend=9, z=+0.04),
    ),
    # 余震 A：乱流带身右摆、臂开始不对称。
    4: dict(
        easing="INOUTSINE",
        body=dict(x=+0.02, y=+0.02, z=-0.02),
        head=dict(pitch=-6, yaw=+4),
        torso=dict(pitch=-6, yaw=+10, roll=+3),
        rightArm=dict(pitch=-98, yaw=-26, roll=+8, bend=22, axis=180),
        leftArm=dict(pitch=-88, yaw=+30, roll=-22, bend=26, axis=180),
        leftLeg=dict(pitch=-8, bend=10, z=-0.04),
        rightLeg=dict(pitch=+7, bend=9, z=+0.03),
    ),
    # 余震 B：反向摆、臂交错涡摆。
    7: dict(
        easing="INOUTSINE",
        body=dict(x=-0.02, y=+0.01, z=-0.01),
        head=dict(pitch=-3, yaw=-4),
        torso=dict(pitch=-3, yaw=-8, roll=-3),
        rightArm=dict(pitch=-72, yaw=-18, roll=+18, bend=28, axis=180),
        leftArm=dict(pitch=-66, yaw=+16, roll=-6, bend=24, axis=180),
        leftLeg=dict(pitch=-7, bend=9, z=-0.04),
        rightLeg=dict(pitch=+6, bend=8, z=+0.03),
    ),
    # 衰减：摆幅收敛。
    10: dict(
        easing="INOUTSINE",
        body=dict(x=+0.01, y=0.0, z=0.0),
        head=dict(pitch=-1, yaw=+2),
        torso=dict(pitch=-1, yaw=+4, roll=+1),
        rightArm=dict(pitch=-46, yaw=-12, roll=+6, bend=20, axis=180),
        leftArm=dict(pitch=-42, yaw=+12, roll=-10, bend=22, axis=180),
        leftLeg=dict(pitch=-5, bend=7, z=-0.03),
        rightLeg=dict(pitch=+4, bend=6, z=+0.02),
    ),
    # 近息。
    14: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=0, yaw=-1),
        torso=dict(pitch=0, yaw=-2, roll=0),
        rightArm=dict(pitch=-22, yaw=-6, roll=+3, bend=12, axis=180),
        leftArm=dict(pitch=-20, yaw=+6, roll=-3, bend=14, axis=180),
        leftLeg=dict(pitch=-3, bend=4, z=-0.02),
        rightLeg=dict(pitch=+2, bend=3, z=+0.01),
    ),
    # 息止过渡。
    17: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
        head=dict(pitch=0, yaw=0),
        torso=dict(pitch=0, yaw=-1, roll=0),
        rightArm=dict(pitch=-8, yaw=-2, roll=+1, bend=5, axis=180),
        leftArm=dict(pitch=-7, yaw=+2, roll=-1, bend=6, axis=180),
        leftLeg=dict(pitch=-1, bend=2, z=-0.01),
        rightLeg=dict(pitch=+1, bend=1, z=+0.01),
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
        name="woliu_turbulence_burst",
        description=(
            "P3 乱流爆重制（20t 非循环，instant 契约：strike 顶点=tick 0 与 "
            "resolver 结算同帧；cast_ticks=40 为元数据）：t0 爆开顶点（双臂 "
            "-140/-136 / torso.pitch -14 后仰 / body.y +0.06）→ 乱流余震（torso."
            "yaw +10/-8 交替 / 双臂不对称涡摆衰减）→ 息止归中立。"
        ),
        end_tick=20,
        stop_tick=22,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
