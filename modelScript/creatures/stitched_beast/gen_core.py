#!/usr/bin/env python3
"""异变缝合兽 —— 核心层生成：隐式场 → .bbmodel + 自检。

本文件只管**核心本体**：那团活着的肉。狼首、蛛足、猪腿全是后续部件层的事，这里
只把它们将来要焊上去的位置（socket）算好、把核心自己的骨骼分段建好。

核心的骨骼分段不是手划的，是 core.LOBES 的一一映射——每块几何归属于对它贡献最大
的 lobe。这样搏动动画可以逐 lobe 给不同相位：主体缓慢起伏、赘生物各自抽搐，看起来
就不像"一个整体在呼吸"，而像"几团东西挤在一张皮里各喘各的"。

用法:
  python3 modelScript/creatures/stitched_beast/gen_core.py          # 生成 + 自检
  python3 modelScript/creatures/stitched_beast/gen_core.py --check  # 只自检不写文件
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
from bbmodel_maker.rig.voxel_rig import Palette, Rig  # noqa: E402

OUT_DIR = HERE.parents[2] / "models" / "stitched_beast"
OUT = OUT_DIR / "StitchedBeastCore.bbmodel"

# 末法调色：没有一个鲜亮色。唯一的暖色是黏液那点病态黄绿，且它是"淌出来的"不是"长出来的"。
# round 2 拉开了明度跨度：round 1 全部挤在中灰棕，渲出来像块石头，读不出"由几块
# 不同来源的肉拼起来"。膜要够亮、静脉要够暗、赘生物的 scar 要偏黄——差异本身就是叙事。
MATS: dict[str, tuple[int, int, int]] = {
    "flesh_pale": (162, 142, 138),
    "flesh_deep": (92, 68, 74),
    "membrane": (198, 186, 168),
    "vein": (76, 40, 46),
    "bile": (140, 140, 88),
    "scar": (148, 122, 108),
    "weld": (96, 58, 58),
    "weld_dark": (66, 38, 42),
    "drip": (118, 116, 80),
    "bud_base": (140, 112, 108),
    "bud_mid": (158, 128, 118),
    "bud_tip": (186, 160, 142),   # 末端最亮：新生组织，还没被环境糟蹋
}

# 垂滴：挂在下腹最低处的黏液柱。数量少而长短不一——等长的一排会读成"流苏"，
# 那是装饰；长短乱的才读成"正在滴"。
DRIP_COUNT = 7
DRIP_FLOOR = 8.5   # 垂滴最低不得低于此高度——再低就垂进腿层，读成触手而非黏液


def _bone_tree(rig: Rig) -> None:
    """root → core_mid → 各 lobe → 各挂载点的芽骨。

    lobe pivot 取 lobe 中心，搏动才绕自己的中心胀缩。芽骨 pivot 取挂载点本身，嫁接
    动画对它做 scale 时才是"从皮上鼓出来"，而不是"从核心中心放大"。

    **每个挂载点都建芽骨**，即使当前 growth=0：芽骨是部件层将来的挂点，动画契约得先
    锁死（"每个槽都能嫁接"），否则部件层接进来时动画得重写一遍。
    """
    rig.bone("root", (0.0, 0.0, 0.0))
    for lb in C.LOBES:
        pivot = tuple(np.array(lb.center) + C.CORE_CENTER)
        rig.bone(lb.name, pivot, lb.parent or "root")
    # 每条芽是一条**骨链**（一节一根骨），不是一根骨。整条绑一根骨的话它只能像棍子
    # 一样绕根部倾倒，末端和根部永远共线——乱抽渲出来是机械的（用户实测反馈）。
    # 链上父子相接，所以对根节做缩放会整条一起缩，休眠/生长那套不用改。
    for s in C.sockets().values():
        parent = s.bone
        for j, piv in enumerate(C.bud_joints(s)):
            name = f"bud_{s.name}" if j == 0 else f"bud_{s.name}_{j}"
            rig.bone(name, tuple(piv), parent)
            parent = name


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
        # 长度直接由**离地下限**反推，不靠"猜一个够短的长度"：腹底会随抖动幅度和场
        # 参数上下浮动，写死长度的话调一次抖动就撞破 DRIP_FLOOR（round 2 实测撞了）。
        want = 1.8 + C._noise(s.iy, s.x0, s.z0, "drip") * 3.7
        drop = max(1.2, min(want, y0 - DRIP_FLOOR))
        r = 0.7 + C._noise(s.iy, s.x0, s.z0, "dr") * 0.6
        rig.cube(s.bone, f"drip_{i}", (cx - r, y0 - drop, cz - r), (cx + r, y0 + 0.6, cz + r),
                 mat="drip")
    return len(chosen)


def part_welds(rig: Rig) -> int:
    """癒合痕：两团组织自己长到一起留下的隆起脊。

    这是缝合兽读得出来的**唯一**标志。没有它，核心只是一团肉；有了它，玩家才看得出
    这团肉是几块分别长进来的。位置不是画上去的——见 core.welds，痕即 lobe 边界。

    **不是外科缝合**：没有线、没有针、没有等距横扣。自体融合（正典称"塑性再生"）留下
    的是不规则隆起——粗细逐段变、局部堆成肉瘤、融合彻底处直接没有痕。等粗等距的脊
    会立刻读成人造缝合口。

    脊用**暗色**而非亮色：先试过偏亮的新生组织色（176,150,138），与 flesh_pale
    只差 14 级明度，渲出来完全看不见。在 MC 的像素尺度上，暗线压在中明度肉上才
    读得出走向。两侧组织颜色的不匹配由 lobe 材质自己承担，不用在这里再涂一遍。

    抬出量取抖动上限的六成：抬满（>JITTER_MAX）脊会整条浮在皮外像根虫，抬太少又会
    被鼓出来的肉块埋掉（round 2 两头都撞过）。六成处大部分段贴着皮、少数被肉半埋——
    半埋恰好是癒合痕该有的样子，它本来就长在皮里。
    """
    n = 0
    for i, w in enumerate(C.welds()):
        base = w.pos + w.normal * (C.JITTER_MAX * 0.6 + w.radius * 0.3)
        # 段长必须大于体素间距（VOX=2）的一半，相邻段才重叠成连续的脊
        a = base - w.tangent * 1.55
        b = base + w.tangent * 1.55
        rig.shaft(w.bone, f"weld_{i}", tuple(a), tuple(b), w.radius, mat="weld")
        n += 1
        if w.bulge:
            # 融合失控处堆出的肉瘤：更外、更粗、压一层暗色，读成"这里长坏了"
            tip = w.pos + w.normal * (C.JITTER_MAX * 0.6 + 0.35 + w.radius * 0.6)
            r = w.radius * 0.85
            rig.cube(w.bone, f"wart_{i}", tuple(tip - r), tuple(tip + r), mat="weld_dark")
            n += 1
    return n


def part_buds(rig: Rig, growth: dict[str, float]) -> int:
    """各挂载点的芽。

    **几何一律按 growth=1 建**，当前生长度由动画的 bone scale 表达（见 core_anim
    的 BUD_DORMANT）。反过来做——几何建在 growth=0 再让动画放大——是行不通的：
    Blockbench/GeckoLib 的几何是静态的，缩放一个 0.44 半径的小疙瘩永远长不出东西，
    实测嫁接动画整条看不见。几何 = 最大形态，动画 = 当前状态。

    `growth` 参数只影响**静态出图**（想看某个中间形态的定格时用）。
    """
    n = 0
    for s in C.sockets().values():
        for k, (ctr, r, mat) in enumerate(C.bud_shape(s, growth.get(s.name, 1.0))):
            # 第 k 节挂到第 k 根骨上——每节各自能转，整条才弯得起来
            bone = f"bud_{s.name}" if k == 0 else f"bud_{s.name}_{k}"
            rig.cube(bone, f"budc_{s.name}_{k}",
                     tuple(ctr - r), tuple(ctr + r), mat=mat)
            n += 1
    return n


def build(growth: dict[str, float] | None = None) -> Rig:
    rig = Rig(Palette(MATS, swatch=8, size=64))
    _bone_tree(rig)
    part_mass(rig)
    part_welds(rig)
    part_drips(rig)
    part_buds(rig, growth or {})
    return rig


# ---------------------------------------------------------------- 自检
def body_bounds(rig: Rig) -> tuple[list[float], list[float]]:
    """**本体**包围盒（不含垂滴与芽）。

    垂滴是挂在外面的黏液；芽的几何按 growth=1 建但运行时默认压到 BUD_DORMANT。
    把这两样算进核心尺寸，"核心多大"这个数字就会随它们漂移，卡不住真正想卡的东西。
    量测与自检必须同一口径，否则打印值和断言值对不上。芽的尺寸另有专门断言（⑧）。
    """
    body = [e for e in rig.elements
            if not e["name"].startswith(("drip_", "budc_"))]
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
    orphans = [b for b in rig.orphan_bones() if not b.startswith("bud_")]
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

    # ⑦ 癒合痕必须存在、覆盖每一处 lobe 交界、且**粗细不齐**。
    #    痕是缝合兽唯一读得出来的标志，这条断言防的是"顺手把接缝抹平"；
    #    粗细方差那条防的是把它做成等粗等距的人造缝合口（正典是自体塑性再生）。
    wl = C.welds()
    if len(wl) < 20:
        bad.append(f"癒合痕只有 {len(wl)} 段——核心必须看得出是几块长到一起的")
    weld_bones = {w.bone for w in wl}
    for lb in C.LOBES:
        if lb.parent and lb.name not in weld_bones:
            bad.append(f"lobe {lb.name} 没有任何癒合痕——它是怎么接上去的？")
    rs = [w.radius for w in wl]
    spread = max(rs) - min(rs)
    if spread < 0.35:
        bad.append(f"癒合痕粗细跨度仅 {spread:.2f}，等粗会读成人造缝合口而非自体融合")
    if not any(w.bulge for w in wl):
        bad.append("没有一处融合肉瘤——通篇均匀的痕不像自己长的")

    # ⑧ 芽：满长时不得粗过挂载面能承的半径（否则嫁接完必然穿模），也不得伸太远
    #    （几何按 growth=1 建，运行时靠 bone scale 压到休眠尺寸）
    for s in socks.values():
        shape = C.bud_shape(s, 1.0)
        for ctr, r, _m in shape:
            if r > s.girth + 0.3:
                bad.append(f"芽 {s.name} 半径 {r:.2f} 超过挂载面 girth {s.girth:.2f}")
        reach = max(float(np.linalg.norm(ctr - s.pos)) + r for ctr, r, _m in shape)
        if reach > 14.0:
            bad.append(f"芽 {s.name} 伸出 {reach:.1f} px 过长（>14），未分化组织不该比部件还大")
        if not C.bud_shape(s, 0.0):
            bad.append(f"挂载点 {s.name} 在 growth=0 时没有任何几何——"
                       f"槽本身该是皮上一处微隆，不是虚无")

    # ⑨ 块数预算。翻倍说明 merge 或材质斑块化退化了。
    if len(rig.elements) > 460:
        bad.append(f"块数 {len(rig.elements)} 超预算 460——检查 MAT_PATCH / merge_slabs")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="只自检不写文件")
    ap.add_argument("--growth", default="", help="嫁接进度，形如 head_c=0.4,limb_fl=0.8")
    args = ap.parse_args()

    growth = {}
    for item in filter(None, args.growth.split(",")):
        k, _, v = item.partition("=")
        growth[k.strip()] = float(v)
    rig = build(growth)
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
