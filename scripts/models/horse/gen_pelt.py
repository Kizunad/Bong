#!/usr/bin/env python3
"""马 —— 皮毛 / 外观层（Round 1/3）。三种毛色 × 三档体型 = 9 份。

第三层，也是**最终进游戏的那层**：骨架和肌肉是解剖参考，皮层才是玩家看到的。

形状不另估一套数字，而是**从肌肉层的包络推导**——按骨骼分组读肌腹点云，沿 z 取包络
再平滑，外扩一点当皮。这样前两层一改（收窄胸廓、削肌肉），外形自动跟着变，不会出现
「肌肉瘦了皮还鼓着」。

马与猫科在外观层的分野：
  · **鬃**（颈脊长毛，倒向一侧）与**额发**、**尾鬃**（远长于尾骨）—— 马的剪影三件套；
  · **距毛**（球节后的毛簇），挽马厚、常马薄、矮马中等；
  · **附蝉**（前肢腕内侧、后肢跗内侧的角质块）—— 马属独有，小但有辨识度；
  · 眼在头**侧上方**且外凸，瞳孔**横长**（不是猫的竖瞳）；
  · 鼻孔大而能张。

毛色由 Coat 定义（一套材质色 + 一组花纹开关），几何三色共用——加一种毛色只加一条
Coat 记录，不碰造型代码。

用法:
  python3 scripts/models/horse/gen_pelt.py                       # 9 份全出
  python3 scripts/models/horse/gen_pelt.py --coat dun --profile medium
  python3 scripts/models/horse/gen_pelt.py --with-anatomy        # 叠在骨+肌上看包裹关系
  python3 scripts/models/horse/gen_pelt.py --list
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
from dataclasses import dataclass, field
from pathlib import Path

from gen_muscle import Skeleton
from gen_skeleton import (
    HEAD_PITCH,
    connected_components,
    NECK,
    NECK_JOINTS,
    PROFILES,
    HeadSpace,
    Profile,
    neck_seams,
    rom,
    shaft_box,
    uid,
)
from PIL import Image

REPO = Path(__file__).resolve().parents[3]
# 目录分工：FINAL 只放最终交付的 9 份皮层（3 毛色 × 3 体型），
# STAGES 放中间产物（骨架 / 肌肉 / 单部位预览 / 带解剖的对照）——两者别混在一起。
FINAL = REPO / "local_models" / "horse"
STAGES = FINAL / "stages"

SWATCH = 8
PELT_ROW = 2  # 贴图第 3 行起（前两行是骨/肌，UV 不动）

# 材质键固定，颜色由 Coat 提供 —— 造型代码只写键名，三种毛色共用同一套几何。
MAT_KEYS = (
    "coat",  # 主体
    "coat_dark",  # 背脊 / 阴影面
    "coat_light",  # 受光面
    "belly",  # 腹侧（多数毛色腹部偏浅）
    "points",  # 鬃尾同色的"点"：下肢、耳缘
    "mane",  # 鬃 / 尾鬃主体
    "mane_tip",  # 鬃梢
    "muzzle",  # 口鼻
    "marking",  # 白章 / 白袜
    "primitive",  # 原始斑纹：鳝背线 / 肩章 / 腿纹
    "hoof",
    "nostril",
    "eye",
    "eye_gloss",
    "chestnut",  # 附蝉
)


@dataclass(frozen=True)
class Coat:
    key: str
    label: str
    mats: dict[str, tuple[int, int, int]]
    features: frozenset[str] = field(default_factory=frozenset)


COATS: dict[str, Coat] = {
    # 锈骝：红褐身 + 黑鬃黑尾黑下肢（真马里最常见的骝毛）。末法配色压低饱和，
    # 像被铁锈与尘土洗过的旧红。
    "rust": Coat(
        "rust",
        "锈骝",
        {
            "coat": (128, 72, 43),
            "coat_dark": (92, 50, 30),
            "coat_light": (156, 94, 58),
            "belly": (112, 64, 40),
            "points": (28, 24, 21),
            "mane": (24, 20, 18),
            "mane_tip": (44, 37, 32),
            "muzzle": (70, 45, 33),
            "marking": (204, 196, 182),
            "primitive": (62, 38, 25),
            "hoof": (46, 40, 36),
            "nostril": (30, 24, 22),
            "eye": (22, 18, 16),
            "eye_gloss": (176, 166, 152),
            "chestnut": (74, 62, 48),
        },
        frozenset({"star", "sock_hind"}),
    ),
    # 枯原：土黄身 + 一整套原始斑纹（鳝背线 / 肩章 / 腿纹）。野型毛色，
    # 在没人管的残土上最该活下来的那一种。
    "dun": Coat(
        "dun",
        "枯原",
        {
            "coat": (148, 126, 82),
            "coat_dark": (114, 96, 61),
            "coat_light": (172, 152, 106),
            "belly": (160, 142, 102),
            "points": (46, 38, 29),
            "mane": (40, 33, 26),
            "mane_tip": (78, 66, 48),
            "muzzle": (92, 80, 56),
            "marking": (206, 200, 186),
            # 原始斑纹要和身色拉开两档才读得出来：首版 58,47,33 与 coat_dark 太近，
            # 鳝背线／肩章／腿纹在侧视里全糊没了
            "primitive": (40, 32, 21),
            "hoof": (58, 50, 42),
            "nostril": (34, 28, 24),
            "eye": (24, 20, 17),
            "eye_gloss": (178, 168, 152),
            "chestnut": (82, 70, 52),
        },
        frozenset({"dorsal_stripe", "shoulder_bar", "leg_bars", "face_mask"}),
    ),
    # 碎雪：灰白斑驳 + 深色头脚。老马转白的过程色，一身斑点像落了雪又化了一半。
    "roan": Coat(
        "roan",
        "碎雪",
        {
            # 身压暗一档、鬃提亮一档：首版身与鬃同在 190 上下，浅鬃直接糊进身上看不出来
            "coat": (146, 140, 131),
            "coat_dark": (104, 99, 92),
            "coat_light": (182, 177, 168),
            "belly": (164, 159, 150),
            "points": (74, 70, 66),
            "mane": (208, 204, 196),
            "mane_tip": (156, 152, 144),
            "muzzle": (86, 82, 78),
            "marking": (214, 210, 200),
            "primitive": (108, 103, 97),
            "hoof": (92, 86, 80),
            "nostril": (36, 32, 30),
            "eye": (26, 22, 20),
            "eye_gloss": (186, 178, 166),
            "chestnut": (96, 88, 76),
        },
        frozenset({"dapple", "dark_head"}),
    ),
}

TORSO_BONES = ("hips", "lumbar", "thorax_back", "thorax_front")
Vec = tuple[float, float, float]


def _lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


# ---------------------------------------------------------------- 接缝交叠
# 皮是一串**刚性**盒子，弯曲全部集中在骨与骨的交界。接缝落在关节 pivot 上时，转角 θ
# 会把接缝张成楔形，半径 r 处开口 2r·sin(θ/2)——首版颈只有三根骨、单关节摊到 51°，
# 鬃在离 pivot 6 个单位处直接裂开 5 个单位（Blockbench 里看就是颈上一排缺口、鬃炸成
# 梳子）。解法两条一起上：① 骨加密（一椎一骨，θ 摊薄）；② 接缝两侧各外扩 δ。
#
# δ 的取法有推导、不是拍的：两侧各外扩 δ 后，半径 r 处 A 的边界在 +δ、B 的边界转到
# θ−atan(δ/r)，不透光的条件是 θ ≤ 2·atan(δ/r)，即 **δ ≥ r·tan(θ/2)**。θ 取该关节
# 登记的 JOINT_ROM，于是这条保证与具体动画无关：只要动画不超 ROM（gen_anim 断言），
# 壳就闭合。
#
# 为什么不做榫（单独一枚细一点的插销）：带内相邻两段的截面本来就在变（颈自 0.098W
# 收到 0.056W），直接外扩不会出现共面 z-fighting；等宽的躯干段另说，见 part_torso。
# 闭式解是**平面铰链**的下界：种子点实际落在交叠体内部（真半径 hypot(r,δ) > r），
# 关节又是俯仰+偏航复合，所以照 δ=r·tan(θ/2) 给总差一口气。这一档余量是量出来的：
# 1.0 时三档十条动画还剩 0.4-1.3 单位的缝，1.35 才全部收进 0.30 以内（shell_check 逐帧核过）。
SEAM_MARGIN = 1.35


def seam_pad(r: float, rom_deg: float) -> float:
    return SEAM_MARGIN * r * math.tan(math.radians(rom_deg) / 2.0)


def face_r(pivot: Vec, half_w: float, y_lo: float, y_hi: float) -> float:
    """接缝面上离关节 pivot 最远的点的距离（面在 z=pivot_z 上，跨 ±half_w × [y_lo,y_hi]）。"""
    return max(math.hypot(half_w, y - pivot[1]) for y in (y_lo, y_hi))


def seam_pads(seams: list[Vec], seg: list[tuple[float, float, float]], roms: list[float]) -> list[float]:
    """一条带的每个分界该外扩多少。

    半径必须取**接缝两侧的并集**：颈自根向头收细，若各段按自己的截面算，细的那侧给的
    交叠就不够——两侧加起来达不到 2r·tan(θ/2)，缝在粗的那一侧照裂（实测 graze 里
    neck_1↔neck_2 还漏 1.9 个单位）。带的两个自由端按各自那一段算。
    """
    out = []
    for k in range(len(seg) + 1):
        near = [seg[i] for i in (k - 1, k) if 0 <= i < len(seg)]
        hw = max(s[0] for s in near)
        lo = min(s[1] for s in near)
        hi = max(s[2] for s in near)
        out.append(seam_pad(face_r(seams[k], hw, lo, hi), roms[k]))
    return out


def _lerp3(a: Vec, b: Vec, t: float) -> Vec:
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t)


def _off(p: Vec, dx: float = 0.0, dy: float = 0.0, dz: float = 0.0) -> Vec:
    return (p[0] + dx, p[1] + dy, p[2] + dz)


def _beyond(a: Vec, b: Vec, d: float) -> Vec:
    """自 a 沿 b→a 方向再走 d。单侧外扩用——对称外扩会把筒伸到下一个关节上去。"""
    v = [a[i] - b[i] for i in range(3)]
    n = math.sqrt(sum(x * x for x in v)) or 1.0
    return tuple(a[i] + v[i] / n * d for i in range(3))


# ---------------------------------------------------------------- 肌肉包络
def _rotmat(rot):
    def R(deg, ax):
        a = math.radians(deg)
        c, s = math.cos(a), math.sin(a)
        if ax == 0:
            return ((1, 0, 0), (0, c, -s), (0, s, c))
        if ax == 1:
            return ((c, 0, s), (0, 1, 0), (-s, 0, c))
        return ((c, -s, 0), (s, c, 0), (0, 0, 1))

    def mul(m, n):
        return tuple(tuple(sum(m[i][k] * n[k][j] for k in range(3)) for j in range(3)) for i in range(3))

    return mul(mul(R(rot[2], 2), R(rot[1], 1)), R(rot[0], 0))


def _corners(e) -> list[Vec]:
    f, t = e["from"], e["to"]
    org = e.get("origin", [0, 0, 0])
    rot = e.get("rotation", [0, 0, 0])
    pts = [(x, y, z) for x in (f[0], t[0]) for y in (f[1], t[1]) for z in (f[2], t[2])]
    if not any(rot):
        return pts
    m = _rotmat(rot)
    return [tuple(org[i] + sum(m[i][k] * (p[k] - org[k]) for k in range(3)) for i in range(3)) for p in pts]


def element_bone_map(data: dict) -> dict[str, str]:
    """element uuid → 所属骨骼名（fmt 4.x / 5.0 通吃）。"""
    groups = {g["uuid"]: g for g in data.get("groups", [])}
    out: dict[str, str] = {}

    def walk(node, bone=None):
        if isinstance(node, str):
            if bone:
                out[node] = bone
            return
        meta = groups.get(node["uuid"], node)
        name = meta.get("name", bone)
        for c in node.get("children", []):
            walk(c, name)

    for root in data["outliner"]:
        walk(root)
    return out


class Envelope:
    """肌肉层按骨骼分组的 z 向包络，供皮层贴着长。"""

    def __init__(self, data: dict) -> None:
        bmap = element_bone_map(data)
        self.pts: dict[str, list[Vec]] = {}
        for e in data["elements"]:
            if not e.get("_muscle"):
                continue
            bone = bmap.get(e["uuid"])
            if bone is None:
                continue
            self.pts.setdefault(bone, []).extend(_corners(e))

    def slice(self, bones, z: float, band: float):
        xs, ys = [], []
        for b in bones:
            for p in self.pts.get(b, ()):
                if abs(p[2] - z) <= band:
                    xs.append(abs(p[0]))
                    ys.append(p[1])
        if len(xs) < 4:
            return None
        return (max(xs), min(ys), max(ys))

    def smooth_profile(self, bones, z0: float, z1: float, step: float, margin: float, band: float, shrink: float = 0.92):
        """沿 z 采包络并平滑。shrink<1 = 别被局部凸起（肩肌）撑大整条皮。"""
        raw = []
        z = z0
        while z <= z1 + 1e-6:
            raw.append((z, self.slice(bones, z, band)))
            z += step
        vals = [s for _, s in raw if s]
        if not vals:
            raise SystemExit("包络为空：肌肉层是否已生成？")
        out = []
        for i, (z, _s) in enumerate(raw):
            near = [raw[j][1] for j in range(max(0, i - 2), min(len(raw), i + 3)) if raw[j][1]]
            if not near:
                near = vals
            hw = sum(n[0] for n in near) / len(near) * shrink + margin
            lo = sum(n[1] for n in near) / len(near) - margin
            hi = sum(n[2] for n in near) / len(near) + margin
            out.append((z, hw, lo, hi))
        return out


# ---------------------------------------------------------------- 贴图 / 造型
def _faces(mat: str) -> dict:
    i = MAT_KEYS.index(mat)
    ox, oy = (i % 8) * SWATCH, (PELT_ROW + i // 8) * SWATCH
    uv = [ox + 1.0, oy + 1.0, ox + SWATCH - 1.0, oy + SWATCH - 1.0]
    return {d: {"uv": list(uv), "texture": 0} for d in ("north", "south", "east", "west", "up", "down")}


def extend_texture(data: dict, coat: Coat) -> None:
    src = data["textures"][0]["source"].split(",", 1)[1]
    img = Image.open(io.BytesIO(base64.b64decode(src))).convert("RGBA")
    px = img.load()
    for i, key in enumerate(MAT_KEYS):
        r, g, b = coat.mats[key]
        ox, oy = (i % 8) * SWATCH, (PELT_ROW + i // 8) * SWATCH
        for y in range(SWATCH):
            for x in range(SWATCH):
                n = ((x * 7 + y * 13 + i * 5) % 5) - 2  # 轻噪：毛面不是平涂塑料
                px[ox + x, oy + y] = (
                    max(0, min(255, r + n * 4)),
                    max(0, min(255, g + n * 4)),
                    max(0, min(255, b + n * 3)),
                    255,
                )
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    data["textures"][0]["source"] = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


class Pelt:
    def __init__(self, skel: Skeleton, P: Profile, coat: Coat) -> None:
        self.skel = skel
        self.P = P
        self.coat = coat
        self.count = 0
        self._names: set[str] = set()

    def has(self, feature: str) -> bool:
        return feature in self.coat.features

    def box(self, bone: str, name: str, frm: Vec, to: Vec, *, rot=None, org=None, mat: str = "coat",
            loose: bool = False, hair: bool = False) -> None:
        """两个声明，问的是**两件不同的事**，别拿一个顶另一个：

        · `loose=True`：这件**本来就该和邻件分开**（耳、唇、额发——它们随各自的骨独立
          动，静止姿蹭在一起只是巧合）。外壳连续性自检据此跳过，否则"张嘴"会被报成裂口。
        · `hair=True`：这件是**毛，不是刚体表面**（鬃、额发、尾鬃）。马具层的贴合判定
          据此跳过——缰绳压进鬃里、笼头低头时扫过鬃，都是真马就会发生的事。

        两者独立：鬃是毛，但整条鬃带跨颈关节的接缝**必须**照样查（它有按 ROM 给的交叠），
        所以鬃是 hair 而不是 loose。反过来耳是 loose 而不是 hair。上一轮把尾鬃梢标成
        loose 来消掉马具那边的误报，是拿错了开关——那是 hair 该管的事。

        这层语义只有造型代码知道，所以在这里声明，不在自检里猜。"""
        if name in self._names:
            raise ValueError(f"重复皮层件名: {name}（uuid 由名字派生，名字必须唯一）")
        self._names.add(name)
        if mat not in MAT_KEYS:
            raise ValueError(f"未知材质 {mat}")
        f = [round(min(a, b), 3) for a, b in zip(frm, to)]
        t = [round(max(a, b), 3) for a, b in zip(frm, to)]
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
                "uuid": uid("pelt", name),
                "_pelt": True,
                "_loose": loose,
                "_hair": hair,
                "from": f,
                "to": t,
                "autouv": 0,
                "color": 2,
                "origin": [round(v, 3) for v in (org or [(a + b) / 2 for a, b in zip(f, t)])],
                "rotation": [round(v, 3) for v in (rot or (0.0, 0.0, 0.0))],
                "faces": _faces(mat),
            },
        )
        self.count += 1

    def limb(self, bone: str, name: str, a: Vec, b: Vec, r0: float, r1: float | None = None, *,
             mat="coat", flat=1.0, joint_rom: float | None = None, slack: float = 0.0) -> None:
        """沿关节 a→b 包一节皮筒（两端半径取均值 —— 轴对齐盒装不下真锥体）。

        joint_rom：该端关节的活动范围。给了就按**截面角点**的力臂算两端外扩量——
        角点不是边心：flat=0.8 的截面，角点力臂比 rz 大 27%，按 rz 算出来的交叠一律偏小。
        slack：筒端离关节 pivot 还差多远，补进外扩里（否则交叠是从筒端算的，不是从
        pivot 算的，差多少就漏多少）。
        """
        r = (r0 + (r0 if r1 is None else r1)) / 2
        ext = 0.0 if joint_rom is None else seam_pad(math.hypot(r * flat, r), joint_rom) + slack
        frm, to, rot, org = shaft_box(a, b, r * flat, r, extend=ext)
        self.box(bone, name, frm, to, rot=rot, org=org, mat=mat)


# ================================================================ 躯干 / 颈
def part_torso(p: Pelt, env: Envelope, P) -> None:
    """躯干皮：沿肌肉包络逐段成形。三层堆出六边形断面（背/胁/腹），
    再叠上马的两条形状线——**腹底自胸围向后上收**（flank tuck）、**腰略收**。"""
    Pr = p.P
    z0 = Pr.z_t1 - 0.06 * Pr.L
    z1 = Pr.z_sacrum + 0.02 * Pr.L
    # 段数刻意压到 5：轴对齐盒堆的躯干，**每个段界都是一条看得见的竖线**——相邻段半宽
    # 不同就露出宽者的端面，半宽相同再让它们重叠就 z-fighting。两条路都走不通，唯一
    # 的办法是把段界做少、且落在身体本来就有分界的位置（肩后 / 胸围 / 腰 / 髋前）。
    step = (z1 - z0) / 5
    prof = env.smooth_profile(TORSO_BONES, z0, z1, step, margin=Pr.u(0.016), band=Pr.u(0.100), shrink=0.94)

    # 五层堆出接近椭圆的断面。三层（腹 0.74 / 胁 1.00 / 背 0.82）各层宽度太接近，
    # 读出来仍是一块方砖 —— 断面要看得出"圆"，宽度差必须拉开到 0.5 ↔ 1.0。
    # 马背在脊上是**窄的**（背线是一道棱，两侧才是竖脊肌鼓起），所以最上层收到 0.58。
    TIERS = (
        ("belly", 0.00, 0.18, 0.54, "belly"),
        ("lower", 0.12, 0.38, 0.86, "coat"),
        ("flank", 0.32, 0.64, 1.00, "coat"),
        ("upper", 0.58, 0.86, 0.90, "coat"),
        ("back", 0.80, 1.00, 0.58, "coat_dark"),
    )

    def _torso_bone(z: float) -> str:
        return "thorax_front" if z < Pr.z_t1 + 0.22 * Pr.L else ("thorax_back" if z < Pr.z_t18 else "lumbar" if z < Pr.z_l6 + 0.02 * Pr.L else "hips")

    seg = []
    for i, (z, hw, lo, hi) in enumerate(prof[:-1]):
        # 腰收：肋后到髋前那一段最窄
        waist = 1.0 - 0.10 * math.exp(-(((z - (Pr.z_t18 + 0.05 * Pr.L)) / (0.10 * Pr.L)) ** 2))
        # 收腹：腹线自胸围（最深处）向后上抬，马的侧影靠这条线
        tuck = Pr.u(0.115) / (1.0 + math.exp(-(z - (Pr.z_t18 - 0.04 * Pr.L)) / (0.055 * Pr.L)))
        seg.append((_torso_bone(z), hw * waist, lo + tuck, hi))

    # 段界**同骨时严格对接**（重叠会让共面的侧面 z-fighting，缝会露出内腔），
    # **跨骨时按 ROM 外扩**——脊柱一节只转 10-14°，但躯干半宽有 5 个单位，
    # 不留交叠时倒毙/受伤那两条会在腰上裂开半个单位。
    pads = []
    for k in range(len(seg) + 1):
        near = [seg[i] for i in (k - 1, k) if 0 <= i < len(seg)]
        cross = len(near) == 2 and near[0][0] != near[1][0]
        if not cross:
            pads.append(0.0)
            continue
        hw = max(s[1] for s in near)
        y0, y1 = min(s[2] for s in near), max(s[3] for s in near)
        r = max(face_r(P(s[0]), hw, y0, y1) for s in near)  # 两根骨的 pivot 取远的那个
        pads.append(seam_pad(r, max(rom(s[0]) for s in near)))

    for i, (bone, hw, lo, hi) in enumerate(seg):
        # prof 的 z 随 i **递增**，所以外扩是前端减、后端加。写反就是每个跨骨段界
        # 反被切掉 2×pad —— 躯干中段直接豁一个三单位宽的洞（静止姿就有）。
        za, zb = prof[i][0] - pads[i], prof[i + 1][0] + pads[i + 1]
        span = hi - lo
        for tag, y0f, y1f, wf, mat in TIERS:
            p.box(bone, f"torso_{tag}_{i + 1}", (-hw * wf, lo + span * y0f, za), (hw * wf, lo + span * y1f, zb), mat=mat)

    # 背中线：跨段连续画一条，避免逐段接缝。枯原毛在这条上再叠鳝背线
    for j in range(0, len(prof) - 1, 2):
        za = prof[j][0]
        zb = prof[min(j + 2, len(prof) - 1)][0]
        top = min(prof[k][3] for k in range(j, min(j + 3, len(prof))))
        bone = "thorax_back" if za < Pr.z_t18 else "lumbar"
        # 背中线也跨骨：同样按 ROM 两端外扩，否则腰上那道接缝在倒毙里裂开
        dpad = seam_pad(face_r(P(bone), Pr.u(0.054), top - Pr.u(0.020), top + Pr.u(0.026)), rom(bone))
        za, zb = za - dpad, zb + dpad
        p.box(bone, f"dorsal_{j // 2 + 1}", (-Pr.u(0.048), top - Pr.u(0.020), za), (Pr.u(0.048), top + Pr.u(0.014), zb), mat="coat_dark")
        # 鳝背线：必须比它下面那条背中线**更宽**（0.054 > 0.048），否则侧视里被背中线
        # 的侧面挡在后面，等于没画。窄条压在宽条上是看不见的。
        if p.has("dorsal_stripe"):
            p.box(bone, f"dorsal_stripe_{j // 2 + 1}", (-Pr.u(0.054), top - Pr.u(0.004), za), (Pr.u(0.054), top + Pr.u(0.026), zb), mat="primitive")
        if p.has("dapple"):  # 碎雪：胁上一排淡斑
            p.box(bone, f"dapple_l_{j // 2 + 1}", (-Pr.u(0.108), top - Pr.u(0.150), za + Pr.u(0.020)), (-Pr.u(0.070), top - Pr.u(0.100), zb - Pr.u(0.020)), mat="coat_light")
            p.box(bone, f"dapple_r_{j // 2 + 1}", (Pr.u(0.070), top - Pr.u(0.150), za + Pr.u(0.020)), (Pr.u(0.108), top - Pr.u(0.100), zb - Pr.u(0.020)), mat="coat_light")

    # 胸前突（马站立时胸口那块前凸的肌肉），正视的轮廓靠它
    y_ch = Pr.y_chest
    p.box("thorax_front", "chest_front", (-Pr.u(0.082), y_ch + Pr.u(0.020), Pr.z_t1 - 0.085 * Pr.L), (Pr.u(0.082), y_ch + Pr.u(0.220), Pr.z_t1 - 0.020 * Pr.L))

    # 尻：躯干最后一段止于 z1，坐骨结节还在其后 —— 直接切掉就是个方屁股。
    # 这里把 z1 → 臀端那一小段补成两层渐窄的圆尻（马的尻是自荐部滑向坐骨的一条弧）。
    # 用**绝对地标**定 z：首版按 L 的比例累加，尻一路做到 z=15 而臀端只在 12.4，
    # 屁股整个甩到尾巴后面去了。
    z_last, hwl, bot, top = prof[-1][0], prof[-1][1], prof[-1][2], prof[-1][3]
    span = top - bot
    for k, (wf, y0f, y1f, z0, z1r) in enumerate((
        (0.84, 0.06, 0.94, z_last - Pr.u(0.10), Pr.z_ischium),
        (0.58, 0.42, 1.00, z_last - Pr.u(0.06), Pr.z_ischium - 0.022 * Pr.L),
    )):
        p.box(
            "hips",
            f"croup_cap_{k + 1}",
            (-hwl * wf, bot + span * y0f, z0),
            (hwl * wf, bot + span * y1f, z1r),
            mat="coat" if k == 0 else "coat_dark",
        )


def crest_y(Pr, z: float) -> float:
    """项韧带那条弦（颈脊）在给定 z 上的高度。鬃与颈皮共用，别各插各的值。"""
    wither_top = (Pr.wither - Pr.u(0.030), Pr.z_wither_peak)
    poll = (Pr.y_poll + Pr.u(0.012), Pr.z_occiput + Pr.u(0.020))
    t = (z - wither_top[1]) / (poll[1] - wither_top[1])
    return _lerp(wither_top[0], poll[0], min(1.0, max(0.0, t))) + Pr.u(0.030)


def part_neck(p: Pelt, env: Envelope, P) -> None:
    """颈皮：断面窄而高（侧扁），上缘走项韧带那条弦（颈脊），下缘在颈椎之下。

    马颈不是圆筒 —— 上宽下窄、左右扁。照猫科套一个圆筒，鬃毛就没地方长。

    分段与颈骨**一一对应**（seams 来自 gen_skeleton，骨/椎/皮/鬃四者同一组分界），
    接缝正落在关节 pivot 上，再按 ROM 两端外扩——见本文件顶上 seam_pad 的推导。
    """
    Pr = p.P
    seams = neck_seams(Pr)

    seg = []
    for i in range(NECK_JOINTS):
        a, b = seams[i], seams[i + 1]  # a = 根侧（本骨 pivot），b = 头侧（下一关节）
        t0, t1 = i / NECK_JOINTS, (i + 1) / NECK_JOINTS
        hw = Pr.u(_lerp(0.098, 0.056, t0))
        # 上缘：项韧带弦（颈脊）；下缘：颈椎下方一段（喉侧）
        y_hi = max(crest_y(Pr, a[2]), crest_y(Pr, b[2]))
        y_lo = min(a[1] - Pr.u(_lerp(0.115, 0.070, t0)), b[1] - Pr.u(_lerp(0.115, 0.070, t1)))
        seg.append((hw, y_lo, y_hi))
    pad = seam_pads(seams, seg, [rom(b) for b in NECK] + [rom("skull")])
    pad[0] *= 2.0  # 鬐甲那道缝是单边的：躯干壳不朝颈里伸，交叠得由颈这边全出

    for i in range(NECK_JOINTS):
        a, b = seams[i], seams[i + 1]
        hw, y_lo, y_hi = seg[i]
        pad_a, pad_b = pad[i], pad[i + 1]
        p.box(NECK[i], f"neck_{i + 1}", (-hw, y_lo, b[2] - pad_b), (hw, y_hi, a[2] + pad_a))
        # 喉下缘提亮（多数毛色颈腹偏浅）。下缘刻意再压 0.02：与颈皮底面共面会 z-fighting。
        p.box(
            NECK[i],
            f"neck_throat_{i + 1}",
            (-hw * 0.82, y_lo - 0.02, b[2] - pad_b),
            (hw * 0.82, y_lo + Pr.u(0.040), a[2] + pad_a),
            mat="belly",
        )

    # 肩章（枯原）：肩上一道横斑。x 要推到躯干半宽**之外**才贴在体表上——
    # 首版 0.070→0.118W 整条埋在肩肌里，渲出来一点看不见。
    if p.has("shoulder_bar"):
        for sx, side in ((-1, "l"), (1, "r")):
            p.box(
                "thorax_front",
                f"shoulder_bar_{side}",
                (sx * Pr.u(0.098), Pr.wither - Pr.u(0.190), Pr.z_t1 + 0.032 * Pr.L),
                (sx * Pr.u(0.150), Pr.wither - Pr.u(0.055), Pr.z_t1 + 0.080 * Pr.L),
                mat="primitive",
            )


# ================================================================ 头
def part_head(p: Pelt, env: Envelope, P) -> None:
    """头：长直脸 + 圆腮 + 大侧眼 + 尖耳 + 大鼻孔。

    马头的辨识度在三处：① 面部**长而直**（鼻梁到吻端几乎一条线）；② 眼长在头的
    **侧上方**且外凸，能看见近乎全周；③ 耳尖而可动，两耳之间垂下额发。
    """
    Pr = p.P
    hs = HeadSpace(None, "skull", (0.0, Pr.y_occiput, Pr.z_occiput), HEAD_PITCH)
    h = Pr.h
    dark_head = p.has("dark_head")

    def hbox(name, lo, hi, *, mat="coat", bone="skull", dp=0.0, loose=False):
        c = tuple((a + b) / 2 for a, b in zip(lo, hi))
        half = tuple(abs(a - b) / 2 for a, b in zip(lo, hi))
        wc = hs.to_world(c)
        p.box(
            bone,
            name,
            tuple(w - hh for w, hh in zip(wc, half)),
            tuple(w + hh for w, hh in zip(wc, half)),
            rot=(HEAD_PITCH + dp, 0.0, 0.0),
            org=wc,
            mat=mat,
            loose=loose,
        )

    base = "coat_dark" if dark_head else "coat"
    # ---- 颅壳 → 面部：逐段前收。整段等宽会把马脸读成一块砖 ----
    for i, (z0, z1, hw, y0, y1, mat) in enumerate((
        (0.070, -0.240, 0.128, -0.115, 0.200, base),  # 枕 / 颅顶
        (-0.240, -0.470, 0.152, -0.150, 0.180, base),  # 额 / 眼区（最宽）
        (-0.470, -0.700, 0.112, -0.215, 0.140, base),  # 中段面
        (-0.700, -0.905, 0.092, -0.250, 0.108, base),  # 吻上段
        (-0.905, -1.035, 0.080, -0.275, 0.082, "muzzle"),  # 鼻端
    )):
        hbox(f"head_shell_{i + 1}", (-h(hw), h(y0), h(z0)), (h(hw), h(y1), h(z1)), mat=mat)

    # 面部中线亮带（多数马脸中线偏浅）+ 白章
    hbox("face_line", (-h(0.038), h(0.088), -h(0.900)), (h(0.038), h(0.150), -h(0.300)), mat="coat_light")
    if p.has("star"):
        hbox("star", (-h(0.046), h(0.104), -h(0.430)), (h(0.046), h(0.156), -h(0.330)), mat="marking")
    if p.has("face_mask"):  # 枯原：脸偏深的"面罩"
        hbox("face_mask", (-h(0.086), -h(0.130), -h(1.030)), (h(0.086), h(0.060), -h(0.820)), mat="primitive")

    for sx, side in ((-1, "l"), (1, "r")):
        # ---- 腮：下颌角那块圆鼓，挂在 jaw 上（张口时跟着动）----
        hbox(f"jowl_{side}", (sx * h(0.086), -h(0.400), -h(0.420)), (sx * h(0.168), -h(0.040), -h(0.090)), mat=base, bone="jaw", loose=True)
        hbox(f"jaw_line_{side}", (sx * h(0.060), -h(0.330), -h(0.900)), (sx * h(0.104), -h(0.170), -h(0.410)), mat=base, bone="jaw", loose=True)
        # ---- 眼：头侧上方、外凸。眼球本体 + 上眼睑 + 一点高光 ----
        hbox(f"eye_socket_{side}", (sx * h(0.140), h(0.010), -h(0.430)), (sx * h(0.176), h(0.110), -h(0.290)), mat=base)
        hbox(f"eye_{side}", (sx * h(0.166), h(0.022), -h(0.412)), (sx * h(0.194), h(0.096), -h(0.308)), mat="eye")
        hbox(f"eye_gloss_{side}", (sx * h(0.190), h(0.062), -h(0.392)), (sx * h(0.202), h(0.088), -h(0.352)), mat="eye_gloss")
        hbox(f"eye_lid_{side}", (sx * h(0.160), h(0.096), -h(0.424)), (sx * h(0.198), h(0.124), -h(0.296)), mat="coat_dark")
        # ---- 耳：尖而窄，向上略外张；耳内深色 ----
        hbox(f"ear_{side}", (sx * h(0.048), h(0.185), -h(0.215)), (sx * h(0.130), h(0.430), -h(0.055)), dp=-8.0, mat=base, loose=True)
        hbox(f"ear_tip_{side}", (sx * h(0.066), h(0.420), -h(0.200)), (sx * h(0.112), h(0.530), -h(0.090)), dp=-8.0, mat="points", loose=True)
        hbox(f"ear_inner_{side}", (sx * h(0.040), h(0.198), -h(0.198)), (sx * h(0.070), h(0.416), -h(0.072)), dp=-8.0, mat="coat_dark", loose=True)
        # ---- 鼻孔：大而斜，马的鼻孔能张 ----
        hbox(f"nostril_{side}", (sx * h(0.026), -h(0.115), -h(1.045)), (sx * h(0.072), -h(0.010), -h(0.955)), mat="nostril")

    # ---- 口鼻：软唇 + 下唇 + 颏 ----
    hbox("muzzle_front", (-h(0.076), -h(0.250), -h(1.048)), (h(0.076), -h(0.100), -h(0.930)), mat="muzzle")
    hbox("lip_upper", (-h(0.068), -h(0.255), -h(1.040)), (h(0.068), -h(0.195), -h(0.900)), mat="muzzle", loose=True)
    hbox("lip_lower", (-h(0.060), -h(0.320), -h(1.020)), (h(0.060), -h(0.250), -h(0.890)), mat="muzzle", bone="jaw", loose=True)
    hbox("chin", (-h(0.052), -h(0.360), -h(0.980)), (h(0.052), -h(0.300), -h(0.860)), mat=base, bone="jaw", loose=True)


# ================================================================ 鬃 / 尾
MANE_SIDE = 1  # 鬃倒向右侧（真马的鬃就是偏向一边，两边对称会读成莫西干）


def part_mane(p: Pelt, env: Envelope, P) -> None:
    """鬃：自枕后沿颈脊铺到鬐甲，整体倒向一侧垂下；两耳之间垂下额发。

    左右对称的鬃会读成莫西干头盔——真马的鬃就是偏向一边的。这里刻意做**不对称**。
    """
    Pr = p.P
    seams = neck_seams(Pr)
    sx = MANE_SIDE
    n = NECK_JOINTS
    thick = {"small": 1.20, "medium": 1.0, "large": 1.15}[Pr.key]  # 矮马/挽马鬃更厚

    # 鬃分段与颈骨一一对应（i=0 在鬐甲、i=6 在枕后），首版按自己一套 t 阈值分骨，
    # 与颈皮的分界错开，两条带在不同的地方裂。
    # 鬃离 pivot 远（垂下大半个颈高），同样的转角在这里张开得最凶——交叠按鬃自己的
    # 半径算，不能跟颈皮共用一个数。
    drops = [Pr.u(0.115 + 0.185 * math.sin(math.pi * (n - i - 0.5) / n)) * thick for i in range(n)]
    seg = []
    for i in range(n):
        ya, yb = crest_y(Pr, seams[i][2]), crest_y(Pr, seams[i + 1][2])
        seg.append((Pr.u(0.086) * thick, min(ya, yb) - drops[i], max(ya, yb) + Pr.u(0.080) * thick))
    # 顶端是**自由端**：鬃到枕后就没了，头上接的是额发（另一条带、随头动）。照 skull 的
    # ROM 给交叠只会把鬃往脸里怼出五个单位，还凭空多出一处"接缝"。
    pads = seam_pads(seams, seg, [rom(b) for b in NECK] + [0.0])
    pads[0] *= 2.0  # 同颈皮：鬃根压在鬐甲上，背那边不朝颈里伸

    for i in range(n):
        a, b = seams[i], seams[i + 1]
        ya, yb = crest_y(Pr, a[2]), crest_y(Pr, b[2])
        y_lo, y_hi = min(ya, yb), max(ya, yb)
        drop = drops[i]
        pad_a, pad_b = pads[i], pads[i + 1]
        za, zb = a[2] + pad_a, b[2] - pad_b
        # 鬃根：骑在颈脊上
        p.box(
            NECK[i],
            f"mane_root_{i + 1}",
            (-Pr.u(0.034), y_lo + Pr.u(0.004), zb),
            (Pr.u(0.034), y_hi + Pr.u(0.080) * thick, za),
            mat="mane",
            hair=True,
        )
        # 鬃披：向一侧垂下，越靠中段越长
        p.box(
            NECK[i],
            f"mane_fall_{i + 1}",
            (sx * Pr.u(0.004), y_lo - drop, zb),
            (sx * Pr.u(0.086) * thick, y_hi + Pr.u(0.056), za),
            rot=(0.0, 0.0, sx * -14.0),
            org=(0.0, y_hi, (a[2] + b[2]) / 2),
            mat="mane",
            hair=True,
        )
        if i in (2, 4):  # 鬃梢：挑两段做出梢色分层，整条同色会读成一块板
            p.box(
                NECK[i],
                f"mane_tip_{i + 1}",
                (sx * Pr.u(0.012), y_lo - drop, zb + Pr.u(0.014)),
                (sx * Pr.u(0.082) * thick, y_lo - drop + Pr.u(0.064), za - Pr.u(0.014)),
                rot=(0.0, 0.0, sx * -14.0),
                org=(0.0, y_hi, (a[2] + b[2]) / 2),
                mat="mane_tip",
                hair=True,
            )

    # 额发：自两耳之间垂到额前
    hs = HeadSpace(None, "skull", (0.0, Pr.y_occiput, Pr.z_occiput), HEAD_PITCH)
    h = Pr.h
    for k, (z0, z1, y0, y1) in enumerate((
        (-0.110, -0.300, 0.150, 0.235),
        (-0.280, -0.470, 0.110, 0.205),
    )):
        lo = (-h(0.052), h(y0), h(z0))
        hi = (h(0.052), h(y1), h(z1))
        c = tuple((a + b) / 2 for a, b in zip(lo, hi))
        half = tuple(abs(a - b) / 2 for a, b in zip(lo, hi))
        wc = hs.to_world(c)
        p.box(
            "skull",
            f"forelock_{k + 1}",
            tuple(w - hh for w, hh in zip(wc, half)),
            tuple(w + hh for w, hh in zip(wc, half)),
            rot=(HEAD_PITCH, 0.0, 0.0),
            org=wc,
            mat="mane" if k == 0 else "mane_tip",
            loose=True,  # 额发挂在头上、鬃挂在颈上，两者只在静止姿相接
            hair=True,
        )


def part_tail(p: Pelt, env: Envelope, P) -> None:
    """尾：肉尾根（dock）+ **远长于尾骨的尾鬃**。马尾的长度全是毛，不是骨。"""
    Pr = p.P
    idx = sorted(int(nm.split("_")[1]) for nm in p.skel.pivot if nm.startswith("tail_"))
    pts = [P(f"tail_{i:02d}") for i in idx]

    # 肉尾根：包住前 3 节尾椎
    for k in range(min(3, len(pts) - 1)):
        r = Pr.u(0.070 - 0.012 * k)
        p.limb(f"tail_{idx[k]:02d}", f"dock_{k + 1}", pts[k], pts[k + 1], r,
               joint_rom=rom(f"tail_{idx[k]:02d}"), mat="coat_dark")

    # 尾鬃：自第 2 节起一路加长加宽，末端垂到接近跗关节高度。
    # 马尾看着有分量是因为它是**一大束**，首版每节只挂一片薄板，读成一根黑棍。
    for k in range(1, len(pts) - 1):
        a, b = pts[k], pts[k + 1]
        t = (k - 1) / max(1, len(pts) - 3)
        wide = Pr.u(0.078 + 0.052 * math.sin(math.pi * min(1.0, t)))
        deep = Pr.u(0.056 + 0.038 * math.sin(math.pi * min(1.0, t)))
        drop = Pr.u(0.130 + 0.330 * t)
        bone = f"tail_{idx[k]:02d}"
        zc = (a[2] + b[2]) / 2
        # 尾鬃悬在尾骨下方大半个尾长，力臂极大——甩尾 / 人立时每个尾关节转 17-21°，
        # 不留交叠就是一节一条缝（shell_check 在 rear 上量到 4.0）。只朝**尾根**方向外扩：
        # 朝尾梢扩会把尾整条加长，那是外形改动不是接缝修补。
        # 尾鬃是**毛**不是皮：它悬在尾骨下方大半个尾长，力臂 10 个单位往上，照 seam_pad
        # 给交叠会把尾整条加长两三个单位、还垂到地里去（实测穿地 0.75）。所以只沿链向
        # 朝**尾根**外扩一小段把接缝盖住，外轮廓（drop / wide）一点不动。
        dy, dz = a[1] - b[1], a[2] - b[2]  # b→a 即朝尾根
        n_ = math.hypot(dy, dz) or 1.0
        pad = min(seam_pad(math.hypot(wide, deep), rom(bone)), n_ * 0.9)
        py, pz = pad * abs(dy) / n_, pad * abs(dz) / n_
        y0 = min(a[1], b[1]) - drop - (py if dy < 0 else 0.0)
        y1 = max(a[1], b[1]) + Pr.u(0.030) + (py if dy > 0 else 0.0)
        z0 = min(a[2], b[2]) - deep - (pz if dz < 0 else 0.0)
        z1 = max(a[2], b[2]) + deep + (pz if dz > 0 else 0.0)
        p.box(bone, f"tailhair_{k}", (-wide, y0, z0), (wide, y1, z1), mat="mane", loose=True, hair=True)
        if k % 2 == 0:
            p.box(
                bone,
                f"tailhair_tip_{k}",
                (-wide * 0.76, min(a[1], b[1]) - drop, zc - deep * 0.8),
                (wide * 0.76, min(a[1], b[1]) - drop + Pr.u(0.075), zc + deep * 0.8),
                mat="mane_tip",
                # 梢与它所属的那股是同一束毛：`loose`（各股本来就分开垂）与 `hair`
                # （马具压过来不算缺陷）两条都得跟着。上一轮只补了 loose 来消掉
                # "蹄铁陷进尾梢 0.93" 的误报——数是对的（倒毙侧躺时尾正好搭在后蹄上），
                # 但用错了开关：那件事归 hair 管。
                loose=True,
                hair=True,
            )


# ================================================================ 四肢
def _leg_column(p: Pelt, P, side: str, sx: int, tag: str) -> None:
    """一条腿的皮：上段跟肌肉、腕/跗以下收成细管、球节鼓、系倾斜、蹄。

    下段的"细"是马腿的形状标志：管骨区皮下只有骨与腱，皮层在这里必须**明显收细**，
    照上段的粗细一路铺下来就成了大象腿。
    """
    Pr = p.P
    dark_lower = p.has("dorsal_stripe") or "points" in p.coat.mats  # 骝/枯原/碎雪都做深下肢
    if tag == "f":
        elbow = P(f"radius_{side}")
        knee = P(f"carpus_{side}")
        fet = P(f"fetlock_f_{side}")
        hoofj = P(f"hoof_f_{side}")
        upper_bone, low_bone, fet_bone, hoof_bone = f"radius_{side}", f"carpus_{side}", f"fetlock_f_{side}", f"hoof_f_{side}"
        r_top, r_knee = Pr.u(0.084), Pr.u(0.042)
    else:
        elbow = P(f"tibia_{side}")
        knee = P(f"tarsus_{side}")
        fet = P(f"fetlock_h_{side}")
        hoofj = P(f"hoof_h_{side}")
        upper_bone, low_bone, fet_bone, hoof_bone = f"tibia_{side}", f"tarsus_{side}", f"fetlock_h_{side}", f"hoof_h_{side}"
        r_top, r_knee = Pr.u(0.098), Pr.u(0.046)

    gauge = Pr.bone_gauge
    # 上段（前臂 / 小腿）：上粗下细
    up_pad = seam_pad(math.hypot(r_top * gauge, r_top * gauge), rom(upper_bone))
    p.limb(upper_bone, f"leg_upper_{tag}_{side}", _beyond(elbow, knee, up_pad), _lerp3(elbow, knee, 0.55), r_top * gauge, mat="coat")
    p.limb(upper_bone, f"leg_upper2_{tag}_{side}", _lerp3(elbow, knee, 0.50), knee, r_knee * 1.24 * gauge, mat="coat",
           joint_rom=rom(low_bone))
    # 腕 / 跗：一个明确的关节鼓。**做成以关节中心为心的方块，不是沿骨的筒**——
    # 腕/跗的活动范围到 96°，而"接缝两侧各外扩 δ"这条路在 θ→90° 时 δ=r·tan(θ/2) 发散，
    # 轴向交叠根本堵不住。以 pivot 为心的方块则不然：绕心转多少度，它罩住的那个半径
    # h 的球都原样还在，裂口只可能出现在 h 之外，所以把 h 取到该处壳的外表面就够了。
    joint_r = r_knee * 1.75 * gauge
    p.box(low_bone, f"leg_joint_{tag}_{side}",
          (knee[0] - joint_r, knee[1] - joint_r, knee[2] - joint_r),
          (knee[0] + joint_r, knee[1] + joint_r, knee[2] + joint_r), mat="coat")
    # 管骨段：细而扁（前后略宽），下肢深色
    mat_low = "points" if dark_lower else "coat"
    p.limb(low_bone, f"cannon_{tag}_{side}", _off(knee, 0, -Pr.u(0.014)), fet, Pr.u(0.034) * gauge, mat=mat_low, flat=0.80,
           joint_rom=rom(fet_bone))
    # 球节：鼓出来的那个球。同腕/跗，96° 的活动范围只能靠"以关节为心的块"堵。
    fet_r = Pr.u(0.040) * 1.35 * gauge
    p.box(fet_bone, f"fetlock_{tag}_{side}",
          (fet[0] - fet_r, fet[1] - fet_r, fet[2] - fet_r),
          (fet[0] + fet_r, fet[1] + fet_r, fet[2] + fet_r), mat=mat_low)
    # 系：52° 前倾插进蹄冠
    pa_r = Pr.u(0.031) * gauge
    p.limb(fet_bone, f"pastern_{tag}_{side}",
           _beyond(_off(fet, 0, -Pr.u(0.010)), hoofj, seam_pad(math.hypot(pa_r, pa_r), rom(fet_bone))),
           _off(hoofj, 0, Pr.u(0.020)), pa_r, mat=mat_low)
    # 蹄：外张的承重环 + 蹄尖上段
    r = Pr.hoof_r * 1.12
    wall_top = Pr.u(0.070) if tag == "f" else Pr.u(0.065)
    p.box(hoof_bone, f"hoof_{tag}_{side}", (hoofj[0] - r, 0.0, hoofj[2] - r * 1.08), (hoofj[0] + r, wall_top * 0.58, hoofj[2] + r * 0.80), mat="hoof")
    # 蹄冠上缘要够到**系的下端**：系停在 hoofj+0.02W，蹄冠原来只到 wall_top，后蹄那两只
    # 差 0.4 个单位没接上，静止姿就是悬空孤岛。往**上**补而不是把系往下伸——系挂在球节上
    # 随腿摆动，伸下去会在支撑相扎进地里（实测穿地 0.30）；蹄只转 12°，向上补是安全的。
    crown = max(wall_top, hoofj[1] + Pr.u(0.030))
    p.box(hoof_bone, f"hoof_top_{tag}_{side}", (hoofj[0] - r * 0.86, wall_top * 0.58, hoofj[2] - r * 1.02), (hoofj[0] + r * 0.86, crown, hoofj[2] + r * 0.12), mat="hoof")

    # 距毛：球节后的毛簇。挽马厚、矮马中、常马薄 —— 三档最直观的外观差之一。
    # 下缘必须夹在 y≥0：挽马那档首版给到 0.098W，比球节离地高度还长，毛垂到 y=−0.37，
    # 于是绑定层取"最低点"当蹄底时抓的是距毛不是蹄，四蹄触地点整体偏了 0.2 单位，
    # 步态自检里表现为挽马专有的离地超差（渲染图上完全看不出来）。
    feather = {"small": 0.055, "medium": 0.034, "large": 0.076}[Pr.key]
    fy0 = max(0.0, fet[1] - Pr.u(feather) - Pr.u(0.030))
    p.box(
        fet_bone,
        f"feather_{tag}_{side}",
        (fet[0] - Pr.u(0.044) * gauge, fy0, fet[2] + Pr.u(0.010)),
        (fet[0] + Pr.u(0.044) * gauge, fet[1] + Pr.u(0.026), fet[2] + Pr.u(0.030) + Pr.u(feather) * 0.5),
        mat=mat_low,
    )

    # 附蝉：前肢在腕上方内侧、后肢在跗下方内侧。马属独有的角质块
    cz = _lerp3(elbow, knee, 0.72) if tag == "f" else _lerp3(knee, fet, 0.24)
    p.box(
        upper_bone if tag == "f" else low_bone,
        f"chestnut_{tag}_{side}",
        (cz[0] - sx * Pr.u(0.030), cz[1] - Pr.u(0.028), cz[2] - Pr.u(0.014)),
        (cz[0] - sx * Pr.u(0.046), cz[1] + Pr.u(0.028), cz[2] + Pr.u(0.014)),
        mat="chestnut",
    )

    # 腿纹（枯原）：管骨上几道横斑，野型斑纹的一部分
    if p.has("leg_bars"):
        for k in range(3):
            t = 0.18 + 0.26 * k
            c = _lerp3(knee, fet, t)
            p.box(
                low_bone,
                f"leg_bar_{tag}_{side}_{k + 1}",
                (c[0] - Pr.u(0.034) * gauge, c[1] - Pr.u(0.012), c[2] - Pr.u(0.034) * gauge),
                (c[0] + Pr.u(0.034) * gauge, c[1] + Pr.u(0.012), c[2] + Pr.u(0.034) * gauge),
                mat="primitive",
            )
    # 白袜（锈骝）：只后肢，且左右不同高 —— 真马的白章本来就不对称
    if p.has("sock_hind") and tag == "h":
        top = 0.42 if side == "l" else 0.24
        c0 = _lerp3(fet, hoofj, -0.10)
        c1 = _lerp3(knee, fet, 1.0 - top)
        p.limb(low_bone, f"sock_{tag}_{side}", c1, c0, Pr.u(0.033) * gauge, mat="marking", flat=0.84)


def part_legs(p: Pelt, env: Envelope, P) -> None:
    for sx, side in ((-1, "l"), (1, "r")):
        _leg_column(p, P, side, sx, "f")
        _leg_column(p, P, side, sx, "h")
    # 肩 / 股的皮由躯干包络覆盖，这里只补肘后与股外的过渡块
    Pr = p.P
    for sx, side in ((-1, "l"), (1, "r")):
        sh = P(f"humerus_{side}")
        p.limb(f"humerus_{side}", f"upper_arm_{side}", _off(sh, 0, Pr.u(0.020)), P(f"radius_{side}"), Pr.u(0.086) * Pr.bone_gauge, mat="coat", flat=0.80,
                joint_rom=rom(f"radius_{side}"))
        st = P(f"tibia_{side}")
        p.limb(f"femur_{side}", f"thigh_{side}", _off(P(f"femur_{side}"), 0, -Pr.u(0.020)), _off(st, 0, Pr.u(0.030)), Pr.u(0.108) * Pr.bone_gauge, mat="coat", flat=0.76,
                joint_rom=rom(f"tibia_{side}"), slack=Pr.u(0.030))


GROUPS = {
    "torso": ("躯干皮 + 背线 / 斑纹", part_torso),
    "neck": ("颈皮 + 肩章", part_neck),
    "head": ("头 / 眼 / 耳 / 口鼻", part_head),
    "mane": ("鬃 + 额发", part_mane),
    "tail": ("尾根 + 尾鬃", part_tail),
    "legs": ("四肢 + 蹄 / 距毛 / 附蝉", part_legs),
}


# ================================================================ CLI
def build(P: Profile, coat: Coat, groups, muscle: Path, with_anatomy: bool) -> Pelt:
    skel = Skeleton(muscle)
    env = Envelope(skel.data)
    extend_texture(skel.data, coat)
    p = Pelt(skel, P, coat)
    for _label, fn in groups:
        fn(p, env, skel.P)
    if not with_anatomy:
        keep = {e["uuid"] for e in skel.data["elements"] if e.get("_pelt")}
        skel.data["elements"] = [e for e in skel.data["elements"] if e["uuid"] in keep]

        def prune(node):
            if isinstance(node, str):
                return node in keep
            node["children"] = [c for c in node.get("children", []) if prune(c)]
            return True

        for root in skel.data["outliner"]:
            prune(root)
    return p


class _Shim:
    """给 connected_components 用的最小外壳（它只读 .elements）。"""

    def __init__(self, els):
        self.elements = els


def check_pelt(p: Pelt) -> list[str]:
    """皮层自检：**贴地**（最低点是四只蹄底，且恰在 y=0）+ 蹄底件齐 + **静止姿单一连通体**。

    骨架层有贴地与连通性两条断言、皮层首版两条都漏了：
      · 贴地漏掉 → 挽马的距毛垂到地下 0.37 单位一直没人发现，直到绑定层拿"最低点"
        当蹄底才暴露；
      · 连通性漏掉 → 躯干段界的外扩符号写反（该减写成了加），每个跨骨段界反被切掉
        2×pad，躯干中段豁了个三单位宽的洞。**动画层的 shell_check 看不见这种洞**——
        它只查静止姿贴合的两件在动画里裂没裂，静止姿就不贴合的对根本不成对。
        所以"静止姿是不是一整块"必须在这一层单独断言。
    凡是要被下游当基准用的东西都得有断言。
    """
    els = [e for e in p.skel.data["elements"] if e.get("_pelt")]
    bad: list[str] = []
    comps = connected_components(_Shim(els))
    if len(comps) > 1:
        detail = " / ".join(f"{len(c)} 件({c[0]}…)" for c in comps[:3])
        bad.append(f"静止姿裂成 {len(comps)} 块，不是一整体：{detail}")
    lo = min(e["from"][1] for e in els)
    if lo < -0.02:
        who = sorted({e["name"] for e in els if e["from"][1] <= lo + 0.02})
        bad.append(f"有件穿到地下：最低 y={lo:.3f}（{', '.join(who[:4])}）")
    for tag in ("f", "h"):
        for side in ("l", "r"):
            hoof = next((e for e in els if e["name"] == f"hoof_{tag}_{side}"), None)
            if hoof is None:
                bad.append(f"缺蹄件 hoof_{tag}_{side}")
            elif abs(hoof["from"][1]) > 0.02:
                bad.append(f"hoof_{tag}_{side} 未着地：y={hoof['from'][1]:.3f}")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser(description="马皮毛层（读肌肉层包络，不回写）")
    ap.add_argument("--profile", choices=[*sorted(PROFILES), "all"], default="all")
    ap.add_argument("--coat", choices=[*sorted(COATS), "all"], default="all")
    ap.add_argument("--group", choices=sorted(GROUPS), help="只生成一个部位")
    ap.add_argument("--with-anatomy", action="store_true", help="保留骨+肌，看包裹关系")
    ap.add_argument("--list", action="store_true", help="列出毛色与部位")
    ap.add_argument("--out", type=Path, help="输出路径（仅单档单色时有效）")
    args = ap.parse_args()

    if args.list:
        print("毛色：")
        for k, c in COATS.items():
            feats = "、".join(sorted(c.features)) or "—"
            print(f"  {k:6s} {c.label}   花纹: {feats}")
        print("部位：")
        for k, (label, _) in GROUPS.items():
            print(f"  {k:6s} {label}")
        return 0

    pkeys = sorted(PROFILES) if args.profile == "all" else [args.profile]
    ckeys = sorted(COATS) if args.coat == "all" else [args.coat]
    todo = [GROUPS[args.group]] if args.group else list(GROUPS.values())
    single = len(pkeys) == 1 and len(ckeys) == 1
    rc = 0

    for ck in ckeys:
        for pk in pkeys:
            P, coat = PROFILES[pk], COATS[ck]
            muscle = STAGES / f"HorseMuscle_{pk}.bbmodel"
            if not muscle.is_file():
                print(f"找不到肌肉层: {muscle}（先跑 gen_muscle.py）")
                return 2
            p = build(P, coat, todo, muscle, args.with_anatomy)
            name = f"HorsePelt_{ck}_{pk}"
            if args.group:
                name += f"_{args.group}"
            if args.with_anatomy:
                name += "_anatomy"
            p.skel.data["name"] = name
            p.skel.data["model_identifier"] = name
            # 只有"完整皮层、无解剖"的那 9 份是交付物；其余一律落 stages/
            final = not args.group and not args.with_anatomy
            out = args.out if (args.out and single) else ((FINAL if final else STAGES) / f"{name}.bbmodel")
            out.parent.mkdir(parents=True, exist_ok=True)
            out.write_text(json.dumps(p.skel.data, ensure_ascii=False, indent=1))
            msg = f"→ {out.relative_to(REPO)}  【{coat.label} · {P.label}】皮层件 {p.count} · 总 cube {len(p.skel.data['elements'])}"
            if final:
                bad = check_pelt(p)
                if bad:
                    rc = 1
                    msg += "\n   ✗ " + "\n   ✗ ".join(bad)
                else:
                    msg += "  ✓ 贴地"
            print(msg)
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
