#!/usr/bin/env python3
"""马骨架生成器 —— 三档体型（矮马 / 常马 / 挽马）。Round 1/3。

真马解剖，不是把狮骨架拉长：
  · **蹄行**（unguligrade）—— 只用第三指着地。腕/跗高悬，管骨（掌骨 III）独存，
    掌骨 II/IV 退化成夹板骨贴在管骨后侧；球节（掌指关节）悬在离地 1/8 体高处，
    系骨 → 冠骨 → 蹄骨一路 50° 前倾插进蹄匣。做成趾行就成了狗。
  · 椎式 C7 · T18 · L6 · S5（融合）· Cd16 —— 18 对肋，腰段短而硬（马背要承重）。
  · 颅骨**眶环闭合**（眶后桥）—— 与猫科开放眶的最大分别；面部极长，门齿与颊齿之间
    留一长段**齿隙**，颊齿是高冠齿，在上下颌里各是一条深齿块。
  · 面嵴自眶下向前延伸 —— 活马脸上看得见的那道棱。
  · 股骨第三转子 · 髋结节 · 跟结节 · 肩胛软骨 —— 马身上顶得出皮的骨点，逐个建出来。

三档不是等比缩放：比例在 Profile 一处定义，全身几何从中派生。

骨骼层级供 GeckoLib 驱动；element 一律写**绝对坐标**（绑定姿态下与骨骼 pivot 自洽），
因为 render_bbmodel.py 只读 elements 不读 outliner。

用法:
  python3 scripts/models/horse/gen_skeleton.py                     # 三档全出
  python3 scripts/models/horse/gen_skeleton.py --profile medium    # 只出常马
  python3 scripts/models/horse/gen_skeleton.py --part skull        # 单件预览
  python3 scripts/models/horse/gen_skeleton.py --check             # 只跑结构自检
  python3 scripts/models/horse/gen_skeleton.py --list              # 列出部件 / 档位
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import uuid
from dataclasses import dataclass, field
from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[3]
# 骨架/肌肉是**中间产物**，放 stages/ 单独存；local_models/horse/ 只留最终 9 份皮层。
OUT_DIR = REPO / "local_models" / "horse" / "stages"

# 单位：MC 像素单位，16 = 1 格 = 1 m。地面 y=0，头朝 -Z（MC north）。

MATS = ("bone", "bone_dark", "cartilage", "tooth", "socket", "hoof")

# uuid 由**名字**确定性派生，不用 uuid4：产物是要入库的，随机 uuid 会让每次重生成
# 都产生一份假 diff（几百行只换了 uuid），review 时分不清真改动。名字在本模块内唯一
# （Rig.cube / Rig.bone 都有重名断言），所以 uuid5 足够。
_NS = uuid.UUID("6f2a1c84-3d5e-4b17-9c0a-7e51d2b8a943")


def uid(kind: str, name: str) -> str:
    return str(uuid.uuid5(_NS, f"horse/{kind}:{name}"))

_COLORS = {
    "bone": (214, 205, 184),
    "bone_dark": (176, 165, 142),
    "cartilage": (198, 200, 192),
    "tooth": (238, 233, 216),
    "socket": (72, 64, 54),
    "hoof": (52, 44, 38),
}

TEX_W = TEX_H = 64
_SWATCH = 8


def _lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def _smoothstep(t: float) -> float:
    t = max(0.0, min(1.0, t))
    return t * t * (3 - 2 * t)


def _curve(knots: list[tuple[float, float]], z: float) -> float:
    """按 z 递减排列的 (z, y) 折线，段内 smoothstep 插值。"""
    if z >= knots[0][0]:
        return knots[0][1]
    if z <= knots[-1][0]:
        return knots[-1][1]
    for (z0, y0), (z1, y1) in zip(knots, knots[1:]):
        if z1 <= z <= z0:
            return _lerp(y0, y1, _smoothstep((z0 - z) / (z0 - z1)))
    return knots[-1][1]


# ================================================================ 体型档位
@dataclass
class Profile:
    """一档体型。所有比例以鬐甲高 `wither` 为基准，绝对几何在 __post_init__ 里派生。

    改体型只动这里的比例；part_* 函数一律从派生量取数，不写死数字。
    """

    key: str
    label: str
    wither: float  # 鬐甲高（单位）
    body_ratio: float  # 体长（肩端→臀端）/ 鬐甲高
    head_ratio: float  # 头长（枕→吻端）/ 鬐甲高
    neck_ratio: float  # 颈长（枕→T1）/ 鬐甲高
    poll_ratio: float  # 枕顶高 / 鬐甲高
    girth_ratio: float  # 胸深（鬐甲→胸底）/ 鬐甲高
    barrel_ratio: float  # 胸廓半宽 / 鬐甲高
    back_ratio: float  # 腰段**棘突尖**高 / 鬐甲高（是骨线不是皮线，皮线还要高一截）
    croup_ratio: float  # 荐结节（骨性尻顶）高 / 鬐甲高
    bone_gauge: float  # 长骨粗细系数（1.0 = 常马）
    hoof_ratio: float  # 蹄半径 / 鬐甲高
    cannon_ratio: float  # 管骨长 / 鬐甲高
    fetlock_ratio: float  # 球节高 / 鬐甲高

    # --- 派生（__post_init__ 填）---
    L: float = field(init=False)
    H: float = field(init=False)
    N: float = field(init=False)

    def __post_init__(self) -> None:
        W = self.wither
        self.L = W * self.body_ratio
        self.H = W * self.head_ratio
        self.N = W * self.neck_ratio

        # --- 纵向（z）地标 ---
        self.z_shoulder = -0.50 * self.L  # 肩端（胸前缘）
        self.z_ischium = 0.50 * self.L  # 臀端（坐骨结节）
        self.z_t1 = -0.40 * self.L
        self.z_t18 = 0.16 * self.L
        self.z_l6 = 0.31 * self.L
        self.z_sacrum = 0.43 * self.L  # 荐椎后缘 = 尾根
        self.z_wither_peak = self.z_t1 + 0.20 * (self.z_t18 - self.z_t1)  # T5

        # --- 竖向（y）地标 ---
        self.y_chest = W * (1.0 - self.girth_ratio)  # 胸底（胸骨腹缘）
        self.y_back = W * self.back_ratio
        self.y_croup = W * self.croup_ratio
        self.y_poll = W * self.poll_ratio
        self.y_occiput = self.y_poll - 0.12 * self.H  # 枕髁（寰枕关节）中心

        # 颈：由颈长 + 枕高反解水平投影，保证颈骨长度不随体型漂移
        dy = self.y_occiput - self.centrum_y_trunk(self.z_t1)
        self.z_occiput = self.z_t1 - math.sqrt(max(self.N * self.N - dy * dy, 1.0))

        # --- 前肢关节 ---
        self.y_f_fetlock = W * self.fetlock_ratio
        self.y_carpus = self.y_f_fetlock + W * self.cannon_ratio
        self.y_elbow = self.y_chest + 0.012 * W
        self.y_shoulder = self.y_chest + 0.085 * W
        self.z_shoulder_j = self.z_shoulder + 0.035 * self.L
        self.z_elbow = self.z_shoulder_j + 0.175 * self.L
        self.z_carpus = self.z_elbow - 0.035 * self.L
        self.z_f_fetlock = self.z_carpus - 0.006 * self.L
        self.x_fore = W * (self.barrel_ratio * 0.72 + 0.018)  # 前肢外展半距

        # --- 后肢关节（自尻高派生，保证三档同一套姿态骨感）---
        self.y_hip = self.y_croup * 0.720
        self.y_stifle = self.y_hip * 0.685
        self.y_hock = self.y_hip * 0.492
        self.y_h_fetlock = W * (self.fetlock_ratio + 0.014)
        self.z_hip = self.z_sacrum - 0.06 * self.L
        self.z_stifle = self.z_hip - 0.140 * self.L
        self.z_hock = self.z_hip + 0.070 * self.L
        self.z_h_fetlock = self.z_hock - 0.030 * self.L
        self.x_hind = W * (self.barrel_ratio * 0.78 + 0.014)

        self.hoof_r = W * self.hoof_ratio
        self.barrel_w = W * self.barrel_ratio

    # ---- 尺寸小工具：比例 → 绝对 ----
    def u(self, frac: float) -> float:
        """鬐甲高的比例 → 绝对长度。"""
        return self.wither * frac

    def r(self, frac: float) -> float:
        """骨干半径：比例 × 粗细系数（挽马骨粗，矮马相对也粗，常马为 1）。"""
        return self.wither * frac * self.bone_gauge

    def h(self, frac: float) -> float:
        """头长的比例 → 绝对长度。"""
        return self.H * frac

    # ---- 中心线 ----
    def centrum_y_trunk(self, z: float) -> float:
        """躯干段椎体中心线（荐 → T1）。马背近乎平直，只在鬐甲前微抬。"""
        W = self.wither
        return _curve(
            [
                (self.z_sacrum, W * 0.800),
                (self.z_l6, W * 0.812),
                (self.z_t18, W * 0.810),
                (self.z_wither_peak, W * 0.828),
                (self.z_t1, W * 0.856),
            ],
            z,
        )

    def centrum_y(self, z: float) -> float:
        """全脊柱中心线：躯干段 + 颈段 S 曲线。

        马的颈椎走**颈下缘**——可见的颈脊是项韧带堆出来的，骨头在下三分之一。
        所以颈段先向前下沉一小段再抬升入枕，直接一条直线拉上去就成了鹅颈。
        """
        if z >= self.z_t1:
            return self.centrum_y_trunk(z)
        t = (self.z_t1 - z) / (self.z_t1 - self.z_occiput)
        t = max(0.0, min(1.0, t))
        y0 = self.centrum_y_trunk(self.z_t1)
        base = _lerp(y0, self.y_occiput, _smoothstep(t))
        dip = self.wither * 0.042 * math.exp(-(((t - 0.26) / 0.30) ** 2))
        return base - dip

    def dorsal_y(self, z: float) -> float:
        """背线（棘突尖连成的轮廓）。鬐甲峰值恰好等于 wither，三档差别全在这条线上。"""
        W = self.wither
        return _curve(
            [
                (self.z_sacrum, self.y_croup),
                (self.z_l6, self.y_back),
                (self.z_t18, self.y_back),
                (self.z_wither_peak, W),
                (self.z_t1, W * 0.985),
            ],
            z,
        )


PROFILES: dict[str, Profile] = {
    # 矮马：头大、腿短、桶身深、鬐甲低平——不是把常马缩小，是换一套比例
    "small": Profile(
        key="small",
        label="矮马",
        wither=19.2,  # 1.20 m
        body_ratio=1.08,
        head_ratio=0.440,
        neck_ratio=0.500,
        poll_ratio=1.200,
        girth_ratio=0.540,
        barrel_ratio=0.118,
        back_ratio=0.912,
        croup_ratio=0.972,
        bone_gauge=1.08,
        hoof_ratio=0.048,
        cannon_ratio=0.145,
        fetlock_ratio=0.130,
    ),
    # 常马：标准比例基准。体长≈鬐甲高（方形马），胸深=腿长（经典二分律）
    "medium": Profile(
        key="medium",
        label="常马",
        wither=24.8,  # 1.55 m
        body_ratio=1.00,
        head_ratio=0.400,
        neck_ratio=0.550,
        poll_ratio=1.160,
        girth_ratio=0.500,
        barrel_ratio=0.104,
        back_ratio=0.882,
        croup_ratio=0.944,
        bone_gauge=1.00,
        hoof_ratio=0.042,
        cannon_ratio=0.160,
        fetlock_ratio=0.120,
    ),
    # 挽马：骨粗、尻高于鬐甲、蹄盘大、颈短厚——挽具受力全靠这套骨相
    "large": Profile(
        key="large",
        label="挽马",
        wither=28.8,  # 1.80 m
        body_ratio=1.06,
        head_ratio=0.375,
        neck_ratio=0.500,
        poll_ratio=1.110,
        girth_ratio=0.525,
        barrel_ratio=0.128,
        back_ratio=0.918,
        croup_ratio=0.998,
        bone_gauge=1.32,
        hoof_ratio=0.056,
        cannon_ratio=0.150,
        fetlock_ratio=0.115,
    ),
}


# ---------------------------------------------------------------- rig 容器
class Rig:
    """收集 element + 骨骼树，最后组装成 .bbmodel。

    与 scripts/models/dainu_lion/gen_skeleton.py 同构（同一套 bbmodel 装配约定），
    但材质表按马换过：无角，多蹄匣。
    """

    def __init__(self) -> None:
        self.elements: list[dict] = []
        self.bones: dict[str, dict] = {}
        self.bone_order: list[str] = []

    def bone(self, name: str, pivot: tuple[float, float, float], parent: str | None = None) -> str:
        if name in self.bones:
            raise ValueError(f"重复骨骼: {name}")
        if parent is not None and parent not in self.bones:
            raise ValueError(f"{name} 的父骨骼 {parent} 尚未定义（骨骼须先父后子）")
        self.bones[name] = {
            "uuid": uid("bone", name),
            "pivot": [round(v, 3) for v in pivot],
            "parent": parent,
            "children": [],
        }
        self.bone_order.append(name)
        return name

    def cube(
        self,
        bone: str,
        name: str,
        frm: tuple[float, float, float],
        to: tuple[float, float, float],
        *,
        rot: tuple[float, float, float] | None = None,
        org: tuple[float, float, float] | None = None,
        mat: str = "bone",
    ) -> dict:
        if bone not in self.bones:
            raise ValueError(f"未定义骨骼: {bone}")
        if mat not in MATS:
            raise ValueError(f"未知材质: {mat}")
        if any(e["name"] == name for e in self.elements):
            raise ValueError(f"重复 element 名: {name}（uuid 由名字派生，名字必须唯一）")
        f = [round(min(a, b), 3) for a, b in zip(frm, to)]
        t = [round(max(a, b), 3) for a, b in zip(frm, to)]
        eid = uid("cube", name)
        el = {
            "name": name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "render_order": "default",
            "allow_mirror_modeling": True,
            "type": "cube",
            "uuid": eid,
            "from": f,
            "to": t,
            "autouv": 0,
            "color": MATS.index(mat) % 8,
            "origin": [round(v, 3) for v in (org or _center(f, t))],
            "rotation": [round(v, 3) for v in (rot or (0.0, 0.0, 0.0))],
            "faces": _faces(mat),
        }
        self.elements.append(el)
        self.bones[bone]["children"].append(eid)
        return el

    def outliner(self) -> list[dict]:
        nodes: dict[str, dict] = {}
        for name in self.bone_order:
            b = self.bones[name]
            nodes[name] = {
                "name": name,
                "origin": b["pivot"],
                "rotation": [0.0, 0.0, 0.0],
                "uuid": b["uuid"],
                "export": True,
                "mirror_uv": False,
                "isOpen": False,
                "locked": False,
                "visibility": True,
                "autouv": 0,
                "children": list(b["children"]),
            }
        roots = []
        for name in self.bone_order:
            parent = self.bones[name]["parent"]
            if parent is None:
                roots.append(nodes[name])
            else:
                nodes[parent]["children"].append(nodes[name])
        return roots

    def bbmodel(self, model_name: str) -> dict:
        return {
            "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
            "name": model_name,
            "model_identifier": model_name,
            "resolution": {"width": TEX_W, "height": TEX_H},
            "elements": self.elements,
            "outliner": self.outliner(),
            "textures": [
                {
                    "path": "",
                    "name": f"{model_name}.png",
                    "folder": "",
                    "namespace": "bong",
                    "id": "0",
                    "particle": True,
                    "render_mode": "default",
                    "visible": True,
                    "mode": "bitmap",
                    "saved": False,
                    "uuid": uid("texture", model_name),
                    "source": "data:image/png;base64," + _texture_b64(),
                }
            ],
        }


def _center(f: list[float], t: list[float]) -> list[float]:
    return [(a + b) / 2 for a, b in zip(f, t)]


Vec = tuple[float, float, float]


def shaft_box(a: Vec, b: Vec, rx: float, rz: float, extend: float = 0.0):
    """把「从关节 a 到关节 b、截面 rx×rz 的柱」解成 (from, to, rotation, origin)。

    纯几何、无副作用——骨干一律靠它定向，肌肉层日后也 import 这个。

    数学：cube 沿局部 +Y 建长 L 的柱，旋转按 render/Blockbench 的 R=Rz·Ry·Rx。
    Rx(p) 把 (0,1,0) 转到 (0,cos p,sin p)，再 Ry(w) 得 (sin w·sin p, cos p, cos w·sin p)，
    与目标单位向量对齐即解出 p=acos(dy/L)、w=atan2(dx,dz)。
    """
    dx, dy, dz = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
    length = math.sqrt(dx * dx + dy * dy + dz * dz)
    if length < 1e-6:
        raise ValueError("关节 a/b 重合，无法定向")
    half = length / 2 + extend
    pitch = math.degrees(math.acos(max(-1.0, min(1.0, dy / length))))
    yaw = math.degrees(math.atan2(dx, dz)) if abs(dx) + abs(dz) > 1e-9 else 0.0
    cx, cy, cz = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2)
    return (
        (cx - rx, cy - half, cz - rz),
        (cx + rx, cy + half, cz + rz),
        (pitch, yaw, 0.0),
        (cx, cy, cz),
    )


def _shaft(
    rig: Rig,
    bone: str,
    name: str,
    a: Vec,
    b: Vec,
    rx: float,
    rz: float | None = None,
    *,
    mat: str = "bone",
    extend: float = 0.0,
) -> None:
    """造一根**端点精确落在关节 a、b 上**的骨干。长骨一律走这里，别手写旋转。"""
    rz = rx if rz is None else rz
    try:
        frm, to, rot, org = shaft_box(a, b, rx, rz, extend)
    except ValueError as exc:
        raise ValueError(f"{name}: {exc}") from exc
    rig.cube(bone, name, frm, to, rot=rot, org=org, mat=mat)


# ---------------------------------------------------------------- 调色板贴图
def _texture_b64() -> str:
    img = Image.new("RGBA", (TEX_W, TEX_H), (0, 0, 0, 0))
    px = img.load()
    for i, mat in enumerate(MATS):
        r, g, b = _COLORS[mat]
        ox, oy = (i % 8) * _SWATCH, (i // 8) * _SWATCH
        for y in range(_SWATCH):
            for x in range(_SWATCH):
                # 轻噪：骨面不是塑料平涂，渲染时能看出体积
                n = ((x * 7 + y * 13 + i * 5) % 5) - 2
                px[ox + x, oy + y] = (
                    max(0, min(255, r + n * 4)),
                    max(0, min(255, g + n * 4)),
                    max(0, min(255, b + n * 3)),
                    255,
                )
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return base64.b64encode(buf.getvalue()).decode()


def _faces(mat: str) -> dict:
    i = MATS.index(mat)
    ox, oy = (i % 8) * _SWATCH, (i // 8) * _SWATCH
    uv = [ox + 1.0, oy + 1.0, ox + _SWATCH - 1.0, oy + _SWATCH - 1.0]
    return {d: {"uv": list(uv), "texture": 0} for d in ("north", "south", "east", "west", "up", "down")}


def _mid(a: Vec, b: Vec) -> Vec:
    return ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2)


def neck_centers(P: Profile) -> list[Vec]:
    """C7 → C1 的椎体中心，末节锚在枕髁前一点点（保证寰椎与颅骨对接）。"""
    z0 = P.z_t1 - abs(P.z_t1 - P.z_occiput) * 0.06
    z1 = P.z_occiput + P.h(0.05)
    return [(0.0, P.centrum_y(_lerp(z0, z1, i / 6)), _lerp(z0, z1, i / 6)) for i in range(7)]


# ---------------------------------------------------------------- 颈：一椎一骨
# 首版颈只有三根骨（base/mid/top），而颈皮与鬃各有 7 段。吃草要把颈弯到 −112°，三根骨
# 摊下来单关节 37-51°，交界两侧的刚性盒子绕 pivot 张成楔形，离 pivot 6 个单位的鬃直接
# 裂开 5 个单位——渲出来颈上一排缺口、鬃像梳子一样炸开（用户在 Blockbench 里看到的
# 就是这个）。裂口宽度是 2r·sin(θ/2)，**只能靠把 θ 摊薄**：关节数翻一倍多，裂口按
# sin 线性收窄，剩下的由皮层的接缝交叠吃掉（见 gen_pelt.seam_overlap）。
NECK_JOINTS = 7
# 单个颈关节的活动范围（度，逐轴）。**壳按这个数留交叠、动画按这个数被断言**——两头认
# 同一个数，接缝才不会在某条动画里突然张开。真马颈椎间的屈伸活动度约 15-25°，取 28；
# 侧弯按整颈能弯到胁侧（~100°）摊到七节；颈椎的**轴向扭转**除寰枢外很小，只给 8。
NECK_ROM = (28.0, 14.0, 8.0)
NECK = tuple(f"neck_{i + 1}" for i in range(NECK_JOINTS))  # 自鬐甲向上：neck_1 → neck_7
TAIL_ROM = (24.0, 24.0, 12.0)  # 单节尾骨（甩尾 / 人立时尾整体扬起 130°，摊到 8 节）


# 各关节的活动范围（度，相对静止姿的最大转角，**逐轴** (x 俯仰, y 偏航, z 翻滚)）。
# **这张表是壳与动画的共同契约**：皮层照它给接缝留交叠（gen_pelt.seam_pad），动画层
# 被断言不许超（gen_anim.sanity）。两头认同一个数，接缝才不会在某条动画里突然张开
# ——各写各的就等于没约定。数值取真马活动度并对现有十条动画留余量；腿的屈伸与
# rig.LIMITS 同源（gen_anim 起手会对拍，谁改了对不上就撞红）。
#
# **为什么逐轴而不是一个标量**：马的四肢是矢状面内的铰链——肘 / 膝 / 腕 / 跗 / 球节
# 在这套 rig 里 y 轴恒为 0（rig.solve_leg 只写 rot[0]，外展只在腿根 rot[2] 且被
# `rig.ABDUCT` 夹在 9°）。一个标量摊给三轴，等于声称"肘能外撇 80°"——壳为一个解剖上
# 不存在、rig 也摆不出来的姿势去留交叠，留不出来就被判红。逐轴之后，扫描查的才是这匹
# 马真能摆出来的姿势。**放宽的只是不存在的自由度，屈伸那一路一度没松。**
JOINT_ROM: dict[str, tuple[float, float, float]] = {
    "root": (90.0, 90.0, 90.0),  # 整体骨，倒毙侧翻 84°；无父骨，不承担接缝
    "hips": (14.0, 14.0, 14.0),
    "lumbar": (12.0, 12.0, 6.0),  # 马的腰段在翻滚方向很硬
    "thorax_back": (12.0, 12.0, 6.0),
    "thorax_front": (10.0, 10.0, 6.0),
    "skull": (84.0, 24.0, 20.0),  # 吃草时头相对末节颈骨几乎垂下来；侧看 / 歪头另计
    "jaw": (24.0, 0.0, 0.0),  # 纯铰链
    "scapula": (20.0, 0.0, 9.0),  # 翻滚 = 外展，上限同 rig.ABDUCT
    "humerus": (56.0, 0.0, 0.0),
    "radius": (80.0, 0.0, 0.0),  # 肘：纯铰链
    "carpus": (108.0, 0.0, 26.0),  # 腕承接腿根外展 + 蹄面找平，翻滚实测到 21°
    "fetlock_f": (24.0, 0.0, 26.0),
    "hoof_f": (12.0, 0.0, 26.0),
    "femur": (48.0, 0.0, 9.0),
    "tibia": (60.0, 0.0, 0.0),  # 膝：纯铰链
    "tarsus": (70.0, 0.0, 0.0),  # 跗：纯铰链
    "fetlock_h": (96.0, 0.0, 32.0),  # 后球节翻滚实测到 27°
    "hoof_h": (12.0, 0.0, 32.0),
}


def rom_axes(bone: str) -> tuple[float, float, float]:
    """这根骨逐轴允许的最大转角。颈 / 尾按族给，四肢去掉 _l/_r 后缀查表。"""
    if bone in JOINT_ROM:
        return JOINT_ROM[bone]
    if bone.startswith("neck_"):
        return NECK_ROM
    if bone.startswith("tail_"):
        return TAIL_ROM
    base = bone[:-2] if bone.endswith(("_l", "_r")) else bone
    if base in JOINT_ROM:
        return JOINT_ROM[base]
    raise KeyError(f"{bone} 没有登记活动范围——新骨必须进 JOINT_ROM，否则皮层不知道留多少交叠")


def rom(bone: str) -> float:
    """这根骨的**最大**转角（三轴取大者）。

    皮层留交叠用这个：接缝的交叠是沿骨轴的一段长度，一段长度盖不了"分轴给量"，
    只能按最坏的那一轴留。逐轴的量给检查器（shell_check.rom_sweep）和动画断言用。
    """
    return max(rom_axes(bone))


def neck_seams(P: Profile) -> list[Vec]:
    """8 个分界点 = 7 个椎间关节 + 顶端。相邻两点之间正好是一节颈椎，也正好是一根颈骨、
    一段颈皮、一段鬃 —— 骨 / 椎 / 皮 / 鬃四者的分界从此是同一组数。

    分界取**椎体中心的中点**而不是椎体中心：关节在椎与椎之间，不在椎体上。
    """
    cen = neck_centers(P)
    chain = [(0.0, P.centrum_y(P.z_t1), P.z_t1), *cen, (0.0, P.y_occiput, P.z_occiput)]
    return [_mid(chain[i], chain[i + 1]) for i in range(NECK_JOINTS + 1)]


def neck_pivots(P: Profile) -> list[Vec]:
    """7 根颈骨的 pivot —— 落在颈皮**相邻两段的共用面**上。

    pivot 必须落在接缝上：这样关节一转，两段仍在 pivot 处相接，张开的只是随半径线性
    增长的楔形（可以用交叠吃掉）；pivot 一旦偏离接缝，整个接缝面平移，从第一度起就裂。
    """
    return neck_seams(P)[:NECK_JOINTS]


def neck_bone_at(P: Profile, z: float) -> str:
    """按 z 落到哪一节颈骨。颈皮 / 鬃 / 肌肉共用这一个分界函数，别各写一份阈值——
    首版颈皮按 t<0.34/0.72 分、鬃按 t<0.28/0.62 分，两条带的接缝落在不同的地方。"""
    zs = [p[2] for p in neck_seams(P)]  # 自鬐甲向头单调递减
    for i in range(NECK_JOINTS):
        if z >= min(zs[i], zs[i + 1]) - 1e-9:
            return NECK[i]
    return NECK[-1]


def _neck_bones(rig: Rig, P: Profile) -> None:
    parent = "thorax_front"
    for name, piv in zip(NECK, neck_pivots(P)):
        rig.bone(name, piv, parent=parent)
        parent = name


# ================================================================ 部件：脊柱
def part_spine(rig: Rig, P: Profile) -> None:
    """颈 7 · 胸 18 · 腰 6 · 荐 5（融合）。棘突高度由 dorsal_y 反推，鬐甲峰值恰好落在 wither。"""
    rig.bone("hips", (0.0, P.centrum_y(P.z_sacrum), P.z_sacrum - P.u(0.03)), parent="root")
    rig.bone("lumbar", (0.0, P.centrum_y(P.z_l6), P.z_l6), parent="hips")
    z_tmid = (P.z_t1 + P.z_t18) / 2
    rig.bone("thorax_back", (0.0, P.centrum_y(P.z_t18), P.z_t18), parent="lumbar")
    rig.bone("thorax_front", (0.0, P.centrum_y(z_tmid), z_tmid), parent="thorax_back")
    _neck_bones(rig, P)

    seg_t = (P.z_t18 - P.z_t1) / 17.0  # 胸椎节距

    # --- 荐椎 5（融合成一块荐骨，背侧一条荐嵴）---
    sac_len = P.L * 0.115
    for i in range(5):
        z = P.z_sacrum - sac_len * (i / 4.0)
        y = P.centrum_y(z)
        rig.cube(
            "hips",
            f"sacral_{i + 1}",
            (-P.u(0.055), y - P.u(0.036), z - sac_len / 7),
            (P.u(0.055), y + P.u(0.036), z + sac_len / 7),
            mat="bone_dark",
        )
    rig.cube(
        "hips",
        "sacral_crest",
        (-P.u(0.022), P.centrum_y(P.z_sacrum) + P.u(0.030), P.z_sacrum - sac_len),
        (P.u(0.022), P.y_croup, P.z_sacrum - sac_len * 0.15),
    )
    # 荐结节（尻顶两个骨点，活马身上顶得出皮）
    for sx, side in ((-1, "l"), (1, "r")):
        rig.cube(
            "hips",
            f"tuber_sacrale_{side}",
            (sx * P.u(0.020), P.y_croup - P.u(0.055), P.z_sacrum - sac_len * 0.95),
            (sx * P.u(0.062), P.y_croup, P.z_sacrum - sac_len * 0.45),
            mat="bone_dark",
        )

    # --- 腰椎 6（横突宽大平展，马腰短而硬）---
    lum_z0, lum_z1 = P.z_l6, P.z_t18 + seg_t * 0.6
    for i in range(6):
        t = i / 5
        z = _lerp(lum_z0, lum_z1, t)
        y = P.centrum_y(z)
        # 6 节铺在 5 段间隔上（t = i/5），节距就得除以 5 —— 除以 6 会让每节短 17%，
        # 椎体两两留缝、腰段整段断开（连通性自检抓的第二处）。
        step = abs(lum_z0 - lum_z1) / 5
        # 椎体前后长必须 **≥ 节距**：椎间盘在这个粒度下等于零厚，椎体本就是节节相接的。
        # 首版写 0.90 节距，六节腰椎各自悬空成岛（连通性自检抓出来的，三视图看不见）。
        rig.cube(
            "lumbar",
            f"lumbar_{6 - i}",
            (-P.u(0.050), y - P.u(0.038), z - step * 0.55),
            (P.u(0.050), y + P.u(0.038), z + step * 0.55),
        )
        sp = P.dorsal_y(z) - y
        rig.cube(
            "lumbar",
            f"lumbar_sp_{6 - i}",
            (-P.u(0.020), y + P.u(0.030), z - step * 0.28),
            (P.u(0.020), y + max(P.u(0.055), sp), z + step * 0.28),
            rot=(12.0, 0.0, 0.0),
            org=(0.0, y + P.u(0.030), z),
        )
        # 横突：水平薄板向外——马的腰横突是一排平翼，不是猫那种斜刺
        for sx, side in ((-1, "l"), (1, "r")):
            rig.cube(
                "lumbar",
                f"lumbar_tp_{6 - i}_{side}",
                (sx * P.u(0.042), y - P.u(0.008), z - step * 0.34),
                (sx * P.u(0.100), y + P.u(0.016), z + step * 0.34),
                rot=(0.0, sx * -8.0, sx * -4.0),
                org=(sx * P.u(0.042), y, z),
                mat="bone_dark",
            )

    # --- 胸椎 18（T3-T11 棘突高耸 = 鬐甲）---
    for i in range(18):
        t = i / 17
        z = _lerp(P.z_t18, P.z_t1, t)
        y = P.centrum_y(z)
        bone = "thorax_back" if z > z_tmid else "thorax_front"
        # 同腰椎：前后长取 1.10 倍节距，保证 T1→T18 是一根连续的柱而不是 18 座孤岛
        rig.cube(
            bone,
            f"thoracic_{18 - i}",
            (-P.u(0.044), y - P.u(0.034), z - seg_t * 0.55),
            (P.u(0.044), y + P.u(0.034), z + seg_t * 0.55),
        )
        sp = P.dorsal_y(z) - y
        rig.cube(
            bone,
            f"thoracic_sp_{18 - i}",
            (-P.u(0.017), y + P.u(0.028), z - seg_t * 0.34),
            (P.u(0.017), y + max(P.u(0.050), sp), z + seg_t * 0.34),
            rot=(_lerp(8.0, -14.0, t), 0.0, 0.0),
            org=(0.0, y + P.u(0.028), z),
        )
        # 肋凹 / 横突（肋头就接在这儿）
        for sx, side in ((-1, "l"), (1, "r")):
            rig.cube(
                bone,
                f"thoracic_tp_{18 - i}_{side}",
                (sx * P.u(0.040), y - P.u(0.012), z - seg_t * 0.24),
                (sx * P.u(0.074), y + P.u(0.020), z + seg_t * 0.24),
                mat="bone_dark",
            )

    # --- 颈椎 7（C1 寰椎宽翼、C2 枢椎长且带背嵴）---
    # 马颈以 40-50° 上升，椎体**必须沿颈轴定向**：首版用轴对齐盒（只在 z 上给长度），
    # 每节之间空出整段 y 差，侧视是 7 个悬空方块。这里改成中心链 —— 每节从「与前一节
    # 的中点」连到「与后一节的中点」，相邻节共用端点，链上不可能有缝。
    cen = neck_centers(P)
    chain = [(0.0, P.centrum_y(P.z_t1), P.z_t1), *cen, (0.0, P.y_occiput, P.z_occiput)]
    step = abs(P.z_t1 - P.z_occiput) / 7
    for i in range(7):
        t = i / 6
        idx = 7 - i  # C7 … C1
        _x, y, z = cen[i]
        bone = NECK[i]  # 一椎一骨
        half_w = P.u(0.038) if idx > 2 else P.u(0.050)
        a = _mid(chain[i], chain[i + 1])
        b = _mid(chain[i + 1], chain[i + 2])
        _shaft(rig, bone, f"cervical_{idx}", a, b, half_w, half_w * 0.92)
        if idx >= 3:  # C7→C3 有矮棘突；寰枢椎另做
            _shaft(
                rig,
                bone,
                f"cervical_sp_{idx}",
                (0.0, y + P.u(0.028), z),
                (0.0, y + P.u(0.028) + P.u(_lerp(0.066, 0.022, t)), z + P.u(_lerp(0.030, 0.010, t))),
                P.u(0.016),
                P.u(0.022),
            )
        for sx, side in ((-1, "l"), (1, "r")):
            wing = P.u(0.104) if idx == 1 else (P.u(0.062) if idx == 2 else P.u(0.040))
            rig.cube(
                bone,
                f"cervical_tp_{idx}_{side}",
                (sx * half_w, y - P.u(0.026), z - step * 0.30),
                (sx * (half_w + wing), y + P.u(0.010), z + step * 0.30),
                mat="bone_dark",
            )
    # C2 枢椎背嵴（马身上很长的一条，向后上斜出）
    _x, y_ax, z_ax = cen[5]
    _shaft(
        rig,
        neck_bone_at(P, z_ax),
        "axis_crest",
        (0.0, y_ax + P.u(0.030), z_ax - P.u(0.030)),
        (0.0, y_ax + P.u(0.086), z_ax + P.u(0.075)),
        P.u(0.018),
        P.u(0.030),
    )


# ================================================================ 部件：尾
TAIL_VERTEBRAE = 16
TAIL_BONES = 8


def part_tail(rig: Rig, P: Profile) -> None:
    """尾椎 16 节 → 8 根骨骼。马尾根平出后再垂——起始段近水平，是马尾的形状标志。

    尾骨（尾根）总长取 0.30 鬐甲高 ≈ 46 cm（真马 40-50 cm）。首版按猫科的节距写，
    16 节堆出 1.5 m 的骨尾，比躯干还长——马的**长尾是鬃毛不是骨头**。
    """
    seg = P.u(0.019)
    z, y = P.z_sacrum, P.centrum_y(P.z_sacrum)
    pts: list[tuple[float, float, float]] = []
    for i in range(TAIL_VERTEBRAE):
        t = i / (TAIL_VERTEBRAE - 1)
        pts.append((z, y, t))
        # 尾根平出 → 中段垂 → 末梢近垂直
        deg = _lerp(6.0, -74.0, _smoothstep(min(1.0, t / 0.55)))
        ang = math.radians(deg)
        z += seg * math.cos(ang)
        y += seg * math.sin(ang)

    parent = "hips"
    for b in range(TAIL_BONES):
        z0, y0, _ = pts[b * 2]
        name = f"tail_{b + 1:02d}"
        rig.bone(name, (0.0, y0, z0), parent=parent)
        for k in range(2):
            zz, yy, tt = pts[b * 2 + k]
            rr = P.u(_lerp(0.038, 0.009, tt))
            rig.cube(
                name,
                f"caudal_{b * 2 + k + 1}",
                (-rr, yy - rr, zz - seg * 0.5),
                (rr, yy + rr, zz + seg * 0.5),
                mat="bone" if b < 5 else "bone_dark",
            )
        parent = name


# ================================================================ 部件：胸廓
# 肋弓横截面控制点：(半宽比例, 深度比例)，自椎侧绕到胸骨侧。
# 马的胸廓深而窄、断面呈梨形（上窄下宽再收），与猫科的卵圆截然不同。
RIB_ARC = ((0.30, 0.0), (0.86, 0.20), (1.00, 0.48), (0.84, 0.78), (0.30, 1.0))


def rib_profile(P: Profile, z: float):
    """该 z 处胸廓的 (半宽, 深度, 椎体顶高, 前后 drift)。

    马：最深处在**肘后**（鞍前的胸围位置），最宽处更靠后（在最后几根肋 / 腹胁）。
    这两个峰不在同一处，所以宽和深各用一条峰值曲线，不能共用一个 bulge。
    """
    span = P.z_t1 - P.z_t18
    t = max(0.0, min(1.0, (z - P.z_t18) / span))  # 0=T18(后) 1=T1(前)
    wide = math.exp(-(((t - 0.30) / 0.44) ** 2))  # 最宽：T12-T14
    deep = math.exp(-(((t - 0.62) / 0.40) ** 2))  # 最深：T6-T8（肘后）
    y_top = P.centrum_y(z)
    depth = y_top - P.u(0.030) - _lerp(P.y_chest + P.u(0.085), P.y_chest, deep)
    return (
        _lerp(P.barrel_w * 0.58, P.barrel_w, wide),
        depth,
        y_top,
        _lerp(0.0, P.u(0.055), t),
    )


def rib_surface_point(P: Profile, z: float, fd: float, sx: int = 1):
    """肋弓外表面上的一点。fd = 深度比例（0 = 椎侧，1 = 胸骨侧）。

    肌肉层日后 import 这个来铺贴合胸廓的肌带——两边共用同一条弧，阔肌才不会
    悬空或陷进肋骨。
    """
    half_w, depth, y_top, drift = rib_profile(P, z)
    fd = max(0.0, min(1.0, fd))
    fw = RIB_ARC[-1][0]
    for (w0, d0), (w1, d1) in zip(RIB_ARC, RIB_ARC[1:]):
        if d0 <= fd <= d1:
            fw = _lerp(w0, w1, (fd - d0) / (d1 - d0)) if d1 > d0 else w0
            break
    return (sx * half_w * fw, y_top - P.u(0.030) - depth * fd, z + drift * fd)


def part_ribcage(rig: Rig, P: Profile) -> None:
    """18 对肋 + 7 节胸骨。马肋是**扁刀片**（前后宽、内外薄），不是圆棍。

    前 8 对（T1-T8）为真肋，经肋软骨汇入胸骨；后 10 对为假肋，末端游离成肋弓。
    """
    z_tmid = (P.z_t1 + P.z_t18) / 2
    z_stern_back = P.z_t1 + 0.30 * P.L
    z_stern_front = P.z_t1 - 0.02 * P.L
    # 肋的前后宽必须 **小于肋间距**，否则 18 根肋在腹侧互相吃掉、糊成一整块平板
    # （首版 rx=0.024W → 宽 1.19 而肋间距只有 0.82，胸廓下半读不出一根肋）。
    # 真马肋宽约 3.5 cm、肋间距约 5 cm，这里按同一比例从节距反推。
    seg_t = (P.z_t18 - P.z_t1) / 17.0
    rx, rz = seg_t * 0.32 * P.bone_gauge, P.r(0.011)

    for i in range(18):
        t = i / 17  # 0 = T18（后），1 = T1（前）
        z = _lerp(P.z_t18, P.z_t1, t)
        bone = "thorax_back" if z > z_tmid else "thorax_front"
        for sx, side in ((-1, "l"), (1, "r")):
            pts = [rib_surface_point(P, z, fd, sx) for _fw, fd in RIB_ARC]
            for seg, (p0, p1) in enumerate(zip(pts[:-1], pts[1:]), start=1):
                _shaft(rig, bone, f"rib_{18 - i}_{side}_{seg}", p0, p1, rx, rz)
            if i >= 10:  # 真肋 T1-T8：肋软骨汇入胸骨
                sz = _lerp(z_stern_back, z_stern_front, (i - 10) / 7)
                _shaft(
                    rig,
                    bone,
                    f"costal_{18 - i}_{side}",
                    pts[-1],
                    (sx * P.u(0.030), P.y_chest + P.u(0.020), sz),
                    P.r(0.014),
                    mat="cartilage",
                )

    # 胸骨 7 节：马胸骨是一条窄龙骨，前端胸骨柄（胸前那个尖）略抬且外露
    for k in range(7):
        tk = k / 6
        z = _lerp(z_stern_back, z_stern_front, tk)
        y = P.y_chest + P.u(0.020) + P.u(0.030) * tk * tk
        rig.cube(
            "thorax_front" if z < z_tmid else "thorax_back",
            f"sternebra_{k + 1}",
            (-P.u(0.030), y - P.u(0.026), z - abs(z_stern_back - z_stern_front) / 14),
            (P.u(0.030), y + P.u(0.026), z + abs(z_stern_back - z_stern_front) / 14),
            mat="bone_dark",
        )
    rig.cube(
        "thorax_front",
        "manubrium",
        (-P.u(0.024), P.y_chest + P.u(0.040), z_stern_front - P.u(0.070)),
        (P.u(0.024), P.y_chest + P.u(0.095), z_stern_front + P.u(0.010)),
        rot=(-18.0, 0.0, 0.0),
        org=(0.0, P.y_chest + P.u(0.060), z_stern_front),
    )


# ================================================================ 部件：颅骨
class HeadSpace:
    """头部局部坐标系 → 世界。

    马头相对颈轴下俯 50-60°，若直接写世界绝对坐标，每一块都得手算旋转后的位置，
    改一次比例就全废。这里把颅骨写在「枕髁为原点、-z 朝吻端、y 向上」的局部系里，
    统一绕 x 轴俯仰后再落到世界。

    数学：box 中心按 Rx(pitch) 搬到世界，再让 box 绕**自身中心**转 pitch+dp。
    合起来 = 先在局部绕自身中心转 dp，再整体过头部变换——刚体，端点不飘。
    只允许绕 x 的局部旋转（dp）：rig.cube 的旋转序是 Rz·Ry·Rx，只有同轴才可叠加。
    """

    def __init__(self, rig: Rig, bone: str, apex: Vec, pitch: float) -> None:
        self.rig, self.bone, self.apex, self.pitch = rig, bone, apex, pitch
        a = math.radians(pitch)
        self.c, self.s = math.cos(a), math.sin(a)

    def to_world(self, p: Vec) -> Vec:
        x, y, z = p
        return (
            self.apex[0] + x,
            self.apex[1] + self.c * y - self.s * z,
            self.apex[2] + self.s * y + self.c * z,
        )

    def box(self, name: str, lo: Vec, hi: Vec, *, mat: str = "bone", dp: float = 0.0, bone: str | None = None) -> None:
        c = tuple((a + b) / 2 for a, b in zip(lo, hi))
        half = tuple(abs(a - b) / 2 for a, b in zip(lo, hi))
        wc = self.to_world(c)
        self.rig.cube(
            bone or self.bone,
            name,
            tuple(w - h for w, h in zip(wc, half)),
            tuple(w + h for w, h in zip(wc, half)),
            rot=(self.pitch + dp, 0.0, 0.0),
            org=wc,
            mat=mat,
        )


HEAD_PITCH = -54.0  # 头相对水平的下俯角（站立自然姿）


def head_space(rig: Rig, P: Profile, bone: str = "skull") -> HeadSpace:
    return HeadSpace(rig, bone, (0.0, P.y_occiput, P.z_occiput), HEAD_PITCH)


def part_skull(rig: Rig, P: Profile) -> None:
    """颅骨：脑颅 + 闭合眶环 + 颧弓/面嵴 + 长面 + 高冠颊齿 + 齿隙 + 门齿。

    与猫科最刺眼的三点差别，这里都刻意做足：
      ① **眶环闭合**——眶后桥把眼眶围成一整圈（猫科是开放的）；
      ② **面部极长**——颊齿列到门齿之间隔着一整段齿隙；
      ③ **面嵴**——自眶下向前的一道横棱，活马脸上看得见。
    """
    rig.bone("skull", (0.0, P.y_occiput, P.z_occiput), parent=NECK[-1])
    hs = head_space(rig, P)
    h = P.h  # 头长比例 → 绝对

    # 咬合面固定在 y=-0.198H：上下齿列共用这一条线。首版上齿到 -0.205、下齿顶到
    # -0.055，两列在颌内穿模了 0.15H —— 侧视看不出来，从下方一看就是一嘴烂牙。
    occl = -0.198

    # --- 脑颅：短、窄，全挤在枕端。面部另起，两段之间靠额骨过渡 ---
    hs.box("cranium", (-h(0.098), -h(0.060), -h(0.300)), (h(0.098), h(0.118), h(0.020)))
    hs.box("nuchal_crest", (-h(0.072), h(0.104), -h(0.075)), (h(0.072), h(0.156), h(0.036)))
    hs.box("occipital_condyle", (-h(0.070), -h(0.075), -h(0.015)), (h(0.070), h(0.030), h(0.058)), mat="cartilage")
    # 额骨只占两眼之间的中线（半宽 < 眶内缘 0.080H）——否则侧视投影把眶孔填死
    hs.box("frontal", (-h(0.078), h(0.070), -h(0.470)), (h(0.078), h(0.126), -h(0.285)))

    for sx, side in ((-1, "l"), (1, "r")):
        # --- 眶环：马的眼眶是**闭合**的一整圈骨（眶后桥），猫科则是开放的。
        #     四根围骨都比面部外凸 0.05H，中间的 socket 凹进去 —— 体素没有布尔减法，
        #     "洞"只能靠一圈凸出的环 + 一块凹进去的深色面读出来。
        hs.box(f"supraorbital_{side}", (sx * h(0.078), h(0.082), -h(0.452)), (sx * h(0.142), h(0.138), -h(0.258)))
        hs.box(f"infraorbital_{side}", (sx * h(0.078), -h(0.042), -h(0.440)), (sx * h(0.140), h(0.010), -h(0.268)), mat="bone_dark")
        hs.box(f"lacrimal_{side}", (sx * h(0.078), -h(0.010), -h(0.468)), (sx * h(0.140), h(0.098), -h(0.420)))
        hs.box(f"postorbital_bar_{side}", (sx * h(0.078), -h(0.010), -h(0.296)), (sx * h(0.140), h(0.098), -h(0.250)))
        hs.box(f"orbit_socket_{side}", (sx * h(0.070), -h(0.008), -h(0.428)), (sx * h(0.130), h(0.100), -h(0.284)), mat="socket")
        # --- 颞窝：眶后到脑颅之间那块凹陷（咬肌通道），给侧廓再添一层深度 ---
        hs.box(f"temporal_fossa_{side}", (sx * h(0.082), h(0.010), -h(0.246)), (sx * h(0.104), h(0.086), -h(0.130)), mat="socket")
        # --- 颧弓：自眶下缘向后到颞区，下颌髁挂在它后下方 ---
        # 颧弓留亮色：与面嵴同用 bone_dark 会连成一大片深色，面嵴那条棱就没了
        hs.box(f"zygomatic_{side}", (sx * h(0.092), -h(0.062), -h(0.282)), (sx * h(0.136), -h(0.004), -h(0.088)))
        # --- 面嵴：自眶下向前的一道横棱。活马脸上看得见的那条明棱，马的招牌 ---
        hs.box(f"facial_crest_{side}", (sx * h(0.080), -h(0.086), -h(0.605)), (sx * h(0.120), -h(0.036), -h(0.284)), mat="bone_dark")
        hs.box(f"ear_canal_{side}", (sx * h(0.076), h(0.028), -h(0.126)), (sx * h(0.100), h(0.088), -h(0.062)), mat="socket")

    # --- 面部：鼻背近直线（马脸的直侧廓），齿槽段深、齿隙段浅，向吻端收窄 ---
    # 鼻背三段接成一条近直线（马脸的直侧廓）：额 0.126 → 鼻后 0.110 → 鼻前 0.082，
    # 每级只落 0.02H 上下；台阶大了就成了凹面的阿拉伯马脸。
    hs.box("nasal_back", (-h(0.052), h(0.052), -h(0.640)), (h(0.052), h(0.110), -h(0.286)))
    hs.box("nasal_front", (-h(0.046), h(0.016), -h(0.935)), (h(0.046), h(0.082), -h(0.632)))
    # 齿槽部（颊齿藏在里面，向下鼓出一整块 —— 高冠齿把上颌撑深了）
    hs.box("maxilla_alveolar", (-h(0.082), h(occl) + h(0.024), -h(0.612)), (h(0.082), h(0.062), -h(0.292)))
    # 齿隙段：门齿到颊齿之间那一长条，浅而窄 —— 马的解剖签名，别填实。
    # 上下颌在这一段之间必须留出看得见的空档（衔铁位），否则整张脸糊成一块板。
    hs.box("maxilla_diastema", (-h(0.058), -h(0.128), -h(0.858)), (h(0.058), h(0.030), -h(0.606)))
    hs.box("incisive", (-h(0.062), -h(0.196), -h(1.000)), (h(0.062), h(0.026), -h(0.850)))
    hs.box("nasal_aperture", (-h(0.042), -h(0.006), -h(0.988)), (h(0.042), h(0.058), -h(0.800)), mat="socket")
    # 硬腭很窄，夹在两条齿列之间（首版一整块宽板把齿列盖死了）
    hs.box("palate", (-h(0.036), h(occl) - h(0.004), -h(0.955)), (h(0.036), h(occl) + h(0.028), -h(0.300)), mat="bone_dark")

    # --- 上齿列：高冠颊齿一整条深齿块（冠面探出齿槽 0.024H，侧视是一条浅色齿线）---
    for sx, side in ((-1, "l"), (1, "r")):
        hs.box(
            f"cheekteeth_up_{side}",
            (sx * h(0.040), h(occl), -h(0.598)),
            (sx * h(0.080), -h(0.062), -h(0.296)),
            mat="tooth",
        )
    hs.box("incisors_up", (-h(0.056), h(occl) - h(0.058), -h(1.006)), (h(0.056), -h(0.182), -h(0.902)), mat="tooth", dp=-16.0)


def part_jaw(rig: Rig, P: Profile) -> None:
    """下颌：深大的升支（腮）+ 长水平支 + 高冠下颊齿 + 下门齿。铰链在颧弓后下的髁突。"""
    hs0 = head_space(rig, P)
    h = P.h
    hinge_local = (0.0, h(0.020), -h(0.190))
    rig.bone("jaw", hs0.to_world(hinge_local), parent="skull")
    hs = HeadSpace(rig, "jaw", (0.0, P.y_occiput, P.z_occiput), HEAD_PITCH)

    occl = -0.198  # 与 part_skull 同一条咬合面
    for sx, side in ((-1, "l"), (1, "r")):
        # 升支：马的腮是一整片宽厚骨板（比猫科深大得多）。板面本身**内缩**到 0.126H，
        # 只留后缘和下缘（下颌角）凸到 0.146H —— 中间那块凹陷是咬肌窝，活马腮上那个坑。
        # 整片写成一个齐平的厚板，侧视就是一块没有起伏的白墙。
        hs.box(f"mandible_ramus_{side}", (sx * h(0.078), -h(0.335), -h(0.256)), (sx * h(0.126), h(0.035), -h(0.070)))
        hs.box(f"ramus_border_{side}", (sx * h(0.078), -h(0.330), -h(0.124)), (sx * h(0.146), h(0.020), -h(0.070)))
        # 咬肌窝是**浅凹**不是洞：用 socket 画满整片腮，侧视会读成第二个眼窝，
        # 把真正的眶孔压成配角。改成 bone_dark 的一小块阴影，只暗示凹陷。
        hs.box(f"masseteric_fossa_{side}", (sx * h(0.118), -h(0.258), -h(0.226)), (sx * h(0.130), -h(0.072), -h(0.146)), mat="bone_dark")
        hs.box(f"mandible_angle_{side}", (sx * h(0.080), -h(0.380), -h(0.240)), (sx * h(0.146), -h(0.305), -h(0.100)), mat="bone_dark")
        hs.box(f"condyle_{side}", (sx * h(0.082), h(0.004), -h(0.222)), (sx * h(0.134), h(0.052), -h(0.156)), mat="cartilage")
        hs.box(f"coronoid_{side}", (sx * h(0.090), h(0.030), -h(0.246)), (sx * h(0.120), h(0.150), -h(0.184)), mat="bone_dark", dp=-10.0)
        # 水平支：自下颌角向前，逐渐收窄并向中线靠拢；齿槽段深，齿隙段细
        hs.box(f"mandible_body_{side}", (sx * h(0.048), -h(0.340), -h(0.618)), (sx * h(0.110), h(occl) - h(0.012), -h(0.250)))
        hs.box(f"mandible_front_{side}", (sx * h(0.036), -h(0.310), -h(0.915)), (sx * h(0.074), -h(0.238), -h(0.612)))
        hs.box(
            f"cheekteeth_low_{side}",
            (sx * h(0.046), -h(0.318), -h(0.600)),
            (sx * h(0.090), h(occl), -h(0.300)),
            mat="tooth",
        )
    hs.box("mandible_symphysis", (-h(0.056), -h(0.330), -h(1.002)), (h(0.056), -h(0.262), -h(0.900)))
    hs.box("incisors_low", (-h(0.050), -h(0.320), -h(0.998)), (h(0.050), -h(0.252), -h(0.905)), mat="tooth", dp=-16.0)


# ================================================================ 部件：前肢
def part_foreleg(rig: Rig, P: Profile, sx: int, side: str) -> None:
    """肩胛（+肩胛软骨）→ 肱骨 → 桡尺（鹰嘴突）→ 腕 7 骨 → 管骨+夹板骨 →
    球节（籽骨）→ 系骨 → 冠骨 → 蹄骨 + 蹄匣。

    **蹄行**：腕关节高悬在 0.28 体高处，之下只有一根管骨和三节指骨着地。
    马没有锁骨，肩胛靠肌肉悬吊在胸廓外侧，所以肩胛不与任何骨相接。
    """
    x = sx * P.x_fore
    sc_top = (sx * P.barrel_w * 0.52, P.dorsal_y(P.z_t1 + P.L * 0.09) - P.u(0.055), P.z_t1 + P.L * 0.095)
    shoulder = (x, P.y_shoulder, P.z_shoulder_j)
    elbow = (x, P.y_elbow, P.z_elbow)
    carpus = (x, P.y_carpus, P.z_carpus)
    fetlock = (x, P.y_f_fetlock, P.z_f_fetlock)
    # 系/冠：一路 ~52° 前倾插进蹄匣，这条蹄-系轴线是马下肢的形状标志
    pastern = (x, fetlock[1] - P.u(0.052), fetlock[2] - P.u(0.042))
    coffin = (x, fetlock[1] - P.u(0.075), fetlock[2] - P.u(0.061))

    sc = rig.bone(f"scapula_{side}", sc_top, parent="thorax_front")
    hu = rig.bone(f"humerus_{side}", shoulder, parent=sc)
    ra = rig.bone(f"radius_{side}", elbow, parent=hu)
    cp = rig.bone(f"carpus_{side}", carpus, parent=ra)
    fk = rig.bone(f"fetlock_f_{side}", fetlock, parent=cp)
    rig.bone(f"hoof_f_{side}", coffin, parent=fk)

    # 肩胛：长扁片，斜 50° 贴在胸廓外侧（rz 大 = 前后宽，rx 小 = 薄）。
    # 首版 rz=0.072W 折合 22 cm 宽的板，从任何角度都压住整个前躯；真马肩胛宽约 18 cm。
    _shaft(rig, sc, f"scapula_blade_{side}", sc_top, shoulder, P.r(0.015), P.r(0.052))
    _shaft(
        rig,
        sc,
        f"scapula_spine_{side}",
        (sc_top[0] + sx * P.u(0.022), sc_top[1], sc_top[2] + P.u(0.012)),
        (shoulder[0] + sx * P.u(0.018), shoulder[1] + P.u(0.045), shoulder[2] + P.u(0.012)),
        P.r(0.014),
        P.r(0.026),
        mat="bone_dark",
    )
    # 肩胛软骨：骨性上缘之上还接一片软骨，鬐甲的高度有它一份
    _shaft(
        rig,
        sc,
        f"scapula_cartilage_{side}",
        sc_top,
        (sc_top[0] - sx * P.u(0.010), sc_top[1] + P.u(0.042), sc_top[2] + P.u(0.008)),
        P.r(0.012),
        P.r(0.042),
        mat="cartilage",
    )
    rig.cube(
        sc,
        f"glenoid_{side}",
        (shoulder[0] - sx * P.u(0.030), shoulder[1] - P.u(0.028), shoulder[2] - P.u(0.030)),
        (shoulder[0] + sx * P.u(0.030), shoulder[1] + P.u(0.028), shoulder[2] + P.u(0.030)),
        mat="cartilage",
    )

    # 肱骨：短而近水平（奔走类的典型），自肩端向后下走到肘
    _shaft(rig, hu, f"humerus_{side}_shaft", shoulder, elbow, P.r(0.030))
    rig.cube(
        hu,
        f"humerus_{side}_condyle",
        (elbow[0] - sx * P.u(0.034), elbow[1] - P.u(0.030), elbow[2] - P.u(0.034)),
        (elbow[0] + sx * P.u(0.034), elbow[1] + P.u(0.030), elbow[2] + P.u(0.034)),
        mat="cartilage",
    )

    # 桡骨（主承重，近垂直）+ 尺骨（仅上半，末端与桡骨融合）
    _shaft(rig, ra, f"radius_{side}_shaft", elbow, carpus, P.r(0.026), P.r(0.020))
    _shaft(
        rig,
        ra,
        f"ulna_{side}_shaft",
        (elbow[0], elbow[1] - P.u(0.010), elbow[2] + P.u(0.028)),
        (_lerp(elbow[0], carpus[0], 0.55), _lerp(elbow[1], carpus[1], 0.55), _lerp(elbow[2], carpus[2], 0.55) + P.u(0.016)),
        P.r(0.013),
        mat="bone_dark",
    )
    # 鹰嘴突（肘尖）：向后上突出，三头肌的力臂
    _shaft(
        rig,
        ra,
        f"olecranon_{side}",
        elbow,
        (elbow[0], elbow[1] + P.u(0.060), elbow[2] + P.u(0.060)),
        P.r(0.022),
        mat="bone_dark",
    )

    # 腕：两排小骨（马「膝」），实际是腕不是膝
    for k, dy in ((0, 0.020), (1, -0.020)):
        rig.cube(
            cp,
            f"carpal_row{k + 1}_{side}",
            (carpus[0] - sx * P.u(0.032), carpus[1] + P.u(dy) - P.u(0.019), carpus[2] - P.u(0.034)),
            (carpus[0] + sx * P.u(0.032), carpus[1] + P.u(dy) + P.u(0.019), carpus[2] + P.u(0.030)),
            mat="cartilage",
        )
    # 副腕骨（腕后那块突出的小骨）
    rig.cube(
        cp,
        f"accessory_carpal_{side}",
        (carpus[0] - sx * P.u(0.024), carpus[1] - P.u(0.008), carpus[2] + P.u(0.030)),
        (carpus[0] + sx * P.u(0.024), carpus[1] + P.u(0.030), carpus[2] + P.u(0.062)),
        mat="bone_dark",
    )

    _cannon(rig, P, cp, fk, side, carpus, fetlock, pastern, coffin, sx, "f")


def _cannon(
    rig: Rig,
    P: Profile,
    bone_cannon: str,
    bone_fetlock: str,
    side: str,
    top: Vec,
    fetlock: Vec,
    pastern: Vec,
    coffin: Vec,
    sx: int,
    tag: str,
) -> None:
    """管骨 + 夹板骨 + 籽骨 + 系/冠/蹄骨 + 蹄匣。前后肢共用（后肢管骨略长略窄）。

    夹板骨（退化的掌/跖骨 II、IV）是马属的解剖签名——只剩两根细钉贴在管骨后侧，
    上端粗、下端在管骨三分之二处收成尖。少了它，下肢就是根光棍。
    """
    hoof_bone = f"hoof_{tag}_{side}"
    r_cannon = P.r(0.024) if tag == "f" else P.r(0.022)
    _shaft(rig, bone_cannon, f"cannon_{tag}_{side}", top, fetlock, r_cannon, r_cannon * 0.80)
    # 夹板骨：贴管骨后侧内外各一，长度约管骨的 2/3。
    # 偏移必须乘 sx（splint1 = 内侧、splint2 = 外侧，两侧同义），否则左右同名件不镜像——
    # 首版写死 ±1，三视图上完全看不出来，是 check() 的镜像断言抓出来的。
    for k, m in ((0, -1), (1, 1)):
        dx = sx * m
        base = (top[0] + dx * P.u(0.026), top[1] - P.u(0.010), top[2] + P.u(0.020))
        tip = (
            _lerp(top[0], fetlock[0], 0.66) + dx * P.u(0.020),
            _lerp(top[1], fetlock[1], 0.66),
            _lerp(top[2], fetlock[2], 0.66) + P.u(0.016),
        )
        _shaft(rig, bone_cannon, f"splint{k + 1}_{tag}_{side}", base, tip, P.r(0.008), mat="bone_dark")

    # 球节 + 近籽骨（球节后侧那对小骨，是屈腱的滑车）
    rig.cube(
        bone_fetlock,
        f"fetlock_joint_{tag}_{side}",
        (fetlock[0] - sx * P.u(0.030), fetlock[1] - P.u(0.024), fetlock[2] - P.u(0.028)),
        (fetlock[0] + sx * P.u(0.030), fetlock[1] + P.u(0.024), fetlock[2] + P.u(0.026)),
        mat="cartilage",
    )
    rig.cube(
        bone_fetlock,
        f"sesamoid_{tag}_{side}",
        (fetlock[0] - sx * P.u(0.026), fetlock[1] - P.u(0.014), fetlock[2] + P.u(0.024)),
        (fetlock[0] + sx * P.u(0.026), fetlock[1] + P.u(0.026), fetlock[2] + P.u(0.052)),
        mat="bone_dark",
    )

    # 系骨（P1）→ 冠骨（P2）→ 蹄骨（P3）
    _shaft(rig, bone_fetlock, f"pastern_long_{tag}_{side}", fetlock, pastern, P.r(0.022), P.r(0.019))
    _shaft(rig, bone_fetlock, f"pastern_short_{tag}_{side}", pastern, coffin, P.r(0.023), P.r(0.020))
    # 蹄骨：楔形，前缘随蹄壁前倾，藏在蹄匣里
    rig.cube(
        hoof_bone,
        f"coffin_bone_{tag}_{side}",
        (coffin[0] - sx * P.hoof_r * 0.66, P.u(0.012), coffin[2] - P.hoof_r * 0.80),
        (coffin[0] + sx * P.hoof_r * 0.66, coffin[1] + P.u(0.006), coffin[2] + P.hoof_r * 0.30),
        rot=(8.0, 0.0, 0.0),
        org=(coffin[0], coffin[1], coffin[2]),
    )
    # 舟骨（蹄骨后上方那块小骨）
    rig.cube(
        hoof_bone,
        f"navicular_{tag}_{side}",
        (coffin[0] - sx * P.hoof_r * 0.42, coffin[1] - P.u(0.020), coffin[2] + P.hoof_r * 0.20),
        (coffin[0] + sx * P.hoof_r * 0.42, coffin[1] + P.u(0.002), coffin[2] + P.hoof_r * 0.52),
        mat="cartilage",
    )
    # 蹄匣：**蹄尖高、蹄踵低、下缘外张**，底面落在 y=0 —— 全身唯一的着地面。
    # 首版是块 22 cm 见方六面等高的砖（真马前蹄宽约 13 cm），读起来像穿了木屐。
    r = P.hoof_r
    wall_top = P.u(0.058) if tag == "f" else P.u(0.054)
    rig.cube(  # 下段：外张的承重环，直接落地
        hoof_bone,
        f"hoof_wall_{tag}_{side}",
        (coffin[0] - sx * r, 0.0, coffin[2] - r * 1.05),
        (coffin[0] + sx * r, wall_top * 0.55, coffin[2] + r * 0.78),
        mat="hoof",
    )
    rig.cube(  # 上段：只在蹄尖/蹄侧继续拔高，蹄踵留低 —— 侧视那道斜蹄壁靠这一级读出来
        hoof_bone,
        f"hoof_toe_{tag}_{side}",
        (coffin[0] - sx * r * 0.84, wall_top * 0.55, coffin[2] - r * 1.00),
        (coffin[0] + sx * r * 0.84, wall_top, coffin[2] + r * 0.10),
        mat="hoof",
    )
    # 蹄冠带（蹄壁上缘那圈，浅一档，把蹄匣和上面的骨分开读）
    rig.cube(
        hoof_bone,
        f"coronet_{tag}_{side}",
        (coffin[0] - sx * r * 0.88, wall_top, coffin[2] - r * 0.95),
        (coffin[0] + sx * r * 0.88, wall_top + P.u(0.013), coffin[2] + r * 0.22),
        mat="cartilage",
    )


# ================================================================ 部件：后肢
def part_pelvis(rig: Rig, P: Profile, sx: int, side: str) -> None:
    """髂（髋结节 + 荐结节侧翼）· 坐（坐骨结节）· 耻。马的髋结节顶得出皮，是外形骨点。"""
    hip = (sx * P.x_hind, P.y_hip, P.z_hip)
    # 髂骨翼：自髋臼向前上外张开到髋结节
    tuber_coxae = (sx * P.u(0.165), P.y_croup - P.u(0.055), P.z_hip - P.L * 0.100)
    # 髂骨翼是一片宽板（不是根棍），髋结节到荐结节之间整块骨面撑起尻部轮廓
    _shaft(rig, "hips", f"ilium_{side}", hip, tuber_coxae, P.r(0.024), P.r(0.058))
    rig.cube(
        "hips",
        f"tuber_coxae_{side}",
        (tuber_coxae[0] - sx * P.u(0.038), tuber_coxae[1] - P.u(0.030), tuber_coxae[2] - P.u(0.040)),
        (tuber_coxae[0] + sx * P.u(0.038), tuber_coxae[1] + P.u(0.030), tuber_coxae[2] + P.u(0.040)),
        mat="bone_dark",
    )
    # 髂骨内板：髋臼 → 荐骨（骨盆与脊柱唯一的骨性连接）
    _shaft(
        rig,
        "hips",
        f"ilium_body_{side}",
        hip,
        (sx * P.u(0.045), P.y_croup - P.u(0.055), P.z_sacrum - P.L * 0.085),
        P.r(0.022),
        P.r(0.060),
        mat="bone_dark",
    )
    # 坐骨：自髋臼向后，末端坐骨结节 = 臀端
    ischium_end = (sx * P.u(0.100), P.y_hip - P.u(0.055), P.z_ischium)
    _shaft(rig, "hips", f"ischium_{side}", hip, ischium_end, P.r(0.024), P.r(0.038), mat="bone_dark")
    rig.cube(
        "hips",
        f"tuber_ischii_{side}",
        (ischium_end[0] - sx * P.u(0.034), ischium_end[1] - P.u(0.030), ischium_end[2] - P.u(0.032)),
        (ischium_end[0] + sx * P.u(0.034), ischium_end[1] + P.u(0.030), ischium_end[2] + P.u(0.020)),
        mat="bone_dark",
    )
    # 耻骨：腹侧闭合骨盆环
    _shaft(
        rig,
        "hips",
        f"pubis_{side}",
        (hip[0] - sx * P.u(0.020), hip[1] - P.u(0.070), hip[2] - P.u(0.010)),
        (sx * P.u(0.024), hip[1] - P.u(0.085), P.z_ischium - P.u(0.060)),
        P.r(0.018),
        mat="bone_dark",
    )
    rig.cube(
        "hips",
        f"acetabulum_{side}",
        (hip[0] - sx * P.u(0.034), hip[1] - P.u(0.032), hip[2] - P.u(0.034)),
        (hip[0] + sx * P.u(0.034), hip[1] + P.u(0.032), hip[2] + P.u(0.034)),
        mat="cartilage",
    )


def part_hindleg(rig: Rig, P: Profile, sx: int, side: str) -> None:
    """股骨（大转子 + **第三转子**）→ 髌 → 胫腓 → 跗（跟结节）→ 管骨 → 球节 → 蹄。

    马属签名：股骨外侧的第三转子（臀中肌止点）—— 别的现生有蹄类都没有。
    后肢的 Z 字（膝在前、跗在后）比前肢陡得多，蹬地功率全从这条折线来。
    """
    x = sx * P.x_hind
    hip = (x, P.y_hip, P.z_hip)
    stifle = (x + sx * P.u(0.012), P.y_stifle, P.z_stifle)
    hock = (x, P.y_hock, P.z_hock)
    h_fetlock = (x, P.y_h_fetlock, P.z_h_fetlock)
    pastern = (x, h_fetlock[1] - P.u(0.052), h_fetlock[2] - P.u(0.040))
    coffin = (x, h_fetlock[1] - P.u(0.080), h_fetlock[2] - P.u(0.060))

    fe = rig.bone(f"femur_{side}", hip, parent="hips")
    ti = rig.bone(f"tibia_{side}", stifle, parent=fe)
    ta = rig.bone(f"tarsus_{side}", hock, parent=ti)
    fk = rig.bone(f"fetlock_h_{side}", h_fetlock, parent=ta)
    rig.bone(f"hoof_h_{side}", coffin, parent=fk)

    # 股骨：马身上最粗的长骨
    _shaft(rig, fe, f"femur_{side}_shaft", hip, stifle, P.r(0.038))
    _shaft(
        rig,
        fe,
        f"greater_trochanter_{side}",
        hip,
        (hip[0] + sx * P.u(0.040), hip[1] + P.u(0.062), hip[2] + P.u(0.014)),
        P.r(0.028),
        mat="bone_dark",
    )
    # 第三转子：股骨干上三分之一处向外侧伸出的骨翼 —— 马属独有
    third_t = (
        _lerp(hip[0], stifle[0], 0.34),
        _lerp(hip[1], stifle[1], 0.34),
        _lerp(hip[2], stifle[2], 0.34),
    )
    rig.cube(
        fe,
        f"third_trochanter_{side}",
        (third_t[0], third_t[1] - P.u(0.030), third_t[2] - P.u(0.024)),
        (third_t[0] + sx * P.u(0.062), third_t[1] + P.u(0.030), third_t[2] + P.u(0.018)),
        rot=(0.0, 0.0, sx * -18.0),
        org=third_t,
        mat="bone_dark",
    )
    rig.cube(
        fe,
        f"femur_{side}_condyle",
        (stifle[0] - sx * P.u(0.038), stifle[1] - P.u(0.034), stifle[2] - P.u(0.038)),
        (stifle[0] + sx * P.u(0.038), stifle[1] + P.u(0.034), stifle[2] + P.u(0.038)),
        mat="cartilage",
    )
    rig.cube(
        fe,
        f"patella_{side}",
        (stifle[0] - sx * P.u(0.024), stifle[1] + P.u(0.004), stifle[2] - P.u(0.078)),
        (stifle[0] + sx * P.u(0.024), stifle[1] + P.u(0.056), stifle[2] - P.u(0.036)),
        mat="cartilage",
    )

    # 胫骨（主）+ 腓骨（只剩上半截，向下收成尖 —— 马的腓骨退化）
    _shaft(rig, ti, f"tibia_{side}_shaft", stifle, hock, P.r(0.032))
    _shaft(
        rig,
        ti,
        f"fibula_{side}",
        (stifle[0] + sx * P.u(0.030), stifle[1] - P.u(0.020), stifle[2] + P.u(0.012)),
        (
            _lerp(stifle[0], hock[0], 0.45) + sx * P.u(0.018),
            _lerp(stifle[1], hock[1], 0.45),
            _lerp(stifle[2], hock[2], 0.45),
        ),
        P.r(0.010),
        mat="bone_dark",
    )

    # 跗骨群 + 跟结节（点 of hock）：向后上突出，跟腱的力臂
    rig.cube(
        ta,
        f"tarsal_bones_{side}",
        (hock[0] - sx * P.u(0.032), hock[1] - P.u(0.036), hock[2] - P.u(0.038)),
        (hock[0] + sx * P.u(0.032), hock[1] + P.u(0.034), hock[2] + P.u(0.028)),
        mat="cartilage",
    )
    _shaft(
        rig,
        ta,
        f"calcaneus_{side}",
        hock,
        (hock[0], hock[1] + P.u(0.078), hock[2] + P.u(0.052)),
        P.r(0.024),
        mat="bone_dark",
    )

    _cannon(rig, P, ta, fk, side, hock, h_fetlock, pastern, coffin, sx, "h")


# ================================================================ 装配
def _root(rig: Rig) -> None:
    rig.bone("root", (0.0, 0.0, 0.0))


def _spine_stub(rig: Rig, P: Profile) -> None:
    """单件预览用：只建父骨骼链，不铺全脊柱。"""
    rig.bone("hips", (0.0, P.centrum_y(P.z_sacrum), P.z_sacrum - P.u(0.03)), parent="root")
    rig.bone("lumbar", (0.0, P.centrum_y(P.z_l6), P.z_l6), parent="hips")
    z_tmid = (P.z_t1 + P.z_t18) / 2
    rig.bone("thorax_back", (0.0, P.centrum_y(P.z_t18), P.z_t18), parent="lumbar")
    rig.bone("thorax_front", (0.0, P.centrum_y(z_tmid), z_tmid), parent="thorax_back")
    _neck_bones(rig, P)


def _both(fn):
    def run(r: Rig, P: Profile) -> None:
        for sx, side in ((-1, "l"), (1, "r")):
            fn(r, P, sx, side)

    return run


PARTS = {
    "skull": ("颅骨", lambda r, P: (_root(r), _spine_stub(r, P), part_skull(r, P))),
    "jaw": ("头（含下颌）", lambda r, P: (_root(r), _spine_stub(r, P), part_skull(r, P), part_jaw(r, P))),
    "spine": ("脊柱", lambda r, P: (_root(r), part_spine(r, P))),
    "tail": ("尾椎", lambda r, P: (_root(r), part_spine(r, P), part_tail(r, P))),
    "ribcage": ("胸廓", lambda r, P: (_root(r), part_spine(r, P), part_ribcage(r, P))),
    "foreleg": ("前肢", lambda r, P: (_root(r), part_spine(r, P), _both(part_foreleg)(r, P))),
    "pelvis": ("骨盆", lambda r, P: (_root(r), part_spine(r, P), _both(part_pelvis)(r, P))),
    "hindleg": (
        "后肢",
        lambda r, P: (_root(r), part_spine(r, P), _both(part_pelvis)(r, P), _both(part_hindleg)(r, P)),
    ),
}


def build_full(P: Profile) -> Rig:
    rig = Rig()
    _root(rig)
    part_spine(rig, P)
    part_ribcage(rig, P)
    part_tail(rig, P)
    part_skull(rig, P)
    part_jaw(rig, P)
    for sx, side in ((-1, "l"), (1, "r")):
        part_pelvis(rig, P, sx, side)
        part_foreleg(rig, P, sx, side)
        part_hindleg(rig, P, sx, side)
    return rig


# ================================================================ 自检
def _obb(e: dict):
    """element → (中心, 半长, 3×3 旋转)。旋转按 Blockbench/render_bbmodel 的 R=Rz·Ry·Rx 绕 origin 施加。"""
    import numpy as np

    def rm(deg: float, axis: int):
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
    half = (t - f) / 2
    center = R @ ((f + t) / 2 - org) + org
    return center, half, R


def connected_components(rig: Rig, tol: float = 0.02) -> list[list[str]]:
    """把每个 element 当有向包围盒，用分离轴定理判相接，返回连通分量（按件数降序）。

    骨架必须是**单一连通体**。悬空件在三视图和 Blockbench 里都极难发现——首版
    椎体前后长短于节距，整条脊柱在每个椎间断开，后躯（骨盆+后肢+尾）整块与前躯
    分离成孤岛，靠目视一直没看出来，是本函数抓的。
    """
    import numpy as np

    els = rig.elements
    boxes = [_obb(e) for e in els]
    aabb = [
        (c - np.array([sum(h[i] * abs(R[k, i]) for i in range(3)) for k in range(3)]),
         c + np.array([sum(h[i] * abs(R[k, i]) for i in range(3)) for k in range(3)]))
        for c, h, R in boxes
    ]

    def overlap(a, b) -> bool:
        ca, ha, Ra = a
        cb, hb, Rb = b
        d = cb - ca
        axes = [Ra[:, i] for i in range(3)] + [Rb[:, i] for i in range(3)]
        for i in range(3):
            for j in range(3):
                cr = np.cross(Ra[:, i], Rb[:, j])
                n = float(np.linalg.norm(cr))
                if n > 1e-8:
                    axes.append(cr / n)
        for ax in axes:
            ra = sum(ha[i] * abs(float(ax @ Ra[:, i])) for i in range(3))
            rb = sum(hb[i] * abs(float(ax @ Rb[:, i])) for i in range(3))
            if abs(float(ax @ d)) > ra + rb + tol:
                return False
        return True

    parent = list(range(len(els)))

    def find(x: int) -> int:
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for i in range(len(els)):
        lo_i, hi_i = aabb[i]
        for j in range(i + 1, len(els)):
            if find(i) == find(j):
                continue
            lo_j, hi_j = aabb[j]
            if bool(np.any(hi_i + tol < lo_j)) or bool(np.any(hi_j + tol < lo_i)):
                continue
            if overlap(boxes[i], boxes[j]):
                parent[find(i)] = find(j)

    comps: dict[int, list[str]] = {}
    for i, e in enumerate(els):
        comps.setdefault(find(i), []).append(e["name"])
    return sorted(comps.values(), key=len, reverse=True)


def check(P: Profile, rig: Rig, verbose: bool = True) -> int:
    """结构自检：左右镜像 · 四蹄着地 · 鬐甲高对表 · 比例落在马的常识区间。

    目视核不出镜像错位（首版左腿整体平移而非镜像，三视图完全看不出来）。
    """
    els = {e["name"]: e for e in rig.elements}
    problems: list[str] = []

    for name, e in els.items():
        if not (name.endswith("_l") or "_l_" in name):
            continue
        mate = name.replace("_l_", "_r_") if "_l_" in name else name[:-2] + "_r"
        m = els.get(mate)
        if m is None:
            problems.append(f"{name}: 缺镜像件 {mate}")
            continue
        if abs(e["from"][0] + m["to"][0]) > 0.02 or abs(e["to"][0] + m["from"][0]) > 0.02:
            problems.append(f"{name}: x 未镜像（{e['from'][0]}..{e['to'][0]} vs {m['from'][0]}..{m['to'][0]}）")
        for axis, label in ((1, "y"), (2, "z")):
            if abs(e["from"][axis] - m["from"][axis]) > 0.02 or abs(e["to"][axis] - m["to"][axis]) > 0.02:
                problems.append(f"{name}: {label} 与镜像件不等")

    ys = [v for e in rig.elements for v in (e["from"][1], e["to"][1])]
    zs = [v for e in rig.elements for v in (e["from"][2], e["to"][2])]
    xs = [v for e in rig.elements for v in (e["from"][0], e["to"][0])]

    # 四蹄着地：最低点必须是蹄底，且恰好落在 y=0
    if abs(min(ys)) > 0.03:
        problems.append(f"贴地异常：最低点 y={min(ys):.3f}（蹄底应为 0）")
    for tag in ("f", "h"):
        for side in ("l", "r"):
            w = els.get(f"hoof_wall_{tag}_{side}")
            if w is None:
                problems.append(f"缺蹄匣 hoof_wall_{tag}_{side}")
            elif abs(w["from"][1]) > 0.03:
                problems.append(f"hoof_wall_{tag}_{side} 未着地：y={w['from'][1]:.3f}")

    # 骨树自检：每个 element 恰好挂在一根骨骼下、骨骼 uuid 不重、只有一个 root。
    # render_bbmodel.py 只读 elements 不读 outliner，骨树坏了在预览图上完全看不出来，
    # 但 Blockbench 打开会丢件、GeckoLib 导出会掉骨。
    tree_ids: list[str] = []

    def walk(nodes: list) -> None:
        for n in nodes:
            if isinstance(n, str):
                tree_ids.append(n)
            else:
                tree_ids.append(n["uuid"])
                walk(n["children"])

    roots = rig.outliner()
    walk(roots)
    if len(roots) != 1 or roots[0]["name"] != "root":
        problems.append(f"骨树根异常：{[r['name'] for r in roots]}（应只有 root）")
    if len(tree_ids) != len(set(tree_ids)):
        problems.append("骨树里有重复 uuid（同一件被挂了两次）")
    el_ids = {e["uuid"] for e in rig.elements}
    missing = el_ids - set(tree_ids)
    if missing:
        problems.append(f"{len(missing)} 个 element 未挂进骨树（Blockbench 打开会丢件）")

    # 连通性：骨架必须是单一连通体，任何独立岛都是悬空件
    comps = connected_components(rig)
    if len(comps) > 1:
        problems.append(f"骨架裂成 {len(comps)} 块（应为 1 块整体）")
        for c in comps[1:6]:
            problems.append(f"  悬空岛 {len(c)} 件：{', '.join(sorted(c)[:6])}{' …' if len(c) > 6 else ''}")

    wither = max(e["to"][1] for n, e in els.items() if n.startswith("thoracic_sp"))
    if abs(wither - P.wither) > 0.35:
        problems.append(f"鬐甲高 {wither:.2f} 与档位标称 {P.wither:.2f} 不符")

    body = P.z_ischium - P.z_shoulder
    total = max(zs) - min(zs)
    head_top = max(ys)
    ratios = {
        "体长/鬐甲高": body / P.wither,
        "头长/鬐甲高": P.H / P.wither,
        "枕高/鬐甲高": P.y_poll / P.wither,
        "胸深/鬐甲高": (P.wither - P.y_chest) / P.wither,
    }
    # 马的常识区间：体长 0.95-1.15、头长 0.36-0.46、枕高 1.05-1.25、胸深 0.46-0.56
    for label, lo, hi in (
        ("体长/鬐甲高", 0.95, 1.15),
        ("头长/鬐甲高", 0.36, 0.46),
        ("枕高/鬐甲高", 1.05, 1.25),
        ("胸深/鬐甲高", 0.46, 0.56),
    ):
        if not lo <= ratios[label] <= hi:
            problems.append(f"{label}={ratios[label]:.3f} 越出马的常识区间 [{lo}, {hi}]")

    if verbose:
        print(f"【{P.label}({P.key})】骨骼 {len(rig.bones)} 根 · cube {len(rig.elements)} 个")
        print(
            f"  鬐甲 {wither:.1f}={wither / 16:.2f} m · 尻 {P.y_croup:.1f}={P.y_croup / 16:.2f} m · "
            f"头顶 {head_top:.1f}={head_top / 16:.2f} m"
        )
        print(
            f"  体长(肩端→臀端) {body:.1f}={body / 16:.2f} m · 含头尾 {total:.1f}={total / 16:.2f} m · "
            f"体宽 {max(xs) - min(xs):.2f}"
        )
        print("  " + " · ".join(f"{k} {v:.3f}" for k, v in ratios.items()))
        print(f"  最低点 y={min(ys):.3f}（蹄底）· 连通分量 {len(comps)}")
        if problems:
            print(f"  ✗ {len(problems)} 处违例：")
            for x in problems[:15]:
                print(f"     {x}")
            if len(problems) > 15:
                print(f"     …另 {len(problems) - 15} 处")
        else:
            print("  ✓ 镜像 / 着地 / 鬐甲对表 / 比例区间 / 骨树 / 连通性 全部通过")
    return len(problems)


# ================================================================ CLI
def model_name(P: Profile) -> str:
    return f"HorseSkeleton_{P.key}"


def write_model(rig: Rig, name: str, out: Path | None = None) -> Path:
    path = out or (OUT_DIR / f"{name}.bbmodel")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(rig.bbmodel(name), ensure_ascii=False, indent=1))
    return path


def main() -> int:
    ap = argparse.ArgumentParser(description="马骨架生成器（三档体型）")
    ap.add_argument("--profile", choices=[*sorted(PROFILES), "all"], default="all", help="体型档位")
    ap.add_argument("--part", choices=sorted(PARTS), help="只生成单个部件（预览用）")
    ap.add_argument("--list", action="store_true", help="列出部件与档位")
    ap.add_argument("--check", action="store_true", help="只跑结构自检，不写文件")
    ap.add_argument("--out", type=Path, help="输出路径（仅单档 / 单件时有效）")
    args = ap.parse_args()

    if args.list:
        print("档位：")
        for k, P in PROFILES.items():
            print(f"  {k:8s} {P.label}  鬐甲 {P.wither / 16:.2f} m")
        print("部件：")
        for k, (label, _) in sorted(PARTS.items()):
            print(f"  {k:10s} {label}")
        return 0

    keys = sorted(PROFILES) if args.profile == "all" else [args.profile]

    if args.check:
        bad = 0
        for k in keys:
            P = PROFILES[k]
            bad += check(P, build_full(P))
        return 1 if bad else 0

    rc = 0
    for k in keys:
        P = PROFILES[k]
        if args.part:
            rig = Rig()
            PARTS[args.part][1](rig, P)
            name = f"Horse_{args.part}_{P.key}"
        else:
            rig = build_full(P)
            name = model_name(P)
        path = write_model(rig, name, args.out if len(keys) == 1 else None)
        print(f"→ {path.relative_to(REPO)}")
        # 单件预览只有半具骨架，着地/鬐甲那几条断言本就不成立——只对整身自检
        if not args.part:
            rc |= check(P, rig)
    return 1 if rc else 0


if __name__ == "__main__":
    raise SystemExit(main())
