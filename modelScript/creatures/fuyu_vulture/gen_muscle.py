#!/usr/bin/env python3
"""腐羽鹫 —— 肌肉层（Round 1/3）。

**读**已有骨架 bbmodel，按其骨骼 pivot 与骨块几何算附着点，生成覆盖其上的骨骼肌，
输出到新文件。绝不回写骨架 —— 骨架可能已被 Blockbench 手工精修（fmt 5.0），
重跑生成器会冲掉那些改动。

只做骨骼肌 + 撑轮廓的腱膜，不做内脏。鸟的肌肉分布和兽差得极远，照兽做全是错的：

  **飞行肌占体重近三成，全长在胸口**。哺乳类的力量在背和臀，鸟的在龙骨突两侧——
  胸大肌（下扑）把胸脯撑成一个楔，背上反而只有薄薄一层。做成兽那样"背厚胸薄"，
  整只鸟的重心和轮廓就全反了。

  **喙上肌是个滑轮**：它和胸大肌一样长在**腹侧**龙骨上，却负责把翅膀**上举**——
  肌腱向上穿过肩带三骨围成的孔（三骨管），绕到肱骨背面去拉。这是鸟类最精妙的一处
  解剖，也是本层必须做出来的东西：一根从胸口爬上肩头再翻到背面的腱。

  **腿的肉全在上段**：髂胫肌包住髂骨到膝，跗跖以下只有腱没有肉（鸡爪上那几根筋）。
  给跗跖裹肌腹，出来就是根火腿。

  **翼前缘的三角膜（propatagium）**决定翼的轮廓：没有它，展翼的前缘是肩-肘-腕
  三点连成的折线；有了它才是一条绷紧的直边。它是腱膜不是肌肉，但不做翼就不成形。

附着点尽量由骨骼 pivot 与骨块包围盒派生（P("humerus_l") / box("keel")），
骨架一改肌肉自动跟随。

用法:
  python3 modelScript/creatures/fuyu_vulture/gen_muscle.py                     # 三档，骨+肌
  python3 modelScript/creatures/fuyu_vulture/gen_muscle.py --size mid
  python3 modelScript/creatures/fuyu_vulture/gen_muscle.py --size mid --only-muscle
  python3 modelScript/creatures/fuyu_vulture/gen_muscle.py --size mid --group flight
  python3 modelScript/creatures/fuyu_vulture/gen_muscle.py --size mid --explode 5
  python3 modelScript/creatures/fuyu_vulture/gen_muscle.py --check
  python3 modelScript/creatures/fuyu_vulture/gen_muscle.py --list
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from gen_skeleton import SPECS  # noqa: E402
# --- modelScript 路径引导：共用底座在 core/ ---
import sys as _sys
from pathlib import Path as _Path
_sys.path.insert(0, str(_Path(__file__).resolve().parents[2] / "core"))
from bbmodel_maker.rig.rigkit import (  # noqa: E402
    Skeleton, SoftTissue, Vec, element_bounds, lerp, mirror_violations, normalize, perp_to,
)

REPO = HERE.parents[2]
MODELS = Path(__file__).resolve().parents[2] / "models" / "fuyu_vulture" / "layers"  # 肌肉层是中间产物

# 肌肉材质追加在贴图第 2 行（第 1 行 8 个骨/角/爪色块保持原位，
# 这样读进来的骨骼 element 的 UV 一个都不用改）。
MUSCLE_MATS = {
    "muscle": (150, 58, 52),  # 浅层肌腹
    "muscle_deep": (104, 38, 35),  # 深层肌（喙上肌一类）
    "tendon": (208, 198, 174),  # 腱 / 腱膜
    "patagium": (126, 96, 72),  # 翼膜（皮膜）。比骨深一档，否则贴在骨边上分不出来
}
MUSCLE_ROW = 1

GROUPS = ("head", "neck", "trunk", "flight", "wing", "leg", "tail")
GROUP_LABEL = {
    "head": "Head: 颞肌 / 翼肌（撕肉的颌力）",
    "neck": "Neck: 复肌 / 颈长肌（S 颈的驱动）",
    "trunk": "Trunk: 背最长肌 / 肋间肌 / 腹斜肌（薄层，铺满就行）",
    "flight": "Flight: 胸大肌 / 喙上肌 + 三骨管滑轮腱",
    "wing": "Wing: 三角肌 / 肱三头 / 肱二头 / 前臂肌 / 翼膜",
    "leg": "Leg: 髂胫肌 / 髂腓肌 / 腓肠肌 / 趾屈肌腱",
    "tail": "Tail: 提尾肌 / 降尾肌",
}


# ---------------------------------------------------------------- 向量小工具
_norm = normalize


def _sub(a: Vec, b: Vec) -> Vec:
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def A_len(a: Vec, b: Vec) -> float:
    return math.sqrt(sum((x - y) ** 2 for x, y in zip(a, b)))


def _mix(a: Vec, b: Vec, t: float) -> Vec:
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t))


def _off(p: Vec, dx: float = 0.0, dy: float = 0.0, dz: float = 0.0) -> Vec:
    return (p[0] + dx, p[1] + dy, p[2] + dz)


def _along(a: Vec, b: Vec, t: float, out: Vec = (0.0, 0.0, 0.0), amount: float = 0.0) -> Vec:
    """a→b 上 t 处，再沿 out 方向偏 amount（把肌腹推到骨的外侧，不与骨共面）。"""
    p = _mix(a, b, t)
    o = _norm(out) if any(out) else (0.0, 0.0, 0.0)
    return (p[0] + o[0] * amount, p[1] + o[1] * amount, p[2] + o[2] * amount)


# ================================================================ 解剖上下文
class Body:
    """从骨架里读出肌肉需要的全部锚点。"""

    def __init__(self, skel: Skeleton, pose: str = "folded") -> None:
        self.s = skel
        self.pose = pose
        (x0, y0, z0), (x1, y1, z1) = element_bounds(skel.data["elements"])
        # 尺度基准从**骨架实测**取，不从 spec 抄 —— 骨架被手工改过也能跟得上
        self.U = (z1 - z0) / 38.0
        self.height = y1
        self.keel = skel.box("keel")
        self.sternum = skel.box("sternum_plate")
        self.trunk_y = skel.P("trunk_front")[1]

    def P(self, bone: str) -> Vec:
        return self.s.P(bone)

    def wing_chain(self, side: str) -> tuple[Vec, Vec, Vec, Vec]:
        """(肩, 肘, 腕, 翼尖)。翼尖从主指骨块的包围盒取，指骨没有自己的 pivot。"""
        sh = self.P(f"humerus_{side}")
        el = self.P(f"ulna_{side}")
        wr = self.P(f"carpus_{side}")
        (dx0, dy0, dz0), (dx1, dy1, dz1) = self.s.box(f"digit2_{side}_p2")
        # 取指骨包围盒里离腕最远的那个角 = 翼尖
        best, bd = None, -1.0
        for cx in (dx0, dx1):
            for cy in (dy0, dy1):
                for cz in (dz0, dz1):
                    d = sum((a - b) ** 2 for a, b in zip((cx, cy, cz), wr))
                    if d > bd:
                        best, bd = (cx, cy, cz), d
        return sh, el, wr, best

    def leg_chain(self, side: str) -> tuple[Vec, Vec, Vec, Vec]:
        return (self.P(f"femur_{side}"), self.P(f"tibiotarsus_{side}"),
                self.P(f"tarsometatarsus_{side}"), self.P(f"toes_{side}"))

    def neck_bones(self) -> list[str]:
        names = [n for n in self.s.pivots if n.startswith("neck_")]
        return sorted(names, key=lambda n: int(n.split("_")[1]))

    def tail_bones(self) -> list[str]:
        names = [n for n in self.s.pivots if n.startswith("tail_")]
        return sorted(names, key=lambda n: int(n.split("_")[1]))


# ================================================================ 飞行肌
def group_flight(m: SoftTissue, B: Body) -> None:
    """胸大肌（下扑）+ 喙上肌（上举，经三骨管滑轮）。

    这两块加起来是全身最重的肌肉。胸大肌铺在最外层、包住龙骨两侧撑出胸脯；
    喙上肌藏在它**深面**，肌腱翻上肩头去拉肱骨背侧。
    """
    U = B.U
    (kx0, ky0, kz0), (kx1, ky1, kz1) = B.keel
    (sx0, sy0, sz0), (sx1, sy1, sz1) = B.sternum

    for sgn, side in ((-1, "l"), (1, "r")):
        sh, el, _wr, _tip = B.wing_chain(side)
        axis = _norm(_sub(el, sh))
        # 肱骨近端的两个止点：腹侧（胸大肌）与背侧（喙上肌）
        insert_v = _along(sh, el, 0.20, (sgn * 0.35, -1.0, 0.0), 0.85 * U)
        insert_d = _along(sh, el, 0.17, (sgn * 0.35, 1.0, 0.0), 0.85 * U)

        # --- 胸大肌：沿龙骨全长起、收束到肱骨腹侧的**扇面** ---
        # 分片而不是分粗束：4 根粗香肠并排渲出来就是四根柱子，撑不出一整块肌肉。
        # 每片做扁（flat），并排叠起来才连成面；片要经胸廓外缘绕过去，直线穿过去
        # 会从胸腔里横穿而过。
        n_pec = 7
        for k in range(n_pec):
            t = k / (n_pec - 1)
            z = lerp(kz0 + 0.5 * U, kz1 - 0.7 * U, t)
            origin = (sgn * (abs(kx1) + 0.35 * U), lerp(ky0 + 0.9 * U, sy1 - 0.2 * U, 0.25 + 0.5 * t), z)
            # 绕行点：胸廓外缘，中段最鼓
            bulge = math.sin(math.pi * (0.15 + 0.75 * t)) ** 0.7
            waypoint = (sgn * (abs(kx1) + lerp(1.5, 3.3, bulge) * U),
                        lerp(origin[1], insert_v[1], 0.55),
                        lerp(z, insert_v[2], 0.35))
            r = lerp(1.25, 0.85, abs(t - 0.4) / 0.6) * U
            m.belly(f"coracoid_{side}", f"pectoralis_{side}_{k + 1}a", origin, waypoint,
                    r, r_end=0.75 * U, mat="muscle", flat=0.62)
            m.belly(f"coracoid_{side}", f"pectoralis_{side}_{k + 1}b", waypoint, insert_v,
                    r * 0.85, r_end=0.45 * U, mat="muscle", flat=0.62)

        # --- 喙上肌：深层，起于龙骨基部，肌腱穿三骨管翻到肱骨背侧 ---
        deep_origin = (sgn * (abs(kx1) + 0.45 * U), lerp(ky0, sy1, 0.42), lerp(kz0, kz1, 0.32))
        canal = _off(sh, dx=sgn * -0.55 * U, dy=-0.35 * U, dz=-0.35 * U)  # 三骨管口，肩关节内侧
        m.belly(f"coracoid_{side}", f"supracoracoideus_{side}", deep_origin,
                _mix(deep_origin, canal, 0.72), 1.05 * U, r_end=0.45 * U, mat="muscle_deep")
        # 滑轮腱：肌腹顶端 → 管口 → 翻过肩头 → 肱骨背侧止点
        over = _off(sh, dx=sgn * 0.30 * U, dy=1.05 * U, dz=-0.10 * U)
        m.strut(f"coracoid_{side}", f"supracoracoid_tendon_{side}_1",
                _mix(deep_origin, canal, 0.70), canal, 0.34 * U, mat="tendon")
        m.strut(f"coracoid_{side}", f"supracoracoid_tendon_{side}_2", canal, over, 0.30 * U, mat="tendon")
        m.strut(f"humerus_{side}", f"supracoracoid_tendon_{side}_3", over, insert_d, 0.28 * U, mat="tendon")

        # 乌喙骨表面的一层薄肌（coracobrachialis），把肩窝填实
        m.belly(f"coracoid_{side}", f"coracobrachialis_{side}",
                _off(sh, dx=sgn * 0.2 * U, dy=-2.6 * U, dz=0.4 * U), insert_v, 0.75 * U,
                r_end=0.35 * U, mat="muscle_deep")
        _ = axis


# ================================================================ 翼
def group_wing(m: SoftTissue, B: Body) -> None:
    """三角肌 / 肱三头 / 肱二头 / 前臂伸屈肌 / 翼前缘三角膜 + 后缘腱膜。"""
    U = B.U
    for sgn, side in ((-1, "l"), (1, "r")):
        sh, el, wr, tip = B.wing_chain(side)
        up_h = _norm((sgn * 0.25, 1.0, 0.0))
        dn_h = _norm((sgn * 0.25, -1.0, 0.0))

        # 三角肌：肩背侧 → 肱骨背侧中段
        m.belly(f"humerus_{side}", f"deltoideus_{side}",
                _off(sh, dx=sgn * 0.5 * U, dy=1.2 * U, dz=-0.3 * U),
                _along(sh, el, 0.52, up_h, 0.75 * U), 0.90 * U, r_end=0.38 * U)
        # 肱三头：肩胛/肱骨背侧 → 尺骨近端（伸肘）
        m.belly(f"humerus_{side}", f"triceps_{side}",
                _along(sh, el, 0.12, up_h, 0.70 * U),
                _along(el, wr, 0.14, up_h, 0.55 * U), 0.95 * U, r_end=0.40 * U)
        # 肱二头：肱骨腹侧 → 桡骨近端（屈肘）
        m.belly(f"humerus_{side}", f"biceps_{side}",
                _along(sh, el, 0.18, dn_h, 0.62 * U),
                _along(el, wr, 0.20, dn_h, 0.45 * U), 0.70 * U, r_end=0.30 * U,
                mat="muscle_deep")

        # 前臂：伸腕肌沿前缘、屈腕肌沿后缘。前臂以下鸟就只剩腱了，肌腹到腕为止。
        # 伸腕肌在翼**前缘**、屈腕肌在后缘，两者沿翼面岔开。参考向量的 x 分量必须为 0，
        # 否则左右两侧各岔各的（手搓 (-fz,0,fx) 就是这么翻的车）。
        perp = perp_to(el, wr, (0.0, 0.0, 1.0) if B.pose == "spread" else (0.0, 1.0, 0.0))
        m.belly(f"ulna_{side}", f"extensor_carpi_{side}",
                _along(el, wr, 0.10, perp, 0.55 * U), _along(el, wr, 0.72, perp, 0.35 * U),
                0.62 * U, r_end=0.26 * U)
        m.belly(f"ulna_{side}", f"flexor_carpi_{side}",
                _along(el, wr, 0.12, perp, -0.55 * U), _along(el, wr, 0.70, perp, -0.32 * U),
                0.50 * U, r_end=0.22 * U, mat="muscle_deep")

        # --- 翼前缘三角膜（propatagium）：肩 → 腕拉一条直边，中间填成三角 ---
        # 骨链是肩-肘-腕的折线（肘后掠），膜填在肩腕连线的**前侧**，把折角补成一条
        # 绷紧的前缘 —— 翼的轮廓全靠它。
        #
        # 展开方向必须落在翼面内：展翼时翼面近水平，膜朝**前**（-z）铺；收翼时翼面
        # 竖在体侧，膜朝上兜。Round 1 一律按 y 方向铺，展翼时膜立成了一排竖薄片，
        # 从正面几乎看不见。
        spread = B.pose == "spread"
        lead = perp_to(sh, wr, (0.0, 0.0, 1.0) if spread else (0.0, 1.0, 0.0))
        # 取背离肘的那一侧 = 前缘
        if sum(lead[i] * (el[i] - _mix(sh, wr, 0.5)[i]) for i in range(3)) > 0:
            lead = (-lead[0], -lead[1], -lead[2])
        w_max = A_len(sh, el) * (0.55 if spread else 0.20)  # 收翼时膜折起来，只留一道边
        lead_n = 9  # 段少了三角就成了两级台阶，前缘看着是折的
        for k in range(lead_n):
            t0, t1 = k / lead_n, (k + 1) / lead_n
            a, b = _mix(sh, wr, t0), _mix(sh, wr, t1)
            # 三角：两端收到 0，肘对应处（t≈0.45）最宽
            w = w_max * math.sin(math.pi * min(1.0, (t0 + t1) / 2 / 0.92)) ** 0.85
            a2 = (a[0] + lead[0] * w, a[1] + lead[1] * w, a[2] + lead[2] * w)
            b2 = (b[0] + lead[0] * w, b[1] + lead[1] * w, b[2] + lead[2] * w)
            lo = [min(a[i], b[i], a2[i], b2[i]) for i in range(3)]
            hi = [max(a[i], b[i], a2[i], b2[i]) for i in range(3)]
            for i in range(3):  # 膜极薄，但不能薄成零厚度的面
                if hi[i] - lo[i] < 0.22 * U:
                    mid = (hi[i] + lo[i]) / 2
                    lo[i], hi[i] = mid - 0.11 * U, mid + 0.11 * U
            m.piece(f"humerus_{side}", f"propatagium_{side}_{k + 1}",
                    (lo[0], lo[1], lo[2]), (hi[0], hi[1], hi[2]), mat="patagium")
        # 后缘腱膜（postpatagium）：次级飞羽的根，沿尺骨后缘一条薄带
        m.strut(f"ulna_{side}", f"postpatagium_{side}",
                _along(el, wr, 0.06, perp, -0.85 * U), _along(el, wr, 0.94, perp, -0.75 * U),
                0.14 * U, 0.55 * U, mat="patagium")
        # 手部只有腱：掌骨背侧一条，牵初级飞羽
        m.strut(f"manus_{side}", f"manus_tendon_{side}", _mix(wr, tip, 0.08), _mix(wr, tip, 0.88),
                0.16 * U, 0.26 * U, mat="tendon")


# ================================================================ 颈
def group_neck(m: SoftTissue, B: Body) -> None:
    """复肌（背侧）+ 颈长肌（腹侧），逐节铺。

    鸟颈的肌肉是**一节一段**的短束，不是一根长肌 —— 长肌拉不出 S 形的分段弯曲。
    """
    U = B.U
    bones = B.neck_bones()
    pts = [B.P(b) for b in bones]
    n = len(bones)
    for i in range(n - 1):
        a, b = pts[i], pts[i + 1]
        t = i / max(1, n - 2)
        d = _norm(_sub(b, a))
        # 背侧法线：切线在矢状面内顺时针转 90°（与骨架的棘突方向同源）
        back = _norm((0.0, -d[2], d[1]))
        belly_r = lerp(1.05, 0.70, abs(t - 0.35) / 0.65) * U
        m.belly(bones[i], f"complexus_{n - i}", _off(a, dy=back[1] * 0.5 * U, dz=back[2] * 0.5 * U),
                _off(b, dy=back[1] * 0.5 * U, dz=back[2] * 0.5 * U), belly_r, r_end=belly_r * 0.7)
        m.belly(bones[i], f"longus_colli_{n - i}",
                _off(a, dy=-back[1] * 0.45 * U, dz=-back[2] * 0.45 * U),
                _off(b, dy=-back[1] * 0.45 * U, dz=-back[2] * 0.45 * U),
                belly_r * 0.72, r_end=belly_r * 0.5, mat="muscle_deep")
        # 侧束（左右一对，转头用）
        for sgn, side in ((-1, "l"), (1, "r")):
            m.belly(bones[i], f"cervical_lateral_{n - i}_{side}",
                    _off(a, dx=sgn * belly_r * 0.85), _off(b, dx=sgn * belly_r * 0.85),
                    belly_r * 0.5, r_end=belly_r * 0.34, mat="muscle_deep")


# ================================================================ 头
def group_head(m: SoftTissue, B: Body) -> None:
    """颞肌 + 翼肌：撕开尸皮那一口的力气来源。"""
    U = B.U
    skull = B.P("skull")
    jaw = B.P("jaw")
    for sgn, side in ((-1, "l"), (1, "r")):
        # 颞肌：颅侧后上 → 下颌升支
        m.belly("skull", f"adductor_mandibulae_{side}",
                (sgn * 1.35 * U, skull[1] + 0.9 * U, skull[2] + 1.5 * U),
                (sgn * 1.15 * U, jaw[1] + 0.5 * U, jaw[2] + 0.2 * U),
                0.72 * U, r_end=0.34 * U)
        # 翼肌（深层，颅底 → 下颌内侧）
        m.belly("skull", f"pterygoideus_{side}",
                (sgn * 0.75 * U, skull[1] - 1.5 * U, skull[2] - 0.4 * U),
                (sgn * 0.85 * U, jaw[1] - 0.1 * U, jaw[2] - 1.6 * U),
                0.46 * U, r_end=0.24 * U, mat="muscle_deep")
    # 颈-颅交界的项韧带止点（抬头靠它）
    m.strut("skull", "nuchal_ligament", _off(skull, dy=1.5 * U, dz=1.2 * U),
            _off(B.P(B.neck_bones()[2]), dy=1.1 * U), 0.30 * U, mat="tendon")


# ================================================================ 腿
def group_leg(m: SoftTissue, B: Body) -> None:
    """髂胫肌 / 髂腓肌 / 腓肠肌 / 趾屈肌腱。

    肉全在髋到膝这一段，跗跖以下**只有腱**——给跗跖裹肌腹就成了火腿肠。
    """
    U = B.U
    for sgn, side in ((-1, "l"), (1, "r")):
        hip, knee, ankle, toe = B.leg_chain(side)
        out = _norm((sgn * 1.0, 0.25, 0.0))
        # 髂胫肌：髂骨背缘 → 膝，鸟腿最大的一块，把大腿包成一个饱满的锥
        m.belly(f"femur_{side}", f"iliotibialis_{side}",
                _off(hip, dx=sgn * 1.15 * U, dy=1.9 * U, dz=-0.4 * U),
                _along(knee, ankle, 0.12, out, 1.05 * U), 2.05 * U, r_end=0.70 * U)
        # 髂腓肌：髂骨后 → 腓骨近端（外展 + 屈膝）
        m.belly(f"femur_{side}", f"iliofibularis_{side}",
                _off(hip, dx=sgn * 0.9 * U, dy=1.4 * U, dz=2.2 * U),
                _along(knee, ankle, 0.22, out, 0.75 * U), 1.05 * U, r_end=0.40 * U,
                mat="muscle_deep")
        # 股内侧（深层，贴着股骨把髋窝填实）
        m.belly(f"femur_{side}", f"femorotibialis_{side}",
                _off(hip, dx=sgn * -0.15 * U, dy=-0.4 * U),
                _mix(knee, ankle, 0.10), 1.10 * U, r_end=0.45 * U, mat="muscle_deep")
        # 腓肠肌：膝后 → 跟腱 → 跗跖近端。肌腹只占胫跗上 2/3，之后收成腱。
        calf_end = _mix(knee, ankle, 0.66)
        m.belly(f"tibiotarsus_{side}", f"gastrocnemius_{side}",
                _along(knee, ankle, 0.08, (sgn * 0.2, 0.0, 1.0), 0.85 * U),
                _along(knee, ankle, 0.66, (sgn * 0.15, 0.0, 1.0), 0.55 * U),
                1.15 * U, r_end=0.42 * U)
        m.strut(f"tibiotarsus_{side}", f"achilles_{side}", calf_end,
                _off(ankle, dz=0.55 * U), 0.26 * U, 0.34 * U, mat="tendon")
        # 趾屈肌腱：过 hypotarsus 滑车 → 沿跗跖后侧 → 分到趾。
        # 鸟站着睡不掉下来就靠这根：踝一弯，腱被拉紧，趾自动扣住。
        m.strut(f"tarsometatarsus_{side}", f"flexor_tendon_{side}",
                _off(ankle, dz=0.62 * U, dy=-0.3 * U), _off(toe, dz=0.34 * U), 0.20 * U, 0.30 * U,
                mat="tendon")
        # 跗跖前侧的伸肌腱（细）
        m.strut(f"tarsometatarsus_{side}", f"extensor_tendon_{side}",
                _off(ankle, dz=-0.52 * U), _off(toe, dz=-0.26 * U), 0.15 * U, 0.22 * U,
                mat="tendon")


# ================================================================ 躯干
def group_trunk(m: SoftTissue, B: Body) -> None:
    """背最长肌 + 肋间肌 + 腹斜肌。

    鸟背上的肉**很薄** —— 胸椎已经愈合成一根刚性梁，没有逐节活动就用不着大块伸肌。
    但薄不等于没有：不铺这一层，背脊就永远是一条露在外面的白骨。
    腹壁同理，缺了它龙骨后方是个空洞。
    """
    U = B.U
    ty = B.trunk_y
    t1 = B.P("trunk_front")
    hips = B.P("hips")
    (_sx0, sy0, sz0), (sx1, _sy1, sz1) = B.sternum

    for sgn, side in ((-1, "l"), (1, "r")):
        # 背最长肌：沿愈合脊两侧的一条薄带，从 T1 铺到髋
        m.belly("trunk_front", f"longissimus_{side}",
                (sgn * 0.95 * U, ty + 0.85 * U, t1[2] + 0.5 * U),
                (sgn * 1.15 * U, ty + 0.55 * U, hips[2] + 1.5 * U),
                0.85 * U, r_end=0.55 * U, flat=0.78, mat="muscle_deep")
        # 肋间肌：肋弓外侧的一层膜状薄肌，把肋间的空档填上
        m.belly("trunk_front", f"intercostal_{side}",
                (sgn * 2.55 * U, ty - 2.2 * U, t1[2] + 1.2 * U),
                (sgn * 2.75 * U, ty - 3.4 * U, hips[2] - 1.0 * U),
                1.05 * U, r_end=0.72 * U, flat=0.42, mat="muscle_deep")
        # 腹斜肌：胸骨后缘 → 耻骨，兜住腹腔
        m.belly("hips", f"obliquus_{side}",
                (sgn * (abs(sx1) + 0.3 * U), sy0 + 0.6 * U, sz1 - 1.0 * U),
                (sgn * 1.5 * U, hips[1] - 2.2 * U, hips[2] + 3.0 * U),
                1.15 * U, r_end=0.60 * U, flat=0.62)


# ================================================================ 尾
def group_tail(m: SoftTissue, B: Body) -> None:
    """提尾肌 / 降尾肌 —— 尾羽扇形的开合靠这一对。"""
    U = B.U
    bones = B.tail_bones()
    if not bones:
        return
    root = B.P(bones[0])
    pyg = B.P("pygostyle")
    m.belly(bones[0], "levator_caudae", _off(root, dy=1.15 * U, dz=-1.4 * U),
            _off(pyg, dy=0.85 * U), 0.95 * U, r_end=0.36 * U)
    m.belly(bones[0], "depressor_caudae", _off(root, dy=-1.05 * U, dz=-1.2 * U),
            _off(pyg, dy=-0.75 * U), 0.80 * U, r_end=0.32 * U, mat="muscle_deep")
    for sgn, side in ((-1, "l"), (1, "r")):
        m.belly(bones[0], f"lateralis_caudae_{side}",
                _off(root, dx=sgn * 1.0 * U, dz=-1.0 * U), _off(pyg, dx=sgn * 0.55 * U),
                0.52 * U, r_end=0.24 * U, mat="muscle_deep")


BUILDERS = {
    "head": group_head,
    "neck": group_neck,
    "trunk": group_trunk,
    "flight": group_flight,
    "wing": group_wing,
    "leg": group_leg,
    "tail": group_tail,
}


# ================================================================ 装配
def build(size: str, groups: tuple[str, ...] = GROUPS, *, pose: str = "folded") -> tuple[Skeleton, Body]:
    spec = SPECS[size]
    name = spec.model + ("_spread" if pose == "spread" else "")
    src = MODELS / f"{name}.bbmodel"
    if not src.exists():
        raise SystemExit(f"骨架不存在：{src}\n先跑 gen_skeleton.py --size {size}"
                         + (" --pose spread" if pose == "spread" else ""))
    skel = Skeleton(src)
    skel.extend_texture(MUSCLE_MATS, MUSCLE_ROW)
    B = Body(skel, pose)
    m = SoftTissue(skel, MUSCLE_MATS, MUSCLE_ROW)
    for g in groups:
        BUILDERS[g](m, B)
    return skel, B


def explode(skel: Skeleton, B: Body, amount: float) -> None:
    """把每个软组织件沿"离体轴的径向"推开，看清层次与附着。"""
    axis_y = B.trunk_y
    for e in skel.added():
        cx = (e["from"][0] + e["to"][0]) / 2
        cy = (e["from"][1] + e["to"][1]) / 2
        d = _norm((cx, cy - axis_y, 0.0))
        for k, a in ((0, d[0]), (1, d[1])):
            e["from"][k] += a * amount
            e["to"][k] += a * amount
            e["origin"][k] += a * amount


def check(size: str, verbose: bool = True) -> int:
    skel, B = build(size)
    added = skel.added()
    problems = list(mirror_violations(added))

    (_x0, y0, _z0), (_x1, y1, _z1) = element_bounds(added)
    if y0 < -0.05 * B.U:
        problems.append(f"软组织插地：最低点 y={y0:.2f}")
    # 跗跖以下不该有肌腹（只有腱）——鸟腿下段没肉，裹上就是火腿
    for e in added:
        if "muscle" in e["name"] or "gastro" in e["name"]:
            ankle_y = min(B.P("tarsometatarsus_l")[1], B.P("tarsometatarsus_r")[1])
            if (e["from"][1] + e["to"][1]) / 2 < ankle_y - 0.5 * B.U:
                problems.append(f"{e['name']}: 肌腹长到踝以下（鸟腿下段只有腱）")

    if verbose:
        spec = SPECS[size]
        print(f"[{size}] {spec.cn}")
        print(f"  软组织 {len(added)} 件（骨 {len(skel.data['elements']) - len(added)} 件）· U={B.U:.3f}")
        by = {}
        for e in added:
            key = e["name"].rsplit("_", 1)[0].rstrip("_lr")
            by[key] = by.get(key, 0) + 1
        print(f"  肌群 {len(by)} 组")
        if problems:
            print(f"  ✗ {len(problems)} 处违例：")
            for p in problems[:12]:
                print(f"     {p}")
        else:
            print("  ✓ 镜像 / 贴地 / 腿下段无肌腹 全部通过")
    return len(problems)


def main() -> int:
    ap = argparse.ArgumentParser(description="腐羽鹫肌肉层生成器")
    ap.add_argument("--size", choices=sorted(SPECS), help="只出单档（默认三档全出）")
    ap.add_argument("--pose", choices=("folded", "spread"), default="folded")
    ap.add_argument("--group", choices=GROUPS, help="只生成单个肌群（预览用）")
    ap.add_argument("--only-muscle", action="store_true", help="摘掉骨骼，只留软组织")
    ap.add_argument("--explode", type=float, default=0.0, help="沿径向散开的距离（看层次）")
    ap.add_argument("--check", action="store_true", help="只跑自检，不写文件")
    ap.add_argument("--list", action="store_true", help="列出肌群")
    args = ap.parse_args()

    if args.list:
        for g in GROUPS:
            print(f"  {g:8s} {GROUP_LABEL[g]}")
        return 0

    keys = [args.size] if args.size else ["small", "mid", "large"]

    if args.check:
        bad = 0
        for k in keys:
            bad += check(k)
            print()
        return 1 if bad else 0

    for k in keys:
        groups = (args.group,) if args.group else GROUPS
        skel, B = build(k, groups, pose=args.pose)
        if args.explode > 0:
            explode(skel, B, args.explode * B.U)
        if args.only_muscle:
            skel.keep_only_added()
        name = SPECS[k].model.replace("Skeleton", "Muscle")
        if args.pose == "spread":
            name += "_spread"
        if args.group:
            name += f"_{args.group}"
        if args.only_muscle:
            name += "_bare"
        if args.explode > 0:
            name += "_explode"
        out = MODELS / f"{name}.bbmodel"
        skel.write(out, name)
        print(f"→ {out.relative_to(REPO)}")
        print(f"   软组织 {len(skel.added())} 件 · 总 {len(skel.data['elements'])} 件")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
