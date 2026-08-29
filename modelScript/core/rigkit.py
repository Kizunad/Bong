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
import sys
from pathlib import Path

import base64
import io
import math
import uuid
from collections.abc import Mapping
from typing import TypeAlias

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import workspace  # noqa: E402

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


def _euler_zyx(m: list[list[float]]) -> tuple[float, float, float]:
    """从旋转矩阵解出 Blockbench 的 (x, y, z) 度（R = Rz·Ry·Rx）。

    万向锁附近把 z 固定为 0 再解 x —— 该处 x 与 z 本就简并，硬解出来的那一对角度会在
    相邻件之间乱跳。
    """
    sy = max(-1.0, min(1.0, -m[2][0]))
    if math.sqrt(max(0.0, 1.0 - sy * sy)) < 1e-6:
        return (math.degrees(math.atan2(-m[1][2], m[1][1])), math.degrees(math.asin(sy)), 0.0)
    return (math.degrees(math.atan2(m[2][1], m[2][2])),
            math.degrees(math.asin(sy)),
            math.degrees(math.atan2(m[1][0], m[0][0])))


def shaft_box(a: Vec, b: Vec, rx: float, rz: float, extend: float = 0.0, up: Vec | None = None):
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
    cx, cy, cz = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2)
    if up is None:
        pitch = math.degrees(math.acos(max(-1.0, min(1.0, dy / length))))
        yaw = math.degrees(math.atan2(dx, dz)) if abs(dx) + abs(dz) > 1e-9 else 0.0
        rot = (pitch, yaw, 0.0)
    else:
        # up = 想让**局部 +Z（薄的那一维）**指向的方向，通常是所在曲面的法线。
        #
        # 不给 up 时公式只出 (pitch, yaw, 0)，没有滚转 —— 局部 x 恒落在水平面内，扁板
        # 永远**平摆**。翼面有上反角（每 1.14 单位升 0.16）时，平摆的宽板一块块错开，
        # 从正后方看就是一段楼梯，而不是一个斜面。给了 up 才能把板转进曲面里。
        ya = (dx / length, dy / length, dz / length)
        d = sum(up[i] * ya[i] for i in range(3))
        za = [up[i] - ya[i] * d for i in range(3)]
        n = math.sqrt(sum(c * c for c in za))
        if n < 1e-4:                       # up 与骨轴共线，退回无滚转
            return shaft_box(a, b, rx, rz, extend)
        za = [c / n for c in za]
        xa = [ya[1] * za[2] - ya[2] * za[1],
              ya[2] * za[0] - ya[0] * za[2],
              ya[0] * za[1] - ya[1] * za[0]]
        rot = _euler_zyx([[xa[i], ya[i], za[i]] for i in range(3)])
    return (
        (cx - rx, cy - half, cz - rz),
        (cx + rx, cy + half, cz + rz),
        rot,
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


def _rotmat3(rot) -> list[list[float]]:
    """R = Rz·Ry·Rx（与 Blockbench / render_bbmodel 一致）。"""
    rx, ry, rz = (math.radians(v) for v in rot)
    cx, sx = math.cos(rx), math.sin(rx)
    cy, sy = math.cos(ry), math.sin(ry)
    cz, sz = math.cos(rz), math.sin(rz)
    return [
        [cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx],
        [sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx],
        [-sy, cy * sx, cy * cx],
    ]


def _matmul(a: list[list[float]], b: list[list[float]]) -> list[list[float]]:
    return [[sum(a[i][k] * b[k][j] for k in range(3)) for j in range(3)] for i in range(3)]


def _mul(R: list[list[float]], v, t) -> tuple[float, float, float]:
    return tuple(sum(R[i][j] * v[j] for j in range(3)) + t[i] for i in range(3))


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

    def bbmodel(self, model_name: str, namespace: str | None = None) -> dict:
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
                    "namespace": namespace or workspace.default().namespace,
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

    def add_bone(self, name: str, pivot: Vec, parent: str, rot: Vec = (0.0, 0.0, 0.0)) -> str:
        """往已读进来的骨树里插一根新骨（羽自带骨用）。

        带 rot 的骨是**绑定姿旋转**：Blockbench 把它和动画旋转**逐分量相加**后再解成矩阵。
        所以把一根羽的轴向烙进它自己的骨里之后，"这根羽在展翼时该转到哪"就退化成两份模型
        的绑定角相减 —— 不用解矩阵，也不会踩欧拉分解的多解。
        """
        if name in self.nodes:
            raise ValueError(f"重复骨骼: {name}")
        if parent not in self.nodes:
            raise KeyError(f"骨架里没有骨骼 {parent}")
        node = {
            "name": name,
            "origin": [round(v, 4) for v in pivot],
            "rotation": [round(v, 4) for v in rot],
            "uuid": str(uuid.uuid4()),
            "export": True,
            "mirror_uv": False,
            "isOpen": False,
            "locked": False,
            "visibility": True,
            "autouv": 0,
            "children": [],
        }
        self.nodes[parent]["children"].append(node)
        self.nodes[name] = node
        self.pivots[name] = tuple(pivot)
        return name

    def attach(self, bone: str, element: dict) -> None:
        if bone not in self.nodes:
            raise KeyError(f"骨架里没有骨骼 {bone}")
        self.data["elements"].append(element)
        self.nodes[bone]["children"].append(element["uuid"])

    def baked_elements(self) -> list[dict]:
        """把每个 element 的坐标从**骨局部系**烘到世界系（绑定姿）。

        quill 出来的羽件 from/to 存的是骨局部坐标、rotation 归零，朝向烙在骨的绑定旋转
        里。任何直接读 element 的东西（自检的镜像/贴地/连通/漏光、render_bbmodel 的默认
        路径）拿到的都是"一根根竖着的板"，不是真几何 —— 会静悄悄地全部通过，等于对整片
        翼失明。所有按坐标判事的地方都必须先过这一道。
        """
        out: list[dict] = []
        by_uuid = {e["uuid"]: e for e in self.data["elements"]}

        def walk(node, R, t):
            if isinstance(node, str):
                e = by_uuid.get(node)
                if e is None:
                    return
                o = e.get("origin") or _center(e["from"], e["to"])
                o2 = _mul(R, o, t)
                Re = _rotmat3(e.get("rotation") or (0.0, 0.0, 0.0))
                d = [o2[i] - o[i] for i in range(3)]
                out.append({**e,
                            "from": [e["from"][i] + d[i] for i in range(3)],
                            "to": [e["to"][i] + d[i] for i in range(3)],
                            "origin": list(o2),
                            "rotation": list(_euler_zyx(_matmul(R, Re)))})
                return
            meta = self.groups.get(node["uuid"], node)
            g = _rotmat3(meta.get("rotation") or (0.0, 0.0, 0.0))
            piv = meta.get("origin") or (0.0, 0.0, 0.0)
            # 骨的局部仿射：绕自己的 pivot 转，再接到父的变换上
            R2 = _matmul(R, g)
            t2 = _mul(R, [piv[i] - sum(g[i][j] * piv[j] for j in range(3)) for i in range(3)], t)
            for c in node.get("children", []):
                walk(c, R2, t2)

        eye = [[1.0 if i == j else 0.0 for j in range(3)] for i in range(3)]
        for root in self.data["outliner"]:
            walk(root, eye, (0.0, 0.0, 0.0))
        return out

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

        sys.path.insert(0, str(Path(__file__).resolve().parents[0]))
        from to_fmt410 import ensure_410

        self.data["name"] = name
        self.data["model_identifier"] = name
        # 强制 4.10 落盘：本类是"读一份 bbmodel → 挂新件 → 写回"，格式版本跟着源文件走。
        # 骨架一旦被 Blockbench 5 手工存过盘，肌肉/皮毛层的产物就悄悄变成 5.0，而 5.0 在
        # 4.x 里打开是一个 cube 都看不见（见 to_fmt410 的说明）——不报错，只是空场景。
        Path(out).write_text(json.dumps(ensure_410(self.data), ensure_ascii=False, indent=1))


def bake_file(src, dst) -> str:
    """读一份 bbmodel，把坐标烘到世界系后另存 —— 给**只读 elements** 的工具用。

    render_bbmodel 默认不读 outliner，所以自带骨的羽（quill）在它眼里是一根根竖着的板。
    出预览图、做漏光检查这类"看图判事"的地方必须先过这一道，否则看的根本不是这只鸟。
    骨架层与肌肉层没有带绑定旋转的骨，烘焙是恒等变换，照走无妨。
    """
    import json
    from pathlib import Path

    sk = Skeleton(src)
    doc = dict(sk.data)
    doc["elements"] = sk.baked_elements()
    doc["outliner"] = [e["uuid"] for e in doc["elements"]]
    Path(dst).write_text(json.dumps(doc, ensure_ascii=False))
    return str(dst)


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
              *, mat: str = "muscle", up: Vec | None = None) -> None:
        """等截面的一段（腱、膜条）。up 见 shaft_box：给了才有滚转，扁板才能贴着曲面转。"""
        frm, to, rot, org = shaft_box(a, b, rx, rx if rz is None else rz, up=up)
        self.piece(bone, name, frm, to, rot=rot, org=org, mat=mat)

    def quill(self, parent: str, name: str, a: Vec, b: Vec, rx: float, rz: float | None = None,
              *, mat: str = "muscle", bone: str | None = None, up: Vec | None = None) -> str:
        """一根**自带骨**的羽：骨的 pivot 落在羽根、绑定旋转烙住羽轴，元素换算进这根骨的
        坐标系（于是它退化成一个从 pivot 沿局部 +Y 伸出去的正方盒，rotation 归零）。

        为什么要这么摆：收翼↔展翼之间每根羽的朝向和长度都不同，只有把羽轴变成骨的局部
        +Y，"绕羽根转"才是纯旋转、"变长"才是 scale=(1,k,1)。元素若仍带自己的 rotation，
        scale 会沿世界轴拉，羽会被拉歪成一把斜刀。

        世界几何与直接 strut 完全一致（骨的绑定旋转把它转了回去），所以已过审的静态外观
        一个字节不动 —— 这一点由 check 里的世界坐标对拍守着。
        """
        rz = rx if rz is None else rz
        frm, to, rot, org = shaft_box(a, b, rx, rz, up=up)
        bone = bone or name
        if bone in self.skel.nodes:
            pivot = self.skel.pivots[bone]   # 复用同一根羽的骨（羽尖压色那一段）
        else:
            pivot = a
            self.skel.add_bone(bone, a, parent, rot)
        # δ = (pivot − 盒心) + R⁻¹(盒心 − pivot)；后者沿羽轴，所以只有 +Y 分量
        d = (pivot[0] - org[0],
             pivot[1] - org[1] + math.dist(org, pivot),
             pivot[2] - org[2])
        self.piece(bone, name,
                   tuple(f + o for f, o in zip(frm, d)),
                   tuple(t + o for t, o in zip(to, d)),
                   org=pivot, mat=mat)
        return bone

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
