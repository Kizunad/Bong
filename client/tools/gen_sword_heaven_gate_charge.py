#!/usr/bin/env python3
"""sword_heaven_gate_charge —— 天门开阖·充能段：举剑过顶渐蓄 + 顶点冻结保持。

heaven_gate 是四阶段引导：cast(elapsed=0) 起播本 charge 动画，phase_system 在
elapsed≥`HEAVEN_GATE_AOE_END=140` 才 emit `sword_heaven_gate_release`（劈下）。
release 与 charge 同 `SWORD_PATH_PRIORITY` 同通道，release 播出即顶替 charge。

**修复（用户实机发现）**：旧版 endTick=60 且 isLoop:false，赌"60t 动画正好卡在
release 交接帧"。但 release 其实在 140t 才来——charge 播完 60t 后经 stopTick 淡出
**回默认站姿**，然后 64→140 整整 ~76t（≈3.8s）停在默认，直到 140t 才劈下：玩家
看到"举臂→掉回默认→再从顶点劈下"两段割裂（设计者 P6 注记的正典统一欠账即此）。
修法：举剑到顶点后**静态冻结定住**（复制 t60 拉满定格帧到远端 endTick），把顶点
姿势撑过 release（140t）。isLoop:false 自限——即便 release 意外未至（丢包/异常），
到 endTick 也只是自然收势，不会像循环无 StopAnim 那样无限挂住。天门 cast 无提前
打断路径（`HeavenGateChanneling` 仅在 elapsed≥140 完成阶段移除），故无需 server StopAnim。

提举段（0→60）为密度精修（4t 步进 16 帧，三阶段渐进 + 蓄满微颤极值直落采样帧）；
60→endTick 为冻结保持（单一定格帧，无运动，无需 ≤4t 帧距）。

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


# 顶点冻结保持段结束帧：撑过 release（elapsed≥140），留足服务端→客户端 RTT 余量。
HOLD_END_TICK = 200

# 提举段 0→60（4t 步进）+ 顶点冻结保持（复制 t60 拉满定格帧到 HOLD_END_TICK；
# 两帧同姿 → 60→HOLD_END 恒定插值 = 静态定住）。charge_frame(t>56) 恒返回定格帧，
# 故 charge_frame(HOLD_END_TICK) 与 charge_frame(60) 逐字段相等。
POSE = {t: charge_frame(t) for t in range(0, 61, 4)}
POSE[HOLD_END_TICK] = charge_frame(HOLD_END_TICK)


def main() -> int:
    emit_json(
        POSE,
        name="sword_heaven_gate_charge",
        description=(
            "天门充能段：0→16 低位提举过顶，16→40 渐升 + roll 交替脉动渐涨，"
            "40→56 高位蓄满微颤（极值直落采样帧）+ 重心渐沉，56→60 拉满定格"
            "（背弓头仰）；60→200 顶点冻结保持撑过 release（140t 顶替）——修用户"
            "实机所见的『举臂→掉回默认→再劈下』割裂（旧 endTick=60 播完淡回默认）。"
        ),
        end_tick=HOLD_END_TICK,
        stop_tick=HOLD_END_TICK + 4,
        is_loop=False,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
