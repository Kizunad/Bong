#!/usr/bin/env python3
"""固定取景 + 诚实的视角命名 —— 治「模型看不见自己做的东西」。

这个模块存在的理由是两次实测教训，都不是推测：

**一、视角标签会骗人。** `render_bbmodel.THREE_VIEW_ANGLES` 把 `yaw=180` 叫 `FRONT`，
而 yaw=180 渲出来的是模型的 **−z 面**。按矩阵验算：`R = rotmat(pitch,0) @ rotmat(yaw,1)`，
背面剔除是「法线转到 +z 才画」，`rotmat(180,1)` 把 (0,0,1) 翻成 (0,0,-1) 剔掉、把
(0,0,-1) 翻成 (0,0,1) 留下 —— 所以 yaw=180 看的是 −z，`yaw=0` 才看 +z。
小草包的骨扣长在 +z 前檐上，于是「正面看不见骨扣」这个**假 bug** 让人连试三个亮度阈值
（r>120 / r>95 / mean+20），全在错的视角上找一个本就不该出现的东西 —— 几何、UV、材质
从头到尾都是对的。

**二、但「哪面是正面」是真歧义，不能盲改。** 各资产的建模朝向不一致：对朝 −z 建的模型，
yaw=180 看到的**确实**是它的正面。所以这里不把 `FRONT` 改写成 `BACK`，而是做成**声明式
朝向解析**：资产声明自己的 `facing`（`+z` / `-z` / `+x` / `-x`），视角名由它派生，
标签上同时写出这一张实际照的是哪个轴面。名字和几何从此对得上，谁也骗不了谁。

**三、自动取景不能跨图比较。** `render()` 不传 `focus` 时每次各算各的包围盒中心与
`scale = (size-60)/span` —— 两张图的屏幕坐标毫无可比性，「这一轮比上一轮矮了一点」这种
判断在自动取景下全是噪声。本模块的 `focus_for()` 一次算出覆盖所有视角的公共取景，
所有图共用，横向、跨轮都能直接叠着看。

左右的约定（只此一条，别再各写各的）：
    **SIDE_R = 在 FRONT 视里出现在观者右手边的那一侧。**
FRONT 视 yaw=Y 时，屏幕右方向对应的模型方向是 `R⁻¹·(1,0,0) = (cosY, 0, sinY)`；
等价写法是 `right = up × facing`。小草包的侧插袋写在 `+x`、作者称「右侧」，facing 是
`+z`（前檐 z 为正）—— 两者一致，这条约定就是照实测定的。
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from pathlib import Path

import numpy as np

# 朝向声明 → 该面的外法线。只认这四个轴向：斜朝向的资产请自己转正再建模，
# 半吊子的「大致朝 +z」会让 SIDE_R/SIDE_L 变成不可核验的口头约定。
FACING_NORMALS: dict[str, tuple[float, float, float]] = {
    "+z": (0.0, 0.0, 1.0),
    "-z": (0.0, 0.0, -1.0),
    "+x": (1.0, 0.0, 0.0),
    "-x": (-1.0, 0.0, 0.0),
}

# `render_bbmodel.THREE_VIEW_ANGLES` 历史上把 yaw=180 叫 FRONT —— 那等价于「假定所有
# 资产都朝 −z」。既有 6 个调用点全部继承这个假定，所以它就是默认值；换默认值等于悄悄
# 把所有历史预览图的语义翻个面，比留着这个默认更危险。
LEGACY_FACING = "-z"

# 六视角接触表的固定顺序。TOP 排在最后一行不是随手排的：俯视里墙面全部边缘化被剔除，
# 它只回答「占地多大 / 有没有件飘在外面」，和另外五张不是一类问题。
SIX_VIEWS: tuple[str, ...] = ("FRONT", "SIDE_R", "BACK", "SIDE_L", "3/4", "TOP")

# 3/4 是「从 FRONT 往 SIDE_L 方向偏 35°、略俯 15°」。写成相对 FRONT 的偏移而不是绝对
# yaw，换朝向时才不会退化成随机角度 —— 历史上 THREE_VIEW_ANGLES 的 145 = 180−35 正是
# 这个偏移，只是当时把它硬编成了绝对值。
THREE_QUARTER_YAW_OFFSET = -35.0
THREE_QUARTER_PITCH = 15.0
TOP_PITCH = 90.0


@dataclass(frozen=True)
class View:
    """一个具名视角。

    name  —— 语义名（FRONT / SIDE_R / …），由资产的 facing 派生，不是硬编的。
    yaw / pitch —— 直接喂给 `render_bbmodel.render()` 的角度。
    shows —— 这一张实际正对镜头的轴面（"+z" / "-x" / "+y"）；斜视角为 ""。
             标签上必须带它，否则名字又能骗人了。
    """

    name: str
    yaw: float
    pitch: float
    shows: str

    @property
    def label(self) -> str:
        """图上那一行字。ASCII only —— PIL 默认位图字体画不了中文和 U+2212。"""
        axis = f" ({self.shows})" if self.shows else ""
        return f"{self.name}{axis} yaw={self.yaw:g} pitch={self.pitch:g}"


def parse_facing(facing: str) -> tuple[float, float, float]:
    """朝向字符串 → 外法线。写错立刻炸，别默默退回某个默认值。"""
    key = str(facing).strip()
    if key not in FACING_NORMALS:
        raise ValueError(
            f"未知朝向 {facing!r}；只认 {', '.join(sorted(FACING_NORMALS))}"
        )
    return FACING_NORMALS[key]


def yaw_for_normal(normal) -> float:
    """让某个轴面正对镜头所需的 yaw（度，[0,360)）。

    渲染器只画法线转到 +z 的面，`R@n` 的 z 分量 = `-sin(yaw)·nx + cos(yaw)·nz`：
        (0,0,1) → yaw 0      (0,0,-1) → yaw 180
        (1,0,0) → yaw 270    (-1,0,0) → yaw 90
    注意 +x 面要转到 **270** 而不是 90 —— 记反了整套 SIDE_L/SIDE_R 就是镜像的。
    """
    n = np.asarray(normal, float)
    if n.shape != (3,):
        raise ValueError(f"法线必须是三元组，收到 {normal!r}")
    norm = float(np.linalg.norm(n))
    if norm < 1e-9:
        raise ValueError("零向量没有朝向")
    n = n / norm
    if abs(n[1]) > 1e-6:
        raise ValueError(f"yaw 只能摆平面朝向，法线 {tuple(n)} 有 y 分量")
    # z' = -sin(yaw)*nx + cos(yaw)*nz 取最大 → yaw = atan2(-nx, nz)
    return math.degrees(math.atan2(-n[0], n[2])) % 360.0


def right_normal(facing: str) -> tuple[float, float, float]:
    """FRONT 视里出现在观者**右手边**的那一侧的外法线 = up × facing。"""
    f = np.asarray(parse_facing(facing), float)
    r = np.cross((0.0, 1.0, 0.0), f)
    return (float(r[0]), float(r[1]), float(r[2]))


def _axis_name(normal) -> str:
    n = np.asarray(normal, float)
    i = int(np.argmax(np.abs(n)))
    return ("+" if n[i] >= 0 else "-") + "xyz"[i]


def views_for(facing: str = LEGACY_FACING, names=SIX_VIEWS) -> tuple[View, ...]:
    """按资产朝向派生具名视角。

    names 决定出哪几张、按什么顺序；重复名字照出（对着同一角度出两张没有意义，
    但拦下来只会逼调用方绕路，不值当）。
    """
    front_n = parse_facing(facing)
    right_n = right_normal(facing)
    left_n = tuple(-v for v in right_n)
    back_n = tuple(-v for v in front_n)
    front_yaw = yaw_for_normal(front_n)

    table = {
        "FRONT": (front_yaw, 0.0, _axis_name(front_n)),
        "BACK": (yaw_for_normal(back_n), 0.0, _axis_name(back_n)),
        "SIDE_R": (yaw_for_normal(right_n), 0.0, _axis_name(right_n)),
        "SIDE_L": (yaw_for_normal(left_n), 0.0, _axis_name(left_n)),
        "TOP": (front_yaw, TOP_PITCH, "+y"),
        "3/4": ((front_yaw + THREE_QUARTER_YAW_OFFSET) % 360.0, THREE_QUARTER_PITCH, ""),
    }
    out = []
    for nm in names:
        if nm not in table:
            raise KeyError(f"未知视角名 {nm!r}；可选 {', '.join(table)}")
        yaw, pitch, shows = table[nm]
        out.append(View(nm, yaw, pitch, shows))
    return tuple(out)


def view_by_name(facing: str, name: str) -> View:
    """单取一个视角。manifest 点名器按名字查角度用的就是它。"""
    return views_for(facing, (name,))[0]


def _rotmat(deg: float, axis: int) -> np.ndarray:
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    if axis == 0:
        return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])
    if axis == 1:
        return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])


def view_matrix(view: View) -> np.ndarray:
    """与 `render_bbmodel.render()` 内部完全一致的视图矩阵。

    两处各写一遍迟早会漂 —— 但 render() 那份是热路径里的局部变量，抽不出来又不想改它
    的签名，所以这里重写一遍并由 `test_framing` 对拍钉死。
    """
    return _rotmat(view.pitch, 0) @ _rotmat(view.yaw, 1)


def model_bounds(path) -> tuple[np.ndarray, np.ndarray]:
    """bbmodel 全部三角形顶点的包围盒（已含 element 自身 rotation）。"""
    from render_bbmodel import load_bbmodel

    tris, _, _, _ = load_bbmodel(path)
    if not tris:
        raise ValueError(f"{Path(path).name} 没有可渲染的面，量不出取景")
    allv = np.array([v for tri in tris for v in tri[0]], float)
    return allv.min(0), allv.max(0)


def focus_for(path, views, *, margin: float = 1.04) -> tuple[tuple[float, float, float], float]:
    """一组视角**共用**的取景 `(center, span)`，直接喂 `render(focus=...)`。

    span 取「所有视角里投影最宽的那一张」，而不是包围盒对角线：对角线对扁平资产浪费
    掉一半画布，而逐视角实测出来的最大跨度既不裁切也不浪费。用包围盒八角算是安全的
    —— 投影是线性的，所有顶点都在盒内，那么顶点集的投影跨度必 ≤ 八角的投影跨度。

    margin 是留白系数（默认 4%）：正好贴边的图在缩略图里会被误读成「顶出去了」。
    """
    if not views:
        raise ValueError("没有视角，算不出公共取景")
    lo, hi = model_bounds(path)
    center = (lo + hi) / 2.0
    corners = np.array([[lo[0] if i & 1 else hi[0],
                         lo[1] if i & 2 else hi[1],
                         lo[2] if i & 4 else hi[2]] for i in range(8)], float) - center
    span = 0.0
    for v in views:
        p = (view_matrix(v) @ corners.T).T[:, :2]
        span = max(span, float((p.max(0) - p.min(0)).max()))
    span = max(span * margin, 1e-6)
    return (float(center[0]), float(center[1]), float(center[2])), span


def render_views(path, views, *, focus=None, size: int = 320, shading: str = "lambert",
                 texture=None, bg=(22, 23, 26), xform=None):
    """按固定取景渲一组视角，返回 [(View, Image)]。

    focus 缺省时**自己算一个公共的**，绝不退化成 render() 的逐图自动取景 —— 那正是
    这个模块要治的病。要跨轮对比就把上一轮的 focus 原样传进来。
    """
    from render_bbmodel import render

    views = tuple(views)
    if focus is None:
        focus = focus_for(path, views)
    out = []
    for v in views:
        img, _ = render(path, yaw=v.yaw, pitch=v.pitch, size=size, bg=bg,
                        focus=focus, shading=shading, texture=texture, xform=xform)
        out.append((v, img))
    return out


def contact_sheet(tiles, *, title: str = "", notes=(), columns: int = 3,
                  bg=(14, 15, 17), fg=(220, 220, 212), warn=(232, 120, 96)):
    """把若干 (标签, 图) 拼成一张给**人**看的接触表。

    notes 是打在图下方的等宽文本行（点名结果、差分自证结果……）。以 "!" 开头的行标红
    —— 三十秒扫一眼就该看见哪条是红的，而不是逐行读。
    """
    from PIL import Image, ImageDraw

    tiles = list(tiles)
    if not tiles:
        raise ValueError("没有图，拼不出接触表")
    if columns < 1:
        raise ValueError(f"columns 必须 ≥ 1，收到 {columns}")

    tw = max(im.width for _, im in tiles)
    th = max(im.height for _, im in tiles)
    gap, lab_h, line_h = 12, 16, 13
    cols = min(columns, len(tiles))
    rows = (len(tiles) + cols - 1) // cols

    head_h = (lab_h + gap) if title else 0
    notes = list(notes)
    foot_h = (gap + line_h * len(notes)) if notes else 0
    width = cols * tw + gap * (cols + 1)
    height = head_h + rows * (th + lab_h + gap) + gap + foot_h

    canvas = Image.new("RGB", (width, height), bg)
    draw = ImageDraw.Draw(canvas)
    if title:
        draw.text((gap, gap // 2), title, fill=fg)

    for i, (label, im) in enumerate(tiles):
        r, c = divmod(i, cols)
        x = gap + c * (tw + gap)
        y = head_h + gap + r * (th + lab_h + gap)
        draw.text((x + 2, y), str(label), fill=fg)
        canvas.paste(im, (x, y + lab_h))

    y = head_h + rows * (th + lab_h + gap) + gap
    for line in notes:
        text = str(line)
        draw.text((gap, y), text, fill=warn if text.startswith("!") else fg)
        y += line_h
    return canvas
