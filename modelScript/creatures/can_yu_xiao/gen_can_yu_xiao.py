#!/usr/bin/env python3
"""残羽鸮（can_yu_xiao）Blockbench .bbmodel 程序化生成器 —— Round 3/3 几何终轮。
Round 3 新增：脸盘同心纹与放射羽纹 + 眉羽 + 右眼高光、胸腹裸皮处露出的肋骨与胸骨、翅腕小翼羽、跗跖羽根桩、趾节关节。

正典依据：docs/worldview.md §十三 荒野。任务卡：末世生物批次 #24（.task-wasteland-creatures.md）：
羽毛脱落一半的大猫头鹰，脸盘裂开、一只眼浑浊，爪子过长，夜里在荒野悄无声息地扑食。

外形：站立的大猫头鹰（高约 44px），长腿、翅膀半张下垂。身上的羽毛只剩一半——胸腹和大腿大片裸露灰粉色皱皮，
翅骨外露；翅上的飞羽稀稀拉拉，有的只剩光秃秃的羽轴、有的折断；尾羽稀疏。脸盘是两个圆盘中间裂开一道深缝，
右眼完好（金黄），左眼浑浊（乳白、带裂纹）；弯钩喙、两簇耳羽。腿细长、跗跖覆鳞，脚趾末端的爪子过长（深色角质，弯钩）。

做法：体素场（creature_kit.Field），只算右半再镜像；羽毛用 1px 粗的胶囊扇形排开（随机缺掉约四成、部分折断、
末段只剩羽轴）。左眼的浑浊是做完镜像后把左眼骨上的立方体改涂色，所以眼骨列入「刻意不对称」。
骨骼（飞行类 ≥ 24）见 BONES。

用法：
    python3 modelScript/creatures/can_yu_xiao/gen_can_yu_xiao.py --self-test
    python3 modelScript/creatures/can_yu_xiao/gen_can_yu_xiao.py --part wings
    python3 modelScript/creatures/can_yu_xiao/gen_can_yu_xiao.py            # 全量：模型 + 接触表 + check.png
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "creatures" / "_kit"))
import creature_kit as kit  # noqa: E402
from creature_kit import Field, Puppet, blotch, hash_cell, pick  # noqa: E402

CREATURE_DIR = Path(__file__).resolve().parent
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/can_yu_xiao")
PARTS_DIR = REVIEW_DIR / "parts"
MODEL_ID = "can_yu_xiao"

MATS = {
    "feather":     (112, 96, 78),     # 羽毛：灰褐三档
    "feather_dk":  (74, 62, 52),
    "feather_lt":  (160, 142, 116),
    "bare":        (150, 118, 110),   # 脱羽后裸露的灰粉皱皮
    "bare_dk":     (112, 86, 82),
    "quill":       (200, 184, 150),   # 光秃秃的羽轴
    "root":        (150, 98, 92),     # 掉羽处留下的羽根桩（粉红带血色的短管）
    "disc":        (176, 160, 132),   # 脸盘
    "disc_dk":     (120, 104, 84),
    "crack":       (34, 28, 26),      # 脸盘裂缝
    "eye":         (216, 150, 40),    # 右眼：金黄
    "eye_ring":    (96, 62, 24),
    "pupil":       (16, 12, 12),
    "eye_cloud":   (210, 210, 200),   # 左眼：浑浊乳白
    "eye_cloud_dk": (150, 150, 144),
    "beak":        (60, 52, 48),      # 钩喙：深色角质
    "beak_lt":     (120, 104, 84),
    "leg":         (128, 112, 100),   # 跗跖：覆鳞灰褐
    "leg_dk":      (86, 72, 64),
    "talon":       (46, 38, 36),      # 过长的爪
    "talon_tip":   (120, 108, 96),
}

# ───────────────────────────────────────────────────────────── 骨骼树
BONES: list[tuple[str, str | None, tuple[float, float, float], str]] = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("body_hips", "root", (0.0, 18.0, 3.0), "臀 / 腰（腿和尾的根）：大片裸皮"),
    ("body_chest", "body_hips", (0.0, 25.0, -1.0), "胸（翅肩的根）：残羽 + 裸皮"),
    ("neck", "body_chest", (0.0, 32.5, -2.0), "颈"),
    ("head", "neck", (0.0, 35.5, -3.5), "头"),
    ("beak_low", "head", (0.0, 33.8, -9.6), "下喙"),
    ("disc_r", "head", (3.6, 36.0, -8.6), "右脸盘（完好）"),
    ("disc_l", "head", (-3.6, 36.0, -8.6), "左脸盘（裂开的一半）"),
    ("eye_r", "head", (3.4, 36.8, -9.0), "右眼（金黄）"),
    ("eye_l", "head", (-3.4, 36.8, -9.0), "左眼（浑浊乳白）"),
    ("tuft_r", "head", (3.2, 40.5, -3.0), "右耳羽"),
    ("tuft_l", "head", (-3.2, 40.5, -3.0), "左耳羽"),
    ("wing_1_r", "body_chest", (6.0, 29.0, 0.0), "右翅·肩臂"),
    ("wing_1_l", "body_chest", (-6.0, 29.0, 0.0), "左翅·肩臂"),
    ("wing_2_r", "wing_1_r", (13.0, 24.0, 3.0), "右翅·前臂"),
    ("wing_2_l", "wing_1_l", (-13.0, 24.0, 3.0), "左翅·前臂"),
    ("wing_3_r", "wing_2_r", (18.0, 19.0, 6.0), "右翅·腕手"),
    ("wing_3_l", "wing_2_l", (-18.0, 19.0, 6.0), "左翅·腕手"),
    ("sec_r", "wing_2_r", (13.0, 24.0, 3.0), "右次级飞羽"),
    ("sec_l", "wing_2_l", (-13.0, 24.0, 3.0), "左次级飞羽"),
    ("prim_r", "wing_3_r", (18.0, 19.0, 6.0), "右初级飞羽"),
    ("prim_l", "wing_3_l", (-18.0, 19.0, 6.0), "左初级飞羽"),
    ("tail_1", "body_hips", (0.0, 16.0, 7.0), "尾 1（尾羽根）"),
    ("tail_2", "tail_1", (0.0, 11.0, 12.0), "尾 2（尾羽梢）"),
    ("leg_1_r", "body_hips", (3.4, 17.0, 1.0), "右腿·大腿"),
    ("leg_1_l", "body_hips", (-3.4, 17.0, 1.0), "左腿·大腿"),
    ("leg_2_r", "leg_1_r", (3.6, 11.0, 0.0), "右腿·跗跖"),
    ("leg_2_l", "leg_1_l", (-3.6, 11.0, 0.0), "左腿·跗跖"),
    ("foot_r", "leg_2_r", (3.8, 1.4, 1.2), "右脚·趾"),
    ("foot_l", "leg_2_l", (-3.8, 1.4, 1.2), "左脚·趾"),
    ("talon_r", "foot_r", (3.8, 1.0, -5.0), "右脚·过长的爪"),
    ("talon_l", "foot_l", (-3.8, 1.0, -5.0), "左脚·过长的爪"),
]
MIN_BONES = 24                      # 任务卡「骨骼数下限」：飞行类 ≥ 24
ASYM_BONES = frozenset({"eye_l", "eye_r", "disc_l"})       # 左眼浑浊、左脸盘多一道撕裂（刻意不对称）


def new_puppet() -> Puppet:
    return Puppet(MATS, BONES)


def new_field(p: Puppet) -> Field:
    return Field(p)


# ───────────────────────────────────────────────────────────── 材质函数
def plumage_fn(i: int, j: int, k: int) -> str:
    """身体：残羽（灰褐三档，胸前横纹）与脱羽后裸露的灰粉皱皮成片相间（约三成裸）。"""
    if blotch(i, j, k, 4, 71) < 30:
        return "bare_dk" if blotch(i, j, k, 2, 73) < 30 else "bare"
    if j % 3 == 0 and blotch(i, j, k, 3, 75) < 45:
        return "feather_dk"
    return pick(blotch(i, j, k, 2, 77), (("feather", 45), ("feather_dk", 25), ("feather_lt", 30)))


def skin_fn(i: int, j: int, k: int) -> str:
    """羽片底下的打底色：掉羽的地方是灰粉皱皮（深浅成片），没掉的地方是羽下的暗影。"""
    if molted(i, j, k, 40):
        return "bare_dk" if blotch(i, j, k, 2, 73) < 35 else "bare"
    return "feather_dk"


def leg_fn(i: int, j: int, k: int) -> str:
    if j >= 9 and blotch(i, j, k, 3, 79) < 55:
        return "bare"                                     # 大腿一半裸皮
    return "leg_dk" if (j + k) % 3 == 0 else "leg"


def vane_fn(i: int, j: int, k: int) -> str:
    """羽片的颜色：横向一道一道的深浅条纹（猫头鹰羽毛的横斑），再混一点浅色羽缘。"""
    if j % 4 == 0:
        return "feather_dk"
    return pick(blotch(i, j, k, 2, 81), (("feather", 50), ("feather_dk", 22), ("feather_lt", 28)))


def blade(f: Field, bone: str, a, b, half_width: float, mat_fn=vane_fn, seed: int = 0, axis: str = "x",
          ragged: float = 0.2, bare_from: float = 1.1) -> None:
    """一片羽：沿羽轴 a→b 画一条亮色羽轴，两侧各 half_width 宽的羽面（宽度沿 axis 方向，厚 1px），
    越近梢越窄；梢部随机缺格，留下参差的缺口和裂隙。bare_from < 1 时那之后只剩光羽轴。
    相邻几片羽请把 a 的深度（z 或 x）错开 1px 并按「上层先画」的顺序调用：先到先得，就叠成瓦片。"""
    ax, ay, az = a
    bx, by, bz = b
    steps = int(max(abs(bx - ax), abs(by - ay), abs(bz - az)) * 2) + 2
    for s in range(steps + 1):
        t = s / steps
        x, y, z = ax + (bx - ax) * t, ay + (by - ay) * t, az + (bz - az) * t
        reach = half_width * (1.0 - t ** 2.2) if t > 0.12 else half_width * 0.6
        ci, cj, ck = math.floor(x), math.floor(y), math.floor(z)
        for d in range(-int(reach + 0.99), int(reach + 0.99) + 1):
            if d == 0:
                f.put(ci, cj, ck, bone, "quill" if t > 0.55 else mat_fn(ci, cj, ck))
                continue
            if abs(d) > reach + 0.35 or t > bare_from:
                continue
            ni, nk = (ci + d, ck) if axis == "x" else (ci, ck + d)
            if t > 0.4 and hash_cell(ni, cj, nk, seed + 7) < (ragged + 0.45 * (t - 0.4)) * 100:
                continue                                   # 梢部的缺口 / 裂隙
            f.put(ni, cj, nk, bone, mat_fn(ni, cj, nk))


def feather_stub(f: Field, bone: str, a, length: float = 2.2) -> None:
    """掉了羽的位置：一小截粉红的羽根桩从裸皮里戳出来。"""
    f.capsule(bone, a, (a[0] + 0.3, a[1] - length, a[2] + 0.4), 0.55, lambda i, j, k: "root", r_end=0.45)


def molted(i: float, j: float, k: float, share: int = 32) -> bool:
    """这一格附近的羽毛已经掉光（按 4px 一块的斑，掉得成片）。"""
    return blotch(int(i), int(j), int(k), 4, 71) < share


def plumage_rows(f: Field, bone: str, center, radii, y_top: float, y_bottom: float, step: float, thetas, seed: int,
                 length: float, share: int = 32) -> None:
    """围着一个椭球一圈圈贴「下垂的羽片」：每一排比上一排低 step，上排的羽梢盖住下排的羽根（先画上排），
    每片长短不一、位置随机缺掉（掉羽处露出椭球本身的裸皮，并留羽根桩）。
    thetas：绕 y 轴的角度（0 = 正前方 -z，π/2 = +x 侧，π = 正后方）。"""
    cx, cy, cz = center
    rx, ry, rz = radii
    y, row = y_top, 0
    while y >= y_bottom:
        s2 = 1.0 - ((y - cy) / ry) ** 2
        if s2 > 0.04:
            s = math.sqrt(s2)
            for n, th in enumerate(thetas):
                sn, cs = math.sin(th), math.cos(th)
                bx, bz = cx + rx * s * sn, cz - rz * s * cs
                h = hash_cell(int(bx * 2), int(y * 2) + row, int(bz * 2), seed)
                nx, nz = sn / rx, -cs / rz
                norm = math.hypot(nx, nz) or 1.0
                nx, nz = nx / norm, nz / norm
                base = (bx + nx * 0.7, y, bz + nz * 0.7)
                if molted(base[0], y, base[2], share):
                    if h < 45:
                        feather_stub(f, bone, (bx + nx * 0.3, y, bz + nz * 0.3))
                    continue
                run = length * (0.65 + 0.7 * hash_cell(n, row, seed, 5) / 100) - (2.2 if h < 22 else 0.0)   # 少数折短
                tip = (base[0] + nx * 1.1, y - run, base[2] + nz * 1.3)
                blade(f, bone, base, tip, 2.1 if abs(cs) > 0.55 else 1.8, seed=seed + row, axis="x" if abs(cs) > 0.55 else "z",
                      ragged=0.18, bare_from=0.7 if h > 82 else 1.1)
        y -= step
        row += 1


# ───────────────────────────────────────────────────────────── 分部件
def part_body(p: Puppet, f: Field) -> None:
    """躯干：臀 + 胸两个椭球打底（掉羽的地方是裸皮，其余是羽下的暗影），再一圈圈贴下垂的羽片，上排盖下排。"""
    f.ellipsoid("body_hips", (0.0, 18.0, 3.0), (5.0, 5.5, 5.5), skin_fn)
    f.ellipsoid("body_chest", (0.0, 25.0, -1.0), (6.6, 8.0, 6.4), skin_fn)
    f.ellipsoid("neck", (0.0, 32.0, -2.0), (4.2, 2.6, 3.6), skin_fn)
    full = [math.pi * n / 9 for n in range(10)]                 # 右半圈：正前 → 右侧 → 正后
    plumage_rows(f, "body_chest", (0.0, 25.0, -1.0), (6.6, 8.0, 6.4), 31.5, 17.0, 2.6, full, 41, 6.5, share=34)
    plumage_rows(f, "body_hips", (0.0, 18.0, 3.0), (5.0, 5.5, 5.5), 21.5, 13.0, 2.6, full, 43, 5.5, share=40)
    plumage_rows(f, "neck", (0.0, 32.0, -2.0), (4.2, 2.6, 3.6), 33.5, 30.5, 1.6, full, 47, 3.0, share=25)


def part_head(p: Puppet, f: Field) -> None:
    """头：圆脑壳 + 两个圆盘（中间裂开一道又深又长的缝，盘缘一圈参差的羽）+ 钩喙 + 下喙 + 一簇簇耳羽；眼在 part_eyes。"""
    f.ellipsoid("head", (0.0, 35.5, -3.5), (6.8, 6.2, 5.6), plumage_fn)
    f.ellipsoid("disc_r", (3.5, 36.0, -8.2), (3.8, 4.8, 1.3), lambda i, j, k: "disc_dk" if blotch(i, j, k, 2, 83) < 35 else "disc")
    for j in range(32, 42):                                  # 脸盘正中的裂缝：x = 0 一列深色，贯穿上下，两侧缝沿再压一格暗边
        for k in range(-10, -8):
            f.put(0, j, k, "disc_r", "crack", overwrite=True)
        if hash_cell(j, 0, 0, 91) < 70:
            f.put(1, j, -10, "disc_r", "disc_dk", overwrite=True)
    for n in range(16):                                      # 盘缘一圈放射状的短羽：长短不一、颜色深浅相间
        phi = math.pi * 2 * n / 16
        if hash_cell(n, 3, 0, 93) < 14:
            continue                                         # 缘上有几处光秃
        rim = (3.5 + 3.9 * math.cos(phi), 36.0 + 4.9 * math.sin(phi), -8.0)
        reach = 1.6 + 1.6 * hash_cell(n, 4, 0, 95) / 100
        tip = (rim[0] + math.cos(phi) * reach, rim[1] + math.sin(phi) * reach, rim[2] + 0.8)
        f.capsule("head", rim, tip, 0.8, lambda i, j, k: "feather_dk" if (i + j) % 2 else "feather", r_end=0.4)
    f.capsule("head", (0.0, 34.6, -9.0), (0.0, 31.6, -12.4), 1.5, lambda i, j, k: "beak_lt" if blotch(i, j, k, 2, 85) < 20 else "beak", r_end=0.6)
    f.capsule("beak_low", (0.0, 33.4, -9.2), (0.0, 32.0, -11.2), 1.0, lambda i, j, k: "beak", r_end=0.5)
    for n in range(3):                                       # 耳羽：三片叠着向上翘，最外一片折断
        a = (3.0 + 0.5 * n, 40.0, -3.0 + 0.9 * n)
        blade(f, "tuft_r", a, (a[0] + 1.0 + 0.6 * n, 46.0 - 1.5 * n - (2.0 if n == 2 else 0.0), a[2] - 0.5), 1.0, seed=60 + n, ragged=0.1)


def part_eyes(p: Puppet, f: Field) -> None:
    """眼：右眼金黄带黑瞳，左眼是镜像出来之后改涂的乳白浑浊（见 clouded_left_eye）。"""
    def eye(i, j, k):
        dx, dy, dz = i + 0.5 - 3.4, j + 0.5 - 36.8, k + 0.5 + 9.4
        if dz < -0.6 and abs(dx) < 1.0 and abs(dy) < 1.0:
            return "pupil"
        return "eye_ring" if math.sqrt(dx * dx + dy * dy + dz * dz) > 1.8 else "eye"

    f.ellipsoid("eye_r", (3.4, 36.8, -9.4), (2.0, 2.0, 1.4), eye, surface=False, overwrite=True)


def clouded_left_eye(p: Puppet) -> None:
    """做完镜像后，把左眼骨上的立方体改涂成浑浊的乳白（瞳孔变成一团灰，外圈不变）。"""
    swap = {"eye": "eye_cloud", "pupil": "eye_cloud_dk"}
    names = list(MATS)
    per_row = p.rig.tex // p.rig.swatch
    inverse = {v: k for k, v in swap.items()}
    for el in p.rig.elements:
        if p.rig.bones["eye_l"]["children"].count(el["uuid"]) == 0:
            continue
        uv = el["faces"]["north"]["uv"]
        mat = names[int(uv[1] // p.rig.swatch) * per_row + int(uv[0] // p.rig.swatch)]
        if mat in swap:
            el["faces"] = p.rig._faces(swap[mat])
    del inverse


def torn_left_face(p: Puppet) -> None:
    """左半边脸多一道从眉心撕到眼眶的深裂口（贴在脸盘表面的几格深色），和一条横过浑浊眼珠的白翳裂纹；右半边没有。"""
    for x, y in ((-1.5, 41.0), (-2.0, 40.0), (-2.5, 39.0)):
        p.box("disc_l", f"tear_{int(-x * 10)}_{int(y)}", (x - 0.5, y, -11.0), (x + 0.5, y + 1.0, -10.0), "crack")
    for x in (-4.9, -3.9, -2.9):
        p.box("eye_l", f"film_{int(-x * 10)}", (x, 36.5, -12.0), (x + 1.0, 37.5, -11.0), "eye_cloud_dk")


WING_JOINTS = ((6.0, 29.0, 0.0), (13.0, 24.0, 3.0), (18.0, 19.0, 6.0), (21.0, 14.0, 9.0))   # 肩 / 肘 / 腕 / 掌尖（右翅）


def part_wings(p: Puppet, f: Field) -> None:
    """翅骨与飞羽：肩臂 / 前臂 / 腕手三段翅骨（外露的粉皮 + 少量残羽），次级飞羽挂在前臂后缘、初级飞羽从腕手扇形展开；
    飞羽随机缺掉约四成、部分折断、末段只剩羽轴。"""
    s, e, w, t = WING_JOINTS
    f.capsule("wing_1_r", s, e, 2.5, lambda i, j, k: "bare" if blotch(i, j, k, 3, 91) < 55 else "feather_dk", r_end=2.1, surface=True)
    f.capsule("wing_2_r", e, w, 2.1, lambda i, j, k: "bare_dk" if blotch(i, j, k, 3, 93) < 40 else "feather", r_end=1.6, surface=True)
    f.capsule("wing_3_r", w, t, 1.6, lambda i, j, k: "bare" if blotch(i, j, k, 2, 95) < 50 else "feather_dk", r_end=1.2)
    # 覆羽：两段翅臂的前缘各贴三排短羽片，一排比一排低、上排盖下排，掉羽处露出翅臂的粉皮并留羽根桩
    for (lo, hi, bone, salt) in ((s, e, "wing_1_r", 120), (e, w, "wing_2_r", 130)):
        for row in range(3):
            for n in range(5):
                u = n / 4
                base = (lo[0] + (hi[0] - lo[0]) * u, lo[1] + (hi[1] - lo[1]) * u + 1.6 - 2.1 * row,
                        lo[2] + (hi[2] - lo[2]) * u - 1.5 - 0.5 * (2 - row))
                h = hash_cell(n, row, 0, salt)
                if h < 30:
                    feather_stub(f, bone, (base[0], base[1] - 1.0, base[2] + 1.2))
                    continue
                blade(f, bone, base, (base[0] + 0.5, base[1] - 4.4 + 0.02 * h - (1.8 if h < 45 else 0.0), base[2] + 0.5), 1.4,
                      seed=salt + row, ragged=0.2)
    # 次级飞羽：前臂后缘一排下垂的长羽片，前后错开 1px 叠成瓦；缺的位置留一截羽根桩，折断的短一截
    for n in range(8):
        u = n / 7
        a = (e[0] + (w[0] - e[0]) * u, e[1] + (w[1] - e[1]) * u - 1.0, e[2] + (w[2] - e[2]) * u + 0.8 + 0.6 * n)
        if hash_cell(n, 1, 0, 101) < 24:
            feather_stub(f, "sec_r", a)
            continue
        length = 10.0 + 2.5 * hash_cell(n, 2, 0, 103) / 100 - (4.0 if hash_cell(n, 3, 0, 105) < 25 else 0.0)        # 少数折断
        b = (a[0] + 0.8 + 0.5 * u, a[1] - length, a[2] + 1.0)
        blade(f, "sec_r", a, b, 1.6, seed=140 + n, ragged=0.22, bare_from=0.62 if hash_cell(n, 4, 0, 107) < 35 else 1.1)
    # 初级飞羽：腕手上九根扇形展开，越靠外越长越窄，同样前后错开；有的只剩光羽轴
    for n in range(9):
        fan = n / 8
        a = (w[0] + (t[0] - w[0]) * 0.5 + 0.5 * n, w[1] + (t[1] - w[1]) * 0.5, w[2] + (t[2] - w[2]) * 0.5 + 0.5 * n)
        if hash_cell(n, 5, 0, 109) < 22:
            feather_stub(f, "prim_r", a)
            continue
        length = 15.0 - 4.0 * abs(fan - 0.3) - (5.0 if hash_cell(n, 6, 0, 111) < 28 else 0.0)
        b = (a[0] + 1.0 + 4.5 * fan, a[1] - length * (1.0 - 0.35 * fan), a[2] + 1.0 + 1.5 * fan)
        blade(f, "prim_r", a, b, 1.7 - 0.5 * fan, seed=160 + n, ragged=0.25, bare_from=0.55 if hash_cell(n, 7, 0, 113) < 40 else 1.1)


def part_tail(p: Puppet, f: Field) -> None:
    """尾羽：五片宽羽片从尾根往后下垂，上下错开叠着，边缘参差，其中一片只剩光羽轴。"""
    for bone, tips, base in (("tail_1", ((0.0, 7.0, 15.0), (2.4, 7.6, 14.0), (4.2, 9.0, 12.5)), 2.0),
                             ("tail_2", ((1.2, 2.0, 18.5), (3.2, 4.0, 16.5)), 1.0)):
        for n, tip in enumerate(tips):
            a = (min(base, tip[0] * 0.3), 15.0 - 0.6 * n, 7.0 + 0.4 * n)
            blade(f, bone, a, tip, 1.5, seed=180 + n, ragged=0.22, bare_from=0.45 if (n == 1 and bone == "tail_2") else 1.1)


def part_legs(p: Puppet, f: Field) -> None:
    """腿：大腿（半裸皮）+ 细长覆鳞的跗跖 + 脚趾（前三后一）+ 过长的深色弯爪。"""
    f.capsule("leg_1_r", (3.4, 17.0, 1.0), (3.6, 11.0, 0.0), 2.6, leg_fn, r_end=1.8, surface=True)
    f.capsule("leg_2_r", (3.6, 11.0, 0.0), (3.8, 1.8, 1.2), 1.5, leg_fn, r_end=1.2)
    toes = ((2.4, 0.8, -4.6), (3.8, 0.8, -5.4), (5.4, 0.8, -4.6), (3.8, 0.8, 5.0))
    for tip in toes:
        f.capsule("foot_r", (3.8, 1.8, 1.2), tip, 1.0, lambda i, j, k: "leg_dk")
    # 前爪：从趾尖先向前抬起再弯钩向下，总长约 9px（原来 5px 直棍），分四段渐细，尖端浅色
    for tip in toes[:3]:
        path = (tip, (tip[0], 1.9, tip[2] - 2.8), (tip[0], 1.7, tip[2] - 5.6), (tip[0], 0.4, tip[2] - 8.0), (tip[0], 0.0, tip[2] - 9.0))
        for n in range(len(path) - 1):
            f.capsule("talon_r", path[n], path[n + 1], 0.95 - 0.17 * n, lambda i, j, k, n=n: "talon_tip" if n == 3 else "talon",
                      r_end=0.95 - 0.17 * (n + 1))
    back = toes[3]                                              # 后趾的爪：向后伸 6px 再弯钩，同样细长
    for n, (a, b) in enumerate(((back, (back[0], 1.8, back[2] + 2.4)), ((back[0], 1.8, back[2] + 2.4), (back[0], 1.0, back[2] + 5.0)),
                                ((back[0], 1.0, back[2] + 5.0), (back[0], 0.0, back[2] + 6.4)))):
        f.capsule("talon_r", a, b, 0.95 - 0.2 * n, lambda i, j, k, n=n: "talon_tip" if n == 2 else "talon", r_end=0.95 - 0.2 * (n + 1))
    # 大腿外一圈下垂的羽片（裤腿），掉得比身上更多，只剩几片
    plumage_rows(f, "leg_1_r", (3.4, 14.5, 1.0), (3.0, 3.2, 3.0), 17.5, 12.0, 2.5,
                 [math.pi * n / 4 for n in range(-3, 4)], 53, 3.8, share=52)


def _is_skin(f: Field, i: int, j: int, k: int) -> bool:
    cell = f.cells.get((i, j, k))
    return cell is not None and cell[1].startswith("bare")


def part_ruff(p: Puppet, f: Field) -> None:
    """脸盘与眼的细部（Round 3）：盘面一圈暗色同心纹 + 放射的浅羽纹，眼上一道眉羽，右眼一粒高光。
    只改已经存在的盘面格（overwrite），所以必须排在 head / eyes 之后。"""
    cx, cy = 3.5, 36.0
    for n in range(72):
        phi = math.tau * n / 72
        for frac, mat in ((0.62, "disc_dk"), (0.88, "feather_dk")):
            i, j = math.floor(cx + 3.8 * frac * math.cos(phi)), math.floor(cy + 4.8 * frac * math.sin(phi))
            for k in (-10, -9):
                cell = f.cells.get((i, j, k))
                if cell is not None and cell[1] in ("disc", "disc_dk"):
                    f.put(i, j, k, "disc_r", mat, overwrite=True)
    for n in range(10):                                           # 放射状浅羽纹：从内环到外环，疏疏落落
        if hash_cell(n, 8, 0, 121) < 30:
            continue
        phi = math.tau * n / 10 + 0.3
        for frac in (0.3, 0.4, 0.5):
            i, j = math.floor(cx + 3.8 * frac * math.cos(phi)), math.floor(cy + 4.8 * frac * math.sin(phi))
            for k in (-10, -9):
                cell = f.cells.get((i, j, k))
                if cell is not None and cell[1] in ("disc", "disc_dk"):
                    f.put(i, j, k, "disc_r", "feather_lt", overwrite=True)
    f.capsule("head", (1.4, 39.8, -9.4), (5.8, 40.6, -8.4), 0.8, lambda i, j, k: "feather_dk" if (i + k) % 2 else "feather",
              r_end=0.6, overwrite=True)                           # 眉羽：压在眼上方，一端翘起
    f.put(3, 37, -11, "eye_r", "quill", overwrite=True)           # 右眼高光


def part_ribs(p: Puppet, f: Field) -> None:
    """胸腹掉羽处露出的肋骨（Round 3）：沿胸椭球表面一道道弧形细骨，只画在裸皮格上，被羽毛盖住的地方不画。"""
    cx, cy, cz, rx, ry, rz = 0.0, 25.0, -1.0, 6.6, 8.0, 6.4
    for j in (19, 21, 23, 25):
        s = math.sqrt(max(0.0, 1.0 - ((j + 0.5 - cy) / ry) ** 2))
        for n in range(20):
            th = math.pi * (0.1 + 0.6 * n / 19)
            i, k = math.floor(cx + (rx * s + 0.4) * math.sin(th)), math.floor(cz - (rz * s + 0.4) * math.cos(th))
            for dk in (0, -1):
                if _is_skin(f, i, j, k + dk) or _is_skin(f, i - 1, j, k + dk):
                    f.put(i, j, k + dk, "body_chest", "quill", overwrite=True)
    for j in range(20, 29):                                       # 胸骨一线：正中偏前，只在裸皮格上
        for k in (-8, -7):
            if _is_skin(f, 0, j, k):
                f.put(0, j, k, "body_chest", "quill", overwrite=True)


def part_limb_detail(p: Puppet, f: Field) -> None:
    """翅腕的小翼羽、跗跖上冒出的羽根桩、趾节关节的疙瘩（Round 3）。"""
    w = WING_JOINTS[2]
    for n in range(3):                                            # 小翼羽：翅腕前缘三片短羽
        a = (w[0] + 0.4 + 0.6 * n, w[1] + 1.4 - 0.4 * n, w[2] - 1.6 - 0.5 * n)
        blade(f, "wing_3_r", a, (a[0] + 1.2 + 0.6 * n, a[1] - 3.4 + 0.5 * n, a[2] - 1.0), 0.9, seed=200 + n, ragged=0.15)
    for n, (y, z) in enumerate(((10.2, -0.6), (8.2, 0.2), (6.2, 0.8), (4.4, 1.2))):     # 跗跖上残留的羽根桩
        feather_stub(f, "leg_2_r", (4.2 + 0.1 * n, y, z), length=1.6)
    for tip in ((2.4, 0.8, -4.6), (3.8, 0.8, -5.4), (5.4, 0.8, -4.6)):                  # 趾节：趾中段鼓一个关节
        f.capsule("foot_r", (tip[0], 1.8, tip[2] * 0.5 + 0.6), (tip[0], 1.9, tip[2] * 0.5 - 0.2), 1.1, lambda i, j, k: "leg", overwrite=True)


PART_FUNCS = {"body": part_body, "head": part_head, "eyes": part_eyes, "wings": part_wings, "tail": part_tail, "legs": part_legs,
              "ruff": part_ruff, "ribs": part_ribs, "limb_detail": part_limb_detail}
BUILD_ORDER = ("legs", "eyes", "head", "body", "wings", "tail", "ruff", "ribs", "limb_detail")   # 腿和眼先占位，身体后铺（先到先得）；Round 3 细部最后改涂


def build(parts) -> Puppet:
    p = new_puppet()
    f = new_field(p)
    for fn in sorted(parts, key=lambda fn: BUILD_ORDER.index(fn.__name__.removeprefix("part_"))):
        fn(p, f)
    f.emit()
    if part_eyes in parts:
        clouded_left_eye(p)
        if part_head in parts:
            torn_left_face(p)
    return p


GATES = kit.Gates("残羽鸮 / can_yu_xiao", MATS, asym=ASYM_BONES, soft_over=frozenset(), min_bones=MIN_BONES,
                  build_small=lambda: build([part_tail]))

VIEWS = (
    ("FRONT 正视（相机在 -z，看裂开的脸盘 / 一黄一浊的眼）", 180.0, 6.0),
    ("3/4 前右斜视（相机在 -z / +x 上方）", 150.0, 18.0),
    ("LEFT 左侧视（相机在 -x，头朝画面左）", 90.0, 0.0),
    ("RIGHT 右侧视（相机在 +x，头朝画面右）", -90.0, 0.0),
    ("TOP 俯视（相机在 +y，头朝画面下方，看翅展）", 180.0, 89.9),
    ("BACK 背视（相机在 +z，看尾羽 / 翅背）", 0.0, 6.0),
)
PART_VIEWS = (("FRONT -z", 180.0, 6.0), ("3/4 前右", 150.0, 18.0), ("LEFT -x", 90.0, 0.0))
FRAMING_VIEWS = (("FRONT 正视", 180.0, 6.0), ("3/4 前右", 150.0, 18.0))


def full_run() -> None:
    p = build(list(PART_FUNCS.values()))
    GATES.self_test(p)
    total = GATES.run(p)
    if total:
        raise SystemExit(f"门禁仍有 {total} 处违例，不出图")
    model = CREATURE_DIR / "CanYuXiao.bbmodel"
    kit.write_model(p, model, "CanYuXiao")
    print(f"✓ {model}: {p.total()} cubes / {len(p.rig.bones)} bones")
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    tmp = CREATURE_DIR / "_tmp_part.bbmodel"
    try:
        needs = {part_wings: [part_body], part_tail: [part_body], part_ruff: [part_head, part_eyes], part_ribs: [part_body],
                 part_limb_detail: [part_wings, part_legs]}
        kit.render_part_previews(PART_FUNCS, lambda fns: build(fns + needs.get(fns[0], [])),
                                 tmp, PARTS_DIR, "CanYuXiao", PART_VIEWS)
    finally:
        tmp.unlink(missing_ok=True)
    kit.make_check(p, BONES, model, REVIEW_DIR, MODEL_ID, "can_yu_xiao（残羽鸮）round 3/3", VIEWS, MIN_BONES, total,
                   ref_crop=(0.0, 0.0, 0.5, 1.0), framing_views=FRAMING_VIEWS, tree_columns=2)


# 画笔库 handheld_texture：owl_* 是本只补的（灰褐残羽 / 拔光后的灰粉裸皮 / 羽根桩 / 浑浊乳白眼）；金眼用现成 eye_amber，靠对比提亮
TEXTURE_MATERIAL = {
    "feather": "owl_feather", "feather_dk": "owl_feather_dk", "feather_lt": "owl_feather_lt",
    "bare": "owl_bare", "bare_dk": "owl_bare_dk", "quill": "owl_quill", "root": "owl_root",
    "disc": "owl_disc", "disc_dk": "owl_feather_dk", "crack": "void",
    "eye": "eye_amber", "eye_ring": "owl_eye_ring", "pupil": "void",
    "eye_cloud": "owl_eye_cloud", "eye_cloud_dk": "owl_eye_cloud_dk",
    "beak": "crow_beak", "beak_lt": "crow_beak", "leg": "owl_scale", "leg_dk": "owl_scale_dk",
    "talon": "crow_claw", "talon_tip": "owl_talon_tip",
}
TEX_VIEWS = (("FRONT -z（脸盘 / 金眼与浊眼）", 180.0, 6.0), ("3/4 前右", 150.0, 18.0), ("BACK 背（翅背 / 尾羽）", 0.0, 6.0))


def textured_run() -> None:
    p = build(list(PART_FUNCS.values()))
    kit.render_check_tex(p, BONES, MATS, REVIEW_DIR, CREATURE_DIR, MODEL_ID, MODEL_ID, TEXTURE_MATERIAL, TEX_VIEWS, "CanYuXiao_tex.bbmodel")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--part", choices=sorted(PART_FUNCS))
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--gates", action="store_true")
    ap.add_argument("--tex", action="store_true", help="只出像素贴图版 CanYuXiao_tex.bbmodel 与 check_tex.png")
    args = ap.parse_args()
    if args.tex:
        textured_run()
        return
    if not (args.part or args.self_test or args.gates):
        full_run()
        return
    p = build([PART_FUNCS[args.part]] if args.part else list(PART_FUNCS.values()))
    if args.self_test:
        GATES.self_test(p)
    elif args.gates:
        GATES.run(p)
    else:
        out = CREATURE_DIR / f"_tmp_{args.part}.bbmodel"
        out.write_text(json.dumps(p.rig.bbmodel("CanYuXiao")), encoding="utf-8")
        print(f"{p.total()} cubes → {out}")


if __name__ == "__main__":
    main()
