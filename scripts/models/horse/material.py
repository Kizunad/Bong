#!/usr/bin/env python3
"""材质层：一格贴图到底怎么刷 —— 骨 / 肌 / 皮 / 马具共用这一份。

一格贴图 8×8 texel，一个面把其中一条带整片拉过去。首版一格里平涂一个颜色、加 ±10
的噪，于是"铁"和"羊毛"在渲染上是**同一件事**：一片没有明暗跨度的中间色。

**金属之所以看起来像金属，靠的不是色相是跨度**——坑里近黑、棱上近白，同一块面上差出
好几档。平涂再怎么挑色相都只能得到一块塑料；挑得再准，也只是"另一种颜色的羊毛"。
反过来也一样：毛要是也有金属那种跨度，就成了塑料马。所以材质不是一个 RGB，是
**「主色 + 做工」**——色相归主色，层次全归做工（`Finish`）。

一格分三条带刷：

  · **侧面带**（第 1-4 行，贴四个侧面）—— 面积的九成九。按做工上纹理：金属打坑与磨亮
    的划痕、锁环密排环点、布走织纹、绳走搓股、革走斑、毛走顺毛的细丝、角质走生长纹。
    纹理的乘数**归一到均值 1**，主色仍然等于表里写的那个 RGB —— 上游那些算了好几轮
    才推到位的配色距离一个都不动，这一层只改"同一块颜色内部有多少层次"。
  · **受光带**（第 5 行，贴 up 面）与**背光带**（第 6 行，贴 down 面）。札甲是上排压
    下排，叠边在侧视里就是一条线；马背朝天、马腹朝地，也是同一件事。把这条线一亮一暗
    地钉死，远处第一眼读到的就是它。
  · 第 0 / 7 行是**防渗行**：贴图缩略（mipmap）时相邻格互相渗色，两头照各自邻带同色
    刷，渗过来也还是自己的颜色。左右两列同理。

提亮往**冷白**里插值、压暗往**暖黑**里插值，不是单纯乘一个数：近黑的重甲乘 1.34 还是
近黑，棱线出不来。真实里边缘那圈亮本来也不是漫反射变亮，是掠射角的反射——对越暗的
材质越明显。压暗那头同理：暗部不是纯黑，是环境色兜底。
"""

from __future__ import annotations

import math
from dataclasses import dataclass

SWATCH = 8
SIDE_W, SIDE_H = SWATCH - 2, 4  # 侧面带 6×4（四周各留一格防渗）
GRAIN_FRES = 0.5  # 纹理只吃一半的冷白插值：坑与划痕是表面微起伏，不是边缘反射


@dataclass(frozen=True)
class Finish:
    """做工：这块材质怎么和光打交道。"""

    lit: float  # 受光面相对主色的明度倍率
    occ: float  # 背光面
    fres: float  # 提亮时往冷白里插多少（暗材质全靠它出棱）
    grain: str  # 侧面带纹理
    amp: float  # 纹理强度


def lum(rgb) -> float:
    return 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]


def _lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def tone(rgb: tuple[int, int, int], k: float, fres: float) -> tuple[int, int, int]:
    """按明度倍率 k 取一档色。提亮往冷白插、压暗往暖黑插，不是单纯乘一个数。

    暗材质是这条的全部理由：重甲 (44,46,54) 乘 1.34 还是 (59,62,72)，肉眼分不出，棱线
    等于没做。往冷白插值之后才出得来。
    """
    if k >= 1.0:
        f, tgt = min(1.0, (k - 1.0) * 1.6) * fres, (214, 226, 242)
    else:
        f, tgt = min(1.0, (1.0 - k) * 0.9) * 0.42, (24, 21, 20)
    return tuple(max(0, min(255, round(_lerp(c * k, q, f)))) for c, q in zip(rgb, tgt))


def grain(kind: str, amp: float, seed: int) -> list[list[float]]:
    """侧面带 6×4 的明度乘数场，**均值归一到 1**（主色不因上纹理而漂）。"""
    hs = [[((x * 73856093) ^ (y * 19349663) ^ (seed * 83492791)) % 9973 for x in range(SIDE_W)]
          for y in range(SIDE_H)]
    order = sorted((hs[y][x], x, y) for y in range(SIDE_H) for x in range(SIDE_W))
    g = [[1.0] * SIDE_W for _ in range(SIDE_H)]
    if kind == "pit":
        # 斜向的受光坡打底，再**定量**打坑与划亮。按哈希取模碰运气会有材质一个坑都没
        # 有，而"有没有跨度"正是这一层唯一要保证的事，不能听天由命——所以按哈希排名
        # 取定数：最低的三格打坑，最高的两格磨亮。
        span = (SIDE_W - 1) + (SIDE_H - 1) * 1.6
        for y in range(SIDE_H):
            for x in range(SIDE_W):
                g[y][x] = 1.0 + amp * 0.42 * (1.0 - 2.0 * (x + y * 1.6) / span)
        for _h, x, y in order[:3]:
            g[y][x] -= amp * 1.55  # 锻打的坑 / 锈蚀点
        for _h, x, y in order[-2:]:
            g[y][x] += amp * 1.75  # 磨亮的棱与划痕
    elif kind == "ring":  # 锁环：亮的是环顶、暗的是环之间的洞
        for y in range(SIDE_H):
            for x in range(SIDE_W):
                g[y][x] = 1.0 + amp * (1.0 if (x + y % 2) % 2 == 0 else -1.0)
    elif kind == "weave":  # 织纹
        for y in range(SIDE_H):
            for x in range(SIDE_W):
                g[y][x] = 1.0 + amp * (1.0 if x % 2 == y % 2 else -1.0)
        for _h, x, y in order[:2]:
            g[y][x] -= amp * 1.6  # 起球 / 磨薄
    elif kind == "twist":  # 搓股
        for y in range(SIDE_H):
            for x in range(SIDE_W):
                g[y][x] = 1.0 + amp * (1.0, 0.15, -1.15)[(x + y) % 3]
    elif kind == "mottle":  # 鞣不匀的斑 + 一道折痕
        for y in range(SIDE_H):
            for x in range(SIDE_W):
                g[y][x] = 1.0 + amp * math.sin((x * 1.7 + y * 2.9 + seed) * 0.9)
        for _h, x, y in order[:1]:
            g[y][x] -= amp * 1.4
    elif kind == "fur":
        # 毛：跨度必须小（一旦有金属那种跨度就成了塑料马），但不能是零，零就是羊毛。
        #
        # **不许有周期**。这一格只有 6×4 个 texel，却要整片拉过整只桶身——任何规则
        # 图案（哪怕一格一格的隔行）拉伸之后都是几道贯穿整只马的宽条，读成灯芯绒不是
        # 毛（首版按"顺毛细丝"做隔列明暗，渲出来就是这样）。所以走无周期的细杂色，
        # 外加一两撮暗毛当旋。
        for y in range(SIDE_H):
            for x in range(SIDE_W):
                g[y][x] = 1.0 + amp * ((hs[y][x] % 1000) / 500.0 - 1.0)
        for _h, x, y in order[:2]:
            g[y][x] -= amp * 1.2  # 旋毛 / 泥点
    elif kind == "hair":
        # 鬃尾：一绺一绺的粗丝，跨度比毛大得多——远处认得出"这是鬃不是身上的毛"
        for y in range(SIDE_H):
            for x in range(SIDE_W):
                g[y][x] = 1.0 + amp * (1.0, -0.35, -1.0, 0.35)[(x + y // 2) % 4]
    elif kind == "horn":  # 角质（蹄 / 附蝉）：横向生长纹 + 一点油光
        for y in range(SIDE_H):
            for x in range(SIDE_W):
                g[y][x] = 1.0 + amp * math.cos(y * 2.4 + 0.35 * math.sin(x * 1.3 + seed))
    elif kind != "flat":
        raise ValueError(f"未知纹理 {kind}")
    m = sum(sum(r) for r in g) / (SIDE_W * SIDE_H)
    return [[v / m for v in r] for r in g]


def cell(rgb: tuple[int, int, int], F: Finish, seed: int) -> list[list[tuple[int, int, int]]]:
    """一格 8×8：0 防渗 / 1-4 侧面 / 5 受光 / 6 背光 / 7 防渗。"""
    g = grain(F.grain, F.amp, seed)
    side = [[tone(rgb, g[y][x], F.fres * GRAIN_FRES) for x in range(SIDE_W)] for y in range(SIDE_H)]
    lit, occ = tone(rgb, F.lit, F.fres), tone(rgb, F.occ, F.fres)
    out = []
    for y in range(SWATCH):
        row = side[min(max(y - 1, 0), SIDE_H - 1)] if y <= SIDE_H else None
        out.append([(row[min(max(x - 1, 0), SIDE_W - 1)] if row else (lit if y == SIDE_H + 1 else occ))
                    for x in range(SWATCH)])
    return out


def paint(px, ox: int, oy: int, rgb, F: Finish, seed: int) -> None:
    """把一格刷进贴图。"""
    c = cell(rgb, F, seed)
    for y in range(SWATCH):
        for x in range(SWATCH):
            px[ox + x, oy + y] = (*c[y][x], 255)


def band_faces(ox: int, oy: int) -> dict:
    """六个面各取哪条带。up / down 取受光 / 背光带，四个侧面取侧面带。"""
    bands = {"up": (oy + SIDE_H + 1.15, oy + SIDE_H + 1.85),
             "down": (oy + SIDE_H + 2.15, oy + SIDE_H + 2.85)}
    out = {}
    for d in ("north", "south", "east", "west", "up", "down"):
        v0, v1 = bands.get(d, (oy + 1.0, oy + 1.0 + SIDE_H))
        out[d] = {"uv": [ox + 1.0, v0, ox + SWATCH - 1.0, v1], "texture": 0}
    return out


def side_mean(rgb, F: Finish, seed: int) -> tuple[int, int, int]:
    """一块材质**实际刷出来**的侧面均色 —— 配色判据量的是这个，不是名义值。

    名义值是"打算刷成什么"，刷出来才是"远处看见什么"。两者现在只差一两个数（纹理
    归一过），但判据要是盯着名义值，往后谁把纹理改偏了它就成了睁眼瞎。
    """
    c = cell(rgb, F, seed)
    px = [c[y][x] for y in range(1, SIDE_H + 1) for x in range(1, SIDE_W + 1)]
    return tuple(round(sum(p[i] for p in px) / len(px)) for i in range(3))


def side_span(rgb, F: Finish, seed: int) -> float:
    """侧面带最亮 / 最暗的**比值**。"铁看起来跟羊毛一样"量的就是它。

    比值不是差值：同一个绝对差在浅色上不显眼、在近黑上刺眼，一刀切的差值会逼着最暗
    那档去做一道它不该有的白边。
    """
    c = cell(rgb, F, seed)
    ls = [lum(c[y][x]) for y in range(1, SIDE_H + 1) for x in range(1, SIDE_W + 1)]
    return max(ls) / max(1.0, min(ls))


def edge_ratio(rgb, F: Finish) -> float:
    """受光带 / 背光带的亮度比 —— 叠边（与马背↔马腹）那条线的全部来源。"""
    return lum(tone(rgb, F.lit, F.fres)) / max(1.0, lum(tone(rgb, F.occ, F.fres)))


def check_finishes(mats: dict, finish_of, span_lim: dict[str, tuple[float, float]],
                   min_edge: float, order: list | None = None) -> list[str]:
    """**该有跨度的要有，不该有的不许有** —— 抱怨"铁跟羊毛一样"这句话的判据化。

    羊毛就是一片平涂的中间色；金属和它的全部区别在于同一块面上差得出几档。这件事在
    渲染图上一眼可辨，但**改色相改不动它**——一格里只有一个颜色的时候，铁色挑来挑去
    挑到的永远是"另一种颜色的羊毛"。所以判据量的不是颜色，是明度跨度。

    上界和下界一样是判据：毛要是也闪，就成了塑料马。跨度不是越大越好，是每种做工各有
    各的量——这正是"材质"和"随便加点噪"的区别。
    """
    bad = []
    keys = order or list(mats)
    for key in keys:
        F = finish_of(key)
        rgb = mats[key]
        sp = side_span(rgb, F, keys.index(key) + 1)
        lo, hi = span_lim[F.grain]
        if not lo <= sp <= hi:
            bad.append(f"材质 {key}（{F.grain}）侧面明暗比 {sp:.2f}，要求 {lo:.2f}-{hi:.2f}"
                       f"{'——平得像羊毛' if sp < lo else '——不该这么闪'}")
        e = edge_ratio(rgb, F)
        if F.grain != "flat" and e < min_edge:
            bad.append(f"材质 {key} 的受光带只比背光带亮 {e:.2f} 倍（下限 {min_edge:.2f}），"
                       f"朝天面与朝地面分不出来")
    return bad
