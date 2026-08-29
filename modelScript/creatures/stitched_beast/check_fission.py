#!/usr/bin/env python3
"""异变缝合兽 —— 碎裂 / 分裂 / 繁殖自检。

要卡住的都是**会让机制退化成特效**的东西：

  · 动量守恒 Σmᵢvᵢ = 0 —— 不守恒就是"被炸飞"，不是"自己崩开"
  · 质量守恒 —— 碎片质量之和必须等于整体，不许凭空多出或吞掉肉
  · 应力单调性 —— 阈值升高只能碎得更细，不能忽多忽少
  · 碎片质量必须**摊得开** —— 一团独大时只有主体活得下来，「各部位分头逃窜」不成立
  · 逃窜速度必须**互不相同** —— 全等速就只剩方向不同了
  · 分裂两半都能活，且割面是最小割
  · 繁殖闭合 —— 碎片基因组是亲代的真子集，不许凭空长出亲代没有的部件；
    掉肢只允许掉在被裂面毁掉的槽上（撕开自己有代价，但代价必须说得出在哪）

用法: python3 modelScript/creatures/stitched_beast/check_fission.py
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import core_anim as A  # noqa: E402
import fission as F  # noqa: E402
import fragment as FR  # noqa: E402
import genome as GN  # noqa: E402
import locomotion as L  # noqa: E402


def main() -> int:
    bad: list[str] = []
    mass = C.lobe_mass()
    total = sum(mass.values())
    edges = C.weld_areas()

    # ---- ① 质量摊得开：最大一团不得超过全身的 1/3
    biggest = max(mass.values()) / total
    print(f"[质量] 总 {total} 体素，最大一团占 {biggest * 100:.1f}%，"
          f"可存活团 {sum(1 for m in mass.values() if m >= F.VIABLE_MASS)} 个")
    if biggest > 0.34:
        bad.append(f"最大一团占 {biggest * 100:.1f}% > 34%——一团独大时爆体只有主体活得下来，"
                   f"「各部位分头逃窜」不成立；摊平 core.LOBES 的半径与权重")

    # ---- ② 应力单调：阈值升高只能碎得更细
    prev = 0
    for th in range(0, 22):
        n = len(F.fracture(th))
        if n < prev:
            bad.append(f"应力阈值 {th} 时碎片数从 {prev} 降到 {n}——应力升高不该让它重新粘上")
        prev = n

    # ---- ③ 爆体：质量守恒 + 动量守恒
    for th in (8, 10, 12, 16):
        frags = F.build_fragments(th)
        msum = sum(f.mass for f in frags)
        if msum != total:
            bad.append(f"阈值 {th}：碎片质量和 {msum} ≠ 整体 {total}——吞肉或凭空生肉")
        p = sum(f.mass * f.launch for f in frags)
        norm = float(np.linalg.norm(p))
        scale = total * max(float(np.linalg.norm(f.launch)) for f in frags)
        print(f"[爆体 ≤{th}] {len(frags)} 块  质量和 {msum}  净动量 |Σmv|/|m·v|max = "
              f"{norm / scale:.2e}")
        if norm / scale > 1e-9:
            bad.append(f"阈值 {th}：净动量 {norm:.3e} 不为零——整团朝一边飞，"
                       f"读成被炸飞而不是自己崩开")
        # 每块都得真的往外飞，速度不能是 0
        for f in frags:
            if float(np.linalg.norm(f.launch)) < 0.5:
                bad.append(f"阈值 {th}：碎片 {'+'.join(f.lobes)} 速度近零，会原地不动")

    # ---- ④ 逃窜速度互不相同（否则只剩方向不同）
    frags = F.build_fragments(10)
    spans = sorted(A.crawl_speed(f.span) / 16.0 for f in frags)
    print("[逃窜] 各碎片蠕动速度 格/s：" + " ".join(f"{v:.3f}" for v in spans))
    if spans[-1] / max(spans[0], 1e-9) < 1.5:
        bad.append(f"最快/最慢碎片速度比仅 {spans[-1] / spans[0]:.2f}——"
                   f"全等速逃窜只剩方向不同；蠕动速度须正比于体长（core_anim.crawl_speed）")

    # ---- ⑤ 健康分裂：两半都能活，且确是最小割
    sp = F.split_seam()
    if sp is None:
        bad.append("找不到健康分裂切面——两半都要能活，检查质量分布或 VIABLE_MASS")
    else:
        a, b, cut = sp
        ma, mb = (sum(mass[n] for n in g) for g in (a, b))
        print(f"[分裂] {'+'.join(a)}（{ma}） ⟋ {'+'.join(b)}（{mb}）割面 {cut}")
        if min(ma, mb) < F.VIABLE_MASS:
            bad.append(f"分裂出活不下去的一半（{min(ma, mb)} < {F.VIABLE_MASS}）")
        # 暴力复核最小性
        lobes = [lb.name for lb in C.LOBES]
        for bits in range(1, 1 << (len(lobes) - 1)):
            x = tuple(sorted(n for i, n in enumerate(lobes) if bits >> i & 1))
            y = tuple(sorted(n for n in lobes if n not in x))
            if not x or not y:
                continue
            if min(sum(mass[n] for n in x), sum(mass[n] for n in y)) < F.VIABLE_MASS:
                continue
            if not (F._connected(x, edges) and F._connected(y, edges)):
                continue
            w = sum(v for (i, j), v in edges.items() if (i in x) != (j in x))
            if w < cut:
                bad.append(f"分裂割面不是最小割：{'+'.join(x)} 的割 {w} < {cut}")
                break

    # ---- ⑥ 繁殖闭合：碎片基因组是亲代真子集，且并起来正好是亲代
    socks = C.sockets()
    g, _gait = L.sample_standing(3, socks=socks)
    seen: list[GN.LimbGene] = []
    for f in frags:
        fg = f.genome(g)
        for x in fg.limbs:
            if x not in g.limbs:
                bad.append(f"碎片长出了亲代没有的肢 {x.socket}——繁殖不是刷怪")
            seen.append(x)
    if len(seen) != len(set(seen)):
        bad.append("同一条肢被两块碎片同时带走——挂载点归属必须唯一")
    # 掉肢是**允许**的，但只允许掉在裂面上的那几条：槽是皮上的一处结构，裂面从它中间
    # 过去时那片皮一半跟着另一半走，两边谁也用不了（fragment.viable_sockets）。
    # 掉在别处 = 真的漏了。
    destroyed = set(FR.lost_sockets(tuple(f.lobes for f in frags)))
    missing = [x.socket for x in g.limbs if x not in seen]
    wrongly = [s for s in missing if s not in destroyed]
    print(f"[繁殖] 裂面毁掉 {len(destroyed)} 个槽 {sorted(destroyed)}；"
          f"随之失去 {len(missing)} 条肢")
    if wrongly:
        bad.append(f"亲代的肢 {wrongly} 没跟任何碎片走、其槽也没被裂面毁掉——掉了")
    if not destroyed:
        bad.append("裂面一个槽都没毁掉——撕开自己不该是免费的，"
                   "检查 fragment.viable_sockets 是否真的按创面判")

    # ---- ⑦ 爆体动画：末帧碎片确实分开了，且不穿地太深
    from bbmodel_maker.rig.anim_rig import Rig
    rig = Rig(A.MODEL)
    end = A.sample(rig, "core_burst", 1.0)
    W = rig.world(end)
    pts = {n: rig.joint(n, W) for n in ("core_fore", "core_hind", "core_mid", "lump_dorsal")}
    rest = rig.world(type(end)())
    spread_end = max(float(np.linalg.norm(pts[a] - pts[b]))
                     for a in pts for b in pts if a < b)
    spread_0 = max(float(np.linalg.norm(rig.joint(a, rest) - rig.joint(b, rest)))
                   for a in pts for b in pts if a < b)
    print(f"[爆体动画] 末帧最远两团相距 {spread_end:.1f}px（静止 {spread_0:.1f}px）")
    if spread_end < spread_0 * 1.5:
        bad.append(f"爆体末帧碎片没散开（{spread_end:.1f} vs 静止 {spread_0:.1f}）")

    if bad:
        print(f"\n✗ {len(bad)} 处违例：")
        for x in bad:
            print(f"   {x}")
        return 1
    print("\n✓ 质量摊平 / 应力单调 / 质量守恒 / 动量守恒 / 逃窜异速 / 最小割分裂 / "
          "繁殖闭合 / 爆体散开 全通过")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
