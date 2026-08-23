#!/usr/bin/env python3
"""拟态灰烬蛛 —— 动画物理自检：恐惧参数不是形容词，是断言。

  · 步态（walk/run/retreat）：支撑相脚贴地 + 等速后移（滑步残差上限）
  · ambush_burst：时长恰 5 tick；首帧 = 折叠姿（方块切换交界帧，超一丝穿帮）；
    末帧 = 站姿；存在过冲（某帧膝峰高过站姿）
  · fold：末帧 = 折叠姿 且 整姿收进 16³（下一帧切方块渲染）
  · bite：突刺角速度 ≥ 蓄力段 2.5 倍（"突刺快过蓄力三倍"的实测门）；螯肢开合幅度到位
  · idle：腿零位移（盯着不动才吓人）；触肢确有微颤
  · death：终帧不穿地、终段静止
  · 循环类：首末帧一致（接缝为零）
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import gen_anim as G  # noqa: E402
from preview import check_fold  # noqa: E402
from spider_rig import LEG_KEYS, Pose, SpiderRig, contact_report, fold_pose  # noqa: E402

TICK = 1.0 / 20.0


def pose_diff(a: Pose, b: Pose, bones: list[str]) -> float:
    """两姿态在给定骨骼上的最大通道差（度/单位）。"""
    worst = 0.0
    for n in bones:
        ca = a[n] if n in a else None
        cb = b[n] if n in b else None
        ra = ca.rot if ca else [0.0] * 3
        rb = cb.rot if cb else [0.0] * 3
        pa = ca.pos if ca else [0.0] * 3
        pb = cb.pos if cb else [0.0] * 3
        worst = max(worst, *(abs(x - y) for x, y in zip(ra, rb)),
                    *(abs(x - y) for x, y in zip(pa, pb)))
    return worst


def main() -> int:
    rig = SpiderRig()
    fold = fold_pose()
    all_bones = list(rig.order)
    problems: list[str] = []

    # ---- 步态：贴地 + 滑步
    for name, resid_max in (("walk", 0.35), ("run", 0.45), ("retreat", 0.45)):
        length, _loop, _n, _fn = G.ANIMS[name]
        spec = {"walk": (6.0, 0.58, 0.045), "run": (8.0, 0.44, 0.06),
                "retreat": (7.0, 0.40, 0.07)}[name]

        def stance_of(pair, side, t, spec=spec):
            u = G.wrap(t + G.leg_group(pair, side) + G.leg_noise(pair, side, spec[2]))
            return u / spec[1] if u < spec[1] else None

        rep = contact_report(rig, lambda t, nm=name: G.sample(rig, nm, t), stance_of, length)
        print(f"[{name}]\n{rep}")
        for line in rep.splitlines():
            r = float(line.rsplit("滑步残差", 1)[1]) if "滑步残差" in line else 0.0
            if r > resid_max:
                problems.append(f"{name}: {line.strip()}（残差上限 {resid_max}）")
            ys = line.split("触地 y", 1)[1].split("后移", 1)[0] if "触地 y" in line else ""
            if ys:
                lo = float(ys.split("..")[0])
                if lo < -0.35:
                    problems.append(f"{name}: 支撑相脚穿地 {lo:+.2f}")

    # ---- 循环接缝
    for name, (length, loop, n, _fn) in G.ANIMS.items():
        if not loop:
            continue
        d = pose_diff(G.sample(rig, name, 0.0), G.sample(rig, name, 0.9999), all_bones)
        if d > 1.5:
            problems.append(f"{name}: 循环接缝跳变 {d:.2f}")

    # ---- ambush_burst
    length, _, _, _ = G.ANIMS["ambush_burst"]
    if abs(length - 5 * TICK) > 1e-6:
        problems.append(f"burst 时长 {length}s ≠ 5 tick")
    d0 = pose_diff(G.sample(rig, "ambush_burst", 0.0), fold, G._LEG_BONES)
    if d0 > 2.0:
        problems.append(f"burst 首帧偏离折叠姿 {d0:.2f}（方块切换交界帧）")
    d1 = pose_diff(G.sample(rig, "ambush_burst", 1.0), Pose(), G._LEG_BONES)
    if d1 > 3.0:
        problems.append(f"burst 末帧偏离站姿 {d1:.2f}")
    stance_knee = max(rig.joint(f"tibia{p}_{s}", rig.world(Pose()))[1] for p, s in LEG_KEYS)
    over = max(max(rig.joint(f"tibia{p}_{s}", rig.world(G.sample(rig, "ambush_burst", tt)))[1]
                   for p, s in LEG_KEYS) for tt in (0.5, 0.55, 0.6, 0.65))
    if over < stance_knee + 0.5:
        problems.append(f"burst 无过冲：峰值膝高 {over:.2f} ≤ 站姿 {stance_knee:.2f}+0.5")
    print(f"[burst] 首帧Δ {d0:.2f} · 末帧Δ {d1:.2f} · 膝峰 {over:.2f} vs 站姿 {stance_knee:.2f}")

    # ---- fold
    df = pose_diff(G.sample(rig, "fold", 1.0), fold, G._LEG_BONES + G._BODY_BONES)
    if df > 0.8:
        problems.append(f"fold 末帧偏离折叠姿 {df:.2f}（下一帧切方块渲染）")
    print(f"[fold] 末帧Δ {df:.3f} · 末帧包围盒：")
    if check_fold(rig, G.sample(rig, "fold", 1.0), reserve=0.0):
        problems.append("fold 末帧溢出 16³")

    # ---- bite：突刺快过蓄力
    def pitch_rate(t0, t1, n=8):
        vals = [G.sample(rig, "bite", t0 + (t1 - t0) * i / n)["prosoma"].rot[0] for i in range(n + 1)]
        dur = (t1 - t0) * G.ANIMS["bite"][0]
        return max(abs(b - a) for a, b in zip(vals, vals[1:])) / (dur / n)
    windup, strike = pitch_rate(0.05, 0.40), pitch_rate(0.42, 0.52)
    print(f"[bite] 蓄力角速度 {windup:.0f}°/s · 突刺 {strike:.0f}°/s（×{strike / max(windup, 1e-6):.1f}）")
    if strike < windup * 2.5:
        problems.append(f"bite 突刺不够快：{strike:.0f}°/s < 蓄力 {windup:.0f}×2.5")
    spread = max(abs(G.sample(rig, "bite", tt)["chelicera_r"].rot[2]) for tt in (0.35, 0.42))
    if spread < 30.0:
        problems.append(f"bite 螯肢开合不足 {spread:.0f}° < 30°")

    # ---- idle：腿死静 + 触肢在颤
    leg_move = max(pose_diff(G.sample(rig, "idle", tt), G.sample(rig, "idle", 0.0),
                             [f"femur{p}_{s}" for p, s in LEG_KEYS]) for tt in (0.25, 0.5, 0.75))
    palp_amp = max(abs(G.sample(rig, "idle", tt)["palp2_l"].rot[0]) for tt in np.linspace(0, 1, 40))
    print(f"[idle] 腿位移 {leg_move:.2f}°(呼吸吸收) · 触肢振幅 {palp_amp:.1f}°")
    if leg_move > 3.0:
        problems.append(f"idle 腿动了 {leg_move:.2f}°——伏击者不该动腿")
    if palp_amp < 2.0:
        problems.append(f"idle 触肢微颤不足 {palp_amp:.1f}°")

    # ---- death：终帧不穿地 + 终段静止
    end = G.sample(rig, "death", 1.0)
    low = rig.lowest(end)
    still = pose_diff(end, G.sample(rig, "death", 0.96), all_bones)
    print(f"[death] 终帧最低点 {low:+.2f} · 终段位移 {still:.2f}")
    if low < -0.4:
        problems.append(f"death 终帧穿地 {low:+.2f}")
    if still > 1.2:
        problems.append(f"death 终段仍在动 {still:.2f}")

    if problems:
        print(f"\n✗ {len(problems)} 处违例：")
        for x in problems:
            print(f"   {x}")
        return 1
    print("\n✓ 步态/接缝/burst/fold/bite/idle/death 全部通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
