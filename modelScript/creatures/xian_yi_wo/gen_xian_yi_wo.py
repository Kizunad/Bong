#!/usr/bin/env python3
"""藓衣蜗（xian_yi_wo）Blockbench .bbmodel 程序化生成器 —— Round 3/3，几何。

正典依据：docs/worldview.md §二 负灵域「噬灵藓」。任务卡：末世生物批次 #17（.task-wasteland-creatures.md）：
背着一座长满灰绿噬灵藓的螺壳的大蜗牛，身体灰白半透（靠浅色和深色内脏轮廓），触角末端发暗光，爬过留下吸灵黏痕。

外形：体长约 30px 的大蜗牛，头朝 -z。身体是四节低伏的灰白软体（浅色 + 侧面几团深紫内脏轮廓），头在前端抬起，
两根长眼柄（两节，末端一点暗紫光，只靠对比度）加两根短下触角，嘴是一小团肉色。背上一座圆盘螺壳（壳面朝 ±x，
一圈圈螺纹从壳心旋出，中心一个凸起的壳心），壳面大片灰绿噬灵藓斑，壳顶和壳缘长出几簇藓块、壳下缘垂着藓丝；
身后拖一条半透明的吸灵黏痕（三段，薄薄贴地）。

做法：体素场（creature_kit.Field），先壳、再身体（先到先得），只算右半再镜像。
骨骼（≥16）：身体四节、头、嘴、眼柄各两节、下触角、壳 + 壳心、四对藓块、黏痕三段。

用法：
    python3 modelScript/creatures/xian_yi_wo/gen_xian_yi_wo.py --self-test
    python3 modelScript/creatures/xian_yi_wo/gen_xian_yi_wo.py --part shell
    python3 modelScript/creatures/xian_yi_wo/gen_xian_yi_wo.py            # 全量：模型 + 接触表 + check.png
"""

from __future__ import annotations

import argparse
import json
import math
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageDraw

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "creatures" / "_kit"))
import creature_kit as kit  # noqa: E402
from creature_kit import Field, Puppet, blotch, hash_cell, pick  # noqa: E402

CREATURE_DIR = Path(__file__).resolve().parent
REVIEW_DIR = Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/xian_yi_wo")
PARTS_DIR = REVIEW_DIR / "parts"
MODEL_ID = "xian_yi_wo"

MATS = {
    "flesh":      (190, 192, 184),    # 灰白半透的软体
    "flesh_lt":   (220, 222, 214),
    "flesh_dk":   (156, 160, 156),
    "organ":      (84, 70, 90),       # 透出来的内脏轮廓
    "organ_red":  (122, 74, 82),
    "stalk":      (150, 134, 138),    # 眼柄 / 下触角
    "stalk_dk":   (110, 96, 104),
    "tip":        (118, 96, 176),     # 眼柄末端暗紫光（靠对比度）
    "tip_core":   (176, 152, 224),
    "mouth":      (150, 112, 106),
    "shell":      (146, 134, 114),    # 螺壳
    "shell_dk":   (104, 94, 80),
    "shell_deep": (72, 64, 54),
    "shell_lt":   (176, 164, 142),
    "moss":       (112, 128, 88),     # 噬灵藓
    "moss_lt":    (140, 156, 108),
    "moss_dk":    (74, 94, 62),
    "slime":      (176, 190, 176),    # 吸灵黏痕
    "slime_dk":   (124, 142, 130),
    "slime_hi":   (218, 230, 218),    # 黏液的亮泡 / 水光（Round 3，只靠对比度）
    "lichen":     (194, 198, 150),    # 壳侧的地衣壳斑（Round 3）
}

BONES: list[tuple[str, str | None, tuple[float, float, float], str]] = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("body_1", "root", (0.0, 3.0, 17.0), "身体第 1 节（尾）"),
    ("body_2", "body_1", (0.0, 3.4, 11.0), "身体第 2 节"),
    ("body_3", "body_2", (0.0, 3.8, 5.0), "身体第 3 节（壳坐在这节上）"),
    ("body_4", "body_3", (0.0, 4.4, -1.5), "身体第 4 节（颈，抬起）"),
    ("head", "body_4", (0.0, 8.0, -8.0), "头（抬起）"),
    ("mouth", "head", (0.0, 6.0, -12.6), "嘴（肉色小团）"),
    ("ant_up_1_r", "head", (1.8, 10.5, -10.5), "右长触角·下节"),
    ("ant_up_1_l", "head", (-1.8, 10.5, -10.5), "左长触角·下节"),
    ("ant_up_2_r", "ant_up_1_r", (3.0, 16.0, -12.6), "右长触角·上节（末端是眼）"),
    ("ant_up_2_l", "ant_up_1_l", (-3.0, 16.0, -12.6), "左长触角·上节（末端是眼）"),
    ("tent_low_r", "head", (2.0, 7.0, -12.4), "右短触角"),
    ("tent_low_l", "head", (-2.0, 7.0, -12.4), "左短触角"),
    ("shell", "body_3", (0.0, 13.0, 5.0), "螺壳（逐圈收小的螺旋，壳口朝前下并外翻）"),
    ("shell_boss_r", "shell", (3.5, 13.0, 5.0), "右壳心（最内几圈，凸起）"),
    ("shell_boss_l", "shell", (-3.5, 13.0, 5.0), "左壳心"),
    ("moss_a_r", "shell", (2.5, 20.5, 9.0), "右藓块 a（壳顶后）"),
    ("moss_a_l", "shell", (-2.5, 20.5, 9.0), "左藓块 a"),
    ("moss_b_r", "shell", (2.5, 20.0, 0.0), "右藓块 b（壳顶前）"),
    ("moss_b_l", "shell", (-2.5, 20.0, 0.0), "左藓块 b"),
    ("moss_c_r", "shell", (2.0, 22.0, 5.0), "右藓块 c（壳顶正中）"),
    ("moss_c_l", "shell", (-2.0, 22.0, 5.0), "左藓块 c"),
    ("moss_d_r", "shell", (2.5, 16.0, 11.0), "右藓丝 d（沿壳面下垂）"),
    ("moss_d_l", "shell", (-2.5, 16.0, 11.0), "左藓丝 d"),
    ("slime_1", "body_1", (0.0, 0.0, 20.0), "黏痕 1"),
    ("slime_2", "slime_1", (0.0, 0.0, 26.0), "黏痕 2"),
    ("slime_3", "slime_2", (0.0, 0.0, 32.0), "黏痕 3"),
]
MIN_BONES = 16                      # 任务卡：≥ 16（身体分 4 节、触角各 2 节、壳与藓块分骨）

SHELL_C = (0.0, 13.0, 5.0)          # 壳心（螺旋轴沿 x 穿过这里）
# 壳 = 一根逐圈收小的管子沿螺旋盘进去：第 t 圈（t 从壳口算起，单位：圈）管心离轴 C0·Q^t、管半径 RHO0·Q^t，
# 管心沿 x 抬高 XC·(1-Q^t) —— 内圈越小越凸，整体是个阶梯状的锥形螺旋；壳口（t=0）朝前下，口沿外翻。
SHELL_TURNS = 2.4
SHELL_Q = 0.55
SHELL_C0, SHELL_RHO0, SHELL_XC = 5.0, 4.2, 7.0
SHELL_MOUTH = math.atan2(-1.0, -1.0)    # 壳口方向（yz 平面里朝前下，-z 为前）
SHELL_LIP_TURN = 0.10                   # 壳口外翻的那一小段（圈）


def new_puppet() -> Puppet:
    return Puppet(MATS, BONES)


def new_field(p: Puppet) -> Field:
    return Field(p)


# ───────────────────────────────────────────────────────────── 材质函数
def _shell_tube(r: float, frac: float):
    """极坐标 (r, 绕壳心的角度 → 圈内小数 frac) 处，各圈管子里顶面最高的那根：返回 (顶面 x 高, 圈数 t, 管内偏离 |u|)。"""
    best = None
    m = 0
    while frac + m <= SHELL_TURNS:
        t = frac + m
        decay = SHELL_Q ** t
        c, rho, xc = SHELL_C0 * decay, SHELL_RHO0 * decay, SHELL_XC * (1.0 - decay)
        if t < SHELL_LIP_TURN:                      # 壳口外翻：最后一小段管子变粗、管心外移
            flare = 1.0 - t / SHELL_LIP_TURN
            rho *= 1.0 + 0.5 * flare
            c *= 1.0 + 0.1 * flare
        d = abs(r - c)
        if d < rho:
            h = xc + math.sqrt(rho * rho - d * d)
            if best is None or h > best[0]:
                best = (h, t, d / rho)
        m += 1
    return best


_SHELL_COLUMNS: dict[tuple[int, int], tuple[int, str, float, float]] = {}


def shell_columns() -> dict[tuple[int, int], tuple[int, str, float, float]]:
    """壳的高度场：{(j, k): (最高格 i, 顶面材质, 圈数 t, 朝上程度)}。壳面朝 ±x，只算 +x 半边。"""
    if _SHELL_COLUMNS:
        return _SHELL_COLUMNS
    for j in range(2, 25):
        for k in range(-7, 16):
            dy, dz = j + 0.5 - SHELL_C[1], k + 0.5 - SHELL_C[2]
            r = math.hypot(dy, dz)
            if r > SHELL_C0 + SHELL_RHO0 + 0.3:
                continue
            frac = ((math.atan2(dy, dz) - SHELL_MOUTH) / (2 * math.pi)) % 1.0
            tube = _shell_tube(r, frac)
            if tube is None or tube[0] < 0.5:
                continue
            h, t, u = tube
            up = dy / max(r, 1e-6)                                  # 1 = 正朝上
            top = int(math.floor(h - 0.5))
            if t < SHELL_LIP_TURN:
                mat = "shell_lt"                                    # 外翻的壳口沿，亮边
            elif blotch(0, j, k, 2, 31) < 38 * max(0.0, up) ** 2 and t > SHELL_LIP_TURN * 2:
                mat = "moss_dk" if blotch(0, j, k, 2, 33) < 25 else ("moss_lt" if blotch(0, j, k, 2, 35) < 35 else "moss")
            elif u > 0.55:
                mat = "shell_deep" if u > 0.8 else "shell_dk"       # 两圈之间的沟
            elif u < 0.38:
                mat = "shell_lt"                                    # 圈脊
            else:
                mat = "shell"
            _SHELL_COLUMNS[(j, k)] = (top, mat, t, up)
    return _SHELL_COLUMNS


def flesh_fn(i: int, j: int, k: int) -> str:
    """软体：灰白底；一道道横向褶皱（凹处深、紧随其后的脊带浅色高光 = 黏液光泽）；侧面中段几团深紫内脏轮廓。"""
    if i >= 2 and 3 <= j <= 6 and blotch(i, j, k, 3, 21) < 30:
        return "organ_red" if blotch(i, j, k, 2, 23) < 25 else "organ"
    if j >= 2:
        phase = k % 4
        if phase == 0:
            return "flesh_dk"
        if phase == 1:
            return "flesh_lt"
    return pick(blotch(i, j, k, 2, 25), (("flesh", 60), ("flesh_lt", 25), ("flesh_dk", 15)))


def foot_fn(i: int, j: int, k: int) -> str:
    return "flesh_lt" if blotch(i, j, k, 2, 71) < 55 else "flesh"


def stalk_fn(end):
    ex, ey, ez = end

    def fn(i, j, k):
        d = math.dist((i + 0.5, j + 0.5, k + 0.5), (ex, ey, ez))
        if d < 1.0:
            return "tip_core"
        return "tip" if d < 2.1 else ("stalk_dk" if blotch(i, j, k, 2, 27) < 35 else "stalk")

    return fn


# ───────────────────────────────────────────────────────────── 分部件
def part_shell(p: Puppet, f: Field) -> None:
    """螺壳：按高度场逐列填实（见 shell_columns），最内几圈（t > 2.5）归壳心骨；
    噬灵藓盖在朝上的壳面上，从壳顶沿壳面一缕缕往下垂（每缕比壳面多凸一格，读得出垂挂）。"""
    cols = shell_columns()
    for (j, k), (top, mat, t, _up) in cols.items():
        bone = "shell_boss_r" if t > 1.7 else "shell"
        for i in range(0, top + 1):
            f.put(i, j, k, bone, mat)
    for (j, k), (top, mat, t, up) in sorted(cols.items()):
        if up < 0.25 or t < SHELL_LIP_TURN * 2 or not mat.startswith("moss") or blotch(0, j, k, 1, 81) >= 16:
            continue
        for d in range(2 + hash_cell(j, k, 83) % 5):
            below = cols.get((j - d, k))
            if below is None:
                break
            f.put(below[0] + 1, j - d, k, "moss_d_r", "moss_dk" if d >= 3 else ("moss_lt" if d == 0 else "moss"), overwrite=True)


def part_body(p: Puppet, f: Field) -> None:
    """身体：长而软的四节（前高后低、尾巴贴地渐细）+ 抬起的颈和头；每节带一圈宽出身体的浅色足缘；
    侧面透出几团深紫内脏轮廓，褶皱脊带和背脊线涂浅色高光（黏液光泽）。"""
    above_ground = lambda i, j, k: j >= 0
    segs = (
        ("body_1", (0.0, 2.4, 16.5), (2.8, 2.4, 7.0)),
        ("body_2", (0.0, 3.4, 9.0), (4.3, 3.6, 4.2)),
        ("body_3", (0.0, 3.9, 2.5), (4.8, 4.0, 4.2)),
        ("body_4", (0.0, 5.6, -4.5), (4.3, 5.2, 4.2)),
    )
    for bone, center, radii in segs:
        f.ellipsoid(bone, center, radii, flesh_fn, clip=above_ground, surface=False)
    f.ellipsoid("head", (0.0, 8.6, -9.8), (3.4, 3.5, 3.3), flesh_fn, surface=False)
    f.ellipsoid("mouth", (0.0, 5.8, -12.8), (1.8, 1.2, 1.1), lambda i, j, k: "mouth")
    for bone, (cx, _cy, cz), (rx, _ry, rz) in segs:
        f.ellipsoid(bone, (0.0, 0.8, cz), (rx + 0.9, 1.5, rz), foot_fn, clip=above_ground, surface=False)
    tops: dict[tuple[int, int], int] = {}
    for (i, j, k), (bone, _) in f.cells.items():
        if bone.startswith("body_") or bone == "head":
            tops[(i, k)] = max(tops.get((i, k), -1), j)
    f.recolor(lambda i, j, k, bone, mat: "flesh_lt" if i <= 2 and k % 4 != 0 and tops.get((i, k)) == j and mat.startswith("flesh") else None)


def part_antennae(p: Puppet, f: Field) -> None:
    """两对触角：一对长触角（两节，粗到细，末端一粒暗紫的眼）、一对朝前下的短触角（末端一个小节）。"""
    eye = (3.8, 21.0, -14.6)
    f.capsule("ant_up_1_r", (1.8, 10.5, -10.5), (3.0, 16.0, -12.6), 1.5, stalk_fn(eye), r_end=1.1)
    f.capsule("ant_up_2_r", (3.0, 16.0, -12.6), eye, 1.1, stalk_fn(eye), r_end=0.9)
    f.ellipsoid("ant_up_2_r", eye, (1.6, 1.5, 1.5), stalk_fn(eye), surface=False)
    stalk_dk = lambda i, j, k: "stalk_dk" if blotch(i, j, k, 2, 29) < 40 else "stalk"
    f.capsule("tent_low_r", (2.0, 7.0, -12.4), (3.2, 4.8, -16.6), 1.1, stalk_dk, r_end=0.8)
    f.ellipsoid("tent_low_r", (3.2, 4.8, -16.6), (1.1, 1.0, 1.1), lambda i, j, k: "mouth", surface=False)


MOSS_CLUMPS = (  # (骨, 壳面上的朝向角°（yz 平面，0 = 朝后，90 = 朝上）, 离壳心的距离, 半径)：壳顶和壳缘凸出的藓团
    ("moss_c_r", 90, 8.0, 2.6), ("moss_a_r", 55, 7.8, 2.3), ("moss_b_r", 125, 7.6, 2.3),
    ("moss_a_r", 15, 7.6, 1.9), ("moss_b_r", 155, 7.0, 1.8),
)


def part_moss(p: Puppet, f: Field) -> None:
    """藓块：盖在壳顶和壳前后缘的几团藓（深浅三色），圆心落在壳面高度场的顶面上，一半嵌进壳里。"""
    cols = shell_columns()
    for n, (bone, deg, dist, r) in enumerate(MOSS_CLUMPS):
        y = SHELL_C[1] + dist * math.sin(math.radians(deg))
        z = SHELL_C[2] + dist * math.cos(math.radians(deg))
        top = cols.get((int(math.floor(y)), int(math.floor(z))), (2,))[0]
        fn = (lambda i, j, k, n=n: pick(blotch(i, j, k, 2, 51 + n), (("moss", 45), ("moss_lt", 30), ("moss_dk", 25))))
        f.ellipsoid(bone, (top + 0.2, y, z), (r, r * 0.9, r), fn)


def part_slime(p: Puppet, f: Field) -> None:
    """吸灵黏痕：身后三段贴地的薄膜（逐段变窄，边缘参差，带深色斑）。"""
    for bone, k0, k1, half in (("slime_1", 20, 26, 4), ("slime_2", 26, 32, 3), ("slime_3", 32, 38, 2)):
        for k in range(k0, k1):
            for i in range(0, half + 1):
                if blotch(i, 0, k, 1, 61) < 18 and i == half:
                    continue                                     # 边缘缺格
                f.put(i, 0, k, bone, "slime_dk" if blotch(i, 0, k, 2, 63) < 30 else "slime")


def part_collar(p: Puppet, f: Field) -> None:
    """外套膜领：壳口与颈之间一圈肥厚的浅色软肉（蜗牛从壳里伸出来的那一圈），上缘一排小肉瘤。"""
    f.ellipsoid("body_3", (0.0, 5.6, -0.8), (5.0, 2.4, 2.4), lambda i, j, k: "flesh_lt" if blotch(i, j, k, 2, 91) < 70 else "flesh", surface=False)
    for k in range(-3, 2):
        for i in range(0, 5, 2):
            if hash_cell(i, k, 0, 93) < 70:
                f.put(i, 8, k, "body_3", "flesh_lt")


def part_tubercles(p: Puppet, f: Field) -> None:
    """皮肤上的小肉瘤：颈和背上成片的、比皮肤深一档的瘤点，沿脊线两侧一路到尾，越往后越稀（要在身体之后放，贴着体表）。"""
    tops: dict[tuple[int, int], int] = {}
    for (i, j, k), (bone, _) in f.cells.items():
        if bone.startswith("body_") or bone == "head":
            tops[(i, k)] = max(tops.get((i, k), -1), j)
    for (i, k), j in sorted(tops.items()):
        if i > 3 or k > 12 or blotch(i, 0, k, 1, 95) >= (34 if k < 4 else 14):
            continue
        bone = "head" if k < -6 else ("body_4" if k < -1 else ("body_3" if k < 5 else "body_2"))
        f.put(i, j + 1, k, bone, "organ" if blotch(i, 0, k, 2, 97) < 25 else "flesh_dk")


def part_lichen(p: Puppet, f: Field) -> None:
    """壳侧的地衣壳斑：壳的侧面（不朝上的面）一块块淡黄绿的硬壳状斑，和朝上面的毛绒噬灵藓区分开。"""
    cols = shell_columns()

    def patch(i: int, j: int, k: int, bone: str, mat: str) -> str | None:
        if not bone.startswith("shell") or not mat.startswith("shell"):
            return None
        column = cols.get((j, k))
        if column is None or column[3] > 0.45 or i != column[0]:
            return None                                       # 只涂壳面最外一格、朝上程度低的格
        return "lichen" if blotch(i, j, k, 3, 99) < 22 else None

    f.recolor(patch)


def part_bubbles(p: Puppet, f: Field) -> None:
    """黏痕上的亮泡与蜗身下缘垂下的黏丝：泡是贴在黏膜上的单格亮点，丝是从足缘往下拉的一小条。"""
    for bone, k0, k1, half in (("slime_1", 20, 26, 4), ("slime_2", 26, 32, 3), ("slime_3", 32, 38, 2)):
        for k in range(k0, k1):
            for i in range(0, half):
                if (i, 0, k) in f.cells and hash_cell(i, k, 0, 101) < 14:
                    f.put(i, 1, k, bone, "slime_hi")
    for k in (14, 17):
        f.put(1, 0, k, "slime_1", "slime_hi")


PART_FUNCS = {"shell": part_shell, "body": part_body, "antennae": part_antennae, "moss": part_moss, "slime": part_slime,
              "collar": part_collar, "tubercles": part_tubercles, "lichen": part_lichen, "bubbles": part_bubbles}
BUILD_ORDER = ("shell", "moss", "body", "collar", "tubercles", "antennae", "slime", "bubbles", "lichen")     # 壳先占位，身体让位；斑 / 泡最后涂


def build(parts) -> Puppet:
    p = new_puppet()
    f = new_field(p)
    for fn in sorted(parts, key=lambda fn: BUILD_ORDER.index(fn.__name__.removeprefix("part_"))):
        fn(p, f)
    f.emit()
    return p


GATES = kit.Gates("藓衣蜗 / xian_yi_wo", MATS, asym=frozenset(), soft_over=frozenset(), min_bones=MIN_BONES,
                  build_small=lambda: build([part_slime]))

VIEWS = (
    ("FRONT 正视（相机在 -z，看眼柄 / 暗光 / 嘴）", 180.0, 8.0),
    ("3/4 前右斜视（相机在 -z / +x 上方）", 150.0, 22.0),
    ("LEFT 左侧视（相机在 -x，头朝画面左，壳面螺纹）", 90.0, 0.0),
    ("RIGHT 右侧视（相机在 +x，头朝画面右）", -90.0, 0.0),
    ("TOP 俯视（相机在 +y，头朝画面下方）", 180.0, 89.9),
    ("BACK 背视（相机在 +z，看壳后与黏痕）", 0.0, 8.0),
)
PART_VIEWS = (("LEFT -x", 90.0, 0.0), ("3/4 前右", 150.0, 22.0), ("TOP +y", 180.0, 89.9))
FRAMING_VIEWS = (("3/4 左前（yaw120）", 120.0, 18.0), ("LEFT 侧（头朝左）", 90.0, 4.0))


def full_run() -> None:
    p = build(list(PART_FUNCS.values()))
    GATES.self_test(p)
    total = GATES.run(p)
    if total:
        raise SystemExit(f"门禁仍有 {total} 处违例，不出图")
    model = CREATURE_DIR / "XianYiWo.bbmodel"
    kit.write_model(p, model, "XianYiWo")
    print(f"✓ {model}: {p.total()} cubes / {len(p.rig.bones)} bones")
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    tmp = CREATURE_DIR / "_tmp_part.bbmodel"
    try:
        needs = {part_moss: [part_shell], part_lichen: [part_shell], part_tubercles: [part_body], part_bubbles: [part_slime]}   # 依赖别的件才有格子可涂 / 可贴
        kit.render_part_previews(PART_FUNCS, lambda fns: build(fns + needs.get(fns[0], [])),
                                 tmp, PARTS_DIR, "XianYiWo", PART_VIEWS)
    finally:
        tmp.unlink(missing_ok=True)
    kit.make_check(p, BONES, model, REVIEW_DIR, MODEL_ID, "xian_yi_wo（藓衣蜗）round 3/3", VIEWS, MIN_BONES, total,
                   ref_crop=(0.0, 0.0, 1.0, 0.52), framing_views=FRAMING_VIEWS, tree_columns=2)
    append_prev_comparison(model)


def append_prev_comparison(model: Path) -> None:
    """check.png 底部加一行：上一版（git 里上次提交的 bbmodel）与本版在同一组视角、同高裁到包围盒的对比。"""
    prev = CREATURE_DIR / "_prev.bbmodel"
    shown = subprocess.run(["git", "show", f"HEAD:{model.relative_to(REPO).as_posix()}"], cwd=REPO, capture_output=True, check=True)
    prev.write_bytes(shown.stdout)
    try:
        tiles = [(f"{tag} · {label}", kit.fit_height(kit.render_model(path, yaw, pitch, 700), 340))
                 for tag, path in (("上一版", prev), ("返工后", model)) for label, yaw, pitch in FRAMING_VIEWS]
    finally:
        prev.unlink(missing_ok=True)
    check = Image.open(REVIEW_DIR / "check.png").convert("RGB")
    row = Image.new("RGB", (max(check.width, sum(t.width + 12 for _, t in tiles) + 10), 340 + 56), (32, 33, 36))
    draw = ImageDraw.Draw(row)
    draw.text((10, 6), "返工对比：左两格 上一版，右两格 返工后（同视角、同高）", fill=(235, 235, 235), font=kit.load_font(17))
    x = 5
    for label, tile in tiles:
        draw.text((x + 4, 32), label, fill=(215, 205, 180), font=kit.load_font(12))
        row.paste(tile, (x, 50))
        x += tile.width + 12
    out = Image.new("RGB", (row.width, check.height + row.height), (25, 26, 29))
    out.paste(check, (0, 0))
    out.paste(row, (0, check.height))
    out.save(REVIEW_DIR / "check.png")
    print(f"✓ check.png 追加返工对比 → {out.width}×{out.height}")


# ───────────────────────────────────────────────────────────── 像素贴图（画笔库 handheld_texture：slug_* / snail_shell* / lichen_crust / slime_* 是本只补的）
TEXTURE_MATERIAL = {
    "flesh": "slug_flesh", "flesh_lt": "slug_flesh_lt", "flesh_dk": "slug_flesh_dk",
    "organ": "slug_organ", "organ_red": "slug_organ_red",
    "stalk": "slug_stalk", "stalk_dk": "slug_stalk_dk", "tip": "glow_purple", "tip_core": "glow_purple_hi", "mouth": "slug_mouth",
    "shell": "snail_shell", "shell_dk": "snail_shell_dk", "shell_deep": "snail_shell_deep", "shell_lt": "snail_shell_lt",
    "moss": "moss", "moss_lt": "moss_light", "moss_dk": "moss_dark",
    "slime": "slime_film", "slime_dk": "slime_film_dk", "slime_hi": "slime_bubble", "lichen": "lichen_crust",
}
TEX_VIEWS = (("3/4 FRONT-RIGHT", 150.0, 22.0), ("LEFT 侧（壳面 / 眼柄）", 90.0, 0.0), ("FRONT -z（眼柄暗光）", 180.0, 8.0))


def textured_run() -> None:
    p = build(list(PART_FUNCS.values()))
    kit.render_check_tex(p, BONES, MATS, REVIEW_DIR, CREATURE_DIR, MODEL_ID, MODEL_ID, TEXTURE_MATERIAL, TEX_VIEWS, "XianYiWo_tex.bbmodel")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--part", choices=sorted(PART_FUNCS))
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--gates", action="store_true")
    ap.add_argument("--tex", action="store_true", help="只出像素贴图版 XianYiWo_tex.bbmodel 与 check_tex.png")
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
        out.write_text(json.dumps(p.rig.bbmodel("XianYiWo")), encoding="utf-8")
        print(f"{p.total()} cubes → {out}")


if __name__ == "__main__":
    main()
