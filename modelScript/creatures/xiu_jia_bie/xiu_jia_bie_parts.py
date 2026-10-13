"""锈甲鳖虫（xiu_jia_bie）几何部件库 —— Round 3/3。

依据 worldview.md §九 L848「金银如废土」：啃废金属为生的甲虫。外形要点（任务卡 #6）：
扁圆厚重的背甲，由锈铁片 / 碎金片 / 铜片层层叠成（片缘翘起、带铆钉孔和锈斑凹坑），
头小、一对短粗钳颚，六条短腿带倒刺，翻过来露出浅色分节软腹。

和 gen_xiu_jia_bie.py 分工：本文件只描述「有哪些骨、每块几何在哪」，渲染 / 门禁 /
接触表在生成器里。每个 part_*() 只往传入的 rig 里加几何，可单独调用做逐件预览。

坐标约定与流水线一致：地面 y=0，虫头朝 -Z。1 单位 = 1px = 1/16 格。
体型取「大狗」的 1.5 倍左右（含头 ≈ 36px 长、25px 宽），实体缩放留给接线阶段。

背甲的做法：一个半椭球穹顶（DOME）当「骨架曲面」，在曲面上按 6 行 × 4 列铺金属片。
每片沿曲面法线倾斜，行与行错开 0.3px 高度形成瓦片叠压；左右各自抽材质（有的铁、
有的金、有的铜），但几何严格镜像，门禁按 _l/_r 配对检查。
"""

from __future__ import annotations

import math
import random

Vec = tuple[float, float, float]
Ellipsoid = tuple[Vec, Vec]  # (中心, 三轴半径)

# ──────────────────────────────────────────────────────────────── 材质
MATS: dict[str, tuple[int, int, int]] = {
    "rust_iron": (122, 66, 38),
    "rust_dark": (80, 48, 34),
    "gold_flake": (186, 148, 66),
    "gold_dull": (140, 110, 58),
    "copper_orange": (156, 86, 48),
    "copper_patina": (92, 128, 104),
    "rivet": (46, 36, 30),
    "chitin_dark": (44, 38, 36),
    "chitin_mid": (72, 62, 55),
    "belly_pale": (188, 174, 146),
    "belly_seg": (132, 118, 96),
    "eye": (18, 16, 16),
    "jaw_gold": (166, 130, 66),
    "jaw_dark": (98, 70, 44),
}

# 门禁只认 8 种颜色（rigkit 按材质序号 % 8 写 color，14 种会撞色），归到 8 个「族」
FAMILY: dict[str, str] = {
    "rust_iron": "iron",
    "rust_dark": "iron",
    "gold_flake": "gold",
    "gold_dull": "gold",
    "copper_orange": "copper",
    "copper_patina": "copper",
    "rivet": "rivet",
    "chitin_dark": "chitin",
    "chitin_mid": "chitin",
    "belly_pale": "belly",
    "belly_seg": "belly",
    "eye": "eye",
    "jaw_gold": "jaw",
    "jaw_dark": "jaw",
}
FAMILY_ORDER = ("chitin", "belly", "iron", "gold", "copper", "rivet", "eye", "jaw")

PLATE_MATS = ("rust_iron", "rust_dark", "gold_flake", "gold_dull", "copper_orange", "copper_patina")
PATCH_MATS = ("gold_flake", "copper_patina", "rust_dark", "copper_orange", "rust_iron")

# ──────────────────────────────────────────────────────────────── 骨骼
# 体节 4 + 头 1 + 钳颚 4 + 触角 2 + 腿 6×3 + 背甲 7 + 尾刺 1 + root = 38 骨
_LEG_Z = (-8.0, -1.0, 6.5)  # 前 / 中 / 后腿的髋 z
_LEG_SPLAY = (-2.2, 0.0, 2.4)  # 膝处相对髋的 z 偏移：前腿向前、后腿向后
_SEG_FOR_LEG = ("seg_1", "seg_2", "seg_3")


def _leg_joints(sx: int, i: int) -> tuple[Vec, Vec, Vec, Vec]:
    """髋、膝、踝、趾尖的坐标。趾尖 y=0.35，免得斜杆的下角陷进地面。"""
    z, d = _LEG_Z[i], _LEG_SPLAY[i]
    return (
        (sx * 9.0, 4.4, z),
        (sx * 11.8, 5.6, z + d),
        (sx * 13.0, 2.4, z + d * 1.6),
        (sx * 13.4, 0.35, z + d * 1.9),
    )


def _bones() -> list[tuple[str, str | None, Vec, str]]:
    b: list[tuple[str, str | None, Vec, str]] = [
        ("root", None, (0.0, 0.0, 0.0), "世界根节点"),
        ("seg_1", "root", (0.0, 5.0, -9.0), "体节 1（前胸）"),
        ("seg_2", "seg_1", (0.0, 5.0, -3.0), "体节 2"),
        ("seg_3", "seg_2", (0.0, 5.0, 3.0), "体节 3"),
        ("seg_4", "seg_3", (0.0, 5.0, 9.0), "体节 4（腹端）"),
        ("head", "seg_1", (0.0, 4.4, -14.0), "小头"),
        ("mand_l1", "head", (-2.2, 3.4, -19.4), "左钳颚根节"),
        ("mand_l2", "mand_l1", (-2.0, 3.4, -22.0), "左钳颚尖节"),
        ("mand_r1", "head", (2.2, 3.4, -19.4), "右钳颚根节"),
        ("mand_r2", "mand_r1", (2.0, 3.4, -22.0), "右钳颚尖节"),
        ("antenna_l", "head", (-1.4, 6.4, -18.6), "左触角"),
        ("antenna_r", "head", (1.4, 6.4, -18.6), "右触角"),
        # 背甲：前胸盾一整块，鞘翅左右各一（可掀开），后盾左右各一，侧裙左右各一
        ("shell_pronotum", "seg_1", (0.0, 9.5, -9.0), "前胸盾"),
        ("shell_elytra_l", "seg_2", (-0.8, 10.5, -6.0), "左鞘翅（可掀）"),
        ("shell_elytra_r", "seg_2", (0.8, 10.5, -6.0), "右鞘翅（可掀）"),
        ("shell_rear_l", "seg_4", (-0.8, 9.0, 8.0), "左后盾"),
        ("shell_rear_r", "seg_4", (0.8, 9.0, 8.0), "右后盾"),
        ("shell_skirt_l", "seg_2", (-10.0, 6.0, 0.0), "左侧裙"),
        ("shell_skirt_r", "seg_2", (10.0, 6.0, 0.0), "右侧裙"),
        ("pygidium", "seg_4", (0.0, 4.6, 12.0), "尾刺"),
    ]
    for i, name in enumerate(("front", "mid", "rear")):
        for sx, t in ((-1, "l"), (1, "r")):
            hip, knee, ankle, _ = _leg_joints(sx, i)
            p = f"leg_{name}_{t}"
            b.append((f"{p}_femur", _SEG_FOR_LEG[i], hip, f"{name}腿 {t} 股节"))
            b.append((f"{p}_tibia", f"{p}_femur", knee, f"{name}腿 {t} 胫节"))
            b.append((f"{p}_tarsus", f"{p}_tibia", ankle, f"{name}腿 {t} 跗节"))
    return b


BONES_DEF = _bones()
LEG_NAMES = ("front", "mid", "rear")

# ──────────────────────────────────────────────────────────────── 椭球切片
# 与 can_bei_tuo_parts.ellipsoid 同一做法（沿 Y 切片，每片十字两块 cube 切圆角）。
# 两个生物各自一份：它们分属不同分支，且这份工具很小，不值得为共享而跨目录 import。
_GAP = 0.04


def ellipsoid(rig, bone: str, name: str, e: Ellipsoid, mat: str, *, step: float = 0.8,
              y_lo: float | None = None, y_hi: float | None = None, chamfer: float = 0.22) -> None:
    (cx, cy, cz), (rx, ry, rz) = e
    lo = cy - ry if y_lo is None else y_lo
    hi = cy + ry if y_hi is None else y_hi
    n, y0 = 0, lo
    while y0 < hi - 0.45:  # 顶端不足 0.45 的残片不要，否则切出渲不出一像素的薄片
        y1 = min(y0 + step, hi)
        t = ((y0 + y1) / 2 - cy) / ry
        if abs(t) < 1.0:
            s = math.sqrt(1.0 - t * t)
            hx, hz = round(rx * s, 1), round(rz * s, 1)
            if hx >= 0.8 and hz >= 0.8:
                k = round(max(0.7, chamfer * min(hx, hz)), 1)
                a0, a1 = y0 + _GAP / 2, y1 - _GAP / 2
                rig.cube(bone, f"{name}_{n:02d}a", (cx - hx + k, a0, cz - hz), (cx + hx - k, a1, cz + hz), mat=mat)
                rig.cube(bone, f"{name}_{n:02d}b", (cx - hx, a0 + 0.03, cz - hz + k),
                         (cx + hx, a1 - 0.03, cz + hz - k), mat=mat)
                n += 1
        y0 = y1


# ──────────────────────────────────────────────────────────────── 背甲曲面
DOME: Ellipsoid = ((0.0, 3.0, 0.0), (12.5, 9.4, 13.5))
PLATE_T = 0.8  # 金属片厚
# 沿曲面法线倾斜的符号约定：渲染器的欧拉角正方向要靠渲出来的图核对（见 Round 1 接触表的说明）
TILT_X = 1.0
TILT_Z = -1.0
EDGE_CURL = -8.0  # Round 3：5 → 8，整片再往上掀一点，片与片之间露出阴影缝
LIP_H = 1.25      # 前缘翘边高度（Round 1 是 0.55，侧视读成一条平线）
JITTER_X = 3.0    # 每片再随机歪一点，不像整齐的瓦
ROW_Z = [-11.0 + 4.4 * k for k in range(6)]  # 6 行
COL_X = [1.9, 5.3, 8.7]  # 鞘翅 3 列（每侧）；第 4 列是侧裙
SKIRT_X = 11.2
PLATE_W = 3.7
PLATE_L = 5.4


def _dome_k(x: float, z: float) -> float:
    (_, _, _), (rx, _, rz) = DOME
    return 1.0 - (x / rx) ** 2 - (z / rz) ** 2


def _dome_y(x: float, z: float) -> float:
    (_, cy, _), (_, ry, _) = DOME
    return cy + ry * math.sqrt(max(_dome_k(x, z), 0.0))


def _dome_normal(x: float, z: float) -> Vec:
    (_, cy, _), (rx, ry, rz) = DOME
    y = _dome_y(x, z)
    n = (x / rx**2, (y - cy) / ry**2, z / rz**2)
    m = math.sqrt(sum(c * c for c in n))
    return (n[0] / m, n[1] / m, n[2] / m)


def _plate(rig, bone: str, name: str, x: float, z: float, w: float, l: float, k: int, j: int, sx: int) -> None:
    """一片金属片 + 前缘翘边 + 两颗铆钉 + 一块锈斑 +（偶尔）一个凹坑。

    x 取该侧的正值，sx 给符号。几何由 (k, j) 种子决定，左右镜像；材质另按侧抽，
    让两边不是照抄。
    """
    if _dome_k(sx * x, z) < 0.07:  # 穹顶边角之外不铺
        return
    tag = "l" if sx < 0 else "r"
    geo = random.Random(k * 31 + j * 7 + 5)
    mat_rng = random.Random(k * 100 + j * 10 + (0 if sx < 0 else 1))
    px = sx * x
    nx, ny, nz = _dome_normal(px, z)
    rot_x = TILT_X * math.degrees(math.atan2(nz, ny)) + EDGE_CURL + geo.uniform(-JITTER_X, JITTER_X)
    rot_z = TILT_Z * math.degrees(math.atan2(nx, ny))
    lift = 0.3 * (k % 3) + geo.uniform(0.0, 0.25)  # 行间错高，避免同面叠压
    c = _dome_y(px, z)
    ctr = (px + nx * (PLATE_T / 2 - 0.2 + lift), c + ny * (PLATE_T / 2 - 0.2 + lift),
           z + nz * (PLATE_T / 2 - 0.2 + lift))
    rot = (rot_x, 0.0, rot_z * 1.0)
    mat = mat_rng.choice(PLATE_MATS)

    def piece(sub: str, lo: Vec, hi: Vec, m: str) -> None:
        # 局部框（以片心为原点、未旋转）→ 绝对坐标；绕片心旋转，和整片一起倾斜
        rig.cube(bone, f"{name}_{sub}_{tag}", (ctr[0] + lo[0], ctr[1] + lo[1], ctr[2] + lo[2]),
                 (ctr[0] + hi[0], ctr[1] + hi[1], ctr[2] + hi[2]), rot=rot, org=ctr, mat=m)

    hw, hl, ht = w / 2, l / 2, PLATE_T / 2
    piece("plate", (-hw, -ht, -hl), (hw, ht, hl), mat)
    piece("lip", (-hw + 0.25, ht, -hl), (hw - 0.25, ht + LIP_H, -hl + 0.7), mat)  # 前缘翘边
    piece("lip_tip", (-hw + 0.9, ht + LIP_H, -hl - 0.1), (hw - 0.9, ht + LIP_H + 0.4, -hl + 0.5), mat)  # 翘边卷口
    piece("under", (-hw + 0.5, -ht - 0.7, -hl + 0.2), (hw - 0.5, -ht, -0.4), "rust_dark")  # 翘起后露出的阴影
    if geo.random() < 0.55:  # 后缘也有翘的
        piece("lip_b", (-hw + 0.4, ht, hl - 0.6), (hw - 0.4, ht + 0.75, hl), mat)
    if x >= 8.7:  # 外侧列：外缘卷起
        a, b = sorted((sx * (hw - 0.6), sx * hw))
        piece("lip_s", (a, ht, -hl + 0.8), (b, ht + 0.9, hl - 0.8), mat)
    # 四角小铆钉（取符号才左右镜像）。只放两颗时两点加一块斑读成人脸，所以铺四角
    for sub, lx, lz in (("rivet_a", sx * (hw - 0.7), -hl + 1.5), ("rivet_b", -sx * (hw - 0.7), -hl + 1.5),
                        ("rivet_c", sx * (hw - 0.7), hl - 0.8), ("rivet_d", -sx * (hw - 0.7), hl - 0.8)):
        piece(sub, (lx - 0.3, ht, lz - 0.3), (lx + 0.3, ht + 0.4, lz + 0.3), "rivet")
    # 锈斑：换一族金属的薄补丁（x 局部坐标按侧取符号，保证镜像）
    pm = mat_rng.choice([m for m in PATCH_MATS if FAMILY[m] != FAMILY[mat]])
    px_l = sx * geo.uniform(-0.4, 0.5)
    pz_l = geo.uniform(-0.2, 1.0)
    pw, pl = geo.uniform(1.2, 1.9), geo.uniform(1.4, 2.4)
    piece("patch", (px_l - pw / 2, ht, pz_l - pl / 2), (px_l + pw / 2, ht + 0.22, pz_l + pl / 2), pm)
    if geo.random() < 0.2:  # 凹坑：深色小方，贴在片面上读作蚀穿的孔
        qx = sx * geo.uniform(-0.6, 0.6)
        qz = geo.uniform(-0.6, 1.2)
        piece("pit", (qx - 0.5, ht + 0.15, qz - 0.5), (qx + 0.5, ht + 0.38, qz + 0.5), "rivet")


# ──────────────────────────────────────────────────────────────── 部件：背甲
def part_carapace(rig) -> None:
    """6 行 × (3 列鞘翅 + 1 列侧裙) 的瓦片叠成穹顶；中缝留 0.4px 黑缝，鞘翅能从这里掀开。"""
    for k, z in enumerate(ROW_Z):
        for sx, t in ((-1, "l"), (1, "r")):
            for j, x in enumerate(COL_X):
                if k == 0:
                    bone = "shell_pronotum"
                elif k == 5:
                    bone = f"shell_rear_{t}"
                else:
                    bone = f"shell_elytra_{t}"
                _plate(rig, bone, f"p{k}{j}", x, z, PLATE_W, PLATE_L, k, j, sx)
            # 侧裙：整列挂在 skirt 骨上（第 0 行属前胸盾）
            bone = "shell_pronotum" if k == 0 else f"shell_skirt_{t}"
            _plate(rig, bone, f"p{k}3", SKIRT_X, z, 3.0, PLATE_L, k, 3, sx)


# ──────────────────────────────────────────────────────────────── 部件：躯干
def part_body(rig) -> None:
    """四节甲壳下的体壁（椭球）+ 浅色分节软腹 + 尾刺。"""
    # 体壁：每节一个椭球，前后收窄；只取 y>=3.0 的上半，下面留给软腹
    spec = [("seg_1", -9.0, 8.4, 4.0), ("seg_2", -3.0, 10.4, 3.8), ("seg_3", 3.0, 10.4, 3.8), ("seg_4", 9.0, 8.6, 4.0)]
    for bone, z, rx, rz in spec:
        ellipsoid(rig, bone, f"hull_{bone[-1]}", ((0.0, 5.2, z), (rx, 2.6, rz)), "chitin_mid", step=0.8, y_lo=3.0)
    # 软腹：5 个横向片，片间 0.6px 沟，两侧各一条更深的侧板
    belly = [(-10.0, 5.4), (-5.0, 7.0), (0.0, 7.6), (5.0, 7.0), (10.0, 5.0)]
    pivots = [(n, z) for n, _, (_, _, z), _ in BONES_DEF if n.startswith("seg_")]
    for i, (z, hw) in enumerate(belly):
        bone = min(pivots, key=lambda p: abs(p[1] - z))[0]
        rig.cube(bone, f"belly{i}", (-hw, 1.85, z - 2.2), (hw, 3.0, z + 2.2), mat="belly_pale")
        for sx, t in ((-1, "l"), (1, "r")):
            a, b = sorted((sx * (hw + 0.1), sx * (hw + 1.5)))
            rig.cube(bone, f"belly_side{i}_{t}", (a, 2.2, z - 2.0), (b, 3.0, z + 2.0), mat="belly_seg")
    # 尾刺：一根向后微翘的锥，三段逐级收细
    rig.shaft("pygidium", "pygidium_00", (0.0, 4.6, 12.0), (0.0, 4.8, 14.2), 1.3, 1.0, mat="rust_dark")
    rig.shaft("pygidium", "pygidium_01", (0.0, 4.8, 14.2), (0.0, 5.3, 16.0), 0.9, 0.7, mat="rust_iron")
    rig.shaft("pygidium", "pygidium_02", (0.0, 5.3, 16.0), (0.0, 6.0, 17.4), 0.5, 0.4, mat="rust_dark")


# ──────────────────────────────────────────────────────────────── 部件：头
def part_head(rig) -> None:
    """小头 + 盾片 + 复眼 + 一对短粗钳颚 + 触角。"""
    ellipsoid(rig, "head", "skull", ((0.0, 4.4, -16.8), (3.8, 2.5, 3.4)), "chitin_dark", step=0.7)
    rig.cube("head", "labrum", (-1.6, 2.5, -20.8), (1.6, 3.8, -19.6), mat="belly_pale")
    # 头盾：一片前倾的锈铁片，带铆钉
    rig.cube("head", "head_shield", (-3.0, 6.3, -19.6), (3.0, 7.0, -14.4), rot=(-12.0, 0.0, 0.0), mat="rust_iron")
    rig.cube("head", "head_shield_lip", (-2.6, 7.0, -19.7), (2.6, 7.5, -19.1), rot=(-12.0, 0.0, 0.0), mat="rust_iron")
    for sx, t in ((-1, "l"), (1, "r")):
        a, b = sorted((sx * 3.2, sx * 3.9))
        rig.cube("head", f"eye_{t}", (a, 4.6, -19.2), (b, 5.9, -17.8), mat="eye")
        a, b = sorted((sx * 2.2, sx * 3.8))
        rig.cube("head", f"cheek_plate_{t}", (a, 5.5, -17.2), (b, 6.2, -15.2), mat="gold_dull")
        a, b = sorted((sx * 0.8, sx * 1.6))
        rig.cube("head", f"head_rivet_{t}", (a, 7.03, -18.2), (b, 7.5, -17.4), mat="rivet")

    # 钳颚：根节向前外撇，尖节向内弯，内缘三枚齿；每侧两段（两根骨）
    for sx, t in ((-1, "l"), (1, "r")):
        b1, b2 = f"mand_{t}1", f"mand_{t}2"
        a, b = sorted((sx * 1.0, sx * 3.4))
        rig.cube(b1, f"mand_base_{t}", (a, 2.4, -22.2), (b, 4.6, -19.4), mat="jaw_gold")
        a, b = sorted((sx * 1.6, sx * 3.0))
        rig.cube(b1, f"mand_base_plate_{t}", (a, 4.62, -21.6), (b, 5.2, -19.8), mat="rust_iron")
        a, b = sorted((sx * 0.5, sx * 2.5))
        rig.cube(b2, f"mand_tip_{t}", (a, 2.5, -25.0), (b, 4.2, -22.2), rot=(0.0, sx * 14.0, 0.0), mat="jaw_dark")
        for i, z in enumerate((-24.4, -23.5, -22.7)):
            a, b = sorted((sx * 0.1, sx * 0.7))
            rig.cube(b2, f"mand_tooth{i}_{t}", (a, 2.8, z - 0.3), (b, 3.8, z + 0.3), mat="jaw_gold")
        # 尖节再往前探一点的钩尖
        a, b = sorted((sx * 0.3, sx * 1.3))
        rig.cube(b2, f"mand_hook_{t}", (a, 2.9, -26.2), (b, 3.9, -24.98), rot=(0.0, sx * 14.0, 0.0), mat="jaw_gold")

    # 触角：两节细杆
    for sx, t in ((-1, "l"), (1, "r")):
        rig.shaft(f"antenna_{t}", f"antenna0_{t}", (sx * 1.4, 6.4, -18.6), (sx * 2.8, 8.6, -21.6), 0.35, 0.35, mat="rust_dark")
        rig.shaft(f"antenna_{t}", f"antenna1_{t}", (sx * 2.8, 8.6, -21.6), (sx * 3.6, 9.4, -24.4), 0.28, 0.28, mat="rust_iron")


# ──────────────────────────────────────────────────────────────── 部件：腿
def _barb(rig, bone: str, name: str, a: Vec, b: Vec, sx: int, length: float) -> None:
    """倒刺：从 a→b 杆上一点向上、向外斜出一根细锥。"""
    d = (sx * 0.5, 1.0, 0.0)
    m = math.sqrt(sum(c * c for c in d))
    tip = tuple(a[i] + d[i] / m * length for i in range(3))
    rig.shaft(bone, name, a, tip, 0.38, 0.38, mat="rust_iron")


def _leg(rig, sx: int, i: int) -> None:
    t = "l" if sx < 0 else "r"
    p = f"leg_{LEG_NAMES[i]}_{t}"
    hip, knee, ankle, toe = _leg_joints(sx, i)

    # 髋球 + 股节
    rig.cube(f"{p}_femur", f"coxa_{i}_{t}", (hip[0] - 1.6, hip[1] - 1.3, hip[2] - 1.6),
             (hip[0] + 1.6, hip[1] + 1.3, hip[2] + 1.6), mat="chitin_mid")
    rig.shaft(f"{p}_femur", f"femur_{i}_{t}", hip, knee, 1.9, 1.7, mat="chitin_dark")
    # 膝甲：一块锈铁方片扣在膝上
    rig.cube(f"{p}_tibia", f"knee_armor_{i}_{t}", (knee[0] - 1.5, knee[1] - 0.2, knee[2] - 1.5),
             (knee[0] + 1.5, knee[1] + 1.2, knee[2] + 1.5), mat="rust_iron")
    rig.cube(f"{p}_tibia", f"knee_rivet_{i}_{t}", (knee[0] - 0.4, knee[1] + 1.22, knee[2] - 0.4),
             (knee[0] + 0.4, knee[1] + 1.7, knee[2] + 0.4), mat="rivet")
    # 胫节 + 三根倒刺（沿杆 30% / 55% / 80%）
    rig.shaft(f"{p}_tibia", f"tibia_{i}_{t}", knee, ankle, 1.5, 1.3, mat="chitin_dark")
    for k, f in enumerate((0.3, 0.55, 0.8)):
        pt = tuple(knee[a] + (ankle[a] - knee[a]) * f for a in range(3))
        _barb(rig, f"{p}_tibia", f"barb{k}_{i}_{t}", (pt[0], pt[1] + 0.5, pt[2]), pt, sx, 2.2 - 0.3 * k)
    # 跗节 + 两个爪趾
    rig.shaft(f"{p}_tarsus", f"tarsus_{i}_{t}", ankle, toe, 1.1, 0.95, mat="chitin_mid")
    for k, dz in enumerate((-0.9, 0.9)):
        rig.cube(f"{p}_tarsus", f"claw{k}_{i}_{t}", (toe[0] - 0.4, toe[1], toe[2] + dz - 0.4),
                 (toe[0] + 0.4, toe[1] + 0.9, toe[2] + dz + 0.4), mat="rust_dark")


def part_legs(rig) -> None:
    for i in range(3):
        for sx in (-1, 1):
            _leg(rig, sx, i)


PARTS_META = {
    "carapace": ("1. 背甲：6 行瓦片 + 侧裙，翘边 / 铆钉 / 锈斑 / 凹坑", [part_carapace]),
    "body": ("2. 躯干：四节体壁 + 浅色分节软腹 + 尾刺", [part_body]),
    "head": ("3. 头：小头 + 盾片 + 复眼 + 双钳颚 + 触角", [part_head]),
    "legs": ("4. 六条腿：髋球 / 股 / 胫（倒刺 + 膝甲）/ 跗 + 爪趾", [part_legs]),
}


def build_all(rig) -> None:
    for _, (_, fns) in PARTS_META.items():
        for fn in fns:
            fn(rig)
