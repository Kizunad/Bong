#!/usr/bin/env python3
"""异变缝合兽 —— 运动层物理自检：把步态参数变成会撞红的断言。

"看起来在走"不是验收标准。这里把每条物理约束写成断言：

  · 摆频公式对——与均匀单杆解析解 f=(1/2π)√(3g/2L) 对拍，并验证 f ∝ 1/√L 单调
  · **支撑相不滑步**——世界系里踩住的脚必须静止。体坐标位移 = stride×duty，
    差一个 duty 因子就是每步都在蹭地（round 1 实测就写错成 stride）
  · 不过伸——每步跨度不得超过该肢的可达直径×SWING
  · 承重肢真的够得着地——骑乘高度解完后 髋高 < 有效肢长
  · 全周期任意时刻 ≥3 只脚着地，且质心投影在支撑多边形内有正余量
  · 循环闭合——t=0 与 t→1 的姿态一致（步态是循环动画）
  · 同 seed 必得同一只兽（可复现，出了问题能重放）
  · 错拍确实发生——12 只里必须有明显多数走出非 1:1 的步数比

用法: python3 scripts/models/stitched_beast/check_gait.py [--count 12]
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import genome as GN  # noqa: E402
import locomotion as L  # noqa: E402


def check_pendulum() -> list[str]:
    """摆频公式与解析解对拍。"""
    bad: list[str] = []
    for length in (10.0, 24.0, 48.0):
        want = math.sqrt(3.0 * L.G_PX / (2.0 * length)) / (2.0 * math.pi)
        # 单节 = 均匀杆，应精确命中解析解
        got = L.natural_hz((length,))
        if abs(got - want) > 1e-9:
            bad.append(f"均匀杆 L={length}：摆频 {got:.6f} ≠ 解析解 {want:.6f}")
        # 切成 8 节的同长杆，质量分布不变，频率必须一样
        seg = L.natural_hz(tuple([length / 8] * 8))
        if abs(seg - want) > 1e-6:
            bad.append(f"L={length} 切 8 节后摆频 {seg:.6f} ≠ 整杆 {want:.6f}"
                       f"——分节不该改变质量分布")
    # f ∝ 1/√L：长肢必须更慢
    hz = [L.natural_hz((x,)) for x in (8.0, 16.0, 32.0, 64.0)]
    if not all(a > b for a, b in zip(hz, hz[1:])):
        bad.append(f"摆频未随长度单调下降：{[round(x, 3) for x in hz]}")
    if abs(hz[0] / hz[2] - 2.0) > 1e-6:
        bad.append(f"长度 4 倍应给出频率 2 倍，实得 {hz[0] / hz[2]:.4f}")
    return bad


def check_gait(g: GN.Genome, gt: L.Gait, socks) -> list[str]:
    bad: list[str] = []
    tag = f"seed={g.seed}"

    for lg in gt.limbs:
        # ① 不过伸
        if lg.stride > lg.max_stride + 1e-6:
            bad.append(f"{tag} {lg.gene.socket} 步幅 {lg.stride:.2f} > 可达 "
                       f"{lg.max_stride:.2f}——腿伸不了那么远")
        # ② 步数合法
        if not (1 <= lg.steps <= L.MAX_STEPS):
            bad.append(f"{tag} {lg.gene.socket} 步数 {lg.steps} 越界 [1,{L.MAX_STEPS}]")
        # ③ 够得着地（骑乘高度解完之后）
        eff = lg.gene.leg_len * L.EXTEND
        if lg.hip[1] - lg.gene.ankle_lift >= eff:
            bad.append(f"{tag} {lg.gene.socket} 髋到踝 "
                       f"{lg.hip[1] - lg.gene.ankle_lift:.1f} ≥ 有效腿长 {eff:.1f}，"
                       f"根本踩不到地却在承重集里")
        # ④ **支撑相不滑步**：世界系里脚必须静止。
        #    世界位置 = 体坐标位置 + 身体位移（身体朝 -z 前进）。
        T = 1.0 / gt.body_hz
        prev = None
        worst = 0.0
        for k in range(240):
            t = k / 240.0
            if not lg.in_stance(t):
                prev = None
                continue
            world_z = lg.foot_at(t)[2] - gt.speed * t * T
            if prev is not None:
                worst = max(worst, abs(world_z - prev))
            prev = world_z
        if worst > 0.05:
            bad.append(f"{tag} {lg.gene.socket} 支撑相脚在世界系移动了 {worst:.3f} px/帧"
                       f"——在蹭地（应为 0）")

    # ⑤ 支撑数与稳定余量
    margin, fewest = L.evaluate(list(gt.limbs), np.array([gt.com[0], gt.com[2]]))
    if fewest < L.MIN_SUPPORT:
        bad.append(f"{tag} 有时刻只有 {fewest} 只脚着地（< {L.MIN_SUPPORT}）")
    if margin <= 0.0:
        bad.append(f"{tag} 稳定余量 {margin:+.2f} ≤ 0——质心跑出支撑多边形，会摔")
    if abs(margin - gt.margin) > 1e-6:
        bad.append(f"{tag} 报告余量 {gt.margin:.4f} 与复算 {margin:.4f} 不符")

    # ⑥ 循环闭合：步态是循环动画。要断言的是**姿态连续** + **状态周期**。
    #
    #    不能拿 in_stance(0) 与 in_stance(1-ε) 比：相位恰为 0 的肢在 t=0 正好进入
    #    支撑相、t=1-ε 处于摆动相末尾，布尔值在边界上翻转是**正确**的，不是接缝跳变
    #    （实测该误报只打在 phase=0.000 的肢上，而其脚位移仅 0.0007 px）。
    for lg in gt.limbs:
        d = float(np.linalg.norm(lg.foot_at(0.0) - lg.foot_at(0.99999)))
        if d > 0.05:
            bad.append(f"{tag} {lg.gene.socket} 循环接缝跳变 {d:.3f} px")
        for q in (0.0, 0.137, 0.5, 0.813):
            if lg.in_stance(q) != lg.in_stance(q + 1.0):
                bad.append(f"{tag} {lg.gene.socket} 着地状态在 t={q} 处非周期")
            if float(np.linalg.norm(lg.foot_at(q) - lg.foot_at(q + 1.0))) > 1e-6:
                bad.append(f"{tag} {lg.gene.socket} 脚位置在 t={q} 处非周期")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--count", type=int, default=12)
    args = ap.parse_args()

    socks = C.sockets()
    bad = check_pendulum()
    print(f"[摆频] 解析解对拍 {'✓' if not bad else '✗'}")

    poly = 0
    rows = 0
    for s in range(1, args.count + 1):
        try:
            g, gt = L.sample_standing(s, socks=socks)
        except ValueError as e:
            bad.append(f"seed={s} 采不到能站的兽：{e}")
            continue
        rows += 1
        bad += check_gait(g, gt, socks)
        ratios = sorted({lg.steps for lg in gt.limbs})
        poly += len(ratios) > 1
        print(f"[seed {s:>2}] 承重{len(gt.limbs)} 骑乘{gt.ride:+6.1f} "
              f"{gt.body_hz:.2f}Hz 走{gt.blocks_per_sec:.2f}/跑{gt.run_blocks_per_sec:.2f}格/s "
              f"余量{gt.margin:+5.2f} 步数比 {':'.join(map(str, ratios))}")

        # 确定性：同 seed 重来一次必须逐字段一致
        g2, gt2 = L.sample_standing(s, socks=socks)
        if [x.gene for x in gt.limbs] != [x.gene for x in gt2.limbs] or \
                abs(gt.speed - gt2.speed) > 1e-9 or abs(gt.ride - gt2.ride) > 1e-9:
            bad.append(f"seed={s} 两次采样结果不一致——步态必须可复现")

    # ⑦ 错拍是招牌：绝大多数个体应当走出非 1:1 的步数比
    if rows and poly < rows * 0.6:
        bad.append(f"只有 {poly}/{rows} 只走错拍步态（<60%）——肢体长度跨度不够，"
                   f"检查 genome.LIMB_SOURCES 的体型跨度")
    print(f"\n{poly}/{rows} 只走错拍步态")

    if bad:
        print(f"\n✗ {len(bad)} 处违例：")
        for x in bad:
            print(f"   {x}")
        return 1
    print("✓ 摆频 / 不滑步 / 不过伸 / 触地 / 支撑 / 稳定 / 循环 / 可复现 / 错拍 全部通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
