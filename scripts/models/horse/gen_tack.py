#!/usr/bin/env python3
"""马 —— 马具层（第四层）：蹄铁 / 鞍 / 缰 / 甲。

前三层（骨 → 肌 → 皮）是"马本身"，这一层是**挂在马身上的东西**。

三条结构约定：

  · **马具单出文件，但共用皮层那套骨骼树**——同一份动画能同时驱动马与马具，换装只换
    一份马具文件，不必为每种装备重出九份皮（马具与毛色无关，出 3 档而不是 9 份）。
  · **尺寸一律从皮层的实件量出来**，不另写一套数字。蹄铁的内外径读的是 `hoof_*` 那两
    个 cube 的真实 from/to，皮层一改（换体型比例、蹄盘加大），马具自动跟着变。这是本
    仓库三层模型一贯的推导方向：下游读上游，永不回写。
  · **贴合与露出都要断言**。马具最容易的两种翻车都在渲染图上看不出来：埋进皮里（等于
    没做）、和皮差半个单位（悬空）。所以"咬住蹄"和"露在蹄外"各是一条硬断言。

分级走矿物正典的金属阶梯（`server/src/mineral/registry.rs`）：
粗铁（凡）→ 杂钢（凡）→ 灵铁（灵）。**不用"玄铁"**——worldview §三 L63 禁玄/陨/星/仙/太/古。

用法:
  python3 scripts/models/horse/gen_tack.py                      # 全部马具 × 三档
  python3 scripts/models/horse/gen_tack.py --kind shoe --tier lingtie --profile large
  python3 scripts/models/horse/gen_tack.py --with-horse         # 叠在皮层上看贴合
  python3 scripts/models/horse/gen_tack.py --list
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import re
from dataclasses import dataclass

import numpy as np
from gen_muscle import Skeleton
from gen_pelt import FINAL, STAGES, SWATCH, _corners
from gen_skeleton import PROFILES, Profile, connected_components, uid
from PIL import Image

TACK_DIR = FINAL / "tack"
TACK_ROW = 4  # 贴图第 5 行起（0-1 行骨/肌，2-3 行皮），追加不动已有 UV
GEOM_COAT = "rust"  # 几何三色同源，取一份当尺寸来源（另两份由 check_geom_same_across_coats 对拍）

Vec = tuple[float, float, float]

# 材质表**只准追加不准插入**：UV 索引由这里的顺序派生，插一行会把已出的马具整体错位。
TACK_MATS: dict[str, tuple[int, int, int]] = {
    # 粗铁走**锈色**而不是灰：灰的粗铁 (96,88,80) 和碎雪的蹄 (92,86,80) 只差 4.5，
    # 整只马渲出来和赤脚一模一样（见 check_contrast）。锈也更合"捡来的铁料"这档。
    "iron_crude": (148, 106, 72),
    "iron_crude_dark": (110, 76, 52),
    "iron_rust": (116, 74, 44),  # 锈斑 / 锈钉
    "steel": (132, 134, 138),  # 杂钢：冷灰，比粗铁亮两档
    "steel_dark": (94, 96, 101),
    # 灵铁压深、往蓝里推：青灰 (78,88,102) 离碎雪的蹄只有 26，同样糊在一起。
    "lingtie": (48, 70, 122),
    "lingtie_dark": (34, 48, 88),
    "glow": (152, 216, 240),  # 灵纹：淡蓝
    "nail": (156, 152, 144),  # 钉头：磨亮的白铁
}


def _faces(mat: str) -> dict:
    i = list(TACK_MATS).index(mat)
    ox, oy = (i % 8) * SWATCH, (TACK_ROW + i // 8) * SWATCH
    uv = [ox + 1.0, oy + 1.0, ox + SWATCH - 1.0, oy + SWATCH - 1.0]
    return {d: {"uv": list(uv), "texture": 0} for d in ("north", "south", "east", "west", "up", "down")}


def extend_texture(data: dict) -> None:
    """追加马具色块。灵纹那格不加噪：发光条要的是均匀亮度，撒噪点会读成脏。"""
    src = data["textures"][0]["source"].split(",", 1)[1]
    img = Image.open(io.BytesIO(base64.b64decode(src))).convert("RGBA")
    px = img.load()
    for i, (key, (r, g, b)) in enumerate(TACK_MATS.items()):
        ox, oy = (i % 8) * SWATCH, (TACK_ROW + i // 8) * SWATCH
        for y in range(SWATCH):
            for x in range(SWATCH):
                n = 0 if key == "glow" else ((x * 7 + y * 13 + i * 5) % 5) - 2
                px[ox + x, oy + y] = (
                    max(0, min(255, r + n * 5)),
                    max(0, min(255, g + n * 5)),
                    max(0, min(255, b + n * 4)),
                    255,
                )
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    data["textures"][0]["source"] = "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()


class Tack:
    def __init__(self, skel: Skeleton, P: Profile) -> None:
        self.skel = skel
        self.P = P
        self.count = 0
        self._names: set[str] = set()

    def box(self, bone: str, name: str, frm: Vec, to: Vec, *, mat: str, glow: bool = False,
            rot=None, org=None) -> None:
        if name in self._names:
            raise ValueError(f"重复马具件名: {name}（uuid 由名字派生，名字必须唯一）")
        self._names.add(name)
        if mat not in TACK_MATS:
            raise ValueError(f"未知马具材质 {mat}")
        f = [round(min(a, b), 3) for a, b in zip(frm, to)]
        t = [round(max(a, b), 3) for a, b in zip(frm, to)]
        if any(t[i] - f[i] < 1e-6 for i in range(3)):
            raise ValueError(f"{name} 是退化盒（某轴厚度为 0）: {f} → {t}")
        self.skel.attach(
            bone,
            {
                "name": name,
                "box_uv": False,
                "rescale": False,
                "locked": False,
                "render_order": "default",
                "allow_mirror_modeling": True,
                "type": "cube",
                "uuid": uid("tack", name),
                "_tack": True,
                # 发光标记：bbmodel 本身没有逐面自发光，引擎侧（GeckoLib emissive 层）
                # 按这个标记挑件。造型层是唯一知道"哪条是灵纹"的人，所以在这里声明。
                "_glow": glow,
                "from": f,
                "to": t,
                "autouv": 0,
                "color": 5,
                "origin": [round(v, 3) for v in (org or [(a + b) / 2 for a, b in zip(f, t)])],
                "rotation": [round(v, 3) for v in (rot or (0.0, 0.0, 0.0))],
                "faces": _faces(mat),
            },
        )
        self.count += 1


# ================================================================ 蹄铁
@dataclass(frozen=True)
class HoofFit:
    """一只蹄的**实测**尺寸，读自皮层的 `hoof_*` / `hoof_top_*` 两件。

    蹄铁不自己算蹄在哪——皮层的蹄是从骨架的蹄关节推的，再推一次必然漂移。
    """

    key: str  # f_l / f_r / h_l / h_r
    bone: str
    x0: float
    x1: float
    wall: float  # 下段蹄壁顶面（自 y=0）
    crown: float  # 蹄冠顶
    z_toe: float  # 蹄尖（−z）
    z_heel: float  # 蹄踵（+z）

    @property
    def cx(self) -> float:
        return (self.x0 + self.x1) / 2

    @property
    def half(self) -> float:
        return (self.x1 - self.x0) / 2

    @property
    def span(self) -> float:
        return self.z_heel - self.z_toe


def read_hooves(data: dict) -> dict[str, HoofFit]:
    els = {e["name"]: e for e in data["elements"] if e.get("_pelt")}
    out: dict[str, HoofFit] = {}
    for tag in ("f", "h"):
        for side in ("l", "r"):
            key = f"{tag}_{side}"
            lo, up = els.get(f"hoof_{key}"), els.get(f"hoof_top_{key}")
            if lo is None or up is None:
                raise SystemExit(f"皮层里找不到 hoof_{key} / hoof_top_{key} —— 蹄铁靠它们定位")
            if any(lo["rotation"]):
                raise SystemExit(f"hoof_{key} 带了旋转，蹄铁的轴对齐推导不再成立")
            out[key] = HoofFit(
                key=key,
                bone=f"hoof_{tag}_{side}",
                x0=lo["from"][0],
                x1=lo["to"][0],
                wall=lo["to"][1],
                crown=up["to"][1],
                z_toe=lo["from"][2],
                z_heel=lo["to"][2],
            )
    return out


@dataclass(frozen=True)
class ShoeSpec:
    """一档蹄铁。所有尺寸都是**蹄半宽的倍数**——蹄铁是配着蹄打的，不是配着鬐甲高打的，
    所以基准取蹄而不是体型。三档换算到常马前蹄（半宽 1.17 单位）约：
    铁条厚 0.26 / 0.37 / 0.47，外扩 0.15 / 0.25 / 0.32。
    """

    key: str
    label: str
    blurb: str
    mat: str
    mat_dark: str
    mat_nail: str
    thick: float  # 铁条厚（y 向）
    over: float  # 外扩出蹄壁
    bite: float  # 内咬进蹄底
    nails: int  # 每侧钉数
    toe_clip: float  # 趾夹高（× 蹄壁高）；0 = 无
    quarter_clip: float  # 侧夹高（× 蹄壁高）；0 = 无
    caulk: float  # 踵铁加高（× 铁条厚）；0 = 无
    glow: bool = False


SHOES: dict[str, ShoeSpec] = {
    # 一档：捡来的铁料现敲的，条细、无夹无踵，钉子还带锈。
    "cutie": ShoeSpec(
        key="cutie",
        label="粗铁蹄铁",
        blurb="残土上最常见的那种：一条铁敲弯了钉上去，磨平就换。",
        mat="iron_crude",
        mat_dark="iron_crude_dark",
        mat_nail="iron_rust",
        thick=0.26,
        over=0.16,
        bite=0.24,
        nails=2,
        toe_clip=0.0,
        quarter_clip=0.0,
        caulk=0.0,
    ),
    # 二档：正经锻出来的。加**趾夹**（前壁那片上翻的舌）与**踵铁**（后端垫高防滑）——
    # 这两处是侧视里一眼能和一档分开的地方。
    "zagang": ShoeSpec(
        key="zagang",
        label="杂钢蹄铁",
        blurb="杂钢回炉锻的，带趾夹与踵铁，能在碎石坡上咬住地。",
        mat="steel",
        mat_dark="steel_dark",
        mat_nail="nail",
        thick=0.32,
        over=0.21,
        bite=0.30,
        nails=3,
        toe_clip=0.52,
        quarter_clip=0.0,
        caulk=1.6,
    ),
    # 三档：灵铁。趾夹 + 两侧夹 + 一圈灵纹（细淡蓝发光条）。
    "lingtie": ShoeSpec(
        key="lingtie",
        label="灵铁蹄铁",
        blurb="灵铁所制，蹄铁一圈刻着导气的细纹，落地时泛淡蓝。",
        mat="lingtie",
        mat_dark="lingtie_dark",
        mat_nail="nail",
        thick=0.40,
        over=0.27,
        bite=0.34,
        nails=3,
        toe_clip=0.72,
        quarter_clip=0.42,
        caulk=1.8,
        glow=True,
    ),
}


HEADROOM = 0.03  # 上长件与"头顶那件皮"之间留的空隙

# 蹄铁底面离蹄底的那一线（× 铁条厚）。两条独立的理由都指向"别贴着 y=0 做"：
#   · **共面 z-fighting**：铁条底面与蹄底同在 y=0，从下方看是一圈闪烁的 U。
#   · **铲地**：悬出蹄壁的外缘离蹄骨转心最远，蹄一翻它先着地。灵铁档矮马实测在 death
#     的 t=0.66 铲地 0.17 单位，超了动画自检的 0.12——而蹄本身一点没穿。
# 抬起来之后蹄底仍是唯一的着地面，绑定层的基准不受影响。0.30 × 铁条厚在常马上是 0.11
# 单位（7 mm），肉眼读不出"悬空"，但把上面两件事都解决了。
# 0.38 这个数由**最狠的那一帧**定：矮马 death 的 t=0.66，整只侧翻，前右蹄外缘转到最低，
# 此时铁条外角比蹄自身的最低点还低。抬到 0.38 后余量约 0.04 单位。改大铁条厚度或外扩
# 就得重跑自检——阈值是动画层给的，不是拍的。
SOLE_LIFT = 0.38


def _ceiling(avoid: list[dict], x0: float, x1: float, z0: float, z1: float) -> float:
    """在给定 x/z 足迹上，头顶最低的那件**非蹄皮件**的下缘。上长的件（踵铁 / 夹）到此为止。

    为什么要算不能写死：挽马的球节半径按 `bone_gauge` 放大到 2.05 单位，球底垂到
    y=1.26，比常马低了一大截——一个对常马刚好的踵铁高度，到挽马身上就直接戳进球节里。
    上长多少得**问皮层**，不能问常数。
    """
    top = float("inf")
    for e in avoid:
        lo, hi = _aabb(e)
        if hi[0] > x0 and lo[0] < x1 and hi[2] > z0 and lo[2] < z1:
            top = min(top, float(lo[1]))
    return top


def part_shoe(t: Tack, fit: HoofFit, spec: ShoeSpec, avoid: list[dict]) -> None:
    """一只蹄的蹄铁：U 形铁条（踵端开口）+ 夹 + 踵铁 + 钉头 + 灵纹。

    铁条是一圈**箍在蹄壁下缘**的带子，不是垫在蹄底下面的板：垫下去整只马就抬高了一个
    铁条厚，而"最低点是蹄底"是绑定层的基准（皮层有断言）。真马钉了掌确实是踩在铁上，
    但那点厚度（10 mm ≈ 0.16 单位）不值得为它动整套骨架。底面离蹄底的一线见 SOLE_LIFT。

    `avoid` = 除本蹄两件之外的全部皮件。凡是**向上长**的件都按它算净空，不写死高度。
    """
    h, b, k = fit.half, fit.bone, fit.key
    th, ov, bi = spec.thick * h, spec.over * h, spec.bite * h
    y0 = th * SOLE_LIFT
    y1 = y0 + th

    x_o0, x_i0 = fit.x0 - ov, fit.x0 + bi  # −x 支臂：外缘 / 内缘
    x_i1, x_o1 = fit.x1 - bi, fit.x1 + ov  # +x 支臂
    z_o = fit.z_toe - ov  # 蹄尖外缘
    z_i = fit.z_toe + bi  # 趾带内缘
    z_b = fit.z_heel + ov * 0.35  # 踵端（略出蹄踵，真马的蹄铁后端就是探出来一点）

    # 趾带 + 两条支臂 = U。后端两臂之间留口，这是蹄铁最好认的轮廓。
    t.box(b, f"shoe_{k}_toe", (x_i0, y0, z_o), (x_i1, y1, z_i), mat=spec.mat_dark)
    arms = (("l", x_o0, x_i0), ("r", x_i1, x_o1))
    for tagx, xa, xb in arms:
        t.box(b, f"shoe_{k}_arm_{tagx}", (xa, y0, z_o), (xb, y1, z_b), mat=spec.mat)

    def up_box(name: str, xa: float, xb: float, za: float, zb: float, want: float, mat: str) -> float:
        """向上长的件：想长到 want，实际到"头顶那件皮的下缘"为止。返回实际高度。"""
        top = min(want, _ceiling(avoid, xa, xb, za, zb) - HEADROOM)
        t.box(b, name, (xa, y0, za), (xb, top, zb), mat=mat)
        return top

    # 踵铁：后端垫高的一小段。真马靠它在硬地上咬住，视觉上是后端明显加厚。
    # 短而矮：首版给到 1.6 倍条厚 × 0.30 蹄宽长，渲出来是踵上两块砖，不是踵铁。
    if spec.caulk:
        cl = max(th * 1.2, h * 0.22)
        for tagx, xa, xb in arms:
            up_box(f"shoe_{k}_caulk_{tagx}", xa, xb, z_b - cl, z_b, y0 + th * spec.caulk, spec.mat_dark)

    # 趾夹：前壁上翻的一片**舌**，卡住铁条不让它前移。侧视里最显眼的分档标志。
    # 关键在"窄"：真的趾夹一指宽。首版按 0.30 蹄半宽给（占了整条铁的四分之一），
    # 加上盒子没有收分，读出来是根柱子而不是一片舌。
    if spec.toe_clip:
        cw = h * 0.20
        up_box(f"shoe_{k}_clip_toe", fit.cx - cw, fit.cx + cw, z_o, fit.z_toe + bi * 0.35,
               y1 + fit.wall * spec.toe_clip, spec.mat_dark)

    # 侧夹：两侧壁各一片，只有灵铁档有。同样求窄。
    if spec.quarter_clip:
        qz0 = fit.z_toe + fit.span * 0.36
        qz1 = qz0 + fit.span * 0.16
        for tagx, xa, xb in (("l", x_o0, fit.x0 + bi * 0.35), ("r", fit.x1 - bi * 0.35, x_o1)):
            up_box(f"shoe_{k}_clip_{tagx}", xa, xb, qz0, qz1, y1 + fit.wall * spec.quarter_clip, spec.mat_dark)

    # 钉头：贴着铁条上缘排一行。**必须与铁条面接**（y 自 y1 起），悬空半个单位就是
    # 一堆飘在蹄壁边上的小方块，而连通性断言正是为这种事设的。
    # 钉头是"头"不是"桩"：高度压到条厚的四成、左右比铁条内缩一点，才读成一排铆点。
    nz = h * 0.10
    nh = th * 0.40
    for i in range(spec.nails):
        f = 0.42 if spec.nails == 1 else 0.24 + 0.50 * i / (spec.nails - 1)
        z = fit.z_toe + fit.span * f
        for tagx, xa, xb in (("l", x_o0 + ov * 0.30, fit.x0 + bi * 0.30),
                             ("r", fit.x1 - bi * 0.30, x_o1 - ov * 0.30)):
            top = min(y1 + nh, _ceiling(avoid, xa, xb, z - nz, z + nz) - HEADROOM)
            if top < y1 + nh * 0.4:
                raise SystemExit(f"{k} 第 {i + 1} 颗钉头顶到皮了（只剩 {top - y1:.3f} 高）——把钉排下移或减薄铁条")
            t.box(b, f"shoe_{k}_nail_{tagx}{i + 1}", (xa, y1, z - nz), (xb, top, z + nz), mat=spec.mat_nail)

    # 灵纹：沿铁条外侧走一圈的细发光条。**贴着铁条面刻，不是支出来一片鳍**——外凸只取
    # 一丝（够避开共面 z-fighting 就行）：首版按 ov*0.34 支出去，等于给最外缘再加一截
    # 力臂，矮马 death 那一帧灵纹先于铁条铲地 0.146，比铁条自己还深 0.036。
    if spec.glow:
        gy, gt, go = y0 + th * 0.30, th * 0.34, th * 0.06
        t.box(b, f"shoe_{k}_glow_toe", (x_i0, gy, z_o - go), (x_i1, gy + gt, z_o + ov * 0.5), mat="glow", glow=True)
        for tagx, xo, sgn in (("l", x_o0, -1.0), ("r", x_o1, 1.0)):
            t.box(
                b,
                f"shoe_{k}_glow_{tagx}",
                (xo + sgn * go, gy, z_o),
                (xo - sgn * ov * 0.5, gy + gt, z_b),
                mat="glow",
                glow=True,
            )


def build_shoes(t: Tack, fits: dict[str, HoofFit], spec: ShoeSpec, pelt_els: list[dict]) -> None:
    for key in ("f_l", "f_r", "h_l", "h_r"):
        avoid = [e for e in pelt_els if e["name"] not in (f"hoof_{key}", f"hoof_top_{key}")]
        part_shoe(t, fits[key], spec, avoid)


# name → (中文名, 分档表, 装配函数)
KINDS = {
    "shoe": ("蹄铁", SHOES, build_shoes),
}


# ================================================================ 自检
class _Shim:
    def __init__(self, els):
        self.elements = els


def _aabb(e: dict) -> tuple[np.ndarray, np.ndarray]:
    pts = np.array(_corners(e), float)
    return pts.min(axis=0), pts.max(axis=0)


def _overlap_vol(a: dict, b: dict) -> float:
    """两件的 AABB 交体积。马具与皮件都是轴对齐盒（蹄件已断言无旋转），AABB 即真形状。"""
    lo_a, hi_a = _aabb(a)
    lo_b, hi_b = _aabb(b)
    d = np.minimum(hi_a, hi_b) - np.maximum(lo_a, lo_b)
    return float(np.prod(d)) if (d > 0).all() else 0.0


def _mirror_suffix(sfx: str) -> str:
    """件名尾的 _l/_r（含 nail_l2 这种带序号的）翻个面；居中件（toe）映射到自己。"""
    m = re.fullmatch(r"(.*)_([lr])(\d*)", sfx)
    if not m:
        return sfx
    return f"{m.group(1)}_{'r' if m.group(2) == 'l' else 'l'}{m.group(3)}"


MIN_BITE = 0.02  # 咬合体积下限（单位³）
MIN_SHOW = 0.10  # 露出蹄外的下限（单位）
STRAY_TOL = 0.02  # 与"不该碰的皮件"的容许交体积


def check_tack(t: Tack, pelt_els: list[dict], fits: dict[str, HoofFit], spec: ShoeSpec) -> list[str]:
    """马具层自检。四条各自对应一种在渲染图上看不出来的翻车：

      · **穿地 / 悬空**：铁条底面必须恰在 y=0。差 0.1 个单位，静止侧视看不出来，
        但绑定层拿最低点当蹄底，四蹄触地点就整体偏了（皮层的距毛就是这么翻的车）。
      · **咬住蹄**：马具与蹄没有实交 = 悬在旁边。三视图里两件同色贴着，看不出来。
      · **露在蹄外**：整件埋进皮里 = 等于没做。这条和上一条是**反向**的一对，
        只查一条必然被另一条方向的错误绕过去。
      · **不碰不该碰的**：夹片长过头会戳进系或距毛。报出具体是哪一件，好回去调数。

    外加连通性（钉头/灵纹悬空）与左右镜像（单侧写错数）。
    """
    els = [e for e in t.skel.data["elements"] if e.get("_tack")]
    bad: list[str] = []
    if not els:
        return ["马具层一件都没有"]

    by_foot: dict[str, list[dict]] = {}
    for e in els:
        # 件名形如 shoe_<tag>_<side>_<...>
        parts = e["name"].split("_")
        by_foot.setdefault(f"{parts[1]}_{parts[2]}", []).append(e)

    lo_all = min(float(_aabb(e)[0][1]) for e in els)
    if lo_all < -0.02:
        who = sorted(e["name"] for e in els if float(_aabb(e)[0][1]) <= lo_all + 0.02)
        bad.append(f"马具穿到地下：最低 y={lo_all:.3f}（{', '.join(who[:4])}）")

    pelt_by_name = {e["name"]: e for e in pelt_els}
    for key, fit in fits.items():
        mine = by_foot.get(key, [])
        if not mine:
            bad.append(f"{key} 这只蹄没有马具件")
            continue
        # 蹄铁必须箍在**蹄壁下缘**：低于 0 是穿地（上面已查），高过蹄壁一半就不是蹄铁了，
        # 是箍在系上的一个环——两头都得夹住，只查一头会被另一头的错误绕过去。
        base = min(float(_aabb(e)[0][1]) for e in mine)
        if not 0.0 < base <= fit.wall * 0.5:
            bad.append(f"{key} 的马具底面 y={base:.3f} 不在蹄壁下缘（应落在 0 与 {fit.wall * 0.5:.2f} 之间）")

        hoof = pelt_by_name[f"hoof_{key}"]
        bite = sum(_overlap_vol(e, hoof) for e in mine)
        if bite < MIN_BITE:
            bad.append(f"{key} 的马具没咬住蹄：与 hoof_{key} 交体积仅 {bite:.4f}")

        lo_h, hi_h = _aabb(hoof)
        lo_t = np.min([_aabb(e)[0] for e in mine], axis=0)
        hi_t = np.max([_aabb(e)[1] for e in mine], axis=0)
        show = max(float(lo_h[0] - lo_t[0]), float(hi_t[0] - hi_h[0]), float(lo_h[2] - lo_t[2]))
        if show < MIN_SHOW:
            bad.append(f"{key} 的马具埋在蹄里：外露最多 {show:.3f} < {MIN_SHOW}")

        allowed = {f"hoof_{key}", f"hoof_top_{key}"}
        for e in mine:
            for pe in pelt_els:
                if pe["name"] in allowed:
                    continue
                v = _overlap_vol(e, pe)
                if v > STRAY_TOL:
                    bad.append(f"{e['name']} 戳进了 {pe['name']}（交体积 {v:.3f}）")

    comps = connected_components(_Shim(els))
    want = len(fits)
    if len(comps) != want:
        detail = " / ".join(f"{len(c)} 件({c[0]}…)" for c in comps[:4])
        bad.append(f"马具应是 {want} 块（每蹄一块），实为 {len(comps)} 块：{detail}")
    elif len({len(c) for c in comps}) != 1:
        bad.append(f"四只蹄的件数不一致：{sorted(len(c) for c in comps)}")

    # 每只蹄的马具必须**关于自己的蹄心左右对称**。查的是"同一只蹄内部"而不是"左蹄 vs
    # 右蹄"：件名尾部的 _l/_r 指的是该蹄自身的 ±x 侧，不是哪条腿，拿左蹄的 arm_l 去比
    # 右蹄的 arm_l 比的是两个同侧件，永远对不上。同蹄内部对称是更强的一条——单侧写错数
    # （只给一边加了外扩）在这里立刻撞红，而左右蹄互比会因为两边一起错而放过。
    for key, fit in fits.items():
        pre = f"shoe_{key}_"
        mine = {e["name"][len(pre):]: e for e in by_foot.get(key, [])}
        for sfx, e in mine.items():
            m = _mirror_suffix(sfx)
            o = mine.get(m)
            if o is None:
                bad.append(f"{pre}{sfx} 没有镜像伙伴 {pre}{m}")
                continue
            dx = abs((e["from"][0] - fit.cx) + (o["to"][0] - fit.cx)) + abs((e["to"][0] - fit.cx) + (o["from"][0] - fit.cx))
            dyz = max(abs(e[q][i] - o[q][i]) for q in ("from", "to") for i in (1, 2))
            if dx > 0.01 or dyz > 0.01:
                bad.append(f"{pre}{sfx} 与 {pre}{m} 不镜像（Δx={dx:.3f} Δyz={dyz:.3f}）")

    for e in els:
        bone = _bone_of(t.skel.data, e["uuid"])
        if not bone or not bone.startswith("hoof_"):
            bad.append(f"{e['name']} 挂在 {bone}，蹄铁只应挂在 hoof_* 骨上（否则会跨关节被扯裂）")
    return bad


def _bone_of(data: dict, uuid_: str) -> str | None:
    groups = {g["uuid"]: g for g in data.get("groups", [])}
    found: list[str] = []

    def walk(node, bone=None):
        if isinstance(node, str):
            if node == uuid_ and bone:
                found.append(bone)
            return
        meta = groups.get(node["uuid"], node)
        for c in node.get("children", []):
            walk(c, meta.get("name", bone))

    for root in data["outliner"]:
        walk(root)
    return found[0] if found else None


# ---------------------------------------------------------------- 动画期贴地
_FRAMES: dict[str, dict[str, list]] = {}


def hoof_frames(pkey: str, n: int = 32) -> dict[str, list]:
    """每档体型的**蹄骨世界变换**逐帧表，按体型缓存一次。

    蹄铁刚性挂在蹄骨上，所以"动画里会不会铲地"只取决于蹄骨的世界变换 + 马具自己的
    角点——与分档无关。先把变换算出来存着，三档蹄铁共用，省掉两轮逆解。
    """
    if pkey in _FRAMES:
        return _FRAMES[pkey]
    import gen_anim as G
    from rig import Rig

    P = PROFILES[pkey]
    rig = Rig(FINAL / f"HorsePelt_{GEOM_COAT}_{pkey}.bbmodel")
    out: dict[str, list] = {}
    for name in G.ANIMS:
        for i in range(n):
            pose = G.sample(rig, P, name, i / n)
            W = rig.world(pose)
            for tag in ("f", "h"):
                for side in ("l", "r"):
                    bone = f"hoof_{tag}_{side}"
                    out.setdefault(bone, []).append((name, i / n, W[bone]))
    rig.residuals.clear()
    _FRAMES[pkey] = out
    return out


SINK_TOL = 0.12  # 与皮层动画自检同一把尺（1 单位 = 1 体素 = 6.25 cm）

_ANIMS: dict[str, list[dict]] = {}


def anim_block(pkey: str) -> list[dict]:
    """把皮层那十条动画烘进马具文件。**按体型缓存**——轨道只随体型变，三档马具共用。

    为什么马具文件里也要有动画：马具挂在同一套骨上，引擎里靠同一份动画驱动；但人在
    Blockbench 里打开的是**这个文件**，没有动画就没法验"跑起来铁还在不在蹄上"。
    交付物要能自己证明自己。
    """
    if pkey not in _ANIMS:
        import gen_anim as G
        from rig import Rig

        rig = Rig(FINAL / f"HorsePelt_{GEOM_COAT}_{pkey}.bbmodel")
        _ANIMS[pkey] = G.animations_block(rig, PROFILES[pkey], list(G.ANIMS))
        rig.residuals.clear()
    return _ANIMS[pkey]


def check_anim_bones(data: dict) -> list[str]:
    """动画轨道引用的骨 uuid 必须在本文件的骨树里真实存在。

    马具文件是把皮件裁掉之后剩下的骨树 —— 裁剪一旦把某根骨连同它的 element 一起
    删掉，轨道就指向一个不存在的 uuid，Blockbench 静默忽略：模型能打开、能播，
    只是那根骨不动。渲静帧完全看不出来。
    """
    have = {g["uuid"] for g in data.get("groups", [])}

    def walk(node):
        if isinstance(node, str):
            return
        have.add(node["uuid"])
        for c in node.get("children", []):
            walk(c)

    for root in data["outliner"]:
        walk(root)
    miss = {u for a in data.get("animations", []) for u in a["animators"] if u not in have}
    return [f"动画引用了 {len(miss)} 根本文件没有的骨（轨道会被静默忽略）"] if miss else []


def check_anim_ground(t: Tack, pkey: str) -> tuple[float, str]:
    """动画里马具的最深穿地。蹄只转 12°，但蹄铁比蹄宽出一截，外缘的力臂更长——
    "蹄自己没穿地"推不出"蹄铁没穿地"，得单独量。"""
    frames = hoof_frames(pkey)
    pts: dict[str, np.ndarray] = {}
    for e in t.skel.data["elements"]:
        if not e.get("_tack"):
            continue
        bone = _bone_of(t.skel.data, e["uuid"])
        pts.setdefault(bone, []).extend(_corners(e))
    worst, who = 0.0, ""
    for bone, corners in pts.items():
        A = np.array(corners, float)
        for name, tt, W in frames[bone]:
            y = (A @ W[:3, :3].T + W[:3, 3])[:, 1].min()
            if -y > worst:
                worst, who = float(-y), f"{bone}@{name} t={tt:.2f}"
    return worst, who


# ================================================================ 装配 / CLI
def build(pkey: str, kind: str, tier: str, with_horse: bool) -> tuple[Tack, list[dict], dict[str, HoofFit]]:
    pelt = FINAL / f"HorsePelt_{GEOM_COAT}_{pkey}.bbmodel"
    if not pelt.is_file():
        raise SystemExit(f"找不到皮层: {pelt}（先跑 gen_pelt.py）")
    skel = Skeleton(pelt)
    pelt_els = [dict(e) for e in skel.data["elements"] if e.get("_pelt")]
    fits = read_hooves(skel.data)
    extend_texture(skel.data)
    t = Tack(skel, PROFILES[pkey])
    _label, table, fn = KINDS[kind]
    fn(t, fits, table[tier], pelt_els)
    if not with_horse:
        keep = {e["uuid"] for e in skel.data["elements"] if e.get("_tack")}
        skel.data["elements"] = [e for e in skel.data["elements"] if e["uuid"] in keep]

        def prune(node):
            if isinstance(node, str):
                return node in keep
            node["children"] = [c for c in node.get("children", []) if prune(c)]
            return True

        for root in skel.data["outliner"]:
            prune(root)
    return t, pelt_els, fits


MIN_CONTRAST = 45.0  # 主色与蹄色的最小 RGB 欧氏距离


def check_contrast() -> list[str]:
    """每档马具的主色必须和**每种毛色的蹄**拉得开——否则远处看不出穿没穿。

    不是审美挑剔，是这个仓库对装备的一贯要求（"玩家能从远处分辨对面在用 X 不是 Y"）。
    首版粗铁 (96,88,80) 与碎雪的蹄 (92,86,80) 相距 4.5，整只马的侧视里蹄铁完全消失，
    和赤脚一个样——而蹄部特写图上它清清楚楚。**特写成立不等于整只成立**，所以这条
    必须是断言，不能只靠渲一张图看看。

    RGB 欧氏距离只是个粗糙代理，够用：这里要挡的是"几乎同色"，不是排颜色的名次。
    """
    from gen_pelt import COATS

    bad = []
    for kind, (_label, table, _fn) in KINDS.items():
        for tk, spec in table.items():
            r0, g0, b0 = TACK_MATS[spec.mat]
            for ck, coat in COATS.items():
                r1, g1, b1 = coat.mats["hoof"]
                d = ((r0 - r1) ** 2 + (g0 - g1) ** 2 + (b0 - b1) ** 2) ** 0.5
                if d < MIN_CONTRAST:
                    bad.append(f"{kind}/{tk} 的 {spec.mat} 与「{coat.label}」的蹄色只差 {d:.1f}，远处看不出穿没穿")
    return bad


def check_geom_same_across_coats(pkey: str) -> list[str]:
    """三种毛色的蹄件必须几何全等——马具只按一种毛色量尺寸，这条塌了就是量错了马。"""
    from gen_pelt import COATS

    ref: dict[str, list] | None = None
    bad = []
    for ck in sorted(COATS):
        f = FINAL / f"HorsePelt_{ck}_{pkey}.bbmodel"
        if not f.is_file():
            continue
        els = {e["name"]: (e["from"], e["to"]) for e in json.loads(f.read_text())["elements"] if e.get("_pelt")}
        cur = {k: v for k, v in els.items() if k.startswith("hoof_")}
        if ref is None:
            ref = cur
        elif cur != ref:
            bad.append(f"{ck} 的蹄件几何与 {GEOM_COAT} 不同——马具尺寸来源不成立")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser(description="马具层（读皮层实件尺寸，不回写）")
    ap.add_argument("--profile", choices=[*sorted(PROFILES), "all"], default="all")
    ap.add_argument("--kind", choices=[*sorted(KINDS), "all"], default="all")
    ap.add_argument("--tier", help="只出一档（配合 --kind）")
    ap.add_argument("--with-horse", action="store_true", help="保留皮层，看贴合关系（落 stages/）")
    ap.add_argument("--skip-anim", action="store_true", help="跳过动画期贴地自检 + 动画烘焙（快，仅用于调几何）")
    ap.add_argument("--list", action="store_true")
    args = ap.parse_args()

    if args.list:
        for k, (label, table, _fn) in KINDS.items():
            print(f"{k}（{label}）：")
            for tk, spec in table.items():
                print(f"  {tk:8s} {spec.label:12s} {spec.blurb}")
        return 0

    pkeys = sorted(PROFILES) if args.profile == "all" else [args.profile]
    kinds = sorted(KINDS) if args.kind == "all" else [args.kind]
    rc = 0

    for msg in check_contrast():
        print(f"  ✗ {msg}")
        rc = 1
    for pk in pkeys:
        for msg in check_geom_same_across_coats(pk):
            print(f"  ✗ [{pk}] {msg}")
            rc = 1

    for kind in kinds:
        label, table, _fn = KINDS[kind]
        tiers = [args.tier] if args.tier else list(table)
        vols: dict[str, list[float]] = {}
        for pk in pkeys:
            for tier in tiers:
                if tier not in table:
                    print(f"未知分档 {tier}（{kind} 有 {', '.join(table)}）")
                    return 2
                spec = table[tier]
                t, pelt_els, fits = build(pk, kind, tier, args.with_horse)
                name = f"Horse{kind.capitalize()}_{tier}_{pk}" + ("_on_horse" if args.with_horse else "")
                t.skel.data["name"] = name
                t.skel.data["model_identifier"] = name
                out = (STAGES if args.with_horse else TACK_DIR) / f"{name}.bbmodel"
                out.parent.mkdir(parents=True, exist_ok=True)
                bad = check_tack(t, pelt_els, fits, spec)
                if not args.skip_anim:
                    t.skel.data["animations"] = anim_block(pk)
                    bad += check_anim_bones(t.skel.data)
                    # 带动画的模型走紧凑 JSON：indent=1 光缩进就占掉近一半体积
                    out.write_text(json.dumps(t.skel.data, ensure_ascii=False, separators=(",", ":")))
                else:
                    out.write_text(json.dumps(t.skel.data, ensure_ascii=False, indent=1))

                sink, who = (0.0, "") if args.skip_anim else check_anim_ground(t, pk)
                if sink > SINK_TOL:
                    bad.append(f"动画里马具铲地 {sink:.2f} > {SINK_TOL}（{who}）")
                vols.setdefault(tier, []).append(
                    sum(
                        float(np.prod(np.array(e["to"]) - np.array(e["from"])))
                        for e in t.skel.data["elements"]
                        if e.get("_tack")
                    )
                )
                mark = "✓" if not bad else "✗"
                extra = "" if args.skip_anim else f" 动画贴地 {sink:.2f}"
                print(f"{mark} {out.relative_to(FINAL.parents[1])}  【{spec.label} · {PROFILES[pk].label}】"
                      f"件 {t.count}{extra}")
                for m in bad:
                    print(f"    ✗ {m}")
                    rc = 1

        # 分档必须**看得出来**：体积严格递增是"远处能分辨"最省事的代理指标。
        if len(tiers) == len(table) and len(pkeys) > 0:
            order = list(table)
            for i in range(len(order) - 1):
                a, b = vols.get(order[i], []), vols.get(order[i + 1], [])
                for j, (va, vb) in enumerate(zip(a, b)):
                    if vb <= va * 1.15:
                        print(f"    ✗ {order[i + 1]} 的用料没比 {order[i]} 明显多（{va:.2f} → {vb:.2f}），分档看不出来")
                        rc = 1
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
