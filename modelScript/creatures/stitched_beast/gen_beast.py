#!/usr/bin/env python3
"""异变缝合兽 —— 整只兽：核心 + 按基因组长出来的肢体 → .bbmodel + 自检。

`gen_core.py` 出的是**无肢阶段**那团核心（每个挂载点上只有一个芽）。这里出的是长齐了
肢体之后的样子：基因组挑中的槽上，芽已经长成部件，几何由 `limbs.py` 从体重和站姿推出来；
没被挑中的槽仍然只是一个芽——那些是它还没捡到料的位置。

**同一个 seed 必得同一只兽**：槽的选择、供体、尺寸、步态相位全部由 seed 决定，几何再
由它们唯一确定。出了问题能重放同一只。

用法:
  python3 modelScript/creatures/stitched_beast/gen_beast.py --seed 7
  python3 modelScript/creatures/stitched_beast/gen_beast.py --seed 7 --check
"""

from __future__ import annotations

import argparse
import copy
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
sys.path.insert(0, str(HERE))

import core as C  # noqa: E402
import gen_core as GC  # noqa: E402
import genome as GN  # noqa: E402
import heads as HD  # noqa: E402
import limbs as LB  # noqa: E402
from bbmodel_maker.rig.voxel_rig import Palette, Rig  # noqa: E402

OUT_DIR = HERE.parents[2] / "models" / "stitched_beast"

MATS = dict(GC.MATS)
MATS.update(LB.LIMB_MATS)
MATS.update(HD.HEAD_MATS)

# 可见半径的**渲染**下限（px）。远端那一节按力学算出来只有零点二三像素——它本来就该是
# 一根腱包着的细骨（马的管骨就是这样），但 MC 的模型精度到不了亚像素，半个像素以下的
# 柱子渲出来会消失或闪烁。这是渲染的界不是物理的界，和 core_anim.FLICK_MAX_HZ 同类。
RENDER_MIN_R = 0.5


HEAD_BONE = {"skull": "head", "jaw": "jaw",
             "ear_l": "earl", "ear_r": "earr",
             "horn_l": "hornl", "horn_r": "hornr"}


def head_bone(hd: HD.Head, part: str) -> str:
    return f"{HEAD_BONE[part]}_{hd.name}"


def bone_tree(rig: Rig, limbs: dict[str, LB.Limb],
              heads: dict[str, HD.Head] | None = None) -> None:
    """core 的骨树 + 逐肢骨链 + 逐头骨链。

    肢骨挂在该槽骑着的那根核心骨上（`sock.bone`），不是一律挂 core_mid——挂错父骨的话
    核心一分节，肢就会被甩出去（核心动画那边踩过）。每节一根骨，pivot 取该节的近端
    关节：将来步态动画驱动的就是这条链。

    头也一样：**下颌、耳、角各是独立的骨**。这不是为了好看——嚼、耳朵抖、顶角是三套
    互不相干的动作，合成一根骨就永远做不了。下颌的 pivot 必须落在颌关节上（推出来的
    那个 `Head.tmj`），落错地方张嘴时下巴会平移而不是转。
    """
    heads = heads or {}
    rig.bone("root", (0.0, 0.0, 0.0))
    for lb in C.LOBES:
        rig.bone(lb.name, tuple(np.array(lb.center) + C.CORE_CENTER), lb.parent or "root")
    for hd in heads.values():
        rig.bone(head_bone(hd, "skull"), tuple(hd.org), hd.sock.bone)
        rig.bone(head_bone(hd, "jaw"),
                 tuple(hd.world((0.0, hd.tmj[0], hd.tmj[1]))), head_bone(hd, "skull"))
        for part in ("ear_l", "ear_r", "horn_l", "horn_r"):
            base = next((p.a for p in hd.pieces if p.part == part), None)
            if base is not None:
                rig.bone(head_bone(hd, part), tuple(hd.world(base)),
                         head_bone(hd, "skull"))
    for s in C.sockets().values():
        limb = limbs.get(s.name)
        if limb is None:
            if s.name in heads:      # 头的骨在上面建过了，这个槽不再是芽
                continue
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
    """逐肢几何：连续包络的肉 + 脚掌 + 体表 + 疤 + 裂口。"""
    n = 0
    for lb in limbs.values():
        s = lb.sock
        base = f"limb_{s.name}_0"
        for i, (p, q) in enumerate(zip(lb.joints, lb.joints[1:])):
            # 沿着这一节把**连续包络**（`Limb.profile`）采样成几段柱子。上一版是每节
            # 一根等粗柱（或按 SPINDLE 分三段），于是关节处粗细直接跳变：大腿 4.8 px
            # 接上小腿 0.5 px，中间没有任何过渡，读出来是"一坨肉后面插了根棍"。
            #
            # 分几段是**分辨率**问题，不是造型问题：段数取到每段长度 ≈ 2 px 为止，
            # 再细也超过模型精度。但要防住 round 2 那个坑——一节粗短的腿分成好几段，
            # 每段比它自己还宽，渲出来是一摞板子。所以段数还要被长细比压住。
            L = float(np.linalg.norm(q - p))
            rmax = max(lb.profile(i, k / 8.0) for k in range(9))
            k_res = int(L / 2.0)                       # 分辨率给的段数
            k_flat = int(L / max(1.2 * rmax, 1e-6))    # 长细比给的上限（别摞成板）
            steps = max(1, min(6, k_res, max(k_flat, 1)))
            for k in range(steps):
                a = p + (q - p) * (k / steps)
                b = p + (q - p) * ((k + 1) / steps)
                r = max(lb.profile(i, (k + 0.5) / steps), RENDER_MIN_R)
                rig.shaft(f"limb_{s.name}_{i}", f"seg_{s.name}_{i}_{k}",
                          tuple(a), tuple(b), r, mat=lb.mats[i])
                n += 1
        if lb.bearing:
            n += part_foot(rig, lb)
        n += part_coat(rig, lb)
        n += part_scars(rig, lb)
        n += part_cracks(rig, lb)
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
        # 人足：**跟 → 跖球 一整片贴地的脚底** + 托住踝的跗骨块 + 五枚趾。
        #
        # 上一版把整只脚画在了踝的**前面**：链上只有 [踝, 跖球, 趾尖]，没有跟，代码
        # 又把 `j[-2]` 当成跟来用，于是脚底从跖球起步、一路铺到趾尖之外。实测那条腿
        # 踝的地面投影在 z=+1.75、地面反力作用点在 z=0，而画出来的脚底是 −2.95…−7.55
        # ——连受力点都没盖住，看上去像"用前脚掌的位置支着一根杆"。跟不在链上是对的
        # （跟骨和掌骨是同一块刚体，不是一节），但渲染必须另外把它取回来。
        ankle, ball, tip = j[-3], j[-2], j[-1]
        heel = lb.heel if lb.heel is not None else ball
        fwd = (ball - heel) * np.array([1.0, 0.0, 1.0])
        run = max(float(np.linalg.norm(fwd)), 1.0)
        u = fwd / run
        # 脚底的**长**由解剖定（跟到跖球），**宽**取"解剖宽"与"压强要求的宽"中的大者
        # ——人足本来就比踩住所需的面积大，多出来的那部分只是压强更低而已。
        wide = max(hw, run * 0.19)
        mid = 0.5 * (heel + ball)
        box(bone, "sole", (mid[0], 0.7, mid[2]), (wide, 0.7, run * 0.5), "hide")
        # 跗骨块：踝正下方那一坨，把胫骨接到脚底上。没有它，踝和脚底之间隔着一段空的，
        # 整条腿看着像插在脚上而不是站在脚上。
        back = 0.5 * (heel + ankle)
        top = max(float(ankle[1]), 1.4)
        box(prev, "tarsal", (back[0], top * 0.5, back[2]),
            (wide * 0.82, top * 0.5,
             float(np.linalg.norm((ankle - heel)[[0, 2]])) * 0.5 + wide * 0.30), "hide")
        # 五趾：(横向位置, 趾粗, 趾长占比)。拇趾在内侧最粗，**第二趾最长**（顶到趾尖
        # 那个关节），往小趾递减——这几个比例是量的不是编的。
        toe_run = max(float(np.linalg.norm((tip - ball)[[0, 2]])), 1.0)
        for off, thick, frac in ((-0.78, 0.26, 0.81), (-0.32, 0.17, 1.00),
                                 (0.06, 0.16, 0.90), (0.40, 0.15, 0.76),
                                 (0.70, 0.13, 0.60)):
            c = ball + u * (toe_run * frac * 0.5)
            box(bone, "toe", (c[0] + off * wide, 0.6, c[2]),
                (wide * thick, 0.6, toe_run * frac * 0.5), "hide")
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


def crack_lines(lb: LB.Limb) -> list[tuple[str, np.ndarray, np.ndarray, np.ndarray]]:
    """算出这条肢上每一道裂口：(种类, 端点a, 端点b, 当地的参考轴)。

    单独拎出来是为了让自检能直接量方向（见 check ⑫）——渲染和判据读同一份数据，
    不是各算各的。
    """
    out: list[tuple[str, np.ndarray, np.ndarray, np.ndarray]] = []
    axes = [lb.joints[i + 1] - lb.joints[i] for i in range(len(lb.joints) - 1)]

    for i in range(1, len(axes)):
        up, dn = axes[i - 1], axes[i]
        nu, nd = float(np.linalg.norm(up)), float(np.linalg.norm(dn))
        if nu < 1e-6 or nd < 1e-6:
            continue
        up, dn = up / nu, dn / nd
        bend = math.degrees(math.acos(float(np.clip(np.dot(up, dn), -1.0, 1.0))))
        if bend < 12.0:
            continue                        # 几乎不折的关节，皮不需要多余的余量
        outward = up - dn                   # 两节轴之差指向弯心，取反就是伸侧
        no = float(np.linalg.norm(outward))
        if no < 1e-6:
            continue
        outward = -outward / no
        ring = np.cross(dn, outward)        # 横裂沿这个方向躺（垂直于肢轴）
        nr = float(np.linalg.norm(ring))
        if nr < 1e-6:
            continue
        ring /= nr
        r = lb.node_r[i]
        for k in range(min(3, 1 + int(bend / 30.0))):
            off = (k - 1) * 0.9 + 0.4 * C._noise(lb.name, i, k, "ck")
            ctr = lb.joints[i] + dn * off + outward * (r * 0.86)
            half = r * (0.55 + 0.35 * C._noise(lb.name, i, k, "cl"))
            if half < 0.25:
                continue                    # 比模型精度还短的缝，画不出来
            out.append(("joint", ctr - ring * half, ctr + ring * half, dn))

    sk = lb.gene.skeleton
    if sk is not None and lb.bearing and sk.foot in ("cloven", "claw"):
        hw, hl = lb.pad
        ball = lb.joints[-2]
        for k, sgn in enumerate((-1.0, 1.0)):
            if C._noise(lb.name, "hoof", k, "g") < 0.35:
                continue                    # 不是每一片都裂
            x = ball[0] + sgn * hw * (0.5 if sk.foot == "cloven" else 0.35)
            z = ball[2] + (0.35 - 0.7 * C._noise(lb.name, "hoof", k, "z")) * hl
            top = hl * (0.5 + 0.35 * C._noise(lb.name, "hoof", k, "h"))
            out.append(("keratin", np.array([x, 0.05, z]), np.array([x, top, z]),
                        np.array([0.0, 1.0, 0.0])))
    return out


def part_cracks(rig: Rig, lb: LB.Limb) -> int:
    """裂口。**位置和方向都是拉出来的**——裂缝一律垂直于把它拉开的那个方向。

    皮在这个尺度上不到十分之一个像素（1 px = 6.25 cm），所以"装上皮肤"不可能表现成
    加粗，只能表现成**表面**。表面上唯一看得见的结构就是它裂在哪儿、朝哪个方向裂。
    两处，方向正好互相垂直，这是可以核验的：

    · **关节的横裂。** 关节要屈伸，外侧（伸侧）的皮必须跟着这段弧长伸缩，是全肢应变
      最大的一圈。拉伸方向沿着肢的轴，裂缝就横着开——大象、犀牛的关节皱、人的指节
      纹都是横的。折得越死裂得越多：条数直接取自这个关节在站姿里的实际折角。
    · **角质的纵裂。** 蹄/爪/甲是管状的角质，失水收缩加上着地时管壁向外张，拉的是
      **周向**，于是裂缝顺着生长方向竖着开——马蹄的裂蹄（quarter crack）就是竖的。

    所以同一条腿上两种裂缝互相垂直。这不是随手撒的纹理，撒歪了自检会红（见 ⑫）。
    """
    last = len(lb.joints) - 2
    lines = crack_lines(lb)
    for k, (kind, a, b, _ref) in enumerate(lines):
        # 挂在**裂口所在那一节**的骨上，腿一动裂口跟着动
        i = last if kind == "keratin" else max(
            min(int(np.argmin([np.linalg.norm(a - p) for p in lb.joints])) - 1,
                last), 0)
        rig.shaft(f"limb_{lb.name}_{i}", f"crack_{lb.name}_{kind}_{k}",
                  tuple(a), tuple(b), 0.3 if kind == "joint" else 0.28, mat="crack")
    return len(lines)


def part_heads(rig: Rig, heads: dict[str, HD.Head]) -> int:
    """逐头几何。**这一层不做任何决定**——摆什么、多大全在 `heads.solve_head` 里推完了，
    这里只把局部坐标搬到世界坐标、按 part 分到对应的骨上。

    每块都走 `Rig.shaft`：头的标架是零滚转的（`heads.head_frame`），而 shaft 解出来的
    方块宽度轴恒水平——两者正好对上，所以任意朝向的头都能用它表达，不需要自己写旋转。
    """
    n = 0
    for hd in heads.values():
        for p in hd.pieces:
            a, b = hd.world(p.a), hd.world(p.b)
            d = float(np.linalg.norm(b - a))
            if d < 1e-3:                       # 退化成一点的块：给它一丝厚度再画
                b = a + hd.e_f * 1e-2
            rig.shaft(head_bone(hd, p.part), f"{p.name}_{hd.name}",
                      tuple(a), tuple(b), max(p.r1, HD.RENDER_MIN),
                      max(p.r2, HD.RENDER_MIN), mat=p.mat)
            n += 1
    return n


def head_sweep(gen, gait, limbs, socks, k: int = 40) -> float:
    """腿在**整个步态周期**里扫过头的最深一处（px）。给 `limbs.build` 的否决钩子。

    肢体层看不见头——头是这一层解的，而腿摆起来会扫进头里：seed 9 实测一条腿从一颗头的
    眼球里穿过去 1.52 px，静止姿完全不碰。判据与静态自检 ⑯ 共用 `overlaps`（含那条按
    "离表皮多远"算的癒合豁免），只是把肢换成逐相位重折之后的姿态。

    只比**含头的那些对**：肢与肢之间由肢体层自己那道门管，这里再算一遍是白工。
    """
    heads = HD.separate({hg.socket: HD.solve_head(hg, socks[hg.socket])
                         for hg in gen.heads})
    if not heads:
        return 0.0
    lgs = {lg.gene.socket: lg for lg in gait.limbs}
    sweeps = {n: LB.cycle_caps(lb, lgs.get(n), k) for n, lb in limbs.items()}
    worst = 0.0
    for i in range(k):
        moved = {}
        for n, lb in limbs.items():
            caps = sweeps[n][i % len(sweeps[n])]
            clone = copy.copy(lb)
            clone.joints = [caps[0][0]] + [c[1] for c in caps]
            moved[n] = clone
        for _na, _nb, d, _wa, _wb in overlaps(moved, heads, tol=0.0):
            if _na.startswith("头") or _nb.startswith("头"):
                worst = max(worst, d)
    return worst


def build(seed: int, *, bud_growth: float = 1.0
          ) -> tuple[Rig, LB.LM.Gait, dict[str, LB.Limb], dict[str, HD.Head]]:
    """`bud_growth` 只影响**没长肢的那些槽**画多大。

    默认 1.0 是几何契约：芽按满尺寸建，当前生长度由动画的 bone scale 表达（见
    `gen_core.part_buds`）。想看这只兽平时的样子，出图时传 `core_anim.BUD_DORMANT`
    ——否则十几个满长的芽会把整只兽埋掉，腿一条都看不见（round 1 实测）。
    """
    socks = C.sockets()
    gen, gait, limbs = LB.build(seed, socks=socks,
                                veto=lambda g, ga, lm: head_sweep(g, ga, lm, socks))
    heads = HD.separate({hg.socket: HD.solve_head(hg, socks[hg.socket])
                         for hg in gen.heads})
    rig = Rig(Palette(MATS, swatch=8, size=64))
    bone_tree(rig, limbs, heads)
    GC.part_mass(rig)
    GC.part_welds(rig)
    GC.part_drips(rig)
    for s in socks.values():                 # 没长肢也没长头的槽仍然只是一个芽
        if s.name in limbs or s.name in heads:
            continue
        for k, (ctr, r, mat) in enumerate(C.bud_shape(s, bud_growth)):
            bone = f"bud_{s.name}" if k == 0 else f"bud_{s.name}_{k}"
            rig.cube(bone, f"budc_{s.name}_{k}", tuple(ctr - r), tuple(ctr + r), mat=mat)
    part_limbs(rig, limbs)
    part_heads(rig, heads)
    return rig, gait, limbs, heads


def gallery(stance: str = "") -> Rig:
    """各物种的腿**并排一条一条看**——回答"这是什么腿"只能靠单件预览，混在整只兽上
    看永远是一团。每条都给同一个载荷、同一个髋高，所以差别全部来自骨架与站姿本身。

    看这张图该看出来：蹄行的（羊/牛/猪）小腿以下是一根竖管加一枚劈开的蹄；趾行的
    （狼/狐/兔/禽）脚跟吊在半空、踮着趾；跖行的（人/鼠）整只脚掌拍在地上还支出个
    脚跟；蛛足是斜插下去的一根细杆。毛/羊毛/鬃/鳞/甲也各是各的。
    """
    rig = Rig(Palette(MATS, swatch=8, size=64))
    rig.bone("root", (0.0, 0.0, 0.0))
    load = LB.body_weight() / 4.0
    names = sorted((n for n, sk in GN.SKELETONS.items()
                    if not stance or sk.stance == stance),
                   key=lambda k: GN.SKELETONS[k].total)
    for i, sp in enumerate(names):
        sk = GN.SKELETONS[sp]
        # 整排**挪离核心**：姿态求解会躲开核心的场（那是对的，腿不该折进肚子里），
        # 而腿谱里核心并不渲出来。原来第一条腿的髋正好落在 (0, 24, 0) 附近，也就是
        # 核心内部，于是它的膝被一团看不见的东西顶到反方向去——同为兽腿，狼的膝朝前
        # 人的膝朝后。腿谱要看的是骨架本身，不该带上这份偏置。
        x = (i - len(names) / 2) * 26.0 + 200.0
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


def head_rig(names: list[str], *, pitch: float = 26.0, per_row: int = 5) -> Rig:
    """把若干供体的头摆成方阵（`pitch` = 间距 px，`per_row` = 每行几颗）。

    只给一个名字就是单件预览——头这么小，混排的三视图里根本看不清哪是哪，得一颗一颗过。
    十颗排成一条长队同样看不清：相机要框住 10:1 的长条，每颗只剩十分之一的画幅。分行。
    """
    rig = Rig(Palette(MATS, swatch=8, size=64))
    rig.bone("root", (0.0, 0.0, 0.0))
    heads = {}
    for i, sp in enumerate(names):
        col, row = i % per_row, i // per_row
        x = (col - min(per_row, len(names)) / 2) * pitch
        z = 12.0 + row * pitch * 1.6
        sock = C.Socket(name=sp, kind="head", pos=np.array([x, 20.0, z]),
                        normal=np.array([0.0, 0.0, -1.0]), bone="root", girth=4.0)
        heads[sp] = HD.solve_head(GN.HeadGene(sp, sp, 1.0), sock)
    for hd in heads.values():
        rig.bone(head_bone(hd, "skull"), tuple(hd.org), "root")
        rig.bone(head_bone(hd, "jaw"),
                 tuple(hd.world((0.0, hd.tmj[0], hd.tmj[1]))), head_bone(hd, "skull"))
        for part in ("ear_l", "ear_r", "horn_l", "horn_r"):
            base = next((p.a for p in hd.pieces if p.part == part), None)
            if base is not None:
                rig.bone(head_bone(hd, part), tuple(hd.world(base)),
                         head_bone(hd, "skull"))
    part_heads(rig, heads)
    return rig


def head_gallery() -> Rig:
    """十种供体的头**并排一颗一颗看**。和腿谱同一个理由：混在整只兽上永远看成一团。

    每颗都朝正前方、挂同一个虚拟槽，所以差别全部来自供体的食性与体型本身。该看出来的：
    食肉的短吻、颌关节压在齿列平面上、颅顶有矢状嵴；食草的长脸、关节抬得老高、颧弓
    往外撑出一对大腮帮子；啮齿的门齿凿；禽的喙；蛙的眼睛长在头顶。
    """
    return head_rig(sorted(GN.HEAD_TEMPLATES, key=lambda k: HD.DONOR[k].head_m))


# ---------------------------------------------------------------- 碰撞
def capsules(limbs, heads):
    """把渲染出来的体近似成胶囊，按**部件**分组。

    分组的粒度是"一条肢 / 一颗头"，不是一块几何——同一条肢内部相邻两节当然会接触，
    那不是穿模。每组带一个癒合区（原点 + 半径）：根部的肉长在一起是对的。
    """
    out = []
    for n, lb in limbs.items():
        caps = []
        for i, (p, q) in enumerate(zip(lb.joints, lb.joints[1:])):
            r = max(lb.profile(i, 0.5), RENDER_MIN_R)
            caps.append((np.asarray(p, float), np.asarray(q, float), r, f"seg{i}"))
        out.append((f"肢 {n}", caps, np.asarray(lb.sock.pos, float), lb.root_r * 1.6))
    for n, hd in heads.items():
        out.append((f"头 {n}", HD.head_capsules(hd), np.asarray(hd.org, float),
                    max(hd.brain_px) * 0.7))
    return out


def overlaps(limbs, heads, *, tol: float = 0.75):
    """所有跨部件的深度互穿。`tol` 取 0.75 px——四分之三格，肉眼一定看得见。"""
    items = capsules(limbs, heads)
    bad = []
    for i in range(len(items)):
        na, ca, oa, ma = items[i]
        for j in range(i + 1, len(items)):
            nb, cb, ob, mb = items[j]
            worst, wa, wb = 0.0, "", ""
            for (a0, a1, ra, an) in ca:
                for (b0, b1, rb, bn) in cb:
                    dd, mid = HD.seg_dist(a0, a1, b0, b1)
                    ov = ra + rb - dd
                    if ov <= worst:
                        continue
                    # **癒合区豁免按"离表皮多远"算，不按根部半径算。**
                    #
                    # 这东西是几团肉长到一起的：贴着躯干那一圈，两条肢的肉本来就该连成
                    # 一片。但"贴着"的尺度不是根部半径——肌腹被埋进体腔的肢根部反而最细
                    # （腹肢实测根粗 1.0 而中段 3.2），拿根粗当尺度会把真正的融合判成穿模。
                    #
                    # 尺度应该是**那一处两块肉的厚度**：接触点离表皮不到 ra+rb，说明两边
                    # 的肉都还搭在躯干上，那是融合。实测两类的差别极干净——一对的接触点
                    # 离表皮 +1.38 px 而两半径和 4.38（融合），另一对 +25.88 px（半空中
                    # 交叉，货真价实的穿模）。
                    q = mid - C.CORE_CENTER
                    g = float(np.linalg.norm(C.grad(q)))
                    if (C.ISO - C.fld(q)) / max(g, 1e-6) < ra + rb:
                        continue
                    worst, wa, wb = ov, an, bn
            if worst > tol:
                bad.append((na, nb, worst, wa, wb))
    bad.sort(key=lambda x: -x[2])
    return bad


# ---------------------------------------------------------------- 自检
def check(rig: Rig, gait, limbs: dict[str, LB.Limb],
          heads: dict[str, HD.Head] | None = None) -> list[str]:
    bad: list[str] = []
    heads = heads or {}
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
        #
        # 两边都得是**力学要的粗细**，不能拿渲染半径当分母：管骨那一节的渲染半径经常由
        # 软组织下限（`SOFT_OVER_BONE`）顶着，那是"骨外面总还有一层"，不是力矩算出来的。
        # 拿它作分母，轻载的腿会被误判——实测四只兽的低载肢 root_need 1.4 / 渲染 0.90
        # 报"只差 1.6×"，而它们的力学需求其实差 3.1×。管骨那一节没有肌腹，力学需求就是
        # 骨半径本身。
        idx = max(len(r) - lb.gene.foot_bones, 1)
        cannon = lb.bone_r[idx]
        if lb.root_need / max(cannon, 1e-9) < 2.0:
            bad.append(f"{lb.name} 根部需求 {lb.root_need:.1f} / 管骨需求 {cannon:.2f} "
                       f"只差 {lb.root_need / max(cannon, 1e-9):.1f}×——腿该显著收细，"
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

    # ⑪ 不许把骨头画在外面，也不许在关节处跳粗细。用户一眼就看出来的那两条：
    #    "没安装上肌肉和皮肤"——远端几节的渲染半径**恰好等于骨半径**，等于露骨；
    #    大腿 4.8 px 直接接上小腿 0.5 px——肉是连续的，不可能在关节处断一截。
    for lb in limbs.values():
        for i in range(len(lb.radius)):
            floor = lb.bone_r[i] * LB.SOFT_OVER_BONE
            thin = min(lb.profile(i, k / 10.0) for k in range(11))
            if thin < floor - 1e-6:
                bad.append(f"{lb.name} 第 {i} 节最细处 {thin:.2f} px 已经贴到骨面 "
                           f"（骨半径 {lb.bone_r[i]:.2f}×{LB.SOFT_OVER_BONE}）——露骨了")
            if i + 1 < len(lb.radius):
                jump = abs(lb.profile(i, 1.0) - lb.profile(i + 1, 0.0))
                if jump > 1e-6:
                    bad.append(f"{lb.name} 第 {i}/{i + 1} 节交界处粗细跳了 {jump:.2f} px"
                               f"——关节两边必须是同一个包络")

    # ⑬ 跖行的脚必须**托住踝**：踝的地面投影落在跟与跖球之间。"跖行"这个词的定义就是
    #    整片脚底承重、踝压在脚底上方；踝投影跑到脚底外面，那条腿就是踩着前脚掌站的。
    #    上一版实测：踝投影 z=+1.75，而脚底从 −2.95 起——整只脚画在了踝的前面。
    for lb in limbs.values():
        if not lb.bearing or lb.gene.stance != "plantigrade":
            continue
        if lb.heel is None:
            bad.append(f"{lb.name} 是跖行却没有跟——脚跟不在链上，得单独取（foot_heel）")
            continue
        ankle, ball = lb.joints[-3], lb.joints[-2]
        seg = (ball - lb.heel)[[0, 2]]
        L2 = float(seg @ seg)
        s = float((ankle - lb.heel)[[0, 2]] @ seg) / max(L2, 1e-9)
        if not -0.02 <= s <= 1.02:
            bad.append(f"{lb.name} 踝的地面投影落在脚底之外（跟→跖球的 {s:+.2f} 处）"
                       f"——跖行必须整片脚底托住踝")

    # ⑭ 折弯平面：兽腿禽腿的膝是**横轴铰链**，只能前后折。脚落到侧面去靠的是髋把整条腿
    #    外展出去，膝仍旧朝前。所以膝相对"髋→踝"连线的偏移必须**主要是前后向**，而且
    #    朝前（除非这条肢被迫改了折向）。上一版把折弯平面取成"髋与脚的竖直面"，脚一落
    #    在侧面膝就朝侧面折——用户看单件图一眼认出来："大腿怎么在小腿上方的侧面？"
    for lb in limbs.values():
        if not lb.bearing or lb.gene.kind == "spider" or len(lb.joints) < 3:
            continue
        h, k, a = lb.joints[0], lb.joints[1], lb.joints[2]
        ax = a - h
        na = float(np.linalg.norm(ax))
        if na < 1e-6:
            continue
        ax = ax / na
        off = (k - h) - float((k - h) @ ax) * ax
        # 卡的是**折弯平面必须含前后方向**，不是"膝的侧向偏移要小"。后者是错的判据：
        # 一条往斜后方伸出去的腿（实测 seed 10 的兔腿 e_d=(+0.60,−0.22,+0.77)），平面
        # 里那条垂线本来就带很大的侧向分量（+4.40），可平面照样含前后方向、膝照样朝前。
        # 按侧向大小去卡会把这条正确的腿判红。
        nrm = np.cross(ax, off)
        nn = float(np.linalg.norm(nrm))
        if nn > 1e-6 and abs(float(nrm / nn @ LB.FWD)) > 0.15:
            bad.append(f"{lb.name} 折弯平面不含前后方向（法向·前后="
                       f"{abs(float(nrm / nn @ LB.FWD)):.2f}）——膝是横轴铰链，"
                       f"只能在含前后的平面里折；脚落到侧面该由髋外展承担")
        elif off[2] > 0.3 and not lb.forced:
            bad.append(f"{lb.name} 膝朝后折（{off[2]:+.2f}）却没标被迫改折向——"
                       f"兽腿禽腿的膝一律朝前，反折的是踝")

    # ⑫ 裂口方向：裂缝一律**垂直于把它拉开的方向**，所以两族互相垂直。
    #    关节的皮被沿肢轴拉 ⇒ 横裂；蹄甲失水沿周向收缩 ⇒ 纵裂。撒歪了这里红。
    for lb in limbs.values():
        for kind, a, b, ref in crack_lines(lb):
            d = b - a
            nd = float(np.linalg.norm(d))
            if nd < 1e-6:
                bad.append(f"{lb.name} 有一道零长的裂口")
                continue
            cos = abs(float(np.dot(d / nd, ref / max(float(np.linalg.norm(ref)), 1e-9))))
            if kind == "joint" and cos > 0.25:
                bad.append(f"{lb.name} 关节裂口和肢轴夹角只有 "
                           f"{math.degrees(math.acos(min(cos, 1.0))):.0f}°——"
                           f"皮是被沿轴拉开的，裂缝必须横着走")
            if kind == "keratin" and cos < 0.9:
                bad.append(f"{lb.name} 蹄甲裂口没顺着生长方向（cos={cos:.2f}）——"
                           f"角质是周向收缩裂的，缝必须竖着走")

    # ⑮ 头不许埋进核心。头是**缝在表皮上**的：枕髁那一圈骑在等值面上（癒合环就画在
    #    那里），但从脑颅往前的每一块都必须在核心之外。埋进去 = 这颗头长反了方向。
    for hd in heads.values():
        for p in hd.pieces:
            for e in (p.a, p.b):
                q = hd.world(e)
                # 癒合区允许骑在表皮上：那一圈头与核心本来就是一团肉。范围取和碰撞检测
                # 同一个尺度（脑颅最大轴的 0.7），别再另定一个"头长的一成"——加上体表
                # 包络之后，咬肌那一块正好落在两者之间，被误判成"头埋进核心"。
                if float(np.linalg.norm(q - hd.org)) < max(hd.brain_px) * 0.7:
                    continue
                f = C.fld(q - C.CORE_CENTER)
                if f >= C.ISO:
                    bad.append(f"{hd.name} 的 {p.name} 埋在核心里（f={f:.2f}）——"
                               f"头是缝在表皮上的，只有枕髁那一圈能骑上去")
                    break
            else:
                continue
            break

    # ⑯ **真正的碰撞检测**：部件与部件之间不许深度互穿。
    #
    #    上一版这一条只比"两个挂载点之间的直线距离"和半个头宽，根本没碰真几何——实测
    #    12 只兽里有 17 处穿插超过半格、最深 4.35 px（腿穿腿）、角穿过另一颗头的癒合环、
    #    耳朵插进另一颗头的脑颅，全部被放过。所以改成对**渲染出来的那些体**逐对求距离。
    #
    #    每一块近似成胶囊（轴 a→b、半径取两个半尺寸的几何平均），两条轴求最短距离，
    #    重叠 = 两半径之和 − 距离。
    #
    #    **癒合区是允许互穿的**：这东西是几团肉长到一起的，根部的肉本来就该连成一片，
    #    所以接触点同时落在两边根部的癒合半径内时不算错。跑到半空中还交叉的才算。
    for gna, gnb, ov, wa, wb in overlaps(limbs, heads):
        bad.append(f"{gna}.{wa} 与 {gnb}.{wb} 互穿 {ov:.2f} px——"
                   f"两者都已离开根部的癒合区，那就是穿模不是长在一起")

    # ⑰ 下颌必须**咬得上**：闭合时下齿列的高度要贴着上齿列（咬合面），不能穿过去，
    #    也不能悬空一截。这条卡的是颌关节的位置——`tmj_lift` 算错了这里立刻红。
    for hd in heads.values():
        u_occ = hd.occ
        low = [p for p in hd.pieces if p.part == "jaw" and p.name.startswith("corpus")]
        # 量的是**骨**不是肉：体表那一层当然会盖过咬合面（那正是唇），扣掉它再比。
        coat = HD.coat_px(hd)
        for p in low[:1]:
            top = max(p.a[1], p.b[1]) + p.r2 - coat
            gap = top - u_occ
            if gap > max(hd.L * 0.06, 0.6):
                bad.append(f"{hd.name} 闭口时下颌体顶面高出咬合面 {gap:.2f} px——"
                           f"下颌穿进上颌了")
            if gap < -max(hd.L * 0.12, 1.0):
                bad.append(f"{hd.name} 闭口时下颌体离咬合面还差 {-gap:.2f} px——"
                           f"这颗头咬不上东西")

    # ⑱ 眼必须看得出去：视轴不能一出眼球就撞回自己的脑袋。侧眼的猎物有一只眼被自己的
    #    身体挡住是**这只兽的属性**（多头正是拿来补视野的，见 HD.vision），不算错；
    #    但视轴穿回自己的头骨是几何错。
    for hd in heads.values():
        for c, dv in zip(hd.eye_pos, hd.eye_dirs()):
            p0 = hd.world(c)
            for t in (0.6, 1.2, 2.0):
                q = p0 + dv * (t * max(hd.eye_r, 0.5) + hd.eye_r)
                loc = np.array([float((q - hd.org) @ hd.e_r),
                                float((q - hd.org) @ hd.e_u),
                                float((q - hd.org) @ hd.e_f)])
                if abs(loc[0]) < hd.pred_W * 0.30 and 0.0 < loc[2] < hd.L * 0.9:
                    bad.append(f"{hd.name} 的视轴一出眼球就穿回自己的头骨"
                               f"（局部 {loc.round(2)}）——眶位算错了")
                    break

    # ⑲ 角基必须扛得住一次对撞。**卡的是储能不是抗弯**——按抗弯算角可以细一半（见
    #    heads.horn_base_r），拿抗弯当判据等于把真正的失效模式漏掉。
    for hd in heads.values():
        if not hd.donor.horn:
            continue
        seg = [p for p in hd.pieces if p.part == "horn_r"]
        if not seg:
            bad.append(f"{hd.name} 有角但一块也没画出来")
            continue
        if hd.horn_r < hd.horn_bend:
            bad.append(f"{hd.name} 角基 {hd.horn_r:.2f} px 连抗弯需要的 "
                       f"{hd.horn_bend:.2f} px 都不到——储能判据反倒比抗弯松，"
                       f"说明其中一条算反了")
        if max(seg[0].r1, seg[0].r2) < hd.horn_r * 0.9:
            bad.append(f"{hd.name} 画出来的角基 {max(seg[0].r1, seg[0].r2):.2f} px "
                       f"细于推出来的 {hd.horn_r:.2f} px")

    # ⑳ 咀嚼肌得装得下：颧弓外张必须至少容下算出来的那块肌肉，否则"腮帮子"是假的。
    for hd in heads.values():
        stand = hd.arch - hd.brain_px[2] / 2.0
        need = (hd.pcsa[1] / 2.0) / max((hd.L - hd.tmj[1]) * 0.3 / hd.px_m, 1e-6) * hd.px_m
        if stand < need * 0.5:
            bad.append(f"{hd.name} 颧弓只外张 {stand:.2f} px，咬肌要 {need:.2f} px"
                       f"——肌肉装不进去，那块腮是画上去的")

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
    ap.add_argument("--heads", action="store_true", help="各供体的头并排出一张")
    ap.add_argument("--head", default="", help="只出某一个供体的头（单件预览）")
    args = ap.parse_args()

    if args.head:
        rig = head_rig([args.head])
        p = rig.save(OUT_DIR / f"Head_{args.head}.bbmodel", f"Head_{args.head}")
        print(f"{args.head} 头：{len(rig.elements)} 块 → {p}")
        return 0

    if args.heads:
        rig = head_gallery()
        p = rig.save(OUT_DIR / "Heads.bbmodel", "Heads")
        print(f"头谱：{len(rig.elements)} 块 → {p}")
        print(HD.report())
        return 0

    if args.gallery:
        for st in sorted({sk.stance for sk in GN.SKELETONS.values()}):
            rig = gallery(st)
            p = rig.save(OUT_DIR / f"Legs_{st}.bbmodel", f"Legs_{st}")
            print(f"腿谱 {st}：{len(rig.elements)} 块 → {p}")
        return 0

    rig, gait, limbs, heads = build(args.seed, bud_growth=args.bud_growth)
    W = LB.body_weight()
    nb = sum(1 for lb in limbs.values() if lb.bearing)
    print(f"缝合兽 seed={args.seed}：{len(rig.elements)} 块 / {len(rig.bones)} 骨 / "
          f"{len(limbs)} 肢（承重 {nb}） / {len(heads)} 头  体重 {W / LB.G:.0f} kg")
    print(LB.report_table(limbs, W))
    print(HD.report(heads))
    cov, blind = HD.vision(heads)
    print(f"[视野] 合眼水平覆盖 {cov:.0f}°，最大连续盲区 {blind:.0f}°")
    lo, hi = rig.bounds()
    print(f"整体 {(hi[0] - lo[0]) / 16:.2f} × {(hi[1] - lo[1]) / 16:.2f} × "
          f"{(hi[2] - lo[2]) / 16:.2f} 格   骑乘 {gait.ride:+.1f} px   "
          f"行走 {gait.blocks_per_sec:.2f} 格/s")

    bad = check(rig, gait, limbs, heads)
    if bad:
        print(f"\n✗ {len(bad)} 处问题：")
        for x in bad:
            print(f"   {x}")
        return 1
    print("✓ 触地 / 不埋进核心 / 根部不互穿 / 粗细单调 / 载荷守恒 / 不陷地 / 骨骼 / "
          "站高 / 不对称 / 不露骨 / 脚托住踝 / 膝朝前 / 裂口方向 / 头不埋 / 头不互穿 / "
          "咬得上 / 看得出去 / 角扛得住 / 咬肌装得下 / 块数 全部通过")
    if not args.check:
        p = rig.save(OUT_DIR / f"StitchedBeast_{args.seed}.bbmodel",
                     f"StitchedBeast_{args.seed}")
        print(f"→ {p}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
