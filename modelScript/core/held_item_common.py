#!/usr/bin/env python3
"""手持物（武器 / 工具 / 盾）的 bbmodel + SML OBJ 双出公共层。

和护甲的 `armor_model_common` 对位，但目标链路不同：

    护甲   box 表 → bbmodel（离线设计）+ ArmorPartModel.CUBE_TABLES（运行时真相）
    手持物 box 表 → bbmodel（离线设计）+ OBJ/MTL/16² 贴图（运行时真相，SML 加载）

**一份 box 表同时出两种产物**，这是本模块存在的理由。此前手持物是两套各写一份：
bbmodel 在 `modelScript/generators/gen_*_shield.py`，OBJ 在
`client/tools/gen_shield_models.py`，两边的坐标靠人肉对齐——盾牌那两件至今
bbmodel 和 OBJ 的 boss 厚度就对不上。这里合成一个源头，从结构上去掉这个失同步点。

渲染链（见 `BongWeaponModelRegistry` / `WeaponRenderBootstrap`）：
    server 下发 template_id → client 合成宿主 vanilla item 的 fake stack
    → 该宿主的 item model JSON 被 SML 劫持到 `bong:models/item/<id>/<id>.obj`
    → 显示 3D 模型

## 坐标约定：**授权系 ≠ 出料系**

授权（box 表）用「握把末端在 y=0、尖端朝 +Y」，`assert_conventions` 会查——这套
读写都顺手。但**出料（OBJ / bbmodel）必须移进方块盒**，因为 MC 的 display 变换
是绕**方块中心**转的，不是绕模型原点：

    ItemRenderer.renderItem:  display 变换之后 translate(-0.5,-0.5,-0.5)
    SML ObjUnbakedModelModel.emitVertex:  只做「-0.5 → blockstate 旋转 → +0.5」，
                                          **不重定心**

所以 OBJ 的 (0,0,0) 落在**方块角**，而 display 的 rotation/translation/scale 全部
以 (0.5,0.5,0.5)（= 8px）为原点。授权系直接出料的话，模型等于挂在离枢轴半个方块
远的角上：TP 里刀飘在拳头外（实测 6.3px，一个拳头才 4px 宽），GUI 里图标被推到
格子左下角。**这不是 display 数值没调好，是差了一整个 0.5 方块的系统性偏移。**

`emit_offset()` 因此把出料整体挪成「**握把点落在方块中心**」：

    emit = (0.5 - 0, 0.5 - grip, 0.5 - 0)   # x/z 授权时就在 0 附近

这样 display 变换的枢轴就是**握把本身**——调手持姿态时绕握把转，正是想要的语义；
GUI/ground/fixed 也一并落回格子中心，不用每个模式各配一套补偿平移。

## UV 约定

OBJ 那条链是**每个面整张贴图铺满**（`_VT` 四角恒为 0,0..1,1），一个 material
一张 16² 图。bbmodel 这边为了长得一样，把各 material 的 16² 图拼成一张图集，
每个面的 uv 取该 material 的整块 tile。两边因此像素级一致。

副作用：贴图会按面拉伸，所以每张图必须画成**通用材质样本**（木纹 / 石片 /
锈斑 / 骨纹 / 绳纹），不能画成"某个面的具体图案"。
"""

from __future__ import annotations

import sys

import base64
import io
import json
import math
import random
import uuid
from dataclasses import dataclass
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import workspace  # noqa: E402

TILE = 16                       # 每个 material 一张 16² 图
MODEL_NAMESPACE = uuid.UUID("2f0f1a7c-6b3e-4d21-9a55-0c9d7e4b8f13")

# ── OBJ 几何常量：和 axe_bone.obj / bone_shield.obj 同构 ────────────────────
# 共享 4 角 UV + 6 面法线，每 box 8 verts。
_VT = ((0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0))
_VN = (
    (0.0, 0.0, 1.0),    # 1 +Z front
    (0.0, 0.0, -1.0),   # 2 -Z back
    (-1.0, 0.0, 0.0),   # 3 -X left
    (1.0, 0.0, 0.0),    # 4 +X right
    (0.0, 1.0, 0.0),    # 5 +Y top
    (0.0, -1.0, 0.0),   # 6 -Y bottom
)
# 每面 4 个本地顶点序（CCW，外法线）+ 该面法线序号
_FACES = (
    ((4, 5, 6, 7), 1),
    ((1, 0, 3, 2), 2),
    ((0, 4, 7, 3), 3),
    ((5, 1, 2, 6), 4),
    ((7, 6, 2, 3), 5),
    ((0, 1, 5, 4), 6),
)
# bbmodel 的面名 → OBJ 法线序（用来给 bbmodel 排同一套朝向）
_BB_FACES = ("north", "south", "west", "east", "up", "down")


@dataclass(frozen=True)
class Box:
    """一个轴对齐盒。center/half 用模型空间单位（1.0 = 16px）。"""

    name: str
    material: str
    center: tuple[float, float, float]
    half: tuple[float, float, float]

    @property
    def low(self) -> tuple[float, float, float]:
        return tuple(self.center[i] - self.half[i] for i in range(3))

    @property
    def high(self) -> tuple[float, float, float]:
        return tuple(self.center[i] + self.half[i] for i in range(3))


@dataclass(frozen=True)
class Material:
    """一个 material = 一张 16² 贴图 + 一个 MTL 条目。"""

    name: str
    kd: tuple[float, float, float]      # MTL 漫反射（无贴图时的兜底色）
    texture: Image.Image


@dataclass(frozen=True)
class HeldItem:
    key: str                            # = template_id，也是资源目录名
    display_name: str
    host_item: str                      # 宿主 vanilla item 的注册名（其 model JSON 被劫持）
    boxes: tuple[Box, ...]
    materials: tuple[Material, ...]
    display: dict[str, dict[str, list]]
    grip: float                         # 拳心对准的模型高度（授权系，方块单位）


# ── 16² 贴图的两个原语 ────────────────────────────────────────────────────
# 手持物贴图全是「通用材质样本」（木纹 / 石片 / 锈斑 / 骨纹 / 绳纹），画法只有两
# 招：底噪 + 斑。原先各躺在 `gen_knife_trio` 私有一份；第二件资产（木棍）要用同一
# 套画法，抄过去就又成了本模块 docstring 里骂的那种"两套各写一份"。
#
# **改这两个函数会改掉所有既有资产的贴图。** 它们的 RNG 调用序就是产物的一部分：
# 每个 pixel 一次 `randint`（warm 时两次），顺序 row-major。想加新效果就加新函数，
# 别在这两个里插调用。


def noise_fill(image: Image.Image, rng: random.Random, base, spread: int,
               warm: int = 0) -> None:
    """整图铺底噪。`warm` 给 R 加、给 B 减同一个随机量（做木/锈那种暖偏）。"""
    pixels = image.load()
    for y in range(image.height):
        for x in range(image.width):
            jitter = rng.randint(-spread, spread)
            tint = rng.randint(-warm, warm) if warm else 0
            pixels[x, y] = (
                max(0, min(255, base[0] + jitter + tint)),
                max(0, min(255, base[1] + jitter)),
                max(0, min(255, base[2] + jitter - tint)),
                255,
            )


def blotch(image: Image.Image, rng: random.Random, count: int, colour, radius) -> None:
    """撒 `count` 块径向渐隐的斑。斑比线更像"自然痕迹"——规则的斜线在任何尺度下
    都读成刮蹭（`tex_iron_forged` 那轮踩过）。"""
    pixels = image.load()
    for _ in range(count):
        cx, cy = rng.uniform(0, 15), rng.uniform(0, 15)
        r = rng.uniform(*radius)
        peak = rng.uniform(0.25, 0.62)
        for y in range(max(0, int(cy - r)), min(16, int(cy + r) + 1)):
            for x in range(max(0, int(cx - r)), min(16, int(cx + r) + 1)):
                d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5 / r
                if d > 1.0:
                    continue
                a = peak * (1.0 - d)
                px = pixels[x, y]
                pixels[x, y] = tuple(
                    int(round(px[i] * (1 - a) + colour[i] * a)) for i in range(3)
                ) + (255,)


# ── 校验 ──────────────────────────────────────────────────────────────────


BLOCK_CENTRE = 0.5              # MC display 变换的枢轴，方块单位


def emit_offset(item: HeldItem) -> tuple[float, float, float]:
    """授权系 → 出料系的整体平移，让**握把点落在方块中心**（见模块 docstring）。"""
    return (BLOCK_CENTRE, BLOCK_CENTRE - item.grip, BLOCK_CENTRE)


def assert_conventions(item: HeldItem) -> None:
    """坐标与材质约定。违反了不是"看着怪"，是 display 变换整套失准。"""
    if not item.boxes:
        raise ValueError(f"{item.key}: 没有 box")

    names: set[str] = set()
    for box in item.boxes:
        if box.name in names:
            raise ValueError(f"{item.key}: 重名 box {box.name}")
        names.add(box.name)
        if any(h <= 0.0 for h in box.half):
            raise ValueError(f"{item.key}/{box.name}: half 必须为正，得到 {box.half}")

    known = {m.name for m in item.materials}
    if len(known) != len(item.materials):
        raise ValueError(f"{item.key}: material 重名")
    for box in item.boxes:
        if box.material not in known:
            raise ValueError(f"{item.key}/{box.name}: 未知 material {box.material}")
    used = {b.material for b in item.boxes}
    if used != known:
        raise ValueError(
            f"{item.key}: material {known - used} 定义了但没有 box 用——"
            f"会白占一张 16² 贴图，且 MTL 里挂个死条目"
        )

    for material in item.materials:
        if material.texture.size != (TILE, TILE):
            raise ValueError(
                f"{item.key}/{material.name}: 贴图 {material.texture.size} 不是 {TILE}²"
            )

    y_min = min(b.low[1] for b in item.boxes)
    y_max = max(b.high[1] for b in item.boxes)
    if abs(y_min) > 1e-6:
        raise ValueError(
            f"{item.key}: 最低点 y={y_min:.4f} 不在 0。握把末端必须落在 y=0、"
            f"尖端朝 +Y，否则这件的 display 变换和 axe_bone 那套基线对不上，"
            f"手持时会插进手掌或飘在外面"
        )
    if not 0.3 <= y_max <= 1.2:
        raise ValueError(f"{item.key}: 全长 {y_max:.3f} 超出手持物合理区间 0.3~1.2")
    for axis, label in ((0, "x"), (2, "z")):
        span = max(b.high[axis] for b in item.boxes) - min(b.low[axis] for b in item.boxes)
        if span > 0.6:
            raise ValueError(f"{item.key}: {label} 向跨度 {span:.3f} 过大，不像手持物")

    # 拳头在世界里约 4px 宽，换算回模型是 4/scale px；握把点必须落在模型上，而且
    # 不能贴着尖端——否则 emit_offset 会把整件推出方块盒，display 枢轴也就没意义了。
    if not 0.0 < item.grip < y_max:
        raise ValueError(
            f"{item.key}: grip={item.grip:.3f} 不在 (0, {y_max:.3f}) 内。"
            f"grip 是拳心对准的模型高度，落在握把中段；出料时整件会平移成"
            f"「grip 点 = 方块中心」，见 emit_offset"
        )


def assert_no_coplanar_faces(item: HeldItem) -> None:
    """揪出"两块外表面落在同一平面且投影相交"的 box 对——体素模型的经典 z-fighting。

    渲染器对同深度的两个面没有稳定取舍，逐像素乱选，渲出来是一片高频噪点，
    肉眼极易误判成"贴图脏"。刀这类件里最容易犯的是**刃分段**：相邻两段图省事
    写成同一个 x 半宽，两段的侧面就共面了。
    """
    boxes = item.boxes
    for i in range(len(boxes)):
        for j in range(i + 1, len(boxes)):
            first, second = boxes[i], boxes[j]
            for axis in range(3):
                overlap = 1.0
                for other in (k for k in range(3) if k != axis):
                    overlap *= max(0.0, min(first.high[other], second.high[other])
                                   - max(first.low[other], second.low[other]))
                if overlap <= 1e-4:      # 只擦到一条边不算，那是正常拼接
                    continue
                for face, a, b in (("max", first.high[axis], second.high[axis]),
                                   ("min", first.low[axis], second.low[axis])):
                    if abs(a - b) < 1e-9:
                        raise ValueError(
                            f"{item.key}: {first.name} 与 {second.name} 的 "
                            f"{'xyz'[axis]}-{face} 面共面于 {a}，投影相交 {overlap:.5f}"
                            f"——会 z-fighting，挪开一块"
                        )


MIN_CONTACT_RATIO = 0.08        # 一块与邻居的接触面积，至少要占自身最大截面的 8%
# 小于这个缝就当贴上了。判据是**看不看得见**：件在真实手持尺寸下约 110px/方块，
# 0.004 方块 ≈ 0.44px。小刀那三把的绳缠道与道之间就留着 0.0003 的缝（0.03px），
# 那是刻意做的参差，不是断开。
GAP_TOLERANCE = 0.004


def _contact_area(first: Box, second: Box) -> float:
    """两块的**接触面积**。分离返回 0；只碰到一条棱或一个角也返回 0。

    不能用"相交体积"：本模块的件大多是**上下相接**的分段（刀柄三段、木条五段），
    相接处体积恒为 0，用体积判会把所有正常拼接都判成断开。面积口径同时覆盖两种
    正当连接——贴面相接（一轴重叠为 0、另两轴有面积）和互相嵌入。
    """
    span = [min(first.high[k], second.high[k]) - max(first.low[k], second.low[k])
            for k in range(3)]
    if min(span) < -GAP_TOLERANCE:
        return 0.0                                  # 中间有肉眼可见的缝
    span = sorted((max(0.0, v) for v in span), reverse=True)
    return span[0] * span[1]                        # 最大的两轴 = 接触面


def assert_boxes_are_connected(item: HeldItem) -> None:
    """整件必须是**一个连通体**，且没有只靠一条棱 / 一个角挂着的碎块。

    体素件最扎眼的缺陷之一：某块从主体上分离出去，渲出来像一块飘在旁边的碎料。
    三视图里未必看得出来——正视被主体挡住、侧视又刚好重叠。

    **这道闸上线当天就抓到一个已 merge 的真缺陷**：`iron_dagger` 的 `handle_body`
    顶到 0.2635，而护环底在 0.2688，中间空着 0.0053——整条刃加护环因此是一块**和
    柄不相连的浮空体**，且与该生成器自己写的「木柄 0~0.269」也对不上。三视图渲了
    三轮没人看出来，因为那道缝在图上不到一个像素。
    """
    boxes = item.boxes
    if len(boxes) < 2:
        return

    adjacency: dict[int, set[int]] = {i: set() for i in range(len(boxes))}
    for i, box in enumerate(boxes):
        w, h, d = (2.0 * v for v in box.half)
        own = max(w * h, h * d, w * d)              # 自身最大截面
        best = 0.0
        for j, other in enumerate(boxes):
            if i == j:
                continue
            area = _contact_area(box, other)
            best = max(best, area)
            if area > 1e-9:
                adjacency[i].add(j)
        if best < own * MIN_CONTACT_RATIO:
            raise ValueError(
                f"{item.key}/{box.name}: 与其余部件的最大接触面积只有自身截面的 "
                f"{best / own * 100:.1f}%（下限 {MIN_CONTACT_RATIO * 100:.0f}%）——"
                f"渲出来是一块飘在旁边的碎块。往主体挪，或加大重叠"
            )

    reached = {0}
    frontier = [0]
    while frontier:
        current = frontier.pop()
        for neighbour in adjacency[current] - reached:
            reached.add(neighbour)
            frontier.append(neighbour)
    if len(reached) != len(boxes):
        stray = sorted(boxes[i].name for i in range(len(boxes)) if i not in reached)
        raise ValueError(f"{item.key}: 这些部件和主体不连通（自成一块）：{stray}")


# ── 手持 display ──────────────────────────────────────────────────────────
# **不要抄 `axe_bone` 的 [0,-90,55] / [0,4,0]**，那组数是原版 `item/handheld` 的，
# 而 handheld 伺候的是**平面 sprite**：贴图里刃走左下→右上的对角线，55° 正好把那
# 条对角线掰正，[0,4,0] 补的是 sprite 握把点（约模型 (3,3)px）到方块中心的偏移。
# 我们这套是**沿 +Y 立着的三维件**，两条前提都不成立——照抄的结果是件朝斜下方
# 58°、握把离拳心 6.3px（拳头才 4px 宽）。
#
# `emit_offset()` 已把握把点放到方块中心（= display 枢轴），于是本仓基线是：
#
#   rotation    [-80, 90, 0]
#               Rx(-80)  **件沿前臂出拳**，只偏 10°。这里**刻意不抄原版剑**：原版
#                        把刃摆成⊥前臂（arm 垂下时刃水平朝前），那是平面 sprite 的
#                        产物，只在**手臂伸直**时成立。Bong 有 bendy-lib 肘弯，⊥ 的
#                        刃会跟着小臂转上去——实测两条匕首动画每一 tick 刃仰角都在
#                        +63~+78°，横划那条刃尖甚至越过肩往身后指，读成"举着火把"。
#                        拳头握持的真解剖是件基本沿前臂出虎口，rx=-80 就是这个。
#               Ry(90)   把件的薄轴（模型 ±Z）转到玩家左右两侧，和原版剑一样：
#                        侧看是一片，正面看是一条棱
#   translation [0, -2, 1.5]
#               握把点该落在**拳心**。MC 的挂点 `R_ATTACH·(1,2,-10)` = 臂系
#               (-1,10,-2)，是臂盒底面往前 2px；拳心在臂盒底面往上 1.5px、z 居中，
#               即 (-1, 8.5, 0)。差值换回 display 前的系就是 (0,-2,1.5)。
#               —— 同一算法量原版剑得 (0,-1.54,1.92)，同量级，互为佐证。
#
# 左手那组**预取反 y/z 旋转**：`Transformation.apply(leftHanded)` 自己还会再取反
# 一次 y/z 旋转并翻 x 平移，两次抵消后左右手才是镜像而不是同姿。
#
# GUI / ground / fixed / head 要的是**整件居中**而不是握把居中，所以那几档用
# `centre_translation` 反解。少了这一步图标会被握把顶得偏出格子。
#
# FPV 两档只把坐标系摆正，**数值未经真机标定**：本 harness 渲不了第一人称，且 FPV
# 手臂另有 plan-fpv-cast-av-v1 在动。

DEFAULT_HAND_ROTATION = (-80, 90, 0)


def _angles(values) -> list:
    """把角度列表规整成 JSON 里好看的形态：整数写成 int，`-0.0` 归零。

    纯写法问题，但**不能省**：左手那组是右手的取反，`-0.0` 会原样写进 model JSON，
    而 diff 里一个 `-0.0` 看不出是"镜像算出来的零"还是"谁手滑改的"。
    """
    out = []
    for v in values:
        v = float(v) + 0.0                       # −0.0 → 0.0
        out.append(int(v) if v.is_integer() else v)
    return out


def centre_translation(rotation, scale: float, centre_px: float,
                       target: tuple[float, float, float] = (0.0, 0.0, 0.0)) -> list[float]:
    """反解「让模型几何中心落在 `target`」的 display translation（px）。

    MC 的点变换是 `p = t + R·S·(v - 8)`；令几何中心（授权系里相对握把点 `centre_px`
    的那一点）落到 `target` 即可。
    """
    rx, ry, rz = (math.radians(v) for v in rotation)

    def _rot_x(v):
        return (v[0], v[1] * math.cos(rx) - v[2] * math.sin(rx),
                v[1] * math.sin(rx) + v[2] * math.cos(rx))

    def _rot_y(v):
        return (v[0] * math.cos(ry) + v[2] * math.sin(ry), v[1],
                -v[0] * math.sin(ry) + v[2] * math.cos(ry))

    def _rot_z(v):
        return (v[0] * math.cos(rz) - v[1] * math.sin(rz),
                v[0] * math.sin(rz) + v[1] * math.cos(rz), v[2])

    # JOML rotationXYZ = Rx·Ry·Rz，作用到向量上是先 Z 再 Y 再 X
    moved = _rot_x(_rot_y(_rot_z((0.0, centre_px * scale, 0.0))))
    return [round(target[i] - moved[i], 3) for i in range(3)]


def hand_display(scale: float, grip: float, length: float, *,
                 rotation: tuple[float, float, float] = DEFAULT_HAND_ROTATION,
                 gui_scale: float = 1.15, gui_spin: float = 45.0,
                 ground_scale: float = 0.45) -> dict:
    """整套 display 变换。`grip` / `length` 单位是方块（授权系）。

    `gui_spin` 是 GUI 图标绕 Z 转的角度：细长件走对角线才占满格子（原版剑/匕首同一
    处理）。粗短件转 45° 反而会露出四角空白，那种件传 0。
    """
    centre_px = (length / 2.0 - grip) * 16.0      # 几何中心相对握把点，px
    rx, ry, rz = rotation
    right = _angles((rx, ry, rz))
    left = _angles((rx, -ry, -rz))

    def centred(rot, sc, target=(0.0, 0.0, 0.0)):
        return {"rotation": _angles(rot), "scale": [sc, sc, sc],
                "translation": centre_translation(rot, sc, centre_px, target)}

    fp = round(scale - 0.04, 4)
    return {
        "thirdperson_righthand": {"rotation": right, "translation": [0, -2.0, 1.5],
                                  "scale": [scale, scale, scale]},
        "thirdperson_lefthand": {"rotation": left, "translation": [0, -2.0, 1.5],
                                 "scale": [scale, scale, scale]},
        "firstperson_righthand": {"rotation": right, "translation": [0, -2.0, -4.0],
                                  "scale": [fp, fp, fp]},
        "firstperson_lefthand": {"rotation": left, "translation": [0, -2.0, -4.0],
                                 "scale": [fp, fp, fp]},
        "ground": centred((0, 0, 0), ground_scale, (0.0, 2.0, 0.0)),
        "gui": centred((0, 0, gui_spin), gui_scale),
        "fixed": centred((0, 180, 0), 1.0),
        "head": centred((0, 0, 0), 1.0, (0.0, 12.0, 0.0)),
    }


# ── OBJ / MTL ─────────────────────────────────────────────────────────────


def build_obj(item: HeldItem) -> str:
    lines = [
        f"# {item.key}.obj -- generated by modelScript/core/held_item_common.py",
        "# 勿手改：改 gen_* 里的 box 表后重跑生成器。",
        f"mtllib {item.key}.mtl",
        f"o {item.key}",
    ]
    lines += [f"vt {u:.4f} {v:.4f}" for u, v in _VT]
    lines += [f"vn {x:.4f} {y:.4f} {z:.4f}" for x, y, z in _VN]

    base = 0
    off = emit_offset(item)
    for box in item.boxes:
        # 出料系 = 授权系 + emit_offset（握把点落方块中心，见模块 docstring）
        lo = tuple(box.low[i] + off[i] for i in range(3))
        hi = tuple(box.high[i] + off[i] for i in range(3))
        corners = (
            (lo[0], lo[1], lo[2]), (hi[0], lo[1], lo[2]),
            (hi[0], hi[1], lo[2]), (lo[0], hi[1], lo[2]),
            (lo[0], lo[1], hi[2]), (hi[0], lo[1], hi[2]),
            (hi[0], hi[1], hi[2]), (lo[0], hi[1], hi[2]),
        )
        lines.append(f"# part: {box.name}")
        lines += [f"v {x:.4f} {y:.4f} {z:.4f}" for x, y, z in corners]
        lines.append(f"usemtl {box.material}")
        for order, normal in _FACES:
            lines.append("f " + " ".join(
                f"{base + k + 1}/{n + 1}/{normal}" for n, k in enumerate(order)
            ))
        base += 8
    return "\n".join(lines) + "\n"


def build_mtl(item: HeldItem, namespace: str | None = None) -> str:
    lines = [f"# {item.key} materials -- generated by held_item_common.py"]
    for index, material in enumerate(item.materials):
        r, g, b = material.kd
        lines += [
            "",
            f"newmtl {material.name}",
            "Ka 1.000000 1.000000 1.000000",
            f"Kd {r:.6f} {g:.6f} {b:.6f}",
            "Ks 0.000000 0.000000 0.000000",
            "Ns 10.000000",
            "d 1.000000",
            "illum 1",
            f"map_Kd {namespace or workspace.default().namespace}:item/{item.key}/{index}",
        ]
    return "\n".join(lines) + "\n"


def build_model_json(item: HeldItem, namespace: str | None = None) -> str:
    ns = namespace or workspace.default().namespace
    return json.dumps(
        {
            "parent": "sml:builtin/obj",
            "model": f"{ns}:models/item/{item.key}/{item.key}.obj",
            "display": item.display,
        },
        ensure_ascii=False,
        indent=2,
    ) + "\n"


# ── bbmodel ───────────────────────────────────────────────────────────────


def build_atlas(item: HeldItem) -> Image.Image:
    """把各 material 的 16² 图横排成一张图集，供 bbmodel 用。

    只是为了让 Blockbench 里看到的和游戏里一致；游戏那条链读的是拆开的单张图。
    """
    count = len(item.materials)
    width = TILE * count
    atlas = Image.new("RGBA", (width, TILE), (0, 0, 0, 0))
    for index, material in enumerate(item.materials):
        atlas.paste(material.texture.convert("RGBA"), (index * TILE, 0))
    return atlas


def _data_url(image: Image.Image) -> str:
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")
    return "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode("ascii")


def build_bbmodel(item: HeldItem, namespace: str | None = None) -> dict:
    """bbmodel 用**出料系 ×16**（即 px）写坐标，Blockbench 的格子才对得上。

    坐标要和 OBJ 逐点一致（同一个 `emit_offset`）——bbmodel 是设计期看的，OBJ 是
    运行时吃的，两边差一个平移就意味着"预览里握得住、进游戏握不住"。

    uuid 全部走 uuid5：uuid4 会让每次重跑都产出一份 diff，git 上分不清"改了造型"
    和"只是重跑了一遍"（棺材那批生成器踩过）。
    """
    index_of = {m.name: i for i, m in enumerate(item.materials)}
    atlas_w = TILE * len(item.materials)
    off = emit_offset(item)
    elements = []
    for box in item.boxes:
        tile = index_of[box.material]
        u0, u1 = tile * TILE, (tile + 1) * TILE
        faces = {
            name: {"uv": [u0, 0, u1, TILE], "texture": 0}
            for name in _BB_FACES
        }
        elements.append({
            "name": box.name,
            "box_uv": False,
            "rescale": False,
            "locked": False,
            "render_order": "default",
            "allow_mirror_modeling": True,
            "type": "cube",
            "uuid": str(uuid.uuid5(MODEL_NAMESPACE, f"{item.key}/{box.name}")),
            "from": [round((v + off[i]) * 16.0, 4) for i, v in enumerate(box.low)],
            "to": [round((v + off[i]) * 16.0, 4) for i, v in enumerate(box.high)],
            "autouv": 0,
            "color": tile % 8,
            "origin": [0.0, 0.0, 0.0],
            "faces": faces,
        })

    groups: dict[str, list[str]] = {}
    for box, element in zip(item.boxes, elements):
        groups.setdefault(box.material, []).append(element["uuid"])
    outliner = [
        {
            "name": material,
            "origin": [0.0, 0.0, 0.0],
            "color": index_of[material] % 8,
            "uuid": str(uuid.uuid5(MODEL_NAMESPACE, f"{item.key}/group/{material}")),
            "export": True,
            "mirror_uv": False,
            "isOpen": True,
            "locked": False,
            "visibility": True,
            "autouv": 0,
            "children": children,
        }
        for material, children in groups.items()
    ]

    return {
        "meta": {"format_version": "4.10", "model_format": "free", "box_uv": False},
        "name": item.key,
        "model_identifier": f"geometry.{namespace or workspace.default().namespace}.{item.key}",
        "visible_box": [2, 2, 1],
        "resolution": {"width": atlas_w, "height": TILE},
        "elements": elements,
        "outliner": outliner,
        "textures": [{
            "path": "",
            "name": f"{item.key}.png",
            "folder": "item",
            "namespace": namespace or workspace.default().namespace,
            "id": "0",
            "width": atlas_w,
            "height": TILE,
            "uv_width": atlas_w,
            "uv_height": TILE,
            "particle": False,
            "render_mode": "default",
            "visible": True,
            "mode": "bitmap",
            "saved": False,
            "uuid": str(uuid.uuid5(MODEL_NAMESPACE, f"{item.key}/texture")),
            "source": _data_url(build_atlas(item)),
        }],
    }


# ── 落盘 ──────────────────────────────────────────────────────────────────


def assert_host_is_claimable(item: HeldItem, host_path: Path,
                             claimed: dict[str, str],
                             namespace: str | None = None) -> None:
    """劫持宿主 model JSON 之前的 fail-fast。**撞车必须炸，不许静默覆盖。**

    宿主机制的粒度是「一个 vanilla item → 一份 model JSON」，写进去就是全局生效。
    没有这道闸的话有两种静默灾难：

    1. **覆盖别人的模板。** `assets/minecraft/models/item/bone.json` 现在指向
       `bone_dagger`；`bone_spike` 也宿在 `bone` 上，`--install` 会把 bone_dagger
       悄悄变成骨刺，而且 git diff 里只是一份 JSON 变了，看不出牵连到哪件物品。
    2. **同一批里两件共宿主。** 后写的赢，前一件白生成，没有任何提示。

    真正的解法是废掉宿主机制本身（`plan-held-item-registration-v1`：每个模板注册
    自己的 render-only Item）。在那之前这道闸至少保证错误是响的。
    """
    if item.host_item in claimed:
        raise ValueError(
            f"{item.key} 与 {claimed[item.host_item]} 共用宿主 {item.host_item!r}——"
            f"宿主粒度是「一个 vanilla item 一份 model JSON」，共宿主必然同形，"
            f"后写的会盖掉前一件。给其中一件换宿主，或走 plan-held-item-registration-v1"
        )
    if not host_path.is_file():
        return
    try:
        existing = json.loads(host_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return          # 读不动就不拦，交给后面的写入报真错
    want = f"{namespace or workspace.default().namespace}:models/item/{item.key}/{item.key}.obj"
    found = existing.get("model")
    if found is not None and found != want:
        raise ValueError(
            f"{item.key} 要劫持的宿主 {item.host_item!r} 已经被占用了：\n"
            f"    {host_path} 当前指向 {found}\n"
            f"    本次会把它改成 {want}\n"
            f"写下去会让原来那件物品在游戏里变成 {item.key} 的样子，而 diff 只显示"
            f"一份 JSON 变了、看不出牵连。换宿主，或走 plan-held-item-registration-v1"
        )


def write_assets(
    items: tuple[HeldItem, ...],
    bbmodel_dir: Path,
    client_resources: Path | None,
    preview_dir: Path | None = None,
    render_previews: bool = True,
    namespace: str | None = None,
) -> dict[str, Path]:
    """bbmodel 恒写；OBJ/MTL/贴图/model JSON 只在给了 client_resources 时写。

    分开是为了让"改造型"和"装进游戏"能拆成两个 commit：前者只动 modelScript，
    后者才碰 client 资源树（那一步要连带同步资源包 sha1）。
    """
    ns = namespace or workspace.default().namespace
    for item in items:
        assert_conventions(item)
        assert_no_coplanar_faces(item)
        assert_boxes_are_connected(item)

    outputs: dict[str, Path] = {}
    claimed_hosts: dict[str, str] = {}      # host_item -> 本批里已占用它的 item.key
    bbmodel_dir.mkdir(parents=True, exist_ok=True)

    for item in items:
        name = "".join(word.capitalize() for word in item.key.split("_")) + ".bbmodel"
        path = bbmodel_dir / name
        path.write_text(
            json.dumps(build_bbmodel(item, ns), ensure_ascii=False, indent=1) + "\n",
            encoding="utf-8",
        )
        outputs[f"bbmodel:{item.key}"] = path

        if client_resources is not None:
            model_dir = client_resources / "assets" / ns / "models" / "item" / item.key
            model_dir.mkdir(parents=True, exist_ok=True)
            (model_dir / f"{item.key}.obj").write_text(build_obj(item), encoding="utf-8")
            (model_dir / f"{item.key}.mtl").write_text(build_mtl(item, ns), encoding="utf-8")
            (model_dir / f"{item.key}.json").write_text(build_model_json(item, ns), encoding="utf-8")
            outputs[f"obj:{item.key}"] = model_dir / f"{item.key}.obj"

            # 劫持宿主 vanilla item 的 model JSON —— 内容与本命名空间那份一致，指向同一 OBJ。
            host_dir = client_resources / "assets" / "minecraft" / "models" / "item"
            host_dir.mkdir(parents=True, exist_ok=True)
            host_path = host_dir / f"{item.host_item}.json"
            assert_host_is_claimable(item, host_path, claimed_hosts, ns)
            claimed_hosts[item.host_item] = item.key
            host_path.write_text(build_model_json(item, ns), encoding="utf-8")
            outputs[f"host:{item.key}"] = host_path

            tex_dir = client_resources / "assets" / ns / "textures" / "item" / item.key
            tex_dir.mkdir(parents=True, exist_ok=True)
            for index, material in enumerate(item.materials):
                material.texture.save(tex_dir / f"{index}.png")
            outputs[f"tex:{item.key}"] = tex_dir

    if render_previews and preview_dir is not None:
        from render_bbmodel import render_three_view

        preview_dir.mkdir(parents=True, exist_ok=True)
        for item in items:
            preview, _ = render_three_view(outputs[f"bbmodel:{item.key}"], size=320)
            path = preview_dir / f"{item.key}_render_three_view.png"
            preview.save(path)
            outputs[f"preview:{item.key}"] = path

    return outputs
