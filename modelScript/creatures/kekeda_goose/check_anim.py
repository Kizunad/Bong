#!/usr/bin/env python3
"""珂珂达 —— 动画后验。这些破绽渲染静帧一律看不出来，只能算。

**查的是导出的关键帧，不是解析采样器。** 引擎播的是关键帧之间的线性插值；解析式再
光滑，采样数不够的话中间那一段照样塌。所以这里先 build_tracks 出真正要写进文件的
关键帧，再把它们插值回姿态来查 —— 查的和玩家看到的是同一个东西。

九项，前六项是物理，后三项是**看起来对不对** —— 后三项全是先在渲图里栽了跟头才补的：

  1. **滑步**  支撑相里脚掌必须贴地、且随支撑进度**匀速**后移。按支撑进度 u 拟合，
     不按时间 t —— 相位偏移会让支撑相跨过 t=0，按 t 拟合出来的残差是假的。
  2. **穿地**  逐帧全模型最低点。
  3. **平衡**  两足动画的头号判据：质心必须落在着地蹼板围出的支撑区里。横向与矢状向
     分开判（判据不同，见 sweep）；没有双支撑段的步态（小跑）只报不判。
  4. **伸展**  两连杆伸到全长的百分之多少。超过 ~0.94 就进 acos 奇异区，逆解对目标的
     微小变化爆炸式响应 —— 所有"落脚咔一下"的抽搐都能追到这个数上。
  5. **抽搐**  逐骨最大**角速度**（度/秒，不是度/帧 —— 后者随采样数变）。
  6. **接缝 / 采样保真**  循环首末帧要逐骨相等；关键帧插值与解析姿态之间几何点最远
     跑了多少（模型单位）。后者超标 = ANIMS 里那个采样数给少了，动作峰值被插值削平。
  7. **出口通畅**  拉粑粑 / 下蛋的释放帧，泄殖腔口正下方不能有自己的身体 —— 否则
     掉出来的东西是从躯干里冒出来的。这一条是这两个动画存在的理由，必须实测。
  8. **剪影**  喙尖 / 尾尖的活动范围，以及头与躯干之间**还看不看得见脖子**。首轮的
     拉粑粑和下蛋物理指标一项不差，渲出来中段却没有脖子 —— 头缩进肩里，鹅最好认的
     特征消失，整只读成一团白。
  9. **接缝断裂**  颈分片之间、下喙与头之间的缝隙。一整块几何跟着单根骨走，骨一转它
     就同时脱开两头 —— 威吓那帧的颈和张嘴时的下喙都栽过，渲出来是方块悬在空中。
"""

from __future__ import annotations

import bisect
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import gen_anim as G  # noqa: E402
from bbmodel_maker.gates import animgate  # noqa: E402
from bbmodel_maker.rig.anim_rig import Pose, build_tracks, wrap  # noqa: E402
from rig import SIDES, Goose, Waddle, leg_chain  # noqa: E402

GAITS = {"walk": G.WALK, "run": G.TROT}
GROUND_TOL = 0.06        # 允许的穿地深度
SKATE_TOL = 0.30         # 支撑相里偏离匀速直线的最大量
#: 单骨最大角速度（度/秒）。**不能用「度/帧」**：同一个动作，采样数一加每帧的度数就
#: 变小，那个数量在量的是采样密度而不是动作快慢（密度另有「采样保真」那项管）。
#: 900°/s 已经是"一眨眼就到位"的量级，再快在 20fps 的渲染下读成瞬移。
JUMP_TOL = 900.0
SEAM_TOL = 0.02
REACH_TOL = 0.94        # 两连杆伸展比上限，再高 acos 就进奇异区
#: 头与躯干的最小间隙。绑定姿是 0.95，取它的一半 —— 0.45 单位 ≈ 2.8 厘米，在一米长
#: 的鹅身上刚够看出「头和身子是分开的」。
NECK_GAP_TOL = 0.45
#: 允许低于上限的时长占比。短暂缩一下是节拍（受击缩脖本来就对），一直缩着才是毛病。
NECK_DWELL_TOL = 0.15
#: 接缝允许的最大缝隙。0 = 刚好贴合；留一点余量给渲染时的抗锯齿。
LINK_BREAK_TOL = 0.05
FIDELITY_TOL = 0.30      # 插值 vs 解析的最大**位移**偏差（模型单位，16 = 1 米）


class Exported:
    """把 build_tracks 的产物插值回 Pose —— 引擎实际播放的那条曲线。"""

    def __init__(self, g: Goose, name: str):
        self.length, self.loop, self.n, _ = G.ANIMS[name]
        self.name = name
        self.tracks = build_tracks(g, lambda t: G.sample(g, name, t), self.length, self.loop, self.n)
        self.times = {b: {c: [tt for tt, _ in v] for c, v in ch.items()}
                      for b, ch in self.tracks.items()}

    def at(self, t01: float) -> Pose:
        t = t01 * self.length
        p = Pose()
        for bone, chans in self.tracks.items():
            for chan, vals in chans.items():
                ts = self.times[bone][chan]
                i = bisect.bisect_right(ts, t) - 1
                if i < 0:
                    v = vals[0][1]
                elif i >= len(vals) - 1:
                    v = vals[-1][1]
                else:
                    (t0, v0), (t1, v1) = vals[i], vals[i + 1]
                    a = (t - t0) / (t1 - t0) if t1 > t0 else 0.0
                    v = [x + (y - x) * a for x, y in zip(v0, v1)]
                attr = {"rotation": "rot", "position": "pos", "scale": "scale"}[chan]
                setattr(p[bone], attr, list(v))
        return p


def contact_feet(g: Goose, pose, tol: float = 0.12):
    """这一帧真正踩在地上的蹼板范围 [(x0,x1),(z0,z1)]；没有脚着地返回 None。

    tol 要小：抬到 0.3 的脚是不承重的，把它算进支撑区会让支撑多边形凭空变宽，
    平衡余量就成了假的（实测 0.35 时小跑的横向余量被虚报成正数）。
    """
    W = g.world(pose)
    xs, zs = [], []
    for s in SIDES:
        n = f"foot_{s}"
        pts = g.bone_points(n) @ W[n][:3, :3].T + W[n][:3, 3]
        sole = pts[pts[:, 1] <= tol]
        if len(sole):
            xs += [float(sole[:, 0].min()), float(sole[:, 0].max())]
            zs += [float(sole[:, 2].min()), float(sole[:, 2].max())]
    if not xs:
        return None
    return (min(xs), max(xs)), (min(zs), max(zs))


def sweep(g: Goose, ex: Exported, sub: int = 4):
    """逐帧扫一遍导出曲线。sub = 每个关键帧区间再插 sub 个点，抓插值塌陷。"""
    n = ex.n * sub
    lows, mx, mz, reaches, air = [], [], [], [], 0
    for i in range(n):
        pose = ex.at(i / n)
        lows.append(g.lowest(pose))
        reaches.append(g.reach(pose))
        box = contact_feet(g, pose)
        if box is None:
            air += 1
        else:
            (x0, x1), (z0, z1) = box
            c = g.mass_center(pose)
            # 横向和矢状向分开报：**两者的判据不一样**。横向失衡没有救 —— 鹅没有第三
            # 只脚，所以摇摆步的横向余量必须恒为正（这正是它摇摆的原因）。矢状向则是
            # 动态的：走路本来就是"往前倒、下一只脚接住"，质心走到支撑脚前面完全正常，
            # 只有原地不动的动作才要求它留在蹼板范围内。
            mx.append(float(min(c[0] - x0, x1 - c[0])))
            mz.append(float(min(c[2] - z0, z1 - c[2])))
    return lows, mx, mz, max(reaches), air


def key_jump(g: Goose, ex: Exported) -> tuple[float, str]:
    """最大单骨角速度（度/秒），取自相邻关键帧。"""
    worst = (0.0, "-")
    for bone, chans in ex.tracks.items():
        for chan, vals in chans.items():
            if chan != "rotation":
                continue
            for (ta, a), (tb, b) in zip(vals, vals[1:]):
                if tb <= ta:
                    continue
                d = max(abs(x - y) for x, y in zip(a, b)) / (tb - ta)
                if d > worst[0]:
                    worst = (d, bone)
    return worst


def fidelity(g: Goose, ex: Exported, name: str, sub: int = 4) -> tuple[float, str]:
    """关键帧插值与解析姿态的最大偏差，量的是**几何点跑了多远**（模型单位）。

    不量角度：同样 3°，长在踝上只让蹼板挪 9 毫米（鹅身尺度），长在颈根上却能把头
    甩出去半个身位 —— 用度数当判据就等于把这两件事看成一样严重。位移才是看得见的
    那个量，0.30 单位 ≈ 1.9 厘米。
    """
    worst = (0.0, "-")
    for i in range(ex.n * sub):
        t = i / (ex.n * sub)
        Wa, Wb = g.world(ex.at(t)), g.world(G.sample(g, name, t))
        for bone in g.order:
            pts = g.bone_points(bone)
            if not len(pts):
                continue
            pa = pts @ Wa[bone][:3, :3].T + Wa[bone][:3, 3]
            pb = pts @ Wb[bone][:3, :3].T + Wb[bone][:3, 3]
            d = float(np.abs(pa - pb).max())
            if d > worst[0]:
                worst = (d, bone)
    return worst


def _cluster(g: Goose, bones, W):
    """一组骨的逐件世界包围盒。"""
    out = []
    for n in bones:
        M = W[n]
        for u in g.bones[n].elements:
            e = g.elements.get(u)
            if e is None:
                continue
            wp = g.corners(e) @ M[:3, :3].T + M[:3, 3]
            out.append((wp.min(axis=0), wp.max(axis=0)))
    return out


def head_clearance(g: Goose, pose) -> float:
    """头这一坨与躯干这一坨的最近间隙（负 = 已经嵌进去了）。

    **不能量"头顶到背面的竖直净空"**：威吓姿的头是往**前**伸的，压根不在身体上方，
    竖直口径量出来是 −1.5，判成"没脖子"，其实那一帧脖子伸得最长。间隙要按三轴分离
    量：两个轴对齐盒沿某轴分开多少，取各轴的最大值（>0 即不相交），再对所有件取最小。
    """
    W = g.world(pose)
    head = _cluster(g, ("skull", "bill_upper", "jaw"), W)
    body = _cluster(g, ("plume_body",), W)
    return animgate.min_gap(head, body)


def _boxes(g: Goose, names, W):
    """按 element 名取世界包围盒。"""
    out = []
    for bone in g.order:
        M = W[bone]
        for u in g.bones[bone].elements:
            e = g.elements.get(u)
            if e is not None and e.get("name") in names:
                wp = g.corners(e) @ M[:3, :3].T + M[:3, 3]
                out.append((wp.min(axis=0), wp.max(axis=0)))
    return out


#: 两组盒之间的最小分离（负 = 有重叠）。判据在 core/animgate.py，那里连
#: 「为什么必须按三轴分离量、不能量竖直净空」的理由一起收着。
_gap = animgate.min_gap


#: 会被姿态拉开的那些接缝：每条链逐段必须首尾相接。
#: 一整块几何跟着单根骨走，骨一转它就同时脱开两头 —— 这类破绽只有渲图看得见，
#: 所以逐段量包围盒的分离，取全动画最坏值。
LINK_CHAINS = {
    # 躯干 → 四片颈 → 头。颈全伸时弦长 +60%，片与片会拉开
    "颈": (
        {"body_core", "body_back", "body_breast"},
        {"neck_0"}, {"neck_1"}, {"neck_2"}, {"neck_3"},
        {"head", "crown"},
    ),
    # 头/上喙 → 下喙。jaw 的 pivot 比喙的实际铰链靠后 2.5 单位，张嘴大了整根会被拽出来
    "喙": ({"head", "bill", "bill_nail"}, {"bill_lower"}),
}


def link_break(g: Goose, ex: Exported, n: int = 60) -> tuple[float, float, str]:
    """所有接缝里最宽的那道（>0 = 断开），以及它出现在哪一刻、属于哪条链。"""
    worst, at, who = -9.0, 0.0, "-"
    for i in range(n):
        t = i / n
        W = g.world(ex.at(t))
        for label, chain in LINK_CHAINS.items():
            groups = [_boxes(g, names, W) for names in chain]
            for a, b in zip(groups, groups[1:]):
                if a and b:
                    d = _gap(a, b)
                    if d > worst:
                        worst, at, who = d, t, label
    return worst, at, who


def silhouette(g: Goose, ex: Exported, n: int = 60):
    """剪影量表：喙尖高度 / 前伸、尾尖高度、**头与躯干之间还看不看得见脖子**。

    数值全过不等于动画好看。首轮渲出来的拉粑粑和下蛋，物理指标一项不差，但中段
    "脖子没了" —— 头缩进肩里，鹅最好认的那个特征在动画里消失，整只读成一团。
    所以把剪影也变成能量的数。
    """
    bill_y, bill_z, tail_y, gap = [], [], [], []
    for i in range(n):
        p = ex.at(i / n)
        W = g.world(p)
        b = g.bill_tip(p)
        bill_y.append(float(b[1]))
        bill_z.append(float(b[2]))
        tp = g.bone_points("tail_base") @ W["tail_base"][:3, :3].T + W["tail_base"][:3, 3]
        tail_y.append(float(tp[:, 1].max()))
        gap.append(head_clearance(g, p))
    # 只报最小值会把"缩一下"和"整段没脖子"判成一样严重 —— 前者是节拍（受击缩脖是对的），
    # 后者才是毛病。所以同时报**低于阈值的时长占比**，判据看它。
    dwell = sum(v < NECK_GAP_TOL for v in gap) / len(gap)
    return (min(bill_y), max(bill_y)), (min(bill_z), max(bill_z)), \
        (min(tail_y), max(tail_y)), min(gap), dwell


def seam(ex: Exported, g: Goose) -> float:
    return animgate.gate_seam(ex.at, g.order, SEAM_TOL).worst


def gait_report(g: Goose, ex: Exported, cfg: dict, n: int = 160) -> list[str]:
    """支撑相诊断：贴地 / 匀速后移 / 逆解残差（走的是导出曲线）。"""
    w = Waddle(g, **cfg)
    out = []
    for s in SIDES:
        us, ys, zs, errs = [], [], [], []
        for i in range(n):
            t = i / n
            u = wrap(t + w.phases[s])
            if u >= w.duty:
                continue
            p = g.limb_tip(ex.at(t), leg_chain(s))
            tgt, _ = w.target(s, t)
            us.append(u / w.duty)
            ys.append(float(p[1]))
            zs.append(float(p[2]))
            errs.append(float(np.linalg.norm(p - tgt)))
        A = np.vstack([np.array(us), np.ones(len(us))]).T
        slope, icpt = np.linalg.lstsq(A, np.array(zs), rcond=None)[0]
        slide = float(np.abs(np.array(zs) - (slope * np.array(us) + icpt)).max())
        bad = "   ← 超标" if (slide > SKATE_TOL or max(errs) > SKATE_TOL) else ""
        out.append(f"    {s} 脚：贴地 y {min(ys):+5.2f}..{max(ys):+5.2f} | 后移 "
                   f"{slope:+5.2f} 单位/支撑相 | 滑步 {slide:5.3f} | IK残差 {max(errs):5.3f}{bad}")
    return out


def release_report(g: Goose, ex: Exported, name: str) -> list[str]:
    t = G.RELEASE[name]
    pose = ex.at(t)
    v = g.vent(pose)
    blocked = g.blocked_below(v, pose)
    out = [f"    释放帧 t={t:.2f}：出口 ({v[0]:+.2f}, {v[1]:+.2f}, {v[2]:+.2f})"
           f" 离地 {v[1] - g.lowest(pose):.2f} · 尾抬 {-pose['tail_base'].rot[0]:.0f}°"]
    out.append("    正下方：通畅 ✓" if not blocked
               else f"    正下方被挡：{', '.join(sorted(set(blocked)))}   ← 会从身体里冒出来")
    return out


def main() -> int:
    g = Goose()
    z0, z1 = g.support_z()
    print(f"支撑区 z {z0:+.2f}..{z1:+.2f} · 脚距 ±{abs(g.rest_feet()['r'][0]):.2f} · "
          f"静止质心高 {g.rest_com()[1]:.2f}\n查的是导出关键帧的插值结果（引擎实际播的曲线）")
    bad = 0
    for name, (length, loop, nsamp, _fn) in G.ANIMS.items():
        ex = Exported(g, name)
        lows, mx, mz, reach, air = sweep(g, ex)
        jump, jb = key_jump(g, ex)
        fid, fb = fidelity(g, ex, name)
        deep = min(lows)
        wx, wz = (min(mx), min(mz)) if mx else (None, None)

        line = f"\n{name:<8} {length:4.2f}s×{nsamp}帧 {'循环' if loop else '单次'}"
        line += f" | 最低点 {deep:+5.2f}"
        line += f" | 余量 横{wx:+5.2f} 矢{wz:+5.2f}" if wx is not None else " | 余量  ————————  "
        line += f" | 伸展 {reach:5.1%}"
        line += f" | 关键帧跳变 {jump:5.1f}° [{jb}]"
        line += f" | 采样保真 {fid:4.2f}u [{fb}]"
        if air:
            line += f" | 腾空 {air}/{len(lows)} 帧"
        if loop:
            line += f" | 接缝 {seam(ex, g):.3f}"
        print(line)
        (by0, by1), (bz0, bz1), (ty0, ty1), neck_gap, dwell = silhouette(g, ex)
        nb, nbt, nbw = link_break(g, ex)
        print(f"    剪影：喙尖 y {by0:5.1f}..{by1:5.1f} z {bz0:6.1f}..{bz1:6.1f}"
              f" · 尾尖 y {ty0:5.1f}..{ty1:5.1f} · 头身间隙最小 {neck_gap:+.2f}"
              f"（贴身时长 {dwell:.0%}）· 最宽接缝 {nb:+.2f}[{nbw}]@t={nbt:.2f}")

        probs = []
        if deep < -GROUND_TOL and name != "death":
            probs.append(f"穿地 {deep:.2f}（限 −{GROUND_TOL}）")
        # 静态平衡只对**真有双支撑**的动作较真。duty ≤ 0.5 的步态（小跑）每一瞬都只有
        # 一只脚、甚至没有脚着地，质心从一只脚荡到另一只脚的途中必然越过支撑区 —— 那
        # 正是"跑"的定义（每步都在摔、下一只脚接住），拿静态判据卡它是判据用错了地方。
        dynamic = air > 0 or (name in GAITS and GAITS[name]["duty"] <= 0.5)
        if wx is not None and wx < 0 and not dynamic and name != "death":
            probs.append(f"质心横向跑出支撑脚 {wx:.2f} —— 没有第三只脚能救，这一帧必倒")
        # 矢状向只对**原地不动**的动作较真：有步态时"往前倒、下一只脚接住"是正常的
        if wz is not None and wz < 0 and name not in GAITS and name != "death":
            probs.append(f"质心矢状向跑出蹼板 {wz:.2f} —— 原地动作会前扑/后坐")
        # 倒地不查伸展：腿离地后由 curl 接管，逆解只在最初那一小段有效，
        # 满伸只出现在两者的混合区，不影响成品姿态
        if reach > REACH_TOL and name != "death":
            probs.append(f"两连杆伸到 {reach:.1%}（限 {REACH_TOL:.0%}）—— 进 acos 奇异区，"
                         f"逆解会对目标的微动爆炸式响应；压低站高或收窄前伸")
        if jump > JUMP_TOL:
            probs.append(f"{jb} 关键帧间跳 {jump:.1f}°（限 {JUMP_TOL}）")
        if fid > FIDELITY_TOL:
            probs.append(f"{fb} 插值位移偏差 {fid:.2f} 单位（限 {FIDELITY_TOL}）—— 采样数 {nsamp} 不够")
        if loop and seam(ex, g) > SEAM_TOL:
            probs.append(f"循环接缝 {seam(ex, g):.3f}（限 {SEAM_TOL}）")
        if name in GAITS:
            for row in gait_report(g, ex, GAITS[name]):
                print(row)
                if "超标" in row:
                    probs.append(row.strip())
        if name in G.RELEASE:
            for row in release_report(g, ex, name):
                print(row)
                if "被挡" in row:
                    probs.append(row.strip())
        if nb > LINK_BREAK_TOL:
            probs.append(f"{nbw}在 t={nbt:.2f} 裂开 {nb:.2f} —— 几何脱节，"
                         f"渲出来是方块悬在空中")
        if dwell > NECK_DWELL_TOL and name != "death":
            probs.append(f"有 {dwell:.0%} 的时长头身间隙不足 {NECK_GAP_TOL}"
                         f"（最小 {neck_gap:.2f}）—— 这段剪影读不出这是只鹅")
        for msg in probs:
            print(f"    ✗ {msg}")
        bad += len(probs)

    print(f"\n{'✓ 全部通过' if not bad else f'✗ {bad} 处违例'}")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
