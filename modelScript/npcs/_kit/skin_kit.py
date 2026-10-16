"""末土 NPC 皮肤工具：标准玩家 64×64 classic（4px 手臂）布局 + 逐像素画法。

只画贴图，不碰玩家骨架和 UV。盒子展开与 vanilla 一致：

    head   (0,0)   8×8×8     hat    (32,0)
    body   (16,16) 8×12×4    jacket (16,32)
    右臂   (40,16) 4×12×4    右袖   (40,32)
    左臂   (32,48) 4×12×4    左袖   (48,48)
    右腿   (0,16)  4×12×4    右裤   (0,32)
    左腿   (16,48) 4×12×4    左裤   (0,48)

「右/左」都是穿戴者自己的左右。每个盒子的四个侧面在贴图上连成一条横带（右侧 → 正面 → 左侧 → 背面），
所以这里用「带坐标」(i, j)：i 是带内横向像素，j 是盒子内的行（0 = 顶）。褶皱、补丁、缝线都按带坐标画，
绕侧面的时候天然连续。
"""

from __future__ import annotations

import colorsys
import math
from typing import Callable

import numpy as np
from PIL import Image

SIDES = ("right", "front", "left", "back")

# part: (u, v, 宽 sx, 高 sy, 深 sz)
BOXES: dict[str, tuple[int, int, int, int, int]] = {
    "head": (0, 0, 8, 8, 8), "hat": (32, 0, 8, 8, 8),
    "body": (16, 16, 8, 12, 4), "jacket": (16, 32, 8, 12, 4),
    "rarm": (40, 16, 4, 12, 4), "rsleeve": (40, 32, 4, 12, 4),
    "larm": (32, 48, 4, 12, 4), "lsleeve": (48, 48, 4, 12, 4),
    "rleg": (0, 16, 4, 12, 4), "rpants": (0, 32, 4, 12, 4),
    "lleg": (16, 48, 4, 12, 4), "lpants": (0, 48, 4, 12, 4),
}
# 第二层相对基础层的外扩（vanilla：帽 0.5，其余 0.25）
OVERLAY_OF = {"hat": "head", "jacket": "body", "rsleeve": "rarm", "lsleeve": "larm", "rpants": "rleg", "lpants": "lleg"}
INFLATE = {"hat": 0.5, "jacket": 0.25, "rsleeve": 0.25, "lsleeve": 0.25, "rpants": 0.25, "lpants": 0.25}

Color = tuple[float, float, float]


def C(hexstr: str) -> Color:
    h = hexstr.lstrip("#")
    return (float(int(h[0:2], 16)), float(int(h[2:4], 16)), float(int(h[4:6], 16)))


def mix(a: Color, b: Color, t: float) -> Color:
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t)


def tone(c: Color, k: float) -> Color:
    """k<0 压暗并偏冷、略加饱和；k>0 提亮并偏暖、略降饱和。像素画的明暗不是纯乘亮度。"""
    r, g, b = (v / 255 for v in c)
    h, s, v = colorsys.rgb_to_hsv(r, g, b)
    if k < 0:
        t = -k
        v *= 1 - 0.55 * t
        s = min(1.0, s * (1 + 0.18 * t))
        h += (0.66 - h) * 0.05 * t
    else:
        v = min(1.0, v * (1 + 0.45 * k))
        s *= 1 - 0.22 * k
        h += (0.12 - h) * 0.08 * k
    r, g, b = colorsys.hsv_to_rgb(h % 1.0, max(0.0, min(1.0, s)), max(0.0, min(1.0, v)))
    return (r * 255, g * 255, b * 255)


def quant(k: float, step: float = 0.07) -> float:
    return round(k / step) * step


def h2(x: int, y: int, s: int = 0) -> float:
    """确定性整数哈希噪声 [0,1)。"""
    n = (x * 374761393 + y * 668265263 + s * 1442695041 + 1013904223) & 0xFFFFFFFF
    n = ((n ^ (n >> 13)) * 1274126177) & 0xFFFFFFFF
    n ^= n >> 16
    return (n & 0xFFFF) / 65535.0


def vnoise(x: float, y: float, s: int = 0, scale: float = 3.0) -> float:
    """平滑值噪声，做污渍 / 褪色的大块形状。"""
    fx, fy = x / scale, y / scale
    x0, y0 = math.floor(fx), math.floor(fy)
    tx, ty = fx - x0, fy - y0
    tx, ty = tx * tx * (3 - 2 * tx), ty * ty * (3 - 2 * ty)
    a = h2(x0, y0, s) * (1 - tx) + h2(x0 + 1, y0, s) * tx
    b = h2(x0, y0 + 1, s) * (1 - tx) + h2(x0 + 1, y0 + 1, s) * tx
    return a * (1 - ty) + b * ty


def face_rect(part: str, face: str) -> tuple[int, int, int, int]:
    ou, ov, sx, sy, sz = BOXES[part]
    return {
        "right": (ou, ov + sz, sz, sy),
        "front": (ou + sz, ov + sz, sx, sy),
        "left": (ou + sz + sx, ov + sz, sz, sy),
        "back": (ou + 2 * sz + sx, ov + sz, sx, sy),
        "top": (ou + sz, ov, sx, sz),
        "bottom": (ou + sz + sx, ov, sx, sz),
    }[face]


def strip_width(part: str) -> int:
    _, _, sx, _, sz = BOXES[part]
    return 2 * sz + 2 * sx


def strip_faces(part: str) -> dict[str, tuple[int, int]]:
    """带内每个面占的列区间 [i0, i1)。"""
    _, _, sx, _, sz = BOXES[part]
    return {"right": (0, sz), "front": (sz, sz + sx), "left": (sz + sx, 2 * sz + sx), "back": (2 * sz + sx, 2 * sz + 2 * sx)}


class Skin:
    def __init__(self, seed: int = 0) -> None:
        self.a = np.zeros((64, 64, 4), float)
        self.seed = seed

    # ---------------------------------------------------------------- 基本读写
    def put(self, x: int, y: int, c: Color | None, alpha: float = 255.0) -> None:
        if not (0 <= x < 64 and 0 <= y < 64):
            return
        if c is None:
            self.a[y, x] = (0, 0, 0, 0)
        else:
            self.a[y, x] = (c[0], c[1], c[2], alpha)

    def get(self, x: int, y: int) -> tuple[Color, float]:
        p = self.a[y, x]
        return (p[0], p[1], p[2]), p[3]

    def sp(self, part: str, i: int, j: int, c: Color | None, alpha: float = 255.0) -> None:
        """带坐标写像素；c=None 抠掉（撕口 / 破洞）。"""
        ou, ov, _, sy, sz = BOXES[part]
        if 0 <= i < strip_width(part) and 0 <= j < sy:
            self.put(ou + i, ov + sz + j, c, alpha)

    def sget(self, part: str, i: int, j: int) -> tuple[Color, float]:
        ou, ov, _, _, sz = BOXES[part]
        return self.get(ou + i, ov + sz + j)

    # ---------------------------------------------------------------- 铺色
    def paint(self, part: str, fn: Callable, faces=SIDES, rows: tuple[int, int] | None = None,
              cols: tuple[int, int] | None = None) -> None:
        """fn(x, y, i, j, face, fx) → None | 颜色 | (颜色, alpha)。侧面按带坐标，top/bottom 的 i=fx、j=fy。"""
        _, _, sx, sy, sz = BOXES[part]
        starts = {"right": 0, "front": sz, "left": sz + sx, "back": 2 * sz + sx}
        for face in faces:
            x0, y0, fw, fh = face_rect(part, face)
            side = face in starts
            for fy in range(fh):
                if side and rows is not None and not (rows[0] <= fy < rows[1]):
                    continue
                for fx in range(fw):
                    i = starts[face] + fx if side else fx
                    if side and cols is not None and not (cols[0] <= i < cols[1]):
                        continue
                    out = fn(x0 + fx, y0 + fy, i, fy, face, fx)
                    if out is None:
                        continue
                    if len(out) == 2:
                        self.put(x0 + fx, y0 + fy, out[0], out[1])
                    else:
                        self.put(x0 + fx, y0 + fy, out)

    def erase(self, part: str, pts) -> None:
        for i, j in pts:
            self.sp(part, i, j, None)

    # ---------------------------------------------------------------- 后处理
    def stain(self, part: str, color: Color, strength: float, seed: int, scale: float = 2.6, thr: float = 0.55,
              rows: tuple[int, int] | None = None, faces=SIDES, cols: tuple[int, int] | None = None) -> None:
        """在已有像素上叠一层污渍 / 褪色：噪声超过 thr 的地方向 color 靠拢。"""
        ou, ov, sx, sy, sz = BOXES[part]
        starts = {"right": 0, "front": sz, "left": sz + sx, "back": 2 * sz + sx}
        for face in faces:
            x0, y0, fw, fh = face_rect(part, face)
            for fy in range(fh):
                if face in starts and rows is not None and not (rows[0] <= fy < rows[1]):
                    continue
                for fx in range(fw):
                    i = starts.get(face, 0) + fx
                    if face in starts and cols is not None and not (cols[0] <= i < cols[1]):
                        continue
                    c, al = self.get(x0 + fx, y0 + fy)
                    if al <= 0:
                        continue
                    n = vnoise(i + (0 if face in starts else 40), fy, seed, scale)
                    if n > thr:
                        t = min(1.0, (n - thr) / (1 - thr)) * strength
                        t = round(t * 3) / 3
                        self.put(x0 + fx, y0 + fy, mix(c, color, t), al)

    def patch(self, part: str, i0: int, j0: int, w: int, h: int, base: Color, seed: int, stitch: Color | None = None,
              grain: float = 0.05) -> None:
        """补丁：自带颗粒、上/左边压暗、下/右边提亮，边上隔一格一针缝线。"""
        for dj in range(h):
            for di in range(w):
                k = (h2(i0 + di, j0 + dj, seed) - 0.5) * 2 * grain
                if dj == 0 or di == 0:
                    k -= 0.10
                if dj == h - 1 or di == w - 1:
                    k += 0.05
                self.sp(part, i0 + di, j0 + dj, tone(base, quant(k)))
        if stitch is not None:
            sc = mix(base, stitch, 0.55)  # 针脚只比补丁亮一档，不抢成白框
            for di in range(0, w, 2):
                self.sp(part, i0 + di, j0, sc)
                self.sp(part, i0 + di, j0 + h - 1, sc)
            for dj in range(1, h - 1, 2):
                self.sp(part, i0, j0 + dj, sc)
                self.sp(part, i0 + w - 1, j0 + dj, sc)

    def image(self) -> Image.Image:
        return Image.fromarray(np.clip(self.a, 0, 255).astype(np.uint8), "RGBA")


# ───────────────────────────────────────────────────────────── 通用着色器
def fold_k(i: int, j: int, folds) -> float:
    """褶皱：folds = [(i0, j0, j1, slope)]，沿行斜着走的一条暗线，右侧一像素提亮。"""
    k = 0.0
    for i0, j0, j1, slope in folds:
        if j0 <= j < j1:
            ic = round(i0 + slope * (j - j0))
            if i == ic:
                k -= 0.2
            elif i == ic + 1:
                k += 0.07
            elif i == ic - 1:
                k -= 0.05
    return k


def cloth(base: Color, seed: int, grain: float = 0.05, folds=(), weave: float = 0.0, ao_rows: tuple[int, int] | None = None,
          sy: int = 12) -> Callable:
    """布料：颗粒 + 可选编织格 + 褶皱 + 下摆 / 袖口环境光遮蔽。"""

    def fn(x, y, i, j, face, fx):
        k = (h2(x, y, seed) - 0.5) * 2 * grain
        if weave:
            k += weave * (1 if (i + j) % 2 == 0 else -1)
        k += fold_k(i, j, folds)
        if face == "left":
            k -= 0.04
        elif face == "back":
            k -= 0.02
        if ao_rows is not None:
            r0, r1 = ao_rows
            if j >= r1 - 1:
                k -= 0.07
            elif j == r0:
                k += 0.03
        return tone(base, quant(k))

    return fn


def flat(c: Color) -> Callable:
    return lambda x, y, i, j, face, fx: c


def skin_tone(base: Color, seed: int, grain: float = 0.035) -> Callable:
    def fn(x, y, i, j, face, fx):
        k = (h2(x, y, seed) - 0.5) * 2 * grain
        return tone(base, quant(k, 0.05))

    return fn


# ───────────────────────────────────────────────────────────── 头：脸 + 头发
def paint_head(S: Skin, skin: Color, hair: Color, seed: int, front_h: list[int], side_h: list[int], back_h: int = 6,
               eye: Color = C("#2a2a30"), eye_white: Color = C("#dcd6c8"), brow: Color | None = None,
               mouth: Color | None = None, hair_hi: Color | None = None, eyes_y: int = 4, eye_gap: int = 2,
               hair_grain: float = 0.12, streak: Color | None = None, streak_frac: float = 0.0) -> None:
    """head 盒：肤色铺底 → 眼眉口 → 头发盖上去。

    front_h：正面 8 列各自的刘海高度（行数）；side_h：侧面 8 列（从后脑到额侧）的鬓发高度；back_h：后脑头发行数。
    """
    ou, ov, sx, sy, sz = BOXES["head"]
    S.paint("head", skin_tone(skin, seed + 1), faces=("right", "front", "left", "back", "top", "bottom"))
    # 下巴下的阴影（底面）与脖子根
    S.paint("head", lambda x, y, i, j, f, fx: tone(skin, -0.28), faces=("bottom",))
    # 脸：额头往下 3 行微暗（头发投影）
    fx0, fy0, _, _ = face_rect("head", "front")
    brow_c = brow or tone(hair, -0.1)
    # 眉
    for fx in (1, 2, 5, 6):
        S.put(fx0 + fx, fy0 + eyes_y - 1, brow_c)
    # 眼：外白内瞳（左右眼镜像：靠鼻梁一侧是瞳）
    lx = (1, 2)
    rx = (5, 6)
    S.put(fx0 + lx[0], fy0 + eyes_y, eye_white)
    S.put(fx0 + lx[1], fy0 + eyes_y, eye)
    S.put(fx0 + rx[0], fy0 + eyes_y, eye)
    S.put(fx0 + rx[1], fy0 + eyes_y, eye_white)
    # 鼻影、口
    S.put(fx0 + 3, fy0 + eyes_y + 1, tone(skin, -0.14))
    S.put(fx0 + 4, fy0 + eyes_y + 1, tone(skin, -0.08))
    mc = mouth or tone(skin, -0.3)
    S.put(fx0 + 3, fy0 + eyes_y + 2, mc)
    S.put(fx0 + 4, fy0 + eyes_y + 2, mc)
    # 耳：侧面中段
    for face in ("right", "left"):
        sx0, sy0, _, _ = face_rect("head", face)
        for dy in (3, 4):
            S.put(sx0 + 4, sy0 + dy, tone(skin, -0.12))
            S.put(sx0 + 3, sy0 + dy, tone(skin, -0.06))

    hi = hair_hi or tone(hair, 0.22)

    def hair_px(x, y, strand_key, rows_depth):
        """发丝顺着一列一列走：整列一个明暗档，再叠很轻的逐点噪声；少数列换成杂色（灰发 / 挑染）。"""
        n = h2(strand_key, 0, seed + 7)
        m = h2(x, y, seed + 9)
        base = streak if (streak is not None and h2(strand_key, 1, seed + 8) < streak_frac) else hair
        k = (-0.14, -0.04, 0.06, 0.17)[int(n * 4) % 4] + (m - 0.5) * hair_grain * 0.5
        c = tone(base, quant(k, 0.05))
        if m > 0.965 and rows_depth <= 1:
            c = hi
        return c

    # 顶面
    S.paint("head", lambda x, y, i, j, f, fx: hair_px(x, y, fx, j), faces=("top",))
    # 后脑
    S.paint("head", lambda x, y, i, j, f, fx: hair_px(x, y, fx + 20, j) if j < back_h else None, faces=("back",))
    # 正面刘海
    S.paint("head", lambda x, y, i, j, f, fx: hair_px(x, y, fx + 40, j) if j < front_h[fx] else None, faces=("front",))
    # 侧面：右侧面 fx 0=后 … 7=前；左侧面反过来
    S.paint("head", lambda x, y, i, j, f, fx: hair_px(x, y, fx + 60, j) if j < side_h[fx] else None, faces=("right",))
    S.paint("head", lambda x, y, i, j, f, fx: hair_px(x, y, fx + 80, j) if j < side_h[7 - fx] else None, faces=("left",))

    # 脸部体积：刘海下投影、鼻梁提亮、颧骨微红、下颌线压暗
    for fx in range(8):
        row = front_h[fx]
        if row < 8:
            c, a = S.get(fx0 + fx, fy0 + row)
            S.put(fx0 + fx, fy0 + row, tone(c, -0.12), a)
    c, a = S.get(fx0 + 3, fy0 + eyes_y)
    S.put(fx0 + 3, fy0 + eyes_y, tone(c, 0.08), a)
    for fx in (1, 6):
        c, a = S.get(fx0 + fx, fy0 + eyes_y + 2)
        S.put(fx0 + fx, fy0 + eyes_y + 2, mix(c, C("#c0705a"), 0.18), a)
    for fx in range(8):
        c, a = S.get(fx0 + fx, fy0 + 7)
        S.put(fx0 + fx, fy0 + 7, tone(c, -0.1), a)


# ───────────────────────────────────────────────────────────── 手：袖口以下露出的前臂 / 手
def paint_hands(S: Skin, skin: Color, seed: int, rows_from: int = 9) -> None:
    for part in ("rarm", "larm"):
        S.paint(part, skin_tone(skin, seed + 11), rows=(rows_from, 12))
        S.paint(part, lambda x, y, i, j, f, fx: tone(skin, -0.22), faces=("bottom",))


# ───────────────────────────────────────────────────────────── 做旧：褶皱 / 积灰 / 局部调色
def adjust(S: Skin, part: str, i: int, j: int, k: float) -> None:
    """把带坐标 (i, j) 已有像素明暗调 k，透明像素不动。"""
    c, a = S.sget(part, i, j)
    if a > 0:
        S.sp(part, i, j, tone(c, k), a)


def wrinkles(S: Skin, part: str, j: int, seed: int, cols: tuple[int, int] | None = None, density: float = 0.55,
             dark: float = -0.16, light: float = 0.06, scale: float = 2.0) -> None:
    """一行横向皱褶：噪声超阈值的一段压暗，正下方一行提亮（关节弯折、腰束处）。"""
    w = strip_width(part)
    i0, i1 = cols if cols else (0, w)
    for i in range(i0, i1):
        if vnoise(i, j, seed, scale) > 1 - density:
            adjust(S, part, i, j, dark)
            adjust(S, part, i, j + 1, light)


def dust_gradient(S: Skin, part: str, color: Color, j0: int, j1: int, p_max: float, seed: int, t_max: float = 0.4,
                  faces=SIDES) -> None:
    """下摆 / 裤脚 / 鞋面积灰：越靠下越多，向 color 靠拢。"""
    for j in range(j0, j1):
        p = p_max * (j - j0 + 1) / (j1 - j0)
        for i in range(strip_width(part)):
            if h2(i, j, seed) < p:
                c, a = S.sget(part, i, j)
                if a > 0:
                    S.sp(part, i, j, mix(c, color, t_max * (0.6 + 0.4 * h2(j, i, seed + 1))), a)


def freckles(S: Skin, part: str, rows: tuple[int, int], color: Color, p: float, seed: int, strength: float = 0.5,
             cols: tuple[int, int] | None = None) -> None:
    """零星色点（矿粉、骨粉、草汁、血迹…）：随机像素向 color 靠拢。"""
    w = strip_width(part)
    i0, i1 = cols if cols else (0, w)
    for j in range(*rows):
        for i in range(i0, i1):
            if h2(i, j, seed) < p:
                c, a = S.sget(part, i, j)
                if a > 0:
                    S.sp(part, i, j, mix(c, color, strength * (0.6 + 0.4 * h2(i + 9, j, seed))), a)


def hat_hair(S: Skin, hair: Color, seed: int, back_rows: int = 7, side_rows: int = 5, side_cols: int = 4,
             top_wisps: int = 4, streak: Color | None = None, streak_frac: float = 0.0) -> None:
    """hat 层的头发：后脑披下的一层（下缘参差）、侧面后半、顶上几缕翘起。"""

    def px(x, y, key):
        n = h2(key, 3, seed + 71)
        m = h2(x, y, seed + 72)
        base = streak if (streak is not None and h2(key, 5, seed + 73) < streak_frac) else hair
        k = (-0.16, -0.05, 0.05, 0.16)[int(n * 4) % 4] + (m - 0.5) * 0.06
        return tone(base, quant(k, 0.05))

    S.paint("hat", lambda x, y, i, j, f, fx: px(x, y, fx + 10) if j < back_rows - (1 if h2(fx, 1, seed + 74) > 0.6 else 0)
            - (1 if h2(fx, 2, seed + 75) > 0.75 else 0) else None, faces=("back",))
    S.paint("hat", lambda x, y, i, j, f, fx: px(x, y, fx + 30) if fx < side_cols and j < side_rows - (1 if h2(fx, j, seed + 76) > 0.7 else 0) else None,
            faces=("right",))
    S.paint("hat", lambda x, y, i, j, f, fx: px(x, y, fx + 50) if fx >= 8 - side_cols and j < side_rows - (1 if h2(fx, j, seed + 77) > 0.7 else 0) else None,
            faces=("left",))
    for k in range(top_wisps):
        fx, fy = int(h2(k, 0, seed + 78) * 8), int(h2(k, 1, seed + 78) * 8)
        x0, y0, _, _ = face_rect("hat", "top")
        S.put(x0 + fx, y0 + fy, tone(hair, 0.06))
