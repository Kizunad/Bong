#!/usr/bin/env python3
"""马 —— 肌肉层（Round 1/3）。三档体型各生成一份。

**读**已有骨架 bbmodel，按其骨骼 pivot 算附着点，生成覆盖其上的骨骼肌，输出到新文件。
绝不回写骨架——骨架可能已在 Blockbench 里手工精修过，生成器覆盖会冲掉那些改动。

只做**骨骼肌**，不做内脏。马与猫科差别最大的三处刻意做足：

  ① **项韧带**（nuchal ligament）——自鬐甲棘突拉到枕的一根粗弹性索，颈椎走颈下缘，
     颈脊那道隆起全靠它撑起来。猫科没有这东西；少了它，马颈就是一根细杆。
  ② **前锯肌吊索**——马无锁骨，前肢与躯干之间没有任何骨性连接，整个躯干是**吊**在
     两块肩胛之间的。前锯肌是这条吊索的主体，必须做厚，否则前躯像挂在两根棍上。
  ③ **腕/跗以下无肌肉**——管骨区皮下只有骨与腱：指伸肌腱走前缘、指深/浅屈肌腱走后缘
     并在球节后绕过籽骨，中间夹悬韧带。照猫科那样把前臂肌一路铺到蹄，马腿就没形了。

附着点全部由骨骼 pivot 派生（`P("femur_l")` 等），所以骨架一改肌肉自动跟随；
只有贴在颅骨具体骨面上的颞肌/咬肌，才走 gen_skeleton 的头部局部坐标系。

肌腹取梭形——3 段，中段粗两端收；单段直筒看着像水管。

用法:
  python3 scripts/models/horse/gen_muscle.py                      # 三档，骨+肌
  python3 scripts/models/horse/gen_muscle.py --profile medium
  python3 scripts/models/horse/gen_muscle.py --only-muscle        # 只看肌肉
  python3 scripts/models/horse/gen_muscle.py --group hindleg
  python3 scripts/models/horse/gen_muscle.py --diagnose           # 裸骨诊断（体素栅格）
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import uuid
from pathlib import Path

from gen_skeleton import HEAD_PITCH, PROFILES, HeadSpace, Profile, neck_centers, shaft_box, uid
from gen_skeleton import rib_surface_point as _rib_pt
from PIL import Image

REPO = Path(__file__).resolve().parents[3]
MODELS = REPO / "local_models" / "horse"

# 肌肉材质追加在贴图第 2 行（第 1 行 6 个骨/软骨/牙/蹄色块保持原位，
# 这样读进来的骨骼 element 的 UV 一个都不用改）。
MUSCLE_MATS = {
    "muscle": (150, 60, 52),  # 浅层肌腹
    "muscle_deep": (106, 40, 36),  # 深层肌
    "tendon": (208, 198, 176),  # 腱 / 韧带
}
SWATCH = 8
MUSCLE_ROW = 1

Vec = tuple[float, float, float]


def _lerp3(a: Vec, b: Vec, t: float) -> Vec:
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t)


def _off(p: Vec, dx: float = 0.0, dy: float = 0.0, dz: float = 0.0) -> Vec:
    return (p[0] + dx, p[1] + dy, p[2] + dz)


# ---------------------------------------------------------------- 读骨架
class Skeleton:
    """兼容 fmt 4.x（outliner 内联 group）与 fmt 5.0（groups 数组 + uuid 引用树）。"""

    def __init__(self, path: Path) -> None:
        self.path = path
        self.data = json.loads(path.read_text())
        self.groups = {g["uuid"]: g for g in self.data.get("groups", [])}
        self.nodes: dict[str, dict] = {}
        self.pivot: dict[str, Vec] = {}
        for root in self.data["outliner"]:
            self._walk(root)
        if not self.pivot:
            raise SystemExit(f"{path}: 读不到骨骼层级")

    def _walk(self, node) -> None:
        if isinstance(node, str):  # element uuid 叶子
            return
        meta = self.groups.get(node["uuid"], node)
        name = meta.get("name")
        if name is not None:
            self.nodes[name] = node
            self.pivot[name] = tuple(meta["origin"])
        for child in node.get("children", []):
            self._walk(child)

    def P(self, bone: str) -> Vec:
        if bone not in self.pivot:
            raise KeyError(f"骨架里没有骨骼 {bone}（有 {len(self.pivot)} 根）")
        return self.pivot[bone]

    def attach(self, bone: str, element: dict) -> None:
        if bone not in self.nodes:
            raise KeyError(f"骨架里没有骨骼 {bone}")
        self.data["elements"].append(element)
        self.nodes[bone]["children"].append(element["uuid"])

    def drop_bone_cubes(self) -> None:
        """--only-muscle：摘掉原骨骼 cube，只留肌肉。"""
        keep = {e["uuid"] for e in self.data["elements"] if e.get("_muscle")}
        self.data["elements"] = [e for e in self.data["elements"] if e["uuid"] in keep]

        def prune(node):
            if isinstance(node, str):
                return node in keep
            node["children"] = [c for c in node.get("children", []) if prune(c)]
            return True

        for root in self.data["outliner"]:
            prune(root)


# ---------------------------------------------------------------- 贴图
def extend_texture(data: dict) -> None:
    """在原贴图上追加肌肉色块（不动已有色块，骨骼 UV 保持有效）。"""
    src = data["textures"][0]["source"].split(",", 1)[1]
    img = Image.open(io.BytesIO(base64.b64decode(src))).convert("RGBA")
    px = img.load()
    for i, (_name, (r, g, b)) in enumerate(MUSCLE_MATS.items()):
        ox, oy = i * SWATCH, MUSCLE_ROW * SWATCH
        for y in range(SWATCH):
            for x in range(SWATCH):
                n = ((x * 7 + y * 13 + i * 5) % 5) - 2  # 轻噪，肌面不是平涂塑料
                px[ox + x, oy + y] = (
                    max(0, min(255, r + n * 5)),
                    max(0, min(255, g + n * 4)),
                    max(0, min(255, b + n * 4)),
                    255,
                )
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    data["textures"][0]["source"] = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


def _faces(mat: str) -> dict:
    i = list(MUSCLE_MATS).index(mat)
    ox, oy = i * SWATCH, MUSCLE_ROW * SWATCH
    uv = [ox + 1.0, oy + 1.0, ox + SWATCH - 1.0, oy + SWATCH - 1.0]
    return {d: {"uv": list(uv), "texture": 0} for d in ("north", "south", "east", "west", "up", "down")}


# ---------------------------------------------------------------- 肌腹
class MuscleBuilder:
    def __init__(self, skel: Skeleton, P: Profile) -> None:
        self.skel = skel
        self.P = P
        self.count = 0
        self._names: set[str] = set()
        # 肌量随骨粗走：挽马 1.29、矮马 1.07、常马 1.0。
        # 首版取 0.75 让肌只长 24% 而骨长了 32%，粗骨从肌下顶出来——挽马那档
        # 胫骨/管骨的裸骨列数是常马的两倍多。覆盖用的肌必须与被它盖的骨同步缩放。
        self.gauge = 1.0 + (P.bone_gauge - 1.0) * 0.90

    def r(self, frac: float) -> float:
        """肌腹半径：鬐甲高的比例 × 肌量系数。"""
        return self.P.wither * frac * self.gauge

    def _emit(self, bone: str, name: str, p0: Vec, p1: Vec, rx: float, rz: float, mat: str) -> None:
        if name in self._names:
            raise ValueError(f"重复肌肉件名: {name}（uuid 由名字派生，名字必须唯一）")
        self._names.add(name)
        frm, to, rot, org = shaft_box(p0, p1, rx, rz)
        self.skel.attach(
            bone,
            {
                "name": name,
                "box_uv": False,
                "rescale": False,
                "locked": False,
                "render_order": "default",
                "allow_mirror_modeling": True,
                "type": "cube",
                "uuid": uid("muscle", name),
                "_muscle": True,
                "from": [round(v, 3) for v in frm],
                "to": [round(v, 3) for v in to],
                "autouv": 0,
                "color": 4,
                "origin": [round(v, 3) for v in org],
                "rotation": [round(v, 3) for v in rot],
                "faces": _faces(mat),
            },
        )

    def belly(
        self,
        bone: str,
        name: str,
        a: Vec,
        b: Vec,
        r_mid: float,
        *,
        r_end: float | None = None,
        mat: str = "muscle",
        flat: float = 1.0,
    ) -> None:
        """梭形肌腹：沿 a→b 分 3 段，中段最粗。flat<1 = 扁片（背阔肌一类）。"""
        r_end = r_mid * 0.5 if r_end is None else r_end
        cuts = (0.0, 0.26, 0.74, 1.0)
        radii = ((r_end + r_mid) / 2, r_mid, (r_mid + r_end) / 2)
        for i, rr in enumerate(radii):
            self._emit(bone, f"{name}_{i + 1}", _lerp3(a, b, cuts[i]), _lerp3(a, b, cuts[i + 1]), rr * flat, rr, mat)
        self.count += 1

    def strip(self, bone: str, name: str, pts: list[Vec], r: float, *, mat: str = "muscle", flat: float = 1.0) -> None:
        """沿一串点铺一条连续肌带（折线跟随曲面）。

        阔肌（背阔/腹外斜）和绕过球节的屈肌腱用它。把整片阔肌做成一个 flat 的大 cube，
        糊在胸廓上像块木板——扁不等于平，阔肌是**贴曲面**的。
        """
        for i, (p0, p1) in enumerate(zip(pts[:-1], pts[1:])):
            self._emit(bone, f"{name}_{i + 1}", p0, p1, r * flat, r, mat)
        self.count += 1

    def rib_band(self, sx: int, *, fd: float, z0: float, z1: float, n: int = 5, lift: float | None = None) -> list[Vec]:
        """沿肋弓表面等深度线取一串点（供 strip 铺贴合胸廓的肌带）。

        lift = 沿径向往外抬一点，让肌带浮在肋骨表面而不是与之共面（z-fighting）。
        """
        P = self.P
        lift = P.u(0.022) if lift is None else lift
        out: list[Vec] = []
        for i in range(n + 1):
            z = z0 + (z1 - z0) * i / n
            x, y, zz = _rib_pt(P, z, fd, sx)
            nx, ny = (1.0 if sx > 0 else -1.0), 0.0
            if fd > 0.75:  # 腹侧法线偏下
                nx, ny = nx * 0.6, -0.8
            elif fd < 0.25:  # 背侧法线偏上
                nx, ny = nx * 0.85, 0.5
            out.append((x + nx * lift, y + ny * lift, zz))
        return out


# ================================================================ 肌群
def grp_head(m: MuscleBuilder, P) -> None:
    """咬肌 + 颞肌。颅面肌贴具体骨面，走头部局部坐标系（不由 pivot 派生）。

    马是**磨**不是咬：咬肌（下颌角那片腮）远大于颞肌，与猫科正好相反。
    """
    Pr = m.P
    hs = HeadSpace(None, "skull", (0.0, Pr.y_occiput, Pr.z_occiput), HEAD_PITCH)
    h = Pr.h
    for sx, side in ((-1, "l"), (1, "r")):
        # 咬肌：颧弓/面嵴下缘 → 下颌角。马腮的形状就是这块肌
        m.belly(
            "jaw",
            f"masseter_{side}",
            hs.to_world((sx * h(0.128), -h(0.060), -h(0.230))),
            hs.to_world((sx * h(0.120), -h(0.320), -h(0.150))),
            m.r(0.032),
            r_end=m.r(0.024),
            flat=0.62,
        )
        # 颞肌：填颞窝（颅顶 ↔ 下颌冠突），马身上比猫科小得多
        m.belly(
            "skull",
            f"temporalis_{side}",
            hs.to_world((sx * h(0.090), h(0.090), -h(0.190))),
            hs.to_world((sx * h(0.100), h(0.040), -h(0.215))),
            m.r(0.020),
            mat="muscle_deep",
        )


def grp_neck(m: MuscleBuilder, P) -> None:
    """项韧带（索状部 + 板状部）· 夹肌 · 臂头肌 · 胸头肌。

    马颈的形状是这四条决定的：颈椎走**颈下缘**，颈脊那道隆起全是项韧带 + 夹肌堆出来的。
    臂头肌与胸头肌之间留出的那条槽 = 颈静脉沟，活马身上看得见，别用肌肉填掉。
    """
    Pr = m.P
    wither_top = (0.0, Pr.wither - Pr.u(0.030), Pr.z_wither_peak)
    # 止点在**项嵴**（枕上方）而不是枕本身：拉到枕高会让韧带整条压低，
    # C2 的背嵴就从韧带上缘钻出来（裸骨诊断点名 axis_crest）。
    poll = (0.0, Pr.y_poll + Pr.u(0.012), Pr.z_occiput + Pr.u(0.020))

    # 索状部：鬐甲棘突 → 项嵴，一根粗弹性索。颈脊的高度由它定
    m.belly("neck_base", "nuchal_funicular", wither_top, poll, m.r(0.044), r_end=m.r(0.030), mat="tendon", flat=0.62)

    # 颈腹侧带（长颈肌 + 气管沟软组织）：沿颈椎链**下缘**铺一条，把颈的下轮廓补出来。
    # 只铺项韧带侧和两侧的肌，从下方看整条颈椎是裸的（裸骨诊断 [腹] 点名 cervical_2/3）。
    m.strip(
        "neck_base",
        "ventral_neck",
        [_off(c, 0.0, -Pr.u(0.052), 0.0) for c in neck_centers(Pr)],
        m.r(0.040),
        mat="muscle_deep",
        flat=0.82,
    )
    # 板状部：自索状部向下扇到各颈椎棘突（这里取三片代表）
    for k, t in enumerate((0.30, 0.52, 0.74)):
        top = _lerp3(wither_top, poll, t)
        z = top[2]
        m.belly(
            "neck_mid" if t > 0.5 else "neck_base",
            f"nuchal_lamina_{k + 1}",
            top,
            (0.0, Pr.centrum_y(z) + Pr.u(0.040), z),
            m.r(0.020),
            r_end=m.r(0.014),
            mat="tendon",
            flat=0.30,
        )

    for sx, side in ((-1, "l"), (1, "r")):
        # 夹肌：鬐甲 → 枕/寰椎翼，铺在项韧带两侧，把颈填成三角断面。
        # 横向必须推到**颈椎横突之外**（C1 寰椎翼伸到 ±0.15W）：贴着中线挂就成了
        # 一条埋在骨头里的肌，横突一根根裸在体表（裸骨诊断点名 cervical/axis_crest）。
        m.belly(
            "neck_base",
            f"splenius_{side}",
            _off(wither_top, sx * Pr.u(0.078), -Pr.u(0.030)),
            _off(poll, sx * Pr.u(0.052), -Pr.u(0.040), Pr.u(0.030)),
            m.r(0.050),
            r_end=m.r(0.032),
            flat=0.78,
        )
        # 臂头肌：枕/下颌角 → 肱骨。颈前下缘那条长肌，颈静脉沟的上界
        m.belly(
            "neck_base",
            f"brachiocephalicus_{side}",
            _off(P("skull"), sx * Pr.u(0.052), -Pr.u(0.030), Pr.u(0.020)),
            _off(P(f"humerus_{side}"), sx * Pr.u(0.020), Pr.u(0.020), -Pr.u(0.012)),
            m.r(0.040),
            r_end=m.r(0.026),
            flat=0.82,
        )
        # 颈深肌群（长颈肌 + 横突间肌 + 肩胛横突肌）：**沿颈椎链本身**走，不是沿项韧带走。
        # 项韧带是拉直的弦、颈椎是下垂的弧，两者在颈中段差出大半个椎体高——只铺韧带侧的
        # 夹肌，中段横突就从肌下钻出来裸在体表（裸骨诊断点名 cervical_4 / axis_crest）。
        cen = neck_centers(Pr)
        for k in range(len(cen) - 1):
            a, b = cen[k], cen[k + 1]
            m.belly(
                "neck_mid" if k >= 3 else "neck_base",
                f"cervical_wrap_{k + 1}_{side}",
                _off(a, sx * Pr.u(0.058)),
                _off(b, sx * Pr.u(0.050)),
                m.r(0.038),
                r_end=m.r(0.034),
                mat="muscle_deep",
            )
        # 胸头肌：胸骨柄 → 下颌角。颈静脉沟的下界；与臂头肌之间**留槽**
        m.belly(
            "neck_base",
            f"sternocephalicus_{side}",
            (sx * Pr.u(0.020), Pr.y_chest + Pr.u(0.090), Pr.z_t1 - Pr.u(0.030)),
            _off(P("skull"), sx * Pr.u(0.058), -Pr.u(0.086), Pr.u(0.050)),
            m.r(0.022),
            r_end=m.r(0.014),
            mat="muscle_deep",
        )


def grp_torso(m: MuscleBuilder, P) -> None:
    """竖脊肌 · 斜方肌 · 菱形肌 · 背阔肌 · **前锯肌吊索** · 胸肌 · 腹壁。"""
    Pr = m.P
    z_stern_front = Pr.z_t1 - 0.02 * Pr.L
    y_stern = Pr.y_chest + Pr.u(0.020)

    # 胸横肌：沿胸骨腹面铺一条，盖住胸底裸露的肋软骨末端
    m.belly(
        "thorax_front",
        "pectoralis_transverse",
        (0.0, y_stern - Pr.u(0.010), z_stern_front + Pr.u(0.020)),
        (0.0, y_stern - Pr.u(0.020), Pr.z_t1 + 0.28 * Pr.L),
        m.r(0.042),
        flat=1.10,
        mat="muscle_deep",
    )

    # 棘上韧带：沿棘突尖自荐拉到鬐甲，与项韧带在鬐甲接成一条。活马背上摸得到的是
    # 这条韧带不是一根根棘突——少了它，18 根胸椎棘突就一根根梳子似地裸在背线上。
    _lig_z = [Pr.z_sacrum - (Pr.z_sacrum - Pr.z_wither_peak) * k / 6 for k in range(7)]
    m.strip(
        "thorax_back",
        "supraspinous_ligament",
        [(0.0, Pr.dorsal_y(z) + Pr.u(0.006), z) for z in _lig_z],
        Pr.u(0.042),
        mat="tendon",
        flat=0.72,
    )

    chain = ["hips", "lumbar", "thorax_back", "thorax_front", "neck_base"]
    radii = (0.062, 0.066, 0.062, 0.054)
    for sx, side in ((-1, "l"), (1, "r")):
        # 竖脊肌：荐 → 颈的一条连续肌索，背线的隆起主要来自它
        for i, (a_name, b_name) in enumerate(zip(chain, chain[1:])):
            m.belly(
                a_name,
                f"longissimus_{side}_{i + 1}",
                _off(P(a_name), sx * Pr.u(0.062), Pr.u(0.048)),
                _off(P(b_name), sx * Pr.u(0.062), Pr.u(0.048)),
                m.r(radii[i]),
                r_end=m.r(radii[i]) * 0.92,  # 连续肌索，两端不收细
            )
        # 斜方肌：鬐甲 → 肩胛冈，薄片。止点要落在**肩胛外侧面之外**，否则肩胛板
        # 直接裸在体表（裸骨诊断里 scapula_spine/scapula_blade 的来源）。
        m.belly(
            "thorax_front",
            f"trapezius_{side}",
            (sx * Pr.u(0.026), Pr.wither - Pr.u(0.050), Pr.z_wither_peak),
            _off(_lerp3(P(f"scapula_{side}"), P(f"humerus_{side}"), 0.48), sx * Pr.u(0.042), 0.0, Pr.u(0.010)),
            m.r(0.044),
            flat=0.34,
        )
        # 菱形肌：鬐甲 → 肩胛上缘（深层，把肩胛拉向脊柱）
        m.belly(
            "thorax_front",
            f"rhomboideus_{side}",
            (sx * Pr.u(0.020), Pr.wither - Pr.u(0.036), Pr.z_wither_peak + Pr.u(0.040)),
            _off(P(f"scapula_{side}"), sx * Pr.u(0.006), Pr.u(0.010), Pr.u(0.010)),
            m.r(0.030),
            flat=0.42,
            mat="muscle_deep",
        )
        # 背阔肌：沿肋弓上段自前向后铺的肌带，跟着胸廓曲面走
        m.strip(
            "thorax_back",
            f"latissimus_{side}",
            m.rib_band(sx, fd=0.20, z0=Pr.z_t1 + Pr.u(0.040), z1=Pr.z_t18 + Pr.u(0.090)),
            m.r(0.062),
            flat=0.36,
        )
        # ---- 前锯肌吊索：马无锁骨，躯干是**吊**在两块肩胛之间的 ----
        # 颈段：颈椎横突 → 肩胛内面上缘
        m.belly(
            "thorax_front",
            f"serratus_cervicis_{side}",
            _off(P("neck_mid"), sx * Pr.u(0.044), -Pr.u(0.020)),
            _off(P(f"scapula_{side}"), sx * Pr.u(0.004), -Pr.u(0.020), Pr.u(0.040)),
            m.r(0.050),
            r_end=m.r(0.030),
            flat=0.50,
            mat="muscle_deep",
        )
        # 胸段：中段肋外面 → 肩胛内面。这条是吊索主体，做厚
        m.belly(
            "thorax_front",
            f"serratus_thoracis_{side}",
            _lerp3(P(f"scapula_{side}"), P(f"humerus_{side}"), 0.30),
            _rib_pt(Pr, Pr.z_t1 + 0.24 * Pr.L, 0.34, sx),
            m.r(0.058),
            r_end=m.r(0.034),
            flat=0.54,
            mat="muscle_deep",
        )
        # 胸浅肌（降部）：胸骨柄 → 肱骨。两前肢之间那两团"胸口"
        m.belly(
            "thorax_front",
            f"pectoralis_descendens_{side}",
            (sx * Pr.u(0.026), y_stern + Pr.u(0.045), z_stern_front - Pr.u(0.020)),
            _off(P(f"humerus_{side}"), sx * Pr.u(0.010), -Pr.u(0.030), -Pr.u(0.010)),
            m.r(0.040),
        )
        # 胸深肌：胸骨外侧 → 肱骨，包住胸廓前下角
        m.belly(
            "thorax_front",
            f"pectoralis_profundus_{side}",
            (sx * Pr.u(0.030), y_stern + Pr.u(0.015), Pr.z_t1 + 0.16 * Pr.L),
            _off(P(f"humerus_{side}"), sx * Pr.u(0.004), -Pr.u(0.010), Pr.u(0.030)),
            m.r(0.048),
            mat="muscle_deep",
        )
        # 腹外斜肌：起自中段肋一路盖到髋，胸廓外侧壁全靠它——少了这片肋骨就裸在体表
        m.strip(
            "thorax_back",
            f"obliquus_{side}",
            m.rib_band(sx, fd=0.52, z0=Pr.z_t1 + Pr.u(0.040), z1=Pr.z_t18 + Pr.u(0.140)),
            m.r(0.062),
            flat=0.36,
            mat="muscle_deep",
        )
        # 腹侧带：接住肋弓下段到腹白线之间，胸底才不透白骨
        m.strip(
            "thorax_back",
            f"obliquus_ventral_{side}",
            m.rib_band(sx, fd=0.82, z0=Pr.z_t1 + Pr.u(0.060), z1=Pr.z_t18 + Pr.u(0.140)),
            m.r(0.056),
            flat=0.40,
            mat="muscle_deep",
        )
        # 腹直肌：剑突 → 耻骨。马腹的**下垂弧线**由它定
        m.belly(
            "lumbar",
            f"rectus_abdominis_{side}",
            (sx * Pr.u(0.052), y_stern - Pr.u(0.010), Pr.z_t1 + 0.26 * Pr.L),
            _off(P("hips"), sx * Pr.u(0.050), -Pr.u(0.150), -Pr.u(0.020)),
            m.r(0.060),
            r_end=m.r(0.044),
            flat=0.46,
        )


def grp_foreleg(m: MuscleBuilder, P) -> None:
    """冈上/冈下肌 · 三角肌 · **肱三头**（臂后那个大三角）· 肱二头 · 前臂肌群，
    腕以下**只有腱**：指伸肌腱走前缘、指屈肌腱走后缘绕籽骨，中间夹悬韧带。"""
    Pr = m.P
    for sx, side in ((-1, "l"), (1, "r")):
        sc, hu, ra, cp = (P(f"{n}_{side}") for n in ("scapula", "humerus", "radius", "carpus"))
        fk, hf = P(f"fetlock_f_{side}"), P(f"hoof_f_{side}")

        # 冈上肌 / 冈下肌：分别填肩胛冈**前**、**后**两个窝。z 向偏移必须大于肩胛板的
        # 前后半宽（0.052W），否则两条肌都压在板厚里，正视看过去仍是一整块裸肩胛板——
        # 侧向裸骨诊断（只查 max|x|）对这种前向暴露是瞎的，得靠正视图眼看。
        m.belly(
            f"scapula_{side}",
            f"supraspinatus_{side}",
            _off(sc, sx * Pr.u(0.036), -Pr.u(0.014), -Pr.u(0.076)),
            _off(hu, sx * Pr.u(0.024), Pr.u(0.030), -Pr.u(0.062)),
            m.r(0.046),
            flat=0.66,
        )
        m.belly(
            f"scapula_{side}",
            f"infraspinatus_{side}",
            _off(sc, sx * Pr.u(0.040), -Pr.u(0.022), Pr.u(0.072)),
            _off(hu, sx * Pr.u(0.028), Pr.u(0.030), Pr.u(0.052)),
            m.r(0.046),
            flat=0.58,
            mat="muscle_deep",
        )
        m.belly(
            f"scapula_{side}",
            f"deltoideus_{side}",
            _off(_lerp3(sc, hu, 0.34), sx * Pr.u(0.040)),
            _off(_lerp3(hu, ra, 0.40), sx * Pr.u(0.026)),
            m.r(0.036),
            flat=0.74,
        )
        # 肱三头：肩胛后角 + 肱骨 → 鹰嘴。马前肢最大的肌，臂后那个大三角轮廓就是它
        m.belly(
            f"humerus_{side}",
            f"triceps_{side}",
            _off(_lerp3(sc, hu, 0.80), sx * Pr.u(0.018), Pr.u(0.030), Pr.u(0.050)),
            _off(ra, sx * Pr.u(0.014), Pr.u(0.048), Pr.u(0.042)),
            m.r(0.078),
            r_end=m.r(0.042),
            flat=0.66,
        )
        # 臂肌：绕肱骨外侧从后向前，把肱骨干那条裸骨盖住
        m.belly(
            f"humerus_{side}",
            f"brachialis_{side}",
            _off(hu, sx * Pr.u(0.026), -Pr.u(0.010), Pr.u(0.026)),
            _off(ra, sx * Pr.u(0.022), Pr.u(0.010), -Pr.u(0.024)),
            m.r(0.034),
            r_end=m.r(0.020),
            flat=0.72,
            mat="muscle_deep",
        )
        m.belly(
            f"humerus_{side}",
            f"biceps_{side}",
            _off(hu, 0.0, 0.0, -Pr.u(0.030)),
            _off(_lerp3(ra, cp, 0.20), 0.0, 0.0, -Pr.u(0.022)),
            m.r(0.030),
        )
        # 前臂：桡侧腕伸肌（前缘）+ 屈肌群（后缘）。**只包上 2/3**，往下过渡成腱——
        # 马的前臂在腕上方就收成一束腱，肌腹一路铺到腕就成了狗腿。
        m.belly(
            f"radius_{side}",
            f"extensor_carpi_{side}",
            _off(ra, sx * Pr.u(0.018), -Pr.u(0.016), -Pr.u(0.034)),
            _off(_lerp3(ra, cp, 0.70), sx * Pr.u(0.010), 0.0, -Pr.u(0.008)),
            m.r(0.044),
            r_end=m.r(0.016),
            flat=0.86,
        )
        m.belly(
            f"radius_{side}",
            f"flexor_carpi_{side}",
            _off(ra, sx * Pr.u(0.020), -Pr.u(0.026), Pr.u(0.036)),
            _off(_lerp3(ra, cp, 0.66), sx * Pr.u(0.010), 0.0, Pr.u(0.020)),
            m.r(0.040),
            r_end=m.r(0.014),
            flat=0.86,
            mat="muscle_deep",
        )
        _distal_tendons(m, P, side, cp, fk, hf, sx, "f")


def grp_hindleg(m: MuscleBuilder, P) -> None:
    """臀中肌 · **阔筋膜张肌**（股前那个三角）· 股二头 · 半腱肌 · 股四头 ·
    腓肠肌 + 跟腱，跗以下同样**只有腱**。"""
    Pr = m.P
    for sx, side in ((-1, "l"), (1, "r")):
        hp = P("hips")
        fe, ti, ta = (P(f"{n}_{side}") for n in ("femur", "tibia", "tarsus"))
        fk, hf = P(f"fetlock_h_{side}"), P(f"hoof_h_{side}")

        # 臀中肌：髂骨翼 → 大转子。马尻的形状就是它，比猫科厚得多。
        # 横向要盖过**髂骨翼**（从髋臼 ±0.095W 一路外张到髋结节 ±0.165W）——
        # 挂在 hips pivot 附近就整条落在骨内侧，髂骨全裸（裸骨诊断头号）。
        # 起止点直接压在**髂骨轴线上**（髋结节 → 大转子）：髂骨板在 x 上很薄（±0.024W），
        # 肌腹只要骑在同一条轴上就能整根吞掉；挂在 hips pivot 附近则整条落在骨内侧。
        # 起点比髋结节靠后 0.045W —— 髋结节（点 of hip）本来就该顶出皮，不该埋掉。
        tuber_coxae = (sx * Pr.u(0.165), Pr.y_croup - Pr.u(0.055), Pr.z_hip - 0.100 * Pr.L)
        trochanter = _off(fe, sx * Pr.u(0.040), Pr.u(0.058), Pr.u(0.014))
        # 走 strip 沿「髋结节 → 髋臼 → 大转子」折线：臀中肌是**盖过髋关节**的，
        # 一根直的梭形肌腹两端定死在髋结节和大转子上，中段就从髂骨翼上方擦过去，
        # 髂骨翼前上角始终露在外面（裸骨诊断里死活压不下去的那一项）。
        m.strip(
            "hips",
            f"gluteus_medius_{side}",
            [
                _off(tuber_coxae, sx * -Pr.u(0.010), Pr.u(0.006), Pr.u(0.030)),
                _lerp3(tuber_coxae, fe, 0.55),
                trochanter,
            ],
            m.r(0.072),
            flat=0.86,
        )
        # 臀浅肌 + 臀筋膜：自荐结节斜盖过髂骨翼到股骨，把尻侧那片骨面收进皮下。
        # 臀中肌走的是髋结节→大转子那条轴，盖不到髂骨翼前上角（裸骨诊断的头号残留）。
        m.belly(
            "hips",
            f"gluteus_superficialis_{side}",
            (sx * Pr.u(0.062), Pr.y_croup - Pr.u(0.030), Pr.z_sacrum - 0.055 * Pr.L),
            _off(fe, sx * Pr.u(0.052), -Pr.u(0.010), Pr.u(0.036)),
            m.r(0.056),
            r_end=m.r(0.034),
            flat=0.54,
            mat="muscle_deep",
        )
        # 阔筋膜张肌：髋结节 → 膝外侧筋膜。股前那个明显的三角，马身上一眼可辨
        m.belly(
            "hips",
            f"tensor_fasciae_latae_{side}",
            _off(hp, sx * Pr.u(0.150), Pr.u(0.050), -Pr.u(0.105)),
            _off(ti, sx * Pr.u(0.046), Pr.u(0.030), -Pr.u(0.030)),
            m.r(0.048),
            r_end=m.r(0.026),
            flat=0.42,
        )
        # 股二头：荐/坐骨 → 胫骨近端。后肢最大的肌，臀腿后廓靠它
        m.belly(
            f"femur_{side}",
            f"biceps_femoris_{side}",
            _off(hp, sx * Pr.u(0.086), Pr.u(0.020), Pr.u(0.100)),
            _off(_lerp3(ti, ta, 0.42), sx * Pr.u(0.030), 0.0, 0.0),
            m.r(0.082),
            r_end=m.r(0.048),
            flat=0.74,
        )
        # 半腱肌：坐骨结节 → 胫骨。臀后那条垂直的沟就在它与股二头之间
        m.belly(
            f"femur_{side}",
            f"semitendinosus_{side}",
            _off(hp, sx * Pr.u(0.036), -Pr.u(0.030), Pr.u(0.150)),
            _off(_lerp3(ti, ta, 0.46), sx * -Pr.u(0.016), 0.0, Pr.u(0.020)),
            m.r(0.052),
            r_end=m.r(0.030),
            mat="muscle_deep",
        )
        # 股四头：股骨 → 髌。膝前的鼓包
        m.belly(
            f"femur_{side}",
            f"quadriceps_{side}",
            _off(fe, sx * Pr.u(0.008), Pr.u(0.010), -Pr.u(0.060)),
            _off(ti, 0.0, Pr.u(0.030), -Pr.u(0.050)),
            m.r(0.062),
            r_end=m.r(0.034),
            flat=0.80,
        )
        # 腓肠肌 → 跟腱：止于跟结节，小腿后缘的鼓包 + 那条绷直的腱
        m.belly(
            f"tibia_{side}",
            f"gastrocnemius_{side}",
            _off(ti, sx * Pr.u(0.020), Pr.u(0.026), Pr.u(0.050)),
            _off(_lerp3(ti, ta, 0.60), sx * Pr.u(0.012), Pr.u(0.016), Pr.u(0.038)),
            m.r(0.050),
            r_end=m.r(0.026),
        )
        # 腓骨长肌/趾伸肌群：贴胫骨外侧面，把小腿那条裸骨盖住
        m.belly(
            f"tibia_{side}",
            f"peroneus_{side}",
            _off(ti, sx * Pr.u(0.026), Pr.u(0.010), Pr.u(0.006)),
            _off(_lerp3(ti, ta, 0.84), sx * Pr.u(0.014), 0.0, 0.0),
            m.r(0.048),
            r_end=m.r(0.018),
            flat=0.84,
            mat="muscle_deep",
        )
        m.belly(
            f"tarsus_{side}",
            f"achilles_{side}",
            _lerp3(ti, ta, 0.64),
            _off(ta, 0.0, Pr.u(0.064), Pr.u(0.044)),
            m.r(0.014),
            r_end=m.r(0.012),
            mat="tendon",
        )
        # 胫前肌：胫骨前缘（小腿前那条）
        m.belly(
            f"tibia_{side}",
            f"tibialis_cranialis_{side}",
            _off(ti, 0.0, -Pr.u(0.020), -Pr.u(0.036)),
            _off(_lerp3(ti, ta, 0.72), 0.0, 0.0, -Pr.u(0.026)),
            m.r(0.028),
            r_end=m.r(0.012),
            mat="muscle_deep",
        )
        _distal_tendons(m, P, side, ta, fk, hf, sx, "h")


def _distal_tendons(m: MuscleBuilder, P, side: str, top: Vec, fetlock: Vec, coffin: Vec, sx: int, tag: str) -> None:
    """腕/跗以下的腱束。**这一段没有肌肉**——马的管骨区皮下只有骨与腱。

    三条并列，从后往前：指浅/深屈肌腱（离骨最远，那条摸得到的"筋"）、悬韧带、
    指伸肌腱（贴在前缘）。屈肌腱在球节后绕过籽骨再转向系骨，所以走 strip 不走直线。
    """
    Pr = m.P
    back = Pr.u(0.030)  # 屈肌腱离管骨后缘的距离
    mid = Pr.u(0.016)
    front = Pr.u(0.020)

    # 指屈肌腱：腕后 → 球节后（绕籽骨）→ 系骨掌侧
    m.strip(
        f"fetlock_{tag}_{side}",
        f"flexor_tendon_{tag}_{side}",
        [
            _off(top, 0.0, -Pr.u(0.010), back),
            _off(_lerp3(top, fetlock, 0.55), 0.0, 0.0, back),
            _off(fetlock, 0.0, Pr.u(0.004), back * 0.94),
            _off(_lerp3(fetlock, coffin, 0.60), 0.0, 0.0, back * 0.50),
            _off(coffin, 0.0, Pr.u(0.006), Pr.u(0.010)),
        ],
        m.r(0.014),
        mat="tendon",
        flat=0.72,
    )
    # 悬韧带：管骨后面 → 籽骨，夹在屈肌腱与管骨之间
    m.belly(
        f"fetlock_{tag}_{side}",
        f"suspensory_{tag}_{side}",
        _off(top, 0.0, -Pr.u(0.020), mid),
        _off(fetlock, 0.0, Pr.u(0.006), mid + Pr.u(0.006)),
        m.r(0.011),
        r_end=m.r(0.009),
        mat="tendon",
        flat=0.86,
    )
    # 指伸肌腱：贴管骨前缘一路到蹄骨伸肌突
    m.strip(
        f"fetlock_{tag}_{side}",
        f"extensor_tendon_{tag}_{side}",
        [
            _off(top, 0.0, -Pr.u(0.010), -front),
            _off(fetlock, 0.0, 0.0, -front),
            _off(coffin, 0.0, Pr.u(0.010), -front * 0.70),
        ],
        m.r(0.010),
        mat="tendon",
        flat=0.90,
    )


def grp_tail(m: MuscleBuilder, P) -> None:
    """尾肌：包裹尾椎，自根部向尖端渐细。马的尾根是肉的（尾鬃另属皮毛层）。"""
    idx = sorted(int(n.split("_")[1]) for n in m.skel.pivot if n.startswith("tail_"))
    for k, i in enumerate(idx[:-1]):
        a, b = P(f"tail_{i:02d}"), P(f"tail_{i + 1:02d}")
        t = k / max(1, len(idx) - 2)
        r = m.r(0.072) * (1.0 - 0.74 * t)
        m.belly(f"tail_{i:02d}", f"tail_muscle_{i:02d}", a, b, r, r_end=r * 0.9)


GROUPS = {
    "head": ("咬肌/颞肌", grp_head),
    "neck": ("项韧带/夹肌/臂头肌/胸头肌", grp_neck),
    "torso": ("竖脊肌/斜方/菱形/背阔/前锯吊索/胸肌/腹壁", grp_torso),
    "foreleg": ("冈上下/三角/肱三头/肱二头/前臂+指腱", grp_foreleg),
    "hindleg": ("臀中肌/阔筋膜张肌/股二头/半腱/股四头/腓肠+指腱", grp_hindleg),
    "tail": ("尾肌", grp_tail),
}


# ================================================================ 延展视图
def explode(data: dict, P: Profile, dist: float) -> None:
    """延展视图：各部件沿「离脊柱轴的径向」散开，肌肉推得比骨远。

    纯预览用——把互相贴合、彼此遮挡的肌腹拉开，好逐条看形态。
    不要拿 explode 出来的文件继续做后续工序。
    """
    for e in data["elements"]:
        cx = (e["from"][0] + e["to"][0]) / 2
        cy = (e["from"][1] + e["to"][1]) / 2
        cz = (e["from"][2] + e["to"][2]) / 2
        dx, dy = cx, cy - P.centrum_y(cz)
        norm = math.hypot(dx, dy)
        if norm < 0.4:  # 落在轴上的（椎体/胸骨）没有径向，直接上抬
            ux, uy = 0.0, 1.0
        else:
            ux, uy = dx / norm, dy / norm
        k = dist * (1.7 if e.get("_muscle") else 0.6)
        for key in ("from", "to", "origin"):
            e[key][0] = round(e[key][0] + ux * k, 3)
            e[key][1] = round(e[key][1] + uy * k, 3)


# ================================================================ 自检
def check_mirror(data: dict) -> list[str]:
    """肌肉左右镜像自检（与骨架同一套判据）。"""
    els = {e["name"]: e for e in data["elements"] if e.get("_muscle")}
    bad: list[str] = []
    for name, e in els.items():
        if not (name.endswith("_l") or "_l_" in name):
            continue
        mate = name.replace("_l_", "_r_") if "_l_" in name else name[:-2] + "_r"
        mm = els.get(mate)
        if mm is None:
            bad.append(f"{name}: 缺镜像件 {mate}")
            continue
        if abs(e["from"][0] + mm["to"][0]) > 0.02 or abs(e["to"][0] + mm["from"][0]) > 0.02:
            bad.append(f"{name}: x 未镜像")
        for axis, label in ((1, "y"), (2, "z")):
            if abs(e["from"][axis] - mm["from"][axis]) > 0.02 or abs(e["to"][axis] - mm["to"][axis]) > 0.02:
                bad.append(f"{name}: {label} 与镜像件不等")
    return bad


# 诊断方向：(名字, 轴, 取最大还是最小)。侧向用 |x| 合并左右。
DIRECTIONS = (
    ("侧", 0, "abs"),
    ("前", 2, "min"),
    ("背", 1, "max"),
    ("腹", 1, "min"),
)


def diagnose_bare_bone(data: dict, step: float = 0.4) -> dict[str, list[tuple[str, int]]]:
    """裸骨诊断：体素栅格化全身，逐列取最外那格，看它是骨还是肌。

    肌肉层最容易出的错是「某片肌少了 / 太薄，肋骨或肩胛直接裸在体表」——三视图上
    骨和肌都是实心块，颜色一近就看不出来。这里按最外层归属统计，直接点名。

    **必须四个方向都查**：首版只查侧向（max|x|），肩胛板在正视里明明是一整块裸骨，
    诊断却一声不吭——因为它侧向确实被冈上/冈下肌盖住了，露的是前面。

    返回 {方向: [(element 名, 暴露列数)] 降序}。**不是 pass/fail**：蹄匣、颅面骨、
    管骨区本来就该露在外面（马的管骨皮下就是骨与腱），要人判断。
    """
    import numpy as np

    els = data["elements"]
    if not els:
        return []

    def obb(e):
        def rm(deg, axis):
            a = math.radians(deg)
            c, s = math.cos(a), math.sin(a)
            if axis == 0:
                return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])
            if axis == 1:
                return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])
            return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])

        f, t = np.array(e["from"], float), np.array(e["to"], float)
        org = np.array(e["origin"], float)
        rot = e["rotation"]
        R = np.eye(3)
        if any(rot):
            R = rm(rot[2], 2) @ rm(rot[1], 1) @ rm(rot[0], 0)
        return R @ ((f + t) / 2 - org) + org, (t - f) / 2, R

    best: dict[str, dict[tuple[int, int], tuple[float, int]]] = {d[0]: {} for d in DIRECTIONS}
    for ei, e in enumerate(els):
        c, half, R = obb(e)
        ext = np.array([sum(half[i] * abs(R[k, i]) for i in range(3)) for k in range(3)])
        lo, hi = c - ext, c + ext
        rng = [np.arange(math.floor(lo[k] / step), math.ceil(hi[k] / step) + 1) for k in range(3)]
        if any(len(r) == 0 for r in rng):
            continue
        gx, gy, gz = np.meshgrid(*rng, indexing="ij")
        pts = np.stack([gx.ravel(), gy.ravel(), gz.ravel()], 1) * step
        q = np.abs((pts - c) @ R)
        inside = np.all(q <= half + 1e-9, axis=1)
        if not inside.any():
            continue
        sel = pts[inside]
        for label, axis, mode in DIRECTIONS:
            bucket = best[label]
            others = [k for k in (0, 1, 2) if k != axis]
            for p in sel:
                key = (int(round(p[others[0]] / step)), int(round(p[others[1]] / step)))
                v = abs(p[axis]) if mode == "abs" else (p[axis] if mode == "max" else -p[axis])
                cur = bucket.get(key)
                if cur is None or v > cur[0]:
                    bucket[key] = (v, ei)

    out: dict[str, list[tuple[str, int]]] = {}
    for label, _axis, _mode in DIRECTIONS:
        tally: dict[str, int] = {}
        for _v, ei in best[label].values():
            if els[ei].get("_muscle"):
                continue
            nm = els[ei]["name"]
            tally[nm] = tally.get(nm, 0) + 1
        out[label] = sorted(tally.items(), key=lambda kv: -kv[1])
    return out


# ================================================================ CLI
def build(P: Profile, groups: list, skeleton: Path) -> tuple[Skeleton, MuscleBuilder]:
    skel = Skeleton(skeleton)
    extend_texture(skel.data)
    m = MuscleBuilder(skel, P)
    for _label, fn in groups:
        fn(m, skel.P)
    return skel, m


def main() -> int:
    ap = argparse.ArgumentParser(description="马肌肉层（读骨架，不回写）")
    ap.add_argument("--profile", choices=[*sorted(PROFILES), "all"], default="all")
    ap.add_argument("--group", choices=sorted(GROUPS), help="只生成一个肌群")
    ap.add_argument("--only-muscle", action="store_true", help="摘掉骨骼 cube，只留肌肉")
    ap.add_argument("--list", action="store_true", help="列出肌群")
    ap.add_argument("--explode", type=float, metavar="D", help="延展视图：各部件沿径向散开 D（预览用）")
    ap.add_argument("--diagnose", action="store_true", help="裸骨诊断：列出最外层仍是骨的部位")
    ap.add_argument("--out", type=Path, help="输出路径（仅单档时有效）")
    args = ap.parse_args()

    if args.list:
        for k, (label, _) in GROUPS.items():
            print(f"  {k:9s} {label}")
        return 0

    keys = sorted(PROFILES) if args.profile == "all" else [args.profile]
    todo = [GROUPS[args.group]] if args.group else list(GROUPS.values())
    rc = 0

    for key in keys:
        P = PROFILES[key]
        skeleton = MODELS / f"HorseSkeleton_{key}.bbmodel"
        if not skeleton.is_file():
            print(f"找不到骨架: {skeleton}（先跑 gen_skeleton.py）")
            return 2
        skel, m = build(P, todo, skeleton)

        if args.only_muscle:
            skel.drop_bone_cubes()
        if args.explode:
            explode(skel.data, P, args.explode)

        name = f"HorseMuscle_{key}" if not args.group else f"HorseMuscle_{args.group}_{key}"
        if args.only_muscle:
            name += "_bare"
        if args.explode:
            name += "_explode"
        skel.data["name"] = name
        skel.data["model_identifier"] = name
        out = args.out if (args.out and len(keys) == 1) else (MODELS / f"{name}.bbmodel")
        out.write_text(json.dumps(skel.data, ensure_ascii=False, indent=1))

        muscle_cubes = sum(1 for e in skel.data["elements"] if e.get("_muscle"))
        print(f"→ {out.relative_to(REPO)}")
        print(f"   【{P.label}】骨骼 {len(skel.pivot)} 根 · 肌腹 {m.count} 条 / {muscle_cubes} cube · 总 {len(skel.data['elements'])}")

        bad = check_mirror(skel.data)
        if bad:
            rc = 1
            print(f"   ✗ 镜像 {len(bad)} 处违例：")
            for x in bad[:8]:
                print(f"      {x}")
        else:
            print("   ✓ 肌肉左右镜像通过")

        if args.diagnose and not args.only_muscle and not args.explode:
            report = diagnose_bare_bone(skel.data)
            for label, rank in report.items():
                total = sum(c for _n, c in rank)
                head = " · ".join(f"{nm}×{c}" for nm, c in rank[:8])
                print(f"   裸骨[{label}] {total:4d} 列 — {head}")
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
