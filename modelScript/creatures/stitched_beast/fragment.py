#!/usr/bin/env python3
"""异变缝合兽 —— 碎片几何：撕下来的那一块，作为一只**独立的兽**该长什么样。

fission.py 回答的是"从哪儿裂、裂成几块、各块多重、往哪飞"。这里回答下一个问题：
**那一块单独拿出来是什么东西？**

碎片不是母体的缩略图，有三件事必须重算：

## 一、创面

母体的表层判据是"6 邻域有一格在体外"。少了另一半之后，原来埋在**内部**的体素就成了
表面——撕裂面于是自动出现，不用画（`core.surface_of`）。哪些格是创面也是推出来的：
缺失的那个邻居在母体里是实心的，就说明肉原来在那儿、现在被扯走了。

创面的法向不能用场梯度：场在体内是光滑的，断口处 -∇f 指向随便哪儿。外法向就是
**缺失邻居的方向**。材质也另起一张表——断口露的是内部组织，不该有黏液（挂在外面的）
也不该有膜（被撑薄的表皮）。

## 二、它拿什么爬

碎片多半只有一团肉。**一块刚体爬不了**：蠕动要两个锚段轮流抓地，一根骨做不到。所以
只带一团肉的碎片必须**重新分节**——沿爬行方向按质量三等分，切成前/中/后。分节不是
解剖结构，是收缩波的几何：蛞蝓的足底也没有节，波照样沿足底走。

带两团以上的碎片就直接沿用母体的骨——它本来就是分开的几块。

爬行方向也是推出来的：**带头槽的碎片朝头槽的方向爬**（那是它将来的前端），一个头槽
都没带的就朝**背离母体**的方向爬——它是被扯下来的，逃的方向就是断口的反向。

## 三、它长不出触手

芽的组织不是凭空出现的（`core.graft_budget`）。碎片只有母体的零头，同样的槽在它身上
只能冒出更短的茬。**这就是它必须去捡尸的原因**——不是设定，是体积算出来的。

用法:
  python3 modelScript/creatures/stitched_beast/fragment.py             # 健康分裂那一块
  python3 modelScript/creatures/stitched_beast/fragment.py --lobes core_hind,core_sag,nodule_r
"""

from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402

SEGMENTS = 3          # 单团碎片重新分节的节数：两个锚 + 一个桥。少一节就爬不动
GROUND = 0.0          # 碎片静止姿的腹底高度。它是爬行动物，肚子就贴在地上——
                      # 长出承重肢之后的抬升由运动层解（locomotion.solve_ride_height）


@dataclass(frozen=True)
class Bone:
    name: str
    pivot: np.ndarray      # 独立坐标系下的枢轴
    parent: str | None


@dataclass
class FragGeom:
    """一块碎片的完整几何，坐标仍在**母体坐标系**里。

    搬到自己原点这一步交给 `gen_fragment`：碎片的落地高度得按**建好之后的真实几何**
    量（癒合脊会凸到质量块之下、抖动还会再外扩），在这一层按体素格底预测必然差几分之
    一像素，实测整只沉进地里 0.06–0.90px。量测口径只能有一个，就放在生成完之后。
    """

    lobes: tuple[str, ...]
    voxels: tuple[C.Voxel, ...]
    bone_of: dict[tuple[int, int, int], str]   # 体素 → 骨（单团碎片会重新分节）
    bones: tuple[Bone, ...]
    sockets: dict[str, C.Socket]
    forward: np.ndarray
    fore: str
    hind: str
    mid: str

    @property
    def mass(self) -> int:
        return C.group_mass(self.lobes)

    @property
    def span(self) -> float:
        """前后锚段枢轴的间距 —— 蠕动的体长基准，和母体 LOBE_SPAN 同一口径。"""
        piv = {b.name: b.pivot for b in self.bones}
        return float(np.dot(piv[self.fore] - piv[self.hind], self.forward))

    @property
    def rough(self) -> float:
        """表面抖动幅度相对母体的缩放 = **体长之比**。

        抖动是相对尺度的扰动。母体 31px 长，同样 ±0.55px 的起伏是肉的纹理；碎片只有
        15px 长、横过去才 6 格体素，同样的起伏让每一格都自成一块，整只读成一堆碎石
        （round 2 出图实测）。按体长缩放之后，两者的**相对**粗糙度才一致。
        """
        pts = np.array([C._voxel_center(v.ix, v.iy, v.iz) for v in self.voxels])
        parent = C.core_bounds()
        return float(np.ptp(pts, axis=0).max() + C.VOX) / float((parent[1] - parent[0]).max())

    @property
    def torn_ratio(self) -> float:
        """表层里创面占的比例。越大说明撕得越狠、这块越"新鲜"。"""
        return sum(v.torn for v in self.voxels) / max(1, len(self.voxels))

    def growth(self) -> float:
        """所有槽同时冒芽时每条能长到的生长度 —— 组织预算按质量比缩小。"""
        budget = C.graft_budget() * self.mass / sum(C.lobe_mass().values())
        return C.spread_growth(tuple(self.sockets.values()), budget)


# ---------------------------------------------------------------- 朝向
def forward_of(lobes: tuple[str, ...]) -> np.ndarray:
    """碎片的爬行方向（水平单位向量）。

    带头槽 → 朝头槽的法向平均：那是它将来长脸的地方，动物朝自己的感觉端走。
    不带头槽 → 朝**背离母体质心**的方向：它是被扯下来的，断口在身后，逃就往断口反向。

    两条规则都不是选的，是碎片自己身上带的信息。
    """
    heads = [s.normal for s in C.sockets().values()
             if s.kind == "head" and s.bone in lobes]
    if heads:
        d = np.mean(heads, axis=0)
    else:
        d = C.group_centroid(lobes) - C.centroid()
        if float(np.linalg.norm(d)) < 1e-6:      # 碎片就是整只兽（阈值太低没裂开）
            d = np.array([0.0, 0.0, -1.0])
    d = np.array([d[0], 0.0, d[2]])
    n = float(np.linalg.norm(d))
    return d / n if n > 1e-6 else np.array([0.0, 0.0, -1.0])


# ---------------------------------------------------------------- 挂载点存活
@lru_cache(maxsize=16)
def viable_sockets(lobes: tuple[str, ...]) -> tuple[str, ...]:
    """这块碎片**真正带得走**的挂载点。

    骑在裂面上的槽会被撕坏。槽是皮上的一处结构（一小片够平、够厚的表皮），裂面正好从
    它中间过去时，那片皮有一半跟着另一半走了——剩下的这半边缘就是创面，芽没有地方生根。
    判据直接问几何：槽最近的存活表层格是不是创面。

    于是「分裂要付代价」是推论不是设定：分出去的两半加起来比母体**少几个槽**，少的
    正好是骑在裂面上的那几个。撕开自己不是免费的。
    """
    vox = C.surface_of(lobes)
    out = []
    for s in C.sockets().values():
        if s.bone not in lobes:
            continue
        near = min(vox, key=lambda v: float(np.linalg.norm(
            C._voxel_center(v.ix, v.iy, v.iz) + C.CORE_CENTER - s.pos)))
        if not near.torn:
            out.append(s.name)
    return tuple(sorted(out))


def lost_sockets(groups: tuple[tuple[str, ...], ...]) -> tuple[str, ...]:
    """一次碎裂里被裂面毁掉的槽（母体有、各碎片都没带走的）。"""
    kept = {n for g in groups for n in viable_sockets(g)}
    return tuple(sorted(set(C.sockets()) - kept))


# ---------------------------------------------------------------- 分节
def _resegment(vox: list[C.Voxel], fwd: np.ndarray) -> tuple[dict, list[str]]:
    """把一团肉沿爬行方向按**质量**三等分，切成 seg_hind / seg_mid / seg_fore。

    按质量分而不是按长度分：等长切法会在两端切出只有两三格的薄片，那种节既撑不住
    锚定也看不出在收缩。等质量切法保证每一节都有实打实的肉。
    """
    solid = [k for k, n in C.solid_grid().items() if n in {v.bone for v in vox}]
    proj = {k: float(np.dot(C._voxel_center(*k), fwd)) for k in solid}
    order = sorted(solid, key=lambda k: proj[k])
    names = ["seg_hind", "seg_mid", "seg_fore"]
    cuts = [proj[order[min(len(order) - 1, (i + 1) * len(order) // SEGMENTS)]]
            for i in range(SEGMENTS - 1)]

    def which(k) -> str:
        p = float(np.dot(C._voxel_center(*k), fwd))
        return names[sum(p >= c for c in cuts)]

    return {k: which(k) for k in solid}, names


# ---------------------------------------------------------------- 构建
@lru_cache(maxsize=8)
def geom(lobes: tuple[str, ...]) -> FragGeom:
    """一块碎片的独立几何。lobes 必须是 core.LOBES 里的名字。"""
    known = {lb.name for lb in C.LOBES}
    bad = [n for n in lobes if n not in known]
    if bad:
        raise ValueError(f"未知 lobe {bad}；可选 {sorted(known)}")
    vox = C.surface_of(lobes)
    if not vox:
        raise ValueError(f"lobe 组 {lobes} 没有表层体素")
    fwd = forward_of(lobes)

    # ---- 骨：两团以上沿用母体分段；只有一团就重新分节（一块刚体爬不了）
    if len(lobes) >= 2:
        bone_of = {(v.ix, v.iy, v.iz): v.bone for v in vox}
        piv = {n: np.array(next(lb.center for lb in C.LOBES if lb.name == n)) + C.CORE_CENTER
               for n in lobes}
    else:
        bone_of, names = _resegment(vox, fwd)
        piv = {}
        for n in names:
            pts = [C._voxel_center(*k) for k, b in bone_of.items() if b == n]
            piv[n] = np.mean(pts, axis=0) + C.CORE_CENTER

    # 骨归属必须**在合并之前**落回体素上：merge_slabs 只按 (bone, mat) 分组，重新分节
    # 后再合并会得到另一套 slab、另一套抖动、另一个最低点。两处各合并一次必然对不上。
    for v in vox:
        v.bone = bone_of[(v.ix, v.iy, v.iz)]

    # ---- 前锚 / 后锚：沿爬行方向投影的两端。中间的那些都是桥
    proj = {n: float(np.dot(p, fwd)) for n, p in piv.items()}
    fore = max(proj, key=lambda n: proj[n])
    hind = min(proj, key=lambda n: proj[n])
    mids = [n for n in piv if n not in (fore, hind)]
    mid = max(mids, key=lambda n: C.lobe_mass().get(n, 0)) if mids else fore

    bones = [Bone("root", np.zeros(3), None)]
    tree = {mid: "root"} if mids or fore == mid else {fore: "root"}
    for n in piv:
        if n not in tree:
            tree[n] = mid
    for n in sorted(piv, key=lambda x: (x != mid, x)):
        bones.append(Bone(n, piv[n], tree[n] if n != mid else "root"))

    # ---- 挂载点：只带走没被裂面撕坏的，且跟着骨走。单团碎片重新分了节，
    #      槽得改挂到覆盖它的那一节上
    keep_socks = set(viable_sockets(lobes))
    socks: dict[str, C.Socket] = {}
    for s in C.sockets().values():
        if s.name not in keep_socks:
            continue
        if len(lobes) >= 2:
            bone = s.bone
        else:
            near = min(bone_of, key=lambda k: float(
                np.linalg.norm(C._voxel_center(*k) + C.CORE_CENTER - s.pos)))
            bone = bone_of[near]
        socks[s.name] = C.Socket(s.name, s.kind, s.pos, s.normal, bone,
                                 s.girth, s.azimuth, s.elevation, dict(s.meta))

    return FragGeom(lobes, tuple(vox), bone_of, tuple(bones), socks,
                    fwd, fore, hind, mid)


def default_lobes() -> tuple[str, ...]:
    """健康分裂分出去的那一半（较轻的一边）—— "分裂的小模型"默认就是它。"""
    import fission as F

    sp = F.split_seam()
    if sp is None:
        raise ValueError("没有可行的健康分裂切面")
    a, b, _cut = sp
    return a if C.group_mass(a) <= C.group_mass(b) else b


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--lobes", default="", help="逗号分隔；缺省取健康分裂分出的那半")
    args = ap.parse_args()
    lobes = tuple(filter(None, args.lobes.split(","))) or default_lobes()
    g = geom(lobes)

    body = np.array([C._voxel_center(v.ix, v.iy, v.iz) for v in g.voxels]) + C.CORE_CENTER
    size = body.max(axis=0) - body.min(axis=0) + C.VOX
    print(f"碎片 {'+'.join(g.lobes)}")
    print(f"  实心 {g.mass} 体素   表层 {len(g.voxels)}（创面 {g.torn_ratio * 100:.0f}%）")
    print(f"  尺寸 {size[0] / 16:.2f} × {size[1] / 16:.2f} × {size[2] / 16:.2f} 格"
          f"   母体 1.68 × 1.43 × 1.92 格")
    print(f"  爬行方向 ({g.forward[0]:+.2f},{g.forward[2]:+.2f})   "
          f"锚段 {g.hind} → {g.fore}   体长基准 {g.span:.1f}px")
    print(f"  骨 {len(g.bones)}：" + "  ".join(
        f"{b.name}←{b.parent or '—'}" for b in g.bones))
    print(f"  带走 {len(g.sockets)} 个槽：" + " ".join(sorted(g.sockets)))
    print(f"  组织预算只够齐长 growth={g.growth():.3f}"
          f"（母体全槽齐长 {C.spread_growth(tuple(C.sockets().values()), C.graft_budget()):.3f}）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
