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
  python3 modelScript/creatures/fuyu_vulture/gen_pelt.py                    # 9 个全出
  python3 modelScript/creatures/fuyu_vulture/gen_pelt.py --size mid --morph xiu
  python3 modelScript/creatures/fuyu_vulture/gen_pelt.py --size mid --pose spread
  python3 modelScript/creatures/fuyu_vulture/gen_pelt.py --size mid --only-pelt   # 只看羽
  python3 modelScript/creatures/fuyu_vulture/gen_pelt.py --check
  python3 modelScript/creatures/fuyu_vulture/gen_pelt.py --list
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
# --- modelScript 路径引导：共用底座在 core/ ---
import sys as _sys
from pathlib import Path as _Path
_sys.path.insert(0, str(_Path(__file__).resolve().parents[2] / "core"))
from bbmodel_maker.rig.rigkit import (  # noqa: E402
    Skeleton, SoftTissue, Vec, bake_file, element_bounds, lerp, mirror_violations,
    normalize, perp_to,
)

REPO = HERE.parents[2]
FINAL_DIR = Path(__file__).resolve().parents[2] / "models" / "fuyu_vulture"  # 最终 9 个外观
MODELS = FINAL_DIR / "layers"  # 底层与各种预览产物

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


def _poly_at(pts: list[Vec], s: float) -> Vec:
    """折线上按**弧长**取点，s∈[0,1]。

    按"每段各分一半采样点"铺是错的：腿的两段长短差着一倍，那样铺出来膝上密、
    膝下疏 —— 恰恰在最该盖住的那截上稀掉。
    """
    segs = [_len(pts[i], pts[i + 1]) for i in range(len(pts) - 1)]
    total = sum(segs)
    if total <= 0.0:
        return pts[0]
    d = max(0.0, min(1.0, s)) * total
    for i, ln in enumerate(segs):
        if d <= ln or i == len(segs) - 1:
            return _mix(pts[i], pts[i + 1], d / ln if ln > 0 else 0.0)
        d -= ln
    return pts[-1]


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

    def plate(roots: list[Vec], k: int, t: float, thick: float, tall,
              shrink: float = 1.0, cover: float = 0.62) -> tuple[float, float]:
        """一根羽的横截面 (rx, rz)。**两种姿态要装反过来。**

        shaft_box 只出 (pitch, yaw, 0)、没有滚转，横截面朝向完全由骨轴决定：羽近乎水平
        时，局部 x 恒落在水平面内、局部 z 恒竖直。于是——
          · 收翼：羽顺体轴往后叠，竖着的那一面正好是层层压住的瓦片，薄的一面贴着体侧。
          · 展翼：羽铺在水平的翼面里，宽面必须**横跨翼展**把相邻两根之间的缝盖住，竖直
            方向只该留一片厚度。
        早先两种姿态共用一组尺寸：展翼时翼展方向只有 0.21 宽，而羽根间距 1.87 —— 整面翼
        是一把梳子，八根竖板中间全是空气（正上方看最明显）。

        宽度按**实测羽根间距**给而不是写死：间距随档位和羽数变，写死的常数换个档就露缝。
        cover 是"半宽相当于几倍羽根间距"：次级与覆羽给 0.62（全宽 1.24 倍间距，稳稳叠成
        一整面），初级给得更大，因为它们从腕上呈**扇形**发散 —— 羽根挨着不代表羽梢挨着，
        实测根部只差 0.03 就贴上、羽梢已经张开 1.12。调宽之后内三分之二实心、只有翼梢
        还留着缝，那正是兀鹫的指状开缝翼尖，不该填掉。
        shrink 只作用在展翼分支（压深的羽尖要比主羽窄一点才不打架）；收翼分支原样返回
        旧常数，一位小数都不动 —— 那九个收翼交付物已经过审了。
        """
        if not spread:
            return thick, (tall(t) if callable(tall) else tall)
        step = _len(roots[k], roots[k - 1 if k else min(1, len(roots) - 1)])
        return cover * step * shrink, thick

    def wing_up(along: Vec, feather: Vec) -> Vec | None:
        """翼面法线：由「展向」与「羽向」张成的平面的法线，强制朝上。

        展翼时给它，羽板才会跟着翼面倾斜而不是一律平摆。上反角让翼每 1.14 单位升 0.16，
        平摆的宽板一块块错开，从正后方看整片翼是一段楼梯 —— 俯视完全看不出来。
        叉积是赝矢量、左右不自动镜像，所以最后统一把 y 分量掰正（两侧同样朝上），这样
        左右天然对称，不会各歪各的。
        """
        if not spread:
            return None
        a, b = normalize(along), normalize(feather)
        n = (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])
        m = math.sqrt(sum(c * c for c in n))
        if m < 1e-4:
            return None
        n = tuple(c / m for c in n)
        return n if n[1] >= 0 else tuple(-c for c in n)

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
        # 羽根覆盖的骨段范围：收翼姿保持原值（那九个交付物已过审），展翼姿铺满整根骨并在
        # 关节处相互压过去 —— 各组只铺自己骨段的中间一截时，肩肘腕三处各留一条没羽的带，
        # 收翼时被体侧挡着看不见，一展开翼就断成三块。
        p0, p1 = (0.00, 1.00) if spread else (0.12, 0.98)
        pri_roots = [_mix(wr, tip, lerp(p0, p1, k / (n_pri - 1))) for k in range(n_pri)]
        for k in range(n_pri):
            t = k / (n_pri - 1)
            root = pri_roots[k]
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
            rx, rz = plate(pri_roots, k, t, 0.10 * U, lambda tt: lerp(0.62, 0.42, tt) * U,
                           cover=0.88)
            up = wing_up(hand_axis, (end[0] - root[0], end[1] - root[1], end[2] - root[2]))
            p.quill(f"manus_{side}", f"primary_{side}_{k + 1}", root, end, rx, rz,
                    mat="feather_flight", bone=f"q_primary_{side}_{k + 1}", up=up)
            # 羽尖压深一档，翼梢那圈深色是猛禽最好认的记号。它挂在**主羽自己那根骨**上：
            # 同一根羽的一段，必须完全跟着主羽走。早先为了消掉一条 0.045 单位的静态亮边把
            # 它拆成独立骨，端点是精确了，可两根骨各自插值 —— 展开到 0.2~0.5 之间羽尖整个
            # 飘离主羽。拿一条看不见的亮边换一段看得见的分离，是笔亏本买卖。
            #
            # 亮边的根子是"羽尖与主羽的截面比例在两姿不相等"，那就把比例做**相等**：两边
            # 都按同一组系数从主羽推。这样一个缩放通道对两件同时精确，亮边也不会回来。
            tw, th = (0.09 / 0.10, lerp(0.52, 0.34, t) / lerp(0.62, 0.42, t))
            tx, tz = (rx * tw, rz * th) if spread else (0.09 * U, lerp(0.52, 0.34, t) * U)
            p.quill(f"manus_{side}", f"primary_tip_{side}_{k + 1}", _mix(root, end, 0.78), end,
                    tx, tz, mat="feather_dark", bone=f"q_primary_{side}_{k + 1}", up=up)

        # --- 次级飞羽：长在尺骨上（骨架那排羽茎瘤就是它们的根），短而齐 ---
        n_sec = max(6, int(9 * U ** 0.3))
        if spread:
            # 展翼时后缘 = 翼面内、朝 +z（身后）的那一侧
            back_fore = perp_to(el, wr, ref)
            if back_fore[2] < 0:
                back_fore = (-back_fore[0], -back_fore[1], -back_fore[2])
        else:
            back_fore = fold_dir  # 收翼：与初级飞羽一起顺体轴叠好
        s0, s1 = (-0.03, 1.02) if spread else (0.08, 0.92)
        sec_roots = [_mix(el, wr, lerp(s0, s1, k / max(1, n_sec - 1))) for k in range(n_sec)]
        for k in range(n_sec):
            t = k / max(1, n_sec - 1)
            root = sec_roots[k]
            ln = hum_len * lerp(0.88, 1.05, t)
            end = _step(root, back_fore, ln)
            rx, rz = plate(sec_roots, k, t, 0.09 * U, 0.58 * U)
            p.quill(f"ulna_{side}", f"secondary_{side}_{k + 1}", root, end, rx, rz,
                    mat="feather_flight", bone=f"q_secondary_{side}_{k + 1}",
                    up=wing_up((wr[0] - el[0], wr[1] - el[1], wr[2] - el[2]), back_fore))

        # --- 覆羽：两层，盖住飞羽根部，把翼前半段铺平 ---
        for layer, (frac, mat) in enumerate(((0.46, "feather_covert"), (0.24, "feather_body")), start=1):
            n_cov = n_sec + 2
            cov_roots = [_mix(el, _mix(wr, tip, 0.35), 0.02 + 0.92 * (k / max(1, n_cov - 1)))
                         for k in range(n_cov)]
            for k in range(n_cov):
                t = k / max(1, n_cov - 1)
                # 覆羽与飞羽只错开**极小**一点：够躲开共面 z-fighting，不够形成可见的
                # 台阶。抬 0.15U 时三层叠出 0.47 的总厚（两片板高），近看就是一层黑块浮
                # 在翼面上；0.06U 之后总厚 0.33，读作一整片翼。
                # 完全同高也不行 —— 两组羽根都铺在骨线上时只差 0.06，从正上方看是互相
                # 穿插的窄条。
                root = _off(cov_roots[k], dy=0.06 * layer * U) if spread else cov_roots[k]
                ln = hum_len * frac
                end = _step(root, back_fore, ln)
                rx, rz = plate(cov_roots, k, t, 0.08 * U, lambda tt: lerp(0.44, 0.34, tt) * U)
                p.quill(f"ulna_{side}" if t < 0.62 else f"manus_{side}",
                        f"covert{layer}_{side}_{k + 1}", root, end, rx, rz, mat=mat,
                        bone=f"q_covert{layer}_{side}_{k + 1}",
                        up=wing_up((wr[0] - el[0], wr[1] - el[1], wr[2] - el[2]), back_fore))
        # 肩羽：盖住肩关节与肱骨，把翼根接进体羽
        # 羽数两姿必须一致：收翼绑定姿是展翼动画的起点，起点少一根，展开后就少一根 ——
        # 早先展翼给 7 根、收翼 4 根，展开之后内翼露出三条骨缝（A/B 对拍里那几道亮线）。
        # 覆盖靠加宽补：plate 的半宽跟着实测羽根间距走，4 根摊满整根肱骨照样叠成一整面。
        n_sca = 4
        a0, a1 = (0.02, 1.02) if spread else (0.05, 0.60)
        sca_roots = [_mix(sh, el, lerp(a0, a1, k / (n_sca - 1))) for k in range(n_sca)]
        for k in range(n_sca):
            t = k / (n_sca - 1)
            root = _off(sca_roots[k], dy=0.06 * U) if spread else sca_roots[k]
            # 展翼时弦长要往肘端**变长**才接得上次级飞羽（0.88 hum_len）；收翼时相反，
            # 越靠肘越短才收得进体侧。
            chord = lerp(0.60, 0.90, t) if spread else lerp(0.62, 0.48, t)
            end = _step(root, back_fore, hum_len * chord)
            rx, rz = plate(sca_roots, k, t, 0.14 * U, 0.62 * U)
            p.quill(f"humerus_{side}", f"scapular_{side}_{k + 1}", root, end, rx, rz,
                    mat="feather_covert", bone=f"q_scapular_{side}_{k + 1}",
                    up=wing_up((el[0] - sh[0], el[1] - sh[1], el[2] - sh[2]), back_fore))


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

    # 放射状排列越往外越稀疏，档位越大周长越长 —— 片数得跟上，否则领羽之间
    # 透出缝来
    n_pairs = 8
    for k in range(n_pairs):
        ang = lerp(0.26, math.pi - 0.26, k / (n_pairs - 1))
        for sgn, side in ((-1, "l"), (1, "r")):
            _one(f"ruff_{side}_{k + 1}", sgn * ang)
    _one("ruff_front", 0.0)  # 喉前
    _one("ruff_back", math.pi)  # 项后


# ================================================================ 羽区：体羽
def _body_profile(t: float) -> float:
    """躯干沿体轴的粗细包络。t=0 胸前，t=1 尾根。

    没有这条包络，躯干就是前后一般粗的**圆筒** —— 正面看是件长袍，侧面看是块板。
    鸟的躯干是水滴形：胸口饱满、中段最粗、往尾根一路收窄。
    """
    return math.sin(math.pi * (0.20 + 0.72 * max(0.0, min(1.0, t)))) ** 0.55


def _body_surface(B: Body, z: float, t: float, theta: float, scale: float = 1.0) -> Vec:
    """躯干体表上的一点。theta=0 背中线，±π 腹中线，正号朝右。

    截面取上窄下宽的椭圆（背脊窄、腹部兜着那道深龙骨），整体再乘沿体轴的粗细包络。
    """
    U = B.U
    (_kx0, ky0, _kz0), _hi = B.keel
    prof = _body_profile(t) * scale
    cy = B.trunk_y - 1.2 * U
    rw = 3.3 * U * prof
    up = (B.trunk_y + 2.2 * U - cy) * prof
    # 腹侧前深后浅：胸口要兜住龙骨，到尾根那边没什么可兜的
    dn = (cy - (ky0 + 2.6 * U)) * prof * lerp(1.0, 0.62, t)
    c = math.cos(theta)
    rv = up if c >= 0 else dn
    return (math.sin(theta) * rw, cy + c * rv, z)


def _contour_ring(p: SoftTissue, B: Body, bone: str, tag: str, z: float, t: float,
                  reach: float, n_ring: int, *, scale: float = 1.0, lean: float = 1.0) -> None:
    """在某个体轴位置上环绕一圈羽片。theta 走满 0→π，左右成对 + 背腹两条中线各一片。

    Round 2 只铺 0.42→2.98 那一段：背中线另有一片补上，**腹中线却空着**，正面看
    就是一条从喉到腹的缝。
    """
    U = B.U
    for j in range(n_ring + 1):
        th = math.pi * j / n_ring
        # 越靠腹侧颜色越浅一档（覆羽色），层次才分得开
        mat = "feather_body" if th < 2.0 else "feather_covert"
        # 片宽跟着**局部弧长**走：腹侧曲率半径比背侧大一倍多，等角度切片在那里的
        # 间距也大一倍多 —— 一律给同宽的片，腹部就整片裂开。
        half = lerp(0.86, 1.55, (1 - math.cos(th)) / 2) * U * scale
        thin = 0.46 * U
        # 片要**贴着体表**铺：向后伸的段里 rx 是横向宽、rz 是垂直厚。背腹两处的片
        # 该横着宽，体侧那圈却该竖着宽 —— 一律按横向给，体侧就成了一排横百叶，
        # 缝里直接看见身体内部。按 |cos θ| 在两者之间过渡。
        c = abs(math.cos(th))
        rx, rz = lerp(thin, half, c), lerp(half, thin, c)
        mid = j == 0 or j == n_ring  # 背中线 / 腹中线：单片，不成对
        for sgn, side in (((0, ""),) if mid else ((-1, "l"), (1, "r"))):
            a = _body_surface(B, z, t, sgn * th, scale)
            b = (a[0] * 1.04, a[1] - 0.3 * U * lean, a[2] + reach)
            name = f"{tag}_{j + 1}" if mid else f"{tag}_{j + 1}_{side}"
            p.strut(bone, name, a, b, rx, rz, mat=mat)


def tract_body(p: SoftTissue, B: Body) -> None:
    """体羽：沿体表铺一层层叠的羽片，环绕整周。

    Round 1 用几个轴对齐大矩形分背/侧/腹三带 —— 渲出来身体就是个箱子。羽毛得是
    **一片压一片**的小片，且一律朝后下倒伏（顺着气流的方向），才有羽的质感。
    """
    U = B.U
    t1 = B.P("trunk_front")
    hips = B.P("hips")
    n_z, n_ring = 9, 14
    z0, z1 = t1[2] - 0.8 * U, hips[2] + 3.2 * U
    span = (z1 - z0) / n_z

    for i in range(n_z):
        tz = i / (n_z - 1)
        _contour_ring(p, B, "trunk_front" if tz < 0.55 else "hips", f"contour_{i + 1}",
                      lerp(z0, z1, tz), tz, span * 1.25, n_ring)

    # --- 胸前封盖 ---
    # 体羽全是"从体表向后伸的片"，只能围成一个**开口朝前的筒**：正面直接看进内腔，
    # 胸口一个大黑洞。这里再往前叠两圈越收越小的羽把筒口封成圆胸。
    for k, (dz, sc, nr) in enumerate(((1.1, 0.76, 9), (2.3, 0.44, 5)), start=1):
        _contour_ring(p, B, "trunk_front", f"breast_cap{k}", z0 - dz * U, 0.0,
                      dz * U * 0.9, nr, scale=sc, lean=0.4)
    # 喉下那一撮，接住裸颈与胸羽的交界
    throat = _body_surface(B, z0 - 2.6 * U, 0.0, math.pi, 0.42)
    p.strut("trunk_front", "breast_front", throat, _off(throat, dy=1.2 * U, dz=2.2 * U),
            1.5 * U, 0.55 * U, mat="feather_body")

    # --- 尾根封盖：同理，后端也别留个洞 ---
    _contour_ring(p, B, "hips", "rump_cap", z1 + 1.2 * U, 1.0, 1.6 * U,
                  max(6, n_ring // 2), scale=0.62, lean=0.5)

    # --- 腋羽：翼根与体侧之间那一簇 ---
    # 收拢的翼贴在体侧，但体羽外缘和翼覆羽各自为政，中间留着一条贯通的长缝 ——
    # 平视看不见，从腹下一看两侧各一道。真鸟这里正好长着腋羽把两边接起来。
    sh = B.P("humerus_r")
    for k in range(6):
        t = k / 5
        z = lerp(sh[2] - 0.6 * U, sh[2] + 10.5 * U, t)
        y = B.trunk_y - lerp(1.4, 4.2, t) * U
        for sgn, side in ((-1, "l"), (1, "r")):
            p.strut("trunk_front", f"axillary_{k + 1}_{side}",
                    (sgn * 2.7 * U, y, z), (sgn * 3.4 * U, y - 0.7 * U, z + 2.6 * U),
                    0.60 * U, 1.95 * U, mat="feather_covert")


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
        # --- 裤子：裹住**股骨 + 胫跗骨**，一路到踝 ---
        # Round 3 只顺着股骨铺（髋→膝，往前下方走），可股骨在鸟身上是横卧着埋进
        # 躯干的，真正露在体外那截"鸡腿"是胫跗骨（膝→踝，往后下方走）。踝比膝
        # 靠后 4.4U，于是腹羽底线到跗跖顶之间整条胫跗骨没人管；羽层又把羽下的骨
        # 剔干净，那段就真的**什么都没有** —— 侧面一看腿是断的。
        # 沿 髋→膝→踝 折线按弧长铺一层筒，接缝处段段相接，不留空档。
        joints = [_off(hip, dy=1.6 * U), hip, knee, _mix(ankle, toe, 0.10)]
        n_seg = 8
        for k in range(n_seg):
            a = _poly_at(joints, k / n_seg)
            b = _poly_at(joints, (k + 1) / n_seg)
            t = (k + 0.5) / n_seg
            r = lerp(2.30, 1.02, t) * U  # 大腿蓬松，往踝收紧
            p.strut(f"femur_{side}" if t < 0.42 else f"tibiotarsus_{side}",
                    f"thigh_feather_{side}_{k + 1}", a, b, r, r * 0.92,
                    mat="feather_body")
        # 外侧几片倒伏的羽：光一个锥筒是条裤管，得有片压片的层次才像羽
        n_sh = 5
        for k in range(n_sh):
            t = k / (n_sh - 1)
            a = _poly_at(joints, lerp(0.16, 0.68, t))
            r = lerp(2.15, 1.25, t) * U
            p.strut(f"femur_{side}" if t < 0.42 else f"tibiotarsus_{side}",
                    f"thigh_shingle_{side}_{k + 1}",
                    _off(a, dx=sgn * r * 0.55),
                    _off(a, dx=sgn * (r * 0.60 + 0.35 * U), dy=-1.5 * U, dz=0.9 * U),
                    0.50 * U, r * 0.72, mat="feather_covert")
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
    skull = B.P("skull")
    for i in range(len(bones) - 1):
        a, b = pts[i], pts[i + 1]
        t = i / max(1, len(bones) - 2)
        # bones[0] 是**贴着颅骨**那一节：颈根粗、往头那头收紧。Round 3 把这条插值
        # 写反了（注释是对的，代码反着写），脖子成了上粗下细的漏斗。
        r = lerp(1.02, 1.62, t) * U
        p.strut(bones[i], f"neck_skin_{len(bones) - i}", a, b, r, r * 1.05, mat="skin_bare")
    # 项部这一节：颅骨 → 第一颈椎。原来的链只连 neck_i→neck_{i+1}，头与颈之间
    # 空着**整整一节**，侧面一看头是浮在脖子上方的。
    p.strut("skull", "neck_skin_nape", _off(skull, dy=-0.6 * U, dz=0.8 * U), pts[0],
            1.05 * U, 1.10 * U, mat="skin_bare")

    # --- 头部裸皮 ---
    # 下缘要压到下颌关节以下：只盖颅骨的话，颌关节那截骨杆吊在头下面晃。
    # 头皮比巩膜环宽，一块整盒糊上去眼睛就没了 —— 眼那一圈按 y/z 挖出来单独收窄，
    # 让巩膜环从眼窝里凸出来。秃鹫的辨识度有一半在那只眼上。
    z_face = skull[2] - 1.2 * U
    z_back = skull[2] + 1.6 * U
    y_low, y_high = skull[1] - 2.6 * U, skull[1] + 2.4 * U
    (ex0, ey0, ez0), (_ex1, ey1, ez1) = B.s.box("sclerotic_r")
    w = 1.9 * U
    p.piece("skull", "head_skin_top", (-w, ey1, z_face), (w, y_high, z_back), mat="skin_bare")
    p.piece("skull", "head_skin_jaw", (-w, y_low, z_face), (w, ey0, z_back), mat="skin_bare")
    p.piece("skull", "head_skin_nape", (-w, ey0, ez1), (w, ey1, z_back), mat="skin_bare")
    p.piece("skull", "head_skin_lore", (-w, ey0, z_face), (w, ey1, ez0), mat="skin_bare")
    p.piece("skull", "head_skin_orbit", (-ex0, ey0, ez0), (ex0, ey1, ez1), mat="skin_bare")
    # 眼球填住巩膜环的孔。不填的话眼窝就是个方黑洞 —— 环是一圈骨片，中间本来是空的。
    ins = 0.58 * U
    for sgn, side in ((-1, "l"), (1, "r")):
        p.piece("skull", f"eye_{side}", (sgn * ex0, ey0 + ins, ez0 + ins),
                (sgn * (ex0 + 0.36 * U), ey1 - ins, ez1 - ins), mat="scale_leg")
    # 面皮：颅骨前壁 → 蜡膜，逐段收细。缺这一截，头到喙之间只剩上颌骨和颧弓两根
    # 细杆撑着，横截面从 3.9 宽一步跌到 1.8 宽 —— 看上去就是"头和喙断开了"。
    (cx0, cy0, cz0), (cx1, cy1, cz1) = B.s.box("cere")
    n_face = 3
    for k in range(n_face):
        t0, t1 = k / n_face, (k + 1) / n_face
        za, zb = lerp(z_face, cz0 + 0.35 * U, t0), lerp(z_face, cz0 + 0.35 * U, t1)
        # 宽度停在巩膜环外沿以内，眼睛才不会被面皮糊住
        w = lerp(1.66 * U, (cx1 - cx0) / 2 + 0.12 * U, t1)
        yl = lerp(skull[1] - 2.2 * U, cy0 - 0.12 * U, t1)
        yh = lerp(skull[1] + 2.3 * U, cy1 + 0.12 * U, t1)
        p.piece("skull", f"face_skin_{k + 1}", (-w, yl, zb), (w, yh, za), mat="skin_bare")
    # 颏下（gular）：下颌两支之间那道皮。不填的话从下方看是两根平行骨条中间通透，
    # 而秃鹫这里本来就长着一块松垂的裸皮。
    # 只兜到下颌支中段就收：一路铺到颏尖的话，喙下多出一层肉色台阶，喙就不成喙了。
    (_mx0, my0, mz0), (mx1, _my1, mz1) = B.s.box("mandible_ramus_r")
    z_chin = lerp(mz1, mz0, 0.55)
    n_gu = 3
    for k in range(n_gu):
        t0, t1 = k / n_gu, (k + 1) / n_gu
        za, zb = lerp(mz1 - 0.3 * U, z_chin, t0), lerp(mz1 - 0.3 * U, z_chin, t1)
        w = lerp(mx1 - 0.05 * U, 0.46 * U, t1)
        yh = lerp(my0 + 1.05 * U, my0 + 0.55 * U, t1)
        p.piece("jaw", f"gular_{k + 1}", (-w, my0 - 0.10 * U, zb), (w, yh, za),
                mat="skin_bare")
    # 喉囊（兀鹫颈前那块垂皮）
    # 要一路兜到领羽根部：颈越长（大档 18 节）裸颈与领羽之间的三角缝越明显，
    # 从前侧一看喉部就是个洞。
    p.piece(bones[-1], "crop_pouch", (-1.9 * U, pts[-1][1] - 0.6 * U, pts[-1][2] - 3.2 * U),
            (1.9 * U, pts[-1][1] + 4.4 * U, pts[-1][2] + 0.4 * U), mat="skin_bare")


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


def _leak_check(size: str, morph: str, res: int = 160) -> list[str]:
    """漏光检查：模型是个**空壳**（羽下的骨都剔掉了），体表破一处就能一眼看穿到背景。

    镜像 / 贴地 / 裸露那几条都查不出这个 —— 胸前整整敞着一个口，自检照样全绿，
    直到在 Blockbench 里正面一看，胸口一个大黑洞。

    做法：从几个方向渲小图，再从图像四边做洪水填充标出**外部**背景；填不到的背景
    像素就是被模型轮廓整个围住的孔 —— 那才是破口。

    不能用"逐行扫描轮廓内的背景"：头与背之间、两腿之间、翼与尾之间本来就有大片
    真实空隙，那样查出来全是误报（实测头颈下方一片全被标红）。这些空隙都连通到
    画面外，洪水填充一路填过去，自然不算。
    """
    import sys as _sys
    _sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "core"))
    from bbmodel_maker.render.render_bbmodel import render  # noqa: PLC0415

    import tempfile  # noqa: PLC0415

    name = SPECS[size].model.replace("Skeleton", "Pelt") + f"_{morph}"
    src = FINAL_DIR / f"{name}.bbmodel"
    if not src.exists():
        return []  # 还没生成，跳过（--check 可以在生成前跑）
    # render_bbmodel 只读 elements 不读 outliner，而自带骨的羽存的是骨局部坐标 —— 直接
    # 渲会看到一堆竖板，漏光检查等于对整片翼失明。先烘到世界系再渲。
    tmp = tempfile.NamedTemporaryFile(suffix=".bbmodel", delete=False)
    tmp.close()
    src = bake_file(src, tmp.name)
    out = []
    for label, yaw, pitch in (("正面", 178.0, 4.0), ("前侧", 145.0, 10.0),
                              ("后方", 2.0, 6.0), ("腹下", 178.0, -55.0)):
        im, _ = render(src, yaw=yaw, pitch=pitch, size=res, bg=(0, 0, 0))
        px = im.load()
        bg = [[sum(px[x, y]) <= 12 for x in range(res)] for y in range(res)]
        seen = [[False] * res for _ in range(res)]
        stack = [(x, y) for x in range(res) for y in (0, res - 1) if bg[y][x]]
        stack += [(x, y) for y in range(res) for x in (0, res - 1) if bg[y][x]]
        while stack:  # 从画面边缘往里填外部背景
            x, y = stack.pop()
            if seen[y][x]:
                continue
            seen[y][x] = True
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                nx, ny = x + dx, y + dy
                if 0 <= nx < res and 0 <= ny < res and bg[ny][nx] and not seen[ny][nx]:
                    stack.append((nx, ny))
        holes = sum(bg[y][x] and not seen[y][x] for y in range(res) for x in range(res))
        # 阈值要能区分**结构性破口**与**羽缝**：胸前那个敞口是几百像素级的，而羽是
        # 一片片铺的，相邻片之间难免漏出几十像素——那是体素羽毛的固有特性，不是缺陷。
        if holes > res * res * 0.0018:
            out.append(f"{label}视图漏光：{holes} 个被模型围住的背景像素（体表有破口）")
    Path(src).unlink(missing_ok=True)
    return out


def _joined_check(elements: list[dict], U: float) -> list[str]:
    """连通性：整只鸟必须是**一整块**，不许有悬空的零件。

    漏光自检查的是"被轮廓围死的洞"，而缺掉一整截肢体是**朝外敞开**的缺口 ——
    洪水填充一路从画面外填进去，那条缝根本不算洞，自检照样全绿。头浮在脖子上方
    （颅骨→第一颈椎那节皮从来没铺过）、腹羽底线到跗跖顶之间整条胫跗骨是空的
    （裤子只顺着股骨走，而踝在膝后 4.4U），两处都是这么漏过去的，最后靠人眼在
    Blockbench 里发现。

    做法：件的**旋转后包围盒**两两判交（放宽一点容忍取整），并查集连通。包围盒
    是保守的（比真盒大），所以只会少报不会多报"断开"。
    """
    eps = 0.06 * U
    boxes = []
    for e in elements:
        (x0, y0, z0), (x1, y1, z1) = element_bounds([e])
        boxes.append((x0 - eps, y0 - eps, z0 - eps, x1 + eps, y1 + eps, z1 + eps))

    parent = list(range(len(boxes)))

    def find(i: int) -> int:
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i

    # 按 x 排序后扫描：断开处的组数很少，全对比也跑得动，但排序能省掉大半比较
    order = sorted(range(len(boxes)), key=lambda i: boxes[i][0])
    for a in range(len(order)):
        ia = order[a]
        for b in range(a + 1, len(order)):
            ib = order[b]
            if boxes[ib][0] > boxes[ia][3]:
                break  # x 已经错开，后面按序只会更远
            if (boxes[ia][1] <= boxes[ib][4] and boxes[ib][1] <= boxes[ia][4]
                    and boxes[ia][2] <= boxes[ib][5] and boxes[ib][2] <= boxes[ia][5]):
                ra, rb = find(ia), find(ib)
                if ra != rb:
                    parent[ra] = rb

    groups: dict[int, list[int]] = {}
    for i in range(len(boxes)):
        groups.setdefault(find(i), []).append(i)
    if len(groups) <= 1:
        return []
    ranked = sorted(groups.values(), key=len, reverse=True)
    out = [f"模型断成 {len(ranked)} 块（应为 1 块）"]
    for g in ranked[1:6]:
        names = sorted({elements[i]["name"] for i in g})
        out.append(f"    悬空 {len(g)} 件：{', '.join(names[:5])}"
                   + ("…" if len(names) > 5 else ""))
    return out


def check(size: str, morph: str, verbose: bool = True) -> int:
    skel, B = build(size, morph)
    # **先把坐标烘到世界系**再做任何按坐标判事的检查。自带骨的羽（quill）存的是骨局部
    # 坐标、rotation 归零，朝向烙在骨的绑定旋转里 —— 直接读 element 拿到的是一根根竖着
    # 的板。少了这一步，镜像/贴地/连通/漏光四项会对整片翼**静悄悄地全部通过**。
    baked = {e["uuid"]: e for e in skel.baked_elements()}
    world = [baked.get(e["uuid"], e) for e in skel.data["elements"]]
    # 羽层挂的件带 _muscle 标记（SoftTissue 统一打的），这里只看本层新增的那批
    added = [baked.get(e["uuid"], e) for e in skel.added() if _is_pelt(e["name"])]
    problems = list(mirror_violations(added))

    (_x0, y0, _z0), (_x1, y1, _z1) = element_bounds(added)
    if y0 < -0.05 * B.U:
        problems.append(f"羽层插地：最低点 y={y0:.2f}")

    # 头颈必须**没有羽** —— 秃鹫的立身之本。
    # 判据取**材质**，不取名字：这条规矩管的是"有没有羽被覆盖上去"，裸皮 / 眼球 /
    # 角质这些非羽件本来就该在头上。按名字白名单放行的话，新加一种皮就会被当成
    # "羽长到头上"误杀 —— 改名字比改判据容易得多，那种自检迟早被人绕过去。
    head_z = B.P("skull")[2]
    for e in added:
        if not str(e.get("_mat", "")).startswith("feather") or e["name"].startswith("ruff"):
            continue
        if (e["from"][2] + e["to"][2]) / 2 < head_z + 2.0 * B.U:
            problems.append(f"{e['name']}: 羽长到头颈上了（秃鹫的头颈是裸的）")

    problems += _joined_check(world, B.U)
    problems += _leak_check(size, morph)

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
            print("  ✓ 镜像 / 贴地 / 头颈裸露 / 连通 / 漏光 全部通过")
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
        "primary", "secondary", "covert", "scapular", "ruff", "contour", "breast_cap",
        "rump_cap", "axillary",
        "breast", "rectrix", "tail_covert", "thigh_feather", "thigh_shingle",
        "tarsal_scale", "toe_scale",
        "neck_skin", "head_skin", "face_skin", "gular", "eye_", "crop_pouch",
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
            # 只有「收翼 · 全羽区 · 不叠解剖 · 不摘底层」这一种组合是交付物，
            # 其余（展翼 / 单羽区 / bare / anatomy）都是看细节用的中间产物。
            final = not (args.with_anatomy or args.only_pelt or args.tract
                         or args.pose != "folded")
            if args.with_anatomy:
                name += "_anatomy"
            if args.pose == "spread":
                name += "_spread"
            if args.tract:
                name += f"_{args.tract}"
            if args.only_pelt:
                name += "_bare"
            out_dir = FINAL_DIR if final else MODELS
            out_dir.mkdir(parents=True, exist_ok=True)
            out = out_dir / f"{name}.bbmodel"
            skel.write(out, name)
            print(f"→ {out.relative_to(REPO)}  ({len(skel.data['elements'])} 件)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
