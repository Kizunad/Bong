#!/usr/bin/env python3
"""异变缝合兽 —— 运动层：由肢体几何**推导**步态，不手调。

缝合兽的肢体来自不同的兽，长短不一。这一层的全部内容是回答一个问题：**一堆长短
不一、位置乱七八糟的腿，怎么把这团肉挪走？**

## 一、肢体是复摆，摆动频率由几何定死

绕髋摆动的自然频率（复摆）：

    f = (1/2π)·√(M·g·D / I)

M 总质量、D 质心到髋的距离、I 绕髋转动惯量。每节按均匀杆算（质量 ∝ 长度）：

    I = Σ [mᵢlᵢ²/12 + mᵢ(dᵢ + lᵢ/2)²]      M = Σ mᵢ      D = Σ mᵢ(dᵢ + lᵢ/2) / M

均匀单杆退化为 f = (1/2π)√(3g/2L)，即 **f ∝ 1/√L**：长肢摆得慢，短肢摆得快。
这不是风格选择，是物理。

## 二、一只兽只有一个速度 → 错拍是推论不是设计

整只兽只能有一个前进速度 v。若每条肢都按自己的自然频率摆，则一个身体步态周期内
它必须走**整数步**：

    n_i = round(f_i / f_body)     f_body := 最慢承重肢的自然频率

短腿一周期迈三步，长腿迈一步。**缝合兽走不出同步步态是这里推出来的**，不是为了
"看起来怪"而故意错开的。它天生就没法齐步走。

但 n_i 不能只由频率定，还要够得上速度：给定身体速度 v，第 i 条肢每步得跨
s_i = v/(n_i·f_body)，而它最多只能跨 s_i^max（由可达半径给出）。所以

    n_i = max( round(f_i/f_body),  ⌈v / (f_body · s_i^max)⌉ )

即**跟不上的短腿会碎步**——提高自己的步频去追身体，而不是拖住全身。速度取各肢
"自己舒服的速度"的中位数（不是最小值：取最小值等于让蹲伏后只剩 3px 可达的那条
残腿锁死整只兽，实测把速度压到 0.1 格/s，追不上任何东西）。碎步超过 MAX_STEPS
仍跟不上的肢判为**拖行**：不再提供支撑，也不再参与限速。

## 三、骑乘高度是解出来的，不是定死的

身体离地多高**不能预设**。撑得高则短肢够不着地、被踢出承重集；蹲得低则长肢折成一团、
可达半径缩水。而"短肢一周期迈两步"正是错拍的来源——如果高度写死，凡是短到能迈两步的
肢都够不着地，错拍会自我抵消，永远只剩齐步（实测：一条 12.9 长的鼠腿在 y=14 的髋位
下 reach=0，直接掉出步态表）。

所以高度是自变量：扫一遍 Δy，取"能参与承重的肢最多、且各肢伸展度都离极限最远"的那个。
这也顺带给出这只兽的**站姿**——蹲伏还是撑高，是它的肢体配置决定的，不是美术选的。

## 四、相位由静态稳定解出来

肢体位置是基因组随机挑的，可能一侧三条另一侧一条。什么时候抬哪条腿不能拍脑袋——
必须保证任一时刻：① 至少 3 只脚着地 ② 质心的地面投影落在支撑多边形**内部**且留
余量。相位就是对这个约束求解出来的，解不出来的基因组**站不住**，应当重采样。

质心取核心本体的体积质心（不对称的，见 core.centroid）——这让稳定性问题是真的。

用法:
  python3 modelScript/creatures/stitched_beast/locomotion.py           # 若干 seed 的步态报告
  python3 modelScript/creatures/stitched_beast/locomotion.py --seed 7
"""

from __future__ import annotations

import argparse
import math
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import genome as GN  # noqa: E402

G_PX = 9.81 * 16.0        # 重力，MC 像素/s²（16 px = 1 m）
EXTEND = 0.85             # 站立时肢体最多伸到全长的几成——伸直的腿没有缓冲，也解不出逆解
SWING = 0.75              # 步幅占可达直径的几成
MIN_SUPPORT = 3           # 任一时刻最少着地脚数
COMFORT = 0.72            # 舒适伸展度：髋高 / 有效肢长的目标值（0=完全折起，1=绷直）
MAX_STEPS = 6             # 单肢每身体周期最多迈几步；再多也跟不上就判拖行
RUN_TEMPO = 2.2           # 奔跑相对自然摆频的倍率（肌肉强驱，越界即耗真元）
RIDE_CLEAR = 3.0          # 骑乘高度下限留给髋的离地余量；上限见 RIDE_UP
RIDE_UP = 6.0             # 骑乘高度上限（相对核心设计高度）
SAMPLES = 240             # 支撑多边形余量的采样点数（每点要跑一次凸包，不能太密）。
                          # 240 而不是 48：一只脚抬起时多边形会突然缩小，而缩小的窗口
                          # 完全可能整个落在格点之间——48 点下相位搜索报"站得住"，下游
                          # 按自己的网格一采就采到质心在多边形外（`contact_forces` 直接
                          # 抛异常，seed 1 实测）。240 同时是 40 与 48 的倍数，于是下游
                          # 各处自检的采样点都是它的子集，两边不可能各说各话。
SUPPORT_FINE = 2          # 着地脚数另用 SAMPLES×这个倍数的细网格。理由见 `_tables`
PHASE_GRID = 16           # 相位候选格点数；SAMPLES 必须是它的整数倍（相位平移 = 数组滚动）
MARGIN_OK = 0.5           # 认定"站得住"的稳定余量下限（px）。solve 用它决定要不要把
                          # 落点放回可达极限，sample_standing 用它筛个体——同一个门槛


# ---------------------------------------------------------------- 摆动频率
def natural_hz(segments: tuple[float, ...]) -> float:
    """复摆自然频率（Hz）。每节视为均匀杆，质量 ∝ 长度。

    不能用"单摆按总长算"糊弄：多节肢的质量分布靠近髋部，转动惯量比同长单摆小，
    摆得更快。蛛足（细长多节）和兽腿（粗短）即使总长相同，频率也不同。
    """
    m = np.array(segments, float)
    d = np.concatenate([[0.0], np.cumsum(m)[:-1]])       # 各节起点到髋的距离
    com = d + m / 2.0
    M = float(m.sum())
    if M < 1e-9:
        return 0.0
    I = float((m * m * m / 12.0 + m * com * com).sum())  # mᵢlᵢ²/12 + mᵢrᵢ²
    D = float((m * com).sum() / M)
    return math.sqrt(M * G_PX * D / I) / (2.0 * math.pi)


# ---------------------------------------------------------------- 单肢步态
@dataclass
class LimbGait:
    gene: GN.LimbGene
    hip: np.ndarray            # 髋（挂载点）世界坐标
    out_dir: np.ndarray        # 水平外展方向（单位）
    reach: float               # 水平可达半径
    hz: float                  # 自然摆动频率
    steps: int                 # 每个身体周期迈几步
    duty: float                # 支撑相占自身步周期的比例
    phase: float               # 身体周期内的相位 ∈ [0,1)
    stride: float = 0.0
    foot: np.ndarray | None = None    # 站立中立落点
    dragged: bool = False             # 碎步到上限仍跟不上：被拖着走，不提供支撑

    @property
    def max_stride(self) -> float:
        return 2.0 * self.reach * SWING

    def in_stance(self, t: float) -> bool:
        """身体周期相位 t ∈[0,1) 时这条肢是否着地。"""
        return ((t - self.phase) * self.steps) % 1.0 < self.duty

    def stance_over(self, a: float, w: float) -> bool:
        """整段 [a, a+w) 里**一直**着地才算数。

        相位搜索按采样点数着地脚数，而着地与否是一段一段的区间——采样格点完全可能跨过
        一个"抬起来了又放下"的窗口，于是解出来的步态在格点上永远有 3 只脚，格点之间却
        只剩 1 只。实测 seed 7 有 1.8% 的时间少于 3 只脚着地（最少到 1），而解里记的
        `min_support` 是 3。改成"整段都得着地"之后，表里数出来的就是全周期的真下界。
        """
        u = ((a - self.phase) * self.steps) % 1.0
        return u < self.duty and u + w * self.steps <= self.duty

    @property
    def excursion(self) -> float:
        """支撑相脚在**体坐标系**里走过的距离。

        = stride × duty，不是 stride。不滑步的条件是支撑相脚在世界系静止，于是它在
        体坐标系里必须以身体速度 v 后移，历时 duty·T_step，走过 v·duty·T_step
        = stride·duty。写成 stride 就等于每步都在蹭地——差的正好是一个 duty 因子。
        """
        return self.stride * self.duty

    @property
    def swing_lift(self) -> float:
        """摆动相脚要抬多高（px）。**一条定长的腿荡过去会扎进地里多深，就得抬多高。**

        髋高 h、支撑相脚走过 e，两个极点处腿长 √(h²+(e/2)²)，中点只要 h。所以一条不
        缩回的腿从后极点荡到前极点，会在中点低于地面 √(h²+(e/2)²) − h。抬升取这个数：
        不是"看着差不多抬这么高"，是几何上非抬不可的那个下限。
        """
        h = float(self.hip[1])
        return math.hypot(h, 0.5 * self.excursion) - h

    def foot_at(self, t: float, extra_lift: float = 0.0) -> np.ndarray:
        """t 时刻脚在**体坐标系**的位置（支撑相后移，摆动相前甩并抬起）。

        体坐标系而非世界系：支撑多边形要和质心比，而质心在体坐标系里是不动的。

        摆动相走**简谐**而不是匀速：这一层判定摆动是被动复摆（`natural_hz` 那条推导的
        全部前提），而复摆在两端角速度为零。匀速插值对稳定性评估无所谓（摆动脚本来就
        被踢出支撑多边形），但它同时是碰撞与动画共用的轨迹——匀速的话脚在离地和落地的
        瞬间还有横向速度，读作蹭地。

        `extra_lift` 留给躯干自己也在起伏的情形：那时摆动腿还要多让出一份。
        """
        u = ((t - self.phase) * self.steps) % 1.0
        e = self.excursion
        assert self.foot is not None
        if u < self.duty:
            s = u / self.duty                      # 支撑：从前极点匀速走到后极点
            return self.foot + np.array([0.0, 0.0, -e * (0.5 - s)])
        s = (u - self.duty) / (1.0 - self.duty)
        off = e * (-0.5 + 0.5 * (1.0 - math.cos(math.pi * s)))
        lift = (self.swing_lift + extra_lift) * math.sin(math.pi * s)
        return self.foot + np.array([0.0, lift, -off])


@dataclass
class Gait:
    genome: GN.Genome
    limbs: tuple[LimbGait, ...]
    com: np.ndarray
    ride: float                # 骑乘高度偏移（相对核心设计高度，负 = 蹲低）
    body_hz: float
    speed: float               # px/s（行走）
    margin: float              # 全周期最小稳定余量（px），<0 = 会摔
    min_support: int

    @property
    def blocks_per_sec(self) -> float:
        return self.speed / 16.0

    @property
    def run_blocks_per_sec(self) -> float:
        """奔跑速度。走是被动复摆（不花力气），跑是把肢体强驱到自然频率之上——
        正典里缝合兽血越低吸气越凶，强驱的代价正好由环境灵气支付。"""
        return self.speed * RUN_TEMPO / 16.0

    def describe(self) -> str:
        rows = [f"步态 seed={self.genome.seed}",
                f"  骑乘高度 {self.ride:+.1f} px（{'蹲伏' if self.ride < -4 else '撑起'}）  "
                f"身体周期 {self.body_hz:.2f} Hz  行走 {self.blocks_per_sec:.2f} / "
                f"奔跑 {self.run_blocks_per_sec:.2f} 格/s",
                f"  稳定余量 {self.margin:+.2f} px  最少着地 {self.min_support}"]
        rows.append(f"  {'槽':<10}{'类型':<10}{'总长':>6}{'自然Hz':>8}{'步/周期':>8}"
                    f"{'占空':>7}{'相位':>7}{'步幅':>7}")
        for lg in sorted(self.limbs, key=lambda x: -x.hz):
            rows.append(f"  {lg.gene.socket:<10}{lg.gene.kind:<10}{lg.gene.length:>6.1f}"
                        f"{lg.hz:>8.2f}{lg.steps:>8d}{lg.duty:>7.2f}{lg.phase:>7.2f}"
                        f"{lg.stride:>7.1f}{'  拖行' if lg.dragged else ''}")
        return "\n".join(rows)


# ---------------------------------------------------------------- 几何
def solve_ride_height(genes, socks) -> float:
    """解骑乘高度 Δy：让尽量多的肢够得着地，且各肢伸展度尽量靠近舒适值。

    评分优先级是**先数量后舒适**——多一条能承重的腿对稳定性的价值远大于剩下几条站得
    舒服一点。同分时取更靠近 COMFORT 的，避免解出"所有腿都绷直勉强够到"这种姿势。
    """
    # 搜索区间由几何推出，不写常数：下限是"把最低的髋压到离地 RIDE_CLEAR"，再低髋就
    # 钻到地里了。写死区间的话小碎片压不下去——碎片本来就该比整只兽蹲得低得多，
    # 固定 -14 的下限让每块碎片的短肢都够不着地，全部退化成蠕动（实测）。
    hips = [float(socks[g.socket].pos[1]) for g in genes if g.load_bearing]
    if not hips:
        return 0.0
    lo, hi = -(min(hips) - RIDE_CLEAR), RIDE_UP
    best, best_dy = (-1, -1e9), 0.0
    for k in range(int((hi - lo) / 0.5) + 1):
        dy = lo + k * 0.5
        n, comfort = 0, 0.0
        for gene in genes:
            if not gene.load_bearing:
                continue
            h = float(socks[gene.socket].pos[1]) + dy - gene.ankle_lift
            eff = gene.leg_len * EXTEND
            if h <= 1.0 or h >= eff:
                continue                       # 够不着地，或髋已经压到踝位以下
            n += 1
            comfort -= abs(h / eff - COMFORT)
        if (n, comfort) > best:
            best, best_dy = (n, comfort), dy
    return best_dy


def hip_geometry(sock: C.Socket, gene: GN.LimbGene, ride: float = 0.0
                 ) -> tuple[np.ndarray, np.ndarray, float]:
    """髋位置、水平外展方向、水平可达半径。

    可达半径由勾股给出：肢体有效长度 L，髋高 h，则水平最远 √(L² − h²)。够不着地
    （h ≥ L）的肢体返回 reach ≤ 0，调用方据此把它踢出承重集——**承重与否是几何
    事实，不是 kind 上的标签**：一条 0.62 倍缩放的蛛足挂在高处就是够不着。
    """
    hip = sock.pos.copy()
    hip[1] += ride
    # **腿连到踝，不连到地面**：蹄行动物的踝抬到掌骨全长那么高（羊/牛的"管骨"），
    # 趾行的抬掌骨的八成，跖行几乎贴地。按总长和髋高算勾股会以为羊腿够得到远得多的
    # 地方——实际那一截长度是竖着用掉的，不能折算成水平可达（腿谱实测：按老算法摆出来
    # 的羊腿股骨横着支出去，像条断腿）。
    h = float(hip[1]) - gene.ankle_lift
    horiz = np.array([sock.normal[0], 0.0, sock.normal[2]])
    n = float(np.linalg.norm(horiz))
    out = horiz / n if n > 1e-6 else np.array([1.0, 0.0, 0.0])
    eff = gene.leg_len * EXTEND
    # 够不够得着要按**踝**判：踝就算落在髋的正下方，也还在身后 ankle_back 那么远。
    # 只比 h 的话会把"其实够不着"的肢放进承重集，到部件层才发现腿伸不到踝（IK 退回
    # 直链，末端偏离 8 px）。这里排除掉，运动层的重采样自然会换一个配置。
    if math.hypot(h, gene.ankle_back) >= eff:
        return hip, out, 0.0
    reach = math.sqrt(max(0.0, eff * eff - h * h))
    return hip, out, reach


# ---------------------------------------------------------------- 稳定性
def _hull(pts: np.ndarray) -> np.ndarray:
    """2D 凸包（Andrew monotone chain），逆时针。

    去重与排序**不走 numpy**：点只有三到六个，而 `np.unique(..., axis=0)` + `np.lexsort`
    在这个规模上全是调用开销。相位搜索一次要跑几十万个凸包，这里省下的是主要开销。
    """
    seen: dict[tuple[float, float], None] = {}
    for q in pts:
        seen[(round(float(q[0]), 6), round(float(q[1]), 6))] = None
    if len(seen) <= 2:
        return np.array(list(seen), float).reshape(-1, 2)
    p = np.array(sorted(seen), float)

    def half(seq):
        # 二维叉积手写成标量运算。`np.cross` 对 2 维输入要绕 moveaxis/normalize_axis_tuple
        # 一大圈，实测它一家占了相位搜索总时间的一半（48 万次调用）——这里的向量只有两个
        # 分量，一行乘减就够了。
        out: list = []
        for q in seq:
            while len(out) >= 2:
                ax, ay = out[-1][0] - out[-2][0], out[-1][1] - out[-2][1]
                bx, by = q[0] - out[-2][0], q[1] - out[-2][1]
                if ax * by - ay * bx > 0:
                    break
                out.pop()
            out.append(q)
        return out

    return np.array(half(p)[:-1] + half(p[::-1])[:-1])


def support_margin(feet: np.ndarray, com: np.ndarray) -> float:
    """质心地面投影到支撑多边形边界的**有向**距离：正 = 在内部，负 = 已经在外面。

    只判"在不在里面"不够——贴着边界站着，任何一点摇晃都会摔。要的是余量。
    """
    h = _hull(feet)
    if len(h) < 3:
        return -1e3
    best = 1e9
    inside = True
    cx, cy = float(com[0]), float(com[1])
    n = len(h)
    for i in range(n):
        ax, ay = float(h[i][0]), float(h[i][1])
        bx, by = float(h[(i + 1) % n][0]), float(h[(i + 1) % n][1])
        ex, ey = bx - ax, by - ay
        l2 = ex * ex + ey * ey
        if l2 < 1e-18:
            continue
        ln = math.sqrt(l2)
        wx, wy = cx - ax, cy - ay
        inside &= (ex * wy - ey * wx) / ln >= 0.0     # 逆时针凸包：正 = 在内侧
        t = min(1.0, max(0.0, (wx * ex + wy * ey) / l2))
        best = min(best, math.hypot(wx - ex * t, wy - ey * t))
    return best if inside else -best


def _tables(limbs: list[LimbGait]) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """预算每条肢在**相位 0** 下、全周期的着地掩码（细网格）、余量用掩码与脚 xz（粗网格）。

    相位取在 PHASE_GRID 格点上时，改相位等价于把这些表沿采样轴**滚动**
    （u = ((t-φ)·steps)%1，t=k/S、φ=j/PHASE_GRID ⇒ 索引平移 j·S/PHASE_GRID）。
    于是相位搜索里不再有任何三角/取模运算，只剩 np.roll 和凸包。

    **着地脚数必须用细网格数**。着地与否是一段一段的区间，48 个格点完全可能整个跨过
    一个"抬起来了又放下"的窗口——实测 seed 7 在格点上永远有 3 只脚，格点之间却掉到
    1 只，占全周期的 1.8%。细网格只做布尔求和（纯 numpy，几乎不要钱），凸包那一步照旧
    留在粗网格上，它才是开销所在。

    细网格上用 `stance_over` 而不是 `in_stance`：要的是**整段都着地**，数出来才是真下界。
    在 1/1200 的窗口下这点保守可以忽略；换到 48 点上就过头了（5/12 的 seed 直接判站不住，
    而它们其实站得住——一条腿落地的同一瞬间另一条抬起，脚数从头到尾没掉过）。
    """
    n, S = len(limbs), SAMPLES
    F = S * SUPPORT_FINE
    fine = np.zeros((n, F), bool)
    mask = np.zeros((n, S), bool)
    xz = np.zeros((n, S, 2))
    tf = np.arange(F) / F
    for i, lg in enumerate(limbs):
        keep = lg.phase
        lg.phase = 0.0
        # 细网格整条一次算完。逐点调 `stance_over` 是纯 Python 的 1200×n 次循环，
        # 相位搜索每评估一个候选就要重来一遍——实测把 `sample_standing` 从 2 秒拖到 32 秒
        u = (tf * lg.steps) % 1.0
        fine[i] = (u < lg.duty) & (u + lg.steps / F <= lg.duty)
        for k in range(S):
            tt = k / S
            mask[i, k] = lg.in_stance(tt)
            f = lg.foot_at(tt)
            xz[i, k] = (f[0], f[2])
        lg.phase = keep
    return fine, mask, xz


def evaluate_tab(fine: np.ndarray, mask: np.ndarray, xz: np.ndarray,
                 shifts: list[int], com2: np.ndarray,
                 floor: float = -1e18) -> tuple[float, int]:
    """按整数相位偏移查表求全周期最小稳定余量与最少着地脚数。

    floor 是分支限界：调用方已知的当前最好分数。一旦某个采样点的余量已经低于它，
    这个候选不可能更优，立刻放弃剩下的采样点——相位搜索里绝大多数候选都是烂的，
    这一刀砍掉的是主要开销。
    """
    # 先跑便宜的：粗网格脚数 → 支撑多边形余量（带分支限界）。**细网格留到最后**——
    # 它是这里唯一按 SAMPLES×SUPPORT_FINE 算的东西，而相位搜索里绝大多数候选在余量
    # 那一步就被剪掉了，没必要为它们算真下界。实测放在最前面时一次 solve 要 800 ms。
    m = np.stack([np.roll(mask[i], s) for i, s in enumerate(shifts)])
    fewest = int(m.sum(axis=0).min())
    if fewest < MIN_SUPPORT:
        return -1e3, fewest
    p = np.stack([np.roll(xz[i], s, axis=0) for i, s in enumerate(shifts)])
    worst = 1e9
    for k in range(SAMPLES):
        worst = min(worst, support_margin(p[m[:, k], k], com2))
        if worst <= floor:
            return worst, fewest       # 已经赢不了了，不必再验细网格
    q = SUPPORT_FINE
    fw = np.stack([np.roll(fine[i], s * q) for i, s in enumerate(shifts)])
    fewest = int(fw.sum(axis=0).min())
    if fewest < MIN_SUPPORT:
        return -1e3, fewest
    return worst, fewest


def evaluate(limbs: list[LimbGait], com2: np.ndarray) -> tuple[float, int]:
    """全周期最小稳定余量与最少着地脚数（直接按当前相位算，供报告与自检用）。"""
    F = SAMPLES * SUPPORT_FINE
    tf = np.arange(F) / F
    on = np.zeros(F, int)
    for lg in limbs:
        u = ((tf - lg.phase) * lg.steps) % 1.0
        on += ((u < lg.duty) & (u + lg.steps / F <= lg.duty)).astype(int)
    fewest = int(on.min())
    worst = 1e9
    # 余量的网格必须和相位搜索用的**同一个**（`SAMPLES`）：搜索优化的是哪个网格上的
    # 余量，验收就只能验哪个网格——两边不一致的话，搜索交出的最优解在验收那里全军覆没
    # （实测把这里加密到 480 而搜索仍是 48，seed 1 的 200 个候选一个都过不了）。
    for k in range(SAMPLES):
        t = k / SAMPLES
        grounded = [lg for lg in limbs if lg.in_stance(t)]
        if len(grounded) < MIN_SUPPORT:
            worst = min(worst, -1e3)
            continue
        feet = np.array([[lg.foot_at(t)[0], lg.foot_at(t)[2]] for lg in grounded])
        worst = min(worst, support_margin(feet, com2))
    return worst, fewest


def feasible(limbs: list[LimbGait], com2: np.ndarray) -> float:
    """**所有**脚都着地时的稳定余量——支撑多边形的上界。

    这是相位优化之前的快速否决：全着地都包不住质心，任何抬腿时序都救不回来。
    没有这一步的话，"三条腿全长在左侧"这类基因组要跑完整轮优化才被判死，
    重采样时绝大部分开销花在注定失败的候选上。
    """
    feet = np.array([[lg.foot[0], lg.foot[2]] for lg in limbs])
    return support_margin(feet, com2)


def _radius_for(lg: LimbGait, want: float) -> float:
    """解"落点摆在哪，踝正好离髋 want"——关于径向距离 r 的二次方程。

    踝在落点正上方 `ankle_lift`、再往身后 `ankle_back`；身后方向与径向外展不正交，
    所以交叉项带 out·ẑ。无解（want 太小）时返回 0。
    """
    h = float(lg.hip[1]) - lg.gene.ankle_lift
    ab = lg.gene.ankle_back
    b = 2.0 * float(lg.out_dir[2]) * ab
    c = ab * ab + h * h - want * want
    disc = b * b - 4.0 * c
    return max(0.0, 0.5 * (-b + math.sqrt(disc))) if disc > 0.0 else 0.0


def stance_radius(lg: LimbGait) -> float:
    """中立落点离髋的**水平**距离。

    取「舒适伸展度」那个距离，不是可达极限。`solve_ride_height` 通篇在把髋高/有效肢长
    往 COMFORT 上凑，落点却推到 reach（= 伸展度 100%），两处对同一个词的用法是矛盾的。

    撑到极限的代价不在这一层，在部件层：地面反力对每个关节的力矩正比于该关节到落点的
    **水平**距离，而肌肉力臂只有节长的一成，于是肌肉截面按这个距离线性放大。实测撑到
    可达极限时根部要粗到挂载面的两倍，收到舒适伸展度只要六成。

    收窄**不换稳定性**——实测把落点收到极限的一半，全周期最小余量从 +3.26 只掉到
    +3.13（seed 3 反而从 +0.52 涨到 +0.66）：支撑多边形是绕各自的髋收缩的，质心本来就
    在中间，收缩不会把它挤出去。所以这是净赚。真掉了余量的个体由 `place_feet` 放回去。

    上限来自行程：脚要沿 z 走 ±e/2，**整段**都得留在可达半径内。落点在 out·r 上，位移在
    ẑ 上，两者一般不正交：

        |out·r − ẑ·off|² = r² + off² − 2·r·off·(out·ẑ)

    对 off ∈ [−e/2, e/2] 取最大（off 取 −sign(out·ẑ)·e/2），令其 ≤ reach²，解得

        r ≤ −(e/2)|out_z| + √( (e/2)²·out_z² − (e/2)² + reach² )

    原来写的是 √(reach² − (e/2)²)，那是 out ⊥ ẑ 的特例。**前后向劈开的那条肢因此被放到
    了它够不着的地方**：支撑相走到极点时连水平距离都超过可达半径，动画层只能把整只兽
    往下压去够（实测 seed 1 有一帧压了 9.57 px，下一帧又弹回 0）。
    """
    # 摆的是**落点**，但腿够的是**踝**。两条更简单的写法都试过、都不行：① 忽略踝的
    # 身后偏移 ⇒ 腿够不着自己的踝，IK 无解退回直链、关节落到地面以下（y=−4.13）；
    # ② 把落点整体前移 ⇒ 支撑多边形跟着前移，质心落到多边形外，seed 12 站不住。
    h = float(lg.hip[1]) - lg.gene.ankle_lift
    eff = lg.gene.leg_len * EXTEND
    want = max(COMFORT * eff, math.hypot(h, lg.gene.ankle_back))
    return min(_radius_for(lg, want), travel_cap(lg))


def travel_cap(lg: LimbGait) -> float:
    """中立落点最远能摆多远，才能保证**整个支撑相**都留在可达半径内。

        |out·r − ẑ·off|² = r² + off² − 2·r·off·(out·ẑ)

    对 off ∈ [−e/2, e/2] 取最大（off 取 −sign(out·ẑ)·e/2），令其 ≤ reach²，解得

        r ≤ −(e/2)|out_z| + √( (e/2)²·out_z² − (e/2)² + reach² )

    原来只在舒适落点那一支写了 √(reach² − (e/2)²)，那是 out ⊥ ẑ 的特例；而**可达极限那
    一支根本没有这道帽**。于是"收窄站不住、放回可达极限"的个体，其前后向劈开的那条肢
    在支撑相极点连水平距离都超出可达半径——动画层只能把整只兽往下压去够（seed 1 实测
    压了 9.57 px，下一帧又弹回 0）。两支现在共用这一个上限。
    """
    half = 0.5 * lg.excursion
    oz = abs(float(lg.out_dir[2]))
    disc = half * half * oz * oz - half * half + lg.reach ** 2
    return max(0.0, -half * oz + math.sqrt(disc)) if disc > 0.0 else 0.0


def place_feet(limbs: list[LimbGait], k: float) -> None:
    """按 k 摆中立落点：0 = 舒适伸展度（见 `stance_radius`），1 = 可达极限。"""
    for lg in limbs:
        r0 = stance_radius(lg)
        # 上界也得按**踝**算，不能用 lg.reach：那个数没把踝的身后偏移算进去，撑到它
        # 就等于要求腿够到够不着的地方，IK 退回直链、腿的末端偏离踝位 8 px（实测）。
        r1 = min(_radius_for(lg, lg.gene.leg_len * EXTEND), lg.reach, travel_cap(lg))
        lg.foot = (np.array([lg.hip[0], 0.0, lg.hip[2]])
                   + lg.out_dir * (r0 + (max(r1, r0) - r0) * k))


def optimize_phases(limbs: list[LimbGait], com2: np.ndarray, *, seed: int,
                    restarts: int = 8, sweeps: int = 3) -> tuple[float, int]:
    """求相位。随机重启 + 逐肢坐标下降——相位空间小（每肢一维、周期性），不需要更重的方法。

    第一个候选用「按 steps 均分」的经典多足相位；随机重启负责跳出它的局部最优
    （肢体分布不对称时，均分相位往往正是摔倒的那个解）。
    """
    rng = np.random.default_rng(seed)
    n = len(limbs)
    step = SAMPLES // PHASE_GRID
    fine, mask, xz = _tables(limbs)

    best_sh = [0] * n
    best = evaluate_tab(fine, mask, xz, best_sh, com2)
    for r in range(restarts):
        if r == 0:   # 经典多足均分相位。肢体分布不对称时它往往正是摔倒的那个解
            sh = [int(round(i * PHASE_GRID / n)) % PHASE_GRID * step for i in range(n)]
        else:
            sh = [int(v) * step for v in rng.integers(0, PHASE_GRID, n)]
        for _ in range(sweeps):
            improved = False
            for i in range(n):
                cur = evaluate_tab(fine, mask, xz, sh, com2)
                keep = sh[i]
                for j in range(PHASE_GRID):
                    sh[i] = j * step
                    sc = evaluate_tab(fine, mask, xz, sh, com2, floor=cur[0])
                    if sc > cur:
                        cur, keep, improved = sc, j * step, True
                sh[i] = keep
            if not improved:
                break
        sc = evaluate_tab(fine, mask, xz, sh, com2, floor=best[0])
        if sc > best:
            best, best_sh = sc, list(sh)

    for lg, s in zip(limbs, best_sh):
        lg.phase = (s / SAMPLES) % 1.0
    return evaluate(limbs, com2)


# ---------------------------------------------------------------- 求解
def solve(g: GN.Genome, socks: dict[str, C.Socket] | None = None,
          com: np.ndarray | None = None) -> Gait:
    """求这具身体的步态。

    `com` 是**这具身体自己的质心**。碎片必须传自己的（见 fission.Fragment.centroid）——
    沿用整只兽的质心会让碎片的支撑多边形去包一个远在体外的点，每块碎片都判"站不住"
    而退化成蠕动（实测四块全退化）。
    """
    socks = socks or C.sockets()
    com = C.centroid() if com is None else np.asarray(com, float)
    com2 = np.array([com[0], com[2]])
    ride = solve_ride_height(g.limbs, socks)

    limbs: list[LimbGait] = []
    for gene in g.limbs:
        sock = socks[gene.socket]
        hip, out, reach = hip_geometry(sock, gene, ride)
        if not gene.load_bearing or reach <= 1.0:
            continue                      # 够不着地 / 本来就不承重的，不进步态求解
        limbs.append(LimbGait(gene, hip, out, reach, natural_hz(gene.segments),
                              1, 0.6, 0.0))
    if len(limbs) < MIN_SUPPORT:
        raise ValueError(f"只有 {len(limbs)} 条肢够得着地（需 {MIN_SUPPORT}）")

    # 身体周期 = 最慢承重肢的自然频率：最慢那条只能一周期一步，其余按整数倍跟上。
    body_hz = min(lg.hz for lg in limbs)
    for lg in limbs:
        lg.steps = int(np.clip(round(lg.hz / body_hz), 1, 4))
        # 步数多的肢单步时间短，占空比给低些；步数少的要长时间撑着
        lg.duty = float(np.clip(0.80 - 0.06 * (lg.steps - 1), 0.55, 0.85))
        lg.foot = np.array([lg.hip[0], 0.0, lg.hip[2]]) + lg.out_dir * lg.reach

    # 速度取各肢"自己舒服的速度"的**中位数**，再让跟不上的肢碎步追。迭代两轮即收敛：
    # 抬高某条肢的步数会放宽它的限速，可能让整体速度还能再涨一点。
    prefer = sorted(lg.steps * body_hz * lg.max_stride for lg in limbs)
    speed = prefer[len(prefer) // 2]
    for _ in range(2):
        for lg in limbs:
            need = math.ceil(speed / max(body_hz * lg.max_stride, 1e-9))
            lg.steps = int(np.clip(max(lg.steps, need), 1, MAX_STEPS))
            lg.dragged = need > MAX_STEPS
            lg.duty = float(np.clip(0.80 - 0.06 * (lg.steps - 1), 0.45, 0.85))
        keep = [lg for lg in limbs if not lg.dragged]
        if len(keep) < MIN_SUPPORT:
            raise ValueError(f"只剩 {len(keep)} 条肢跟得上，其余全在拖行")
        speed = min(lg.steps * body_hz * lg.max_stride for lg in keep)
    for lg in limbs:
        lg.stride = speed / (lg.steps * body_hz)

    # 拖行的肢不提供支撑：它没在踩地，是被拽着蹭过去的
    limbs = [lg for lg in limbs if not lg.dragged]

    # 中立落点先收到舒适伸展度（省下部件层一半的肢体粗细，见 stance_radius），够稳就
    # 收工；不够稳再放回可达极限重解一次相位，取好的那个。判据必须是**相位优化之后**
    # 的余量：feasible 是全着地的上界，收窄可能保住上界却在摆动相塌掉（实测 seed 12
    # 就是这么从"能站"变成"站不住"的）。
    best: tuple | None = None
    for k in (0.0, 1.0):
        place_feet(limbs, k)
        if feasible(limbs, com2) <= 0.0:
            continue
        m, few = optimize_phases(limbs, com2, seed=g.seed)
        if best is None or m > best[0]:
            best = (m, few, k, [lg.phase for lg in limbs])
        if m >= MARGIN_OK:
            break
    if best is None:
        raise ValueError("全脚着地都包不住质心，任何相位都站不住")
    margin, fewest, k, phases = best
    place_feet(limbs, k)
    for lg, ph in zip(limbs, phases):
        lg.phase = ph
    return Gait(g, tuple(limbs), com, ride, body_hz, speed, margin, fewest)


def sample_standing(seed: int, *, tries: int = 200, skip: int = 0,
                    socks: dict[str, C.Socket] | None = None) -> tuple[GN.Genome, Gait]:
    """采一只**站得住**的兽。

    基因组随机挑槽，完全可能挑出"三条腿全在左边"这种站不住的配置。合法性（genome
    .validate）管的是"是不是缝合兽"，站不站得住得靠力学算——所以在这里重采样，
    而不是把稳定性塞进 genome 去猜。

    预算从 60 提到 200：着地脚数改成细网格真下界之后（见 `_tables`），过去靠采样格点
    对齐蒙混过关的那些配置被否掉了，合格率跟着降。seed 12 在 60 次内采不到，200 次内
    采得到（余量 +1.73）。这是搜索预算，不是物理常数。

    `skip` 跳过前若干个合格候选，取下一个。给 `limbs.build` 用：站得住只是第一道门，
    "这具身体塞不塞得下这么多这么粗的腿"是第二道，而后者要解完粗细才知道——那一层
    发现塞不下时，就回来要下一只。
    """
    it = iter_standing(seed, tries=tries, socks=socks)
    for _ in range(skip):
        if next(it, None) is None:
            break
    got = next(it, None)
    if got is None:
        raise ValueError(f"seed={seed} 试了 {tries} 次没采到站得住的配置")
    return got


def iter_standing(seed: int, *, tries: int = 200,
                  socks: dict[str, C.Socket] | None = None):
    """依次吐出这个 seed 下**所有**站得住的配置。

    下游（`limbs.build`）要连着看好几只才能挑出"腿也装得下"的那只，而一只一只重来会把
    前面的候选反复重解——seed 4 在 200 次里只有两只站得住，靠 `skip` 取第二只就得把第一只
    再解一遍。生成器一遍扫完。
    """
    for k in range(tries):
        s = seed * 1000 + k
        try:
            g = GN.sample(s, socks=socks)
            gait = solve(g, socks)
        except ValueError:
            continue
        if gait.margin > MARGIN_OK and gait.min_support >= MIN_SUPPORT:
            yield g, gait


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=None)
    ap.add_argument("--count", type=int, default=3)
    args = ap.parse_args()

    socks = C.sockets()
    seeds = [args.seed] if args.seed is not None else list(range(1, args.count + 1))
    for s in seeds:
        g, gait = sample_standing(s, socks=socks)
        print(g.describe())
        print(gait.describe())
        ratios = sorted({lg.steps for lg in gait.limbs})
        print(f"  → 步数比 {':'.join(map(str, ratios))}"
              f"{'（错拍：肢体长短不一，走不出齐步）' if len(ratios) > 1 else '（同拍）'}")
        print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
