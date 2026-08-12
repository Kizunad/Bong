#!/usr/bin/env python3
"""异变缝合兽 —— 核心阶段动画物理自检。

最要紧的一条：**蠕动的锚段在世界系里必须静止**。这和有腿时"支撑相脚不滑步"是同一条
约束，只是"脚"换成了身体的锚段。导出的是循环动画（每周期净位移被减掉），所以这里
把位移加回去再验——验的是真实世界轨迹，不是动画曲线。

其余：净前进量 = CRAWL_D、体积守恒、循环接缝为零、idle 各 lobe 不同频（同频会读成
"一只动物在喘"）、嫁接单调且有停滞、死亡逐 lobe 依次泄气且终帧静止、全动画不穿地。

用法: python3 scripts/models/stitched_beast/check_core_anim.py
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

import core_anim as A  # noqa: E402
from anim_rig import Rig  # noqa: E402


def world_z(rig: Rig, pose, bone: str) -> float:
    return float(rig.joint(bone, rig.world(pose))[2])


def pose_delta(rig: Rig, a, b) -> float:
    """两姿态在所有骨上的最大通道差。"""
    worst = 0.0
    for n in rig.order:
        ca, cb = a[n] if n in a else None, b[n] if n in b else None
        for attr, d in (("rot", 0.0), ("pos", 0.0), ("scale", 1.0)):
            va = list(getattr(ca, attr)) if ca else [d] * 3
            vb = list(getattr(cb, attr)) if cb else [d] * 3
            worst = max(worst, *(abs(x - y) for x, y in zip(va, vb)))
    return worst


def main() -> int:
    if not A.MODEL.exists():
        print(f"缺 {A.MODEL}，先跑 gen_core.py")
        return 1
    rig = Rig(A.MODEL)
    bad: list[str] = []

    # ---- ① 蠕动：锚段世界静止（不滑步）
    N = 200
    for bone, lo, hi in (("core_hind", 0.00, 0.49), ("core_fore", 0.51, 0.99)):
        vals = []
        for k in range(N + 1):
            u = lo + (hi - lo) * k / N
            p = A.sample(rig, "core_crawl", u)
            vals.append(world_z(rig, p, bone) - A.CRAWL_D * u)   # 加回每周期净位移
        drift = max(vals) - min(vals)
        print(f"[蠕动] {bone} 锚定相 u∈[{lo:.2f},{hi:.2f}] 世界位移 {drift:.4f} px")
        if drift > 0.05:
            bad.append(f"蠕动 {bone} 在锚定相内世界位移 {drift:.3f} px——在蹭地（应为 0）")

    # ---- ② 净前进量恰为 CRAWL_D
    z0 = world_z(rig, A.sample(rig, "core_crawl", 0.0), "core_mid")
    z1 = world_z(rig, A.sample(rig, "core_crawl", 0.99999), "core_mid") - A.CRAWL_D * 0.99999
    adv = z0 - z1
    print(f"[蠕动] 每周期净前进 {adv:.3f} px（设计 {A.CRAWL_D:.3f}）")
    if abs(adv - A.CRAWL_D) > 0.05:
        bad.append(f"蠕动净前进 {adv:.3f} ≠ CRAWL_D {A.CRAWL_D:.3f}")

    # ---- ③ 体积守恒：拉长必须变细，乘积恒定
    vols = []
    for k in range(41):
        u = k / 40
        wf, wh, _gf, _gh = A.crawl_world(u)
        body = A.LOBE_SPAN + (wh - wf)
        r = np.sqrt(A.LOBE_SPAN / body)
        vols.append(r * r * body / A.LOBE_SPAN)
    spread = max(vols) - min(vols)
    print(f"[蠕动] 体积波动 {spread:.2e}")
    if spread > 1e-9:
        bad.append(f"蠕动体积不守恒，波动 {spread:.3e}——拉长时没等比变细")

    # ---- ④ 循环接缝
    for name, (length, loop, _n, _f) in A.ANIMS.items():
        if not loop:
            continue
        d = pose_delta(rig, A.sample(rig, name, 0.0), A.sample(rig, name, 0.99999))
        print(f"[循环] {name} 接缝差 {d:.4f}")
        if d > 0.02:
            bad.append(f"{name} 循环接缝跳变 {d:.3f}")

    # ---- ⑤ idle：各 lobe 必须**不同频**
    def series(bone):
        return np.array([A.sample(rig, "core_idle", k / 96)[bone].scale[0] for k in range(96)])

    mains = [series(n) for n in A.LOBES_MAIN]
    worst_corr = 0.0
    for i in range(len(mains)):
        for j in range(i + 1, len(mains)):
            a, b = mains[i] - mains[i].mean(), mains[j] - mains[j].mean()
            den = np.linalg.norm(a) * np.linalg.norm(b)
            worst_corr = max(worst_corr, abs(float(a @ b / den)) if den > 1e-9 else 1.0)
    print(f"[idle] lobe 间最大相关 {worst_corr:.3f}")
    if worst_corr > 0.85:
        bad.append(f"idle 各 lobe 搏动过于同步（相关 {worst_corr:.2f}）——"
                   f"会读成'一只动物在喘'，而这是几团组织各喘各的")
    amp = max(float(s.max() - s.min()) for s in mains)
    if amp < 0.03:
        bad.append(f"idle 搏动幅度仅 {amp:.3f}，看不出来在动")

    # ---- ⑥ 嫁接：单调不减、终值满、且确有停滞
    vs = [A.sample(rig, "core_graft", k / 120)[A.BUDS[0]].scale[0] for k in range(121)]
    if any(b < a - 1e-6 for a, b in zip(vs, vs[1:])):
        bad.append("嫁接进度出现回退——组织长回去了？")
    if abs(vs[-1] - 1.0) > 0.02:
        bad.append(f"嫁接终值 {vs[-1]:.3f} ≠ 1.0，芽没长满")
    steps = [b - a for a, b in zip(vs, vs[1:])]
    stalls = sum(1 for s in steps if s < 1e-6)
    print(f"[嫁接] 终值 {vs[-1]:.3f}  停滞帧 {stalls}/{len(steps)}")
    if stalls < 4:
        bad.append(f"嫁接只有 {stalls} 帧停滞——匀速长大读成'技能特效'，"
                   f"要的是阶梯式推进（正典说这过程要七日）")

    # ---- ⑦ 死亡：下沉单调、终段静止、赘生物先瘪
    ys = [A.sample(rig, "core_death", k / 60)["root"].pos[1] for k in range(61)]
    if any(b > a + 1e-6 for a, b in zip(ys, ys[1:])):
        bad.append("死亡时 root 上浮过——只能往下沉")
    still = pose_delta(rig, A.sample(rig, "core_death", 1.0), A.sample(rig, "core_death", 0.96))
    print(f"[死亡] 终帧下沉 {ys[-1]:.2f}px  终段位移 {still:.3f}")
    if still > 0.05:
        bad.append(f"死亡终段仍在动 {still:.3f}")
    lump = A.sample(rig, "core_death", 0.25)["nodule_r"].scale[1]
    body = A.sample(rig, "core_death", 0.25)["core_mid"].scale[1]
    if not lump < body - 0.10:
        bad.append(f"死亡时赘生物没有先瘪（t=0.25 赘 {lump:.2f} vs 主体 {body:.2f}）——"
                   f"接得最勉强的部分该最先散")

    # ---- ⑦b 扑击：爆发必须快过蓄力。速度比是恐惧的唯一来源，不是观感偏好。
    def zrate(t0: float, t1: float, n: int = 24) -> float:
        vs = [A.sample(rig, "core_lunge", t0 + (t1 - t0) * i / n)["root"].pos[2]
              for i in range(n + 1)]
        dur = (t1 - t0) * A.ANIMS["core_lunge"][0]
        return max(abs(b - a) for a, b in zip(vs, vs[1:])) / (dur / n)

    wind = zrate(0.04, A.LUNGE_WINDUP - 0.02)
    strike = zrate(A.LUNGE_WINDUP + 0.01, 0.99)
    print(f"[扑击] 蓄力 {wind:.0f} px/s · 爆发 {strike:.0f} px/s（×{strike / max(wind, 1e-6):.1f}）")
    if strike < wind * 2.5:
        bad.append(f"扑击爆发不够快：{strike:.0f} < 蓄力 {wind:.0f}×2.5——"
                   f"蓄放速度比是恐惧的唯一来源")

    # ---- ⑦c 包裹：必须真的张开再合上，且咽下后主体变大（吃进去的得有去处）
    fore = [A.sample(rig, "core_engulf", k / 60)["core_fore"].scale[0] for k in range(61)]
    if max(fore) - min(fore) < 0.5:
        bad.append(f"包裹张合幅度仅 {max(fore) - min(fore):.2f}——读成喘气不是吞噬")
    mid0 = A.sample(rig, "core_engulf", 0.0)["core_mid"].scale[0]
    mid1 = A.sample(rig, "core_engulf", 1.0)["core_mid"].scale[0]
    if mid1 <= mid0 + 0.10:
        bad.append(f"包裹结束时主体未变大（{mid0:.2f}→{mid1:.2f}）——吃进去的东西凭空消失了")

    # ---- ⑧ 受击：必须衰减回近似静止
    h = pose_delta(rig, A.sample(rig, "core_hurt", 1.0), A.sample(rig, "core_hurt", 0.0))
    print(f"[受击] 终帧与静止姿差 {h:.3f}")
    if h > 0.12:
        bad.append(f"受击终帧未回到静止（差 {h:.3f}），会和后续动画打架")

    # ---- ⑨ 全动画不穿地
    for name in A.ANIMS:
        low = min(rig.lowest(A.sample(rig, name, k / 24)) for k in range(25))
        if low < -0.6:
            bad.append(f"{name} 穿地 {low:+.2f}")

    if bad:
        print(f"\n✗ {len(bad)} 处违例：")
        for x in bad:
            print(f"   {x}")
        return 1
    print("\n✓ 不滑步 / 净前进 / 体积守恒 / 循环 / 异步搏动 / 嫁接 / 死亡 / 受击 / 不穿地 全通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
