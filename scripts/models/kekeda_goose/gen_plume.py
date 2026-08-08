#!/usr/bin/env python3
"""珂珂达（kekeda_goose）绒羽 / 外观层 —— 最终成品那一层。

**核心做法：球是绒羽的包络，不是一个漂浮的球壳。**

参考照片里那只"完美球体"的笑点是解剖学事实——鸭雁的身体本体只有球的一半出头，
剩下全是绒羽（加一层鹅脂）。所以这层不是往身上套个球，而是：

  给绒羽定一个**目标外壳**（近似椭球），再从躯干实体表面朝外壳长羽簇。
  羽簇长度 = 该方向上"外壳 − 体表"的距离，各处不同：
  胸前和体侧的绒羽薄（底下有胸肌顶着），腰窝和颈肩交界的绒羽厚到 2.5 单位。
  这样球是被撑出来的，凑近看每一簇的长短都对得上底下的解剖。

不长羽的地方要留白：喙、跗跖以下的腿脚（角质）、眼。留错了立刻穿帮——
参考照片里最抓眼的三处橙色（喙 / 两只蹼）恰恰都是**没有羽毛**的部位。

分部件（逐件可单独预览）：
  body    躯干绒羽球        neck   颈羽套筒（把头和球连起来的那一段）
  head    头羽 + 眼         bill   喙（从骨架层取形，角质不长羽）
  wing    翼覆羽 + 初级飞羽  tail   尾羽扇 + 尾上覆羽
  legs    跗跖 + 蹼足

用法:
  python3 scripts/models/kekeda_goose/gen_plume.py                  # 成品
  python3 scripts/models/kekeda_goose/gen_plume.py --part body
  python3 scripts/models/kekeda_goose/gen_plume.py --with-anatomy   # 叠在骨+肌上
  python3 scripts/models/kekeda_goose/gen_plume.py --list
"""

from __future__ import annotations

import argparse
import math
import random
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))

import gen_muscle as MU  # noqa: E402
import gen_skeleton as SK  # noqa: E402
from voxel_rig import Palette, Rig, Vec, lerp, smoothstep  # noqa: E402

OUT_DIR = SK.OUT_DIR
SEED = 0x9005E  # 固定种子：羽簇抖动必须可复现，两次生成的几何要逐字一致

PLUME_MATS = {
    # 纯白在末法残土里太干净。取暖白 + 一档灰白压腹侧 + 一档脏白给下腹和泄殖腔周围，
    # 远看仍是"大白鹅"，凑近有旧毛的层次
    # 三档之间的差值要小。首版 down/shade/grime 跨了 47 级灰，再按 rng 随机撒到
    # 相邻羽簇上，整只读成迪斯科球——绒羽的真实观感是几乎均匀的一片白，
    # 明暗来自朝向不是来自颜色。现在压到 16 级，且按高度平滑分层不随机撒
    "down": (240, 236, 227),
    "down_shade": (224, 219, 208),
    "down_grime": (206, 199, 182),
    "quill": (203, 197, 180),      # 飞羽 / 尾羽：比绒羽灰，带羽轴感
    "quill_dark": (172, 166, 150),
    "eye": (26, 24, 22),
    "eye_ring": (236, 230, 214),
}
# 调色板必须把**三层的材质全拼进来且顺序固定**：骨架 → 肌肉 → 绒羽。
# element 的 uv 是按各自生成时的调色板下标算的，叠层预览时是直接搬 element 过来的。
# 首版只拼了 SK+PLUME，肌肉那 7 种材质的下标位置被绒羽占掉 —— 半剖图里整片胸肌
# 渲成了白色，而且"看着像羽毛"，不盯着找根本发现不了。
PALETTE = Palette({**SK.MATS, **MU.MUSCLE_MATS, **PLUME_MATS})

# ================================================================ 绒羽外壳
# 目标外壳：略扁于正球（真鸟纵向稍长），中心比躯干几何中心靠后一点点 ——
# 胸前有喙和颈挡着，视觉重心本来就偏后
PUFF_C: Vec = (0.0, 7.85, -0.30)
PUFF_R: Vec = (4.75, 4.72, 5.15)
CLUMP_BANDS = 11


def puff_point(phi: float, theta: float) -> Vec:
    """外壳上的点。phi 从顶(0)到底(π)，theta 绕 Y 轴（0 = 正后方 +Z）。"""
    s = math.sin(phi)
    d = (s * math.sin(theta), math.cos(phi), s * math.cos(theta))
    return tuple(c + r * v for c, r, v in zip(PUFF_C, PUFF_R, d))


def inside_body(p: Vec) -> bool:
    """点是否在带肉躯干内。截面按 (半宽, 腹缘~背缘) 当椭圆算。"""
    x, y, z = p
    if not (MU.BODY_Z0 <= z <= MU.BODY_Z1):
        return False
    half, ylo, yhi = MU.body_profile(z)
    yc, yr = (ylo + yhi) / 2, (yhi - ylo) / 2
    if half <= 1e-6 or yr <= 1e-6:
        return False
    return (x / half) ** 2 + ((y - yc) / yr) ** 2 <= 1.0


def clump_root(outer: Vec, *, max_len: float = 2.9, min_len: float = 0.85) -> Vec:
    """从外壳点朝壳心走，找到体表交点 = 羽簇根部。

    没打到实体（颈窝下方、腿间这些方向）就按 max_len 截断——否则羽簇会一路穿到
    对侧去，腹侧整片绒羽会连成一坨实心块，凑近看没有任何层次。
    """
    d = tuple(c - o for c, o in zip(PUFF_C, outer))
    total = math.sqrt(sum(v * v for v in d)) or 1.0
    d = tuple(v / total for v in d)
    steps = 26
    for i in range(1, steps + 1):
        t = total * i / steps
        if t > max_len:
            break
        p = tuple(o + v * t for o, v in zip(outer, d))
        if inside_body(p):
            t = max(min_len, t)
            return tuple(o + v * t for o, v in zip(outer, d))
    return tuple(o + v * max_len for o, v in zip(outer, d))


def down_mat(p: Vec, rng: random.Random) -> str:
    """按高度**平滑分层**选羽色：背侧亮、腹侧压暗、下腹带脏。

    只在分界线附近抖 ±0.04 让边缘不齐；不整体随机撒——随机撒会让相邻羽簇跳色，
    远看就是一身斑点，而不是一只白鹅。
    """
    h = (p[1] - (PUFF_C[1] - PUFF_R[1])) / (2 * PUFF_R[1]) + rng.uniform(-0.04, 0.04)
    if h < 0.11:
        return "down_grime"
    return "down_shade" if h < 0.32 else "down"


def part_body(rig: Rig) -> None:
    """躯干绒羽球：按纬度带铺羽簇，每簇从体表长到外壳。"""
    rng = random.Random(SEED)
    rig.bone("plume_body", (0.0, PUFF_C[1], PUFF_C[2]), parent="trunk_back")
    for b in range(CLUMP_BANDS):
        phi = math.pi * (b + 0.5) / CLUMP_BANDS
        # 每带的簇数正比于该纬度的周长，密度才均匀；两极不能和赤道同数，
        # 否则极点挤成一坨、赤道拉出缝
        n = max(4, round(16 * math.sin(phi)))
        arc = 2 * math.pi * PUFF_R[0] * math.sin(phi) / n     # 相邻簇的弧距
        band_h = math.pi * PUFF_R[1] / CLUMP_BANDS
        for k in range(n):
            theta = 2 * math.pi * (k + 0.5 * (b % 2)) / n     # 隔带错半格 = 交错排列
            outer = puff_point(phi, theta)
            root = clump_root(outer)
            # 簇尖略微越壳，长短抖动 —— 齐平的话球面像高尔夫球，不像毛
            over = rng.uniform(0.05, 0.34)
            d = tuple(o - r for o, r in zip(outer, root))
            ln = math.sqrt(sum(v * v for v in d)) or 1.0
            tip = tuple(o + v / ln * over for o, v in zip(outer, d))
            # 半宽必须 > 间距的一半，相邻簇才互相压住。首版取 0.36~0.46 倍间距
            # （即全宽只有间距的 0.72~0.92），簇与簇之间留出缝，球面成了网格。
            w = min(arc, band_h) * rng.uniform(0.60, 0.70)
            rig.shaft("plume_body", f"down_{b}_{k}", root, tip,
                      max(0.30, w), max(0.30, w), mat=down_mat(outer, rng))


# ================================================================ 颈羽
def part_neck(rig: Rig) -> None:
    """颈羽套筒。这段是参考照片里"头直接坐在球上"的成因——颈其实很长，
    只是折成 S 又裹了一圈厚绒羽，从外面看只剩球顶到头底那一小截。"""
    rng = random.Random(SEED ^ 0x11)
    # 不要"沿曲线串一串粗筒"：颈羽半径 1.3~2.3，而 t 切细之后每段弧长不到 0.4，
    # 段比自己粗五六倍 —— 串出来是一摞垂直于曲线的**大圆盘**，侧看正是一把扇子。
    # （和颈肌那次"竖鳍"是同一个病：截面尺寸必须小于段长。）
    # 改成和躯干球同一种构造：沿曲线取若干站，每站在**曲线的法平面**内朝外长羽簇。
    # 羽簇是细的径向短棒，跟段长无关，从根上避开这个坑。
    STATIONS, SECTORS = 8, 9
    for s in range(STATIONS):
        t = 0.34 + 0.66 * (s + 0.5) / STATIONS
        p = SK.neck_at(t)
        bone = f"neck_{min(int(t * SK.NECK_VERTEBRAE), SK.NECK_VERTEBRAE - 1)}"
        # 法平面标架：颈曲线整条在 x=0 的矢状面里，所以 u 恒取 x 轴，
        # v = 切向 × u 一定落在矢状面内，不必做退化处理
        q = SK.neck_at(min(1.0, t + 0.02))
        ty, tz = q[1] - p[1], q[2] - p[2]
        n = math.hypot(ty, tz) or 1.0
        ty, tz = ty / n, tz / n
        core = MU.neck_radius(t)
        out_r = lerp(2.25, 1.34, smoothstep((t - 0.34) / 0.66))
        for k in range(SECTORS):
            a = 2 * math.pi * (k + 0.5 * (s % 2)) / SECTORS
            ca, sa = math.cos(a), math.sin(a)
            # u=(1,0,0)，v = T×u = (0, tz, -ty)
            d = (ca, sa * tz, -sa * ty)
            r_out = out_r * rng.uniform(0.94, 1.07)
            root = tuple(pp + dd * core * 0.85 for pp, dd in zip(p, d))
            tip = tuple(pp + dd * r_out for pp, dd in zip(p, d))
            # 已经埋在躯干球里的那些别画：白填一层还会从球面上顶出疙瘩
            if sum(((tt - cc) / rr) ** 2 for tt, cc, rr in zip(tip, PUFF_C, PUFF_R)) < 0.94:
                continue
            w = 2 * math.pi * out_r / SECTORS * 0.62
            rig.shaft(bone, f"neck_down_{s}_{k}", root, tip, w, w,
                      mat="down" if d[1] > -0.55 else "down_shade")


# ================================================================ 头 / 喙 / 眼
def part_head(rig: Rig) -> None:
    """头羽：包住脑颅，**在喙根前干净收口**。眼是一颗小黑豆，位置高而靠前。"""
    c, r = MU.head_profile()
    rng = random.Random(SEED ^ 0x22)
    # 眼所在的方向要**空出来**。首版羽簇铺满整个头壳，眼球在 x=±1.6，而羽簇能伸到
    # ±2.1 —— 眼睛被自己的脸毛埋了，正面完全看不见。留一小片无羽区，眼才嵌得进去
    eye_dir = (1.0, 0.18, -0.62)
    en = math.sqrt(sum(v * v for v in eye_dir))
    eye_dir = tuple(v / en for v in eye_dir)
    for b in range(5):
        phi = math.pi * (b + 0.5) / 5
        # 头羽的 theta 必须**对称采样**（±θ 成对），不能像躯干那样绕一圈均分。
        # 均分时左右两侧落点不一样，让眼的那片无羽区就只在一边开出来 ——
        # 正面看成了独眼。躯干上这种不对称是有机感，脸上是缺陷。
        half = max(2, round(4.5 * math.sin(phi)))
        for k in range(2 * half):
            j, sgn = divmod(k, 2)
            theta = (1 if sgn == 0 else -1) * math.pi * (j + 0.5) / half
            s = math.sin(phi)
            d = (s * math.sin(theta), math.cos(phi), s * math.cos(theta))
            outer = tuple(cc + (rr + 0.18) * v for cc, rr, v in zip(c, r, d))
            # 喙根之前不铺羽：羽毛盖到喙上就成了毛脸怪
            if outer[2] < SK.BILL_ROOT_Z + 0.35:
                continue
            if abs(d[0] * eye_dir[0]) + d[1] * eye_dir[1] + d[2] * eye_dir[2] > 0.86:
                continue                      # 让位给眼（左右都让，故 x 取绝对值）
            root = tuple(cc + rr * 0.55 * v for cc, rr, v in zip(c, r, d))
            rig.shaft("skull", f"head_down_{b}_{k}", root, outer, 0.46, 0.46,
                      mat="down" if d[1] > -0.45 else "down_shade")
    # 额羽：喙根到头顶那道过渡，鸭雁这里有条明显的分界线
    rig.cube("skull", "forehead", (-1.05, c[1] + 0.35, SK.BILL_ROOT_Z + 0.20),
             (1.05, c[1] + 1.42, c[2] - 0.30), mat="down")
    # 颊羽：把眼窝托起来
    for sx, side in ((-1, "l"), (1, "r")):
        rig.cube("skull", f"cheek_{side}",
                 (sx * 0.95, c[1] - 1.20, c[2] - 1.35), (sx * 1.62, c[1] + 0.55, c[2] + 0.95), mat="down")

    # 眼：小、黑、一颗豆子，长在头侧偏高偏前处。视野接近全周 —— 也是它总能
    # 先发现你的原因。两件事要同时成立：① 凸出羽面（x 顶到 ±2.0 外）才在侧视看得见；
    # ② 位置够靠前、且**朝前那面留够宽**，正视才有那颗小黑点。
    # 只满足 ① 的话正面就是只白团子，谁在看谁完全读不出来
    # 眼圈必须**退到眼珠后面**再从四周露边。首版眼圈前缘 z=-5.25、眼珠 -5.11，
    # 轴对齐盒子没有"环"的概念——正视时那圈浅色把黑眼珠整个盖死，只剩两块白方片。
    # 眼珠要在所有能看见的方向上都比眼圈突出：前（-z）、外（±x）各让 0.10 以上
    for sx, side in ((-1, "l"), (1, "r")):
        rig.cube("skull", f"eye_ring_{side}",
                 (sx * 1.26, c[1] + 0.04, c[2] - 1.18), (sx * 1.92, c[1] + 0.92, c[2] - 0.14), mat="eye_ring")
        rig.cube("skull", f"eye_{side}",
                 (sx * 1.42, c[1] + 0.20, c[2] - 1.34), (sx * 2.04, c[1] + 0.76, c[2] - 0.30), mat="eye")


def part_bill(rig: Rig) -> None:
    """喙：直接复用骨架层的角质件（喙就是角质，本来就不长羽，不需要另做一套）。"""
    src = Rig(SK.PALETTE)
    SK.part_trunk(src)
    SK.part_neck(src)
    SK.part_skull(src)
    SK.part_bill(src)
    keep = {tuple(SK.PALETTE.uv(m)) for m in ("keratin", "keratin_dark", "lamella", "socket")}
    want = {"bill_upper", "jaw"}
    for name in ("bill_upper", "jaw"):
        for eid in src.bones[name]["children"]:
            e = next(x for x in src.elements if x["uuid"] == eid)
            if tuple(e["faces"]["north"]["uv"]) not in keep:
                continue
            # uv 依赖调色板下标；本层前缀与骨架一致（见 gen_muscle 的同款断言）
            rig.elements.append(e)
            rig.bones[name if name in want else "skull"]["children"].append(eid)


# ================================================================ 翼 / 尾
def part_wing(rig: Rig, sx: int, side: str) -> None:
    """翼覆羽（贴体侧的一层）+ 初级飞羽（收起来搭过尾根，尖端露在球外）。"""
    # 覆羽：沿体侧铺一片，压在绒羽球外层，读作"翅膀收在这儿"
    for i in range(4):
        z0, z1 = lerp(-2.6, 2.2, i / 4), lerp(-2.6, 2.2, (i + 1) / 4)
        half, _, yhi = MU.body_profile((z0 + z1) / 2)
        rig.cube(f"wing_{side}", f"covert_{side}_{i}",
                 (sx * (half + 0.55), yhi - 3.30, z0), (sx * (half + 1.62), yhi - 0.55, z1),
                 mat="quill" if i % 2 else "down_shade")
    # 初级飞羽：收翼时是**互相叠合的一摞**贴在腰侧，不是张开的扇子。
    # 首版按 x 拉开 0.8、z 拉到 6.6，3/4 视角下成了从屁股后伸出来的一把折扇；
    # 参考照片那只球身上根本看不见翅膀。现在收紧到几乎共线，只让最外侧几根的
    # 尖端探出球面一点点——这一点点是判断"它是只鸟"的最低限度
    root = (sx * 2.90, 9.55, 0.90)
    for i in range(6):
        t = i / 5
        # 尖端别越过球面太多：探出 1.7 单位时侧看是从球腰横伸出来的一片白板，
        # 把"球"的轮廓戳破了。收到刚好蹭着球面，只在 3/4 视角露一线
        tip = (sx * lerp(2.55, 2.90, t), lerp(9.05, 8.65, t), lerp(3.45, 4.05, t))
        rig.shaft(f"hand_{side}", f"primary_{side}_{i}", root, tip,
                  0.32 - 0.024 * i, 0.11, mat="quill" if i % 2 else "quill_dark")
    # 三级飞羽：肘部那几根短的，压在体侧收口。要压进绒羽球以内，
    # 探出去就是从球腰上横伸出来的一块白板
    rig.cube(f"forearm_{side}", f"tertial_{side}",
             (sx * 2.30, 9.15, 0.30), (sx * 3.55, 10.05, 2.70), mat="quill")


def part_tail(rig: Rig) -> None:
    """尾羽扇：短、上翘。鸭雁的尾在鼓成球的身体后面只剩一小撮，是判断朝向的关键。"""
    # 尾扇要小。首版尖端伸到 z=8.3、高到 11.35，从侧面看是根翘起的尖刺，
    # 把"球"的轮廓戳破了。鸭雁鼓成球时尾巴只露一小撮，够判朝向就行
    for i in range(9):
        t = (i - 4) / 4                       # -1 .. 1，中间那根最长
        rig.shaft("tail_base", f"rectrix_{i}",
                  (t * 0.50, SK.trunk_y(5.6) + 0.25, 5.30),
                  (t * 1.55, lerp(10.70, 10.10, abs(t)), lerp(7.15, 6.35, abs(t))),
                  0.32, 0.15, mat="quill" if i % 2 else "quill_dark")
    # 尾上覆羽 / 尾下覆羽：把扇根盖住，别让尾羽像插上去的
    rig.cube("tail_base", "tail_covert_up", (-1.35, 10.05, 4.55), (1.35, 11.05, 6.30), mat="down")
    rig.cube("tail_base", "tail_covert_down", (-1.25, 8.45, 4.40), (1.25, 9.50, 6.05), mat="down_grime")


# ================================================================ 腿脚
def part_legs(rig: Rig) -> None:
    """跗跖 + 蹼足：整段角质，一根羽毛都不长。橙色，是全身第二、第三个视觉焦点。"""
    for sx, side in ((-1, "l"), (1, "r")):
        _hip, _knee, ankle, toe_base = SK.leg_joints(sx)
        # 胫跗骨下段那一小截也露在绒羽外（球底 y≈3.1，踝在 3.3）
        rig.shaft(f"tibia_{side}", f"shank_{side}",
                  (ankle[0], ankle[1] + 0.85, ankle[2] - 0.10), ankle, 0.52, 0.54, mat="keratin")
        rig.shaft(f"tarsus_{side}", f"tarsus_{side}", ankle, toe_base, 0.50, 0.52, mat="keratin")
        for i in range(5):                    # 跗跖鳞：横向一圈圈，鸟腿的读点
            t = (i + 0.5) / 5
            p = [lerp(a, b, t) for a, b in zip(ankle, toe_base)]
            rig.cube(f"tarsus_{side}", f"scute_{side}_{i}",
                     (p[0] - 0.56, p[1] - 0.14, p[2] - 0.58), (p[0] + 0.56, p[1] + 0.14, p[2] + 0.56),
                     mat="keratin_dark")
        tips: dict[str, Vec] = {}
        for name, dx, dz, r0 in SK.TOES:
            end = (toe_base[0] + sx * dx, 0.34, dz)
            tips[name] = end
            for j in range(3):
                p0 = tuple(lerp(u, v, j / 3) for u, v in zip(toe_base, end))
                p1 = tuple(lerp(u, v, (j + 1) / 3) for u, v in zip(toe_base, end))
                r = lerp(r0 + 0.14, (r0 + 0.14) * 0.6, (j + 1) / 3)
                rig.shaft(f"foot_{side}", f"toe_{name}_{side}_{j}", p0, p1, r, r, mat="keratin")
            rig.shaft(f"foot_{side}", f"nail_{name}_{side}", end,
                      (end[0] + sx * dx * 0.10, 0.20, dz - 0.40), 0.14, 0.14, mat="keratin_dark")
        rig.shaft(f"foot_{side}", f"hallux_{side}", toe_base,
                  (toe_base[0] + sx * 0.10, 0.66, 1.05), 0.18, 0.18, mat="keratin")
        for a_name, b_name in (("ii", "iii"), ("iii", "iv")):
            pa, pb = tips[a_name], tips[b_name]
            for k in range(5):
                f0, f1 = k / 5, (k + 1) / 5
                fm = (f0 + f1) / 2
                reach = 0.88 + 0.12 * abs(2 * fm - 1)
                zf = lerp(lerp(pa[2], pb[2], fm), toe_base[2], 1.0 - reach)
                x0, x1 = lerp(pa[0], pb[0], f0), lerp(pa[0], pb[0], f1)
                # 名字不能和骨架层的蹼件撞（那边也叫 web_*）：叠层预览时
                # 自检按 name 建字典，同名会互相覆盖，报出来的是假违例
                rig.cube(f"foot_{side}", f"webskin_{a_name}{b_name}_{side}_{k}",
                         (min(x0, x1), 0.08, min(zf, toe_base[2])),
                         (max(x0, x1), 0.40, max(zf, toe_base[2])), mat="keratin")


# ================================================================ 装配
PARTS = {
    "body": ("躯干绒羽球", part_body),
    "neck": ("颈羽套筒", part_neck),
    "head": ("头羽 + 眼", part_head),
    "bill": ("喙", part_bill),
    "wing": ("翼覆羽 + 飞羽", lambda r: [part_wing(r, sx, s) for sx, s in ((-1, "l"), (1, "r"))]),
    "tail": ("尾羽扇", part_tail),
    "legs": ("跗跖 + 蹼足", part_legs),
}


def build(only: str | None = None, with_anatomy: bool = False, cutaway: bool = False) -> Rig:
    rig = Rig(PALETTE)
    # 前缀对拍要覆盖**所有会被搬进来的层**，不能只查骨架：漏查哪层，那层的
    # element 就会静默地渲成别的颜色
    for src_pal, who in ((SK.PALETTE, "骨架"), (MU.PALETTE, "肌肉")):
        assert PALETTE.names[:len(src_pal.names)] == src_pal.names, \
            f"调色板前缀与{who}层不一致，搬过来的 element uv 会指错色块"
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
        # 半剖：左半的绒羽整片拿掉，露出底下的骨与肌。
        # 直接把羽层叠在解剖层上是白叠——羽把一切盖死，出来的图和成品图一模一样，
        # 没有任何诊断价值。半剖才看得出各处绒羽有多厚、哪些方向其实是空的。
        # 判"在左半"用 origin（旋转件的真实中心），不用 from/to 中点：
        # 羽簇是旋转过的柱，from/to 是旋转**前**的盒，中点会落在别处
        dropped = {e["uuid"] for e in rig.elements[n_anat:] if e["origin"][0] < -0.15}
        rig.elements = [e for e in rig.elements if e["uuid"] not in dropped]
        for b in rig.bones.values():
            b["children"] = [c for c in b["children"] if c not in dropped]
    return rig


def report(rig: Rig, *, symmetric: bool = True) -> int:
    # 半剖模型左右本来就不对称，别拿镜像自检去量它
    problems = rig.mirror_problems() if symmetric else []
    lo, hi = rig.bounds()
    width, height, length = hi[0] - lo[0], hi[1], hi[2] - lo[2]

    # 参考照片的读点：球宽 ≈ 站高的 0.55~0.62，露腿 ≈ 站高的 0.15~0.22
    ball_ratio = (2 * PUFF_R[0]) / height
    shank = (PUFF_C[1] - PUFF_R[1]) / height
    if not 0.55 <= ball_ratio <= 0.66:
        problems.append(f"球太{'小' if ball_ratio < 0.55 else '大'}：球宽/站高 = {ball_ratio:.2f}（应 0.55~0.66）")
    if not 0.13 <= shank <= 0.24:
        problems.append(f"露腿比例失衡：球底/站高 = {shank:.2f}（应 0.13~0.24）")
    if lo[1] < -0.05 or lo[1] > 0.30:
        problems.append(f"贴地异常：最低点 y={lo[1]:.2f}；最低几件："
                        f"{', '.join(f'{n}@{y:.2f}' for y, n in rig.lowest(4))}")

    print(f"cube {len(rig.elements)} 个 · 骨骼 {len(rig.bones)} 根")
    print(f"站高 {height:.2f} = {height / 16:.2f} m · 球宽 {2 * PUFF_R[0]:.2f}"
          f"（占站高 {ball_ratio:.0%}）· 全宽 {width:.2f} · 全长 {length:.2f} = {length / 16:.2f} m")
    print(f"球心 {PUFF_C[1]:.2f} · 球底 {PUFF_C[1] - PUFF_R[1]:.2f}（露腿 {shank:.0%}）· 最低点 {lo[1]:.2f}")
    if problems:
        print(f"\n✗ {len(problems)} 处违例：")
        for p in problems[:12]:
            print(f"   {p}")
    else:
        print("\n✓ 镜像 / 球体比例 / 露腿比例 / 贴地 全部通过")
    return len(problems)


def main() -> int:
    ap = argparse.ArgumentParser(description="珂珂达绒羽 / 外观层")
    ap.add_argument("--part", choices=sorted(PARTS))
    ap.add_argument("--with-anatomy", action="store_true",
                help="半剖：左半留骨+肌，右半留绒羽，看各处羽厚")
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
    ok = report(rig, symmetric=not args.with_anatomy)
    return 1 if ok and not args.part else 0


if __name__ == "__main__":
    raise SystemExit(main())
