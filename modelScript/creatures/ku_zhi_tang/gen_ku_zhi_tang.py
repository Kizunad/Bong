#!/usr/bin/env python3
"""枯枝螳（ku_zhi_tang）Blockbench .bbmodel 程序化生成器 —— Round 3/3 几何终轮（在第三次返工版上补细节）。
Round 3 新增：灰绿地衣斑、翘皮与裂口、口须与唇片、复眼琥珀圈、翅鞘枯叶斑与虫蛀孔。

正典依据：docs/worldview.md §五 死域朽木（伪灵皮材料）。任务卡：末世生物第五批 #18。

外形（对照参考图）：外形与死域朽木一样的伏击螳螂。身体和六条步足都是一根根枯枝：每一段用几十块小方块
沿骨线堆出，粗细逐段变化，表面是参差的树皮（亮 / 暗 / 灰斑 / 锈褐混色）、凸出的枝节和断枝杈，关节处鼓一个
疙瘩；
  · 前胸细长，斜着抬起，顶着一枚倒三角头：脸板逐行收窄到下颚尖，两侧各一个鼓出的复眼（深色 + 一点琥珀），
    头顶一簇冠刺，两根触角各两节、向上再向前弯；
  · 前肢是镰刀：股节从胸前斜向前上举起，胫节折回来贴着股节，内缘各一排倒刺（股节一排短刺，胫节一排长刺），
    末端是弯爪；
  · 中后足四条长腿：基节 → 股节外上平伸到膝 → 胫节斜着长长落地（足尖外撇）→ 三节细跗 + 爪，
    股胫上有向外的小刺与断枝；
  · 腹部五节向后下垂，每节一圈暗环、背上翘皮、侧面小刺，尾端两根尾须；
  · 背上两片枯叶状翅鞘顺着腹背斜斜盖下去：叶形轮廓（中间宽、两头尖）、中脉 + 斜脉、边缘锯齿、翘起的皮。

坐标：1 block = 16px，原点在足尖中间的地面，+y 向上，正面朝 -z（GeckoLib 约定），+x 为螳的右侧
（骨名 *_r）、-x 为左侧（*_l）。高约 38px（约 2.4 格），长约 36px。
左右两侧各自抖动（树皮不该左右一模一样），所以带 _l / _r 的骨都登记为刻意不对称。

用法：
    python3 modelScript/creatures/ku_zhi_tang/gen_ku_zhi_tang.py --self-test   # 门禁差分自证
    python3 modelScript/creatures/ku_zhi_tang/gen_ku_zhi_tang.py --part head    # 单件预览
    python3 modelScript/creatures/ku_zhi_tang/gen_ku_zhi_tang.py                # 全量：模型 + 审阅图
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
import creature_voxel as vox  # noqa: E402
from creature_voxel import ellipsoid_shell, jcube, limb, pick, taper_chain  # noqa: E402

CREATURE_DIR = Path(__file__).resolve().parent
SEED = 16

MATS = {
    "bark":      (118, 98, 74),       # 枯枝主色
    "bark_dk":   (78, 62, 46),        # 皮裂 / 环 / 倒刺
    "bark_lt":   (156, 134, 104),     # 翘起的皮 / 翅鞘亮面
    "bark_grey": (136, 128, 112),     # 风化灰斑
    "bark_rust": (136, 94, 58),       # 参考图里朽木上的锈褐
    "knot":      (60, 46, 36),        # 节疤
    "moss":      (100, 112, 80),      # 朽木上的灰绿苔点
    "eye":       (34, 32, 30),        # 复眼
    "eye_hi":    (206, 180, 120),     # 复眼高光（一点琥珀）
    "claw":      (54, 44, 36),        # 爪尖 / 上颚
    "lichen":    (118, 130, 96),      # 地衣斑（灰绿）
    "lichen_lt": (170, 176, 140),     # 地衣亮边
    "eye_amb":   (150, 92, 40),       # 复眼琥珀圈
}
# 树皮一类的材质（含爪尖：爪长在枝上）互相压着是同一棵枯枝，允许薄重叠；复眼只放行自己的琥珀圈与高光
_SOFT = ("bark", "bark_dk", "bark_lt", "bark_grey", "bark_rust", "knot", "moss", "claw", "lichen", "lichen_lt")
SOFT_OVER = frozenset(
    [frozenset((a, b)) for a in _SOFT for b in _SOFT if a != b]
    + [frozenset((a, b)) for a in ("eye", "eye_hi", "eye_amb", "claw") for b in ("eye", "eye_hi", "eye_amb") if a != b]
)

BARK = (("bark", 0.34), ("bark_lt", 0.18), ("bark_grey", 0.16), ("bark_rust", 0.14), ("bark_dk", 0.14), ("knot", 0.04))
BARK_BUMP = (("bark_lt", 0.3), ("bark_dk", 0.3), ("bark_grey", 0.2), ("bark_rust", 0.2))

HEAD_AT = (0.0, 34.5, -11.0)       # 头骨枢轴（头按局部坐标写，整体平移到这里）

# 躯干中心线：(x, y, z, 半径)
ABD_PATH = ((0.0, 14.2, 3.2, 2.9), (0.0, 13.0, 8.0, 3.1), (0.0, 11.7, 13.0, 3.0), (0.0, 10.3, 18.0, 2.6),
            (0.0, 9.0, 22.8, 2.1), (0.0, 7.9, 27.0, 1.5))
NECK_PATH = ((0.0, 15.2, -1.8, 2.7), (0.0, 20.0, -4.4, 2.3), (0.0, 25.2, -7.2, 1.9), (0.0, 30.4, -9.4, 1.6), (0.0, 33.0, -10.4, 1.4))


def abd_at(i: int) -> tuple[float, float, float]:
    p = ABD_PATH[min(i, len(ABD_PATH) - 1)]
    return (0.0, p[1], p[2])


def sided(name: str, sx: int) -> str:
    return f"{name}_r" if sx > 0 else f"{name}_l"


# (name, parent, pivot[px], 说明)。骨骼先父后子。+x = 右侧（*_r）。
BONES: list[kit.Bone] = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("metathorax", "root", (0.0, 14.2, 2.0), "中后胸（承载中后足与翅鞘）"),
    ("abd_0", "metathorax", abd_at(0), "腹第 1 节"),
    ("abd_1", "abd_0", abd_at(1), "腹第 2 节"),
    ("abd_2", "abd_1", abd_at(2), "腹第 3 节"),
    ("abd_3", "abd_2", abd_at(3), "腹第 4 节"),
    ("abd_4", "abd_3", abd_at(4), "腹第 5 节（尾端）"),
    ("cercus_l", "abd_4", (-0.9, 8.3, 26.5), "左尾须"),
    ("cercus_r", "abd_4", (0.9, 8.3, 26.5), "右尾须"),
    ("wing_l", "metathorax", (-0.8, 15.6, 2.4), "左翅鞘（枯叶状）"),
    ("wing_r", "metathorax", (0.8, 15.6, 2.4), "右翅鞘"),
    ("prothorax_1", "metathorax", (0.0, 15.2, -1.8), "前胸第 1 段（抬起的细颈根）"),
    ("prothorax_2", "prothorax_1", (0.0, 20.0, -4.4), "前胸第 2 段"),
    ("prothorax_3", "prothorax_2", (0.0, 25.2, -7.2), "前胸第 3 段（接头）"),
    ("head", "prothorax_3", HEAD_AT, "三角头：颅 / 分面复眼 / 面板 / 冠刺"),
    ("jaw_l", "head", (-0.8, 30.0, -12.0), "左上颚"),
    ("jaw_r", "head", (0.8, 30.0, -12.0), "右上颚"),
    ("antenna1_l", "head", (-1.4, 37.0, -11.6), "左触角第 1 节"),
    ("antenna1_r", "head", (1.4, 37.0, -11.6), "右触角第 1 节"),
    ("antenna2_l", "antenna1_l", (-3.4, 43.0, -13.0), "左触角第 2 节（前折）"),
    ("antenna2_r", "antenna1_r", (3.4, 43.0, -13.0), "右触角第 2 节"),
    ("foreleg_coxa_l", "prothorax_2", (-2.6, 22.0, -5.0), "左前足基节"),
    ("foreleg_coxa_r", "prothorax_2", (2.6, 22.0, -5.0), "右前足基节"),
    ("foreleg_femur_l", "foreleg_coxa_l", (-3.4, 23.5, -6.0), "左镰臂股节（带一排短刺）"),
    ("foreleg_femur_r", "foreleg_coxa_r", (3.4, 23.5, -6.0), "右镰臂股节"),
    ("foreleg_tibia_l", "foreleg_femur_l", (-3.8, 30.0, -15.0), "左镰臂胫节（折回，带一排长刺）"),
    ("foreleg_tibia_r", "foreleg_femur_r", (3.8, 30.0, -15.0), "右镰臂胫节"),
    ("foreleg_claw_l", "foreleg_tibia_l", (-3.4, 19.5, -10.0), "左镰爪"),
    ("foreleg_claw_r", "foreleg_tibia_r", (3.4, 19.5, -10.0), "右镰爪"),
    ("midleg_femur_l", "metathorax", (-2.6, 14.0, -0.2), "左中足股节（向外上平伸）"),
    ("midleg_femur_r", "metathorax", (2.6, 14.0, -0.2), "右中足股节"),
    ("midleg_tibia_l", "midleg_femur_l", (-9.4, 18.0, -1.6), "左中足胫节（长长斜落）"),
    ("midleg_tibia_r", "midleg_femur_r", (9.4, 18.0, -1.6), "右中足胫节"),
    ("midleg_foot_l", "midleg_tibia_l", (-15.6, 3.2, -4.4), "左中足跗节 + 爪"),
    ("midleg_foot_r", "midleg_tibia_r", (15.6, 3.2, -4.4), "右中足跗节"),
    ("hindleg_femur_l", "metathorax", (-2.6, 14.0, 4.4), "左后足股节"),
    ("hindleg_femur_r", "metathorax", (2.6, 14.0, 4.4), "右后足股节"),
    ("hindleg_tibia_l", "hindleg_femur_l", (-10.4, 18.5, 7.0), "左后足胫节"),
    ("hindleg_tibia_r", "hindleg_femur_r", (10.4, 18.5, 7.0), "右后足胫节"),
    ("hindleg_foot_l", "hindleg_tibia_l", (-16.4, 3.2, 13.4), "左后足跗节 + 爪"),
    ("hindleg_foot_r", "hindleg_tibia_r", (16.4, 3.2, 13.4), "右后足跗节"),
]
# 树皮的抖动是逐块随机的，左右不镜像、中线件也不对称，所以登记为刻意不对称；只有上颚与尾须是严格镜像的一对，留给镜像门禁
ASYM_BONES = frozenset(name for name, *_ in BONES if not name.startswith(("jaw_", "cercus_")))


def ring(b: kit.Builder, bone: str, tag: str, pos, t, rad: float, n: int, mat: str, rng: random.Random) -> None:
    """围着枝干的一圈暗环：n 块围成一圈，比枝身略鼓。"""
    u, v = vox.perpendicular_basis(np.asarray(t, dtype=float))
    for m in range(n):
        ang = 2.0 * math.pi * m / n + rng.uniform(-0.1, 0.1)
        off = min(rad + 0.25, rad * 0.8 + 0.5) * (math.cos(ang) * u + math.sin(ang) * v)
        jcube(b, bone, f"{tag}_{m}", np.asarray(pos) + off, 1.15, mat, rng, jit=0.05)


def twig(b: kit.Builder, bone: str, tag: str, pos, direction, length: float, width: float, rng: random.Random) -> None:
    """断枝杈：从枝身斜伸出一小截，末端收尖。"""
    d = np.asarray(direction, dtype=float)
    d /= np.linalg.norm(d)
    taper_chain(b, bone, tag, pos, np.asarray(pos) + d * length, width, width * 0.45, (("bark_dk", 0.6), ("knot", 0.4)), rng)


def knob(b: kit.Builder, bone: str, tag: str, pos, size: float, rng: random.Random) -> None:
    """关节处的疙瘩：一大一小两块错开。"""
    jcube(b, bone, f"{tag}_a", pos, size, pick(rng, (("knot", 0.5), ("bark_dk", 0.5))), rng)
    jcube(b, bone, f"{tag}_b", np.asarray(pos) + np.array([rng.uniform(-0.3, 0.3), 0.35, rng.uniform(-0.3, 0.3)]),
          size * 0.7, "bark_lt", rng)


# ───────────────────────────────────────────────────────────── 躯干
def part_thorax(b: kit.Builder) -> None:
    """中后胸：粗一些的树干段，两侧脊、背甲片、翘皮；腿根与翅根的疙瘩。"""
    rng = random.Random(SEED + 1)
    limb(b, "metathorax", "meta", ((0.0, 14.0, 4.2, 3.2), (0.0, 14.8, 0.8, 3.3), (0.0, 15.4, -2.0, 2.8)), BARK, rng, bumps=5,
         bump_mats=BARK_BUMP)
    for sx in (1, -1):
        for k, (y, z) in enumerate(((13.4, 3.4), (13.6, 0.0), (13.2, 5.6))):
            knob(b, "metathorax", f"leg_root{sx}{k}", (sx * 3.4, y, z), 1.6, rng)
    for k in range(5):                                         # 背上的鳞甲片
        jcube(b, "metathorax", f"plate{k}", (rng.uniform(-1.4, 1.4), 17.6 - 0.1 * k, 3.4 - 1.5 * k), (2.2, 0.8, 1.6),
              pick(rng, BARK_BUMP), rng)


def part_abdomen(b: kit.Builder) -> None:
    """腹部五节：逐节变细向后下垂，每节一圈暗环、背上一片翘皮、侧面一根小刺；尾端接两根尾须。"""
    rng = random.Random(SEED + 2)
    for i in range(5):
        bone = f"abd_{i}"
        a, c = ABD_PATH[i], ABD_PATH[i + 1]
        seg = limb(b, bone, f"abd{i}", (a, c), BARK, rng, bumps=4, bump_mats=BARK_BUMP)
        t = np.array(c[:3]) - np.array(a[:3])
        t /= np.linalg.norm(t)
        ring(b, bone, f"abd{i}_ring", seg[-1][0], t, seg[-1][2] * 0.95, 9, "bark_dk", rng)
        mid = (np.array(a[:3]) + np.array(c[:3])) / 2
        jcube(b, bone, f"abd{i}_flake", mid + np.array([0.0, a[3] * 0.95, 0.0]), (1.8, 0.7, 2.0), "bark_lt", rng)
        for sx in (1, -1):
            twig(b, bone, f"abd{i}_thorn{sx}", mid + np.array([sx * a[3] * 0.9, -0.5, 0.0]), (sx, -0.3, 0.4), 1.8, 0.8, rng)
    # 尾须：严格左右镜像的一对（b.put "lr"），不加抖动——整只螳里参与镜像门禁的件之一
    for k in range(9):
        x, y, z, w = 0.9 + 0.12 * k, 8.0 - 0.08 * k, 27.4 + 0.5 * k, 1.0 - 0.06 * k
        b.put("cercus", f"cercus_{k}", (x - w / 2, y - w / 2, z - w / 2), (x + w / 2, y + w / 2, z + w / 2),
              "bark_dk" if k % 2 == 0 else "knot", "lr")


def part_neck(b: kit.Builder) -> None:
    """前胸三段：细长斜举的树枝，每段一圈暗环 + 断枝杈 + 翘皮，三段之间鼓疙瘩。"""
    rng = random.Random(SEED + 3)
    for i in range(3):
        bone = f"prothorax_{i + 1}"
        a, c = NECK_PATH[i], NECK_PATH[i + 1]
        seg = limb(b, bone, f"pro{i}", (a, c), BARK, rng, bumps=4, bump_mats=BARK_BUMP)
        t = np.array(c[:3]) - np.array(a[:3])
        t /= np.linalg.norm(t)
        ring(b, bone, f"pro{i}_ring", seg[-1][0], t, seg[-1][2] * 0.95, 8, "bark_dk", rng)
        mid = (np.array(a[:3]) + np.array(c[:3])) / 2
        knob(b, bone, f"pro{i}_knot", mid + np.array([0.0, 0.0, -a[3] * 0.9]), 1.1, rng)
        jcube(b, bone, f"pro{i}_flake", mid + np.array([0.0, a[3] * 0.85, a[3] * 0.5]), (1.6, 1.4, 0.7), "bark_lt", rng)
        side = 1 if i % 2 else -1
        twig(b, bone, f"pro{i}_twig", mid + np.array([side * a[3] * 0.9, 0.0, 0.3]), (side, 0.3, 0.5), 2.2, 0.8, rng)


# ───────────────────────────────────────────────────────────── 头
def part_head(b: kit.Builder) -> None:
    """倒三角头：脸板逐行收窄到下颚尖，两侧鼓出复眼（深色 + 琥珀高光），头顶冠刺，触角根座。"""
    rng = random.Random(SEED + 4)
    hx, hy, hz = HEAD_AT
    with b.offset((hx, hy, hz)):
        rows = 9
        for j in range(rows):                                   # 脸板：每行比上一行窄
            y = 2.9 - 0.85 * j
            half = 4.6 * (1.0 - j / (rows + 0.5)) + 0.5
            for x in np.arange(-half, half + 0.01, 0.95):
                mat = pick(rng, (("bark_lt", 0.3), ("bark", 0.3), ("bark_grey", 0.22), ("bark_rust", 0.12), ("bark_dk", 0.06)))
                jcube(b, "head", f"face_{j}_{x:+.1f}", (float(x), y, -1.0), (1.0, 0.95, 1.5), mat, rng, jit=0.05)
        for j in range(4):                                      # 后脑壳：叠在脸板后面
            y = 2.7 - 0.9 * j
            half = 3.2 - 0.35 * j
            for x in np.arange(-half, half + 0.01, 1.0):
                jcube(b, "head", f"skull_{j}_{x:+.1f}", (float(x), y, 0.6), (1.1, 1.0, 1.4), pick(rng, BARK), rng, jit=0.05)
        for sx in (1, -1):                                      # 复眼：鼓出的体素椭球，深色为主，缀琥珀
            ellipsoid_shell(b, "head", f"eye{sx}", (sx * 4.4, 1.4, -0.6), (1.5, 2.0, 1.5), 0.8,
                            (("eye", 0.85), ("claw", 0.15)), rng)
            for k, (dx, dy, dz) in enumerate(((-0.7, 0.9, -2.0), (-0.1, 1.5, -2.0), (-0.9, 0.2, -1.9))):
                jcube(b, "head", f"eye_hi{sx}{k}", (sx * (4.4 + dx), 1.4 + dy, -0.6 + dz), 0.7, "eye_hi", rng, jit=0.02)
        for k in range(5):                                      # 冠刺：头顶一簇向后斜的短刺
            x = (k - 2) * 1.1
            taper_chain(b, "head", f"crown{k}", (x, 3.4, 0.0), (x * 1.2, 5.4 - 0.4 * abs(k - 2), 1.8), 0.9, 0.4,
                        (("bark_dk", 0.5), ("bark", 0.5)), rng)
        for sx in (1, -1):                                      # 触角根座
            jcube(b, "head", f"ant_base{sx}", (sx * 1.4, 3.4, -0.6), 1.2, "bark_dk", rng)
        jcube(b, "head", "chin_a", (0.0, -5.2, -1.0), (1.5, 1.2, 1.6), "bark_dk", rng)


def part_jaws(b: kit.Builder) -> None:
    """上颚：下颚尖两侧各一根向内弯的暗色颚钩。"""
    hx, hy, hz = HEAD_AT
    with b.offset((hx, hy, hz)):              # 严格左右镜像的一对（b.put "lr"），不加抖动
        for k, (x, y, z, w) in enumerate(((0.95, -5.0, -1.8, 1.1), (0.8, -5.9, -2.1, 0.95), (0.6, -6.7, -2.4, 0.75),
                                          (0.35, -7.3, -2.7, 0.55))):
            b.put("jaw", f"jaw_{k}", (x - w / 2, y - w / 2, z - w / 2), (x + w / 2, y + w / 2, z + w / 2), "claw", "lr")


def part_antennae(b: kit.Builder) -> None:
    """触角：每根两节，第一节从头顶向外上斜出，第二节更细、向前弯，末端一点淡色。"""
    rng = random.Random(SEED + 6)
    for sx in (1, -1):
        limb(b, sided("antenna1", sx), f"ant1{sx}",
             ((sx * 1.4, 37.0, -11.6, 0.45), (sx * 2.6, 40.2, -12.4, 0.4), (sx * 3.4, 43.0, -13.0, 0.35)),
             (("bark_dk", 0.6), ("bark", 0.4)), rng, bumps=0, step=0.45)
        limb(b, sided("antenna2", sx), f"ant2{sx}",
             ((sx * 3.4, 43.0, -13.0, 0.33), (sx * 4.8, 45.0, -16.0, 0.3), (sx * 6.4, 45.4, -20.5, 0.28)),
             (("bark_dk", 0.6), ("bark_lt", 0.4)), rng, bumps=0, step=0.45)


# ───────────────────────────────────────────────────────────── 前肢：镰刀
def part_forelegs(b: kit.Builder) -> None:
    """前肢镰刀：基节 → 股节斜向前上举（内缘一排短刺）→ 胫节折回来贴着股节（内缘一排长刺）→ 弯爪。"""
    rng = random.Random(SEED + 7)
    for sx in (1, -1):
        coxa, femur, tibia, claw = (sided(n, sx) for n in ("foreleg_coxa", "foreleg_femur", "foreleg_tibia", "foreleg_claw"))
        shoulder = (sx * 2.6, 22.0, -5.0)
        knee_a = (sx * 5.0, 24.2, -7.2)
        elbow = (sx * 7.6, 30.6, -15.6)
        wrist = (sx * 5.4, 20.6, -9.6)
        limb(b, coxa, f"coxa{sx}", ((*shoulder, 1.3), (*knee_a, 1.5)), BARK, rng, bumps=3, bump_mats=BARK_BUMP)
        fem = limb(b, femur, f"femur{sx}", ((*knee_a, 1.35), (sx * 6.4, 27.6, -11.4, 1.2), (*elbow, 1.0)), BARK, rng, bumps=4,
                   bump_mats=BARK_BUMP)
        knob(b, femur, f"elbow{sx}", elbow, 1.6, rng)
        for k, (pos, t, rad) in enumerate(fem[1::2]):             # 股节内缘一排短刺：朝体内（-sx）斜下
            taper_chain(b, femur, f"fthorn{sx}{k}", pos + np.array([-sx * rad * 0.8, -0.4, 0.0]),
                        pos + np.array([-sx * (rad + 2.0), -1.5, 0.3]), 0.8, 0.35, (("bark_dk", 0.6), ("knot", 0.4)), rng)
        tib = limb(b, tibia, f"tibia{sx}", ((*elbow, 0.9), (sx * 6.8, 26.2, -13.8, 0.8), (sx * 6.0, 23.0, -11.8, 0.65), (*wrist, 0.5)),
                   BARK, rng, bumps=3, bump_mats=BARK_BUMP)
        for k, (pos, t, rad) in enumerate(tib[1:]):               # 胫节内缘一排长刺：朝体内斜下并微向后
            taper_chain(b, tibia, f"tthorn{sx}{k}", pos + np.array([-sx * rad * 0.8, 0.0, 0.5]),
                        pos + np.array([-sx * (rad + 2.8), -1.6, 1.2]), 0.8, 0.3, (("claw", 0.5), ("bark_dk", 0.5)), rng,
                        curl=(0.0, 0.5, 0.0))
        taper_chain(b, claw, f"claw{sx}", wrist, (sx * 4.6, 18.2, -8.4), 1.0, 0.4, "claw", rng, curl=(0.0, 0.0, 0.8))


# ───────────────────────────────────────────────────────────── 步足
def walking_leg(b: kit.Builder, sx: int, name: str, z0: float, knee: tuple, foot: tuple, rng: random.Random) -> None:
    """一条步足：股节从胸侧向外上平伸到膝（粗），胫节斜着长长落到足尖（细），再三节细跗 + 爪。"""
    femur, tibia, tarsus = sided(f"{name}_femur", sx), sided(f"{name}_tibia", sx), sided(f"{name}_foot", sx)
    root = (sx * 3.0, 14.0, z0)
    k = (sx * knee[0], knee[1], knee[2])
    f = (sx * foot[0], foot[1], foot[2])
    limb(b, femur, f"{name}f{sx}", ((*root, 1.1), (sx * 6.0, 16.2, (z0 + k[2]) / 2, 1.0), (*k, 0.85)), BARK, rng, bumps=3,
         bump_mats=BARK_BUMP)
    knob(b, femur, f"{name}knee{sx}", k, 1.9, rng)
    mid = (k[0] + (f[0] - k[0]) * 0.5, (k[1] + f[1]) / 2 + 1.0, k[2] + (f[2] - k[2]) * 0.5, 0.65)
    leg = limb(b, tibia, f"{name}t{sx}", ((*k, 0.8), mid, (f[0], f[1] + 2.4, f[2], 0.5)), BARK, rng, bumps=2, bump_mats=BARK_BUMP)
    for i, (pos, t, rad) in enumerate(leg[1::3]):                 # 小刺与断枝，朝外（sx）
        twig(b, tibia, f"{name}tw{sx}{i}", pos + np.array([sx * rad * 0.7, 0.0, 0.0]), (sx, 0.5, 0.2), 1.6, 0.7, rng)
    limb(b, tarsus, f"{name}tar{sx}", ((f[0], f[1] + 2.4, f[2], 0.55), (f[0] + sx * 0.8, f[1] + 1.0, f[2] - 0.8, 0.45)),
         (("bark_dk", 0.5), ("bark", 0.5)), rng, bumps=0, step=0.6)
    for d in (-0.7, 0.0, 0.7):                                    # 三个爪尖
        taper_chain(b, tarsus, f"{name}claw{sx}{d:+.1f}", (f[0] + sx * 0.8 + d, f[1] + 1.0, f[2] - 0.8),
                    (f[0] + sx * (1.2 + abs(d) * 0.4) + d * 1.5, f[1] - 0.2, f[2] - 2.4), 0.7, 0.35, "claw", rng)


def part_midlegs(b: kit.Builder) -> None:
    """中足两条：股节平伸，胫节长长落地，足尖外撇。"""
    rng = random.Random(SEED + 8)
    for sx in (1, -1):
        walking_leg(b, sx, "midleg", -0.2, (9.4, 18.0, -1.6), (15.6, 0.8, -4.4), rng)


def part_hindlegs(b: kit.Builder) -> None:
    """后足两条：比中足更向后外撇，膝更高。"""
    rng = random.Random(SEED + 9)
    for sx in (1, -1):
        walking_leg(b, sx, "hindleg", 4.4, (10.4, 18.5, 7.0), (16.4, 0.8, 13.4), rng)


# ───────────────────────────────────────────────────────────── 翅鞘：枯叶
def part_wings(b: kit.Builder) -> None:
    """两片枯叶状翅鞘：叶形轮廓（中间宽两头尖）、中脉 + 斜脉、锯齿边缘、翘起的皮，顺着腹背斜斜盖下去。"""
    rng = random.Random(SEED + 10)
    root = np.array([0.0, 18.0, 2.4])
    tip = np.array([0.0, 9.0, 33.0])
    for sx in (1, -1):
        bone = sided("wing", sx)
        n_s = 30
        for si in range(n_s + 1):
            s = si / n_s
            centre = root + (tip - root) * s + np.array([0.0, 2.2 * math.sin(math.pi * s), 0.0])   # 背上微微拱起
            half = 7.0 * math.sin(math.pi * min(1.0, s ** 0.85)) + 0.5
            for ui in range(int(half / 1.05) + 1):
                u = ui * 1.05
                x = sx * (0.8 + u)
                y = centre[1] - 0.44 * u * u / max(1.0, half)                                   # 外沿向下垂
                zc = centre[2] + 0.18 * u                                                       # 外沿略靠后：斜脉的走向
                on_vein = ui == 0 or abs(((s * 30 + u * 1.3) % 4.0) - 2.0) < 0.5
                edge = u + 1.05 > half
                if on_vein:
                    mat = "knot" if ui == 0 else "bark_dk"
                else:
                    mat = pick(rng, (("bark_lt", 0.3), ("bark", 0.26), ("bark_grey", 0.2), ("bark_rust", 0.2), ("moss", 0.04)))
                jcube(b, bone, f"w{sx}_{si}_{ui}", (x, y, zc), (1.15, 0.8, 1.2), mat, rng, jit=0.05)
                if edge and si % 2 == 0:                                                        # 锯齿：边缘每隔一格伸出一小齿
                    jcube(b, bone, f"w{sx}_{si}_{ui}_tooth", (x + sx * 0.8, y - 0.3, zc + 0.8), (0.8, 0.6, 1.1), "bark_dk", rng,
                          jit=0.03)
                if not on_vein and rng.random() < 0.07:                                         # 翘起的皮
                    jcube(b, bone, f"w{sx}_{si}_{ui}_curl", (x, y + 0.7, zc), (1.0, 0.5, 1.0), "bark_lt", rng, jit=0.1)


# ───────────────────────────────────────────────────────────── Round 3 细节
def part_lichen(b: kit.Builder) -> None:
    """灰绿地衣斑：贴在胸背、腹背、前肢股节外侧和四条步足的膝上，每处三五块薄片，朽木上的苔点。"""
    rng = random.Random(SEED + 11)
    spots = (
        ("metathorax", (0.0, 17.9, 2.0), 3), ("metathorax", (2.2, 17.0, 0.0), 3), ("metathorax", (-2.0, 17.1, 4.5), 3),
        ("abd_1", (0.0, 15.9, 9.0), 4), ("abd_2", (0.0, 14.5, 14.0), 3), ("abd_3", (0.0, 12.9, 19.0), 3),
        ("prothorax_1", (1.4, 17.8, -2.4), 3), ("prothorax_2", (-1.3, 22.6, -5.8), 3),
    )
    for si, (bone, (x, y, z), n) in enumerate(spots):
        for k in range(n):
            mat = "lichen" if k % 3 else "lichen_lt"
            jcube(b, bone, f"lichen_{si}_{k}", (x + rng.uniform(-1.2, 1.2), y + 0.05 * k, z + rng.uniform(-1.2, 1.2)),
                  (rng.uniform(1.0, 1.8), 0.55, rng.uniform(1.0, 1.8)), mat, rng, jit=0.04)
    for sx in (1, -1):
        for name, pos in (("foreleg_femur", (6.4, 28.8, -12.4)), ("midleg_femur", (6.0, 17.8, -0.9)),
                          ("hindleg_femur", (6.4, 17.4, 5.2))):
            for k in range(3):
                jcube(b, sided(name, sx), f"lichen_{name}{sx}{k}", (sx * pos[0] + rng.uniform(-0.6, 0.6), pos[1] + 0.9 + 0.2 * k, pos[2] + rng.uniform(-0.9, 0.9)),
                      (1.0, 0.5, 1.2), "lichen", rng, jit=0.04)


def part_peel(b: kit.Builder) -> None:
    """翘皮与裂口：腹节背上的卷皮、胸侧的剥落片、前肢股节的纵向暗裂，让枝干像真的朽皮而不是涂色。"""
    rng = random.Random(SEED + 12)
    for i in range(5):
        a, c = ABD_PATH[i], ABD_PATH[i + 1]
        mid = (np.array(a[:3]) + np.array(c[:3])) / 2
        for sx in (1, -1):
            jcube(b, f"abd_{i}", f"peel{i}{sx}", mid + np.array([sx * a[3] * 0.85, a[3] * 0.45, 0.4]), (0.7, 1.4, 1.9), "bark_lt", rng, jit=0.05)
            jcube(b, f"abd_{i}", f"crack{i}{sx}", mid + np.array([sx * a[3] * 0.4, a[3] * 0.98, -0.6]), (0.5, 0.45, 2.2), "knot", rng, jit=0.05)
    for sx in (1, -1):
        for k, (y, z) in enumerate(((15.4, 1.6), (14.2, 3.6), (16.0, -0.8))):
            jcube(b, "metathorax", f"mpeel{sx}{k}", (sx * 3.7, y, z), (0.8, 1.6, 1.8), "bark_lt", rng, jit=0.05)
        femur = sided("foreleg_femur", sx)
        for k in range(4):
            t = k / 3.0
            jcube(b, femur, f"fcrack{sx}{k}", (sx * (5.2 + 2.2 * t), 25.0 + 5.4 * t, -7.8 - 7.4 * t), (0.45, 0.45, 1.8), "knot", rng, jit=0.05)


def part_palps(b: kit.Builder) -> None:
    """口须：下颚两侧各一对短须（下唇须），头下方的唇片；螳螂吃东西靠它们扒拉。"""
    rng = random.Random(SEED + 13)
    hx, hy, hz = HEAD_AT
    with b.offset((hx, hy, hz)):
        for sx in (1, -1):
            for k, (x, y, z, w) in enumerate(((1.6, -4.4, -2.4, 0.8), (2.0, -5.3, -2.9, 0.65))):
                jcube(b, "head", f"palp{sx}{k}", (sx * x, y, z), w, "bark_dk" if k == 0 else "knot", rng, jit=0.03)
        for k, x in enumerate((-0.9, 0.0, 0.9)):
            jcube(b, "head", f"labrum{k}", (x, -3.9, -2.2), (0.95, 0.7, 0.9), "bark_lt" if k == 1 else "bark_dk", rng, jit=0.03)


def part_eye_rims(b: kit.Builder) -> None:
    """复眼的琥珀圈：眼面外侧一圈橙褐小块 + 眼中心一粒亮点，参考图里复眼是橙褐色带暗瞳，不是全黑。"""
    rng = random.Random(SEED + 14)
    hx, hy, hz = HEAD_AT
    with b.offset((hx, hy, hz)):
        for sx in (1, -1):
            cx, cy, cz = sx * 4.4, 1.4, -0.6
            for k in range(10):
                ang = 2.0 * math.pi * k / 10
                jcube(b, "head", f"eye_rim{sx}{k}", (cx + sx * 0.5 + 0.4 * math.cos(ang), cy + 1.5 * math.sin(ang), cz - 1.5 + 0.35 * math.cos(ang)),
                      (0.7, 0.7, 0.6), "eye_amb", rng, jit=0.03)
            jcube(b, "head", f"eye_core{sx}", (cx + sx * 0.4, cy, cz - 2.1), 0.8, "eye_hi", rng, jit=0.02)


def part_wing_spots(b: kit.Builder) -> None:
    """翅鞘上的枯叶斑：沿叶面撒锈褐 / 暗褐斑块和几处虫蛀小孔（深色），枯叶的脉间不该是干净的一片。"""
    rng = random.Random(SEED + 15)
    root = np.array([0.0, 18.0, 2.4])
    tip = np.array([0.0, 9.0, 33.0])
    for sx in (1, -1):
        bone = sided("wing", sx)
        for k in range(26):
            s = rng.uniform(0.12, 0.9)
            half = 7.0 * math.sin(math.pi * min(1.0, s ** 0.85)) + 0.5
            u = rng.uniform(1.0, max(1.1, half - 0.8))
            centre = root + (tip - root) * s + np.array([0.0, 2.2 * math.sin(math.pi * s), 0.0])
            pos = (sx * (0.8 + u), centre[1] - 0.44 * u * u / max(1.0, half) + 0.55, centre[2] + 0.18 * u)
            if k % 5 == 0:
                jcube(b, bone, f"hole{sx}{k}", pos, (1.1, 0.5, 1.1), "knot", rng, jit=0.03)
            else:
                jcube(b, bone, f"spot{sx}{k}", pos, (rng.uniform(1.0, 2.0), 0.5, rng.uniform(1.0, 2.2)),
                      "bark_rust" if k % 2 else "bark_dk", rng, jit=0.04)


PARTS = {
    "thorax": part_thorax,
    "abdomen": part_abdomen,
    "neck": part_neck,
    "head": part_head,
    "jaws": part_jaws,
    "antennae": part_antennae,
    "forelegs": part_forelegs,
    "midlegs": part_midlegs,
    "hindlegs": part_hindlegs,
    "wings": part_wings,
    "lichen": part_lichen,
    "peel": part_peel,
    "palps": part_palps,
    "eye_rims": part_eye_rims,
    "wing_spots": part_wing_spots,
}

SPEC = kit.CreatureSpec(
    model_id="ku_zhi_tang",
    title="枯枝螳",
    class_name="KuZhiTang",
    creature_dir=CREATURE_DIR,
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    min_bones=26,
    round_label="round 3/3",
    soft_over=SOFT_OVER,
    asym_bones=ASYM_BONES,
    ref_prompt=(
        "an ambush praying mantis that looks exactly like a dead twig: grey-brown bark-textured segmented slender body, "
        "bark rings and knots and curling bark flakes, a tall raised prothorax, small triangular head with two large "
        "dark compound eyes with a faint amber glint, long thin antennae, raptorial forelegs like barbed sickles folded "
        "forward with the tibia tucked under the femur, thin stick-like mid and hind legs, two dead-leaf-shaped wing cases "
        "folded on its back; detail close-ups: head and eyes, barbed foreleg sickle, bark ringed abdomen, wing case "
        "veins, foot claw"
    ),
)

# ───────────────────────────────────────────────────────────── 像素贴图
# 画笔库 handheld_texture：bark* 是本只补的树皮画笔，地衣 / 苔 / 琥珀眼复用现成的
TEXTURE_MATERIAL = {
    "bark": "bark", "bark_dk": "bark_dark", "bark_lt": "bark_light", "bark_grey": "bark_grey", "bark_rust": "bark_rust",
    "knot": "bark_knot", "moss": "moss", "lichen": "lichen", "lichen_lt": "moss_light",
    "eye": "eye", "eye_amb": "glint_amber", "eye_hi": "glint_core", "claw": "claw_rust",
}


def textured_run() -> None:
    """平色 bbmodel（全量产出）→ 贴图版 KuZhiTang_tex.bbmodel（骨骼层级原样保留，动画靠它）+ check_tex.png。"""
    from creature_tex import texture_bbmodel, write_check_tex

    flat = SPEC.creature_dir / f"{SPEC.class_name}.bbmodel"
    tex = SPEC.creature_dir / f"{SPEC.class_name}_tex.bbmodel"
    b = kit.build(SPEC)
    lo, hi = b.rig.bounds()
    center = tuple((lo[i] + hi[i]) / 2 for i in range(3))
    span = max(hi[i] - lo[i] for i in range(3)) * 1.1
    texture_bbmodel(flat, tex, tuple(MATS), TEXTURE_MATERIAL)
    head = ((0.0, 33.0, -12.0), 24.0)
    views = (("3/4 FRONT-RIGHT", 150.0, 22.0, (center, span)), ("LEFT 侧（翅鞘 / 树皮）", 90.0, 0.0, (center, span)),
             ("头部特写（复眼琥珀）", 180.0, 6.0, head))
    out = write_check_tex(flat, tex, SPEC.review_dir / "check_tex.png", views, SPEC.review_dir / "ku_zhi_tang_ref.png")
    print(f"✓ {tex.name}；check_tex.png → {out}")


if __name__ == "__main__":
    if "--tex" in sys.argv:
        textured_run()
    else:
        kit.run_cli(SPEC)
