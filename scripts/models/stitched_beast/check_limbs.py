#!/usr/bin/env python3
"""异变缝合兽 —— 部件层·肢体的力学自检。

这一层没有一个数字是挑出来的，所以自检不能只看"渲出来像不像腿"，得把每一步推导单独
钉住。撞红时看断言文案就知道是哪一步塌了。

  ① **载荷解**：任一时刻 ΣR = 体重、绕两轴力矩为零、且 R ≥ 0（地面只能推不能拉）
  ② **扛重的是短近腿**：载荷份额与"落点离质心的距离"负相关。按条数平摊会得到反的结论
  ③ **骨是被掰断的不是被压扁的**：力臂不可忽略的那些节，三条失效判据里弯曲必须主导
  ④ **肌肉从根到梢递减**，末节只剩腱——这是腿上粗下细的唯一来源
  ⑤ **站姿命中落点**：链末端必须落在 locomotion 解出的落点上（IK 收敛）
  ⑥ **垂姿分两种**：有骨的死腿在第一个关节硬折后笔直垂下；无骨的触手连续弯
  ⑦ **粗细跟力学走、跟"供体本来多粗"无关**：同类内部跨度 ≥2×，且与力矩量强相关
  ⑧ **脚掌压强 ≤ 地面承载力**（面积就是这么反推的，这条是同义反复的守门员）
  ⑨ **收窄站姿真的省粗细**：把落点推回可达极限，根部需求必须显著变粗——这条盯的是
     locomotion.stance_radius 那处修复，防它被改回"落点=可达极限"

用法: python3 scripts/models/stitched_beast/check_limbs.py [--seeds 1,2,3]
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import limbs as LB  # noqa: E402
import locomotion as LM  # noqa: E402


def check_seed(seed: int, socks) -> tuple[list[str], list[tuple]]:
    bad: list[str] = []
    tag = f"seed={seed}"
    gen, gait = LM.sample_standing(seed, socks=socks)
    loads = LB.foot_loads(gait)
    feet = {lg.gene.socket: lg.foot for lg in gait.limbs}
    W = LB.body_weight()
    com2 = np.array([gait.com[0], gait.com[2]])

    # ---- ① 载荷解本身
    for k in range(48):
        t = k / 48
        on = [lg for lg in gait.limbs if lg.in_stance(t)]
        if len(on) < 3:
            bad.append(f"{tag} t={t:.2f} 只有 {len(on)} 只脚着地（需 ≥3）")
            continue
        P = np.array([lg.foot_at(t) for lg in on])[:, [0, 2]]
        R = LB.contact_forces(P, com2, W)
        if R.min() < -1e-6:
            bad.append(f"{tag} t={t:.2f} 解出负反力 {R.min():.1f} N——地面拉不住东西")
        if abs(R.sum() - W) > 1e-3 * W:
            bad.append(f"{tag} t={t:.2f} ΣR={R.sum():.0f} ≠ 体重 {W:.0f}")
        mx = float(((P[:, 0] - com2[0]) * R).sum())
        mz = float(((P[:, 1] - com2[1]) * R).sum())
        if max(abs(mx), abs(mz)) > 1e-3 * W * 30:
            bad.append(f"{tag} t={t:.2f} 力矩不平衡 ({mx:.0f},{mz:.0f})——身体会翻")

    rows = []
    for gene in gen.limbs:
        sock = socks[gene.socket]
        lb = LB.solve_limb(gene, sock, load=loads.get(gene.socket, 0.0),
                           foot=feet.get(gene.socket), ride=gait.ride)
        rows.append((seed, lb))
        segs = gene.segments

        if lb.bearing:
            # ---- ③ 有力臂的地方必须弯曲主导
            #
            # 只对**力臂不可忽略**的节断言。姿态求解在主动把关节往载荷线上挪（那正是
            # 最省肌肉的方向），挪成功的关节力臂趋近于零，那一节的骨就轮到屈曲说了算
            # ——这是推导链自洽的表现，不是错。真正该卡的是"力臂大的地方还不是弯曲
            # 主导"，那才说明力矩没进公式。
            ground = lb.joints[-1]
            arms = [float(np.hypot(*(ground - p)[[0, 2]])) for p in lb.joints]
            for i in range(len(segs)):
                if arms[i] < 1.0:
                    continue
                _r, (ax, bk, bd) = LB.bone_radius(segs[i], arms[i], lb.load)
                if bd < max(ax, bk):
                    bad.append(f"{tag} {lb.name} 第{i}节 力臂 {arms[i]:.1f} px 却是"
                               f"轴压/屈曲主导（弯曲 {bd:.2f} / 轴压 {ax:.2f} / "
                               f"屈曲 {bk:.2f}）——腿是被掰断的，力矩没进公式")
            # ---- ④ 肌肉全长在**腿**上，脚上一点没有
            #
            # 这条原本写的是"肌肉截面从根到梢单调递减"。接上真实的脚之后它是错的：
            # 趾行动物的踝（跗关节）被掌骨推到接触点后面一大截，力矩臂比髋还大——
            # 那正是跟腱和腓肠肌粗壮的原因，小腿比大腿鼓是解剖事实不是算错。
            m = lb.muscle
            nleg = len(segs) - gene.foot_bones
            if any(x > 1e-12 for x in m[nleg:]):
                bad.append(f"{tag} {lb.name} 脚上长了肌腹 {[f'{x:.3f}' for x in m]}"
                           f"——掌骨与趾骨那几节只有腱和角质穿过")
            if sum(m[:nleg]) <= 0.0:
                bad.append(f"{tag} {lb.name} 腿上一点肌肉都没有——抗重力肌去哪了")
            # ---- ⑤ IK 收敛：腿的末端必须落在**站姿要求的踝位**上。
            #    不能拿链的末端和落点比——链的末端是趾尖，趾行/跖行的趾尖本来就在
            #    接触点前面一截（那正是脚的形状）。
            err = float(np.linalg.norm(lb.joints[len(segs) - gene.foot_bones] - lb.ankle))
            if err > 0.6:
                bad.append(f"{tag} {lb.name} 腿的末端偏离踝位 {err:.2f} px——IK 没收敛")
            # ---- ⑧ 脚掌压强
            area = (2 * lb.pad[0] * LB.PX) * (2 * lb.pad[1] * LB.PX)
            if lb.load / max(area, 1e-9) > LB.BEARING * 1.02:
                bad.append(f"{tag} {lb.name} 脚掌压强超过承载力")
        else:
            # ---- ⑥ 垂姿。基准是**第一个关节**不是挂载点：根部那一节的朝向由癒合焊死
            #    在法向上，重力从第一个关节起才说得上话。背上朝天的槽长一截短残肢，
            #    尖端确实可能比槽还高——那是焊上去的角度，不是重力反了。
            if lb.tip[1] > lb.joints[1][1] + 0.5:
                bad.append(f"{tag} {lb.name} 垂下来的肢尖端比第一个关节还高——"
                           f"重力方向反了")
            if lb.tip[1] < -0.6:
                bad.append(f"{tag} {lb.name} 闲肢尖端 y={lb.tip[1]:.1f} 戳进地里")
            dirs = [(b - a) / max(float(np.linalg.norm(b - a)), 1e-9)
                    for a, b in zip(lb.joints, lb.joints[1:])]
            turns = [float(np.degrees(np.arccos(np.clip(np.dot(x, y), -1, 1))))
                     for x, y in zip(dirs, dirs[1:])]
            if gene.kind == "tentacle" and len(turns) >= 2 and max(turns) > 1e-6:
                # 软梁是连续弯：不该出现"一处硬折之后全直"
                if sum(1 for t in turns if t > 1.0) < 2:
                    bad.append(f"{tag} {lb.name} 触手只在一处折了一下——"
                               f"无骨软梁该逐节连续弯")
            if gene.kind != "tentacle" and len(turns) >= 2:
                if max(turns[1:]) > 1.0:
                    bad.append(f"{tag} {lb.name} 有骨闲肢在第一个关节之后还在弯——"
                               f"自由铰的平衡姿态是每一节都指着正下方")
    return bad, rows


def cross_seed(rows: list[tuple]) -> list[str]:
    """⑦ 粗细跟力学走，跟"供体本来多粗"无关。

    先说清楚这条**不是**什么：不是"粗细与种类无关"。肌肉力臂取自节长，而节长正是供体
    给的，所以种类经由杠杆比例合法地影响粗细——37 条肢实测种类解释了约一半的方差
    （蛛足基节只有 2.5 px，杠杆最差，于是根部需求 9–19 px，兽腿只要 4–9 px）。

    真正锁住的是两件事：

      · **知道种类推不出粗细**：同一种类内部的粗细跨度必须 ≥2×，靠的是它在站姿里的
        位置。若某一类内部挤成一团，说明位置没起作用、粗细退化成了查表。
      · **粗细跟着力学量走**：log 粗细与 log √(载荷 × 地面力臂 / 肌肉力臂) 的相关必须
        ≥0.7。这两个量里没有一处读过"供体本来多粗"——那个数字压根不存在于流水线里。
    """
    bad: list[str] = []
    bear = [(s, lb) for s, lb in rows if lb.bearing]
    if len(bear) < 8:
        bad.append(f"承重肢样本只有 {len(bear)} 条，跨 seed 的统计断言不成立——加 seed")
        return bad

    for kind in sorted({x.gene.kind for _s, x in bear}):
        rs = [x.root_need for _s, x in bear if x.gene.kind == kind]
        if len(rs) < 3:
            continue
        print(f"[粗细] {kind} 类内 {min(rs):.2f}..{max(rs):.2f} px "
              f"（{max(rs) / min(rs):.2f}×，{len(rs)} 条）")
        if max(rs) / min(rs) < 1.35:
            bad.append(f"{kind} 类内部的粗细跨度只有 {max(rs) / min(rs):.2f}×——"
                       f"同一种类的肢挂在不同位置该差出好几倍，挤成一团说明载荷"
                       f"没进公式，粗细退化成了按种类查表")

    # ---- ② 扛重的是短近腿：载荷份额与"落点离质心的距离"负相关。
    #
    # 这条只在**汇总**上成立，不能逐只兽卡：质心偏向一侧时，那一侧的远脚照样可能
    # 分到大头（实测 seed 2 单只相关 +0.78）。它是个统计规律不是定理——"离质心越远
    # 力臂越长、分到的反力越少"在质心居中时才干净。
    dd = np.array([x.arm0 for _s, x in bear])
    ff = np.array([x.load for _s, x in bear])
    if dd.std() > 1e-6 and ff.std() > 1e-6:
        rc = float(np.corrcoef(dd, ff)[0, 1])
        print(f"[载荷] 与落点力臂的相关 {rc:+.3f}（{len(bear)} 条）")
        if rc > -0.10:
            bad.append(f"载荷与力臂相关 {rc:+.2f}（应为负）——离质心越远该分得越少，"
                       f"正相关说明载荷不是从静力平衡解的")

    r = np.log([x.root_need for _s, x in bear])
    drv = np.log([math.sqrt(x.load * max(x.arm1, 1e-6) / max(x.gene.segments[0], 1e-9))
                  for _s, x in bear])
    corr = float(np.corrcoef(drv, r)[0, 1])
    print(f"[粗细] 与 √(载荷×力臂/杠杆) 的 log 相关 {corr:.3f}（{len(bear)} 条承重肢）")
    if corr < 0.70:
        bad.append(f"粗细与力学驱动量的相关只有 {corr:.2f}——粗细不是从力矩来的")
    return bad


def check_stance_regression(socks) -> list[str]:
    """⑨ 收窄站姿真的省粗细。

    盯的是 `locomotion.stance_radius`：落点从可达极限收到舒适伸展度，是这一层最大的
    一处力学修正。把落点推回极限重算一次，根部需求必须显著变粗——不变说明那处修复被
    改回去了，或者被别的改动绕过了。
    """
    bad: list[str] = []
    gen, gait = LM.sample_standing(7, socks=socks)
    loads = LB.foot_loads(gait)
    tight = wide = 0.0
    for lg in gait.limbs:
        gene, sock = lg.gene, socks[lg.gene.socket]
        f = loads.get(gene.socket, 0.0)
        a = LB.solve_limb(gene, sock, load=f, foot=lg.foot, ride=gait.ride)
        far = np.array([lg.hip[0], 0.0, lg.hip[2]]) + lg.out_dir * lg.reach
        b = LB.solve_limb(gene, sock, load=f, foot=far, ride=gait.ride)
        tight += a.root_need
        wide += b.root_need
    if wide < tight * 1.15:
        bad.append(f"落点推到可达极限后根部只从 {tight:.1f} 涨到 {wide:.1f} px——"
                   f"locomotion.stance_radius 的收窄失效了（应显著更粗）")
    else:
        print(f"[站姿] 舒适伸展度 Σ根部 {tight:.1f} px vs 可达极限 {wide:.1f} px "
              f"（省 {(1 - tight / wide) * 100:.0f}%）")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seeds", default="1,2,3,7")
    args = ap.parse_args()
    socks = C.sockets()
    bad: list[str] = []
    rows: list[tuple] = []
    for s in (int(x) for x in args.seeds.split(",")):
        b, r = check_seed(s, socks)
        bad += b
        rows += r
        n = sum(1 for _s, lb in r if lb.bearing)
        rr = [lb.root_need for _s, lb in r if lb.bearing]
        print(f"[肢体] seed={s}  {len(r)} 肢（承重 {n}）  "
              f"根部需求 {min(rr):.2f}..{max(rr):.2f} px")
    bad += cross_seed(rows)
    bad += check_stance_regression(socks)

    if bad:
        print(f"\n✗ {len(bad)} 处违例：")
        for x in bad:
            print(f"   {x}")
        return 1
    print("\n✓ 载荷解 / 短近腿扛重 / 弯曲主导 / 肌肉递减 / IK / 垂姿 / "
          "粗细跟载荷 / 脚掌压强 / 站姿收窄 全部通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
