#!/usr/bin/env python3
"""异变缝合兽 —— 核心层生成：隐式场 → .bbmodel + 自检。

本文件只管**核心本体**：那团活着的肉。狼首、蛛足、猪腿全是后续部件层的事，这里
只把它们将来要焊上去的位置（socket）算好、把核心自己的骨骼分段建好。

核心的骨骼分段不是手划的，是 core.LOBES 的一一映射——每块几何归属于对它贡献最大
的 lobe。这样搏动动画可以逐 lobe 给不同相位：主体缓慢起伏、赘生物各自抽搐，看起来
就不像"一个整体在呼吸"，而像"几团东西挤在一张皮里各喘各的"。

用法:
  python3 scripts/models/stitched_beast/gen_core.py          # 生成 + 自检
  python3 scripts/models/stitched_beast/gen_core.py --check  # 只自检不写文件
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
from voxel_rig import Palette, Rig  # noqa: E402

OUT_DIR = HERE.parent / "local_models"
OUT = OUT_DIR / "StitchedBeastCore.bbmodel"

# 末法调色：没有一个鲜亮色。唯一的暖色是黏液那点病态黄绿，且它是"淌出来的"不是"长出来的"。
MATS: dict[str, tuple[int, int, int]] = {
    "flesh_pale": (150, 132, 130),
    "flesh_deep": (108, 84, 88),
    "membrane": (182, 168, 156),
    "vein": (84, 52, 56),
    "bile": (134, 132, 92),
    "scar": (126, 104, 100),
    "stitch": (52, 44, 42),
    "drip": (118, 116, 80),
}

# 垂滴：挂在下腹最低处的黏液柱。数量少而长短不一——等长的一排会读成"流苏"，
# 那是装饰；长短乱的才读成"正在滴"。
DRIP_COUNT = 7


def _bone_tree(rig: Rig) -> None:
    """root → core_mid → 各 lobe。pivot 取 lobe 中心，搏动才绕自己的中心胀缩。"""
    rig.bone("root", (0.0, 0.0, 0.0))
    for lb in C.LOBES:
        pivot = tuple(np.array(lb.center) + C.CORE_CENTER)
        rig.bone(lb.name, pivot, lb.parent or "root")


def part_mass(rig: Rig) -> int:
    """核心主体：等值面表层 slab。"""
    n = 0
    for s in C.merge_slabs(C.voxelize()):
        frm, to = C.slab_box(s)
        rig.cube(s.bone, f"mass_{s.bone}_{s.iy}_{s.x0}_{s.z0}", tuple(frm), tuple(to), mat=s.mat)
        n += 1
    return n


def part_drips(rig: Rig) -> int:
    """下腹垂滴。

    落点取下腹表层最低的一批 slab，按确定性噪声挑 DRIP_COUNT 个且强制彼此拉开——
    不拉开的话噪声会把它们挑在相邻格上，渲出来是一小撮而不是散在肚皮下。
    """
    slabs = [s for s in C.merge_slabs(C.voxelize()) if s.bone in ("core_sag", "core_mid")]
    cand = []
    for s in slabs:
        frm, to = C.slab_box(s)
        cand.append((float(frm[1]), s, frm, to))
    cand.sort(key=lambda r: r[0])
    chosen: list[tuple] = []
    for y0, s, frm, to in cand:
        cx, cz = (frm[0] + to[0]) / 2, (frm[2] + to[2]) / 2
        if any(abs(cx - a) < 4.0 and abs(cz - b) < 4.0 for a, b, _ in chosen):
            continue
        chosen.append((cx, cz, (y0, s, frm, to)))
        if len(chosen) >= DRIP_COUNT:
            break
    for i, (cx, cz, (y0, s, _frm, _to)) in enumerate(chosen):
        # 长度上限压在 5.5：更长的垂滴会垂到腿层里去，读成"触手"而不是"正在滴的黏液"。
        drop = 1.8 + C._noise(s.iy, s.x0, s.z0, "drip") * 3.7
        r = 0.7 + C._noise(s.iy, s.x0, s.z0, "dr") * 0.6
        rig.cube(s.bone, f"drip_{i}", (cx - r, y0 - drop, cz - r), (cx + r, y0 + 0.6, cz + r),
                 mat="drip")
    return len(chosen)


def build() -> Rig:
    rig = Rig(Palette(MATS, swatch=8, size=64))
    _bone_tree(rig)
    part_mass(rig)
    part_drips(rig)
    return rig


# ---------------------------------------------------------------- 自检
def body_bounds(rig: Rig) -> tuple[list[float], list[float]]:
    """**本体**包围盒（不含垂滴）。

    垂滴是挂在外面的黏液，把它算进核心尺寸会让"核心多大"这个数字随垂滴长短漂移，
    卡不住真正想卡的东西。量测与自检必须同一口径，否则打印值和断言值对不上。
    """
    body = [e for e in rig.elements if not e["name"].startswith("drip_")]
    lo = [min(min(c[i] for c in Rig.corners(e)) for e in body) for i in range(3)]
    hi = [max(max(c[i] for c in Rig.corners(e)) for e in body) for i in range(3)]
    return lo, hi


def check(rig: Rig) -> list[str]:
    bad: list[str] = []
    blo, bhi = body_bounds(rig)
    size = [(bhi[i] - blo[i]) / 16.0 for i in range(3)]

    # ① 不对称是硬约束。缝合兽是吞噬堆出来的，不是长出来的——**别把它修对称**。
    #    voxel_rig.mirror_problems 在这里是反向用的：镜像件缺失是**期望**行为，
    #    所以这里不调它，改断言不对称度下限。
    asym = C.asymmetry()
    if asym < 0.20:
        bad.append(f"核心左右过于对称（不对称度 {asym * 100:.1f}% < 20%）——"
                   f"这不是缺陷待修，是正典要求，别去平衡 LOBES")

    # ② 尺寸：固元级 400HP，得压得住场。太小读成史莱姆，太大塞不进洞穴。
    if not (1.5 <= size[0] <= 2.4):
        bad.append(f"核心宽 {size[0]:.2f} 格越界 [1.5, 2.4]")
    if not (1.1 <= size[1] <= 1.9):
        bad.append(f"核心高 {size[1]:.2f} 格越界 [1.1, 1.9]")
    if not (1.6 <= size[2] <= 2.6):
        bad.append(f"核心长 {size[2]:.2f} 格越界 [1.6, 2.6]")

    # ③ 腿的活动空间：核心最低点（垂滴不算）必须离地够高，否则部件层没处放腿。
    if blo[1] < 11.0:
        bad.append(f"核心底 y={blo[1]:.1f} 过低（<11），腿层无空间")
    drips = [e for e in rig.elements if e["name"].startswith("drip_")]
    drip_low = min(min(c[1] for c in Rig.corners(e)) for e in drips) if drips else 99.0
    if drip_low < 8.0:
        bad.append(f"垂滴最低 y={drip_low:.1f} 垂进腿层（<8），会读成触手")

    # ④ 骨骼：每根 lobe 骨都得真的分到几何，否则搏动动画驱动的是空骨。
    orphans = rig.orphan_bones()
    if orphans:
        bad.append(f"空骨：{', '.join(orphans)}")
    for lb in C.LOBES:
        n = len(rig.bones[lb.name]["children"])
        if n < 5:
            bad.append(f"lobe {lb.name} 只分到 {n} 块几何——weight 太低被主体吞了")

    # ⑤ 挂载点：必须真落在表皮上（f≈ISO），且法向朝外。
    socks = C.sockets()
    for s in socks.values():
        f = C.fld(s.pos - C.CORE_CENTER)
        if abs(f - C.ISO) > 0.02:
            bad.append(f"socket {s.name} 不在等值面上（f={f:.3f}）")
        outward = C.fld(s.pos - C.CORE_CENTER + s.normal * 1.5)
        if outward >= C.ISO:
            bad.append(f"socket {s.name} 法向指向体内")
        if s.bone not in rig.bones:
            bad.append(f"socket {s.name} 绑到不存在的骨 {s.bone}")
        if s.girth < 1.4:
            bad.append(f"socket {s.name} 承接面太窄（girth={s.girth:.2f}）")

    # ⑥ 挂载点互不重叠：两个部件焊在同一块表皮上必然穿模。
    names = sorted(socks)
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            d = float(np.linalg.norm(socks[a].pos - socks[b].pos))
            if d < 4.0:
                bad.append(f"socket {a} 与 {b} 相距仅 {d:.1f}，部件会互穿")

    # ⑦ 块数预算：189 上下。翻倍说明 merge 或材质斑块化退化了。
    if len(rig.elements) > 320:
        bad.append(f"块数 {len(rig.elements)} 超预算 320——检查 MAT_PATCH / merge_slabs")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="只自检不写文件")
    args = ap.parse_args()

    rig = build()
    blo, bhi = body_bounds(rig)
    flo, fhi = rig.bounds()
    socks = C.sockets()
    print(f"核心：{len(rig.elements)} 块 / {len(rig.bones)} 骨 / {len(socks)} 挂载点")
    print(f"本体 {(bhi[0] - blo[0]) / 16:.2f} × {(bhi[1] - blo[1]) / 16:.2f} × "
          f"{(bhi[2] - blo[2]) / 16:.2f} 格（含垂滴 {(fhi[1] - flo[1]) / 16:.2f} 高）"
          f"   不对称度 {C.asymmetry() * 100:.1f}%")
    print(f"腹底 y={blo[1]:.1f}   最低几件："
          + "  ".join(f"{n}@{y:.1f}" for y, n in rig.lowest(4)))

    bad = check(rig)
    if bad:
        print(f"\n✗ {len(bad)} 处问题：")
        for x in bad:
            print(f"   {x}")
        return 1
    print("✓ 不对称 / 尺寸 / 离地 / 骨骼 / 挂载点 / 块数 全部通过")
    if not args.check:
        p = rig.save(OUT, "StitchedBeastCore")
        print(f"→ {p}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
