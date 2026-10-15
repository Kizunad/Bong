#!/usr/bin/env python3
"""碎岩猿（sui_yan_yuan）Blockbench .bbmodel 程序化生成器 —— Round 3/3（在第三次返工版上拉开手臂、毛按绺分色、减块）。

正典依据：docs/worldview.md §十三 青云残峰「中级野兽」。任务卡：末世生物第四批 #12。

外形（对照参考图）：驼背长臂的灰毛山猿，像一只披着草绳和碎石的大猩猩。
  · 毛：身体、双臂、大腿、小腿都被一绺绺竖直垂下的长毛盖住（灰白 / 灰褐 / 深褐混色，几百绺，长短不一），
    肩背一圈鬃毛盖到上臂；毛瀑布之间露出深色裸皮的掌、脚、脸；
  · 脸：深色裸皮的猿脸——前突的眉骨压着深陷的眼窝（一点琥珀），塌鼻、宽吻、下颌带两颗外露的牙；
    头顶和脑后披毛，脸前不长毛；
  · 岩甲：肩上三块叠起的碎岩（棱角缺口、灰绿苔斑），前臂外侧两片岩护甲，用草绳一圈圈捆在臂上；
    胸前两道草绳交叉斜挎；背上长出几块带苔的岩瘤；
  · 腰：一圈草绳腰带，前后左右垂着褐色的碎布 / 毛条腰裙；
  · 手：粗大的深色裸皮手，四指弯成拳，指节凸出；右手攥着一块拳头大的石头（单独一骨，投掷用），左手半握；
  · 脚：宽厚的裸皮脚掌，五趾分开。
站姿含胸驼背，肩高约 27px，头顶约 32px，垂臂指节落到 y≈2。

坐标：1 block = 16px，原点在两脚中间的地面，+y 向上，正面朝 -z（GeckoLib 约定），+x 为猿的右侧
（骨名 *_r）、-x 为左侧（*_l）。毛 / 岩 / 皮的抖动逐块随机，所以除两只耳朵（严格镜像的一对）外都登记为刻意不对称。

用法：
    python3 modelScript/creatures/sui_yan_yuan/gen_sui_yan_yuan.py --self-test   # 门禁差分自证
    python3 modelScript/creatures/sui_yan_yuan/gen_sui_yan_yuan.py --part head    # 单件预览
    python3 modelScript/creatures/sui_yan_yuan/gen_sui_yan_yuan.py                # 全量：模型 + 审阅图
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
from creature_voxel import ellipsoid_shell, jcube, pick, rock_slab, taper_chain  # noqa: E402

CREATURE_DIR = Path(__file__).resolve().parent
SEED = 11
HEAD_AT = (0.0, 26.8, -10.4)       # 头骨枢轴（头按局部坐标写，b.offset 平移到这里）
# Round 3：用户审 Round 1 时说「正面手臂和躯干轮廓分得不够开」——上臂以下整条手臂（上臂 / 前臂 / 岩护甲 / 手 / 指 / 拇指 / 握着的石头）
# 沿 x 向外拉开 ARM_OUT，肩与肩岩甲不动（肩毛团与上臂仍有重叠，手臂不会脱开）
ARM_OUT = 1.8

MATS = {
    "fur_dark":  (84, 76, 68),
    "fur":       (118, 108, 96),
    "fur_lt":    (152, 140, 124),
    "fur_grey":  (184, 174, 156),     # 毛尖 / 老灰毛
    "cloth":     (132, 92, 58),       # 腰裙的褐色碎布 / 毛条
    "skin":      (84, 66, 58),        # 脸盘 / 掌心裸皮
    "skin_lt":   (112, 90, 76),
    "rock":      (124, 120, 112),
    "rock_dk":   (86, 82, 78),
    "rock_lt":   (158, 154, 144),
    "rock_crack": (48, 46, 44),
    "lichen":    (96, 110, 80),       # 岩瘤上的灰绿苔
    "rope":      (150, 124, 82),      # 绑岩块的草绳
    "rope_dk":   (106, 86, 56),
    "bone":      (206, 196, 166),     # 牙
    "claw":      (62, 54, 46),
    "void":      (22, 20, 20),        # 眼窝 / 鼻孔
    "eye":       (214, 150, 52),
}
# 软物（毛 / 皮 / 岩 / 绳 / 布）盖在彼此身上允许重叠；牙与爪嵌在皮里也算贴合；眼与深影不放行
_SOFT = ("fur_dark", "fur", "fur_lt", "fur_grey", "cloth", "skin", "skin_lt", "rock", "rock_dk", "rock_lt", "rock_crack",
         "lichen", "rope", "rope_dk", "bone", "claw")
SOFT_OVER = frozenset([frozenset((a, b)) for a in _SOFT for b in _SOFT if a != b]
                      + [frozenset(("void", m)) for m in _SOFT])

FUR = (("fur_grey", 0.28), ("fur_lt", 0.3), ("fur", 0.26), ("fur_dark", 0.16))              # 上身：偏灰白
FUR_LOW = (("fur_lt", 0.2), ("fur", 0.34), ("fur_dark", 0.46))                              # 下身 / 前臂：偏深
SKIN = (("skin", 0.6), ("skin_lt", 0.3), ("fur_dark", 0.1))
ROCK = (("rock", 0.32), ("rock_lt", 0.22), ("rock_dk", 0.2), ("lichen", 0.14), ("rock_crack", 0.04), ("rock", 0.08))
ROCK_MOSSY = (("rock", 0.28), ("lichen", 0.34), ("rock_lt", 0.16), ("rock_dk", 0.16), ("rock_crack", 0.06))
ROPE = (("rope", 0.55), ("rope_dk", 0.45))
CLOTH = (("cloth", 0.55), ("fur_dark", 0.25), ("rope_dk", 0.2))

# (name, parent, pivot[px], 说明)。骨骼先父后子。+x = 右侧（*_r）。
BONES: list[kit.Bone] = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("hips", "root", (0.0, 12.0, 2.0), "骨盆 + 臀毛 + 腰带与腰裙"),
    ("spine_lower", "hips", (0.0, 14.5, 1.0), "腹 / 腰：毛层"),
    ("spine_upper", "spine_lower", (0.0, 19.5, -1.5), "含胸的桶形胸腔 + 胸毛 + 胸前交叉草绳"),
    ("mane", "spine_upper", (0.0, 25.0, -2.0), "肩背鬃毛（披一圈，层层参差）"),
    ("chest_rock", "spine_upper", (0.0, 20.5, -8.0), "胸口嵌的岩片"),
    ("back_rock_a", "spine_upper", (-2.4, 27.0, 2.6), "背上岩瘤 A（带苔，两块）"),
    ("back_rock_b", "spine_upper", (2.8, 25.0, 3.4), "背上岩瘤 B（带苔）"),
    ("neck", "spine_upper", (0.0, 24.6, -6.2), "颈（短粗前探）"),
    ("head", "neck", HEAD_AT, "脑颅 / 眉骨 / 宽吻 / 塌鼻 / 眼窝"),
    ("jaw", "head", (0.0, 25.2, -11.4), "下颌 + 两颗外露牙"),
    ("ear_l", "head", (-3.8, 27.2, -10.0), "左耳"),
    ("ear_r", "head", (3.8, 27.2, -10.0), "右耳"),
    ("shoulder_l", "spine_upper", (-7.2, 23.0, -2.5), "左肩毛团"),
    ("shoulder_r", "spine_upper", (7.2, 23.0, -2.5), "右肩毛团"),
    ("pauldron_l", "shoulder_l", (-8.8, 25.6, -2.6), "左肩岩甲（三层叠片）"),
    ("pauldron_r", "shoulder_r", (8.8, 25.6, -2.6), "右肩岩甲"),
    ("upper_arm_l", "shoulder_l", (-9.2 - ARM_OUT, 22.0, -3.2), "左上臂（粗毛臂）"),
    ("upper_arm_r", "shoulder_r", (9.2 + ARM_OUT, 22.0, -3.2), "右上臂"),
    ("forearm_l", "upper_arm_l", (-9.8 - ARM_OUT, 15.4, -4.8), "左前臂（毛 + 腕上粗毛圈）"),
    ("forearm_r", "upper_arm_r", (9.8 + ARM_OUT, 15.4, -4.8), "右前臂"),
    ("arm_rock_l", "forearm_l", (-10.0 - ARM_OUT, 13.4, -5.8), "左前臂岩护甲（两片 + 草绳）"),
    ("arm_rock_r", "forearm_r", (10.0 + ARM_OUT, 13.4, -5.8), "右前臂岩护甲"),
    ("hand_l", "forearm_l", (-9.8 - ARM_OUT, 8.2, -6.4), "左掌（裸皮掌心 + 指根）"),
    ("hand_r", "forearm_r", (9.8 + ARM_OUT, 8.2, -6.4), "右掌"),
    ("fingers_l", "hand_l", (-9.8 - ARM_OUT, 5.2, -7.6), "左四指 + 爪尖"),
    ("fingers_r", "hand_r", (9.8 + ARM_OUT, 5.2, -7.6), "右四指 + 爪尖"),
    ("thumb_l", "hand_l", (-8.0 - ARM_OUT, 6.6, -7.2), "左拇指"),
    ("thumb_r", "hand_r", (8.0 + ARM_OUT, 6.6, -7.2), "右拇指"),
    ("rock_held", "hand_r", (9.6 + ARM_OUT, 4.4, -10.0), "右手攥的投掷石（单独一骨）"),
    ("thigh_l", "hips", (-4.2, 11.4, 1.6), "左大腿（毛腿）"),
    ("thigh_r", "hips", (4.2, 11.4, 1.6), "右大腿"),
    ("shin_l", "thigh_l", (-4.4, 6.4, 0.6), "左小腿"),
    ("shin_r", "thigh_r", (4.4, 6.4, 0.6), "右小腿"),
    ("foot_l", "shin_l", (-4.4, 2.8, 0.2), "左脚（宽脚掌）"),
    ("foot_r", "shin_r", (4.4, 2.8, 0.2), "右脚"),
    ("toes_l", "foot_l", (-4.4, 1.2, -3.4), "左脚趾 + 爪"),
    ("toes_r", "foot_r", (4.4, 1.2, -3.4), "右脚趾"),
]
# 毛 / 岩 / 皮的抖动逐块随机，所以登记为刻意不对称；两只耳朵是严格镜像的一对，留给镜像门禁
ASYM_BONES = frozenset(name for name, *_ in BONES if not name.startswith("ear_"))


def sided(name: str, sx: int) -> str:
    return f"{name}_r" if sx > 0 else f"{name}_l"


# Round 3：毛按「绺」分组上色。原来每根毛各自随机抽毛色，颜色碎成一粒一粒；现在把表面按 TUFT_X × TUFT_Y × TUFT_Z
# 的格子分成几大绺，同一格里的毛共用一个主色（按格子位置定，邻格不同色）、长度取同一个基准，只有 TUFT_NOISE 比例的毛
# 随机串色；毛条每块沿竖直拉长 TUFT_STRETCH 倍，一条毛用更少的块，整体方块数明显下降。
TUFT_X, TUFT_Y, TUFT_Z = 3.4, 4.6, 3.4
TUFT_NOISE = 0.12
TUFT_STRETCH = 3.0
SPACING_SCALE = 1.2             # 毛根栅格间距整体放大，毛根数减到约 70%


def tuft_key_rng(tag: str, px: float, py: float, pz: float) -> random.Random:
    """同一部位同一格子的毛用同一个随机种子（决定这一绺的主色和基准长度）。"""
    name = tag.rstrip("0123456789-")
    cell = (int(math.floor(px / TUFT_X)), int(math.floor(py / TUFT_Y)), int(math.floor(pz / TUFT_Z)))
    name_hash = sum(ord(c) * (i + 3) for i, c in enumerate(name))          # 不用内置 hash()：字符串 hash 每次进程不一样，产物就不可复现
    return random.Random(name_hash * 7919 + cell[0] * 104729 + cell[1] * 1299709 + cell[2] * 15485863)


def fur_cover(b: kit.Builder, bone_of, tag: str, center, radii, rng: random.Random, mats, spacing: float = 1.5,
              length: tuple[float, float] = (5.0, 10.0), floor: float = 8.5, width: tuple[float, float] = (0.8, 1.05),
              skip=None) -> int:
    """给一个椭球的表面披毛（同 creature_voxel.fur_cover 的栅格，但毛色与长度按「绺」分组，见上面 TUFT_*）。"""
    cx, cy, cz = center
    rx, ry, rz = radii
    spacing *= SPACING_SCALE
    n = 0

    def grow(px: float, py: float, pz: float, sx_: float) -> None:
        nonlocal n
        if skip and skip(sx_, py, pz):
            return
        grp = tuft_key_rng(tag, px, py, pz)
        base_mat = pick(grp, mats)
        base_len = grp.uniform(*length)
        mat = pick(rng, mats) if rng.random() < TUFT_NOISE else base_mat
        ln = min(base_len * rng.uniform(0.85, 1.15), py - floor)
        if ln > 1.5:
            vox.strand(b, bone_of(sx_, py, pz), f"{tag}{n}", (px, py, pz), ln, rng.uniform(*width), rng, mat, stretch=TUFT_STRETCH)
            n += 1

    for x in np.arange(-rx, rx + 0.01, spacing):                           # 顶面
        for z in np.arange(cz - rz, cz + rz + 0.01, spacing):
            s = 1.0 - (x / rx) ** 2 - ((z - cz) / rz) ** 2
            if s <= 0.0:
                continue
            y = cy + ry * math.sqrt(s)
            px, pz = x + rng.uniform(-0.35, 0.35), z + rng.uniform(-0.35, 0.35)
            grow(cx + px * 0.96, y - 0.3, pz, cx + px)
    for z in np.arange(cz - rz, cz + rz + 0.01, spacing):                  # 两侧
        y = cy + ry * 0.85
        while y > floor + 1.5:
            s = 1.0 - ((y - cy) / ry) ** 2 - ((z - cz) / rz) ** 2
            if s > 0.0:
                x = rx * math.sqrt(s)
                for sign in (1, -1):
                    py, pz = y + rng.uniform(-0.35, 0.35), z + rng.uniform(-0.35, 0.35)
                    grow(cx + sign * x * 0.93, py, pz, cx + sign * x)
            y -= spacing * 1.3
    return n


def fur(b: kit.Builder, bone_of, tag: str, center, radii, rng: random.Random, mats=FUR, **kw) -> int:
    """给一个椭球披毛：先铺一层深色的体素毛底（毛丝之间不至于透出空洞），再从表面垂下一绺绺长毛（按绺分色）。"""
    ellipsoid_shell(b, bone_of(center[0], center[1], center[2]), tag + "_core", center, radii, 1.6,
                    (("fur_dark", 0.5), ("fur", 0.5)), rng, clip=kw.pop("clip", None))
    return fur_cover(b, bone_of, tag, center, radii, rng, mats, **kw)


def rope_ring(b: kit.Builder, bone: str, tag: str, center, rx: float, rz: float, rng: random.Random, n: int = 14,
              size: float = 1.15) -> None:
    """绕在肢体 / 腰上的一圈草绳：椭圆上均匀放 n 块，交替两种绳色。"""
    for m in range(n):
        a = 2.0 * math.pi * m / n
        jcube(b, bone, f"{tag}_{m}", (center[0] + rx * math.cos(a), center[1] + rng.uniform(-0.15, 0.15),
                                       center[2] + rz * math.sin(a)), size, "rope" if m % 2 else "rope_dk", rng, jit=0.05)


# ───────────────────────────────────────────────────────────── 躯干
def part_hips(b: kit.Builder) -> None:
    """骨盆：毛底 + 垂毛；一圈草绳腰带，前后左右垂下褐色碎布 / 毛条腰裙。"""
    rng = random.Random(SEED + 1)
    fur(b, lambda x, y, z: "hips", "hip", (0.0, 11.5, 2.0), (5.8, 4.2, 4.6), rng, mats=FUR_LOW, floor=6.5, length=(3.5, 6.5))
    rope_ring(b, "hips", "belt", (0.0, 13.4, 2.0), 5.7, 4.6, rng, n=20, size=1.3)
    for m in range(20):                                              # 腰裙：褐色宽条，前面最长
        a = 2.0 * math.pi * (m + 0.5) / 20
        x, z = 5.7 * math.cos(a), 2.0 + 4.7 * math.sin(a)
        front = max(0.0, -math.sin(a))
        vox.strand(b, "hips", f"skirt{m}", (x, 13.0, z), 4.0 + 4.0 * front + rng.uniform(0.0, 1.5), rng.uniform(1.4, 1.9), rng,
                   CLOTH, sway=0.4)


def part_belly(b: kit.Builder) -> None:
    """腹 / 腰：桶形的毛腹，垂毛一直盖到腰带。"""
    rng = random.Random(SEED + 2)
    fur(b, lambda x, y, z: "spine_lower", "bel", (0.0, 15.8, 0.8), (5.4, 3.6, 4.6), rng, mats=FUR_LOW, floor=11.5, length=(3.0, 6.0))


def part_chest(b: kit.Builder) -> None:
    """含胸的桶形胸腔：宽厚的毛胸，垂毛瀑布；胸前两道草绳交叉斜挎，中间嵌一片小岩。"""
    rng = random.Random(SEED + 3)
    center, radii = (0.0, 21.0, -2.4), (7.4, 5.8, 5.6)
    fur(b, lambda x, y, z: "spine_upper", "chest", center, radii, rng, floor=12.0, length=(5.0, 9.0), spacing=1.9)
    for sign in (1, -1):                                             # 交叉斜挎的草绳：从一侧肩贴着胸前表面拉到另一侧腰
        x0, y0, x1, y1 = sign * 7.0, 25.2, -sign * 3.4, 14.6
        steps = 22
        for s in range(steps + 1):
            t = s / steps
            x, y = x0 + (x1 - x0) * t, y0 + (y1 - y0) * t
            q = 1.0 - ((x - center[0]) / radii[0]) ** 2 - ((y - center[1]) / radii[1]) ** 2
            if q <= 0.05:
                continue
            z = center[2] - radii[2] * math.sqrt(q) - 0.5
            jcube(b, "spine_upper", f"strap{sign}_{s}", (x, y, z), 1.25, "rope" if s % 2 else "rope_dk", rng, jit=0.05)
    rock_slab(b, "chest_rock", "chest_rock", (0.0, 20.6, -8.2), (4.0, 3.4, 1.6), rng, ROCK)
    rope_ring(b, "chest_rock", "chest_knot", (0.0, 20.6, -7.4), 2.5, 0.9, rng, n=8, size=0.9)


def part_mane(b: kit.Builder) -> None:
    """肩背鬃毛：披在肩上的一圈长毛，从颈后一路垂到上臂和背。"""
    rng = random.Random(SEED + 4)
    fur(b, lambda x, y, z: "mane", "mane", (0.0, 24.2, -1.0), (8.4, 2.8, 5.0), rng, floor=16.0, length=(6.0, 10.0), spacing=1.9)


def part_neck(b: kit.Builder) -> None:
    """颈：短粗前探的毛颈。"""
    rng = random.Random(SEED + 5)
    fur(b, lambda x, y, z: "neck", "neck", (0.0, 24.6, -6.4), (3.8, 3.2, 3.4), rng, floor=19.0, length=(3.0, 5.0), spacing=1.9)


def part_back_rocks(b: kit.Builder) -> None:
    """背上长出的岩瘤：三块高低不一的石笋，灰绿苔斑盖了大半，根部被毛埋住，草绳从胸前绕过来在背上打结。"""
    rng = random.Random(SEED + 6)
    ellipsoid_shell(b, "back_rock_a", "rock_a1", (-2.4, 28.2, 2.6), (2.6, 4.4, 2.4), 1.3, ROCK_MOSSY, rng)
    ellipsoid_shell(b, "back_rock_a", "rock_a2", (0.4, 26.6, 4.6), (2.0, 3.2, 2.0), 1.3, ROCK, rng)
    ellipsoid_shell(b, "back_rock_b", "rock_b", (2.9, 26.0, 3.4), (2.5, 3.8, 2.3), 1.3, ROCK_MOSSY, rng)
    for sign in (1, -1):                                             # 背上交叉的草绳
        for s in range(14):
            t = s / 13
            jcube(b, "back_rock_b", f"bstrap{sign}_{s}", (sign * (6.4 - 12.0 * t * 0.5), 24.6 - 7.0 * t, 4.4 + 0.3 * t), 1.2,
                  "rope" if s % 2 else "rope_dk", rng, jit=0.05)


# ───────────────────────────────────────────────────────────── 头
def part_head(b: kit.Builder) -> None:
    """猿脸：深色裸皮的颅与宽吻、前突的眉骨压着深陷的眼窝（一点琥珀）、塌鼻、嘴线；头顶与脑后披毛，脸前不长毛。"""
    rng = random.Random(SEED + 7)
    with b.offset(HEAD_AT):
        ellipsoid_shell(b, "head", "skull", (0.0, 0.0, 0.0), (3.7, 3.6, 3.8), 1.0, SKIN, rng,
                        carve=[((sx * 1.7, 0.3, -3.3), 0.8) for sx in (1, -1)])
        ellipsoid_shell(b, "head", "muzzle", (0.0, -1.5, -3.6), (2.6, 1.9, 1.9), 1.0, SKIN, rng,
                        carve=[((sx * 0.8, -0.9, -5.4), 0.45) for sx in (1, -1)])
        for sx in (1, -1):
            jcube(b, "head", f"socket{sx}", (sx * 1.7, 0.3, -3.2), 1.3, "void", rng, jit=0.03)
            jcube(b, "head", f"eye{sx}", (sx * 1.7, 0.25, -3.9), 0.7, "eye", rng, jit=0.02)
            jcube(b, "head", f"nostril{sx}", (sx * 0.8, -0.9, -5.2), 0.8, "void", rng, jit=0.02)
            for k in range(3):                                       # 眉骨：横过眼窝上沿、向前下方压出的一道骨梁
                jcube(b, "head", f"brow{sx}{k}", (sx * (0.8 + 1.0 * k), 1.5 - 0.15 * k, -3.9 + 0.1 * k), (1.4, 1.2, 1.4),
                      "skin_lt" if k % 2 == 0 else "skin", rng, jit=0.05)
            jcube(b, "head", f"cheek{sx}", (sx * 3.0, -0.6, -2.4), (1.4, 1.6, 1.6), "skin_lt", rng)
        jcube(b, "head", "brow_mid", (0.0, 1.7, -4.0), (1.6, 1.2, 1.2), "skin_lt", rng)
        jcube(b, "head", "nose_bridge", (0.0, -0.1, -4.9), (1.5, 1.5, 1.2), "skin_lt", rng)
        jcube(b, "head", "mouth_line", (0.0, -2.5, -5.2), (3.6, 0.5, 0.6), "rock_crack", rng, jit=0.02)
        vox.fur_cover(b, lambda x, y, z: "head", "headfur", (0.0, 0.9, 1.6), (4.8, 4.4, 3.8), rng, FUR, spacing=1.7,
                      length=(3.5, 7.0), floor=-4.0, skip=lambda x, y, z: z < -1.6 and y < 3.2)


def part_jaw(b: kit.Builder) -> None:
    """下颌：宽厚的颌，嘴角两颗外露的牙向上翘出唇外，颌下一把短须。"""
    rng = random.Random(SEED + 8)
    with b.offset(HEAD_AT):
        ellipsoid_shell(b, "jaw", "jaw", (0.0, -3.4, -3.0), (2.6, 1.3, 2.4), 1.0, SKIN, rng)
        for sx in (1, -1):
            taper_chain(b, "jaw", f"tusk{sx}", (sx * 1.8, -3.0, -5.0), (sx * 2.0, -0.9, -5.6), 1.1, 0.6, "bone", rng, curl=(0.0, 0.0, -0.3))
        for k in range(8):
            vox.strand(b, "jaw", f"beard{k}", (rng.uniform(-1.8, 1.8), -4.4, rng.uniform(-4.2, -1.6)), rng.uniform(1.8, 3.6), 0.9, rng,
                       (("fur_dark", 0.6), ("fur", 0.4)))


def part_ears(b: kit.Builder) -> None:
    """耳：猿的小圆耳，一对严格镜像（b.put "lr"，不加抖动）：外圈 + 耳孔。"""
    with b.offset(HEAD_AT):
        b.put("ear", "ear_rim", (3.7, -0.4, 0.0), (4.7, 1.8, 1.4), "skin_lt", "lr")
        b.put("ear", "ear_back", (3.4, 0.0, 0.8), (3.8, 1.4, 1.8), "skin", "lr")
        b.put("ear", "ear_hole", (4.2, 0.3, -0.2), (4.8, 1.1, 0.5), "rock_crack", "lr")


# ───────────────────────────────────────────────────────────── 臂 / 手
def part_arms(b: kit.Builder) -> None:
    """长臂：粗毛的上臂 + 又粗又长的前臂，竖直长毛一直垂到腕；腕上一圈粗毛圈。"""
    rng = random.Random(SEED + 9)
    for sx in (1, -1):
        up, fo = sided("upper_arm", sx), sided("forearm", sx)
        fur(b, lambda x, y, z, nm=up: nm, f"ua{sx}", (sx * (9.2 + ARM_OUT), 19.4, -3.4), (3.1, 5.4, 3.1), rng, floor=13.0, length=(4.5, 8.0),
            spacing=1.9)
        fur(b, lambda x, y, z, nm=fo: nm, f"fa{sx}", (sx * (9.8 + ARM_OUT), 12.0, -5.2), (3.0, 4.9, 3.0), rng, mats=FUR_LOW, floor=6.0,
            length=(4.0, 7.0), spacing=1.9)


def part_shoulders(b: kit.Builder) -> None:
    """肩毛团：肩头的一团毛，把上臂根部包起来。"""
    rng = random.Random(SEED + 10)
    for sx in (1, -1):
        sh = sided("shoulder", sx)
        fur(b, lambda x, y, z, nm=sh: nm, f"shd{sx}", (sx * 7.6, 23.4, -2.6), (3.6, 3.2, 3.6), rng, floor=18.0, length=(4.0, 7.0),
            spacing=1.9)


def part_pauldrons(b: kit.Builder) -> None:
    """肩岩甲：每肩三块叠起的碎岩（棱角缺口、灰绿苔斑），草绳在肩上交叉捆住。"""
    rng = random.Random(SEED + 11)
    for sx in (1, -1):
        bone = sided("pauldron", sx)
        rock_slab(b, bone, f"pa{sx}", (sx * 9.0, 26.4, -2.6), (4.8, 3.0, 5.0), rng, ROCK)
        rock_slab(b, bone, f"pb{sx}", (sx * 10.8, 24.2, -3.8), (3.0, 3.6, 4.0), rng, ROCK)
        rock_slab(b, bone, f"pc{sx}", (sx * 8.0, 28.0, -0.8), (3.4, 2.6, 3.2), rng, ROCK_MOSSY)
        for s in range(10):                                          # 草绳：压在岩片前缘
            jcube(b, bone, f"pstrap{sx}_{s}", (sx * (11.0 - 0.9 * s), 24.0 + 0.45 * s, -5.6), 1.15, "rope" if s % 2 else "rope_dk", rng,
                  jit=0.05)


def part_arm_rocks(b: kit.Builder) -> None:
    """前臂岩护甲：外侧一片长岩、前面一片宽岩，三圈草绳把它们捆在臂上。"""
    rng = random.Random(SEED + 12)
    for sx in (1, -1):
        bone = sided("arm_rock", sx)
        rock_slab(b, bone, f"ra{sx}", (sx * (11.6 + ARM_OUT), 13.8, -5.2), (2.4, 5.4, 4.4), rng, ROCK)
        rock_slab(b, bone, f"rb{sx}", (sx * (9.8 + ARM_OUT), 12.0, -8.0), (3.8, 4.6, 1.8), rng, ROCK)
        for k, y in enumerate((15.8, 13.2, 10.6)):
            rope_ring(b, bone, f"rr{sx}{k}", (sx * (9.8 + ARM_OUT), y, -5.2), 3.2, 3.1, rng, n=14, size=1.2)


def part_hands(b: kit.Builder) -> None:
    """手：粗大的深色裸皮手，四指弯成拳、指节凸出、爪尖短黑；拇指压在食指旁。右手攥着的石头见 part_rock_held。"""
    rng = random.Random(SEED + 13)
    for sx in (1, -1):
        hand, fingers, thumb = sided("hand", sx), sided("fingers", sx), sided("thumb", sx)
        hx = sx * (9.8 + ARM_OUT)
        ellipsoid_shell(b, hand, f"palm{sx}", (hx, 6.8, -6.4), (2.6, 2.4, 2.8), 1.1, SKIN, rng)
        for k in range(4):                                           # 四指：从掌前缘弯下去
            x = hx + (k - 1.5) * 1.35
            taper_chain(b, fingers, f"fg{sx}{k}", (x, 5.6, -8.0), (x, 3.0 + 0.3 * (sx < 0) * (k % 2), -9.4 - 0.2 * k), 1.35, 1.0, SKIN, rng,
                        curl=(0.0, 0.0, -0.8))
            jcube(b, fingers, f"knuckle{sx}{k}", (x, 6.4, -8.2), (1.4, 1.2, 1.2), "skin_lt", rng, jit=0.05)
            jcube(b, fingers, f"claw{sx}{k}", (x, 2.5 + 0.3 * (k % 2), -9.8 - 0.2 * k), 0.6, "claw", rng, jit=0.03)
        taper_chain(b, thumb, f"tm{sx}", (hx - sx * 2.0, 6.8, -6.8), (hx - sx * 1.6, 4.4, -8.6), 1.5, 1.0, SKIN, rng, curl=(0.0, 0.0, -0.5))


def part_rock_held(b: kit.Builder) -> None:
    """右手攥的投掷石：拳头大、棱角分明的灰岩，单独一骨，好在投掷动画里甩出去。"""
    rng = random.Random(SEED + 15)
    rock_slab(b, "rock_held", "stone", (9.6 + ARM_OUT, 4.6, -10.4), (3.6, 3.2, 3.4), rng, ROCK, cell=1.2)


# ───────────────────────────────────────────────────────────── 腿 / 脚
def part_legs(b: kit.Builder) -> None:
    """腿：粗毛的大腿和小腿，竖直长毛盖到脚踝。"""
    rng = random.Random(SEED + 16)
    for sx in (1, -1):
        th, sh = sided("thigh", sx), sided("shin", sx)
        fur(b, lambda x, y, z, nm=th: nm, f"th{sx}", (sx * 4.3, 9.6, 1.6), (3.3, 4.7, 3.5), rng, mats=FUR_LOW, floor=5.0, length=(4.0, 7.0),
            spacing=1.9)
        fur(b, lambda x, y, z, nm=sh: nm, f"shin{sx}", (sx * 4.4, 5.0, 0.6), (2.7, 3.2, 2.7), rng, mats=FUR_LOW, floor=2.6, length=(3.0, 5.0),
            spacing=1.9)


def part_feet(b: kit.Builder) -> None:
    """脚：宽厚的深色裸皮脚掌，五趾分开，趾尖一点短爪。"""
    rng = random.Random(SEED + 17)
    for sx in (1, -1):
        foot, toes = sided("foot", sx), sided("toes", sx)
        ellipsoid_shell(b, foot, f"foot{sx}", (sx * 4.4, 1.7, -0.6), (3.0, 1.7, 4.4), 1.2, SKIN, rng, clip=lambda x, y, z: y > 0.0)
        for k in range(5):
            x = sx * (4.4 + (k - 2) * 1.2)
            jcube(b, toes, f"toe{sx}{k}", (x, 1.0, -4.9 - 0.2 * (k in (0, 4))), (1.2, 1.5, 1.8), "skin_lt" if k % 2 else "skin", rng, jit=0.05)
            jcube(b, toes, f"toeclaw{sx}{k}", (x, 0.6, -6.1), 0.6, "claw", rng, jit=0.03)


PARTS = {
    "hips": part_hips,
    "belly": part_belly,
    "chest": part_chest,
    "mane": part_mane,
    "neck": part_neck,
    "back_rocks": part_back_rocks,
    "head": part_head,
    "jaw": part_jaw,
    "ears": part_ears,
    "arms": part_arms,
    "shoulders": part_shoulders,
    "pauldrons": part_pauldrons,
    "arm_rocks": part_arm_rocks,
    "hands": part_hands,
    "rock_held": part_rock_held,
    "legs": part_legs,
    "feet": part_feet,
}

SPEC = kit.CreatureSpec(
    model_id="sui_yan_yuan",
    title="碎岩猿",
    class_name="SuiYanYuan",
    creature_dir=CREATURE_DIR,
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    min_bones=28,
    round_label="round 3/3",
    soft_over=SOFT_OVER,
    asym_bones=ASYM_BONES,
    ref_prompt=(
        "a hunched long-armed grey-furred mountain ape, knuckles almost touching the ground, shaggy ragged mane around "
        "shoulders, chunks of broken grey rock strapped with straw rope on its shoulders and forearms like armor, two "
        "lichen-covered rock lumps growing on its back, a fist-sized stone gripped in its right hand, dark bare-skin face "
        "with heavy brow ridge and two exposed tusks; detail close-ups: head, rock shoulder armor, forearm rock armor with "
        "rope, hand holding the stone, back rock lumps"
    ),
)

if __name__ == "__main__":
    kit.run_cli(SPEC)
