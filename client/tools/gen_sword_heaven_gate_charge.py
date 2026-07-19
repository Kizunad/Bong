#!/usr/bin/env python3
"""sword_heaven_gate_charge —— 天门开阖·充能段：举剑过顶渐蓄（P2 批次二后半精修）。

heaven_gate 是两段式先例：charge 段（`HEAVEN_GATE_CHARGE_END=60` tick 固定
充能相位）+ release 段（`sword_heaven_gate_release`）。charge 动画 60t 与充能
窗**完全等长对齐**——非循环 hold-末帧范式在此语义下无死帧（充能满即入
release，末帧即交接帧）；与 isLoop 正典的统一问题归 P6 注记（附录 A）。

本版是密度精修（review 返工补 P2 欠账）：旧资产 60t 仅 4 关键帧（0/12/42/60，
最大帧距 30t），远低于精度标准 #3 的主轴 ≤4t 红线。重制为 4t 步进参数化生成
（16 帧），三阶段渐进 + 蓄满微颤极值直落采样帧（不经正弦零点）。

母题：开天门。双手低位合握剑柄（沉腰起手），缓缓提举过顶（0→16 提举、
16→40 渐升 + 剑身左右微脉动=真元灌注），高位蓄满后微颤渐强（40→56，颤幅
2→6 交替极值帧），56→60 拉满定格（背微弓、头仰望「门」）。与 manifest
（胸前凝形送出）/ 基础剑招（中位挥斩）动向完全区分。

时序（精度标准 #3 密度）：
  0→16   提举：低位合握 → 胸前 → 过顶半程（OUTSINE）
  16→40  渐升脉动：举至全高（pitch →-150），roll 交替脉动幅度渐涨（INOUTSINE）
  40→56  蓄满微颤：高位保持，颤幅 2→6 交替极值帧、重心渐沉（INOUTSINE）
  56→60  拉满定格：背弓头仰、剑指天门（OUTSINE，末帧=release 交接帧）
endTick=60，stopTick=64，非循环。主轴：rightArm.pitch / leftArm.pitch /
torso.pitch（4t 步进机械保证 ≤4t 帧距）。
"""

from __future__ import annotations

from anim_common import emit_json


def charge_frame(t: int) -> dict:
    """充能段某 tick 姿态（4t 步进采样；微颤极值直落采样帧）。"""
    # 交替符号：4t 步进下相邻帧 +1/-1 轮换，微颤/脉动极值永远落在采样帧上。
    alt = 1.0 if (t // 4) % 2 == 0 else -1.0
    if t <= 16:
        # 提举：低位 → 过顶半程。
        k = t / 16.0
        ease = "OUTSINE"
        arm_pitch = -40.0 - 62.0 * k  # -40 → -102
        arm_yaw = 14.0 - 6.0 * k
        arm_bend = 46.0 - 24.0 * k
        pulse = 0.0
        torso_pitch = +4.0 - 6.0 * k
        head_pitch = +4.0 - 12.0 * k
        body_y = -0.03 + 0.02 * k
        sink = 0.0
    elif t <= 40:
        # 渐升脉动：过顶半程 → 全高，roll 脉动渐涨。
        k = (t - 16.0) / 24.0
        ease = "INOUTSINE"
        arm_pitch = -102.0 - 48.0 * k  # -102 → -150
        arm_yaw = 8.0 - 4.0 * k
        arm_bend = 22.0 - 12.0 * k
        pulse = (2.0 + 3.0 * k) * alt
        torso_pitch = -2.0 - 4.0 * k
        head_pitch = -8.0 - 6.0 * k
        body_y = -0.01 + 0.01 * k
        sink = 0.0
    elif t <= 56:
        # 蓄满微颤：高位保持、颤幅渐涨、重心渐沉。
        k = (t - 40.0) / 16.0
        ease = "INOUTSINE"
        arm_pitch = -150.0 - 2.0 * k
        arm_yaw = 4.0
        arm_bend = 10.0
        pulse = (3.0 + 3.0 * k) * alt
        torso_pitch = -6.0 - 2.0 * k
        head_pitch = -14.0 - 3.0 * k
        body_y = 0.0 - 0.03 * k
        sink = k
    else:
        # 拉满定格（t=60）：背弓头仰、剑指天门。
        ease = "OUTSINE"
        arm_pitch = -154.0
        arm_yaw = 4.0
        arm_bend = 8.0
        pulse = 0.0
        torso_pitch = -9.0
        head_pitch = -18.0
        body_y = -0.04
        sink = 1.0
    return dict(
        easing=ease,
        body=dict(x=0.0, y=body_y, z=-0.02 - 0.02 * sink),
        head=dict(pitch=head_pitch, yaw=0),
        torso=dict(pitch=torso_pitch, yaw=pulse * 0.6),
        rightArm=dict(
            pitch=arm_pitch,
            yaw=-arm_yaw,
            roll=+6 + pulse,
            bend=arm_bend,
            axis=180,
        ),
        leftArm=dict(
            pitch=arm_pitch + 4.0,
            yaw=+arm_yaw,
            roll=-6 - pulse,
            bend=arm_bend + 4.0,
            axis=180,
        ),
        leftLeg=dict(pitch=-6 - 5 * sink, bend=8 + 7 * sink, z=-0.03 - 0.02 * sink),
        rightLeg=dict(pitch=+5 + 4 * sink, bend=7 + 6 * sink, z=+0.03 + 0.01 * sink),
    )


POSE = {t: charge_frame(t) for t in range(0, 61, 4)}


def main() -> int:
    emit_json(
        POSE,
        name="sword_heaven_gate_charge",
        description=(
            "P2 天门充能段精修（60t 非循环 = HEAVEN_GATE_CHARGE_END 充能窗全"
            "对齐，旧 4 关键帧重制为 4t 步进 16 帧）：0→16 低位提举过顶，16→40 "
            "渐升 + roll 交替脉动渐涨，40→56 高位蓄满微颤（极值直落采样帧）+ "
            "重心渐沉，56→60 拉满定格（背弓头仰，末帧=release 交接帧）。"
        ),
        end_tick=60,
        stop_tick=64,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
