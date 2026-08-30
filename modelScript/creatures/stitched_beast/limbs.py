#!/usr/bin/env python3
"""异变缝合兽 —— 部件层·肢体：一条捡来的腿在这具身体上会长成什么样。

**肢体的粗细不是画出来的，是称出来的。** 基因组只给了每条肢的**节段长度**和它是从
哪只野兽身上拆的；那是死掉的供体留下的全部信息。剩下的一切——每一节多粗、怎么弯、
脚掌多大、哪一段还看得出原主人——都由这具身体的重量和这条肢在站姿里的位置决定。

推导链（每一步都能单独核验，见 `check_limbs.py`）：

  ① **体重**从核心的实心体素数来（ρ=1000 kg/m³，肉的密度）。1.38 m³ → 1381 kg。
  ② **每条肢分到多少载荷**由静力平衡解，不是按条数平摊。三点着地时竖直反力是
     **静定**的（ΣF=W、绕两轴的力矩为零，三个未知数三个方程）；着地多于三只时取
     最小范数解并把负反力（脚在往下拽地）逐轮剔除。结果是**离质心近的短肢扛大头**
     （实测某只兽的 13 px 腹肢独扛 71%），而远远撑出去的长肢主要在维持平衡（26%）。
     按 W/n 平摊会把这个反过来算。
  ③ **支撑相内的峰值**再乘 π/2：一条肢的地面反力在自己的支撑相里是半个正弦，
     均值必须等于②解出的准静态份额，于是峰值 = 均值 × π/2。这个因子是推出来的，
     不是拍的安全余量。
  ④ **姿态**由「够到 locomotion 解出的落点」唯一确定。整条链绕根部旋转是刚体运动，
     不改变首尾距离，所以折叠量 φ 与朝向 θ₀ 可以分开解：先二分 φ 让链的首尾距离
     等于 |髋−脚|，再一步算出 θ₀。弯的**方向**是供体的事实（兽腿膝前肘后、禽腿踝
     反折、蛛足先抬后落），写在 `BEND` 里。
  ⑤ **骨半径**取三条失效判据的上界：轴压屈服、欧拉屈曲、**弯曲**。实测弯曲永远
     主导（另两条小一个数量级）——腿是被掰断的，不是被压扁的。力矩臂是该关节到
     地面接触点的**水平**距离，所以站得越开、腿越粗。
  ⑥ **肉的粗细**由肌肉截面给：抗重力肌必须产生 τ/a 的力，a 是肌肉力臂（取相邻较长
     一节的 ARM_RATIO）。肌腹长在关节**近端**那一节上，更远的关节只有腱穿过——腱比
     肌肉强两百倍，所以远端细。这就是腿为什么上粗下细，不是造型偏好。而根部那一节
     还要额外背上**嫁接关节自己的肌肉**：脊椎动物把髋伸肌放在骨盆上，这东西的核心是
     个没有骨架的肉囊，锚不住任何东西。肌腹粗不过它所在那一节长度的一半（再粗就不是
     肌肉是个球），**装不下的那份只能长在体内**——蜘蛛的基节肌本来就在体腔里。
  ⑦ **材质分界**同样是算的：一节的横截面里本体新长的肌腹与供体原有的骨+腱谁占多数，
     外面就露谁。于是**脚永远是别人的**（那一节按定义没有肌腹），一眼看得出供体物种。
  ⑧ **脚掌**面积 = 峰值载荷 / 地面承载力。载荷大的脚自然大。
  ⑨ **不承重的肢**（触手 / 退化残肢 / 够不着地的）载荷为零，粗细改由**组织预算**定
     （`core.graft_budget`，和芽同一份料）：同样一份料，36 px 的触手摊成一条细绳，
     6.9 px 的残肢缩成一个肉疙瘩。垂下来的姿态分两种——有骨的自由铰在第一个关节硬折
     后笔直垂下，无骨的软梁按悬臂自重挠度连续弯；垂到地面的就摊在地上拖着。

**粗细跟"它原来多粗"完全无关**——供体的粗细信息一次都没进过公式。进公式的只有两样：
它在这具身体的站姿里分到多少载荷（跟种类无关），和它的节长比例给出多大的肌肉杠杆
（种类的事）。所以：

  · 同一种类内部粗细差 2–4 倍（位置说了算）
  · 但种类仍然解释了约一半的粗细方差（37 条肢实测 49%），因为杠杆比例是种类带来的

最刺眼的一条由此掉出来：**蛛足要的肌肉这具身体给不起**。它的基节只有 2.5 px 长、肌肉
力臂小得可怜，同时腿又最长、地面反力的力矩最大，算出来根部需求 9–19 px（兽腿只要
4–9 px）。而肌腹粗不过基节长度的一半 ⇒ 96% 的驱动肌只能塞在体腔里。所以一条蛛足在
外面看是**细的**，鼓起来的是它旁边那团核心。捡到蛛足是坏事，这是算出来的，不是设定的。

用法:
  python3 modelScript/creatures/stitched_beast/limbs.py --seed 7
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
import locomotion as LM  # noqa: E402
from bbmodel_maker.rig import voxel_rig as VR  # noqa: E402

# ---------------------------------------------------------------- 物性常数
# 全部是**量出来的真实值**，不是可以拧的旋钮。改这里等于改物理，不是改风格。
PX = 1.0 / 16.0            # 一像素多少米（16 px = 1 格 = 1 m）
RHO = 1000.0               # 肉的密度 kg/m³（≈水）
G = 9.81
SIGMA_BONE = 170e6         # 皮质骨抗压/抗弯强度 Pa
E_BONE = 18e9              # 皮质骨杨氏模量 Pa
SIGMA_MUSCLE = 0.3e6       # 肌肉最大等长应力 Pa（生理横截面积 × 30 N/cm²）
SIGMA_TENDON = 70e6        # 腱的工作应力 Pa
SIGMA_SOFT = 0.5e6         # 无骨软组织（触手）的抗拉强度 Pa
E_SOFT = 0.5e6             # 无骨软组织杨氏模量 Pa
ARM_RATIO = 0.12           # 肌肉力臂 / 相邻较长一节的长度
BEARING = 1.0e5            # 地面承载力 Pa（松散废土，≈松砂）
PEAK_SHAPE = math.pi / 2   # 支撑相半正弦的峰值/均值比——推出来的，见模块 docstring ③
SAFETY = 2.0               # 骨的安全系数。活体实测普遍 2–4（Biewener），这东西是
                           # 捡来的废件拼的，取区间下沿
PENALTY = 400.0            # 姿态求解里"每穿模一像素"折算成多少肌肉代价。取到足以压过
                           # 肌肉项的量级即可——它表达的是"宁可多长肉也不要穿模"
CORE_PENALTY = 1.0 * PENALTY   # 穿核心与穿邻居**同权**。两个方向都试过，都更糟：给核心
                           # 2.5 倍 ⇒ 互穿从 0 反弹回 4.24 px；给邻居的罚项封顶 ⇒ 同样
                           # 反弹。两条罚项争的是同一个**无解**的局面，谁重谁就全赢，
                           # 调权重只是在两种错之间搬家。真正能动的自由度是**落点**
                           # （见 `_splay`）——姿态只能改折法，改不了这条腿要伸到哪去。

# 骨外面那层软组织的厚度。**这一条不是推出来的，是量出来的**，得说清楚：上面每个
# 半径都是"某样东西必须扛住某个力"解出来的，而腱鞘、筋膜、皮下、血管、皮这几层不扛
# 主要载荷——力矩归零它们也不会消失。漏掉它们的后果是远端那几节的渲染半径**恰好等于
# 骨半径**，也就是把骨头直接画在外面（用户第一句话："看着应该没安装上肌肉和皮肤吧"）。
#
# 量出来的比例：远端肢的外径 ≈ 骨径的两倍。马的管骨 ø35 mm，整个管部连屈腱带皮
# ø70–80 mm；狼、禽的跗跖同量级。所以这里取 2.0，作用是给每一节一个**下限**。
#
# 注意皮**不在**这个厚度里起作用：1 px = 6.25 cm，兽皮 2–5 mm 连十分之一像素都不到。
# 皮在这个尺度上不是厚度，是**表面**——它的存在只能靠材质和裂痕表达（见
# `gen_beast.part_cracks`），加粗一个像素来"表示有皮"是假的。
SOFT_OVER_BONE = 2.0

# 关节弯折方向。这是**供体的解剖事实**，不是造型选择：兽腿的膝向前折、禽腿的膝藏在
# 体羽里而外面看到的"反折膝"其实是踝、蛛足先把腿节抬到身体之上再落下去。
#
# **符号约定**（只取正负，幅度不用）：`+` = 这个关节朝参考方向鼓出去。参考方向由铰链
# 轴决定，见 `stand_pose`：兽/禽的膝是横轴铰链 ⇒ 参考方向是**正前方**；蛛足是外展类，
# 折在通过径向的竖直面里 ⇒ 参考方向是**正上方**。
#
# 这三行原先是按旧的（径向、且正负相反的）平面调出来的，换成上面这套约定之后，禽和蛛
# 的符号与本注释描述的解剖事实正好相反——实测表现为**每一条禽腿蛛足都被判"被迫改折向"**
# （力学解一律把它们翻回来）。既然力学和注释一致、只有表和它们不一致，那就是表错了。
BEND: dict[str, tuple[float, ...]] = {
    "mammal": (1.0, -1.0, 1.0),      # 膝朝前，其后逐节交替
    "bird": (1.0, -1.0, 1.0),        # 膝同样朝前——反折的是踝，由 foot_chain 摆
    "spider": (1.6, 1.0, -0.4),      # 腿节先抬到身体之上，再落下去（第三项用不到）
    "tentacle": (1.0,) * 5,
    "vestigial": (1.0,),
}

# **掌骨相对地面的倾角**——这一个数字就把三类腿分开了，是解剖学分类不是造型选择：
#   · 蹄行（羊/牛/猪）掌骨竖成一根管，只有蹄尖着地 ⇒ 踝抬得最高，小腿以下是一根细杆
#   · 趾行（狼/狐/兔/禽）脚跟离地、踮着趾走 ⇒ 中间那个"向后折的关节"其实是踝
#   · 跖行（人/鼠）整个脚掌贴地 ⇒ 踝低、脚后跟支出来一块
# 同样四根骨、同样的载荷，站姿一换剪影就完全不同。上一版没有这一层，六条腿一个样。
META_DEG = GN.META_DEG          # 解剖数据，和 locomotion 共用一份（见 genome）
FWD = np.array([0.0, 0.0, -1.0])     # 前进方向：脚趾一律朝前，不管这条腿从哪边支出去
UP = np.array([0.0, 1.0, 0.0])

# 供体的体表。捡来的那截还看得出是从谁身上拆的——这是观察不是设计。
DONOR_MAT: dict[str, str] = {
    "spider": "chitin",
    "mammal": "hide",
    "bird": "scute",
    "tentacle": "flesh_deep",
    "vestigial": "necro",
}

LIMB_MATS: dict[str, tuple[int, int, int]] = {
    "chitin": (48, 42, 46),      # 蛛甲：几乎黑，有蜡质高光
    "hide": (86, 70, 58),        # 兽皮：脏褐
    "scute": (122, 110, 78),     # 禽腿鳞片：土黄
    "necro": (74, 68, 62),       # 干掉的残肢
    "graft": (146, 116, 108),    # 本体长上去的新肉（比核心 flesh_pale 略红）
    "graft_deep": (96, 72, 74),
    "collar": (108, 64, 62),     # 根部癒合环
    "hoof": (58, 50, 44),        # 蹄：角质，最暗
    "claw": (40, 36, 34),        # 爪/钩
    "pad": (92, 74, 72),         # 肉垫
    "fur": (104, 88, 74),        # 兽毛（比 hide 亮一档才看得出蓬松边缘）
    "wool": (168, 158, 140),     # 羊毛：全身最亮的一处，扎眼是对的
    "bristle": (70, 60, 52),     # 猪鬃
    "plume": (118, 104, 92),     # 禽腿上段的覆羽——鳞只长在跗跖骨上，大腿是有毛的
    "crack": (34, 26, 24),       # 裂口：看见的是**裂缝底下的暗**，不是一条画上去的线
}


# ---------------------------------------------------------------- 体重与载荷
def body_weight() -> float:
    """这具身体的重量（N）。从核心的**实心**体素数来——表层壳算的是面积不是体积。"""
    return len(C.solid_grid()) * C.VOX ** 3 * PX ** 3 * RHO * G


def contact_forces(P: np.ndarray, com2: np.ndarray, W: float) -> np.ndarray:
    """给定着地点与质心投影，解各脚的竖直反力（N）。

    平衡方程只有三条（ΣR = W、绕 x 与 z 的力矩为零），所以三点着地是**静定**的，
    多于三点是超静定，要另加一条准则。约束是硬的：**地面只能推不能拉**，R ≥ 0。

    做法是枚举所有支撑子集，各自求最小范数解，取满足 R≥0 且平衡残差为零的解里范数
    最小的那个（等价于 Lawson–Hanson 主动集，但脚数 ≤6 时直接枚举更短更不会错）。

    不能用"先取全体最小范数解、见负就剔"的贪心：剔掉一只脚会改变支撑多边形，剩下
    三点可能已经包不住质心，于是解出 111% 体重压在一条腿上这种不可能值（实测撞过）。
    质心在全体着地点凸包内时非负解一定存在——存在性由 locomotion 的稳定余量保证。
    """
    n = len(P)
    A = np.vstack([np.ones(n), P[:, 0] - com2[0], P[:, 1] - com2[1]])
    b = np.array([W, 0.0, 0.0])
    best: np.ndarray | None = None
    for mask in range(1, 1 << n):
        idx = [i for i in range(n) if mask >> i & 1]
        if len(idx) < 3:
            continue
        r, *_ = np.linalg.lstsq(A[:, idx], b, rcond=None)
        if r.min() < -1e-6 or np.linalg.norm(A[:, idx] @ r - b) > 1e-6 * W:
            continue
        R = np.zeros(n)
        R[idx] = r
        if best is None or np.linalg.norm(R) < np.linalg.norm(best):
            best = R
    if best is None:
        raise ValueError("质心落在支撑多边形外：任何非负反力组合都撑不住")
    return best


def foot_loads(gait: LM.Gait, *, samples: int = 48) -> dict[str, float]:
    """每条承重肢在整个步态周期里分到的**峰值**竖直载荷（N）。

    这一步是整层的地基：按 W/条数 平摊会得到完全相反的结论。实测远远撑出去的长肢只
    分到两三成，而缩在质心下面的短肢扛到八九成——**扛重的是短腿，长腿在扶着**。
    """
    W = body_weight()
    com2 = np.array([gait.com[0], gait.com[2]])
    peak = {lg.gene.socket: 0.0 for lg in gait.limbs}
    for k in range(samples):
        t = k / samples
        on = [lg for lg in gait.limbs if lg.in_stance(t)]
        if len(on) < 3:
            continue
        P = np.array([lg.foot_at(t) for lg in on])[:, [0, 2]]
        R = contact_forces(P, com2, W)
        for lg, f in zip(on, R):
            s = lg.gene.socket
            peak[s] = max(peak[s], float(f))
    # 支撑相内不是恒力：半正弦的均值等于准静态份额 ⇒ 峰值 = 均值 × π/2
    return {k: v * PEAK_SHAPE for k, v in peak.items()}


# ---------------------------------------------------------------- 姿态
def _chain(segs: tuple[float, ...], bends: np.ndarray) -> np.ndarray:
    """链在自身平面里的各节端点（首节沿 −y，起点在原点）。bends 是各关节的**折角**。

    整条绕根部转是刚体运动、不改变首尾距离——所以"折多少"和"朝哪"可以分开解，
    调用方先用折角凑够首尾距离，再一步把整条转到目标方向上。
    """
    pts = [np.zeros(2)]
    th = 0.0
    for i, L in enumerate(segs):
        if i > 0:
            th += float(bends[i - 1])
        pts.append(pts[-1] + L * np.array([math.sin(th), -math.cos(th)]))
    return np.array(pts)


def _spans(segs: np.ndarray, cw: np.ndarray, phis: np.ndarray) -> np.ndarray:
    """一批折叠量各自对应的首尾距离。整批一次算完——这是姿态搜索里唯一的热点。"""
    th = phis[:, None] * cw[None, :]
    x = (segs * np.sin(th)).sum(axis=1)
    y = (segs * np.cos(th)).sum(axis=1)
    return np.hypot(x, y)


def _fit_span(segs: tuple[float, ...], w: np.ndarray, target: float) -> np.ndarray | None:
    """按权重 w 分配折角，求总折叠量使首尾距离等于 target。返回各关节折角。

    **不能直接二分**：折角同号时首尾距离随折叠量单调收缩，但兽腿是交替折的（+−+），
    Z 形链的首尾距离先缩后涨，二分会收到一个假根上去。所以先粗扫找第一个跨越点，
    再在那一段里二分。找不到跨越点说明这个分配折不到位，返回 None。
    """
    S = np.asarray(segs, float)
    cw = np.concatenate([[0.0], np.cumsum(w)])
    grid = np.linspace(0.0, math.radians(170.0), 96)
    span = _spans(S, cw, grid)
    hit = np.nonzero((span[:-1] >= target) & (span[1:] < target))[0]
    if not len(hit):
        return None
    lo, hi = grid[hit[0]], grid[hit[0] + 1]
    for _ in range(30):
        mid = 0.5 * (lo + hi)
        if float(_spans(S, cw, np.array([mid]))[0]) > target:
            lo = mid
        else:
            hi = mid
    return w * (0.5 * (lo + hi))


def _simplex(n: int, div: int = 8):
    """n 维单纯形上的均匀格点（各分量非负、和为 1）——即把 div 拆成 n 份的所有整数组合。"""
    def rec(k: int, rem: int):
        if k == 1:
            yield (rem,)
            return
        for i in range(rem + 1):
            for rest in rec(k - 1, rem - i):
                yield (i,) + rest

    for comb in rec(n, div):
        yield np.array(comb, float) / div


def stand_pose(hip: np.ndarray, foot: np.ndarray, segs: tuple[float, ...],
               kind: str, clearance: list[float] | None = None,
               descend: bool = True,
               load_pt: np.ndarray | None = None,
               avoid: list | None = None,
               own_r: list[float] | None = None) -> tuple[list[np.ndarray], bool]:
    """站姿：**最省肌肉**的那个折法。世界坐标的各关节点。

    "折到够着落点"有无穷多解——把折叠量摊给哪个关节是自由的。选哪一个不该我挑：
    动物站着不动时采取的姿态是**让抗重力肌出力最小**的那个，因为那份力要一直出下去。
    关节 j 要出的肌力正比于 d_j/a_j（d_j = 该关节到地面接触点的水平距离，a_j = 肌肉
    力臂），所以目标函数就是 Σ d_j/a_j——**把关节尽量挪到载荷线上**。

    这条判据同时管住了造型：先前用一组手挑的折叠权重（蛛足 −1.6/1/0.4 那种），折出来的
    关节忽左忽右，肌肉截面跟着忽粗忽细，渲出来是一串珠子。改成最小化肌力之后关节自己
    贴住载荷线，腿从根到梢顺着收——**上粗下细不是造型偏好，是省力的结果**。

    `clearance` 是逐关节的**最低高度**：省力的解会把某个关节甩到地面以下（膝盖穿地，
    实测蛛足深到 −8.5 px），那是数学上更省但物理上不存在的姿态。第一轮不知道肉多厚，
    先只要求关节本身不低于地面；量出粗细后第二轮再要求"连肉都不许压进地里"。

    留给供体的只剩**折的方向**（`BEND` 的符号）：兽腿膝前肘后所以逐节交替，禽腿的
    "反折膝"其实是踝，蛛足先把腿节抬到身体之上再落下去。方向是解剖事实，幅度是算的。
    """
    d = foot - hip
    target = float(np.linalg.norm(d))
    e_d = d / max(target, 1e-9)
    # 力臂量到**地面反力的作用点**，不是量到链的末端。这两个点不是一回事：链解到踝，
    # 而踝在落点**后面**（趾行退 0.62 个掌骨长）。拿踝当受力点，代价函数就会一路把膝
    # 往后推——实测所有兽腿都被判成"往后折更省力"并触发改折向，膝全朝后。
    load = np.asarray(load_pt if load_pt is not None else foot, float)
    # 折弯**平面**由关节的铰链轴定，不是由脚落在哪儿定。膝是横轴铰链 ⇒ 腿只能在前后
    # 方向上折；脚要落到侧面去，靠的是髋（球窝关节）把整条腿**外展**出去，而膝依旧
    # 朝前。上一版把平面取成"髋与脚所构成的竖直面"，脚一落在侧面，膝就跟着朝侧面折
    # ——那是外展类（蛛足）的折法，兽腿禽腿这么折等于把膝当成了髋。用户看单件图一眼
    # 认出来："大腿怎么在小腿上方的侧面？"
    #
    # 所以平面 = 张成(髋→脚, 前后方向)：既穿过落点（不然够不着），又让膝朝前。
    ref = UP if kind == "spider" else FWD
    e_b = ref - float(ref @ e_d) * e_d
    nb = float(np.linalg.norm(e_b))
    if nb < 1e-6:                     # 参考方向与腿共线：退回竖直面
        alt = np.array([1.0, 0.0, 0.0]) if kind == "spider" else UP
        e_b = alt - float(alt @ e_d) * e_d
        nb = float(np.linalg.norm(e_b))
    e_b = e_b / max(nb, 1e-9)
    nj = max(1, len(segs) - 1)
    signs = np.sign(np.array(BEND.get(kind, (1.0,) * nj)[:nj], float))
    arm = np.array([ARM_RATIO * max(segs[j - 1], segs[j]) for j in range(1, len(segs))])
    floor = np.array(clearance if clearance is not None else [0.0] * (len(segs) + 1))

    def place(bends: np.ndarray, rot: float) -> list[np.ndarray]:
        """平面内的链 → 世界坐标。平面基底是 (弯向, 髋→脚)，不再是 (水平, 竖直)。"""
        P = _chain(segs, bends)
        c, s = math.cos(rot), math.sin(rot)
        return [hip - e_b * (c * p[0] - s * p[1]) - e_d * (s * p[0] + c * p[1])
                for p in P]

    def aim(bends: np.ndarray) -> float:
        """把整条链转到末端正对落点。整条绕根部转是刚体运动，不改变首尾距离。"""
        P = _chain(segs, bends)
        return -math.atan2(P[-1][0], -P[-1][1])

    def penetration(pt: np.ndarray) -> float:
        """这一点扎进核心多深（px）。场在体内光滑，用 (f−ISO)/|∇f| 换算成距离。"""
        q = pt - C.CORE_CENTER
        f = C.fld(q)
        if f < C.ISO:
            return 0.0
        g = float(np.linalg.norm(C.grad(q)))
        return (f - C.ISO) / max(g, 1e-6)

    def search(sg: np.ndarray):
        """在这一组折向下取**总代价最小**的姿态。

        代价 = 肌肉需求 + 穿地深度 + 扎进自己身体的深度，后两项按 PENALTY 计价。
        先前是"硬拒不合格的候选、一个都没有就退回兜底解"，那条级联很脆：净空要求稍微
        提一点就全军覆没，退回来的偏偏是膝盖插进地里那个（实测 −7.2 px），而且换个
        seed 又变成膝盖埋进肚子（f=1.32）。改成惩罚项之后永远有最优解，而且"宁可多长
        点肌肉也不要穿模"这个取舍是显式的。
        """
        best, best_cost = None, 1e18
        for w in _simplex(nj):
            bends = _fit_span(segs, w * sg, target)
            if bends is None:
                continue                 # 这个分配折不到位（全压在一个关节上时会这样）
            rot = aim(bends)
            pts = place(bends, rot)
            y = np.array([float(p[1]) for p in pts])
            # 力臂是**世界系的水平**距离，不是平面内的：外展之后腿的平面本来就不竖直，
            # 拿平面内坐标当水平距离会把外展那一份算漏。
            armh = np.array([float(np.hypot(*(p - load)[[0, 2]])) for p in pts])
            cost = float((armh[1:-1] / arm).sum())
            # 站着的兽腿，关节该**从髋一路往下降**。折叠量一大，"把膝盖抬到髋以上"
            # 也能凑够首尾距离而且更省肌肉，于是解出来股骨朝天支出去——腿谱上四条蹄行
            # 腿全长成了钩子。蛛足不受此限：它本来就是先抬后落，那正是它的读法。
            #
            # 和穿模一样记成**惩罚**不是硬拒：关节只有一个时（兽腿/禽腿的膝）候选就
            # 那么一两个，硬拒会把它们全毙掉、退回一条够不着踝的直链（实测末端偏离
            # 8.15 px）。宁可要一条膝盖略微抬起来的腿，也不要一条伸不到脚的腿。
            if descend:
                cost += PENALTY * float(np.clip(np.diff(y) + 0.4, 0, None).sum())
            cost += PENALTY * float(np.clip(floor[:len(y) - 1] - y[:-1], 0, None).sum())
            for pt in pts[1:]:
                cost += CORE_PENALTY * penetration(pt)
            # **别折进邻居身上。** 和"别折进核心"是同一条约束的另一半：核心是它自己的
            # 身体，邻肢是别人的身体，两者都不该被这条腿占掉。少了这一项，实测 8/12 只
            # 兽有腿穿腿，最深 4.24 px——省力的解会毫不犹豫地把小腿折进旁边那条腿里，
            # 因为在它的目标函数里那块空间是免费的。
            if avoid:
                for i, (aa, bb) in enumerate(zip(pts, pts[1:])):
                    rr = own_r[i] if own_r and i < len(own_r) else 0.0
                    for (q0, q1, rq) in avoid:
                        dd, _mid = VR.seg_dist(aa, bb, q0, q1)
                        ov = rq + rr - dd
                        if ov > 0.0:
                            cost += PENALTY * ov
            if cost < best_cost:
                best, best_cost = (bends, rot), cost
        return best, best_cost

    best, best_cost = search(signs)
    forced = False
    # 供体自己的折法可能在这具身体上怎么摆都要穿模。那就**按不属于它的方式折**——
    # 捡来的部件装在不匹配的身体上本来就会这样。逐个试其它折向，取总代价最小的。
    for bits in range(1, 1 << nj):
        alt = signs * np.array([-1.0 if bits >> i & 1 else 1.0 for i in range(nj)])
        cand, cost = search(alt)
        if cand is not None and (best is None or cost < best_cost * 0.9):
            best, best_cost, forced = cand, cost, True
    if best is None:
        best = (np.zeros(nj), 0.0)
        forced = True

    return place(best[0], best[1]), forced


def bend_plane(hip, target, kind: str,
               normal: np.ndarray | None = None) -> tuple[np.ndarray, np.ndarray, float]:
    """折弯平面的基底 (e_b, e_d) 与首尾距离——和 `stand_pose` 用的是同一套构造。

    抽出来是给动画层用的：走一步只是把同一条腿折到另一个距离上，平面的定法必须和解
    站姿时一模一样，否则动画的第一帧就和静止模型对不上。

    `normal` 是**已知的平面法向**（站姿解出来的那个）。给了它就直接用，因为默认那套
    构造在肢体接近竖直时是病态的：蛛足的参考方向是 UP，而它的腿本来就几乎竖直，
    `UP − (UP·e_d)e_d` 的模只有 0.004，方向由舍入误差决定——逐帧解就会翻面。实测
    limb_ml 的膝在相邻两帧之间从 z=−0.62 跳到 +2.69，一帧 11.71 px，而它一步才走 9.18。
    法向本身是良态的（它按定义垂直于腿），所以拿它反推平面既连续又和站姿一致。
    """
    d = np.asarray(target, float) - np.asarray(hip, float)
    L = float(np.linalg.norm(d))
    e_d = d / max(L, 1e-9)
    if normal is not None:
        e_b = np.cross(e_d, np.asarray(normal, float))
        nb = float(np.linalg.norm(e_b))
        if nb > 1e-6:
            return e_b / nb, e_d, L
    ref = UP if kind == "spider" else FWD
    e_b = ref - float(ref @ e_d) * e_d
    nb = float(np.linalg.norm(e_b))
    if nb < 1e-6:
        alt = np.array([1.0, 0.0, 0.0]) if kind == "spider" else UP
        e_b = alt - float(alt @ e_d) * e_d
        nb = float(np.linalg.norm(e_b))
    return e_b / max(nb, 1e-9), e_d, L


def synergy(joints: list[np.ndarray], kind: str) -> tuple[np.ndarray, float]:
    """从已解出的站姿里**读回**这条腿的折叠协同：(各关节折角的分配 w, 总折叠量 φ)。

    动画层需要它是因为**走一步不该重解一次姿态**。`stand_pose` 是在一族折法里挑总代价
    最小的那个；逐帧重跑，代价面一变，解就可能跳到另一个局部最优上去——渲出来是膝盖
    凭空翻一下。动物也不是这么走路的：站姿定下来的是"折叠量怎么在关节间分配"这个协同，
    迈一步只是沿着同一个协同伸缩。

    所以这里只把站姿解里已经含着的分配比例量出来，`refold` 再沿着它伸缩。折角在平面内
    量：站姿本来就是在 `bend_plane` 那个平面里解的，投影是精确的，不是近似。
    """
    e_b, e_d, _L = bend_plane(joints[0], joints[-1], kind)
    hip = np.asarray(joints[0], float)
    # 平面坐标（与 stand_pose.place 互逆；整条链的刚体转角不影响折角，不必解出来）
    pts = [np.array([-float((np.asarray(p, float) - hip) @ e_b),
                     -float((np.asarray(p, float) - hip) @ e_d)]) for p in joints]
    segs = [b - a for a, b in zip(pts, pts[1:])]
    bends = []
    for a, b in zip(segs, segs[1:]):
        cross = float(a[0] * b[1] - a[1] * b[0])
        bends.append(math.atan2(cross, float(a @ b)))
    bends = np.array(bends, float)
    total = float(np.abs(bends).sum())
    if total < 1e-6:                     # 站姿解出来是一根直棍：没有协同可读，退回折向
        nj = max(1, len(joints) - 2)
        sg = np.sign(np.array(BEND.get(kind, (1.0,) * nj)[:nj], float))
        return sg / max(np.abs(sg).sum(), 1e-9), 0.0
    return bends / total, total


def fold_phi(segs: tuple[float, ...], w: np.ndarray, target: float,
             hint: float) -> float:
    """沿协同 w 折到首尾距离 `target` 所需的总折叠量，取**离 hint 最近**的那个解支。

    不能像 `_fit_span` 那样取第一个跨越点。Z 形链（兽腿 +−+）的首尾距离随折叠量先缩
    后涨，同一个 target 有好几个解，而"第一个"在相邻两帧之间是会跳的——腿走着走着突然
    换一种折法。动画要的是**连续**，所以拿上一帧的解当锚，每帧只在自己那一支上滑动。
    """
    S = np.asarray(segs, float)
    cw = np.concatenate([[0.0], np.cumsum(w)])
    grid = np.linspace(0.0, math.radians(178.0), 256)
    span = _spans(S, cw, grid)
    diff = span - target
    idx = np.nonzero(diff[:-1] * diff[1:] <= 0.0)[0]
    if not len(idx):                     # 够不着/收不拢：取最接近的那个折叠量
        return float(grid[int(np.argmin(np.abs(diff)))])
    best, bestd = 0.0, 1e18
    for i in idx:
        lo, hi = float(grid[i]), float(grid[i + 1])
        for _ in range(34):
            mid = 0.5 * (lo + hi)
            if float(_spans(S, cw, np.array([mid]))[0]) > target:
                lo = mid
            else:
                hi = mid
        phi = 0.5 * (lo + hi)
        if abs(phi - hint) < bestd:
            best, bestd = phi, abs(phi - hint)
    return best


def refold(hip, target, segs: tuple[float, ...], kind: str, w: np.ndarray,
           hint: float, normal: np.ndarray | None = None
           ) -> tuple[list[np.ndarray], float]:
    """把链按协同 w 折到 hip→target，返回 (各关节世界坐标, 本次的总折叠量)。

    返回折叠量是给下一帧当 hint 用的——连续性靠它维持，见 `fold_phi`。
    `normal` 是站姿那个折弯平面的法向，逐帧解务必传，理由见 `bend_plane`。
    """
    e_b, e_d, L = bend_plane(hip, target, kind, normal)
    phi = fold_phi(segs, w, L, hint)
    bends = w * phi
    P = _chain(segs, bends)
    rot = -math.atan2(P[-1][0], -P[-1][1])          # 整条转到末端正对目标
    c, s = math.cos(rot), math.sin(rot)
    hip = np.asarray(hip, float)
    pts = [hip - e_b * (c * p[0] - s * p[1]) - e_d * (s * p[0] + c * p[1]) for p in P]
    return pts, phi


def foot_chain(stance: str, contact: np.ndarray, lens: tuple[float, ...],
               out: np.ndarray, deg: float | None = None) -> list[np.ndarray]:
    """脚：从**踝**到趾尖的各关节点。腿的 IK 只解到踝，脚由站姿摆。

    上一版把脚当成链的最后两节一起 IK，于是"脚"只是两根越来越细的杆——狼爪、羊蹄、
    人足全是同一个东西。真实的差别在**哪几根骨着地**：

      · 蹄行 —— 蹄尖着地，掌骨近乎竖直（80°）。小腿以下是一根细长的管骨加一枚蹄。
      · 趾行 —— 趾节平铺地面，掌骨抬起 52°。所以狼的"后弯的膝"其实是它的踝。
      · 跖行 —— 整个掌骨平贴地面，踝低、脚后跟向后支出来一块。人和鼠是这一类。
      · 外展 —— 跗节斜插地面，只有尖端一点接触（蛛足）。

    接触点 `contact` 是 locomotion 解出的落点，其含义随站姿变：蹄行是蹄尖，趾行是
    掌趾关节（脚掌前掌那一点），跖行是整个脚底的形心，外展是钩尖。地面反力永远作用
    在这一点上——所以力矩臂量到它，不是量到趾尖。
    """
    th = math.radians(META_DEG[stance] if deg is None else deg)
    back = -FWD
    if stance == "sprawling":
        (tar,) = lens
        return [contact + UP * (tar * math.sin(th)) - out * (tar * math.cos(th)), contact]
    meta, toes = lens
    if stance == "unguligrade":
        pastern = contact + UP * toes                       # 蹄从接触点竖起来
        return [pastern + UP * (meta * math.sin(th)) + back * (meta * math.cos(th)),
                pastern, contact]
    if stance == "digitigrade":
        ball = contact                                       # 掌趾关节就压在地上
        return [ball + UP * (meta * math.sin(th)) + back * (meta * math.cos(th)),
                ball, ball + FWD * toes]
    heel = contact + back * (meta * 0.5)                     # 跖行：脚底整片贴地
    ball = contact + FWD * (meta * 0.5)
    ankle = heel + UP * (meta * math.sin(th) + 0.6) + FWD * (meta * 0.25)
    return [ankle, ball, ball + FWD * toes]


def foot_heel(stance: str, contact: np.ndarray,
              lens: tuple[float, ...]) -> np.ndarray | None:
    """跖行足向后支出去的**跟**在哪儿。

    跟不在 `foot_chain` 的链上，因为它不是一节——跟骨和掌骨是**同一块刚体**，只是
    往踝的后下方支出一块。链上没有它，渲染就得另外拿；上一版没拿，于是整只脚画在了
    踝的前面：踝的地面投影在 z=+1.75，而画出来的脚底从 −2.95 起步，连地面反力作用点
    （z=0）都没盖住。看上去就是"用脚前掌的位置支着一根杆"。
    """
    if stance != "plantigrade":
        return None
    meta = lens[0]
    return contact - FWD * (meta * 0.5)


def hang_pose(sock: C.Socket, segs: tuple[float, ...], radii: list[float],
              *, jointed: bool, avoid: list | None = None) -> list[np.ndarray]:
    """不承重的肢：从挂载点沿法向长出来，然后被自己的重量拽下去。

    **根部那一节的朝向不是自由的**——芽就是沿挂载点法向长出去的，那道癒合把它焊死在
    这个角度上。往下才轮到重力说话，而重力怎么说取决于这条肢里有没有关节：

    · **有关节的**（捡来的腿萎缩了、或够不着地被踢出承重集）：关节没有肌肉张力就是个
      自由铰，自由铰链在重力下的平衡姿态是**每一节都指着正下方**。所以它像提线木偶
      一样，在第一个关节处硬折一下，然后笔直垂下去。这不是近似，是零刚度铰链的精确解。
    · **无骨的**（触手）：它是一根连续的软梁，没有铰，弯要靠自身抗弯刚度扛。等截面
      悬臂自重端部转角 θ = wL³/(6EI)，代入 w = ρgπr²、I = πr⁴/4 得
      **θ = 2ρgL³/(3Er²)** —— 只跟长度与粗细有关。长而细的算出来远超 90°（小挠度理论
      在这里已经失效，钳到竖直：它就是整条垂下去了），短而粗的只有十几度、几乎还翘着。

    于是"死腿硬折、触手软垂"这两种读法不用分别造型，是两种组织结构各自的平衡解。
    """
    d = np.array([0.0, -1.0, 0.0])
    out = [np.asarray(sock.pos, float)]
    dirv = np.asarray(sock.normal, float).copy()
    for i, (L, r) in enumerate(zip(segs, radii)):
        if i > 0:
            if jointed:
                dirv = d.copy()
            else:
                Lm, rm = L * PX, max(r * PX, 1e-4)
                th = 2.0 * RHO * G * Lm ** 3 / (3.0 * E_SOFT * rm * rm)
                cur = math.acos(float(np.clip(np.dot(dirv, d), -1.0, 1.0)))
                th = min(th, cur)          # 转不过"已经指着正下方"
                axis = np.cross(dirv, d)
                na = float(np.linalg.norm(axis))
                if na > 1e-9 and th > 1e-6:
                    dirv = C.axis_angle(axis / na, th) @ dirv
                    dirv /= max(float(np.linalg.norm(dirv)), 1e-9)
        # 垂下去会撞上自己的身体：背上、肩上那些槽的法向是朝上的，一节沿法向长出去、
        # 后面直接垂下来，就会从核心里穿过去（实测 vest_dl 的第 2 个关节 f=1.02，埋在
        # 肉里）。真实的答案不是穿过去也不是停住，是**贴着身体滑下去**——所以撞到就把
        # 朝向往外法向掰一点重试，节长不变。
        nxt = out[-1] + dirv * L
        for _ in range(12):
            if C.fld(nxt - C.CORE_CENTER) < C.ISO:
                break
            g = -C.grad(nxt - C.CORE_CENTER)
            g = g / max(float(np.linalg.norm(g)), 1e-9)
            dirv = dirv + 0.6 * g
            dirv /= max(float(np.linalg.norm(dirv)), 1e-9)
            nxt = out[-1] + dirv * L

        # 撞上**邻居**也一样：贴着滑开。垂肢完全在步态之外（它没有落点、不参与承重），
        # 所以承重肢那套避让和掰落点一个都作用不到它身上——实测有一只兽的垂肢直接从
        # 旁边那条承重腿的中段里穿过去 5.82 px，是全部十二只里最深的一处。死腿挂在
        # 别的东西上会滑开，这和上面"贴着身体滑下去"是同一件事。
        if avoid:
            for _ in range(12):
                worst, at = 0.0, None
                for (q0, q1, rq) in avoid:
                    dd, mid = VR.seg_dist(out[-1], nxt, q0, q1)
                    ov = rq + r - dd
                    if ov > worst:
                        worst, at = ov, mid
                if worst <= 0.0 or at is None:
                    break
                away = nxt - at
                na = float(np.linalg.norm(away))
                if na < 1e-9:
                    break
                dirv = dirv + 0.5 * away / na
                dirv /= max(float(np.linalg.norm(dirv)), 1e-9)
                nxt = out[-1] + dirv * L
                if C.fld(nxt - C.CORE_CENTER) >= C.ISO:   # 别滑进自己身体里
                    break

        # 垂到地面就**摊在地上**：地面撑住了它，这一节之后只能沿地面铺开（长触手拖在
        # 地上是这么来的，不是造型）。铺的方向取当前朝向的水平分量；正垂直下来时没有
        # 水平分量，用挂载点法向的水平分量代替——那是它长出来的方向。
        if nxt[1] < r:
            flat = np.array([dirv[0], 0.0, dirv[2]])
            if float(np.linalg.norm(flat)) < 1e-6:
                flat = np.array([sock.normal[0], 0.0, sock.normal[2]])
            if float(np.linalg.norm(flat)) < 1e-6:
                flat = np.array([1.0, 0.0, 0.0])
            dirv = flat / float(np.linalg.norm(flat))
            nxt = np.array([out[-1][0], r, out[-1][2]]) + dirv * L
        out.append(nxt)
    return out


# ---------------------------------------------------------------- 截面
def bone_radius(L_px: float, arm_px: float, F: float) -> tuple[float, tuple[float, ...]]:
    """一节骨该多粗（px），取三条失效判据的上界。

    · 轴压屈服 r = √(F/πσ)
    · 欧拉屈曲 r = (4k²L²F/π³E)^(1/4)（两端铰接 k=1）
    · **弯曲** r = (4Fa/πσ)^(1/3)，a = 该关节到地面接触点的水平距离

    实测第三条永远主导，另两条小一个数量级——四足动物的腿是被掰断的，不是被压扁的。
    """
    if F <= 0.0:
        return 0.0, (0.0, 0.0, 0.0)
    Fs = F * SAFETY
    Lm, am = L_px * PX, arm_px * PX
    ax = math.sqrt(Fs / (math.pi * SIGMA_BONE))
    bk = (4.0 * Lm * Lm * Fs / (math.pi ** 3 * E_BONE)) ** 0.25
    bd = (4.0 * Fs * am / (math.pi * SIGMA_BONE)) ** (1.0 / 3.0)
    return max(ax, bk, bd) / PX, (ax / PX, bk / PX, bd / PX)


def budget_radii(segs: tuple[float, ...], budget: float, girth: float) -> list[float]:
    """不承重的肢的粗细：**有多少料就多粗**，形状由自重弯矩收细。

    强度在这里根本不是约束——闲肢只要扛住自己，按强度反推出来的半径不到十分之一像素
    （初版实测全部算成 0.00）。决定一条闲肢多粗的是**这具身体愿意在它身上放多少肉**，
    而那个量已经在核心层推过了：`core.graft_budget` = 同时能持有的未分化组织，等于
    一条部件的用料（正典"一次一条、每条七日"反推的）。芽用的是同一份预算，闲肢就是
    芽长歪了的结果，用同一份是自洽的。

    于是**长度反过来决定粗细**：同样一份料，36 px 的触手摊成 2.0 px 半径的一条绳，
    6.9 px 的退化残肢缩成 4.6 px 半径的一个肉疙瘩。"触手细、残肢粗"不用分别造型。

    形状仍是推的：第 i 节根部承的弯矩 = 它以远所有肉的重量 × 力臂（按水平甩出去的最坏
    姿态算，乱抽正是这么甩的），抗弯所需半径 ∝ M^{1/3}；均匀杆 M ∝ 剩余长度²，于是
    r ∝ 剩余长度^{2/3}，梢部收成尖而不是一根等粗的棍。比例与绝对粗细无关（M ∝ r²
    同倍缩放），所以先迭代出形状再一次性缩放到预算体积。

    根部封顶在 girth：挂载面接不下更粗的东西（`core._girth` 量的就是这个），多出来的
    料就留在体内没长出去。
    """
    n = len(segs)
    shape = [1.0] * n
    for _ in range(6):
        M = []
        for i in range(n):
            m, arm = 0.0, 0.0
            for k in range(i, n):
                Lk = segs[k]
                m += shape[k] ** 2 * Lk * (arm + 0.5 * Lk)
                arm += Lk
            M.append(m)
        shape = [(M[i] / M[0]) ** (1.0 / 3.0) if M[0] > 0 else 1.0 for i in range(n)]
    vol = sum(math.pi * shape[i] ** 2 * segs[i] for i in range(n))
    k = math.sqrt(budget / vol) if vol > 0 else 1.0
    k = min(k, girth / max(shape[0], 1e-9))
    return [s * k for s in shape]


# ---------------------------------------------------------------- 一条肢
@dataclass
class Limb:
    gene: GN.LimbGene
    sock: C.Socket
    load: float                     # 峰值竖直载荷 N（0 = 不承重）
    joints: list[np.ndarray]        # 各关节世界坐标，[0]=髋，[-1]=尖/脚
    radius: list[float]             # 每节的可见半径 px
    bone_r: list[float]             # 每节的骨半径 px
    muscle: list[float]             # 每节的肌肉截面 m²
    arms: list[float]               # 每个关节到地面接触点的水平距离 px（= 地面反力的力臂）
    ankle: np.ndarray               # 站姿要求的踝位（IK 的目标）；不承重时为零向量
    mats: list[str]                 # 每节的材质
    forced: bool                    # 供体自己的折法在这具身体上站不住，被迫改折向
    weld_r: float                   # 接合痕半径 px（剪切传力所需，从来不是瓶颈）
    root_need: float                # 根部**需要**的半径（未被 girth 钳过），自检用
    pad: tuple[float, float]        # 脚掌 (半宽 px, 半长 px)；不承重为 (0,0)
    node_r: list[float]             # 每个**关节处**的包络半径 px（len = len(joints)）
    heel: np.ndarray | None = None  # 跖行足的跟（不在链上，见 foot_heel）；其余为 None

    @property
    def name(self) -> str:
        return self.sock.name

    def profile(self, i: int, t: float) -> float:
        """第 i 节上、沿节长走到 t∈[0,1] 处的**外表面**半径。

        肉是**一整条连续的包络**，不是一节一个粗细。上一版每节渲成一根等粗（或三段
        纺锤）的柱子，于是在关节处直接跳变——大腿 4.8 px 接着小腿 0.5 px，中间没有
        过渡。那读出来就是"一块肉后面插了根棍"，不是腿。

        真实的外形由两样东西叠出来，这里就取两者的上包络：

        · **关节处的包络**（`node_r`）：肌腹**不跨关节**，所以关节那一圈只有骨 + 腱 +
          皮下 + 皮，是整条肢最细的地方。两侧必须取同一个值——肉是连续的，不能从
          关节两边看过去粗细不一样。
        · **肌腹的鼓包**：峰值就是解出来的 `radius[i]`，位置偏**近端**（约 38%），
          两端收进腱里归零。所以是小腿肚在上、跟腱在下，不是一根均匀的棒。
        """
        a, b = self.node_r[i], self.node_r[i + 1]
        t = min(max(t, 0.0), 1.0)
        line = a + (b - a) * t                      # 关节到关节的锥形过渡
        belly = self.radius[i] * math.sin(math.pi * t ** 0.72)   # 峰值偏近端
        return max(line, belly)

    @property
    def bearing(self) -> bool:
        return self.load > 0.0

    @property
    def length(self) -> float:
        return sum(float(np.linalg.norm(b - a))
                   for a, b in zip(self.joints, self.joints[1:]))

    @property
    def root_r(self) -> float:
        return self.radius[0]

    @property
    def tip(self) -> np.ndarray:
        return self.joints[-1]

    @property
    def buried(self) -> float:
        """根部肌肉里**装不下、只能长在体内**的那一份（占需求的比例）。

        蛛足这类基节极短的肢，这个数会很高——它的驱动肌本来就在体腔里，不在腿上。
        """
        return max(0.0, 1.0 - self.radius[0] ** 2 / max(self.root_need ** 2, 1e-9))

    @property
    def arm0(self) -> float:
        """髋到地面接触点的水平距离——这条肢在整只兽的支撑几何里站得多开。"""
        return self.arms[0] if self.arms else 0.0

    @property
    def arm1(self) -> float:
        """根部那一节要抗的力矩臂——决定根部粗细的就是它乘载荷。"""
        return self.arms[1] if len(self.arms) > 1 else 0.0


def solve_limb(gene: GN.LimbGene, sock: C.Socket, *, load: float = 0.0,
               foot: np.ndarray | None = None, ride: float = 0.0,
               territory: float = 1e9,
               avoid: list | None = None) -> Limb:
    """一条肢的完整几何。承重的按落点解站姿，其余的垂下来。

    `territory` 是到**最近的另一个已占用挂载点**的距离：两条肢的肌腹不能占同一块空间，
    所以根部还要再封一道顶（半个间距）。不封的话相邻两条腿的大腿会互相穿进去——用户
    在 round 2 的图上看到的穿模就是这个。多出来的肌肉和基节太短时一样，只能长在体内。
    """
    segs = gene.segments
    n = len(segs)
    boneless = gene.kind == "tentacle"

    if load > 0.0 and foot is not None:
        hip = sock.pos.copy()
        hip[1] += ride
        ground = np.asarray(foot, float)
        # **腿只解到踝，脚由站姿摆**（见 foot_chain）。上一版整条一起 IK，于是"脚"只是
        # 两根越来越细的杆，狼爪羊蹄人足全一个样。现在最后一两根骨归站姿管，IK 的目标
        # 变成踝——蹄行的踝因此抬得比趾行高一大截，剪影自己就分开了。
        nfoot = 1 if gene.stance == "sprawling" else 2
        nleg = n - nfoot
        horiz = np.array([ground[0] - hip[0], 0.0, ground[2] - hip[2]])
        hn = float(np.linalg.norm(horiz))
        out = horiz / hn if hn > 1e-6 else np.array([1.0, 0.0, 0.0])
        # 掌骨立得多直是**可达性反过来定的**：站姿给一个标称倾角，但脚把踝往身后推
        # （趾行推 0.62 个掌骨长），推远了腿就够不着——够不着时 IK 无解、退回一条直链，
        # 于是关节落到地面以下（实测 y=−4.13）。腿短的动物真实的应对就是把脚立起来：
        # 掌骨越竖，踝越近也越高，两头都省。所以从标称角往上加，取第一个够得着的。
        span = sum(segs[:nleg])
        base = META_DEG[gene.stance]
        paw = foot_chain(gene.stance, ground, segs[nleg:], out)
        for extra in range(0, 40, 4):
            paw = foot_chain(gene.stance, ground, segs[nleg:], out,
                             min(base + extra, 88.0))
            if float(np.linalg.norm(paw[0] - hip)) <= 0.97 * span:
                break
        ankle = paw[0]
        heel = foot_heel(gene.stance, ground, segs[nleg:])

        # 脚掌：面积 = 载荷 / 地面承载力。踩不住就陷进灰里。**这一步得在算粗细之前**
        # ——脚掌的长度就是脚里那几根骨头的等效力臂（见下面 eff）。
        side = math.sqrt(load / BEARING) / PX
        pad = (side * 0.5, side * 0.62)

        # 两轮：第一轮只禁止关节本身穿地，量出粗细后第二轮连肉一起禁（见 stand_pose）
        clear: list[float] | None = None
        mine: list[float] | None = None
        forced = False
        for _ in range(3):
            leg, forced = stand_pose(hip, paw[0], segs[:nleg], gene.kind, clear,
                                     descend=gene.kind != "spider", load_pt=ground,
                                     avoid=avoid, own_r=mine)
            joints = leg + paw[1:]
            # 力臂 = 该关节到**地面接触点**的水平距离。接触点是 `ground` 不是趾尖——
            # 跖行/趾行的趾节还往前伸一截，地面反力不在那儿。
            arms = [float(np.hypot(*(ground - p)[[0, 2]])) for p in joints]
            # 但**脚里面**的关节不能这么算：把地面反力缩成一个点，对脚掌以内的骨头是
            # 假杠杆。脚掌是一片有长度的接触面（`pad`），一根趾骨只撬得动它**远端那
            # 一小片**的力，撬不动整只脚的。照点接触算的后果实测得很清楚：人足的趾骨
            # 比掌骨还粗（0.51 vs 0.41 px），末节反而成了最粗的一节——自检直接红。
            #
            # 正确的做法是按分布力积分：设接触面沿前进方向半长 hl、压强均匀，位于中心
            # 前方 d 处的关节，其远端那一片占总载荷 (hl−d)/2hl，形心在 (hl−d)/2 处，
            # 于是**等效力臂** = 份额 × 形心距。踝以上的关节不受此限——整片脚掌都在它
            # 的远端，力矩仍旧是全载荷乘到形心的距离。
            eff = list(arms)
            hl = max(pad[1], 1e-6)
            for j in range(nleg + 1, len(joints)):
                d = float(np.dot(joints[j] - ground, FWD))     # 沿脚尖方向的前伸量
                if d >= hl:
                    eff[j] = 0.0                               # 已经伸出接触面之外
                else:
                    eff[j] = (hl - d) / (2.0 * hl) * (hl - d) / 2.0
            bone_r = [bone_radius(segs[i], eff[i], load)[0] for i in range(n)]

            # 肌肉：关节 j 的肌腹长在第 j−1 节上，更远的关节只有腱穿过（腱强两百倍
            # ⇒ 远端细）。**根部那一节还要额外背上嫁接关节（j=0）自己的肌肉**：脊椎
            # 动物把髋伸肌的肌腹放在骨盆上，而这东西的核心是个没有骨架的肉囊，锚不住
            # 任何东西——驱动那道接合的肌肉只能长在肢这一侧。腿根鼓成一坨是这么来的。
            #
            # 这里**不乘安全系数**：安全系数是防结构断裂的，而肌肉不会"断"——它只是
            # 出不出得了那么大力。要多少力就得长多少肌肉，没有余量一说。
            muscle, sinew = [], []
            for i in range(n):
                A = B = 0.0
                for j in ([0] if i == 0 else []) + list(range(i + 1, n)):
                    tau = load * eff[j] * PX
                    a = ARM_RATIO * max(segs[max(j - 1, 0)], segs[min(j, n - 1)]) * PX
                    # **脚上没有肌腹**——掌骨与趾骨那几节只有腱和角质穿过（马的管骨、
                    # 羊的蹄都是这样）。不这么分的话，趾尖比接触点还靠前会让力臂反过来
                    # 变大，肌肉截面从根往梢反而涨（实测 0.096 → 0.161）。
                    if j <= i + 1 and i < nleg:
                        A += tau / (a * SIGMA_MUSCLE)      # 长在这一节上的肌腹
                    else:
                        B += tau / (a * SIGMA_TENDON)      # 只是穿过去的腱
                muscle.append(A)
                sinew.append(B)
            radius = [math.sqrt((bone_r[i] * PX) ** 2
                                + (muscle[i] + sinew[i]) / math.pi) / PX
                      for i in range(n)]
            # 净空要**几何上可能**：越靠近脚的关节，离地高度天然受限于它到脚尖还剩多长
            # 链。照搬半径会要求一个 2.2 px 长的末节把踝抬到 3.0 px 高——无解，于是所有
            # 候选被否、退回那个穿地的兜底解（实测蛛足关节深到 −7.3 px）。
            rest = np.cumsum(list(segs)[::-1])[::-1]
            clear = [min(max(radius[max(i - 1, 0)], radius[min(i, n - 1)]), float(rest[i]))
                     for i in range(nleg)] + [0.0]   # 末项是踝，由站姿定高不受此约束
            # 下一轮解姿态时"我自己有多粗"也知道了，避让邻居才能按肉算而不是按中轴算
            mine = [radius[min(i, n - 1)] for i in range(nleg)]
    else:
        arms = [0.0] * (n + 1)
        sinew = [0.0] * n
        ankle = np.zeros(3)
        forced = False
        radius = budget_radii(segs, C.graft_budget(), sock.girth)
        # 无骨的（触手）里没有硬芯；有骨的闲肢里那根骨还在，只是外面的肌肉萎缩掉了
        bone_r = [0.0] * n if boneless else [r * 0.35 for r in radius]
        muscle = [0.0] * n
        joints = hang_pose(sock, segs, radius, jointed=not boneless,
                           avoid=avoid)
        pad = (0.0, 0.0)
        heel = None

    # 材质分界是**算出来的**：一节的横截面里，本体长上去的新肉（肌腹）与供体原有的
    # 组织（骨 + 只是穿过去的腱）谁占多数，外面就露谁。近端全是新长的肌腹 ⇒ 一片本体
    # 的肉色；越往远端肌腹越少、只剩腱和骨 ⇒ 露出供体自己的皮/甲/鳞。
    #
    # 于是**脚永远是别人的**——那一节按定义没有肌腹。玩家看得出这条腿是从哪种野兽
    # 身上拆的，靠的就是脚那一截，而不是靠整条腿刷成一个颜色。
    donor = DONOR_MAT.get(gene.kind, "hide")
    mats = []
    for i in range(n):
        own = muscle[i]
        theirs = sinew[i] + math.pi * (bone_r[i] * PX) ** 2
        mats.append("graft" if own > theirs else donor)

    # 肌腹粗不过它所在那一节长。**先前写成"超过挂载面 girth 就长一圈癒合环把载荷摊
    # 开"是错的**：肌肉根本不从接合面里穿过去，它堆在体外，接合面只负责传力（传力需要
    # 的面积由剪切给出，见 weld_r，只有一两像素）。按那个错模型渲出来是几块半径 9 px
    # 的板贴在身上，侧视图里整个剪影都被吃掉了。
    #
    # 真正的界限是解剖比例：一段肌腹的粗细不会超过它所附着那一节长度的一半，再粗就
    # 不是肌肉是个球了。**装不下的那部分只能长在体内**——蜘蛛驱动基节的肌肉本来就在
    # 体腔里，不在腿上。于是短基节的蛛足根部反而是细的，鼓起来的是它旁边那团核心；
    # 长股骨的兽腿则真的有一条看得见的粗大腿。这条同时解释了两种腿看起来为什么不一样。
    need = radius[0]
    cap = [0.5 * segs[i] for i in range(n)]
    cap[0] = min(cap[0], 0.5 * territory)
    radius = [min(r, cap[i]) for i, r in enumerate(radius)]

    # 骨外面那层不扛载荷的软组织（见 SOFT_OVER_BONE）。**钳完再抬**：上面那个 cap
    # 管的是肌腹能堆多大，管不着腱鞘和皮下——那几层再薄也不会薄到零。没有这一步，
    # 远端几节的半径**恰好等于骨半径**，画出来就是把骨头露在外面。
    envelope = [bone_r[i] * SOFT_OVER_BONE for i in range(n)]
    if boneless:
        # 无骨的软柱（触手）没有"肌腹 vs 腱"的分工——整根都是连续的软组织，也就没有
        # 关节可收细。照 bone_r 算包络会得到 0，画出零长的裂口直接崩（4 只兽实测）。
        envelope = list(radius)
    radius = [max(radius[i], envelope[i]) for i in range(n)]

    # 关节处的包络：肌腹不跨关节，所以那一圈只剩骨+腱+皮。两侧取同一个值——从关节
    # 哪一边看过去都得一样粗，肉是连续的。根部是例外：那儿的肌腹直接连进体内，不收。
    node_r = [max(envelope[max(i - 1, 0)], envelope[min(i, n - 1)])
              for i in range(n + 1)]
    node_r[0] = max(node_r[0], radius[0])

    # 接合面传力靠剪切，需要的面积 A = F/σ：软组织 0.5 MPa 下只要一两像素半径，
    # 所以**接合从来不是瓶颈**，它只是皮上一圈看得见的痕。
    weld_r = math.sqrt((bone_r[0] * PX) ** 2 + load / (math.pi * SIGMA_SOFT)) / PX
    weld_r = max(weld_r, 0.6)
    return Limb(gene, sock, load, joints, radius, bone_r, muscle, arms, ankle,
                mats, forced, weld_r, need, pad, node_r, heel)


FIT_TRIES = 18         # "腿装得下"这道门最多回头要几只兽。互穿在候选里很常见（seed 3
                       # 前十二只里有九只超过 0.75 px），10 只不够——它第 11 只才干净
FIT_SCAN = 600         # 为凑够那几只，最多扫多少个基因组。稳定性那道门本身已经很挑
                       # （seed 4 在 200 个里只有两只站得住），扫窄了就只剩一两只可选
FIT_TOL = 0.75         # 与自检 ⑯ 同一个互穿阈值


def build(seed: int, *, socks: dict[str, C.Socket] | None = None,
          veto=None) -> tuple[GN.Genome, LM.Gait, dict[str, Limb]]:
    """一只站得住、而且**装得下自己这些腿**的兽的全部肢体几何。

    第二道门是这里新加的。站得住只是运动层那一关，而"这具身体塞不塞得下这么多这么粗
    的腿"要解完粗细才知道——两条肢的挂载点只隔 8 px，各自中段却有 3.5 px 粗，需要 7 px
    净空，**落点怎么摆都要穿**（`_splay` 的 docstring 记的就是这一例）。这跟"三条腿全
    在左边所以站不住"是同一类事实：不是求解器没解好，是这个基因组不成立。所以处理方式
    也一样——回头跟运动层要下一只。

    `veto(gen, gait, limbs) -> px`：本层看不见的那部分互穿（头），由上层算好报进来。
    """
    socks = socks or C.sockets()
    best: tuple | None = None
    for attempt, (gen, gait) in enumerate(
            LM.iter_standing(seed, tries=FIT_SCAN, socks=socks)):
        if attempt >= FIT_TRIES:
            break
        got = _build_one(gen, gait, socks)
        lgs = {lg.gene.socket: lg for lg in got[1].limbs}
        # 这一层看不见头。头由 `gen_beast` 解，而腿会扫进头里（seed 9 实测一条腿从一颗头
        # 的眼球里穿过去 1.52 px）——所以留一个钩子让上层把它那一份也报进来，判据仍是同
        # 一条：装不下就回头要下一只。
        extra = 0.0 if veto is None else float(veto(*got))
        # 两件事一起判：**腿之间塞不塞得下**，以及**腿折起来会不会陷进自己肚子里**。
        # 后者原本只由 `stand_pose` 的惩罚项软性压制，压不住时就留给自检报红（历史上
        # seed 2/4 长期挂在这一条上）。它和前者是同一类事实——这个基因组在这具身体上
        # 摆不下——所以处理方式也该一样：回头要下一只，而不是硬摆一个穿模的姿势。
        # 两条阈值不一样，因为两条自检的判据不一样：互穿允许 0.75 px（和自检 ⑯ 同一个
        # 阈值，肉长在一起是对的），而"埋进核心 / 压进地面"是**一点都不许**。
        ov = max(_worst_overlap(got[2], lgs), extra)
        bu = max(_worst_burial(got[2]), _worst_ground(got[2], lgs))
        if ov <= FIT_TOL and bu <= 0.0:
            return got
        # 都不合格时怎么挑最不差的那只？**先看静止姿**。静止姿是真正被写进 .bbmodel 的
        # 那份几何——动画只是它的一个视图。把两者同权比较的话，门会为了修一处只在某个
        # 相位出现的问题而换掉一只静止姿完全合格的兽（实测 seed 5 的静止互穿因此从 0
        # 变成 1.61 px）。所以按字典序：静止的违规量优先，周期的排第二。
        st = max(_worst_overlap(got[2]) - FIT_TOL, _worst_burial(got[2]))
        score = (max(st, 0.0), max(ov - FIT_TOL, bu))
        if best is None or score < best[0]:
            best = (score, got)
    if best is None:
        raise ValueError(f"seed={seed} 没采到装得下自己腿的配置")
    return best[1]


def _worst_ground(limbs: dict[str, Limb], lgs: dict) -> float:
    """整个周期里，肢体的**肉**最深压进地面多少（px）。

    `stand_pose` 解静止姿时逐关节封了离地净空（`clearance`），可 `refold` 沿着定好的
    协同伸缩，管不了这个——走到某些相位膝盖就会往下探（seed 11 实测压到地面以下 0.66 px）。
    和"塞不下"一样，这也是"这个基因组在这具身体上走不了"的证据，交给同一道门去重采。
    """
    w = 0.0
    for n, lb in limbs.items():
        lg = lgs.get(n)
        if lg is None:
            continue
        # 净空的定义**照抄 `stand_pose` 的 `clearance`**：逐关节取两侧包络的大者，再被
        # "它到脚尖还剩多长链"钳住（够不着的净空要求本来就无解，静态解算器就是这么写的）。
        # 拿中段最粗那个半径去要求端点比静态解算器本身还严——门于是把静态完全合格的候选
        # 也否掉，退而取一个更差的（seed 5 实测静态互穿从 0 变成 1.61 px）。
        n_seg = len(lb.gene.segments)
        nfoot = 1 if lb.gene.stance == "sprawling" else 2
        nleg = n_seg - nfoot
        rest = np.cumsum(list(lb.gene.segments)[::-1])[::-1]
        need = [min(max(lb.node_r[max(i - 1, 0)], lb.node_r[min(i, n_seg)]),
                    float(rest[i])) for i in range(nleg)]
        for caps in cycle_caps(lb, lg):
            pts = [caps[0][0]] + [c[1] for c in caps]
            for i in range(1, nleg):           # 髋骑在表皮上、脚本来就贴地，都不算
                w = max(w, need[i] - float(pts[i][1]))
    return w


def _worst_burial(limbs: dict[str, Limb]) -> float:
    """最深的一处"关节埋进核心"（px）。判据与自检 ② 一致：根部那一节骑在表皮上是对的，
    从第二个关节起才算。返回的是**距离**不是场值，好和互穿用同一个阈值比。"""
    w = 0.0
    for lb in limbs.values():
        for p in lb.joints[1:]:
            q = np.asarray(p, float) - C.CORE_CENTER
            f = C.fld(q)
            if f < C.ISO:
                continue
            g = float(np.linalg.norm(C.grad(q)))
            w = max(w, (f - C.ISO) / max(g, 1e-6))
    return w


def _worst_overlap(limbs: dict[str, Limb], lgs: dict | None = None) -> float:
    """全身最深的一处互穿。给了 `lgs` 就按**整个步态周期**量，不只量静止姿。

    门必须和自检量的是同一件事。只量静止姿的话，seed 7 这种"站着 0.46 px、走到 t=0.17
    穿 1.06 px"的配置会被放行——而它一动起来就露馅。
    """
    names = list(limbs)
    w = 0.0
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            w = max(w, _worst_pair(limbs[a], limbs[b], lgs))
    return w


def _build_one(gen: GN.Genome, gait: LM.Gait, socks: dict[str, C.Socket]
               ) -> tuple[GN.Genome, LM.Gait, dict[str, Limb]]:
    loads = foot_loads(gait)
    feet = {lg.gene.socket: lg.foot for lg in gait.limbs}
    used = [socks[g.socket].pos for g in gen.limbs]
    out: dict[str, Limb] = {}
    # **按载荷从大到小依次解。** 避让是贪心的（后解的躲开先解的），所以顺序不是随意的：
    # 扛得最重的那条肢自由度最小（它的关节必须压在载荷线上，挪一点就要多长一截肌肉），
    # 该让它先占位置。反过来先摆闲肢，重载肢会被逼出很别扭的折法。
    avoid: list = []
    for gene in sorted(gen.limbs, key=lambda g: -loads.get(g.socket, 0.0)):
        me = socks[gene.socket].pos
        near = min((float(np.linalg.norm(me - q)) for q in used
                    if float(np.linalg.norm(me - q)) > 1e-6), default=1e9)
        out[gene.socket] = solve_limb(
            gene, socks[gene.socket],
            load=loads.get(gene.socket, 0.0),
            foot=feet.get(gene.socket),
            ride=gait.ride,
            territory=near,
            avoid=avoid,
        )
        lb = out[gene.socket]
        avoid += [(np.asarray(a, float), np.asarray(b, float),
                   max(lb.profile(i, 0.5), 0.5))
                  for i, (a, b) in enumerate(zip(lb.joints, lb.joints[1:]))]

    out = _splay(gen, gait, socks, loads, feet, out)
    return gen, gait, out


def limb_caps(lb: Limb) -> list[tuple[np.ndarray, np.ndarray, float]]:
    """一条肢的各节，近似成胶囊（轴 + 中段半径）。碰撞检测与避让共用。"""
    return [(np.asarray(a, float), np.asarray(b, float), max(lb.profile(i, 0.5), 0.5))
            for i, (a, b) in enumerate(zip(lb.joints, lb.joints[1:]))]


def meta_angle(stance: str, contact, hip, out, foot_segs, span: float) -> float:
    """掌骨立多陡——**由可达性反解**，和 `solve_limb` 同一条判据，只是改成连续二分。

    `solve_limb` 解静止姿时按 4° 阶梯往上试，取第一个够得着的。逐帧解要连续（阶梯会
    一格一格跳），而且**逐帧解它就自动给出抬跟**：落点钉在地上、髋越过去之后腿够不着
    踝，掌骨只好再立起来一点。抬跟不是画的，是同一条约束在时间上的展开。

    碰撞检测与动画必须调同一个函数：各写一份的话，掰落点掰的是另一条腿。
    """
    def ok(d: float) -> bool:
        paw = foot_chain(stance, contact, foot_segs, out, d)
        return float(np.linalg.norm(paw[0] - hip)) <= 0.97 * span

    base = META_DEG[stance]
    if ok(base):
        return base
    if not ok(88.0):
        return 88.0
    lo, hi = base, 88.0
    for _ in range(18):
        mid = 0.5 * (lo + hi)
        if ok(mid):
            hi = mid
        else:
            lo = mid
    return hi


def cycle_caps(lb: Limb, lg, k: int = 40) -> list[list[tuple[np.ndarray, np.ndarray, float]]]:
    """这条肢在**整个步态周期**里扫过的一串姿态，各自近似成胶囊。

    静止姿不打架不代表走起来不打架——腿在一个周期里扫过的体积比它站着时占的大得多。
    实测 seed 7 的 limb_fl 与 limb_ml 静止姿完全不碰，走到 t=0.40 却互穿 2.06 px。

    只重折不重解姿态：折叠协同是站姿定死的（见 `synergy`），走一步只是沿它伸缩。落点
    走 `LimbGait.foot_at`——和动画用的是同一条轨迹，这是必须的：两边各写一份的话，掰
    落点掰的是另一条腿（实测这里只看到 1.12 px，而动画真穿 1.97 px）。

    `k` 默认 40，和 `check_beast_anim.N` 一样。**门与自检必须在同一组相位上比**：门只采
    16 个相位时，峰值落在两个采样点之间就漏掉了（seed 2 实测门看到 ≤0.75 而自检量出
    0.85）。和 `locomotion.SAMPLES` 取 40/48 的公倍数是同一条纪律。
    """
    cached = getattr(lb, "_sweep", None)
    if cached is not None and cached[0] == k:
        return cached[1]
    if lg is None or lg.foot is None:
        return [limb_caps(lb)]
    nfoot = 1 if lb.gene.stance == "sprawling" else 2
    nleg = len(lb.gene.segments) - nfoot
    if nleg < 1:
        return [limb_caps(lb)]
    leg_segs = lb.gene.segments[:nleg]
    foot_segs = lb.gene.segments[nleg:]
    w, phi = synergy(lb.joints[:nleg + 1], lb.gene.kind)
    e_b, e_d, _L = bend_plane(lb.joints[0], lb.joints[nleg], lb.gene.kind)
    nrm = np.cross(e_b, e_d)
    nrm = nrm / max(float(np.linalg.norm(nrm)), 1e-9)
    hip = np.asarray(lb.joints[0], float)
    d0 = np.asarray(lg.foot, float) - hip
    d0[1] = 0.0
    out = d0 / max(float(np.linalg.norm(d0)), 1e-9)
    rad = [max(lb.profile(i, 0.5), 0.5) for i in range(len(lb.gene.segments))]
    # **静止姿必须也在这一串里**：模型是按中立落点建的，它是渲染出来的那一帧，不许穿。
    # 而它未必落在任何一个采样相位上（中立落点是支撑相的中点，不是 j/k 里的某个）。
    poses = [limb_caps(lb)]
    # **两圈**：第一圈把折叠量 φ 接力算到绕回起点，第二圈才留下几何。动画层的 `_march`
    # 也是两圈——两边必须选到同一个解支，否则门比的是另一条腿。
    for rnd in range(2):
        if rnd:
            poses = [limb_caps(lb)]
        for j in range(k):
            con = lg.foot_at(j / k)
            deg = meta_angle(lb.gene.stance, con, hip, out, foot_segs, sum(leg_segs))
            paw = foot_chain(lb.gene.stance, con, foot_segs, out, deg)
            leg, phi = refold(hip, paw[0], leg_segs, lb.gene.kind, w, phi, nrm)
            pts = leg + paw[1:]
            poses.append([(np.asarray(a, float), np.asarray(b, float), rad[i])
                          for i, (a, b) in enumerate(zip(pts, pts[1:]))])
    # 挂在这条肢自己身上。`solve_limb` 每次都造新对象，所以缓存天然随几何一起作废——
    # 不这么做的话 n 条肢两两比较会把同一条的扫掠算 n−1 遍（6 条肢就是 5 倍白工）。
    lb._sweep = (k, poses)
    return poses


def _caps_worst(ca, cb) -> float:
    w = 0.0
    for (a0, a1, ra) in ca:
        for (b0, b1, rb) in cb:
            dd, _m = VR.seg_dist(a0, a1, b0, b1)
            w = max(w, ra + rb - dd)
    return w


def _worst_pair(a: Limb, b: Limb, lgs: dict | None = None) -> float:
    """两条肢最深的一处互穿。给了 `lgs` 就按**整个周期扫过的姿态**逐相位比，不只比站姿。"""
    if lgs is None:
        return _caps_worst(limb_caps(a), limb_caps(b))
    pa = cycle_caps(a, lgs.get(a.sock.name))
    pb = cycle_caps(b, lgs.get(b.sock.name))
    # 同一相位才可能真的撞上——两条腿在不同时刻各自占过同一块空间不算穿模
    n = max(len(pa), len(pb))
    return max(_caps_worst(pa[i % len(pa)], pb[i % len(pb)]) for i in range(n))


def _splay(gen, gait, socks, loads, feet, out: dict[str, Limb]) -> dict[str, Limb]:
    """**把还在打架的两条腿的落点侧向掰开，再解一次姿态。**

    姿态求解器只能改折法，改不了落点——而落点是 `locomotion.place_feet` 定的，那一层
    按可达半径、舒适伸展度与静态稳定摆点，**从头到尾不知道腿有多粗**。实测最坏的一对：
    挂载点相距 8.6 px、落点相距 8.8 px，可两条腿的中段各有 3.2 与 3.4 px 粗，需要
    6.6 px 净空——并排放不下，怎么折都要穿。这是层间的信息缺口，不是求解器的毛病。

    这里补的正是那条缺口：**绕各自的髋在地平面上转开**，到髋的距离一分不动（可达半径
    与舒适伸展度因此完全不变），只动方位角。转开会改变支撑多边形，所以每次都重新量
    静态稳定余量：掉得太多就退回原样，把这一对留给自检去报——宁可报出来，也不要为了
    让自检变绿把兽摆成站不住的。

    locomotion 不能反过来 import 这一层（会成环），所以这一步只能在有了粗细之后做。
    """
    load_names = [g.socket for g in gen.limbs if g.socket in feet]
    if len(load_names) < 2:
        return out

    lgs_all = {lg.gene.socket: lg for lg in gait.limbs}

    def worst_all(sol: dict[str, Limb]) -> float:
        """这一版布局里最深的一处互穿（px）。

        **只算互穿，不把"关节埋进核心"也算进来。** 试过合并计分：结果是既治不好埋，
        又把本来能修好的互穿一起卡住（实测同一只兽从"只剩埋" 变成"埋 + 2.45 px 互穿"）。
        原因是两者不同源——互穿能靠挪落点修，埋是姿态层两条无解约束的残留，把它塞进
        同一个贪心的接受判据里，只会让能修的那件也不敢动。
        """
        m = 0.0
        for i, a in enumerate(load_names):
            for b in load_names[i + 1:]:
                if a in sol and b in sol:
                    m = max(m, _worst_pair(sol[a], sol[b], lgs_all))
        return m
    lgs = lgs_all
    com2 = np.array([gait.com[0], gait.com[2]])
    # **量的必须是全周期的真余量，不是 `feasible` 那个"全脚着地"的上界。** 掰落点会改变
    # 支撑多边形，而 `feasible` 只看所有脚都在地上的那一刻——摆动相里少一只脚时多边形
    # 更小，正是它可能包不住质心的时候。用上界当判据的后果实测得很清楚：seed 9 掰完之后
    # 某个相位的质心落到多边形外，下游 `contact_forces` 直接抛异常（"任何非负反力组合都
    # 撑不住"），整只兽建不出来。
    base_margin = LM.evaluate(gait.limbs, com2)[0]
    moved = False
    for _ in range(8):
        pairs = []
        for i, a in enumerate(load_names):
            for b in load_names[i + 1:]:
                if a in out and b in out:
                    w = _worst_pair(out[a], out[b], lgs)
                    if w > 0.5:
                        pairs.append((w, a, b))
        if not pairs:
            break
        pairs.sort(reverse=True)
        w, a, b = pairs[0]
        ok = False
        # 注意：一轮只处理最深的那一对，处理完**整条腿都会重解**，其余各对的几何随之
        # 变化，所以下一轮必须重新找而不是沿用这一轮的名单。
        for extra in (0.7, 1.2, 1.8, 2.6):
            trial = {}
            for name, other in ((a, b), (b, a)):
                lg = lgs.get(name)
                if lg is None or lg.foot is None:
                    continue
                hip = np.array([lg.hip[0], 0.0, lg.hip[2]])
                # **平移，不是旋转。** 绕髋转对垂直下垂的肢完全无效——腹肢的落点就在髋
                # 的正下方，力臂是 0.0（实测），转多少度它都不动。改成沿"背离对方落点"
                # 的方向在地平面上推出去，再夹回自己的可达半径里。
                away = (np.asarray(out[name].tip, float)
                        - np.asarray(out[other].tip, float))
                away[1] = 0.0
                nn = float(np.linalg.norm(away))
                if nn < 1e-6:
                    continue
                cand = np.asarray(lg.foot, float) + away / nn * (w * extra * 0.5)
                v = cand - hip
                r = float(np.hypot(v[0], v[2]))
                lim = float(lg.reach)
                if r > lim:                       # 够不着就贴着可达边界放
                    cand = hip + v / max(r, 1e-9) * lim
                cand[1] = 0.0
                trial[name] = cand
            if not trial:
                break
            old_feet = {n: np.array(lgs[n].foot, float) for n in trial}
            for n, f in trial.items():
                lgs[n].foot = f
            margin = LM.evaluate(gait.limbs, com2)[0]
            if margin < max(base_margin * 0.6, LM.MARGIN_OK):   # 掰开会站不住 ⇒ 退回去
                for n, f in old_feet.items():
                    lgs[n].foot = f
                continue
            for n in trial:
                feet[n] = lgs[n].foot
            ok, moved = True, True
            break
        if not ok:
            # 这一对掰不开（掰开就站不住）——换下一对试，别整个放弃
            done = {(a, b)}
            nxt = next(((w2, a2, b2) for w2, a2, b2 in pairs
                        if (a2, b2) not in done), None)
            if nxt is None:
                break
            w, a, b = nxt
            continue
        # 用新落点把全部承重肢重解一遍（避让顺序仍按载荷从大到小）
        before = worst_all(out)
        keep = dict(out)
        avoid2: list = []
        for gene in sorted(gen.limbs, key=lambda g: -loads.get(g.socket, 0.0)):
            me = socks[gene.socket].pos
            near = min((float(np.linalg.norm(me - socks[g.socket].pos))
                        for g in gen.limbs if g.socket != gene.socket), default=1e9)
            out[gene.socket] = solve_limb(
                gene, socks[gene.socket], load=loads.get(gene.socket, 0.0),
                foot=feet.get(gene.socket), ride=gait.ride, territory=near,
                avoid=avoid2)
            avoid2 += limb_caps(out[gene.socket])
        # **只有全局最深的那处穿插真的变浅了才收下这一步。** 掰开一对会把所有腿重解，
        # 完全可能把另一对挤得更死——实测有一只兽因此从 2.65 px 恶化到 5.82 px。
        # 贪心必须带这条守卫，否则它会一路"改进"到更糟。
        if worst_all(out) >= before - 1e-3:
            out = keep
            for n, f in old_feet.items():
                lgs[n].foot = f
                feet[n] = f
            break
    if moved:
        LM.optimize_phases(gait.limbs, com2, seed=gen.seed)
    return out


def report_table(limbs: dict[str, Limb], W: float) -> str:
    """逐肢一行。载荷那一列是**支撑相峰值**，除以体重可以大于 100%——半正弦的峰值
    高过均值，而均值才是准静态份额（见 PEAK_SHAPE）。"""
    rows = [f"  {'槽':<10}{'类型':<10}{'供体':<11}{'总长':>6}{'峰值N':>8}{'/体重':>7}"
            f"{'根粗':>6}{'梢粗':>6}{'接合痕':>7}{'埋在体内':>9}{'脚掌':>9}  材质"]
    for lb in sorted(limbs.values(), key=lambda x: -x.load):
        pad = f"{lb.pad[0] * 2:.1f}×{lb.pad[1] * 2:.1f}" if lb.bearing else "—"
        col = f"{lb.weld_r:.1f}" if lb.weld_r else "—"
        rows.append(f"  {lb.name:<10}{lb.gene.kind:<10}{lb.gene.source:<11}"
                    f"{lb.gene.length:>6.1f}{lb.load:>8.0f}{lb.load / W * 100:>6.0f}%"
                    f"{lb.radius[0]:>6.2f}{lb.radius[-1]:>6.2f}{col:>7}"
                    f"{lb.buried:>8.0%}{pad:>9}"
                    f"  {'/'.join(lb.mats)}{'  ← 被迫改折向' if lb.forced else ''}")
    return "\n".join(rows)


def report(seed: int) -> str:
    _gen, gait, limbs = build(seed)
    W = body_weight()
    head = (f"肢体 seed={seed}   体重 {W:.0f} N（{W / G:.0f} kg）  "
            f"骑乘 {gait.ride:+.1f} px")
    return head + "\n" + report_table(limbs, W)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=None)
    ap.add_argument("--count", type=int, default=3)
    args = ap.parse_args()
    seeds = [args.seed] if args.seed is not None else list(range(1, args.count + 1))
    for s in seeds:
        print(report(s))
        print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
