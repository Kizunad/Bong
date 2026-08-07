#!/usr/bin/env python3
"""马 —— 外壳连续性自检：动画里皮毛壳有没有裂开。

为什么要单独一层：逆解残差与穿地只管**骨**摆得对不对，管不到骨上挂的**壳**。皮毛是
一串刚性盒子，弯曲全部集中在骨与骨的交界；颈只有三节骨而要弯到 −112° 去吃草，单个
关节就摊到 50°，交界两侧的盒子绕 pivot 各转 25°，离 pivot 6 个单位的鬃梢直接拉开
2·6·sin25° ≈ 5 个单位——渲出来是颈上一排楔形裂口，鬃像梳子一样炸开。

这一类缺陷静止姿一点看不出来（静止姿本来就是壳的设计姿），逐帧渲染也容易被缩略图
盖过去，只能靠算。

查哪些对（三道筛，每一道都是被假阳性/假阴性逼出来的）：
  · **直接父子骨**——一个关节两侧的壳必须闭合。隔一个关节以上的贴合（腹皮蹭到前臂）
    分开是肢体在摆，不是壳裂了。
  · **两件都罩住该关节的 pivot**——这是"跨该关节的焊缝"的定义。少了这条，同一条腿上
    离关节老远的两件只要角上啃到一点就被当成接缝。皮层的分段本来就是照关节切的，
    所以这条不会漏掉真接缝。
  · **两件都没被造型层标 loose**——耳 / 唇 / 颌线 / 额发 / 尾鬃股本来就该各动各的，
    "张嘴"不是裂口。这层语义只有造型代码知道，所以由它声明，不在这里猜。

怎么量：
  1. 在静止姿的贴合区里撒点，取真正同时落在两件里的，每个点朝**各自那一件的中心**
     推进 SEED_INSET，得到一对确在肉里的种子。直接用贴合面上的点不行——它的两个像
     都贴着各自的外表面，连线会擦着体表在外面走一路，报成"整条视线全空"，而那里
     只是壳弯了个角。
  2. 两枚种子分别按两根骨摆过去，取**弦**上最长的一段"不落在任何一件里"的连续段，
     那才是裂口宽度。两个像离得远不等于看得见缝：屈曲 96° 的腕，接缝上一点的两个像
     本来就分开 4 个单位，而它们之间整段埋在肉里。光走直线，所以取弦。
"""

from __future__ import annotations

import sys
from itertools import combinations
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

from rig import Rig, euler  # noqa: E402

# 判"贴合"时给的余量。**必须含"恰好贴面"这一档**——颈皮是一段接一段拼上去的，相邻
# 两段共用一个面、交叠为零，而这正是最容易裂开的地方。
SNUG = 0.02
PROBE = 17  # 视线上的采样点数（决定裂口宽度的分辨率）
SEED_GRID = 7  # 静止姿贴合区里撒点的网格边长
SEED_KEEP = 9  # 每对保留几个共有材料点
SEED_INSET = 0.20  # 接触点朝各自件内推进多少，得到一对确在肉里的种子
GAP_TOL = 0.30  # 允许的最大可见裂口（单位 = 体素；1 单位 = 6.25 cm）


def _obb(e: dict) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """元件 → (中心, 半长, 三轴)，静止姿模型坐标。"""
    f, t = np.array(e["from"], float), np.array(e["to"], float)
    c, h = (f + t) / 2.0, np.abs((t - f) / 2.0)
    R = np.eye(3)
    rot = e.get("rotation", [0, 0, 0])
    if any(rot):
        o = np.array(e.get("origin", [0, 0, 0]), float)
        R = euler(rot)
        c = R @ (c - o) + o
    return c, h, R


def _longest_run(free: np.ndarray) -> np.ndarray:
    """逐行求最长的连续 True 段长度（以"间隔数"计，两端点算 PROBE−1 段）。"""
    n = free.shape[1]
    best = np.zeros(free.shape[0], int)
    cur = np.zeros(free.shape[0], int)
    for k in range(n):
        cur = np.where(free[:, k], cur + 1, 0)
        best = np.maximum(best, cur)
    return np.maximum(0, best - 1)


class Shell:
    """一次性备好静止姿的壳信息，之后每帧只做矩阵乘。"""

    def __init__(self, rig: Rig):
        self.rig = rig
        self.bone: list[str] = []
        self.name: list[str] = []
        self.c: list[np.ndarray] = []
        self.h: list[np.ndarray] = []
        self.R: list[np.ndarray] = []
        self.loose: list[bool] = []
        for bone in rig.order:
            for u in rig.bones[bone].elements:
                e = rig.elements[u]
                c, h, R = _obb(e)
                self.bone.append(bone)
                self.name.append(e.get("name", u))
                self.loose.append(bool(e.get("_loose")))
                self.c.append(c)
                self.h.append(h)
                self.R.append(R)
        self.C = np.array(self.c)
        self.Hh = np.array(self.h)
        self.RR = np.array(self.R)

        self.pi: list[int] = []
        self.pj: list[int] = []
        seeds: list[np.ndarray] = []
        aabb = [(c - np.abs(R) @ h, c + np.abs(R) @ h) for c, h, R in zip(self.c, self.h, self.R)]
        for i, j in combinations(range(len(self.c)), 2):
            bi, bj = self.bone[i], self.bone[j]
            if bi == bj:
                continue
            # 只查**直接父子**：隔一个关节以上的贴合（腹皮蹭到前臂）分开是肢体在摆，
            # 不是壳裂了。一个关节两侧的壳必须闭合，这是能立的规矩。
            if rig.bones[bi].parent != bj and rig.bones[bj].parent != bi:
                continue
            if self.loose[i] or self.loose[j]:  # 造型层声明"本来就该分开"（耳 / 唇 / 额发）
                continue
            # **两件都得罩住这个关节的 pivot**——这才是"跨该关节的焊缝"的定义。少了这条，
            # 同一条腿上离关节老远的两件只要角上啃到一点（大腿皮的下角蹭到跗鼓的上角）
            # 就被当成接缝，而它们分开是关节在屈，不是壳裂了。接缝按定义落在 pivot 上
            # （皮层的分段就是照关节切的），所以这条不会漏掉真接缝。
            piv = rig.bones[bj if rig.bones[bj].parent == bi else bi].origin[None, :]
            if not (self._in(piv, i)[0] and self._in(piv, j)[0]):
                continue
            lo = np.maximum(aabb[i][0], aabb[j][0]) - SNUG
            hi = np.minimum(aabb[i][1], aabb[j][1]) + SNUG
            if np.any(hi < lo):
                continue
            # AABB 交集只是粗筛。带自旋的皮筒，AABB 比实体大得多——直接拿 AABB 的角点
            # 当"两件共有的材料点"，取到的多半是**两件都不在**的空中一点，摆姿后它们
            # 被各自的骨甩开几个单位，报出来全是假裂口。必须在交集里撒点、只留真正
            # 同时落在两件里的。
            g = np.linspace(0.0, 1.0, SEED_GRID)
            grid = np.array([[x, y, z] for x in g for y in g for z in g]) * (hi - lo) + lo
            keep = grid[self._in(grid, i) & self._in(grid, j)]
            if len(keep) < 2:
                continue
            keep = keep[np.linspace(0, len(keep) - 1, SEED_KEEP).round().astype(int)]
            # 一个接触点派生**两枚**种子：分别朝两件的中心里推 SEED_INSET。
            # 直接拿接触点本身不行——它落在两件的表面上，两个像都贴着各自的外表面，
            # 连线会擦着体表在外面走一路，报成"整条视线全空"，而那里只是壳弯了个角。
            # 各自推进内部之后，静止姿下这两点相距 ~2ε 且都在肉里；摆姿后连线若还跑到
            # 体外，那才是真的看得穿。**这一步对"零交叠、恰好贴面"的接缝同样成立**——
            # 而那正是最容易裂的一种（首版整条颈都是），不能靠"要求深在两件之内"过滤掉。
            self.pi.append(i)
            self.pj.append(j)
            seeds.append((self._inward(keep, i), self._inward(keep, j)))
        self.seed_a = np.array([a for a, _ in seeds]) if seeds else np.zeros((0, SEED_KEEP, 3))
        self.seed_b = np.array([b for _, b in seeds]) if seeds else np.zeros((0, SEED_KEEP, 3))
        self.ai = np.array(self.pi, int)
        self.aj = np.array(self.pj, int)

    def _inward(self, pts: np.ndarray, k: int) -> np.ndarray:
        """把接触点朝第 k 件的中心推 SEED_INSET，得到一枚**确在该件内部**的种子。"""
        v = self.c[k][None, :] - pts
        n = np.linalg.norm(v, axis=1, keepdims=True)
        return pts + v / np.maximum(n, 1e-9) * np.minimum(SEED_INSET, n * 0.9)

    def _in(self, pts: np.ndarray, k: int, slack: float = SNUG) -> np.ndarray:
        """静止姿下，这些点是否落在第 k 件里（slack 为负 = 要求落在里面这么深）。"""
        loc = (pts - self.c[k]) @ self.R[k]
        return (np.abs(loc) <= self.h[k] + slack).all(axis=1)

    def inside_any(self, pts: np.ndarray, C: np.ndarray, RR: np.ndarray) -> np.ndarray:
        """pts 里每个点是否落在**任何**一件里（已摆姿的 OBB）。"""
        if not len(pts):
            return np.zeros(0, bool)
        d = pts[:, None, :] - C[None, :, :]  # (P, N, 3)
        loc = np.einsum("nij,pnj->pni", RR.transpose(0, 2, 1), d)
        # 余量与选种子时同为 SNUG。少了这一步，落在**面上**的种子（贴合区的角，正好在
        # 两件的表面上）连自己都算"在外面"，整条视线报成全空——颈上每一道接缝都会
        # 凭空多出 2 个单位的假裂口。
        return (np.abs(loc) <= self.Hh[None] + SNUG).all(axis=2).any(axis=1)

    def gaps(self, pose) -> list[tuple[float, str, str]]:
        """本帧所有**可见**裂口，(宽度, 件 A, 件 B)。"""
        if not len(self.seed_a):
            return []
        W = self.rig.world(pose)
        M = np.array([W[b] for b in self.bone])
        Rw = M[:, :3, :3]
        C = np.einsum("nij,nj->ni", Rw, self.C) + M[:, :3, 3]
        RR = Rw @ self.RR

        pa = np.einsum("pij,pkj->pki", Rw[self.ai], self.seed_a) + M[self.ai][:, None, :3, 3]
        pb = np.einsum("pij,pkj->pki", Rw[self.aj], self.seed_b) + M[self.aj][:, None, :3, 3]
        d = np.linalg.norm(pa - pb, axis=2)  # (对, 采样点)
        hit = np.argwhere(d > GAP_TOL)
        if not len(hit):
            return []
        # 两个像离得远**不等于**看得见缝：屈曲 96° 的腕，接缝上一点的两个像本来就分开
        # 2r·sin48°≈4 个单位，而它们之间整段都埋在肉里。裂口的正确量法是**这条视线上
        # 露空的那一段有多长**——光走直线，所以取弦，逐点问"落在任何一件里吗"，
        # 再取最长的一段连续露空。露空 0 = 看不见缝，不管两端分得多开。
        ts = np.linspace(0.0, 1.0, PROBE)[None, :, None]
        A = pa[hit[:, 0], hit[:, 1]][:, None, :]
        B = pb[hit[:, 0], hit[:, 1]][:, None, :]
        probe = (A * (1 - ts) + B * ts).reshape(-1, 3)
        free = ~self.inside_any(probe, C, RR).reshape(-1, PROBE)
        span = _longest_run(free) / (PROBE - 1) * d[hit[:, 0], hit[:, 1]]

        best: dict[tuple[int, int], float] = {}
        for (pidx, _k), gap in zip(hit, span):
            if gap <= GAP_TOL:
                continue
            key = (self.pi[pidx], self.pj[pidx])
            best[key] = max(best.get(key, 0.0), float(gap))
        out = [(v, self.name[i], self.name[j]) for (i, j), v in best.items()]
        out.sort(reverse=True)
        return out


def check(rig: Rig, sampler, names, samples: int = 24) -> list[tuple[str, float, str, float]]:
    """逐动画逐帧取最宽的可见裂口。sampler(name, t01) → Pose。"""
    sh = Shell(rig)
    rows = []
    for name in names:
        worst, tag, at = 0.0, "—", 0.0
        for i in range(samples):
            t = i / samples
            g = sh.gaps(sampler(name, t))
            if g and g[0][0] > worst:
                worst, tag, at = g[0][0], f"{g[0][1]} ↮ {g[0][2]}", t
        rows.append((name, worst, tag, at))
    return rows


def main() -> int:
    import argparse

    import gen_anim as G
    from gen_skeleton import PROFILES
    from rig import FINAL

    ap = argparse.ArgumentParser(description="外壳连续性自检")
    ap.add_argument("--profile", default=None)
    ap.add_argument("--coat", default="rust")
    args = ap.parse_args()

    rc = 0
    for k in [args.profile] if args.profile else ["small", "medium", "large"]:
        rig = Rig(FINAL / f"HorsePelt_{args.coat}_{k}.bbmodel")
        P = PROFILES[k]
        print(f"[{k}]")
        for name, worst, tag, at in check(rig, lambda n, t: G.sample(rig, P, n, t), list(G.ANIMS)):
            ok = worst <= GAP_TOL
            rc += 0 if ok else 1
            print(f"  {'✓' if ok else '✗'} {name:7s} 最宽可见裂口 {worst:5.2f} @ {tag} t={at:.2f}")
    print("\n" + ("✓ 外壳全程闭合" if rc == 0 else f"✗ {rc} 条动画出现可见裂口"))
    return 1 if rc else 0


if __name__ == "__main__":
    raise SystemExit(main())
