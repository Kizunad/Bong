#!/usr/bin/env python3
"""异变缝合兽 —— 整只兽：核心 + 按基因组长出来的肢体 → .bbmodel + 自检。

`gen_core.py` 出的是**无肢阶段**那团核心（每个挂载点上只有一个芽）。这里出的是长齐了
肢体之后的样子：基因组挑中的槽上，芽已经长成部件，几何由 `limbs.py` 从体重和站姿推出来；
没被挑中的槽仍然只是一个芽——那些是它还没捡到料的位置。

**同一个 seed 必得同一只兽**：槽的选择、供体、尺寸、步态相位全部由 seed 决定，几何再
由它们唯一确定。出了问题能重放同一只。

用法:
  python3 scripts/models/stitched_beast/gen_beast.py --seed 7
  python3 scripts/models/stitched_beast/gen_beast.py --seed 7 --check
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
import gen_core as GC  # noqa: E402
import limbs as LB  # noqa: E402
from voxel_rig import Palette, Rig  # noqa: E402

OUT_DIR = HERE.parent / "local_models"

MATS = dict(GC.MATS)
MATS.update(LB.LIMB_MATS)

# 可见半径的**渲染**下限（px）。远端那一节按力学算出来只有零点二三像素——它本来就该是
# 一根腱包着的细骨（马的管骨就是这样），但 MC 的模型精度到不了亚像素，半个像素以下的
# 柱子渲出来会消失或闪烁。这是渲染的界不是物理的界，和 core_anim.FLICK_MAX_HZ 同类。
RENDER_MIN_R = 0.5

# 肌腹沿节长的粗细剖面（相对峰值）。纺锤形：中段最粗，两端收进腱里。
SPINDLE = (0.66, 1.0, 0.74)


def bone_tree(rig: Rig, limbs: dict[str, LB.Limb]) -> None:
    """core 的骨树 + 逐肢骨链。

    肢骨挂在该槽骑着的那根核心骨上（`sock.bone`），不是一律挂 core_mid——挂错父骨的话
    核心一分节，肢就会被甩出去（核心动画那边踩过）。每节一根骨，pivot 取该节的近端
    关节：将来步态动画驱动的就是这条链。
    """
    rig.bone("root", (0.0, 0.0, 0.0))
    for lb in C.LOBES:
        rig.bone(lb.name, tuple(np.array(lb.center) + C.CORE_CENTER), lb.parent or "root")
    for s in C.sockets().values():
        limb = limbs.get(s.name)
        if limb is None:
            parent = s.bone
            for j, piv in enumerate(C.bud_joints(s)):
                name = f"bud_{s.name}" if j == 0 else f"bud_{s.name}_{j}"
                rig.bone(name, tuple(piv), parent)
                parent = name
            continue
        parent = s.bone
        for j, piv in enumerate(limb.joints[:-1]):
            name = f"limb_{s.name}_{j}"
            rig.bone(name, tuple(piv), parent)
            parent = name


def part_limbs(rig: Rig, limbs: dict[str, LB.Limb]) -> int:
    """逐肢几何：接合痕 + 各节柱（肌腹分三段收成纺锤）+ 脚掌。"""
    n = 0
    for lb in limbs.values():
        s = lb.sock
        base = f"limb_{s.name}_0"
        # 接合痕：皮上一圈看得见的隆起，半径由**剪切传力**所需的面积给出（一两像素）。
        # 这里曾经画的是一个"把载荷摊开"的大癒合环（半径到 9 px），那个模型是错的——
        # 肌肉不从接合面穿过去，接合面只传力。渲出来是几块贴在身上的板。
        rig.shaft(base, f"weld_{s.name}",
                  tuple(s.pos - s.normal * 0.7), tuple(s.pos + s.normal * 0.5),
                  lb.weld_r, mat="collar")
        n += 1
        for i, (p, q) in enumerate(zip(lb.joints, lb.joints[1:])):
            r = max(lb.radius[i], RENDER_MIN_R)
            # 肌腹是**纺锤形**的：算出来的那个半径是所需生理横截面的**峰值**，出现在
            # 腹的中段，两端收进腱里。整节渲成一根等粗的柱子就成了一块方砖——round 2
            # 的图上六条腿的大腿全是大方块，读不出是肉。分三段按 SPINDLE 收，两端收细
            # 还顺带把关节处的方角削掉了。细到只剩腱骨的节（末节）没有腹，不分段。
            if r <= RENDER_MIN_R * 2.0 or lb.muscle[i] <= 0.0:
                rig.shaft(f"limb_{s.name}_{i}", f"seg_{s.name}_{i}",
                          tuple(p), tuple(q), r, mat=lb.mats[i])
                n += 1
                continue
            for k, f in enumerate(SPINDLE):
                a = p + (q - p) * (k / len(SPINDLE))
                b = p + (q - p) * ((k + 1) / len(SPINDLE))
                rig.shaft(f"limb_{s.name}_{i}", f"seg_{s.name}_{i}_{k}",
                          tuple(a), tuple(b), max(r * f, RENDER_MIN_R), mat=lb.mats[i])
                n += 1
        if lb.bearing:
            # 脚掌：面积 = 峰值载荷 / 地面承载力。压强超了就陷进灰里，所以载荷大的脚大。
            t = lb.joints[-1]
            hw, hl = lb.pad
            h = max(1.0, lb.radius[-1] * 2.0)
            rig.cube(f"limb_{s.name}_{len(lb.joints) - 2}", f"pad_{s.name}",
                     (t[0] - hw, 0.0, t[2] - hl), (t[0] + hw, h, t[2] + hl),
                     mat=lb.mats[-1])
            n += 1
    return n


def build(seed: int, *, bud_growth: float = 1.0
          ) -> tuple[Rig, LB.LM.Gait, dict[str, LB.Limb]]:
    """`bud_growth` 只影响**没长肢的那些槽**画多大。

    默认 1.0 是几何契约：芽按满尺寸建，当前生长度由动画的 bone scale 表达（见
    `gen_core.part_buds`）。想看这只兽平时的样子，出图时传 `core_anim.BUD_DORMANT`
    ——否则十几个满长的芽会把整只兽埋掉，腿一条都看不见（round 1 实测）。
    """
    gen, gait, limbs = LB.build(seed)
    rig = Rig(Palette(MATS, swatch=8, size=64))
    bone_tree(rig, limbs)
    GC.part_mass(rig)
    GC.part_welds(rig)
    GC.part_drips(rig)
    for s in C.sockets().values():           # 没长肢的槽仍然只是一个芽
        if s.name in limbs:
            continue
        for k, (ctr, r, mat) in enumerate(C.bud_shape(s, bud_growth)):
            bone = f"bud_{s.name}" if k == 0 else f"bud_{s.name}_{k}"
            rig.cube(bone, f"budc_{s.name}_{k}", tuple(ctr - r), tuple(ctr + r), mat=mat)
    part_limbs(rig, limbs)
    return rig, gait, limbs


# ---------------------------------------------------------------- 自检
def check(rig: Rig, gait, limbs: dict[str, LB.Limb]) -> list[str]:
    bad: list[str] = []
    W = LB.body_weight()

    # ① 承重的脚必须**踩在地上**：站姿是按落点解的，脚离地或穿地都说明 IK 没收敛
    for lb in limbs.values():
        if not lb.bearing:
            continue
        y = float(lb.tip[1])
        if abs(y) > 0.6:
            bad.append(f"{lb.name} 脚尖 y={y:+.2f} 没落在地面——站姿解错了")
        err = float(np.linalg.norm(lb.tip - np.array(
            [lb.tip[0], 0.0, lb.tip[2]])))
        if err > 0.6:
            bad.append(f"{lb.name} 落点偏 {err:.2f} px")

    # ② 肢体不能从核心里穿出来：除根部那一节外，各关节必须在等值面之外。
    #    根部本来就骑在表皮上，所以从第二个关节起判。
    for lb in limbs.values():
        for j, p in enumerate(lb.joints[1:], 1):
            f = C.fld(np.asarray(p, float) - C.CORE_CENTER)
            if f >= C.ISO:
                bad.append(f"{lb.name} 第 {j} 个关节埋在核心里（f={f:.2f}）")

    # ③ 相邻两条肢的根部不能互相穿。挂载点之间只保证 ≥4 px（gen_core ⑥），而承重肢的
    #    根部半径由载荷定，完全可能粗到把邻居吃掉——这时该换槽或换供体，不是硬塞。
    names = sorted(limbs)
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            la, lbb = limbs[a], limbs[b]
            d = float(np.linalg.norm(la.sock.pos - lbb.sock.pos))
            need = la.root_r + lbb.root_r
            if d < need * 0.75:
                bad.append(f"{a} 与 {b} 根部相距 {d:.1f} px，两根合起来 {need:.1f} px"
                           f"——焊在一起了")

    # ④ 腿必须**上粗下细**——但卡的是**需求**不是渲染出来的半径。两处不能混：
    #    · 不要求逐节单调：蹲得深的腿，中段那个甩出载荷线的关节确实要更多肌肉去扛
    #      （真实动物蹲下时小腿也会鼓）。
    #    · 根部可以是全条**最细**的：肌腹粗不过基节长度的一半，装不下的埋在体内，
    #      而蛛足的基节只有 3 px——它外面看就是细的（`buried` 达 96%）。那正是推导的
    #      结论，不是缺陷。所以"根部不是最粗"只在**没被埋没截断**时才算违例。
    for lb in limbs.values():
        if not lb.bearing:
            continue
        r = lb.radius
        if min(r) != r[-1]:
            bad.append(f"{lb.name} 末节不是最细的 {[round(x, 2) for x in r]}——"
                       f"末节只有腱穿过，它必须是最细的一节")
        if r.index(max(r)) > (len(r) - 1) // 2 and lb.buried < 0.02:
            bad.append(f"{lb.name} 最粗的一节在远端 {[round(x, 2) for x in r]} 且没有"
                       f"埋没截断（buried={lb.buried:.0%}）——抗重力肌都长在近端，"
                       f"远端鼓起来说明力矩臂算反了")
        if lb.root_need / max(r[-1], 1e-9) < 3.0:
            bad.append(f"{lb.name} 根部需求/梢粗只差 "
                       f"{lb.root_need / max(r[-1], 1e-9):.1f}×——腿该显著收细，"
                       f"等粗说明力矩没进公式")

    # ⑤ 载荷守恒：任一时刻所有着地脚的反力之和必须等于体重。这是整层的地基，
    #    错了后面全部跟着错，所以在这里再验一遍（check_limbs 验的是解法本身）。
    com2 = np.array([gait.com[0], gait.com[2]])
    for k in range(24):
        t = k / 24
        on = [lg for lg in gait.limbs if lg.in_stance(t)]
        if len(on) < 3:
            continue
        P = np.array([lg.foot_at(t) for lg in on])[:, [0, 2]]
        R = LB.contact_forces(P, com2, W)
        if abs(R.sum() - W) > 1e-3 * W:
            bad.append(f"t={t:.2f} 反力和 {R.sum():.0f} ≠ 体重 {W:.0f}")
            break

    # ⑥ 脚掌压强不得超过地面承载力——超了就是站在灰里往下陷
    for lb in limbs.values():
        if not lb.bearing:
            continue
        area = (2 * lb.pad[0] * LB.PX) * (2 * lb.pad[1] * LB.PX)
        if area <= 0 or lb.load / area > LB.BEARING * 1.02:
            bad.append(f"{lb.name} 脚掌压强 {lb.load / max(area, 1e-9) / 1e3:.0f} kPa "
                       f"超过地面承载力 {LB.BEARING / 1e3:.0f} kPa——它会陷下去")

    # ⑦ 骨骼：每根肢骨都得分到几何，且没有空骨
    orphans = [b for b in rig.orphan_bones() if not b.startswith("bud_")]
    if orphans:
        bad.append(f"空骨：{', '.join(orphans)}")

    # ⑧ 站高：**关节中轴不许入地**，但肉的下半侧陷进去一点是对的——地面是松散废土
    #    （脚掌尺寸就是按它的承载力反推的），一条蹲得深的腿把小腿肚压进灰里合乎同一
    #    个模型。所以卡的是中轴线，不是包围盒。
    for lb in limbs.values():
        for j, p in enumerate(lb.joints):
            if float(p[1]) < -0.05:
                bad.append(f"{lb.name} 第 {j} 个关节 y={float(p[1]):+.2f} 在地面以下——"
                           f"关节中轴入地是姿态解错了，不是陷进灰里")
    low = min(min(c[1] for c in Rig.corners(e)) for e in rig.elements)
    deep = max((max(lb.radius) for lb in limbs.values()), default=0.0)
    if low < -deep - 0.6:
        bad.append(f"最低点 y={low:+.2f} 比最粗一节的半径 {deep:.2f} 还深——"
                   f"这不是陷进灰里，是有东西整个埋在地下")
    body = [e for e in rig.elements
            if e["name"].startswith(("mass_", "wart_")) or e["name"].startswith("weld_c")]
    blow = min(min(c[1] for c in Rig.corners(e)) for e in body) + gait.ride
    if blow <= 0.0:
        bad.append(f"核心底 y={blow:.1f} 压到地面——腿等于没有")
    # 只报不判：蹲到什么程度是运动层的事（`RIDE_CLEAR` 允许把最低的髋压到离地 3 px），
    # 这一层无权替它定"蹲得太低"。
    print(f"[站姿] 核心腹底离地 {blow:.1f} px（骑乘 {gait.ride:+.1f}）")

    # ⑨ 不对称：长齐了肢也不许对称。左右各有几条承重肢必须不等，或长度差得开。
    left = sorted(lb.length for lb in limbs.values() if lb.sock.pos[0] < 0 and lb.bearing)
    right = sorted(lb.length for lb in limbs.values() if lb.sock.pos[0] > 0 and lb.bearing)
    if len(left) == len(right) and all(abs(a - b) < 1.5 for a, b in zip(left, right)):
        bad.append("左右承重肢镜像对称——缝合兽不该对称")

    # ⑩ 块数预算
    if len(rig.elements) > 560:
        bad.append(f"块数 {len(rig.elements)} 超预算 560")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--bud-growth", type=float, default=1.0,
                    help="没长肢的槽画多大；出图看真实样子传 0.1（见 build）")
    args = ap.parse_args()

    rig, gait, limbs = build(args.seed, bud_growth=args.bud_growth)
    W = LB.body_weight()
    nb = sum(1 for lb in limbs.values() if lb.bearing)
    print(f"缝合兽 seed={args.seed}：{len(rig.elements)} 块 / {len(rig.bones)} 骨 / "
          f"{len(limbs)} 肢（承重 {nb}）  体重 {W / LB.G:.0f} kg")
    print(LB.report_table(limbs, W))
    lo, hi = rig.bounds()
    print(f"整体 {(hi[0] - lo[0]) / 16:.2f} × {(hi[1] - lo[1]) / 16:.2f} × "
          f"{(hi[2] - lo[2]) / 16:.2f} 格   骑乘 {gait.ride:+.1f} px   "
          f"行走 {gait.blocks_per_sec:.2f} 格/s")

    bad = check(rig, gait, limbs)
    if bad:
        print(f"\n✗ {len(bad)} 处问题：")
        for x in bad:
            print(f"   {x}")
        return 1
    print("✓ 触地 / 不埋进核心 / 根部不互穿 / 粗细单调 / 载荷守恒 / 不陷地 / "
          "骨骼 / 站高 / 不对称 / 块数 全部通过")
    if not args.check:
        p = rig.save(OUT_DIR / f"StitchedBeast_{args.seed}.bbmodel",
                     f"StitchedBeast_{args.seed}")
        print(f"→ {p}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
