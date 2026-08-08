#!/usr/bin/env python3
"""材质层：一格贴图到底怎么刷 —— 骨 / 肌 / 皮 / 马具共用这一份。

一格贴图 8×8 texel，一个面把其中一条带整片拉过去。首版一格里平涂一个颜色、加 ±10
的噪，于是"铁"和"羊毛"在渲染上是**同一件事**：一片没有明暗跨度的中间色。

**金属之所以看起来像金属，靠的不是色相是跨度**——坑里近黑、棱上近白，同一块面上差出
好几档。平涂再怎么挑色相都只能得到一块塑料；挑得再准，也只是"另一种颜色的羊毛"。
反过来也一样：毛要是也有金属那种跨度，就成了塑料马。所以材质不是一个 RGB，是
**「主色 + 做工」**——色相归主色，层次全归做工（`Finish`）。

一格分三条带刷：

  · **侧面带**（第 1-4 行，贴四个侧面）—— 面积的九成九。按做工上纹理：札甲走上暗下亮
    的叠边加铆钉、金属打坑与磨亮的划痕、锁环走横排、布走纬线、绳走搓股、革走斑、
    毛走无周期的细杂色、角质走生长纹。纹理的乘数**归一到均值 1**，主色仍然等于表里
    写的那个 RGB —— 上游那些算了好几轮才推到位的配色距离一个都不动，这一层只改
    "同一块颜色内部有多少层次"。

    纹理一律**走横向结构，不撒匀噪**。一格只有 6×4 个 texel，要整片拉过一块几个单位
    见方的甲：匀撒的噪拉伸之后就是一张大格子的棋盘（首版的坑、锁环、织纹全是棋盘，
    渲出来整只马糊着一层格子——"疙疙瘩瘩"说的就是它）。跨度改由**两三条结构线**担：
    比值只看最亮 / 最暗两格，与"有多少格是噪"无关，所以"金属要有跨度"和"甲面要干净"
    可以同时成立。横向也不是随手挑的：四个侧面的 v 轴恒是世界 y，横条顺着马身走。
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
        # 竖向的受光坡打底（上亮下暗，像一段被天光照着的圆面），再**定量**打一个坑、
        # 划一道亮。
        #
        # 跨度由**这两格**担，不由整片噪担——这是"有材质"和"疙疙瘩瘩"的分界线。一格
        # 只有 6×4 个 texel，却要整片拉过一块几个单位见方的甲：撒得越匀，拉伸之后越像
        # 一张大格子的棋盘（首版三坑两亮，渲出来整只马糊着一层格子）。判据要的是明暗
        # 比，比值由最亮 / 最暗两格决定，与"有多少格是噪"无关——所以噪减到最少、跨度
        # 一点不让。
        # 坡走 y 不走斜向：四个侧面的 v 轴恒是世界 y，走 y 就是一道横的明暗，顺着马身；
        # 走斜向拉伸之后是一道斜跨整片甲的带。
        for y in range(SIDE_H):
            for x in range(SIDE_W):
                g[y][x] = 1.0 + amp * 0.34 * (1.0 - 2.0 * y / (SIDE_H - 1))
        for _h, x, y in order[:1]:
            g[y][x] -= amp * 1.75  # 锻打的坑 / 锈蚀点
        for _h, x, y in order[-1:]:
            g[y][x] += amp * 1.95  # 磨亮的棱与划痕
    elif kind == "ring":
        # 锁环：环是**一排一排**穿的，所以走横条不走棋盘——亮的是这一排环顶，暗的是
        # 排与排之间的缝。棋盘那一版在肋侧拉成了一片大格子（同 `pit` 的道理）。
        for y in range(SIDE_H):
            for x in range(SIDE_W):
                g[y][x] = 1.0 + amp * (1.0 if y % 2 == 0 else -1.0)
        for _h, x, y in order[:1]:
            g[y][x] -= amp * 1.15  # 掉环 / 锈死的一小片
    elif kind == "lamellar":
        # 札甲：一排札片。**横向结构，竖向干净**——这是参考图里那身甲的全部读法。
        #   · 下缘一道亮：札片是上排压下排、下缘露在外面的那条自由边，迎光最亮；
        #   · 上缘一道暗：被上一排压着的阴影；
        #   · 中间两行**基本平**，只在一行上点两颗铆钉。
        # 跨度全部由这三条结构线担，所以"金属要有跨度"和"甲面不许疙疙瘩瘩"可以同时
        # 成立——真正让人觉得脏的是**匀撒的噪**，不是明暗差本身。
        for x in range(SIDE_W):
            g[0][x] = 1.0 - amp * 1.05  # 上缘：压在下面的阴影
            g[SIDE_H - 1][x] = 1.0 + amp * 1.25  # 下缘：露在外的自由边
        for x in (1, SIDE_W - 2):
            g[1][x] += amp * 1.05  # 铆钉
        g[2][(SIDE_W - 1) // 2] -= amp * 0.55  # 编绳穿过的那个孔
    elif kind == "quilt":
        # 绗缝的衬里：一道一道横着的棉行，行与行之间是缝线压出的沟。布的跨度本来就小，
        # 这里靠**结构**而不是靠强度被认出来（同 `lamellar` 的道理，只是幅度小一档）。
        for y in range(SIDE_H):
            for x in range(SIDE_W):
                g[y][x] = 1.0 + amp * (0.9 if y % 2 == 0 else -0.9)
        for x in range(0, SIDE_W, 3):
            g[1][x] -= amp * 0.8  # 缝线的针脚
    elif kind == "weave":  # 织纹：横着的纬线。棋盘那一版在毡垫上拉成了大格子
        for y in range(SIDE_H):
            for x in range(SIDE_W):
                g[y][x] = 1.0 + amp * (0.8 if y % 2 == 0 else -0.8)
        for _h, x, y in order[:1]:
            g[y][x] -= amp * 1.8  # 起球 / 磨薄
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


def band_faces(ox: int, oy: int, edge: bool = True) -> dict:
    """六个面各取哪条带。up / down 取受光 / 背光带，四个侧面取侧面带。

    `edge=False` 时六个面**一律走侧面带**。受光 / 背光带说的是"叠边在侧视里的那条
    线"——它假设 up 面是一条**窄边**（札片薄、竖着立，从侧面看它的顶就是一条线）。
    把同一条带贴到一块平躺的板上就完全不是那么回事：那块板的 up 面是它**最大的一片**，
    整片被刷成受光色，渲出来是一块比周围亮两档的白板（面帘的顶板、搭后的盖子实测就是
    这样，远看像给马扣了块石板）。所以谁是窄边由**盒子自己的尺寸**决定，不由材质决定。
    """
    bands = {"up": (oy + SIDE_H + 1.15, oy + SIDE_H + 1.85),
             "down": (oy + SIDE_H + 2.15, oy + SIDE_H + 2.85)} if edge else {}
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
