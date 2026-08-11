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
  python3 scripts/models/stitched_beast/locomotion.py           # 若干 seed 的步态报告
  python3 scripts/models/stitched_beast/locomotion.py --seed 7
"""

from __future__ import annotations

import argparse
import math
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
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
RIDE_RANGE = (-14.0, 6.0) # 骑乘高度搜索区间（相对核心设计高度）
SAMPLES = 48              # 稳定性采样点数
PHASE_GRID = 16           # 相位候选格点数；SAMPLES 必须是它的整数倍（相位平移 = 数组滚动）


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

    @property
    def excursion(self) -> float:
        """支撑相脚在**体坐标系**里走过的距离。

        = stride × duty，不是 stride。不滑步的条件是支撑相脚在世界系静止，于是它在
        体坐标系里必须以身体速度 v 后移，历时 duty·T_step，走过 v·duty·T_step
        = stride·duty。写成 stride 就等于每步都在蹭地——差的正好是一个 duty 因子。
        """
        return self.stride * self.duty

    def foot_at(self, t: float) -> np.ndarray:
        """t 时刻脚在**体坐标系**的位置（支撑相后移，摆动相前甩）。

        体坐标系而非世界系：支撑多边形要和质心比，而质心在体坐标系里是不动的。
        """
        u = ((t - self.phase) * self.steps) % 1.0
        e = self.excursion
        if u < self.duty:
            s = u / self.duty                      # 支撑：从前极点匀速走到后极点
            off = e * (0.5 - s)
        else:
            s = (u - self.duty) / (1.0 - self.duty)
            off = e * (-0.5 + s)                   # 摆动：抬回前极点
        assert self.foot is not None
        return self.foot + np.array([0.0, 0.0, -off])


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
    best, best_dy = (-1, -1e9), 0.0
    lo, hi = RIDE_RANGE
    for k in range(int((hi - lo) / 0.5) + 1):
        dy = lo + k * 0.5
        n, comfort = 0, 0.0
        for gene in genes:
            if not gene.load_bearing:
                continue
            h = float(socks[gene.socket].pos[1]) + dy
            eff = gene.length * EXTEND
            if h <= 1.0 or h >= eff:
                continue                       # 够不着地，或髋已经压到地面以下
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
    h = float(hip[1])
    horiz = np.array([sock.normal[0], 0.0, sock.normal[2]])
    n = float(np.linalg.norm(horiz))
    out = horiz / n if n > 1e-6 else np.array([1.0, 0.0, 0.0])
    eff = gene.length * EXTEND
    reach = math.sqrt(max(0.0, eff * eff - h * h)) if eff > h else 0.0
    return hip, out, reach


# ---------------------------------------------------------------- 稳定性
def _hull(pts: np.ndarray) -> np.ndarray:
    """2D 凸包（Andrew monotone chain），逆时针。"""
    p = np.unique(np.round(pts, 6), axis=0)
    if len(p) <= 2:
        return p
    p = p[np.lexsort((p[:, 1], p[:, 0]))]

    def half(seq):
        out: list = []
        for q in seq:
            while len(out) >= 2 and np.cross(out[-1] - out[-2], q - out[-2]) <= 0:
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
    for a, b in zip(h, np.roll(h, -1, axis=0)):
        e = b - a
        ln = float(np.linalg.norm(e))
        if ln < 1e-9:
            continue
        cr = float(np.cross(e, com - a)) / ln         # 逆时针凸包：正 = 在内侧
        inside &= cr >= 0
        t = float(np.dot(com - a, e)) / (ln * ln)
        d = float(np.linalg.norm(com - (a + e * np.clip(t, 0.0, 1.0))))
        best = min(best, d)
    return best if inside else -best


def _tables(limbs: list[LimbGait]) -> tuple[np.ndarray, np.ndarray]:
    """预算每条肢在**相位 0** 下、全周期各采样点的着地掩码与脚 xz。

    相位取在 PHASE_GRID 格点上时，改相位等价于把这两张表沿采样轴**滚动**
    （u = ((t-φ)·steps)%1，t=k/SAMPLES、φ=j/PHASE_GRID ⇒ 索引平移 j·SAMPLES/PHASE_GRID）。
    于是相位搜索里不再有任何三角/取模运算，只剩 np.roll 和凸包。
    """
    n, S = len(limbs), SAMPLES
    mask = np.zeros((n, S), bool)
    xz = np.zeros((n, S, 2))
    for i, lg in enumerate(limbs):
        keep = lg.phase
        lg.phase = 0.0
        for k in range(S):
            tt = k / S
            mask[i, k] = lg.in_stance(tt)
            f = lg.foot_at(tt)
            xz[i, k] = (f[0], f[2])
        lg.phase = keep
    return mask, xz


def evaluate_tab(mask: np.ndarray, xz: np.ndarray, shifts: list[int],
                 com2: np.ndarray, floor: float = -1e18) -> tuple[float, int]:
    """按整数相位偏移查表求全周期最小稳定余量与最少着地脚数。

    floor 是分支限界：调用方已知的当前最好分数。一旦某个采样点的余量已经低于它，
    这个候选不可能更优，立刻放弃剩下的采样点——相位搜索里绝大多数候选都是烂的，
    这一刀砍掉的是主要开销。
    """
    m = np.stack([np.roll(mask[i], s) for i, s in enumerate(shifts)])
    fewest = int(m.sum(axis=0).min())
    if fewest < MIN_SUPPORT:
        return -1e3, fewest
    p = np.stack([np.roll(xz[i], s, axis=0) for i, s in enumerate(shifts)])
    worst = 1e9
    for k in range(SAMPLES):
        worst = min(worst, support_margin(p[m[:, k], k], com2))
        if worst <= floor:
            break
    return worst, fewest


def evaluate(limbs: list[LimbGait], com2: np.ndarray) -> tuple[float, int]:
    """全周期最小稳定余量与最少着地脚数（直接按当前相位算，供报告与自检用）。"""
    worst, fewest = 1e9, 99
    for k in range(SAMPLES):
        t = k / SAMPLES
        grounded = [lg for lg in limbs if lg.in_stance(t)]
        fewest = min(fewest, len(grounded))
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


def optimize_phases(limbs: list[LimbGait], com2: np.ndarray, *, seed: int,
                    restarts: int = 8, sweeps: int = 3) -> tuple[float, int]:
    """求相位。随机重启 + 逐肢坐标下降——相位空间小（每肢一维、周期性），不需要更重的方法。

    第一个候选用「按 steps 均分」的经典多足相位；随机重启负责跳出它的局部最优
    （肢体分布不对称时，均分相位往往正是摔倒的那个解）。
    """
    rng = np.random.default_rng(seed)
    n = len(limbs)
    step = SAMPLES // PHASE_GRID
    mask, xz = _tables(limbs)

    best_sh = [0] * n
    best = evaluate_tab(mask, xz, best_sh, com2)
    for r in range(restarts):
        if r == 0:   # 经典多足均分相位。肢体分布不对称时它往往正是摔倒的那个解
            sh = [int(round(i * PHASE_GRID / n)) % PHASE_GRID * step for i in range(n)]
        else:
            sh = [int(v) * step for v in rng.integers(0, PHASE_GRID, n)]
        for _ in range(sweeps):
            improved = False
            for i in range(n):
                cur = evaluate_tab(mask, xz, sh, com2)
                keep = sh[i]
                for j in range(PHASE_GRID):
                    sh[i] = j * step
                    sc = evaluate_tab(mask, xz, sh, com2, floor=cur[0])
                    if sc > cur:
                        cur, keep, improved = sc, j * step, True
                sh[i] = keep
            if not improved:
                break
        sc = evaluate_tab(mask, xz, sh, com2, floor=best[0])
        if sc > best:
            best, best_sh = sc, list(sh)

    for lg, s in zip(limbs, best_sh):
        lg.phase = (s / SAMPLES) % 1.0
    return evaluate(limbs, com2)


# ---------------------------------------------------------------- 求解
def solve(g: GN.Genome, socks: dict[str, C.Socket] | None = None) -> Gait:
    socks = socks or C.sockets()
    com = C.centroid()
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
        raise ValueError(f"seed={g.seed} 只有 {len(limbs)} 条肢够得着地，站不住")

    # 身体周期 = 最慢承重肢的自然频率：最慢那条只能一周期一步，其余按整数倍跟上。
    body_hz = min(lg.hz for lg in limbs)
    for lg in limbs:
        lg.steps = int(np.clip(round(lg.hz / body_hz), 1, 4))
        # 步数多的肢单步时间短，占空比给低些；步数少的要长时间撑着
        lg.duty = float(np.clip(0.80 - 0.06 * (lg.steps - 1), 0.55, 0.85))
        # 中立落点：髋垂直投到地面，再沿水平外展方向推出可达半径
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
            raise ValueError(f"seed={g.seed} 只剩 {len(keep)} 条肢跟得上，其余全在拖行")
        speed = min(lg.steps * body_hz * lg.max_stride for lg in keep)
    for lg in limbs:
        lg.stride = speed / (lg.steps * body_hz)

    # 拖行的肢不提供支撑：它没在踩地，是被拽着蹭过去的
    limbs = [lg for lg in limbs if not lg.dragged]
    if feasible(limbs, com2) <= 0.0:
        raise ValueError(f"seed={g.seed} 全脚着地都包不住质心，任何相位都站不住")

    margin, fewest = optimize_phases(limbs, com2, seed=g.seed)
    return Gait(g, tuple(limbs), com, ride, body_hz, speed, margin, fewest)


def sample_standing(seed: int, *, tries: int = 60,
                    socks: dict[str, C.Socket] | None = None) -> tuple[GN.Genome, Gait]:
    """采一只**站得住**的兽。

    基因组随机挑槽，完全可能挑出"三条腿全在左边"这种站不住的配置。合法性（genome
    .validate）管的是"是不是缝合兽"，站不站得住得靠力学算——所以在这里重采样，
    而不是把稳定性塞进 genome 去猜。
    """
    for k in range(tries):
        s = seed * 1000 + k
        try:
            g = GN.sample(s, socks=socks)
            gait = solve(g, socks)
        except ValueError:
            continue
        if gait.margin > 0.5 and gait.min_support >= MIN_SUPPORT:
            return g, gait
    raise ValueError(f"seed={seed} 试了 {tries} 次没采到站得住的配置")


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
