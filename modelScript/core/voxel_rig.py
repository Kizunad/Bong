#!/usr/bin/env python3
"""体素生物流水线的公共底座：调色板贴图 + Rig 容器 + 定向骨干。

来源是怠怒之狮 gen_skeleton.py 里那套自带的 Rig/shaft/palette；那份就地留着不动，
本模块只给后续生物用，免得每加一只兽就抄一遍同样的一百来行。

三件东西：
  Palette    一张 N×N 色块贴图 + 每种材质的 uv —— 省掉手画贴图这一步
  Rig        element 池 + 骨骼树 → .bbmodel（fmt 4.10）
  shaft_box  把"从关节 a 到关节 b、截面 rx×rz 的柱"解成 (from, to, rotation, origin)

约定（与 render_bbmodel.py / Blockbench 保持一致）：
  · 单位 = MC 像素，16 = 1 格 = 1 m；地面 y=0；生物头朝 -Z（MC north）
  · 旋转 R = Rz·Ry·Rx，绕 element 自己的 origin
  · element 一律写**绝对坐标**（绑定姿态下与骨骼 pivot 自洽）——render_bbmodel.py
    只读 elements 不读 outliner，写相对坐标会渲成一堆散块
"""

from __future__ import annotations
import sys

import base64
import io
import json
import math
import uuid
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import workspace  # noqa: E402

Vec = tuple[float, float, float]


# ---------------------------------------------------------------- 数值小工具
def lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def clamp(v: float, lo: float, hi: float) -> float:
    return lo if v < lo else (hi if v > hi else v)


def smoothstep(t: float) -> float:
    t = clamp(t, 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def catmull(knots: list[Vec] | tuple[Vec, ...], t: float) -> Vec:
    """Catmull-Rom 采样：t∈[0,1] 走完整条折线，必过每个 knot。

    端点用自身当虚拟控制点（而不是外推），首末段切线才不会甩出去——颈椎、尾椎
    这类"给几个解剖标志点、中间要连续过渡"的曲线全靠它。
    """
    n = len(knots)
    if n == 0:
        raise ValueError("catmull 需要至少一个 knot")
    if n == 1:
        return tuple(knots[0])  # type: ignore[return-value]
    t = clamp(t, 0.0, 1.0)
    seg = min(int(t * (n - 1)), n - 2)
    u = t * (n - 1) - seg

    def pick(i: int) -> Vec:
        return knots[max(0, min(n - 1, i))]

    p0, p1, p2, p3 = pick(seg - 1), pick(seg), pick(seg + 1), pick(seg + 2)
    u2, u3 = u * u, u * u * u
    return tuple(  # type: ignore[return-value]
        0.5 * (2 * b + (-a + c) * u + (2 * a - 5 * b + 4 * c - d) * u2 + (-a + 3 * b - 3 * c + d) * u3)
        for a, b, c, d in zip(p0, p1, p2, p3)
    )


def curve_length(knots, samples: int = 400) -> float:
    """曲线弧长（数值积分）。用来按真实长度分配椎节数，别按参数 t 均分。"""
    prev = catmull(knots, 0.0)
    total = 0.0
    for i in range(1, samples + 1):
        cur = catmull(knots, i / samples)
        total += math.dist(prev, cur)
        prev = cur
    return total


# ---------------------------------------------------------------- 定向骨干
def shaft_box(a: Vec, b: Vec, rx: float, rz: float, extend: float = 0.0):
    """把"从关节 a 到关节 b、截面 rx×rz 的柱"解成 (from, to, rotation, origin)。

    纯几何、无副作用——骨干、肌腹、羽簇全靠它定向。

    数学：cube 沿局部 +Y 建长 L 的柱，旋转按 R=Rz·Ry·Rx。Rx(p) 把 (0,1,0) 转到
    (0,cos p,sin p)，再 Ry(w) 得 (sin w·sin p, cos p, cos w·sin p)，与目标单位向量
    对齐即解出 p=acos(dy/L)、w=atan2(dx,dz)。

    别手写 from/to + rotation：绕 origin 一转端点就飞离关节，渲出来是散块不是骨链。
    """
    dx, dy, dz = (b[0] - a[0], b[1] - a[1], b[2] - a[2])
    length = math.sqrt(dx * dx + dy * dy + dz * dz)
    if length < 1e-6:
        raise ValueError("关节 a/b 重合，无法定向")
    half = length / 2 + extend
    pitch = math.degrees(math.acos(clamp(dy / length, -1.0, 1.0)))
    yaw = math.degrees(math.atan2(dx, dz)) if abs(dx) + abs(dz) > 1e-9 else 0.0
    cx, cy, cz = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2)
    return (
        (cx - rx, cy - half, cz - rz),
        (cx + rx, cy + half, cz + rz),
        (pitch, yaw, 0.0),
        (cx, cy, cz),
    )


def seg_dist(p0, p1, q0, q1):
    """两条线段的最短距离 + 最近点。碰撞检测的底座——肢体层的姿态求解、头颅层的互相
    顶开、整只兽的自检，三处共用这一份，别各写各的。

    标准解法：把参数域 [0,1]² 上的二次型求极小，再把越界的解夹回边界。
    """
    import numpy as np
    p0, p1, q0, q1 = (np.asarray(x, float) for x in (p0, p1, q0, q1))
    u, v, w = p1 - p0, q1 - q0, p0 - q0
    a, b, c, d, e = u @ u, u @ v, v @ v, u @ w, v @ w
    den = a * c - b * b
    if den < 1e-9:
        sN, sD, tN, tD = 0.0, 1.0, e, (c if c > 1e-9 else 1.0)
    else:
        sN, sD, tN, tD = (b * e - c * d), den, (a * e - b * d), den
        if sN < 0:
            sN, tN, tD = 0.0, e, (c if c > 1e-9 else 1.0)
        elif sN > sD:
            sN = sD
            tN, tD = e + b, (c if c > 1e-9 else 1.0)
    if tN < 0:
        tN = 0.0
        if -d < 0:
            sN = 0.0
        elif -d > a:
            sN = sD
        else:
            sN, sD = -d, (a if a > 1e-9 else 1.0)
    elif tN > tD:
        tN = tD
        if (-d + b) < 0:
            sN = 0.0
        elif (-d + b) > a:
            sN = sD
        else:
            sN, sD = (-d + b), (a if a > 1e-9 else 1.0)
    sc = 0.0 if abs(sD) < 1e-9 else sN / sD
    tc = 0.0 if abs(tD) < 1e-9 else tN / tD
    pa, pb = p0 + sc * u, q0 + tc * v
    return float(np.linalg.norm(pa - pb)), (pa + pb) * 0.5


# ---------------------------------------------------------------- 调色板贴图
class Palette:
    """材质名 → 一块纯色 swatch。整张贴图就是个色块表，uv 直接查表。

    带轻噪：平涂色块在正交投影下没有体积感，加 ±noise 的抖动才看得出面。
    """

    def __init__(self, colors: dict[str, tuple[int, int, int]], *, swatch: int = 8,
                 size: int = 64, noise: int = 4) -> None:
        self.names: tuple[str, ...] = tuple(colors)
        self.colors = dict(colors)
        self.swatch = swatch
        self.size = size
        self.noise = noise
        self.per_row = size // swatch
        if len(self.names) > self.per_row * self.per_row:
            raise ValueError(f"{len(self.names)} 种材质放不进 {size}×{size} 贴图"
                             f"（每行 {self.per_row} 块，最多 {self.per_row ** 2} 种）")

    def index(self, mat: str) -> int:
        try:
            return self.names.index(mat)
        except ValueError:
            raise ValueError(f"未知材质 {mat!r}；可用：{', '.join(self.names)}") from None

    def uv(self, mat: str) -> list[float]:
        i = self.index(mat)
        ox, oy = (i % self.per_row) * self.swatch, (i // self.per_row) * self.swatch
        # 内缩 1px：uv 贴着 swatch 边界时相邻色块会渗色（mipmap / 采样精度）
        return [ox + 1.0, oy + 1.0, ox + self.swatch - 1.0, oy + self.swatch - 1.0]

    def faces(self, mat: str) -> dict:
        uv = self.uv(mat)
        return {d: {"uv": list(uv), "texture": 0}
                for d in ("north", "south", "east", "west", "up", "down")}

    def texture_b64(self) -> str:
        img = Image.new("RGBA", (self.size, self.size), (0, 0, 0, 0))
        px = img.load()
        for i, mat in enumerate(self.names):
            r, g, b = self.colors[mat]
            ox, oy = (i % self.per_row) * self.swatch, (i // self.per_row) * self.swatch
            for y in range(self.swatch):
                for x in range(self.swatch):
                    n = ((x * 7 + y * 13 + i * 5) % 5) - 2
                    px[ox + x, oy + y] = (
                        clamp(r + n * self.noise, 0, 255),
                        clamp(g + n * self.noise, 0, 255),
                        clamp(b + n * (self.noise - 1), 0, 255),
                        255,
                    )
        buf = io.BytesIO()
        img.save(buf, format="PNG")
        return base64.b64encode(buf.getvalue()).decode()


# ---------------------------------------------------------------- rig 容器
class Rig:
    """收集 element + 骨骼树，最后组装成 .bbmodel。"""

    def __init__(self, palette: Palette) -> None:
        self.palette = palette
        self.elements: list[dict] = []
        self.bones: dict[str, dict] = {}
        self.bone_order: list[str] = []

    # -- 骨骼 --------------------------------------------------------------
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
        return tuple(self.bones[name]["pivot"])  # type: ignore[return-value]

    # -- 几何 --------------------------------------------------------------
    def cube(self, bone: str, name: str, frm: Vec, to: Vec, *, mat: str,
             rot: Vec | None = None, org: Vec | None = None) -> dict:
        if bone not in self.bones:
            raise ValueError(f"未定义骨骼: {bone}")
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
            "color": self.palette.index(mat) % 8,
            "origin": [round(v, 3) for v in (org or [(a + b) / 2 for a, b in zip(f, t)])],
            "rotation": [round(v, 3) for v in (rot or (0.0, 0.0, 0.0))],
            "faces": self.palette.faces(mat),
        }
        self.elements.append(el)
        self.bones[bone]["children"].append(eid)
        return el

    def shaft(self, bone: str, name: str, a: Vec, b: Vec, rx: float, rz: float | None = None,
              *, mat: str, extend: float = 0.0) -> dict:
        """造一根端点精确落在关节 a、b 上的柱。"""
        try:
            frm, to, rot, org = shaft_box(a, b, rx, rx if rz is None else rz, extend)
        except ValueError as exc:
            raise ValueError(f"{name}: {exc}") from exc
        return self.cube(bone, name, frm, to, rot=rot, org=org, mat=mat)

    # -- 导出 --------------------------------------------------------------
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

    def bbmodel(self, model_name: str, namespace: str | None = None) -> dict:
        return {
            "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
            "name": model_name,
            "model_identifier": model_name,
            "resolution": {"width": self.palette.size, "height": self.palette.size},
            "elements": self.elements,
            "outliner": self.outliner(),
            "textures": [{
                "path": "",
                "name": f"{model_name}.png",
                "folder": "",
                "namespace": namespace or workspace.default().namespace,
                "id": "0",
                "particle": True,
                "render_mode": "default",
                "visible": True,
                "mode": "bitmap",
                "saved": False,
                "uuid": str(uuid.uuid4()),
                "source": "data:image/png;base64," + self.palette.texture_b64(),
            }],
        }

    def save(self, path: Path, model_name: str) -> Path:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(self.bbmodel(model_name), ensure_ascii=False, indent=1))
        return path

    # -- 自检 --------------------------------------------------------------
    @staticmethod
    def corners(e: dict) -> list[Vec]:
        """element 的 8 个**旋转后**世界角点。R=Rz·Ry·Rx，绕自身 origin，与
        render_bbmodel.load_bbmodel 逐字一致。

        必须算旋转：柱状件在旋转前是一根沿 +Y 的竖高盒，直接读 from/to 量出来的
        站高、体宽、贴地全是假数（一根近水平的趾骨会谎报 2 单位的 y 跨度）。
        """
        f, t = e["from"], e["to"]
        raw = [(x, y, z) for x in (f[0], t[0]) for y in (f[1], t[1]) for z in (f[2], t[2])]
        rot = e.get("rotation") or (0.0, 0.0, 0.0)
        if not any(rot):
            return raw
        org = e.get("origin") or [0.0, 0.0, 0.0]
        rx, ry, rz = (math.radians(a) for a in rot)
        cx, sx = math.cos(rx), math.sin(rx)
        cy, sy = math.cos(ry), math.sin(ry)
        cz, sz = math.cos(rz), math.sin(rz)
        out = []
        for p in raw:
            x, y, z = (p[i] - org[i] for i in range(3))
            y, z = y * cx - z * sx, y * sx + z * cx          # Rx
            x, z = x * cy + z * sy, -x * sy + z * cy         # Ry
            x, y = x * cz - y * sz, x * sz + y * cz          # Rz
            out.append((x + org[0], y + org[1], z + org[2]))
        return out

    def bounds(self) -> tuple[list[float], list[float]]:
        """整体包围盒（含 element 旋转）。"""
        lo = [1e9, 1e9, 1e9]
        hi = [-1e9, -1e9, -1e9]
        for e in self.elements:
            for c in self.corners(e):
                for i in range(3):
                    lo[i] = min(lo[i], c[i])
                    hi[i] = max(hi[i], c[i])
        return lo, hi

    def lowest(self, n: int = 5) -> list[tuple[float, str]]:
        """最低的 n 个 element（含旋转）。贴地/穿地排查用——只报一个总最低点，
        排查时还得自己翻是哪块，不如直接列出来。"""
        rows = [(min(c[1] for c in self.corners(e)), e["name"]) for e in self.elements]
        return sorted(rows)[:n]

    def mirror_problems(self, tol: float = 0.02) -> list[str]:
        """左右镜像自检：`*_l` / `*_l_*` 必须有 x 取反、y/z 相等的 `_r` 对应件。

        目视核不出这类错——狮子那版首战就是左脚掌骨整体平移而非镜像（少乘一个 sx），
        三视图上完全看不出来。
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
                out.append(f"{name}: x 未镜像（{e['from'][0]}..{e['to'][0]} vs "
                           f"{m['from'][0]}..{m['to'][0]}）")
            for axis, label in ((1, "y"), (2, "z")):
                if abs(e["from"][axis] - m["from"][axis]) > tol or abs(e["to"][axis] - m["to"][axis]) > tol:
                    out.append(f"{name}: {label} 与镜像件不等")
            if abs(e["rotation"][0] - m["rotation"][0]) > tol:
                out.append(f"{name}: pitch 与镜像件不等（{e['rotation'][0]} vs {m['rotation'][0]}）")
            for axis, label in ((1, "yaw"), (2, "roll")):
                if abs(e["rotation"][axis] + m["rotation"][axis]) > tol:
                    out.append(f"{name}: {label} 未取反（{e['rotation'][axis]} vs {m['rotation'][axis]}）")
        return out

    def orphan_bones(self) -> list[str]:
        """既没挂 element、也没有子骨骼的骨——多半是写漏了。"""
        has_child_bone = {b["parent"] for b in self.bones.values() if b["parent"]}
        return [n for n, b in self.bones.items()
                if not b["children"] and n not in has_child_bone]
