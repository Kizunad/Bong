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
LOBES: tuple[Lobe, ...] = (
    Lobe("core_mid", (0.0, 0.0, 0.0), (13.0, 10.0, 14.0), 1.00),
    Lobe("core_fore", (0.5, 1.5, -10.5), (10.5, 8.5, 9.5), 0.85, "core_mid"),
    Lobe("core_hind", (-1.2, -0.5, 10.5), (11.5, 9.5, 10.5), 0.90, "core_mid"),
    Lobe("core_sag", (0.5, -7.5, 1.0), (10.5, 5.5, 13.0), 0.75, "core_mid"),
    # —— 以下三处刻意不对称，别"修正" ——
    Lobe("lump_l", (-11.5, 2.5, -2.5), (6.0, 6.0, 7.5), 1.05, "core_mid"),
    Lobe("lump_dorsal", (4.0, 9.5, 4.0), (6.5, 5.5, 7.5), 1.00, "core_mid"),
    Lobe("nodule_r", (10.0, -1.5, 9.5), (4.5, 4.2, 4.6), 0.95, "core_hind"),
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
    if bone in ("lump_l", "lump_dorsal", "nodule_r"):
        return "scar"
    px, py, pz = ix // MAT_PATCH, iy // MAT_PATCH, iz // MAT_PATCH
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


def voxelize() -> list[Voxel]:
    """等值面内的**表层**体素。内部体素直接丢——永远看不见，留着只是拖慢渲染。

    表层判据是 6 邻域有一个在体外。用 6 邻域而不是 26 邻域：26 邻域会把仅在对角
    相邻的薄壁也判成实心内部，等值面细颈处会破洞。
    """
    lo = ((_LC - _LR * 2.2).min(axis=0) / VOX).astype(int) - 1
    hi = ((_LC + _LR * 2.2).max(axis=0) / VOX).astype(int) + 2
    inside: set[tuple[int, int, int]] = set()
    for ix in range(lo[0], hi[0]):
        for iy in range(lo[1], hi[1]):
            for iz in range(lo[2], hi[2]):
                if fld(_voxel_center(ix, iy, iz)) >= ISO:
                    inside.add((ix, iy, iz))
    nb = ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))
    surf: list[tuple[int, int, int]] = []
    for (ix, iy, iz) in sorted(inside):
        if all((ix + a, iy + b, iz + c) in inside for a, b, c in nb):
            continue
        surf.append((ix, iy, iz))

    own = {k: owner(_voxel_center(*k)) for k in surf}
    sset = set(surf)
    out: list[Voxel] = []
    for (ix, iy, iz) in surf:
        p = _voxel_center(ix, iy, iz)
        bone = own[(ix, iy, iz)]
        n = -grad(p)
        n = n / np.linalg.norm(n)
        # 缝：邻域里出现别的 lobe = 这里是两团肉接上的地方。缝的位置不是画上去的，
        # 它和骨骼分段共用同一个 owner() 划分——「哪两块是分别缝进来的」这件事
        # 在场里只有一个答案。
        others = sorted({own[k] for a, b, c in nb
                         if (k := (ix + a, iy + b, iz + c)) in sset and own[k] != bone})
        out.append(Voxel(ix, iy, iz, bone, _material(bone, n, ix, iy, iz), n, tuple(others)))
    return out


def merge_slabs(voxels: list[Voxel]) -> list[Slab]:
    """逐层贪心矩形合并。**只合并 (bone, mat) 相同的格**——跨骨合并会让一块几何
    同时属于两根骨，动画一动就撕开；跨材质合并会把黏液抹到背上。

    先沿 x 拉最长的一条 run，再把这条 run 往 +z 推到不能再推为止。不追求最优分解
    （那是 NP-hard），追求可复现。
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
        while ok(x1 + 1, iy, iz):
            x1 += 1
        z1 = iz
        while all(ok(a, iy, z1 + 1) for a in range(ix, x1 + 1)):
            z1 += 1
        for a in range(ix, x1 + 1):
            for c in range(iz, z1 + 1):
                used.add((a, iy, c))
        out.append(Slab(iy, ix, x1 + 1, iz, z1 + 1, v.bone, v.mat))
    return out


def slab_box(s: Slab) -> tuple[np.ndarray, np.ndarray]:
    """slab → 世界坐标 (from, to)，带确定性表面抖动。

    抖动是必要的而不是装饰：等值面体素化后每一面都是完美平面，渲出来是一堆瓷砖。
    随机胀缩让轮廓有毛刺，才读得出"这是一团肉"而不是"这是个体素球"。抖动只外扩
    不内缩到负体积，且相邻 slab 各自抖各自的 —— 缝隙由重叠掩盖。

    幅度对大 slab 加码：round 1 用统一 ±0.35，大片平坦区域仍渲成整齐的梯田
    （3/4 视图上右侧一整面阶梯状台地）。面积越大越需要打断，所以按跨度线性加权。
    """
    span = max(s.x1 - s.x0, s.z1 - s.z0)
    amp = JITTER_BASE + min(span - 1, 4) * JITTER_PER_SPAN
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
