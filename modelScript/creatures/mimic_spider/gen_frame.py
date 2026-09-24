#!/usr/bin/env python3
"""拟态灰烬蛛（mimic_spider）节肢框架层 —— Round 1/3。

外骨骼动物没有"骨骼→肌肉→皮"三层：肌肉长在壳里面。这里做两层——
**框架层**（本文件）：头胸部/腹部体块 + 螯肢 + 触肢 + 8 腿 × 4 节的结构节段，
是几何与绑定的权威；**甲壳层**（gen_shell.py）读框架派生甲板/眼簇/刚毛。

设计从拟态态倒推（worldview §七:731 拟态灰烬蛛）：
  · 伪装态 client 渲真方块，所以折叠姿必须收进 16×16×16 方块体积——这是硬约束，
    由 preview.py --fold 的 FK 包围盒断言把守，不靠目测。
  · 展开态走恐惧剪影：细长高拱腿（膝关节高出背线 4+ 单位，猎人蛛式），足展 ~2.6 格。
  · 右后腿（*4_r）整肢 0.8 缩比 + 独立材质：再生短腿——对称=安全，一处不对称让
    整只读作"出过事的活物"。爪也只留一只（再生肢丢了一根爪）。

腿节段取 4 节：基节(coxa) → 腿节(femur) → 胫节(tibia) → 跗节(tarsus)。真蛛有 7 节
（转节/膝节/跖节并入相邻段）——GeckoLib 骨数与 IK 复杂度都不值得 7 节。

坐标：16 单位 = 1 格 = 1 米，地面 y=0，头朝 −z。骨骼层级供 GeckoLib 驱动；
element 一律写**绝对坐标**（render_bbmodel.py 只读 elements 不读 outliner），
组 rest rotation 恒为 0，腿的方位角在动画层用共轭旋转处理（见 preview.py）。

用法:
  python3 modelScript/creatures/mimic_spider/gen_frame.py               # 全框架
  python3 modelScript/creatures/mimic_spider/gen_frame.py --part legs   # 单件预览
  python3 modelScript/creatures/mimic_spider/gen_frame.py --list
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
OUT_DIR = Path(__file__).resolve().parents[2] / "models" / "mimic_spider"

# ---------------------------------------------------------------- 全局尺度
# 体轴：头胸部 z −7..+0.5，腹部 z +1.5..+8.5 → 体长 15.5 单位（含螯肢 ~17，
# 折叠姿靠 root 后移 0.5 收进 z ±8）。
PROSOMA_FRONT_Z = -7.0
PROSOMA_BACK_Z = 0.5
ABDOMEN_FRONT_Z = 1.5
ABDOMEN_BACK_Z = 8.5

BODY_Y = 8.0          # 腿根附着高度（头胸部侧壁中线）
CARAPACE_TOP_Y = 12.0  # 背甲顶
STERNUM_BOT_Y = 5.2    # 胸板底

# 腿：方位角（度，0 = 正侧向 +x，正 = 朝前 −z），前后四对
LEG_AZ = (52.0, 18.0, -14.0, -46.0)
LEG_ATTACH_Z = (-5.6, -3.8, -2.0, -0.2)
ATTACH_X = 3.4

# 节段长度与仰角（度）。跗节仰角不给定——由"落地 y=0"解出，保证爪尖精确贴地。
# 长度上限受折叠姿约束：femur+tibia 必须能在 16³ 方块内完成 Z 形对折（preview.py）。
L_COXA, L_FEMUR, L_TIBIA, L_TARSUS = 2.5, 10.0, 13.0, 5.5
E_COXA, E_FEMUR, E_TIBIA = 12.0, 52.0, -70.0
REGROWN = "4_r"        # 再生短腿
REGROWN_SCALE = 0.8
REGROWN_E_FEMUR = 38.0  # 新腿没学会高拱：膝抬不起来
REGROWN_E_TIBIA = -76.0  # 短腿踮脚：胫节更陡才够得着地

MATS = ("chitin", "chitin_dark", "chitin_new", "membrane", "fang", "socket")

# 8 眼窝：不对称簇——中央主眼一对偏大，侧眼两排，右侧两枚刻意偏移。
# 对称的眼阵是装饰，不对称的眼阵在"看你"。甲壳层按同坐标放眼球（唯一暖色）。
EYES = (
    ("eye_am_l", (-0.95, 12.0, -7.0), 0.75),   # 中央主眼
    ("eye_am_r", (0.95, 12.0, -7.0), 0.75),
    ("eye_al_l", (-1.95, 11.7, -6.7), 0.55),   # 前侧眼
    ("eye_al_r", (1.95, 12.2, -6.7), 0.55),    # ← 右前侧眼抬高：不对称
    ("eye_pm_l", (-0.6, 12.55, -6.1), 0.45),   # 后中眼
    ("eye_pm_r", (0.6, 12.55, -6.1), 0.45),
    ("eye_pl_l", (-1.8, 12.5, -5.8), 0.5),     # 后侧眼
    ("eye_pl_r", (1.75, 12.45, -5.6), 0.5),    # ← 右后侧眼位置微偏
)


# ---------------------------------------------------------------- rig 容器
class Rig:
    """收集 element + 骨骼树，最后组装成 .bbmodel（与 dainu_lion 同构）。"""

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
        mat: str = "chitin",
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
            "origin": [round(v, 3) for v in (org or [(a + b) / 2 for a, b in zip(f, t)])],
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
                    "uuid": str(uuid.uuid4()),
                    "source": "data:image/png;base64," + _texture_b64(),
                }
            ],
        }


Vec = tuple[float, float, float]


def shaft_box(a: Vec, b: Vec, rx: float, rz: float, extend: float = 0.0):
    """"从关节 a 到关节 b、截面 rx×rz 的柱"→ (from, to, rotation, origin)。

    数学同 dainu_lion：cube 沿局部 +Y 建长 L 的柱，R=Rz·Ry·Rx 下
    p=acos(dy/L)、w=atan2(dx,dz) 对齐目标方向。
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


def _shaft(rig: Rig, bone: str, name: str, a: Vec, b: Vec, rx: float,
           rz: float | None = None, *, mat: str = "chitin", extend: float = 0.0) -> None:
    rz = rx if rz is None else rz
    try:
        frm, to, rot, org = shaft_box(a, b, rx, rz, extend)
    except ValueError as exc:
        raise ValueError(f"{name}: {exc}") from exc
    rig.cube(bone, name, frm, to, rot=rot, org=org, mat=mat)


def _knuckle(rig: Rig, bone: str, name: str, p: Vec, r: float, mat: str = "chitin_dark") -> None:
    rig.cube(bone, name, (p[0] - r, p[1] - r, p[2] - r), (p[0] + r, p[1] + r, p[2] + r), mat=mat)


# ---------------------------------------------------------------- 调色板贴图
TEX_W = TEX_H = 64
_SWATCH = 8
_COLORS = {
    "chitin": (96, 89, 82),        # 灰褐甲壳——残灰色系，正典"共时灰质化"
    "chitin_dark": (52, 47, 43),   # 关节髁 / 深部
    "chitin_new": (152, 140, 114),  # 再生新甲：苍黄，明显"没长旧"
    "membrane": (74, 62, 56),      # 节间膜 / 腹柄
    "fang": (34, 28, 24),          # 硬化螯牙 / 爪
    "socket": (40, 36, 33),        # 眼窝
}


def _texture_b64() -> str:
    img = Image.new("RGBA", (TEX_W, TEX_H), (0, 0, 0, 0))
    px = img.load()
    for i, mat in enumerate(MATS):
        r, g, b = _COLORS[mat]
        ox, oy = (i % 8) * _SWATCH, (i // 8) * _SWATCH
        for y in range(_SWATCH):
            for x in range(_SWATCH):
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


# ---------------------------------------------------------------- 腿几何
def leg_joints(pair: int, side: str) -> list[Vec]:
    """一条腿的关节世界坐标 [基节根, 转节, 膝, 踝, 爪尖]。

    仰角链正解，跗节仰角由"爪尖 y=0"反解——落地是解出来的，不是摆出来的。
    再生腿(4_r)整肢缩比后胫节加陡（REGROWN_E_TIBIA），短腿踮着脚才够得着地。
    """
    sx = 1.0 if side == "r" else -1.0
    key = f"{pair}_{side}"
    scale = REGROWN_SCALE if key == REGROWN else 1.0
    lens = [L_COXA * scale, L_FEMUR * scale, L_TIBIA * scale]
    if key == REGROWN:
        elevs = [E_COXA, REGROWN_E_FEMUR, REGROWN_E_TIBIA]
    else:
        elevs = [E_COXA, E_FEMUR, E_TIBIA]

    az = math.radians(LEG_AZ[pair - 1])
    u = (sx * math.cos(az), 0.0, -math.sin(az))  # 方位单位向量（水平）
    pts: list[Vec] = [(sx * ATTACH_X, BODY_Y, LEG_ATTACH_Z[pair - 1])]
    for length, elev in zip(lens, elevs):
        e = math.radians(elev)
        h, v = length * math.cos(e), length * math.sin(e)
        p = pts[-1]
        pts.append((p[0] + u[0] * h, p[1] + v, p[2] + u[2] * h))

    # 跗节：落地反解
    lt = L_TARSUS * scale
    ankle_y = pts[-1][1]
    drop = min(lt, ankle_y)
    h = math.sqrt(max(0.0, lt * lt - drop * drop))
    p = pts[-1]
    pts.append((p[0] + u[0] * h, p[1] - drop, p[2] + u[2] * h))
    return pts


# ================================================================ 部件：头胸部
def part_prosoma(rig: Rig) -> None:
    """头胸部框架：背甲板 + 眼丘（8 眼窝，不对称簇）+ 胸板 + 基节根座。"""
    b = "prosoma"

    # 背甲：一块前倾主板 + 前缘下折板（额区）
    rig.cube(b, "carapace_main", (-3.2, CARAPACE_TOP_Y - 1.4, -6.6), (3.2, CARAPACE_TOP_Y, 0.5),
             rot=(-4.0, 0.0, 0.0), org=(0.0, CARAPACE_TOP_Y - 0.7, -3.0))
    rig.cube(b, "carapace_brow", (-2.6, CARAPACE_TOP_Y - 2.2, -7.2), (2.6, CARAPACE_TOP_Y - 0.6, -5.8),
             rot=(14.0, 0.0, 0.0), org=(0.0, CARAPACE_TOP_Y - 1.4, -6.4))
    # 侧壁（腿根从这里长出）
    for s, sx in (("l", -1.0), ("r", 1.0)):
        rig.cube(b, f"pleuron_{s}", (sx * 2.6, 6.4, -6.4), (sx * 3.6, 11.0, 0.3), mat="chitin_dark")
    # 胸板
    rig.cube(b, "sternum", (-2.0, STERNUM_BOT_Y, -6.0), (2.0, STERNUM_BOT_Y + 1.1, 0.0),
             mat="chitin_dark")
    # 前壁封口（吻端）
    rig.cube(b, "clypeus", (-2.4, 6.2, -7.3), (2.4, 10.6, -6.4))

    # 眼丘 + 8 眼窝（坐标见模块级 EYES——甲壳层按同位置放眼球）
    rig.cube(b, "eye_mound", (-2.2, 11.2, -6.9), (2.2, 12.6, -5.2), mat="chitin_dark")
    for name, (x, y, z), r in EYES:
        rig.cube(b, name, (x - r, y - r, z - r * 0.7), (x + r, y + r, z + r * 0.7), mat="socket")


# ================================================================ 部件：腹部
def part_abdomen(rig: Rig) -> None:
    """腹部框架：腹柄 + 主囊体（后仰）+ 背嵴 + 纺器双突。"""
    b = "abdomen"
    # 腹柄（pedicel）：头胸−腹之间的细腰，膜质
    _shaft(rig, b, "pedicel", (0.0, 8.6, PROSOMA_BACK_Z), (0.0, 9.2, ABDOMEN_FRONT_Z + 0.4),
           0.9, mat="membrane")
    # 主囊体：微后仰（尾端上抬），威吓时再抬高
    rig.cube(b, "opisthosoma", (-3.7, 5.6, ABDOMEN_FRONT_Z), (3.7, 12.2, ABDOMEN_BACK_Z),
             rot=(-9.0, 0.0, 0.0), org=(0.0, 8.9, ABDOMEN_FRONT_Z + 1.0))
    rig.cube(b, "abdomen_ridge", (-1.4, 12.0, 2.4), (1.4, 13.1, 7.2),
             rot=(-9.0, 0.0, 0.0), org=(0.0, 8.9, ABDOMEN_FRONT_Z + 1.0), mat="chitin_dark")
    # 纺器：尾端两枚小突（外伸 0.7 收敛——折叠姿的 z 后界就卡在这里）
    for s, sx in (("l", -1.0), ("r", 1.0)):
        _shaft(rig, b, f"spinneret_{s}", (sx * 0.8, 7.0, ABDOMEN_BACK_Z - 0.4),
               (sx * 1.2, 7.9, ABDOMEN_BACK_Z + 0.7), 0.45, mat="chitin_dark")


# ================================================================ 部件：螯肢/触肢
def part_chelicerae(rig: Rig) -> None:
    """螯肢：基节（paturon）垂吊于吻端下 + 可开合的螯牙。牙是独立骨骼——
    bite 动画要张到 40°+ 露内侧，展示口器内侧是硬编码的厌恶触发器。"""
    for s, sx in (("l", -1.0), ("r", 1.0)):
        cb = f"chelicera_{s}"
        rig.cube(cb, f"paturon_{s}", (sx * 0.25, 5.2, -8.3), (sx * 1.95, 8.2, -6.8))
        fb = f"fang_{s}"
        _shaft(rig, fb, f"fang_{s}", (sx * 1.1, 5.3, -7.6), (sx * 0.35, 3.3, -8.3),
               0.42, mat="fang")


def part_palps(rig: Rig) -> None:
    """触肢：两节迷你腿，前举微抬——idle 微颤的主要载体（高频低幅，永不全静）。"""
    for s, sx in (("l", -1.0), ("r", 1.0)):
        root = (sx * 2.1, 7.2, -6.6)
        mid = (sx * 4.6, 8.9, -8.6)
        tip = (sx * 5.4, 8.2, -11.4)
        _shaft(rig, f"palp1_{s}", f"palp_femur_{s}", root, mid, 0.55)
        _knuckle(rig, f"palp1_{s}", f"palp_knee_{s}", mid, 0.7)
        _shaft(rig, f"palp2_{s}", f"palp_tarsus_{s}", mid, tip, 0.5)


# ================================================================ 部件：腿
def part_leg(rig: Rig, pair: int, side: str) -> None:
    """单腿：基节→腿节→胫节→跗节 + 关节髁 + 爪。再生腿(4_r)换新甲材质、只剩一爪。"""
    key = f"{pair}_{side}"
    regrown = key == REGROWN
    mat = "chitin_new" if regrown else "chitin"
    p0, p1, p2, p3, tip = leg_joints(pair, side)

    _shaft(rig, f"coxa{key}", f"coxa_{key}", p0, p1, 1.35, mat=mat, extend=0.3)
    _knuckle(rig, f"coxa{key}", f"trochanter_{key}", p1, 1.0)
    _shaft(rig, f"femur{key}", f"femur_{key}", p1, p2, 1.05, mat=mat)
    _knuckle(rig, f"femur{key}", f"patella_{key}", p2, 1.15)
    _shaft(rig, f"tibia{key}", f"tibia_{key}", p2, p3, 0.8, mat=mat)
    _knuckle(rig, f"tibia{key}", f"ankle_{key}", p3, 0.75)
    _shaft(rig, f"tarsus{key}", f"tarsus_{key}", p3, tip, 0.6, mat=mat)

    # 爪：从爪尖向下前方的两根小刺（再生腿只剩一根）
    az = math.radians(LEG_AZ[pair - 1])
    sx = 1.0 if side == "r" else -1.0
    u = (sx * math.cos(az), 0.0, -math.sin(az))
    perp = (-u[2] * sx, 0.0, u[0] * sx)  # 水平面内垂直于腿轴
    claws = (1,) if regrown else (-1, 1)
    for c in claws:
        base = (tip[0] + perp[0] * 0.35 * c, tip[1] + 0.5, tip[2] + perp[2] * 0.35 * c)
        end = (tip[0] + u[0] * 1.4 + perp[0] * 0.5 * c, 0.0, tip[2] + u[2] * 1.4 + perp[2] * 0.5 * c)
        _shaft(rig, f"tarsus{key}", f"claw_{key}_{'a' if c < 0 else 'b'}", base, end, 0.3, mat="fang")


# ================================================================ 组装
def _skeleton_bones(rig: Rig) -> None:
    """骨骼树：root → prosoma → {abdomen, 螯肢×2(+牙), 触肢×2×2, 腿×8×4}。"""
    rig.bone("root", (0.0, BODY_Y, 0.0))
    rig.bone("prosoma", (0.0, 8.6, PROSOMA_BACK_Z), "root")
    rig.bone("abdomen", (0.0, 8.9, ABDOMEN_FRONT_Z + 0.4), "prosoma")
    for s, sx in (("l", -1.0), ("r", 1.0)):
        rig.bone(f"chelicera_{s}", (sx * 1.1, 8.2, -7.55), "prosoma")
        rig.bone(f"fang_{s}", (sx * 1.1, 5.3, -7.6), f"chelicera_{s}")
        rig.bone(f"palp1_{s}", (sx * 2.1, 7.2, -6.6), "prosoma")
        rig.bone(f"palp2_{s}", (sx * 4.6, 8.9, -8.6), f"palp1_{s}")
    for pair in (1, 2, 3, 4):
        for side in ("l", "r"):
            key = f"{pair}_{side}"
            p0, p1, p2, p3, _tip = leg_joints(pair, side)
            rig.bone(f"coxa{key}", p0, "prosoma")
            rig.bone(f"femur{key}", p1, f"coxa{key}")
            rig.bone(f"tibia{key}", p2, f"femur{key}")
            rig.bone(f"tarsus{key}", p3, f"tibia{key}")


PARTS = {
    "prosoma": (part_prosoma,),
    "abdomen": (part_abdomen,),
    "chelicerae": (part_chelicerae,),
    "palps": (part_palps,),
    "legs": tuple(
        (lambda r, p=pair, s=side: part_leg(r, p, s))
        for pair in (1, 2, 3, 4) for side in ("l", "r")
    ),
}


def build(only: str | None = None) -> Rig:
    rig = Rig()
    _skeleton_bones(rig)
    for name, fns in PARTS.items():
        if only is not None and name != only:
            continue
        for fn in fns:
            fn(rig)
    return rig


# ================================================================ 自检
def check(rig: Rig) -> int:
    """结构自检：镜像 · 落地 · 高拱 · 再生腿确实短 · 尺度。返回违例数。"""
    problems: list[str] = []

    # 落地：8 只爪尖 y 必须 ≈0（跗节反解应保证；抓的是几何参数改坏）
    for pair in (1, 2, 3, 4):
        for side in ("l", "r"):
            tip = leg_joints(pair, side)[-1]
            if abs(tip[1]) > 0.05:
                problems.append(f"leg {pair}_{side}: 爪尖离地 y={tip[1]:+.2f}")

    # 镜像：1-3 对腿 + 螯肢 + 触肢严格镜像；第 4 对豁免（再生腿刻意不对称）
    for pair in (1, 2, 3):
        jl, jr = leg_joints(pair, "l"), leg_joints(pair, "r")
        for i, (a, b) in enumerate(zip(jl, jr)):
            if abs(a[0] + b[0]) > 0.02 or abs(a[1] - b[1]) > 0.02 or abs(a[2] - b[2]) > 0.02:
                problems.append(f"leg pair {pair} 关节 {i} 未镜像")

    # 再生腿必须显著短——防止有人"顺手修对称"把设计修没了
    tip_l = leg_joints(4, "l")[-1]
    tip_r = leg_joints(4, "r")[-1]
    reach_l = math.hypot(tip_l[0] - (-ATTACH_X), tip_l[2] - LEG_ATTACH_Z[3])
    reach_r = math.hypot(tip_r[0] - ATTACH_X, tip_r[2] - LEG_ATTACH_Z[3])
    if reach_r > reach_l * 0.9:
        problems.append(f"再生腿不够短：reach r={reach_r:.1f} vs l={reach_l:.1f}（应 ≤0.9×）")

    # 高拱：膝峰必须高出背甲顶 3+ 单位（恐惧剪影的硬指标）
    knee_top = max(leg_joints(p, s)[2][1] for p in (1, 2, 3) for s in ("l", "r"))
    if knee_top < CARAPACE_TOP_Y + 3.0:
        problems.append(f"膝峰 {knee_top:.1f} 未高出背甲 {CARAPACE_TOP_Y} 至少 3 单位")

    xs = [v for e in rig.elements for v in (e["from"][0], e["to"][0])]
    zs = [v for e in rig.elements for v in (e["from"][2], e["to"][2])]
    span = max(xs) - min(xs)
    print(f"骨骼 {len(rig.bones)} 根 · cube {len(rig.elements)} 个")
    body_len = ABDOMEN_BACK_Z - (-8.3)  # 螯肢前缘 → 腹尾
    print(f"足展 {span:.1f} 单位 = {span / 16:.2f} 格 · 膝峰 y {knee_top:.1f}"
          f"（背甲 {CARAPACE_TOP_Y}） · 体长(含螯) {body_len:.1f} · 总长(含腿) {max(zs) - min(zs):.1f}")
    if problems:
        print(f"\n✗ {len(problems)} 处违例：")
        for x in problems[:20]:
            print(f"   {x}")
    else:
        print("\n✓ 落地/镜像/再生腿/高拱全部通过")
    return len(problems)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--part", choices=sorted(PARTS))
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()
    if args.list:
        for name in PARTS:
            print(name)
        return 0

    rig = build(args.part)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    suffix = f"_{args.part}" if args.part else ""
    out = OUT_DIR / f"MimicSpiderFrame{suffix}.bbmodel"
    out.write_text(json.dumps(rig.bbmodel(f"MimicSpiderFrame{suffix}"), ensure_ascii=False))
    print(f"→ {out}")
    return check(rig) if not args.part else 0


if __name__ == "__main__":
    raise SystemExit(main())
