#!/usr/bin/env python3
"""sword_manifest_cast —— 剑意化形：疾凝送出→目送余韵（P2 批次二后半，review 返工重做）。

cast_ticks=40 是**元数据**：`cast_manifest`（skill_register.rs）在 cast 起始
tick 立即 spawn SwordIntentEntity 追击实体、无引导窗。返工前的 46t 版本把化形
送出顶点放在 tick 40，而化形剑 tick 0 已在场上飞——表演/结算脱节（PR #1240
review blocker）。本版遵循「动画 strike 顶点对齐真实结算点」：20t 非循环，
顶点 = tick 6（紧贴 tick 0 结算），凝形微颤保留在压缩的拉开段、目送余韵拉长。
时长对拍 allowlist 条目保留（endTick=20 与 cast=40 元数据错配如实驻表）。

母题：疾凝送出。双掌急合胸前（0→2 快速 anticipation，掌间凝剑意），竖轴急
拉开凝形（t4 上掌抬/下掌沉、剑身瞬间成形），右手翻腕虚握一送——化形剑离手
（顶点 t6），随后长目送余韵（头随剑意望远、双臂缓落，t9→t17）收势。与
heaven_gate（高举过顶蓄力）/ 基础剑招（持实剑挥斩）动向完全区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→2   疾合：双掌急合胸前、俯首注视掌间
  strike       2→8   凝形送出：t4 竖轴急拉开（右掌 -98 / 左掌 -26）→ t6 翻腕
                     虚握前送顶点（rightArm pitch -92 前指 / torso.yaw +14 /
                     body.z +0.16，INQUAD）→ t8 送出定格
  recovery     8→20  目送余韵：t11 头随剑意抬望（head.pitch -12）→ t14 双臂
                     缓落 → t17 直身 → t20 归中立（INOUTSINE）
endTick=20，stopTick=22，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
torso.yaw / body.z（全程 ≤3t 帧距）。
"""

from __future__ import annotations

from anim_common import emit_json

POSE = {
    # 疾合起手：双掌急合胸前、俯首。
    0: dict(
        easing="OUTQUAD",
        body=dict(x=0.0, y=-0.02, z=-0.02),
        head=dict(pitch=+9, yaw=0),
        torso=dict(pitch=+5, yaw=-2),
        rightArm=dict(pitch=-62, yaw=-24, roll=-10, bend=66, axis=180),
        leftArm=dict(pitch=-58, yaw=+26, roll=+10, bend=62, axis=180),
        leftLeg=dict(pitch=-6, bend=8, z=-0.03),
        rightLeg=dict(pitch=+5, bend=7, z=+0.02),
    ),
    # 合紧微沉：掌间凝意到最紧。
    2: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=-0.04, z=-0.03),
        head=dict(pitch=+12, yaw=0),
        torso=dict(pitch=+7, yaw=-4),
        rightArm=dict(pitch=-66, yaw=-30, roll=-14, bend=76, axis=180),
        leftArm=dict(pitch=-62, yaw=+32, roll=+14, bend=72, axis=180),
        leftLeg=dict(pitch=-9, bend=12, z=-0.04),
        rightLeg=dict(pitch=+7, bend=10, z=+0.03),
    ),
    # 竖轴急拉开凝形：上掌抬、下掌沉，剑身瞬间「拉长成形」。
    4: dict(
        easing="INQUAD",
        body=dict(x=0.0, y=-0.03, z=0.0),
        head=dict(pitch=+6, yaw=0),
        torso=dict(pitch=+5, yaw=-2),
        rightArm=dict(pitch=-98, yaw=-16, roll=-20, bend=30, axis=180),
        leftArm=dict(pitch=-26, yaw=+20, roll=+16, bend=34, axis=180),
        leftLeg=dict(pitch=-10, bend=13, z=-0.05),
        rightLeg=dict(pitch=+8, bend=11, z=+0.04),
    ),
    # 翻腕虚握前送顶点（紧贴 tick 0 结算点）：化形剑离手飞出。
    6: dict(
        easing="INQUAD",
        body=dict(x=-0.01, y=-0.02, z=+0.16),
        head=dict(pitch=-2, yaw=+4),
        torso=dict(pitch=+8, yaw=+14),
        rightArm=dict(pitch=-92, yaw=-6, roll=+18, bend=6, axis=180),
        leftArm=dict(pitch=-20, yaw=+18, roll=+6, bend=40, axis=180),
        leftLeg=dict(pitch=-18, bend=19, z=-0.09),
        rightLeg=dict(pitch=+14, bend=16, z=+0.06),
    ),
    # 送出定格：臂保持前指、剑意远去。
    8: dict(
        easing="OUTSINE",
        body=dict(x=-0.01, y=-0.02, z=+0.14),
        head=dict(pitch=-6, yaw=+3),
        torso=dict(pitch=+7, yaw=+11),
        rightArm=dict(pitch=-90, yaw=-7, roll=+14, bend=8, axis=180),
        leftArm=dict(pitch=-18, yaw=+16, roll=+5, bend=38, axis=180),
        leftLeg=dict(pitch=-17, bend=18, z=-0.08),
        rightLeg=dict(pitch=+13, bend=15, z=+0.06),
    ),
    # 目送 A：头随剑意抬望远方、前指臂微落。
    11: dict(
        easing="INOUTSINE",
        body=dict(x=-0.01, y=-0.01, z=+0.10),
        head=dict(pitch=-12, yaw=+2),
        torso=dict(pitch=+5, yaw=+8),
        rightArm=dict(pitch=-72, yaw=-8, roll=+8, bend=14, axis=180),
        leftArm=dict(pitch=-14, yaw=+12, roll=+4, bend=28, axis=180),
        leftLeg=dict(pitch=-13, bend=14, z=-0.06),
        rightLeg=dict(pitch=+10, bend=12, z=+0.04),
    ),
    # 目送 B：双臂缓落。
    14: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=-0.01, z=+0.06),
        head=dict(pitch=-8, yaw=+1),
        torso=dict(pitch=+3, yaw=+5),
        rightArm=dict(pitch=-40, yaw=-8, roll=+4, bend=16, axis=180),
        leftArm=dict(pitch=-10, yaw=+8, roll=+3, bend=18, axis=180),
        leftLeg=dict(pitch=-9, bend=10, z=-0.04),
        rightLeg=dict(pitch=+7, bend=8, z=+0.03),
    ),
    # 直身。
    17: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=+0.02),
        head=dict(pitch=-3, yaw=0),
        torso=dict(pitch=+1, yaw=+2),
        rightArm=dict(pitch=-16, yaw=-4, roll=+2, bend=8, axis=180),
        leftArm=dict(pitch=-5, yaw=+4, roll=+1, bend=8, axis=180),
        leftLeg=dict(pitch=-4, bend=5, z=-0.02),
        rightLeg=dict(pitch=+3, bend=4, z=+0.01),
    ),
    # 归中立。
    20: dict(
        easing="INOUTSINE",
        body=dict(x=0.0, y=0.0, z=0.0),
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
        name="sword_manifest_cast",
        description=(
            "P2 剑意化形专属（20t 非循环，strike 顶点 t6 对齐瞬发结算点——"
            "cast_manifest tick 0 即 spawn SwordIntentEntity；cast_ticks=40 为"
            "元数据、错配如实驻 allowlist）：anticipation 0→2 双掌疾合，strike "
            "2→8 竖轴急拉开凝形→翻腕虚握前送（pitch -92 / torso.yaw +14 / "
            "body.z +0.16），recovery 8→20 目送余韵（head.pitch -12 随剑意望远）"
            "→归中立。"
        ),
        end_tick=20,
        stop_tick=22,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
