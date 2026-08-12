#!/usr/bin/env python3
"""怠怒之狮 —— 肌肉层（Round 1/3）。

**读**已有骨架 bbmodel，按其骨骼 pivot 算附着点，生成覆盖其上的骨骼肌，
输出到新文件。绝不回写骨架——骨架已被手工精修过（Blockbench fmt 5.0），
生成器覆盖会冲掉那些改动。

只做**骨骼肌**，不做内脏：颞肌/咬肌 · 颈背索(项韧带-夹肌)/胸头肌 ·
竖脊肌/背阔肌/胸肌/前锯肌/腹壁 · 三角肌/冈下肌/肱三头/肱二头/前臂肌群 ·
臀肌/股二头/股四头/半腱肌/腓肠肌/胫前肌 · 尾肌。

附着点全部由骨骼 pivot 派生（P("femur_l") 等），所以骨架一改、肌肉自动跟随；
只有颅面几处贴在颅骨具体骨面上的（颞窝/颧弓下），才用绝对坐标。

肌腹取梭形——3 段，中段粗两端收，单段直筒看着像水管。

用法:
  python3 scripts/models/dainu_lion/gen_muscle.py                # 骨+肌
  python3 scripts/models/dainu_lion/gen_muscle.py --only-muscle  # 只看肌肉
  python3 scripts/models/dainu_lion/gen_muscle.py --list
  python3 scripts/models/dainu_lion/gen_muscle.py --group hindleg
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import uuid
from pathlib import Path

from gen_skeleton import STERNUM_Y as SY
from gen_skeleton import rib_surface_point, shaft_box
from PIL import Image

REPO = Path(__file__).resolve().parents[3]
MODELS = REPO / "local_models" / "dainu_lion"
SKELETON = MODELS / "DainuLionSkeleton.bbmodel"

# 肌肉材质追加在贴图第 2 行（第 1 行 7 个骨/角/牙色块保持原位，
# 这样读进来的骨骼 element 的 UV 一个都不用改）。
MUSCLE_MATS = {
    "muscle": (146, 56, 50),  # 浅层肌腹
    "muscle_deep": (104, 38, 35),  # 深层肌
    "tendon": (206, 196, 172),  # 腱膜 / 跟腱
}
SWATCH = 8
MUSCLE_ROW = 1


def _lerp3(a, b, t):
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


def _off(p, dx=0.0, dy=0.0, dz=0.0):
    return (p[0] + dx, p[1] + dy, p[2] + dz)


def _rib_band(sx: int, *, fd: float, z0: float, z1: float, n: int = 5, lift: float = 0.55):
    """沿肋弓表面等深度线取一串点（供 strip 铺贴合胸廓的肌带）。

    lift = 沿径向往外抬一点，让肌带浮在肋骨表面而不是与之共面（z-fighting）。
    """
    out = []
    for i in range(n + 1):
        z = z0 + (z1 - z0) * i / n
        x, y, zz = rib_surface_point(z, fd, sx)
        nx, ny = (1.0 if sx > 0 else -1.0), 0.0
        if fd > 0.75:  # 腹侧法线偏下
            nx, ny = nx * 0.6, -0.8
        elif fd < 0.25:  # 背侧法线偏上
            nx, ny = nx * 0.85, 0.5
        out.append((x + nx * lift, y + ny * lift, zz))
    return out


# ---------------------------------------------------------------- 读骨架
class Skeleton:
    """兼容 fmt 4.x（outliner 内联 group）与 fmt 5.0（groups 数组 + uuid 引用树）。"""

    def __init__(self, path: Path) -> None:
        self.path = path
        self.data = json.loads(path.read_text())
        self.groups = {g["uuid"]: g for g in self.data.get("groups", [])}
        self.nodes: dict[str, dict] = {}  # 骨骼名 → outliner 节点（用于挂 cube）
        self.pivot: dict[str, tuple] = {}
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

    def P(self, bone: str) -> tuple:
        if bone not in self.pivot:
            raise KeyError(f"骨架里没有骨骼 {bone}（有 {len(self.pivot)} 根）")
        return self.pivot[bone]

    def attach(self, bone: str, element: dict) -> None:
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
    for i, (name, (r, g, b)) in enumerate(MUSCLE_MATS.items()):
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
    def __init__(self, skel: Skeleton) -> None:
        self.skel = skel
        self.count = 0

    def belly(
        self,
        bone: str,
        name: str,
        a,
        b,
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
        for i, r in enumerate(radii):
            p0 = _lerp3(a, b, cuts[i])
            p1 = _lerp3(a, b, cuts[i + 1])
            frm, to, rot, org = shaft_box(p0, p1, r * flat, r)
            self.skel.attach(
                bone,
                {
                    "name": f"{name}_{i + 1}",
                    "box_uv": False,
                    "rescale": False,
                    "locked": False,
                    "render_order": "default",
                    "allow_mirror_modeling": True,
                    "type": "cube",
                    "uuid": str(uuid.uuid4()),
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
        self.count += 1

    def strip(self, bone: str, name: str, pts, r: float, *, mat: str = "muscle", flat: float = 1.0) -> None:
        """沿一串点铺一条连续肌带（折线跟随曲面）。

        阔肌（背阔/腹外斜）用它贴着肋弓走。首版把整片阔肌做成一个 flat 的
        大 cube，糊在胸廓上像块木板——扁不等于平，阔肌是**贴曲面**的。
        """
        for i, (p0, p1) in enumerate(zip(pts[:-1], pts[1:])):
            frm, to, rot, org = shaft_box(p0, p1, r * flat, r)
            self.skel.attach(
                bone,
                {
                    "name": f"{name}_{i + 1}",
                    "box_uv": False,
                    "rescale": False,
                    "locked": False,
                    "render_order": "default",
                    "allow_mirror_modeling": True,
                    "type": "cube",
                    "uuid": str(uuid.uuid4()),
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
        self.count += 1


# ================================================================ 肌群
def grp_head(m: MuscleBuilder, P) -> None:
    """颞肌 + 咬肌。颅面肌贴具体骨面，用绝对坐标（不由 pivot 派生）。"""
    for sx, side in ((-1, "l"), (1, "r")):
        # 颞肌：填颞窝（矢状嵴 ↔ 下颌冠突），咬合力的主力
        m.belly("skull", f"temporalis_{side}", (sx * 1.2, 27.0, -20.6), (sx * 2.6, 23.8, -20.2), 1.45, mat="muscle_deep")
        # 咬肌：颧弓 ↔ 下颌角
        m.belly("jaw", f"masseter_{side}", (sx * 2.6, 23.6, -19.6), (sx * 2.0, 20.9, -21.6), 1.25)


def grp_neck(m: MuscleBuilder, P) -> None:
    """颈背索（项韧带+夹肌，狮子粗颈的来源）+ 胸头肌。"""
    for sx, side in ((-1, "l"), (1, "r")):
        m.belly(
            "neck_base",
            f"nuchal_{side}",
            _off(P("thorax_front"), sx * 1.7, 3.4, 1.0),
            _off(P("skull"), sx * 1.4, 1.4, 2.2),
            2.25,
            r_end=1.6,
        )
        m.belly(
            "neck_base",
            f"sternocephalicus_{side}",
            (sx * 1.6, SY + 1.8, -11.0),
            _off(P("skull"), sx * 2.2, -3.4, -1.4),
            1.2,
            mat="muscle_deep",
        )


def grp_torso(m: MuscleBuilder, P) -> None:
    """竖脊肌 · 背阔肌 · 胸肌 · 前锯肌 · 腹壁。"""
    # 胸横肌：沿胸骨腹面铺一条，盖住胸底的肋软骨末端（诊断里 y4~8 那片裸骨）
    m.belly("thorax_front", "pectoralis_transverse", (0.0, SY - 0.4, -10.4), (0.0, SY - 0.9, -0.8), 1.5, flat=1.15, mat="muscle_deep")

    # 竖脊肌：脊柱两侧纵行，从荐一路到颈——背线隆起主要来自它
    chain = ["hips", "lumbar", "thorax_back", "thorax_front", "neck_base"]
    radii = [1.75, 1.75, 1.6, 1.45]
    for sx, side in ((-1, "l"), (1, "r")):
        for i, (a_name, b_name) in enumerate(zip(chain, chain[1:])):
            m.belly(
                a_name,
                f"longissimus_{side}_{i + 1}",
                _off(P(a_name), sx * 2.45, 1.5, 0),
                _off(P(b_name), sx * 2.45, 1.5, 0),
                radii[i],
                r_end=radii[i] * 0.92,  # 连续肌索，两端不收细
            )
        # 背阔肌：沿肋弓上段（fd 0.20）自前向后铺的肌带，跟着胸廓曲面走
        m.strip(
            "thorax_back",
            f"latissimus_{side}",
            _rib_band(sx, fd=0.20, z0=-9.5, z1=7.0, n=5),
            1.75,
            flat=0.42,
        )
        # 胸肌：胸骨 ↔ 肱骨近端（前肢内收）
        m.belly(
            "thorax_front",
            f"pectoralis_{side}",
            (sx * 1.3, SY + 0.8, -6.4),
            _off(P("humerus_" + side), sx * 0.4, -1.6, 0.4),
            1.55,
        )
        # 前锯肌：肩胛内面 ↔ 中段肋（四足动物靠它把躯干"吊"在肩胛之间）
        m.belly(
            "thorax_front",
            f"serratus_{side}",
            _lerp3(P(f"scapula_{side}"), P(f"humerus_{side}"), 0.35),
            (sx * 2.9, 13.4, -3.6),
            1.55,
            mat="muscle_deep",
            flat=0.55,
        )
        # 腹壁（腹直）：肋弓 ↔ 骨盆。是壁不是内脏
        m.belly(
            "lumbar",
            f"abdominal_{side}",
            (sx * 2.8, SY + 2.0, -0.5),
            _off(P("hips"), sx * 2.5, -4.6, -0.5),
            1.85,
            r_end=1.5,
            flat=0.45,
        )
        # 腹外斜肌：起自第 5 肋一路盖到髋。胸廓 y8~16 的外侧壁全靠它——
        # 少了这片，肋骨就一根根裸露在体表外（网格诊断实证）。
        m.strip(
            "thorax_back",
            f"obliquus_{side}",
            _rib_band(sx, fd=0.52, z0=-9.5, z1=7.5, n=5),
            1.85,
            flat=0.40,
            mat="muscle_deep",
        )
        # 腹侧带：接住肋弓下段到腹白线之间，胸底才不透白骨
        m.strip(
            "thorax_back",
            f"obliquus_ventral_{side}",
            _rib_band(sx, fd=0.82, z0=-9.0, z1=7.5, n=5),
            1.7,
            flat=0.44,
            mat="muscle_deep",
        )
        # 深胸肌：胸骨外侧 ↔ 肱骨，包住胸廓前下角
        m.belly(
            "thorax_front",
            f"pectoralis_deep_{side}",
            (sx * 1.6, SY + 0.4, -9.2),
            _off(P("humerus_" + side), sx * 0.2, -2.4, 1.6),
            1.75,
            mat="muscle_deep",
        )
        # 冈上肌：肩胛冈前方，补住肩胛板上缘（否则肩头一片白骨）
        m.belly(
            "scapula_" + side,
            f"supraspinatus_{side}",
            _off(P(f"scapula_{side}"), sx * 0.7, -0.8, -1.6),
            _off(P(f"humerus_{side}"), sx * 0.6, 1.2, -1.0),
            1.6,
            flat=0.5,
        )


def grp_foreleg(m: MuscleBuilder, P) -> None:
    """三角肌 · 冈下肌 · 肱三头 · 肱二头 · 前臂肌群。"""
    for sx, side in ((-1, "l"), (1, "r")):
        sc, hu, ra, ff = (P(f"{n}_{side}") for n in ("scapula", "humerus", "radius", "forefoot"))
        m.belly("scapula_" + side, f"deltoid_{side}", _lerp3(sc, hu, 0.66), _lerp3(hu, ra, 0.42), 1.5, flat=0.78)
        m.belly(
            "scapula_" + side,
            f"infraspinatus_{side}",
            _off(sc, sx * 1.0, -1.4, 0.6),
            _off(hu, sx * 0.9, 1.4, 0.4),
            1.75,
            flat=0.42,
            mat="muscle_deep",
        )
        # 肱三头：肩胛后角/肱骨近端 → 鹰嘴。猫科前肢最厚的肌，上臂后缘的轮廓由它决定
        m.belly("humerus_" + side, f"triceps_{side}", _off(_lerp3(sc, hu, 0.86), 0, 1.2, 1.8), _off(ra, 0, 1.9, 1.7), 2.0, flat=0.68)
        m.belly("humerus_" + side, f"biceps_{side}", _off(hu, 0, 0.4, -1.4), _off(_lerp3(ra, ff, 0.22), 0, 0, -0.9), 1.2)
        # 前臂屈伸肌群：只包上 2/3，往下过渡成腱
        m.belly("radius_" + side, f"antebrachial_{side}", _off(ra, 0, -0.4, 0.2), _lerp3(ra, ff, 0.68), 1.35, r_end=0.8)
        m.belly("forefoot_" + side, f"fore_tendon_{side}", _lerp3(ra, ff, 0.72), _off(ff, 0, -0.2, -0.3), 0.62, mat="tendon")


def grp_hindleg(m: MuscleBuilder, P) -> None:
    """臀肌 · 股二头 · 股四头 · 半腱肌 · 腓肠肌 · 胫前肌 + 跟腱。"""
    for sx, side in ((-1, "l"), (1, "r")):
        hp = P("hips")
        fe, ti, ta, hf = (P(f"{n}_{side}") for n in ("femur", "tibia", "tarsus", "hindfoot"))
        m.belly("hips", f"gluteus_{side}", _off(hp, sx * 2.1, 1.3, -1.6), _off(fe, sx * 1.1, 1.7, 0.6), 1.95, flat=0.82)
        # 股二头：坐骨 → 胫骨近端。后肢最大的肌，臀腿外廓靠它
        m.belly("femur_" + side, f"biceps_femoris_{side}", _off(hp, sx * 2.0, -1.0, 4.6), _off(_lerp3(ti, ta, 0.28), sx * 0.2, 0, 0), 2.4, r_end=1.5, flat=0.72)
        m.belly("femur_" + side, f"quadriceps_{side}", _off(fe, sx * 0.2, 0.4, -1.9), _off(ti, 0, 1.1, -1.9), 2.1, r_end=1.3, flat=0.8)
        m.belly("femur_" + side, f"semitendinosus_{side}", _off(hp, sx * 1.3, -1.6, 5.0), _off(_lerp3(ti, ta, 0.5), sx * -0.6, 0, 0.6), 1.45, mat="muscle_deep")
        # 腓肠肌 + 跟腱：止于跟结节，小腿后缘的鼓包
        m.belly("tibia_" + side, f"gastrocnemius_{side}", _off(ti, sx * 0.2, 0.9, 1.9), _off(_lerp3(ti, ta, 0.62), 0, 0.6, 1.5), 1.7, r_end=1.05)
        m.belly("tarsus_" + side, f"achilles_{side}", _lerp3(ti, ta, 0.66), _off(ta, 0, 1.5, 2.2), 0.55, mat="tendon")
        m.belly("tibia_" + side, f"tibialis_{side}", _off(ti, 0, -0.6, -1.3), _off(_lerp3(ta, hf, 0.35), 0, 0, -1.1), 1.0, mat="muscle_deep")


def grp_tail(m: MuscleBuilder, P) -> None:
    """尾肌：包裹尾椎，自根部向尖端渐细。"""
    idx = sorted(int(n.split("_")[1]) for n in m.skel.pivot if n.startswith("tail_"))
    for k, i in enumerate(idx[:-1]):
        a, b = P(f"tail_{i:02d}"), P(f"tail_{i + 1:02d}")
        t = k / max(1, len(idx) - 2)
        r = 1.55 - 1.2 * t
        m.belly(f"tail_{i:02d}", f"tail_muscle_{i:02d}", a, b, r, r_end=r * 0.9)


def explode(data: dict, dist: float) -> None:
    """延展视图：各部件沿"离脊柱轴的径向"散开，肌肉推得比骨远。

    纯预览用——把互相贴合、彼此遮挡的肌腹拉开，好逐条看形态。
    不要拿 explode 出来的文件继续做后续工序。
    """
    from gen_skeleton import spine_y

    for e in data["elements"]:
        cx = (e["from"][0] + e["to"][0]) / 2
        cy = (e["from"][1] + e["to"][1]) / 2
        cz = (e["from"][2] + e["to"][2]) / 2
        dx, dy = cx - 0.0, cy - spine_y(cz)
        norm = (dx * dx + dy * dy) ** 0.5
        if norm < 0.4:  # 落在轴上的（椎体/胸骨）没有径向，直接上抬
            ux, uy = 0.0, 1.0
        else:
            ux, uy = dx / norm, dy / norm
        k = dist * (1.7 if e.get("_muscle") else 0.6)
        for key in ("from", "to", "origin"):
            e[key][0] = round(e[key][0] + ux * k, 3)
            e[key][1] = round(e[key][1] + uy * k, 3)


GROUPS = {
    "head": ("颞肌/咬肌", grp_head),
    "neck": ("颈背索/胸头肌", grp_neck),
    "torso": ("竖脊肌/背阔肌/胸肌/前锯肌/腹壁", grp_torso),
    "foreleg": ("三角肌/冈下肌/肱三头/肱二头/前臂", grp_foreleg),
    "hindleg": ("臀肌/股二头/股四头/半腱肌/腓肠肌/胫前肌", grp_hindleg),
    "tail": ("尾肌", grp_tail),
}


def main() -> int:
    ap = argparse.ArgumentParser(description="怠怒之狮肌肉层（读骨架，不回写）")
    ap.add_argument("--skeleton", type=Path, default=SKELETON, help="输入骨架 bbmodel")
    ap.add_argument("--group", choices=sorted(GROUPS), help="只生成一个肌群")
    ap.add_argument("--only-muscle", action="store_true", help="摘掉骨骼 cube，只留肌肉")
    ap.add_argument("--list", action="store_true", help="列出肌群")
    ap.add_argument("--explode", type=float, metavar="D", help="延展视图：各部件沿径向散开 D（预览用）")
    ap.add_argument("--out", type=Path, help="输出路径")
    args = ap.parse_args()

    if args.list:
        for k, (label, _) in GROUPS.items():
            print(f"  {k:9s} {label}")
        return 0

    if not args.skeleton.is_file():
        print(f"找不到骨架: {args.skeleton}")
        return 2

    skel = Skeleton(args.skeleton)
    extend_texture(skel.data)
    m = MuscleBuilder(skel)
    todo = [GROUPS[args.group]] if args.group else list(GROUPS.values())
    for _label, fn in todo:
        fn(m, skel.P)

    if args.only_muscle:
        skel.drop_bone_cubes()
    if args.explode:
        explode(skel.data, args.explode)

    name = "DainuLionMuscle" if not args.group else f"DainuLionMuscle_{args.group}"
    if args.only_muscle:
        name += "_bare"
    if args.explode:
        name += "_explode"
    skel.data["name"] = name
    skel.data["model_identifier"] = name
    out = args.out or (MODELS / f"{name}.bbmodel")
    out.write_text(json.dumps(skel.data, ensure_ascii=False, indent=1))

    muscle_cubes = sum(1 for e in skel.data["elements"] if e.get("_muscle"))
    print(f"→ {out}")
    print(f"   骨架 {args.skeleton.name}（{len(skel.pivot)} 根骨骼）+ 肌腹 {m.count} 条 / {muscle_cubes} cube")
    print(f"   总 cube {len(skel.data['elements'])}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
