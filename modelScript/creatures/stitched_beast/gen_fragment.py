#!/usr/bin/env python3
"""异变缝合兽 —— 碎片模型生成：碎片几何 → .bbmodel + 自检。

**这不是把核心等比缩小**。碎片是被撕下来的，身上有三样母体没有的东西：

  创面      断口露的是内部组织，材质另起一张表（见 core._torn_material）
  重新分节  只带一团肉的碎片沿爬行方向切成前/中/后三节——一块刚体蠕动不了
  短茬      组织预算按质量缩小，同样的槽在它身上只冒得出更短的芽

以及一样母体有而它没有的：**癒合痕**。只带一团肉的碎片身上一道痕都没有——痕是两块
组织长到一起的记号，它只有一块。缝合兽不是生下来就缝合的，那身痕是它一辈子吞下去的
东西留下的账。

用法:
  python3 modelScript/creatures/stitched_beast/gen_fragment.py
  python3 modelScript/creatures/stitched_beast/gen_fragment.py --lobes core_hind,core_sag,nodule_r
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import fragment as FR  # noqa: E402
import gen_core as G  # noqa: E402
from bbmodel_maker.rig.voxel_rig import Palette, Rig  # noqa: E402

OUT_DIR = G.OUT_DIR
OUT = OUT_DIR / "StitchedBeastShard.bbmodel"

# 创面三色：湿的内部组织。比外皮**暗且偏红**——外皮是被风干、被环境糟蹋过的，
# 断口是刚裂开的。三色不是渐变，是断口本来就深浅不匀（脂肪层、肌层、扯断的血管）。
TORN_MATS = {
    "torn": (150, 74, 72),
    "torn_deep": (104, 44, 50),
    "torn_vein": (58, 26, 34),
}
MATS = {**G.MATS, **TORN_MATS}

DRIP_MAX = 4          # 碎片最多挂几条垂滴；实际条数按质量比缩


def _bone_tree(rig: Rig, g: FR.FragGeom) -> None:
    for b in g.bones:
        rig.bone(b.name, tuple(b.pivot), b.parent)
    # 骨链，同 gen_core：一节一根骨，整条才弯得起来
    for s in g.sockets.values():
        parent = s.bone
        for j, piv in enumerate(C.bud_joints(s)):
            name = f"bud_{s.name}" if j == 0 else f"bud_{s.name}_{j}"
            rig.bone(name, tuple(piv), parent)
            parent = name


def part_mass(rig: Rig, g: FR.FragGeom) -> int:
    n = 0
    for s in C.merge_slabs(list(g.voxels)):
        frm, to = C.slab_box(s, rough=g.rough)
        rig.cube(s.bone, f"mass_{s.bone}_{s.iy}_{s.x0}_{s.z0}",
                 tuple(frm), tuple(to), mat=s.mat)
        n += 1
    return n


def part_welds(rig: Rig, g: FR.FragGeom) -> int:
    """碎片身上**残存**的癒合痕。

    只带一团肉的碎片一道都没有——这不是漏了，是推论：痕是两块组织的交界，它只有一块。
    自检因此不能照抄核心那条"至少 20 段"，得按 lobe 数分情况断言。
    """
    n = 0
    for i, w in enumerate(C.welds(list(g.voxels))):
        base = w.pos + w.normal * (C.JITTER_MAX * 0.6 + w.radius * 0.3)
        rig.shaft(w.bone, f"weld_{i}", tuple(base - w.tangent * 1.55),
                  tuple(base + w.tangent * 1.55), w.radius, mat="weld")
        n += 1
        if w.bulge:
            tip = w.pos + w.normal * (C.JITTER_MAX * 0.6 + 0.35 + w.radius * 0.6)
            r = w.radius * 0.85
            rig.cube(w.bone, f"wart_{i}", tuple(tip - r), tuple(tip + r), mat="weld_dark")
            n += 1
    return n


def part_drips(rig: Rig, g: FR.FragGeom, shift: np.ndarray) -> int:
    """垂滴。**优先挂在创面上**——断口是新的，还在渗；外皮上那点黏液是陈的。

    条数按质量比缩：一块 96 体素的肉挂七条黏液柱会变成一只水母。

    必须在**落地之后**生成：垂多长是由离地余量反推的，不知道地在哪就算不出来。
    """
    want = max(1, round(DRIP_MAX * g.mass / sum(C.lobe_mass().values())))
    pool = [v for v in g.voxels if v.torn] or list(g.voxels)

    def at(v: C.Voxel) -> np.ndarray:
        return C._voxel_center(v.ix, v.iy, v.iz) + C.CORE_CENTER + shift

    # 挂得住要同时满足两条：朝下的面（挂得住）+ 离地够高（垂得下）。碎片肚子贴着地，
    # 只按"最低"挑必然挑到已经压在地上的那几格，垂滴无处可垂（实测扎地 -0.8）。
    down = sorted((v for v in pool if v.normal[1] < 0.1 and at(v)[1] - C.VOX / 2 >= 1.5),
                  key=lambda v: at(v)[1])
    chosen: list[C.Voxel] = []
    for v in down:
        p = at(v)
        if any(abs(p[0] - at(u)[0]) < 3.0 and abs(p[2] - at(u)[2]) < 3.0 for u in chosen):
            continue
        chosen.append(v)
        if len(chosen) >= want:
            break
    for i, v in enumerate(chosen):
        p = at(v)
        y0 = p[1] - C.VOX / 2
        # 长度由**离地余量**反推，不写死：碎片贴地，写死长度必然扎进地里
        drop = min(1.2 + C._noise(v.ix, v.iy, v.iz, "fd") * 2.6, y0 - 0.3)
        r = 0.55 + C._noise(v.ix, v.iy, v.iz, "fr") * 0.45
        rig.cube(v.bone, f"drip_{i}", (p[0] - r, y0 - drop, p[2] - r),
                 (p[0] + r, y0 + 0.5, p[2] + r), mat="drip")
    return len(chosen)


def part_buds(rig: Rig, g: FR.FragGeom, shift: np.ndarray, growth: float = 1.0) -> int:
    """芽。**资产几何一律按 growth=1 建**（与核心同约定），当前生长度由动画的 bone
    scale 表达——Blockbench/GeckoLib 的几何是静态的，反过来做（几何建小、动画放大）
    永远长不出东西。

    `growth` 只用于**出图**：碎片的组织预算撑不到 1.0，按 1.0 出图会看到一只满身
    长茬的海胆，那是它这辈子都到不了的形态（实测 round 1 出图整只被芽淹没）。
    预览要按 `FragGeom.growth()` 出，看到的才是它在世界里的样子。
    """
    n = 0
    for s in g.sockets.values():
        for k, (ctr, r, mat) in enumerate(C.bud_shape(s, growth)):
            bone = f"bud_{s.name}" if k == 0 else f"bud_{s.name}_{k}"
            rig.cube(bone, f"budc_{s.name}_{k}",
                     tuple(ctr - r + shift), tuple(ctr + r + shift), mat=mat)
            n += 1
    return n


def translate(rig: Rig, d: np.ndarray) -> None:
    """把整只（骨枢轴 + 所有件）平移 d。"""
    for b in rig.bones.values():
        b["pivot"] = [round(v + d[i], 3) for i, v in enumerate(b["pivot"])]
    for e in rig.elements:
        for key in ("from", "to", "origin"):
            e[key] = [round(v + d[i], 3) for i, v in enumerate(e[key])]


def build(lobes: tuple[str, ...], growth: float | None = None) -> tuple[Rig, FR.FragGeom, np.ndarray]:
    """碎片 rig。

    落地这一步必须**先建后量**：癒合脊是沿法向凸出去的、slab 还带抖动，最低点不是
    体素格底。先在母体坐标系里把本体（质量 + 癒合痕）建完，量出真实最低点再整体平移，
    然后才生成垂滴和芽——垂滴的长度是由离地余量反推的，得在知道地面之后算。
    """
    g = FR.geom(lobes)
    rig = Rig(Palette(MATS, swatch=8, size=64))
    _bone_tree(rig, g)
    part_mass(rig, g)
    part_welds(rig, g)
    blo, bhi = body_bounds(rig)
    shift = np.array([-(blo[0] + bhi[0]) / 2, FR.GROUND - blo[1], -(blo[2] + bhi[2]) / 2])
    translate(rig, shift)
    part_drips(rig, g, shift)
    part_buds(rig, g, shift, 1.0 if growth is None else growth)
    return rig, g, shift


# ---------------------------------------------------------------- 自检
def body_bounds(rig: Rig) -> tuple[list[float], list[float]]:
    body = [e for e in rig.elements if not e["name"].startswith(("drip_", "budc_"))]
    lo = [min(min(c[i] for c in Rig.corners(e)) for e in body) for i in range(3)]
    hi = [max(max(c[i] for c in Rig.corners(e)) for e in body) for i in range(3)]
    return lo, hi


def check(rig: Rig, g: FR.FragGeom) -> list[str]:
    bad: list[str] = []
    blo, bhi = body_bounds(rig)

    # ① 它必须是**撕下来的**：没有创面就说明它本来就是独立的一块，那不叫碎片
    if g.torn_ratio < 0.15:
        bad.append(f"创面只占表层 {g.torn_ratio * 100:.0f}%（<15%）——"
                   f"这块看起来不像被撕下来的")

    # ② 但创面也不能是**大部分**：那样它就不是一块肉，是一层壳或一片碎渣。
    #    别写成"创面必须朝同一侧"——中段碎片前后都被扯开，两个断口的法向本来就相反，
    #    按相干度判会把完全正确的中段判成壳（core_mid+lump_l 实测相干度 0.20）。
    if g.torn_ratio > 0.70:
        bad.append(f"创面占表层 {g.torn_ratio * 100:.0f}%（>70%）——"
                   f"这块几乎全是断口，是碎渣不是碎片")

    # ③ 贴地：碎片是爬行动物，静止姿肚子就在地上。悬空会读成"飘着的肉"
    if abs(blo[1] - FR.GROUND) > 0.05:
        bad.append(f"腹底 y={blo[1]:.2f} 未贴地（应为 {FR.GROUND:.2f}）")
    drips = [e for e in rig.elements if e["name"].startswith("drip_")]
    if drips:
        dlow = min(min(c[1] for c in Rig.corners(e)) for e in drips)
        if dlow < -0.05:
            bad.append(f"垂滴扎进地里 y={dlow:.2f}")

    # ④ 每根骨都得真分到几何：空的节在动画里是驱动不了的
    orphans = [b for b in rig.orphan_bones() if not b.startswith("bud_")]
    if orphans:
        bad.append(f"空骨：{', '.join(orphans)}")
    for b in g.bones:
        if b.name == "root":
            continue
        if len(rig.bones[b.name]["children"]) < 3:
            bad.append(f"节 {b.name} 只分到 {len(rig.bones[b.name]['children'])} 块几何——"
                       f"分节切得太碎，蠕动时会看成一片薄片在抖")

    # ⑤ 蠕动的两个锚必须分得开：span 太小就没有可收缩的行程
    if g.fore == g.hind:
        bad.append("前后锚是同一根骨——这块爬不了")
    if g.span < 3.0:
        bad.append(f"锚段间距仅 {g.span:.1f}px（<3）——收缩行程看不出来")

    # ⑥ 挂载点必须真落在**这块**皮上。靠近断口的槽可能整块肉都被扯走了，
    #    那种槽必须已经在 sockets_of 里被剔掉，否则芽会长在半空。
    for s in g.sockets.values():
        near = min(g.voxels, key=lambda v: float(np.linalg.norm(
            C._voxel_center(v.ix, v.iy, v.iz) + C.CORE_CENTER - s.pos)))
        d = float(np.linalg.norm(
            C._voxel_center(near.ix, near.iy, near.iz) + C.CORE_CENTER - s.pos))
        if d > C.VOX * 1.6:
            bad.append(f"槽 {s.name} 离最近的肉 {d:.1f}px，悬在空中")
        if near.torn:
            bad.append(f"槽 {s.name} 落在创面上——那块皮已经被扯走了，"
                       f"芽长不出来（sockets_of 该把它剔掉）")
        if s.bone not in rig.bones:
            bad.append(f"槽 {s.name} 绑到不存在的骨 {s.bone}")

    # ⑦ 癒合痕按 lobe 数分情况：多团必须有痕（它们是长到一起的），单团必须没有
    wl = C.welds(list(g.voxels))
    if len(g.lobes) == 1 and wl:
        bad.append(f"单团碎片却有 {len(wl)} 段癒合痕——它只有一块组织，痕是哪来的？")
    if len(g.lobes) > 1 and len(wl) < 4:
        bad.append(f"{len(g.lobes)} 团组织只有 {len(wl)} 段癒合痕——它们是怎么连着的？")

    # ⑧ 组织预算：它长不出母体那样的触手，这条别被"顺手放大芽"绕过去
    grow = g.growth()
    if grow >= 0.95:
        bad.append(f"碎片齐长 growth={grow:.2f} 接近满长——组织预算没起作用，"
                   f"检查 core.graft_budget / FragGeom.growth")
    used = sum(C.bud_tissue(s, grow) for s in g.sockets.values())
    budget = C.graft_budget() * g.mass / sum(C.lobe_mass().values())
    if used > budget * 1.02:
        bad.append(f"芽用料 {used:.0f} 超预算 {budget:.0f} px³")

    # ⑨ 尺寸：必须明显小于母体，否则"分裂出小东西"这件事读不出来
    size = [(bhi[i] - blo[i]) / 16.0 for i in range(3)]
    if max(size) > 1.7:
        bad.append(f"碎片最大边 {max(size):.2f} 格，和母体（1.92）差不多大")

    # ⑩ 块数预算
    if len(rig.elements) > 260:
        bad.append(f"块数 {len(rig.elements)} 超预算 260")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--lobes", default="")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--out", default="")
    args = ap.parse_args()

    lobes = tuple(filter(None, args.lobes.split(","))) or FR.default_lobes()
    rig, g, _shift = build(lobes)
    blo, bhi = body_bounds(rig)
    print(f"碎片 {'+'.join(g.lobes)}：{len(rig.elements)} 块 / {len(rig.bones)} 骨 / "
          f"{len(g.sockets)} 槽")
    print(f"本体 {(bhi[0] - blo[0]) / 16:.2f} × {(bhi[1] - blo[1]) / 16:.2f} × "
          f"{(bhi[2] - blo[2]) / 16:.2f} 格   创面占表层 {g.torn_ratio * 100:.0f}%   "
          f"癒合痕 {len(C.welds(list(g.voxels)))} 段")
    print(f"锚段 {g.hind} → {g.fore}（{g.span:.1f}px）   齐长 growth={g.growth():.3f}")

    bad = check(rig, g)
    if bad:
        print(f"\n✗ {len(bad)} 处问题：")
        for x in bad:
            print(f"   {x}")
        return 1
    print("✓ 创面 / 贴地 / 分节 / 锚段 / 挂载点 / 癒合痕 / 组织预算 / 尺寸 全部通过")
    if not args.check:
        p = rig.save(Path(args.out) if args.out else OUT, "StitchedBeastShard")
        print(f"→ {p}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
