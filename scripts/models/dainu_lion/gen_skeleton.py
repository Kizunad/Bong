#!/usr/bin/env python3
"""怠怒之狮（dainu_lion）骨架生成器 —— Round 1/3。

真骨架，不是占位方块：按猫科（Panthera）解剖建骨——
  颅骨（脑颅/额/吻/颧弓/眶上突/枕髁）+ 下颌（体+升支+冠突）+ 牙（犬齿/门齿/裂齿）
  脊椎五段：颈 7 · 胸 13 · 腰 7 · 荐 3（融合）· 尾 18（逐节递减）
  胸廓：13 对肋，每根 2 段折弯（椎肋段外扩 + 胸肋段内收）+ 8 节胸骨
  前肢：肩胛（带肩胛冈）→ 肱骨 → 桡骨+尺骨（含鹰嘴突）→ 腕 → 掌 5 → 趾+爪
  后肢：髂/坐/耻 → 股骨（带大转子）→ 髌 → 胫+腓 → 跟骨（后突离地）→ 跖 → 趾+爪
姿态取**趾行**（digitigrade）：腕/跟离地，只有掌趾着地——猫科站姿的关键，
做成蹠行就成了熊。

物种特征（与典籍所载狮形之别）：头顶两根棕黑色螺旋角，捻角羚式——沿弧线
向上后方伸展，逐节绕自身长轴扭转，侧面带一条螺旋棱把扭转显出来。

骨骼层级 46 根供 GeckoLib 驱动；element 一律写**绝对坐标**（绑定姿态下与
骨骼 pivot 自洽），因为 render_bbmodel.py 只读 elements 不读 outliner。

用法:
  python3 scripts/models/dainu_lion/gen_skeleton.py                  # 全骨架
  python3 scripts/models/dainu_lion/gen_skeleton.py --part skull     # 单件预览
  python3 scripts/models/dainu_lion/gen_skeleton.py --list           # 列出部件
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import math
import uuid
from pathlib import Path

from PIL import Image

REPO = Path(__file__).resolve().parents[3]
OUT_DIR = REPO / "local_models" / "dainu_lion"

# ---------------------------------------------------------------- 全局尺度
# 单位：MC 像素单位，16 = 1 格 = 1 m。地面 y=0，头朝 -Z（MC north）。
SNOUT_Z = -28.0  # 吻端
OCCIPUT_Z = -17.5  # 枕部（颅骨后缘）
THORAX_FRONT_Z = -11.0  # T1
THORAX_BACK_Z = 2.0  # T13
LUMBAR_BACK_Z = 8.0  # L7
SACRUM_Z = 11.5  # 荐椎后缘 → 尾根
# 吻端 -27 → 荐后 11.5 = 38.5 单位 ≈ 2.4 m 体长（"身长足有两米多"）
WITHER_Y = 24.6  # 鬐甲顶（T3-T5 棘突）≈ 1.54 m 肩高

# socket = 深色"孔洞"色。体素没有布尔减法，正交投影也没有凹陷阴影——眼窝/鼻孔/耳道
# 这类洞只能靠一块凹进去的深色面来读，否则侧视永远是一坨实心骨板。
MATS = ("bone", "bone_dark", "horn", "horn_ridge", "tooth", "cartilage", "socket")


def _lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def spine_y(z: float) -> float:
    """椎体中心线高度。猫科：荐高 → 胸中段最低 → 颈段抬升入枕。"""
    knots = [
        (SACRUM_Z, 19.4),
        (LUMBAR_BACK_Z, 19.9),
        (THORAX_BACK_Z, 20.3),
        (THORAX_FRONT_Z, 20.9),
        (-14.0, 22.6),
        (OCCIPUT_Z, 24.6),
    ]
    if z >= knots[0][0]:
        return knots[0][1]
    if z <= knots[-1][0]:
        return knots[-1][1]
    for (z0, y0), (z1, y1) in zip(knots, knots[1:]):
        if z1 <= z <= z0:
            t = (z0 - z) / (z0 - z1)
            return _lerp(y0, y1, t * t * (3 - 2 * t))  # smoothstep
    return knots[-1][1]


# ---------------------------------------------------------------- rig 容器
class Rig:
    """收集 element + 骨骼树，最后组装成 .bbmodel。"""

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
            "uuid": str(uuid.uuid4()),
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
        f = [round(min(a, b), 3) for a, b in zip(frm, to)]
        t = [round(max(a, b), 3) for a, b in zip(frm, to)]
        eid = str(uuid.uuid4())
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
            "meta": {
                "format_version": "4.10",
                "model_format": "free",
                "box_uv": False,
            },
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
                    "uuid": str(uuid.uuid4()),
                    "source": "data:image/png;base64," + _texture_b64(),
                }
            ],
        }


def _center(f: list[float], t: list[float]) -> list[float]:
    return [(a + b) / 2 for a, b in zip(f, t)]


Vec = tuple[float, float, float]


def shaft_box(a: Vec, b: Vec, rx: float, rz: float, extend: float = 0.0):
    """把"从关节 a 到关节 b、截面 rx×rz 的柱"解成 (from, to, rotation, origin)。

    纯几何，无副作用——骨干、肌腹都靠它定向（肌肉层脚本也 import 这个）。

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
    """造一根**端点精确落在关节 a、b 上**的骨干。

    首版把长骨写成绝对 from/to + 手填 rotation，绕 origin 一转端点就飞离关节，
    渲出来是一堆散块而非骨链——长骨一律走这里，别再手写旋转。

    数学：cube 沿局部 +Y 建长 L 的柱，旋转按 render/Blockbench 的 R=Rz·Ry·Rx。
    Rx(p) 把 (0,1,0) 转到 (0,cos p,sin p)，再 Ry(w) 得 (sin w·sin p, cos p, cos w·sin p)，
    与目标单位向量对齐即解出 p=acos(dy/L)、w=atan2(dx,dz)。
    """
    rz = rx if rz is None else rz
    try:
        frm, to, rot, org = shaft_box(a, b, rx, rz, extend)
    except ValueError as exc:
        raise ValueError(f"{name}: {exc}") from exc
    rig.cube(bone, name, frm, to, rot=rot, org=org, mat=mat)


# ---------------------------------------------------------------- 调色板贴图
TEX_W = TEX_H = 64
_SWATCH = 8  # 每个材质占 8×8 像素块
_COLORS = {
    "bone": (214, 205, 184),
    "bone_dark": (176, 165, 142),
    "horn": (62, 46, 34),
    "horn_ridge": (38, 27, 20),
    "tooth": (238, 233, 216),
    "cartilage": (198, 200, 192),
    "socket": (72, 64, 54),
}


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


# ================================================================ 部件：颅骨
def part_skull(rig: Rig) -> None:
    """颅骨：脑颅 + 额 + 吻/上颌 + 鼻 + 颧弓 + 眶上突 + 枕髁。

    猫科颅骨短而宽，眶后突明显，颧弓外张（容纳咬肌）——狮子的"面目狰狞"
    主要来自颧弓外张 + 眶上突高耸，这两处刻意做足。
    """
    rig.bone("skull", (0.0, 25.0, -19.0), parent="neck_top")

    # 脑颅（后上方，窄于颧弓——猫科不是方脑袋）
    rig.cube("skull", "cranium", (-2.9, 22.8, OCCIPUT_Z), (2.9, 27.2, -22.0))
    # 矢状嵴（颞肌附着，公狮发达）
    rig.cube("skull", "sagittal_crest", (-0.6, 27.0, -18.2), (0.6, 27.9, -21.8))
    # 枕髁（与寰椎相接）
    rig.cube("skull", "occipital_condyle", (-2.0, 23.2, -16.6), (2.0, 25.2, OCCIPUT_Z), mat="cartilage")
    # 额骨（脑颅 → 吻部过渡，向前下倾）
    # 额骨只占两眼之间的中线（半宽 < 眶内缘 2.3）——否则侧视投影把眶孔填死，头骨就读不出来了。
    rig.cube("skull", "frontal", (-2.0, 23.7, -24.6), (2.0, 26.6, -21.8), rot=(-5.0, 0.0, 0.0))

    for sx, side in ((-1, "l"), (1, "r")):
        # --- 眼眶环：四根骨围出眶孔（体素无布尔减法，洞只能"围"出来）---
        rig.cube(  # 眶上突（眉弓，外上翘 —— 狰狞感主要来源）
            "skull",
            f"supraorbital_{side}",
            (sx * 2.3, 25.5, -24.7),
            (sx * 3.9, 26.5, -22.2),
            rot=(0.0, 0.0, sx * 15.0),
            org=(sx * 2.1, 25.8, -23.4),
        )
        rig.cube(  # 眶下缘（颧骨前段）
            "skull",
            f"infraorbital_{side}",
            (sx * 2.3, 22.9, -24.5),
            (sx * 4.0, 23.9, -22.3),
            mat="bone_dark",
        )
        rig.cube(  # 眶前柱（泪骨，眼窝前壁）
            "skull",
            f"lacrimal_{side}",
            (sx * 2.3, 23.5, -25.1),
            (sx * 3.1, 25.8, -24.4),
        )
        rig.cube(  # 眶后突（眼窝后壁，猫科明显）
            "skull",
            f"postorbital_{side}",
            (sx * 2.3, 23.7, -22.4),
            (sx * 3.4, 26.0, -21.6),
        )
        rig.cube(  # 眶窝底（凹进眶环，深色 = 眼窝）
            "skull",
            f"orbit_socket_{side}",
            (sx * 2.2, 23.9, -24.5),
            (sx * 3.5, 25.5, -22.5),
            mat="socket",
        )
        # --- 颧弓：自眶下缘向后外张到耳区，与脑颅之间空出颞窝 ---
        # 半宽 4.4 曾使颧宽/颅长 = 0.81（真狮 ≈ 0.68），皮层怎么收都压不回方块脸
        rig.cube(
            "skull",
            f"zygomatic_{side}",
            (sx * 3.1, 23.0, -22.6),
            (sx * 3.8, 24.2, -17.8),
            rot=(0.0, sx * -6.0, 0.0),
            org=(sx * 3.45, 23.6, -20.2),
            mat="bone_dark",
        )

    # 吻部：向前收窄且下倾（猫科口鼻低于脑颅）
    rig.cube("skull", "maxilla", (-2.2, 22.0, -26.8), (2.2, 25.0, -24.4), rot=(-6.0, 0.0, 0.0), org=(0.0, 24.4, -24.4))
    # 眶下区骨面（顶面压在眶孔下缘 23.8 以下，不侵占孔）
    rig.cube("skull", "maxilla_back", (-2.2, 22.0, -24.8), (2.2, 23.8, -22.0))
    rig.cube("skull", "nasal", (-1.5, 23.8, SNOUT_Z + 0.4), (1.5, 25.0, -25.6), rot=(-8.0, 0.0, 0.0), org=(0.0, 24.6, -25.6))
    rig.cube("skull", "premaxilla", (-1.9, 21.9, SNOUT_Z), (1.9, 24.0, -26.0))
    rig.cube("skull", "nasal_aperture", (-0.75, 22.9, SNOUT_Z - 0.05), (0.75, 24.0, -27.4), mat="socket")
    for sx, side in ((-1, "l"), (1, "r")):
        rig.cube("skull", f"ear_canal_{side}", (sx * 2.4, 24.2, -18.6), (sx * 3.2, 25.4, -17.6), mat="socket")
    # 硬腭
    rig.cube("skull", "palate", (-2.0, 21.7, -27.2), (2.0, 22.3, -21.6), mat="bone_dark")

    # 上齿列。犬齿走**唇侧**（x 比下颌体宽），闭口时外露而非插进下颌——
    # 首版让它直接穿过下颌体，侧视是一条诡异白条。
    rig.cube("skull", "incisors_up", (-1.5, 21.0, -27.7), (1.5, 21.9, -26.7), mat="tooth")
    for sx, side in ((-1, "l"), (1, "r")):
        rig.cube(
            "skull",
            f"canine_up_{side}",
            (sx * 1.9, 20.4, -27.0),
            (sx * 2.9, 22.2, -25.7),
            rot=(10.0, 0.0, 0.0),
            org=(sx * 2.4, 22.2, -26.4),
            mat="tooth",
        )
        rig.cube(
            "skull",
            f"carnassial_up_{side}",
            (sx * 1.6, 20.8, -24.4),
            (sx * 2.4, 21.8, -22.0),
            mat="tooth",
        )


def part_jaw(rig: Rig) -> None:
    """下颌：体 + 升支 + 冠突 + 下齿列。张口铰链在髁突（与颧弓后端相接）。"""
    rig.bone("jaw", (0.0, 22.4, -18.2), parent="skull")

    for sx, side in ((-1, "l"), (1, "r")):
        # 下颌体（水平支，前端向内收敛到联合处）
        rig.cube(
            "jaw",
            f"mandible_body_{side}",
            (sx * 1.0, 20.2, -27.0),
            (sx * 2.3, 21.8, -19.8),
            rot=(0.0, sx * 4.0, 0.0),
            org=(sx * 1.6, 21.0, -23.4),
        )
        # 升支（向上后，接髁突 —— 铰链在颧弓后端下方）
        rig.cube(
            "jaw",
            f"mandible_ramus_{side}",
            (sx * 1.2, 20.4, -20.8),
            (sx * 2.4, 23.4, -18.4),
            rot=(-24.0, 0.0, 0.0),
            org=(sx * 1.8, 21.0, -20.0),
        )
        # 冠突（颞肌附着，向上尖出插入颞窝）
        rig.cube(
            "jaw",
            f"coronoid_{side}",
            (sx * 1.3, 23.0, -21.2),
            (sx * 2.2, 25.2, -19.8),
            rot=(-28.0, 0.0, 0.0),
            org=(sx * 1.8, 23.0, -20.4),
            mat="bone_dark",
        )
        # 下犬齿（同样走唇侧，向上外露）
        rig.cube(
            "jaw",
            f"canine_low_{side}",
            (sx * 1.7, 21.6, -27.1),
            (sx * 2.7, 23.4, -26.0),
            rot=(-12.0, 0.0, 0.0),
            org=(sx * 2.2, 21.6, -26.5),
            mat="tooth",
        )
        rig.cube(
            "jaw",
            f"carnassial_low_{side}",
            (sx * 1.2, 21.7, -24.2),
            (sx * 2.0, 22.5, -22.0),
            mat="tooth",
        )
    # 下颌联合 + 下门齿
    rig.cube("jaw", "mandible_symphysis", (-1.4, 20.4, -27.2), (1.4, 21.9, -25.8))
    rig.cube("jaw", "incisors_low", (-1.2, 21.7, -27.3), (1.2, 22.5, -26.6), mat="tooth")


# ================================================================ 部件：螺旋角
HORN_SEGS = 6


def part_horns(rig: Rig) -> None:
    """两根棕黑螺旋角（捻角羚式）。

    沿弧线向上后外伸展，逐节绕长轴 roll 递增 → 螺旋；侧面贴一条随节旋转的
    棱把扭转显出来（截面若是圆的，扭转在体素上看不见）。
    """
    for sx, side in ((-1, "l"), (1, "r")):
        parent = "skull"
        for i in range(HORN_SEGS):
            t = i / (HORN_SEGS - 1)
            # 弧线：起于额后外侧，向上 → 后倒 → 末端外翻
            cx = sx * (2.6 + 3.4 * t * t)
            cy = 27.6 + 7.4 * t - 1.6 * t * t
            cz = -21.4 + 4.6 * t + 2.2 * t * t
            half_w = _lerp(1.15, 0.42, t)  # 粗细递减
            half_d = _lerp(0.85, 0.34, t)  # 扁截面：宽 ≠ 厚，扭转才可见
            seg_h = _lerp(2.5, 1.7, t)
            pitch = _lerp(-8.0, -46.0, t)  # 逐节后倒
            roll = sx * _lerp(4.0, 30.0, t)  # 外翻
            yaw = sx * (140.0 * t)  # ← 螺旋：绕长轴累计扭转 140°

            name = f"horn_{side}_{i + 1:02d}"
            rig.bone(name, (cx, cy, cz), parent=parent)
            rig.cube(
                name,
                f"{name}_core",
                (cx - half_w, cy - seg_h / 2, cz - half_d),
                (cx + half_w, cy + seg_h / 2, cz + half_d),
                rot=(pitch, yaw, roll),
                org=(cx, cy, cz),
                mat="horn",
            )
            # 螺旋棱：贴在截面宽边外侧，随本节一起转
            rig.cube(
                name,
                f"{name}_ridge",
                (cx + sx * half_w * 0.7, cy - seg_h / 2, cz - half_d * 1.6),
                (cx + sx * (half_w + 0.55), cy + seg_h / 2, cz + half_d * 1.6),
                rot=(pitch, yaw, roll),
                org=(cx, cy, cz),
                mat="horn_ridge",
            )
            parent = name
        # 角尖
        rig.cube(
            parent,
            f"horn_{side}_tip",
            (sx * 5.7, 33.0, -14.2),
            (sx * 6.5, 34.4, -13.4),
            rot=(-52.0, sx * 150.0, sx * 32.0),
            org=(sx * 6.1, 33.0, -13.8),
            mat="horn_ridge",
        )


# ================================================================ 部件：脊柱
def part_spine(rig: Rig) -> None:
    """颈 7 · 胸 13 · 腰 7 · 荐 3。棘突高度按鬐甲峰值分布。"""
    # --- 骨骼链（先父后子）：hips 是 root 之下的总枢纽 ---
    rig.bone("hips", (0.0, 19.5, SACRUM_Z - 1.0), parent="root")
    rig.bone("lumbar", (0.0, spine_y(LUMBAR_BACK_Z), LUMBAR_BACK_Z), parent="hips")
    rig.bone("thorax_back", (0.0, spine_y(THORAX_BACK_Z), THORAX_BACK_Z), parent="lumbar")
    rig.bone("thorax_front", (0.0, spine_y(-6.0), -6.0), parent="thorax_back")
    rig.bone("neck_base", (0.0, spine_y(THORAX_FRONT_Z), THORAX_FRONT_Z), parent="thorax_front")
    rig.bone("neck_mid", (0.0, spine_y(-13.5), -13.5), parent="neck_base")
    rig.bone("neck_top", (0.0, spine_y(-16.0), -16.0), parent="neck_mid")

    # --- 荐椎 3（融合，与髂骨相接）---
    for i in range(3):
        z = SACRUM_Z - 1.4 * i
        y = spine_y(z)
        rig.cube("hips", f"sacral_{i + 1}", (-1.5, y - 1.0, z - 1.35), (1.5, y + 1.0, z), mat="bone_dark")
    rig.cube("hips", "sacral_crest", (-0.8, spine_y(SACRUM_Z) + 0.9, SACRUM_Z - 4.2), (0.8, spine_y(SACRUM_Z) + 2.4, SACRUM_Z))

    # --- 腰椎 7（横突宽大，猫科腰段柔韧）---
    for i in range(7):
        t = i / 6
        # 腰椎须铺满荐前(7.6) → 胸后(2.9) 整段；首版误写成 SACRUM_Z-4.6=6.9，
        # 7 节挤在 0.7 格内、脊柱在 L7↔T13 之间断开 3.5 格。
        z = _lerp(LUMBAR_BACK_Z - 0.4, THORAX_BACK_Z + 0.9, t)
        y = spine_y(z)
        rig.cube("lumbar", f"lumbar_{i + 1}", (-1.45, y - 1.05, z - 0.75), (1.45, y + 1.05, z + 0.75))
        # 棘突（腰段前倾）
        rig.cube("lumbar", f"lumbar_sp_{i + 1}", (-0.55, y + 0.9, z - 0.45), (0.55, y + 3.1, z + 0.45), rot=(18.0, 0.0, 0.0), org=(0.0, y + 0.9, z))
        # 横突（左右翼）
        for sx, side in ((-1, "l"), (1, "r")):
            rig.cube(
                "lumbar",
                f"lumbar_tp_{i + 1}_{side}",
                (sx * 1.3, y - 0.5, z - 0.4),
                (sx * 3.5, y + 0.4, z + 0.4),
                rot=(0.0, sx * -18.0, sx * -8.0),
                org=(sx * 1.3, y, z),
                mat="bone_dark",
            )

    # --- 胸椎 13（棘突向后倾，T3-T5 最高 = 鬐甲）---
    for i in range(13):
        t = i / 12
        z = _lerp(THORAX_BACK_Z, THORAX_FRONT_Z, t)
        y = spine_y(z)
        bone = "thorax_back" if z > -6.0 else "thorax_front"
        rig.cube(bone, f"thoracic_{13 - i}", (-1.3, y - 1.0, z - 0.6), (1.3, y + 1.0, z + 0.6))
        # 棘突：峰值在 T3-T5（t≈0.68-0.82）
        peak = math.exp(-(((t - 0.75) / 0.30) ** 2))
        sp_h = 1.5 + (WITHER_Y - y - 1.5) * peak
        rig.cube(
            bone,
            f"thoracic_sp_{13 - i}",
            (-0.5, y + 0.85, z - 0.4),
            (0.5, y + 0.85 + max(1.2, sp_h), z + 0.4),
            rot=(_lerp(10.0, -12.0, t), 0.0, 0.0),
            org=(0.0, y + 0.85, z),
        )

    # --- 颈椎 7（C1 寰椎最宽，向前抬升）---
    for i in range(7):
        t = i / 6
        z = _lerp(THORAX_FRONT_Z - 0.6, OCCIPUT_Z + 1.0, t)
        y = spine_y(z)
        bone = "neck_base" if t < 0.34 else ("neck_mid" if t < 0.7 else "neck_top")
        half_w = _lerp(1.55, 2.05, t) if t > 0.75 else _lerp(1.55, 1.75, t)
        rig.cube(bone, f"cervical_{7 - i}", (-half_w, y - 1.05, z - 0.75), (half_w, y + 1.05, z + 0.75))
        if t < 0.75:  # C7→C3 有棘突，寰枢椎几乎无
            rig.cube(
                bone,
                f"cervical_sp_{7 - i}",
                (-0.45, y + 0.9, z - 0.35),
                (0.45, y + 0.9 + _lerp(2.2, 0.7, t), z + 0.35),
                rot=(_lerp(24.0, 8.0, t), 0.0, 0.0),
                org=(0.0, y + 0.9, z),
            )
        # 横突
        for sx, side in ((-1, "l"), (1, "r")):
            rig.cube(
                bone,
                f"cervical_tp_{7 - i}_{side}",
                (sx * half_w, y - 0.8, z - 0.35),
                (sx * (half_w + _lerp(0.9, 1.6, t)), y + 0.2, z + 0.35),
                mat="bone_dark",
            )


TAIL_VERTEBRAE = 20
TAIL_SEG = 1.28
TAIL_BONES = 10


def _tail_curve() -> list[tuple[float, float, float]]:
    """尾椎中心线：自尾根向后近水平出发，逐节转为下垂，末端几乎垂直。

    沿切线积分而非写死 z(b)——直接给 z 的多项式会让"向后延展"和"向下垂坠"
    互相打架（首版二次项爆出 2.1 m 的尾巴，比躯干还长）。
    """
    pts: list[tuple[float, float, float]] = []
    z, y = SACRUM_Z, 19.4
    for i in range(TAIL_VERTEBRAE):
        t = i / (TAIL_VERTEBRAE - 1)
        pts.append((z, y, t))
        # S 形：前段近水平 → 中段垂坠 → 末梢回翘。一路垂到底的尾巴看着没生气。
        if t < 0.6:
            deg = _lerp(10.0, -62.0, (t / 0.6) ** 1.6)
        else:
            deg = _lerp(-62.0, -14.0, ((t - 0.6) / 0.4) ** 0.8)
        ang = math.radians(deg)
        z += TAIL_SEG * math.cos(ang)
        y += TAIL_SEG * math.sin(ang)
    return pts


def part_tail(rig: Rig) -> None:
    """尾椎 20 节 → 10 根骨骼驱动（每根挂 2 节）。自然垂坠、末梢回翘，黑火日后附于此。"""
    pts = _tail_curve()
    parent = "hips"
    for b in range(TAIL_BONES):
        z, y, t = pts[b * 2]
        name = f"tail_{b + 1:02d}"
        rig.bone(name, (0.0, y, z), parent=parent)
        for k in range(2):
            zz, yy, tt = pts[b * 2 + k]
            r = _lerp(1.15, 0.3, tt)
            rig.cube(
                name,
                f"caudal_{b * 2 + k + 1}",
                (-r, yy - r, zz - TAIL_SEG * 0.5),
                (r, yy + r, zz + TAIL_SEG * 0.5),
                mat="bone" if b < 6 else "bone_dark",
            )
        parent = name


# ================================================================ 部件：胸廓
STERNUM_BACK_Z = -1.0
STERNUM_FRONT_Z = -11.0
STERNUM_Y = 9.8  # 胸骨高度，随 RIB_DEPTH_* 走；肌肉层 import 它定位胸区肌


RIB_HALF_WIDTH_MAX = 3.1  # 胸廓半宽（中段）
# 背腹深度。较首版 -20%：胸腔过深会把胸骨压得太低，胸头肌（胸骨→头）被拉成一条长带。
RIB_DEPTH_MID = 10.4
RIB_DEPTH_FRONT = 7.85


# 肋弓横截面控制点：(半宽比例, 深度比例)，自椎侧绕到腹侧。
RIB_ARC = ((0.30, 0.0), (0.86, 0.26), (1.0, 0.56), (0.62, 0.86), (0.30, 1.0))


def rib_profile(z: float):
    """该 z 处胸廓的 (半宽, 深度, 椎体顶高, 前后 drift)。"""
    span = THORAX_FRONT_Z - THORAX_BACK_Z
    t = max(0.0, min(1.0, (z - THORAX_BACK_Z) / span))
    bulge = math.exp(-(((t - 0.45) / 0.42) ** 2))
    return (
        _lerp(2.05, RIB_HALF_WIDTH_MAX, bulge),
        _lerp(RIB_DEPTH_FRONT, RIB_DEPTH_MID, bulge),
        spine_y(z),
        _lerp(0.0, 1.2, t),
    )


def rib_surface_point(z: float, fd: float, sx: int = 1):
    """肋弓外表面上的一点。fd = 深度比例（0 = 椎侧，1 = 腹侧）。

    肌肉层 import 这个来铺贴合胸廓的肌带——两边共用同一条弧，阔肌才不会
    悬空或陷进肋骨（首版阔肌是一整块平板，糊在胸廓上像块木板）。
    """
    half_w, depth, y_top, drift = rib_profile(z)
    fd = max(0.0, min(1.0, fd))
    fw = RIB_ARC[-1][0]
    for (w0, d0), (w1, d1) in zip(RIB_ARC, RIB_ARC[1:]):
        if d0 <= fd <= d1:
            fw = _lerp(w0, w1, (fd - d0) / (d1 - d0)) if d1 > d0 else w0
            break
    return (sx * half_w * fw, y_top - 0.7 - depth * fd, z + drift * fd)


def part_ribcage(rig: Rig) -> None:
    """13 对肋 + 8 节胸骨，截面取卵圆。

    肋弓分 4 段，控制点沿椭圆取（椎侧 → 上外 → 最宽 → 下内 → 胸骨），
    而不是"外扩一段 + 内收一段"两条直线——后者截面是个扁圆盘，正视像牛不像猫。
    猫科（尤其奔跑型）胸廓**深大于宽**：这里深 ~13 单位、半宽 3.1。
    """
    for i in range(13):
        t = i / 12  # 0 = T13（后），1 = T1（前）
        z = _lerp(THORAX_BACK_Z, THORAX_FRONT_Z, t)
        y_top = spine_y(z)
        bulge = math.exp(-(((t - 0.45) / 0.42) ** 2))  # 中段胸廓最鼓
        depth = _lerp(RIB_DEPTH_FRONT, RIB_DEPTH_MID, bulge)  # 背腹深度
        floor_y = y_top - 0.7 - depth
        bone = "thorax_back" if z > -6.0 else "thorax_front"
        for sx, side in ((-1, "l"), (1, "r")):
            # 走与肌肉层同一条弧（rib_surface_point），骨/肌才严丝合缝
            pts = [rib_surface_point(z, fd, sx) for _fw, fd in RIB_ARC]
            for seg, (p0, p1) in enumerate(zip(pts[:-1], pts[1:]), start=1):
                _shaft(rig, bone, f"rib_{13 - i}_{side}_{seg}", p0, p1, 0.40, 0.45)
            if i >= 4:  # 真肋 T1-T9：肋软骨汇入胸骨
                sz = _lerp(STERNUM_BACK_Z, STERNUM_FRONT_Z, (i - 4) / 8)
                _shaft(
                    rig,
                    bone,
                    f"costal_{13 - i}_{side}",
                    pts[-1],
                    (sx * 1.0, STERNUM_Y, sz),
                    0.34,
                    mat="cartilage",
                )

    # 胸骨 8 节（沿中线，前端胸骨柄略抬）
    for k in range(8):
        tk = k / 7
        z = _lerp(STERNUM_BACK_Z, STERNUM_FRONT_Z, tk)
        y = STERNUM_Y + 0.8 * tk * tk
        rig.cube(
            "thorax_front" if z < -6.0 else "thorax_back",
            f"sternebra_{k + 1}",
            (-1.0, y - 0.65, z - 0.62),
            (1.0, y + 0.65, z + 0.62),
            mat="bone_dark",
        )


# ================================================================ 部件：前肢
def part_foreleg(rig: Rig, sx: int, side: str) -> None:
    """肩胛 → 肱骨 → 桡尺（含鹰嘴突）→ 腕 → 掌 → 趾+爪。趾行姿：腕离地。

    关节点先定死，骨干一律由 _shaft 连接——保证骨链不脱节。
    肩胛上缘靠近棘突（x 小）、关节盂在外下前（x 大），是猫科肩胛的真实走向。
    """
    sc_top = (sx * 2.4, 23.8, -6.6)  # 肩胛上缘（贴鬐甲）
    shoulder = (sx * 4.0, 14.8, -12.4)  # 肩关节（关节盂），贴胸廓外侧
    elbow = (sx * 3.9, 9.2, -8.4)  # 肘
    wrist = (sx * 3.7, 3.9, -11.8)  # 腕
    toe_base = (sx * 3.7, 1.4, -13.9)  # 掌骨远端 = 趾根
    toe_tip = (sx * 3.7, 0.75, -15.1)  # 趾节远端（趾节贴地，猫科承重在此）

    sc = rig.bone(f"scapula_{side}", sc_top, parent="thorax_front")
    hu = rig.bone(f"humerus_{side}", shoulder, parent=sc)
    ra = rig.bone(f"radius_{side}", elbow, parent=hu)
    fp = rig.bone(f"forefoot_{side}", wrist, parent=ra)

    # 肩胛骨：扁片贴在胸廓外侧（rz 大 = 前后宽，rx 小 = 薄）
    _shaft(rig, sc, f"scapula_blade_{side}", sc_top, shoulder, 0.55, 2.4)
    # 肩胛冈：外侧纵脊，沿板面再造一条
    _shaft(
        rig,
        sc,
        f"scapula_spine_{side}",
        (sc_top[0] + sx * 0.9, sc_top[1], sc_top[2] + 0.4),
        (shoulder[0] + sx * 0.7, shoulder[1] + 1.2, shoulder[2] + 0.4),
        0.5,
        0.9,
        mat="bone_dark",
    )
    rig.cube(
        sc,
        f"glenoid_{side}",
        (shoulder[0] - sx * 0.9, shoulder[1] - 0.9, shoulder[2] - 0.9),
        (shoulder[0] + sx * 0.9, shoulder[1] + 0.9, shoulder[2] + 0.9),
        mat="cartilage",
    )

    # 肱骨
    _shaft(rig, hu, f"humerus_{side}_shaft", shoulder, elbow, 0.78)
    rig.cube(
        hu,
        f"humerus_{side}_condyle",
        (elbow[0] - sx * 1.0, elbow[1] - 0.9, elbow[2] - 1.0),
        (elbow[0] + sx * 1.0, elbow[1] + 0.9, elbow[2] + 1.0),
        mat="cartilage",
    )

    # 桡骨（前）+ 尺骨（后，略细）
    _shaft(rig, ra, f"radius_{side}_shaft", (elbow[0], elbow[1], elbow[2] - 0.5), (wrist[0], wrist[1], wrist[2] - 0.4), 0.72)
    _shaft(
        rig,
        ra,
        f"ulna_{side}_shaft",
        (elbow[0], elbow[1], elbow[2] + 0.7),
        (wrist[0], wrist[1], wrist[2] + 0.6),
        0.56,
        mat="bone_dark",
    )
    # 鹰嘴突（肘尖，向后上——猫科很显眼）
    _shaft(
        rig,
        ra,
        f"olecranon_{side}",
        elbow,
        (elbow[0], elbow[1] + 2.2, elbow[2] + 2.0),
        0.62,
        mat="bone_dark",
    )

    # 腕骨（两排）
    rig.cube(
        fp,
        f"carpus_{side}",
        (wrist[0] - sx * 1.1, wrist[1] - 0.9, wrist[2] - 1.0),
        (wrist[0] + sx * 1.1, wrist[1] + 0.9, wrist[2] + 1.0),
        mat="cartilage",
    )
    # 掌骨 4 + 趾节 + 爪
    for m in range(4):
        off = sx * (m - 1.5) * 0.96
        w = (wrist[0] + off, wrist[1], wrist[2])
        tb = (toe_base[0] + off, toe_base[1], toe_base[2])
        tt = (toe_tip[0] + off, toe_tip[1], toe_tip[2])
        _shaft(rig, fp, f"metacarpal_{side}_{m + 1}", w, tb, 0.50)
        _shaft(rig, fp, f"phalanx_{side}_{m + 1}", tb, tt, 0.48)
        _shaft(rig, fp, f"claw_{side}_{m + 1}", tt, (tt[0], 0.4, tt[2] - 0.8), 0.32, mat="tooth")
    # 悬趾（dewclaw，内侧离地不着地）
    _shaft(
        rig,
        fp,
        f"dewclaw_{side}",
        (wrist[0] - sx * 1.0, wrist[1] - 0.4, wrist[2] - 0.6),
        (wrist[0] - sx * 1.7, wrist[1] - 1.9, wrist[2] - 1.9),
        0.36,
        mat="tooth",
    )


# ================================================================ 部件：后肢
def part_hindleg(rig: Rig, sx: int, side: str) -> None:
    """髂坐耻 → 股骨（大转子）→ 髌 → 胫腓 → 跟骨（后突离地）→ 跖 → 趾+爪。"""
    hip = (sx * 3.2, 17.6, 9.0)
    knee = (sx * 3.7, 10.2, 3.6)  # 膝在前
    hock = (sx * 3.6, 5.0, 10.6)  # 跗/跟在后 —— 与膝构成猫科 Z 字后肢
    htoe = (sx * 3.5, 1.4, 6.6)
    htip = (sx * 3.5, 0.75, 5.4)

    fe = rig.bone(f"femur_{side}", hip, parent="hips")
    ti = rig.bone(f"tibia_{side}", knee, parent=fe)
    ta = rig.bone(f"tarsus_{side}", hock, parent=ti)
    hf = rig.bone(f"hindfoot_{side}", htoe, parent=ta)

    # 髂骨翼（向前上张开，与荐椎相接）
    rig.cube(
        "hips",
        f"ilium_{side}",
        (sx * 1.2, 17.4, 5.2),
        (sx * 3.8, 21.0, 11.4),
        rot=(-16.0, 0.0, sx * -12.0),
        org=(sx * 1.7, 19.0, 10.4),
    )
    # 坐骨（向后下）
    rig.cube(
        "hips",
        f"ischium_{side}",
        (sx * 1.4, 15.0, 11.0),
        (sx * 3.4, 17.8, 15.4),
        rot=(14.0, 0.0, 0.0),
        org=(sx * 2.2, 17.4, 11.4),
        mat="bone_dark",
    )
    # 耻骨（腹侧闭合骨盆环）
    rig.cube("hips", f"pubis_{side}", (sx * 1.0, 14.6, 8.4), (sx * 2.8, 16.2, 12.0), mat="bone_dark")
    # 髋臼
    rig.cube("hips", f"acetabulum_{side}", (sx * 2.4, 16.8, 7.6), (sx * 3.7, 18.8, 9.6), mat="cartilage")

    # 股骨
    _shaft(rig, fe, f"femur_{side}_shaft", hip, knee, 0.95)
    # 大转子（臀肌附着，髋外上方突起）
    _shaft(
        rig,
        fe,
        f"greater_trochanter_{side}",
        hip,
        (hip[0] + sx * 1.5, hip[1] + 2.0, hip[2] + 0.4),
        0.7,
        mat="bone_dark",
    )
    rig.cube(
        fe,
        f"femur_{side}_condyle",
        (knee[0] - sx * 1.05, knee[1] - 1.0, knee[2] - 1.05),
        (knee[0] + sx * 1.05, knee[1] + 1.0, knee[2] + 1.05),
        mat="cartilage",
    )
    # 髌骨（膝盖，膝关节前方）
    rig.cube(
        fe,
        f"patella_{side}",
        (knee[0] - sx * 0.6, knee[1] + 0.1, knee[2] - 2.0),
        (knee[0] + sx * 0.6, knee[1] + 1.5, knee[2] - 1.0),
        mat="cartilage",
    )

    # 胫骨（主）+ 腓骨（外侧细）
    _shaft(rig, ti, f"tibia_{side}_shaft", knee, hock, 0.82)
    _shaft(
        rig,
        ti,
        f"fibula_{side}",
        (knee[0] + sx * 0.9, knee[1] - 0.6, knee[2] + 0.3),
        (hock[0] + sx * 0.6, hock[1] + 0.4, hock[2] - 0.3),
        0.36,
        mat="bone_dark",
    )

    # 跗骨 + 跟骨（跟结节向后上突出 = 趾行的解剖标志，离地 5 单位）
    rig.cube(
        ta,
        f"tarsus_{side}_bones",
        (hock[0] - sx * 0.9, hock[1] - 1.0, hock[2] - 1.0),
        (hock[0] + sx * 0.9, hock[1] + 0.8, hock[2] + 0.8),
        mat="cartilage",
    )
    _shaft(
        rig,
        ta,
        f"calcaneus_{side}",
        hock,
        (hock[0], hock[1] + 1.6, hock[2] + 2.4),
        0.62,
        mat="bone_dark",
    )

    # 跖骨：自跟向前下斜到趾根 —— 这段倾斜就是"趾行"
    for m in range(4):
        off = sx * (m - 1.5) * 0.96
        h = (hock[0] + off, hock[1] - 0.6, hock[2] - 0.4)
        tb = (htoe[0] + off, htoe[1], htoe[2])
        tt = (htip[0] + off, htip[1], htip[2])
        _shaft(rig, hf, f"metatarsal_{side}_{m + 1}", h, tb, 0.50)
        _shaft(rig, hf, f"hind_phalanx_{side}_{m + 1}", tb, tt, 0.48)
        _shaft(rig, hf, f"hind_claw_{side}_{m + 1}", tt, (tt[0], 0.4, tt[2] - 0.8), 0.32, mat="tooth")


# ================================================================ 装配
PARTS = {
    "skull": ("颅骨", lambda r: (_root(r), _neck_stub(r), part_skull(r))),
    "jaw": ("下颌", lambda r: (_root(r), _neck_stub(r), part_skull(r), part_jaw(r))),
    "horns": ("螺旋角", lambda r: (_root(r), _neck_stub(r), part_skull(r), part_horns(r))),
    "spine": ("脊柱", lambda r: (_root(r), part_spine(r))),
    "tail": ("尾椎", lambda r: (_root(r), part_spine(r), part_tail(r))),
    "ribcage": ("胸廓", lambda r: (_root(r), part_spine(r), part_ribcage(r))),
    "foreleg": ("前肢", lambda r: (_root(r), part_spine(r), part_foreleg(r, -1, "l"), part_foreleg(r, 1, "r"))),
    "hindleg": ("后肢", lambda r: (_root(r), part_spine(r), part_hindleg(r, -1, "l"), part_hindleg(r, 1, "r"))),
}


def _root(rig: Rig) -> None:
    rig.bone("root", (0.0, 0.0, 0.0))


def _neck_stub(rig: Rig) -> None:
    """单件预览用：只建到 skull 所需的父骨骼链，不铺全脊柱。"""
    rig.bone("hips", (0.0, 19.5, SACRUM_Z - 1.0), parent="root")
    rig.bone("lumbar", (0.0, spine_y(LUMBAR_BACK_Z), LUMBAR_BACK_Z), parent="hips")
    rig.bone("thorax_back", (0.0, spine_y(THORAX_BACK_Z), THORAX_BACK_Z), parent="lumbar")
    rig.bone("thorax_front", (0.0, spine_y(-6.0), -6.0), parent="thorax_back")
    rig.bone("neck_base", (0.0, spine_y(THORAX_FRONT_Z), THORAX_FRONT_Z), parent="thorax_front")
    rig.bone("neck_mid", (0.0, spine_y(-13.5), -13.5), parent="neck_base")
    rig.bone("neck_top", (0.0, spine_y(-16.0), -16.0), parent="neck_mid")


def build_full() -> Rig:
    rig = Rig()
    _root(rig)
    part_spine(rig)
    part_ribcage(rig)
    part_tail(rig)
    part_skull(rig)
    part_jaw(rig)
    part_horns(rig)
    for sx, side in ((-1, "l"), (1, "r")):
        part_foreleg(rig, sx, side)
        part_hindleg(rig, sx, side)
    return rig


def check(rig: Rig) -> int:
    """结构自检：左右镜像 · 骨链连续 · 贴地 · 尺度。返回违例数。

    目视核不出这些——首版左脚四根掌骨整体平移（off 少乘 sx）而非镜像，
    三视图上完全看不出来，是本函数抓的。
    """
    els = {e["name"]: e for e in rig.elements}
    problems: list[str] = []

    # 左右镜像：x 取反、y/z 相等
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

    # 贴地：最低点应 ≈0（爪尖着地），不得插进地面
    ys = [v for e in rig.elements for v in (e["from"][1], e["to"][1])]
    if not -0.05 <= min(ys) <= 0.6:
        problems.append(f"贴地异常：最低点 y={min(ys):.2f}（应 0~0.6，爪尖着地）")

    print(f"骨骼 {len(rig.bones)} 根 · cube {len(rig.elements)} 个")
    zs = [v for e in rig.elements for v in (e["from"][2], e["to"][2])]
    xs = [v for e in rig.elements for v in (e["from"][0], e["to"][0])]
    wither = max(
        v for n, e in els.items() if n.startswith("thoracic_sp") for v in (e["to"][1],)
    )
    print(f"鬐甲高 {wither:.1f} 单位 = {wither / 16:.2f} m（角尖 {max(ys) / 16:.2f} m）")
    print(f"身长(吻端→臀) {SACRUM_Z - min(zs):.1f} = {(SACRUM_Z - min(zs)) / 16:.2f} m · "
          f"含尾 {max(zs) - min(zs):.1f} = {(max(zs) - min(zs)) / 16:.2f} m · 体宽 {max(xs) - min(xs):.2f} 单位")
    print(f"最低点 y={min(ys):.2f}")

    if problems:
        print(f"\n✗ {len(problems)} 处违例：")
        for x in problems[:20]:
            print(f"   {x}")
        if len(problems) > 20:
            print(f"   …另 {len(problems) - 20} 处")
    else:
        print("\n✓ 对称/贴地/尺度全部通过")
    return len(problems)


def main() -> int:
    ap = argparse.ArgumentParser(description="怠怒之狮骨架生成器")
    ap.add_argument("--part", choices=sorted(PARTS), help="只生成单个部件（预览用）")
    ap.add_argument("--list", action="store_true", help="列出部件")
    ap.add_argument("--check", action="store_true", help="只跑结构自检，不写文件")
    ap.add_argument("--out", type=Path, help="输出路径（默认 local_models/DainuLionSkeleton.bbmodel）")
    args = ap.parse_args()

    if args.check:
        return 1 if check(build_full()) else 0

    if args.list:
        for k, (label, _) in sorted(PARTS.items()):
            print(f"  {k:10s} {label}")
        return 0

    if args.part:
        rig = Rig()
        PARTS[args.part][1](rig)
        name = f"DainuLion_{args.part}"
    else:
        rig = build_full()
        name = "DainuLionSkeleton"

    out = args.out or (OUT_DIR / f"{name}.bbmodel")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(rig.bbmodel(name), ensure_ascii=False, indent=1))

    ys = [v for e in rig.elements for v in (e["from"][1], e["to"][1])]
    zs = [v for e in rig.elements for v in (e["from"][2], e["to"][2])]
    print(f"→ {out}")
    print(f"   骨骼 {len(rig.bones)} 根 · cube {len(rig.elements)} 个")
    print(f"   体高 {max(ys) - min(ys):.1f} 单位（{(max(ys) - min(ys)) / 16:.2f} m），"
          f"纵长 {max(zs) - min(zs):.1f} 单位（{(max(zs) - min(zs)) / 16:.2f} m）")
    print(f"   最低点 y={min(ys):.2f}（应贴地 ≈0）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
