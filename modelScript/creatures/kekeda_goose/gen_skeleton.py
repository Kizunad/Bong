#!/usr/bin/env python3
"""珂珂达（kekeda_goose）骨架生成器 —— Round 1/3。

按雁形目（Anseriformes）解剖建骨，不是拿方块拼个鸭子：

  颅骨    脑颅 + 巨眼眶 + 方骨/颧弓细杆 + 上下喙（栉板 lamellae 逐枚建出来）
  颈椎    17 节，异凹型，静止姿折成 S —— 这是全鸟最长的一段，却整条藏在绒羽里
  躯干    愈合胸椎（notarium）+ 综荐骨（synsacrum）+ 尾综骨（pygostyle）
  胸廓    7 对肋，椎肋段 + 胸肋段两折，带**钩突**（uncinate process，鸟类专属）
  龙骨    胸骨 + 深大的龙骨突（carina）—— 全身最深的骨，飞肌都挂在它上面
  肩带    叉骨（furcula）+ 乌喙骨（coracoid）+ 肩胛骨，三骨孔的三角撑
  翼      肱骨 → 桡尺骨 → 腕掌骨 → 指骨，静止姿折成 Z 贴在体侧
  腿      股骨（短、近水平，整根埋在体腔里）→ 胫跗骨 → 跗跖骨 → 4 趾 3 蹼

两条容易做错的：
  · 站立时看得见的那个"向后弯的膝盖"**是踝**（跗间关节）。真膝在体腔里，
    股骨几乎水平。把它当膝盖画，鹅就成了长腿鹤。
  · 球是羽毛不是身体。骨架宽度只有绒羽球的一半出头，剩下的全靠 gen_plume 的
    绒羽层撑出来 —— 参考照片里那只"完美球体"的笑点就是这个解剖事实。

骨骼层级供 GeckoLib 驱动；element 一律写**绝对坐标**（绑定姿态下与 pivot 自洽），
因为 render_bbmodel.py 只读 elements 不读 outliner。

用法:
  python3 modelScript/creatures/kekeda_goose/gen_skeleton.py               # 全骨架
  python3 modelScript/creatures/kekeda_goose/gen_skeleton.py --part skull  # 单件预览
  python3 modelScript/creatures/kekeda_goose/gen_skeleton.py --list
  python3 modelScript/creatures/kekeda_goose/gen_skeleton.py --check
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))

from bbmodel_maker.rig.voxel_rig import Palette, Rig, Vec, catmull, curve_length, lerp  # noqa: E402

REPO = HERE.parents[2]
OUT_DIR = Path(__file__).resolve().parents[2] / "models" / "kekeda_goose"

# ================================================================ 全局尺度
# 单位 = MC 像素，16 = 1 格 = 1 m。地面 y=0，头朝 -Z。
# 目标：站高 ≈ 15.9 单位 ≈ 0.99 m —— 狮头鹅级别的大白鹅，到玩家膝盖上方。
BACK_Y = 10.3          # 背线（愈合胸椎顶面）
KEEL_BOTTOM_Y = 5.4    # 龙骨腹缘最低点
BREAST_Z = -4.4        # 胸前缘（叉骨联合 / 胸骨前突）
TAIL_BASE_Z = 4.6      # 尾综骨根
HIP_Z = 1.5            # 髋臼
HIP_X = 1.70           # 髋臼半距 —— 参考照片里两腿间距只有球半径的 1/3，
                       # 所以单脚支撑时重心远在支撑脚外侧，走路必须靠侧倾兜回来。
                       # "摇摆步"（waddle）不是风格化，是这个几何逼出来的。

# 颈椎静止姿：折成 S 缩在肩上。下段后仰、上段前探，全长与伸直时相同
# （骨头不会缩短，折叠只改曲率）——伸颈威吓时把这条曲线拉直即可。
#
# S 要**折得竖**。首版末端落在 z=-4.30、脑颅中心 -5.15，比球心靠前 4.85 ——
# 差不多整个球半径，头等于探在球前方，颈那 3 个单位的空档只能靠羽毛硬填，
# 填出来就是块脖套。参考照片里头基本是坐在球顶上的，只有喙探出去。
NECK_KNOTS: tuple[Vec, ...] = (
    (0.0, 9.75, -1.30),
    (0.0, 11.65, 0.15),
    (0.0, 13.25, -0.45),
    (0.0, 13.95, -1.80),
    (0.0, 13.75, -3.10),
)
NECK_VERTEBRAE = 17     # 雁属 17~18 节，取 17

OCCIPUT: Vec = NECK_KNOTS[-1]
SKULL_C: Vec = (0.0, 14.45, -3.95)   # 脑颅中心
BILL_ROOT_Z = -4.95                  # 蜡膜/喙根
BILL_TIP_Z = -8.10                   # 喙尖。首版取 -8.4（喙长 2.25 ≈ 0.9 倍头长，
                                     # 比例查着没错），渲出来却是块砖：问题不在长度
                                     # 而在**厚度**——鸭喙宽高比约 2:1，我按 1.8:1
                                     # 画又只有 2.25 长，三维接近立方体就读成方块。
                                     # 现在压扁到 3:1 并加长到 3.15。
LAMELLAE = 8                         # 单侧栉板枚数（鹅嘴那排"牙"）

MATS = {
    "bone": (214, 205, 184),
    "bone_dark": (176, 165, 142),
    "cartilage": (198, 200, 192),
    "socket": (70, 62, 52),        # 眼眶/鼻孔：体素没有布尔减法，洞只能靠深色面读出来
    "keratin": (214, 132, 56),     # 喙 / 跗跖鳞：陶土橙，不是塑料橙
    "keratin_dark": (168, 94, 38),
    "lamella": (236, 226, 204),    # 栉板
}
PALETTE = Palette(MATS)


def trunk_y(z: float) -> float:
    """躯干椎体中心线高度。鸟背几乎是直的（愈合），只在荐后微降、尾根上翘。"""
    knots = [(-3.2, BACK_Y - 0.1), (0.6, BACK_Y - 0.2), (2.6, BACK_Y - 0.45),
             (TAIL_BASE_Z, BACK_Y - 0.7), (5.8, BACK_Y - 0.1)]
    if z <= knots[0][0]:
        return knots[0][1]
    if z >= knots[-1][0]:
        return knots[-1][1]
    for (z0, y0), (z1, y1) in zip(knots, knots[1:]):
        if z0 <= z <= z1:
            t = (z - z0) / (z1 - z0)
            return lerp(y0, y1, t * t * (3 - 2 * t))
    return knots[-1][1]


def neck_at(t: float) -> Vec:
    return catmull(NECK_KNOTS, t)


# ================================================================ 部件：颅骨
def part_skull(rig: Rig) -> None:
    """脑颅 + 眼眶 + 颧弓/方骨细杆 + 枕髁。

    鸟颅的辨识点是**眼眶巨大**：眼球占了颅骨侧面一大半，两眶之间只剩一层薄骨板。
    照哺乳类那样画个小眼窝，出来就是个缩小的兽头。
    """
    cx, cy, cz = SKULL_C
    rig.bone("skull", OCCIPUT, parent="neck_16")

    # 脑颅：后大前小，顶面圆
    rig.cube("skull", "cranium", (-1.08, cy - 1.05, cz - 0.95), (1.08, cy + 1.10, cz + 1.25), mat="bone")
    rig.cube("skull", "cranium_crown", (-0.80, cy + 1.02, cz - 0.70), (0.80, cy + 1.32, cz + 0.85), mat="bone")
    rig.cube("skull", "occiput", (-0.86, cy - 0.95, cz + 1.15), (0.86, cy + 0.70, cz + 1.55), mat="bone_dark")
    # 枕髁：与寰椎（neck_16）对接的那颗球，位置必须落在颈曲线末端上
    rig.cube("skull", "occipital_condyle",
             (-0.34, OCCIPUT[1] - 0.30, OCCIPUT[2] - 0.05), (0.34, OCCIPUT[1] + 0.34, OCCIPUT[2] + 0.42),
             mat="cartilage")

    for sx, side in ((-1, "l"), (1, "r")):
        # 眼眶：一整块深色，占满颅侧
        rig.cube("skull", f"orbit_{side}",
                 (sx * 0.72, cy - 0.52, cz - 1.02), (sx * 1.16, cy + 0.86, cz + 0.52), mat="socket")
        # 眶上缘（眉骨），把眼眶框出来
        rig.cube("skull", f"supraorbital_{side}",
                 (sx * 0.66, cy + 0.80, cz - 1.10), (sx * 1.20, cy + 1.10, cz + 0.60), mat="bone")
        rig.cube("skull", f"suborbital_{side}",
                 (sx * 0.66, cy - 0.76, cz - 1.05), (sx * 1.14, cy - 0.46, cz + 0.45), mat="bone")
        # 颧弓 / 方轭骨：眶下那根细杆，鸟颅特征之一（哺乳类是粗弓）
        rig.shaft("skull", f"jugal_bar_{side}",
                  (sx * 1.00, cy - 0.62, cz + 0.30), (sx * 0.86, cy - 0.30, BILL_ROOT_Z + 0.20),
                  0.13, 0.13, mat="bone_dark")
        # 方骨：连下颌的那块，位于耳区
        rig.shaft("skull", f"quadrate_{side}",
                  (sx * 0.92, cy - 0.05, cz + 1.05), (sx * 0.88, cy - 0.72, cz + 0.55),
                  0.24, 0.24, mat="bone")


def part_bill(rig: Rig) -> None:
    """上下喙 + 喙甲 + 鼻孔 + 栉板。

    鸭雁的喙是**扁而宽、越往前越宽**，尖端有一小片硬"喙甲"（nail）。侧缘那排栉板
    真长得像牙——滤食用的，但正因为像牙，鹅张嘴嘶叫时才有威慑力，是这只生物
    唯一的"武器"读点，得逐枚建出来而不是刻在贴图上。
    """
    cy = SKULL_C[1]
    rig.bone("bill_upper", (0.0, cy - 0.10, BILL_ROOT_Z), parent="skull")
    rig.bone("jaw", (0.0, cy - 0.78, SKULL_C[2] + 0.55), parent="skull")

    # ---- 上喙：6 段，越往前越宽越扁，末端再收圆。
    # (z, 半宽, 腹缘 dy, 背缘 dy) —— 背缘从喙根一路下压：喙不是水平伸出去的，
    # 它顺着额线往前下方倾，侧看是个楔子。首版没压，喙就"翘"在脸前面。
    seg = [
        (SKULL_C[2] + 0.35, 0.86, -0.55, +0.60),
        (BILL_ROOT_Z, 1.02, -0.62, +0.26),
        (lerp(BILL_ROOT_Z, BILL_TIP_Z, 0.30), 1.20, -0.74, -0.02),
        (lerp(BILL_ROOT_Z, BILL_TIP_Z, 0.60), 1.26, -0.92, -0.24),
        (lerp(BILL_ROOT_Z, BILL_TIP_Z, 0.86), 1.20, -1.10, -0.46),
        (BILL_TIP_Z + 0.20, 0.98, -1.22, -0.62),
    ]
    for i, ((z0, w0, yl0, yh0), (z1, w1, yl1, yh1)) in enumerate(zip(seg, seg[1:])):
        w, yl, yh = (w0 + w1) / 2, (yl0 + yl1) / 2, (yh0 + yh1) / 2
        # 每段切三层（上薄下薄中间满宽）＝ 六边形截面。整段一个矩形截面时，
        # 正面看喙是块贴在脸上的橙方片；喙的横截面其实是上下扁、两侧圆的
        h = yh - yl
        for j, (frac, y0, y1) in enumerate((
            (0.80, yl + h * 0.72, yh),
            (1.00, yl + h * 0.22, yl + h * 0.76),
            (0.84, yl, yl + h * 0.26),
        )):
            rig.cube("bill_upper", f"maxilla_{i}_{j}",
                     (-w * frac, cy + y0, z1), (w * frac, cy + y1, z0), mat="keratin")
    # 喙甲：尖端那片下弯的硬壳，鸭雁独有的小指甲
    rig.cube("bill_upper", "bill_nail",
             (-0.62, cy - 1.34, BILL_TIP_Z), (0.62, cy - 0.66, BILL_TIP_Z + 0.55), mat="keratin_dark")

    def profile(z: float) -> tuple[float, float]:
        """喙在 z 处的 (半宽, 腹缘 y)。下颌和栉板都从这里取，三者才咬得住；
        各写各的常数一改就错位（首版下颌就是这么埋进上喙里的）。"""
        for (z0, w0, yl0, _), (z1, w1, yl1, _) in zip(seg, seg[1:]):
            if z1 <= z <= z0:
                t = (z0 - z) / (z0 - z1)
                return lerp(w0, w1, t), cy + lerp(yl0, yl1, t)
        return seg[-1][1], cy + seg[-1][2]

    for sx, side in ((-1, "l"), (1, "r")):
        # 鼻孔：喙根偏后的一对深色孔
        rig.cube("bill_upper", f"naris_{side}",
                 (sx * 0.58, cy - 0.20, BILL_ROOT_Z - 0.85), (sx * 0.94, cy + 0.16, BILL_ROOT_Z - 0.15),
                 mat="socket")
        # 栉板：沿上下喙缘各一排，卡在两喙咬合缝上略微外露 —— 闭嘴时是条锯齿线，
        # 张嘴嘶叫才整排亮出来。这是这只生物唯一的"武器"读点
        for i in range(LAMELLAE):
            t = (i + 0.5) / LAMELLAE
            z = lerp(BILL_ROOT_Z - 0.35, BILL_TIP_Z + 0.70, t)
            w, ylow = profile(z)
            rig.cube("bill_upper", f"lamella_up_{side}_{i}",
                     (sx * (w - 0.20), ylow - 0.04, z - 0.13), (sx * (w + 0.07), ylow + 0.17, z + 0.13),
                     mat="lamella")
            rig.cube("jaw", f"lamella_lo_{side}_{i}",
                     (sx * (w - 0.24), ylow - 0.30, z - 0.13), (sx * (w + 0.03), ylow - 0.09, z + 0.13),
                     mat="lamella")

    # ---- 下颌：两条下颌支从方骨向前，汇成一枚托住上喙的浅勺
    for sx, side in ((-1, "l"), (1, "r")):
        rig.shaft("jaw", f"mandible_ramus_{side}",
                  (sx * 0.90, cy - 0.86, SKULL_C[2] + 0.62), (sx * 0.70, cy - 1.02, BILL_ROOT_Z - 0.40),
                  0.20, 0.28, mat="bone")
    for i in range(5):
        z0 = lerp(BILL_ROOT_Z - 0.40, BILL_TIP_Z + 0.15, i / 5)
        z1 = lerp(BILL_ROOT_Z - 0.40, BILL_TIP_Z + 0.15, (i + 1) / 5)
        w, ylow = profile((z0 + z1) / 2)
        w *= 0.92                       # 上喙包住下喙，下颌略窄一圈
        rig.cube("jaw", f"mandible_{i}", (-w, ylow - 0.46, z1), (w, ylow - 0.02, z0), mat="keratin")
    # 舌：鸭雁的舌厚实带侧刺，张嘴才看得见
    rig.cube("jaw", "tongue",
             (-0.52, cy - 1.02, BILL_TIP_Z + 1.20), (0.52, cy - 0.78, BILL_ROOT_Z - 0.30), mat="cartilage")


# ================================================================ 部件：颈椎
def part_neck(rig: Rig) -> None:
    """17 节颈椎沿 S 曲线排布。

    按**弧长**均分，不按参数 t 均分——Catmull 的 t 在曲率大的地方走得慢，
    直接按 t 切会让弯道处椎节挤成一堆、直道处拉出缝。
    """
    total = curve_length(NECK_KNOTS)
    # 弧长 → 参数 t 的查找表
    samples = 600
    acc, table = 0.0, [(0.0, 0.0)]
    prev = neck_at(0.0)
    for i in range(1, samples + 1):
        t = i / samples
        cur = neck_at(t)
        acc += math.dist(prev, cur)
        table.append((acc, t))
        prev = cur

    def t_at_arc(s: float) -> float:
        for (a0, t0), (a1, t1) in zip(table, table[1:]):
            if a0 <= s <= a1:
                return lerp(t0, t1, (s - a0) / max(1e-9, a1 - a0)) if a1 > a0 else t0
        return 1.0

    joints = [neck_at(t_at_arc(total * i / NECK_VERTEBRAE)) for i in range(NECK_VERTEBRAE + 1)]

    parent = "trunk_front"
    for i in range(NECK_VERTEBRAE):
        a, b = joints[i], joints[i + 1]
        name = f"neck_{i}"
        rig.bone(name, a, parent=parent)
        parent = name
        # 椎体：越往上越细（颈根粗、寰枢椎细）
        t = i / (NECK_VERTEBRAE - 1)
        r = lerp(0.52, 0.34, t)
        rig.shaft(name, f"cervical_{i}", a, b, r, r, mat="bone", extend=0.06)
        # 神经棘 / 横突：只在下半段明显（上段颈椎近乎光滑，方便折叠）
        if i < 11:
            up = 1.0 - 0.55 * t
            rig.cube(name, f"cervical_spine_{i}",
                     (-0.16, a[1] + r * 0.6, a[2] - 0.22), (0.16, a[1] + r * 0.6 + 0.34 * up, a[2] + 0.30),
                     mat="bone_dark")
        for sx, side in ((-1, "l"), (1, "r")):
            rig.cube(name, f"cervical_tp_{side}_{i}",
                     (sx * r * 0.7, a[1] - 0.16, a[2] - 0.14), (sx * (r * 0.7 + 0.26), a[1] + 0.16, a[2] + 0.20),
                     mat="bone_dark")

    # 气管：沿颈腹侧走，鹅嘶叫的声源在气管末端的鸣管。软骨环做成分节的
    for i in range(13):
        t0, t1 = t_at_arc(total * i / 13), t_at_arc(total * (i + 0.62) / 13)
        p0, p1 = neck_at(t0), neck_at(t1)
        off = 0.44 if i > 2 else 0.30
        rig.shaft(f"neck_{min(i, NECK_VERTEBRAE - 1)}", f"trachea_{i}",
                  (p0[0], p0[1] - off, p0[2] - off * 0.5), (p1[0], p1[1] - off, p1[2] - off * 0.5),
                  0.24, 0.24, mat="cartilage")


# ================================================================ 部件：躯干
def part_trunk(rig: Rig) -> None:
    """愈合胸椎 + 综荐骨 + 尾综骨。鸟的脊柱大半是**融死**的，飞行要求刚性躯干。"""
    rig.bone("root", (0.0, 0.0, 0.0))
    rig.bone("hips", (0.0, trunk_y(HIP_Z), HIP_Z), parent="root")
    rig.bone("trunk_back", (0.0, trunk_y(0.6), 0.6), parent="hips")
    rig.bone("trunk_front", (0.0, trunk_y(-3.0), -3.0), parent="trunk_back")
    rig.bone("tail_base", (0.0, trunk_y(TAIL_BASE_Z), TAIL_BASE_Z), parent="hips")

    # 愈合胸椎（notarium）：一整条，只在背面留出愈合的棘突脊
    rig.cube("trunk_front", "notarium", (-0.62, trunk_y(-1.2) - 0.62, -3.30), (0.62, trunk_y(-1.2) + 0.30, 0.70),
             mat="bone")
    rig.cube("trunk_front", "notarium_crest", (-0.24, trunk_y(-1.2) + 0.26, -3.10), (0.24, trunk_y(-1.2) + 0.72, 0.55),
             mat="bone_dark")
    # 综荐骨：与髂骨愈合成一块骨盆顶板
    rig.cube("hips", "synsacrum", (-0.66, trunk_y(2.4) - 0.58, 0.60), (0.66, trunk_y(2.4) + 0.34, 4.30), mat="bone")
    # 游离尾椎 5 节 + 尾综骨（上翘的犁形骨，尾羽都插在它上面）
    for i in range(5):
        z0 = TAIL_BASE_Z - 0.30 + i * 0.42
        rig.cube("tail_base", f"caudal_{i}", (-0.34, trunk_y(z0) - 0.34, z0), (0.34, trunk_y(z0) + 0.30, z0 + 0.40),
                 mat="bone")
    rig.shaft("tail_base", "pygostyle", (0.0, trunk_y(6.0) - 0.10, 6.00), (0.0, trunk_y(6.0) + 1.10, 6.95),
              0.30, 0.26, mat="bone_dark")


RIB_PAIRS = 7


def part_ribcage(rig: Rig) -> None:
    """7 对肋 + 钩突 + 胸骨 + 龙骨突。

    钩突（uncinate process）是每根肋向后搭到下一根上的骨钩，只有鸟有——它把
    整个胸廓锁成刚性盒子，飞肌用力时胸廓才不会被拉塌。少了它就是个爬行类胸廓。
    龙骨深度直接决定这只鸟像不像鸟：占体高的三成。
    """
    rig.bone("sternum", (0.0, 7.0, -1.0), parent="trunk_front")

    for i in range(RIB_PAIRS):
        t = i / (RIB_PAIRS - 1)
        z_vert = lerp(-3.00, 0.55, t)            # 椎端
        z_stern = lerp(-3.60, 0.05, t)           # 胸骨端
        half = lerp(1.95, 2.55, math.sin(math.pi * (0.22 + 0.72 * t)))  # 胸廓中段最宽
        y_vert = trunk_y(z_vert) - 0.30
        y_bend = lerp(7.95, 7.55, t)             # 折点（椎肋 → 胸肋）
        y_stern = lerp(7.05, 7.20, t)
        for sx, side in ((-1, "l"), (1, "r")):
            bend = (sx * half, y_bend, lerp(z_vert, z_stern, 0.55))
            rig.shaft("trunk_front", f"rib_vert_{side}_{i}",
                      (sx * 0.58, y_vert, z_vert), bend, 0.17, 0.24, mat="bone")
            rig.shaft("sternum", f"rib_stern_{side}_{i}",
                      bend, (sx * lerp(0.85, 1.65, t), y_stern, z_stern), 0.16, 0.22, mat="cartilage")
            if 0 < i < RIB_PAIRS - 1:   # 钩突：向后斜搭下一根
                rig.shaft("trunk_front", f"uncinate_{side}_{i}",
                          (bend[0] * 0.96, bend[1] + 0.15, bend[2] - 0.10),
                          (bend[0] * 0.90, bend[1] + 0.62, bend[2] + 0.95), 0.13, 0.13, mat="bone_dark")

    # ---- 胸骨板：前窄后宽的一块大板
    plate = [(-4.00, 0.95), (-2.40, 1.85), (0.00, 2.35), (2.20, 2.10), (3.40, 1.45)]
    for i, ((z0, w0), (z1, w1)) in enumerate(zip(plate, plate[1:])):
        w = (w0 + w1) / 2
        rig.cube("sternum", f"sternum_plate_{i}", (-w, 6.82, z0), (w, 7.30, z1), mat="bone")
    # ---- 龙骨突：矢状薄板，腹缘中段最低
    keel = [(-4.40, 6.30), (-3.00, 5.62), (-1.20, KEEL_BOTTOM_Y), (0.80, 5.58), (2.40, 6.60)]
    for i, ((z0, y0), (z1, y1)) in enumerate(zip(keel, keel[1:])):
        rig.cube("sternum", f"keel_{i}", (-0.30, min(y0, y1), z0), (0.30, 6.95, z1), mat="bone")
    rig.cube("sternum", "keel_apex", (-0.42, KEEL_BOTTOM_Y - 0.16, -3.10), (0.42, 6.10, 0.60), mat="bone_dark")


def part_girdle(rig: Rig) -> None:
    """肩带三骨：叉骨（弹簧）+ 乌喙骨（撑杆）+ 肩胛骨（刀条）。

    三者在肩头围出**三骨孔**（foramen triosseum），上乌喙肌的腱从孔里穿过再拐到
    肱骨背面 —— 一个真正的滑轮：肌肉长在胸骨下方，却能把翅膀往上抬。
    """
    for sx, side in ((-1, "l"), (1, "r")):
        sh = (sx * 1.90, 10.05, -2.85)
        rig.shaft("trunk_front", f"coracoid_{side}", sh, (sx * 1.00, 7.32, -3.70), 0.30, 0.34, mat="bone")
        rig.shaft("trunk_front", f"scapula_{side}", sh, (sx * 1.72, 10.28, 1.50), 0.16, 0.42, mat="bone")
        rig.shaft("trunk_front", f"furcula_{side}", (sx * 1.82, 9.95, -2.95), (sx * 0.10, 8.00, -4.05),
                  0.18, 0.22, mat="bone")
        rig.cube("trunk_front", f"glenoid_{side}",
                 (sx * 1.55, 9.70, -3.15), (sx * 2.20, 10.35, -2.45), mat="cartilage")
    rig.cube("trunk_front", "furcula_symphysis", (-0.34, 7.72, -4.30), (0.34, 8.22, -3.80), mat="bone_dark")


def part_pelvis(rig: Rig) -> None:
    """骨盆：髂骨顶板与综荐骨愈合，坐骨/耻骨在下后方；髋臼是腿的起点。"""
    for sx, side in ((-1, "l"), (1, "r")):
        # 髂骨：一块薄板，从胸后一路铺到尾根
        rig.cube("hips", f"ilium_{side}", (sx * 0.60, 9.35, -0.70), (sx * 2.35, 10.30, 4.20), mat="bone")
        rig.cube("hips", f"acetabulum_{side}",
                 (sx * (HIP_X - 0.52), 8.20, HIP_Z - 0.58), (sx * (HIP_X + 0.46), 9.10, HIP_Z + 0.56),
                 mat="cartilage")
        rig.shaft("hips", f"ischium_{side}", (sx * 1.62, 8.60, HIP_Z + 0.30), (sx * 1.58, 9.05, 4.05),
                  0.42, 0.16, mat="bone_dark")
        rig.shaft("hips", f"pubis_{side}", (sx * 1.35, 8.10, HIP_Z + 0.20), (sx * 1.30, 8.05, 4.35),
                  0.16, 0.14, mat="bone_dark")


# ================================================================ 部件：翼
def part_wing(rig: Rig, sx: int, side: str) -> None:
    """折叠翼：肱骨向后 → 桡尺骨折回向前 → 腕掌骨再向后，一个 Z。

    静止姿整只翼贴在体侧、埋在绒羽里，只有初级飞羽的尖端搭过尾根露出来。
    威吓展翼时这个 Z 一次性抖开 —— 这只生物两种剪影（球 / 张翼）的全部机关。
    """
    # 折叠翼的四个点必须都**贴着背线**（y 9.4~10.3）。首版把肘放到 y=8.9，
    # 整只翼垂到体侧中段，Z 折读成了三根散骨。真鸟收翼时肱骨几乎水平地往后压，
    # 肘尖顶在背轮廓上——那个小尖角就是收翼的辨识点。
    sh = (sx * 1.90, 10.05, -2.80)
    elbow = (sx * 2.62, 9.72, 1.15)      # 肘：向后，顶在背线上
    wrist = (sx * 2.92, 10.05, -2.10)    # 腕：折回向前，几乎回到肩旁
    manus = (sx * 3.00, 9.62, 0.95)      # 掌：再向后
    digit = (sx * 2.94, 9.30, 2.60)      # 指端：搭过尾根，初级飞羽从这里长出去

    rig.bone(f"wing_{side}", sh, parent="trunk_front")
    rig.bone(f"forearm_{side}", elbow, parent=f"wing_{side}")
    rig.bone(f"hand_{side}", wrist, parent=f"forearm_{side}")

    rig.shaft(f"wing_{side}", f"humerus_{side}", sh, elbow, 0.36, 0.40, mat="bone")
    rig.cube(f"wing_{side}", f"deltoid_crest_{side}",
             (sx * 1.62, 9.92, -2.95), (sx * 2.32, 10.42, -2.05), mat="bone_dark")
    rig.shaft(f"forearm_{side}", f"ulna_{side}", elbow, wrist, 0.28, 0.30, mat="bone")
    rig.shaft(f"forearm_{side}", f"radius_{side}",
              (elbow[0] + sx * 0.30, elbow[1] + 0.42, elbow[2] - 0.10),
              (wrist[0] + sx * 0.24, wrist[1] + 0.34, wrist[2] - 0.05), 0.15, 0.17, mat="bone")
    rig.shaft(f"hand_{side}", f"carpometacarpus_{side}", wrist, manus, 0.24, 0.30, mat="bone")
    rig.shaft(f"hand_{side}", f"digit_major_{side}", manus, digit, 0.17, 0.20, mat="bone")
    # 小翼羽的那根拇指骨，卡在腕前
    rig.shaft(f"hand_{side}", f"alula_{side}", wrist, (sx * 3.16, 9.90, -3.30), 0.14, 0.15, mat="bone_dark")


# ================================================================ 部件：腿
def leg_joints(sx: int) -> tuple[Vec, Vec, Vec, Vec]:
    """髋 → 膝 → 踝 → 跖趾。返回的第三个点才是站立时**看得见**的那个弯。"""
    hip = (sx * HIP_X, 8.60, HIP_Z)
    knee = (sx * 2.05, 7.10, -1.00)     # 真膝：埋在体腔里，股骨近水平
    ankle = (sx * 1.95, 3.30, 0.70)     # 跗间关节 = 肉眼所见的"倒膝盖"
    toe_base = (sx * 1.90, 0.55, 0.15)
    return hip, knee, ankle, toe_base


TOES = (
    # (名字, 末端 x 偏移, 末端 z, 基部粗细) —— II 内、III 中、IV 外，III 最长。
    # x 偏移相对**脚**、再乘 sx，所以负值一律朝内侧：两脚才是镜像而不是平移。
    # 张开度按参考照片给：那双蹼几乎和头一样宽，是这只生物除了球以外最大的读点。
    ("ii", -1.45, -2.20, 0.26),
    ("iii", -0.10, -2.95, 0.29),
    ("iv", 1.70, -1.95, 0.27),
)


def part_leg(rig: Rig, sx: int, side: str) -> None:
    """股骨 / 胫跗骨 / 跗跖骨 / 4 趾 / 3 蹼。

    比例是鸟腿的读点：股骨短到几乎看不见，胫跗骨（"大腿肉"）是最长的一节，
    跗跖骨才是露在羽毛外面那截"腿"。把跗跖骨当小腿画短了，鹅就蹲成了鸭。
    """
    hip, knee, ankle, toe_base = leg_joints(sx)
    rig.bone(f"femur_{side}", hip, parent="hips")
    rig.bone(f"tibia_{side}", knee, parent=f"femur_{side}")
    rig.bone(f"tarsus_{side}", ankle, parent=f"tibia_{side}")
    rig.bone(f"foot_{side}", toe_base, parent=f"tarsus_{side}")

    rig.shaft(f"femur_{side}", f"femur_{side}", hip, knee, 0.42, 0.44, mat="bone")
    rig.cube(f"femur_{side}", f"trochanter_{side}",
             (sx * (HIP_X - 0.30), 8.55, HIP_Z - 0.75), (sx * (HIP_X + 0.52), 9.25, HIP_Z + 0.20), mat="bone_dark")
    rig.cube(f"tibia_{side}", f"patella_{side}",
             (sx * 1.70, 7.20, -1.55), (sx * 2.40, 7.80, -0.95), mat="cartilage")
    rig.shaft(f"tibia_{side}", f"tibiotarsus_{side}", knee, ankle, 0.40, 0.42, mat="bone")
    rig.shaft(f"tibia_{side}", f"fibula_{side}",
              (knee[0] + sx * 0.44, knee[1] - 0.20, knee[2] + 0.10),
              (ankle[0] + sx * 0.30, ankle[1] + 1.30, ankle[2] - 0.05), 0.13, 0.14, mat="bone_dark")
    rig.cube(f"tarsus_{side}", f"intertarsal_{side}",
             (sx * 1.55, 3.00, 0.25), (sx * 2.35, 3.75, 1.15), mat="cartilage")
    # 跗跖骨：露在外面的那截，包一层橙色角质鳞（骨架层也画上，否则腿看着是白的）
    rig.shaft(f"tarsus_{side}", f"tarsometatarsus_{side}", ankle, toe_base, 0.34, 0.36, mat="bone")
    for i in range(4):
        t = (i + 0.5) / 4
        p = [lerp(a, b, t) for a, b in zip(ankle, toe_base)]
        rig.cube(f"tarsus_{side}", f"tarsal_scale_{side}_{i}",
                 (p[0] - 0.42, p[1] - 0.22, p[2] - 0.44), (p[0] + 0.42, p[1] + 0.22, p[2] + 0.42),
                 mat="keratin")

    # ---- 前三趾：每趾 3 节，逐节变细、末端带爪
    tips: dict[str, Vec] = {}
    for name, dx, dz, r0 in TOES:
        a = toe_base
        end = (toe_base[0] + sx * dx, 0.30, dz)
        tips[name] = end
        for j in range(3):
            t0, t1 = j / 3, (j + 1) / 3
            p0 = tuple(lerp(u, v, t0) for u, v in zip(a, end))
            p1 = tuple(lerp(u, v, t1) for u, v in zip(a, end))
            r = lerp(r0, r0 * 0.55, t1)
            rig.shaft(f"foot_{side}", f"phalanx_{name}_{side}_{j}", p0, p1, r, r, mat="keratin")
        rig.shaft(f"foot_{side}", f"claw_{name}_{side}",
                  end, (end[0] + sx * dx * 0.12, 0.18, dz - 0.42), 0.13, 0.13, mat="keratin_dark")

    # ---- 后趾（拇趾）：小、朝后、略离地
    rig.shaft(f"foot_{side}", f"phalanx_i_{side}",
              toe_base, (toe_base[0] + sx * 0.10, 0.62, 1.05), 0.16, 0.16, mat="keratin")

    # ---- 蹼：II–III、III–IV 之间的两片薄膜（雁形目是"蹼足"，后趾不连蹼）
    # 按 x 切列做，不按趾长参数做：脚的主要观察角是斜上方俯视，按 x 切出来的
    # 阶梯边缘正好顺着蹼的轮廓；按 t 切会得到一排斜着错开的条，俯视像把梳子。
    WEB_COLS = 5
    for a_name, b_name in (("ii", "iii"), ("iii", "iv")):
        pa, pb = tips[a_name], tips[b_name]
        for k in range(WEB_COLS):
            f0, f1 = k / WEB_COLS, (k + 1) / WEB_COLS
            x0, x1 = lerp(pa[0], pb[0], f0), lerp(pa[0], pb[0], f1)
            fm = (f0 + f1) / 2
            # 前缘内凹：真蹼够不到趾尖，两趾之间还要往回缩，边缘是弧不是直线
            reach = 0.88 + 0.12 * abs(2 * fm - 1)
            z_front = lerp(lerp(pa[2], pb[2], fm), toe_base[2], 1.0 - reach)
            rig.cube(f"foot_{side}", f"web_{a_name}{b_name}_{side}_{k}",
                     (min(x0, x1), 0.10, min(z_front, toe_base[2])),
                     (max(x0, x1), 0.38, max(z_front, toe_base[2])), mat="keratin_dark")


# ================================================================ 装配
def _stub_trunk(rig: Rig) -> None:
    """单件预览用：只铺到该部件所需的父骨链。"""
    part_trunk(rig)


PARTS: dict[str, tuple[str, object]] = {
    "trunk": ("躯干脊柱", lambda r: part_trunk(r)),
    "ribcage": ("胸廓 + 龙骨", lambda r: (_stub_trunk(r), part_ribcage(r))),
    "girdle": ("肩带三骨", lambda r: (_stub_trunk(r), part_girdle(r))),
    "neck": ("颈椎 S 曲", lambda r: (_stub_trunk(r), part_neck(r))),
    "skull": ("颅骨", lambda r: (_stub_trunk(r), part_neck(r), part_skull(r))),
    "bill": ("喙 + 栉板", lambda r: (_stub_trunk(r), part_neck(r), part_skull(r), part_bill(r))),
    "wing": ("折叠翼", lambda r: (_stub_trunk(r), part_girdle(r),
                                  part_wing(r, -1, "l"), part_wing(r, 1, "r"))),
    "pelvis": ("骨盆", lambda r: (_stub_trunk(r), part_pelvis(r))),
    "leg": ("腿 + 蹼足", lambda r: (_stub_trunk(r), part_pelvis(r),
                                    part_leg(r, -1, "l"), part_leg(r, 1, "r"))),
}


def build_full() -> Rig:
    rig = Rig(PALETTE)
    part_trunk(rig)
    part_ribcage(rig)
    part_girdle(rig)
    part_pelvis(rig)
    part_neck(rig)
    part_skull(rig)
    part_bill(rig)
    for sx, side in ((-1, "l"), (1, "r")):
        part_wing(rig, sx, side)
        part_leg(rig, sx, side)
    return rig


def check(rig: Rig) -> int:
    problems = rig.mirror_problems()
    lo, hi = rig.bounds()

    if not -0.05 <= lo[1] <= 0.30:
        problems.append(f"贴地异常：最低点 y={lo[1]:.2f}（蹼应平铺在地面上，0~0.3）；"
                        f"最低几件：{', '.join(f'{n}@{y:.2f}' for y, n in rig.lowest(4))}")
    keel = min(e["from"][1] for e in rig.elements if e["name"].startswith("keel"))
    if not 5.0 <= keel <= 6.0:
        problems.append(f"龙骨深度异常：腹缘 y={keel:.2f}（应 5.0~6.0）")
    # 绒羽球靠 gen_plume 撑，骨架自身必须明显窄于成品球，否则羽层没地方长
    if hi[0] - lo[0] > 8.0:
        problems.append(f"骨架过宽：{hi[0] - lo[0]:.2f} 单位（应 <8，球宽由绒羽层负责）")

    neck_len = curve_length(NECK_KNOTS)
    print(f"骨骼 {len(rig.bones)} 根 · cube {len(rig.elements)} 个")
    print(f"站高 {hi[1]:.2f} 单位 = {hi[1] / 16:.2f} m · 体宽 {hi[0] - lo[0]:.2f} · "
          f"全长(喙尖→尾综骨) {hi[2] - lo[2]:.2f} = {(hi[2] - lo[2]) / 16:.2f} m")
    print(f"颈椎弧长 {neck_len:.2f} 单位 = {neck_len / 16:.2f} m（{NECK_VERTEBRAE} 节，"
          f"折叠后占纵向 {NECK_KNOTS[-1][1] - NECK_KNOTS[0][1]:.2f}）")
    print(f"龙骨腹缘 y={keel:.2f} · 背线 y={BACK_Y:.2f} · 躯干深 {BACK_Y - keel:.2f}")
    print(f"最低点 y={lo[1]:.2f}")

    orphans = rig.orphan_bones()
    if orphans:
        problems.append(f"空骨骼（无 element 无子骨）：{', '.join(orphans)}")

    if problems:
        print(f"\n✗ {len(problems)} 处违例：")
        for x in problems[:20]:
            print(f"   {x}")
        if len(problems) > 20:
            print(f"   …另 {len(problems) - 20} 处")
    else:
        print("\n✓ 对称 / 贴地 / 龙骨深度 / 骨架宽度 全部通过")
    return len(problems)


def main() -> int:
    ap = argparse.ArgumentParser(description="珂珂达骨架生成器")
    ap.add_argument("--part", choices=sorted(PARTS), help="只生成单个部件（预览用）")
    ap.add_argument("--list", action="store_true", help="列出部件")
    ap.add_argument("--check", action="store_true", help="只跑结构自检，不写文件")
    ap.add_argument("--out", type=Path)
    args = ap.parse_args()

    if args.check:
        return 1 if check(build_full()) else 0
    if args.list:
        for k, (label, _) in sorted(PARTS.items()):
            print(f"  {k:9s} {label}")
        return 0

    if args.part:
        rig = Rig(PALETTE)
        PARTS[args.part][1](rig)  # type: ignore[operator]
        name = f"Kekeda_{args.part}"
    else:
        rig = build_full()
        name = "KekedaSkeleton"

    out = rig.save(args.out or (OUT_DIR / f"{name}.bbmodel"), name)
    lo, hi = rig.bounds()
    print(f"→ {out}")
    print(f"   骨骼 {len(rig.bones)} 根 · cube {len(rig.elements)} 个")
    print(f"   站高 {hi[1]:.2f} 单位（{hi[1] / 16:.2f} m）· 纵长 {hi[2] - lo[2]:.2f} · 体宽 {hi[0] - lo[0]:.2f}")
    print(f"   最低点 y={lo[1]:.2f}（蹼应贴地 ≈0）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
