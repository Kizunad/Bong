#!/usr/bin/env python3
"""体素骨架装配工具箱 —— Rig 容器 + 骨干几何 + 调色板贴图。

自怠怒之狮的 gen_skeleton.py 提炼：那份把 Rig 类、shaft 数学、调色贴图和狮子解剖
写在一个文件里，另做物种就得整段抄。这里只留与物种无关的部分，材质表由调用方传入。
不改 dainu_lion —— 它的骨架已被手工精修过，动它的依赖等于赌上那些改动。

坐标约定（全流水线一致）：16 单位 = 1 格 = 1 m，地面 y=0，兽头朝 -Z（MC north）。
element 一律写**绝对坐标**（绑定姿态下与骨骼 pivot 自洽），因为 render_bbmodel.py
只读 elements 不读 outliner。
"""

from __future__ import annotations

import base64
import io
import math
import uuid
from collections.abc import Mapping
from typing import TypeAlias

from PIL import Image

Vec: TypeAlias = tuple[float, float, float]
RGB: TypeAlias = tuple[int, int, int]


def lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def smoothstep(a: float, b: float, t: float) -> float:
    t = max(0.0, min(1.0, t))
    return lerp(a, b, t * t * (3 - 2 * t))


def curve(knots: list[tuple[float, float]], x: float) -> float:
    """按 (x, y) 控制点做分段 smoothstep 插值；knots 须按 x 升序。"""
    if x <= knots[0][0]:
        return knots[0][1]
    if x >= knots[-1][0]:
        return knots[-1][1]
    for (x0, y0), (x1, y1) in zip(knots, knots[1:]):
        if x0 <= x <= x1:
            return smoothstep(y0, y1, (x - x0) / (x1 - x0)) if x1 > x0 else y0
    return knots[-1][1]


def shaft_box(a: Vec, b: Vec, rx: float, rz: float, extend: float = 0.0):
    """把「从关节 a 到关节 b、截面 rx×rz 的柱」解成 (from, to, rotation, origin)。

    纯几何，无副作用 —— 骨干、肌腹都靠它定向。

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


def _center(f: list[float], t: list[float]) -> list[float]:
    return [(a + b) / 2 for a, b in zip(f, t)]


def _rotate(p: Vec, rot, org) -> Vec:
    """按 Blockbench / render_bbmodel.py 的 R = Rz·Ry·Rx 绕 origin 转一个点。"""
    x, y, z = (p[0] - org[0], p[1] - org[1], p[2] - org[2])
    rx, ry, rz = (math.radians(v) for v in rot)
    cx, sx_ = math.cos(rx), math.sin(rx)
    y, z = y * cx - z * sx_, y * sx_ + z * cx
    cy, sy = math.cos(ry), math.sin(ry)
    x, z = x * cy + z * sy, -x * sy + z * cy
    cz, sz = math.cos(rz), math.sin(rz)
    x, y = x * cz - y * sz, x * sz + y * cz
    return (x + org[0], y + org[1], z + org[2])


class Rig:
    """收集 element + 骨骼树，最后组装成 .bbmodel。"""

    def __init__(self, mats: Mapping[str, RGB], *, tex: int = 64, swatch: int = 8) -> None:
        if len(mats) > (tex // swatch) ** 2:
            raise ValueError(f"材质 {len(mats)} 种放不进 {tex}×{tex} 贴图")
        self.mats = dict(mats)
        self.mat_names = tuple(mats)
        self.tex = tex
        self.swatch = swatch
        self.elements: list[dict] = []
        self.bones: dict[str, dict] = {}
        self.bone_order: list[str] = []

    # ---------------------------------------------------------------- 骨骼树
    def bone(self, name: str, pivot: Vec, parent: str | None = None) -> str:
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

    def pivot(self, name: str) -> Vec:
        b = self.bones.get(name)
        if b is None:
            raise ValueError(f"未定义骨骼: {name}")
        x, y, z = b["pivot"]
        return (x, y, z)

    # ---------------------------------------------------------------- 元素
    def cube(
        self,
        bone: str,
        name: str,
        frm: Vec,
        to: Vec,
        *,
        rot: Vec | None = None,
        org: Vec | None = None,
        mat: str = "bone",
    ) -> dict:
        if bone not in self.bones:
            raise ValueError(f"未定义骨骼: {bone}")
        if mat not in self.mats:
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
            "color": self.mat_names.index(mat) % 8,
            "origin": [round(v, 3) for v in (org or _center(f, t))],
            "rotation": [round(v, 3) for v in (rot or (0.0, 0.0, 0.0))],
            "faces": self._faces(mat),
        }
        self.elements.append(el)
        self.bones[bone]["children"].append(eid)
        return el

    def shaft(
        self,
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

        长骨若写成绝对 from/to + 手填 rotation，绕 origin 一转端点就飞离关节，
        渲出来是一堆散块而非骨链 —— 长骨一律走这里，别手写旋转。
        """
        rz = rx if rz is None else rz
        try:
            frm, to, rot, org = shaft_box(a, b, rx, rz, extend)
        except ValueError as exc:
            raise ValueError(f"{name}: {exc}") from exc
        self.cube(bone, name, frm, to, rot=rot, org=org, mat=mat)

    def taper(
        self,
        bone: str,
        prefix: str,
        pts: list[Vec],
        radii: list[float],
        *,
        mat: str = "bone",
        flat: float = 1.0,
    ) -> None:
        """沿折线铺一串逐段收细的骨干（喙、尾综骨、角这类锥体）。

        单根 cube 撑不出锥度：体素只有等截面柱，锥感全靠分段递减。
        flat = 横向半径相对纵向的比例（<1 = 扁）。
        """
        if len(pts) != len(radii):
            raise ValueError(f"{prefix}: 点数 {len(pts)} 与半径数 {len(radii)} 不等")
        for i, (a, b) in enumerate(zip(pts, pts[1:]), start=1):
            r = (radii[i - 1] + radii[i]) / 2
            self.shaft(bone, f"{prefix}_{i:02d}", a, b, r * flat, r, mat=mat)

    # ---------------------------------------------------------------- 贴图
    def _swatch_origin(self, mat: str) -> tuple[int, int]:
        i = self.mat_names.index(mat)
        per_row = self.tex // self.swatch
        return (i % per_row) * self.swatch, (i // per_row) * self.swatch

    def _faces(self, mat: str) -> dict:
        ox, oy = self._swatch_origin(mat)
        uv = [ox + 1.0, oy + 1.0, ox + self.swatch - 1.0, oy + self.swatch - 1.0]
        return {d: {"uv": list(uv), "texture": 0} for d in ("north", "south", "east", "west", "up", "down")}

    def texture_b64(self) -> str:
        img = Image.new("RGBA", (self.tex, self.tex), (0, 0, 0, 0))
        px = img.load()
        for i, mat in enumerate(self.mat_names):
            r, g, b = self.mats[mat]
            ox, oy = self._swatch_origin(mat)
            for y in range(self.swatch):
                for x in range(self.swatch):
                    # 轻噪：骨面不是塑料平涂，渲染时能看出体积。
                    # 幅度必须压住：一个 swatch 只有 8×8 像素，铺到龙骨突那种大平面上会被
                    # 最近邻放大成整齐的棋盘格 —— 看着像贴错图，而不是骨头。
                    n = ((x * 7 + y * 13 + i * 5) % 3) - 1
                    px[ox + x, oy + y] = (
                        max(0, min(255, r + n * 3)),
                        max(0, min(255, g + n * 3)),
                        max(0, min(255, b + n * 2)),
                        255,
                    )
        buf = io.BytesIO()
        img.save(buf, format="PNG")
        return base64.b64encode(buf.getvalue()).decode()

    # ---------------------------------------------------------------- 导出
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
            "resolution": {"width": self.tex, "height": self.tex},
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
                    "source": "data:image/png;base64," + self.texture_b64(),
                }
            ],
        }

    # ---------------------------------------------------------------- 自检
    def bounds(self) -> tuple[Vec, Vec]:
        """**旋转后**的真实包围盒。

        直接取 from/to 的极值是错的：斜置的骨干（趾骨、肋弓、喙）在局部坐标里是一根
        沿 +Y 的长柱，from[1] 落在离实际端点很远的地方 —— 量出来的"贴地"和"全长"
        会凭空差出半根骨头，而这些数字正是逐轮调参的唯一依据。
        """
        lo = [math.inf] * 3
        hi = [-math.inf] * 3
        for e in self.elements:
            f, t = e["from"], e["to"]
            rot, org = e.get("rotation") or (0, 0, 0), e.get("origin") or _center(f, t)
            corners = [
                (f[0] if i & 1 else t[0], f[1] if i & 2 else t[1], f[2] if i & 4 else t[2])
                for i in range(8)
            ]
            if any(rot):
                corners = [_rotate(c, rot, org) for c in corners]
            for c in corners:
                for a in range(3):
                    lo[a] = min(lo[a], c[a])
                    hi[a] = max(hi[a], c[a])
        return ((lo[0], lo[1], lo[2]), (hi[0], hi[1], hi[2]))

    def mirror_violations(self, tol: float = 0.02) -> list[str]:
        """左右件必须 x 取反、y/z 相等。

        目视核不出这些 —— 一侧的一串骨整体平移（漏乘 sx）而非镜像，三视图上
        完全看不出来，只有对拍能抓。
        """
        els = {e["name"]: e for e in self.elements}
        out: list[str] = []
        for name, e in els.items():
            if not (name.endswith("_l") or "_l_" in name):
                continue
            mate = name.replace("_l_", "_r_") if "_l_" in name else name[:-2] + "_r"
            m = els.get(mate)
            if m is None:
                out.append(f"{name}: 缺镜像件 {mate}")
                continue
            if abs(e["from"][0] + m["to"][0]) > tol or abs(e["to"][0] + m["from"][0]) > tol:
                out.append(f"{name}: x 未镜像（{e['from'][0]}..{e['to'][0]} vs {m['from'][0]}..{m['to'][0]}）")
            for axis, label in ((1, "y"), (2, "z")):
                if abs(e["from"][axis] - m["from"][axis]) > tol or abs(e["to"][axis] - m["to"][axis]) > tol:
                    out.append(f"{name}: {label} 与镜像件不等")
        return out
