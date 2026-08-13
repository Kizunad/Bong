#!/usr/bin/env python3
"""异变缝合兽 —— 黏状核心：隐式场 + 体素化 + 挂载点枚举。

正典（worldview.md §七 L735、《异兽三形考》§兽·噬）：缝合兽不是刷出来的怪，是几只
普通野兽在极度饥饿下互相吞噬融合的**残余**。真正活着的只有这团核心；狼首、猪躯、
蛛足全是从尸体上"借"来后花数日重新编织进去的部件。

所以核心不能建成箱子——箱子是"设计过的形状"，而这东西没有被设计过。这里把核心
定义成一团**各向异性高斯和的隐式场**，等值面 f=1 即表皮：

    f(p) = Σ wᵢ · exp(-|(p - cᵢ) / sᵢ|²)        ∇f = Σ wᵢ·exp(-rᵢ²)·(-2)(p-cᵢ)/sᵢ²

四件东西全部从同一个场里掉出来，不另外手摆：

  几何   等值面体素化 → 逐层贪心矩形合并 → **只留表层**（内部方块永远看不见）
  骨骼   每个体素归属于对它 f 贡献最大的正权 lobe —— lobe 就是骨，分段自动
  癒合痕 两个 lobe 在表面交界处即为痕（见 `welds`）—— 与骨骼分段共用同一划分
  挂载点 从核心内部朝候选方位射线，撞 f=1 处即表面点，法向取 -∇f 归一化

「不对称」是硬约束不是风格：`lump_l` 长在左肋、`lump_dorsal` 偏右背、`nodule_r`
在右后——三处刻意破对称。gen_core 的自检**断言核心左右不镜像**，防后来者好心"修正"。

**融合痕不是缝合口**（决策，别改回去）：正典《异兽三形考》§兽·噬 写的是「不是物理
接合——是一层层组织的重新编织……一种余不知名的**塑性再生**」。所以这里没有线、没有
针、没有等距横扣，只有不规则的隆起脊、粗细随机、局部堆成肉瘤、融合彻底处无痕。
"缝合兽"是它的名字，不是它的做法。

单位与朝向沿用体素生物流水线：MC 像素，16 = 1 格，地面 y=0，头朝 -Z，`_l` = 负 x。
"""

from __future__ import annotations

import hashlib
import math
from dataclasses import dataclass, field
from functools import lru_cache

import numpy as np

Vec = tuple[float, float, float]

# ---------------------------------------------------------------- 全局尺度
CORE_CENTER = np.array([0.0, 24.0, 0.0])   # 核心质心（站立姿；腿在其下伸展）
VOX = 2.0                                   # 体素边长
ISO = 1.0                                   # 等值面阈值
MAT_PATCH = 2                               # 材质斑块边长（体素数），见 _material
JITTER_BASE = 0.55                          # 表面抖动基准幅度，见 slab_box
JITTER_PER_SPAN = 0.22                      # 每多跨一格加的抖动
JITTER_MAX = JITTER_BASE + 4 * JITTER_PER_SPAN   # 抖动最大外扩；癒合脊必须抬得比它高
MAX_RUN = 3                                 # 单个 slab 最多跨几格，见 merge_slabs
TEAR_ROUGH = 1.9                            # 断口相对表皮的抖动倍率，见 slab_box


# ---------------------------------------------------------------- 场定义
@dataclass(frozen=True)
class Lobe:
    """一团肉。name 同时是骨名，归属该 lobe 的体素挂到同名骨上。"""

    name: str
    center: Vec          # 相对 CORE_CENTER
    radii: Vec           # 各轴特征半径
    weight: float
    parent: str | None = None   # 骨骼父节点（None = 挂 core）


# 主体三段 + 下垂腹 + 三处破对称的赘生物。
# 半径是"高斯特征半径"不是硬边界：等值面落在 ~1.0-1.2 倍 radii 处，视 weight 而定。
#
# 赘生物的 weight 必须 ≈1.0：低权重（试过 0.4-0.6）的团块撑不出自己的等值面，会被
# 主体吞掉——几何上只鼓一点点，且 owner() 判归 core_mid，等于既看不见也没有独立骨，
# 动画里无法单独搏动。要它是个"东西"，它就得自己够得着 f=1。
# **各团质量必须相当**，不能一团独大。core_mid 曾占全身 45%（390/866），后果有两条：
# ① "几只野兽融成一只"读不出来，只读成"一坨肉带几个疣"；② 爆体时只有主体够格活下去，
# 「不同部位各自逃窜」根本发生不了，健康分裂也找不到可行切面（怎么切都有一半活不下去）。
# 见 fission.py —— 碎片存活线是质量，摊平质量才有多块可存活的碎片。
LOBES: tuple[Lobe, ...] = (
    Lobe("core_mid", (0.0, 0.5, 0.0), (10.0, 8.6, 9.5), 0.94),
    Lobe("core_fore", (0.5, 1.5, -11.5), (11.0, 8.8, 11.0), 0.94, "core_mid"),
    Lobe("core_hind", (-1.2, -0.5, 11.5), (11.5, 9.4, 11.5), 0.94, "core_mid"),
    Lobe("core_sag", (0.5, -7.5, 0.5), (10.5, 6.2, 13.5), 0.82, "core_mid"),
    # —— 以下三处刻意不对称，别"修正" ——
    Lobe("lump_l", (-11.5, 2.5, -2.5), (6.6, 6.6, 8.0), 1.05, "core_mid"),
    Lobe("lump_dorsal", (4.0, 9.5, 4.0), (7.0, 6.0, 8.0), 1.00, "core_mid"),
    Lobe("nodule_r", (10.5, -1.5, 10.0), (5.2, 4.8, 5.2), 0.95, "core_hind"),
)

# 负权凹槽：在两团肉的交界处**刻**一圈沟。
#
# 高斯和永远是凸的——只加正权 lobe，等值面必然鼓成一只圆面包（round 2 实测正视图
# 就是个穹顶）。而自体融合的交界处一定有沟：两团肉挤到一起，接缝处组织下陷，沟上
# 再长出癒合脊。有沟才读得出"这是几团东西挤在一张皮里"，没沟就是一个球。
#
# 形状取压扁的盘（沿交界法向薄、另两轴宽），等于沿该方向切一圈腰。
# 负权 lobe **不参与 owner()**（argmax 只在正权段里取），所以不会抢骨骼归属。
GROOVES: tuple[Lobe, ...] = (
    Lobe("groove_neck", (0.0, 1.5, -6.0), (14.0, 8.0, 2.6), -0.42),
    Lobe("groove_waist", (-0.5, 1.0, 6.5), (14.0, 9.0, 2.8), -0.38),
    Lobe("groove_lump_l", (-9.5, 2.5, -2.5), (2.4, 7.5, 8.5), -0.34),
    Lobe("groove_dorsal", (3.5, 7.5, 4.0), (7.5, 2.4, 8.5), -0.30),
)

_ALL: tuple[Lobe, ...] = LOBES + GROOVES
_LC = np.array([lb.center for lb in _ALL])
_LR = np.array([lb.radii for lb in _ALL])
_LW = np.array([lb.weight for lb in _ALL])
_LN = [lb.name for lb in _ALL]
_NPOS = len(LOBES)


def contributions(p: np.ndarray) -> np.ndarray:
    """每个 lobe（含负权凹槽）在 p 处的贡献（相对 CORE_CENTER 的局部坐标）。"""
    d = (np.asarray(p, float) - _LC) / _LR
    return _LW * np.exp(-np.einsum("ij,ij->i", d, d))


def fld(p: np.ndarray) -> float:
    return float(contributions(p).sum())


def grad(p: np.ndarray, eps: float = 1e-3) -> np.ndarray:
    """解析梯度。∇f 指向场增大方向（朝内），外法向取 -∇f。"""
    p = np.asarray(p, float)
    c = contributions(p)
    g = ((c[:, None] * (-2.0) * (p - _LC) / (_LR ** 2))).sum(axis=0)
    n = np.linalg.norm(g)
    return g if n > eps else np.array([0.0, 1.0, 0.0])


def owner(p: np.ndarray) -> str:
    """p 处贡献最大的**正权** lobe —— 骨骼归属由此派生，不手工划分段。

    argmax 只在正权段里取：负权凹槽是"刻掉的地方"，没有肉，不该拥有几何。
    """
    return _LN[int(np.argmax(contributions(p)[:_NPOS]))]


@lru_cache(maxsize=1)
def solid_grid() -> dict[tuple[int, int, int], str]:
    """**实心**体素 → 所属 lobe。整条流水线的单一真相源。

    质量、质心、表层、接合面积、碎片几何全部从这一张表派生。以前每个量测函数各扫一遍
    网格（`fld` 逐点调用），碎片层要按任意 lobe 子集反复取几何，那种扫法直接不可用。

    向量化一次算完：把整格网的中心堆成 (N,3)，对 11 个 lobe 同时求高斯贡献。归属仍取
    正权段的 argmax，与 `owner()` 同一口径——两处若不一致，骨骼和质量就会各说各话。
    """
    lo = ((_LC - _LR * 2.2).min(axis=0) / VOX).astype(int) - 1
    hi = ((_LC + _LR * 2.2).max(axis=0) / VOX).astype(int) + 2
    ax = [np.arange(lo[i], hi[i]) for i in range(3)]
    idx = np.stack(np.meshgrid(*ax, indexing="ij"), axis=-1).reshape(-1, 3)
    pts = (idx + 0.5) * VOX
    d = (pts[:, None, :] - _LC[None, :, :]) / _LR[None, :, :]
    contrib = _LW[None, :] * np.exp(-np.einsum("ijk,ijk->ij", d, d))
    keep = contrib.sum(axis=1) >= ISO
    own = contrib[:, :_NPOS].argmax(axis=1)
    return {tuple(int(v) for v in k): _LN[int(o)]
            for k, o in zip(idx[keep], own[keep])}


def surface_hit(direction, *, origin=None, lo: float = 0.5, hi: float = 34.0,
                iters: int = 48) -> tuple[np.ndarray, np.ndarray]:
    """从 origin 沿 direction 射线求表面点，返回（局部坐标点, 单位外法向）。

    origin 默认核心中心；头槽/尾槽给一个沿 z 的偏移，让射线从前段/后段射出——统一
    从质心射的话，细长身体的前后端方位角分辨率极差，几个槽会挤成一堆。

    场沿射线单调下降不保证成立（穿过赘生物时会反弹），所以用二分而非牛顿：先粗扫
    找到第一个 f<ISO 的采样区间，再在该区间二分。粗扫步长必须小于最薄赘生物的尺度，
    否则会跳过一层表皮撞到更外面的那层。
    """
    o = np.zeros(3) if origin is None else np.asarray(origin, float)
    u = np.asarray(direction, float)
    u = u / np.linalg.norm(u)
    if fld(o + u * lo) < ISO:
        raise ValueError(f"射线起点已在体外（f={fld(o + u * lo):.3f}）；起点必须在肉里")
    span = None
    prev_t = lo
    steps = int((hi - lo) / 0.4)
    for i in range(1, steps + 1):
        t = lo + (hi - lo) * i / steps
        if fld(o + u * t) < ISO:
            span = (prev_t, t)
            break
        prev_t = t
    if span is None:
        raise ValueError(f"射线 {direction} 在 {hi} 单位内未穿出等值面")
    a, b = span
    for _ in range(iters):
        m = 0.5 * (a + b)
        if fld(o + u * m) >= ISO:
            a = m
        else:
            b = m
    pt = o + u * (0.5 * (a + b))
    n = -grad(pt)
    return pt, n / np.linalg.norm(n)


# ---------------------------------------------------------------- 体素化
@dataclass
class Voxel:
    ix: int
    iy: int
    iz: int
    bone: str
    mat: str
    normal: np.ndarray = field(default_factory=lambda: np.array([0.0, 1.0, 0.0]))
    seam: tuple[str, ...] = ()      # 邻域里出现的其它 lobe；非空 = 这格在缝上
    torn: bool = False              # 这一面是撕开的创面，不是原本的皮（见 surface_of）


@dataclass
class Slab:
    """一层内合并出来的长方块（体素坐标，半开区间）。"""

    iy: int
    x0: int
    x1: int
    z0: int
    z1: int
    bone: str
    mat: str


def _noise(*parts) -> float:
    """确定性噪声 ∈ [0,1)。同一坐标每次跑必得同一值——渲染出的瑕疵要可复现才能修。"""
    h = hashlib.md5("|".join(str(p) for p in parts).encode()).digest()
    return int.from_bytes(h[:4], "big") / 2**32


def _voxel_center(ix: int, iy: int, iz: int) -> np.ndarray:
    return np.array([(ix + 0.5) * VOX, (iy + 0.5) * VOX, (iz + 0.5) * VOX])


def _material(bone: str, normal: np.ndarray, ix: int, iy: int, iz: int) -> str:
    """表层材质。核心没有"设计过的配色"——只有三件事看得出来：
    肉、被撑薄到透光的膜、以及淌不掉的黏液。

    噪声按 MAT_PATCH 大小的**斑块**取，不逐体素取：逐体素等于给每格掷一次骰子，
    相邻格几乎不同材质，既渲成一身彩色噪点，又让 merge_slabs 完全合并不动
    （实测合并率从 1.66:1 掉不下去）。斑块化同时解决观感和块数两件事。

    黏液按**朝下的面**判，不按高度判：`p[1] < 阈值` 会在肚子上切出一条笔直的水平
    色带（round 1 实测：渲出来像站在一滩藻里）。黏液是顺着重力挂在下垂面上的，
    该出现在每一处朝下的凸缘，不该出现在同高度的侧壁。
    """
    px, py, pz = ix // MAT_PATCH, iy // MAT_PATCH, iz // MAT_PATCH
    if bone in ("lump_l", "lump_dorsal", "nodule_r"):
        # 赘生物**不能整块刷同一个色**：round 2 三处赘生物全是均匀 tan，从后方看
        # lump_dorsal 就是个平顶木箱。混进本体的肉色/暗色才读成肉，而不是贴的板。
        s = _noise(px, py, pz, "s")
        return "scar" if s < 0.58 else ("flesh_deep" if s < 0.78 else "flesh_pale")
    # 门槛压在 0.55 而不是 0.30：椭球的整个下半面法向 y 都 < -0.3，用 0.30 等于把
    # 下半身整个刷绿（round 2 实测像半只兽泡在藻里）。黏液只挂在**明显朝下**的凸缘。
    down = float(-normal[1])                       # +1 = 完全朝下
    if down > 0.55 and _noise(px, py, pz, "b") < (down - 0.55) * 1.3:
        return "bile"
    n = _noise(px, py, pz, "m")
    if n < 0.20:                         # 撑薄处透出底下的暗色
        return "membrane"
    if n < 0.30:
        return "vein"
    return "flesh_pale"


NB = ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))


def surface_of(lobes: tuple[str, ...] | None = None) -> list[Voxel]:
    """给定 lobe 子集的**表层**体素；`lobes=None` 即整只兽。

    表层判据是 6 邻域有一个不在集合里。用 6 邻域而不是 26 邻域：26 邻域会把仅在对角
    相邻的薄壁也判成实心内部，等值面细颈处会破洞。

    **撕裂面是从这里掉出来的**，不用另外画：整只兽身上被埋在内部的体素，在子集里
    就成了表面。`v.torn` 记录"缺失的那个邻居在整只兽里是实心的"——是则这一面是撕开的
    创面。少了另一半，创面就自动出现，这是推论不是美术。

    创面的法向也不能用场梯度：场在体内是连续的，撕裂处的 -∇f 指向随便哪个方向。创面
    的外法向就是**缺失邻居的方向**——肉原来在那边，现在没了。
    """
    solid = solid_grid()
    keep = solid if lobes is None else {k: v for k, v in solid.items() if v in lobes}
    shell: dict[tuple[int, int, int], list] = {}
    for k in sorted(keep):
        miss = [(k[0] + a, k[1] + b, k[2] + c) for a, b, c in NB
                if (k[0] + a, k[1] + b, k[2] + c) not in keep]
        if miss:
            shell[k] = miss

    out: list[Voxel] = []
    for k, miss in shell.items():
        ix, iy, iz = k
        bone = keep[k]
        torn = any(m in solid for m in miss)
        if torn:
            n = np.array([sum(m[i] for m in miss) - len(miss) * k[i] for i in range(3)], float)
        else:
            n = -grad(_voxel_center(ix, iy, iz))
        n = n / max(float(np.linalg.norm(n)), 1e-9)
        # 缝：邻域里出现别的 lobe = 这里是两团肉接上的地方。缝的位置不是画上去的，
        # 它和骨骼分段共用同一个 owner() 划分——「哪两块是分别缝进来的」这件事
        # 在场里只有一个答案。撕走的那半的缝自动消失：邻居不在集合里就不算缝，
        # 那里现在是创面。
        #
        # 邻域**只查表层**（shell）不查实心：癒合痕是两块肉的皮在表面相接的那一圈，
        # 而埋在皮下的 A/B 界面既看不见、也不该记进痕。查实心会把皮下界面一并算上，
        # 实测痕从 58 段涨到 95 段，`weld_areas` 的接合强度表跟着变——碎裂图谱和
        # 最小割全部改口径。这条别改回去。
        others = sorted({keep[j] for a, b, c in NB
                         if (j := (ix + a, iy + b, iz + c)) in shell and keep[j] != bone})
        mat = _torn_material(ix, iy, iz) if torn else _material(bone, n, ix, iy, iz)
        out.append(Voxel(ix, iy, iz, bone, mat, n, tuple(others), torn))
    return out


@lru_cache(maxsize=1)
def _voxelize_cached() -> tuple[Voxel, ...]:
    return tuple(surface_of(None))


def voxelize() -> list[Voxel]:
    """整只兽的表层体素。"""
    return list(_voxelize_cached())


def _torn_material(ix: int, iy: int, iz: int) -> str:
    """创面材质。撕开的断口露的是**内部**组织，和被环境糟蹋过的外皮不是一回事：
    没有黏液（黏液是挂在外面的）、没有膜（膜是被撑薄的表皮），只有湿的深色肉，
    偶尔一段扯断的血管。所以它单独一张材质表，不复用 `_material`。
    """
    n = _noise(ix // MAT_PATCH, iy // MAT_PATCH, iz // MAT_PATCH, "t")
    return "torn_vein" if n < 0.22 else ("torn_deep" if n < 0.62 else "torn")


def merge_slabs(voxels: list[Voxel]) -> list[Slab]:
    """逐层贪心矩形合并。**只合并 (bone, mat) 相同的格**——跨骨合并会让一块几何
    同时属于两根骨，动画一动就撕开；跨材质合并会把黏液抹到背上。

    先沿 x 拉最长的一条 run，再把这条 run 往 +z 推到不能再推为止。不追求最优分解
    （那是 NP-hard），追求可复现。

    跨度硬上限 MAX_RUN：不限的话大面积同材质区会合并成一整张平板，抖动幅度相对
    跨度太小压不住，渲出来是梯田和平顶木箱（round 2 顶视图/后视图实测）。限制跨度
    等于从源头上不产生大平面，代价是块数上升——这个代价值得付。
    """
    grid = {(v.ix, v.iy, v.iz): v for v in voxels}
    used: set[tuple[int, int, int]] = set()
    out: list[Slab] = []
    for key in sorted(grid, key=lambda k: (k[1], k[2], k[0])):
        if key in used:
            continue
        ix, iy, iz = key
        v = grid[key]
        tag = (v.bone, v.mat)

        def ok(a: int, b: int, c: int) -> bool:
            u = grid.get((a, b, c))
            return u is not None and (u.bone, u.mat) == tag and (a, b, c) not in used

        x1 = ix
        while x1 - ix + 1 < MAX_RUN and ok(x1 + 1, iy, iz):
            x1 += 1
        z1 = iz
        while z1 - iz + 1 < MAX_RUN and all(ok(a, iy, z1 + 1) for a in range(ix, x1 + 1)):
            z1 += 1
        for a in range(ix, x1 + 1):
            for c in range(iz, z1 + 1):
                used.add((a, iy, c))
        out.append(Slab(iy, ix, x1 + 1, iz, z1 + 1, v.bone, v.mat))
    return out


def slab_box(s: Slab, rough: float = 1.0) -> tuple[np.ndarray, np.ndarray]:
    """slab → 世界坐标 (from, to)，带确定性表面抖动。

    抖动是必要的而不是装饰：等值面体素化后每一面都是完美平面，渲出来是一堆瓷砖。
    随机胀缩让轮廓有毛刺，才读得出"这是一团肉"而不是"这是个体素球"。抖动只外扩
    不内缩到负体积，且相邻 slab 各自抖各自的 —— 缝隙由重叠掩盖。

    幅度对大 slab 加码：round 1 用统一 ±0.35，大片平坦区域仍渲成整齐的梯田
    （3/4 视图上右侧一整面阶梯状台地）。面积越大越需要打断，所以按跨度线性加权。

    `rough` 给**小个体**用。抖动是相对尺度的表面扰动：同一个绝对幅度，在 31px 长的
    母体上是肉的纹理，在 15px 长的碎片上就是把它砸成一堆碎石（round 2 出图实测，
    整只读成叠起来的方块而不是一团肉）。所以调用方按"自己有多大"缩这个幅度。
    """
    span = max(s.x1 - s.x0, s.z1 - s.z0)
    amp = (JITTER_BASE + min(span - 1, 4) * JITTER_PER_SPAN) * rough
    if s.mat.startswith("torn"):
        # 断口比皮**毛糙**。皮是长出来的，长的过程会把表面抹平；断口是扯出来的，
        # 裂纹走的是组织里最弱的那条路，本来就参差。用同一套抖动幅度渲出来是一堵
        # 平整的红墙，读成"被刀切过"而不是"被撕开"。
        amp *= TEAR_ROUGH
    j = [(_noise(s.iy, s.x0, s.z0, s.bone, k) - 0.3) * amp for k in range(6)]
    frm = np.array([s.x0 * VOX - j[0], s.iy * VOX - j[1], s.z0 * VOX - j[2]])
    to = np.array([s.x1 * VOX + j[3], (s.iy + 1) * VOX + j[4], s.z1 * VOX + j[5]])
    return frm + CORE_CENTER, to + CORE_CENTER


@dataclass
class Weld:
    """一段**癒合痕**：两团组织自己长到一起留下的隆起。

    不是外科缝合。正典写得很明白（《异兽三形考》§兽·噬）：「不是物理接合——是一层层
    组织的重新编织……这是一种余不知名的**塑性再生**」。所以这里没有线、没有针、没有
    等距的横扣——那是人造缝合口的读法。自体融合留下的是：不规则的隆起脊、粗细随机、
    局部堆成瘤、以及两侧本来就不同的组织颜色。

    pos 表皮点，normal 外法向，tangent 沿癒合痕走向；radius 该段的隆起粗细；
    bulge 表示这一段融合失控堆成了肉瘤。
    """

    pos: np.ndarray
    normal: np.ndarray
    tangent: np.ndarray
    radius: float
    bulge: bool
    bone: str = "core_mid"


def welds(voxels: list[Voxel] | None = None, *, step: int = 1) -> list[Weld]:
    """癒合痕：两个 lobe 在表面交界处的那一圈。

    位置不是画上去的——缝即 lobe 边界，和骨骼分段共用同一个 owner() 划分：「哪两块
    是分别长进来的」这件事在场里只有一个答案。

    走向由邻格连线估计：把同为交界格的 6 邻域方向平均后投影到切平面即局部切向。
    孤立格退化为法向的任一正交方向，不影响观感。

    粗细逐段随机、且有一成几率**断开**（融合彻底的地方看不出痕）、一成几率堆成瘤。
    等粗等距的脊会立刻读成人造缝合口——不规则本身就是"自己长的"的证据。

    step 必须是 1：先试过隔格取（step=2）+ 粗段，相邻段接不上，渲出来是一堆散落的
    暗红方块而不是一条脊。要读成"线"，段就得细、且与邻段重叠——细而密胜过粗而疏。
    """
    vs = voxels if voxels is not None else voxelize()
    seam = [v for v in vs if v.seam]
    sset = {(v.ix, v.iy, v.iz) for v in seam}
    nb = ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))
    out: list[Weld] = []
    for i, v in enumerate(sorted(seam, key=lambda u: (u.iy, u.iz, u.ix))):
        if i % step:
            continue
        key = (v.ix, v.iy, v.iz)
        if _noise(*key, "gap") < 0.12:          # 融合彻底处：无痕
            continue
        acc = np.zeros(3)
        for a, b, c in nb:
            if (v.ix + a, v.iy + b, v.iz + c) in sset:
                acc += np.array([a, b, c], float)
        t = acc - v.normal * float(np.dot(acc, v.normal))    # 投影到切平面
        if np.linalg.norm(t) < 1e-6:
            ref = np.array([0.0, 1.0, 0.0]) if abs(v.normal[1]) < 0.9 else np.array([1.0, 0.0, 0.0])
            t = np.cross(v.normal, ref)
        t = t / np.linalg.norm(t)
        bulge = _noise(*key, "bulge") < 0.14
        r = 0.30 + _noise(*key, "r") * 0.32 + (0.38 if bulge else 0.0)
        out.append(Weld(_voxel_center(*key) + CORE_CENTER, v.normal, t, r, bulge, v.bone))
    return out


# ---------------------------------------------------------------- 挂载点
@dataclass
class Socket:
    """核心表面的一处缝合位。

    genome 往上挂什么由基因组决定；本模块只负责回答"这个方位的表皮在哪、朝哪、
    这块地方能承多粗的部件"。
    """

    name: str
    kind: str            # limb / head / vestige
    pos: np.ndarray      # 世界坐标
    normal: np.ndarray   # 单位外法向
    bone: str            # 骑在哪根核心骨上
    girth: float         # 可挂部件的最大根部半径
    azimuth: float = 0.0
    elevation: float = 0.0
    meta: dict = field(default_factory=dict)


def _dir(az_deg: float, el_deg: float) -> np.ndarray:
    """方位角 az（0=正前 -Z，+ 向右 +X）+ 仰角 el → 单位方向。"""
    az, el = math.radians(az_deg), math.radians(el_deg)
    return np.array([math.sin(az) * math.cos(el), math.sin(el), -math.cos(az) * math.cos(el)])


def _girth(pt: np.ndarray, n: np.ndarray, origin: np.ndarray) -> float:
    """局部平坦度 → 能承多粗的根部。

    在切平面上取 8 个方向、各走 3.0 单位，看表面偏离切平面多少。偏得越少越平，
    越平能焊上去的部件根部越粗。凸起的小赘生物上焊不了一条大腿——这条约束不是
    美学，是"焊上去会穿模"的几何事实。
    """
    ref = np.array([0.0, 1.0, 0.0]) if abs(n[1]) < 0.9 else np.array([1.0, 0.0, 0.0])
    t1 = np.cross(n, ref)
    t1 /= np.linalg.norm(t1)
    t2 = np.cross(n, t1)
    dev = 0.0
    for k in range(8):
        a = 2 * math.pi * k / 8
        probe = pt + (math.cos(a) * t1 + math.sin(a) * t2) * 3.0
        try:
            hit, _ = surface_hit(probe - origin, origin=origin)
        except ValueError:
            return 1.5
        dev = max(dev, abs(float(np.dot(hit - pt, n))))
    return float(np.clip(4.2 - dev * 0.55, 1.4, 4.2))


# 候选方位表。腿槽压在下半身（承重）、头槽集中在前段（但**故意留一个背上的**）、
# 退化槽散在背侧——那是吞下去没长完的东西留下的疤。
#
# 第四列 origin_z 是射线起点沿 z 的偏移：前段的槽从前段射、后段的槽从后段射。
# 全从质心射的话细长身体两端方位分辨率极差，实测背侧四个槽会落进同一小块表皮。
_SLOTS: tuple[tuple[str, str, float, float, float], ...] = (
    # (name, kind, azimuth, elevation, origin_z)
    ("limb_fl", "limb", -58.0, -30.0, -6.0),
    ("limb_fr", "limb", 56.0, -28.0, -6.0),
    ("limb_ml", "limb", -96.0, -22.0, 0.0),
    ("limb_mr", "limb", 96.0, -18.0, 0.0),
    ("limb_hl", "limb", -122.0, -28.0, 6.0),
    ("limb_hr", "limb", 124.0, -26.0, 6.0),
    ("limb_vl", "limb", -66.0, -70.0, -2.0),     # 腹侧：从肚子底下长出来的
    ("limb_vr", "limb", 72.0, -68.0, 4.0),
    ("head_c", "head", 0.0, 4.0, -8.0),
    ("head_l", "head", -40.0, 18.0, -8.0),
    ("head_r", "head", 38.0, -4.0, -8.0),
    ("head_low", "head", -10.0, -34.0, -8.0),    # 从胸口垂下来的那个
    ("head_dorsal", "head", 34.0, 74.0, 2.0),    # 背上的。留着，它就是要不合理
    ("vest_dl", "vestige", -74.0, 52.0, -4.0),
    ("vest_dr", "vestige", 88.0, 40.0, 8.0),
    ("vest_tail", "vestige", 172.0, 14.0, 8.0),
    ("vest_nape", "vestige", -18.0, 60.0, -9.0),
)


def sockets() -> dict[str, Socket]:
    """全部候选挂载点。基因组只从这里选，不自己造坐标。"""
    out: dict[str, Socket] = {}
    for name, kind, az, el, oz in _SLOTS:
        o = np.array([0.0, 0.0, oz])
        pt, n = surface_hit(_dir(az, el), origin=o)
        out[name] = Socket(
            name=name,
            kind=kind,
            pos=pt + CORE_CENTER,
            normal=n,
            bone=owner(pt),
            girth=_girth(pt, n, o),
            azimuth=az,
            elevation=el,
        )
    return out


# ---------------------------------------------------------------- 量测
def core_bounds() -> tuple[np.ndarray, np.ndarray]:
    lo = np.array([1e9] * 3)
    hi = np.array([-1e9] * 3)
    for s in merge_slabs(voxelize()):
        a, b = slab_box(s)
        lo = np.minimum(lo, a)
        hi = np.maximum(hi, b)
    return lo, hi


def weld_areas(voxels: tuple[Voxel, ...] | None = None) -> dict[tuple[str, str], int]:
    """每对 lobe 之间的**接合面积**（交界体素数）。

    这就是那道癒合痕的强度：两团肉长在一起的接触面越大，扯开它需要的力越大。裂开时
    从哪儿裂**不需要画**——面积最小的那道接合先断。分裂 / 爆体 / 掉块全部由这张表推出
    （见 fission.py）。

    键统一排序，(a,b) 与 (b,a) 归并成一条。
    """
    vs = voxels if voxels is not None else _voxelize_cached()
    out: dict[tuple[str, str], int] = {}
    for v in vs:
        for other in v.seam:
            key = (v.bone, other) if v.bone < other else (other, v.bone)
            out[key] = out.get(key, 0) + 1
    return out


@lru_cache(maxsize=1)
def lobe_mass() -> dict[str, int]:
    """每个 lobe 的**实心**体素数——碎片能不能活下去看的是这个，不是表层面积。"""
    out: dict[str, int] = {}
    for n in solid_grid().values():
        out[n] = out.get(n, 0) + 1
    return out


@lru_cache(maxsize=1)
def lobe_centroid() -> dict[str, np.ndarray]:
    """每个 lobe 的实心质心（世界坐标）。爆开时碎片的飞出方向由此定。"""
    acc: dict[str, list] = {}
    for k, n in solid_grid().items():
        a = acc.setdefault(n, [np.zeros(3), 0])
        a[0] += _voxel_center(*k)
        a[1] += 1
    return {n: a[0] / a[1] + CORE_CENTER for n, a in acc.items()}


def group_mass(lobes: tuple[str, ...]) -> int:
    """一组 lobe 的实心体素数。"""
    m = lobe_mass()
    return sum(m.get(n, 0) for n in lobes)


def group_centroid(lobes: tuple[str, ...]) -> np.ndarray:
    """一组 lobe 的实心质心（世界坐标）。"""
    m, c = lobe_mass(), lobe_centroid()
    tot = group_mass(lobes)
    if tot <= 0:
        raise ValueError(f"lobe 组 {lobes} 没有实心体素")
    return sum(c[n] * m[n] for n in lobes if n in m) / tot


# ---------------------------------------------------------------- 芽（嫁接前体）
def bud_shape(sock: Socket, growth: float) -> list[tuple[np.ndarray, float, str]]:
    """挂载点上的**芽**：还没长成部件的那团未分化组织。返回 [(中心, 半径, 材质)]。

    正典（《异兽三形考》§兽·噬）：那头幼兽的第四条腿是从野狗尸体上"借"的，花了约七日
    才长进去——"一层层组织的重新编织"。所以部件不是瞬间出现的，中间有一段它只是一个
    鼓包的时期，这个时期得看得见。

    growth=0 也**不是没有东西**：挂载点本身就是皮上一处微隆，是这具身体"这里可以再长
    点什么"的记号。0 → 1 的过程是这团东西沿法向鼓出、变粗、末端开始分叉。

    分节而不是一个球：单球放大只是"变大"，多节沿法向排开且逐节收细，才读得出
    **方向性**——它在朝外长，而不是在肿。

    节数与收细曲线（四节、末节收到根部的 0.28）是**为"够得着"服务的**，不是为"变大"：
    这团组织的用处是往外试探，细长才有意义。先做过三节、收到 0.50 的粗胖版本，
    乱抽动画渲出来每条都读成贴在皮上的鳞片或木片，不是触手（round 2 侧视实测）。
    长细比从 2.7:1 提到 3.4:1、末节半径减半之后才读得出"一根"。

    嫁接时它照样是芽——粗细由 `growth` 从微隆一路长到挂载面满径，只是长的过程里
    始终是有指向的一条，而不是一个瘤。
    """
    g = float(np.clip(growth, 0.0, 1.0))
    r0 = 0.8 + (sock.girth - 0.8) * g          # 根部：从微隆长到挂载面满径
    out = []
    for k, (frac, shrink, mat) in enumerate(((0.0, 1.00, "bud_base"),
                                             (0.62, 0.70, "bud_mid"),
                                             (1.20, 0.46, "bud_mid"),
                                             (1.72, 0.28, "bud_tip"))):
        if k > 0 and g < 0.18:
            break                               # 太早，还只是一处微隆，没有节
        d = r0 * frac * (0.45 + 1.35 * g)
        out.append((sock.pos + sock.normal * d, r0 * shrink * (0.55 + 0.45 * g), mat))
    return out


_TISSUE_STEP = 0.35        # 芽体积数值积分的采样步长（px）
_TISSUE_CACHE: dict[tuple[str, float], float] = {}


def bud_tissue(sock: Socket, growth: float) -> float:
    """一条芽在给定生长度下的**新增组织体积**（px³）。

    不能拿三个节点的包围盒体积相加：三节沿法向排开是**互相重叠**的，而且根节大半
    埋在体内——那部分是本来就有的肉，不是新长出来的。两项误差同向叠加，按包围盒
    算会把用料高估到两倍以上。

    这个数字有实际后果：芽的组织不是凭空出现的，得有来源。见 `graft_budget`。

    结果按 (槽名, 生长度) 记账：这是数值积分，而嫁接时长、组织预算、乱抽缩放三处都要
    反复问同一批槽。不记的话每 import 一次 core_anim 就要白算三四十遍。
    """
    key = (sock.name, round(float(growth), 6))
    hit = _TISSUE_CACHE.get(key)
    if hit is not None:
        return hit
    nodes = bud_shape(sock, growth)
    if not nodes:
        return 0.0
    lo = np.min([c - r for c, r, _ in nodes], axis=0)
    hi = np.max([c + r for c, r, _ in nodes], axis=0)
    ax = [np.arange(lo[i], hi[i] + _TISSUE_STEP, _TISSUE_STEP) for i in range(3)]
    pts = np.stack(np.meshgrid(*ax, indexing="ij"), axis=-1).reshape(-1, 3)
    inside = np.zeros(len(pts), bool)
    for c, r, _m in nodes:
        inside |= np.all(np.abs(pts - c) <= r, axis=1)
    pts = pts[inside]
    if not len(pts):
        _TISSUE_CACHE[key] = 0.0
        return 0.0
    d = (pts[:, None, :] - CORE_CENTER - _LC[None, :, :]) / _LR[None, :, :]
    f = (_LW[None, :] * np.exp(-np.einsum("ijk,ijk->ij", d, d))).sum(axis=1)
    _TISSUE_CACHE[key] = float((f < ISO).sum()) * _TISSUE_STEP ** 3
    return _TISSUE_CACHE[key]


def _tangent_basis(n: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    ref = np.array([0.0, 1.0, 0.0]) if abs(n[1]) < 0.9 else np.array([1.0, 0.0, 0.0])
    t1 = np.cross(n, ref)
    t1 /= np.linalg.norm(t1)
    return t1, np.cross(n, t1)


def axis_angle(axis: np.ndarray, rad: float) -> np.ndarray:
    """轴角 → 旋转矩阵（Rodrigues）。"""
    a = np.asarray(axis, float)
    a = a / max(float(np.linalg.norm(a)), 1e-9)
    K = np.array([[0, -a[2], a[1]], [a[2], 0, -a[0]], [-a[1], a[0], 0]])
    return np.eye(3) + math.sin(rad) * K + (1 - math.cos(rad)) * (K @ K)


# 芽能摆多大角度：**由芽自己决定，与它长在哪儿无关**。
#
# 本来想按"周围哪边最空"逐槽推——芽往一侧倒时尖端会朝表皮压过去，凸出去的地方
# 倒得开、凹进去的地方一倒就顶进肉里。写完实测：17 个槽在满长、七成、短茬三档下
# **全部**顶到 78° 的折返上限，一个受表皮限制的都没有。原因是几何上必然的：核心
# 整体是凸的，而挂载点的法向朝外，绕根部转不到 90° 的方向都在切平面外侧。
#
# 所以这里没有可推的东西，留一个诚实的常数：茬能倒到接近贴皮，再多就是往回折了。
# 别再写一个逐槽的版本——那只会算出 17 个一样的数，看着像推导，其实是常数。
BUD_FOLD_DEG = 78.0

@lru_cache(maxsize=1)
def graft_budget() -> float:
    """一次能摊出去的未分化组织总量（px³）= **一条芽长满所需的料**。

    正典里嫁接是**一次一条**、每条要七日（《异兽三形考》§兽·噬）。把这句话当成
    对组织存量的量测：它同时能持有的未分化组织，就是一条部件的量。这不是拍出来的
    比例，是从"一次一条"反推的——若存量足够两条，正典就不会写一次一条。

    推论：想让每个挂载点同时冒芽（见 core_anim 的乱抽），同一份料得摊到 17 个槽上，
    于是每个都只能是**短茬**。全身长满触手这件事，它负担不起。
    """
    return max(bud_tissue(s, 1.0) for s in sockets().values())


def spread_growth(socks: tuple[Socket, ...], budget: float) -> float:
    """把 budget 的组织**均摊**到这些槽上时，每条芽能长到的生长度。

    总用料对 growth 单调递增，直接二分。解不出（预算比最小形态还少）时返回 0。
    """
    if not socks:
        return 0.0
    lo, hi = 0.0, 1.0
    if sum(bud_tissue(s, 1.0) for s in socks) <= budget:
        return 1.0
    for _ in range(28):
        mid = 0.5 * (lo + hi)
        if sum(bud_tissue(s, mid) for s in socks) < budget:
            lo = mid
        else:
            hi = mid
    return lo        # 取下界而非中点：中点可能落在预算之上，超支 3% 也是超支


def spread_scale(socks: tuple[Socket, ...], budget: float) -> float:
    """同一份预算摊到这些槽上时，动画该把芽骨**缩放**到多少。

    和 `spread_growth` 是同一件事的两种参数化，用在两个层：几何层调的是 `bud_shape`
    的 growth（节距与半径各按自己的式子变），动画层能调的只有骨骼的**等比缩放**。
    等比缩放下体积按 s³ 走，于是有闭式解 s = (预算 / 满长总用料)^(1/3)，不用二分。

    两个数不相等是对的——它们是同一个预算在两套形变下的解。
    """
    total = sum(bud_tissue(s, 1.0) for s in socks)
    if total <= 0:
        return 0.0
    return min(1.0, (budget / total) ** (1.0 / 3.0))


@lru_cache(maxsize=1)
def centroid() -> np.ndarray:
    """核心本体的**体积质心**（世界坐标）。

    步态稳定性算的是"质心投影在不在支撑多边形里"，所以这个点必须是真的、不对称的。
    直接拿 CORE_CENTER 顶替会把稳定性问题做成假的——核心本身就是偏的（左肋一个瘤、
    右后一个结节），质心不在中轴上，这正是随机肢体配置站不站得住的关键。

    体素按等值面表层取，但质心要算**实心**体积——表层壳的质心会被薄壁处的面积权重
    带偏。这里用体素占据集直接算，不走 merge_slabs。
    """
    grid = solid_grid()
    if not grid:
        raise ValueError("等值面内没有体素——场参数坏了")
    return np.mean([_voxel_center(*k) for k in grid], axis=0) + CORE_CENTER


def asymmetry() -> float:
    """左右不对称度：镜像后仍在体外的表层体素占比。

    缝合兽的核心**必须**不对称——它是吞噬堆积出来的，不是长出来的。这个数字被
    gen_core 的自检卡下限，防后来者"顺手把它修对称"。
    """
    vs = voxelize()
    bad = 0
    for v in vs:
        p = _voxel_center(v.ix, v.iy, v.iz)
        if fld(np.array([-p[0], p[1], p[2]])) < ISO:
            bad += 1
    return bad / max(1, len(vs))


if __name__ == "__main__":
    vs = voxelize()
    sl = merge_slabs(vs)
    lo, hi = core_bounds()
    print(f"表层体素 {len(vs)} → 合并后 {len(sl)} 块")
    print(f"包围盒 x {lo[0]:+.1f}..{hi[0]:+.1f}  y {lo[1]:+.1f}..{hi[1]:+.1f}  "
          f"z {lo[2]:+.1f}..{hi[2]:+.1f}")
    print(f"尺寸 {(hi[0] - lo[0]) / 16:.2f} × {(hi[1] - lo[1]) / 16:.2f} × "
          f"{(hi[2] - lo[2]) / 16:.2f} 格")
    print(f"不对称度 {asymmetry() * 100:.1f}%")
    per_bone: dict[str, int] = {}
    for s in sl:
        per_bone[s.bone] = per_bone.get(s.bone, 0) + 1
    print("每骨块数：" + "  ".join(f"{k}={v}" for k, v in sorted(per_bone.items())))
    print("\n挂载点：")
    for s in sockets().values():
        print(f"  {s.name:<12} {s.kind:<8} bone={s.bone:<12} "
              f"pos=({s.pos[0]:+6.1f},{s.pos[1]:+6.1f},{s.pos[2]:+6.1f}) "
              f"n=({s.normal[0]:+.2f},{s.normal[1]:+.2f},{s.normal[2]:+.2f}) "
              f"girth={s.girth:.2f}")
