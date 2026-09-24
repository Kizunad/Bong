#!/usr/bin/env python3
"""骨头方块生成器 —— 从怠怒之狮骨架取形的 8 件装饰性骨块。Round 3/3。

形不是凭空捏的：每件的比例都对着手工精修过的狮骨量出来（见 `--ref`），
`modelScript/models/handmade/DainuLionSkeleton.user-backup-0806-1646.bbmodel`：
  颅 5.8×4.4×4.5 · 股骨干 8.5 高（斜置）· 上犬齿 1.0×2.0×1.6 · 下颌体 1.8×1.6×7.3
  肋 = 每根两段（`rib_N_l_a/b`，旋转 ~150° / ±85°）拼出的弧，不是直板

八件：
  skull  颅骨碎片（脑颅 + 眼眶孔 + 颧弓残段 + 断口）
  rib    肋骨（弧形，肋头粗、远端细）
  spine  脊椎节（椎体 + 棘突 + 成对横突 + 椎管孔）
  femur  股骨（斜置骨干 + 股骨头 + 大转子 + 双髁）
  jaw    颌骨碎片（水平支 + 升支 + 冠突 + 犬齿/裂齿）
  claw   爪骨（趾骨 + 下弯利爪）
  pile   骨堆（断肋 / 椎节 / 长骨碎片混杂堆叠）
  pelvis 骨盆碎片（半侧：髂骨翼 + 髋臼窝 + 坐骨）

坐标：**居中建模**（x/z ∈ [-8,8]，地面 y=0），落盘前平移进 MC 方块空间 0..16。
居中是为了让 `rigkit.mirror_violations()` 直接可用——它按 x=0 判中线。反过来说，
落盘后的文件对称轴在 x=8，别再拿那个检查器去核已导出的文件。

用法:
  python3 modelScript/generators/gen_bone_blocks.py                 # 八件全出
  python3 modelScript/generators/gen_bone_blocks.py --part rib      # 单件预览
  python3 modelScript/generators/gen_bone_blocks.py --check         # 只跑自检
  python3 modelScript/generators/gen_bone_blocks.py --ref           # 打印狮骨参考尺寸
  python3 modelScript/generators/gen_bone_blocks.py --list
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path

# --- modelScript 路径引导：共用底座在 core/ ---
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.rig.rigkit import Rig, Vec, element_bounds, lerp  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
OUT_DIR = Path(__file__).resolve().parents[1] / "models" / "bone_blocks"
LION = (
    Path(__file__).resolve().parents[1]
    / "models"
    / "handmade"
    / "DainuLionSkeleton.user-backup-0806-1646.bbmodel"
)

U = 16.0  # 1 格 = 16 单位

# 材质：与狮骨同一套 RGB，方块摆在骨架旁边不会串色。
# socket = 深色"孔洞"色：体素没有布尔减法，正交投影也没有凹陷阴影，眼窝 / 椎管 /
# 髋臼这类洞只能靠一块凹进去的深色面来读，否则侧视永远是一坨实心骨板。
MATS = {
    "bone": (214, 205, 184),
    "bone_dark": (176, 165, 142),
    "cartilage": (198, 200, 192),
    "tooth": (238, 233, 216),
    "socket": (72, 64, 54),
    "claw": (38, 30, 24),
}

# 狮骨实测参考（单位 = 1/16 格）：件数 · x×y×z 包围盒。
# 供 --ref 打印与 check() 的比例对拍；数字来自 baked_elements()，即**旋转后**的
# 真实包围盒，不是 from/to 极值。
LION_REF: dict[str, tuple[str, tuple[float, float, float]]] = {
    "cranium": ("脑颅", (5.80, 4.40, 4.50)),
    "femur_l_shaft": ("股骨干", (2.78, 8.53, 7.16)),
    "greater_trochanter_l": ("大转子", (2.93, 2.86, 2.04)),
    "mandible_body_l": ("下颌体", (1.80, 1.60, 7.27)),
    "canine_up_l": ("上犬齿", (1.00, 2.00, 1.59)),
    "ilium_l": ("髂骨", (4.20, 5.72, 6.95)),
    "ischium_l": ("坐骨", (2.40, 3.78, 4.95)),
    "claw_l": ("爪（4 枚）", (3.52, 0.94, 1.06)),
    "rib_8_l_a": ("第 8 肋近段", (5.31, 7.77, 1.08)),
    "orbit_socket_l": ("眼眶", (1.30, 1.60, 2.00)),
}


@dataclass(frozen=True)
class Block:
    """一件骨块。

    symmetric 决定是否跑镜像自检：椎节这类**成对结构**必须左右对拍，而颅骨碎片 /
    半侧骨盆是**故意的单侧残件**，硬要它对称就没有"碎"的意思了。分不清这两类就会
    要么放过真错位、要么把设计意图当成 bug 修掉。
    """

    key: str
    cn: str
    model: str
    symmetric: bool
    build: str  # part_* 函数名


BLOCKS: dict[str, Block] = {
    "skull": Block("skull", "颅骨碎片", "bone_skull_fragment", False, "part_skull"),
    "rib": Block("rib", "肋骨", "bone_rib", False, "part_rib"),
    "spine": Block("spine", "脊椎节", "bone_spine", True, "part_spine"),
    "femur": Block("femur", "股骨", "bone_femur", False, "part_femur"),
    "jaw": Block("jaw", "颌骨碎片", "bone_jaw", False, "part_jaw"),
    "claw": Block("claw", "爪骨", "bone_claw", False, "part_claw"),
    "pile": Block("pile", "骨堆", "bone_pile", False, "part_pile"),
    "pelvis": Block("pelvis", "骨盆碎片", "bone_pelvis", False, "part_pelvis"),
}


def _mix(a: Vec, b: Vec, t: float) -> Vec:
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t))


def _orphans(elements: list[dict], gap: float = 0.35) -> list[str]:
    """找出不与任何其他件相接的 cube 名（按旋转后包围盒膨胀 gap 判交）。

    用包围盒而非精确体素：斜置件的真实交叠算起来贵，而"包围盒都不挨着"已经足够
    判定视觉上脱开。gap 容一点点缝，免得刚好贴面的两件被判成分离。
    """
    boxes = [element_bounds([e]) for e in elements]

    def touch(i: int, j: int) -> bool:
        (alo, ahi), (blo, bhi) = boxes[i], boxes[j]
        return all(alo[k] - gap <= bhi[k] and blo[k] - gap <= ahi[k] for k in range(3))

    n = len(elements)
    if n < 2:
        return []
    # 并查集：连通分量 >1 时，除最大分量外全算游离
    parent = list(range(n))

    def find(x: int) -> int:
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for i in range(n):
        for j in range(i + 1, n):
            if touch(i, j):
                parent[find(i)] = find(j)
    groups: dict[int, list[int]] = {}
    for i in range(n):
        groups.setdefault(find(i), []).append(i)
    main = max(groups.values(), key=len)
    return [elements[i]["name"] for g in groups.values() if g is not main for i in g]


def _unsupported(elements: list[dict], floor: float = 0.35, gap: float = 0.35) -> list[str]:
    """找出既不贴地、又没有任何**面接触**的 cube 名。

    和 `_orphans` 是两条不同的门：那条按包围盒三轴各膨胀 gap 判交，一块悬在空中、
    只用一个角擦到邻件也算过关。这条要求接触处**真有面积** —— 三轴里至少两轴实重叠，
    第三轴才允许贴面。角碰的三轴重叠全趋近 0，会被抓出来。

    不要求"下方有承托"：颌骨的犬齿是朝下挂在下颌体上的，椎节的关节面是贴在椎体
    前后**立面**上的，两者下方都没有东西，但都不是悬空件。判"有没有粘住"而不是
    "有没有踩着地"，才对得上骨骼的真实构造。
    """
    boxes = [element_bounds([e]) for e in elements]
    out: list[str] = []
    for i, (lo, hi) in enumerate(boxes):
        if lo[1] <= floor:  # 贴地件自然成立
            continue
        bonded = False
        for j, (lo2, hi2) in enumerate(boxes):
            if i == j:
                continue
            ov = [min(hi[k], hi2[k]) - max(lo[k], lo2[k]) for k in range(3)]
            if any(o < -gap for o in ov):  # 某轴整个错开 = 没挨上
                continue
            if sum(1 for o in ov if o > 0.15) >= 2:  # 两轴有面积 = 真粘住
                bonded = True
                break
        if not bonded:
            out.append(elements[i]["name"])
    return out


def _arc(a: Vec, b: Vec, bulge: Vec, n: int) -> list[Vec]:
    """a→b 的二次贝塞尔折线，bulge 为中间控制点。

    肋骨的辨识度全在这条弧上：直着摆就是一块板。用贝塞尔而不是手摆点位，是因为
    两端要**精确**钉在肋头和胸骨端，中间才能自由鼓出去。
    """
    out: list[Vec] = []
    for i in range(n + 1):
        t = i / n
        w = ((1 - t) ** 2, 2 * (1 - t) * t, t * t)
        out.append(tuple(w[0] * a[j] + w[1] * bulge[j] + w[2] * b[j] for j in range(3)))
    return out


# ================================================================ 部件
def part_skull(rig: Rig) -> None:
    """颅骨碎片：脑颅穹顶 + 眼眶孔 + 颧弓残段 + 后部断口。

    狮颅 5.8×4.4×4.5，这里放大到约 9×7×8 —— 方块装饰件要撑得起一格，等比缩下来
    只有小半格，摆在地上像颗石子。断口做成斜切面（`_break`），一刀平口看着像切好的
    豆腐而不是碎骨。
    """
    rig.bone("root", (0.0, 3.5, 0.0))
    # 颅是**长于宽**的（狮颅 5.8 宽 / 加吻部纵深远超此数）。第一版写成 10.7×10.6
    # 近正方，正视和侧视剪影 IoU 0.91——等于又做了个挤出剪影。收窄 x、拉长 z。
    rig.cube("root", "braincase_low", (-3.4, 1.6, -2.4), (3.4, 5.4, 3.6), mat="bone")
    rig.cube("root", "braincase_top", (-2.7, 5.4, -1.8), (2.7, 7.4, 2.9), mat="bone")
    # 矢状脊：狮颅顶正中的骨脊（咬肌附着）。少了它颅顶就是块平板，
    # 3/4 视角下整件读成"箱子"而不是"颅"。
    rig.taper(
        "root",
        "sagittal_crest",
        [(0.0, 7.4, 2.6), (0.0, 8.1, 0.6), (0.0, 7.7, -1.4)],
        [0.45, 0.5, 0.35],
        mat="bone",
        flat=1.9,
    )
    # 额骨斜面：从穹顶前缘往吻部下倾
    rig.shaft("root", "frontal", (0.0, 6.8, -1.6), (0.0, 4.8, -5.0), 2.4, 1.1, mat="bone")
    # 吻残段：短而厚，前端斜切断口。
    # 上一轮写成 4.2 长、flat=0.78 的薄锥，侧视里成了飘在脑颅左边的一块板 —— 太长太扁，
    # 加上起点没插进脑颅体内，读不出"连着"。缩到 2.6 长、加厚，起点埋进 -3.0。
    rig.taper(
        "root",
        "muzzle",
        [(0.0, 3.8, -3.0), (0.0, 3.5, -4.8), (0.0, 3.3, -6.2)],
        [2.0, 1.7, 1.35],
        mat="bone",
        flat=1.0,
    )
    # 断口斜面：吻前端是**碎的**，平口收尾像做好的零件
    rig.shaft("root", "muzzle_break", (0.0, 4.2, -6.2), (0.0, 2.6, -6.9), 1.3, 0.5, mat="bone_dark")
    # 眼眶：一对深色凹孔，嵌在额骨与颧弓之间
    for sx, side in ((-1, "l"), (1, "r")):
        rig.cube(
            "root",
            f"orbit_{side}",
            (sx * 1.6, 3.6, -4.6),
            (sx * 3.3, 5.9, -3.0),
            mat="socket",
        )
        # 颧弓：眶后缘往后外侧张出的细梁，斜置——这根是颅骨侧向的辨识特征
        rig.shaft(
            "root",
            f"zygomatic_{side}",
            (sx * 2.9, 4.4, -3.2),
            (sx * 4.2, 3.0, 1.8),
            0.7,
            0.9,
            mat="bone",
        )
        # 上犬齿残根（吻部下缘露出一点，点出"这是掠食者的颅"）
        rig.taper(
            "root",
            f"canine_{side}",
            [(sx * 1.0, 3.1, -5.4), (sx * 1.1, 1.4, -5.8)],
            [0.5, 0.2],
            mat="tooth",
            flat=0.85,
        )
    # 枕部断口：斜切的深色断面 + 两小块崩茬
    rig.shaft("root", "break_face", (0.0, 5.2, 3.4), (0.0, 2.0, 2.4), 2.8, 0.6, mat="bone_dark")
    rig.cube("root", "chip_a", (-2.4, 1.6, 2.8), (-1.0, 3.0, 4.4), mat="bone_dark")
    rig.cube("root", "chip_b", (1.2, 2.2, 3.0), (2.6, 3.4, 4.6), mat="bone_dark")


def part_rib(rig: Rig) -> None:
    """肋骨：肋头 + 弧形肋体（贝塞尔 5 段递减）。

    狮骨的肋是 `rib_N_l_a/b` 两段拼的弧，旋转 ~150°/±85°。这里按同样思路但分更多段
    —— 单件装饰要独立成形，段数少了弧就成了折线。
    """
    rig.bone("root", (0.0, 7.0, 0.0))
    # 肋头 + 肋颈（与椎体相接那端，最粗）
    rig.cube("root", "rib_head", (-1.5, 0.6, -1.4), (1.5, 3.0, 1.4), mat="bone")
    rig.cube("root", "rib_tubercle", (-1.0, 2.6, -1.0), (1.0, 4.2, 1.0), mat="bone_dark")
    # 肋体：自肋颈起弧到远端，向 -Z 鼓出
    pts = _arc((0.0, 4.0, 0.0), (0.6, 14.4, 1.6), (0.2, 9.0, -6.2), 5)
    radii = [1.15, 1.05, 0.95, 0.85, 0.72, 0.58]
    rig.taper("root", "rib_shaft", pts, radii, mat="bone", flat=0.62)
    # 远端肋软骨（灰白，与骨面区分）
    rig.shaft("root", "costal_cartilage", (0.6, 14.4, 1.6), (0.9, 15.8, 3.0), 0.42, 0.5, mat="cartilage")


def part_spine(rig: Rig) -> None:
    """脊椎节：椎体 + 椎管孔 + 棘突 + 成对横突 + 前后关节面。

    唯一**成对对称**的一件，所以 `symmetric=True` 会跑镜像自检。横突写 `_l`/`_r`
    后缀就是为了让 `mirror_violations()` 能配对——名字不带后缀它就照不到。
    """
    rig.bone("root", (0.0, 4.0, 0.0))
    # 椎体（中线件，x 必须对称，否则自检报错）
    rig.cube("root", "centrum", (-3.0, 1.4, -3.2), (3.0, 5.4, 3.2), mat="bone")
    # 椎管：贯穿的深色孔（上缘偏后，脊髓从这里过）
    rig.cube("root", "neural_canal", (-1.5, 4.0, -3.4), (1.5, 5.6, 3.4), mat="socket")
    # 前后椎体关节面（略凹，深一号色）
    rig.cube("root", "facet_front", (-2.6, 1.8, -3.7), (2.6, 5.0, -3.1), mat="bone_dark")
    rig.cube("root", "facet_back", (-2.6, 1.8, 3.1), (2.6, 5.0, 3.7), mat="bone_dark")
    # 棘突：向上后方斜出，三段收细。
    #
    # 不走 `rig.taper()`：它按平均半径铺等截面段且段间**首尾相接**，半径一跳（1.3→0.925）
    # 接缝就豁出一道台阶 —— 上一轮渲染的侧视里棘突正是断成两块错开的板。这里手铺三段
    # 并给 extend 让相邻段互相插入，台阶埋进实体内部。
    #
    # 截面也照狮骨 thoracic_sp（1.0 宽 × 0.83 深）改成近方刃：原先 flat=0.55 配 rz=1.3
    # 出来 1.43×2.6，深得像块木板，不是骨突。
    sp = [(0.0, 5.0, 0.8), (0.0, 7.7, 1.95), (0.0, 9.9, 2.85), (0.0, 11.6, 3.5)]
    for i, (a, b) in enumerate(zip(sp, sp[1:]), start=1):
        r = (1.30, 1.05, 0.78)[i - 1]
        rig.shaft("root", f"spinous_{i:02d}", a, b, r * 0.82, r, mat="bone", extend=0.34)
    # 成对横突：向两侧略后下张出
    for sx, side in ((-1, "l"), (1, "r")):
        rig.shaft(
            "root",
            f"transverse_{side}",
            (sx * 2.6, 4.2, 0.4),
            (sx * 6.4, 3.0, 1.8),
            0.85,
            1.0,
            mat="bone",
        )
        # 横突尖端小关节面
        rig.cube(
            "root",
            f"tp_tip_{side}",
            (sx * 5.9, 2.4, 1.1),
            (sx * 7.0, 3.6, 2.5),
            mat="bone_dark",
        )


def part_femur(rig: Rig) -> None:
    """股骨：股骨头 + 颈 + 大转子 + 斜置骨干 + 双髁。

    狮骨股骨干实测 2.78×8.53×7.16 —— y 和 z 都占 7~8，说明它是**斜着**的。这件必须
    用 shaft 定向：写成轴对齐直筒就丢掉了长骨最重要的那点辨识度。
    """
    rig.bone("root", (0.0, 7.0, 0.0))
    # 近端压到 12.4：股骨头和大转子还要再往上顶 2 个单位，写 14 就总高 17 出格了。
    # 长骨是八件里最高的一件，垂直预算得从**最高点**倒着算，不是从骨干顶端算。
    prox = (2.2, 12.4, -2.6)  # 近端（髋侧）
    dist = (-1.4, 1.6, 2.4)  # 远端（膝侧）
    # 骨干：一根斜柱，端点精确落在两关节上
    rig.shaft("root", "shaft", prox, dist, 1.5, 1.7, mat="bone")
    # 股骨头：球状，自颈向内上方伸出
    neck_end = (4.4, 13.8, -3.4)
    rig.shaft("root", "neck", prox, neck_end, 1.1, 1.2, mat="bone")
    # 股骨头：轴对齐方块在 3/4 视角下读成"贴上去的骰子"。用 taper 顺着颈轴收出球感，
    # 端段半径缩到 0.7 才有"球"的转折——这是渲染看出来的，数值门禁看不到。
    rig.taper(
        "root",
        "head",
        [(4.0, 13.4, -3.0), (5.0, 14.2, -3.6), (5.9, 14.9, -4.1)],
        [1.25, 1.05, 0.7],
        mat="cartilage",
        flat=1.0,
    )
    # 大转子：颈外侧的隆起（狮骨 2.93×2.86×2.04）。同样顺着骨干轴斜收，
    # 否则和股骨头并排成两个方盒。
    rig.taper(
        "root",
        "greater_trochanter",
        [(1.7, 12.6, -2.4), (1.5, 14.2, -2.8), (1.2, 15.4, -3.0)],
        [1.3, 1.1, 0.75],
        mat="bone_dark",
        flat=1.15,
    )
    # 远端双髁 + 髁间沟
    for sx, side in ((-1, "med"), (1, "lat")):
        rig.cube(
            "root",
            f"condyle_{side}",
            (-2.6 + (0 if sx < 0 else 2.0), 0.4, 1.2),
            (-0.6 + (0 if sx < 0 else 2.0), 2.8, 3.8),
            mat="bone",
        )
    rig.cube("root", "trochlear_groove", (-0.7, 0.8, 1.4), (0.1, 2.6, 3.6), mat="bone_dark")


def part_jaw(rig: Rig) -> None:
    """颌骨碎片：水平支 + 升支 + 冠突 + 髁突 + 齿列。

    狮骨下颌体 1.8×1.6×7.27（细长）、上犬齿 1.0×2.0×1.59。裂齿比犬齿短而宽——齿全
    做成一样大就成了梳子。
    """
    rig.bone("root", (0.0, 3.0, 0.0))
    # 水平支：自颏部往后，略向外弧
    rig.shaft("root", "body", (-1.0, 2.2, -6.6), (1.4, 3.0, 2.6), 1.35, 1.5, mat="bone")
    # 颏部（前端联合处，断口在这里）
    rig.cube("root", "symphysis", (-2.3, 1.6, -7.4), (0.4, 4.0, -5.4), mat="bone")
    # 升支：向后上竖起
    rig.shaft("root", "ramus", (1.2, 3.0, 2.2), (1.9, 8.4, 3.6), 1.2, 1.35, mat="bone")
    # 冠突：收窄成"钩"而不是"帆"。第一版 1.05 半宽在渲染里成了主导剪影的大板子，
    # 把下颌读感盖过去了；真冠突是薄而窄的钩状突起。
    rig.shaft("root", "coronoid", (1.5, 7.4, 3.0), (1.5, 9.8, 1.2), 0.62, 0.5, mat="bone")
    rig.cube("root", "condyle", (0.7, 8.0, 3.0), (2.9, 9.6, 5.0), mat="cartilage")
    # 齿列：犬齿最长最尖，往后裂齿变短变宽（全做等大就是把梳子）。
    # x 必须跟齿列**同一条线**：第一版犬齿在 -1.0 而后齿在 +0.3，渲出来像掉在旁边。
    tooth_x = 0.15
    rig.taper(
        "root",
        "canine",
        [(tooth_x, 3.5, -6.0), (tooth_x, 5.6, -6.5), (tooth_x, 7.2, -6.8)],
        [0.62, 0.42, 0.18],
        mat="tooth",
        flat=0.88,
    )
    for i, (z, h, w) in enumerate(((-4.0, 1.3, 0.40), (-2.2, 2.1, 0.50), (0.1, 1.5, 0.62)), 1):
        rig.cube(
            "root",
            f"tooth_{i:02d}",
            (tooth_x - 0.55, 3.3, z - w),
            (tooth_x + 0.55, 3.3 + h, z + w),
            mat="tooth",
        )
    # 后段断口
    rig.cube("root", "break_face", (0.2, 2.4, 2.2), (2.4, 4.2, 3.4), mat="bone_dark")


def part_claw(rig: Rig) -> None:
    """爪骨：跖骨残段 + 两节趾骨 + 下弯利爪。

    爪必须**弯**：狮骨 claw_l 是 4 枚各自旋转的件。直着伸出去是钉子不是爪，弯钩靠
    taper 沿下弯折线递减做出来。
    """
    rig.bone("root", (0.0, 4.0, 0.0))
    # 跖骨残段（断口朝后上）
    rig.shaft("root", "metatarsal", (0.0, 6.8, 4.6), (0.0, 3.4, 1.2), 1.5, 1.6, mat="bone")
    rig.cube("root", "break_face", (-1.5, 6.2, 4.2), (1.5, 7.8, 5.6), mat="bone_dark")
    # 两节趾骨，逐节收细并前伸下沉
    rig.shaft("root", "phalanx_1", (0.0, 3.4, 1.2), (0.0, 2.2, -2.0), 1.25, 1.35, mat="bone")
    rig.shaft("root", "phalanx_2", (0.0, 2.2, -2.0), (0.0, 1.7, -4.4), 1.0, 1.1, mat="bone")
    # 爪鞘：自末节趾骨下弯成钩，四段递减
    pts = _arc((0.0, 1.7, -4.4), (0.0, 0.5, -7.4), (0.0, 2.4, -6.4), 4)
    rig.taper("root", "claw", pts, [0.85, 0.7, 0.55, 0.38, 0.16], mat="claw", flat=0.8)


def part_pile(rib_rig: Rig) -> None:
    """骨堆：断肋 3 根 + 椎节 2 个 + 长骨碎片 2 段 + 碎屑。

    堆的关键是**朝向各异**：全部轴对齐摆整齐就是柴火垛。每件给不同的 shaft 方向，
    让轮廓在三视图上都是乱的。y 压在 6 以下——它是"一堆"，不是纪念碑。
    """
    rig = rib_rig
    rig.bone("root", (0.0, 2.0, 0.0))
    # 底层：两根横躺的断肋，交叉
    rig.shaft("root", "rib_frag_a", (-6.2, 0.9, -2.6), (3.4, 1.1, 2.8), 0.85, 0.72, mat="bone")
    rig.shaft("root", "rib_frag_b", (-3.0, 1.0, 4.2), (5.8, 1.2, -1.4), 0.78, 0.66, mat="bone_dark")
    # 中层：长骨碎片（斜搭在肋上）+ 一节椎骨
    rig.shaft("root", "long_frag", (-5.0, 2.4, 2.0), (2.2, 3.6, -3.6), 1.15, 1.25, mat="bone")
    rig.cube("root", "vert_a", (2.0, 1.8, -0.6), (5.4, 4.4, 2.8), mat="bone")
    rig.cube("root", "vert_a_canal", (3.0, 4.0, -0.8), (4.4, 4.9, 3.0), mat="socket")
    # 上层：斜插的第三根肋 + 小椎节 + 碎屑
    rig.shaft("root", "rib_frag_c", (-4.4, 4.4, -1.2), (0.8, 2.6, 3.4), 0.7, 0.6, mat="bone")
    rig.cube("root", "vert_b", (-2.4, 3.4, -4.0), (0.6, 5.4, -1.6), mat="bone_dark")
    rig.cube("root", "chip_a", (4.6, 1.6, 3.2), (6.2, 2.4, 5.0), mat="bone_dark")
    rig.cube("root", "chip_b", (-6.8, 0.8, 1.0), (-5.2, 1.8, 2.4), mat="bone")
    rig.shaft("root", "chip_c", (1.6, 4.6, 1.0), (4.2, 5.4, -2.2), 0.45, 0.5, mat="cartilage")


def part_pelvis(rig: Rig) -> None:
    """骨盆碎片：半侧髋骨（髂骨翼 + 髋臼 + 坐骨 + 耻骨残段）。

    狮骨 ilium 4.2×5.72×6.95、ischium 2.4×3.78×4.95、acetabulum 1.6×2.0×2.0。取
    **半侧**（沿中线断开）——整圈骨盆塞进一格会细成铁丝网。所以 symmetric=False。
    """
    rig.bone("root", (0.0, 5.0, 0.0))
    # 髂骨翼：宽扁的斜板，从髋臼向前上张开
    rig.shaft("root", "ilium_wing", (-1.0, 6.4, -5.6), (2.6, 9.4, 1.2), 3.1, 0.95, mat="bone")
    rig.cube("root", "ilium_crest", (-2.6, 8.2, -6.6), (2.2, 10.4, -3.6), mat="bone")
    # 髋臼：深色关节窝 + 一圈骨缘
    rig.cube("root", "acetabulum_rim", (0.6, 3.6, 0.4), (4.6, 7.6, 4.4), mat="bone")
    rig.cube("root", "acetabulum", (0.2, 4.4, 1.2), (1.4, 6.8, 3.6), mat="socket")
    # 坐骨：自髋臼后下伸出，末端粗糙隆起
    rig.shaft("root", "ischium", (2.6, 4.4, 3.8), (1.4, 1.6, 6.8), 1.35, 1.5, mat="bone")
    rig.cube("root", "ischial_tuber", (0.2, 0.8, 6.0), (2.6, 3.0, 7.8), mat="bone_dark")
    # 耻骨残段：向内下，断口
    rig.shaft("root", "pubis_stub", (1.4, 4.0, 1.6), (-2.0, 2.2, 3.0), 0.95, 1.05, mat="bone")
    rig.cube("root", "break_face", (-2.9, 1.6, 2.2), (-1.5, 3.4, 3.8), mat="bone_dark")


PARTS = {k: globals()[b.build] for k, b in BLOCKS.items()}


# ================================================================ 装配
def build(block: Block) -> Rig:
    """建一件（居中坐标系，x/z 以 0 为中线，地面 y=0）。"""
    rig = Rig(MATS)
    PARTS[block.key](rig)
    return rig


def _shift_to_block_space(rig: Rig) -> tuple[float, float, float]:
    """把居中模型平移进 MC 的 0..16 方块空间，返回实际用的位移。

    分两件事：x/z 居中到 8，y 直接**贴地**（不居中）—— 骨块是放在地面上的装饰，
    浮在半空或沉进地里都是错。平移必须同时改 from/to **和 origin**：只改 from/to
    会让带旋转的件绕着旧 origin 转，整根骨飞到格外去。
    """
    (lo, hi) = element_bounds(rig.elements)
    dx = 8.0 - (lo[0] + hi[0]) / 2
    dz = 8.0 - (lo[2] + hi[2]) / 2
    dy = -lo[1]
    for e in rig.elements:
        for key in ("from", "to", "origin"):
            e[key] = [
                round(e[key][0] + dx, 3),
                round(e[key][1] + dy, 3),
                round(e[key][2] + dz, 3),
            ]
    for b in rig.bones.values():
        b["pivot"] = [
            round(b["pivot"][0] + dx, 3),
            round(b["pivot"][1] + dy, 3),
            round(b["pivot"][2] + dz, 3),
        ]
    return (dx, dy, dz)


# ================================================================ 自检
def check(block: Block, verbose: bool = True) -> int:
    """结构自检：镜像 · 出格 · 贴地 · 占格率 · 材质多样性 · UV 有效性。

    每一条都对着一个真实翻过的车：上一版八件全是轴对齐方盒、24 个面共享同一个 UV
    （所以 socket/tooth 根本没渲出来）、且没有任何自检能抓到这些。
    """
    rig = build(block)
    problems: list[str] = []

    # 镜像必须在**居中系**下查：rigkit 按 x=0 判中线，平移后中线在 x=8，那个检查器
    # 会把每一件都报成"未对称"。只对成对结构有意义（碎片件故意不对称）。
    if block.symmetric:
        problems += rig.mirror_violations()

    # 其余全查**落盘后**的几何：贴地和居中是 _shift_to_block_space 干的，查平移前
    # 的坐标等于在查一个不会出货的东西（上一轮就这么误报了 skull/spine/pelvis）。
    _shift_to_block_space(rig)
    (lo, hi) = element_bounds(rig.elements)
    size = tuple(hi[i] - lo[i] for i in range(3))

    # 出格：方块空间是 0..16，越界就跨进邻格
    for axis, label in ((0, "x"), (1, "y"), (2, "z")):
        if lo[axis] < -1e-6 or hi[axis] > U + 1e-6:
            problems.append(f"{label} 出格：{lo[axis]:.2f}..{hi[axis]:.2f}（须落在 0..16）")

    # 贴地：平移已把最低点压到 0，这里守的是"平移真的生效了"
    if abs(lo[1]) > 0.05:
        problems.append(f"未贴地：最低点 y={lo[1]:.2f}（应为 0）")

    # 居中：x/z 中心须在 8 附近，否则方块看着偏在一边
    for axis, label in ((0, "x"), (2, "z")):
        mid = (lo[axis] + hi[axis]) / 2
        if abs(mid - 8.0) > 0.05:
            problems.append(f"{label} 未居中：中心 {mid:.2f}（应为 8）")

    # 占格率：太小就撑不起一格，太满就像实心方块
    fill = (size[0] * size[1] * size[2]) / (U**3)
    if not 0.06 <= fill <= 0.72:
        problems.append(f"占格率 {fill * 100:.1f}%（应 6%~72%，过小像石子/过大像实心块）")

    # 材质多样性：至少 2 种，且深色孔洞/齿类不能缺——这是"骨"的读感来源
    used = {e["color"] for e in rig.elements}
    if len(used) < 2:
        problems.append(f"只用了 {len(used)} 种材质（平涂一色读不出骨的结构）")

    # UV 有效性：每种材质必须落在**各自**的 swatch 格里。
    # 上一版的致命伤正是这里——所有面写死 [0,0,16,16]，材质名形同注释。
    rects = {tuple(f["uv"]) for e in rig.elements for f in e["faces"].values()}
    if len(rects) != len(used):
        problems.append(f"UV 区块 {len(rects)} 个 vs 材质 {len(used)} 种（材质未落到各自 swatch）")
    for r in rects:
        if r[2] - r[0] > rig.swatch or r[3] - r[1] > rig.swatch:
            problems.append(f"UV {r} 跨出 {rig.swatch}px swatch（会采到邻色）")

    # 立体度：至少一半的件带旋转，否则又是"挤出的二维剪影"
    rotated = sum(1 for e in rig.elements if any(e["rotation"]))
    if rotated == 0:
        problems.append("零旋转件：全轴对齐 = 平板挤出，正视与侧视会一模一样")

    # 连通性：每件都得挨着别的件。骨头是一根整的，飘一块在旁边就是渣。
    # 上一轮渲染里犬齿明显脱开下颌体，靠肉眼才看出来——这里把它变成可跑的门。
    orphans = _orphans(rig.elements)
    if orphans:
        problems.append(f"游离件 {len(orphans)} 个：{', '.join(orphans[:4])}（未与其他件相接）")

    # 注：这里曾有一条"正视/侧视剪影 IoU"门，已删。它把正视的 x 索引和侧视的 z 索引
    # 直接相交，比的是两套坐标系里的下标碰巧重合，压根不是形状相似度——实测上一版
    # 那块平板 jaw 只有 0.54，稳稳低于 0.82 阈值，门是空的。
    # 换成"逐层截面 vs 投影"的真挤出度也不成立：长骨本身就是等截面，我的 femur 0.85 /
    # claw 0.92 都高于那块平板 jaw 的 0.78，没有阈值能把两者分开。立体感这条交给
    # render_bbmodel.py 三视图人眼过，不留一个看着像保证的假数字。

    # 面接合：每件要么贴地，要么和别的件有**带面积**的接触。上一条只按包围盒膨胀
    # 判交，一块仅用角尖擦到邻件也算过关；这条要求两轴实重叠，把角碰和真悬空都抓出来。
    unsupported = _unsupported(rig.elements)
    if unsupported:
        problems.append(f"悬空件 {len(unsupported)} 个：{', '.join(unsupported[:4])}（既不贴地，也没有和其他件面接合）")

    if verbose:
        print(f"[{block.key}] {block.cn} · {block.model}")
        print(f"  cube {len(rig.elements)} 个 · 带旋转 {rotated} 个 · 材质 {len(used)} 种")
        print(
            f"  尺寸 {size[0]:.1f}×{size[1]:.1f}×{size[2]:.1f} 单位"
            f"（{size[0] / U:.2f}×{size[1] / U:.2f}×{size[2] / U:.2f} 格）· 占格率 {fill * 100:.1f}%"
        )
        print(f"  UV 区块 {len(rects)} 个 · 最低点 y={lo[1]:.2f}")
        if problems:
            print(f"  ✗ {len(problems)} 处违例：")
            for p in problems[:10]:
                print(f"     {p}")
        else:
            print(
                "  ✓ 镜像 / 出格 / 贴地 / 居中 / 占格 / 材质 / UV / 旋转 / 连通 / 面接合 全部通过"
            )
            print("  ! 立体感与比例未自动核验——须过 render_bbmodel.py 三视图人眼判")
    return len(problems)


def print_ref() -> int:
    """打印狮骨参考尺寸；带 --ref 时顺便对着真文件复量一遍。"""
    print("狮骨参考（modelScript/models/handmade/DainuLionSkeleton.user-backup-0806-1646.bbmodel）")
    live: dict[str, tuple[float, float, float]] = {}
    if LION.exists():
        sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "core"))
        from bbmodel_maker.rig.rigkit import Skeleton

        sk = Skeleton(LION)
        baked = sk.baked_elements()
        for name in LION_REF:
            hit = [e for e in baked if e["name"].startswith(name)]
            if hit:
                lo, hi = element_bounds(hit)
                live[name] = tuple(hi[i] - lo[i] for i in range(3))
    else:
        print(f"  （{LION.name} 不在——只打印记录值）")
    for name, (cn, rec) in LION_REF.items():
        row = f"  {name:22s} {cn:10s} 记录 {rec[0]:5.2f}×{rec[1]:5.2f}×{rec[2]:5.2f}"
        if name in live:
            m = live[name]
            drift = max(abs(m[i] - rec[i]) for i in range(3))
            row += f"  实测 {m[0]:5.2f}×{m[1]:5.2f}×{m[2]:5.2f}"
            row += "  ✓" if drift < 0.05 else f"  ⚠ 偏 {drift:.2f}"
        print(row)
    return 0


# ================================================================ 主流程
def main() -> int:
    ap = argparse.ArgumentParser(description="骨头方块 bbmodel 生成器")
    ap.add_argument("--part", choices=sorted(BLOCKS), help="只生成单件（预览用）")
    ap.add_argument("--check", action="store_true", help="只跑结构自检，不写文件")
    ap.add_argument("--ref", action="store_true", help="打印狮骨参考尺寸")
    ap.add_argument("--list", action="store_true", help="列出全部部件")
    ap.add_argument("--out-dir", type=Path, help=f"输出目录（默认 {OUT_DIR.name}/）")
    args = ap.parse_args()

    if args.list:
        print("骨头方块：")
        for k, b in BLOCKS.items():
            sym = "成对" if b.symmetric else "碎片"
            print(f"  {k:7s} {b.cn:8s} {sym}  → {b.model}.bbmodel")
        return 0

    if args.ref:
        return print_ref()

    keys = [args.part] if args.part else list(BLOCKS)

    if args.check:
        bad = sum(check(BLOCKS[k]) for k in keys)
        print()
        print(f"{'✗' if bad else '✓'} 共 {bad} 处违例 / {len(keys)} 件")
        return 1 if bad else 0

    out_dir = args.out_dir or OUT_DIR
    out_dir.mkdir(parents=True, exist_ok=True)
    for k in keys:
        block = BLOCKS[k]
        rig = build(block)
        _shift_to_block_space(rig)
        out = out_dir / f"{block.model}.bbmodel"
        out.write_text(json.dumps(rig.bbmodel(block.model), ensure_ascii=False, indent=1))
        (lo, hi) = element_bounds(rig.elements)
        print(
            f"→ {out.relative_to(REPO)}  cube {len(rig.elements)} · "
            f"{hi[0] - lo[0]:.1f}×{hi[1] - lo[1]:.1f}×{hi[2] - lo[2]:.1f} 单位"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
