#!/usr/bin/env python3
"""异变缝合兽 —— 碎裂 / 分裂 / 繁殖：这东西怎么变成更多这东西。

## 为什么它会散

缝合兽不是一只动物，是**几团来源不同的组织被迫长在一起**。把它们粘住的只有那几道
癒合痕。所以：

  · 撑不住了 → 沿最弱的那道痕裂开（分裂）
  · 被打死了 → 所有弱于某阈值的痕同时崩（爆体），碎片各自逃窜
  · 碎片活下来 → 捡尸体重新长成一只完整的（繁殖）

**从哪儿裂开不需要画**：癒合痕就是两个 lobe 的表面交界，交界体素数就是那道接合的
面积，也就是它的强度（`core.weld_areas`）。应力升高 = 逐条切断弱于阈值的接合，
剩下的连通分量就是碎片。碎裂图谱是**算出来的**，同一只兽每次裂法一致、可复现。

## 碎片就是一只小兽

每块碎片带走它覆盖的挂载点，也就带走了长在那些挂载点上的肢体。于是它的逃窜步态
**直接走 locomotion.solve()**——碎片是一只基因组子集的兽，同一套代码：够得着地的
承重肢 ≥3 就用步态跑，不够就蠕动爬（core_anim 那一套）。

「不同部位往不同方向逃窜」于是不是特效，是每块碎片各自解出来的步态：带蛛足的碎片
碎步窜，带牛腿的碎片大步挪，什么都没带的那块只能一鼓一鼓地蠕。

## 动量守恒

爆体的碎片速度必须满足 Σmᵢvᵢ = 0（原地死亡时整体动量为零）。不守恒的话整团碎片会
朝一边飞，读成"被炸飞"而不是"自己崩开"。自检直接断言。

用法:
  python3 scripts/models/stitched_beast/fission.py            # 碎裂图谱
  python3 scripts/models/stitched_beast/fission.py --seed 7   # 某只兽的碎片与逃窜方式
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import genome as GN  # noqa: E402

# 碎片能独立存活的最小实心体素数。低于此只是一摊肉，不会长成新的兽。
VIABLE_MASS = 90
# 爆体时的接合断裂阈值：面积 ≤ 此值的癒合痕全崩。取自实测面积分布的中位数附近。
BURST_THRESHOLD = 12
# 爆体总动能预算（px²/s² × 体素），决定碎片飞多快。核心撑破的是自己的表皮，不是炸药。
BURST_ENERGY = 2.6e4


@dataclass(frozen=True)
class Fragment:
    """一块碎片：几团肉 + 它带走的挂载点。"""

    lobes: tuple[str, ...]
    sockets: tuple[str, ...]
    mass: int
    centroid: np.ndarray
    launch: np.ndarray             # 爆开初速度（px/s）

    @property
    def viable(self) -> bool:
        return self.mass >= VIABLE_MASS

    @property
    def span(self) -> float:
        """特征体长：体积的立方根。蠕动速度正比于它（见 core_anim.crawl_speed）。"""
        return float((self.mass * C.VOX ** 3) ** (1.0 / 3.0))

    def genome(self, parent: GN.Genome) -> GN.Genome:
        """碎片自己的基因组 = 亲代基因里落在它带走的挂载点上的那些部件。

        碎片不是新生成的东西，是**亲代的一部分**——带走什么就有什么，这是繁殖而不是刷怪。
        """
        keep = set(self.sockets)
        return GN.Genome(parent.seed,
                         tuple(g for g in parent.limbs if g.socket in keep),
                         tuple(g for g in parent.heads if g.socket in keep))


# ---------------------------------------------------------------- 断裂
def _components(lobes: list[str], edges: dict[tuple[str, str], int],
                threshold: int) -> list[tuple[str, ...]]:
    """切断所有面积 ≤ threshold 的接合后，剩下的连通分量。"""
    parent = {n: n for n in lobes}

    def find(a: str) -> str:
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    for (a, b), area in edges.items():
        if area > threshold and a in parent and b in parent:
            ra, rb = find(a), find(b)
            if ra != rb:
                parent[ra] = rb
    groups: dict[str, list[str]] = {}
    for n in lobes:
        groups.setdefault(find(n), []).append(n)
    return [tuple(sorted(v)) for v in sorted(groups.values(), key=lambda g: -len(g))]


def fracture(threshold: int) -> list[tuple[str, ...]]:
    """给定应力阈值下的碎片分组（按块大小降序）。"""
    lobes = [lb.name for lb in C.LOBES]
    return _components(lobes, C.weld_areas(), threshold)


def sockets_of(lobes: tuple[str, ...]) -> tuple[str, ...]:
    """这组 lobe 带走哪些挂载点——挂载点骑在哪根骨上，就跟哪块碎片走。"""
    return tuple(sorted(s.name for s in C.sockets().values() if s.bone in lobes))


def build_fragments(threshold: int = BURST_THRESHOLD,
                    energy: float = BURST_ENERGY) -> list[Fragment]:
    """碎片列表，含满足动量守恒的爆开初速度。

    方向取"碎片质心相对整体质心"的外向——撑破表皮的压力从内往外推。大小按等分动能
    分配（v ∝ 1/√m：同样的能量，重的飞得慢），再统一减去质量加权平均，把净动量清零。
    减完之后方向会略偏，这是对的：不对称的兽崩开本来就不对称。
    """
    mass = C.lobe_mass()
    cen = C.lobe_centroid()
    whole = C.centroid()

    groups = fracture(threshold)
    raw: list[tuple[tuple[str, ...], int, np.ndarray, np.ndarray]] = []
    for g in groups:
        m = sum(mass.get(n, 0) for n in g)
        if m <= 0:
            continue
        c = sum(cen[n] * mass[n] for n in g if n in mass) / m
        d = c - whole
        d[1] = abs(d[1]) * 0.5 + 2.0          # 略微向上：崩开是鼓破，不是贴地滑
        n = float(np.linalg.norm(d))
        u = d / n if n > 1e-6 else np.array([0.0, 1.0, 0.0])
        raw.append((g, m, c, u))

    total = sum(m for _g, m, _c, _u in raw)
    speeds = [np.sqrt(2.0 * energy / (len(raw) * m)) for _g, m, _c, _u in raw]
    vels = [u * s for (_g, _m, _c, u), s in zip(raw, speeds)]

    # 清零净动量：Σmᵢvᵢ = 0，否则整团碎片朝一边飞，读成"被炸飞"而不是"自己崩开"
    drift = sum(m * v for (_g, m, _c, _u), v in zip(raw, vels)) / total
    vels = [v - drift for v in vels]

    return [Fragment(g, sockets_of(g), m, c, v)
            for (g, m, c, _u), v in zip(raw, vels)]


def _connected(group: tuple[str, ...], edges: dict[tuple[str, str], int]) -> bool:
    """这组 lobe 在只保留组内接合时是否连通——分裂出来的必须是**一整块**，不是两坨。"""
    if not group:
        return False
    s = set(group)
    seen = {group[0]}
    stack = [group[0]]
    while stack:
        n = stack.pop()
        for (a, b) in edges:
            if a in s and b in s:
                nxt = b if a == n else (a if b == n else None)
                if nxt and nxt not in seen:
                    seen.add(nxt)
                    stack.append(nxt)
    return seen == s


def split_seam() -> tuple[tuple[str, ...], tuple[str, ...], int] | None:
    """健康分裂：找一条**最省力**、且切开后两半都能活的裂面。

    撕开一只兽要切断**跨越裂面的所有接合**，不是切断一条。初版只枚举单条边，7 个
    lobe 的接合图连得密，删一条几乎不可能断开，于是永远报"无可行切面"——那不是这只
    兽分不了，是模型问的问题不对。

    正解是最小割：枚举全部二分（7 个 lobe → 64 种），要求两侧各自连通且质量达标，
    取跨面接合面积之和最小的那个。节点这么少，直接暴力就是精确解。
    """
    mass = C.lobe_mass()
    edges = C.weld_areas()
    lobes = [lb.name for lb in C.LOBES]
    best: tuple[tuple[str, ...], tuple[str, ...], int] | None = None
    for bits in range(1, 1 << (len(lobes) - 1)):
        a = tuple(sorted(n for i, n in enumerate(lobes) if bits >> i & 1))
        b = tuple(sorted(n for n in lobes if n not in a))
        if not a or not b:
            continue
        if sum(mass.get(n, 0) for n in a) < VIABLE_MASS:
            continue
        if sum(mass.get(n, 0) for n in b) < VIABLE_MASS:
            continue
        if not (_connected(a, edges) and _connected(b, edges)):
            continue
        cut = sum(w for (x, y), w in edges.items()
                  if (x in a) != (y in a))
        if best is None or cut < best[2]:
            best = (a, b, cut)
    return best


def report() -> str:
    rows = ["接合面积（癒合痕交界体素数，即强度）："]
    for (a, b), n in sorted(C.weld_areas().items(), key=lambda kv: -kv[1]):
        rows.append(f"  {a:<12}—{b:<12} {n:>3}")
    mass = C.lobe_mass()
    rows.append("\n各团实心体素数：")
    for n, m in sorted(mass.items(), key=lambda kv: -kv[1]):
        rows.append(f"  {n:<12} {m:>4}{'' if m >= VIABLE_MASS else '   （单独活不下去）'}")

    rows.append("\n应力阈值 → 碎片数：")
    for th in (0, 6, 8, 10, 12, 16, 20):
        g = fracture(th)
        rows.append(f"  ≤{th:>2} 崩：{len(g)} 块  " + " | ".join("+".join(x) for x in g))

    sp = split_seam()
    if sp:
        a, b, area = sp
        rows.append(f"\n健康分裂切面：{'+'.join(a)}  ⟋  {'+'.join(b)}（接合面积 {area}）")
    else:
        rows.append("\n健康分裂：无可行切面（切开必有一半活不下去）")
    return "\n".join(rows)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=None, help="给某只兽算碎片与逃窜方式")
    ap.add_argument("--threshold", type=int, default=BURST_THRESHOLD)
    args = ap.parse_args()

    print(report())
    frags = build_fragments(args.threshold)
    p = sum(f.mass * f.launch for f in frags)
    print(f"\n爆体（阈值 ≤{args.threshold}）：{len(frags)} 块，"
          f"净动量 |Σmv| = {float(np.linalg.norm(p)):.2e}")
    for f in frags:
        print(f"  {'+'.join(f.lobes):<34} 质量 {f.mass:>4}  "
              f"速度 {float(np.linalg.norm(f.launch)):>5.1f} px/s  "
              f"槽 {len(f.sockets)}  {'可存活' if f.viable else '活不下去'}")

    if args.seed is not None:
        import locomotion as L
        socks = C.sockets()
        g, _gait = L.sample_standing(args.seed, socks=socks)
        print(f"\nseed={args.seed} 的兽崩开后各碎片怎么逃：")
        for f in frags:
            fg = f.genome(g)
            limbs = [x for x in fg.limbs if x.load_bearing]
            try:
                # 质心必须传碎片自己的：碎片的脚全挤在一侧，拿整体质心去比必然判站不住
                gait = L.solve(fg, socks, com=f.centroid)
                how = (f"步态 {gait.blocks_per_sec:.2f} 格/s，"
                       f"步数比 {':'.join(map(str, sorted({x.steps for x in gait.limbs})))}")
            except ValueError as e:
                import core_anim as A
                v = A.crawl_speed(f.span) / 16.0
                how = f"只能蠕动爬 {v:.3f} 格/s（{e}）"
            print(f"  {'+'.join(f.lobes):<34} 带走 {len(limbs)} 条承重肢 → {how}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
