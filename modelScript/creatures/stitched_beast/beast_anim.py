#!/usr/bin/env python3
"""异变缝合兽 —— 整兽动画：带腿走路，头跟着晃。

前面几层已经把这只兽**怎么走**解完了：运动层解出了每条肢每周期迈几步、占空多少、相位
排在哪、步幅多长；肢体层解出了每条腿的折法。这一层不再决定任何事，只把那些数接到时间轴
上——但接的过程里冒出了四条**只有动起来才看得见**的推论，它们是这一层真正的内容。

## 一、躯干在走的时候是**端平的**——这是量出来的结论，不是省事

第一版按倒立摆（compass gait）写：支撑相里腿是根定长撑杆，脚钉住不动，髋绕着脚划弧，
所以髋高 = √(ℓ² − d²)，跨步中点最高、两端落下来。六条腿各要一个高度，取按地面反力加权
的最小二乘平面折中。**解出来横滚 ±28.5°、俯仰 ±32.1°**——一只兽走路不会这么摇。

错在"定长撑杆"这个前提，而它恰恰和本仓库自己的肢体层矛盾：`refold` 里腿的长度是自由的，
折叠量随时可变。所以真正该问的不是"撑杆要求躯干在多高"，而是**"躯干不动的话，腿够不
够得着"**——逐帧检查每条支撑肢在掌骨立到极限（88°）之后还够不够得着，够不着才把躯干往
下压，压到刚好够为止。这是约束，不是那条撑杆公式。

按这个写法一跑，躯干在某几帧被压下去 9.57 px、下一帧又弹回 0（seed 1 实测，相邻两帧
跳 12.84 px）。追下去发现**问题不在这一层**：运动层的 `stance_radius` 给中立落点封了一道
"整个支撑相都得留在可达半径内"的顶，但那道顶写成 √(reach² − (e/2)²)——只在落点的外展方向
与前进方向正交时才对；而且**可达极限那一支根本没封**。于是前后向劈开的那条肢被摆到了它
够不着的地方，动画层只好把整只兽往下压去够。顶改成正确的一般式（见 `locomotion.travel_cap`）
之后，seed 1/3/7 的"最紧可达"分别落到 0.88 / 0.92 / 0.82，**升沉恒等于 0**。

于是结论回来了，而且这次站得住：**没有任何东西逼躯干上下动**。这不是巧合——六足以上的
静稳定步行者本来就能把躯干端平（昆虫、蟹就是这么走的），四足动物那种明显的上下颠簸是
"腿不够用"逼出来的。这只兽腿有余量，于是它走起来躯干平得像块板。逼降机制留着：`--list`
的"最紧可达"一栏 >1 时它就会动，那是"这只兽的腿真的不够用了"的信号。

## 二、脚要抬多高，同一条推导给了两遍

设髋高 h、支撑相脚在体坐标系里走过 e（= 步幅 × 占空，见 `LimbGait.excursion`）。
两个极点处腿的长度是 √(h² + (e/2)²)，中点处只要 h。于是同一个量 δ = √(h²+(e/2)²) − h
同时是：

  · **躯干在一步里起伏的幅度**（撑杆在中点最直，把身体顶高 δ）；
  · **摆动腿必须缩回去的量**（不缩的话，一条定长的腿从后极点荡到前极点会在中点扎进
    地里 δ 深）。

净空还要再加一份躯干升沉的全幅：摆动腿的相位和别的肢并不锁在一起，别人把躯干压下去的
时候它正悬在半空。这一项在本仓库的十二个 seed 上恰好是 0（见上一节），但公式里留着——
一旦有个基因组把某条腿逼到伸直，躯干开始压，它立刻就有值。

抬升与摆动轨迹都**放回了运动层**（`LimbGait.swing_lift` / `foot_at`），不留在这里。碰撞
检测（`limbs.cycle_caps` → `_splay` 掰落点）和动画必须走同一条轨迹：各写一份的时候，
掰落点掰的是另一条腿——`_splay` 只看到 1.12 px 的互穿，而动画真穿 1.97 px。

摆动相的**水平**轨迹同时改成简谐而不是匀速：肢体层判定摆动是被动复摆（`natural_hz` 那条
推导的全部前提），复摆两端角速度为零。匀速插值对稳定性评估是够的（摆动脚本来就被踢出
支撑多边形），但当动画看就是机械臂——脚在离地和落地的瞬间横向速度不为零，读作蹭地。

## 三、掌骨角一逐帧解，脚跟自己就抬起来了

`solve_limb` 解站姿时，掌骨的倾角是**反过来由可达性定的**：从标称角往上加，取第一个
腿够得着踝的角度。把同一件事逐帧做一遍——落点在体坐标系里往后走，髋越过它，够不着了
掌骨就得再立起来一点——**跖行足的脚跟就自己抬起来了**。抬跟不是画的，是同一条可达性
约束在时间上的展开。原来的 4° 阶梯换成连续二分（`limbs.meta_angle`），免得逐帧跳。

## 四、"踩住的东西不许自己转"——两处都栽在这上面

同一条纪律犯了两次，都是**静止姿看不出、一动起来才露馅**的那种：

  · `foot_chain` 的外展方向原本按当下的髋→落点重算。腿一前后扫，这个方向就转，跗节
    连带贴在它上面的甲板一起绕着自己拧——蛛足的脚因此比站着时多扎进地里 0.54 px。
    踩住的脚是不动的，摆动的是它上面那几节；所以外展方向锁在静止姿那个值上。
  · 骨的局部旋转要由标架对拍定出来，而标架里那个"参考向量"原本沿整条链传播。传到脚
    那几节时它带着上游累积的扭转，于是一节**方向完全没变**的趾骨凭空绕自身轴拧一下，
    把角点压进地里 0.43 px。脚是按世界方向搭的，它的标架就得锚在世界系里（见
    `frames_along` 的 `anchor`）。

## 头

头没有颈——头颅层里它是直接缝在核心上的。所以这一层能给头的动作全部受制于**接合面那
一圈软组织能剪多少**：绕枕髁转 θ 会让癒合环一侧拉伸 θ·r/ℓ，软组织的可用应变约 0.3，
于是 θ_max = 0.3·ℓ/r。算出来只有十几度、位移半像素——**这只兽做不出真正的稳像**，走起来
整颗头跟着躯干甩，只在接合面那点余量里挣扎一下。这不是省事，是"把头直接缝上去"的代价。

在这点余量之内，头做两件都能推的事：
  · **稳像**：抵掉躯干的转动。增益按眼睛动不动得了分——有喙的（禽）眼球几乎不能转，
    稳像全靠头，增益取满；有可动眼球的分掉一部分，取 `GAZE_GAIN`。
  · **耳与下颌是挂着的**：它们不主动动，被躯干的加速度甩。各自按自己的复摆频率算受迫
    响应（增益 1/√((1−r²)²+(2ζr)²)、相位滞后 atan2(2ζr,1−r²)），所以大耳朵甩得慢、
    下颌几乎不动。同一套公式也用在**不承重的闲肢**上——它们本来就是挂着的。

整兽的每颗头另外还带一整套头颅层的动作（嚼/咬/威吓/…），骨名和头颅层完全一致，直接
复用；只是要绕头自己的标架换基——头颅层的动作是在"头朝 −z"那个预览位上写的，而这只兽
的头爱朝哪朝哪。

用法:
  python3 modelScript/creatures/stitched_beast/beast_anim.py --seed 7
  python3 modelScript/creatures/stitched_beast/beast_anim.py --seed 7 --list
  python3 modelScript/creatures/stitched_beast/beast_anim.py --all
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import core_anim as CA  # noqa: E402
import gen_beast as GB  # noqa: E402
import head_anim as HA  # noqa: E402
import heads as HD  # noqa: E402
import limbs as LB  # noqa: E402
import locomotion as LM  # noqa: E402
from bbmodel_maker.rig.anim_rig import (Pose, Rig, build_tracks, euler, euler_of,  # noqa: E402
                      write_bbmodel, write_geckolib)

OUT_DIR = HERE.parents[2] / "models" / "stitched_beast"

# ---------------------------------------------------------------- 常数
ZETA = 0.25       # 生物组织的阻尼比。肌肉黏弹性主导，活体实测 0.2–0.3（观察值）
EPS_SOFT = 0.30   # 软组织可用剪切应变。再大就撕了（观察值）
GAZE_GAIN = 0.55  # 有可动眼球时，稳像分给"转头"的那一份。哺乳类行走时头对躯干的
                  # 稳定增益实测 0.4–0.7；眼球补掉剩下的（观察值）
BODY_N = 128      # 躯干刚体解的稠密采样数（加速度靠它做二阶差分）
STEP_RES = 8      # 每一"步"至少采几帧——最碎步的那条肢决定整条动画的采样数


# ---------------------------------------------------------------- 小工具
def _n(v: np.ndarray) -> np.ndarray:
    return v / max(float(np.linalg.norm(v)), 1e-9)


def align(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    """把单位向量 a 转到 b 的**最小**旋转（Rodrigues）。反向时随便挑一条垂直轴转 180°。"""
    c = float(np.clip(a @ b, -1.0, 1.0))
    if c > 1.0 - 1e-12:
        return np.eye(3)
    v = np.cross(a, b)
    s = float(np.linalg.norm(v))
    if s < 1e-9:                       # 正反向
        w = np.array([1.0, 0.0, 0.0])
        if abs(a[0]) > 0.9:
            w = np.array([0.0, 1.0, 0.0])
        v = _n(np.cross(a, w))
        K = np.array([[0, -v[2], v[1]], [v[2], 0, -v[0]], [-v[1], v[0], 0]])
        return np.eye(3) + 2.0 * K @ K
    K = np.array([[0, -v[2], v[1]], [v[2], 0, -v[0]], [-v[1], v[0], 0]])
    return np.eye(3) + K + K @ K * ((1.0 - c) / (s * s))


def frames_along(pts: list[np.ndarray], ref: np.ndarray,
                 anchor: np.ndarray | None = None,
                 anchor_from: int = 10 ** 9) -> list[np.ndarray]:
    """沿一串关节点建**最小转动标架**：每节一个正交基 [切向, 参考, 切×参考]。

    需要它是因为骨的局部旋转必须由"这一节转到哪"唯一定出来，而只给切向是不够的——绕
    自身轴的扭转还差一个自由度。拿一个正交参考向量沿链传播（每节只做把上一节切向转到
    本节切向的最小旋转），扭转就被压到最小。不这么做的话渲出来的柱子会绕着自己转。

    **但脚那几节不能跟着传播。** 脚是 `foot_chain` 按世界方向（UP / FWD / 固定的外展
    方向）现搭的，方向没变的那一节，它的世界朝向就该原样不动；而传播过来的参考向量带着
    上游几节累积的扭转，会让一节"方向没变"的骨凭空绕自身轴拧一下——趾节因此把角点压进
    地里 0.43 px。所以从 `anchor_from` 起改用一个**世界系里固定**的参考向量 `anchor`，
    静止姿与摆好的姿态用的是同一个，方向不变则旋转恒为单位阵。
    """
    dirs = [_n(b - a) for a, b in zip(pts, pts[1:])]
    out: list[np.ndarray] = []
    r = _n(ref - float(ref @ dirs[0]) * dirs[0])
    for i, u in enumerate(dirs):
        if i >= anchor_from and anchor is not None:
            w = anchor - float(anchor @ u) * u
            if float(np.linalg.norm(w)) > 1e-6:
                out.append(np.column_stack([u, _n(w), np.cross(u, _n(w))]))
                continue
        if i > 0:
            r = _n(align(dirs[i - 1], u) @ r)
            r = _n(r - float(r @ u) * u)
        out.append(np.column_stack([u, r, np.cross(u, r)]))
    return out


def chain_rot(rest: list[np.ndarray], posed: list[np.ndarray],
              n_rest: np.ndarray, n_posed: np.ndarray,
              parent: np.ndarray, anchor: np.ndarray | None = None,
              anchor_from: int = 10 ** 9) -> list[list[float]]:
    """一条骨链的逐骨**局部**欧拉角：让静止的各节转到摆好的各节上。

    骨 j 的世界旋转 Q_j 由标架对拍给出（`frames_along`），局部旋转 = 上一根的世界旋转
    的逆乘自己。链首的"上一根"是父骨——对肢来说就是躯干，所以躯干一歪，腿的根部会自动
    把这一份抵掉，落点仍旧钉在地上。
    """
    Fr = frames_along(rest, n_rest, anchor, anchor_from)
    Fp = frames_along(posed, n_posed, anchor, anchor_from)
    out, prev = [], parent
    for a, b in zip(Fr, Fp):
        Q = b @ a.T
        out.append(euler_of(prev.T @ Q))
        prev = Q
    return out


def resp(f_nat: float, f_drive: float) -> tuple[float, float]:
    """受迫复摆的稳态 (增益, 相位滞后/周期)。挂着的东西——耳廓、下颌、闲肢——都走这一条。

    驱动比 r = f_drive/f_nat：慢驱动（r≪1）跟着走、增益 ~1；共振附近被阻尼压住；快驱动
    （r≫1）跟不上、反相且幅度衰减。所以**大耳朵在走路时甩得明显，下颌几乎不动**——两者
    差别全部来自各自的复摆频率，不是给它们各拍一个幅度。
    """
    if f_nat <= 1e-6:
        return 0.0, 0.0
    r = f_drive / f_nat
    den = math.hypot(1.0 - r * r, 2.0 * ZETA * r)
    gain = 1.0 / max(den, 1e-6)
    lag = math.atan2(2.0 * ZETA * r, 1.0 - r * r) / (2.0 * math.pi)
    return gain, lag


# ---------------------------------------------------------------- 躯干
class Reach:
    """一条承重肢的可达性摘要：腿段能撑多远、髋在哪、脚由哪几节组成。

    `Body` 只需要这些就能回答"躯干不动的话这条肢够不够得着"，不必把整条 `Limb` 拖进来。
    """

    def __init__(self, lb: LB.Limb):
        nfoot = 1 if lb.gene.stance == "sprawling" else 2
        nleg = len(lb.gene.segments) - nfoot
        self.span = 0.97 * sum(lb.gene.segments[:nleg])   # 和 solve_limb 同一条判据
        self.hip = np.asarray(lb.joints[0], float)
        self.foot_segs = lb.gene.segments[nleg:]
        self.stance = lb.gene.stance


class Body:
    """躯干的刚体运动：**只被"腿够不够得着"逼着动**（见模块 docstring 一）。

    不是"每条腿要求躯干在多高、取个折中"——腿的长度在这套模型里本来就是自由的
    （`refold` 随时改折叠量）。唯一硬的东西是可达性：掌骨立到极限之后仍够不着落点的
    那条肢，会把躯干往下拽，拽到刚好够为止。够得着的时候躯干一动不动。
    """

    def __init__(self, gait: LM.Gait, limbs: dict[str, LB.Limb], n: int = BODY_N):
        self.gait = gait
        self.T = 1.0 / max(gait.body_hz, 1e-6)
        self.pivot = np.asarray(gait.com, float)
        com2 = np.array([gait.com[0], gait.com[2]])
        self.reach = {lg.gene.socket: Reach(limbs[lg.gene.socket])
                      for lg in gait.limbs}
        h = np.zeros(n)
        rl = np.zeros(n)
        pt = np.zeros(n)
        self.tight = 0.0        # 全周期最紧的"需求/可用"比——>1 才会逼降躯干
        for k in range(n):
            t = k / n
            h[k], rl[k], pt[k] = self._fit(t, com2)
        self.heave, self.roll, self.pitch = h, rl, pt
        self.n = n

    def _fit(self, t: float, com2: np.ndarray) -> tuple[float, float, float]:
        """这一帧躯干得下沉/歪多少，腿才都够得着。够得着就一动不动，返回全零。"""
        rows, rhs = [], []
        for lg in self.gait.limbs:
            if not lg.in_stance(t):
                continue
            rc = self.reach[lg.gene.socket]
            Lm, hip, segs, stance = rc.span, rc.hip, rc.foot_segs, rc.stance
            con = lg.foot_at(t)
            hz = np.array([con[0] - hip[0], 0.0, con[2] - hip[2]])
            nn = float(np.linalg.norm(hz))
            out = hz / nn if nn > 1e-6 else np.array([1.0, 0.0, 0.0])
            # 掌骨已经立到极限（88°，`meta_deg` 的上界）——肢自己能做的都做完了
            ank = LB.foot_chain(stance, con, segs, out, 88.0)[0]
            dxz = float(np.hypot(hip[0] - ank[0], hip[2] - ank[2]))
            self.tight = max(self.tight, float(np.linalg.norm(hip - ank)) / max(Lm, 1e-9))
            if dxz >= Lm:                       # 连水平距离都超了：只能整只压到踝的高度
                need = float(hip[1]) - float(ank[1])
            else:
                need = float(hip[1]) - (float(ank[1]) + math.sqrt(Lm * Lm - dxz * dxz))
            # **每条支撑肢都进方程，够得着的那条要求 0**。只把"在喊"的那几条放进来的话，
            # 一条肢跨过"够得着/够不着"的界限时方程的**结构**会变（行数变、少于三行时还
            # 会换一套解法），解跟着跳——实测 seed 1 的躯干在相邻两帧间跳了 12.84 px。
            rows.append([1.0, float(hip[0]) - com2[0], float(hip[2]) - com2[1]])
            rhs.append(-max(need, 0.0))
        if not rows:
            return 0.0, 0.0, 0.0
        A = np.array(rows)
        b = np.array(rhs)
        sol, *_ = np.linalg.lstsq(A, b, rcond=None)
        # **最小二乘是折中，折中会亏待最惨的那一条。** 可达性是硬约束不是愿望：解完平面
        # 还得把残余的欠量整只补下去，否则那条肢仍旧够不着，`refold` 只能夹到可达边界，
        # 腿与脚就此脱开——渲出来是脚陷进地里（实测 0.31 px）。补的是均匀下沉，不再歪。
        sol[0] += float(min((b - A @ sol).min(), 0.0))
        a, bx, cz = (float(v) for v in sol)
        roll = math.degrees(math.asin(float(np.clip(bx, -0.6, 0.6))))
        pitch = math.degrees(math.asin(float(np.clip(-cz, -0.6, 0.6))))
        return a, roll, pitch

    # ---------- 取值 ----------
    def _lerp(self, arr: np.ndarray, t: float) -> float:
        x = (t % 1.0) * self.n
        i = int(math.floor(x)) % self.n
        f = x - math.floor(x)
        return float(arr[i] * (1.0 - f) + arr[(i + 1) % self.n] * f)

    def at(self, t: float) -> tuple[float, float, float]:
        return (self._lerp(self.heave, t), self._lerp(self.roll, t),
                self._lerp(self.pitch, t))

    def matrix(self, t: float) -> np.ndarray:
        _a, roll, pitch = self.at(t)
        return euler([pitch, 0.0, roll])

    def channel(self, t: float) -> tuple[list[float], list[float]]:
        """root 骨的 (rot, pos)。root 的原点在 (0,0,0)，所以绕质心转要靠 pos 补回来。"""
        a, roll, pitch = self.at(t)
        R = euler([pitch, 0.0, roll])
        p = self.pivot - R @ self.pivot + np.array([0.0, a, 0.0])
        return [pitch, 0.0, roll], [float(p[0]), float(p[1]), float(p[2])]

    def apply(self, t: float, pt: np.ndarray) -> np.ndarray:
        rot, pos = self.channel(t)
        return euler(rot) @ np.asarray(pt, float) + np.array(pos)

    def accel(self, t: float, pt: np.ndarray) -> np.ndarray:
        """某个体上固定点的世界加速度（px/s²）。二阶中心差分，周期上取。"""
        dt = 1.0 / self.n
        p0 = self.apply(t - dt, pt)
        p1 = self.apply(t, pt)
        p2 = self.apply(t + dt, pt)
        return (p0 - 2.0 * p1 + p2) / (dt * self.T) ** 2

    @property
    def sway(self) -> float:
        """升沉的全幅（px）——摆动腿要留的净空里有它一份。"""
        return float(self.heave.max() - self.heave.min())


# ---------------------------------------------------------------- 肢
class LimbPlan:
    """一条肢在整个步态周期里的解法。**站姿定下来的协同这里只读不改**（见 limbs.synergy）。"""

    def __init__(self, lb: LB.Limb, lg: LM.LimbGait | None, body: Body):
        self.lb = lb
        self.lg = lg
        self.body = body
        self._nrm: np.ndarray | None = None
        segs = lb.gene.segments
        self.n = len(segs)
        if lg is None:                       # 挂着的：整条被躯干的加速度甩
            self.nleg = self.n
            self.hz = LM.natural_hz(segs)
            self.gain, self.lag = resp(self.hz, body.gait.body_hz)
            return
        self.nfoot = 1 if lb.gene.stance == "sprawling" else 2
        self.nleg = self.n - self.nfoot
        self.leg_segs = segs[:self.nleg]
        self.foot_segs = segs[self.nleg:]
        rest_leg = lb.joints[:self.nleg + 1]
        self.w, self.phi0 = LB.synergy(rest_leg, lb.gene.kind)
        # 摆动净空：定长腿荡过去会扎进地里的那一份（`LimbGait.swing_lift`，运动层算的）
        # + 躯干自己的升沉全幅。前一项现在归运动层——碰撞检测和动画必须用**同一条轨迹**，
        # 各写一份的话 `_splay` 掰的是另一条腿（实测它只看到 1.12 px 而实际穿 1.97 px）。
        self.clear = lg.swing_lift + body.sway
        # **踩住的脚不会自己转。** `foot_chain` 的外展方向 `out` 原本按当下的髋→落点重算，
        # 于是腿一前后扫、外展方向一转，跗节（连带贴在它上面的那片甲板）就绕着自己拧——
        # 实测蛛足的甲板因此比静止时多扎进地里 0.54 px。真实的踩地是跗节钉住不动，摆动的
        # 是它上面那几节。所以外展方向锁在静止姿那个值上。只有 sprawling 用得着它，其余
        # 站姿的脚本来就只按 UP/FWD 摆。
        d0 = np.asarray(lg.foot, float) - np.asarray(lb.joints[0], float)
        d0[1] = 0.0
        self.out0 = _n(d0) if float(np.linalg.norm(d0)) > 1e-6 else np.array([1.0, 0.0, 0.0])
        # 脚的侧向轴：`foot_chain` 只在 (外展方向, UP, FWD) 张成的世界方向上摆，绕的是
        # 这条轴。静止姿和每一帧共用它，方向没变的骨就一动不动。
        lat = np.cross(np.array([0.0, 1.0, 0.0]), self.out0)
        self.lateral = _n(lat) if float(np.linalg.norm(lat)) > 1e-6 else np.array([1.0, 0.0, 0.0])
        self.phi_tab = self._march()

    def _march(self, n: int = BODY_N) -> np.ndarray:
        """整周期的折叠量 φ(t)，**逐帧接力解出来**。

        `fold_phi` 取离 hint 最近的解支，而 Z 形链的同一个首尾距离有好几个解支。拿静止
        姿那个固定的 φ₀ 当 hint 不够：走到某些相位时另一支更近，解就跳过去——实测蛛足
        limb_ml 在 t=0.07 一帧跳了 11.71 px，而它一步才走 9.18 px。

        所以在这里把整条 φ(t) 一次性接力算完（走两圈让首尾也接上），查询时插值。这样
        既连续，又不需要按顺序采样——`sample()` 可以被渲染器以任意 t 调用。
        """
        tab = np.zeros(n)
        hint = self.phi0
        for _ in range(2):                     # 第二圈让 t=0 处也用上绕回来的 hint
            for k in range(n):
                t = k / n
                hip = self.body.apply(t, self.lb.joints[0])
                con = self.contact(t)
                deg = LB.meta_angle(self.lb.gene.stance, con, hip, self.out0,
                                    self.foot_segs, sum(self.leg_segs))
                ank = LB.foot_chain(self.lb.gene.stance, con, self.foot_segs,
                                    self.out0, deg)[0]
                _eb, _ed, L = LB.bend_plane(hip, ank, self.lb.gene.kind)
                hint = LB.fold_phi(self.leg_segs, self.w, L, hint)
                tab[k] = hint
        return tab

    def phi_at(self, t: float) -> float:
        n = len(self.phi_tab)
        x = (t % 1.0) * n
        i = int(math.floor(x)) % n
        f = x - math.floor(x)
        return float(self.phi_tab[i] * (1.0 - f) + self.phi_tab[(i + 1) % n] * f)

    # ---------- 落点 ----------
    def contact(self, t: float, *, still: bool = False) -> np.ndarray:
        """t 时刻脚的接触点（体坐标系）。支撑相贴地后移，摆动相简谐前甩并抬起。

        `still=True` 是站着不动：脚钉在中立落点上。站姿动画里核心照旧在搏动，脚却不该
        跟着上下——那一份必须由腿的屈伸吃掉，所以站姿也要走同一条重折路径。
        """
        lg = self.lg
        assert lg is not None
        if still:
            return np.asarray(lg.foot, float)
        return lg.foot_at(t, self.body.sway)

    def joints(self, t: float, hip: np.ndarray | None = None, *,
               still: bool = False) -> tuple[list[np.ndarray], np.ndarray]:
        """t 时刻这条肢的各关节世界坐标 + 折弯平面法向。

        `hip` 由调用方从**父骨的真实世界矩阵**里取——核心一搏动，挂载点跟着被推出去，
        那一份位移必须进到髋里。自己算的话腿会浮起来（实测脚扎进地里 1.5 px）。
        """
        lb = self.lb
        if hip is None:
            hip = self.body.apply(t, lb.joints[0])
        con = self.contact(t, still=still)
        deg = LB.meta_angle(lb.gene.stance, con, hip, self.out0,
                            self.foot_segs, sum(self.leg_segs))
        paw = LB.foot_chain(lb.gene.stance, con, self.foot_segs, self.out0, deg)
        nrm = self.rest_normal()
        leg, _phi = LB.refold(hip, paw[0], self.leg_segs, lb.gene.kind, self.w,
                              self.phi0 if still else self.phi_at(t), nrm)
        e_b, e_d, _L = LB.bend_plane(hip, paw[0], lb.gene.kind, nrm)
        return leg + paw[1:], _n(np.cross(e_b, e_d))

    def rest_normal(self) -> np.ndarray:
        """站姿那个折弯平面的法向。**算一次就不再变**——它是这条腿"往哪边折"的身份，
        逐帧重算会在肢体接近竖直时翻面（见 `limbs.bend_plane`）。"""
        if self._nrm is None:
            e_b, e_d, _L = LB.bend_plane(self.lb.joints[0],
                                         self.lb.joints[self.nleg], self.lb.gene.kind)
            self._nrm = _n(np.cross(e_b, e_d))
        return self._nrm

    # ---------- 姿态 ----------
    def pose(self, p: Pose, t: float, R_body: np.ndarray, W: dict, rig: Rig,
             *, still: bool = False) -> None:
        lb = self.lb
        name = lb.sock.name
        root = f"limb_{name}_0"
        par = rig.bones[root].parent
        A = W[par][:3, :3] if par else np.eye(3)
        # **抵掉父骨的均匀缩放**。核心搏动是给 lobe 加 scale 的，而肢挂在 lobe 上——不抵
        # 的话核心一鼓腿跟着长 4%，脚直接扎进地里 1.5 px，而且旋转永远回代不上（纯旋转
        # 变不出长度变化）。挂载点本身该被推出去，所以只抵缩放不抵位移。
        sc = float(np.linalg.norm(A[:, 0]))
        if abs(sc - 1.0) > 1e-6:
            p[root].scale = [1.0 / sc] * 3
        hip = (W[par] @ np.append(lb.joints[0], 1.0))[:3] if par else lb.joints[0]
        if self.lg is None:
            self._hang(p, t, R_body)
            return
        posed, n_pose = self.joints(t, hip, still=still)
        # 脚那几节锚在世界系的侧向轴上（`foot_chain` 就是按世界方向搭的），见 frames_along
        rots = chain_rot(lb.joints, posed, self.rest_normal(), n_pose, R_body,
                         self.lateral, self.nleg)
        for j, r in enumerate(rots):
            p[f"limb_{name}_{j}"].rot = list(r)

    def _hang(self, p: Pose, t: float, R_body: np.ndarray) -> None:
        """挂着的肢：整条绕挂载点摆。受迫复摆的稳态响应，相位滞后由阻尼给出。"""
        lb = self.lb
        a = self.body.accel(t - self.lag, lb.joints[0])
        a[1] = 0.0                             # 竖直分量只改变"垂"的松紧，不改朝向
        mag = float(np.linalg.norm(a))
        if mag < 1e-6:
            return
        # 挂着的东西朝**合加速度的反向**偏：等效重力 = g − a
        theta = math.atan2(mag * self.gain, LM.G_PX)
        axis = _n(np.cross(np.array([0.0, -1.0, 0.0]), -a / mag))
        K = np.array([[0, -axis[2], axis[1]], [axis[2], 0, -axis[0]],
                      [-axis[1], axis[0], 0]])
        Q = np.eye(3) + math.sin(theta) * K + (1.0 - math.cos(theta)) * K @ K
        p[f"limb_{lb.sock.name}_0"].rot = list(euler_of(R_body.T @ Q @ R_body))


# ---------------------------------------------------------------- 头
def head_matrix(hd: HD.Head) -> np.ndarray:
    """头自己的标架（右/上/前）。头颅层的动作是在"头朝 −z"的预览位上写的，装到这只兽
    身上要绕这个标架换基，否则一颗朝天长的头会绕世界 x 轴点头——那是**横着**摇。"""
    return np.column_stack([hd.e_r, hd.e_u, -hd.e_f])


def graft_range(hd: HD.Head) -> tuple[float, float]:
    """接合面能给出的 (最大转角°, 最大位移 px)。**这只兽没有颈，头的活动范围只有这么多。**

    癒合环是一段长 ℓ、半径 r 的软组织套（`heads._assemble` 里那圈 collar）。绕枕髁转 θ
    让一侧拉伸 θ·r/ℓ，平移 δ 让它剪切 δ/ℓ；软组织可用应变约 0.3，两式反解即得上限。
    算出来是十几度、半个像素——所以下面所有头部动作都被钳在这个尺度里，不是它们做不出，
    是缝上去的头本来就只有这点余量。
    """
    bw, bh = hd.brain_px[2], hd.brain_px[1]
    ell = hd.L * 0.11 + hd.standoff
    r = max(bw * 0.58, bh * 0.56, 1e-6)
    return math.degrees(EPS_SOFT * ell / r), EPS_SOFT * ell


def head_passive(hd: HD.Head, p: Pose, t: float, body: Body) -> None:
    """走路时头做的事：在接合面的余量里稳像，耳与下颌被甩。"""
    b = HA.bones(hd)
    cap_deg, cap_px = graft_range(hd)
    _a, roll, pitch = body.at(t)
    gain = 1.0 if hd.donor.beak else GAZE_GAIN     # 眼球转不了的，稳像全靠头
    p[b["head"]].rot = [float(np.clip(-gain * pitch, -cap_deg, cap_deg)), 0.0,
                        float(np.clip(-gain * roll, -cap_deg, cap_deg))]
    # 稳住高度：躯干升沉多少就反向压多少，同样钳在接合面的余量里
    dy = float(np.clip(-gain * _a, -cap_px, cap_px))
    dz = 0.0
    if hd.donor.beak:
        # 禽类眼球几乎不动，靠"定点—前冲"稳像。定点意味着体坐标系里头要往后滑，滑的量
        # 是身体这段时间走过的距离——但没有颈，滑不动，只剩接合面这半个像素的顿挫。
        step = body.gait.speed * body.T
        u = (t * 2.0) % 1.0
        dz = float(np.clip(step * (u - 0.5), -cap_px, cap_px))
    p[b["head"]].pos = [0.0, dy, dz]
    for key, ln in (("ear_l", hd.ear_plate[0]), ("ear_r", hd.ear_plate[0])):
        if key not in b or ln <= 1e-6:
            continue
        gn, lag = resp(LM.natural_hz((float(ln),)), body.gait.body_hz)
        a2 = body.accel(t - lag, hd.org)
        sgn = -1.0 if key == "ear_l" else 1.0
        p[b[key]].rot = [math.degrees(math.atan2(gn * float(a2[2]), LM.G_PX)),
                         sgn * math.degrees(math.atan2(gn * float(a2[0]), LM.G_PX)),
                         0.0]
    jaw_len = max(abs(hd.tmj[1] - hd.bite_px), 1e-6)
    gn, lag = resp(LM.natural_hz((jaw_len,)), body.gait.body_hz)
    a2 = body.accel(t - lag, hd.org)
    # 下颌只能往下掉，掉多少由竖直加速度定：身体往上顶时下颌被甩开
    p[b["jaw"]].rot = [max(0.0, math.degrees(math.atan2(gn * float(a2[1]),
                                                        LM.G_PX))), 0.0, 0.0]


def rebase(src: Pose, hd: HD.Head) -> Pose:
    """把头颅层写在"头朝 −z"预览位上的动作换到这颗头自己的标架上。"""
    F = head_matrix(hd)
    out = Pose()
    for k, ch in src.items():
        c = ch.copy()
        if any(c.rot):
            c.rot = euler_of(F @ euler(c.rot) @ F.T)
        if any(c.pos):
            c.pos = list(F @ np.array(c.pos, float))
        out[k] = c
    return out


# ---------------------------------------------------------------- 装配
class Beast:
    def __init__(self, seed: int):
        self.seed = seed
        self.rig, self.gait, self.limbs, self.heads = GB.build(seed)
        self.model = self.rig.save(OUT_DIR / f"BeastRig_{seed}.bbmodel",
                                   f"BeastRig_{seed}")
        self.body = Body(self.gait, self.limbs)
        by_sock = {lg.gene.socket: lg for lg in self.gait.limbs}
        self.plans = [LimbPlan(lb, by_sock.get(n), self.body)
                      for n, lb in self.limbs.items()]
        self.T = self.body.T
        steps = max([lg.steps for lg in self.gait.limbs] or [1])
        self.samples = int(np.clip(STEP_RES * steps, 32, 96))

    # ---------- 动作 ----------
    def _stand(self, rig: Rig, t: float, length: float,
               *, still: bool) -> Pose:
        """走与站共用的装配。差别只有一处：脚是在走还是钉着。"""
        p = CA.anim_idle(rig, t, length)       # 核心照旧在喘，芽压回休眠尺寸
        rot, pos = self.body.channel(t)
        # **核心搏动那一份竖直位移必须交给肢体去抵**，所以从 root 通道里取出来单独传下去，
        # 不能让它只留在 root 上——留在 root 上的话，核心一鼓整只兽连脚一起浮起来。
        lift = float(p["root"].pos[1])
        p["root"].rot = list(rot)
        p["root"].pos = [pos[0], pos[1] + lift, pos[2]]
        # **两遍**：躯干与各 lobe 摆完才知道每个挂载点被搏动推到了哪，肢体第二遍才解得准
        W = rig.world(p)
        R = euler(rot)
        for pl in self.plans:
            pl.pose(p, t, R, W, rig, still=still)
        return p

    def walk(self, rig: Rig, t: float, length: float) -> Pose:
        p = self._stand(rig, t, length, still=False)
        for hd in self.heads.values():
            head_passive(hd, p, t, self.body)
        return p

    def idle(self, rig: Rig, t: float, length: float) -> Pose:
        """站着：核心搏动 + 头各喘各的 + 挂着的肢被那点起伏带着晃。"""
        p = self._stand(rig, t, length, still=True)
        for hd in self.heads.values():
            for k, ch in rebase(HA.anim_idle(hd, rig, t, length), hd).items():
                p[k] = ch
        return p

    def head_action(self, hd: HD.Head, fn, rig: Rig, t: float, length: float) -> Pose:
        p = Pose()
        for k, ch in rebase(fn(hd, rig, t, length), hd).items():
            p[k] = ch
        return p

    def anims(self) -> dict:
        out: dict[str, tuple[float, bool, int, object]] = {
            "beast_walk": (self.T, True, self.samples, self.walk),
            "beast_idle": (max(4.0, self.T * 2.0), True, 40, self.idle),
        }
        for hd in self.heads.values():
            for name, (ln, loop, n, fn) in HA.anims(hd).items():
                key = f"{name.replace('head_', 'head_' + hd.name + '_', 1)}"
                out[key] = (ln, loop, n,
                            (lambda h, f: lambda rig, t, length:
                             self.head_action(h, f, rig, t, length))(hd, fn))
        return out


# ---------------------------------------------------------------- 渲染器接口
MODEL: Path | None = None
ANIMS: dict = {}
_BE: Beast | None = None


def use(seed: int) -> Beast:
    global MODEL, ANIMS, _BE
    _BE = Beast(seed)
    MODEL, ANIMS = _BE.model, _BE.anims()
    return _BE


def sample(rig: Rig, name: str, t: float) -> Pose:
    length, _loop, _n, fn = ANIMS[name]
    return fn(rig, t, length)


def export(seed: int, *, quiet: bool = False) -> tuple[Path, Path]:
    be = use(seed)
    rig = Rig(be.model)
    entries = []
    for name, (length, loop, n, fn) in ANIMS.items():
        tracks = build_tracks(rig, lambda t, f=fn, ln=length: f(rig, t, ln),
                              length, loop, n)
        entries.append((name, length, loop, tracks))
        if not quiet:
            kf = sum(len(v) for c in tracks.values() for v in c.values())
            print(f"  {name:<22} {length:>5.2f}s {'循环' if loop else '单次'}  "
                  f"{len(tracks):>3} 骨  {kf:>5} 关键帧")
    out_a = OUT_DIR / f"BeastAnim_{seed}.bbmodel"
    out_g = HERE / f"stitched_beast_{seed}.animation.json"
    write_bbmodel(be.model, out_a, f"BeastAnim_{seed}", entries)
    write_geckolib(out_g, "bong", f"stitched_beast_{seed}", entries)
    return out_a, out_g


def report(seed: int) -> str:
    be = use(seed)
    g = be.gait
    rows = [f"seed {seed}：{len(be.limbs)} 肢（承重 {len(g.limbs)}）  "
            f"{len(be.heads)} 头  周期 {be.T:.2f}s（{g.body_hz:.2f} Hz）  "
            f"行走 {g.blocks_per_sec:.2f} 格/s  采样 {be.samples}",
            f"  躯干：升沉 {be.body.sway:.2f} px  "
            f"横滚 ±{max(abs(be.body.roll.min()), abs(be.body.roll.max())):.1f}°  "
            f"俯仰 ±{max(abs(be.body.pitch.min()), abs(be.body.pitch.max())):.1f}°  "
            f"最紧可达 {be.body.tight:.2f}"
            f"{'（逼降躯干）' if be.body.tight > 1.0 else '（无逼降）'}"]
    for pl in sorted(be.plans, key=lambda x: x.lb.sock.name):
        if pl.lg is None:
            rows.append(f"  {pl.lb.sock.name:<10} 挂着  复摆 {pl.hz:.2f} Hz  "
                        f"受迫增益 {pl.gain:.2f}  滞后 {pl.lag:.2f} 周期")
        else:
            rows.append(f"  {pl.lb.sock.name:<10} {pl.lg.steps} 步/周期  "
                        f"占空 {pl.lg.duty:.2f}  行程 {pl.lg.excursion:5.2f} px  "
                        f"抬升 {pl.clear:5.2f} px")
    for hd in sorted(be.heads.values(), key=lambda h: h.name):
        cap, px = graft_range(hd)
        rows.append(f"  {hd.name:<10} {hd.kind:<8} 接合面余量 {cap:5.1f}° / {px:4.2f} px"
                    f"{'  （喙：定点—前冲）' if hd.donor.beak else ''}")
    return "\n".join(rows)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()

    seeds = list(range(1, 13)) if args.all else [args.seed]
    for s in seeds:
        if args.list:
            print(report(s))
            continue
        print(f"seed {s}：")
        a, g = export(s, quiet=args.all)
        print(f"  → {a}\n  → {g}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
