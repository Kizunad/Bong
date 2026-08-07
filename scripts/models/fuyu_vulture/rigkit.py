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


def normalize(v: Vec) -> Vec:
    n = math.sqrt(sum(c * c for c in v)) or 1.0
    return (v[0] / n, v[1] / n, v[2] / n)


def perp_to(a: Vec, b: Vec, ref: Vec) -> Vec:
    """骨轴 a→b 的「岔开方向」单位向量：把参考向量 ref 对骨轴做正交化。

    并排的两根骨（桡/尺）、骨两侧的一对肌（伸腕/屈腕）都靠它岔开；沿同一条线摆
    就会重叠成一根。

    用正交化而不是叉积：叉积是赝矢量，左右镜像时结果**不**跟着 x 取反，两侧会各岔
    各的。正交化只要 ref 本身 x 分量为 0，结果就天然镜像对称——这条约束在这里
    强制检查，因为违反它的症状（左右不对称）在三视图上几乎看不出来。
    """
    if abs(ref[0]) > 1e-9:
        raise ValueError(f"参考向量 x 分量须为 0（否则左右不镜像），收到 {ref}")
    d = normalize((b[0] - a[0], b[1] - a[1], b[2] - a[2]))
    dot = sum(ref[i] * d[i] for i in range(3))
    v = tuple(ref[i] - d[i] * dot for i in range(3))
    m = math.sqrt(sum(c * c for c in v))
    if m < 1e-4:  # 骨轴与参考共线，退化到另一个轴
        return normalize((0.0, ref[2], -ref[1]))
    return (v[0] / m, v[1] / m, v[2] / m)


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
        return element_bounds(self.elements)

    def mirror_violations(self, tol: float = 0.02) -> list[str]:
        return mirror_violations(self.elements, tol)


# ================================================================ 元素级工具
def element_bounds(elements: list[dict]) -> tuple[Vec, Vec]:
    """一组 element **旋转后**的真实包围盒。

    直接取 from/to 的极值是错的：斜置的骨干（趾骨、肋弓、喙）在局部坐标里是一根
    沿 +Y 的长柱，from[1] 落在离实际端点很远的地方 —— 量出来的"贴地"和"全长"
    会凭空差出半根骨头，而这些数字正是逐轮调参的唯一依据。
    """
    if not elements:
        raise ValueError("空元素集，量不出包围盒")
    lo = [math.inf] * 3
    hi = [-math.inf] * 3
    for e in elements:
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


def mirror_violations(elements: list[dict], tol: float = 0.02) -> list[str]:
    """左右件必须 x 取反、y/z 相等；不带 _l/_r 的件必须**自身**关于中线对称。

    目视核不出这些 —— 一侧的一串件整体平移（漏乘 sx）而非镜像，三视图上完全看不
    出来，只有对拍能抓。中线件更阴险：它没有配对方，光查左右配对根本照不到它，
    写偏了就是一块孤零零飘在身旁的方块。
    """
    els = {e["name"]: e for e in elements}
    out: list[str] = []
    for name, e in els.items():
        if not (name.endswith(("_l", "_r")) or "_l_" in name or "_r_" in name):
            if abs(e["from"][0] + e["to"][0]) > tol:
                out.append(f"{name}: 中线件未对称（x {e['from'][0]}..{e['to'][0]}，"
                           f"中心 {(e['from'][0] + e['to'][0]) / 2:.2f} 应为 0）")
            continue
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


# ================================================================ 读已有骨架
class Skeleton:
    """读一份 .bbmodel，供上层往里挂新 element（肌肉 / 皮毛层用）。

    **读文件**而不是直接调生成器：骨架一旦被 Blockbench 手工精修过（fmt 5.0 存盘），
    重跑生成器就会把那些改动冲掉。上层只认文件，骨架怎么来的与它无关。

    兼容 fmt 4.x（outliner 内联 group）与 fmt 5.0（groups 数组 + uuid 引用树）。
    """

    def __init__(self, path) -> None:
        import json
        from pathlib import Path

        self.path = Path(path)
        self.data = json.loads(self.path.read_text())
        self.groups = {g["uuid"]: g for g in self.data.get("groups", [])}
        self.nodes: dict[str, dict] = {}  # 骨骼名 → outliner 节点（用于挂 cube）
        self.pivots: dict[str, Vec] = {}
        for root in self.data["outliner"]:
            self._walk(root)
        if not self.pivots:
            raise SystemExit(f"{self.path}: 读不到骨骼层级")
        self.by_name = {e["name"]: e for e in self.data["elements"]}

    def _walk(self, node) -> None:
        if isinstance(node, str):  # element uuid 叶子
            return
        meta = self.groups.get(node["uuid"], node)
        name = meta.get("name")
        if name is not None:
            self.nodes[name] = node
            self.pivots[name] = tuple(meta["origin"])
        for child in node.get("children", []):
            self._walk(child)

    def P(self, bone: str) -> Vec:
        if bone not in self.pivots:
            raise KeyError(f"骨架里没有骨骼 {bone}（现有 {len(self.pivots)} 根）")
        return self.pivots[bone]

    def box(self, prefix: str) -> tuple[Vec, Vec]:
        """名字以 prefix 开头的那组骨块的包围盒。

        龙骨突、胸骨这类附着面的位置要从**骨架实际几何**取，不能在肌肉层重算一遍
        —— 两处各算各的，骨架一改肌肉就浮空了。
        """
        hit = [e for n, e in self.by_name.items() if n.startswith(prefix)]
        if not hit:
            raise KeyError(f"骨架里没有名字以 {prefix} 开头的骨块")
        return element_bounds(hit)

    def attach(self, bone: str, element: dict) -> None:
        if bone not in self.nodes:
            raise KeyError(f"骨架里没有骨骼 {bone}")
        self.data["elements"].append(element)
        self.nodes[bone]["children"].append(element["uuid"])

    def added(self, flag: str = "_muscle") -> list[dict]:
        return [e for e in self.data["elements"] if e.get(flag)]

    def keep_only_added(self, flag: str = "_muscle") -> None:
        """--only-muscle：摘掉原骨骼 cube，只留新挂上去的。"""
        keep = {e["uuid"] for e in self.data["elements"] if e.get(flag)}
        self.data["elements"] = [e for e in self.data["elements"] if e["uuid"] in keep]

        def prune(node):
            if isinstance(node, str):
                return node in keep
            node["children"] = [c for c in node.get("children", []) if prune(c)]
            return True

        for root in self.data["outliner"]:
            prune(root)

    def extend_texture(self, mats: Mapping[str, RGB], row: int, swatch: int = 8) -> None:
        """在原贴图上追加一行色块（不动已有色块，读进来的骨骼 UV 保持有效）。"""
        src = self.data["textures"][0]["source"].split(",", 1)[1]
        img = Image.open(io.BytesIO(base64.b64decode(src))).convert("RGBA")
        px = img.load()
        if (row + 1) * swatch > img.height:
            raise ValueError(f"贴图只有 {img.height}px 高，放不下第 {row} 行色块")
        for i, (_name, (r, g, b)) in enumerate(mats.items()):
            ox, oy = i * swatch, row * swatch
            for y in range(swatch):
                for x in range(swatch):
                    n = ((x * 7 + y * 13 + i * 5) % 3) - 1  # 轻噪，肌面不是平涂塑料
                    px[ox + x, oy + y] = (
                        max(0, min(255, r + n * 4)),
                        max(0, min(255, g + n * 3)),
                        max(0, min(255, b + n * 3)),
                        255,
                    )
        buf = io.BytesIO()
        img.save(buf, format="PNG")
        self.data["textures"][0]["source"] = (
            "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()
        )

    def write(self, out, name: str) -> None:
        import json
        import sys
        from pathlib import Path

        sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
        from to_fmt410 import ensure_410

        self.data["name"] = name
        self.data["model_identifier"] = name
        # 强制 4.10 落盘：本类是"读一份 bbmodel → 挂新件 → 写回"，格式版本跟着源文件走。
        # 骨架一旦被 Blockbench 5 手工存过盘，肌肉/皮毛层的产物就悄悄变成 5.0，而 5.0 在
        # 4.x 里打开是一个 cube 都看不见（见 to_fmt410 的说明）——不报错，只是空场景。
        Path(out).write_text(json.dumps(ensure_410(self.data), ensure_ascii=False, indent=1))


class SoftTissue:
    """往 Skeleton 上挂软组织（肌腹 / 腱 / 膜）。UV 指向贴图的指定行。"""

    def __init__(self, skel: Skeleton, mats: Mapping[str, RGB], row: int, swatch: int = 8) -> None:
        self.skel = skel
        self.mats = tuple(mats)
        self.row = row
        self.swatch = swatch
        self.count = 0

    def _faces(self, mat: str) -> dict:
        if mat not in self.mats:
            raise ValueError(f"未知软组织材质: {mat}")
        ox, oy = self.mats.index(mat) * self.swatch, self.row * self.swatch
        uv = [ox + 1.0, oy + 1.0, ox + self.swatch - 1.0, oy + self.swatch - 1.0]
        return {d: {"uv": list(uv), "texture": 0} for d in ("north", "south", "east", "west", "up", "down")}

    def piece(self, bone: str, name: str, frm: Vec, to: Vec, *, rot=None, org=None, mat: str = "muscle") -> None:
        f = [round(min(a, b), 3) for a, b in zip(frm, to)]
        t = [round(max(a, b), 3) for a, b in zip(frm, to)]
        self.skel.attach(bone, {
            "name": name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "render_order": "default",
            "allow_mirror_modeling": True,
            "type": "cube",
            "uuid": str(uuid.uuid4()),
            "_muscle": True,
            "_mat": mat,  # 自检按材质判类，别再靠名字白名单（漏一项就静默放行/误杀）
            "from": f,
            "to": t,
            "autouv": 0,
            "color": 4,
            "origin": [round(v, 3) for v in (org or _center(f, t))],
            "rotation": [round(v, 3) for v in (rot or (0.0, 0.0, 0.0))],
            "faces": self._faces(mat),
        })
        self.count += 1

    def strut(self, bone: str, name: str, a: Vec, b: Vec, rx: float, rz: float | None = None,
              *, mat: str = "muscle") -> None:
        """等截面的一段（腱、膜条）。"""
        frm, to, rot, org = shaft_box(a, b, rx, rx if rz is None else rz)
        self.piece(bone, name, frm, to, rot=rot, org=org, mat=mat)

    def belly(self, bone: str, name: str, a: Vec, b: Vec, r_mid: float, *,
              r_end: float | None = None, mat: str = "muscle", flat: float = 1.0) -> None:
        """梭形肌腹：沿 a→b 分 3 段，中段最粗。

        单段直筒看着像水管 —— 肌肉两端收进腱，中间鼓起来，这个形状是"活体感"的来源。
        flat<1 = 扁片（阔肌一类）。
        """
        r_end = r_mid * 0.5 if r_end is None else r_end
        cuts = (0.0, 0.26, 0.74, 1.0)
        radii = ((r_end + r_mid) / 2, r_mid, (r_mid + r_end) / 2)
        for i, r in enumerate(radii):
            p0 = _mix(a, b, cuts[i])
            p1 = _mix(a, b, cuts[i + 1])
            frm, to, rot, org = shaft_box(p0, p1, r * flat, r)
            self.piece(bone, f"{name}_{i + 1}", frm, to, rot=rot, org=org, mat=mat)


def _mix(a: Vec, b: Vec, t: float) -> Vec:
    return (lerp(a[0], b[0], t), lerp(a[1], b[1], t), lerp(a[2], b[2], t))
