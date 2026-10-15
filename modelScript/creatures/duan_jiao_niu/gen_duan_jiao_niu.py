#!/usr/bin/env python3
"""断角野牛（duan_jiao_niu）Blockbench .bbmodel 程序化生成器 —— Round 1/3（第三次返工版），只做几何。

正典依据：docs/worldview.md §十三 初醒原「低级野兽」/ §七 兽潮。任务卡：末世生物第五批 #20。

外形（对照参考图）：瘦骨嶙峋的荒原野牛。前半身是一座高耸的肩峰和胸颈，被一层层垂下来的长毛盖住：
毛不是贴面，而是几百绺从身体表面往下垂的细毛条（粗细、长短、深浅各不同，下端渐细、带锈色），一直垂到
前腿上段、胸前和颏下；后半身没毛，一根根肋条顶着皮弯出来（肋间镂空、露出黑暗的腹腔），脊椎一节节凸着，
髋骨高高顶起，几道旧伤疤爬在肋上。头沉沉地低垂：宽颅、眉弓压眼、长吻、黑鼻垫，额前和下巴垂着乱毛；
右角完整外弯再上挑，左角从根部上方折断，断口参差、挂着碎角茬。
四肢分段：前腿被长毛盖到小臂，细长掌骨，大块分瓣蹄；后腿是瘦骨大腿加向后折的飞节，几乎没毛。
尾细长，尾梢一把枯毛。

坐标：1 block = 16px，原点在四蹄中间的地面，+y 向上，正面朝 -z（GeckoLib 约定），+x 为牛的右侧
（骨名 *_r）、-x 为左侧（*_l）。肩峰高约 28px，体长（鼻到臀）约 40px。
毛、肋、皮的抖动逐块随机，所以除四个蹄子（严格镜像的一对）外都登记为刻意不对称。

用法：
    python3 modelScript/creatures/duan_jiao_niu/gen_duan_jiao_niu.py --self-test   # 门禁差分自证
    python3 modelScript/creatures/duan_jiao_niu/gen_duan_jiao_niu.py --part head    # 单件预览
    python3 modelScript/creatures/duan_jiao_niu/gen_duan_jiao_niu.py                # 全量：模型 + 审阅图
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
from creature_voxel import ellipsoid_shell, jcube, limb, pick, taper_chain  # noqa: E402

CREATURE_DIR = Path(__file__).resolve().parent
SEED = 18

MATS = {
    "hide":      (112, 90, 72),       # 瘦出骨架的皮
    "hide_dk":   (72, 58, 46),        # 凹陷处 / 腹下 / 肋间
    "hide_lt":   (150, 128, 102),     # 凸起的骨头处磨浅
    "scar":      (176, 150, 126),     # 旧伤疤
    "blood":     (146, 88, 80),       # 伤疤里的干血
    "bone":      (212, 196, 164),     # 肋骨 / 脊椎 / 髋骨
    "bone_dk":   (168, 150, 120),
    "fur_dk":    (60, 49, 40),        # 打结的毛团
    "fur":       (94, 77, 61),
    "fur_lt":    (124, 106, 84),
    "fur_rust":  (140, 98, 58),       # 毛梢的锈色
    "horn":      (188, 174, 144),
    "horn_dk":   (122, 108, 86),
    "horn_fresh": (228, 216, 188),    # 断茬的新茬
    "hoof":      (46, 38, 32),
    "void":      (24, 20, 18),        # 鼻孔 / 眼窝 / 腹腔的深影
    "eye":       (198, 152, 62),
}
# 皮 / 毛 / 骨 / 疤 / 角 / 蹄互相压着是同一头牛，允许重叠；眼与深影不放行
_SOFT = ("hide", "hide_dk", "hide_lt", "scar", "blood", "bone", "bone_dk", "fur_dk", "fur", "fur_lt", "fur_rust",
         "horn", "horn_dk", "horn_fresh", "hoof")
SOFT_OVER = frozenset([frozenset((a, b)) for a in _SOFT for b in _SOFT if a != b]
                       + [frozenset(("void", m)) for m in _SOFT])     # 腹腔深影被毛 / 皮压着是同一个洞
FURS = (("fur", 0.34), ("fur_dk", 0.3), ("fur_lt", 0.14), ("fur_rust", 0.22))
FACE = (("hide_lt", 0.34), ("scar", 0.16), ("hide", 0.34), ("fur", 0.1), ("hide_dk", 0.06))
LEG = (("hide", 0.4), ("hide_lt", 0.3), ("hide_dk", 0.15), ("fur", 0.15))
BONES_PALE = (("bone", 0.5), ("bone_dk", 0.2), ("hide_lt", 0.2), ("scar", 0.1))

# 返工（用户「老问题，身体太短了」）：参考图侧视量得 躯干长 / 肩高 ≈ 1.08、头颈长 / 躯干长 ≈ 0.26；
# 旧模型 0.97 / 0.51——躯干偏短，而且头颈从胸前缘往前伸出一大截，参考里的头是缩在肩上毛团前面的。
# 做法：肩 / 胸 / 肩峰 / 鬃 / 前腿整组沿 -z 前移 FRONT_DZ（躯干前缘前伸、前后腿间距加大、露肋的腰腹变长），
# 颈 / 头 / 下颌 / 耳 / 角整组沿 +z 后收 NECK_DZ（头缩进肩毛团前面）。骨盆 / 后腿 / 尾不动。
FRONT_DZ = -3.5
NECK_DZ = 3.0
# 头整体抬高 HEAD_DY：旧版头太低，下巴只离地 3.4px，颏下长须一直垂到地面以下 6.8px（穿地）；抬高后须尖离地 ≥ 0.4px
HEAD_DY = 3.0
HEAD_AT = (0.0, 12.0 + HEAD_DY, -17.5 + NECK_DZ)

# (name, parent, pivot[px], 说明)。骨骼先父后子。+x = 右侧（*_r）。
BONES: list[kit.Bone] = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("hips", "root", (0.0, 16.0, 11.0), "骨盆：髋骨顶起的皮 + 臀毛"),
    ("spine_lower", "hips", (0.0, 16.0, 5.0), "塌陷的腰腹 + 露肋的后半胸"),
    ("spine_upper", "spine_lower", (0.0, 16.0, -2.0 + FRONT_DZ), "前半胸：下垂的长毛"),
    ("hump", "spine_upper", (0.0, 21.0, -4.0 + FRONT_DZ), "肩峰（高耸的肩胛 + 打结的毛堆）"),
    ("mane", "hump", (0.0, 22.0, -9.0 + FRONT_DZ), "颈背上的乱鬃（一团团打结）"),
    ("neck_1", "spine_upper", (0.0, 19.0, -9.4 + NECK_DZ), "颈根"),
    ("neck_2", "neck_1", (0.0, 17.0, -11.6 + NECK_DZ), "颈前段"),
    ("head", "neck_2", HEAD_AT, "低垂的长脸：颅 / 眉弓 / 眼 / 吻 / 鼻"),
    ("jaw", "head", (0.0, 8.0 + HEAD_DY, -19.5 + NECK_DZ), "下颌 + 长须"),
    ("ear_l", "head", (-4.4, 15.0 + HEAD_DY, -16.3 + NECK_DZ), "左耳"),
    ("ear_r", "head", (4.4, 15.0 + HEAD_DY, -16.3 + NECK_DZ), "右耳"),
    ("horn_l", "head", (-3.6, 15.6 + HEAD_DY, -18.3 + NECK_DZ), "左角：折断的根部与断茬"),
    ("horn_r", "head", (3.6, 15.6 + HEAD_DY, -18.3 + NECK_DZ), "右角：完整外弯上挑"),
    ("tail_1", "hips", (0.0, 19.0, 14.5), "尾根"),
    ("tail_2", "tail_1", (0.0, 17.2, 16.3), "尾第 2 节"),
    ("tail_3", "tail_2", (0.0, 15.0, 17.0), "尾第 3 节"),
    ("tail_4", "tail_3", (0.0, 13.0, 17.2), "尾梢枯毛"),
    ("f_upper_l", "spine_upper", (-3.8, 15.0, -5.5 + FRONT_DZ), "左前臂上段（肩 + 上臂 + 肘）"),
    ("f_upper_r", "spine_upper", (3.8, 15.0, -5.5 + FRONT_DZ), "右前臂上段"),
    ("f_fore_l", "f_upper_l", (-3.8, 9.6, -5.5 + FRONT_DZ), "左前腿下段（膝 + 小臂）"),
    ("f_fore_r", "f_upper_r", (3.8, 9.6, -5.5 + FRONT_DZ), "右前腿下段"),
    ("f_cannon_l", "f_fore_l", (-3.7, 5.2, -5.6 + FRONT_DZ), "左前掌骨（细长，后面挂毛）"),
    ("f_cannon_r", "f_fore_r", (3.7, 5.2, -5.6 + FRONT_DZ), "右前掌骨"),
    ("f_hoof_l", "f_cannon_l", (-3.7, 2.6, -5.5 + FRONT_DZ), "左前蹄（分瓣 + 悬蹄）"),
    ("f_hoof_r", "f_cannon_r", (3.7, 2.6, -5.5 + FRONT_DZ), "右前蹄"),
    ("h_thigh_l", "hips", (-4.4, 19.0, 11.0), "左后腿大腿（瘦骨 + 皮）"),
    ("h_thigh_r", "hips", (4.4, 19.0, 11.0), "右后腿大腿"),
    ("h_shank_l", "h_thigh_l", (-4.2, 13.0, 9.5), "左后腿小腿（向后折的飞节上段）"),
    ("h_shank_r", "h_thigh_r", (4.2, 13.0, 9.5), "右后腿小腿"),
    ("h_cannon_l", "h_shank_l", (-3.8, 7.6, 12.4), "左后掌骨"),
    ("h_cannon_r", "h_shank_r", (3.8, 7.6, 12.4), "右后掌骨"),
    ("h_hoof_l", "h_cannon_l", (-3.8, 2.6, 11.8), "左后蹄"),
    ("h_hoof_r", "h_cannon_r", (3.8, 2.6, 11.8), "右后蹄"),
]
# 毛 / 肋 / 皮的抖动逐块随机，所以登记为刻意不对称；四个蹄子是严格镜像的一对，留给镜像门禁
ASYM_BONES = frozenset(name for name, *_ in BONES if "hoof" not in name)


# ───────────────────────────────────────────────────────────── 毛
HEAD_K = 1.2        # 头部（头 / 下颌 / 角 / 耳 / 额毛 / 须）按 1.0 比例写，整体放大这么多才与身子配比


def strand(b: kit.Builder, bone: str, tag: str, start, length: float, width: float, rng: random.Random,
           mats=FURS, sway: float = 0.35, k: float = 1.0) -> None:
    """一绺垂毛：从 start 往下垂 length，宽 width 逐渐收到 0.45 倍；末段略偏出一点，像垂着的湿毛。
    每块沿竖直方向拉长 2.2 倍，读成一条细毛而不是一摞方块。"""
    x, y, z = start
    end = (x + rng.uniform(-sway, sway), y - length, z + rng.uniform(-sway, sway))
    taper_chain(b, bone, tag, start, end, width, width * 0.45, mats, rng, overlap=0.7, stretch=2.2, k=k)


# 头部各部件的包装：同样的调用，只是整体乘 HEAD_K（局部坐标，配合 b.offset 平移到头骨位置）
def hjcube(b, bone, name, center, size, mat, rng, jit=0.1):
    jcube(b, bone, name, center, size, mat, rng, jit=jit, k=HEAD_K)


def hellipsoid(b, bone, tag, center, radii, cell, mats, rng, carve=(), clip=None):
    ellipsoid_shell(b, bone, tag, center, radii, cell, mats, rng, carve=carve, clip=clip, k=HEAD_K)


def hstrand(b, bone, tag, start, length, width, rng, mats=FURS, sway=0.35):
    strand(b, bone, tag, start, length, width, rng, mats=mats, sway=sway, k=HEAD_K)


def htaper(b, bone, tag, p0, p1, w0, w1, mat, rng, curl=(0.0, 0.0, 0.0)):
    taper_chain(b, bone, tag, p0, p1, w0, w1, mat, rng, curl=curl, k=HEAD_K)


def fur_cover(b: kit.Builder, bone_of, tag: str, center, radii, rng: random.Random, spacing: float = 1.5,
              length=(5.0, 10.0), floor: float = 8.5, width=(0.8, 1.05)) -> None:
    """给一个椭球的表面披毛：顶面按 (x, z) 栅格、两侧按 (y, z) 栅格各起一批垂毛，下端不低于 floor。

    侧面的毛从表面起笔，一排压着一排往下垂，像参考图里层层叠叠的瀑布。bone_of(x, y, z) 决定每绺归哪块骨。
    """
    cx, cy, cz = center
    rx, ry, rz = radii
    n = 0
    for x in np.arange(-rx, rx + 0.01, spacing):                           # 顶面
        for z in np.arange(cz - rz, cz + rz + 0.01, spacing):
            s = 1.0 - (x / rx) ** 2 - ((z - cz) / rz) ** 2
            if s <= 0.0:
                continue
            y = cy + ry * math.sqrt(s)
            px, pz = x + rng.uniform(-0.35, 0.35), z + rng.uniform(-0.35, 0.35)
            ln = min(rng.uniform(*length), y - floor)
            if ln > 1.5:
                strand(b, bone_of(px, y, pz), f"{tag}{n}", (px * 0.96, y - 0.3, pz), ln, rng.uniform(*width), rng)
                n += 1
    for z in np.arange(cz - rz, cz + rz + 0.01, spacing):                  # 两侧
        y = cy + ry * 0.85
        while y > floor + 1.5:
            s = 1.0 - ((y - cy) / ry) ** 2 - ((z - cz) / rz) ** 2
            if s > 0.0:
                x = rx * math.sqrt(s)
                for sign in (1, -1):
                    py, pz = y + rng.uniform(-0.35, 0.35), z + rng.uniform(-0.35, 0.35)
                    ln = min(rng.uniform(*length), py - floor)
                    if ln > 1.5:
                        strand(b, bone_of(x, py, pz), f"{tag}{n}", (sign * x * 0.93, py, pz), ln, rng.uniform(*width), rng)
                        n += 1
            y -= spacing * 1.3


def front_bone(x: float, y: float, z: float) -> str:
    """前半身毛的归属（z 是 part_front 的局部坐标，前移之前）：肩峰顶上归 hump、颈背归 mane、其余归 spine_upper。"""
    if z < -9.0 and y >= 22.5:
        return "mane"
    if y > 23.5:
        return "hump"
    return "spine_upper"


# ───────────────────────────────────────────────────────────── 躯干
def part_front(b: kit.Builder) -> None:
    """前半身：肩峰高耸的胸颈体块（体素椭球）+ 满身垂毛瀑布 + 肩峰上打结的毛堆；整组前移 FRONT_DZ。"""
    rng = random.Random(SEED + 1)
    mass = ((0.0, 19.0, -3.5), (5.6, 8.6, 9.0))
    with b.offset((0.0, 0.0, FRONT_DZ)):
        ellipsoid_shell(b, "spine_upper", "chest", mass[0], mass[1], 1.5, (("fur_dk", 0.5), ("hide_dk", 0.3), ("fur", 0.2)), rng,
                        clip=lambda x, y, z: y > 10.5)
        ellipsoid_shell(b, "hump", "hump", (0.0, 24.0, -5.5), (4.4, 4.6, 5.6), 1.4, (("fur_dk", 0.4), ("fur", 0.3), ("fur_rust", 0.3)), rng)
        fur_cover(b, front_bone, "fr", mass[0], mass[1], rng, floor=9.2)
        for k in range(9):                                                  # 肩峰顶上直立的乱毛：往上炸的短簇
            x, z = rng.uniform(-3.0, 3.0), rng.uniform(-9.0, -2.5)
            y0 = 27.0 + 1.2 * (1.0 - abs(x) / 4.0)
            taper_chain(b, "hump", f"hump_tuft{k}", (x, y0 - 1.5, z), (x * 1.15, y0 + 1.5, z + rng.uniform(-0.6, 0.6)), 1.3, 0.55,
                        FURS, rng, overlap=0.7)


def part_neck(b: kit.Builder) -> None:
    """颈：下压的粗颈（体素椭球），颈背一路披着鬃毛，颈下垂着胸前的长毛帘；整组后收 NECK_DZ，缩进肩毛团前面。"""
    rng = random.Random(SEED + 2)
    with b.offset((0.0, 0.0, NECK_DZ)):
        ellipsoid_shell(b, "neck_1", "neck", (0.0, 19.5, -10.8), (4.0, 5.2, 4.6), 1.4, (("fur_dk", 0.4), ("hide_dk", 0.3), ("fur", 0.3)), rng)
        limb(b, "neck_2", "neck2", ((0.0, 17.5, -13.0, 3.6), (0.0, 15.0, -15.2, 3.2)), (("fur_dk", 0.5), ("fur", 0.5)), rng, bumps=3,
             bump_mats=FURS)
        fur_cover(b, lambda x, y, z: "neck_1" if y > 22.0 else "neck_2", "nk", (0.0, 19.5, -10.8), (4.0, 5.2, 4.6), rng,
                  spacing=1.5, length=(4.0, 8.0), floor=11.0)


def part_ribcage(b: kit.Builder) -> None:
    """后半胸：七对肋条一根根顶着皮弯出来（肋间镂空），脊椎一节节凸着，几道旧伤疤，腹腔里是黑暗的深影。"""
    rng = random.Random(SEED + 3)
    ellipsoid_shell(b, "spine_lower", "belly", (0.0, 16.4, 7.0), (3.9, 4.8, 6.6), 1.5, (("hide_dk", 0.55), ("void", 0.45)), rng,
                    clip=lambda x, y, z: y < 19.0)
    for i in range(8):                                                      # 八对肋条（前半身前移后露出来的胸段多一对）
        z = 0.25 + 1.35 * i
        ytop = 23.0 - 0.35 * i                                              # 背线在这里向后塌
        reach = 4.9 - 0.12 * i
        depth = 8.0 - 0.15 * i
        for sign in (1, -1):
            for step in range(10):
                a = 0.18 + step * 0.185
                jcube(b, "spine_lower", f"rib{i}_{sign}_{step}", (sign * reach * math.sin(a), ytop - depth * (1.0 - math.cos(a)), z),
                      (1.25, 1.35, 1.05), pick(rng, BONES_PALE), rng, jit=0.06)
    for i in range(19):                                                     # 脊椎：一节节凸起的棘突，从前移后的肩峰下一路排到髋骨
        z = -3.6 + FRONT_DZ + 1.0 * i
        y = 23.2 - 0.3 * max(0.0, i - 4) - (0.0 if i < 4 else 0.0)
        bone = "spine_upper" if z < 1.0 else ("spine_lower" if z < 9.0 else "hips")
        jcube(b, bone, f"vert{i}", (0.0, y, z), (1.5, 1.4, 1.1), pick(rng, BONES_PALE), rng, jit=0.05)
        jcube(b, bone, f"spine_pt{i}", (0.0, y + 1.0, z), (0.9, 1.0, 0.8), "bone", rng, jit=0.04)
    for k, (x, z0, z1) in enumerate(((4.9, 2.0, 7.5), (5.1, 4.0, 9.0))):    # 旧伤疤：斜着爬过肋条的两道
        for sign in (1, -1):
            taper_chain(b, "spine_lower", f"scar{k}_{sign}", (sign * x, 19.8, z0), (sign * (x + 0.3), 14.4, z1), 0.9, 0.7,
                        (("scar", 0.6), ("blood", 0.4)), rng, curl=(sign * 0.4, 0.0, 0.0))


def part_hips(b: kit.Builder) -> None:
    """骨盆：髋骨高高顶起的皮、尖角的坐骨、凹下去的臀沟、臀上几绺枯毛。"""
    rng = random.Random(SEED + 4)
    for sign in (1, -1):
        # 髋骨：一块向上向外顶起的楔形骨，尖上带一点疤
        for k, (dx, y, z, w) in enumerate(((4.2, 20.2, 10.8, 1.7), (4.8, 20.8, 10.6, 1.5), (5.3, 21.5, 10.4, 1.3), (5.6, 22.2, 10.4, 1.1), (5.7, 22.8, 10.4, 0.9))):
            jcube(b, "hips", f"hip{sign}_{k}", (sign * dx, y, z), w, "bone" if k % 2 == 0 else "bone_dk", rng, jit=0.05)
        for k in range(3):                                                  # 坐骨
            jcube(b, "hips", f"sit{sign}_{k}", (sign * (2.4 + 0.2 * k), 14.4 + 0.9 * k, 15.0), 1.4, "bone_dk", rng, jit=0.05)
    for k in range(10):                                                     # 臀上枯毛
        x = rng.uniform(-3.4, 3.4)
        strand(b, "hips", f"rump_fur{k}", (x, 20.4, rng.uniform(10.0, 14.0)), rng.uniform(2.5, 4.5), 1.1, rng)


# ───────────────────────────────────────────────────────────── 头
def part_head(b: kit.Builder) -> None:
    """低垂的长脸：体素椭球的宽颅 + 长吻、眉弓压眼（深陷的眼窝里一点琥珀）、黑鼻垫、额前乱毛、耳。"""
    rng = random.Random(SEED + 5)
    hx, hy, hz = HEAD_AT
    with b.offset((hx, hy, hz)):
        hellipsoid(b, "head", "skull", (0.0, 0.0, 0.0), (3.5, 4.3, 4.5), 1.0, FACE, rng,
                        carve=[((sx * 3.3, 1.0, -2.6), 0.9) for sx in (1, -1)])
        hellipsoid(b, "head", "muzzle", (0.0, -3.6, -4.6), (2.7, 3.4, 3.8), 1.0, FACE, rng)
        for sx in (1, -1):
            hjcube(b, "head", f"socket{sx}", (sx * 3.0, 1.0, -2.8), 1.5, "void", rng, jit=0.03)
            hjcube(b, "head", f"eye{sx}", (sx * 3.4, 1.0, -3.1), 0.8, "eye", rng, jit=0.02)
            for k in range(4):                                              # 眉弓：压在眼上的一道骨梁
                hjcube(b, "head", f"brow{sx}{k}", (sx * (3.2 - 0.25 * k), 2.4 - 0.1 * k, -1.4 - 0.9 * k), (1.1, 1.0, 1.2),
                      pick(rng, FACE), rng, jit=0.05)
            hjcube(b, "head", f"nostril{sx}", (sx * 1.2, -5.4, -8.1), 1.0, "void", rng, jit=0.02)
            hjcube(b, "head", f"eyelid{sx}", (sx * 3.2, 2.1, -3.0), (1.3, 0.5, 1.2), "hide_dk", rng, jit=0.02)       # Round 3：沉重的上眼睑压在眼上
            hjcube(b, "head", f"eyebag{sx}", (sx * 3.3, -0.1, -2.9), (1.1, 0.45, 1.0), "scar", rng, jit=0.02)       # 下眼睑发白的眼袋
        for sx in (-1, 0, 1):                                               # 黑鼻垫：三块并排，中间的略低
            hjcube(b, "head", f"nose{sx}", (sx * 1.2, -6.2 - 0.3 * (sx == 0), -7.6), (1.4, 1.5, 1.6), "hoof", rng, jit=0.04)
        for k in range(18):                                                 # 额前乱毛：垂在脸上，躲开眼睛
            x = rng.uniform(-2.4, 2.4)
            z = rng.uniform(-3.0, 0.5)
            hstrand(b, "head", f"forelock{k}", (x, 4.0 - 0.8 * abs(x) / 2.4, z), rng.uniform(3.5, 6.5), rng.uniform(1.0, 1.3), rng)
        for k in range(10):                                                 # 颊边的鬓毛
            for sx in (1, -1):
                hstrand(b, "head", f"cheek{sx}{k}", (sx * rng.uniform(3.2, 3.7), rng.uniform(-1.0, 1.5), rng.uniform(-0.5, 3.0)),
                       rng.uniform(3.0, 6.0), 1.1, rng)
        for sx in (1, -1):                                                  # 耳：向外斜后的叶片，内里发红
            base = np.array([sx * 3.6, 3.2, 1.2])
            htaper(b, sided("ear", sx), f"ear{sx}", base, base + np.array([sx * 3.2, 0.8, 1.3]), 2.0, 0.8,
                   (("hide_lt", 0.5), ("scar", 0.3), ("blood", 0.2)), rng)


def sided(name: str, sx: int) -> str:
    return f"{name}_r" if sx > 0 else f"{name}_l"


def part_jaw(b: kit.Builder) -> None:
    """下颌 + 颏下的长须：一把垂到胸口的乱须。"""
    rng = random.Random(SEED + 6)
    hx, hy, hz = HEAD_AT
    with b.offset((hx, hy, hz)):
        hellipsoid(b, "jaw", "jaw", (0.0, -6.8, -4.2), (2.1, 1.1, 3.2), 1.0, FACE, rng)
        for k in range(20):
            x = rng.uniform(-1.8, 1.8)
            z = rng.uniform(-6.2, -1.0)
            hstrand(b, "jaw", f"beard{k}", (x, -7.2, z), rng.uniform(2.5, 5.0), rng.uniform(1.0, 1.4), rng,
                   mats=(("fur_rust", 0.4), ("fur", 0.35), ("fur_lt", 0.25)))


def part_horns(b: kit.Builder) -> None:
    """右角完整：粗根外弯再上挑，逐级收尖；左角从根部上方折断：只剩一截粗根，断口参差、几片新茬。"""
    rng = random.Random(SEED + 7)
    hx, hy, hz = HEAD_AT
    horn_mats = (("horn", 0.5), ("horn_dk", 0.35), ("horn", 0.15))
    with b.offset((hx, hy, hz)):
        htaper(b, "horn_r", "horn_r_a", (3.6, 3.6, -0.8), (8.4, 6.6, -1.4), 2.8, 2.1, horn_mats, rng, curl=(0.0, 1.0, 0.5))
        htaper(b, "horn_r", "horn_r_b", (8.4, 6.6, -1.4), (10.8, 11.4, -3.4), 2.1, 0.55, horn_mats, rng, curl=(1.4, 0.0, -0.8))
        htaper(b, "horn_l", "horn_l_a", (-3.6, 3.6, -0.8), (-6.9, 5.0, -1.2), 2.8, 2.4, horn_mats, rng, curl=(0.0, 0.4, 0.2))
        for k, (end, w) in enumerate((((-7.6, 7.6, -1.2), 1.5), ((-6.5, 7.0, -2.0), 1.2), ((-7.2, 6.4, -0.2), 1.1), ((-8.0, 6.2, -1.9), 0.9))):
            htaper(b, "horn_l", f"horn_l_shard{k}", (-6.9 + 0.2 * (k % 2), 5.0, -1.2 - 0.2 * (k // 2)), end, w, 0.45,
                        (("horn_fresh", 0.7), ("horn", 0.3)), rng)
        for k in range(3):                                                  # 断口挂着的角丝
            htaper(b, "horn_l", f"horn_l_thread{k}", (-6.9, 4.8, -1.6 + 0.6 * k), (-7.9, 2.8 - 0.4 * k, -1.8 + 0.6 * k), 0.8, 0.4,
                        (("horn_dk", 0.5), ("horn_fresh", 0.5)), rng)
        # Round 3：右角套环脊（角质的生长纹，每隔一段一圈略粗的深色环）、角尖一截最深；左角断口多两片参差新茬 + 一块骨髓芯
        for seg, (p0, p1, w0, w1, rings) in enumerate((((3.6, 3.6, -0.8), (8.4, 6.6, -1.4), 2.8, 2.1, (0.22, 0.45, 0.68, 0.88)),
                                                       ((8.4, 6.6, -1.4), (10.8, 11.4, -3.4), 2.1, 0.55, (0.25, 0.5, 0.75)))):
            for k, t in enumerate(rings):
                c = tuple(p0[i] + (p1[i] - p0[i]) * t for i in range(3))
                w = w0 + (w1 - w0) * t
                hjcube(b, "horn_r", f"horn_r_ring{seg}{k}", c, w + 0.45, "horn_dk", rng, jit=0.03)
        hjcube(b, "horn_r", "horn_r_tip", (10.9, 11.9, -3.6), 0.7, "horn_dk", rng, jit=0.02)
        hjcube(b, "horn_l", "horn_l_marrow", (-6.9, 5.0, -1.2), 1.4, "bone", rng, jit=0.03)
        for k, end in enumerate(((-6.0, 6.8, -0.6), (-8.4, 5.8, -1.0))):
            htaper(b, "horn_l", f"horn_l_shard{4 + k}", (-6.9, 5.0, -1.2), end, 1.0, 0.35, (("horn_fresh", 0.8), ("bone", 0.2)), rng)


# ───────────────────────────────────────────────────────────── 四肢
def hoof_pair(b: kit.Builder, stem: str, x: float, z0: float, z1: float) -> None:
    """蹄：一大块分瓣的深色蹄壁（两瓣并排、前缘一阶阶收）+ 上面套着一圈蹄冠 + 悬蹄。严格左右镜像（b.put "lr"）。"""
    for half, (xa, xb) in enumerate(((x - 1.5, x), (x, x + 1.5))):
        b.put(stem, f"{stem}_wall{half}", (xa, 0.0, z0), (xb, 2.6, z1), "hoof", "lr")
        b.put(stem, f"{stem}_toe{half}", (xa + 0.1, 0.0, z0 - 0.7), (xb - 0.1, 1.5, z0), "hoof", "lr")
        b.put(stem, f"{stem}_crown{half}", (xa - 0.1, 2.6 - 0.1 * half, z0 + 0.3 + 0.15 * half), (xb + 0.1, 3.6 + 0.1 * half, z1 - 0.3 - 0.15 * half), "hide_dk", "lr")
    b.put(stem, f"{stem}_dew", (x - 0.9, 3.0, z1 - 0.4), (x + 0.9, 4.2, z1 + 0.8), "hoof", "lr")
    # Round 3：两瓣之间一道深缝（蹄的裂口），球节上一圈浅色的角质环
    b.put(stem, f"{stem}_cleft", (x - 0.15, 0.0, z0 - 0.6), (x + 0.15, 2.0, z1 - 0.2), "void", "lr")
    b.put(stem, f"{stem}_fetlock", (x - 1.7, 3.6, z0 + 0.4), (x + 1.7, 4.3, z1 - 0.4), "bone_dk", "lr")


def part_forelegs(b: kit.Builder) -> None:
    """前腿：肩臂（藏在毛里）→ 肘 → 小臂 → 膝 → 细长掌骨（后面挂毛）→ 球节 → 分瓣蹄 + 悬蹄。"""
    rng = random.Random(SEED + 8)
    with b.offset((0.0, 0.0, FRONT_DZ)):                                    # 前腿随前半身一起前移
        _forelegs(b, rng)


def _forelegs(b: kit.Builder, rng: random.Random) -> None:
    for sx in (1, -1):
        upper, fore, cannon = (sided(n, sx) for n in ("f_upper", "f_fore", "f_cannon"))
        limb(b, upper, f"fup{sx}", ((sx * 4.0, 17.0, -5.5, 2.6), (sx * 3.9, 12.0, -5.5, 2.1), (sx * 3.8, 10.0, -5.5, 1.9)), (("fur_dk", 0.4), ("fur", 0.3), ("hide_dk", 0.3)), rng, bumps=4,
             bump_mats=FURS)
        fl = limb(b, fore, f"ffo{sx}", ((sx * 3.8, 10.0, -5.5, 1.9), (sx * 3.7, 7.4, -5.6, 1.5), (sx * 3.7, 5.4, -5.6, 1.3)), LEG, rng,
                  bumps=3, bump_mats=BONES_PALE)
        jcube(b, fore, f"knee{sx}", (sx * 3.8, 9.4, -6.4), 2.2, "hide_lt", rng)
        limb(b, cannon, f"fca{sx}", ((sx * 3.7, 5.4, -5.6, 1.15), (sx * 3.7, 3.4, -5.6, 1.1)), LEG, rng, bumps=2, bump_mats=BONES_PALE)
        for k in range(8):                                                  # 小臂后缘垂下的长毛
            strand(b, fore, f"feather{sx}{k}", (sx * rng.uniform(3.2, 4.4), rng.uniform(8.8, 10.4), rng.uniform(-4.4, -3.6)),
                   rng.uniform(3.0, 5.5), 1.1, rng)
        for k in range(7):                                                  # Round 3：球节上方一圈垂下盖住蹄冠的长毛（毛靴）
            a = rng.uniform(0.0, 2.0 * math.pi)
            strand(b, cannon, f"fetlock_fur{sx}{k}", (sx * 3.7 + 1.1 * math.cos(a), rng.uniform(4.4, 5.2), -5.6 + 1.1 * math.sin(a)),
                   rng.uniform(2.2, 3.4), 0.95, rng, sway=0.25)
        del fl
    hoof_pair(b, "f_hoof", 3.0, -7.0, -3.8)


def part_hindlegs(b: kit.Builder) -> None:
    """后腿：瘦骨大腿 → 膝 → 小腿向后折到飞节 → 细长掌骨 → 球节 → 蹄；几乎没毛，骨节凸出。"""
    rng = random.Random(SEED + 9)
    for sx in (1, -1):
        thigh, shank, cannon = (sided(n, sx) for n in ("h_thigh", "h_shank", "h_cannon"))
        limb(b, thigh, f"hth{sx}", ((sx * 4.6, 19.4, 11.0, 3.0), (sx * 4.4, 16.0, 10.4, 2.4), (sx * 4.2, 13.0, 9.6, 1.9)), LEG, rng,
             bumps=4, bump_mats=BONES_PALE)
        jcube(b, thigh, f"stifle{sx}", (sx * 4.2, 13.2, 8.4), 2.2, "bone_dk", rng)
        limb(b, shank, f"hsh{sx}", ((sx * 4.2, 13.0, 9.6, 1.7), (sx * 4.0, 10.0, 11.2, 1.45), (sx * 3.8, 7.8, 12.4, 1.3)), LEG, rng,
             bumps=3, bump_mats=BONES_PALE)
        for k in range(3):                                                  # 飞节：向后顶出的骨角
            jcube(b, shank, f"hock{sx}{k}", (sx * 3.9, 8.8 - 0.5 * k, 13.6 + 0.3 * k), 1.6 - 0.2 * k, "bone", rng, jit=0.05)
        limb(b, cannon, f"hca{sx}", ((sx * 3.8, 7.8, 12.4, 1.15), (sx * 3.8, 3.4, 11.9, 1.1)), LEG, rng, bumps=2, bump_mats=BONES_PALE)
    hoof_pair(b, "h_hoof", 3.0, 10.2, 13.4)


def part_tail(b: kit.Builder) -> None:
    """尾：细长，三节垂下，尾根两侧有旧毛，尾梢一大把枯毛。"""
    rng = random.Random(SEED + 10)
    limb(b, "tail_1", "tail1", ((0.0, 19.4, 14.0, 1.0), (0.0, 18.2, 15.6, 0.9)), LEG, rng, bumps=2, bump_mats=FURS)
    limb(b, "tail_2", "tail2", ((0.0, 18.2, 15.6, 0.9), (0.0, 16.2, 16.6, 0.8)), LEG, rng, bumps=2, bump_mats=FURS)
    limb(b, "tail_3", "tail3", ((0.0, 16.2, 16.6, 0.8), (0.0, 13.4, 17.2, 0.7)), LEG, rng, bumps=2, bump_mats=FURS)
    for k in range(22):                                                     # 尾梢一大把枯毛
        a = rng.uniform(0.0, 2.0 * math.pi)
        r = rng.uniform(0.0, 1.5)
        strand(b, "tail_4", f"tuft{k}", (r * math.cos(a), 14.0 - 0.4 * (k % 3), 17.2 + r * math.sin(a)), rng.uniform(4.5, 8.5),
               rng.uniform(1.0, 1.4), rng, sway=0.6)


PARTS = {
    "front": part_front,
    "neck": part_neck,
    "ribcage": part_ribcage,
    "hips": part_hips,
    "head": part_head,
    "jaw": part_jaw,
    "horns": part_horns,
    "forelegs": part_forelegs,
    "hindlegs": part_hindlegs,
    "tail": part_tail,
}

SPEC = kit.CreatureSpec(
    model_id="duan_jiao_niu",
    title="断角野牛",
    class_name="DuanJiaoNiu",
    creature_dir=CREATURE_DIR,
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    min_bones=32,
    soft_over=SOFT_OVER,
    asym_bones=ASYM_BONES,
    round_label="round 3/3",
    # 比例量尺：参考图是 CPA 生图（上排正 / 侧 / 背），侧视在上排中间；肩前缘 = 肩毛团最前缘（颈与肩交界处），
    # 臀后缘 = 髋 / 大腿后缘（尾根前），鼻尖手动指定（侧视里弯角比鼻子更靠前）。数字是裁剪后参考图上的像素列，
    # 量尺线会画进 check.png，点得不对一眼能看出来。模型侧：躯干前缘 = 肩 / 肩峰骨的最前，后缘 = 骨盆骨的最后。
    proportion={
        "ref_crop": (350, 0, 1215, 548),
        "ref_torso_x": (210.0, 775.0),
        "ref_nose_x": 62.0,
        "front_bones": ("spine_upper", "hump"),
        "rear_bones": ("hips",),
    },
    ref_prompt=(
        "an emaciated wasteland bison, bony and starved: high humped shoulders, sunken flanks, every rib visible, hip "
        "bones sticking up through the hide, sparse knotted clumps of matted fur only on the neck, hump and haunches, "
        "old scars on the ribs and back, long low face with a tuft of beard, its RIGHT horn intact curving outward and "
        "up while its LEFT horn is broken off at the middle leaving a jagged pale stump, thin tail with a tuft of dead "
        "hair; detail close-ups: head with broken horn, rib cage, hump and matted fur, hip bone, hoof"
    ),
)

if __name__ == "__main__":
    kit.run_cli(SPEC)
