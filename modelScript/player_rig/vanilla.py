"""原版玩家六部件（及外层）的几何与 UV，按 Yarn 1.20.1 ModelPart.Cuboid 的算法重建。

坐标统一换算为模板坐标（像素，脚底原点，Y 向上，前方 -Z，玩家右侧 -X）：
    x = pivotX + 模型 x，y = 24 - (pivotY + 模型 y)，z = pivotZ + 模型 z。
常量来自 PlayerEntityModel.getTexturedModelData(Dilation.NONE, slim) 的实际导出，
tests/vanilla_player_dump.json 保存该导出，test_vanilla.py 逐顶点比对本文件的重建结果。
"""

from __future__ import annotations

from dataclasses import dataclass

# 部件名 -> (u, v, 最小角相对枢轴, 尺寸, 膨胀)；枢轴按体型给出
_COMMON = {
    "head": (0, 0, (-4, -8, -4), (8, 8, 8), 0.0),
    "hat": (32, 0, (-4, -8, -4), (8, 8, 8), 0.5),
    "body": (16, 16, (-4, 0, -2), (8, 12, 4), 0.0),
    "jacket": (16, 32, (-4, 0, -2), (8, 12, 4), 0.25),
    "right_leg": (0, 16, (-2, 0, -2), (4, 12, 4), 0.0),
    "right_pants": (0, 32, (-2, 0, -2), (4, 12, 4), 0.25),
    "left_leg": (16, 48, (-2, 0, -2), (4, 12, 4), 0.0),
    "left_pants": (0, 48, (-2, 0, -2), (4, 12, 4), 0.25),
}
_ARM_UV = {"right_arm": (40, 16), "right_sleeve": (40, 32), "left_arm": (32, 48), "left_sleeve": (48, 48)}
_PIVOT = {
    "head": (0, 0, 0), "hat": (0, 0, 0), "body": (0, 0, 0), "jacket": (0, 0, 0),
    "right_leg": (-1.9, 12, 0), "right_pants": (-1.9, 12, 0),
    "left_leg": (1.9, 12, 0), "left_pants": (1.9, 12, 0),
}

#: 内层部件 -> 外层部件
OVERLAY = {
    "head": "hat", "body": "jacket", "right_arm": "right_sleeve", "left_arm": "left_sleeve",
    "right_leg": "right_pants", "left_leg": "left_pants",
}
FACES = ("north", "south", "east", "west", "up", "down")


@dataclass(frozen=True)
class Part:
    name: str
    u: int
    v: int
    pivot: tuple  # 模型空间枢轴（Y 向下）
    low: tuple  # 模板坐标最小角（未膨胀）
    size: tuple
    dilation: float

    @property
    def high(self):
        return tuple(round(a + b, 4) for a, b in zip(self.low, self.size))


def part(name: str, slim: bool) -> Part:
    if name in _ARM_UV:
        u, v = _ARM_UV[name]
        width = 3 if slim else 4
        side = -1 if name.startswith("right") else 1
        pivot = (5 * side, 2.5 if slim else 2.0, 0)
        # 右臂方块从枢轴向内（+X）延伸 1，向外延伸 width-1；左臂相反
        rel_x = -(width - 1) if side < 0 else -1
        dilation = 0.25 if name.endswith("sleeve") else 0.0
        rel = (rel_x, -2, -2)
        size = (width, 12, 4)
    else:
        u, v, rel, size, dilation = _COMMON[name]
        pivot = _PIVOT[name]
    low = (round(pivot[0] + rel[0], 4), round(24 - (pivot[1] + rel[1] + size[1]), 4), round(pivot[2] + rel[2], 4))
    return Part(name, u, v, pivot, low, size, dilation)


def model_quads(p: Part, extra: float | None = None):
    """返回 6 个四边形，顶点为 (x, y, z, u像素, v像素)，位置为模板坐标。

    extra 为膨胀；None 取部件自带值，0 取未膨胀方块（用于 UV 映射）。
    """
    e = p.dilation if extra is None else extra
    w, h, d = p.size
    rel = (p.low[0] - p.pivot[0], 24 - p.pivot[1] - p.low[1] - h, p.low[2] - p.pivot[2])  # 模型空间最小角
    x0, y0, z0 = rel[0] - e, rel[1] - e, rel[2] - e
    x1, y1, z1 = rel[0] + w + e, rel[1] + h + e, rel[2] + d + e
    v = [(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0),
         (x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)]
    u, vv = p.u, p.v
    i, j, k, l = u, u + d, u + d + w, u + d + w + w
    m, n = u + d + w + d, u + d + w + d + w
    o, pp, q = vv, vv + d, vv + d + h
    # (顶点序, u1, v1, u2, v2) 与 Yarn Cuboid 构造一致
    spec = [
        ((5, 1, 2, 6), k, pp, m, q),  # 东 (sides[0])
        ((0, 4, 7, 3), i, pp, j, q),  # 西 (sides[1])
        ((5, 4, 0, 1), j, o, k, pp),  # 模型 down (sides[2])
        ((2, 3, 7, 6), k, pp, l, o),  # 模型 up (sides[3])
        ((1, 0, 3, 2), j, pp, k, q),  # 北 (sides[4])
        ((4, 5, 6, 7), m, pp, n, q),  # 南 (sides[5])
    ]
    quads = []
    for order, u1, v1, u2, v2 in spec:
        corners = [(u2, v1), (u1, v1), (u1, v2), (u2, v2)]
        quad = []
        for index, (cu, cv) in zip(order, corners):
            mx, my, mz = v[index]
            quad.append((round(p.pivot[0] + mx, 4), round(24 - (p.pivot[1] + my), 4),
                         round(p.pivot[2] + mz, 4), float(cu), float(cv)))
        quads.append(quad)
    return quads


_AXIS = {"north": (2, -1), "south": (2, 1), "east": (0, 1), "west": (0, -1), "up": (1, 1), "down": (1, -1)}


def face_quad(p: Part, face: str):
    """取模板坐标下朝向 face 的原版四边形（未膨胀）。"""
    axis, sign = _AXIS[face]
    quads = model_quads(p, 0.0)
    target = p.high[axis] if sign > 0 else p.low[axis]
    found = [q for q in quads if all(abs(vx[axis] - target) < 1e-6 for vx in q)]
    if len(found) != 1:
        raise ValueError(f"{p.name} 找不到 {face} 面")
    return found[0]


def _affine(quad):
    """四边形的 (u, v) 各只随一个模板轴线性变化：返回 [(轴, 基点, 基值, 斜率)] x2。"""
    result = []
    for channel in (3, 4):
        found = None
        for base in quad:
            for other in quad:
                if abs(other[channel] - base[channel]) < 1e-9:
                    continue
                axes = [a for a in range(3) if abs(other[a] - base[a]) > 1e-9]
                if len(axes) == 1:
                    found = (base, other, axes[0])
                    break
            if found:
                break
        if not found:
            raise ValueError("原版 UV 不是轴对齐映射")
        base, other, axis = found
        slope = (other[channel] - base[channel]) / (other[axis] - base[axis])
        result.append((axis, base[axis], base[channel], slope))
    return result


def uv_at(p: Part, face: str, point) -> tuple:
    """原版该面在模板坐标点 point 处的皮肤像素坐标 (u, v)。"""
    (au, bu, vu, su), (av, bv, vv, sv) = _affine(face_quad(p, face))
    return (vu + su * (point[au] - bu), vv + sv * (point[av] - bv))
