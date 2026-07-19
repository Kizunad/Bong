#!/usr/bin/env python3
"""sword_manifest_cast —— 剑意化形：双掌凝剑拉开→握柄送出（P2 批次二后半重制）。

cast_ticks=40，**瞬发结算型**（`cast_manifest` 立即 spawn SwordIntentEntity 追击
实体、无引导窗，plan 附录 A 决策 (b)——保守精修不两段化：化形是 40t 完整凝形
演出，追击期玩家可自由行动，循环段无停止信号可挂）。旧 40t/32 moves 低密度
无 recovery，重制为 46t：凝形 0→40 覆盖 cast 元数据期（顶点 = tick 40 化形
送出），recovery 40→46。时长对拍 allowlist 条目保留（cast≥40 机械断言要求
isLoop，通道语义不适用——附录 A 记录）。

母题：凝剑成形。双掌于胸前相对（掌间凝聚剑意），随灌注双掌沿竖轴缓缓拉开
（上掌抬、下掌沉，剑身在掌间逐渐「拉长成形」，4t 步进渐开 + 微颤），拉满后
右手翻腕虚握剑柄、左掌向外一送——化形剑离手飞出（顶点），目送收势。与
heaven_gate（高举过顶蓄力）/ 基础剑招（持实剑挥斩）动向完全区分。

时序（精度标准 #1/#2/#3）：
  anticipation 0→28  双掌竖轴拉开凝形：右掌上抬（pitch -60→-102）、左掌下沉
                     （pitch -60→-24），4t 步进 + 3t 周期微颤（凝形张力），
                     俯首注视掌间（OUTSINE 族）
  strike       28→40 握柄送出：31 右手翻腕虚握（roll 翻转）→ 34 左掌让位 →
                     37 前送半程 → 40 化形送出顶点（rightArm pitch -95 前指 /
                     torso.yaw +14 / body.z +0.18，INQUAD）
  recovery     40→46 由送出位落臂目送回中立（INOUTSINE，t43 中段帧）
endTick=46，stopTick=48，非循环。主打击轴：rightArm.pitch / leftArm.pitch /
torso.yaw / body.z（全程 ≤4t 帧距）。
"""

from __future__ import annotations

import math

from anim_common import emit_json


def condense_frame(t: int) -> dict:
    """凝形期（0→28）某 tick：双掌竖轴渐拉开 + 微颤。"""
    k = t / 28.0
    tremor = math.sin(2.0 * math.pi * t / 3.0) * 1.6 * (0.4 + 0.6 * k)
    return dict(
        easing="OUTSINE",
        body=dict(x=0.0, y=-0.02 - 0.03 * k, z=-0.02 - 0.015 * k),
        head=dict(pitch=+8 + 5 * k, yaw=0),
        torso=dict(pitch=+4 + 3 * k, yaw=-4 * k),
        rightArm=dict(
            pitch=-60 - 42 * k + tremor,
            yaw=-16 - 4 * k,
            roll=+6 + 4 * k,
            bend=64 - 20 * k,
            axis=180,
        ),
        leftArm=dict(
            pitch=-60 + 36 * k - tremor,
            yaw=+16 + 4 * k,
            roll=-6 - 4 * k,
            bend=64 + 12 * k,
            axis=180,
        ),
        leftLeg=dict(pitch=-6 - 4 * k, bend=8 + 6 * k, z=-0.03 - 0.01 * k),
        rightLeg=dict(pitch=+5 + 4 * k, bend=7 + 5 * k, z=+0.03 + 0.01 * k),
    )


# 凝形期：0→28 每 4t 一帧（主轴密度 ≤4t 机械保证）。
POSE = {t: condense_frame(t) for t in range(0, 29, 4)}

POSE.update(
    {
        # 翻腕虚握：右手转腕扣向剑柄位（roll 翻转），左掌托稳剑身下端。
        31: dict(
            easing="INQUAD",
            body=dict(x=0.0, y=-0.05, z=-0.03),
            head=dict(pitch=+12, yaw=-2),
            torso=dict(pitch=+7, yaw=-6),
            rightArm=dict(pitch=-98, yaw=-14, roll=-14, bend=52, axis=180),
            leftArm=dict(pitch=-28, yaw=+22, roll=-12, bend=70, axis=180),
            leftLeg=dict(pitch=-10, bend=14, z=-0.04),
            rightLeg=dict(pitch=+9, bend=12, z=+0.04),
        ),
        # 左掌让位：化形剑交右手掌控，左掌向外侧翻开。
        34: dict(
            easing="INQUAD",
            body=dict(x=0.0, y=-0.04, z=+0.02),
            head=dict(pitch=+8, yaw=-1),
            torso=dict(pitch=+6, yaw=-2),
            rightArm=dict(pitch=-100, yaw=-12, roll=-6, bend=40, axis=180),
            leftArm=dict(pitch=-30, yaw=+34, roll=-18, bend=48, axis=180),
            leftLeg=dict(pitch=-12, bend=15, z=-0.05),
            rightLeg=dict(pitch=+10, bend=13, z=+0.04),
        ),
        # 前送半程：右臂开始前送、躯干转正发力。
        37: dict(
            easing="INQUAD",
            body=dict(x=0.0, y=-0.025, z=+0.10),
            head=dict(pitch=+2, yaw=0),
            torso=dict(pitch=+8, yaw=+6),
            rightArm=dict(pitch=-98, yaw=-8, roll=+2, bend=22, axis=180),
            leftArm=dict(pitch=-24, yaw=+30, roll=-14, bend=40, axis=180),
            leftLeg=dict(pitch=-16, bend=18, z=-0.07),
            rightLeg=dict(pitch=+13, bend=15, z=+0.05),
        ),
        # 化形送出顶点 = cast 完成瞬间（tick 40）：右臂前指送剑离手、弓步前压。
        40: dict(
            easing="INQUAD",
            body=dict(x=-0.01, y=-0.015, z=+0.18),
            head=dict(pitch=-2, yaw=+2),
            torso=dict(pitch=+10, yaw=+14),
            rightArm=dict(pitch=-95, yaw=-4, roll=+8, bend=4, axis=180),
            leftArm=dict(pitch=-16, yaw=+26, roll=-10, bend=34, axis=180),
            leftLeg=dict(pitch=-22, bend=22, z=-0.10),
            rightLeg=dict(pitch=+17, bend=20, z=+0.06),
        ),
        # 收势中段：落臂直身、目送化形剑。
        43: dict(
            easing="INOUTSINE",
            body=dict(x=0.0, y=-0.005, z=+0.08),
            head=dict(pitch=-3, yaw=+1),
            torso=dict(pitch=+4, yaw=+6),
            rightArm=dict(pitch=-48, yaw=-6, roll=+4, bend=12, axis=180),
            leftArm=dict(pitch=-10, yaw=+14, roll=-6, bend=18, axis=180),
            leftLeg=dict(pitch=-12, bend=12, z=-0.06),
            rightLeg=dict(pitch=+10, bend=10, z=+0.04),
        ),
        # 归中立。
        46: dict(
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
)


def main() -> int:
    emit_json(
        POSE,
        name="sword_manifest_cast",
        description=(
            "P2 剑意化形重制（46t 非循环，决策 (b) 保守精修）：anticipation 0→28 "
            "双掌竖轴拉开凝形（右掌 -60→-102 / 左掌 -60→-24，3t 微颤张力），strike "
            "28→40 翻腕虚握→左掌让位→前送化形（pitch -95 前指 / torso.yaw +14 / "
            "body.z +0.18），recovery 40→46 目送回中立。"
        ),
        end_tick=46,
        stop_tick=48,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
