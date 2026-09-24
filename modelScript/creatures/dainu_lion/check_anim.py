#!/usr/bin/env python3
"""怠怒之狮 —— 动画后验。四足动画的破绽渲染静帧一律看不出来，只能算。

查四件事：
  1. 滑步：支撑相里脚掌必须贴地、且随支撑进度**匀速**后移（拟合按支撑进度 u，不按
     时间 t——相位偏移会让支撑相跨过 t=0，按 t 拟合出来的残差是假的）。
  2. 穿地：逐帧全模型最低点。腾空动作除外，落地帧不该扎进地里。
  3. 接缝：循环动画首末帧姿态必须逐骨相等，否则每轮循环抖一下。
  4. 抽搐：相邻帧的逐骨角度跳变上限，抓 keyed() 关键点排布造成的折角。
"""

from __future__ import annotations

import numpy as np

import gen_anim as G
from rig import Rig


def gait_report(rig: Rig, name: str, cfg: dict) -> list[str]:
    length = G.ANIMS[name][0]
    gait = G.Gait(rig, **cfg)
    out = []
    for side, hind in G.LEGS:
        k = gait.key(side, hind)
        us, ys, zs, errs = [], [], [], []
        for i in range(160):
            t = i / 160
            u = G.wrap(t + gait.phases[k])
            if u >= gait.duty:
                continue
            pose = G.sample(rig, name, t)
            p = rig.foot_world(pose, side, hind)
            tgt, _ = gait.target(side, hind, t)
            us.append(u / gait.duty)
            ys.append(p[1])
            zs.append(p[2])
            errs.append(float(np.linalg.norm(p - tgt)))
        A = np.vstack([np.array(us), np.ones(len(us))]).T
        slope, icpt = np.linalg.lstsq(A, np.array(zs), rcond=None)[0]
        slide = float(np.abs(np.array(zs) - (slope * np.array(us) + icpt)).max())
        speed = slope / (gait.duty * length)
        flag = "" if (slide < 0.35 and max(errs) < 0.35) else "   ← 超标"
        out.append(f"    {k}: 贴地y {min(ys):+5.2f}..{max(ys):+5.2f} | 后移 {speed:5.1f} u/s"
                   f" | 滑步 {slide:5.3f} | IK残差 {max(errs):5.3f}{flag}")
    return out


def sweep(rig: Rig, name: str, n: int = 48):
    poses = [G.sample(rig, name, i / n) for i in range(n)]
    lows, jumps = [], []
    prev = None
    for pose in poses:
        W = rig.world(pose)
        lo = 1e9
        for b in rig.order:
            if not rig.bones[b].elements:
                continue
            pts = rig.bone_points(b)
            wp = (W[b][:3, :3] @ pts.T).T + W[b][:3, 3]
            lo = min(lo, float(wp[:, 1].min()))
        lows.append(lo)
        cur = {b: list(pose[b].rot) for b in rig.order if b in pose}
        if prev is not None:
            jumps.append(max((max(abs(cur.get(b, [0, 0, 0])[k] - prev.get(b, [0, 0, 0])[k]) for k in range(3)), b)
                             for b in set(cur) | set(prev)))
        prev = cur
    worst_jump = max(jumps) if jumps else (0.0, "-")
    return lows, worst_jump


def seam(rig: Rig, name: str) -> float:
    a, b = G.sample(rig, name, 0.0), G.sample(rig, name, 1.0)
    d = 0.0
    for bone in rig.order:
        for attr in ("rot", "pos", "scale"):
            va = getattr(a[bone], attr) if bone in a else None
            vb = getattr(b[bone], attr) if bone in b else None
            if va is None or vb is None:
                continue
            d = max(d, max(abs(x - y) for x, y in zip(va, vb)))
    return d


def main() -> int:
    rig = Rig()
    gaits = {"walk": G.WALK, "run": G.RUN}
    for name, (length, loop, _n, _fn) in G.ANIMS.items():
        lows, (jump, jb) = sweep(rig, name)
        deep = min(lows)
        air = max(lows)
        line = f"{name:<7} {length:4.2f}s {'循环' if loop else '单次'}"
        line += f" | 最低点 {deep:+5.2f} (最高帧 {air:+5.2f}) | 帧间最大跳变 {jump:5.1f}° [{jb}]"
        if loop:
            line += f" | 接缝 {seam(rig, name):.3f}"
        print(line)
        if name in gaits:
            for row in gait_report(rig, name, gaits[name]):
                print(row)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
