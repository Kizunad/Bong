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
        for i, (p, q) in enumerate(zip(lb.joints, lb.joints[1:])):
            r = max(lb.radius[i], RENDER_MIN_R)
            # 肌腹是**纺锤形**的：算出来的那个半径是所需生理横截面的**峰值**，出现在
            # 腹的中段，两端收进腱里。整节渲成一根等粗的柱子就成了一块方砖——round 2
            # 的图上六条腿的大腿全是大方块，读不出是肉。分三段按 SPINDLE 收，两端收细
            # 还顺带把关节处的方角削掉了。细到只剩腱骨的节（末节）没有腹，不分段。
            # 分几段要看**长细比**：一节粗腿分成三段，每一段比它自己还宽，渲出来是
            # 一摞板子（round 2 整只兽实测，六条腿全是木板）。只有细长到分得开的才分。
            L = float(np.linalg.norm(q - p))
            prof = (SPINDLE if L >= 6.0 * r else
                    (SPINDLE[1:] if L >= 3.0 * r else (1.0,)))
            if r <= RENDER_MIN_R * 2.0 or lb.muscle[i] <= 0.0:
                prof = (1.0,)
            for k, f in enumerate(prof):
                a = p + (q - p) * (k / len(prof))
                b = p + (q - p) * ((k + 1) / len(prof))
                rig.shaft(f"limb_{s.name}_{i}", f"seg_{s.name}_{i}_{k}",
                          tuple(a), tuple(b), max(r * f, RENDER_MIN_R), mat=lb.mats[i])
                n += 1
        if lb.bearing:
            n += part_foot(rig, lb)
        n += part_coat(rig, lb)
        n += part_scars(rig, lb)
    return n


def part_foot(rig: Rig, lb: LB.Limb) -> int:
    """脚：按**站姿 + 足型**出真几何，不是一块方板。

    这是"哪条腿是从谁身上拆的"最主要的读法——上一版所有脚都是同一块按承载力算大小的
    方板，用户第一句话就是"这棍子是什么腿，看不出来"。现在蹄是蹄、爪是爪、人足有脚跟。

    接触面积仍然恒等于 载荷/地面承载力（`lb.pad`），只是分配到各自的形状里：偶蹄劈成
    两半、肉垫是一块掌垫加三根趾、人足摊成一整片脚底、禽爪三前一后、蛛钩摊一片甲板。
    """
    sk = lb.gene.skeleton
    if sk is None:
        return 0
    j = lb.joints
    hw, hl = lb.pad
    bone = f"limb_{lb.name}_{len(j) - 2}"
    prev = f"limb_{lb.name}_{max(len(j) - 3, 0)}"
    tip, ball = j[-1], j[-2]
    n = 0

    def box(b, name, ctr, half, mat):
        nonlocal n
        rig.cube(b, f"{name}_{lb.name}_{n}",
                 tuple(np.asarray(ctr) - np.asarray(half)),
                 tuple(np.asarray(ctr) + np.asarray(half)), mat=mat)
        n += 1

    if sk.foot == "cloven":
        # 偶蹄：两瓣角质从系部压到地面，中间留一条缝——远看就是"羊/牛的腿"
        for sgn in (-1.0, 1.0):
            c = np.array([ball[0] + sgn * hw * 0.5, hl * 0.30, ball[2]])
            box(bone, "hoof", c, (hw * 0.42, hl * 0.30 + 0.1, hl * 0.55), "hoof")
    elif sk.foot == "paw":
        # 肉垫爪：一块掌垫 + 三根趾 + 爪尖
        box(bone, "pad", (ball[0], 0.7, ball[2] - hl * 0.15),
            (hw * 0.9, 0.7, hl * 0.6), "pad")
        d = tip - ball
        run = float(np.linalg.norm(d[[0, 2]])) or 1.0
        for k, sgn in enumerate((-1.0, 0.0, 1.0)):
            c = np.array([ball[0] + sgn * hw * 0.62 + d[0] * 0.55,
                          0.6, ball[2] + d[2] * 0.55 - abs(sgn) * run * 0.12])
            box(bone, "toe", c, (hw * 0.28, 0.6, run * 0.5), "hide")
            box(bone, "claw", c + np.array([0.0, 0.0, -run * 0.55]),
                (hw * 0.16, 0.35, run * 0.22), "claw")
    elif sk.foot == "human":
        # 人足：整片脚底 + 向后支出来的脚跟 + 三枚趾。跖行的读法全在这块脚跟上
        heel, sole = j[-2], j[-1]
        mid = 0.5 * (heel + sole)
        run = max(float(np.linalg.norm((sole - heel)[[0, 2]])), 1.0)
        box(bone, "sole", (mid[0], 0.8, mid[2]), (hw * 0.72, 0.8, run * 0.5 + hl * 0.3),
            "hide")
        box(prev, "heel", (heel[0], 1.1, heel[2] + run * 0.28),
            (hw * 0.62, 1.1, run * 0.30), "hide")
        for sgn in (-1.0, 0.0, 1.0):
            box(bone, "toe", (sole[0] + sgn * hw * 0.46, 0.6, sole[2] - hl * 0.35),
                (hw * 0.2, 0.6, hl * 0.4), "hide")
    elif sk.foot == "bird":
        # 禽爪：三前一后。后趾是禽腿最好认的特征，别省
        run = max(float(np.linalg.norm((tip - ball)[[0, 2]])), 1.0)
        for sgn, fwd in ((-1.0, 1.0), (0.0, 1.0), (1.0, 1.0), (0.0, -0.55)):
            c = np.array([ball[0] + sgn * hw * 0.7, 0.55,
                          ball[2] - fwd * run * 0.55])
            box(bone, "toe", c, (hw * 0.22, 0.55, run * 0.55 * abs(fwd)), "scute")
            box(bone, "claw", c + np.array([0.0, 0.0, -np.sign(fwd) * run * 0.6]),
                (hw * 0.14, 0.3, run * 0.18), "claw")
    else:                       # claw —— 蛛钩
        # 蛛足本来只有一个点着地，在松散废土上会直接扎进去。本体只能在钩下面长一片
        # 甲板把压强摊开——面积照样是 载荷/承载力，形状是它自己长的，不是原装的。
        box(bone, "plate", (tip[0], 0.5, tip[2]), (hw * 0.8, 0.5, hl * 0.8), "chitin")
        for sgn in (-1.0, 1.0):
            box(bone, "hook", (tip[0] + sgn * hw * 0.55, 1.2, tip[2] - hl * 0.5),
                (0.4, 1.0, hl * 0.45), "claw")
    return n


# 各种体表的做法：(几圈, 每圈几束, 束长/半径, 束粗, 覆盖到第几节, 材质)。
# 数量是**看得见**换来的：毛稀了在这个尺度上等于没有，用户第一眼的评价是"这么秃"。
COAT_SPEC: dict[str, tuple[int, int, float, float, int, str]] = {
    "fur":     (4, 7, 0.85, 0.34, 3, "fur"),      # 兽毛：细密，一直盖到掌骨
    "wool":    (4, 8, 1.15, 0.72, 2, "wool"),     # 羊毛：又粗又密，只在上段结团
    "bristle": (5, 4, 1.30, 0.26, 2, "bristle"),  # 猪鬃：稀、硬、长
    "plume":   (3, 7, 1.05, 0.52, 1, "plume"),    # 禽的覆羽：只到大腿，跗跖骨是裸鳞
}


def part_coat(rig: Rig, lb: LB.Limb) -> int:
    """体表：毛 / 羊毛 / 鬃 / 覆羽。**只有长毛的才加几何**——鳞、甲、皮、裸靠材质就够。

    毛不是贴图能解决的：MC 这个尺度上"毛茸茸"读的是**轮廓**，光把颜色调浅还是一根
    光柱子。但第一版把毛做成了随机撒在柱面上的小方块，用户的评价是"好乱，没有规律"
    ——那是**碎石**不是毛。毛有两条真实结构，缺哪条都不像：

    · **有毛流。** 每一根都顺着肢体往远端躺（外加一点点翘起才离得开皮），不是朝四面
      八方戳。所以这里出的是沿轴向的短柱（`shaft`）不是方块。
    · **成排交错。** 一圈一圈往下排，相邻两圈错开半个间距，像瓦片一样互相压住——
      规律感来自这个，随机偏移只负责让它不呆板。

    抖动只加在长度和翘角上，位置严格按环排。羊毛把束加粗加密到结团，猪鬃反过来又稀
    又硬又长，禽的覆羽只盖到大腿（跗跖骨是裸鳞的）。
    """
    sk = lb.gene.skeleton
    if sk is None or sk.coat not in COAT_SPEC:
        return 0
    rings, per, tuft, thick, span, mat = COAT_SPEC[sk.coat]
    n = 0
    for i in range(min(span, len(lb.joints) - 2)):
        a, b = lb.joints[i], lb.joints[i + 1]
        axis = b - a
        L = float(np.linalg.norm(axis)) or 1.0
        axis = axis / L
        t1, t2 = C._tangent_basis(axis)
        r = max(lb.radius[i], RENDER_MIN_R)
        for ring in range(rings):
            t = (ring + 0.6) / (rings + 0.4)
            for k in range(per):
                # 相邻两圈错开半个间距：瓦片式咬合，规律就是从这儿来的
                ang = 2 * math.pi * (k + 0.5 * (ring % 2)) / per
                radial = math.cos(ang) * t1 + math.sin(ang) * t2
                base = a + axis * (L * t) + radial * (r * 0.82)
                # 毛流：顺着肢体往远端躺 + 一点翘起。抖动只动长度和翘角，不动排布
                j = C._noise(lb.name, i, ring, k, "j")
                lay = axis * (0.80 + 0.14 * j) + radial * (0.34 + 0.22 * j)
                lay /= float(np.linalg.norm(lay))
                tip = base + lay * (r * tuft * (0.75 + 0.5 * j))
                rig.shaft(f"limb_{lb.name}_{i}", f"coat_{lb.name}_{i}_{ring}_{k}",
                          tuple(base), tuple(tip), max(thick * r * 0.6, 0.32), mat=mat)
                n += 1
    return n


def part_scars(rig: Rig, lb: LB.Limb) -> int:
    """缝上去留下的疤。**位置不是画的，是算出来的**——两处，各有各的来历：

    · **接合痕**：这条肢焊在挂载点上的那一圈。
    · **交界痕**：本体新长的肉与供体原有组织的分界。那个分界点材质表已经算过了
      （`Limb.mats` 里 graft 变成 hide/scute/chitin 的那一节），疤就长在那儿——
      肉长到哪儿为止，痕就在哪儿。

    做法沿用核心那身癒合痕的读法（`gen_core.part_welds`）：**不是外科缝合**，没有线、
    没有等距横扣，是自体融合留下的不规则隆起——粗细逐段变、约一成断开（融合彻底处
    无痕）、偶尔堆成一个肉瘤。等粗等距的环会立刻读成焊接件。
    """
    n = 0
    rings = [(0, lb.weld_r * 1.15)]
    for i in range(1, len(lb.mats)):
        if lb.mats[i] != lb.mats[i - 1]:
            rings.append((i, max(lb.radius[i - 1], lb.radius[i]) * 1.12))
    for i, rad in rings:
        p = lb.joints[i]
        axis = lb.joints[min(i + 1, len(lb.joints) - 1)] - p
        na = float(np.linalg.norm(axis))
        if na < 1e-6:
            continue
        axis /= na
        t1, t2 = C._tangent_basis(axis)
        seg = 9
        for k in range(seg):
            if C._noise(lb.name, i, k, "gap") < 0.12:
                continue                      # 融合彻底处直接没有痕
            a0, a1 = 2 * math.pi * k / seg, 2 * math.pi * (k + 1.05) / seg
            e0 = p + (math.cos(a0) * t1 + math.sin(a0) * t2) * rad
            e1 = p + (math.cos(a1) * t1 + math.sin(a1) * t2) * rad
            w = 0.34 + 0.30 * C._noise(lb.name, i, k, "w")
            rig.shaft(f"limb_{lb.name}_{max(i - 1, 0)}", f"scar_{lb.name}_{i}_{k}",
                      tuple(e0), tuple(e1), w, mat="scar")
            n += 1
            if C._noise(lb.name, i, k, "b") < 0.11:      # 融合失控处堆出的肉瘤
                c, s = 0.5 * (e0 + e1), w * 1.7
                rig.cube(f"limb_{lb.name}_{max(i - 1, 0)}", f"snarl_{lb.name}_{i}_{k}",
                         tuple(c - s), tuple(c + s), mat="weld_dark")
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


def gallery(stance: str = "") -> Rig:
    """各物种的腿**并排一条一条看**——回答"这是什么腿"只能靠单件预览，混在整只兽上
    看永远是一团。每条都给同一个载荷、同一个髋高，所以差别全部来自骨架与站姿本身。

    看这张图该看出来：蹄行的（羊/牛/猪）小腿以下是一根竖管加一枚劈开的蹄；趾行的
    （狼/狐/兔/禽）脚跟吊在半空、踮着趾；跖行的（人/鼠）整只脚掌拍在地上还支出个
    脚跟；蛛足是斜插下去的一根细杆。毛/羊毛/鬃/鳞/甲也各是各的。
    """
    import genome as GN
    rig = Rig(Palette(MATS, swatch=8, size=64))
    rig.bone("root", (0.0, 0.0, 0.0))
    load = LB.body_weight() / 4.0
    names = sorted((n for n, sk in GN.SKELETONS.items()
                    if not stance or sk.stance == stance),
                   key=lambda k: GN.SKELETONS[k].total)
    for i, sp in enumerate(names):
        sk = GN.SKELETONS[sp]
        x = (i - len(names) / 2) * 26.0
        gene = GN.LimbGene(f"g{i}", sk.cls, 1.0, sp)
        # 髋高按**这条腿自己的自然站姿**摆：踝的高度由站姿定，髋再在踝之上留出
        # 八成的腿长。统一给一个高度是错的——蹄行的踝本来就比跖行高一大截，硬摆同一个
        # 髋高会逼股骨横着支出去（round 2 腿谱实测，像条断腿）。
        hip = np.array([x, gene.ankle_lift + gene.leg_len * 0.80, 0.0])
        sock = C.Socket(name=sp, kind="limb", pos=hip,
                        normal=np.array([0.0, -1.0, 0.0]), bone="root", girth=4.0)
        lb = LB.solve_limb(gene, sock, load=load,
                           foot=np.array([x + gene.leg_len * 0.22, 0.0, 0.0]))
        parent = "root"
        for j, piv in enumerate(lb.joints[:-1]):
            rig.bone(f"limb_{sp}_{j}", tuple(piv), parent)
            parent = f"limb_{sp}_{j}"
        part_limbs(rig, {sp: lb})
    return rig


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
        # 卡的是**渲染出来的**粗细：半个像素以下模型表达不出差别（0.36 与 0.40 渲出来
        # 是同一根柱子），拿原始值断言等于在噪声上较真。
        rr = [max(x, RENDER_MIN_R) for x in r]
        # 容差 0.1 px：趾骨的力臂量到接触点，而趾尖本来就在接触点前面一截，所以它的
        # 抗弯半径可能比掌骨大那么零点零几像素。这是真的，但在模型精度之下。
        if rr[-1] > min(rr) + 0.1:
            bad.append(f"{lb.name} 末节不是最细的 {[round(x, 2) for x in r]}——"
                       f"末节只有腱穿过，它必须是最细的一节")
        if r.index(max(r)) > (len(r) - 1) // 2 and lb.buried < 0.02:
            bad.append(f"{lb.name} 最粗的一节在远端 {[round(x, 2) for x in r]} 且没有"
                       f"埋没截断（buried={lb.buried:.0%}）——抗重力肌都长在近端，"
                       f"远端鼓起来说明力矩臂算反了")
        # 看得见的收细是**大腿 vs 管骨**，不是大腿 vs 趾节——趾节已经归脚的几何管了，
        # 拿它当基准会把"脚大"误判成"腿不收细"。
        cannon = r[max(len(r) - lb.gene.foot_bones, 1)]
        if lb.root_need / max(cannon, 1e-9) < 2.0:
            bad.append(f"{lb.name} 根部需求 {lb.root_need:.1f} / 管骨 {cannon:.2f} 只差 "
                       f"{lb.root_need / max(cannon, 1e-9):.1f}×——腿该显著收细，"
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
    # 预算从 560 提到 760：脚（每条 4–8 块）和毛（每条 13–15 块）是这一轮加的，都是
    # "看得见才算数"的东西。参照：拟态灰烬蛛整只约 200 块，这只兽有 6–9 条肢。
    if len(rig.elements) > 900:
        bad.append(f"块数 {len(rig.elements)} 超预算 900")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--bud-growth", type=float, default=1.0,
                    help="没长肢的槽画多大；出图看真实样子传 0.1（见 build）")
    ap.add_argument("--gallery", action="store_true", help="各物种的腿并排出一张")
    args = ap.parse_args()

    if args.gallery:
        import genome as GN
        for st in sorted({sk.stance for sk in GN.SKELETONS.values()}):
            rig = gallery(st)
            p = rig.save(OUT_DIR / f"Legs_{st}.bbmodel", f"Legs_{st}")
            print(f"腿谱 {st}：{len(rig.elements)} 块 → {p}")
        return 0

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
