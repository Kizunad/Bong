"""25 骨 + 2 挂点的切分表（模板坐标：像素，脚底原点，Y 向上，前 -Z，右 -X）。"""

from __future__ import annotations

from dataclasses import dataclass, field

from . import vanilla

BONE_ORDER = (
    "root", "pelvis", "waist", "chest", "head",
    "r_upper_arm", "r_forearm", "r_hand", "r_thumb", "r_fingers_a", "r_fingers_b", "r_item",
    "l_upper_arm", "l_forearm", "l_hand", "l_thumb", "l_fingers_a", "l_fingers_b", "l_item",
    "r_thigh", "r_shin", "r_foot", "r_toe", "l_thigh", "l_shin", "l_foot", "l_toe",
)
PARENT = {
    "root": None, "pelvis": "root", "waist": "pelvis", "chest": "waist", "head": "chest",
    "r_thigh": "pelvis", "l_thigh": "pelvis",
}
for _s in "rl":
    PARENT.update({
        f"{_s}_upper_arm": "chest", f"{_s}_forearm": f"{_s}_upper_arm", f"{_s}_hand": f"{_s}_forearm",
        f"{_s}_thumb": f"{_s}_hand", f"{_s}_fingers_a": f"{_s}_hand", f"{_s}_fingers_b": f"{_s}_hand",
        f"{_s}_item": f"{_s}_hand", f"{_s}_shin": f"{_s}_thigh", f"{_s}_foot": f"{_s}_shin",
        f"{_s}_toe": f"{_s}_foot",
    })
#: 动画可写轨道的 25 根骨（不含挂点）
JOINTS = tuple(b for b in BONE_ORDER if not b.endswith("_item"))

# 原版 HeldItemFeatureRenderer 在手臂局部空间的物品原点（像素，模型空间 Y 向下）：
#   translate(±1/16, 0.125, -0.625) 经 Rx(-90°)·Ry(180°) 后为 (∓1, 10, -2)。
ITEM_OFFSET_FROM_SHOULDER = {"r": (-1.0, 10.0, -2.0), "l": (1.0, 10.0, -2.0)}


@dataclass
class Cube:
    low: tuple
    high: tuple
    outer: bool
    skin_part: str  # 对应的原版部件名（内层或外层）
    base_part: str  # 内层部件名，用于取原版几何
    faces: dict = field(default_factory=dict)  # face -> [u1, v1, u2, v2]


@dataclass
class Bone:
    name: str
    parent: str | None
    pivot: tuple
    cubes: list = field(default_factory=list)


def _box(lo, hi):
    return tuple(round(v, 4) for v in lo), tuple(round(v, 4) for v in hi)


def _segments(slim: bool):
    """骨名 -> (原版内层部件, 方块范围)；整像素切口，范围拼起来恰为原版部件。"""
    seg = {
        "head": ("head", _box((-4, 24, -4), (4, 32, 4))),
        "pelvis": ("body", _box((-4, 12, -2), (4, 16, 2))),
        "waist": ("body", _box((-4, 16, -2), (4, 20, 2))),
        "chest": ("body", _box((-4, 20, -2), (4, 24, 2))),
    }
    top = 23.5 if slim else 24.0
    width = 3 if slim else 4
    for s, sign in (("r", -1), ("l", 1)):
        x_in, x_out = 4 * sign, (4 + width) * sign
        x0, x1 = sorted((x_in, x_out))
        arm = f"{'right' if s == 'r' else 'left'}_arm"
        seg[f"{s}_upper_arm"] = (arm, _box((x0, top - 5, -2), (x1, top, 2)))
        seg[f"{s}_forearm"] = (arm, _box((x0, top - 9, -2), (x1, top - 5, 2)))
        seg[f"{s}_hand"] = (arm, _box((x0, top - 10, -2), (x1, top - 9, 2)))
        seg[f"{s}_thumb"] = (arm, _box((x0, top - 12, -2), (x1, top - 10, -1)))
        seg[f"{s}_fingers_a"] = (arm, _box((x0, top - 12, -1), (x1, top - 10, 1)))
        seg[f"{s}_fingers_b"] = (arm, _box((x0, top - 12, 1), (x1, top - 10, 2)))
        lx0, lx1 = (-3.9, 0.1) if s == "r" else (-0.1, 3.9)
        leg = f"{'right' if s == 'r' else 'left'}_leg"
        seg[f"{s}_thigh"] = (leg, _box((lx0, 6, -2), (lx1, 12, 2)))
        seg[f"{s}_shin"] = (leg, _box((lx0, 1, -2), (lx1, 6, 2)))
        seg[f"{s}_foot"] = (leg, _box((lx0, 0, 0), (lx1, 1, 2)))
        seg[f"{s}_toe"] = (leg, _box((lx0, 0, -2), (lx1, 1, 0)))
    return seg


def _pivots(slim: bool):
    top = 23.5 if slim else 24.0
    cx = 5.5 if slim else 6.0  # 肘、腕、手指绕方块中心
    pv = {
        "root": (0, 0, 0), "pelvis": (0, 12, 0), "waist": (0, 16, 0), "chest": (0, 20, 0), "head": (0, 24, 0),
    }
    for s, sign in (("r", -1), ("l", 1)):
        pv.update({
            f"{s}_upper_arm": (5 * sign, top - 2, 0),  # 原版 PlayerEntityModel 肩枢轴
            f"{s}_forearm": (cx * sign, top - 5, 0),
            f"{s}_hand": (cx * sign, top - 9, 0),
            f"{s}_thumb": (cx * sign, top - 10, -1.5),
            f"{s}_fingers_a": (cx * sign, top - 10, 0),
            f"{s}_fingers_b": (cx * sign, top - 10, 1.5),
            f"{s}_thigh": (1.9 * sign, 12, 0), f"{s}_shin": (1.9 * sign, 6, 0),
            f"{s}_foot": (1.9 * sign, 1, 0), f"{s}_toe": (1.9 * sign, 0, 0),
        })
        # 挂点：肩枢轴 + 原版物品原点偏移（模型空间 Y 向下 -> 模板 Y 向上取反）
        ox, oy, oz = ITEM_OFFSET_FROM_SHOULDER[s]
        shoulder = pv[f"{s}_upper_arm"]
        pv[f"{s}_item"] = (shoulder[0] + ox, shoulder[1] - oy, shoulder[2] + oz)
    return {k: tuple(round(v, 4) for v in p) for k, p in pv.items()}


def _face_uv(base_part: vanilla.Part, face: str, low, high):
    """按读取器的逐面坐标约定，取该面两个对角的原版皮肤坐标。"""
    # 与 Skeleton/SkinnedMesh 的 unit 约定一致：(0,0) 角与 (1,1) 角
    corners = {
        "north": ((high[0], high[1], low[2]), (low[0], low[1], low[2])),
        "south": ((low[0], high[1], high[2]), (high[0], low[1], high[2])),
        "west": ((low[0], high[1], low[2]), (low[0], low[1], high[2])),
        "east": ((high[0], high[1], high[2]), (high[0], low[1], low[2])),
        "up": ((low[0], high[1], low[2]), (high[0], high[1], high[2])),
        "down": ((low[0], low[1], high[2]), (high[0], low[1], low[2])),
    }[face]
    u1, v1 = vanilla.uv_at(base_part, face, corners[0])
    u2, v2 = vanilla.uv_at(base_part, face, corners[1])
    return [round(u1, 4), round(v1, 4), round(u2, 4), round(v2, 4)]


def build(slim: bool) -> list[Bone]:
    segments = _segments(slim)
    pivots = _pivots(slim)
    bones = []
    for name in BONE_ORDER:
        bone = Bone(name, PARENT[name], pivots.get(name, (0, 0, 0)))
        if name in segments:
            base_name, (low, high) = segments[name]
            base = vanilla.part(base_name, slim)
            for outer in (False, True):
                skin_name = vanilla.OVERLAY[base_name] if outer else base_name
                skin = vanilla.part(skin_name, slim)
                cube = Cube(low, high, outer, skin_name, base_name)
                for face, (axis, sign) in vanilla._AXIS.items():
                    # 只保留落在原版部件外表面上的面；切口面不画，由模组蒙皮连接
                    plane = high[axis] if sign > 0 else low[axis]
                    edge = base.high[axis] if sign > 0 else base.low[axis]
                    if abs(plane - edge) < 1e-6:
                        cube.faces[face] = _face_uv(skin, face, low, high)
                bone.cubes.append(cube)
        bones.append(bone)
    return bones
