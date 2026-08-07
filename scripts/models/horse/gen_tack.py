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
from gen_skeleton import PROFILES, Profile, _obb, connected_components, uid
from PIL import Image

TACK_DIR = FINAL / "tack"
TACK_ROW = 4  # 贴图第 5 行起（0-1 行骨/肌，2-3 行皮），追加不动已有 UV
GEOM_COAT = "rust"  # 几何三色同源，取一份当尺寸来源（另两份由 check_geom_same_across_coats 对拍）

Vec = tuple[float, float, float]


def _lerpf(a: float, b: float, s: float) -> float:
    return a + (b - a) * s

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
    # --- 马鞍起追加（只准往后加：UV 索引由顺序派生）---
    # 毡往**浅**里推、革往**深**里推：三种毛色（锈骝 128,72,43 / 枯原 148,126,82 /
    # 碎雪 146,140,131）都落在中等明度的暖色带上，马具挤进这条带里就整片糊掉。
    # 一浅一深各自绕开——首版毡 (122,114,100) 离碎雪暗部只有 24.8、革 (118,82,52)
    # 离锈骝身色只有 16.8，正是"棕鞍配栗马"这个经典读不出来的组合。
    "felt": (186, 180, 168),  # 旧毡：洗到发灰的白
    "felt_dark": (140, 134, 122),
    "leather": (58, 40, 32),  # 粗革：鞣得不匀、油到发乌的深棕
    "leather_dark": (38, 26, 22),
    "rope": (150, 134, 96),  # 麻绳
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


# ================================================================ 皮层表面采样
# 马具靠它贴上去的皮件。**必须逐个正则匹配，不能只看前缀**：`dorsal_1` 是三色共有的
# 背中线，`dorsal_stripe_1` 是枯原专属的鳝背线——前缀写 "dorsal_" 会把后者一起读进来，
# 于是马具尺寸随毛色变（`check_geom_same_across_coats` 立刻撞红，就是这么发现的）。
SHAPE_RE = re.compile(r"(torso_\w+|dorsal_\d+|croup_cap_\d+|chest_front)$")


def is_shape(name: str) -> bool:
    return bool(SHAPE_RE.fullmatch(name))


class Torso:
    """皮层躯干的断面采样器。马鞍 / 肚带 / 马甲都靠它贴上去。

    躯干皮是**轴对齐盒**堆出来的（`gen_pelt.part_torso`），所以"哪些盒盖住这个点"
    直接就是真形状，不必再解一遍肌肉包络。三个查询各对应一种贴法：
      · `at(z)`  —— 该 z 的半宽 / 背顶 / 腹底，定马具的纵向范围；
      · `top_at(z, x)` —— 背在横向某处的高度。**背不是平的**，鞍垫按一个高度铺过去，
        中间压进脊里、两边悬在空中；
      · `half_at(z, y)` —— 桶身在某高度的半宽。肚带垂直挂下去只在最宽处贴着，
        上下两头都是空的。
    """

    def __init__(self, pelt_els: list[dict]) -> None:
        self.boxes = [(e["from"], e["to"]) for e in pelt_els if is_shape(e["name"])]
        if not self.boxes:
            raise SystemExit("皮层里找不到躯干件——马具无处可贴")

    def _cover(self, q: dict[int, float]):
        for f, t in self.boxes:
            if all(f[i] - 1e-6 <= v <= t[i] + 1e-6 for i, v in q.items()):
                yield f, t

    def at(self, z: float) -> tuple[float, float, float]:
        got = list(self._cover({2: z}))
        if not got:
            raise SystemExit(f"躯干在 z={z:.2f} 处没有皮件——马具定位越界了")
        return (max(max(t[0], -f[0]) for f, t in got),
                max(t[1] for f, t in got),
                min(f[1] for f, t in got))

    def top_at(self, z: float, x: float) -> float:
        got = list(self._cover({0: x, 2: z}))
        if not got:
            raise SystemExit(f"躯干在 (x={x:.2f}, z={z:.2f}) 处没有皮件")
        return max(t[1] for f, t in got)

    def half_at(self, z: float, y: float) -> float:
        got = list(self._cover({1: y, 2: z}))
        if not got:
            raise SystemExit(f"躯干在 (y={y:.2f}, z={z:.2f}) 处没有皮件")
        return max(max(t[0], -f[0]) for f, t in got)


@dataclass
class Fit:
    """马具装配所需的全部「来自皮层的量」。装配函数只准从这里取数，不准另写常数。"""

    P: Profile
    pelt_els: list[dict]
    hooves: dict[str, "HoofFit"]
    torso: Torso


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


def build_shoes(t: Tack, fit: Fit, spec: ShoeSpec) -> None:
    for key in ("f_l", "f_r", "h_l", "h_r"):
        avoid = [e for e in fit.pelt_els if e["name"] not in (f"hoof_{key}", f"hoof_top_{key}")]
        part_shoe(t, fit.hooves[key], spec, avoid)


# ================================================================ 马鞍
# 整副鞍挂在**一根骨**上（`SADDLE_BONE`）。理由不是省事：真鞍有硬鞍架，本来就是刚体，
# 马背在它下面屈伸。若为了"贴合"把鞍拆到两根骨上，脊一弯鞍就从中间裂开——那是把
# 皮层的接缝问题原样搬进马具层。刚体挂一根骨，代价只是前后端在大幅屈伸时略微陷进皮里
# 一点，这一项由 `check_anim_fit` 逐帧盯着。
SADDLE_BONE = "thorax_back"

# 骑手坐姿需要的最小座面（单位；1 单位 = 1 体素 = 6.25 cm）。玩家模型的胯宽 8 单位、
# 臀深约 4 单位——座面小于这个数，后面做骑乘动画时人就是浮在鞍上而不是坐在鞍上。
# 这条现在就断言，不等做动画时才发现鞍根本坐不下人。
SEAT_MIN_Z = 4.0
SEAT_MIN_X = 3.0

# 镫底在座面**以下**多少（单位）。这是**骑手的尺寸，不是马的**——玩家模型腿长约 12
# 单位，屈膝踩镫时脚落在髋下 9–12 单位处。首版按鬐甲高的比例给（0.50–0.52W），于是
# 常马的镫离座面 15.3 单位、挽马 18.3 —— 玩家的腿根本够不着，做骑乘动画时脚只能悬空
# 或者把腿拉长。三档马一个骑手，这个量当然得是绝对值。
STIRRUP_DROP = 10.5
STIRRUP_REACH = (8.0, 12.0)  # 可接受的座→镫落差区间


@dataclass(frozen=True)
class SaddleSpec:
    key: str
    label: str
    blurb: str
    mat: str  # 主面
    mat_dark: str  # 暗部 / 鞍桥
    mat_trim: str  # 金属件（镫、扣、包角）
    length: float  # 鞍全长 / 体长
    pad_th: float  # 鞍垫厚（× 鬐甲高）
    pad_half: float  # 鞍垫横向覆盖（× 该处躯干半宽）
    seat_h: float  # 座面高出鞍垫（× 鬐甲高）
    pommel: float  # 前鞍桥高出座面（× 鬐甲高）；0 = 无鞍桥（光垫子）
    cantle: float  # 后鞍桥高出座面
    flap: float  # 鞍翼下垂（× 鬐甲高）；0 = 无
    girth_w: float  # 肚带宽（× 体长）
    stirrup: bool  # 有没有镫（高度不由分档定，见 STIRRUP_DROP）
    glow: bool = False


SADDLES: dict[str, SaddleSpec] = {
    # 一档：一块折了几折的旧毡，麻绳一捆。没有鞍架、没有镫——上马靠跳，骑久了磨大腿。
    "felt": SaddleSpec(
        key="felt", label="破毡鞍", blurb="几折旧毡加一道麻绳。没有鞍桥没有镫，骑久了磨腿。",
        mat="felt", mat_dark="felt_dark", mat_trim="rope",
        length=0.25, pad_th=0.080, pad_half=0.76, seat_h=0.0,
        pommel=0.0, cantle=0.0, flap=0.0, girth_w=0.030, stirrup=False,
    ),
    # 二档：木鞍架蒙粗皮。有前后鞍桥、鞍翼、粗铁镫——这是"能长途骑"的分界线。
    "leather": SaddleSpec(
        key="leather", label="粗革鞍", blurb="木鞍架蒙粗皮，前后鞍桥齐全，粗铁镫。能骑长途。",
        mat="leather", mat_dark="leather_dark", mat_trim="iron_crude",
        length=0.29, pad_th=0.050, pad_half=0.82, seat_h=0.076,
        pommel=0.078, cantle=0.090, flap=0.190, girth_w=0.040, stirrup=True,
    ),
    # 三档：皮面灵铁骨。鞍桥包灵铁、沿鞍桥一道灵纹，镫也是灵铁。
    "lingtie": SaddleSpec(
        key="lingtie", label="灵铁鞍", blurb="皮面灵铁骨，鞍桥包铁刻纹，落鞍时泛淡蓝。",
        # 主面仍是革——二三档本来就都是皮鞍，差别在**配件**（鞍桥包灵铁 + 灵纹 + 灵铁镫）。
        # 硬给三档换一种棕色反而是为了分档而分档，玩家读到的也不是"材质更好"。
        mat="leather", mat_dark="lingtie_dark", mat_trim="lingtie",
        length=0.31, pad_th=0.056, pad_half=0.86, seat_h=0.086,
        pommel=0.094, cantle=0.108, flap=0.210, girth_w=0.046, stirrup=True,
        glow=True,
    ),
}

# 座面占鞍垫横向的比例。首版 0.62 —— 于是座比垫窄了近一半，整副鞍从侧面读成"插在杆上
# 的一块板"。真鞍的座板与鞍垫几乎同宽，垫只在边缘多出一圈。
SEAT_WF = 0.88

PAD_NZ, PAD_NX = 3, 3  # 鞍垫的纵向 / 横向分段：背不是平的，一块板铺过去中间压脊两边悬空


def part_saddle(t: Tack, fit: Fit, spec: SaddleSpec) -> None:
    Pr, T, b = fit.P, fit.torso, SADDLE_BONE
    L, W = Pr.L, Pr.wither
    # 鞍位：鬐甲峰后一点起，长度按分档。真鞍就压在肩胛之后、最后一根肋之前。
    z0 = Pr.z_wither_peak + 0.055 * L
    z1 = z0 + spec.length * L
    pad_th = Pr.u(spec.pad_th)

    # **骑手不随马缩小**：矮马按比例出的鞍座只有 2.9×3.6 单位，坐不下人（`SEAT_MIN_*`）。
    # 真实里给小马配成年人的鞍，看上去也确实偏大——所以这里是把座面**撑到绝对下限**，
    # 而不是放宽断言。撑座面就得同时撑鞍垫，否则座板悬在垫外。
    seat_frac = 0.60
    z1 = max(z1, z0 + (SEAT_MIN_Z * 1.08) / seat_frac)
    hw_ref = T.at((z0 + z1) / 2)[0]
    half_need = max(hw_ref * spec.pad_half, SEAT_MIN_X * 1.08 / 2 / SEAT_WF)
    xs = [half_need * k / PAD_NX for k in range(PAD_NX + 1)]
    zs = [_lerpf(z0, z1, k / PAD_NZ) for k in range(PAD_NZ + 1)]
    tops = {(i, j): T.top_at((zs[i] + zs[i + 1]) / 2, (xs[j] + xs[j + 1]) / 2)
            for i in range(PAD_NZ) for j in range(PAD_NX)}
    # **所有格共用一个底面**（取全场最低的那格再往下留一点）。各格按自己的背高单独定底，
    # 背一弯，中带比外带高出一个厚度以上，相邻两格在 y 上就不再重叠——鞍垫散成一堆
    # 互不相连的小板（连通性断言报的就是这个）。共用底面多出来的部分埋在马体内，看不见。
    floor = min(tops.values()) - pad_th * 0.45
    pad_top: dict[tuple[int, int], float] = {}
    for i in range(PAD_NZ):
        for j in range(PAD_NX):
            top = tops[(i, j)]
            pad_top[(i, j)] = top + pad_th
            for sgn, side in ((-1.0, "l"), (1.0, "r")):
                if j == 0 and side == "r":
                    continue  # 中带跨脊，只出一件
                xa = sgn * xs[j] if j else -xs[1]
                xb = sgn * xs[j + 1] if j else xs[1]
                nm = f"saddle_pad_{i + 1}{j + 1}" if j == 0 else f"saddle_pad_{i + 1}{j + 1}_{side}"
                t.box(b, nm, (xa, floor, zs[i]), (xb, top + pad_th, zs[i + 1]), mat=spec.mat_dark)

    seat_base = max(pad_top.values())
    seat_top_y = seat_base  # 光垫子档骑手就坐在垫顶；有鞍架的在下面覆盖
    x_seat = xs[PAD_NX] * SEAT_WF

    if spec.seat_h == 0.0:
        # 光垫子档：没有鞍架，只在垫上压一道卷边，好认出"这是马具不是马"
        t.box(b, "saddle_roll_front", (-x_seat, seat_base - pad_th * 0.3, z0), (x_seat, seat_base + pad_th * 0.7, z0 + 0.02 * L), mat=spec.mat)
        t.box(b, "saddle_roll_back", (-x_seat, seat_base - pad_th * 0.3, z1 - 0.02 * L), (x_seat, seat_base + pad_th * 0.7, z1), mat=spec.mat)
    else:
        # --- 座 + 前后鞍桥 ---
        # 座**要凹**：前高、中低、后高才是马鞍的剪影。首版是一块平板加两根立柱，
        # 侧视读出来是"板上插了两根天线"。三段各自定高，中段压下去一截。
        seat_h = Pr.u(spec.seat_h)
        m = (1.0 - seat_frac) / 2
        sz0, sz1 = _lerpf(z0, z1, m), _lerpf(z0, z1, 1.0 - m)
        dip = seat_h * 0.34
        seg = ((0.00, 0.30, 0.10), (0.30, 0.68, 1.00), (0.68, 1.00, 0.22))  # (起, 止, 下凹比例)
        for k, (a, c, dp) in enumerate(seg):
            t.box(b, f"saddle_seat_{k + 1}", (-x_seat, seat_base - pad_th * 0.2, _lerpf(sz0, sz1, a)),
                  (x_seat, seat_base + seat_h - dip * dp, _lerpf(sz0, sz1, c)), mat=spec.mat)
        seat_top = seat_top_y = seat_base + seat_h
        # 鞍桥分两级收进去：一块盒子直上直下读成柱子，两级才读得出"拱"。
        for nm, hgt, za, zb, wf in (
            ("pommel", spec.pommel, z0 + 0.008 * L, sz0 + 0.010 * L, 0.80),
            ("cantle", spec.cantle, sz1 - 0.010 * L, z1 - 0.008 * L, 0.98),
        ):
            xw, h1 = x_seat * wf, Pr.u(hgt)
            t.box(b, f"saddle_{nm}", (-xw, seat_base, za), (xw, seat_top + h1 * 0.55, zb), mat=spec.mat_dark)
            zc = (za + zb) / 2
            t.box(b, f"saddle_{nm}_cap", (-xw * 0.80, seat_top + h1 * 0.45, _lerpf(za, zc, 0.22)),
                  (xw * 0.80, seat_top + h1, _lerpf(zc, zb, 0.78)), mat=spec.mat_dark)
            if spec.glow:  # 灵纹刻在鞍桥顶棱上，贴面不支鳍（同蹄铁）
                gy = seat_top + h1
                t.box(b, f"saddle_{nm}_glow", (-xw * 0.74, gy - Pr.u(0.012), _lerpf(za, zc, 0.26)),
                      (xw * 0.74, gy + Pr.u(0.004), _lerpf(zc, zb, 0.74)), mat="glow", glow=True)

    # --- 鞍翼：两侧垂下的皮片。**必须挂在桶身外面** ---
    # 首版把它放在鞍垫的外缘（x = xs[3] ≈ 2.6），可桶身在那个高度已经宽到 3.2 ——
    # 整片鞍翼埋在马体内，一点看不见。鞍是搭在背上的，背窄；鞍翼垂到肋上，肋宽。
    # 所以横向位置得**按鞍翼自己那一段高度上的桶身半宽**来定，不能继承鞍垫的。
    fz0, fz1 = _lerpf(z0, z1, 0.14), _lerpf(z0, z1, 0.88)
    zc_f = (fz0 + fz1) / 2
    y_side_top = min(pad_top[(i, PAD_NX - 1)] for i in range(PAD_NZ))
    y_side_bot = y_side_top - Pr.u(max(spec.flap, 0.16))
    hw_side = max(T.half_at(zc_f, _lerpf(y_side_bot, y_side_top, k / 6)) for k in range(7))
    if spec.flap:
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            xo = sgn * hw_side
            t.box(b, f"saddle_flap_{side}", (xo - sgn * Pr.u(0.014), y_side_top - Pr.u(spec.flap), fz0),
                  (xo + sgn * Pr.u(0.020), y_side_top, fz1), mat=spec.mat)

    # --- 肚带：绕桶身一圈。侧带分 3 段各按该高度的半宽收，直上直下只在最宽处贴着 ---
    zg0 = z0 + 0.015 * L
    zg1 = zg0 + spec.girth_w * L
    zgc = (zg0 + zg1) / 2
    _hw_g, ytop_g, ybot_g = T.at(zgc)
    gt = Pr.u(0.018)
    ys = [_lerpf(ybot_g, min(ytop_g, seat_base), k / 3) for k in range(4)]
    # 每一格的 x 覆盖**该格上下两端各自的半宽**，不是格中点那一个值。桶身自下而上变宽，
    # 按中点算的话相邻两格在 x 上错开一整个台阶，肚带从中间断成几截（连通性断言撞红）。
    # 覆盖两端 → 相邻格必然在交界的半宽上共面，接得上，同时台阶也顺着桶身走。
    eps = gt * 0.25
    hws = [T.half_at(zgc, min(max(y, ybot_g + eps), ys[3] - eps)) for y in ys]
    for k in range(3):
        lo, hi = min(hws[k], hws[k + 1]) - gt * 0.5, max(hws[k], hws[k + 1]) + gt * 0.5
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            t.box(b, f"saddle_girth_{side}{k + 1}", (sgn * lo, ys[k], zg0), (sgn * hi, ys[k + 1], zg1), mat=spec.mat_dark)
    hw_b = hws[0] + gt * 0.5
    t.box(b, "saddle_girth_belly", (-hw_b, ybot_g - gt * 0.5, zg0), (hw_b, ybot_g + gt * 0.6, zg1), mat=spec.mat_dark)
    # 束带（billet）：肚带上端拐进鞍垫底下与鞍连成一体。真鞍就是这么系的，而在模型里
    # 它同时是**结构必需**——没有它，两条侧带的最高一格在 x 上比鞍垫还宽，整副鞍在
    # 连通性断言里散成"鞍 + 左带 + 右带"三块。
    for sgn, side in ((-1.0, "l"), (1.0, "r")):
        xi = sgn * xs[PAD_NX] * 0.55
        # 外端必须够到**最上一格侧带的外面**：桶身自上而下变宽，按束带自己那个高度算
        # 出来的半宽比侧带窄，够不着——两者中间留一道缝，鞍与带还是两块。
        xo = sgn * (max(T.half_at(zgc, ys[3] - gt), T.half_at(zgc, (ys[2] + ys[3]) / 2)) + gt * 0.7)
        t.box(b, f"saddle_billet_{side}", (xi, ys[3] - gt * 1.4, zg0), (xo, ys[3], zg1), mat=spec.mat_dark)
    # 扣：肚带侧面一枚金属件，一眼看出这是"束紧的带子"不是"画上去的一道深色"
    t.box(b, "saddle_buckle_l", (-T.half_at(zgc, ys[2]) - gt, ys[2], zg0 - gt * 0.4),
          (-T.half_at(zgc, ys[2]) + gt * 0.4, ys[2] + gt * 2.2, zg1 + gt * 0.4), mat=spec.mat_trim)
    t.box(b, "saddle_buckle_r", (T.half_at(zgc, ys[2]) - gt * 0.4, ys[2], zg0 - gt * 0.4),
          (T.half_at(zgc, ys[2]) + gt, ys[2] + gt * 2.2, zg1 + gt * 0.4), mat=spec.mat_trim)

    # --- 马镫：革带 + 铁环。踏板高度按 spec 定，骑乘动画的脚就落在这上面 ---
    if spec.stirrup:
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            # 镫挂在鞍翼外侧——同样按桶身实际半宽定，不继承鞍垫的
            xo = sgn * (hw_side + Pr.u(0.030))
            # 镫挂在座的中后段——骑手的腿在那儿，而不是压在马肩上。首版取 0.42 偏前，
            # 肩臂大幅前摆时直接扫过镫环（实测穿插 1.55）。
            ztop = _lerpf(z0, z1, 0.56)
            y_bot = seat_top_y - STIRRUP_DROP
            y_top = min(pad_top.values())
            lz = Pr.u(0.024)
            t.box(b, f"saddle_stirrup_leather_{side}", (xo - sgn * Pr.u(0.010), y_bot + Pr.u(0.040), ztop - lz),
                  (xo + sgn * Pr.u(0.010), y_top, ztop + lz), mat=spec.mat)
            # 镫革挂点（stirrup bar）：真鞍的鞍架上就有这么一根横档，革带穿在上面。
            # 在模型里它同时是**结构必需**——革带整条挂在鞍垫外侧，不加这根横档，
            # 镫与革带合起来是一块飘在马腿旁边的孤岛。
            t.box(b, f"saddle_stirrup_bar_{side}", (sgn * xs[PAD_NX] * 0.70, y_top - Pr.u(0.022), ztop - lz),
                  (xo + sgn * Pr.u(0.010), y_top, ztop + lz), mat=spec.mat_trim)
            rw, rt, rh = Pr.u(0.040), Pr.u(0.013), Pr.u(0.052)
            # 环：左右两根竖梁 + 踏板 + **顶梁**，中间留孔（实心方块读不成"环"）。
            # 顶梁不是装饰：革带只有 ±0.010W 宽，正好从两根竖梁中间穿过去谁也碰不着，
            # 镫整个成了飘在腿边的孤岛（连通性断言报的就是这个）。真镫本来也是闭合的环。
            for i, dx in enumerate((-1.0, 1.0)):
                t.box(b, f"saddle_stirrup_ring_{side}_{i + 1}",
                      (xo + dx * rw, y_bot, ztop - lz * 1.5), (xo + dx * (rw - rt), y_bot + rh, ztop + lz * 1.5),
                      mat=spec.mat_trim)
            t.box(b, f"saddle_stirrup_tread_{side}", (xo - rw, y_bot, ztop - lz * 1.5),
                  (xo + rw, y_bot + rt, ztop + lz * 1.5), mat=spec.mat_trim)
            t.box(b, f"saddle_stirrup_bow_{side}", (xo - rw, y_bot + rh - rt, ztop - lz * 1.5),
                  (xo + rw, y_bot + rh, ztop + lz * 1.5), mat=spec.mat_trim)
            if spec.glow:
                t.box(b, f"saddle_stirrup_glow_{side}", (xo - rw * 0.8, y_bot + rt, ztop - lz * 0.5),
                      (xo + rw * 0.8, y_bot + rt + Pr.u(0.008), ztop + lz * 0.5), mat="glow", glow=True)


def build_saddle(t: Tack, fit: Fit, spec: SaddleSpec) -> None:
    part_saddle(t, fit, spec)


def check_saddle(t: Tack, fit: Fit, spec: SaddleSpec) -> list[str]:
    """马鞍自检。除通用三条外，四条各挡一种翻车：

      · **坐在背上**：鞍垫与躯干皮有实交，且鞍垫顶面高过背线。悬在背上方半个单位，
        侧视图看不出来（鞍本来就该在背上），跑起来就是一副飘着的鞍。
      · **人坐得下**：座面必须有一块够大的近水平区域。这条现在断言，是因为骑乘动画
        要拿它当落点——等做动画时才发现鞍坐不下人，就得回来重做几何。
      · **肚带真的绕过去了**：肚带最低点必须低于腹底，且与鞍垫连成一体。侧视里
        "带子贴在肚子侧面"和"带子绕过肚子"长得一模一样。
      · **镫够得着**：镫底离地高度要落在骑手能踩到的范围。太高人骑上去脚悬空，
        太低扫地。
    """
    Pr, T = fit.P, fit.torso
    els = tack_els(t)
    bad = check_common(t, fit, lambda b: b == SADDLE_BONE)
    if not els:
        return bad
    by_name = {e["name"]: e for e in els}
    pads = [e for e in els if e["name"].startswith("saddle_pad_")]
    if not pads:
        return bad + ["没有鞍垫件"]

    torso_els = [e for e in fit.pelt_els if is_shape(e["name"])]
    bite = sum(_overlap_vol(p, pe) for p in pads for pe in torso_els)
    if bite < 1.0:
        bad.append(f"鞍垫没坐在背上：与躯干皮交体积仅 {bite:.3f}")
    for p in pads:
        zc = (p["from"][2] + p["to"][2]) / 2
        xc = (abs(p["from"][0]) + abs(p["to"][0])) / 2
        if p["to"][1] <= T.top_at(zc, xc) + 1e-6:
            bad.append(f"{p['name']} 埋在背线以下（顶 {p['to'][1]:.2f} ≤ 背 {T.top_at(zc, xc):.2f}）")

    # 座面：有鞍架就是座板本身；光垫子档骑手直接坐在垫上，座面是**整块垫的并集**
    # ——按单格算会得出 1.7×2.0，那是格子的尺寸不是能坐的面积。
    seats = [e for e in els if e["name"].startswith("saddle_seat")] or pads
    sx = max(e["to"][0] for e in seats) - min(e["from"][0] for e in seats)
    sz = max(e["to"][2] for e in seats) - min(e["from"][2] for e in seats)
    seat_top = max(e["to"][1] for e in seats)
    if sz < SEAT_MIN_Z or sx < SEAT_MIN_X:
        bad.append(f"座面 {sx:.1f}×{sz:.1f} 单位，坐不下骑手（至少 {SEAT_MIN_X}×{SEAT_MIN_Z}）")

    girth = [e for e in els if "girth" in e["name"]]
    if not girth:
        bad.append("没有肚带——鞍会滑下去")
    else:
        zc = (min(e["from"][2] for e in girth) + max(e["to"][2] for e in girth)) / 2
        belly = T.at(zc)[2]
        low = min(e["from"][1] for e in girth)
        if low > belly + 0.02:
            bad.append(f"肚带没绕到腹下：最低 {low:.2f} > 腹底 {belly:.2f}")

    stir = [e for e in els if "stirrup_tread" in e["name"]]
    if spec.stirrup:
        if not stir:
            bad.append("分档声明有镫，却一件都没出")
        else:
            # 判据以**骑手**为基准（座面到镫的落差），不是以地面为基准。同一个骑手要能
            # 骑三档马，"离地多高"在三档上意义完全不同，"腿够不够得着"才是一样的。
            drop = seat_top - min(e["from"][1] for e in stir)
            lo, hi = STIRRUP_REACH
            if not lo <= drop <= hi:
                bad.append(f"座面到镫底 {drop:.1f} 单位，不在骑手腿够得着的 {lo}–{hi}（玩家腿长约 12）")
    elif stir:
        bad.append("分档声明无镫，却出了镫件")

    comps = connected_components(_Shim(els))
    if len(comps) != 1:
        detail = " / ".join(f"{len(c)} 件({c[0]}…)" for c in comps[:3])
        bad.append(f"整副鞍应是一整体，实为 {len(comps)} 块：{detail}")
    return bad


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


def comp_type(name: str) -> str:
    """件名 → **部件类型**（去掉序号与左右）。`shoe_f_l_nail_l2` → `shoe_nail`。
    分档核验拿它比"这一档是不是真多了东西"，而不是比件数——件数多可能只是同一部件
    切得更碎。"""
    toks = [re.sub(r"\d+$", "", p) for p in name.split("_")]
    return "_".join(p for p in toks if p and p not in ("l", "r", "f", "h"))


def _mirror_suffix(sfx: str) -> str:
    """件名尾的 _l/_r（含 nail_l2 这种带序号的）翻个面；居中件（toe）映射到自己。"""
    m = re.fullmatch(r"(.*)_([lr])(\d*)", sfx)
    if not m:
        return sfx
    return f"{m.group(1)}_{'r' if m.group(2) == 'l' else 'l'}{m.group(3)}"


MIN_BITE = 0.02  # 咬合体积下限（单位³）
MIN_SHOW = 0.10  # 露出蹄外的下限（单位）
STRAY_TOL = 0.02  # 与"不该碰的皮件"的容许交体积


def tack_els(t: Tack) -> list[dict]:
    return [e for e in t.skel.data["elements"] if e.get("_tack")]


def check_common(t: Tack, fit: Fit, bone_ok) -> list[str]:
    """不分种类的三条：不穿地、只挂允许的骨、件名前缀一致。

    「只挂允许的骨」是最要紧的一条：马具挂错骨不会报错、静止姿一模一样，只有跑起来
    才会看见鞍随着后腿甩、蹄铁留在原地。静帧永远抓不到。
    """
    els = tack_els(t)
    bad: list[str] = []
    if not els:
        return ["马具层一件都没有"]
    lo_all = min(float(_aabb(e)[0][1]) for e in els)
    if lo_all < -0.02:
        who = sorted(e["name"] for e in els if float(_aabb(e)[0][1]) <= lo_all + 0.02)
        bad.append(f"马具穿到地下：最低 y={lo_all:.3f}（{', '.join(who[:4])}）")
    for e in els:
        bone = _bone_of(t.skel.data, e["uuid"])
        if not bone_ok(bone):
            bad.append(f"{e['name']} 挂在 {bone}，不在本种马具允许的骨上（跨关节会被扯裂 / 随错的部位甩动）")
    return bad


def check_shoe(t: Tack, fit: Fit, spec: ShoeSpec) -> list[str]:
    """蹄铁自检。四条各自对应一种在渲染图上看不出来的翻车：

      · **穿地 / 悬空**：铁条底面必须恰在 y=0。差 0.1 个单位，静止侧视看不出来，
        但绑定层拿最低点当蹄底，四蹄触地点就整体偏了（皮层的距毛就是这么翻的车）。
      · **咬住蹄**：马具与蹄没有实交 = 悬在旁边。三视图里两件同色贴着，看不出来。
      · **露在蹄外**：整件埋进皮里 = 等于没做。这条和上一条是**反向**的一对，
        只查一条必然被另一条方向的错误绕过去。
      · **不碰不该碰的**：夹片长过头会戳进系或距毛。报出具体是哪一件，好回去调数。

    外加连通性（钉头/灵纹悬空）与左右镜像（单侧写错数）。
    """
    pelt_els, fits = fit.pelt_els, fit.hooves
    els = tack_els(t)
    bad = check_common(t, fit, lambda b: bool(b) and b.startswith("hoof_"))
    if not els:
        return bad

    by_foot: dict[str, list[dict]] = {}
    for e in els:
        # 件名形如 shoe_<tag>_<side>_<...>
        parts = e["name"].split("_")
        by_foot.setdefault(f"{parts[1]}_{parts[2]}", []).append(e)

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
    return bad


# ================================================================ 种类注册
@dataclass(frozen=True)
class Kind:
    label: str
    table: dict
    build: object
    check: object
    against: tuple[str, ...]  # 它压在**哪种毛色材质**上——配色对比度按这些查
    min_contrast: float  # 对比度下限。按"颜色在这件马具里承担多少辨识职责"定，见下


# 对比度门槛为什么分种类：**颜色承担的辨识职责不一样**。
#   · 蹄铁是贴在蹄上的一条细带，不改变剪影——玩家能不能看出马蹄上有铁，全靠颜色，
#     所以门槛高（45）。
#   · 马鞍改变整只马的剪影（鞍桥、垂下的鞍翼、晃着的镫、绕过肚子的带），远处一眼
#     就知道"这马配了鞍"。颜色只需"不是同一块颜色"，不必抢眼，所以门槛低（32）。
# 三种毛色（锈骝 128,72,43 / 枯原 148,126,82 / 碎雪 146,140,131）连同各自的暗部
# 几乎铺满了中等明度的暖色带，一刀切 45 会把**所有**棕色皮革排除掉——那不是在保证
# 可辨识，是在替美术拍板。
KINDS: dict[str, Kind] = {
    "shoe": Kind("蹄铁", SHOES, build_shoes, check_shoe, ("hoof",), 45.0),
    "saddle": Kind("马鞍", SADDLES, build_saddle, check_saddle, ("coat", "coat_dark"), 32.0),
}


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


# ---------------------------------------------------------------- 动画期核验
_FRAMES: dict[str, list] = {}


def bone_frames(pkey: str, n: int = 32) -> list[tuple[str, float, dict]]:
    """每档体型的**全骨世界变换**逐帧表，按体型缓存一次。

    马具刚性挂在骨上，所以"动画里会不会铲地 / 会不会陷进皮里"只取决于骨的世界变换
    加上马具自己的角点——与分档无关。算一次三档共用，省掉两轮逆解。
    """
    if pkey not in _FRAMES:
        import gen_anim as G
        from rig import Rig

        P = PROFILES[pkey]
        rig = Rig(FINAL / f"HorsePelt_{GEOM_COAT}_{pkey}.bbmodel")
        out = []
        for name in G.ANIMS:
            for i in range(n):
                out.append((name, i / n, rig.world(G.sample(rig, P, name, i / n))))
        rig.residuals.clear()
        _FRAMES[pkey] = out
    return _FRAMES[pkey]


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


def _by_bone(data: dict, pred) -> dict[str, list[dict]]:
    out: dict[str, list[dict]] = {}
    for e in data["elements"]:
        if pred(e):
            out.setdefault(_bone_of(data, e["uuid"]), []).append(e)
    return out


def check_anim_ground(t: Tack, pkey: str) -> tuple[float, str]:
    """动画里马具的最深穿地。蹄只转 12°，但蹄铁比蹄宽出一截，外缘的力臂更长——
    "蹄自己没穿地"推不出"蹄铁没穿地"，得单独量。"""
    pts = {b: np.array([c for e in els for c in _corners(e)], float)
           for b, els in _by_bone(t.skel.data, lambda e: e.get("_tack")).items()}
    worst, who = 0.0, ""
    for name, tt, W in bone_frames(pkey):
        for bone, A in pts.items():
            y = (A @ W[bone][:3, :3].T + W[bone][:3, 3])[:, 1].min()
            if -y > worst:
                worst, who = float(-y), f"{bone}@{name} t={tt:.2f}"
    return worst, who


# 容许增量按**这一对各自能保证什么**分三档，不是一刀切：
#   · body —— 刚性主体（鞍垫/座/鞍桥/鞍翼、整副蹄铁）对着躯干。这是"马具不会陷进
#     马里"的核心保证，卡得最紧。
#   · strap —— 束具（肚带/束带/扣）对着躯干。带子在真马身上是**被顶开**的，刚体盒
#     做不到，所以躯干一屈伸它必然陷进去一点。
#   · limb —— 任何马具对着**腿**。马腿收在桶身底下（前肢中线 x≈2.3，桶身半宽≈3.4，
#     腿在体内侧不在体外侧），肚带绕桶身一圈就必然横在肘的摆动弧上；袭步与倒毙里前臂
#     折到腹下，从肚带中间穿过去。这是本方案已知且接受的代价，给它一个**量出来的**
#     上限，而不是混进前两档里放大到谁都拦不住。
# body 定在 0.45：挽马倒毙那一帧鞍垫前段陷进背 0.40，这是"刚体鞍挂一根骨"这个决定
# **量出来的**代价（背在鞍下屈伸，鞍不跟着弯）。0.40 单位 = 2.5 cm，且方向是**陷进去**
# ——那一侧被马体挡着看不见；真正难看的是反方向（鞍浮起来离背），那条另有 `CONTACT`
# 断言专管，不靠这个数兜。
FIT_TOL = {"body": 0.45, "strap": 0.85, "limb": 2.40}
LIMB_BONES = ("scapula", "humerus", "radius", "carpus", "femur", "tibia", "tarsus", "fetlock", "hoof")
STRAP_WORDS = ("girth", "billet", "buckle")
# 必须**全程贴着马**的部件类型：鞍垫是鞍与马之间唯一的接触面，它一旦整片离开马体，
# 玩家看到的就是一副浮在背上方的鞍——比陷进去难看得多，而陷入深度那条判据完全看不见
# 这个方向（它只会报 0）。所以单独立一条：这些件在每一帧都必须与马体有实交。
MUST_CONTACT = ("saddle_pad",)


def check_anim_fit(t: Tack, pkey: str) -> dict[str, tuple[float, str]]:
    """动画里马具**比静止姿更深地陷进皮**多少，按上面三档分别取最大。

    马鞍是刚体、只挂一根骨，而它下面的背在屈伸——这是设计上就接受的代价（见
    `SADDLE_BONE` 注释），但"接受"必须有个数。

    三处判据设计要点：
      · 查的是**增量**不是绝对深度。鞍垫本来就该嵌进皮里一截（那是贴合），只有"动画
        让它比静止时陷得更深"才是问题。
      · 增量必须**逐对**算。首版拿"全场静止姿最深"当唯一基线去减每帧的"全场最深"，
        而鞍垫为了连通性共用底面、深埋在躯干里，那个全局基线一口气把腿穿过肚带的
        1.8 单位减没了——报出来只有 0.75，还以为没事。
      · **跳过皮层声明 `_loose` 的件**（耳/唇/额发/尾鬃股）。那些本来就是各自飘的，
        马蹄从垂下的尾鬃里穿过去不是缺陷。这跟 `shell_check` 认同一个声明——造型层
        说了哪些件不参与刚体贴合判定，两处判据都照办，不各猜各的。

    只比对**挂在别的骨上**的皮件：同骨的相对位置永不改变，算了也是零，白烧时间。
    """
    tack_by_bone = _by_bone(t.skel.data, lambda e: e.get("_tack"))
    pelt_by_bone = _by_bone(t.skel.data, lambda e: e.get("_pelt") and not e.get("_loose"))
    empty = {k: (0.0, "") for k in FIT_TOL}
    if not pelt_by_bone:  # 纯马具文件（没带皮），这条无从查起
        return empty
    tack = [(tb, e["name"], np.array(_corners(e), float),
             "strap" if any(w in e["name"] for w in STRAP_WORDS) else "body")
            for tb, els in tack_by_bone.items() for e in els]
    pelt = [(pb, e["name"], np.array(_corners(e), float), *_obb(e),
             pb.startswith(LIMB_BONES)) for pb, els in pelt_by_bone.items() for e in els]

    contact: dict[str, float] = {}  # 每个"必须贴着"的件，在全部帧里最差的那一次接触深度

    def scan(W, acc: dict, tag: str) -> None:
        # 先用世界 AABB 粗筛：47 × 155 对里真正靠近的只有个位数，逐对做 OBB 是白烧
        tw = [(tb, tn, A @ W[tb][:3, :3].T + W[tb][:3, 3], cls) for tb, tn, A, cls in tack]
        pw = [(pb, pn, C @ W[pb][:3, :3].T + W[pb][:3, 3], c, h, R, lb) for pb, pn, C, c, h, R, lb in pelt]
        tb_box = [(w.min(axis=0), w.max(axis=0)) for _, _, w, _ in tw]
        pb_box = [(w.min(axis=0), w.max(axis=0)) for _, _, w, _, _, _, _ in pw]
        for i, (tbone, tn, world, cls) in enumerate(tw):
            tlo, thi = tb_box[i]
            best = -1e9
            want = comp_type(tn) in MUST_CONTACT
            for j, (pbone, pn, _pw, c, h, R, is_limb) in enumerate(pw):
                same = pbone == tbone
                if same and not want:
                    continue
                plo, phi = pb_box[j]
                if (thi < plo).any() or (phi < tlo).any():
                    continue
                Wp = W[pbone]
                q = np.abs(((world - Wp[:3, 3]) @ Wp[:3, :3] - c) @ R)
                d = float((h[None, :] - q).min(axis=1).max())
                # 接触判据要把**同骨**的皮件也算进来：鞍垫压着的躯干件多半就挂在
                # thorax_back 上，与鞍同骨——按"增量"那条的规矩跳过同骨，接触这边就
                # 一个候选都不剩，`best` 停在哨兵值上，报出来是 −1e9 这种鬼数。
                # 同骨恰恰是最实的接触：相对位置永不改变，贴上了就永远贴着。
                if want and not is_limb and d > best:
                    best = d
                if same:
                    continue
                k = (tn, pn)
                if d > acc.get(k, (0.0, "", ""))[0]:
                    acc[k] = (d, "limb" if is_limb else cls, tag)
            if want and tag != "rest":
                # 一个候选都没有 = 这一帧整片飘在体外。哨兵值不要直接漏进报告里
                # （−1e9 读起来像 bug 不像结论），压成一个能看的负数。
                contact[tn] = min(contact.get(tn, 1e9), max(best, -9.99))

    rest: dict = {}
    scan({b: np.eye(4) for b in {*tack_by_bone, *pelt_by_bone}}, rest, "rest")
    worst: dict = {}
    for name, tt, W in bone_frames(pkey):
        scan(W, worst, f"{name} t={tt:.2f}")

    out = dict(empty)
    for (tn, pn), (d, bucket, when) in worst.items():
        inc = d - rest.get((tn, pn), (0.0,))[0]
        if inc > out[bucket][0]:
            out[bucket] = (inc, f"{tn}↔{pn}@{when}")
    if contact:
        tn = min(contact, key=lambda k: contact[k])
        out["contact"] = (contact[tn], tn)
    return out


# ================================================================ 装配 / CLI
def build(pkey: str, kind: str, tier: str) -> tuple[Tack, Fit]:
    pelt = FINAL / f"HorsePelt_{GEOM_COAT}_{pkey}.bbmodel"
    if not pelt.is_file():
        raise SystemExit(f"找不到皮层: {pelt}（先跑 gen_pelt.py）")
    skel = Skeleton(pelt)
    pelt_els = [dict(e) for e in skel.data["elements"] if e.get("_pelt")]
    fit = Fit(P=PROFILES[pkey], pelt_els=pelt_els, hooves=read_hooves(skel.data), torso=Torso(pelt_els))
    extend_texture(skel.data)
    t = Tack(skel, fit.P)
    KINDS[kind].build(t, fit, KINDS[kind].table[tier])
    return t, fit


def drop_pelt(t: Tack) -> None:
    """裁掉皮件、**保留整棵骨树**——动画轨道按骨 uuid 索引，骨树缺一根那条轨道就被
    静默忽略（`check_anim_bones` 查的就是这个）。"""
    keep = {e["uuid"] for e in t.skel.data["elements"] if e.get("_tack")}
    t.skel.data["elements"] = [e for e in t.skel.data["elements"] if e["uuid"] in keep]

    def prune(node):
        if isinstance(node, str):
            return node in keep
        node["children"] = [c for c in node.get("children", []) if prune(c)]
        return True

    for root in t.skel.data["outliner"]:
        prune(root)


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
    for kind, K in KINDS.items():
        for tk, spec in K.table.items():
            r0, g0, b0 = TACK_MATS[spec.mat]
            for coat in COATS.values():
                for key in K.against:
                    r1, g1, b1 = coat.mats[key]
                    d = ((r0 - r1) ** 2 + (g0 - g1) ** 2 + (b0 - b1) ** 2) ** 0.5
                    if d < K.min_contrast:
                        bad.append(f"{kind}/{tk} 的 {spec.mat} 与「{coat.label}」的 {key} 只差 {d:.1f}"
                                   f"（下限 {K.min_contrast:.0f}），远处看不出穿没穿")
    return bad


def check_geom_same_across_coats(pkey: str) -> list[str]:
    """马具**读到的每一件**皮件在三种毛色里必须几何全等。

    马具只按一种毛色量尺寸（`GEOM_COAT`），这条塌了就是量错了马。范围必须覆盖
    `is_shape` 认的那些与蹄件——只查蹄的话，鞍读的躯干件漂了照样查不出来。
    毛色专属的花纹件（dorsal_stripe / dapple）本来就只在一种毛色里存在，所以采样器
    从一开始就不读它们；这条断言与那条排除是同一个约定的两面。
    """
    from gen_pelt import COATS

    ref: dict[str, tuple] | None = None
    bad = []
    for ck in sorted(COATS):
        f = FINAL / f"HorsePelt_{ck}_{pkey}.bbmodel"
        if not f.is_file():
            continue
        cur = {e["name"]: (tuple(e["from"]), tuple(e["to"]))
               for e in json.loads(f.read_text())["elements"]
               if e.get("_pelt") and (is_shape(e["name"]) or e["name"].startswith("hoof_"))}
        if ref is None:
            ref = cur
        elif cur != ref:
            diff = sorted(set(cur) ^ set(ref)) or [k for k in cur if ref.get(k) != cur[k]]
            bad.append(f"{ck} 的皮件几何与 {GEOM_COAT} 不同（{', '.join(diff[:3])}）——马具尺寸来源不成立")
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
        for k, K in KINDS.items():
            print(f"{k}（{K.label}）：")
            for tk, spec in K.table.items():
                print(f"  {tk:9s} {spec.label:8s} {spec.blurb}")
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
        K = KINDS[kind]
        tiers = [args.tier] if args.tier else list(K.table)
        vols: dict[str, list[float]] = {}
        for pk in pkeys:
            for tier in tiers:
                if tier not in K.table:
                    print(f"未知分档 {tier}（{kind} 有 {', '.join(K.table)}）")
                    return 2
                spec = K.table[tier]
                t, fit = build(pk, kind, tier)
                vols.setdefault(tier, []).append((
                    sum(float(np.prod(np.array(e["to"]) - np.array(e["from"]))) for e in tack_els(t)),
                    {comp_type(e["name"]) for e in tack_els(t)},
                ))
                bad = K.check(t, fit, spec)
                # 贴合类核验必须在**裁掉皮层之前**做——裁完就没有可比对的皮了
                sink = 0.0
                who = ""
                fits = {k: (0.0, "") for k in FIT_TOL}
                if not args.skip_anim:
                    sink, who = check_anim_ground(t, pk)
                    if sink > SINK_TOL:
                        bad.append(f"动画里马具铲地 {sink:.2f} > {SINK_TOL}（{who}）")
                    fits = check_anim_fit(t, pk)
                    for bucket, (d, w) in fits.items():
                        if bucket in FIT_TOL and d > FIT_TOL[bucket]:
                            bad.append(f"[{bucket}] 比静止姿多陷进皮 {d:.2f} > {FIT_TOL[bucket]}（{w}）")
                    if "contact" in fits and fits["contact"][0] <= 0.0:
                        bad.append(f"鞍垫在动画里整片离开马体（{fits['contact'][1]} 最差接触 "
                                   f"{fits['contact'][0]:.2f}）——鞍会看着浮在背上方")

                name = f"Horse{kind.capitalize()}_{tier}_{pk}" + ("_on_horse" if args.with_horse else "")
                if not args.with_horse:
                    drop_pelt(t)
                t.skel.data["name"] = name
                t.skel.data["model_identifier"] = name
                out = (STAGES if args.with_horse else TACK_DIR) / f"{name}.bbmodel"
                out.parent.mkdir(parents=True, exist_ok=True)
                if not args.skip_anim:
                    t.skel.data["animations"] = anim_block(pk)
                    bad += check_anim_bones(t.skel.data)
                    # 带动画的模型走紧凑 JSON：indent=1 光缩进就占掉近一半体积
                    out.write_text(json.dumps(t.skel.data, ensure_ascii=False, separators=(",", ":")))
                else:
                    out.write_text(json.dumps(t.skel.data, ensure_ascii=False, indent=1))

                mark = "✓" if not bad else "✗"
                extra = "" if args.skip_anim else (
                    f" 贴地 {sink:.2f} " + " ".join(f"{k}{fits[k][0]:+.2f}" for k in FIT_TOL))
                print(f"{mark} {out.relative_to(FINAL.parents[1])}  【{spec.label} · {PROFILES[pk].label}】"
                      f"件 {t.count}{extra}")
                for m in bad:
                    print(f"    ✗ {m}")
                    rc = 1

        # 分档必须**看得出来**。首版只查"用料递增 ≥1.15×"——对蹄铁成立（那一档差别
        # 就是铁更厚更宽），对马鞍不成立：二档与三档都是皮鞍，差别在**配件**（鞍桥包
        # 灵铁、灵纹、灵铁镫），硬要求体积多 15% 只会逼出一副没来由的大鞍。
        # 改成两条一起查：**必须多出新的部件类型**（真的多了东西）+ **用料不得减少**。
        if len(tiers) == len(K.table) and len(pkeys) > 0:
            order = list(K.table)
            for i in range(len(order) - 1):
                a, b = vols.get(order[i], []), vols.get(order[i + 1], [])
                for (va, ca), (vb, cb) in zip(a, b):
                    if not cb - ca:
                        print(f"    ✗ {order[i + 1]} 相对 {order[i]} 没有任何新部件类型，分档看不出来")
                        rc = 1
                    if vb <= va:
                        print(f"    ✗ {order[i + 1]} 的用料没比 {order[i]} 多（{va:.1f} → {vb:.1f}）")
                        rc = 1
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
