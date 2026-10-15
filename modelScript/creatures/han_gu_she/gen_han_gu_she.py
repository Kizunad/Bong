#!/usr/bin/env python3
"""旱骨蛇（han_gu_she）Blockbench .bbmodel 程序化生成器 —— Round 3/3（在第三次返工版上加叠鳞 / 小棘突 / 唇鳞板 / 颊窝）。

正典依据：docs/worldview.md §十三 北荒（灵气 0.05）。任务卡：末世生物第五批 #21。

外形（对照参考图）：干瘪到只剩一层皮贴着骨的长蛇，下半身盘成一圈（尾尖缩在圈心），前半身从圈前缘
昂起成一条 S 形的颈，头朝前微俯。身体是一串环：每一环由十来块小方块围成「皮壳」，方块大小和半径
都带抖动，所以轮廓是参差的碎块而不是光滑管子；
  · 背脊：每隔一环立一根向上的棘突（颈上的更高），中间一排较白的椎体；
  · 颈前（腹侧）：每三环一道肋骨弧，肋间镂空，能看到里面的脊柱和暗衬；
  · 皮：干皮 / 晒白皮 / 翘卷的碎鳞 / 暗沟 / 裂纹按块随机混色，一部分碎鳞向外翘起；
  · 上颈两侧垂着几缕脱落的残皮。
头是一枚蛇头骨：体素椭球逐级收边的颅 + 吻，眼窝挖空（深处一点枯黄）、眉骨、鼻孔、上颌一排细齿、
两颗外露毒牙；下颌大张，口腔暗黑，舌干成黑褐分叉细条。

坐标：1 block = 16px，原点在蛇腹着地处，+y 向上，正面朝 -z（GeckoLib 约定），+x 为蛇的右侧。
骨骼按「沿身体从尾尖到颈根的环组」分，每组一根骨（链式父子），尾尖那一根最细。
体节骨沿路径摆放，不左右镜像，所以都登记为刻意不对称。

用法：
    python3 modelScript/creatures/han_gu_she/gen_han_gu_she.py --self-test   # 门禁差分自证
    python3 modelScript/creatures/han_gu_she/gen_han_gu_she.py --part head    # 单件预览
    python3 modelScript/creatures/han_gu_she/gen_han_gu_she.py                # 全量：模型 + 审阅图
"""

from __future__ import annotations

import math
import random
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
import creature_kit as kit  # noqa: E402
import creature_voxel as vox  # noqa: E402
from creature_voxel import pick, smooth  # noqa: E402

CREATURE_DIR = Path(__file__).resolve().parent

MATS = {
    "skin":    (150, 128, 96),      # 干皮
    "skin_dk": (96, 79, 58),        # 腹侧 / 皮下阴影 / 暗沟
    "skin_lt": (188, 166, 130),     # 晒白的脊背
    "flake":   (206, 190, 156),     # 翘起的碎鳞
    "rust":    (132, 82, 52),       # 锈褐的旧血痂（参考图里碎鳞间的赭红）
    "crack":   (58, 46, 36),        # 裂纹 / 暗衬
    "bone":    (216, 206, 178),
    "bone_dk": (164, 152, 124),
    "tongue":  (104, 58, 50),       # 干成黑褐的舌
    "void":    (24, 20, 18),        # 眼窝 / 鼻孔
    "eye":     (186, 142, 58),      # 眼窝深处一点枯黄
}
# 皮 / 碎鳞 / 骨 / 裂纹贴在一起，允许重叠；舌压在骨与暗皮上也算贴合
_SOFT = ("skin", "skin_dk", "skin_lt", "flake", "rust", "crack", "bone", "bone_dk")
SOFT_OVER = frozenset(
    [frozenset((a, b)) for a in _SOFT for b in _SOFT if a != b]
    + [frozenset(("tongue", m)) for m in ("bone", "bone_dk", "crack", "skin_dk")]
)

SEED = 7
HEAD_K = 1.3                # 头部（头 / 下颌 / 牙 / 舌 / 残皮）相对按 1.0 写的坐标的放大倍数
JAW_DROP = 2.2             # 下颌（连舌）比闭口位置再下沉多少（按 1.0 比例），张得足够大才让毒牙露出来
HEAD_BONES = frozenset(("head", "jaw", "fang_l", "fang_r", "tongue_1", "tongue_2", "shred_l", "shred_r"))
STEP = 1.45                 # 相邻两环的间距 px
GROUP = 6                   # 每根体节骨管几环
BODY_Y_NECK_END = 40.0


# ───────────────────────────────────────────────────────────── 中心线：盘圈 + S 颈
def spiral_points() -> list[tuple[float, float, float, float]]:
    """尾尖在圈心，向外盘 1.5 圈，末端在圈的 +x 侧、朝 -z 前进。返回 (x, y, z, 管半径)。"""
    pts = []
    turns = 1.5
    n = 400
    for k in range(n + 1):
        t = k / n
        r_path = 3.5 + 14.5 * t ** 0.85
        rad = 0.55 + 4.35 * smooth(t / 0.62) if t < 0.62 else 4.9
        # 相位取 t=1 时角度恰为 0：末端落在 +x 轴上，z 随角度减小的方向走，即朝 -z 前进
        theta = 2.0 * math.pi * turns * (t - 1.0)
        mound = 10.0 * (1.0 - t) ** 1.2                     # 内圈叠在外圈上，盘成一堆而不是一张饼
        pts.append((r_path * math.cos(theta), rad + mound, -r_path * math.sin(theta), rad))
    return pts


NECK_CONTROLS = (          # (x, y, z, 管半径)：从圈上 +x 侧绕到前缘，再 S 形昂起
    (15.5, 4.9, -6.0, 4.9),
    (11.5, 5.6, -13.0, 4.9),
    (5.0, 8.4, -16.0, 4.8),
    (1.0, 14.0, -13.5, 4.9),
    (0.0, 20.5, -9.0, 4.6),
    (0.0, 28.0, -7.8, 4.3),
    (0.0, 34.5, -10.0, 4.1),
    (0.0, 39.0, -14.5, 3.8),
    (0.0, 40.5, -19.5, 3.3),
    (0.0, 40.5, -24.0, 3.3),
)


def catmull(points: list[np.ndarray], per: int = 24) -> list[np.ndarray]:
    """均匀 Catmull-Rom，端点复制。"""
    p = [points[0]] + points + [points[-1]]
    out = []
    for i in range(1, len(p) - 2):
        p0, p1, p2, p3 = p[i - 1], p[i], p[i + 1], p[i + 2]
        for k in range(per):
            t = k / per
            out.append(0.5 * ((2 * p1) + (-p0 + p2) * t + (2 * p0 - 5 * p1 + 4 * p2 - p3) * t * t
                              + (-p0 + 3 * p1 - 3 * p2 + p3) * t ** 3))
    out.append(p[-2])
    return out


def resample(poly: list[np.ndarray], step: float) -> list[np.ndarray]:
    """按弧长（只算 xyz）等间距重采样。"""
    out = [poly[0]]
    need = step                      # 距离下一个采样点还差多远
    for a, b in zip(poly, poly[1:]):
        seg = float(np.linalg.norm(b[:3] - a[:3]))
        if seg == 0.0:
            continue
        pos = 0.0
        while seg - pos >= need:
            pos += need
            out.append(a + (b - a) * (pos / seg))
            need = step
        need -= seg - pos
    return out


class Sample:
    __slots__ = ("p", "rad", "t", "d", "s", "idx", "neck")

    def __init__(self, p, rad, t, d, s, idx, neck):
        self.p, self.rad, self.t, self.d, self.s, self.idx, self.neck = p, rad, t, d, s, idx, neck


def build_samples() -> list[Sample]:
    spiral = [np.array(v, dtype=float) for v in spiral_points()]
    neck = [np.array(v, dtype=float) for v in NECK_CONTROLS]
    # 螺旋末几点 + 颈控制点一起过样条，保证接口处切线连续
    tail_in = spiral[-30::6]
    poly = spiral[:-30] + catmull(tail_in + neck, per=28)
    pts = resample(poly, STEP)
    # 螺旋段与颈段的分界：离螺旋末端点最近的那一环
    end = spiral[-31][:3]
    boundary = min(range(len(pts)), key=lambda i: float(np.linalg.norm(pts[i][:3] - end)))

    samples: list[Sample] = []
    d = np.array([0.0, 1.0, 0.0])
    prev_t = None
    for i, q in enumerate(pts):
        nxt = pts[min(i + 1, len(pts) - 1)]
        prv = pts[max(i - 1, 0)]
        t = nxt[:3] - prv[:3]
        t = t / np.linalg.norm(t)
        if prev_t is not None:                       # 平行移动：把背向量随切线一起转，不扭
            d = d - np.dot(d, t) * t
            d = d / np.linalg.norm(d)
        prev_t = t
        # 颈越抬越高，背向量从「平行移动的结果」过渡到 yz 平面内的规则：
        # 颈竖直时背朝 +z（头朝 -z，腹在前），颈朝 -z 平伸时背朝上。平行移动在颈先拐向圈心时会把背扭到 +x，所以要校正。
        w = smooth((q[1] - 5.5) / 10.0)
        planar = np.array([0.0, -t[2], t[1]])
        dd = (1.0 - w) * d + w * planar
        dd = dd - np.dot(dd, t) * t
        dd = dd / np.linalg.norm(dd)
        side = np.cross(t, dd)
        samples.append(Sample(q[:3].copy(), float(q[3]), t, dd, side, i, i >= boundary))
    return samples


SAMPLES = build_samples()
N_SAMPLES = len(SAMPLES)
N_COIL = sum(1 for s in SAMPLES if not s.neck)


def bone_name(i: int) -> str:
    return f"seg_{i // GROUP:02d}"


def build_bones() -> list[kit.Bone]:
    ngroups = (N_SAMPLES - 1) // GROUP + 1
    head_at = SAMPLES[-1].p
    bones: list[kit.Bone] = [("root", None, (0.0, 0.0, 0.0), "世界根")]
    parent = "root"
    for g in range(ngroups):
        s = SAMPLES[min(g * GROUP, N_SAMPLES - 1)]
        where = "尾尖" if g == 0 else ("盘圈" if s.idx < N_COIL else "颈")
        name = f"seg_{g:02d}"
        bones.append((name, parent, tuple(float(v) for v in s.p), f"{where}第 {g + 1} 组环：皮壳 + 棘突 + 椎体"
                      + ("" if s.idx < N_COIL else " + 肋弧")))
        parent = name
    hx, hy, hz = (float(v) for v in head_at)
    k = HEAD_K

    def at(dx: float, dy: float, dz: float) -> tuple[float, float, float]:
        return (hx + dx * k, hy + dy * k, hz + dz * k)

    bones += [
        ("head", parent, at(0, 0, 0), "蛇头骨：体素椭球颅与吻 / 眼窝 / 眉骨 / 鼻孔 / 上颌细齿"),
        ("jaw", "head", at(0, -2.4, -3.5), "下颌：张开的体素椭球 + 下齿 + 口腔暗衬"),
        ("fang_l", "head", at(-1.7, -2.4, -12.5), "左毒牙（外露）"),
        ("fang_r", "head", at(1.7, -2.4, -12.5), "右毒牙"),
        ("tongue_1", "jaw", at(0, -4.6, -8.0), "舌根（干成细条）"),
        ("tongue_2", "tongue_1", at(0, -5.0, -14.0), "舌叉（分叉的两梢）"),
        ("shred_l", "head", at(-3.2, -0.5, -1.0), "左侧挂着的几缕脱落残皮"),
        ("shred_r", "head", at(3.2, -0.5, -1.0), "右侧残皮"),
    ]
    return bones


BONES = build_bones()
SEG_BONES = [b[0] for b in BONES if b[0].startswith("seg_")]
ASYM_BONES = frozenset(SEG_BONES + ["tongue_1", "tongue_2", "shred_l", "shred_r", "head", "jaw"])


# ───────────────────────────────────────────────────────────── 通用：抖动方块
def jcube(b: kit.Builder, bone: str, name: str, center, size, mat: str, rng: random.Random, jit: float = 0.1) -> None:
    """共用模块的抖动方块；头部各骨的件按 1.0 比例写，整体放大 HEAD_K 后才与身子配比。"""
    vox.jcube(b, bone, name, center, size, mat, rng, jit=jit, k=HEAD_K if bone in HEAD_BONES else 1.0)


SCALES = (("skin", 0.3), ("skin_lt", 0.14), ("flake", 0.16), ("skin_dk", 0.14), ("rust", 0.2), ("crack", 0.06))


# ───────────────────────────────────────────────────────────── 分部件：身体各环
def ring_cube_size(rad: float) -> float:
    return min(1.75, 0.55 + 0.34 * rad)


def body_rings(b: kit.Builder, lo: int, hi: int, tag: str) -> None:
    """[lo, hi) 这几环：皮壳 + 棘突 + 椎体；颈上再加肋弧、暗衬、残皮。"""
    rng = random.Random(SEED * 1000 + lo)
    for i in range(lo, min(hi, N_SAMPLES)):
        s = SAMPLES[i]
        bone = bone_name(i)
        rad, c = s.rad, ring_cube_size(s.rad)
        thin = rad < 1.3
        if thin:
            c = max(c, STEP + 0.35)                      # 尾尖每环只有一块，要比环间距大，才和前后环相接
        rib_ring = s.neck and i % 3 == 0 and not thin
        n = 1 if thin else max(8, math.ceil(2.0 * math.pi * rad / (c * 0.9)))
        for j in range(n):
            phi = 2.0 * math.pi * j / n
            belly = math.pi - abs(phi - math.pi)                                      # 离背中线的角距：0=背 π=腹
            if thin:
                offset = np.zeros(3)
            else:
                if not s.neck and belly > 2.2:
                    continue                                                          # 盘圈贴地的腹面看不见
                if s.neck and belly > 1.8 and not rib_ring:
                    continue                                                          # 颈腹侧肋间镂空
                r_eff = rad * (1.0 + rng.uniform(-0.1, 0.07))
                offset = r_eff * (math.cos(phi) * s.d + math.sin(phi) * s.s)
            pos = s.p + offset
            mat = pick(rng, SCALES)
            if s.neck and belly > 1.8 and rib_ring:
                mat = "bone" if rng.random() < 0.7 else "bone_dk"
                pos = s.p + (rad + 0.55) * (math.cos(phi) * s.d + math.sin(phi) * s.s)
            if not thin and belly < 0.22:
                mat = "skin_lt" if rng.random() < 0.6 else "bone_dk"                  # 背中线一溜较白的椎体
            jcube(b, bone, f"{tag}{i}_{j}", pos, c * (1.0 + rng.uniform(-0.1, 0.1)), mat, rng)
            if not thin and rng.random() < 0.17 and mat in ("flake", "skin", "skin_lt") and not rib_ring:
                lift = (rad + c * 0.75) * (math.cos(phi) * s.d + math.sin(phi) * s.s)         # 翘起的碎鳞
                jcube(b, bone, f"{tag}{i}_{j}_peel", s.p + lift, (c * 0.9, c * 0.9, c * 0.9), "flake", rng, jit=0.2)
        if thin:
            continue
        if i % 2 == 1:
            # Round 3 叠鳞：奇数环在皮壳外面再叠一层错半格的碎鳞片（隔块一片、沿身向前偏半个环距），一片压一片像瓦；
            # 腹侧（盘圈贴地面 / 颈肋间）不叠。参考图里那种层层翘卷的鳞就靠这一层
            for j in range(0, n, 2):
                phi = 2.0 * math.pi * (j + 0.5) / n
                belly = math.pi - abs(phi - math.pi)
                if (not s.neck and belly > 2.0) or (s.neck and belly > 1.6):
                    continue
                pos = s.p + (rad + c * 0.62) * (math.cos(phi) * s.d + math.sin(phi) * s.s) - s.t * (STEP * 0.5)
                plate = c * rng.uniform(0.85, 1.05)
                mat = "flake" if rng.random() < 0.5 else ("rust" if rng.random() < 0.35 else "skin_lt")
                jcube(b, bone, f"{tag}{i}_plate{j}", pos, (plate, plate * 0.7, plate), mat, rng, jit=0.12)
        if i % 2 == 0:                                                        # 背棘突：逐级叠高、逐级收细，相邻两级相接
            levels = (4 if s.neck else 3) + (1 if i % 4 == 0 else 0)
            wide = (1.9, 1.6, 1.3, 1.0, 0.8)
            for k in range(levels):
                reach = rad + 0.4 + sum(wide[m] * 0.8 for m in range(k))
                jcube(b, bone, f"{tag}{i}_spine{k}", s.p + reach * s.d, wide[k], "bone_dk" if k == levels - 1 else "bone", rng,
                      jit=0.05)
        else:                                                                 # Round 3：奇数环再立一根矮的小棘突，脊背整条都是锯齿
            for k in range(2):
                reach = rad + 0.35 + 0.95 * k
                jcube(b, bone, f"{tag}{i}_spinelet{k}", s.p + reach * s.d, (1.1, 1.0, 1.1)[k] * 0.85, "bone_dk" if k else "bone", rng, jit=0.04)
        if s.neck:                                                            # 脊柱与暗衬：肋间镂空处看见的东西
            jcube(b, bone, f"{tag}{i}_core", s.p, c * 0.8, "bone_dk" if i % 2 else "bone", rng, jit=0.05)
            for ph in (-0.9, 0.0, 0.9):
                jcube(b, bone, f"{tag}{i}_lining{ph:+.0f}", s.p + 0.52 * rad * (math.cos(ph) * s.d + math.sin(ph) * s.s),
                      c * 1.15, "crack", rng, jit=0.05)
            if s.idx > N_SAMPLES - 14 and i % 2 == 1:                         # 上颈两侧的脱落残皮
                for sign in (1, -1):
                    base = s.p + (rad + 0.2) * (sign * s.s)
                    for k in range(rng.randint(2, 3)):
                        jcube(b, bone, f"{tag}{i}_shred{sign:+d}_{k}", base - np.array([0, 1.0 * k + 0.7, 0]),
                              (0.9, 1.2, 0.9), "flake" if k % 2 == 0 else "skin_lt", rng, jit=0.12)


def part_tail(b: kit.Builder) -> None:
    """尾与盘圈内圈：骨针似的尾尖逐环变粗，贴地的环只留背和两侧。"""
    body_rings(b, 0, N_COIL // 3, "t")


def part_coil(b: kit.Builder) -> None:
    """盘圈外圈：翘卷的碎鳞、背脊椎体与棘突，参差的轮廓。"""
    body_rings(b, N_COIL // 3, N_COIL, "c")


def part_neck(b: kit.Builder) -> None:
    """S 形立起的颈：背上高棘突，颈前每三环一道肋弧、肋间镂空露出脊柱与暗衬，上颈挂残皮。"""
    body_rings(b, N_COIL, N_SAMPLES, "n")


# ───────────────────────────────────────────────────────────── 体素椭球（逐级收边的弧面）
def ellipsoid_shell(b: kit.Builder, bone: str, tag: str, center, radii, cell: float, mats, rng: random.Random,
                    carve=(), clip=None) -> None:
    """共用模块的体素椭球；头部各骨按 HEAD_K 放大（carve / clip 仍按未放大的局部坐标）。"""
    vox.ellipsoid_shell(b, bone, tag, center, radii, cell, mats, rng, carve=carve, clip=clip,
                        k=HEAD_K if bone in HEAD_BONES else 1.0)


SKULL = (("bone", 0.5), ("bone_dk", 0.2), ("flake", 0.14), ("skin_lt", 0.08), ("crack", 0.04), ("rust", 0.04))


def head_origin() -> tuple[float, float, float]:
    return tuple(float(v) for v in SAMPLES[-1].p)


def part_head(b: kit.Builder) -> None:
    """蛇头骨：体素椭球颅 + 吻、挖空的眼窝（深处枯黄）、眉骨、鼻孔、颊弓、矢状嵴、上颌一排细齿。"""
    rng = random.Random(SEED + 11)
    hx, hy, hz = head_origin()
    with b.offset((hx, hy, hz)):
        eye_c = [((sx * 2.55, 0.9, -5.2), 1.35) for sx in (1, -1)]
        nose_c = [((sx * 0.9, 0.4, -12.2), 0.7) for sx in (1, -1)]
        pit_c = [((sx * 2.15, -0.4, -8.0), 0.6) for sx in (1, -1)]                # Round 3：颊窝的挖空
        # 颅：偏长的椭球，后高前低
        ellipsoid_shell(b, "head", "cran", (0.0, 0.3, -3.6), (3.3, 2.9, 4.1), 1.0, SKULL, rng, carve=eye_c)
        # 吻：更扁更长的椭球，前端收窄
        ellipsoid_shell(b, "head", "snout", (0.0, -0.9, -9.0), (2.3, 1.7, 3.9), 1.0, SKULL, rng, carve=nose_c + pit_c)
        # 吻端
        ellipsoid_shell(b, "head", "tip", (0.0, -1.1, -12.2), (1.5, 1.2, 1.3), 0.9, SKULL, rng, carve=nose_c)
        # 眼窝：黑底 + 一点枯黄；下缘一道骨梁，眉骨压在上沿
        for sx in (1, -1):
            jcube(b, "head", f"socket_void_{sx}", (sx * 2.2, 0.9, -5.3), 1.7, "void", rng, jit=0.05)
            jcube(b, "head", f"socket_eye_{sx}", (sx * 3.0, 0.95, -5.8), (0.5, 0.9, 0.9), "eye", rng, jit=0.03)
            for k in range(4):                                          # 眉骨：向前下斜的一串块
                jcube(b, "head", f"brow_{sx}_{k}", (sx * (2.7 - 0.1 * k), 2.3 - 0.25 * k, -3.8 - 1.0 * k), (1.2, 0.9, 1.1),
                      "bone" if k % 2 else "bone_dk", rng, jit=0.05)
            for k in range(3):                                          # 眼窝下缘与颊弓
                jcube(b, "head", f"cheek_{sx}_{k}", (sx * (3.1 - 0.15 * k), -0.4 - 0.1 * k, -3.0 - 1.4 * k), (1.0, 0.9, 1.5),
                      "bone_dk", rng, jit=0.05)
            jcube(b, "head", f"nostril_{sx}", (sx * 0.9, 0.4, -12.6), 0.9, "void", rng, jit=0.03)
        # 矢状嵴与额脊：头顶一条逐级升高的骨梁，前面接鼻梁
        for k in range(6):
            h = 3.3 + (0.4 if k in (1, 2, 3) else 0.0)
            jcube(b, "head", f"crest_{k}", (0.0, h - 0.1 * abs(k - 2), -1.6 - 1.3 * k), (0.9, 1.0 + 0.2 * (k in (1, 2)), 1.2),
                  "bone" if k % 2 == 0 else "bone_dk", rng, jit=0.05)
        for k in range(4):
            jcube(b, "head", f"bridge_{k}", (0.0, 1.1 - 0.15 * k, -8.2 - 1.3 * k), (0.9, 0.9, 1.3), "bone_dk", rng, jit=0.05)
        # 头顶几块翘起的碎鳞
        for k, (x, z) in enumerate(((1.2, -2.2), (-1.4, -3.2), (0.4, -4.8), (-0.8, -0.8))):
            jcube(b, "head", f"head_peel_{k}", (x, 3.1, z), (1.5, 0.5, 1.4), "flake", rng, jit=0.1)
        # 上颌唇缘与上排细齿（前长后短，向下）
        for sx in (1, -1):
            for k in range(9):
                z = -4.5 - 0.95 * k
                length = 1.2 + 0.07 * k + (0.4 if k == 8 else 0.0)
                # 牙立在颅 / 吻两个椭球的下缘上：x 取该处较宽那个椭球半宽的 0.75
                half = max(3.3 * math.sqrt(max(0.0, 1.0 - ((z + 3.6) / 4.1) ** 2)),
                           2.3 * math.sqrt(max(0.0, 1.0 - ((z + 9.0) / 3.9) ** 2)))
                jcube(b, "head", f"utooth_{sx}_{k}", (sx * 0.75 * half, -2.35 - length / 2 + 0.55, z), (0.55, length, 0.6), "bone",
                      rng, jit=0.03)
        # 腭：上颌里面一层暗板，张嘴时看到的是它
        jcube(b, "head", "palate", (0.0, -2.2, -8.0), (3.0, 0.5, 8.0), "crack", rng, jit=0.02)
        # Round 3：上唇鳞板——牙列外沿一排小骨片（前小后大）；颊窝（热感窝）——吻两侧眼与鼻之间一个暗坑，坑底一点暗皮
        for sx in (1, -1):
            for k in range(8):
                z = -4.8 - 0.95 * k
                half = max(3.3 * math.sqrt(max(0.0, 1.0 - ((z + 3.6) / 4.1) ** 2)), 2.3 * math.sqrt(max(0.0, 1.0 - ((z + 9.0) / 3.9) ** 2)))
                jcube(b, "head", f"labial_{sx}_{k}", (sx * (half + 0.05), -1.7, z), (0.55, 0.6 + 0.04 * (7 - k), 0.8), "bone_dk" if k % 2 else "bone",
                      rng, jit=0.03)
            jcube(b, "head", f"pit_lining_{sx}", (sx * 1.5, -0.4, -8.0), (1.0, 0.9, 1.0), "crack", rng, jit=0.02)


def part_jaw(b: kit.Builder) -> None:
    """下颌：大张开的体素椭球双肢，后高前低、前端更靠下；口腔深处暗衬，下排细齿，颌尖。"""
    rng = random.Random(SEED + 23)
    hx, hy, hz = head_origin()
    with b.offset((hx, hy - JAW_DROP * HEAD_K, hz)):
        # 后段（贴着方骨）和前段分两个椭球，前段下沉
        ellipsoid_shell(b, "jaw", "ramus", (0.0, -4.0, -4.0), (2.5, 1.2, 3.2), 1.0, SKULL, rng,
                        clip=lambda x, y, z: y < -3.3)
        ellipsoid_shell(b, "jaw", "mand", (0.0, -5.6, -8.8), (2.0, 1.1, 3.8), 1.0, SKULL, rng,
                        clip=lambda x, y, z: y < -4.7)
        ellipsoid_shell(b, "jaw", "chin", (0.0, -6.5, -12.6), (1.3, 1.0, 1.7), 0.9, SKULL, rng)
        # 方骨：下颌后端伸向颅骨的两根铰接柱，让张开的下颌仍然挂在颅上
        for sx in (1, -1):
            for k in range(7):
                jcube(b, "jaw", f"quadrate_{sx}_{k}", (sx * 2.7, -4.2 + 0.95 * k, -2.4 - 0.25 * (k % 2)), (1.2, 1.2, 1.6),
                      "bone" if k % 2 else "bone_dk", rng, jit=0.04)
        # 口腔：颌底一层暗皮 + 两侧暗衬
        jcube(b, "jaw", "floor_a", (0.0, -4.3, -4.6), (3.2, 0.5, 4.0), "crack", rng, jit=0.02)
        jcube(b, "jaw", "floor_b", (0.0, -5.7, -9.2), (2.4, 0.5, 4.8), "crack", rng, jit=0.02)
        # 下排细齿：比上排短，向上
        for sx in (1, -1):
            for k in range(7):
                z = -6.0 - 1.0 * k
                # 牙要立在下颌前段椭球（中心 z=-8.8、半宽 2.0、前后各 3.8）的边缘上：x 取该处半宽的 0.7
                half = 2.0 * math.sqrt(max(0.1, 1.0 - ((z + 8.8) / 3.8) ** 2))
                jcube(b, "jaw", f"ltooth_{sx}_{k}", (sx * 0.7 * half, -5.0, z), (0.5, 1.0 + 0.04 * k, 0.6), "bone", rng,
                      jit=0.03)
        # 颌尖一撮碎骨
        for k, x in enumerate((-0.5, 0.0, 0.5)):
            jcube(b, "jaw", f"chin_tip_{k}", (x, -7.2 - 0.2 * (k == 1), -13.6), (0.55, 0.7, 0.8), "bone_dk", rng, jit=0.03)


def part_fangs(b: kit.Builder) -> None:
    """两颗外露毒牙：粗根 → 弯下的牙身 → 细尖，牙身前面一道毒液沟。"""
    hx, hy, hz = head_origin()
    k_s = HEAD_K
    with b.offset((hx, hy, hz)):
        # 两颗牙严格左右镜像（b.put "lr" 成对），不加抖动：这是整只蛇里唯一参与镜像门禁的一对
        segs = ((-2.6, 1.15, 1.0, "bone"), (-3.7, 1.0, 1.0, "bone"), (-4.8, 0.85, 1.0, "bone"),
                (-5.8, 0.65, 0.9, "bone"), (-6.7, 0.5, 0.8, "bone_dk"), (-7.4, 0.35, 0.6, "bone_dk"))
        for k, (y, w, h, mat) in enumerate(segs):
            zc = -12.0 - 0.18 * k
            lo = (1.7 * k_s - w * k_s / 2, y * k_s - h * k_s / 2, zc * k_s - w * k_s / 2)
            hi = (1.7 * k_s + w * k_s / 2, y * k_s + h * k_s / 2, zc * k_s + w * k_s / 2)
            b.put("fang", f"fang_{k}", lo, hi, mat, "lr")
        b.put("fang", "fang_groove", (1.7 * k_s - 0.15, -5.9 * k_s, -12.55 * k_s - 0.15),
              (1.7 * k_s + 0.15, -2.9 * k_s, -12.55 * k_s + 0.15), "crack", "lr")


def part_tongue(b: kit.Builder) -> None:
    """干成黑褐细条的分叉舌：舌根贴在颌底、伸出口外，末梢分叉并略下垂。"""
    rng = random.Random(SEED + 41)
    hx, hy, hz = head_origin()
    with b.offset((hx, hy - JAW_DROP * HEAD_K, hz)):
        for k in range(5):
            jcube(b, "tongue_1", f"tongue_a{k}", (0.0, -4.55 - 0.1 * k, -7.5 - 1.3 * k), (0.8, 0.5, 1.4), "tongue", rng, jit=0.02)
        for k in range(4):
            jcube(b, "tongue_2", f"tongue_b{k}", (0.0, -5.1 - 0.35 * k, -14.1 - 1.2 * k), (0.7, 0.5, 1.3), "tongue", rng, jit=0.02)
        for sx in (1, -1):
            for k in range(3):
                jcube(b, "tongue_2", f"tongue_fork{sx}_{k}", (sx * (0.55 + 0.4 * k), -6.3 - 0.4 * k, -19.0 - 1.1 * k),
                      (0.55, 0.45, 1.2), "tongue", rng, jit=0.02)


def part_shreds(b: kit.Builder) -> None:
    """头两侧垂下的脱落残皮：几缕长短不一的薄皮条，缀着碎鳞。"""
    rng = random.Random(SEED + 53)
    hx, hy, hz = head_origin()
    with b.offset((hx, hy, hz)):
        for sx, side in ((1, "r"), (-1, "l")):
            bone = f"shred_{side}"
            for col, (dz, drop) in enumerate(((0.0, 5), (-1.5, 3), (1.5, 6), (-3.0, 2), (3.0, 4))):
                for k in range(drop):
                    mat = "flake" if (k + col) % 3 == 0 else ("skin_lt" if (k + col) % 3 == 1 else "skin")
                    jcube(b, bone, f"shred_{side}_{col}_{k}", (sx * (3.55 - 0.1 * k), 1.2 - 0.95 * k, dz - 1.2),
                          (0.7, 1.0, 0.9), mat, rng, jit=0.06)


PARTS = {
    "tail": part_tail,
    "coil": part_coil,
    "neck": part_neck,
    "head": part_head,
    "jaw": part_jaw,
    "fangs": part_fangs,
    "tongue": part_tongue,
    "shreds": part_shreds,
}

SPEC = kit.CreatureSpec(
    model_id="han_gu_she",
    title="旱骨蛇",
    class_name="HanGuShe",
    creature_dir=CREATURE_DIR,
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    min_bones=24,
    round_label="round 3/3",
    soft_over=SOFT_OVER,
    asym_bones=ASYM_BONES,
    ref_prompt=(
        "a desiccated drought snake, dried to a thin skin stretched over bone: long body coiled on cracked dry "
        "ground with its front body raised, every vertebra protruding as a ridge along the spine, ribs visible through "
        "the skin, parchment-like cracked skin with peeling curled scales, a skull-shaped head with empty eye sockets "
        "and exposed fangs, a dried black-brown forked tongue, a tail thinning to a bone needle; detail close-ups: skull "
        "head with fangs, spine vertebrae ridge, cracked peeling scales, ribs through skin, tail needle"
    ),
)

if __name__ == "__main__":
    kit.run_cli(SPEC)
