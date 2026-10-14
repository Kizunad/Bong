#!/usr/bin/env python3
"""枯藤腰带（ku_teng_yao_dai）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #10（.task-wasteland-gear.md）。穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

外形：枯藤编的宽腰带，绕腰一圈。腰线 y≈11.7–14.0 正是双臂（x=±4..±8、y≥12）的摆动空间，
所以腰带只在身体前后（z<-2.05 / z>2.05）走满整圈宽度，到两侧（|x|≥4.05）就沉到 y<12 的胯上，
侧面一段挂在髋骨线上，不进手臂盒也不进腿盒。藤纹用斜放的短藤条左右对折（人字编）。
挂件：右前胯一把骨柄皮鞘小刀、前中一只网兜干葫芦、前左一支骨哨、左前胯一圈麻绳；背后正中一枚骨扣，
右后一束打结的藤梢垂下来。每件挂件单独一骨，方便以后做晃动。

分部件：part_*() 逐件写，--part 单件预览；骨骼树在 BONES 里一处定义。
用法：
    python3 modelScript/gear/ku_teng_yao_dai/gen_ku_teng_yao_dai.py --self-test   # 门禁差分自证
    python3 modelScript/gear/ku_teng_yao_dai/gen_ku_teng_yao_dai.py --part knife  # 单件预览
    python3 modelScript/gear/ku_teng_yao_dai/gen_ku_teng_yao_dai.py --worn        # 只出穿戴三面
    python3 modelScript/gear/ku_teng_yao_dai/gen_ku_teng_yao_dai.py               # 全量：模型 + check.png
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质（RGB，每种占贴图一个 8×8 色块）
MATS = {
    "vine":       (116, 90, 58),     # 枯藤主色
    "vine_lt":    (150, 122, 80),
    "vine_dk":    (80, 62, 42),
    "vine_dry":   (168, 142, 98),    # 干枯的藤梢
    "cord":       (154, 130, 86),
    "cord_dk":    (108, 88, 56),
    "bone_lt":    (228, 218, 190),
    "bone_dk":    (164, 150, 120),
    "bone_stain": (126, 106, 78),
    "hide":       (112, 80, 58),     # 鞘皮
    "hide_dk":    (74, 54, 42),
    "iron":       (88, 80, 76),
    "rust":       (150, 84, 48),
    "rust_dk":    (104, 56, 34),
    "gourd":      (158, 102, 58),
    "gourd_dk":   (118, 74, 44),
    "gourd_lt":   (188, 132, 82),
    "cork":       (150, 116, 80),
    "void":       (28, 24, 22),
}
SOFT = ("vine", "vine_lt", "vine_dk", "vine_dry", "cord", "cord_dk", "hide", "hide_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("bone_lt", "hide_dk", "vine_dk"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("belt_front", "root", (0.0, 12.9, -2.4), "前腰带：底带 + 人字藤纹 + 上下边绳"),
    ("belt_back", "root", (0.0, 12.9, 2.4), "后腰带（同前）"),
    ("belt_side_r", "root", (4.4, 11.0, 0.0), "右侧腰带：沉到 y<12 的胯上，避开手臂盒"),
    ("belt_side_l", "root", (-4.4, 11.0, 0.0), "左侧腰带（镜像）"),
    ("buckle_back", "belt_back", (0.0, 12.9, 3.2), "背后正中骨扣 + 绳圈"),
    ("hang_knife", "belt_front", (3.6, 13.0, -3.0), "右前胯：骨柄皮鞘小刀 + 皮扣环（会晃）"),
    ("hang_gourd", "belt_front", (1.0, 11.2, -3.4), "前中：网兜干葫芦 + 悬绳（会晃）"),
    ("hang_whistle", "belt_front", (-2.0, 11.2, -3.2), "前左：骨哨 + 悬绳（会晃）"),
    ("hang_coil", "belt_front", (-3.7, 11.0, -3.2), "左前胯：一圈麻绳 + 挂钩（会晃）"),
    ("belt_tail", "belt_back", (3.6, 12.0, 3.0), "右后垂下的藤梢束 + 打结（会晃）"),
]
ASYM_BONES = frozenset({"hang_knife", "hang_gourd", "hang_whistle", "hang_coil", "belt_tail", "buckle_back"})

# ───────────────────────────────────────────────────────────── 常量
Y_LO, Y_HI = 11.7, 14.0           # 前 / 后腰带 y 范围
TX = 4.0                          # 前 / 后底带半宽（躯干宽）
BX = 4.75                         # 含拐角的腰带半宽
ZF = (-2.7, -2.05)                # 前带底带 z
ZB = (2.05, 2.7)                  # 后带底带 z
SIDE_Y = (10.0, 11.95)            # 侧带 y：整段在手臂盒（y≥12）下面


def _strand(g: Gear, bone: str, name: str, cx: float, cy: float, z0: float, z1: float, ang: float, mat: str,
            length: float = 2.4, width: float = 0.8) -> None:
    """一根斜放的短藤条（绕 z 轴转 ang 度），中心 (cx, cy)，厚度沿 z 为 z0..z1。"""
    g.pair(bone, name, (cx - length / 2, cy - width / 2, z0), (cx + length / 2, cy + width / 2, z1), mat,
           rot=(0.0, 0.0, ang), org=(cx, cy, (z0 + z1) / 2))


# ───────────────────────────────────────────────────────────── 分部件：腰带本体
def _band(g: Gear, bone: str, tag: str, z_in: float, z_out: float) -> None:
    """前 / 后腰带：一块底带 + 三行人字藤纹（左右镜像成对，斜向相反）+ 上下各一道边绳 + 边上的藤须。"""
    s = 1 if z_out > z_in else -1                     # 向外的方向：后带 +z，前带 -z
    z0, z1 = (z_in, z_in + s * 0.65)
    zlo, zhi = min(z0, z1), max(z0, z1)
    g.box(bone, f"{tag}_base", (-TX, Y_LO, zlo), (TX, Y_HI, zhi), "vine_dk")
    # 拐角：前 / 后带在身体侧缘（|x|∈[4,4.75]）整段落到 y10，把侧带接住；z 范围在手臂盒（|z|<2）之外
    g.pair(bone, f"{tag}_corner", (TX, SIDE_Y[0], zlo), (BX, Y_HI, zhi), "vine")
    zo_a, zo_b = (zhi, zhi + 0.35) if s > 0 else (zlo - 0.35, zlo)
    rows = ((12.15, (0.75, 2.25, 3.75)), (12.85, (1.5, 3.0, 4.5)), (13.55, (0.75, 2.25, 3.75)))
    mats = ("vine", "vine_lt", "vine_dk", "vine_dry")
    for r, (cy, xs) in enumerate(rows):
        for k, cx in enumerate(xs):
            _strand(g, bone, f"{tag}_strand_r{r}_{k}", cx, cy, zo_a, zo_b, 38.0 * s, mats[(r + k) % 4])
    g.box(bone, f"{tag}_knot_mid", (-0.55, 12.45, zo_a), (0.55, 13.25, zo_b + 0.1), "vine_lt")
    # 上下边绳：贴着底带上下沿，比底带厚一点
    g.box(bone, f"{tag}_edge_top", (-TX, Y_HI, zlo - 0.05), (TX, Y_HI + 0.35, zhi + 0.05), "cord_dk")
    g.box(bone, f"{tag}_edge_bot", (-TX, Y_LO - 0.35, zlo - 0.05), (TX, Y_LO, zhi + 0.05), "cord")


def part_front(g: Gear) -> None:
    """前腰带：底带 + 人字藤纹 + 上下边绳。"""
    _band(g, "belt_front", "front", ZF[1], ZF[0])
    # 藤须：带边几根翘起的细藤（不对称由挂件骨以外的位置体现，这里左右镜像）
    for k, x in enumerate((1.2, 3.4)):
        g.pair("belt_front", f"front_whisker_{k}", (x, Y_HI + 0.35, -2.55), (x + 0.3, Y_HI + 0.95 + 0.2 * k, -2.25), "vine_dry")


def part_back(g: Gear) -> None:
    """后腰带：同前，藤纹在背面一侧。"""
    _band(g, "belt_back", "back", ZB[0], ZB[1])
    for k, x in enumerate((1.6, 3.3)):
        g.pair("belt_back", f"back_whisker_{k}", (x, Y_HI + 0.35, 2.25), (x + 0.3, Y_HI + 0.95 + 0.2 * k, 2.55), "vine_dry")


def part_sides(g: Gear) -> None:
    """侧带：沉到 y∈[10,11.95] 的胯上，x∈[4.05,4.75]，沿 z 通到前后带的角上；藤纹斜线沿 z 方向（绕 x 轴转）。"""
    b = "belt_side"
    g.pair(b, "side_base", (TX, SIDE_Y[0], -2.05), (BX, SIDE_Y[1], 2.05), "vine_dk", sided=True)
    # 一行四根斜藤（长 1.6，转 38° 后 y 向最多 ±0.8，不超过 y12），左右斜向交替
    for k, cz in enumerate((-1.4, -0.45, 0.45, 1.4)):
        mat = ("vine", "vine_lt", "vine_dk", "vine_dry")[k % 4]
        g.pair(b, f"side_strand_{k}", (BX, 10.95 - 0.4, cz - 0.8), (BX + 0.35, 10.95 + 0.4, cz + 0.8), mat,
               rot=((38.0 if k % 2 == 0 else -38.0), 0.0, 0.0), org=(BX + 0.17, 10.95, cz), sided=True)
    g.pair(b, "side_edge_bot", (TX, SIDE_Y[0] - 0.3, -2.05), (BX + 0.05, SIDE_Y[0], 2.05), "cord", sided=True)


# ───────────────────────────────────────────────────────────── 分部件：背后骨扣 / 藤梢
def part_buckle(g: Gear) -> None:
    """背后正中的骨扣：一根横骨穿过绳圈，两端是骨节，下面一小截绳头。"""
    b = "buckle_back"
    z = 3.05
    g.box(b, "bar", (-1.4, 12.5, z), (1.4, 13.3, z + 0.5), "bone_lt")
    g.box(b, "knob_l", (-2.1, 12.2, z - 0.05), (-1.3, 13.6, z + 0.65), "bone_dk")
    g.box(b, "knob_r", (1.3, 12.2, z - 0.05), (2.1, 13.6, z + 0.65), "bone_dk")
    g.box(b, "loop_top", (-0.5, 13.3, z), (0.5, 14.3, z + 0.35), "cord_dk")
    g.box(b, "loop_bot", (-0.5, 11.6, z), (0.5, 12.5, z + 0.35), "cord")
    g.box(b, "tie_tail", (0.3, 9.6, z), (0.7, 11.6, z + 0.35), "cord")


def part_tail(g: Gear) -> None:
    """右后垂下的藤梢束：四根长短不一、末端发散的藤条，中间一道锈铁箍和一个打结。"""
    b = "belt_tail"
    z = 2.9
    for k, (x, h, mat) in enumerate(((3.2, 3.6, "vine"), (3.8, 4.8, "vine_lt"), (4.4, 3.0, "vine_dk"), (2.6, 2.6, "vine_dry"))):
        g.box(b, f"tail_strand_{k}", (x, Y_LO - h, z), (x + 0.5, Y_LO, z + 0.5), mat)
        g.box(b, f"tail_tip_{k}", (x + 0.05, Y_LO - h - 0.8, z + 0.05 * (k % 2)), (x + 0.45, Y_LO - h, z + 0.4 + 0.05 * (k % 2)), "vine_dry")
    g.box(b, "tail_band", (2.5, 8.9, z - 0.1), (5.0, 9.7, z + 0.6), "rust")
    g.box(b, "tail_band_dk", (2.8, 8.9, z + 0.6), (4.7, 9.4, z + 0.8), "rust_dk")
    g.box(b, "tail_knot", (3.0, 9.7, z - 0.1), (4.4, 10.5, z + 0.7), "cord_dk")


# ───────────────────────────────────────────────────────────── 分部件：挂件
def part_knife(g: Gear) -> None:
    """右前胯骨柄皮鞘小刀：皮鞘（尖端收窄）+ 两道锈铁箍 + 皮扣环 + 露在腰带上面的骨柄与双骨节。"""
    b = "hang_knife"
    g.box(b, "sheath", (2.9, 6.2, -3.5), (4.3, 11.6, -2.7), "hide")
    g.box(b, "sheath_dk", (4.0, 6.4, -3.4), (4.35, 11.4, -2.8), "hide_dk")
    g.box(b, "sheath_tip", (3.2, 5.2, -3.3), (4.0, 6.2, -2.9), "hide_dk")
    g.box(b, "sheath_tip_cap", (3.4, 4.7, -3.2), (3.8, 5.2, -3.0), "rust_dk")
    for k, y in enumerate((7.4, 10.1)):
        g.box(b, f"band_{k}", (2.8, y, -3.6), (4.4, y + 0.65, -2.65), "iron")
        g.box(b, f"band_rust_{k}", (3.0, y + 0.1, -3.85), (3.7, y + 0.5, -3.6), "rust")
    g.box(b, "loop", (3.0, 11.2, -3.2), (4.2, 12.9, -2.65), "hide_dk")               # 皮扣环：绕过腰带
    g.box(b, "loop_stitch", (3.3, 11.6, -3.45), (3.9, 12.0, -3.2), "cord_dk")
    g.box(b, "hilt", (3.2, 14.0, -3.3), (4.0, 15.5, -2.9), "hide_dk")                 # 柄
    g.box(b, "hilt_wrap", (3.1, 14.4, -3.4), (4.1, 14.8, -2.8), "cord")
    g.box(b, "pommel_a", (3.0, 15.5, -3.4), (3.65, 16.6, -2.8), "bone_lt")           # 骨节：两个小疙瘩
    g.box(b, "pommel_b", (3.65, 15.5, -3.4), (4.3, 16.5, -2.8), "bone_dk")
    g.box(b, "guard", (2.9, 13.9, -3.5), (4.3, 14.2, -2.7), "iron")


def part_gourd(g: Gear) -> None:
    """前中干葫芦：上下两截 + 肩 + 颈 + 木塞，外面一张草绳网，一根悬绳挂在腰带上。"""
    b = "hang_gourd"
    g.box(b, "belly", (-0.2, 6.4, -4.5), (2.2, 9.2, -2.75), "gourd")
    g.box(b, "belly_shade", (1.9, 6.6, -4.3), (2.3, 9.0, -2.95), "gourd_dk")
    g.box(b, "belly_hi", (0.0, 7.2, -4.6), (0.5, 8.6, -4.3), "gourd_lt")
    g.box(b, "base", (0.2, 5.8, -4.2), (1.8, 6.4, -3.0), "gourd_dk")
    g.box(b, "shoulder", (0.2, 9.2, -4.2), (1.8, 9.9, -3.0), "gourd")
    g.box(b, "neck", (0.55, 9.9, -3.9), (1.45, 10.5, -3.3), "gourd_dk")
    g.box(b, "cork", (0.45, 10.5, -4.0), (1.55, 11.0, -3.2), "cork")
    g.box(b, "link", (0.7, 11.0, -3.7), (1.3, 11.5, -3.3), "cord_dk")
    g.box(b, "loop", (0.6, 11.2, -3.3), (1.4, 12.6, -2.7), "hide_dk")
    for k, y in enumerate((7.0, 8.3)):                                                  # 草绳网：两道横绳 + 两道竖绳
        g.box(b, f"net_h_{k}", (-0.35, y, -4.65), (2.35, y + 0.4, -2.65), "cord")
    for k, x in enumerate((0.3, 1.5)):
        g.box(b, f"net_v_{k}", (x, 6.5, -4.8), (x + 0.4, 9.1, -4.4), "cord_dk")
    g.box(b, "net_knot", (0.8, 8.9, -4.85), (1.4, 9.5, -4.45), "cord_dk")


def part_whistle(g: Gear) -> None:
    """前左骨哨：一截扁骨 + 吹口 + 哨孔 + 悬绳绕两圈；绳从腰带上的皮环垂下来。"""
    b = "hang_whistle"
    g.box(b, "body", (-2.5, 7.8, -3.5), (-1.5, 10.4, -2.9), "bone_lt")
    g.box(b, "body_shade", (-1.75, 7.9, -3.4), (-1.45, 10.2, -3.0), "bone_dk")
    g.box(b, "mouth", (-2.3, 10.4, -3.4), (-1.7, 11.0, -3.0), "bone_lt")
    g.box(b, "hole", (-2.2, 8.4, -3.7), (-1.8, 9.0, -3.5), "void")
    g.box(b, "end_cap", (-2.6, 7.4, -3.6), (-1.4, 7.8, -2.8), "bone_dk")
    g.box(b, "wrap_a", (-2.6, 10.0, -3.6), (-1.4, 10.45, -2.8), "cord")
    g.box(b, "wrap_b", (-2.6, 9.4, -3.6), (-1.4, 9.8, -2.8), "cord_dk")
    g.box(b, "link", (-2.2, 11.0, -3.3), (-1.8, 11.7, -3.0), "cord_dk")
    g.box(b, "loop", (-2.4, 11.4, -3.2), (-1.6, 12.8, -2.7), "hide_dk")


def part_coil(g: Gear) -> None:
    """左前胯麻绳圈：三圈叠放（xy 平面的环，沿 z 叠）+ 铁钩挂在腰带上 + 两缕垂尾。"""
    b = "hang_coil"
    for k in range(3):
        g.ring(b, f"coil_{k}", (-3.7, 8.8, -3.1 - 0.6 * k), 1.5, 0.7, 0.6, "cord_dk" if k % 2 else "cord")
    g.box(b, "hook", (-4.05, 10.7, -3.3), (-3.4, 12.8, -2.7), "iron")
    g.box(b, "hook_rust", (-4.1, 10.9, -3.5), (-3.3, 11.4, -3.3), "rust")
    g.box(b, "tie", (-4.9, 8.3, -4.3), (-2.5, 8.9, -3.0), "cord_dk")
    g.box(b, "tail_a", (-4.4, 5.8, -3.8), (-4.0, 8.3, -3.4), "cord")
    g.box(b, "tail_b", (-3.6, 6.4, -3.6), (-3.2, 8.3, -3.2), "cord_dk")


PART_FUNCS = {
    "front": part_front,
    "back": part_back,
    "sides": part_sides,
    "buckle": part_buckle,
    "tail": part_tail,
    "knife": part_knife,
    "gourd": part_gourd,
    "whistle": part_whistle,
    "coil": part_coil,
}

SPEC = GearSpec(
    gear_id="ku_teng_yao_dai",
    model_name="KuTengYaoDai",
    title="枯藤腰带 / ku_teng_yao_dai",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=9,
    wear_note="挂点：腰一圈（前后带 y∈[11.7,14]，两侧沉到 y<12 的胯上避开手臂盒）+ 前胯挂件 + 背后骨扣",
    worn_focus=((0.0, 11.0, 0.0), 22.0),
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
