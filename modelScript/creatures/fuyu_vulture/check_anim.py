#!/usr/bin/env python3
"""腐羽鹫 —— 动画后验。两足动画的破绽渲染静帧一律看不出来，只能算。

查六件事，每一件都对应一类"看图看不出来、进游戏一眼假"的毛病：
  1. 滑步：支撑相里趾尖必须贴地、且随支撑进度**匀速**后移（按支撑进度 u 拟合，不按
     时间 t —— 相位偏移会让支撑相跨过 t=0，按 t 拟合出来的残差是假的）。
  2. 逆解残差：目标解不出来时闭式解会夹到可达边界，脚就"够不着"地面。
  3. 穿地：逐帧全模型最低点。地面动作不该扎进地里，翼尖扫地尤其常见 —— 张翼威慑那
     一下最容易把初级飞羽插进土里。
  4. 接缝：循环动画首末帧姿态必须逐骨相等，否则每轮循环抖一下。
  5. 断点：逐骨角度的**值跳变**（细分步长后不衰减的那种），抓姿态被后一步整段覆盖、
     逆解在边界上换解支这类问题。快而连续的动作（起飞蹬地）不该被误判，见 discontinuity。
  6. 平衡（两足专属）：单支撑相里质心必须真的压到支撑脚上方。四足有静态三角支撑，没有
     这条约束，所以狮子那份后验里没有；少了它，走路是在冰面上平移。
  7. 导出保真度：把**导出的关键帧**线性插值回来与原函数对拍。前六条查的都是那条连续函
     数，可进游戏播的是采样 + 裁剪之后的结果 —— 采样数给少了照样白做。

**查不到的**：姿态本身对不对。这六条全是连续性与接触约束，一个从头到尾摆错但摆得很平
滑的动作照样满分 —— 那一层只能靠 render_anim.py 出图用眼睛看。

用法:
  python3 modelScript/creatures/fuyu_vulture/check_anim.py            # 中档全部动作
  python3 modelScript/creatures/fuyu_vulture/check_anim.py --size all
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import gen_anim as G  # noqa: E402
from bbmodel_maker.gates import animgate  # noqa: E402
from bbmodel_maker.rig.animkit import Pose, build_tracks  # noqa: E402
from rig import SIDES, VultureRig, unfold_pose  # noqa: E402

SKATE_TOL = 0.055     # 相对髋高；超过这个值肉眼就看得出脚在蹭地
# 穿地容差取**绝对值**而不是体高比例：16 单位 = 1 格 = 16 纹理像素，所以 1 单位就是一个
# 像素，0.5 单位以下看不见。这条本来是抓"整只翼尖扫地""脚陷进土里"这种以单位计的错，不
# 是抓亚像素。另一头的现实是跗跖（裸胫）在静止姿下离地只有 0.25/0.40/0.58 单位（三档），
# 步幅两端腿一斜它就沉下去零点几 —— 那是模型几何的固有余量，不是动画做错了。
SINK_TOL = 0.5
JUMP_FLOOR = 8.0      # 低于这个跳变量不值得判连续性（噪声占比太大）
RATIO_TOL = 0.45      # 细分 8 倍后仍剩这么多 = 值跳变（平滑段降到 ~0.13，折线拐点 ~0.25）
SEAM_TOL = 0.02
BALANCE_MIN = 0.45    # 单支撑相里质心至少要压到支撑脚的这个比例上
# 导出关键帧回放与原函数的最大世界位移，单位 = 纹理像素（16 单位 = 1 格 = 16 px）。
# 半个像素以下看不见；给 0.8 留一点余量，同时仍远小于"少了一截"那种量级。
FID_TOL = 0.8
# 展开后与展翼模型的逐件世界偏差上限，同样按纹理像素算（1 单位 = 1 px）
UNFOLD_TOL = 0.5


#: 质心近似：按 element 体积加权的形心。骨/羽密度当然不同，但这里只用来判「重心有没有
#: 压到支撑脚那一侧」，方向对就够，不需要真密度。判据在 core/animgate.py。
com = animgate.volume_center


def gait_report(rig: VultureRig, name: str, gait, H: float) -> list[str]:
    out = []
    for s in SIDES:
        us, ys, zs, errs = [], [], [], []
        for i in range(180):
            t = i / 180
            u = (t + gait.phases[s]) % 1.0
            if u >= gait.duty:
                continue
            pose = G.sample(rig, name, t)
            p = rig.tip_world(pose, rig.leg(s))
            tgt, _ = gait.target(s, t)
            us.append(u / gait.duty)
            ys.append(p[1])
            zs.append(p[2])
            errs.append(float(np.linalg.norm(p - tgt)))
        A = np.vstack([np.array(us), np.ones(len(us))]).T
        slope, icpt = np.linalg.lstsq(A, np.array(zs), rcond=None)[0]
        slide = float(np.abs(np.array(zs) - (slope * np.array(us) + icpt)).max())
        bad = slide > SKATE_TOL * H or max(errs) > SKATE_TOL * H
        out.append(f"    {s} 支撑: 贴地y {min(ys):+5.2f}..{max(ys):+5.2f} | 后移 {slope:5.2f}"
                   f" | 滑步 {slide / H:6.4f}H | 逆解残差 {max(errs) / H:6.4f}H"
                   f"{'   ← 超标' if bad else ''}")
    return out


def balance_report(rig: VultureRig, name: str, gait) -> str:
    """单支撑相里质心的横向位置是否站到了支撑脚那一侧。"""
    worst, best, n_bad, n = float("inf"), 0.0, 0, 0
    for i in range(72):
        t = i / 72
        stance = [s for s in SIDES if gait.stance(s, t)]
        if len(stance) != 1:
            continue
        s = stance[0]
        foot_x = gait.rest[s][0]
        cx = float(com(rig, G.sample(rig, name, t))[0])
        n += 1
        frac = cx / foot_x                      # >0 = 与支撑脚同侧，1.0 = 正压在脚上
        worst = min(worst, frac)
        best = max(best, frac)
        if frac <= 0.0:
            n_bad += 1
    worst = 0.0 if worst == float("inf") else worst
    # 只查"符号对不对"是不够的：把横移整个删掉之后，光靠骨盆侧倾也能让质心偏出零点几个
    # 百分点，同侧率仍是 100% —— 而那正是"在冰面上平移"的样子。所以还要求峰值真的压过去。
    flag = ""
    if n_bad:
        flag = "   ← 质心没压过去"
    elif best < BALANCE_MIN:
        flag = f"   ← 横移不足（峰值 {best:.2f} < {BALANCE_MIN}）"
    return (f"    平衡: 单支撑 {n} 帧，质心同侧率 {(n - n_bad) / max(n, 1):.0%}，"
            f"侧向占比 {worst:+.2f}..{best:+.2f}（1.0 = 正压在支撑脚上）{flag}")


#: 最低点及其所属骨骼。只报一个数字的话，「穿地 0.43」根本不知道该去调什么 —— 是脚陷了、
#: 翼尖扫地了、还是尾羽拖地了，改法完全不同。判据在 core/animgate.py。
lowest_bone = animgate.lowest_bone


def _rots(rig: VultureRig, name: str, t: float) -> dict[str, list[float]]:
    pose = G.sample(rig, name, t)
    return {b: list(pose[b].rot) for b in rig.order if b in pose}


def _diff(a: dict, b: dict) -> tuple[float, str]:
    """逐骨最大角度差。按 360 取模 —— euler(θ) 与 euler(θ±360) 是同一个姿态，从旋转矩阵
    解出来的角度必然会在 ±180 处翻面，那是表示法的跳变不是姿态的跳变（导出时 build_tracks
    会解缠，真正播出来是连续的）。不取模的话展翼动作会被报成 359° 的假断点。"""
    z = [0.0, 0.0, 0.0]

    def d(x: float, y: float) -> float:
        return abs((x - y + 180.0) % 360.0 - 180.0)

    return max((max(d(a.get(k, z)[i], b.get(k, z)[i]) for i in range(3)), k)
               for k in set(a) | set(b))


def discontinuity(rig: VultureRig, name: str, n: int = 60, sub: int = 8,
                  top: int = 6) -> tuple[float, str, float]:
    """区分「真断点」和「本来就快 / 有拐点」：在**同一区间**把步长细分再量一次。

    连续函数的相邻帧差与步长成正比 —— 细分 8 倍，该处的最大子步降到约 1/8。折线拐点
    （关键点排布、逆解顶到可达边界后又松开）降到约 1/4：拐点两侧各占半个区间，最陡的那
    个子步是"整段变化量 ÷ 4"。只有真正的**值跳变**（姿态被后一步整段覆盖、分支条件在两
    帧之间翻面）不随步长缩小，细分比停在 1 附近。

    细分倍数不能只取 2：拐点在 2 倍细分下的比值恰好是 0.5，和真跳变的判据挤在一起，分
    不开（实测落地那一处拐点报 0.52、起飞那处报 0.68，全被误判成断点）。

    只卡"每帧不得超过 N 度"分不开这三者：起飞蹬地那一下本就上千度每秒该放行，落地那次
    同样量级的跳却是姿态被覆盖，该抓。比的也必须是**同一个区间** —— 拿全局最大值除全局
    最大值会串到别处去（起飞在 n 与 2n 下的最大值根本不在一个时刻）。
    """
    ts = [i / n for i in range(n + 1)]
    rots = [_rots(rig, name, t) for t in ts]
    steps = sorted(((_diff(rots[i], rots[i + 1]), i) for i in range(n)), reverse=True)
    worst = (steps[0][0][0], steps[0][0][1], 0.0) if steps else (0.0, "-", 0.0)
    for (val, who), i in steps[:top]:
        if val < JUMP_FLOOR:
            break
        fine = [rots[i]] + [_rots(rig, name, ts[i] + (ts[i + 1] - ts[i]) * j / sub)
                            for j in range(1, sub)] + [rots[i + 1]]
        step = max(_diff(fine[j], fine[j + 1])[0] for j in range(sub))
        if step / val > worst[2]:
            worst = (val, who, step / val)
    return worst


_HULL: dict[str, dict[str, np.ndarray]] = {}


def _hull(rig: VultureRig) -> dict[str, np.ndarray]:
    """每骨取自己几何的包围盒八角 —— 位移量的代表点。

    不用全部角点：整只六百个方块、四千八百个角，对拍两百帧就是千万级点乘。包围盒八角
    已经覆盖了该骨能偏出去的最远处，而这里要的正是最远处。
    """
    key = str(rig.path)
    if key not in _HULL:
        out = {}
        for n in rig.order:
            pts = rig.bone_points(n)
            if not len(pts):
                continue
            lo, hi = pts.min(axis=0), pts.max(axis=0)
            out[n] = np.array([[x, y, z] for x in (lo[0], hi[0])
                               for y in (lo[1], hi[1]) for z in (lo[2], hi[2])])
        _HULL[key] = out
    return _HULL[key]


def _pose_from_tracks(tracks: dict, tt: float) -> Pose:
    return animgate.pose_from_tracks(tracks, tt, Pose)


def fidelity(rig: VultureRig, name: str, fine: int = 193) -> tuple[float, str]:
    """导出保真度：拿**导出的关键帧**线性插值回来重建姿态，量与原函数的最大世界位移。

    前面几条查的都是那条连续函数本身，可进游戏播的是采样 + 裁剪之后的关键帧。采样给疏
    了、裁剪给狠了，函数再完美也白搭 —— 这一条把这两件事一起兜住。

    判据必须是**位移**不是角度：3° 落在趾骨上是零点几个像素，落在整条翼上就是小半个身
    位。早先按角度卡 2.5°，走路的跗跖被判超标（实际偏差 0.3 个纹理像素，看不见），而
    翼根上同样 2° 的漂移反倒放行了。单位就是纹理像素（16 单位 = 1 格 = 16 px）。

    fine 取质数：与采样数有公因子时探针会成片落在关键帧上（误差恰好为 0）或恰好落在
    两帧正中（误差最大），量出来的曲线随 n 上下横跳，看着像"加密反而更差"。
    """
    clip = G.ANIMS[name]
    tracks = build_tracks(rig, lambda t: G.sample(rig, name, t), clip.length, clip.loop,
                          clip.samples, clip.at)
    hull = _hull(rig)
    worst, who = 0.0, "-"
    for i in range(fine):
        t01 = i / (fine - 1)
        Wa = rig.world(G.sample(rig, name, t01))
        Wb = rig.world(_pose_from_tracks(tracks, t01 * clip.length))
        for n, pts in hull.items():
            a = pts @ Wa[n][:3, :3].T + Wa[n][:3, 3]
            b = pts @ Wb[n][:3, :3].T + Wb[n][:3, 3]
            e = float(np.abs(a - b).max())
            if e > worst:
                worst, who = e, n
    return worst, who


def seam(rig: VultureRig, name: str) -> float:
    a, b = G.sample(rig, name, 0.0), G.sample(rig, name, 1.0)
    d = 0.0
    for bone in rig.order:
        for attr in ("rot", "pos", "scale"):
            if bone not in a or bone not in b:
                continue
            d = max(d, max(abs(x - y) for x, y in zip(getattr(a[bone], attr), getattr(b[bone], attr))))
    return d


def unfold_report(folded: VultureRig, spread: VultureRig) -> tuple[str, bool]:
    """展翼姿态对拍：把 unfold_pose 施加到收翼绑定姿，逐件比世界角点。

    这是"展翼动画到底对不对"的唯一硬判据 —— 姿态是从两份模型解出来的，解错了图上未必
    看得出（羽根差半个单位、某一组羽数对不上，都只表现为一道细缝）。名字集合也一起查：
    收翼绑定姿是展开动画的**起点**，起点少一根羽，展开后那儿就是个洞。
    """
    fn = {e["name"] for e in folded.elements.values()}
    sn = {e["name"] for e in spread.elements.values()}
    if fn != sn:
        return (f"    展翼对拍: 两姿元素集合不同（收翼多 {len(fn - sn)}、展翼多 {len(sn - fn)}）"
                f"   ← 展开后会缺件"), True
    pose = unfold_pose(folded, spread)
    Wf, Ws = folded.world(pose), spread.world()
    sbone = {spread.elements[v]["name"]: b for b in spread.order for v in spread.bones[b].elements}
    worst, who = 0.0, "-"
    for name in folded.order:
        for u in folded.bones[name].elements:
            e = folded.elements[u]
            sb = sbone[e["name"]]
            se = next(spread.elements[v] for v in spread.bones[sb].elements
                      if spread.elements[v]["name"] == e["name"])
            a = folded.corners(e) @ Wf[name][:3, :3].T + Wf[name][:3, 3]
            b = spread.corners(se) @ Ws[sb][:3, :3].T + Ws[sb][:3, 3]
            d = float(np.abs(np.sort(a, axis=0) - np.sort(b, axis=0)).max())
            if d > worst:
                worst, who = d, e["name"]
    bad = worst > UNFOLD_TOL
    return (f"    展翼对拍: 动骨 {len(pose)}，逐件世界偏差 {worst:.3f}px [{who}]"
            f"{'   ← 展开后与展翼模型对不上' if bad else ''}"), bad


def run(size: str, only: list[str] | None) -> int:
    bad = 0
    rigs: dict[str, VultureRig] = {}
    names = only or list(G.ANIMS)
    print(f"=== {size}")
    for name in names:
        clip = G.ANIMS[name]
        if clip.kind not in rigs:
            rigs[clip.kind] = G.default_rig(size, "jin", spread=clip.kind == "flight")
        rig = rigs[clip.kind]
        k = G.K(rig)
        lows = [lowest_bone(rig, G.sample(rig, name, i / 60)) for i in range(60)]
        jump, jb, ratio = discontinuity(rig, name)
        (deep, who), (air, _) = min(lows), max(lows)
        line = (f"  {name:<8} {clip.length:4.2f}s {'循环' if clip.loop else '单次'} "
                f"[{clip.kind:<6}] | 最低点 {deep:+6.2f} [{who}] (最高帧 {air:+6.2f})"
                f" | 跳变 {jump:5.1f}° [{jb}] 细分比 {ratio:.2f}")
        if clip.loop:
            sm = seam(rig, name)
            line += f" | 接缝 {sm:.3f}"
            if sm > SEAM_TOL:
                line += " ←接缝超标"
                bad += 1
        # 穿地：飞行动作整只在空中，只对着地的那些帧有意义
        if clip.kind == "ground" and deep < -SINK_TOL:
            line += "  ← 穿地"
            bad += 1
        if jump > JUMP_FLOOR and ratio > RATIO_TOL:
            line += "  ← 断点（细分不衰减 = 姿态被跳掉，不是快动作）"
            bad += 1
        fd, fw = fidelity(rig, name)
        line += f" | 导出位移偏差 {fd:4.2f}px [{fw}]"
        if fd > FID_TOL:
            line += "  ← 采样太疏或裁剪太狠"
            bad += 1
        print(line)
        gait = {"walk": k.walk, "run": k.run}.get(name)
        if gait is not None:
            for row in gait_report(rig, name, gait, k.H):
                print(row)
                if "超标" in row:
                    bad += 1
            row = balance_report(rig, name, gait)
            print(row)
            if "←" in row:
                bad += 1
    if "ground" in rigs and (only is None or "unfold" in names or "fold" in names):
        row, flag = unfold_report(rigs["ground"], G.default_rig(size, "jin", spread=True))
        print(row)
        bad += 1 if flag else 0
    return bad


def main() -> int:
    ap = argparse.ArgumentParser(description="腐羽鹫动画后验")
    ap.add_argument("--size", default="mid", choices=(*G.SIZES, "all"))
    ap.add_argument("--only", nargs="*")
    args = ap.parse_args()
    sizes = G.SIZES if args.size == "all" else (args.size,)
    bad = sum(run(s, args.only) for s in sizes)
    print("  ✓ 全部通过" if not bad else f"  ✗ {bad} 项超标")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
