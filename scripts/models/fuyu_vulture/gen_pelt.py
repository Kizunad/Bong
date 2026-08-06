#!/usr/bin/env python3
"""腐羽鹫 —— 羽毛层（Round 1/3）。三档 × 三变色 = 9 个外观。

**读**肌肉层 bbmodel，在最外面铺羽。羽毛是这只鸟真正被看见的部分：骨和肌决定形状，
羽决定轮廓和颜色。

鸟的"皮毛"不是均匀一层绒 —— 羽只长在**羽区（pterylae）**，中间隔着**裸区
（apteria）**。而秃鹫最标志的特征恰恰是**不长羽的地方**：

  **头颈裸露**。把头插进尸腔掏内脏，羽毛会糊满腐肉再也梳不干净 —— 所以头颈退成
  一段皱皮。给秃鹫的头颈铺上羽，它就变成了一只普通猛禽。
  **颈基一圈领羽（ruff）**。裸颈与体羽的交界立起一圈蓬松的羽领，像围脖。它是
  "秃鹫感"的另一半：光溜的脖子从一团炸开的羽毛里伸出来。
  **跗跖裸露**，只有鳞。

翼面全部由羽构成，骨只到腕：
  初级飞羽（primaries）长在腕掌骨与指骨上，最长，构成翼尖；
  次级飞羽（secondaries）长在尺骨上（骨架层那排羽茎瘤就是它们的根）；
  覆羽（coverts）层层叠在飞羽根部，把翼前半段铺平。
展翼时这三组才是翼的实体 —— 骨架只占翼面的前缘一条。

三种变色按栖息地分（末法残土的同族异色）：
  烬羽 jin  灰烬带常见型，炭灰压深
  锈斑 xiu  血谷型，铁锈红褐
  枯白 ku   北境荒原 / 老年个体，褪成骨白

用法:
  python3 scripts/models/fuyu_vulture/gen_pelt.py                    # 9 个全出
  python3 scripts/models/fuyu_vulture/gen_pelt.py --size mid --morph xiu
  python3 scripts/models/fuyu_vulture/gen_pelt.py --size mid --pose spread
  python3 scripts/models/fuyu_vulture/gen_pelt.py --size mid --only-pelt   # 只看羽
  python3 scripts/models/fuyu_vulture/gen_pelt.py --check
  python3 scripts/models/fuyu_vulture/gen_pelt.py --list
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from gen_muscle import Body  # noqa: E402
from gen_skeleton import SPECS  # noqa: E402
from rigkit import (  # noqa: E402
    Skeleton, SoftTissue, Vec, element_bounds, lerp, mirror_violations, normalize, perp_to,
)

REPO = HERE.parents[2]
MODELS = REPO / "local_models" / "fuyu_vulture"

# 羽毛材质追加在贴图第 3 行（第 1 行骨、第 2 行肌，都保持原位不动）。
PELT_ROW = 2

# ---------------------------------------------------------------- 三种变色
# 键必须完全一致（顺序也一致）—— UV 按索引取，换个变色只换颜色不换坐标。
MORPHS: dict[str, dict] = {
    "jin": {
        "cn": "烬羽",
        "note": "灰烬带常见型，炭灰压深",
        "colors": {
            "feather_body": (78, 74, 71),
            "feather_covert": (98, 93, 88),
            "feather_flight": (46, 44, 44),
            "feather_tail": (54, 51, 51),
            "feather_ruff": (134, 127, 116),
            "feather_dark": (31, 29, 29),
            "skin_bare": (152, 120, 108),
            "scale_leg": (86, 79, 71),
        },
    },
    "xiu": {
        "cn": "锈斑",
        "note": "血谷型，铁锈红褐",
        "colors": {
            "feather_body": (98, 60, 42),
            "feather_covert": (120, 76, 50),
            "feather_flight": (58, 36, 26),
            "feather_tail": (68, 43, 31),
            "feather_ruff": (154, 120, 86),
            "feather_dark": (40, 25, 18),
            "skin_bare": (170, 112, 96),
            "scale_leg": (104, 69, 45),
        },
    },
    "ku": {
        "cn": "枯白",
        "note": "北境荒原 / 老年个体，褪成骨白",
        "colors": {
            "feather_body": (168, 160, 147),
            "feather_covert": (187, 179, 165),
            "feather_flight": (110, 104, 98),
            "feather_tail": (122, 116, 108),
            "feather_ruff": (207, 201, 189),
            "feather_dark": (79, 75, 71),
            "skin_bare": (196, 168, 152),
            "scale_leg": (151, 143, 131),
        },
    },
}
MAT_KEYS = tuple(MORPHS["jin"]["colors"])

TRACTS = ("ruff", "body", "wing", "tail", "leg", "bare")
TRACT_LABEL = {
    "ruff": "Ruff: 颈基领羽（裸颈与体羽的交界）",
    "body": "Body: 体羽（背 / 胸 / 腹侧层叠）",
    "wing": "Wing: 初级 / 次级飞羽 + 覆羽",
    "tail": "Tail: 尾羽扇",
    "leg": "Leg: 腿羽 + 跗跖鳞",
    "bare": "Bare: 裸头裸颈的皱皮",
}


def _mix(a: Vec, b: Vec, t: float) -> Vec:
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t))


def _off(p: Vec, dx: float = 0.0, dy: float = 0.0, dz: float = 0.0) -> Vec:
    return (p[0] + dx, p[1] + dy, p[2] + dz)


def _step(p: Vec, d: Vec, length: float) -> Vec:
    n = normalize(d)
    return (p[0] + n[0] * length, p[1] + n[1] * length, p[2] + n[2] * length)


def _len(a: Vec, b: Vec) -> float:
    return math.sqrt(sum((x - y) ** 2 for x, y in zip(a, b)))


def _trailing(a: Vec, b: Vec, el: Vec, ref: Vec) -> Vec:
    """翼轴 a→b 的**后缘**方向：翼面内、垂直于轴、指向肘所在的那一侧。

    飞羽全长在后缘上；取反了羽就从翼的前面长出来，翼变成一把倒着的梳子。
    """
    v = perp_to(a, b, ref)
    mid = _mix(a, b, 0.5)
    if sum(v[i] * (el[i] - mid[i]) for i in range(3)) < 0:
        v = (-v[0], -v[1], -v[2])
    return v


# ================================================================ 羽区：飞羽
def tract_wing(p: SoftTissue, B: Body) -> None:
    """初级飞羽（腕→翼尖）+ 次级飞羽（肘→腕）+ 两层覆羽。

    每根羽是一块扁板，根粗梢细、逐根**错开**成叠瓦状 —— 排成一把整齐的梳子就假了，
    真羽是后一根压着前一根的。
    """
    U = B.U
    spread = B.pose == "spread"
    ref = (0.0, 0.0, 1.0) if spread else (0.0, 1.0, 0.0)

    for sgn, side in ((-1, "l"), (1, "r")):
        sh, el, wr, tip = B.wing_chain(side)
        hum_len = _len(sh, el)

        # --- 初级飞羽：长在腕掌骨与指骨上，最长，越靠翼尖越长且越后掠 ---
        # 收翼时飞羽不缩短，而是**顺体轴收拢**指向尾后方。照展翼那样朝翼面后缘伸，
        # 收起来的翼尖会直接垂到地上（小档翼相对最长，先撞到的就是它）。
        fold_dir = (sgn * 0.10, -0.06, 0.99)
        n_pri = 8
        back_hand = _trailing(wr, tip, el, ref)
        hand_axis = normalize((tip[0] - wr[0], tip[1] - wr[1], tip[2] - wr[2]))
        for k in range(n_pri):
            t = k / (n_pri - 1)
            root = _mix(wr, tip, 0.12 + 0.86 * t)
            # 长度：中段最长，翼尖那几根略短（真翼的尖是圆的，不是尖的）
            ln = hum_len * lerp(1.05, 1.62, math.sin(math.pi * (0.25 + 0.62 * t)) ** 0.7)
            if spread:
                # 方向：后缘为主，越靠翼尖越往翼展方向甩（后掠）
                d = tuple(back_hand[i] * (1.0 - 0.42 * t) + hand_axis[i] * (0.20 + 0.55 * t)
                          for i in range(3))
            else:
                d = tuple(fold_dir[i] + hand_axis[i] * 0.18 for i in range(3))
                ln *= 0.62  # 收拢时相邻飞羽层层重叠，露在外面的长度只有展开时的一半多
            end = _step(root, d, ln)
            p.strut(f"manus_{side}", f"primary_{side}_{k + 1}", root, end,
                    0.10 * U, lerp(0.62, 0.42, t) * U, mat="feather_flight")
            # 羽尖压深一档，翼梢那圈深色是猛禽最好认的记号
            p.strut(f"manus_{side}", f"primary_tip_{side}_{k + 1}", _mix(root, end, 0.78), end,
                    0.09 * U, lerp(0.52, 0.34, t) * U, mat="feather_dark")

        # --- 次级飞羽：长在尺骨上（骨架那排羽茎瘤就是它们的根），短而齐 ---
        n_sec = max(6, int(9 * U ** 0.3))
        if spread:
            # 展翼时后缘 = 翼面内、朝 +z（身后）的那一侧
            back_fore = perp_to(el, wr, ref)
            if back_fore[2] < 0:
                back_fore = (-back_fore[0], -back_fore[1], -back_fore[2])
        else:
            back_fore = fold_dir  # 收翼：与初级飞羽一起顺体轴叠好
        for k in range(n_sec):
            t = k / max(1, n_sec - 1)
            root = _mix(el, wr, 0.08 + 0.84 * t)
            ln = hum_len * lerp(0.88, 1.05, t)
            end = _step(root, back_fore, ln)
            p.strut(f"ulna_{side}", f"secondary_{side}_{k + 1}", root, end,
                    0.09 * U, 0.58 * U, mat="feather_flight")

        # --- 覆羽：两层，盖住飞羽根部，把翼前半段铺平 ---
        for layer, (frac, mat) in enumerate(((0.46, "feather_covert"), (0.24, "feather_body")), start=1):
            n_cov = n_sec + 2
            for k in range(n_cov):
                t = k / max(1, n_cov - 1)
                root = _mix(el, _mix(wr, tip, 0.35), 0.02 + 0.92 * t)
                ln = hum_len * frac
                end = _step(root, back_fore, ln)
                p.strut(f"ulna_{side}" if t < 0.62 else f"manus_{side}",
                        f"covert{layer}_{side}_{k + 1}", root, end,
                        0.08 * U, lerp(0.44, 0.34, t) * U, mat=mat)
        # 肩羽：盖住肩关节与肱骨，把翼根接进体羽
        for k in range(4):
            t = k / 3
            root = _mix(sh, el, 0.05 + 0.55 * t)
            end = _step(root, back_fore, hum_len * lerp(0.62, 0.48, t))
            p.strut(f"humerus_{side}", f"scapular_{side}_{k + 1}", root, end,
                    0.14 * U, 0.62 * U, mat="feather_covert")


# ================================================================ 羽区：领羽
def tract_ruff(p: SoftTissue, B: Body) -> None:
    """颈基一圈领羽：裸颈与体羽的交界，向外上炸开。

    光脖子从一团炸开的羽里伸出来 —— "秃鹫感"的一半在这里。
    """
    U = B.U
    bones = B.neck_bones()
    # 颈基那几节（编号最大 = 最靠胸）
    base = B.P(bones[-1])
    up = B.P(bones[-3]) if len(bones) >= 3 else B.P(bones[-1])
    axis = normalize((up[0] - base[0], up[1] - base[1], up[2] - base[2]))
    # 成对生成，不按角度一圈排编号 —— 一圈排下来左右两半的序号对不上，
    # 镜像对拍会把整圈都判成"缺镜像件"。中线上的两根单独出。
    root = _off(base, dy=1.5 * U)

    def _one(name: str, ang: float) -> None:
        radial = (math.sin(ang), 0.0, math.cos(ang))
        d = tuple(radial[i] + axis[i] * 0.55 for i in range(3))
        ln = lerp(5.2, 3.6, abs(math.cos(ang))) * U
        p.strut(bones[-1], name, root, _step(root, d, ln), 1.05 * U, 0.55 * U,
                mat="feather_ruff")

    n_pairs = 6
    for k in range(n_pairs):
        ang = lerp(0.30, math.pi - 0.30, k / (n_pairs - 1))
        for sgn, side in ((-1, "l"), (1, "r")):
            _one(f"ruff_{side}_{k + 1}", sgn * ang)
    _one("ruff_front", 0.0)  # 喉前
    _one("ruff_back", math.pi)  # 项后


# ================================================================ 羽区：体羽
def _body_surface(B: Body, z: float, theta: float) -> Vec:
    """躯干体表上的一点。theta=0 背中线，±π 腹中线，正号朝右。

    截面取上窄下宽的椭圆：背脊窄，腹部要兜住那道深龙骨。
    """
    U = B.U
    (_kx0, ky0, _kz0), _hi = B.keel
    cy = B.trunk_y - 1.2 * U
    rw = 2.75 * U
    up = B.trunk_y + 2.2 * U - cy
    dn = cy - (ky0 + 1.4 * U)  # 腹羽收在龙骨下缘之上，不兜到最底
    c = math.cos(theta)
    rv = up if c >= 0 else dn
    return (math.sin(theta) * rw, cy + c * rv, z)


def tract_body(p: SoftTissue, B: Body) -> None:
    """体羽：沿体表铺一层层叠的羽片，从背中线绕到腹中线。

    Round 1 用几个轴对齐大矩形分背/侧/腹三带 —— 渲出来身体就是个箱子。羽毛得是
    **一片压一片**的小片，且一律朝后下倒伏（顺着气流的方向），才有羽的质感。
    """
    U = B.U
    t1 = B.P("trunk_front")
    hips = B.P("hips")
    n_z, n_ring = 9, 13
    z0, z1 = t1[2] - 0.8 * U, hips[2] + 3.2 * U
    span = (z1 - z0) / n_z

    for i in range(n_z):
        tz = i / (n_z - 1)
        z = lerp(z0, z1, tz)
        bone = "trunk_front" if tz < 0.55 else "hips"
        # 背中线一片
        base = _body_surface(B, z, 0.0)
        p.strut(bone, f"dorsal_{i + 1}", base, _off(base, dy=-0.25 * U, dz=span * 1.35),
                1.15 * U, 0.42 * U, mat="feather_body")
        for j in range(n_ring):
            th = lerp(0.42, math.pi - 0.16, j / (n_ring - 1))
            # 越靠腹侧颜色越浅一档（覆羽色），层次才分得开
            mat = "feather_body" if th < 2.0 else "feather_covert"
            # 片宽跟着**局部弧长**走：腹侧曲率半径比背侧大一倍多，等角度切片在那里的
            # 间距也大一倍多 —— 一律给同宽的片，腹部就整片裂开。
            half = lerp(0.82, 1.45, (1 - math.cos(th)) / 2) * U
            thin = 0.46 * U
            # 片要**贴着体表**铺：向后伸的段里 rx 是横向宽、rz 是垂直厚。背腹两处的片
            # 该横着宽，体侧那圈却该竖着宽 —— 一律按横向给，体侧就成了一排横百叶，
            # 缝里直接看见身体内部。按 |cos θ| 在两者之间过渡。
            c = abs(math.cos(th))
            rx, rz = lerp(thin, half, c), lerp(half, thin, c)
            for sgn, side in ((-1, "l"), (1, "r")):
                a = _body_surface(B, z, sgn * th)
                # 每片向后倒伏 + 略微离体（叠瓦）
                b = (a[0] * 1.04, a[1] - 0.3 * U, a[2] + span * 1.25)
                p.strut(bone, f"contour_{i + 1}_{j + 1}_{side}", a, b, rx, rz, mat=mat)
    # 胸前那一撮（龙骨最前端，兜住喙下方）。必须落在**腹中线**上 ——
    # theta 给成 0.86π 会把它甩到体侧，成了一块孤零零飘在身旁的方块。
    front = _body_surface(B, z0 - 0.6 * U, math.pi)
    p.strut("trunk_front", "breast_front", front, _off(front, dy=1.1 * U, dz=2.0 * U),
            1.7 * U, 0.55 * U, mat="feather_body")


# ================================================================ 羽区：尾
def tract_tail(p: SoftTissue, B: Body) -> None:
    """尾羽扇：12 根，附在尾综骨上向后铺开。尾骨很短，长的是这些羽。"""
    U = B.U
    pyg = B.P("pygostyle")
    # 同领羽：左右成对出，别按扇形角度一路排编号
    n_pairs = 6
    fan = B.U * 13.5  # 尾羽长度：约躯干的四成
    root = _off(pyg, dy=-0.2 * U, dz=0.3 * U)
    for k in range(n_pairs):
        t = k / (n_pairs - 1)
        ang = lerp(0.05, 0.42, t)  # 张角：中央那对几乎并拢，最外一对张开
        ln = fan * lerp(1.0, 0.86, t)  # 中央尾羽最长，向外递减 —— 扇缘才是圆的
        for sgn, side in ((-1, "l"), (1, "r")):
            d = (sgn * math.sin(ang), -0.12, math.cos(ang))
            # 羽片要够宽才连得成扇面；细条排出来是一把叉子
            p.strut("pygostyle", f"rectrix_{side}_{k + 1}", root, _step(root, d, ln),
                    1.15 * U, 0.20 * U, mat="feather_tail")
    # 尾上覆羽：盖住尾羽根部
    p.piece("pygostyle", "tail_covert", (-1.6 * U, pyg[1] - 0.4 * U, pyg[2] - 1.2 * U),
            (1.6 * U, pyg[1] + 1.3 * U, pyg[2] + 3.4 * U), mat="feather_covert")


# ================================================================ 羽区：腿
def tract_leg(p: SoftTissue, B: Body) -> None:
    """大腿有羽（"裤子"），跗跖以下只有鳞 —— 羽在这里戛然而止。"""
    U = B.U
    for sgn, side in ((-1, "l"), (1, "r")):
        hip, knee, ankle, toe = B.leg_chain(side)
        # 腿羽（"裤子"）：包住髋到膝。一整个大盒会在展翼露腿时现出棋盘纹的方块 ——
        # 大平面把 8×8 的色块拉开采样，看着像贴错图。拆成几片顺腿倒伏的羽。
        n_th = 4
        for k in range(n_th):
            t = k / (n_th - 1)
            y = lerp(hip[1] + 1.4 * U, ankle[1] + 1.2 * U, t)
            r = lerp(2.05, 1.35, t) * U
            a = (sgn * 2.2 * U, y, lerp(hip[2] + 1.0 * U, knee[2] - 0.4 * U, t))
            p.strut(f"femur_{side}", f"thigh_feather_{side}_{k + 1}", a,
                    _off(a, dx=sgn * 0.55 * U, dy=-0.9 * U, dz=1.5 * U),
                    r, 0.62 * U, mat="feather_body")
        # 跗跖鳞：一圈薄壳裹住骨，逐节分段（鳞是横向排的）
        n_sc = 4
        for k in range(n_sc):
            a = _mix(ankle, toe, k / n_sc)
            b = _mix(ankle, toe, (k + 1) / n_sc)
            p.strut(f"tarsometatarsus_{side}", f"tarsal_scale_{side}_{k + 1}", a, b,
                    0.78 * U, 0.86 * U, mat="scale_leg")
        # 趾鳞：只包第一节，爪留在外面。壳要贴着趾骨薄薄一层 ——
        # 裹厚了就从地面下面鼓出来（趾本来就贴着 y=0）。
        p.strut(f"toes_{side}", f"toe_scale_{side}", _off(toe, dy=0.06 * U),
                _off(toe, dy=-0.16 * U, dz=-1.6 * U), 0.95 * U, 0.40 * U, mat="scale_leg")


# ================================================================ 裸区
def tract_bare(p: SoftTissue, B: Body) -> None:
    """裸头裸颈的皱皮。

    **不铺羽**才是这一层在头颈上要做的事：给秃鹫的头颈裹上羽，它就变成一只普通猛禽。
    这里只贴一层薄皮，让颈椎的异变骨钉照旧穿出来。
    """
    U = B.U
    bones = B.neck_bones()
    pts = [B.P(b) for b in bones]
    for i in range(len(bones) - 1):
        a, b = pts[i], pts[i + 1]
        t = i / max(1, len(bones) - 2)
        # 越靠头越细：颈根粗、贴到颅骨那截收紧
        r = lerp(1.45, 1.00, t) * U
        p.strut(bones[i], f"neck_skin_{len(bones) - i}", a, b, r, r * 1.05, mat="skin_bare")
    # 头部裸皮：盖住颅骨但露出喙与眼眶
    skull = B.P("skull")
    p.piece("skull", "head_skin", (-1.9 * U, skull[1] - 1.9 * U, skull[2] - 1.2 * U),
            (1.9 * U, skull[1] + 2.4 * U, skull[2] + 1.4 * U), mat="skin_bare")
    # 喉囊（兀鹫颈前那块垂皮）
    p.piece(bones[-1], "crop_pouch", (-1.5 * U, pts[-1][1] + 1.2 * U, pts[-1][2] - 2.6 * U),
            (1.5 * U, pts[-1][1] + 4.0 * U, pts[-1][2] - 0.6 * U), mat="skin_bare")


BUILDERS = {
    "ruff": tract_ruff,
    "body": tract_body,
    "wing": tract_wing,
    "tail": tract_tail,
    "leg": tract_leg,
    "bare": tract_bare,
}


# ================================================================ 装配
def build(size: str, morph: str, tracts: tuple[str, ...] = TRACTS, *, pose: str = "folded",
          anatomy: bool = False):
    """默认铺在**骨架**上：羽是外观层，最终形象里不该从羽缝里透出红肌肉。
    anatomy=True 时铺在肌肉层上，用来看羽/肌/骨的包裹关系。
    """
    spec = SPECS[size]
    base = spec.model if not anatomy else spec.model.replace("Skeleton", "Muscle")
    name = base + ("_spread" if pose == "spread" else "")
    src = MODELS / f"{name}.bbmodel"
    if not src.exists():
        which = "gen_muscle.py" if anatomy else "gen_skeleton.py"
        raise SystemExit(f"底层不存在：{src}\n先跑 {which} --size {size}"
                         + (" --pose spread" if pose == "spread" else ""))
    skel = Skeleton(src)
    colors = MORPHS[morph]["colors"]
    if tuple(colors) != MAT_KEYS:
        raise ValueError(f"变色 {morph} 的材质键与基准不一致（UV 按索引取，顺序必须相同）")
    skel.extend_texture(colors, PELT_ROW)
    B = Body(skel, pose=pose)
    p = SoftTissue(skel, colors, PELT_ROW)
    for t in tracts:
        BUILDERS[t](p, B)
    if not anatomy and tracts == TRACTS:
        _prune_hidden(skel)
    return skel, B


def check(size: str, morph: str, verbose: bool = True) -> int:
    skel, B = build(size, morph)
    # 羽层挂的件带 _muscle 标记（SoftTissue 统一打的），这里只看本层新增的那批
    added = [e for e in skel.added() if _is_pelt(e["name"])]
    problems = list(mirror_violations(added))

    (_x0, y0, _z0), (_x1, y1, _z1) = element_bounds(added)
    if y0 < -0.05 * B.U:
        problems.append(f"羽层插地：最低点 y={y0:.2f}")

    # 头颈必须**没有**羽 —— 秃鹫的立身之本
    head_z = B.P("skull")[2]
    for e in added:
        if e["name"].startswith(("neck_skin", "head_skin", "crop_pouch", "ruff")):
            continue
        if (e["from"][2] + e["to"][2]) / 2 < head_z + 2.0 * B.U:
            problems.append(f"{e['name']}: 羽长到头颈上了（秃鹫的头颈是裸的）")

    tagged = [e for e in skel.data["elements"] if e.get("_muscle")]
    if len(tagged) != len(added):
        problems.append(
            f"羽层挂了 {len(tagged)} 件，但按名字只认出 {len(added)} 件 —— "
            f"_is_pelt 的前缀表漏了某组羽（改过名字？）")

    if verbose:
        spec = SPECS[size]
        print(f"[{size}·{morph}] {spec.cn} · {MORPHS[morph]['cn']}")
        print(f"  羽件 {len(added)} · 总 {len(skel.data['elements'])} 件 · U={B.U:.3f}")
        if problems:
            print(f"  ✗ {len(problems)} 处违例：")
            for x in problems[:10]:
                print(f"     {x}")
        else:
            print("  ✓ 镜像 / 贴地 / 头颈裸露 全部通过")
    return len(problems)


# 羽层之外仍然**露在体外**、必须留下的底层件。其余骨全在羽下面，渲染看不见还占面数。
# 颈钉留着是因为它是异变族征 —— 那排骨钉本来就该从裸颈上穿出来。
EXPOSED = (
    "rhamphotheca", "lower_rhamphotheca", "beak_hook", "cere", "naris", "maxilla",  # 喙
    "mandible", "incisors",  # 下颌
    "claw", "toe", "hind_claw",  # 爪与趾
    "orbit_socket", "sclerotic", "supraorbital", "jugal",  # 眼
    "cervical_spine",  # 颈椎骨钉（异变族征）
    "frontal_crest", "pygostyle_blade", "alula_claw",  # 额嵴 / 尾刃 / 翼爪
)


def _is_exposed(name: str) -> bool:
    return name.startswith(EXPOSED)


def _prune_hidden(skel: Skeleton) -> None:
    """只留羽层 + 露在体外的底层件。

    保留判据用**本层挂件的标记**，不用名字白名单 —— 白名单漏一项就静默丢掉整组羽：
    体羽从 flank/ventral 改名成 contour 那次，187 片体羽被当成"看不见的骨"全剔了，
    躯干整个空掉，而自检只看过滤后的件，一声不吭。
    """
    keep_uuid = set()
    kept = []
    for e in skel.data["elements"]:
        if e.get("_muscle") or _is_exposed(e["name"]):
            kept.append(e)
            keep_uuid.add(e["uuid"])
    skel.data["elements"] = kept

    def prune(node):
        if isinstance(node, str):
            return node in keep_uuid
        node["children"] = [c for c in node.get("children", []) if prune(c)]
        return True

    for root in skel.data["outliner"]:
        prune(root)


def _is_pelt(name: str) -> bool:
    return name.startswith((
        "primary", "secondary", "covert", "scapular", "ruff", "dorsal", "contour",
        "breast", "rectrix", "tail_covert", "thigh_feather", "tarsal_scale", "toe_scale",
        "neck_skin", "head_skin", "crop_pouch",
    ))


def main() -> int:
    ap = argparse.ArgumentParser(description="腐羽鹫羽毛层生成器")
    ap.add_argument("--size", choices=sorted(SPECS), help="只出单档")
    ap.add_argument("--morph", choices=sorted(MORPHS), help="只出单个变色")
    ap.add_argument("--pose", choices=("folded", "spread"), default="folded")
    ap.add_argument("--tract", choices=TRACTS, help="只生成单个羽区（预览用）")
    ap.add_argument("--only-pelt", action="store_true", help="摘掉底层，只留羽")
    ap.add_argument("--with-anatomy", action="store_true", help="铺在肌肉层上（看包裹关系）")
    ap.add_argument("--check", action="store_true", help="只跑自检，不写文件")
    ap.add_argument("--list", action="store_true", help="列出变色与羽区")
    args = ap.parse_args()

    if args.list:
        print("变色：")
        for k, v in MORPHS.items():
            print(f"  {k:5s} {v['cn']}  {v['note']}")
        print("羽区：")
        for t in TRACTS:
            print(f"  {t:6s} {TRACT_LABEL[t]}")
        return 0

    sizes = [args.size] if args.size else ["small", "mid", "large"]
    morphs = [args.morph] if args.morph else list(MORPHS)

    if args.check:
        bad = 0
        for s in sizes:
            for mo in morphs:
                bad += check(s, mo)
        return 1 if bad else 0

    for s in sizes:
        for mo in morphs:
            tracts = (args.tract,) if args.tract else TRACTS
            skel, _B = build(s, mo, tracts, pose=args.pose, anatomy=args.with_anatomy)
            if args.only_pelt:
                skel.data["elements"] = [e for e in skel.data["elements"] if _is_pelt(e["name"])]
                keep = {e["uuid"] for e in skel.data["elements"]}

                def prune(node, keep=keep):
                    if isinstance(node, str):
                        return node in keep
                    node["children"] = [c for c in node.get("children", []) if prune(c)]
                    return True

                for root in skel.data["outliner"]:
                    prune(root)
            name = SPECS[s].model.replace("Skeleton", "Pelt") + f"_{mo}"
            if args.with_anatomy:
                name += "_anatomy"
            if args.pose == "spread":
                name += "_spread"
            if args.tract:
                name += f"_{args.tract}"
            if args.only_pelt:
                name += "_bare"
            out = MODELS / f"{name}.bbmodel"
            skel.write(out, name)
            print(f"→ {out.relative_to(REPO)}  ({len(skel.data['elements'])} 件)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
