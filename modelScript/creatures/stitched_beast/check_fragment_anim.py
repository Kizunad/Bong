#!/usr/bin/env python3
"""异变缝合兽 —— 碎片动画自检。

碎片用的是和母体**同一套约束**，所以这里量的也是同一批东西：锚段世界静止、净前进
等于自己的行程、体积守恒、循环接缝、乱抽的三条主张、逐槽嫁接。

多出来的两条只有碎片才需要：

  · **行进方向必须与 forward 同向**。骨骼缩放只能沿模型轴，所以蠕动的伸缩与位移都
    换算到主轴上，换算里有一个负号（crawl_world 返回的是核心系的 z 坐标，核心朝
    −z 走）。漏掉它，碎片会倒着爬——而不滑步、体积守恒、循环接缝**全部照样绿**，
    只有方向是反的。这条是唯一能抓住它的断言。
  · **必须爬得比母体慢**。逃窜速度正比于锚段间距，碎片的锚段挨得近，所以它跑不掉。
    这是碎片非去捡尸不可的理由，不能让它跑得跟母体一样快。

用法: python3 modelScript/creatures/stitched_beast/check_fragment_anim.py
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import core_anim as A  # noqa: E402
import fragment_anim as FA  # noqa: E402
from bbmodel_maker.rig.anim_rig import Rig  # noqa: E402
from check_core_anim import pose_delta  # noqa: E402


def main() -> int:
    if not FA.MODEL.exists():
        print(f"缺 {FA.MODEL}，先跑 gen_fragment.py")
        return 1
    rig = Rig(FA.MODEL)
    g = FA.GEOM
    bad: list[str] = []
    i, sgn = FA.axis()

    def along(pose, bone: str) -> float:
        """沿伸缩主轴的世界坐标（已折算成"朝 forward 为正"）。"""
        return float(rig.joint(bone, rig.world(pose))[i]) * sgn

    # ---- ① 锚段世界静止（不滑步）：和母体同一条约束
    N = 200
    for bone, lo, hi in ((g.hind, 0.00, 0.49), (g.fore, 0.51, 0.99)):
        vals = []
        for k in range(N + 1):
            u = lo + (hi - lo) * k / N
            # 加回每周期净位移（导出的是循环动画）。along 已折算成朝 forward 为正，
            # 所以这里是**加**不是减——母体那条量的是 z 坐标，前进时变负，符号相反。
            vals.append(along(FA.sample(rig, "shard_crawl", u), bone) + FA.CRAWL_D * u)
        drift = max(vals) - min(vals)
        print(f"[蠕动] {bone} 锚定相 u∈[{lo:.2f},{hi:.2f}] 世界位移 {drift:.4f} px")
        if drift > 0.05:
            bad.append(f"蠕动 {bone} 在锚定相内世界位移 {drift:.3f} px——在蹭地")

    # ---- ② 净前进量 = 自己的行程，**且方向朝 forward**
    a0 = along(FA.sample(rig, "shard_crawl", 0.0), g.mid)
    a1 = along(FA.sample(rig, "shard_crawl", 0.99999), g.mid) + FA.CRAWL_D * 0.99999
    adv = a1 - a0
    print(f"[蠕动] 每周期净前进 {adv:+.3f} px（设计 {FA.CRAWL_D:+.3f}，朝 forward 为正）")
    if abs(adv - FA.CRAWL_D) > 0.05:
        bad.append(f"净前进 {adv:+.3f} ≠ {FA.CRAWL_D:+.3f}"
                   + ("——**倒着爬**：crawl_world 返回的是核心系 z 坐标，换到主轴要取负"
                      if adv < 0 else ""))

    # ---- ③ 爬得比母体慢：逃窜速度正比于锚段间距，这是它跑不掉的理由
    v = FA.CRAWL_D * FA.CRAWL_HZ / 16.0
    vp = A.CRAWL_D * A.CRAWL_HZ / 16.0
    print(f"[蠕动] 碎片 {v:.3f} 格/s vs 母体 {vp:.3f} 格/s")
    if v >= vp:
        bad.append(f"碎片爬得不比母体慢（{v:.3f} ≥ {vp:.3f}）——"
                   f"速度须正比于锚段间距，检查 fragment_anim.SPAN")

    # ---- ④ 伸缩主轴必须贴着真实爬行方向，否则伸缩会横着撕开身体
    print(f"[朝向] 主轴 {'xyz'[i]}{'+' if sgn > 0 else '-'} 与 forward 对齐度 {FA.alignment():.3f}")
    if FA.alignment() < 0.7:
        bad.append(f"主轴与爬行方向只对齐 {FA.alignment():.2f}——伸缩轴与前进方向差太多，"
                   f"这块碎片得改用真实方向的形变，不能用骨骼轴缩放")

    # ---- ⑤ 循环接缝（相对判据，同 check_core_anim ④）
    for name, (_len, loop, n, _f) in FA.ANIMS.items():
        if not loop:
            continue
        step = max(pose_delta(rig, FA.sample(rig, name, k / n), FA.sample(rig, name, (k + 1) / n))
                   for k in range(n))
        d = pose_delta(rig, FA.sample(rig, name, 0.0), FA.sample(rig, name, 1.0 - 1.0 / n))
        print(f"[循环] {name} 接缝差 {d:.4f}（正常帧间步 {step:.4f}）")
        if d > step * 1.5 + 1e-6:
            bad.append(f"{name} 循环接缝跳变 {d:.3f}，是正常帧间步的 {d / max(step, 1e-9):.1f} 倍")

    # ---- ⑥ 乱抽：各抽各的、载体不动、体积守恒
    sc = FA.thrash_scale()
    names = list(g.sockets)
    # 芽是骨链，尖端在最后一节上。力臂取满长（见 check_core_anim）
    tipbone = {n: A.tendril(n)[0][-1] for n in names}
    local = {n: np.asarray(C.bud_shape(g.sockets[n], 1.0)[-1][0], float) for n in names}
    # 采样密度得盯着**最快的那条**：脉冲的起手只占一个周期的 FLICK_ATTACK，采稀了
    # 量到的峰值是插值出来的低值，会把摆得好好的芽误判成"没在摆"（实测 0.30 vs 真实 0.45）
    M = int(np.ceil(max(k for n in names for k, _p in A.joint_flicks(n, sc, A.THRASH_LEN))
                    / A.FLICK_ATTACK * 6))
    trk = {n: [] for n in names}
    for k in range(M):
        W = rig.world(FA.sample(rig, "shard_thrash", k / M))
        for n in names:
            T = W[tipbone[n]]
            trk[n].append(T[:3, :3] @ local[n] + T[:3, 3])
    trk = {n: np.array(v) for n, v in trk.items()}
    rel = {n: float(np.linalg.norm(v - v.mean(axis=0), axis=1).max()) / A.bud_reach(n, sc)
           for n, v in trk.items()}
    print(f"[乱抽] 相对摆幅 {min(rel.values()):.2f}..{max(rel.values()):.2f}")
    med = float(np.median(list(rel.values())))
    if min(rel.values()) < max(0.20, 0.5 * med):
        n = min(rel, key=lambda k: rel[k])
        bad.append(f"芽 {n} 尖端位移只有自身长度的 {rel[n]:.2f}——它没在摆")
    worst, pair = 0.0, ("", "")
    for a_i, a in enumerate(names):
        for b in names[a_i + 1:]:
            x, y = trk[a] - trk[a].mean(axis=0), trk[b] - trk[b].mean(axis=0)
            den = np.linalg.norm(x) * np.linalg.norm(y)
            c = abs(float((x * y).sum() / den)) if den > 1e-9 else 1.0
            if c > worst:
                worst, pair = c, (a, b)
    print(f"[乱抽] 两两轨迹最大互相关 {worst:.3f}（{pair[0]} / {pair[1]}）")
    if worst > 0.60:
        bad.append(f"{pair[0]} 与 {pair[1]} 摆得太同步（相关 {worst:.2f}）")
    body = [FA.sample(rig, "shard_thrash", k / 24)[g.mid].scale[0] for k in range(25)]
    if max(body) - min(body) > 1e-6:
        bad.append(f"乱抽时载体在动（{min(body):.3f}..{max(body):.3f}）")
    gained = sum(C.bud_tissue(s, 1.0) for s in g.sockets.values()) * sc ** 3
    lost = g.mass * C.VOX ** 3 * (1.0 - body[0] ** 3)
    print(f"[乱抽] 芽增 {gained:.0f} px³ / 载体减 {lost:.0f} px³")
    if abs(gained - lost) > max(1.0, gained * 0.02):
        bad.append(f"乱抽体积不守恒：芽增 {gained:.0f} 载体减 {lost:.0f}")

    # 碎片的料更少 ⇒ 茬更短 ⇒ 力臂更小 ⇒ 抽得比母体急。这是同一条 f ∝ 1/L 的推论
    # 比根节：碎片的茬更短 ⇒ 力臂更小 ⇒ 抽得更急。梢节两边都顶到 FLICK_MAX_HZ 上限，
    # 拿它比不出东西
    k_shard = max(A.joint_flicks(n, sc, A.THRASH_LEN)[0][0] for n in names)
    k_core = max(A.joint_flicks(n, A.thrash_scale(), A.THRASH_LEN)[0][0]
                 for n in C.sockets() if C.sockets()[n].girth > 2.0)
    print(f"[乱抽] 碎片根节最快 {k_shard} 下/循环 vs 母体根节最快 {k_core} 下")
    if k_shard <= k_core:
        bad.append(f"碎片抽得不比母体急（{k_shard} ≤ {k_core}）——茬更短就该更快，"
                   f"检查 thrash_scale 是否真按质量比缩了预算")

    # ---- ⑦ 逐槽嫁接
    grafts = [n for n in FA.ANIMS if n.startswith("shard_graft_")]
    if len(grafts) != len(g.sockets):
        bad.append(f"{len(grafts)} 条嫁接动画 ≠ {len(g.sockets)} 个槽")
    for n in grafts:
        sock = n[len("shard_graft_"):]
        vs = [FA.sample(rig, n, k / 160)[f"bud_{sock}"].scale[0] for k in range(161)]
        if any(b < a - 1e-6 for a, b in zip(vs, vs[1:])):
            bad.append(f"{n} 生长出现回退")
        if abs(vs[-1] - 1.0) > 0.02:
            bad.append(f"{n} 终值 {vs[-1]:.3f} ≠ 1.0")
        if sum(1 for a, b in zip(vs, vs[1:]) if b - a < 1e-6) < 4:
            bad.append(f"{n} 没有停滞段")
        mid = FA.sample(rig, n, 0.5)
        wrong = [m for m in g.sockets if m != sock
                 and abs(mid[f"bud_{m}"].scale[0] - A.BUD_DORMANT) > 1e-6]
        if wrong:
            bad.append(f"{n} 顺带长了别的槽 {wrong[:3]}")

    # ---- ⑧ 死亡：单调塌陷、终段静止
    ss = [FA.sample(rig, "shard_death", k / 60)[g.mid].scale[1] for k in range(61)]
    if any(b > a + 1e-6 for a, b in zip(ss, ss[1:])):
        bad.append("死亡时载体又鼓回去了——只能瘪下去")
    still = pose_delta(rig, FA.sample(rig, "shard_death", 1.0), FA.sample(rig, "shard_death", 0.96))
    if still > 0.05:
        bad.append(f"死亡终段仍在动 {still:.3f}")

    # ---- ⑨ 全动画不穿地。碎片本来就贴着地，余量比母体小得多
    for name in FA.ANIMS:
        low = min(rig.lowest(FA.sample(rig, name, k / 20)) for k in range(21))
        if low < -0.6:
            bad.append(f"{name} 穿地 {low:+.2f}")

    if bad:
        print(f"\n✗ {len(bad)} 处违例：")
        for x in bad:
            print(f"   {x}")
        return 1
    print("\n✓ 不滑步 / 前进方向 / 比母体慢 / 主轴对齐 / 循环 / 乱抽 / 嫁接 / 死亡 / "
          "不穿地 全通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
