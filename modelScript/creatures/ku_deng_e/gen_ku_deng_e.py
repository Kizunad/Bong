#!/usr/bin/env python3
"""枯灯蛾（ku_deng_e）Blockbench .bbmodel 程序化生成器 —— Round 3/3，几何。

正典依据：docs/worldview.md §二「天道陷阱（伪灵脉）」L38、§十三 L1277：天道会在荒野升起短期的高灵气点引诱修士
自相残杀。枯灯蛾是这类陷阱的**指示物**：聚在伪灵脉和灵眼周围的大蛾，看见它们成群盘旋，多半有人要死。
任务卡：末世生物批次 #8（.task-wasteland-creatures.md）。

外形：一只比人手大得多的枯蛾。翼膜半透——不做透明，靠**浅奶白底色 + 高对比的深褐翅脉 + 破洞和参差撕边**读出来；
前翅在上、后翅在下偏后，每侧前后翅分开，各三段（翅根 / 翅中 / 翅尖）。胸部是一团灰褐绒毛，头上一对大复眼、
一根卷起的虹吸口器、一对羽状栉齿触角（三节）；腹部四节（深色环带），**尾端亮着一点残光**（琥珀色，只靠亮度对比，
不做发光层）；三对细长的节肢腿（每条三段）。

坐标：1 block = 16px，+y 向上，正面朝 -z（GeckoLib 约定），+x 为蛾的右侧（骨名 *_r）。悬停姿态：身体水平，
翅向两侧平展，腿自然下垂。翼膜的破洞左右不同（刻意不对称），其余部件左右镜像。
分部件：part_*() 逐件写，--part 单件预览；骨架在 BONES 里一处定义。

用法：
    python3 modelScript/creatures/ku_deng_e/gen_ku_deng_e.py --self-test   # 门禁差分自证
    python3 modelScript/creatures/ku_deng_e/gen_ku_deng_e.py --part wings  # 单件预览
    python3 modelScript/creatures/ku_deng_e/gen_ku_deng_e.py               # 全量：模型 + 接触表 + check.png
"""

from __future__ import annotations

import argparse
import copy
import importlib.util
import json
import math
import sys
from pathlib import Path

from PIL import Image, ImageDraw

from bbmodel_maker.gates import gatekit
from bbmodel_maker.render.render_bbmodel import render
from bbmodel_maker.rig.rigkit import Rig

REPO = Path(__file__).resolve().parents[3]
CREATURE_DIR = Path(__file__).resolve().parent
# 审阅输出写进被 gitignore 的 modelScript/out/review；参考图是调度另存的素材，仓库里没有，缺了就不画参考栏
REVIEW_DIR = REPO / "modelScript" / "out" / "review" / "img" / "creatures" / "ku_deng_e"
PARTS_DIR = REVIEW_DIR / "parts"
REFERENCE_CANDIDATES = (
    REVIEW_DIR / "ku_deng_e.png",
    Path("/home/serverkizuna/Code/Bong/.agent-worktrees/model-review/img/creatures/ku_deng_e/ku_deng_e.png"),
)

sys.path.insert(0, str(REPO / "modelScript" / "tools"))
from cjk_font import load_font  # noqa: E402

# 仓库内的共面 (Z-fighting) 门禁：与已安装库同名，按路径单独加载
_spec = importlib.util.spec_from_file_location(
    "repo_coplanar", REPO / "modelScript" / "bbmodel_maker" / "gates" / "coplanar.py")
coplanar = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(coplanar)

MODEL_ID = "ku_deng_e"

# ───────────────────────────────────────────────────────────── 材质（RGB，每种占贴图一个 8×8 色块）
MATS = {
    "fur_dk":     (62, 56, 50),       # 胸毛深灰褐
    "fur":        (96, 88, 78),
    "fur_lt":     (132, 122, 108),
    "fur_pale":   (186, 176, 154),    # 领毛 / 脸毛
    "band":       (176, 160, 130),    # 腹节浅环
    "chit_dk":    (44, 36, 30),       # 触角干 / 腿节 / 关节
    "chit":       (78, 62, 48),
    "chit_lt":    (120, 98, 74),
    "eye":        (22, 18, 20),
    "eye_hi":     (96, 84, 80),       # 复眼的一点反光
    "wing":       (214, 204, 182),    # 翼膜：奶白
    "wing_lt":    (238, 232, 216),
    "wing_stain": (156, 132, 104),    # 翼上的褐斑
    "vein":       (88, 62, 44),       # 翅脉：深褐，高对比
    "wing_edge":  (122, 88, 60),      # 前缘脉：粗而深
    "glow":       (236, 168, 64),     # 腹尖残光：琥珀（只靠亮度对比）
    "glow_hi":    (255, 218, 128),
    "ember":      (168, 96, 40),
}
FUR = ("fur_dk", "fur", "fur_lt", "fur_pale", "band")
CHIT = ("chit_dk", "chit", "chit_lt", "eye", "eye_hi")
WINGM = ("wing", "wing_lt", "wing_stain", "vein", "wing_edge")
GLOW = ("glow", "glow_hi", "ember")


def _pairs(*groups):
    out = []
    for g in groups:
        out += [(a, b) for a in g for b in g if a != b]
    return out


# 允许薄重叠的材质对：毛盖毛、关节件压在毛上、翅根压在胸毛上、腹尖光压在腹毛上
_SOFT = (_pairs(FUR, CHIT, WINGM, GLOW) +
         [(a, b) for a in FUR for b in CHIT] + [(a, b) for a in WINGM for b in CHIT + FUR] +
         [(a, b) for a in GLOW for b in FUR])
SOFT_OVER = frozenset(frozenset(p) for p in _SOFT)

# ───────────────────────────────────────────────────────────── 骨骼树
# (name, parent, pivot[px, 世界坐标], 说明)。先父后子。+x = 右侧（*_r）。
THORAX_C = (0.0, 14.0, -0.5)
BONES: list[tuple[str, str | None, tuple[float, float, float], str]] = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("thorax", "root", (0.0, 14.0, -0.5), "胸：绒毛团 + 领毛 + 翅基"),
    ("head", "thorax", (0.0, 14.8, -5.4), "头：复眼 + 脸毛"),
    ("proboscis", "head", (0.0, 13.1, -11.0), "卷起的虹吸口器"),
    ("ant_1_l", "head", (-1.0, 16.8, -8.8), "左触角第 1 节（基节，栉齿短）"),
    ("ant_1_r", "head", (1.0, 16.8, -8.8), "右触角第 1 节"),
    ("ant_2_l", "ant_1_l", (-2.6, 19.8, -10.4), "左触角第 2 节（栉齿最长）"),
    ("ant_2_r", "ant_1_r", (2.6, 19.8, -10.4), "右触角第 2 节"),
    ("ant_3_l", "ant_2_l", (-4.8, 22.4, -11.2), "左触角第 3 节（梢，栉齿渐短）"),
    ("ant_3_r", "ant_2_r", (4.8, 22.4, -11.2), "右触角第 3 节"),
    ("abd_1", "thorax", (0.0, 13.2, 4.3), "腹第 1 节"),
    ("abd_2", "abd_1", (0.0, 12.8, 7.7), "腹第 2 节"),
    ("abd_3", "abd_2", (0.0, 12.3, 11.1), "腹第 3 节"),
    ("abd_4", "abd_3", (0.0, 11.8, 14.5), "腹第 4 节"),
    ("abd_tip", "abd_4", (0.0, 11.4, 17.9), "腹尖：残光"),
    ("fw_1_l", "thorax", (-4.2, 15.6, -3.0), "左前翅·翅根"),
    ("fw_1_r", "thorax", (4.2, 15.6, -3.0), "右前翅·翅根"),
    ("fw_2_l", "fw_1_l", (-9.0, 15.6, -5.6), "左前翅·翅中"),
    ("fw_2_r", "fw_1_r", (9.0, 15.6, -5.6), "右前翅·翅中"),
    ("fw_3_l", "fw_2_l", (-17.0, 15.6, -5.4), "左前翅·翅尖"),
    ("fw_3_r", "fw_2_r", (17.0, 15.6, -5.4), "右前翅·翅尖"),
    ("hw_1_l", "thorax", (-4.2, 14.9, 2.2), "左后翅·翅根"),
    ("hw_1_r", "thorax", (4.2, 14.9, 2.2), "右后翅·翅根"),
    ("hw_2_l", "hw_1_l", (-9.0, 14.9, 3.4), "左后翅·翅中"),
    ("hw_2_r", "hw_1_r", (9.0, 14.9, 3.4), "右后翅·翅中"),
    ("hw_3_l", "hw_2_l", (-17.0, 14.9, 6.5), "左后翅·翅尖"),
    ("hw_3_r", "hw_2_r", (17.0, 14.9, 6.5), "右后翅·翅尖"),
]
# 三对腿：每条三段（股节 / 胫节 / 跗节）。点 = 髋 → 膝 → 踝 → 趾尖（右侧坐标）
LEG_PATHS = {
    "f": ((1.8, 10.8, -2.8), (4.2, 8.6, -4.4), (5.2, 5.2, -5.8), (5.4, 2.6, -6.4)),
    "m": ((2.0, 10.6, 0.0), (5.0, 8.4, 0.0), (6.4, 5.0, 0.5), (6.8, 2.4, 0.7)),
    "h": ((1.9, 10.4, 3.0), (4.6, 8.4, 4.4), (5.8, 5.0, 5.6), (6.2, 2.4, 6.2)),
}
LEG_NAMES = {"f": "前足", "m": "中足", "h": "后足"}
for _k, _pts in LEG_PATHS.items():
    for _side, _sign in (("l", -1.0), ("r", 1.0)):
        for _seg in (1, 2, 3):
            _par = "thorax" if _seg == 1 else f"leg_{_k}{_seg - 1}_{_side}"
            BONES.append((f"leg_{_k}{_seg}_{_side}", _par, (_sign * _pts[_seg - 1][0], _pts[_seg - 1][1], _pts[_seg - 1][2]),
                          f"{'左' if _side == 'l' else '右'}{LEG_NAMES[_k]}·{('股节', '胫节', '跗节')[_seg - 1]}"))


class Puppet:
    """Rig 的薄封装：左右镜像成对建件。"""

    def __init__(self) -> None:
        self.rig = Rig(MATS, tex=64, swatch=8)
        for name, parent, pivot, _ in BONES:
            self.rig.bone(name, pivot, parent)

    def box(self, bone, name, f, t, mat, rot=None, org=None):
        return self.rig.cube(bone, name, f, t, mat=mat, rot=rot, org=org)

    def pair(self, bone, name, f, t, mat, sided=True):
        """f/t 按右侧（+x）坐标给，左侧自动镜像。sided=False 时两件都挂 bone（中线骨上的成对件）。"""
        self.rig.cube(f"{bone}_r" if sided else bone, f"{name}_r", f, t, mat=mat)
        self.rig.cube(f"{bone}_l" if sided else bone, f"{name}_l", (-t[0], f[1], f[2]), (-f[0], t[1], t[2]), mat=mat)

    def total(self):
        return len(self.rig.elements)


def _mx(pt):
    return (-pt[0], pt[1], pt[2])


# ───────────────────────────────────────────────────────────── 绒毛体：椭球表面的 1px 体素
def _fur_pick(ix: int, iy: int, iz: int, weights) -> str:
    """按位置哈希挑毛色；ix 取绝对值，所以左右镜像一致。"""
    h = (abs(ix) * 73856093 ^ iy * 19349663 ^ iz * 83492791) % 100
    acc = 0
    for mat, w in weights:
        acc += w
        if h < acc:
            return mat
    return weights[-1][0]


def fur_ellipsoid(p: Puppet, bone: str, tag: str, center, radii, weights, band=None, solid=False,
                 fluff=0, fluff_z=None) -> None:
    """椭球表面的 1px 体素（只留露在外面的格），每格按哈希挑毛色；沿 x 把同色连续格并成长条。
    band=(z 周期, 浅环占比, 浅环材质)：腹节上按 z 画一圈圈浅环。
    fluff = 外圈蓬毛：表面格向外再长一层，按哈希留 fluff%（只在 fluff_z 的 z 格号范围内，避开相邻部件）；翅根一带不长。"""
    cx, cy, cz = center
    rx, ry, rz = radii

    def inside(i, j, k):
        return ((i + 0.5 - cx) / rx) ** 2 + ((j + 0.5 - cy) / ry) ** 2 + ((k + 0.5 - cz) / rz) ** 2 <= 1.0

    span = (0, int(math.ceil(cx + rx)) + 1, int(math.floor(cy - ry)) - 1,
            int(math.ceil(cy + ry)) + 1, int(math.floor(cz - rz)) - 1, int(math.ceil(cz + rz)) + 1)
    cells = {}
    for i in range(span[0], span[1]):                 # 只算右半（i ≥ 0），左半按镜像生成，保证左右严格对称
        for j in range(span[2], span[3]):
            for k in range(span[4], span[5]):
                if not inside(i, j, k):
                    continue
                if not solid and all(inside(i + d[0], j + d[1], k + d[2]) for d in
                       ((1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1))):
                    continue
                mat = _fur_pick(i, j, k, weights)
                if band and ((k - int(math.floor(cz - rz))) % band[0]) < band[1]:
                    mat = band[2]
                cells[(i, j, k)] = mat
    if fluff:
        lo, hi = fluff_z if fluff_z else (span[4], span[5])
        for (i, j, k) in list(cells):
            for d in ((1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)):
                ni, nj, nk = i + d[0], j + d[1], k + d[2]
                if (ni, nj, nk) in cells or inside(ni, nj, nk) or not lo <= nk <= hi:
                    continue
                if ni >= 3 and 13 <= nj <= 16:        # 翅根 / 肩节一带不长蓬毛
                    continue
                if (abs(ni) * 31 + nj * 17 + nk * 57) % 100 < fluff:
                    cells[(ni, nj, nk)] = _fur_pick(ni, nj, nk, weights)
    n = 0
    for (i, j, k) in sorted(cells, key=lambda c: (c[2], c[1], c[0])):
        mat = cells.get((i, j, k))
        if mat is None:
            continue
        i1 = i
        while cells.get((i1 + 1, j, k)) == mat:
            i1 += 1
        for x in range(i, i1 + 1):
            del cells[(x, j, k)]
        p.rig.cube(bone, f"{tag}_{n:03d}_r", (i, j, k), (i1 + 1, j + 1, k + 1), mat=mat)
        p.rig.cube(bone, f"{tag}_{n:03d}_l", (-i1 - 1, j, k), (-i, j + 1, k + 1), mat=mat)
        n += 1


THORAX_W = (("fur_dk", 38), ("fur", 34), ("fur_lt", 18), ("fur_pale", 10))
ABD_W = (("fur_dk", 52), ("fur", 38), ("fur_lt", 10))


# ───────────────────────────────────────────────────────────── 分部件：胸 / 头 / 触角 / 口器
def part_thorax(p: Puppet) -> None:
    """胸：一团灰褐绒毛（椭球表面，逐格毛色错落）+ 前端一圈浅色领毛 + 两个翅基肩节。"""
    fur_ellipsoid(p, "thorax", "thorax_fur", THORAX_C, (4.4, 3.9, 4.8), THORAX_W, fluff=62, fluff_z=(-5, 3))
    # 领毛：胸前端的一圈浅毛，脸后一环
    p.box("thorax", "collar_top", (-0.6, 15.8, -5.2), (0.6, 17.0, -4.3), "fur_pale")
    p.box("thorax", "collar_low", (-0.6, 12.4, -5.2), (0.6, 13.6, -4.3), "fur_pale")
    for i, (x, y) in enumerate(((1.75, 15.75), (2.35, 14.15))):
        p.pair("thorax", f"collar_{i}", (x - 0.6, y - 0.6, -5.2), (x + 0.6, y + 0.6, -4.3), "fur_pale", sided=False)
    # 肩节：翅根两处小关节球（深色），连着前后翅
    p.pair("fw_1", "fw_shoulder", (3.2, 14.95, -4.25), (4.9, 16.4, -2.2), "chit_dk")
    p.pair("hw_1", "hw_shoulder", (3.2, 14.25, 1.2), (4.9, 15.55, 3.25), "chit_dk")


def part_head(p: Puppet) -> None:
    """头：绒毛脑颅 + 一对大复眼（深色底 + 一点反光）+ 脸前的浅色毛簇。"""
    fur_ellipsoid(p, "head", "head_fur", (0.0, 14.8, -8.0), (2.9, 2.8, 2.6), THORAX_W)
    p.pair("head", "eye", (2.3, 13.9, -10.2), (3.9, 16.6, -6.9), "eye", sided=False)
    p.pair("head", "eye_hi", (2.9, 15.4, -10.6), (3.7, 16.2, -10.2), "eye_hi", sided=False)
    p.box("head", "face_tuft_a", (-1.2, 13.0, -11.2), (1.2, 14.8, -10.5), "fur_pale")
    p.box("head", "face_tuft_b", (-0.8, 15.0, -11.1), (0.8, 16.4, -10.5), "fur_lt")


def part_proboscis(p: Puppet) -> None:
    """虹吸口器：一截往下再向后卷起的细管（三格，逐格变细）。"""
    p.box("proboscis", "proboscis_a", (-0.4, 11.7, -11.4), (0.4, 13.1, -10.6), "chit")
    p.box("proboscis", "proboscis_b", (-0.35, 10.9, -11.0), (0.35, 11.7, -10.3), "chit_dk")
    p.box("proboscis", "proboscis_c", (-0.3, 10.7, -10.3), (0.3, 11.4, -9.6), "chit")


ANT_PTS = ((1.15, 16.8, -8.8), (2.6, 19.8, -10.4), (4.8, 22.4, -11.2), (7.2, 24.2, -10.6))


def part_antennae(p: Puppet) -> None:
    """羽状栉齿触角：三节深色细干，每节两侧各一排平行的栉齿（z 向），第 2 节最长，往梢渐短。"""
    teeth_len = (2.0, 3.6, 2.6)
    for seg in range(3):
        a, b = ANT_PTS[seg], ANT_PTS[seg + 1]
        bone = f"ant_{seg + 1}"
        r = 0.5 - 0.08 * seg
        p.rig.shaft(f"{bone}_r", f"stem_{seg + 1}_r", a, b, r, r, mat="chit_dk")
        p.rig.shaft(f"{bone}_l", f"stem_{seg + 1}_l", _mx(a), _mx(b), r, r, mat="chit_dk")
        for k in range(4):
            t = (k + 0.5) / 4
            x, y, z = (a[i] + (b[i] - a[i]) * t for i in range(3))
            length = teeth_len[seg] * (1.0 - 0.18 * k)
            p.pair(bone, f"tooth_{seg + 1}_{k}_f", (x - 0.2, y - 0.2, z - r - length), (x + 0.2, y + 0.2, z - r + 0.1), "chit")
            p.pair(bone, f"tooth_{seg + 1}_{k}_b", (x - 0.2, y - 0.2, z + r - 0.1), (x + 0.2, y + 0.2, z + r + length), "chit")


# ───────────────────────────────────────────────────────────── 分部件：腹 / 残光
ABD_CZ = (6.0, 9.4, 12.8, 16.2)
ABD_RX = (3.6, 3.4, 3.0, 2.4)
ABD_Y = (13.2, 12.8, 12.3, 11.8)


def part_abdomen(p: Puppet) -> None:
    """腹：四节椭球绒毛，每节一道前缘浅环，逐节收细并略下垂。"""
    for i in range(4):
        fur_ellipsoid(p, f"abd_{i + 1}", f"abd_{i + 1}_fur", (0.0, ABD_Y[i], ABD_CZ[i]),
                      (ABD_RX[i], ABD_RX[i] * 0.92, 1.7), ABD_W, band=(4, 1, "band"))


def part_glow(p: Puppet) -> None:
    """腹尖残光：一粒琥珀色的小尖，里层更亮（只靠亮度对比，不做发光层）。"""
    fur_ellipsoid(p, "abd_tip", "tip_fur", (0.0, 11.5, 19.4), (1.8, 1.7, 1.5), (("glow", 70), ("ember", 30)), solid=True)
    p.box("abd_tip", "tip_core", (-0.6, 11.1, 18.95), (0.6, 11.9, 19.9), "glow_hi")


# ───────────────────────────────────────────────────────────── 分部件：腿
def part_legs(p: Puppet) -> None:
    """三对节肢腿，每条三段：股节（浅毛套）→ 胫节（深色，带两根刺）→ 跗节（细，趾尖一点爪）。"""
    for key, pts in LEG_PATHS.items():
        for seg in range(3):
            a, b = pts[seg], pts[seg + 1]
            bone = f"leg_{key}{seg + 1}"
            r = (0.8, 0.6, 0.4)[seg]
            mat = ("fur", "chit", "chit_dk")[seg]
            p.rig.shaft(f"{bone}_r", f"{bone}_shaft_r", a, b, r, r, mat=mat)
            p.rig.shaft(f"{bone}_l", f"{bone}_shaft_l", _mx(a), _mx(b), r, r, mat=mat)
            if seg == 1:                                                   # 胫节两根刺
                for t in (0.35, 0.7):
                    x, y, z = (a[i] + (b[i] - a[i]) * t for i in range(3))
                    p.pair(bone, f"{bone}_spur_{int(t * 10)}", (x + r - 0.1, y - 0.3, z - 0.3), (x + r + 0.7, y + 0.3, z + 0.3), "chit_lt")
        tip = pts[3]
        p.pair(f"leg_{key}3", f"{key}_claw", (tip[0] - 0.3, tip[1] - 0.3, tip[2] - 0.5), (tip[0] + 0.3, tip[1] + 0.5, tip[2] + 0.5), "chit_lt")


# ───────────────────────────────────────────────────────────── 分部件：翅（翅脉 + 半透翼膜）
# 翅轮廓（右侧，x 向外、z 向后），顺时针
FORE_OUTLINE = ((4.2, -3.4), (11.0, -7.2), (19.0, -6.6), (25.0, -2.2), (24.4, 2.6), (18.0, 4.8), (10.0, 4.2), (4.2, 2.8))
HIND_OUTLINE = ((4.2, 2.0), (11.0, 2.6), (18.0, 6.6), (19.6, 12.2), (14.0, 13.8), (8.0, 11.2), (4.2, 6.6))
FORE_Y, HIND_Y = 15.6, 14.9
FORE_ROOT, HIND_ROOT = (4.2, -3.0), (4.2, 2.4)
SEG_X = (9.0, 17.0)                       # 翅根 | 翅中 | 翅尖 的分界
HOLES = {                                 # 破洞（圆心 x, z, 半径）：左右不同
    ("fw", "r"): ((13.0, -1.5, 1.6), (20.5, 0.4, 1.2), (8.0, 2.4, 1.0), (24.0, -0.5, 1.7), (17.0, 4.8, 1.5), (11.0, -6.8, 1.3)),
    ("fw", "l"): ((15.5, 1.2, 1.5), (9.5, -2.5, 1.1), (21.5, -3.0, 1.0), (23.6, 1.8, 1.7), (12.0, 4.6, 1.4), (17.5, -6.4, 1.3)),
    ("hw", "r"): ((12.5, 8.0, 1.7), (16.8, 10.6, 1.0), (19.4, 9.6, 1.6), (8.2, 12.0, 1.7)),
    ("hw", "l"): ((10.0, 5.6, 1.2), (15.2, 8.8, 1.8), (18.8, 12.6, 1.6), (9.4, 3.0, 1.2)),
}
STAINS = {"fw": ((14.0, -5.4, 2.4), (21.5, -1.0, 2.0)), "hw": ((15.0, 10.0, 2.2),)}   # 褐斑（近前缘 / 翅尖）


def _inside(poly, x, z):
    hit = False
    for (x0, z0), (x1, z1) in zip(poly, poly[1:] + poly[:1]):
        if (z0 > z) != (z1 > z) and x < (x1 - x0) * (z - z0) / (z1 - z0) + x0:
            hit = not hit
    return hit


def _edge_dist(poly, x, z, edge_idx=None):
    best = 9e9
    edges = list(zip(poly, poly[1:] + poly[:1]))
    for n, ((x0, z0), (x1, z1)) in enumerate(edges):
        if edge_idx is not None and n not in edge_idx:
            continue
        dx, dz = x1 - x0, z1 - z0
        t = max(0.0, min(1.0, ((x - x0) * dx + (z - z0) * dz) / (dx * dx + dz * dz)))
        best = min(best, math.hypot(x - (x0 + t * dx), z - (z0 + t * dz)))
    return best


def _vein_lines(kind):
    """翅脉：前缘脉（沿轮廓前缘）+ 从翅根向外扇出的放射脉 + 一条横脉。返回 (点列表) 的列表。"""
    if kind == "fw":
        root, fans = FORE_ROOT, ((25.0, -2.0), (23.0, 1.0), (20.0, 3.8), (15.0, 4.2), (9.0, 3.4))
        cross = ((12.5, -5.5), (14.6, 3.6))
    else:
        root, fans = HIND_ROOT, ((18.0, 6.4), (19.0, 10.4), (15.0, 12.6), (10.0, 10.4))
        cross = ((10.5, 4.2), (13.4, 11.4))
    return [[root, f] for f in fans] + [list(cross)]


def _dist_seg(x, z, a, b):
    dx, dz = b[0] - a[0], b[1] - a[1]
    t = max(0.0, min(1.0, ((x - a[0]) * dx + (z - a[1]) * dz) / (dx * dx + dz * dz)))
    return math.hypot(x - (a[0] + t * dx), z - (a[1] + t * dz))


def wing_cells(kind: str, side: str) -> dict:
    """某一侧某一对翅的格子 {(ix, iz): 材质}，ix/iz 为 1px 格号（右侧坐标系）。
    膜 = 轮廓内 − 破洞 − 撕边随机缺格（边缘 1.2px 内约 1/4 缺）；脉 = 前缘脉 + 放射脉 + 横脉；斑 = 褐斑圆内；
    最后剔除没有四邻接的孤格（它们在悬空门里会判悬空）。"""
    poly = FORE_OUTLINE if kind == "fw" else HIND_OUTLINE
    holes = HOLES[(kind, side)]
    lead = (0, 1, 2) if kind == "fw" else (0, 1, 2)
    lines = _vein_lines(kind)
    cells = {}
    for ix in range(4, 27):
        for iz in range(-9, 16):
            cx, cz = ix + 0.5, iz + 0.5
            if not _inside(poly, cx, cz):
                continue
            if any(math.hypot(cx - hx, cz - hz) < hr for hx, hz, hr in holes):
                continue
            edge = _edge_dist(poly, cx, cz)
            if edge < 1.2 and ((ix * 73 + iz * 31 + (7 if side == "l" else 0) + (11 if kind == "hw" else 0)) % 3 == 0) and ix > 6:
                continue
            if _edge_dist(poly, cx, cz, set(lead)) < 0.9 and kind == "fw":
                mat = "wing_edge"
            elif any(_dist_seg(cx, cz, a, b) < 0.42 for line in lines for a, b in zip(line, line[1:])):
                mat = "vein"
            elif any(math.hypot(cx - sx, cz - sz) < sr for sx, sz, sr in STAINS[kind]) and (ix + iz) % 3:
                mat = "wing_stain"
            else:
                mat = "wing_lt" if (ix * 5 + iz * 3) % 7 == 0 else "wing"
            cells[(ix, iz)] = mat
    changed = True
    while changed:
        changed = False
        for ix, iz in list(cells):
            if not any((ix + dx, iz + dz) in cells for dx, dz in ((1, 0), (-1, 0), (0, 1), (0, -1))):
                del cells[(ix, iz)]
                changed = True
    return cells


def _wing(p: Puppet, kind: str) -> None:
    y0 = FORE_Y if kind == "fw" else HIND_Y
    for side, sign in (("r", 1), ("l", -1)):
        cells = wing_cells(kind, side)
        n = 0
        for (ix, iz) in sorted(cells, key=lambda c: (c[0], c[1])):
            mat = cells.get((ix, iz))
            if mat is None:
                continue
            iz1 = iz
            while cells.get((ix, iz1 + 1)) == mat:
                iz1 += 1
            for z in range(iz, iz1 + 1):
                del cells[(ix, z)]
            seg = 1 if ix + 0.5 < SEG_X[0] else (2 if ix + 0.5 < SEG_X[1] else 3)
            half = 0.3 if mat in ("vein", "wing_edge") else 0.2
            x0, x1 = (ix, ix + 1) if sign > 0 else (-ix - 1, -ix)
            p.box(f"{kind}_{seg}_{side}", f"{kind}_{ix:02d}_{n:03d}_{side}", (x0, y0 - half, iz), (x1, y0 + half, iz1 + 1), mat)
            n += 1


def part_forewings(p: Puppet) -> None:
    """前翅：奶白半透膜（高对比深褐翅脉 + 粗前缘脉 + 褐斑 + 破洞 + 参差撕边），左右破洞不同，三段骨。"""
    _wing(p, "fw")


def part_hindwings(p: Puppet) -> None:
    """后翅：在前翅下偏后，较短而圆，同样的脉 / 斑 / 破洞，三段骨。"""
    _wing(p, "hw")


PART_FUNCS = {
    "thorax": part_thorax,
    "head": part_head,
    "proboscis": part_proboscis,
    "antennae": part_antennae,
    "abdomen": part_abdomen,
    "glow": part_glow,
    "legs": part_legs,
    "forewings": part_forewings,
    "hindwings": part_hindwings,
}


def build(parts: list) -> Puppet:
    p = Puppet()
    for fn in parts:
        fn(p)
    return p


# ───────────────────────────────────────────────────────────── 门禁
MIN_BONES = 24                      # 任务卡「骨骼数下限」：飞行类（蝠 / 蛾）≥ 24
ASYM_BONES = frozenset(f"{k}_{s}_{side}" for k in ("fw", "hw") for s in (1, 2, 3) for side in ("l", "r"))


class EntityGates(gatekit.AssetGates):
    """实体模型的门禁：去掉「越出 0..16 方块空间」一道 —— 生物不是方块。"""

    def specs(self):
        return tuple(spec for spec in super().specs() if spec[0] != "overflow")


GATES = EntityGates("枯灯蛾 / ku_deng_e", MATS, asym=ASYM_BONES, soft_over=SOFT_OVER)


def gate_rig(p: Puppet) -> Rig:
    """门禁用副本：rigkit 把 color 写成「材质序号 % 8」，材质多于 8 种会撞号，副本里改回完整序号。"""
    rig = copy.deepcopy(p.rig)
    per_row = rig.tex // rig.swatch
    for el in rig.elements:
        uv = el["faces"]["north"]["uv"]
        el["color"] = int(uv[1] // rig.swatch) * per_row + int(uv[0] // rig.swatch)
    GATES.colors = {i: n for i, n in enumerate(list(MATS))}
    return rig


def coplanar_violations(p: Puppet) -> list[str]:
    flat = [e for e in p.rig.elements if not any(e["rotation"])]
    return coplanar.check_coplanar_faces(flat)


def moving_bone_count(p: Puppet) -> int:
    return sum(1 for b in p.rig.bones.values() if b["children"])


def run_gates(p: Puppet) -> int:
    total = GATES.report(gate_rig(p), px=16.0, note="（实体不受 16px 方块空间限制，已去掉越界一道）")
    cop = coplanar_violations(p)
    print(f"  {'✓' if not cop else '✗'} 共面 (Z-fighting): {len(cop)}")
    for line in cop[:8]:
        print(f"      - {line}")
    bones, moving = len(p.rig.bones), moving_bone_count(p)
    ok = moving >= MIN_BONES
    print(f"  {'✓' if ok else '✗'} 骨骼数: 共 {bones}（含 root），带几何的可动骨 {moving}（下限 {MIN_BONES}）")
    return total + len(cop) + (0 if ok else 1)


def self_test(p: Puppet) -> None:
    """差分自证：干净模型须全绿，每道门注入缺陷须被抓到。"""
    broken = GATES.self_test(gate_rig(p))
    clean = coplanar_violations(p)
    victim = next(e for e in p.rig.elements if not any(e["rotation"]))
    twin = copy.deepcopy(victim)
    twin["name"] = victim["name"] + "__twin"
    hits = coplanar.check_coplanar_faces([e for e in p.rig.elements if not any(e["rotation"])] + [twin])
    if clean or not hits:
        broken += 1
        print(f"  ✗ 共面: 干净 {len(clean)} / 注入后 {len(hits)}（须 0 / >0）")
    else:
        print(f"  ✓ 共面: 干净 0 → 注入同位复制件 {victim['name']} 后 {len(hits)}")
    small = build([part_legs])
    if moving_bone_count(small) >= MIN_BONES:
        broken += 1
        print("  ✗ 骨骼数门没有鉴别力")
    else:
        print(f"  ✓ 骨骼数: 完整 {moving_bone_count(p)} ≥ {MIN_BONES}；只含腿的件 {moving_bone_count(small)} 被拦下")
    if broken:
        raise SystemExit(f"自证未通过：{broken} 道门失效")
    print("✓ 差分自证全绿")


# ───────────────────────────────────────────────────────────── 输出：bbmodel / 分件预览 / 接触表 / check.png
BG = (119, 119, 119)
SIX_VIEWS = (
    ("FRONT 正视（相机在 -z，看脸 / 触角 / 翅前缘）", 180.0, 0.0),
    ("3/4 前右斜视（相机在 -z / +x 上方）", 150.0, 22.0),
    ("LEFT 左侧视（相机在 -x，头朝画面左，腹尖残光在右）", 90.0, 0.0),
    ("RIGHT 右侧视（相机在 +x，头朝画面右）", -90.0, 0.0),
    ("TOP 俯视（相机在 +y，头朝画面下方，看前后翅全貌）", 180.0, 89.9),
    ("BACK 背视（相机在 +z，看后翅与腹尖）", 0.0, 0.0),
)


def write_model(p: Puppet, path: Path, name: str) -> None:
    path.write_text(json.dumps(p.rig.bbmodel(name), ensure_ascii=False, indent=1), encoding="utf-8")


def render_model(path: Path, yaw: float, pitch: float, size: int) -> Image.Image:
    return render(path, yaw=yaw, pitch=pitch, size=size, bg=BG)[0]


def render_part_previews(tmp: Path) -> None:
    """逐件单独预览：每个 part_*() 只放自己的立方体，出正 / 3/4 / 俯视三视角。"""
    PARTS_DIR.mkdir(parents=True, exist_ok=True)
    font_t, font_s = load_font(15), load_font(12)
    for idx, (key, fn) in enumerate(PART_FUNCS.items(), start=1):
        part = build([fn])
        write_model(part, tmp, f"KuDengE_{key}")
        w = 300
        sheet = Image.new("RGB", (w * 3, w + 44), (32, 33, 36))
        d = ImageDraw.Draw(sheet)
        d.text((8, 5), f"part_{key}: {fn.__doc__.splitlines()[0]}  ({part.total()} cubes)", fill=(235, 235, 235), font=font_t)
        for k, (label, yaw, pitch) in enumerate((("FRONT -z", 180.0, 0.0), ("3/4 前右", 150.0, 22.0), ("TOP +y", 180.0, 89.9))):
            sheet.paste(render_model(tmp, yaw, pitch, w), (k * w, 24))
            d.text((k * w + 6, w + 26), label, fill=(200, 200, 200), font=font_s)
        sheet.save(PARTS_DIR / f"part_{idx:02d}_{key}.png")
    print(f"✓ 分件预览 {len(PART_FUNCS)} 张 → {PARTS_DIR}")


def render_six_views(model_path: Path, title: str, cell: int = 380) -> Image.Image:
    font_h, font_l = load_font(17), load_font(12)
    sheet = Image.new("RGB", (cell * 3 + 20, cell * 2 + 70), (32, 33, 36))
    d = ImageDraw.Draw(sheet)
    d.text((10, 6), title, fill=(235, 235, 235), font=font_h)
    for i, (label, yaw, pitch) in enumerate(SIX_VIEWS):
        x, y = 5 + (i % 3) * (cell + 5), 32 + (i // 3) * (cell + 28)
        sheet.paste(render_model(model_path, yaw, pitch, cell), (x, y + 22))
        d.rectangle([x, y, x + cell - 1, y + 21], fill=(22, 23, 26))
        d.text((x + 6, y + 3), label, fill=(235, 235, 235), font=font_l)
    return sheet


def bone_tree_lines(p: Puppet) -> list[str]:
    children: dict[str | None, list[str]] = {}
    for name, parent, _, _ in BONES:
        children.setdefault(parent, []).append(name)
    desc = {name: d for name, _, _, d in BONES}
    lines: list[str] = []

    def walk(name: str, prefix: str, last: bool, root: bool) -> None:
        n = len(p.rig.bones[name]["children"]) if name in p.rig.bones else 0
        branch = "" if root else ("└─ " if last else "├─ ")
        lines.append(f"{prefix}{branch}{name}  [{n}]  {desc[name]}")
        kids = children.get(name, [])
        for i, k in enumerate(kids):
            walk(k, prefix + ("" if root else ("   " if last else "│  ")), i == len(kids) - 1, False)

    walk("root", "", True, True)
    return lines


def bone_tree_panel(p: Puppet, height: int) -> Image.Image:
    lines = bone_tree_lines(p)
    font_h, font_s, font_t = load_font(17), load_font(13), load_font(12)
    step, w = 16, 720
    h = max(height, 110 + step * len(lines) + 20)
    panel = Image.new("RGB", (w, h), (32, 33, 37))
    d = ImageDraw.Draw(panel)
    d.rectangle([0, 0, w - 1, h - 1], outline=(70, 75, 85), width=2)
    bones, moving = len(p.rig.bones), moving_bone_count(p)
    d.text((14, 8), "骨骼层级树 BONE HIERARCHY（[n] = 该骨自带的立方体数）", fill=(180, 235, 200), font=font_h)
    d.text((14, 34), f"总骨骼 {bones}（含 root）｜带几何的可动骨 {moving}｜任务卡飞行类下限 ≥ {MIN_BONES} ✓", fill=(240, 210, 150), font=font_s)
    d.text((14, 54), f"立方体 {p.total()}｜材质 {len(MATS)} 种", fill=(190, 210, 235), font=font_s)
    d.line([(12, 78), (w - 12, 78)], fill=(65, 70, 80), width=1)
    y = 86
    for ln in lines:
        d.text((14, y), ln, fill=(228, 228, 234), font=font_t)
        y += step
    return panel


def load_reference() -> "Image.Image | None":
    for path in REFERENCE_CANDIDATES:
        if path.exists():
            return Image.open(path).convert("RGB")
    return None


def make_check(p: Puppet, model_path: Path, gate_total: int) -> None:
    ref = load_reference()
    if ref is None:
        print("没有参考图，跳过 check.png（接触表与骨骼树照出）")
    font_h = load_font(18)
    six = render_six_views(model_path, f"ku_deng_e（枯灯蛾）round 3/3 · {p.total()} cubes · "
                                       f"{len(p.rig.bones)} 骨（可动 {moving_bone_count(p)}）· 门禁违例 {gate_total}")
    tree = bone_tree_panel(p, six.height)
    six.save(REVIEW_DIR / "contact_sheet.png")
    tree.save(REVIEW_DIR / "bones_tree.png")
    if ref is None:
        return
    lw = 640
    ref_s = ref.resize((lw, round(ref.height * lw / ref.width)), Image.Resampling.LANCZOS)
    canvas = Image.new("RGB", (lw + six.width + tree.width + 50, max(six.height, tree.height) + 50), (25, 26, 29))
    d = ImageDraw.Draw(canvas)
    d.text((10, 8), "参考图 + 爆炸图（gen_exploded_ref.py）", fill=(215, 205, 180), font=font_h)
    canvas.paste(ref_s, (10, 38))
    x = lw + 25
    d.text((x, 8), "本模型（几何 only：无贴图 / 动画 / 接线 / manifest）", fill=(140, 230, 180), font=font_h)
    canvas.paste(six, (x, 38))
    x += six.width + 15
    d.text((x, 8), f"骨骼树（{len(p.rig.bones)} 骨）", fill=(180, 220, 240), font=font_h)
    canvas.paste(tree, (x, 38))
    canvas.save(REVIEW_DIR / "check.png")
    print(f"✓ check.png → {REVIEW_DIR / 'check.png'} ({canvas.width}×{canvas.height})")


def full_run() -> None:
    p = build(list(PART_FUNCS.values()))
    self_test(p)
    total = run_gates(p)
    if total:
        raise SystemExit(f"门禁仍有 {total} 处违例，不出图")
    model = CREATURE_DIR / "KuDengE.bbmodel"
    write_model(p, model, "KuDengE")
    print(f"✓ {model}: {p.total()} cubes / {len(p.rig.bones)} bones")
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    tmp = CREATURE_DIR / "_tmp_part.bbmodel"
    try:
        render_part_previews(tmp)
    finally:
        tmp.unlink(missing_ok=True)
    make_check(p, model, total)


# ───────────────────────────────────────────────────────────── 像素贴图（画笔库 handheld_texture，材质见 moth_* 条目）
TEXTURE_MATERIAL = {name: f"moth_{name}" for name in MATS}
TEXTURE_MATERIAL["wing_stain"] = "moth_eyespot"          # 翅上褐斑画成眼斑


def write_textured(p: Puppet, out_path: Path, flat: bool = False) -> Path:
    """每面独立 UV 的像素贴图版（flat=True 出平色对照）。write_textured_bbmodel 只按组平铺 outliner，
    这里按 BONES 改回真骨骼层级（带枢轴、uuid），动画才绑得上。"""
    sys.path.insert(0, str(REPO / "modelScript" / "generators"))
    sys.path.insert(0, str(REPO / "modelScript" / "generators" / "meridian_factory"))
    from handheld_texture import flat_override, handheld_override  # noqa: E402
    from pixel_texture import write_textured_bbmodel  # noqa: E402

    rig = p.rig
    names = list(MATS)
    per_row = rig.tex // rig.swatch
    owner = {uid: bone for bone, b in rig.bones.items() for uid in b["children"]}
    cubes = []
    for el in rig.elements:
        uv = el["faces"]["north"]["uv"]
        material = names[int(uv[1] // rig.swatch) * per_row + int(uv[0] // rig.swatch)]
        cube = {"name": el["name"], "from": el["from"], "to": el["to"], "material": TEXTURE_MATERIAL[material],
                "group": owner[el["uuid"]]}
        if any(el["rotation"]):
            cube["rotation"], cube["origin"] = el["rotation"], el["origin"]
        cubes.append(cube)
    write_textured_bbmodel(cubes, out_path, MODEL_ID, None, flat_override if flat else handheld_override)
    doc = json.loads(out_path.read_text(encoding="utf-8"))
    by_group = {g["name"]: g["children"] for g in doc["outliner"]}
    import uuid as uuidlib
    nodes = {name: {"name": name, "uuid": str(uuidlib.uuid4()), "origin": list(pivot), "children": list(by_group.get(name, []))}
             for name, _, pivot, _ in BONES}
    outliner = []
    for name, parent, _, _ in BONES:
        (outliner if parent is None else nodes[parent]["children"]).append(nodes[name])
    doc["outliner"] = outliner
    for element in doc["elements"]:
        element["type"] = "cube"            # 离线 codec 只认显式声明的 cube
    doc["textures"][0]["folder"] = "entity"
    out_path.write_text(json.dumps(doc, ensure_ascii=False), encoding="utf-8")
    return out_path


def render_check_tex(p: Puppet) -> None:
    """check_tex.png：参考 | 平色 | 贴图，三个视角各一对（TOP 看翅膜 / 3/4 / FRONT）。"""
    flat = write_textured(p, CREATURE_DIR / "_tmp_flat.bbmodel", flat=True)
    tex = write_textured(p, CREATURE_DIR / "KuDengE_tex.bbmodel")
    size = 440
    views = (("3/4 FRONT-RIGHT", 150.0, 22.0), ("TOP +y（翅膜）", 180.0, 89.9), ("BACK +z（腹尖残光）", 0.0, 8.0))
    ref = load_reference()
    ref_width = 0
    if ref is not None:
        ref = ref.resize((int(ref.width * size / ref.height), size), Image.Resampling.LANCZOS)
        ref_width = ref.width
    canvas = Image.new("RGB", (ref_width + len(views) * 2 * (size + 8) + 8, size + 30), (28, 29, 33))
    draw = ImageDraw.Draw(canvas)
    font = load_font(13)
    if ref is not None:
        canvas.paste(ref, (0, 30))
        draw.text((4, 8), "REFERENCE", fill=(215, 205, 180), font=font)
    x = ref_width + 8
    for label, yaw, pitch in views:
        for title, path in (("FLAT", flat), ("TEXTURED", tex)):
            canvas.paste(render(path, yaw=yaw, pitch=pitch, size=size, bg=BG, shading="mc")[0], (x, 30))
            draw.text((x + 4, 8), f"{title}: {label}", fill=(230, 230, 230), font=font)
            x += size + 8
    flat.unlink()
    REVIEW_DIR.mkdir(parents=True, exist_ok=True)
    canvas.save(REVIEW_DIR / "check_tex.png")
    print(f"✓ {tex.name}；check_tex.png → {REVIEW_DIR}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--part", choices=sorted(PART_FUNCS))
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--gates", action="store_true")
    ap.add_argument("--tex", action="store_true", help="只出像素贴图版 KuDengE_tex.bbmodel 与 check_tex.png")
    args = ap.parse_args()
    if args.tex:
        render_check_tex(build(list(PART_FUNCS.values())))
        return
    if not (args.part or args.self_test or args.gates):
        full_run()
        return
    p = build([PART_FUNCS[args.part]] if args.part else list(PART_FUNCS.values()))
    if args.self_test:
        self_test(p)
        return
    if args.gates:
        run_gates(p)
        return
    out = CREATURE_DIR / f"_tmp_{args.part}.bbmodel"
    out.write_text(json.dumps(p.rig.bbmodel("KuDengE")), encoding="utf-8")
    print(f"{p.total()} cubes → {out}")


if __name__ == "__main__":
    main()
