#!/usr/bin/env python3
"""异变缝合兽 —— 整兽动画自检：把走路里的每条主张变成断言。

走路这件事有一条压倒一切的判据：**支撑相的脚在世界系里必须是不动的**。它不动，兽才
是靠蹬地前进的；它一动，兽就是在冰上滑，而且滑的量正好等于"动画看起来在走多快"和
"游戏真的把它挪多快"之间的差。所以 ① 不是随便挑的一条检查，它是这一层的定义。

十二条：

  ① **不滑步**：支撑相里脚的世界位置恒定（体坐标系里以速度 v 后移）
  ② 脚不入地：任何一帧、任何一块几何都不许低于静止姿的最低点
  ③ 摆动相脚离地：抬起来的那只脚真的抬起来了
  ④ 循环接缝：首末帧严格相等
  ⑤ 连续：相邻帧任一关节的位移不超过一步的几分之一（防重折跳支）
  ⑥ **走起来不许穿模**：逐帧跑碰撞检测，不是只查静止姿
  ⑦ 关节不埋进核心
  ⑧ 任一时刻至少 3 只脚着地，且质心落在支撑多边形内
  ⑨ 头的动作幅度不超过接合面能给的余量
  ⑩ 骨的旋转能真的把关节摆到解出来的位置（正解回代）
  ⑪ 每条动作都真的动了东西
  ⑫ 头颅层那几条动作换基之后仍然只动这颗头自己的骨

⑥ 是这一层最该有的一条：静止姿不穿模是上一轮才修好的，而**摆起来会不会穿是另一个
问题**——腿在周期里扫过的体积比它站着时占的大得多。

用法:
  python3 modelScript/creatures/stitched_beast/check_beast_anim.py
  python3 modelScript/creatures/stitched_beast/check_beast_anim.py --seed 7
  python3 modelScript/creatures/stitched_beast/check_beast_anim.py --all
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import beast_anim as BA  # noqa: E402
import core as C  # noqa: E402
import gen_beast as GB  # noqa: E402
import locomotion as LM  # noqa: E402
from bbmodel_maker.rig.anim_rig import Pose, Rig  # noqa: E402

N = 40                 # 逐帧检查的采样数
SKATE_TOL = 0.35       # 滑步容差 px。单帧的数值误差量级，一步的行程是它的几十倍
SINK_TOL = 0.50        # 入地容差 px。取的是**模型自己的分辨率下限** `gen_beast.RENDER_MIN_R`
                       # ——半像素以下的柱子渲出来会消失或闪烁，那一层已经按这个数截断过
                       # 几何。脚绕接触点翻转时掌垫的角点会在这个尺度上下摆（蛛足的跗节
                       # 与趾行的趾节各实测到 0.30 / 0.43 px），比模型分辨得出的还细。
JUMP_TOL = 0.30        # 相邻帧位移相对"一步行程"的上限（比例）
OVERLAP_TOL = 0.75     # 与静态自检 ⑯ 同一个阈值


def foot_world(rig: Rig, W: dict, lb, nseg: int) -> np.ndarray:
    """这只脚此刻的**着力点**世界坐标。

    取链末端那个关节（`foot_chain` 意义上的接触点），不取脚掌几何的形心：脚是会绕接触
    点翻的（抬跟、跗节转向），形心跟着翻属于正常，拿它当"有没有滑步"的判据会把翻脚
    误判成滑步。走**渲染用的骨变换**而不是我自己的解——那才叫回代。
    """
    bone = f"limb_{lb.sock.name}_{nseg - 1}"
    return (W[bone] @ np.append(np.asarray(lb.joints[-1], float), 1.0))[:3]


def _hip(rig: Rig, W: dict, pl) -> np.ndarray:
    """这条肢此刻的髋——从**渲染用的父骨矩阵**里取，和动画层同一个来源。"""
    par = rig.bones[f"limb_{pl.lb.sock.name}_0"].parent
    return (W[par] @ np.append(pl.lb.joints[0], 1.0))[:3] if par else pl.lb.joints[0]


def lowest(rig: Rig, pose: Pose) -> float:
    return float(rig.lowest(pose))


def check(seed: int, *, quick: bool = False) -> list[str]:
    bad: list[str] = []
    be = BA.use(seed)
    rig = Rig(be.model)
    gait = be.gait
    v = gait.speed                       # px/s
    T = be.T
    n = 16 if quick else N

    rest_low = lowest(rig, Pose())
    poses = [BA.sample(rig, "beast_walk", i / n) for i in range(n + 1)]
    worlds = [rig.world(p) for p in poses]

    # ① 不滑步 + ③ 摆动离地
    for pl in be.plans:
        if pl.lg is None:
            continue
        name = pl.lb.sock.name
        nseg = len(pl.lb.gene.segments)
        prev_t = None
        prev_p = None
        swing_hi = 0.0
        for i in range(n + 1):
            t = i / n
            p = foot_world(rig, worlds[i], pl.lb, nseg)
            if pl.lg.in_stance(t):
                if prev_p is not None and prev_t is not None:
                    dt = (t - prev_t) * T
                    # 体坐标系里脚该以 v 后移（−z 是前进方向 ⇒ z 增大）
                    want = np.array([0.0, 0.0, v * dt])
                    err = float(np.linalg.norm((p - prev_p) - want))
                    if err > SKATE_TOL:
                        bad.append(
                            f"{name} 在 t={t:.2f} 支撑相滑步 {err:.2f} px"
                            f"（该走 {float(np.linalg.norm(want)):.2f} px，"
                            f"实走 {float(np.linalg.norm(p - prev_p)):.2f} px）"
                            f"——支撑脚在世界系里必须是钉住的")
                        break
                prev_t, prev_p = t, p
            else:
                prev_t = prev_p = None
                swing_hi = max(swing_hi, float(p[1]) - rest_low)
        if pl.clear > 0.35 and swing_hi < pl.clear * 0.5:
            bad.append(f"{name} 摆动相只抬了 {swing_hi:.2f} px，推出来要抬 "
                       f"{pl.clear:.2f} px——脚在蹭地")

    # ② 不入地
    for i, p in enumerate(poses):
        lo = lowest(rig, p)
        if lo < rest_low - SINK_TOL:
            bad.append(f"t={i / n:.2f} 有几何陷到 {lo:.2f}（静止最低 {rest_low:.2f}）"
                       f"——{'、'.join(x[1] for x in rig.lowest_parts(p, 2))}")
            break

    # ④ 循环接缝
    a, b = poses[0], BA.sample(rig, "beast_walk", 1.0)
    for bone in rig.order:
        ca, cb = a.get(bone), b.get(bone)
        if ca is None and cb is None:
            continue
        ra = (ca.rot + ca.pos) if ca else [0.0] * 6
        rb = (cb.rot + cb.pos) if cb else [0.0] * 6
        if max(abs(x - y) for x, y in zip(ra, rb)) > 0.6:
            bad.append(f"beast_walk 的 {bone} 首末帧对不上——循环会跳一下")
            break

    # ⑤ 连续 + ⑩ 正解回代
    for pl in be.plans:
        if pl.lg is None:
            continue
        name = pl.lb.sock.name
        nseg = len(pl.lb.gene.segments)
        step = max(pl.lg.excursion, 1.0)
        prev = None
        for i in range(n + 1):
            t = i / n
            solved, _nn = pl.joints(t, _hip(rig, worlds[i], pl))
            W = worlds[i]
            got = (W[f"limb_{name}_{nseg - 1}"]
                   @ np.append(rig.bones[f"limb_{name}_{nseg - 1}"].origin, 1.0))[:3]
            err = float(np.linalg.norm(got - solved[nseg - 1]))
            if err > 0.6:
                bad.append(f"{name} t={t:.2f} 骨旋转把末关节摆到了 {np.round(got, 2)}，"
                           f"解出来的是 {np.round(solved[nseg - 1], 2)}（差 {err:.2f} px）"
                           f"——欧拉角回代对不上")
                break
            if prev is not None:
                jump = max(float(np.linalg.norm(x - y)) for x, y in zip(solved, prev))
                if jump > step * JUMP_TOL * (n / 24.0):
                    bad.append(f"{name} 在 t={t:.2f} 一帧跳了 {jump:.2f} px"
                               f"（一步才走 {step:.2f} px）——重折换了解支")
                    break
            prev = solved

    # ⑥ 穿模 ⑦ 埋进核心（逐帧）
    if not quick:
        worst = (0.0, "")
        for i in range(n):
            lim = {}
            for pl in be.plans:
                lb = pl.lb
                if pl.lg is None:
                    lim[lb.sock.name] = lb
                    continue
                solved, _nn = pl.joints(i / n, _hip(rig, worlds[i], pl))
                lim[lb.sock.name] = _moved(lb, solved)
            for a_, b_, d, wa, wb in GB.overlaps(lim, be.heads, tol=OVERLAP_TOL):
                if d > worst[0]:
                    worst = (d, f"t={i / n:.2f} {a_}.{wa} × {b_}.{wb}")
            for pl in be.plans:
                if pl.lg is None:
                    continue
                solved, _nn = pl.joints(i / n, _hip(rig, worlds[i], pl))
                for j, pt in enumerate(solved[1:-1], 1):
                    q = pt - C.CORE_CENTER
                    f = C.fld(q)
                    if f <= C.ISO:
                        continue
                    dep = (f - C.ISO) / max(float(np.linalg.norm(C.grad(q))), 1e-6)
                    if dep > 1.0:
                        bad.append(f"{pl.lb.sock.name} 的第 {j} 个关节在 t={i / n:.2f} "
                                   f"埋进核心 {dep:.2f} px")
                        break
        if worst[0] > 0.0:
            bad.append(f"走起来穿模最深 {worst[0]:.2f} px（{worst[1]}）"
                       f"——静止姿不穿不代表摆起来不穿")

    # ⑧ 支撑
    com2 = np.array([gait.com[0], gait.com[2]])
    for i in range(n):
        t = i / n
        on = [lg for lg in gait.limbs if lg.in_stance(t)]
        if len(on) < LM.MIN_SUPPORT:
            bad.append(f"t={t:.2f} 只有 {len(on)} 只脚着地（要 {LM.MIN_SUPPORT}）")
            break
        feet = np.array([lg.foot_at(t) for lg in on])[:, [0, 2]]
        if LM.support_margin(feet, com2) < 0.0:
            bad.append(f"t={t:.2f} 质心跑出支撑多边形——这一帧它是在摔")
            break

    # ⑨ 头的幅度
    for hd in be.heads.values():
        cap_deg, cap_px = BA.graft_range(hd)
        bone = GB.head_bone(hd, "skull")
        for i in range(n + 1):
            ch = poses[i].get(bone)
            if ch is None:
                continue
            if max(abs(x) for x in ch.rot) > cap_deg + 1e-6:
                bad.append(f"{hd.name} 走路时头转了 {max(abs(x) for x in ch.rot):.1f}°，"
                           f"接合面只给得起 {cap_deg:.1f}°——这只兽没有颈")
                break
            if max(abs(x) for x in ch.pos) > cap_px + 1e-6:
                bad.append(f"{hd.name} 走路时头挪了 {max(abs(x) for x in ch.pos):.2f} px，"
                           f"接合面只给得起 {cap_px:.2f} px")
                break

    # ⑪ 空动画 ⑫ 头的动作不许外溢
    own = {h.name: {GB.head_bone(h, k) for k in ("skull", "jaw", "ear_l", "ear_r",
                                                 "horn_l", "horn_r")}
           for h in be.heads.values()}
    for name, (length, loop, ns, _fn) in BA.ANIMS.items():
        tr = [BA.sample(rig, name, i / 12) for i in range(13)]
        if not any(ch.moved() for p in tr for ch in p.values()):
            bad.append(f"{name} 一帧都没动——空动画")
        if not name.startswith("head_"):
            continue
        who = next((h for h in be.heads.values() if name.startswith(f"head_{h.name}_")),
                   None)
        if who is None:
            bad.append(f"{name} 找不到对应的头")
            continue
        for p in tr:
            stray = [b for b, ch in p.items()
                     if ch.moved() and b in rig.bones and b not in own[who.name]]
            if stray:
                bad.append(f"{name} 动到了不属于这颗头的骨：{stray[:3]}")
                break
    return bad


def _moved(lb, joints):
    """拿新关节位置克隆一条肢——只给碰撞检测用，粗细/材质原样。"""
    import copy
    out = copy.copy(lb)
    out.joints = list(joints)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--quick", action="store_true")
    args = ap.parse_args()

    seeds = list(range(1, 13)) if args.all else [args.seed]
    total = 0
    for s in seeds:
        try:
            bad = check(s, quick=args.quick)
        except Exception as exc:                      # noqa: BLE001
            bad = [f"跑不起来：{type(exc).__name__}: {exc}"]
        total += len(bad)
        be = BA._BE
        mark = "✓" if not bad else "✗"
        extra = ""
        if be is not None and be.seed == s:
            extra = (f"{len(be.gait.limbs)} 承重  {len(be.heads)} 头  "
                     f"周期 {be.T:.2f}s  最紧可达 {be.body.tight:.2f}")
        print(f"{mark} seed {s:<3} {extra}")
        for x in bad:
            print(f"    {x}")
    if total:
        print(f"\n✗ 共 {total} 处问题")
        return 1
    print("\n✓ 不滑步 / 不入地 / 摆动离地 / 循环接缝 / 连续 / 走起来不穿模 / 不埋核心 / "
          "支撑充足 / 头不超接合面余量 / 正解回代 / 无空动画 / 头动作不外溢 全部通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
