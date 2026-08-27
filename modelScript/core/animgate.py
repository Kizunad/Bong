#!/usr/bin/env python3
"""动画后验的共享判据 —— 把 `creatures/*/check_*.py` 里 2694 行的教训收上来。

那些文件里**注释比代码值钱**：每一条阈值都是一轮调试换来的标定。抽取时连注释和理由
一起搬 —— 数字背后的理由丢了，阈值就会被下一个模型随手改松。

**分工**：本模块提供「判据」，不提供「测量」。哪几根骨是脚、什么算支撑相、颈是怎么
分片的 —— 这些是物种特有的，留在各 creature 的 rig.py / check_*.py 里。creature 交给
本模块的是**测量闭包**（`lowest_at(t)` / `boxes_at(t)` / `frac_at(t)` …），本模块负责
拿它们做判定并把标定好的门限和理由集中在一处。

**查的必须是导出的关键帧，不是解析采样器。** 引擎播的是关键帧之间的线性插值；解析式
再光滑，采样数不够的话中间那一段照样塌。`pose_from_tracks()` 就是把 `build_tracks` 的
产物插值回姿态用的 —— 两个 creature 各自独立写了一遍同一个东西。

五道门 + 每道门自己的缺陷注入器（理由同 `gatekit`：判据本身会假绿，而模型不会怀疑它。
`self_test()` 里报不出自己该抓的缺陷的门，直接算失效）。
"""

from __future__ import annotations

import math
from collections.abc import Callable
from dataclasses import dataclass, field

import numpy as np

# ---------------------------------------------------------------- 标定门限
#
# 穿地容差取**绝对值**而不是体高比例：16 单位 = 1 格 = 16 纹理像素，所以 1 单位就是一个
# 像素，0.5 单位以下看不见。这条本来是抓「整只翼尖扫地」「脚陷进土里」这种以单位计的错，
# 不是抓亚像素。另一头的现实是跗跖（裸胫）在静止姿下离地只有 0.25/0.40/0.58 单位（三档），
# 步幅两端腿一斜它就沉下去零点几 —— 那是模型几何的固有余量，不是动画做错了。
SINK_TOL = 0.5

# 支撑相里偏离匀速直线的最大量（模型单位）。鹅那份用绝对值 0.30，鹫那份按髋高归一
# （0.055·H）—— 两种写法都对，取决于物种尺度差多大。这里给绝对值默认，需要归一的
# creature 自己乘。
SKATE_TOL = 0.30

# 循环首末帧的逐骨最大通道差。0 = 严格相等；留 0.02 给浮点累积，再大一轮循环就抖一下。
SEAM_TOL = 0.02

# 接缝允许的最大缝隙（>0 = 断开）。0 = 刚好贴合；留一点余量给渲染时的抗锯齿。
LINK_BREAK_TOL = 0.05

# 逐帧互穿容差（px）。取的是**模型自己的分辨率下限**：半像素以下的柱子渲出来会消失或
# 闪烁，几何层已经按这个数截断过。脚绕接触点翻转时掌垫的角点会在这个尺度上下摆
# （蛛足的跗节与趾行的趾节各实测到 0.30 / 0.43 px），比模型分辨得出的还细。
OVERLAP_TOL = 0.75

# 单支撑相里质心至少要压到支撑脚的这个比例上（1.0 = 正压在脚上）。
BALANCE_MIN = 0.45

# 默认逐帧采样数。低于 40 抓不住峰值，高于 200 只是白烧时间。
FRAMES = 60


@dataclass
class AnimGateResult:
    key: str
    label: str
    ok: bool
    worst: float
    detail: str
    extra: dict = field(default_factory=dict)


# ================================================================ 导出曲线回放
_ATTR = {"rotation": "rot", "position": "pos", "scale": "scale"}


def pose_from_tracks(tracks: dict, tt: float, pose_factory):
    """把 `build_tracks` 的产物在 tt 秒处线性插值回一个 Pose。

    **这是所有后验的入口**：前面几条查的都是那条连续函数本身，可进游戏播的是采样 +
    裁剪之后的关键帧。采样给疏了、裁剪给狠了，函数再完美也白搭。

    pose_factory 交出一个空 Pose（`animkit.Pose` / `anim_rig.Pose` 都行）—— 本模块
    不 import 任何一份骨架实现，省得两条动画栈在这里又缠成一团。
    """
    p = pose_factory()
    for bone, chans in tracks.items():
        for chan, vals in chans.items():
            # 两端都要钳住。只写「落不进任何区间就取末值」是不对的 —— tt 落在首帧
            # **之前**时那样会突然跳到动作结尾。区间内正常插值，区间外各自贴住那一端。
            if tt <= vals[0][0]:
                got = vals[0][1]
            else:
                got = vals[-1][1]
                for j in range(len(vals) - 1):
                    if vals[j][0] <= tt <= vals[j + 1][0]:
                        t0, v0 = vals[j]
                        t1, v1 = vals[j + 1]
                        s = (tt - t0) / (t1 - t0) if t1 > t0 else 0.0
                        got = [v0[k] + (v1[k] - v0[k]) * s for k in range(3)]
                        break
            setattr(p[bone], _ATTR[chan], list(got))
    return p


# ================================================================ 测量适配器
def bone_boxes(rig, pose, groups: dict, *, merge: bool = False) -> dict:
    """{组名: 该组骨骼几何在世界空间的包围盒}。

    默认**逐件**保留（值是一串盒），因为链断裂/互穿要的是「最近的那一对」；合并成一个
    大 AABB 会把它冲淡。要粗口径就传 merge=True。

    rig 只需要给出 `world(pose)` 和 `bone_points(骨名)` —— `animkit.PoseRig` 和
    `anim_rig.Rig` 都满足，creature 本地的 rig 也大多满足。
    """
    W = rig.world(pose)
    out = {}
    for label, bones in groups.items():
        boxes = []
        for n in bones:
            if n not in W:
                continue
            local = rig.bone_points(n)
            if not len(local):
                continue
            wp = np.asarray(local) @ W[n][:3, :3].T + W[n][:3, 3]
            boxes.append((wp.min(0), wp.max(0)))
        if not boxes:
            continue
        if merge:
            lo = np.min([b[0] for b in boxes], axis=0)
            hi = np.max([b[1] for b in boxes], axis=0)
            out[label] = (lo, hi)
        else:
            out[label] = boxes
    return out


def lowest_bone(rig, pose) -> tuple[float, str]:
    """最低点及其所属骨骼。

    只报一个数字的话，「穿地 0.43」根本不知道该去调什么 —— 是脚陷了、翼尖扫地了、
    还是尾羽拖地了，改法完全不同。
    """
    W = rig.world(pose)
    best, who = 1e9, "-"
    for n in rig.order:
        pts = rig.bone_points(n)
        if not len(pts):
            continue
        y = float((np.asarray(pts) @ W[n][:3, :3].T + W[n][:3, 3])[:, 1].min())
        if y < best:
            best, who = y, n
    return best, who


def volume_center(rig, pose) -> np.ndarray:
    """质心近似：按 element 体积加权的形心。

    骨/羽密度当然不同，但这里只用来判「重心有没有压到支撑脚那一侧」，方向对就够，
    不需要真密度。
    """
    W = rig.world(pose)
    tot, acc = 0.0, np.zeros(3)
    for n in rig.order:
        for u in rig.bones[n].elements:
            e = rig.elements.get(u)
            if e is None:
                continue
            f, t = np.array(e["from"], float), np.array(e["to"], float)
            vol = float(np.prod(np.maximum(t - f, 1e-3)))
            c = (f + t) / 2
            acc += vol * (W[n][:3, :3] @ c + W[n][:3, 3])
            tot += vol
    return acc / max(tot, 1e-9)


def aabb_gap(a, b) -> float:
    """两个 AABB 的间隙：>0 = 分开多远，≤0 = 重叠多深（取负）。

    按**三轴分离**量，取各轴的最大值 —— 不能量「竖直净空」那种单轴口径：威吓姿的头是
    往前伸的，压根不在身体上方，竖直口径量出来是 −1.5、判成「没脖子」，其实那一帧脖子
    伸得最长。
    """
    (alo, ahi), (blo, bhi) = a, b
    seps = [max(blo[k] - ahi[k], alo[k] - bhi[k]) for k in range(3)]
    return float(max(seps))


def _as_boxes(v):
    """把「一个盒」和「一串盒」统一成一串。

    合并成一个大 AABB 会把「一组件里最近的那一对」冲淡：颈四片各自的盒挨着，合并之后
    整条颈是一个大盒，链断裂就永远量不出来。所以逐件保留，取最近的一对。
    """
    if len(v) == 2 and np.shape(v[0]) == (3,) and np.shape(v[1]) == (3,):
        return [v]
    return list(v)


def min_gap(a, b) -> float:
    """两**组**盒之间的最小分离（负 = 有重叠）。单个盒也收，自动当成只有一件的组。"""
    return min(aabb_gap(x, y) for x in _as_boxes(a) for y in _as_boxes(b))


# ================================================================ 五道门
def gate_ground(lowest_at, n: int = FRAMES, sink_tol: float = SINK_TOL) -> AnimGateResult:
    """穿地：逐帧全模型最低点。

    lowest_at(t01) → (y, 骨名)。地面动作不该扎进地里，翼尖扫地尤其常见 —— 张翼威慑
    那一下最容易把初级飞羽插进土里。
    """
    worst, who, at = 1e9, "-", 0.0
    for i in range(n):
        t = i / n
        y, bone = lowest_at(t)
        if y < worst:
            worst, who, at = float(y), bone, t
    ok = worst >= -sink_tol
    return AnimGateResult(
        "ground", "穿地", ok, worst,
        f"最低点 {worst:+.2f} [{who}] @t={at:.2f}"
        + ("" if ok else f"   ← 扎进地里 {-worst - sink_tol:.2f}px（容差 {sink_tol}）"),
        {"bone": who, "t": at})


def slip_residual(us, zs) -> tuple[float, float]:
    """支撑相里脚掌偏离匀速直线的最大量，以及后移速率。

    **按支撑进度 u 拟合，不按时间 t** —— 相位偏移会让支撑相跨过 t=0，按 t 拟合出来的
    残差是假的（那条直线被硬掰过一次原点）。
    """
    us = np.asarray(us, float)
    zs = np.asarray(zs, float)
    if len(us) < 2:
        raise ValueError("支撑相采样少于两点，拟合不出滑步")
    A = np.vstack([us, np.ones(len(us))]).T
    slope, icpt = np.linalg.lstsq(A, zs, rcond=None)[0]
    return float(slope), float(np.abs(zs - (slope * us + icpt)).max())


def gate_slip(stance_samples: dict, skate_tol: float = SKATE_TOL) -> AnimGateResult:
    """滑步：支撑相里脚掌必须贴地、且随支撑进度**匀速**后移。

    stance_samples: {脚名: (us, zs)}，us 是支撑进度 0..1，zs 是该脚的世界 z。
    两足动画的头号破绽 —— 渲染静帧一律看不出来，只能算。
    """
    if not stance_samples:
        raise ValueError("没有支撑相样本，滑步门无从判定")
    worst, who, rate = -1.0, "-", 0.0
    for foot, (us, zs) in stance_samples.items():
        slope, res = slip_residual(us, zs)
        if res > worst:
            worst, who, rate = res, foot, slope
    ok = worst <= skate_tol
    return AnimGateResult(
        "slip", "滑步", ok, worst,
        f"最大残差 {worst:.3f} [{who}]，后移 {rate:+.2f}/支撑相"
        + ("" if ok else f"   ← 超标（容差 {skate_tol}）"),
        {"foot": who, "rate": rate})


def gate_seam(pose_at, bones, seam_tol: float = SEAM_TOL,
              loop: bool = True) -> AnimGateResult:
    """循环接缝：首末帧姿态必须逐骨相等，否则每轮循环抖一下。

    单次动作（loop=False）不查 —— 它本来就该停在别处。
    """
    if not loop:
        return AnimGateResult("seam", "循环接缝", True, 0.0, "单次动作，不查接缝")
    a, b = pose_at(0.0), pose_at(1.0)
    worst, who, chan = 0.0, "-", "-"
    for bone in bones:
        if bone not in a or bone not in b:
            continue
        for attr in ("rot", "pos", "scale"):
            d = max(abs(x - y) for x, y in zip(getattr(a[bone], attr), getattr(b[bone], attr)))
            if d > worst:
                worst, who, chan = float(d), bone, attr
    ok = worst <= seam_tol
    return AnimGateResult(
        "seam", "循环接缝", ok, worst,
        f"接缝 {worst:.4f} [{who}.{chan}]"
        + ("" if ok else f"   ← 每轮循环抖一下（容差 {seam_tol}）"),
        {"bone": who, "channel": chan})


def gate_overlap(boxes_at, pairs, n: int = FRAMES,
                 tol: float = OVERLAP_TOL) -> AnimGateResult:
    """逐帧互穿：**摆起来会不会穿是另一个问题**，静止姿不穿模不代表走起来不穿。

    boxes_at(t01) → {组名: (lo, hi)}；pairs 是要查的组对。容差见 OVERLAP_TOL 那段。
    """
    worst, who, at = 0.0, "-", 0.0
    for i in range(n):
        t = i / n
        boxes = boxes_at(t)
        for a, b in pairs:
            if a not in boxes or b not in boxes:
                continue
            depth = -min_gap(boxes[a], boxes[b])
            if depth > worst:
                worst, who, at = float(depth), f"{a} × {b}", t
    ok = worst <= tol
    return AnimGateResult(
        "overlap", "逐帧互穿", ok, worst,
        f"最深 {worst:.2f}px [{who}] @t={at:.2f}"
        + ("" if ok else f"   ← 走起来穿模（容差 {tol}）"),
        {"pair": who, "t": at})


def gate_chain_break(boxes_at, chains: dict, n: int = FRAMES,
                     tol: float = LINK_BREAK_TOL) -> AnimGateResult:
    """链断裂：相邻分片之间不许拉开缝。

    一整块几何跟着单根骨走，骨一转它就同时脱开两头 —— 威吓那帧的颈和张嘴时的下喙都
    栽过，渲出来是方块悬在空中。

    chains: {链名: [组名, 组名, ...]}，按相邻两两量间隙。
    """
    worst, who, at = -9.0, "-", 0.0
    for i in range(n):
        t = i / n
        boxes = boxes_at(t)
        for label, groups in chains.items():
            for a, b in zip(groups, groups[1:]):
                if a not in boxes or b not in boxes:
                    continue
                d = min_gap(boxes[a], boxes[b])
                if d > worst:
                    worst, who, at = d, label, t
    ok = worst <= tol
    return AnimGateResult(
        "chain", "链断裂", ok, worst,
        f"最宽接缝 {worst:+.2f} [{who}] @t={at:.2f}"
        + ("" if ok else f"   ← 分片脱开（容差 {tol}）"),
        {"chain": who, "t": at})


def gate_balance(frac_at, n: int = FRAMES, min_peak: float = BALANCE_MIN) -> AnimGateResult:
    """质心平衡（两足专属）：单支撑相里质心必须真的压到支撑脚上方。

    四足有静态三角支撑，没有这条约束；少了它，走路是在冰面上平移。

    frac_at(t01) → 质心相对支撑脚的侧向占比（>0 = 与支撑脚同侧，1.0 = 正压在脚上），
    非单支撑相返回 None。

    **只查符号对不对是不够的**：把横移整个删掉之后，光靠骨盆侧倾也能让质心偏出零点几
    个百分点，同侧率仍是 100% —— 而那正是在冰面上平移的样子。所以还要求**峰值真的
    压过去**。这条门的鉴别力全在第二个判据上。
    """
    worst, best, n_bad, cnt = float("inf"), 0.0, 0, 0
    for i in range(n):
        frac = frac_at(i / n)
        if frac is None:
            continue
        cnt += 1
        worst = min(worst, float(frac))
        best = max(best, float(frac))
        if frac <= 0.0:
            n_bad += 1
    if cnt == 0:
        return AnimGateResult("balance", "质心平衡", True, 0.0,
                              "没有单支撑相（双支撑步态），不判")
    worst = 0.0 if worst == float("inf") else worst
    flag = ""
    if n_bad:
        flag = "   ← 质心没压过去"
    elif best < min_peak:
        flag = f"   ← 横移不足（峰值 {best:.2f} < {min_peak}）—— 同侧率再高也是在冰面上平移"
    return AnimGateResult(
        "balance", "质心平衡", not flag, best,
        f"单支撑 {cnt} 帧，同侧率 {(cnt - n_bad) / cnt:.0%}，"
        f"侧向占比 {worst:+.2f}..{best:+.2f}（1.0 = 正压在支撑脚上）{flag}",
        {"frames": cnt, "same_side": (cnt - n_bad) / cnt, "peak": best})


# ================================================================ 缺陷注入器
# 动画门吃的是**测量闭包**，所以注入缺陷 = 包一层闭包，不用碰任何几何。
class AnimInjectionImpossible(RuntimeError):
    """这条动作上造不出该门要抓的那种缺陷。"""


def sink_by(lowest_at, depth: float = 2.0):
    """整只沉进地里 depth 单位 —— 穿地门必须报。"""
    def wrapped(t):
        y, bone = lowest_at(t)
        return y - depth, bone
    return wrapped


def skate_by(stance_samples: dict, amount: float = 1.0):
    """给支撑相的后移曲线掺一个正弦扰动 —— 匀速被破坏，滑步门必须报。"""
    out = {}
    for foot, (us, zs) in stance_samples.items():
        us = np.asarray(us, float)
        out[foot] = (us, np.asarray(zs, float) + amount * np.sin(us * math.pi))
    return out


def break_seam_by(pose_at, bone: str, delta: float = 5.0):
    """把末帧某根骨拧开 delta 度 —— 循环接缝门必须报。"""
    def wrapped(t):
        p = pose_at(t)
        if t >= 1.0:
            p[bone].rot = [v + delta for v in p[bone].rot]
        return p
    return wrapped


def _group_span(boxes) -> tuple[np.ndarray, np.ndarray]:
    """一组盒的整体包围盒。注入器要整组一起挪，不能只挪其中一件。"""
    items = _as_boxes(boxes)
    return (np.min([b[0] for b in items], axis=0), np.max([b[1] for b in items], axis=0))


def _shift_group(boxes, off: np.ndarray) -> list:
    return [(np.asarray(lo, float) + off, np.asarray(hi, float) + off)
            for lo, hi in _as_boxes(boxes)]


def overlap_by(boxes_at, a: str, b: str):
    """把 a 组整体挪到与 b 组同心 —— 逐帧互穿门必须报。

    **必须走 `_as_boxes` 归一化**：`bone_boxes()` 默认逐件返回一串盒（合并成一个大
    AABB 会把「组里最近的那一对」冲淡），直接 `alo, ahi = boxes[a]` 在组里不止一件时
    抛 `ValueError: too many values to unpack`，整个 `self_test()` 当场崩掉 —— 而
    self_test 正是用来证明门有鉴别力的那一步，它自己崩了就什么都证明不了。
    """
    def wrapped(t):
        boxes = dict(boxes_at(t))
        if a not in boxes or b not in boxes:
            raise AnimInjectionImpossible(f"这一帧没有 {a} 或 {b}，造不出互穿")
        alo, ahi = _group_span(boxes[a])
        blo, bhi = _group_span(boxes[b])
        boxes[a] = _shift_group(boxes[a], (blo + bhi) / 2 - (alo + ahi) / 2)
        return boxes
    return wrapped


def snap_chain_by(boxes_at, group: str, gap: float = 3.0):
    """把链上某一节整体拉开 gap —— 链断裂门必须报。

    同 `overlap_by`：组里可能不止一件，一律走 `_as_boxes` 归一化后整组平移。
    """
    def wrapped(t):
        boxes = dict(boxes_at(t))
        if group not in boxes:
            raise AnimInjectionImpossible(f"这一帧没有 {group}，造不出断链")
        boxes[group] = _shift_group(boxes[group], np.array([0.0, gap, 0.0]))
        return boxes
    return wrapped


def flatten_balance_by(frac_at, keep: float = 0.02):
    """把横移压扁到几乎为零，但**符号全保留** —— 同侧率仍是 100%。

    这正是「在冰面上平移」那一档：只查符号的门在这里会全绿。峰值判据是唯一能抓住它的
    东西，所以这个注入器就是那条判据的存在证明。
    """
    def wrapped(t):
        frac = frac_at(t)
        return None if frac is None else frac * keep
    return wrapped


# ================================================================ 组装
@dataclass
class AnimGates:
    """一条动作的门禁声明。测量闭包由 creature 交出来，判据和门限在库里。"""

    title: str
    lowest_at: Callable | None = None
    stance_samples: dict | None = None
    pose_at: Callable | None = None
    bones: tuple = ()
    loop: bool = True
    boxes_at: Callable | None = None
    overlap_pairs: tuple = ()
    chains: dict | None = None
    frac_at: Callable | None = None
    seam_bone: str | None = None
    overlap_probe: tuple | None = None
    chain_probe: str | None = None
    frames: int = FRAMES
    sink_tol: float = SINK_TOL
    skate_tol: float = SKATE_TOL
    seam_tol: float = SEAM_TOL
    overlap_tol: float = OVERLAP_TOL
    chain_tol: float = LINK_BREAK_TOL
    balance_min: float = BALANCE_MIN

    def specs(self):
        """(门函数, 注入器) 对，只包含声明齐全的门。

        门函数收一个可选的**替换测量源**：差分自证时把注入器包过的闭包喂进去，
        跑的是同一道判据、同一套门限，只有测量被改坏了。
        """
        out = []
        if self.lowest_at is not None:
            out.append((lambda src=None: gate_ground(
                self.lowest_at if src is None else src, self.frames, self.sink_tol),
                lambda: sink_by(self.lowest_at)))
        if self.stance_samples:
            out.append((lambda src=None: gate_slip(
                self.stance_samples if src is None else src, self.skate_tol),
                lambda: skate_by(self.stance_samples)))
        if self.pose_at is not None and self.loop:
            bone = self.seam_bone or (self.bones[0] if self.bones else None)
            if bone is None:
                raise ValueError(f"{self.title}: 声明了 pose_at 却没给 bones，接缝门无从查起")
            out.append((lambda src=None: gate_seam(
                self.pose_at if src is None else src, self.bones, self.seam_tol, self.loop),
                lambda b=bone: break_seam_by(self.pose_at, b)))
        if self.boxes_at is not None and self.overlap_pairs:
            probe = self.overlap_probe or self.overlap_pairs[0]
            out.append((lambda src=None: gate_overlap(
                self.boxes_at if src is None else src, self.overlap_pairs,
                self.frames, self.overlap_tol),
                lambda p=probe: overlap_by(self.boxes_at, p[0], p[1])))
        if self.boxes_at is not None and self.chains:
            probe = self.chain_probe or next(iter(self.chains.values()))[-1]
            out.append((lambda src=None: gate_chain_break(
                self.boxes_at if src is None else src, self.chains,
                self.frames, self.chain_tol),
                lambda g=probe: snap_chain_by(self.boxes_at, g)))
        if self.frac_at is not None:
            out.append((lambda src=None: gate_balance(
                self.frac_at if src is None else src, self.frames, self.balance_min),
                lambda: flatten_balance_by(self.frac_at)))
        if not out:
            raise ValueError(f"{self.title}: 一个测量闭包都没给，没有门可跑")
        return tuple(out)

    def run_all(self) -> list[AnimGateResult]:
        return [fn() for fn, _ in self.specs()]

    def report(self) -> int:
        print(f"{self.title} 动画后验:")
        bad = 0
        for g in self.run_all():
            bad += 0 if g.ok else 1
            print(f"  {'✓' if g.ok else '✗'} {g.label}: {g.detail}")
        print(f"  → {bad} 道门未过")
        print("  注：姿态本身对不对，这几条量不出 —— 一个从头到尾摆错但摆得很平滑的动作"
              "照样满分，那一层只能出图用眼睛看。")
        return bad

    def self_test(self, *, verbose: bool = True) -> int:
        """每道门：干净必须过，注入对应缺陷后必须报。返回失效门数。"""
        if verbose:
            print(f"{self.title} 动画门差分自证:")
        broken = 0
        for fn, injector in self.specs():
            clean = fn()
            if not clean.ok:
                broken += 1
                if verbose:
                    print(f"  ✗ {clean.label}: 干净动作上就没过（{clean.detail}），"
                          f"这道门已经在自己报警，鉴别力无从谈起")
                continue
            try:
                hit = fn(injector())
            except AnimInjectionImpossible as exc:
                broken += 1
                if verbose:
                    print(f"  ✗ {clean.label}: 造不出缺陷 —— {exc}")
                continue
            if hit.ok:
                broken += 1
                if verbose:
                    print(f"  ✗ {clean.label}: 注入缺陷后仍然过（{hit.detail}），**没有鉴别力**")
                continue
            if verbose:
                print(f"  ✓ {clean.label}: 干净 {clean.worst:+.3f} → 注入后 {hit.worst:+.3f}，报出")
        n = len(self.specs())
        if verbose:
            print(f"  → {n - broken}/{n} 道门有鉴别力")
        return broken
