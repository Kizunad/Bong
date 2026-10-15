#!/usr/bin/env python3
"""粗麻斗篷（cu_ma_dou_peng）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #9（.task-wasteland-gear.md）。穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，头 y∈[24,32]，+x = 穿戴者右侧。

外形：破边粗麻斗篷，带尖顶兜帽。穿戴件不能穿进身体，所以整件是「套在外面的壳」：
  · 兜帽：罩在头外的一圈（后壁 z≥4.05、两侧壁 |x|≥4.05、顶壁 y≥32.05，壳厚 1.35px），正面（-z）敞开露脸，
    后上方堆成三级台阶的尖顶，帽沿一圈厚边；两侧壁下沿落在肩披上；
  · 肩披：盖在两臂上沿（y 24.05–24.75、|x|≥4.05），两侧沿着手臂外侧垂下（|x|≥8.05），
    前后各一幅（背幅 z∈[2.05,…]，前襟 z∈[-…,-2.05]，前襟中间留 3.2px 敞口）；
  · 披身分三档逐级外鼓（越往下离身体越远），像布料自然撑开；下摆全是参差的麻条，长短不一（不对称，单独一骨）；
  · 领口：草绳绕颈，正中一枚骨扣，几缕绳穗垂下来；右肩一块锈铁补片压在披上；背幅 / 前襟缝着几块深色补丁，补丁边是粗针脚。

分部件：part_*() 逐件写，--part 单件预览；骨骼树在 BONES 里一处定义，按挂点分骨。
用法：
    python3 modelScript/gear/cu_ma_dou_peng/gen_cu_ma_dou_peng.py --self-test   # 门禁差分自证
    python3 modelScript/gear/cu_ma_dou_peng/gen_cu_ma_dou_peng.py --part hood   # 单件预览
    python3 modelScript/gear/cu_ma_dou_peng/gen_cu_ma_dou_peng.py --worn        # 只出穿戴三面
    python3 modelScript/gear/cu_ma_dou_peng/gen_cu_ma_dou_peng.py               # 全量：模型 + check.png
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质（RGB，每种占贴图一个 8×8 色块）
MATS = {
    "linen":      (146, 128, 100),   # 粗麻主色
    "linen_dk":   (112, 96, 74),
    "linen_lt":   (172, 154, 122),
    "linen_grey": (120, 114, 104),
    "linen_brn":  (100, 80, 62),
    "patch":      (98, 68, 54),      # 深色皮补丁
    "patch_dk":   (72, 50, 42),
    "cord":       (154, 130, 86),
    "cord_dk":    (108, 88, 56),
    "bone_lt":    (228, 218, 190),
    "bone_dk":    (164, 150, 120),
    "iron":       (88, 80, 76),
    "rust":       (150, 84, 48),
    "rust_dk":    (104, 56, 34),
}
SOFT = ("linen", "linen_dk", "linen_lt", "linen_grey", "linen_brn", "patch", "patch_dk", "cord", "cord_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("hood", "root", (0.0, 28.0, 0.5), "兜帽壳：后壁 / 两侧壁 / 顶 / 帽沿厚边（脸朝 -z 敞开）"),
    ("hood_peak", "hood", (0.0, 33.4, 3.4), "兜帽后上方的三级尖顶"),
    ("collar", "root", (0.0, 23.5, 3.0), "颈后垫圈：兜帽下沿到背幅上沿的过渡"),
    ("mantle_r", "root", (6.4, 24.4, 0.0), "右肩披：盖在右臂上沿"),
    ("mantle_l", "root", (-6.4, 24.4, 0.0), "左肩披"),
    ("drape_r", "root", (8.4, 20.0, 0.0), "右侧幅：沿右臂外侧垂下，越往下越外鼓"),
    ("drape_l", "root", (-8.4, 20.0, 0.0), "左侧幅"),
    ("cape_back", "root", (0.0, 22.0, 2.4), "背幅：披在背后，上沿贴颈，下垂到小腿，分三档外鼓"),
    ("cape_front_r", "root", (4.8, 22.0, -2.4), "右前襟（中间留敞口），分三档外鼓"),
    ("cape_front_l", "root", (-4.8, 22.0, -2.4), "左前襟"),
    ("hem", "cape_back", (0.0, 10.0, 2.4), "下摆参差麻条：背幅 / 前襟 / 侧幅（长短不一，不对称）"),
    ("patches", "cape_back", (0.0, 18.0, 3.0), "缝在背幅 / 前襟上的皮补丁"),
    ("stitches", "patches", (0.0, 18.0, 3.1), "补丁边的粗针脚"),
    ("neck_tie", "root", (0.0, 22.8, -2.3), "领口草绳 + 骨扣 + 绳穗"),
    ("shoulder_plate", "mantle_r", (6.3, 24.9, 0.0), "右肩锈铁补片 + 铆钉 + 绑绳"),
]
# 刻意不对称：下摆麻条、补丁、针脚、绳穗、肩上铁片
ASYM_BONES = frozenset({"hem", "patches", "stitches", "neck_tie", "shoulder_plate"})

# ───────────────────────────────────────────────────────────── 常量
MX = 8.75                         # 披肩最外缘 x（手臂外侧在 |x|=8）
ARM_OUT = 8.05                    # 侧幅内缘 x
Y_HEAD = 24.0                     # 头底 / 躯干顶
Z_BK0, Z_FR1 = 2.05, -2.05        # 背幅内面 / 前襟内面（贴着躯干后 / 前面）
# 背幅 / 前襟的三档：(y 下沿, y 上沿, 外面 z)；越往下越外鼓
BACK_TIERS = ((19.0, 24.0, 2.75), (14.5, 19.0, 3.35), (10.5, 14.5, 4.0))
FRONT_TIERS = ((19.5, 24.0, -2.75), (17.2, 19.5, -3.3), (15.6, 17.2, -3.8))
DRAPE_TIERS = ((19.5, 23.4, 8.75), (17.2, 19.5, 9.3))   # 侧幅两档：(y 下沿, y 上沿, 外面 x)

HOOD_X, HOOD_Z0, HOOD_Z1 = 5.4, -3.9, 5.4   # 兜帽壳外缘 x / 前缘 / 后缘
HOOD_TOP = 33.4


# ───────────────────────────────────────────────────────────── 分部件：兜帽
def part_hood(g: Gear) -> None:
    """兜帽壳：后壁 + 两侧壁 + 顶 + 帽沿一圈厚边；前面（-z）敞开，头的 |x|<4、y<32 区域一件不占。"""
    b = "hood"
    g.box(b, "wall_back", (-HOOD_X, Y_HEAD, 4.05), (HOOD_X, HOOD_TOP, HOOD_Z1), "linen")
    g.pair(b, "wall_back_fold", (1.8, 25.2, HOOD_Z1), (3.0, 31.6, HOOD_Z1 + 0.3), "linen_dk")
    g.box(b, "wall_back_fold_c", (-0.6, 24.6, HOOD_Z1), (0.6, 30.4, HOOD_Z1 + 0.3), "linen_lt")
    g.pair(b, "wall_side", (4.05, 24.75, HOOD_Z0 + 0.6), (HOOD_X, HOOD_TOP, 4.05), "linen")
    g.pair(b, "wall_side_fold", (HOOD_X, 25.8, 0.0), (HOOD_X + 0.3, 31.6, 1.4), "linen_dk")
    g.pair(b, "wall_side_fold2", (HOOD_X, 26.6, -2.4), (HOOD_X + 0.3, 30.2, -1.4), "linen_lt")
    g.pair(b, "wall_side_fold3", (HOOD_X, 25.4, 2.4), (HOOD_X + 0.3, 29.0, 3.2), "linen_grey")
    g.box(b, "top", (-4.05, 32.05, HOOD_Z0 + 0.6), (4.05, HOOD_TOP, 4.05), "linen_lt")
    # 帽沿厚边：前缘一圈（上沿一条、两侧各一条），比壁厚 0.6px 向前探出
    g.box(b, "rim_top", (-HOOD_X, 32.05, HOOD_Z0), (HOOD_X, HOOD_TOP, HOOD_Z0 + 0.6), "linen_dk")
    g.pair(b, "rim_side", (4.05, 25.4, HOOD_Z0), (HOOD_X, 32.05, HOOD_Z0 + 0.6), "linen_dk")
    # 帽沿磨损：边缘几处参差的麻丝（向前探），上沿四缕、两侧各两缕（长短不一）
    for k, (x, w) in enumerate(((0.6, 0.9), (2.2, 1.3), (3.7, 0.8), (4.6, 0.7))):
        g.pair(b, f"rim_top_fray_{k}", (x, 32.05, HOOD_Z0 - 0.4 - 0.1 * k), (x + w, HOOD_TOP - 0.3, HOOD_Z0), "linen" if k % 2 else "linen_lt")
    for k, (y, h) in enumerate(((26.4, 1.0), (28.4, 1.6))):
        g.pair(b, f"rim_side_fray_{k}", (4.3 + 0.5 * k, y, HOOD_Z0 - 0.4), (4.9 + 0.5 * k, y + h, HOOD_Z0), "linen_dk")


def part_peak(g: Gear) -> None:
    """兜帽尖顶：六级薄台阶一边收窄一边向后倾（斜率一致，读作一条顺滑的弧），顶端再垂下三节软尖。"""
    b = "hood_peak"
    # (y 下, y 上, 半宽, 前缘 z, 后缘 z)：每级半宽收 0.7–0.8、前后缘各后移 1.0 左右，轮廓成一条向后上方的斜线
    steps = ((HOOD_TOP, HOOD_TOP + 0.8, 4.6, 0.0, 5.5), (HOOD_TOP + 0.8, HOOD_TOP + 1.6, 3.9, 1.0, 5.7),
             (HOOD_TOP + 1.6, HOOD_TOP + 2.4, 3.1, 2.0, 6.0), (HOOD_TOP + 2.4, HOOD_TOP + 3.1, 2.3, 3.0, 6.4),
             (HOOD_TOP + 3.1, HOOD_TOP + 3.7, 1.6, 4.0, 6.9), (HOOD_TOP + 3.7, HOOD_TOP + 4.2, 1.0, 5.0, 7.5))
    for k, (y0, y1, hx, z0, z1) in enumerate(steps):
        g.box(b, f"peak_{k}", (-hx, y0, z0), (hx, y1, z1), "linen" if k % 2 == 0 else "linen_lt")
        if k < len(steps) - 1:                                                              # 最后一级后面接软尖，不再加缝
            g.box(b, f"peak_seam_{k}", (-0.25, y0, z1), (0.25, y1, z1 + 0.25), "linen_brn")     # 后缘的一道缝，随台阶错位成一条斜线
    # 顶端下垂的软尖：三节逐渐变细，向后下方耷拉
    g.box(b, "peak_droop_0", (-0.7, HOOD_TOP + 2.6, 7.5), (0.7, HOOD_TOP + 4.2, 8.3), "linen_dk")
    g.box(b, "peak_droop_1", (-0.5, HOOD_TOP + 1.4, 7.9), (0.5, HOOD_TOP + 2.6, 8.6), "linen")
    g.box(b, "peak_droop_2", (-0.3, HOOD_TOP + 0.5, 8.3), (0.3, HOOD_TOP + 1.4, 8.9), "linen_brn")


# ───────────────────────────────────────────────────────────── 分部件：颈后 / 肩披 / 侧幅
def part_collar(g: Gear) -> None:
    """颈后垫圈：背面一块宽垫（兜帽下沿到背幅上沿的过渡，在背幅后面），两侧小耳（兜帽侧壁下沿到背幅）。"""
    b = "collar"
    g.box(b, "collar_back", (-4.05, 22.8, BACK_TIERS[0][2]), (4.05, Y_HEAD, HOOD_Z1), "linen_dk")
    g.pair(b, "collar_ear", (4.05, 22.8, BACK_TIERS[0][2]), (HOOD_X, Y_HEAD, HOOD_Z1), "linen_dk")


def part_mantle(g: Gear) -> None:
    """肩披：盖在手臂上沿（|x|≥4.05、y 24.05–24.75、z 前后各探出 0.7px），外沿折下一小截，前后沿各一条翻边。"""
    b = "mantle"
    z0, z1 = -2.75, BACK_TIERS[0][2]
    g.pair(b, "mantle_top", (4.05, 24.05, z0), (MX, 24.75, z1), "linen", sided=True)
    g.pair(b, "mantle_edge", (ARM_OUT, 23.4, z0 + 0.05), (MX, 24.05, z1 - 0.05), "linen_dk", sided=True)
    g.pair(b, "mantle_hem_f", (5.6, 24.35, z0 - 0.25), (7.6, 24.75, z0), "linen_lt", sided=True)
    g.pair(b, "mantle_hem_b", (5.6, 24.35, z1), (7.6, 24.75, z1 + 0.25), "linen_lt", sided=True)


def part_drape(g: Gear) -> None:
    """侧幅：沿手臂外侧垂下，分两档（上档贴手臂、下档向外鼓），竖向折痕凸起，下沿参差由 hem 骨负责。"""
    b = "drape"
    for k, (y0, y1, xo) in enumerate(DRAPE_TIERS):
        g.pair(b, f"drape_tier_{k}", (ARM_OUT, y0, -2.65 - 0.3 * k), (xo, y1, 2.65 + 0.3 * k + 0.1), "linen" if k == 0 else "linen_grey", sided=True)
        g.pair(b, f"drape_fold_{k}", (xo, y0 + 0.4, -1.2), (xo + 0.3, y1 - 0.4, 0.0), "linen_dk", sided=True)
        g.pair(b, f"drape_fold_b_{k}", (xo, y0 + 0.2, 1.2), (xo + 0.3, y1 - 0.6, 2.0), "linen_lt", sided=True)


def _hits_patch(x: float, y0: float, y1: float, z: float) -> bool:
    """x 处（左右两侧都算）宽 0.5px 的竖折痕，在 y0..y1、外面 z 上会不会压到补丁。"""
    for _, px0, px1, py0, py1, pz, _mat in PATCHES:
        if pz != z or py1 + 0.3 <= y0 or py0 - 0.3 >= y1:
            continue
        for sx in (x, -x):
            if px0 - 0.3 < sx + 0.25 and sx - 0.25 < px1 + 0.3:
                return True
    return False


def part_back(g: Gear) -> None:
    """背幅：从颈后垂到下摆线（y10.5），分三档外鼓；每档逐列深浅交替，外面有凸起的竖折痕。"""
    b = "cape_back"
    edges = (0.0, 1.5, 3.0, 4.5, 6.0, 7.2, ARM_OUT)
    for t, (y0, y1, zo) in enumerate(BACK_TIERS):
        for i in range(6):
            mat = ("linen", "linen_dk", "linen_grey")[(i + t) % 3]
            top = y1 + (0.05 if (i >= 3 and t == 0) else 0.0)    # 肩披以外的几列上沿接到肩披下面
            g.pair(b, f"back_t{t}_c{i}", (edges[i], y0, Z_BK0), (edges[i + 1], top, zo), mat)
        for i, x in enumerate((1.5, 4.5, 7.2)):                   # 竖向折痕：比幅面凸起 0.25px，让开补丁和颈后垫圈
            top = y1 - (1.3 if t == 0 else 0.5)
            if _hits_patch(x, y0 + 0.3, top, zo):
                continue
            g.pair(b, f"back_fold_t{t}_{i}", (x - 0.25, y0 + 0.3, zo), (x + 0.25, top, zo + 0.25), "linen_dk" if (i + t) % 2 else "linen_lt")


def part_front(g: Gear) -> None:
    """前襟：两片，从肩披垂到大腿（y15.6），分三档外鼓；中间敞开 3.2px 露出胸口，敞口内沿有一条厚边。"""
    for sx, tag in ((1, "r"), (-1, "l")):
        b = f"cape_front_{tag}"
        x0, x1 = (1.6, ARM_OUT) if sx > 0 else (-ARM_OUT, -1.6)
        for t, (y0, y1, zo) in enumerate(FRONT_TIERS):
            g.rig.cube(b, f"front_t{t}_{tag}", (x0, y0, zo), (x1, y1, Z_FR1), mat=("linen", "linen_grey", "linen_dk")[t])
            xf = x0 + 2.2 if sx > 0 else x1 - 2.2
            g.rig.cube(b, f"front_fold_t{t}_{tag}", (xf - 0.25, y0 + 0.3, zo - 0.25), (xf + 0.25, y1 - 0.4, zo), mat="linen_lt")
        e0, e1 = (x0 - 0.05, x0 + 0.5) if sx > 0 else (x1 - 0.5, x1 + 0.05)
        g.rig.cube(b, f"front_edge_{tag}", (e0, 17.5, FRONT_TIERS[0][2] - 0.25), (e1, 24.0, FRONT_TIERS[0][2]), mat="linen_dk")


# ───────────────────────────────────────────────────────────── 分部件：下摆麻条
def part_hem(g: Gear) -> None:
    """下摆参差麻条：背幅（y10.5 往下）、前襟（y15.6 往下）、侧幅（y17.2 往下）。每条长短、深浅都不同，不对称。"""
    b = "hem"
    zb0, zb1 = Z_BK0 + 0.1, BACK_TIERS[2][2] - 0.1
    back = ((-8.2, -7.0, 2.8, "linen_dk"), (-6.8, -5.4, 4.2, "linen"), (-5.2, -4.2, 2.0, "linen_grey"), (-4.0, -2.6, 3.6, "linen_dk"),
            (-2.4, -1.2, 5.0, "linen"), (-1.0, 0.4, 2.4, "linen_lt"), (0.6, 1.8, 3.8, "linen_dk"), (2.0, 3.4, 5.4, "linen"),
            (3.6, 4.6, 2.2, "linen_grey"), (4.8, 6.2, 4.4, "linen_dk"), (6.4, 7.4, 2.6, "linen"), (7.6, 8.6, 3.4, "linen_lt"))
    for i, (x0, x1, h, mat) in enumerate(back):
        g.box(b, f"hem_back_{i}", (x0, 10.5 - h, zb0), (x1, 10.5, zb1), mat)
        g.box(b, f"hem_back_tip_{i}", (x0 + 0.25, 10.5 - h - 0.9, zb0 + 0.3), (x1 - 0.35, 10.5 - h, zb1 - 0.3), mat)
        g.box(b, f"hem_back_thread_{i}", (x0 + 0.4, 10.5 - h - 1.7, zb0 + 0.6), (x0 + 0.7, 10.5 - h - 0.9, zb1 - 0.6), "linen_dk")
    zf0, zf1 = FRONT_TIERS[2][2] + 0.1, Z_FR1 - 0.1
    front_r = ((1.7, 2.9, 2.6, "linen_dk"), (3.1, 4.4, 1.4, "linen"), (4.6, 5.8, 3.2, "linen_grey"), (6.0, 7.2, 1.8, "linen_dk"), (7.4, 8.05, 2.8, "linen"))
    front_l = ((-8.0, -6.8, 3.0, "linen"), (-6.6, -5.2, 1.6, "linen_dk"), (-5.0, -3.8, 3.6, "linen_grey"), (-3.6, -2.4, 2.2, "linen"), (-2.3, -1.7, 1.2, "linen_dk"))
    for tag, run in (("r", front_r), ("l", front_l)):
        for i, (x0, x1, h, mat) in enumerate(run):
            g.box(b, f"hem_front_{tag}{i}", (x0, 15.6 - h, zf0), (x1, 15.6, zf1), mat)
            g.box(b, f"hem_front_tip_{tag}{i}", (x0 + 0.2, 15.6 - h - 0.8, zf0 + 0.3), (x1 - 0.2, 15.6 - h, zf1 - 0.3), mat)
    side_r = ((-2.4, -1.0, 2.2, "linen_grey"), (-0.8, 0.6, 3.4, "linen"), (0.8, 2.0, 1.8, "linen_dk"))
    side_l = ((-2.2, -0.9, 3.0, "linen"), (-0.7, 0.5, 1.6, "linen_dk"), (0.7, 2.1, 2.6, "linen_grey"))
    for tag, sx, run in (("r", 1, side_r), ("l", -1, side_l)):
        xo = DRAPE_TIERS[1][2]
        xa, xb = (ARM_OUT + 0.15, xo - 0.1) if sx > 0 else (-xo + 0.1, -ARM_OUT - 0.15)
        for i, (z0, z1, h, mat) in enumerate(run):
            g.rig.cube(b, f"hem_side_{tag}{i}", (xa, 17.2 - h, z0), (xb, 17.2, z1), mat=mat)
            g.rig.cube(b, f"hem_side_tip_{tag}{i}", (xa + 0.1, 17.2 - h - 0.7, z0 + 0.15), (xb - 0.1, 17.2 - h, z1 - 0.15), mat=mat)


# ───────────────────────────────────────────────────────────── 分部件：补丁 / 针脚
# (名, x0, x1, y0, y1, 外面 z, 补丁色)：背幅三块（分别落在三档上），前襟两块
PATCHES = (
    ("a", 1.8, 5.4, 14.9, 18.6, BACK_TIERS[1][2], "patch"),
    ("b", -6.0, -2.6, 11.0, 14.2, BACK_TIERS[2][2], "linen_brn"),
    ("c", -1.4, 1.0, 19.6, 21.8, BACK_TIERS[0][2], "linen_grey"),
)


def part_patches(g: Gear) -> None:
    """皮补丁：背幅三块（分落三档）、前襟两块，凸起 0.25px；补丁下沿有一道更暗的缝边。"""
    b = "patches"
    for name, x0, x1, y0, y1, z, mat in PATCHES:
        g.box(b, f"patch_back_{name}", (x0, y0, z), (x1, y1, z + 0.25), mat)
        g.box(b, f"patch_back_{name}_edge", (x0, y0, z + 0.25), (x1, y0 + 0.4, z + 0.5), "patch_dk")
    zf = FRONT_TIERS[0][2]
    g.box(b, "patch_front_r", (4.7, 20.2, zf - 0.25), (7.2, 22.8, zf), "patch")
    g.box(b, "patch_front_l", (-7.2, 19.8, zf - 0.25), (-4.8, 21.8, zf), "linen_brn")


def part_stitches(g: Gear) -> None:
    """补丁边的粗针脚：每块补丁四边各缝一排短线（骨白 / 草绳色交替），沿边间隔 0.9px。"""
    b = "stitches"
    for name, x0, x1, y0, y1, zz, _ in PATCHES:
        z = zz + 0.25
        # 横边的针脚在 x0+0.6..x1-0.6，竖边的在 y0+1.1..y1-0.6，四个角上互不相碰
        for side, (fx, fy) in (("top", (None, y1)), ("bot", (None, y0 + 0.5)), ("lt", (x0, None)), ("rt", (x1, None))):
            if fx is None:
                a, c = x0 + 0.6, x1 - 0.6
                n = max(2, int((c - a) / 0.9))
                for i in range(n):
                    x = a + (c - a) * (i + 0.5) / n
                    g.box(b, f"st_{name}_{side}_{i}", (x - 0.2, fy - 0.3, z + 0.05), (x + 0.2, fy + 0.3, z + 0.3), "cord" if i % 2 else "bone_lt")
            else:
                a, c = y0 + 1.1, y1 - 0.6
                n = max(1, int((c - a) / 0.9))
                for i in range(n):
                    y = a + (c - a) * (i + 0.5) / n
                    g.box(b, f"st_{name}_{side}_{i}", (fx - 0.3, y - 0.2, z + 0.05), (fx + 0.3, y + 0.2, z + 0.3), "cord" if i % 2 else "bone_lt")


# ───────────────────────────────────────────────────────────── 分部件：领口绳 / 肩上铁片
def part_neck_tie(g: Gear) -> None:
    """领口草绳：胸前一条横绳 + 正中骨扣（两端带骨节）+ 三缕垂下的绳穗，右边多垂一条打结的长绳。"""
    b = "neck_tie"
    zf = Z_FR1
    g.box(b, "cord_bar", (-3.9, 22.6, zf - 0.55), (3.9, 23.2, zf - 0.05), "cord")
    g.box(b, "cord_bar_dk", (-3.9, 23.2, zf - 0.4), (3.9, 23.45, zf - 0.05), "cord_dk")
    g.box(b, "toggle", (-1.3, 22.4, zf - 1.0), (1.3, 23.4, zf - 0.55), "bone_lt")
    g.box(b, "toggle_knob_l", (-1.9, 22.1, zf - 1.2), (-1.2, 23.7, zf - 0.4), "bone_dk")
    g.box(b, "toggle_knob_r", (1.2, 22.1, zf - 1.2), (1.9, 23.7, zf - 0.4), "bone_dk")
    for k, (x, h) in enumerate(((-0.8, 3.2), (0.0, 4.4), (0.9, 2.6))):
        g.box(b, f"tassel_{k}", (x - 0.2, 22.1 - h, zf - 0.8), (x + 0.2, 22.4, zf - 0.4), "cord" if k % 2 == 0 else "cord_dk")
    g.box(b, "tail_knot", (2.2, 21.6, zf - 0.75), (3.0, 22.6, zf - 0.25), "cord_dk")
    g.box(b, "tail", (2.4, 17.4, zf - 0.6), (2.8, 21.6, zf - 0.3), "cord")


def part_plate(g: Gear) -> None:
    """右肩锈铁补片：一块铁板压在肩披上，四角铆钉、一道锈迹，周围两道绑绳把它缝死。"""
    b = "shoulder_plate"
    g.box(b, "plate", (5.9, 24.75, -1.8), (8.2, 25.4, 1.2), "iron")        # 厚铁板（0.65px），压在肩披上
    g.box(b, "plate_rust", (6.2, 25.4, -1.4), (7.2, 25.65, -0.2), "rust")
    g.box(b, "plate_rust2", (7.4, 25.4, 0.2), (8.0, 25.65, 0.9), "rust_dk")
    for k, (x, z) in enumerate(((6.1, -1.6), (7.7, -1.6), (6.1, 0.7), (7.7, 0.7))):
        g.box(b, f"rivet_{k}", (x, 25.35, z), (x + 0.4, 25.7, z + 0.4), "rust_dk")
    g.box(b, "lash_a", (5.7, 25.3, -1.0), (8.3, 25.6, -0.6), "cord")
    g.box(b, "lash_b", (5.7, 24.7, 0.1), (8.3, 24.95, 0.4), "cord_dk")


PART_FUNCS = {
    "hood": part_hood,
    "peak": part_peak,
    "collar": part_collar,
    "mantle": part_mantle,
    "drape": part_drape,
    "back": part_back,
    "front": part_front,
    "hem": part_hem,
    "patches": part_patches,
    "stitches": part_stitches,
    "neck_tie": part_neck_tie,
    "plate": part_plate,
}

SPEC = GearSpec(
    gear_id="cu_ma_dou_peng",
    model_name="CuMaDouPeng",
    title="粗麻斗篷 / cu_ma_dou_peng",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：头（兜帽壳）+ 双肩（盖在手臂上沿）+ 背（背幅垂到小腿）",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
