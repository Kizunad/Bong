#!/usr/bin/env python3
"""珂珂达（kekeda_goose）绒羽 / 外观层 —— 最终成品那一层。呆萌向。

**造型原则：可爱靠干净的大形，不靠细碎的表面。**

最初把绒羽做成上百簇朝各方向的旋转小方块（"羽簇从体表长到外壳"，解剖上讲得通），
渲出来是个刺球：每簇的边角各自吃光，表面全是噪点，圆是圆了但很丑。现在反过来 ——
球用**分带切片**堆，全是轴对齐块，台阶顺着曲率走。

三条决定光滑度的：
  · 每条纬带切成 N 块拼近似**圆**截面。一带一个盒子的话横截面是正方形，四个角在
    3/4 视角下是四道硬棱 —— 正面侧面都圆、一转 45° 就是座金字塔。
  · 纬带按**极角**分、并往两极加密。按 y 均分的话两极宽度突变、球顶球底各出现
    一圈方台；按极角分好一些，但球顶朝上的台阶面在 MC 口径里是最亮的 1.0，
    一圈圈亮环仍显眼，所以再往两极压一压。
  · 判"圆不圆滑"必须用 render(..., shading="mc")。默认的 lambert 光相邻朝向差
    2.5 倍，会把阶梯面照成一身竖条纹（MC 原版只差 1.33 倍）。曾误以为是截面不够密，
    加密之后条纹反而更细更多 —— 那是在修渲染器的锅。

呆萌的几个杠杆，都做进来了：
  · 头相对身体放大到球宽的 0.39 倍（真鸭约 0.30）—— 幼体比例
  · 眼睛小而齐平、**带一粒高光点**。高光是可爱度最省力的一招；但眼珠做大做凸会在
    3/4 视角变成挂在头侧的黑板（"墨镜"），所以只留 0.08 的最小凸出
  · 喙短、宽、干净：不露栉板（那排"牙"是威吓姿态的读点，留在骨架层）
  · 腿短粗，蹼足做成一整片扇形，不拆细趾
  · 配色收干净：一档暖白 + 一档腹侧浅灰，去掉脏白；喙脚取偏亮的暖橙

不长羽的地方仍严格留白：喙、跗跖以下、眼 —— 参考照片里最抓眼的三处橙
（喙 + 两只蹼）恰好都是无羽部位。

分部件（逐件可单独预览）：
  body 躯干球 · neck 颈 · head 头 · bill 喙 + 眼 · wing 收翼 · tail 尾墩 · legs 腿脚

翅膀是**贴着体表长的一片薄壳**、挂在自己的 wing_l/r 骨上：静止时净外凸只有 0.08，
读作身体的一部分；骨骼一转整片掀起，底下的体表是完整的。

用法:
  python3 scripts/models/kekeda_goose/gen_plume.py                  # 成品
  python3 scripts/models/kekeda_goose/gen_plume.py --part body
  python3 scripts/models/kekeda_goose/gen_plume.py --with-anatomy   # 半剖，看羽厚
  python3 scripts/models/kekeda_goose/gen_plume.py --list
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

import gen_muscle as MU  # noqa: E402
import gen_skeleton as SK  # noqa: E402
from voxel_rig import Palette, Rig, Vec, lerp  # noqa: E402

OUT_DIR = SK.OUT_DIR

PLUME_MATS = {
    # 只留两档白。首版三档跨 47 级灰又随机撒，读成迪斯科球；即便压到 16 级，
    # 多一档"脏白"仍然是在给干净的大形添噪 —— 呆萌不要旧毛层次
    "down": (243, 240, 232),
    "down_shade": (224, 219, 207),
    "bill_h": (240, 158, 66),        # 暖橙偏亮：末法调色本偏暗，但这只要的是讨喜
    "bill_dark": (203, 120, 44),
    "eye_h": (26, 24, 22),
    "eye_light": (252, 252, 250),    # 眼高光 —— 可爱度性价比最高的一块
}
# noise 调到 2：默认 ±4 的抖动在喙这种大面积平色上会读成木纹
PALETTE = Palette({**SK.MATS, **MU.MUSCLE_MATS, **PLUME_MATS}, noise=2)

# ================================================================ 形体常数
BALL_C: Vec = (0.0, 7.70, -0.25)
BALL_R: Vec = (4.80, 4.80, 5.10)
# 密度档位（实测，球身块数 / 最大台阶）。光滑度基本由这两个数决定：
#    7 带 ×  3 环 =  21 块 / 3.24u  —— 成了个十字疙瘩，不能用
#   19 带 ×  7 环 = 133 块 / 1.22u  —— 台阶明显
#   27 带 × 10 环 = 270 块 / 0.86u  —— 拐点，特写也站得住（当前档）
#   37 带 × 14 环 = 518 块 / 0.63u  —— 更细，但收益已经很小
# 游戏观看距离上 19 / 27 / 37 三档几乎看不出差别；要压块数就往下调这两个数。
BALL_BANDS = 27

HEAD_C: Vec = (0.0, 14.25, -3.85)
HEAD_R: Vec = (1.88, 1.78, 1.72)
HEAD_BANDS = 13

# sin(θ) 的指数，<1 让两极更钝。正球两极收得尖，读作几何体；
# 钝一点才像一团被绒毛撑起来的东西
BLUNT = 0.84


POLE_BIAS = 0.35


def _theta(u: float) -> float:
    """极角分布：两极加密、赤道放宽。

    等角均分已经比等高均分好（等高分的话两极宽度突变，球顶球底各出现一圈方台），
    但球顶那几圈**朝上的台阶面**在 MC 口径里是最亮的 1.0，一圈圈亮环特别显眼。
    往两极压一压，那些环就细了；赤道那边宽度本来就几乎不变，切厚不吃亏。
    g'(u) = 1 - k·cos(2πu) 恒正，所以单调，不会翻带。
    """
    return math.pi * (u - POLE_BIAS * math.sin(2 * math.pi * u) / (2 * math.pi))


def bands(center: Vec, radii: Vec, n: int, *, blunt: float = BLUNT):
    """椭球切片 → [(y0, y1, hx, hz)]。"""
    _, cy, _ = center
    rx, ry, rz = radii
    out = []
    for i in range(n):
        th0, th1 = _theta(i / n), _theta((i + 1) / n)
        y1, y0 = cy + ry * math.cos(th0), cy + ry * math.cos(th1)
        s = math.sin((th0 + th1) / 2) ** blunt
        out.append((y0, y1, rx * s, rz * s))
    return out


# 每条切片拆成 N 块轴对齐盒子，并集近似**圆**截面（角点全落在同一外接圆上）。
# 单块盒子的横截面是正方形，四个角在 3/4 视角下是四道硬棱 —— 正面侧面都圆、
# 一转 45° 就露馅成方筒（实测像座金字塔）。
#
# 判这类"够不够圆滑"的问题时务必用 render(..., shading="mc")：本预览器默认的
# lambert 光把 +x 面夹到 0.32、+z 面给到 0.80，相邻 facet 差 2.5 倍，阶梯面被照成
# 一身竖条纹；MC 原版是 0.6 / 0.8，只差 1.33 倍，同一个模型看上去干净得多。
# 我一度以为是截面不够密，加到 7 块条纹反而更细更多 —— 那是在修渲染器的锅。
BODY_SECTION = 10
HEAD_SECTION = 7


def ring(n: int):
    """n 块盒子的 (x, z) 半径系数，并集近似单位圆。"""
    return [(math.cos(math.pi / 2 * (i + 0.5) / n), math.sin(math.pi / 2 * (i + 0.5) / n))
            for i in range(n)]


def ball_hx_hz(y: float) -> tuple[float, float]:
    """球在高度 y 处横截面的 (半宽, 半深)。"""
    t = (y - BALL_C[1]) / BALL_R[1]
    if abs(t) >= 1.0:
        return 0.0, 0.0
    s = math.sqrt(1 - t * t) ** BLUNT
    return BALL_R[0] * s, BALL_R[2] * s


def ball_surface_x(y: float, z: float) -> float:
    """体表在 (y, z) 处的 x。翅膀贴着这条曲面长，才是"长在身上"而不是"贴上去"。"""
    hx, hz = ball_hx_hz(y)
    if hz <= 1e-6:
        return 0.0
    u = (z - BALL_C[2]) / hz
    return hx * math.sqrt(1 - u * u) if abs(u) < 1.0 else 0.0


# ================================================================ 躯干球
def part_body(rig: Rig) -> None:
    """绒羽球：一摞等角切片。腹侧压一档浅灰当底阴影，别再分更多档。"""
    rig.bone("plume_body", (0.0, BALL_C[1], BALL_C[2]), parent="trunk_back")
    for i, (y0, y1, hx, hz) in enumerate(bands(BALL_C, BALL_R, BALL_BANDS)):
        mat = "down_shade" if i < 4 else "down"
        for j, (ax, az) in enumerate(ring(BODY_SECTION)):
            rig.cube("plume_body", f"down_band_{i}_{j}",
                     (-hx * ax, y0, BALL_C[2] - hz * az), (hx * ax, y1, BALL_C[2] + hz * az), mat=mat)


def part_neck(rig: Rig) -> None:
    """颈：球顶到头底的一小截。真颈很长（17 节折成 S），整条埋在球里，露出来就这点。"""
    steps = 7
    for i in range(steps):
        t0, t1 = i / steps, (i + 1) / steps
        y0, y1 = lerp(10.60, 12.80, t0), lerp(10.60, 12.80, t1)
        tm = (t0 + t1) / 2
        zc = lerp(-2.20, HEAD_C[2] + 0.10, tm)
        w = lerp(2.25, 1.74, tm)
        for j, (ax, az) in enumerate(ring(HEAD_SECTION)):
            rig.cube("neck_12", f"neck_band_{i}_{j}",
                     (-w * ax, y0, zc - w * 0.92 * az), (w * ax, y1, zc + w * 0.92 * az), mat="down")


def part_head(rig: Rig) -> None:
    """头：同一套切片。相对球宽 0.39（真鸭约 0.30）—— 幼体比例，呆萌的第一杠杆。"""
    for i, (y0, y1, hx, hz) in enumerate(bands(HEAD_C, HEAD_R, HEAD_BANDS)):
        for j, (ax, az) in enumerate(ring(HEAD_SECTION)):
            rig.cube("skull", f"head_band_{i}_{j}",
                     (-hx * ax, y0, HEAD_C[2] - hz * az), (hx * ax, y1, HEAD_C[2] + hz * az), mat="down")


# ================================================================ 喙 / 眼
BILL_ROOT_Z = -5.45
BILL_TIP_Z = -7.85


def part_bill(rig: Rig) -> None:
    """喙：短、宽、干净。不露栉板 —— 那排"牙"是威吓姿态的读点，留在骨架层。

    每段的 (半宽, 腹缘, 背缘) 一起收，正面才是个圆头铲子，而不是贴在脸上的橙方片。
    """
    # 关键点定形，中间插值加密到 8 段。首版只切 4 段，每段之间上缘差 0.2、
    # 侧看是四条横楞，整只喙读成个小木条箱
    # 上缘（culmen）几乎拉平，锥度全放到下缘 —— 真鸭喙的背线本来就是直的，只在
    # 尖端才下弯。而且 MC 口径里朝上的面最亮（1.0）、朝下的最暗（0.5）：上缘每一级
    # 台阶都会亮成一道横楞，下缘的台阶则基本看不见。把落差挪到下缘，同样的锥度、
    # 一半的楞
    key = [
        (0.00, 1.02, 13.54, 14.26),
        (0.34, 1.24, 13.40, 14.16),
        (0.68, 1.32, 13.26, 14.06),
        (1.00, 0.98, 13.14, 13.84),
    ]

    def at(t: float):
        for a, b in zip(key, key[1:]):
            if a[0] <= t <= b[0]:
                u = (t - a[0]) / (b[0] - a[0])
                u = u * u * (3 - 2 * u)
                return tuple(lerp(a[i], b[i], u) for i in (1, 2, 3))
        return key[-1][1:]

    z_root = HEAD_C[2] - 1.15
    N = 8
    for i in range(N):
        t0, t1 = i / N, (i + 1) / N
        w, y0, y1 = at((t0 + t1) / 2)
        rig.cube("bill_upper", f"bill_{i}",
                 (-w, y0, lerp(z_root, BILL_TIP_Z, t1)), (w, y1, lerp(z_root, BILL_TIP_Z, t0)),
                 mat="bill_h")
    # 喙甲：尖端一小片深色。只做一小片，做大了显凶
    rig.cube("bill_upper", "bill_nail",
             (-0.60, 13.14, BILL_TIP_Z), (0.60, 13.50, BILL_TIP_Z + 0.40), mat="bill_dark")
    # 下喙：薄薄一条托在下面，侧面才看得出上下两片，而不是一根实棍
    rig.cube("jaw", "bill_lower",
             (-1.00, 12.96, BILL_TIP_Z + 0.30), (1.00, 13.30, BILL_ROOT_Z + 0.30), mat="bill_dark")


def part_eyes(rig: Rig) -> None:
    """眼：大、圆、略朝前，带高光点。

    深度关系是这里唯一的坑：轴对齐盒子没有"环"或"贴面"的概念，谁在前谁把谁盖死。
    三件必须逐件更靠前（-z 更小）：头 → 眼 → 高光。首版就是眼圈前缘比眼珠还靠前，
    正视时整颗黑眼珠被白圈盖死，只剩两块白方片。
    """
    # 眼珠别做太大、高光更别做大：首版眼 1.00 高、高光占了里面一大块，
    # 两只连起来读成一副护目镜。真正可爱的是"小黑豆 + 角上一粒白点"
    # 小一点、齐平一点。凸出去太多会在 3/4 视角变成挂在头侧的黑板（"墨镜"），
    # 但完全齐平又会和头的切片共面打架，所以留 0.08 的最小凸出
    ey0, ey1 = 14.24, 14.86
    # 头的外缘要按眼**跨度内最宽**的那一带算 —— 也就是 [ey0,ey1] 里最靠近头中心的
    # 高度。按眼中心算是错的：眼下缘那几带比中心处更宽，会把眼珠下半截埋进头里，
    # 剩下的高光就跑到黑眼珠外侧，正面看像两面小白旗
    y_widest = min(max(HEAD_C[1], ey0), ey1)
    ct = max(-1.0, min(1.0, (y_widest - HEAD_C[1]) / HEAD_R[1]))
    s = math.sin(math.acos(ct)) ** BLUNT
    hx, hz = HEAD_R[0] * s, HEAD_R[2] * s
    front = HEAD_C[2] - hz
    for sx, side in ((-1, "l"), (1, "r")):
        # 进深要浅。首版眼盒深 1.10，从 3/4 看是块包住半个头侧的黑板 —— 墨镜就是
        # 这么来的。眼是一颗贴在脸上的豆子，三维尺寸得接近
        rig.cube("skull", f"eye_{side}",
                 (sx * 1.16, ey0, front - 0.08), (sx * (hx + 0.06), ey1, front + 0.42), mat="eye_h")
        # 高光是眼珠里的一小粒白点：x 范围必须**落在眼珠以内**（否则成了外挂的白片），
        # z 必须比眼珠更靠前（否则被眼珠盖住）
        # 一"粒"白点，不是一"片"。占到眼珠一半宽时，正面看就成了黑白两色的斑纹，
        # 而不是一只带反光的眼睛
        rig.cube("skull", f"eye_light_{side}",
                 (sx * 1.46, ey1 - 0.22, front - 0.19), (sx * (hx - 0.28), ey1 - 0.05, front - 0.02),
                 mat="eye_light")


# ================================================================ 翼 / 尾
def part_wing(rig: Rig, sx: int, side: str) -> None:
    """收起的翅膀：贴着体表长的一片薄壳，挂在自己的 wing_l/r 骨上。

    做法上和"贴一块板"的区别在于每一格的内缘都取自 ball_surface_x —— 面片顺着球面
    弯，静止时读作身体的一部分（只鼓出 0.4 单位），骨骼一转就整片掀起来，底下的
    体表是完整的。上一版是块平板，球身磨圆之后一贴就成了补丁。

    形状按羽片收：上下两端的 z 跨度收窄，中段最长 —— 收翼的轮廓是枚水滴不是矩形。
    """
    # 厚度只有 0.18，且内缘再往体内埋 0.10 —— 净外凸约 0.08，静止时看不出是块独立的
    # 东西。首版给了 0.40 厚、每列各自贴面，六行五列的台阶叠出来是块带棱的鳞板贴在
    # 体侧，比没有还糟。"一体式"的意思是静止时它就是身体的一部分，掀起来才是翅膀。
    y0, y1 = 6.35, 10.35
    zc, z_reach = 0.35, 3.05
    rows, cols, thick, sink = 5, 4, 0.18, 0.10
    for i in range(rows):
        ya, yb = lerp(y0, y1, i / rows), lerp(y0, y1, (i + 1) / rows)
        v = ((ya + yb) / 2 - (y0 + y1) / 2) / ((y1 - y0) / 2)
        zh = z_reach * math.sqrt(max(0.0, 1 - v * v)) ** 0.55
        for j in range(cols):
            za, zb = zc - zh + 2 * zh * j / cols, zc - zh + 2 * zh * (j + 1) / cols
            xin = ball_surface_x((ya + yb) / 2, (za + zb) / 2) - sink
            if xin <= 0.4:
                continue
            rig.cube(f"wing_{side}", f"wing_{side}_{i}_{j}",
                     (sx * xin, ya, za), (sx * (xin + thick), yb, zb), mat="down")


def part_tail(rig: Rig) -> None:
    """尾墩：屁股后一个上翘的小墩子。留着它是为了一眼看出朝向。"""
    # 分四段收，别用两个方块 —— 身体磨圆之后，屁股后面挂两个方块特别显眼
    key = ((4.00, 1.34, 9.20, 10.60), (4.85, 1.16, 9.42, 10.72),
           (5.55, 0.86, 9.70, 10.78), (6.15, 0.48, 9.98, 10.70))
    for i, (a, b) in enumerate(zip(key, key[1:])):
        w, y0, y1 = (a[1] + b[1]) / 2, (a[2] + b[2]) / 2, (a[3] + b[3]) / 2
        rig.cube("tail_base", f"tail_nub_{i}", (-w, y0, a[0]), (w, y1, b[0]), mat="down")


# ================================================================ 腿脚
LEG_X = 1.78
ANKLE_Y = 3.15


def part_legs(rig: Rig) -> None:
    """腿短粗、蹼做成一整片扇形。

    上一版按解剖拆了 4 趾 3 蹼 + 5 圈跗跖鳞，二十几块碎片挂在腿上 —— 这个尺度下
    全是噪点。呆萌要的是两根干净的橙柱子加两片大脚板。
    """
    for sx, side in ((-1, "l"), (1, "r")):
        for i, (y0, y1, w, zc, d) in enumerate((
            (1.95, ANKLE_Y + 0.25, 0.74, 0.30, 0.70),
            (0.60, 2.05, 0.64, 0.10, 0.62),
        )):
            rig.cube(f"tarsus_{side}", f"shank_{side}_{i}",
                     (sx * LEG_X - w, y0, zc - d), (sx * LEG_X + w, y1, zc + d), mat="bill_h")
        # 脚板：三级递宽的扇形，前缘再切两道浅口示意三趾。切太深就又碎了
        # 脚板要够厚够大。首版 0.46 厚，正面看是两片贴地的橙纸片
        for i, (z0, z1, w) in enumerate(((0.86, -0.35, 1.02), (-0.35, -1.55, 1.52), (-1.55, -2.65, 1.92))):
            rig.cube(f"foot_{side}", f"web_{side}_{i}",
                     (sx * LEG_X - w, 0.0, z1), (sx * LEG_X + w, 0.62, z0), mat="bill_h")
        for tag, k in (("a", -1), ("b", 1)):
            # 偏移量要连 sx 一起乘：只乘 k 的话左脚的两道口子是"平移"过去的不是镜像的
            rig.cube(f"foot_{side}", f"web_notch_{side}_{tag}",
                     (sx * (LEG_X + k * 0.46), 0.03, -2.65), (sx * (LEG_X + k * 0.80), 0.60, -2.00),
                     mat="bill_dark")


# ================================================================ 装配
PARTS = {
    "body": ("躯干球", part_body),
    "neck": ("颈", part_neck),
    "head": ("头", part_head),
    "bill": ("喙 + 眼", lambda r: (part_bill(r), part_eyes(r))),
    "wing": ("翼隆", lambda r: [part_wing(r, sx, s) for sx, s in ((-1, "l"), (1, "r"))]),
    "tail": ("尾墩", part_tail),
    "legs": ("腿脚", part_legs),
}


def build(only: str | None = None, with_anatomy: bool = False, cutaway: bool = False) -> Rig:
    rig = Rig(PALETTE)
    # 前缀对拍要覆盖**所有会被搬进来的层**，不能只查骨架：漏查哪层，那层的 element
    # 就会静默地渲成别的颜色（半剖图里整片胸肌曾被渲成白的，看着还挺像羽毛）
    for src_pal, who in ((SK.PALETTE, "骨架"), (MU.PALETTE, "肌肉")):
        assert PALETTE.names[: len(src_pal.names)] == src_pal.names, (
            f"调色板前缀与{who}层不一致，搬过来的 element uv 会指错色块"
        )
    src = MU.build() if with_anatomy else SK.build_full()
    for name in src.bone_order:
        b = src.bones[name]
        rig.bone(name, tuple(b["pivot"]), b["parent"])
    if with_anatomy:
        rig.elements.extend(src.elements)
        for name in src.bone_order:
            rig.bones[name]["children"] = list(src.bones[name]["children"])

    n_anat = len(rig.elements)
    for key, (_label, fn) in PARTS.items():
        if only is None or key == only:
            fn(rig)  # type: ignore[operator]

    if cutaway:
        # 半剖：外观层只留右半，露出底下的骨与肌。直接两层叠起来是白叠 —— 外观把
        # 一切盖死，出来的图和成品图一模一样，没有诊断价值。
        # 切片是轴对齐整片，所以**裁 x 范围**而不是整块丢；旋转件没法这么裁，
        # 按 origin 落在哪半边整块处理
        dropped = set()
        for e in rig.elements[n_anat:]:
            if any(e["rotation"]):
                if e["origin"][0] < -0.15:
                    dropped.add(e["uuid"])
            elif e["to"][0] <= 0.0:
                dropped.add(e["uuid"])
            elif e["from"][0] < 0.0:
                e["from"][0] = 0.0
        rig.elements = [e for e in rig.elements if e["uuid"] not in dropped]
        for b in rig.bones.values():
            b["children"] = [c for c in b["children"] if c not in dropped]
    return rig


def report(rig: Rig, *, symmetric: bool = True) -> int:
    # 半剖模型左右本来就不对称，别拿镜像自检去量它
    problems = rig.mirror_problems() if symmetric else []
    lo, hi = rig.bounds()
    height = hi[1]

    ball_w, head_w = 2 * BALL_R[0], 2 * HEAD_R[0]
    ball_ratio = ball_w / height
    shank = (BALL_C[1] - BALL_R[1]) / height
    head_ratio = head_w / ball_w
    if not 0.55 <= ball_ratio <= 0.66:
        problems.append(f"球太{'小' if ball_ratio < 0.55 else '大'}：球宽/站高 = {ball_ratio:.2f}（应 0.55~0.66）")
    if not 0.13 <= shank <= 0.24:
        problems.append(f"露腿比例失衡：球底/站高 = {shank:.2f}（应 0.13~0.24）")
    # 呆萌靠幼体比例：头相对球要明显大于真鸭的 0.30，但不能大到成了不倒翁
    if not 0.34 <= head_ratio <= 0.46:
        problems.append(f"头身比不在呆萌区间：头宽/球宽 = {head_ratio:.2f}（应 0.34~0.46）")
    if lo[1] < -0.05 or lo[1] > 0.30:
        problems.append(
            f"贴地异常：最低点 y={lo[1]:.2f}；最低几件："
            f"{', '.join(f'{n}@{y:.2f}' for y, n in rig.lowest(4))}"
        )

    print(f"cube {len(rig.elements)} 个 · 骨骼 {len(rig.bones)} 根")
    print(f"站高 {height:.2f} = {height / 16:.2f} m · 球宽 {ball_w:.2f}（占站高 {ball_ratio:.0%}）"
          f" · 全宽 {hi[0] - lo[0]:.2f} · 全长 {hi[2] - lo[2]:.2f} = {(hi[2] - lo[2]) / 16:.2f} m")
    print(f"头宽 {head_w:.2f}（球宽的 {head_ratio:.0%}，真鸭约 30%）· "
          f"球底 {BALL_C[1] - BALL_R[1]:.2f}（露腿 {shank:.0%}）· 最低点 {lo[1]:.2f}")
    if problems:
        print(f"\n✗ {len(problems)} 处违例：")
        for p in problems[:12]:
            print(f"   {p}")
    else:
        print("\n✓ 镜像 / 球体比例 / 头身比 / 露腿比例 / 贴地 全部通过")
    return len(problems)


def main() -> int:
    ap = argparse.ArgumentParser(description="珂珂达绒羽 / 外观层")
    ap.add_argument("--part", choices=sorted(PARTS))
    ap.add_argument("--with-anatomy", action="store_true", help="半剖：左半留骨+肌，右半留外观")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--check", action="store_true", help="只报告，不写文件")
    ap.add_argument("--out", type=Path)
    args = ap.parse_args()

    if args.list:
        for k, (label, _) in PARTS.items():
            print(f"  {k:5s} {label}")
        return 0
    if args.check:
        return 1 if report(build()) else 0

    rig = build(only=args.part, with_anatomy=args.with_anatomy, cutaway=args.with_anatomy)
    name = "KekedaPlume"
    if args.part:
        name += f"_{args.part}"
    if args.with_anatomy:
        name += "_anatomy"
    out = rig.save(args.out or (OUT_DIR / f"{name}.bbmodel"), name)
    print(f"→ {out}")
    bad = report(rig, symmetric=not args.with_anatomy)
    return 1 if bad and not args.part else 0


if __name__ == "__main__":
    raise SystemExit(main())
