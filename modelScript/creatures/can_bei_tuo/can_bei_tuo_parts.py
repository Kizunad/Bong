"""残背驮兽（can_bei_tuo）几何部件库 —— Round 3/3。

和 gen_can_bei_tuo.py 分工：本文件只描述「有哪些骨、每块几何在哪」，渲染 / 门禁 / 接触表
都在生成器里。每个 part_*() 只往传入的 rig 里加几何，可以单独调用做逐件预览。

Round 2 → Round 3 的主要变化（调度意见逐条对应）：

1. 躯干不再是方盒：胸 / 桶腹 / 臀三个椭球，用 `ellipsoid()` 沿 Y 切片，每片是「十字形」
   两块 cube（切掉四角的圆角矩形），从前视、侧视、俯视看都是逐级收边的弧面。
   侧腹的皮肤褶皱按椭球表面逐段贴合（`side_fold()`），不再是悬空的横条。
2. 噬灵藓分三层：① 罩住背脊的外壳（三个比躯干大一圈的椭球，只取上半）；② 压在外壳上
   高低不一的藓垫；③ 藓垫顶的三级叠瘤。壳沿下缘垂下 40 余缕长短不一的须，点缀白花。
   外壳 / 藓垫 / 须分挂在 7 根藓骨上，走路时各摆各的。
3. 头：整体放大，吻部宽扁，鼻孔外有鼻翼；两侧各一支 6 段粗角，先向外、再向前、尖端微收；
   颈下与下巴挂垂藓。
4. 腿：粗膝（比小腿每侧宽 0.9px），蹄分两瓣，前掌带前突蹄尖、后肢带飞节距。
"""

from __future__ import annotations

import math
import random

Vec = tuple[float, float, float]
Ellipsoid = tuple[Vec, Vec]  # (中心, 三轴半径)

# ──────────────────────────────────────────────────────────────── 材质
# 13 种材质。门禁只认 8 种颜色（rigkit 把材质序号 % 8 写进 element.color），
# 所以另有 FAMILY 把它们归到 8 个「族」，生成器里的 CanBeiTuoRig 按族写 color。
MATS: dict[str, tuple[int, int, int]] = {
    "hide_base": (96, 88, 79),
    "hide_dark": (72, 66, 60),
    "hide_belly": (118, 108, 96),
    "skin_fold": (52, 47, 43),
    "moss_layer1": (68, 92, 62),
    "moss_layer2": (96, 124, 82),
    "moss_crust": (132, 154, 108),
    "moss_flower": (240, 242, 232),
    "horn_bone": (118, 106, 92),
    "snout_horn": (96, 86, 76),
    "hoof_horn": (44, 40, 38),
    "eye": (30, 28, 27),
    "socket": (22, 20, 19),
}

FAMILY: dict[str, str] = {
    "hide_base": "hide",
    "hide_dark": "hide",
    "hide_belly": "hide",
    "skin_fold": "fold",
    "moss_layer1": "moss",
    "moss_layer2": "moss",
    "moss_crust": "moss",
    "moss_flower": "flower",
    "horn_bone": "horn",
    "snout_horn": "horn",
    "hoof_horn": "hoof",
    "eye": "eye",
    "socket": "socket",
}
FAMILY_ORDER = ("hide", "fold", "moss", "flower", "horn", "hoof", "eye", "socket")

# ──────────────────────────────────────────────────────────────── 骨骼
# (name, parent, pivot, 说明)。共 41 根：脊 3 + 颈 2 + 头 / 下颌 / 双耳 / 双角 + 尾 4
# + 四腿各 4 段 + 颈垂藓 / 颏藓 + 背上 7 根藓骨。
BONES_DEF: list[tuple[str, str | None, Vec, str]] = [
    ("root", None, (0.0, 0.0, 0.0), "世界根节点"),
    ("hips", "root", (0.0, 12.0, 8.5), "骨盆（脊第 3 节）"),
    ("lumbar", "hips", (0.0, 12.5, 2.0), "腰腹（脊第 2 节）"),
    ("thorax", "lumbar", (0.0, 13.0, -6.0), "胸廓与肩峰（脊第 1 节）"),
    ("neck_base", "thorax", (0.0, 12.0, -11.0), "颈根"),
    ("neck_top", "neck_base", (0.0, 11.0, -15.5), "颈上段"),
    ("neck_moss_dewlap", "neck_base", (0.0, 5.5, -13.0), "颈下垂藓"),
    ("head", "neck_top", (0.0, 10.5, -17.5), "大头"),
    ("jaw", "head", (0.0, 4.2, -25.0), "下颌"),
    ("moss_beard", "jaw", (0.0, 2.2, -29.0), "颏下垂藓"),
    ("ear_l", "head", (-6.2, 12.5, -17.0), "左耳"),
    ("ear_r", "head", (6.2, 12.5, -17.0), "右耳"),
    ("horn_l", "head", (-6.0, 13.5, -20.0), "左角"),
    ("horn_r", "head", (6.0, 13.5, -20.0), "右角"),
    ("tail_1", "hips", (0.0, 13.0, 15.0), "尾节 1"),
    ("tail_2", "tail_1", (0.0, 9.5, 16.5), "尾节 2"),
    ("tail_3", "tail_2", (0.0, 6.5, 17.0), "尾节 3"),
    ("tail_moss", "tail_3", (0.0, 4.0, 17.0), "尾端藓穗"),
    ("leg_fl_thigh", "thorax", (-6.4, 11.5, -8.2), "前左大腿"),
    ("leg_fl_knee", "leg_fl_thigh", (-6.4, 6.0, -8.2), "前左膝"),
    ("leg_fl_shank", "leg_fl_knee", (-6.4, 3.9, -8.2), "前左小腿"),
    ("foot_fl", "leg_fl_shank", (-6.4, 1.8, -8.2), "前左蹄"),
    ("leg_fr_thigh", "thorax", (6.4, 11.5, -8.2), "前右大腿"),
    ("leg_fr_knee", "leg_fr_thigh", (6.4, 6.0, -8.2), "前右膝"),
    ("leg_fr_shank", "leg_fr_knee", (6.4, 3.9, -8.2), "前右小腿"),
    ("foot_fr", "leg_fr_shank", (6.4, 1.8, -8.2), "前右蹄"),
    ("leg_bl_thigh", "hips", (-6.4, 12.0, 7.6), "后左大腿"),
    ("leg_bl_hock", "leg_bl_thigh", (-6.4, 6.0, 7.6), "后左飞节"),
    ("leg_bl_shank", "leg_bl_hock", (-6.4, 3.9, 7.6), "后左跖"),
    ("foot_bl", "leg_bl_shank", (-6.4, 1.8, 7.6), "后左蹄"),
    ("leg_br_thigh", "hips", (6.4, 12.0, 7.6), "后右大腿"),
    ("leg_br_hock", "leg_br_thigh", (6.4, 6.0, 7.6), "后右飞节"),
    ("leg_br_shank", "leg_br_hock", (6.4, 3.9, 7.6), "后右跖"),
    ("foot_br", "leg_br_shank", (6.4, 1.8, 7.6), "后右蹄"),
    ("moss_hump_crest", "thorax", (0.0, 18.0, -4.6), "肩峰藓壳"),
    ("moss_flank_fl", "thorax", (-7.5, 15.0, -6.0), "前左藓须裙"),
    ("moss_flank_fr", "thorax", (7.5, 15.0, -6.0), "前右藓须裙"),
    ("moss_spine_mid", "lumbar", (0.0, 16.0, 2.0), "腰背藓壳"),
    ("moss_flank_bl", "lumbar", (-7.5, 15.0, 4.0), "后左藓须裙"),
    ("moss_flank_br", "lumbar", (7.5, 15.0, 4.0), "后右藓须裙"),
    ("moss_rump_crest", "hips", (0.0, 16.0, 9.0), "臀背藓壳"),
]

# ──────────────────────────────────────────────────────────────── 椭球工具
# 轴向约定与流水线一致：地面 y=0，兽头朝 -Z。1 单位 = 1px = 1/16 格。
_GAP = 0.04  # 相邻切片之间留的缝，避免同面 z-fighting


def _half_x(e: Ellipsoid, y: float, z: float) -> float:
    """椭球在 (y, z) 处的 x 半宽；点在球外返回 0。"""
    (_, cy, cz), (rx, ry, rz) = e
    k = 1.0 - ((y - cy) / ry) ** 2 - ((z - cz) / rz) ** 2
    return rx * math.sqrt(k) if k > 0 else 0.0


def _top_y(e: Ellipsoid, x: float, z: float) -> float | None:
    """椭球在 (x, z) 处的上表面 y；点在球外返回 None。"""
    (cx, cy, cz), (rx, ry, rz) = e
    k = 1.0 - ((x - cx) / rx) ** 2 - ((z - cz) / rz) ** 2
    return cy + ry * math.sqrt(k) if k > 0 else None


def ellipsoid(
    rig,
    bone: str,
    name: str,
    e: Ellipsoid,
    mat: str,
    *,
    step: float = 1.0,
    y_lo: float | None = None,
    y_hi: float | None = None,
    mat_at=None,
    chamfer: float = 0.22,
) -> None:
    """用沿 Y 的切片逼近椭球。体素只有直角，弧面只能靠逐级收边。

    每片由两块 cube 十字叠成（一块沿 X 窄 / 沿 Z 满长，一块沿 X 满宽 / 沿 Z 缩短），
    等于把矩形的四角切掉 `chamfer`，俯视才是圆角而不是方盒。
    mat_at(y_center) 可按高度换材质（腹浅背深）。
    """
    (cx, cy, cz), (rx, ry, rz) = e
    lo = cy - ry if y_lo is None else y_lo
    hi = cy + ry if y_hi is None else y_hi
    n = 0
    y0 = lo
    while y0 < hi - 0.45:  # 顶端不足 0.45 的残片不要，否则切出渲不出一像素的薄片
        y1 = min(y0 + step, hi)
        yc = (y0 + y1) / 2
        t = (yc - cy) / ry
        if abs(t) < 1.0:
            s = math.sqrt(1.0 - t * t)
            hx, hz = round(rx * s, 1), round(rz * s, 1)
            if hx >= 0.8 and hz >= 0.8:
                m = mat_at(yc) if mat_at else mat
                k = round(max(0.7, chamfer * min(hx, hz)), 1)
                a0, a1 = y0 + _GAP / 2, y1 - _GAP / 2
                rig.cube(bone, f"{name}_{n:02d}a", (cx - hx + k, a0, cz - hz), (cx + hx - k, a1, cz + hz), mat=m)
                rig.cube(
                    bone,
                    f"{name}_{n:02d}b",
                    (cx - hx, a0 + 0.03, cz - hz + k),
                    (cx + hx, a1 - 0.03, cz + hz - k),
                    mat=m,
                )
                n += 1
        y0 = y1


def side_fold(rig, bone: str, name: str, e: Ellipsoid, y: float, z0: float, z1: float, *, th: float = 0.5) -> None:
    """沿椭球侧面贴一道皮肤褶皱：每 2.4px 一段，各段按所在位置的表面宽度外凸 0.5px。"""
    z, i = z0, 0
    while z < z1 - 0.4:
        ze = min(z + 2.4, z1)
        w = _half_x(e, y, (z + ze) / 2)
        if w > 3.0:
            for s, tag in ((-1, "l"), (1, "r")):
                rig.cube(
                    bone,
                    f"{name}{i}_{tag}",  # 门禁按 _l/_r 结尾配对镜像
                    (s * (w - 0.9), y - th / 2, z + 0.1),
                    (s * (w + 0.5), y + th / 2, ze - 0.1),
                    mat="skin_fold",
                )
        z, i = ze, i + 1


# ──────────────────────────────────────────────────────────────── 躯干几何常量
CHEST: Ellipsoid = ((0.0, 11.6, -6.8), (8.2, 6.8, 6.2))
BARREL: Ellipsoid = ((0.0, 11.0, 2.0), (8.6, 6.4, 12.0))
RUMP: Ellipsoid = ((0.0, 11.6, 9.0), (7.8, 6.0, 6.6))
HUMP: Ellipsoid = ((0.0, 16.0, -4.6), (6.4, 7.8, 7.2))
NECK_BASE: Ellipsoid = ((0.0, 11.2, -13.0), (5.8, 6.0, 4.2))
NECK_TOP: Ellipsoid = ((0.0, 10.2, -17.4), (5.6, 5.4, 3.6))

# 藓壳：比躯干大一圈、只取上半（y_lo 以上）。(骨, 椭球, y_lo)
MOSS_SHELLS: list[tuple[str, Ellipsoid, float]] = [
    ("moss_hump_crest", ((0.0, 16.2, -4.6), (7.6, 8.4, 8.6)), 14.6),
    ("moss_spine_mid", ((0.0, 11.4, 2.0), (9.3, 6.5, 12.6)), 14.0),
    ("moss_rump_crest", ((0.0, 11.8, 9.0), (8.5, 6.2, 7.2)), 14.2),
]
# 藓垫（第二层）：(骨, x, z, 半宽 x, 半长 z, 半高 y, 材质)，压在壳上高低错落
MOSS_PADS: list[tuple[str, float, float, float, float, float, str]] = [
    ("moss_hump_crest", -2.0, -7.2, 3.8, 3.4, 2.0, "moss_layer2"),
    ("moss_hump_crest", 2.6, -3.4, 3.4, 3.8, 2.4, "moss_layer2"),
    ("moss_hump_crest", -1.0, -0.4, 2.8, 2.6, 1.8, "moss_layer1"),
    ("moss_spine_mid", -3.4, 3.0, 3.8, 3.2, 1.7, "moss_layer2"),
    ("moss_spine_mid", 3.0, 6.2, 3.6, 3.4, 1.6, "moss_layer1"),
    ("moss_rump_crest", -2.2, 10.8, 3.2, 3.0, 1.5, "moss_layer2"),
    ("moss_rump_crest", 2.8, 12.6, 3.0, 2.6, 1.4, "moss_layer1"),
]
# 藓瘤（第三层）：(骨, x, z, 底宽)，三级叠起
MOSS_CLUMPS: list[tuple[str, float, float, float]] = [
    ("moss_hump_crest", 0.0, -4.8, 3.0),
    ("moss_hump_crest", -1.8, -7.0, 2.2),
    ("moss_hump_crest", 2.4, -3.6, 2.4),
    ("moss_hump_crest", -0.8, -0.5, 1.8),
    ("moss_spine_mid", -3.2, 3.2, 2.4),
    ("moss_spine_mid", 3.2, 6.0, 2.0),
    ("moss_spine_mid", 0.2, 4.2, 1.8),
    ("moss_rump_crest", -2.0, 10.6, 2.0),
    ("moss_rump_crest", 3.0, 12.5, 1.8),
    ("moss_rump_crest", 0.2, 13.0, 1.6),
]
# 白花（x, z）：落在壳 / 垫 / 瘤旁边
MOSS_FLOWERS: list[tuple[float, float]] = [
    (-3.6, -9.4), (3.8, -8.2), (-4.6, -4.2), (4.9, -0.8), (0.9, -6.6), (-3.0, -1.6),
    (3.4, 1.6), (-4.4, 5.6), (1.4, 2.6), (4.6, 8.8), (-3.8, 8.0), (0.0, 8.6),
    (-1.0, 13.4), (4.2, 11.4), (-4.8, 12.0),
]


def _shell_surface(x: float, z: float) -> float:
    """藓壳 + 藓垫在 (x, z) 的最高表面 y。"""
    best = 0.0
    for _, e, y_lo in MOSS_SHELLS:
        y = _top_y(e, x, z)
        if y is not None and y > y_lo:
            best = max(best, y)
    for _, px, pz, rx, rz, ry, _m in MOSS_PADS:
        base = _shell_surface_shell_only(px, pz)
        y = _top_y(((px, base - 0.35 * ry, pz), (rx, ry, rz)), x, z)
        if y is not None:
            best = max(best, y)
    return best


def _shell_surface_shell_only(x: float, z: float) -> float:
    best = 0.0
    for _, e, y_lo in MOSS_SHELLS:
        y = _top_y(e, x, z)
        if y is not None and y > y_lo:
            best = max(best, y)
    return best


def _shell_half_x(y: float, z: float) -> float:
    return max(_half_x(e, y, z) for _, e, _lo in MOSS_SHELLS)


def _moss_bone_at(z: float, side: int) -> str:
    """按前后位置挑藓须挂哪根骨：前段挂前裙，中段挂后裙，臀后挂臀骨。"""
    tag = "l" if side < 0 else "r"
    if z < -1.5:
        return f"moss_flank_f{tag}"
    if z < 8.0:
        return f"moss_flank_b{tag}"
    return "moss_rump_crest"


# ──────────────────────────────────────────────────────────────── 小件
def _strand(rig, bone: str, name: str, cx: float, cz: float, y_top: float, length: float,
            fx: float, fz: float, mat_a: str, mat_b: str) -> None:
    """一缕垂须：粗上段 + 细下段，下段比上段缩小 35%。"""
    cut = y_top - 0.55 * length
    rig.cube(bone, f"{name}_a", (cx - fx, cut, cz - fz), (cx + fx, y_top, cz + fz), mat=mat_a)
    rig.cube(bone, f"{name}_b", (cx - fx * 0.65, y_top - length, cz - fz * 0.65),
             (cx + fx * 0.65, cut - 0.03, cz + fz * 0.65), mat=mat_b)


def _flower(rig, bone: str, name: str, x: float, y: float, z: float) -> None:
    """白色小花：两条交叉的薄片，读作四瓣。"""
    rig.cube(bone, f"{name}_p1", (x - 0.7, y, z - 0.25), (x + 0.7, y + 0.45, z + 0.25), mat="moss_flower")
    rig.cube(bone, f"{name}_p2", (x - 0.25, y + 0.03, z - 0.7), (x + 0.25, y + 0.48, z + 0.7), mat="moss_flower")


def _clump(rig, bone: str, name: str, x: float, y: float, z: float, w: float, rng: random.Random) -> None:
    """三级叠瘤：每级缩小、随机偏一点，轮廓参差。"""
    cx, cz, h = x, z, 0.9
    yy = y
    for i, k in enumerate((1.0, 0.7, 0.42)):
        half = w * k / 2
        rig.cube(bone, f"{name}_{i}", (cx - half, yy, cz - half), (cx + half, yy + h, cz + half),
                 mat="moss_crust" if i < 2 else "moss_layer2")
        yy += h + 0.03
        cx += rng.uniform(-0.3, 0.3)
        cz += rng.uniform(-0.3, 0.3)
        h *= 0.9


def _mirror_x(a: float, b: float, sx: int) -> tuple[float, float]:
    return (sx * a, sx * b)


# ──────────────────────────────────────────────────────────────── 部件：躯干
def part_body(rig) -> None:
    """胸 / 桶腹 / 臀三个椭球 + 肩峰 + 侧腹皮褶。"""

    def belly_up(y: float) -> str:
        if y < 8.0:
            return "hide_belly"
        return "hide_dark" if y > 14.5 else "hide_base"

    ellipsoid(rig, "thorax", "chest", CHEST, "hide_base", step=1.0, mat_at=belly_up)
    ellipsoid(rig, "lumbar", "barrel", BARREL, "hide_base", step=1.0, mat_at=belly_up)
    ellipsoid(rig, "hips", "rump", RUMP, "hide_base", step=1.0, mat_at=belly_up)
    # 肩峰：只取 y>13 的上段，其余被胸腹吃掉；外面再罩藓壳
    ellipsoid(rig, "thorax", "hump", HUMP, "hide_dark", step=1.5, y_lo=13.0, y_hi=20.5)

    # 胸骨前凸 + 胸前一道垂褶
    rig.cube("thorax", "chest_prow", (-3.4, 6.6, -13.9), (3.4, 12.0, -13.1), mat="hide_dark")
    rig.cube("thorax", "chest_prow_pad", (-2.4, 7.4, -14.35), (2.4, 11.0, -13.93), mat="hide_belly")

    # 侧腹：桶腹四道横褶 + 胸两道 + 臀两道（各道长短、高度错开）
    for i, (y, z0, z1) in enumerate([(7.1, -6.0, 6.0), (9.1, -8.0, 8.5), (11.1, -9.0, 9.0), (13.1, -7.0, 7.5)]):
        side_fold(rig, "lumbar", f"fold_barrel{i}", BARREL, y, z0, z1, th=0.5 + 0.05 * (i % 2))
    for i, (y, z0, z1) in enumerate([(9.3, -11.0, -3.6), (11.3, -10.0, -4.4)]):
        side_fold(rig, "thorax", f"fold_chest{i}", CHEST, y, z0, z1)
    for i, (y, z0, z1) in enumerate([(8.1, 7.0, 13.4), (10.1, 8.0, 13.0)]):
        side_fold(rig, "hips", f"fold_rump{i}", RUMP, y, z0, z1)

    # 臀尾根的隆肉
    rig.cube("hips", "tail_base_pad", (-2.2, 10.2, 15.4), (2.2, 14.0, 16.4), mat="hide_dark")


# ──────────────────────────────────────────────────────────────── 部件：颈
def part_neck(rig) -> None:
    """粗短垂颈两节 + 侧面皱褶 + 喉下垂藓。"""
    ellipsoid(rig, "neck_base", "neck_b", NECK_BASE, "hide_base", step=1.0,
              mat_at=lambda y: "hide_belly" if y < 7.2 else "hide_base")
    ellipsoid(rig, "neck_top", "neck_t", NECK_TOP, "hide_base", step=1.0,
              mat_at=lambda y: "hide_belly" if y < 6.4 else "hide_base")
    side_fold(rig, "neck_base", "fold_neck_b", NECK_BASE, 11.2, -16.4, -9.6)
    side_fold(rig, "neck_top", "fold_neck_t", NECK_TOP, 9.2, -20.0, -15.0)

    # 喉下垂藓：一块托底的藓垫 + 七缕长短不一的须 + 一朵花
    rig.cube("neck_moss_dewlap", "dewlap_base", (-3.4, 4.9, -16.6), (3.4, 5.9, -10.0), mat="moss_layer1")
    rng = random.Random(41)
    for i, (x, z, ln) in enumerate([(-2.8, -11.2, 3.2), (-1.4, -13.4, 4.6), (0.0, -11.4, 3.6),
                                    (1.5, -14.6, 4.8), (2.8, -12.2, 3.0), (-0.2, -15.6, 2.6),
                                    (-2.6, -15.2, 2.2)]):
        _strand(rig, "neck_moss_dewlap", f"dewlap_s{i}", x, z, 5.0, ln, 0.55, 0.55,
                "moss_layer2" if i % 2 else "moss_layer1", "moss_crust" if rng.random() < 0.5 else "moss_layer2")
    _flower(rig, "neck_moss_dewlap", "dewlap_flower", 0.4, 5.85, -12.6)


# ──────────────────────────────────────────────────────────────── 部件：头
def part_head(rig) -> None:
    """大而低垂的头：脑颅椭球 + 宽扁吻 + 鼻孔 + 深眼窝 + 粗角 + 垂耳 + 下颌 + 颏藓。"""
    cranium: Ellipsoid = ((0.0, 10.6, -21.2), (6.4, 5.6, 5.8))
    ellipsoid(rig, "head", "cranium", cranium, "hide_base", step=0.8,
              mat_at=lambda y: "hide_dark" if y > 13.5 else "hide_base")
    # 额板 + 头顶一小丛藓
    rig.cube("head", "forehead", (-4.6, 14.9, -26.0), (4.6, 16.2, -18.4), mat="hide_dark")
    rig.cube("head", "crown_moss", (-2.6, 16.23, -22.6), (2.6, 17.0, -18.8), mat="moss_layer1")
    rig.cube("head", "crown_moss_top", (-1.4, 17.03, -21.6), (1.4, 17.6, -19.4), mat="moss_layer2")

    # 宽扁吻：椭球 + 前端鼻垫（鼻垫往前凸，才有「吻」的体积）
    snout: Ellipsoid = ((0.0, 7.6, -29.5), (5.4, 3.6, 4.8))
    ellipsoid(rig, "head", "snout", snout, "hide_dark", step=0.8)
    rig.cube("head", "snout_pad", (-4.4, 4.6, -35.1), (4.4, 10.0, -34.2), mat="snout_horn")
    rig.cube("head", "snout_pad_bevel", (-3.6, 5.2, -35.55), (3.6, 9.4, -35.13), mat="snout_horn")
    # 鼻梁隆起 + 吻上两道横褶
    rig.cube("head", "nose_ridge", (-2.0, 10.9, -32.0), (2.0, 11.9, -25.0), mat="snout_horn")
    rig.cube("head", "nose_ridge_top", (-1.2, 11.93, -29.5), (1.2, 12.5, -26.0), mat="snout_horn")
    rig.cube("head", "snout_wrinkle0", (-1.9, 11.88, -31.6), (1.9, 12.3, -30.9), mat="skin_fold")
    rig.cube("head", "snout_wrinkle1", (-1.9, 11.88, -29.0), (1.9, 12.3, -28.3), mat="skin_fold")
    # 鼻孔：深凹的孔 + 上缘鼻翼 + 两侧外扩的鼻翼块
    for s in (-1, 1):
        x0, x1 = _mirror_x(1.3, 2.8, s)
        rig.cube("head", f"nostril_{'l' if s < 0 else 'r'}", (min(x0, x1), 6.8, -35.75), (max(x0, x1), 8.1, -35.5), mat="socket")
        x0, x1 = _mirror_x(0.8, 3.5, s)
        rig.cube("head", f"nostril_brow_{'l' if s < 0 else 'r'}", (min(x0, x1), 8.4, -35.7), (max(x0, x1), 9.1, -35.15), mat="snout_horn")
        x0, x1 = _mirror_x(4.45, 5.7, s)
        rig.cube("head", f"nostril_wing_{'l' if s < 0 else 'r'}", (min(x0, x1), 6.4, -34.9), (max(x0, x1), 9.2, -31.4), mat="hide_base")
    rig.cube("head", "mouth_line", (-3.8, 4.7, -35.32), (3.8, 5.05, -35.06), mat="socket")

    # 眼：深眼窝 + 浑浊眼珠 + 粗眉骨 + 眼袋；颊部厚肌
    for s, t in ((-1, "l"), (1, "r")):
        xa, xb = sorted(_mirror_x(6.15, 6.55, s))
        rig.cube("head", f"eye_socket_{t}", (xa, 10.0, -23.4), (xb, 11.9, -21.4), mat="socket")
        xa, xb = sorted(_mirror_x(6.5, 6.7, s))
        rig.cube("head", f"eye_{t}", (xa, 10.5, -22.8), (xb, 11.4, -22.0), mat="eye")
        xa, xb = sorted(_mirror_x(5.9, 7.0, s))
        rig.cube("head", f"brow_{t}", (xa, 12.0, -24.2), (xb, 13.3, -20.4), mat="hide_dark")
        xa, xb = sorted(_mirror_x(6.2, 6.7, s))
        rig.cube("head", f"eye_bag_{t}", (xa, 9.1, -23.2), (xb, 9.95, -21.4), mat="skin_fold")
        xa, xb = sorted(_mirror_x(5.2, 6.3, s))
        rig.cube("head", f"cheek_{t}", (xa, 6.0, -26.0), (xb, 9.0, -19.6), mat="hide_dark")

    # 角：外→前的六段，每段变细；两段带环脊。尖端回收一点
    for s, t in ((-1, "l"), (1, "r")):
        bone = f"horn_{t}"
        segs = [  # (x0, x1, y0, y1, z0, z1, 材质)
            (6.2, 8.8, 12.2, 15.4, -22.2, -18.4, "horn_bone"),
            (8.6, 11.4, 12.8, 15.6, -24.2, -20.6, "horn_bone"),
            (9.6, 12.0, 13.2, 15.7, -27.0, -23.8, "horn_bone"),
            (9.4, 11.4, 14.0, 15.9, -29.6, -26.8, "horn_bone"),
            (8.6, 10.4, 14.6, 16.0, -31.8, -29.4, "snout_horn"),
        ]
        for i, (x0, x1, y0, y1, z0, z1, m) in enumerate(segs):
            xa, xb = sorted(_mirror_x(x0, x1, s))
            rig.cube(bone, f"horn{i}_{t}", (xa, y0, z0), (xb, y1, z1), mat=m)
        for i, (x0, x1, y0, y1, z) in enumerate([(8.4, 11.6, 12.6, 15.8, -21.9), (9.4, 12.2, 13.0, 15.9, -25.1)]):
            xa, xb = sorted(_mirror_x(x0, x1, s))
            rig.cube(bone, f"horn_ring{i}_{t}", (xa, y0, z - 0.25), (xb, y1, z + 0.25), mat="snout_horn")

    # 垂耳：三段向外下垂
    for s, t in ((-1, "l"), (1, "r")):
        bone = f"ear_{t}"
        for i, (x0, x1, y0, y1, z0, z1, m) in enumerate([
            (6.2, 8.0, 11.0, 13.4, -18.2, -15.6, "hide_dark"),
            (7.8, 9.6, 9.2, 11.6, -18.0, -15.8, "hide_base"),
            (8.8, 10.4, 7.6, 9.6, -17.8, -16.2, "hide_dark"),
        ]):
            xa, xb = sorted(_mirror_x(x0, x1, s))
            rig.cube(bone, f"ear{i}_{t}", (xa, y0, z0), (xb, y1, z1), mat=m)

    # 下颌：宽实，颏前突；底下一道浅色
    rig.cube("jaw", "jaw_body", (-4.2, 2.2, -34.0), (4.2, 4.1, -24.0), mat="hide_base")
    rig.cube("jaw", "jaw_chin", (-3.8, 2.4, -35.0), (3.8, 3.9, -34.03), mat="hide_dark")
    rig.cube("jaw", "jaw_under", (-3.0, 1.9, -31.0), (3.0, 2.17, -25.0), mat="hide_belly")
    for s, t in ((-1, "l"), (1, "r")):
        xa, xb = sorted(_mirror_x(4.23, 5.4, s))
        rig.cube("jaw", f"jaw_cheek_{t}", (xa, 2.6, -29.6), (xb, 4.6, -24.4), mat="hide_dark")

    # 颏下垂藓：五缕（头低，须短，免得拖地）
    for i, (x, z, ln) in enumerate([(-2.8, -29.0, 1.3), (-1.2, -31.5, 1.7), (0.4, -27.6, 1.5),
                                    (1.9, -30.5, 1.6), (3.0, -27.5, 1.2)]):
        _strand(rig, "moss_beard", f"beard_s{i}", x, z, 2.05, ln, 0.5, 0.5,
                "moss_layer1" if i % 2 else "moss_layer2", "moss_crust")
    _flower(rig, "moss_beard", "beard_flower", 0.4, 2.0, -26.4)


# ──────────────────────────────────────────────────────────────── 部件：藓
def part_moss(rig) -> None:
    """三层噬灵藓：壳 → 垫 → 瘤，沿壳缘垂须，点缀白花。"""
    rng = random.Random(20261013)

    # 第一层：罩住背脊的壳，低处深、高处浅；顶部一层老干藓
    def shell_mat(y: float) -> str:
        if y < 16.5:
            return "moss_layer1"
        return "moss_crust" if y > 23.0 else "moss_layer2"

    for bone, e, y_lo in MOSS_SHELLS:
        ellipsoid(rig, bone, f"shell_{bone[5:]}", e, "moss_layer2", step=1.0, y_lo=y_lo, mat_at=shell_mat, chamfer=0.3)

    # 第二层：压在壳上的藓垫，每块的底沉进壳里 35%
    for i, (bone, x, z, rx, rz, ry, mat) in enumerate(MOSS_PADS):
        base = _shell_surface_shell_only(x, z)
        e: Ellipsoid = ((x, base - 0.35 * ry, z), (rx, ry, rz))
        ellipsoid(rig, bone, f"pad{i}", e, mat, step=0.7, y_lo=base - 0.8, chamfer=0.3)

    # 第三层：垫顶的叠瘤
    for i, (bone, x, z, w) in enumerate(MOSS_CLUMPS):
        _clump(rig, bone, f"clump{i}", x, _shell_surface(x, z) - 0.2, z, w, rng)

    # 白花
    for i, (x, z) in enumerate(MOSS_FLOWERS):
        bone = "moss_hump_crest" if z < -1.5 else ("moss_spine_mid" if z < 8.0 else "moss_rump_crest")
        _flower(rig, bone, f"flower{i}", x, _shell_surface(x, z) - 0.12, z)

    # 壳缘垂须：两侧沿 z 隔 1.5~2.6px 一缕，长度 2~7px，偶尔一缕特别长
    y_rim = 14.6
    for side, tag in ((-1, "l"), (1, "r")):
        z, i = -11.0, 0
        while z < 14.0:
            xs = _shell_half_x(y_rim, z)
            if xs > 4.0:
                ln = rng.uniform(2.0, 7.0) + (2.0 if rng.random() < 0.12 else 0.0)
                bone = _moss_bone_at(z, side)
                _strand(rig, bone, f"strand_{tag}{i}", side * (xs - 0.1), z, y_rim + rng.uniform(-0.15, 0.15), ln,
                        0.75, 0.6, rng.choice(("moss_layer1", "moss_layer2")),
                        rng.choice(("moss_layer2", "moss_crust")))
                if rng.random() < 0.3:
                    _flower(rig, bone, f"strand_flower_{tag}{i}", side * (xs + 0.55), y_rim - ln * 0.4, z)
            z += rng.uniform(1.2, 2.1)
            i += 1

    # 前后两面也垂须：胸前（挂前裙骨）与臀后（挂臀骨）
    for i, x in enumerate([-5.6, -3.4, -1.0, 1.2, 3.6, 5.8]):
        ln = rng.uniform(2.5, 6.0)
        zf = -13.2 + 0.0
        _strand(rig, "moss_flank_fl" if x < 0 else "moss_flank_fr", f"strand_front{i}", x, zf + 0.5,
                15.0 + rng.uniform(-0.2, 0.2), ln, 0.65, 0.75, "moss_layer1", "moss_layer2")
        _strand(rig, "moss_rump_crest", f"strand_rear{i}", x * 0.95, 15.6 + 0.2,
                14.8 + rng.uniform(-0.2, 0.2), rng.uniform(2.5, 6.0), 0.65, 0.75, "moss_layer2", "moss_crust")


# ──────────────────────────────────────────────────────────────── 部件：腿
def _leg(rig, sx: int, front: bool) -> None:
    """一条腿：大腿（上粗下收）→ 粗膝/飞节 → 小腿 → 系 → 双瓣蹄。"""
    p = "f" if front else "b"
    t = "l" if sx < 0 else "r"
    thigh = f"leg_{p}{t}_thigh"
    mid = f"leg_{p}{t}_{'knee' if front else 'hock'}"
    shank = f"leg_{p}{t}_shank"
    foot = f"foot_{p}{t}"
    tag = f"{p}{t}"

    def box(bone: str, name: str, x0: float, x1: float, y0: float, y1: float, z0: float, z1: float, mat: str) -> None:
        a, b = sorted(_mirror_x(x0, x1, sx))
        rig.cube(bone, f"{name}_{p}_{t}", (a, y0, z0), (b, y1, z1), mat=mat)  # 以 _l/_r 结尾才会被门禁配对

    zt = (-11.0, -5.4) if front else (4.4, 10.8)  # 大腿 z
    zk = (-11.2, -5.0) if front else (4.6, 11.0)  # 膝 / 飞节 z
    zs = (-10.0, -6.2) if front else (5.8, 9.4)  # 小腿 z
    zh = (-10.6, -5.6) if front else (5.0, 10.0)  # 蹄 z
    y_up = 11.4 if front else 12.0

    # 大腿：上段粗、下段收一点，外侧一块肌肉 + 两道皮褶 + 垂藓
    box(thigh, "thigh_up", 4.0, 9.0, 8.6, y_up, zt[0], zt[1], "hide_dark")
    box(thigh, "thigh_low", 4.3, 8.6, 6.0, 8.55, zt[0] + 0.2, zt[1] - 0.2, "hide_base")
    zm = (zt[0] + zt[1]) / 2
    box(thigh, "thigh_muscle", 9.03, 9.4, 7.2, 10.8, zm - 1.9, zm + 1.9, "hide_base")
    box(thigh, "thigh_fold0", 8.65, 9.05, 8.65, 9.2, zt[0] + 0.6, zt[1] - 0.6, "skin_fold")
    box(thigh, "thigh_fold1", 8.62, 9.05, 6.9, 7.4, zt[0] + 0.9, zt[1] - 0.9, "skin_fold")
    box(thigh, "thigh_moss", 9.43, 9.9, 8.6, 10.8, zm - 1.5, zm + 1.2, "moss_layer1")
    box(thigh, "thigh_moss_drip", 9.46, 9.85, 7.0, 8.57, zm - 0.8, zm + 0.3, "moss_layer2")

    # 粗膝 / 飞节：比小腿每侧宽 0.9px；前膝有膝盖皮，后飞节向后顶出一个距
    box(mid, "joint", 3.9, 8.9, 3.9, 6.0, zk[0], zk[1], "hide_dark")
    box(mid, "joint_band0", 3.8, 9.0, 4.55, 4.95, zk[0] + 0.5, zk[1] - 0.5, "skin_fold")
    if front:
        box(mid, "kneecap", 4.6, 8.2, 4.2, 5.7, -11.9, -11.22, "skin_fold")
        box(mid, "kneecap_boss", 5.4, 7.4, 4.7, 5.3, -12.2, -11.92, "hide_dark")
    else:
        box(mid, "hock_spur", 4.9, 7.9, 4.4, 5.6, 11.02, 11.8, "skin_fold")
        box(mid, "hock_spur_tip", 5.7, 7.1, 4.8, 5.4, 11.82, 12.2, "hide_dark")

    # 小腿 + 系
    box(shank, "shank", 4.7, 8.1, 1.85, 3.88, zs[0], zs[1], "hide_base")
    box(shank, "pastern", 4.5, 8.3, 1.85, 2.7, zs[0] - 0.3, zs[1] + 0.2, "hide_dark")
    box(shank, "dewclaw", 5.8, 7.0, 1.5, 2.3, zs[1] + 0.03, zs[1] + 0.6, "hoof_horn")  # 悬蹄在小腿后侧

    # 双瓣蹄：两瓣之间留 0.6px 的蹄裂；前蹄尖前突，后蹄尖后突
    for k, (x0, x1) in enumerate([(4.35, 6.3), (6.9, 8.85)]):
        box(foot, f"hoof{k}", x0, x1, 0.0, 1.8, zh[0], zh[1], "hoof_horn")
        tip = (zh[0] - 0.6, zh[0] - 0.03) if front else (zh[0] - 0.6, zh[0] - 0.03)
        box(foot, f"hoof_tip{k}", x0 + 0.1, x1 - 0.1, 0.0, 1.1, tip[0], tip[1], "hoof_horn")
        box(foot, f"hoof_crown{k}", x0 - 0.1, x1 + 0.05, 1.82, 2.3, zh[0] + 0.3, zh[1] - 0.2, "skin_fold")


def part_legs(rig) -> None:
    for sx in (-1, 1):
        for front in (True, False):
            _leg(rig, sx, front)


# ──────────────────────────────────────────────────────────────── 部件：尾
def part_tail(rig) -> None:
    """短粗垂尾四节，渐细；尾端一簇蓬松藓穗 + 白花。"""
    rig.cube("tail_1", "tail_root", (-1.7, 10.0, 16.5), (1.7, 14.0, 17.9), mat="hide_dark")
    rig.cube("tail_1", "tail_root_bevel", (-1.3, 9.0, 16.55), (1.3, 10.0, 17.7), mat="hide_base")
    rig.cube("tail_2", "tail_mid", (-1.4, 6.6, 16.9), (1.4, 9.5, 18.1), mat="hide_base")
    rig.cube("tail_2", "tail_mid_moss_l", (-1.95, 7.0, 17.0), (-1.43, 9.0, 18.0), mat="moss_layer1")
    rig.cube("tail_2", "tail_mid_moss_r", (1.43, 7.0, 17.0), (1.95, 9.0, 18.0), mat="moss_layer1")
    rig.cube("tail_3", "tail_end", (-1.1, 4.4, 17.1), (1.1, 6.55, 18.1), mat="hide_dark")
    rig.cube("tail_3", "tail_tuft", (-2.1, 3.6, 16.9), (2.1, 5.4, 18.5), mat="moss_layer2")
    rig.cube("tail_moss", "tail_tassel", (-1.5, 2.0, 17.0), (1.5, 3.57, 18.3), mat="moss_crust")
    rig.cube("tail_moss", "tail_tassel_l", (-2.3, 2.6, 17.2), (-1.53, 3.5, 18.1), mat="moss_layer1")
    rig.cube("tail_moss", "tail_tassel_r", (1.53, 2.6, 17.2), (2.3, 3.5, 18.1), mat="moss_layer1")
    rig.cube("tail_moss", "tail_drip", (-0.6, 1.0, 17.4), (0.6, 1.97, 18.0), mat="moss_layer2")
    _flower(rig, "tail_moss", "tail_flower", 0.0, 3.62, 17.7)


PARTS_META = {
    "body": ("1. 躯干：胸/桶腹/臀椭球 + 肩峰 + 侧腹皮褶", [part_body]),
    "head": ("2. 颈与大头：垂颈 + 宽吻鼻孔 + 前弯粗角 + 垂耳 + 颏/喉垂藓", [part_neck, part_head]),
    "moss": ("3. 噬灵藓：壳 → 垫 → 瘤三层 + 垂须 + 白花（7 根藓骨）", [part_moss]),
    "legs": ("4. 四腿：粗膝/飞节 + 双瓣蹄 + 腿侧垂藓", [part_legs]),
    "tail": ("5. 短尾：四节渐细 + 藓穗", [part_tail]),
}


def build_all(rig) -> None:
    for _, (_, fns) in PARTS_META.items():
        for fn in fns:
            fn(rig)
