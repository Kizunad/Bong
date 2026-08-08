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

到了甲这一层又多一条：**跨装备也要断言**。四种马具是四份文件四条产线，各自的判据都
看不见对方——甲把鞍架空、甲把缰埋掉、骑手够不着缰，四份文件全绿也照样发生。凡是要问
"另一种马具占了哪儿"的地方，一律**把它真造一遍再量实件**（`other_tack`），不抄常数。

分级走矿物正典的金属阶梯（`server/src/mineral/registry.rs`）：
粗铁（凡）→ 杂钢（凡）→ 灵铁（灵）。**不用"玄铁"**——worldview §三 L63 禁玄/陨/星/仙/太/古。
甲多两档非金属 / 重甲（粗布、锁环、淬黑厚板），仍不出这条阶梯的口径。

用法:
  python3 scripts/models/horse/gen_tack.py                      # 全部马具 × 三档
  python3 scripts/models/horse/gen_tack.py --kind shoe --tier lingtie --profile large
  python3 scripts/models/horse/gen_tack.py --with-horse         # 叠在皮层上看贴合
  python3 scripts/models/horse/gen_tack.py --suit               # 四种一起穿，看整套
  python3 scripts/models/horse/gen_tack.py --list
"""

from __future__ import annotations

import argparse
import base64
import io
import itertools
import json
import math
import re
from dataclasses import dataclass

import numpy as np
from gen_muscle import Skeleton
from gen_pelt import FINAL, STAGES, SWATCH, _corners, crest_y, seam_pad
from gen_skeleton import (
    HEAD_PITCH,
    NECK,
    NECK_ROM,
    PROFILES,
    HeadSpace,
    Profile,
    _obb,
    connected_components,
    neck_seams,
    rom,
    uid,
)
from PIL import Image

TACK_DIR = FINAL / "tack"
TACK_ROW = 4  # 贴图第 5 行起（0-1 行骨/肌，2-3 行皮），追加不动已有 UV
GEOM_COAT = "rust"  # 几何三色同源，取一份当尺寸来源（另两份由 check_geom_same_across_coats 对拍）

Vec = tuple[float, float, float]


def _lerpf(a: float, b: float, s: float) -> float:
    return a + (b - a) * s

# 材质表**只准追加不准插入**：UV 索引由这里的顺序派生，插一行会把已出的马具整体错位。
#
# 表里若干色是被 `check_contrast` **推**到现在这个位置的，不是重新挑的：判据只报"离某
# 种毛色太近"，改法一律取**离原色最近的合规色**——颜色是造型层拍过的，判据该把它推到
# 刚好过线，不该替美术重选一个。首轮判据只查主色（`mat`），把 mat_dark / mat_trim 漏了；
# 补上之后一次撞出五处：毡的暗部离碎雪身色 12.4（破毡鞍最大的一块面，在青毛马上整片
# 消失）、粗铁离枯原身色 22.4（粗革鞍的镫与扣）、麻绳离枯原身色 16.2（破毡鞍的肚带）、
# 粗铁暗部与杂钢暗部离碎雪的蹄 34.8 / 23.3（蹄铁的趾带与夹）。
TACK_MATS: dict[str, tuple[int, int, int]] = {
    # 粗铁走**锈色**而不是灰：灰的粗铁 (96,88,80) 和碎雪的蹄 (92,86,80) 只差 4.5，
    # 整只马渲出来和赤脚一模一样（见 check_contrast）。锈也更合"捡来的铁料"这档。
    "iron_crude": (166, 92, 52),
    "iron_crude_dark": (118, 74, 40),
    "iron_rust": (116, 74, 44),  # 锈斑 / 锈钉
    "steel": (132, 134, 138),  # 杂钢：冷灰，比粗铁亮两档
    "steel_dark": (94, 106, 125),
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
    "felt_dark": (122, 126, 108),
    "leather": (46, 30, 24),  # 粗革：鞣得不匀、油到发乌的深棕
    "leather_dark": (26, 17, 14),
    "rope": (180, 164, 116),  # 麻绳：新打的草黄
    # --- 缰绳起追加（只准往后加：UV 索引由顺序派生）---
    "rope_tar": (52, 46, 36),  # 上过桐油的绳结 / 磨损段：麻绳唯一能和三种毛色都拉开的暗部
    # --- 马甲起追加（只准往后加：UV 索引由顺序派生）---
    # 甲盖住整只马最大的一片面，配色比别的马具更难：三种毛色（连同暗部）几乎铺满了
    # **中等明度的暖色带**，而金属天然就落在中等明度的灰带上。碎雪的暗部 (104,99,92)
    # 正是一块中灰——任何"看着像铁"的中灰都会撞上它。所以五档一律往两头躲：
    # 布往蓝里、锁环往深冷里、板甲往亮里、重甲往黑里，中灰那一段整个让开。
    "cloth": (92, 106, 132),  # 粗布：褪掉大半的蓝草染（末法唯一还染得起的颜色）
    "cloth_dark": (58, 68, 90),
    "mail": (70, 82, 88),  # 锁环：环挨环，整片读成一块发暗的冷灰
    "mail_dark": (46, 56, 62),
    "plate": (158, 168, 184),  # 杂钢板：磨得发亮的那一档，靠**亮**从毛色里跳出来
    "plate_dark": (104, 116, 136),
    "iron_black": (56, 58, 64),  # 重甲：淬黑的粗铁，靠**黑**跳出来（剪影即分档）
    "iron_black_dark": (36, 38, 44),
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
            rot=None, org=None, chain: tuple[str, int] | None = None) -> None:
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
                # 链声明：(链名, 序号)。跨骨的带（缰绳）分段之后，"相邻两段还连着吗"
                # 只有动画期才答得出——静止姿它们当然连着，交叠是照 ROM 给的。
                # 同 `_glow`：造型层是唯一知道哪几件本来是一条带的人，所以在这里声明，
                # 不让下游按件名去猜。
                "_chain": list(chain) if chain else None,
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
SHAPE_RE = re.compile(r"torso_\w+|dorsal_\d+|croup_cap_\d+|chest_front")

# 马具**读得到**的全部皮件。四处采样器（躯干 / 蹄 / 头 / 颈）认的就是这一组，
# `check_geom_same_across_coats` 也照它对拍三种毛色——两边共用一个定义，才不会出现
# "采样器多读了一件毛色专属的花纹，对拍却没查它"（马具只按一种毛色量尺寸，读到毛色
# 专属件就等于量错了马）。往采样器里加新的皮件，必须同时进这张表。
READ_RE = re.compile("|".join((
    SHAPE_RE.pattern,
    r"hoof_\w+",  # 蹄铁
    r"head_shell_\d+|jaw_line_[lr]|jowl_[lr]|chin|lip_upper|lip_lower",  # 笼头
    r"neck_\d+|neck_throat_\d+|mane_(root|fall|tip)_\d+",  # 缰绳沿颈走（鬃也得让开）
    r"dock_\d+",  # 搭后要给尾根让出洞
)))


def is_shape(name: str) -> bool:
    return bool(SHAPE_RE.fullmatch(name))


def is_read(name: str) -> bool:
    return bool(READ_RE.fullmatch(name))


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
        named = [(e["name"], e["from"], e["to"]) for e in pelt_els if is_shape(e["name"])]
        self.boxes = [(f, t) for _n, f, t in named]
        if not self.boxes:
            raise SystemExit("皮层里找不到躯干件——马具无处可贴")
        self.z0 = min(f[2] for f, _t in self.boxes)
        self.z1 = max(t[2] for _f, t in self.boxes)
        # 桶身**本体**的前缘：不含胸前那一块（`chest_front`，只到胸底那么高）也不含
        # 背中线（`dorsal_*`，只有背棱那么窄）。整只马最前面那一小段其实只有这两件，
        # 中间是空的——一排甲片按 `z0` 起手，前端那一截就悬在胸前的空处。
        self.z_barrel0 = min(f[2] for n, f, _t in named if n.startswith("torso_"))

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

    def half_or_none(self, z: float, y: float) -> float | None:
        got = list(self._cover({1: y, 2: z}))
        return max(max(t[0], -f[0]) for f, t in got) if got else None

    def half_at(self, z: float, y: float) -> float:
        v = self.half_or_none(z, y)
        if v is None:
            raise SystemExit(f"躯干在 (y={y:.2f}, z={z:.2f}) 处没有皮件")
        return v

    def band(self, z: float, y0: float, y1: float, n: int = 7) -> tuple[float, float]:
        """[y0,y1] 这一段桶身的（最窄, 最宽）半宽。

        两处不能想当然：
          · 查询点要**钳进该 z 的皮内**。甲片的上下缘本来就该压到背线以上 / 腹线以下
            一点（那一截埋在马里，看不见），钳一下比拒绝服务有用——这里要的是"这片甲
            该做多宽"，不是"这个点上有没有皮"。
          · 钳进 `at(z)` 的上下界**并不保证有皮**：那两个界是并集的极值，中间可以有洞。
            身体最前端就是——那儿只剩 `chest_front`（低）与背中线（高），中间一段是空
            的。所以逐点问，没皮的点跳过，不是整条崩掉。
        """
        _hw, ytop, ybot = self.at(z)
        eps = (ytop - ybot) * 1e-3
        vals = [v for k in range(n)
                if (v := self.half_or_none(
                    z, min(max(_lerpf(y0, y1, k / (n - 1)), ybot + eps), ytop - eps))) is not None]
        if not vals:
            raise SystemExit(f"躯干在 z={z:.2f} 的 y[{y0:.2f},{y1:.2f}] 整段没有皮件")
        return min(vals), max(vals)


# 躯干骨自头向尾。每根**子骨的 pivot 就是它与父骨的关节**（骨树是 hips → lumbar →
# thorax_back → thorax_front），所以这三个 pivot 的 z 就是躯干上的三道关节。
BARD_TRUNK = ("thorax_front", "thorax_back", "lumbar", "hips")


class Trunk:
    """躯干的**分段**采样器：一条纵向的甲带该在哪儿断开、断口两边各留多少交叠。

    与颈那边（`NeckLine`）是同一条道理，只是分界不再来自 `neck_seams` 而来自骨树的
    pivot：接缝压在关节上，两段就绕着共同的那一点转，张开的只是随半径线性增长的楔形，
    可以用交叠吃掉；接缝一旦偏离关节，整个接缝面平移，从第一度起就裂。
    """

    def __init__(self, P: Profile, pivot: dict[str, Vec]) -> None:
        self.P = P
        miss = [b for b in BARD_TRUNK if b not in pivot]
        if miss:
            raise SystemExit(f"骨树里找不到躯干骨 {miss} —— 甲无处分段")
        # (关节 z, 关节 pivot, 这道关节头侧的骨)；按 z 升序 = 自头向尾
        self.joints = [(pivot[b][2], pivot[b], b) for b in BARD_TRUNK[:-1]]
        self.joints.sort()

    def bone_at(self, z: float) -> str:
        for jz, _p, bone in self.joints:
            if z < jz:
                return bone
        return BARD_TRUNK[-1]

    def split(self, za: float, zb: float, y_lo: float, y_hi: float,
              hw: float) -> list[tuple[str, float, float, float, float]]:
        """[za,zb] 按关节切段 → [(骨, z起, z止, 起端交叠, 止端交叠)]。

        交叠按 `seam_pad(face_r(...), ROM)` 给——与皮层躯干（`gen_pelt.part_torso`）
        同一把尺、同一张 ROM 表。半径含横向半宽 `hw`：脊柱这几节不只俯仰，倒毙时整条
        脊还绕 z 侧倾，横向偏移**确实**参与摆动（与枕关节那道缝正相反，那里绕 x 转，
        横向偏移整个落在轴上）。

        试过再卡一条"探出不得过邻段之半"（想拦住甲片探过整个邻段够到再下一段）：
        躯干这边 `body` 一点不动（0.77 → 0.77，那个数由**板自己的厚度**决定，不由
        探出长度决定），鸡颈那边直接把接缝拉裂（0.60 → −0.63，颈一节只有两个单位长，
        照 ROM 该给的交叠本来就比邻段还长）。所以不卡——量不出好处的几何不留。
        """
        cuts = [jz for jz, _p, _b in self.joints if za + 1e-6 < jz < zb - 1e-6]
        edges = [za, *cuts, zb]
        out = []
        for i in range(len(edges) - 1):
            z0, z1 = edges[i], edges[i + 1]
            bone = self.bone_at((z0 + z1) / 2)
            pads = []
            for k, z in ((i, z0), (i + 1, z1)):
                if k == 0 or k == len(edges) - 1:
                    pads.append(0.0)
                    continue
                piv = next(p for jz, p, _b in self.joints if abs(jz - z) < 1e-6)
                nb = [self.bone_at(z - 1e-4), self.bone_at(z + 1e-4)]
                pads.append(seam_pad(max(math.hypot(hw, y - piv[1]) for y in (y_lo, y_hi)),
                                     max(rom(b) for b in nb)))
            out.append((bone, z0, z1, pads[0], pads[1]))
        return out


@dataclass
class Fit:
    """马具装配所需的全部「来自皮层的量」。装配函数只准从这里取数，不准另写常数。"""

    P: Profile
    pelt_els: list[dict]
    hooves: dict[str, "HoofFit"]
    torso: Torso
    head: "Head"
    neck: "NeckLine"
    trunk: Trunk


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
            # 两根竖梁按**外侧 / 内侧**命名，不按 ±x 编号：编号版本里"1 号"在左边是外
            # 梁、在右边是内梁，同一个名字指着两样东西。几何本来就是镜像的，坏的是名字
            # ——而按名字配对的左右镜像判据因此永远对不上（`check_mirror` 抓出来的）。
            for tag, s in (("out", 1.0), ("in", -1.0)):
                dx = sgn * s
                t.box(b, f"saddle_stirrup_ring_{tag}_{side}",
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
    bad += check_mirror(els)
    return bad


# ================================================================ 缰（笼头 + 缰绳）
# 缰是第一件**跨关节**的马具。蹄铁与鞍都是"整件挂一根骨"的刚体，那条路子在缰上走不
# 通：一根骨吃不下从嘴角到鬐甲这段距离。所以缰绳照皮层的办法逐骨分段，接缝按同一把尺
# （`seam_pad(r, rom)`）留交叠，两头认同一张 ROM 表。
#
# 难的是**枕关节**：十条动画里头相对末节颈骨实测转到 80°（graze t=0.12，三档体型都是），
# 交叠得给到 1.35·r·tan40° ≈ 1.13·r —— 缰要是像真缰那样从嘴角直拉向骑手的手，它在枕关节
# 附近离转心 6.5 个单位，套接就得七八个单位，一低头照样张开。
#
# 出路不是加大交叠，是把接缝**放到转轴上**：缰在枕髁高度换骨。头局部系的原点就是枕髁，
# 所以头段与颈段各自含住转心那一点，头一俯仰两段绕着**共同的那一点**转，永远不分开
# （r 只剩缰自身离中线的横向偏移 ≈2 个单位，交叠 2.7 就够）。这正是
# `gen_skeleton.neck_pivots`「pivot 必须落在接缝上」那条道理，换到一条带上。
#
# 几何上这也不是迁就：真马配好缰绳、骑手没提缰时，缰就是从嘴角上到耳后、再顺着颈搭
# 到鬐甲的——换骨点落在枕后，本来就是缰在马身上真正的转折处。
BRIDLE_BONE = "skull"

# 笼头各带在**头局部系**里的位置（× 头长）。与 `gen_pelt.part_head` 同一套坐标：
# 那边写 h(0.092) 的地方这边也写 h(0.092)，改头型两边一起跟着变。
Z_CROWN = -0.020  # 项带：耳后（耳在 z[-0.215,-0.055]，压在耳上马会甩头）
Z_BROW = -0.300  # 额带：耳前、额发之下
Z_NOSE = -0.600  # 鼻革：颧下、鼻孔之上（鼻孔 z[-1.045,-0.955]，压住鼻孔是要闷死马）
Z_THROAT = -0.150  # 咽革：腮下（腮 z[-0.420,-0.090]）
Z_BIT = -0.880  # 衔铁：嘴角（上唇 z[-1.040,-0.900]）
Y_BIT = -0.232  # 上唇 y[-0.255,-0.195] 与下唇 y[-0.320,-0.250] 的交界 = 口裂

# 带的**宽**与**厚**是两回事，下限也不是一个数——首版拿同一个下限套两处，结果每条带
# 都离脸鼓出 0.85 个单位，倒毙侧躺时下侧的颊带整条戳进地里 0.21（动画铲地判据抓的）。
#   · 宽（沿脸铺开的那一维）决定**看不看得见**：一个体素 = 6.25 cm，真缰绳（2 cm）换算
#     过来只有 0.3 个单位，远处直接消失。所以宽压 0.85 的下限。
#   · 厚（离脸鼓出来的那一维）只需要够避开共面 z-fighting。真皮带就是贴在脸上的，
#     鼓出来 5 cm 是给马戴了一圈护栏。
# 与座面 `SEAT_MIN_*` 同类：下限来自**渲染器与观察者**，不来自马，矮马的缰不跟着细。
TACK_MIN_W = 0.85
TACK_MIN_T = 0.16


class Head:
    """头的**局部系**采样器（原点 = 枕髁，−z 朝吻端，y 向上）。

    头皮件整体按 HEAD_PITCH 俯仰过，世界 AABB 是斜的——直接量世界坐标问"脸多宽"，
    得到的是被俯仰角污染的数。所以笼头一律写在头局部系里，最后统一过一次头部变换。

    只收 `rotation` 恰为 HEAD_PITCH 的件：耳（dp=-8）另有自己的俯角，把它算进"颅壳
    多宽"会让项带凭空外扩一截。

    坐标一律用**头长的比例**，与 `gen_pelt.part_head` 里 `h(0.092)` 的那个 0.092 是同一
    个数。混用绝对长度与比例是这一层最容易犯的错（比例的 −0.6 和绝对的 −0.6 差一个
    数量级，还都跑得通）。
    """

    SHELL = re.compile(r"head_shell_\d+")
    UNDER = re.compile(r"head_shell_\d+|jaw_line_[lr]|chin|lip_lower")

    def __init__(self, P: Profile, pelt_els: list[dict]) -> None:
        self.P = P
        self.hs = HeadSpace(None, BRIDLE_BONE, (0.0, P.y_occiput, P.z_occiput), HEAD_PITCH)
        self.local: dict[str, tuple[Vec, Vec]] = {}
        for e in pelt_els:
            box = self.local_of(e) if is_read(e["name"]) else None
            if box is not None:
                self.local[e["name"]] = box
        if not any(self.SHELL.fullmatch(n) for n in self.local):
            raise SystemExit("皮层里找不到 head_shell_* —— 笼头无处可贴")

    def _cover(self, pat: re.Pattern, z: float):
        for n, (lo, hi) in self.local.items():
            if pat.fullmatch(n) and lo[2] - 1e-6 <= z <= hi[2] + 1e-6:
                yield lo, hi

    def shell(self, z: float) -> tuple[float, float, float]:
        """局部 z 处颅壳的（半宽, 顶, 底）。"""
        got = list(self._cover(self.SHELL, z))
        if not got:
            raise SystemExit(f"颅壳在局部 z={z:.3f} 处没有皮件——笼头定位越界了")
        return (max(max(hi[0], -lo[0]) for lo, hi in got),
                max(hi[1] for lo, hi in got),
                min(lo[1] for lo, hi in got))

    def under(self, z: float) -> float:
        """局部 z 处**含下颌件**的最低 y。鼻革与咽革要从下颌底下绕过去，只问颅壳会短半截。"""
        got = list(self._cover(self.UNDER, z))
        return min(lo[1] for lo, hi in got) if got else self.shell(z)[2]

    def to_world(self, p: Vec) -> Vec:
        """头局部（**头长比例**）→ 世界（绝对）。"""
        H = self.P.H
        return self.hs.to_world((p[0] * H, p[1] * H, p[2] * H))

    def local_of(self, e: dict) -> tuple[Vec, Vec] | None:
        """世界件 → 头局部 AABB（头长比例）。只对 `rotation` 恰为 HEAD_PITCH 的件成立
        （额外的 dp 会把盒转出局部轴对齐），别的返回 None 让判据自己报出来。"""
        rx, ry, rz = e["rotation"]
        if ry or rz or abs(rx - HEAD_PITCH) > 1e-6:
            return None
        a = math.radians(HEAD_PITCH)
        c, s = math.cos(a), math.sin(a)
        H = self.P.H
        ctr = [(e["from"][i] + e["to"][i]) / 2 for i in range(3)]
        half = [(e["to"][i] - e["from"][i]) / 2 / H for i in range(3)]
        d = (ctr[0], ctr[1] - self.P.y_occiput, ctr[2] - self.P.z_occiput)
        lc = (d[0] / H, (c * d[1] + s * d[2]) / H, (-s * d[1] + c * d[2]) / H)
        return (tuple(lc[i] - half[i] for i in range(3)), tuple(lc[i] + half[i] for i in range(3)))

    def put(self, t: Tack, name: str, lo: Vec, hi: Vec, *, mat: str, dp: float = 0.0,
            glow: bool = False, chain=None, bone: str = BRIDLE_BONE) -> None:
        """局部系轴对齐盒 →（绕自身中心转 dp）→ 过头部变换 → 落成世界件。

        只允许绕**局部 x** 转（dp）：件的旋转序是 Rz·Ry·Rx，只有同轴才能和头部俯仰
        叠加成一个刚体变换（同 `gen_skeleton.HeadSpace`）。所以笼头每一段都走在
        y-z 平面里，横向（x）的变化靠分段，不靠转。
        """
        c = tuple((a + b) / 2 for a, b in zip(lo, hi))
        half = tuple(abs(a - b) / 2 * self.P.H for a, b in zip(lo, hi))
        wc = self.to_world(c)
        t.box(bone, name, tuple(w - h for w, h in zip(wc, half)), tuple(w + h for w, h in zip(wc, half)),
              mat=mat, glow=glow, chain=chain, rot=(HEAD_PITCH + dp, 0.0, 0.0), org=wc)

    def strap(self, t: Tack, name: str, x0: float, x1: float, p0: tuple[float, float],
              p1: tuple[float, float], th: float, *, mat: str, glow: bool = False,
              chain=None, ext: tuple[float, float] = (0.0, 0.0)) -> None:
        """局部系里 p0→p1（各是一对 (y, z)）的一段带，厚 th、横向占 x0..x1，两端各外延 ext。"""
        dy, dz = p1[0] - p0[0], p1[1] - p0[1]
        ln = math.hypot(dy, dz)
        if ln < 1e-6:
            raise ValueError(f"{name}: 起止点重合，量不出方向")
        uy, uz = dy / ln, dz / ln
        my = (p0[0] + p1[0]) / 2 + uy * (ext[1] - ext[0]) / 2
        mz = (p0[1] + p1[1]) / 2 + uz * (ext[1] - ext[0]) / 2
        half = (ln + ext[0] + ext[1]) / 2
        # 未转之前带沿局部 +z 拉长；绕 x 转 a 把 (0,0,1) 送到 (0,−sin a, cos a)
        self.put(t, name, (x0, my - th / 2, mz - half), (x1, my + th / 2, mz + half),
                 mat=mat, glow=glow, chain=chain, dp=math.degrees(math.atan2(-uy, uz)))


def _slice_abs_x(e: dict, y: float) -> float | None:
    """盒（可带旋转）在高度 y 的截面上，最大的 |x|。

    取平面 y=常数与 12 条棱的交点后求最大——线性函数在凸多边形上的极值必在顶点，
    所以这是**精确解**，不是采样近似。鬃是绕 z 转过的斜盒，按 AABB 估会把缰往外
    推出一大截（估多了缰就飘在颈外）。
    """
    c, h, R = _obb(e)
    sgn = list(itertools.product((-1.0, 1.0), repeat=3))
    pts = [c + R @ (np.array(s) * h) for s in sgn]
    best: float | None = None
    for i in range(8):
        for j in range(i + 1, 8):
            if sum(a != b for a, b in zip(sgn[i], sgn[j])) != 1:
                continue
            a, b = pts[i], pts[j]
            if (a[1] - y) * (b[1] - y) > 0:
                continue
            if abs(b[1] - a[1]) < 1e-9:
                cand = (abs(a[0]), abs(b[0])) if abs(a[1] - y) < 1e-9 else ()
            else:
                s = (y - a[1]) / (b[1] - a[1])
                cand = (abs(a[0] + s * (b[0] - a[0])),)
            for v in cand:
                best = v if best is None else max(best, v)
    return best


class NeckLine:
    """缰绳沿颈走的那条线，以及每一节颈骨上"皮伸到多远"。

    **高度**取颈脊弦（`crest_y`，骨/皮/鬃共用的那条）之下一个固定落差。落差不是挑的：
    由「在枕关节处正好落到转心高度」定死——接缝要压在转轴上（见本节抬头的推导），那
    缰在枕后的高度就只能是枕髁的高度，反推出落差。改头颈比例它自己会跟着走。

    **横向**按**位置**问，不按骨问：
      · 鬃也挂在颈骨上，只问颈皮的话缰就从鬃里穿过去；
      · 颈皮各段之间有按 ROM 给的交叠，粗的那节会伸到细的那节的地盘上。只问"缰所在
        那根骨"的皮，缰就正好埋在邻节那块更宽的皮里——静止姿看不出来（被邻节的皮挡
        着），一屈伸就被吞掉（挽马倒毙时实测 0.87）。
    `_slice_abs_x` 按高度切精确截面：鬃是绕 z 转过的斜盒，按 AABB 估会把缰推到颈外。
    """

    def __init__(self, P: Profile, by_bone: dict[str, list[dict]]) -> None:
        self.P = P
        self.drop = crest_y(P, P.z_occiput) - P.y_occiput
        self.seams = neck_seams(P)
        self.els: list[tuple[float, float, dict]] = []
        for bone in NECK:
            for e in by_bone.get(bone, []):
                if not is_read(e["name"]):
                    continue
                lo, hi = _aabb(e)
                self.els.append((float(lo[2]), float(hi[2]), e))

    def y_at(self, z: float) -> float:
        return crest_y(self.P, z) - self.drop

    def outer_at(self, z: float, y: float, hair: bool = True) -> float | None:
        """没有皮的地方返回 None。判据用这个——判据的职责是**报告**坏模型，不是崩在
        它上面；崩掉的话后面几条根本没机会跑（弄坏一处就看不到别处还坏不坏了）。

        `hair=False` 只问**颈皮本身**，不含鬃。两种问法各有各的用处，都不是默认：
          · 缰要**让开**鬃（从鬃上面过去），所以缰问的是含鬃的那个值；
          · 鸡颈是**盖住**鬃的（真甲就压在鬃上），要贴的是颈皮。照含鬃的值做，鸡颈就
            浮在鬃外面、离颈皮一大截——而鬃是 `_hair`，贴合判据把它跳过，于是报出
            "整片飘在体外"（首版实测哨兵值 −9.99）。
        """
        got = [v for z0, z1, e in self.els
               if z0 - 1e-6 <= z <= z1 + 1e-6 and (hair or not e.get("_hair"))
               and (v := _slice_abs_x(e, y)) is not None]
        return max(got) if got else None

    def outer(self, z: float, y: float, hair: bool = True) -> float:
        """造型用这个——量不到就是定位越界，属于写错了代码，该当场停下。"""
        v = self.outer_at(z, y, hair)
        if v is None:
            raise SystemExit(f"颈在 (y={y:.2f}, z={z:.2f}) 处没有{'' if hair else '（不含鬃的）'}皮件")
        return v

    def outer_span(self, za: float, ya: float, zb: float, yb: float, n: int = 5,
                   hair: bool = True) -> float:
        """一整段所跨范围内最外的那个值。只问两端会漏掉中段鼓出来的皮。"""
        return max(self.outer(_lerpf(za, zb, k / (n - 1)), _lerpf(ya, yb, k / (n - 1)), hair)
                   for k in range(n))


@dataclass(frozen=True)
class ReinSpec:
    key: str
    label: str
    blurb: str
    mat: str  # 主带
    mat_dark: str  # 暗部 / 结
    mat_trim: str  # 金属件（衔铁 / 衔环 / 扣）
    strap: float  # 头上各带的宽（× 头长，压 TACK_MIN_W 下限）
    thick: float  # 带厚（× 头长，压下限）
    rein_w: float  # 缰绳截面（× 头长，压下限）
    bit: bool  # 有没有衔铁。一档是**无衔**的绳笼头，靠压鼻梁控马
    browband: bool
    throat: bool
    glow: bool = False


REINS: dict[str, ReinSpec] = {
    # 一档：一圈麻绳打的无衔笼头。没有衔铁 = 没有口内着力点，只能靠勒鼻梁——
    # 松了不听使唤，紧了磨破鼻梁。这是"能牵着走"与"能骑着使唤"的分界线。
    "rope": ReinSpec(
        key="rope", label="绳缰", blurb="麻绳打的无衔笼头，勒鼻控马。松了不听使唤，紧了磨破鼻梁。",
        mat="rope", mat_dark="rope_tar", mat_trim="rope_tar",
        strap=0.052, thick=0.040, rein_w=0.046,
        bit=False, browband=False, throat=False,
    ),
    # 二档：正经笼头。额带 + 咽革 + 粗铁衔与衔环——有了口内着力点才谈得上骑乘操控。
    "leather": ReinSpec(
        key="leather", label="粗革缰", blurb="正经笼头：额带、咽革、粗铁衔。有了口内着力点才使得上劲。",
        mat="leather", mat_dark="leather_dark", mat_trim="iron_crude",
        strap=0.058, thick=0.034, rein_w=0.050,
        bit=True, browband=True, throat=True,
    ),
    # 三档：灵铁衔与灵铁扣，额带一道灵纹。主带仍是革（同马鞍二三档：差别在**配件**）。
    "lingtie": ReinSpec(
        key="lingtie", label="灵铁缰", blurb="灵铁衔与灵铁扣，额带一道灵纹，勒口时泛淡蓝。",
        mat="leather", mat_dark="lingtie_dark", mat_trim="lingtie",
        strap=0.062, thick=0.038, rein_w=0.054,
        bit=True, browband=True, throat=True, glow=True,
    ),
}


def part_bridle(t: Tack, fit: Fit, spec: ReinSpec) -> tuple[float, float, float]:
    """笼头：全部挂 `skull`。返回缰绳的**系点**（头局部系，x 是缰的**内侧面**该落在哪
    ——给中心线的话缰会有一半陷进脸里），二三档在衔环、一档在鼻革结。

    下颌那几件皮（腮 / 颌线 / 下唇 / 颏）都挂在 `jaw` 上而且皮层已声明 `loose`，所以
    鼻革从下颌底下绕过去、张口时下颌顶进鼻革，是**真马就会发生的事**（鼻革本来就是
    限制张口的），不是缺陷。
    """
    H = fit.P.H
    Hd = fit.head
    sw = max(H * spec.strap, TACK_MIN_W)  # 带宽：看得见
    th = max(H * spec.thick, TACK_MIN_T)  # 带厚：只需避开共面
    swl, thl = sw / H, th / H  # 回到局部系的"头长比例"
    m, md, mt = spec.mat, spec.mat_dark, spec.mat_trim

    hw_c, top_c, _ = Hd.shell(Z_CROWN)
    hw_n, top_n, _ = Hd.shell(Z_NOSE)
    bot_n = Hd.under(Z_NOSE)
    hw_b = Hd.shell(Z_BIT)[0]
    # 颊带横向：外侧必须**越过腮**（腮 x 到 0.168H，挂在 jaw 上会动），否则一张口
    # 颊带就埋进腮里。所以问的是腮而不是颅壳。
    hw_j = max(hi[0] for n, (lo, hi) in Hd.local.items() if n.startswith("jowl_"))

    # ---- 项带：压在枕后、耳前的那道横梁。**只箍到颅壳外一线**——按腮宽给（0.168H）
    # 会让它在正面比整个头还宽，读成戴了个箍。腮在耳下，项带压根不经过那里 ----
    x_c = hw_c + thl
    Hd.put(t, "rein_crown", (-x_c, top_c - thl * 0.4, Z_CROWN - swl / 2),
           (x_c, top_c + thl * 0.6, Z_CROWN + swl / 2), mat=md)

    for sgn, side in ((-1.0, "l"), (1.0, "r")):
        # ---- 颊带上段：自项带端沿脸侧下行，**走在眼后**（眼 z[-0.43,-0.29]）----
        # x 从项带端一直铺到**腮外**：项带箍着颅壳（窄），颊带要垂过腮（宽），两者
        # 差着 0.04H。一条等宽的带接不上，就让它在 x 上同时盖住两处——皮带贴在弯
        # 曲的颊上，本来看着就是这么个"越往下越厚"。
        # 起点还要**探进项带里**（ext）：带厚只有 0.16 个单位，端面对端面差个零头就
        # 断开（挽马那副绳缰的项带就这么落了单）。
        lo_x, hi_x = sorted((sgn * (hw_c - thl * 0.4), sgn * (hw_j + thl)))
        Hd.strap(t, f"rein_cheek_up_{side}", lo_x, hi_x,
                 (top_c - thl, Z_CROWN), (-0.150, -0.230), swl, mat=m, ext=(thl * 2.0, 0.0))
        # ---- 颊带下段：转向前下方接到衔环。x 收回来，与上段在 x 上留交叠才连得上 ----
        lo2, hi2 = sorted((sgn * (hw_b + thl * 1.6), sgn * (hw_j - thl * 0.6)))
        Hd.strap(t, f"rein_cheek_lo_{side}", lo2, hi2,
                 (-0.140, -0.245), (Y_BIT, Z_BIT), swl, mat=m, ext=(0.0, swl * 0.4))

    # ---- 鼻革：绕鼻梁与下颌一整圈。四件闭环——只做上面那道横梁，侧视图和"绕过去了"
    # 长得一模一样（同肚带那条判据的道理）----
    x_n = hw_n + thl
    Hd.put(t, "rein_nose_top", (-x_n, top_n - thl * 0.3, Z_NOSE - swl / 2),
           (x_n, top_n + thl * 0.7, Z_NOSE + swl / 2), mat=m)
    Hd.put(t, "rein_nose_bot", (-x_n, bot_n - thl * 0.7, Z_NOSE - swl / 2),
           (x_n, bot_n + thl * 0.3, Z_NOSE + swl / 2), mat=m)
    for sgn, side in ((-1.0, "l"), (1.0, "r")):
        lo_x, hi_x = sorted((sgn * hw_n, sgn * x_n))
        Hd.put(t, f"rein_nose_{side}", (lo_x, bot_n - thl * 0.7, Z_NOSE - swl / 2),
               (hi_x, top_n + thl * 0.7, Z_NOSE + swl / 2), mat=m)
        if spec.glow:
            # 灵纹也刻一道在鼻革**侧面**：额带那道只在正面 / 斜 3/4 看得见，纯侧视里
            # 它是立着的一条边，整只马看过去等于没有——而侧视正是玩家最常看到的角度。
            # 三档与二档除了配件就靠这道纹分，它必须在主视角上成立。
            go = sgn * thl * 0.25
            Hd.put(t, f"rein_nose_glow_{side}", (sgn * x_n - go, (top_n + bot_n) / 2 - swl * 0.62, Z_NOSE - swl * 0.30),
                   (sgn * x_n + go, (top_n + bot_n) / 2 + swl * 0.62, Z_NOSE + swl * 0.30), mat="glow", glow=True)

    # ---- 额带（二档起）：额前一道横梁 + 两条沿颅顶接回项带的连梁。
    # 侧视里最好认的一件，灵纹也刻在它上面 ----
    if spec.browband:
        hw_w, top_w, _ = Hd.shell(Z_BROW)
        x_w = hw_w + thl
        Hd.put(t, "rein_brow", (-x_w, top_w - thl * 0.2, Z_BROW - swl * 0.6),
               (x_w, top_w + thl * 0.8, Z_BROW + swl * 0.6), mat=md)
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            lo_x, hi_x = sorted((sgn * (hw_w - thl * 0.2), sgn * x_w))
            Hd.strap(t, f"rein_brow_{side}", lo_x, hi_x,
                     (top_w, Z_BROW), (top_c, Z_CROWN), thl, mat=md, ext=(swl * 0.5, swl * 0.5))
        if spec.glow:
            Hd.put(t, "rein_brow_glow", (-x_w * 0.86, top_w + thl * 0.8, Z_BROW - swl * 0.34),
                   (x_w * 0.86, top_w + thl * 1.0, Z_BROW + swl * 0.34), mat="glow", glow=True)

    # ---- 咽革（二档起）：自项带端绕过咽喉。真马配它就是防笼头被从耳上撸下来 ----
    if spec.throat:
        bot_t = Hd.under(Z_THROAT)
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            lo_x, hi_x = sorted((sgn * (hw_j - thl * 0.4), sgn * (hw_j + thl * 0.8)))
            Hd.strap(t, f"rein_throat_{side}", lo_x, hi_x,
                     (top_c - thl, Z_CROWN), (bot_t + thl * 0.6, Z_THROAT), thl, mat=m)
        x_t = hw_j + thl * 0.8
        Hd.put(t, "rein_throat_bar", (-x_t, bot_t - thl * 0.4, Z_THROAT - thl * 0.7),
               (x_t, bot_t + thl * 0.6, Z_THROAT + thl * 0.7), mat=m)

    # ---- 衔铁与衔环（二档起）----
    if spec.bit:
        x_r = hw_b + thl * 2.2
        Hd.put(t, "rein_bit", (-x_r, Y_BIT - thl * 0.4, Z_BIT - thl * 0.5),
               (x_r, Y_BIT + thl * 0.4, Z_BIT + thl * 0.5), mat=mt)
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            # 衔环做**实心圆片**而不是空心环：环外径按真尺寸（7 cm）只有 1.1 个单位，
            # 挖空之后梁宽不到半个体素，远处先糊成一团、再消失。马镫是另一回事——
            # 它外径 4 倍于此，挖空才读得出"环"（见 saddle_stirrup_*）。尺度决定做法。
            lo_x, hi_x = sorted((sgn * (hw_b + thl * 1.2), sgn * (x_r + thl * 0.5)))
            Hd.put(t, f"rein_ring_{side}", (lo_x, Y_BIT - swl * 0.9, Z_BIT - swl * 0.55),
                   (hi_x, Y_BIT + swl * 0.9, Z_BIT + swl * 0.55), mat=mt)
        anchor = (hw_b + thl * 1.2, Y_BIT, Z_BIT)
    else:
        # 无衔档：缰系在鼻革侧面的绳结上。这是这一档"使不上劲"的形状来源。
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            lo_x, hi_x = sorted((sgn * (hw_n + thl * 0.2), sgn * (x_n + thl * 1.1)))
            Hd.put(t, f"rein_knot_{side}", (lo_x, (top_n + bot_n) / 2 - swl * 0.7, Z_NOSE - swl * 0.8),
                   (hi_x, (top_n + bot_n) / 2 + swl * 0.7, Z_NOSE + swl * 0.8), mat=md)
        anchor = (hw_n + thl * 0.4, (top_n + bot_n) / 2, Z_NOSE)
    return anchor


def part_reins(t: Tack, fit: Fit, spec: ReinSpec, anchor: tuple[float, float, float]) -> None:
    """两条缰：自系点上行到枕后换骨，再顺着颈搭到鬐甲。

    分段与颈骨一一对应（同颈皮 / 鬃），接缝落在关节 pivot 上、按 `seam_pad` 外扩。
    唯独头↔颈那道缝压在**枕髁**上——头局部系的原点，也就是转心本身。
    """
    P, Hd, NL = fit.P, fit.head, fit.neck
    H, W = P.H, P.wither
    rw = max(H * spec.rein_w, TACK_MIN_W)
    gap = W * 0.006  # 缰离皮的空隙：贴着皮做会共面 z-fighting
    seams = NL.seams

    # 缰在枕后的横向位置：**与末节颈骨那一段缰取同一个 x**。两边各按各的位置算皮宽的
    # 话，头段与颈段在 x 上错开半个带宽——那道缝本来就是两条带斜着交叉，截面再错开，
    # 交叠只剩 0.11；对齐之后靠的是整条截面。这是本设计最要紧的一道缝，值得为它把
    # 顺序倒过来算（先量颈那一段，再定头段的落点）。
    z_top, y_top = seams[len(NECK) - 1][2], NL.y_at(seams[len(NECK) - 1][2])
    x_poll = NL.outer_span(P.z_occiput, P.y_occiput, z_top, y_top) + gap
    # 头↔颈那道缝的交叠。半径要量的是**离转轴的距离**，不是离转心那个点的距离——
    # 枕关节绕 x 轴俯仰，缰在枕后的横向偏移（x≈2）整个落在轴上，一点不参与摆动。
    # `seam_pad` 的常规用法（`gen_pelt.face_r`）把 x 半宽也算进 r，对颈皮那种宽件是
    # 稳妥的保守，对一条横跨到 x=2 的带就成了灾难：算出来 3.2 个单位的套接，那截
    # 探过枕后的矛，低头时扫进颈里 1.08、倒毙时戳进地里 0.23（两条判据同时报的
    # 其实是同一件事）。这里接缝的截面正骑在轴上，离轴最远只有半个带厚。
    # 试过再往上加套接长度（×3、×6）：这道缝的余量**一点不动**——两条带是斜交，
    # 撑住它的是截面不是长度。加长只是白给一截探出枕后的杆，所以不加。
    pad_poll = seam_pad(rw / 2, rom(BRIDLE_BONE))

    for sgn, side in ((-1.0, "l"), (1.0, "r")):
        ch = f"rein_{side}"
        # --- 头段：系点 → 枕髁。局部系里枕髁就是原点，所以终点写 (0, 0) ---
        # 分两段：系点那头贴着脸（x 小），枕后那头要够到颈侧（x 大）。一个盒转不出
        # x 的变化（只能绕局部 x 转），靠两段在 x 上交叠接上。
        ax, ay, az = anchor
        rwl = rw / H  # 缰的截面，局部系（头长比例）
        px = x_poll / H  # 枕后那一段的内侧面 —— 与颈段第一节同值（见 x_poll 处）
        # 每一段各占**一条带宽**，不是从系点一路铺到枕后的一整块板：首版把 x 写成
        # 「系点到枕后」的整个跨度，出来是一片 1.6 单位宽的斜板贴在脸上。
        # 段数不是凑的：x 从系点（贴着嘴角）到枕后（够到颈侧）要挪 0.74 个单位（挽马），
        # 分两段就是一段一挪，交叠只剩 0.11——静止姿就险，`check_anim_chain` 报的正是
        # 它。分三段把落差摊薄到三分之一，交叠回到 0.6 以上。
        NH = 3
        for k in range(NH):
            s0, s1 = k / NH, (k + 1) / NH
            sm = (s0 + s1) / 2
            xin = _lerpf(ax, px, sm)
            x_lo, x_hi = sorted((sgn * xin, sgn * (xin + rwl)))
            Hd.strap(t, f"rein_line_head_{k + 1}_{side}", x_lo, x_hi,
                     (_lerpf(ay, 0.0, s0), _lerpf(az, 0.0, s0)),
                     (_lerpf(ay, 0.0, s1), _lerpf(az, 0.0, s1)), rwl, mat=spec.mat, chain=(ch, k),
                     ext=(rwl * (0.5 if k == 0 else 0.7),
                          pad_poll / H if k == NH - 1 else rwl * 0.7))

        # --- 颈段：自枕髁起，逐骨一段，到鬐甲为止 ---
        # z 的起点是枕髁（-21.99）而不是 seams[-1]（-21.74）：那一小段归末节颈骨管，
        # 缰在这里换骨，接缝正压在枕髁上。
        z_head = P.z_occiput
        for k in range(len(NECK) - 1, -1, -1):
            bone = NECK[k]
            za = z_head if k == len(NECK) - 1 else seams[k + 1][2]
            zb = seams[k][2]
            ya, yb = (P.y_occiput if k == len(NECK) - 1 else NL.y_at(za)), NL.y_at(zb)
            # 末段（挨着鬐甲那节）多探出去一点：骑手的手落在这儿，缰得够得着
            if k == 0:
                zb += 0.030 * P.L
                yb = NL.y_at(zb)
            xr = NL.outer_span(za, ya, zb, yb) + gap
            # 每一段的交叠按**这一节自己的**半径给：缰离 pivot 的距离随颈高变
            r_a = math.hypot(xr + rw, ya - (P.y_occiput if k == len(NECK) - 1 else seams[k + 1][1]))
            r_b = math.hypot(xr + rw, yb - seams[k][1])
            pad_a = pad_poll if k == len(NECK) - 1 else seam_pad(r_a, NECK_ROM)
            pad_b = 0.0 if k == 0 else seam_pad(r_b, NECK_ROM)
            # 厚度逐段错开 3%：相邻两段在交叠区里同宽同斜时上下面近共面，会闪。
            # 差这一点点谁也看不出来，但把共面消掉了。
            th = rw * (1.0 + 0.03 * (k % 2))
            _strap_world(t, bone, f"rein_line_neck_{k + 1}_{side}",
                         sgn * xr, sgn * (xr + rw), (ya, za), (yb, zb), th,
                         mat=spec.mat, chain=(ch, NH + len(NECK) - 1 - k), ext=(pad_a, pad_b))
        if spec.glow:
            # 灵纹只点在末段（骑手手边那一截）：整条缰刻满会读成一根发光棍
            e = [x for x in tack_els(t) if x["name"] == f"rein_line_neck_1_{side}"][0]
            lo, hi = e["from"], e["to"]
            t.box(NECK[0], f"rein_line_glow_{side}",
                  (lo[0] + (hi[0] - lo[0]) * 0.25, hi[1], lo[2] + (hi[2] - lo[2]) * 0.30),
                  (lo[0] + (hi[0] - lo[0]) * 0.75, hi[1] + rw * 0.22, lo[2] + (hi[2] - lo[2]) * 0.70),
                  mat="glow", glow=True, rot=tuple(e["rotation"]), org=tuple(e["origin"]))


def _strap_world(t: Tack, bone: str, name: str, x0: float, x1: float,
                 p0: tuple[float, float], p1: tuple[float, float], th: float, *,
                 mat: str, chain=None, glow: bool = False,
                 ext: tuple[float, float] = (0.0, 0.0)) -> None:
    """世界系里 p0→p1（各是一对 (y, z)）的一段带。颈骨静止姿无旋转，所以世界即局部。"""
    dy, dz = p1[0] - p0[0], p1[1] - p0[1]
    ln = math.hypot(dy, dz)
    uy, uz = dy / ln, dz / ln
    my = (p0[0] + p1[0]) / 2 + uy * (ext[1] - ext[0]) / 2
    mz = (p0[1] + p1[1]) / 2 + uz * (ext[1] - ext[0]) / 2
    half = (ln + ext[0] + ext[1]) / 2
    ang = math.degrees(math.atan2(-uy, uz))
    t.box(bone, name, (x0, my - th / 2, mz - half), (x1, my + th / 2, mz + half),
          mat=mat, glow=glow, chain=chain, rot=(ang, 0.0, 0.0), org=((x0 + x1) / 2, my, mz))


def build_rein(t: Tack, fit: Fit, spec: ReinSpec) -> None:
    part_reins(t, fit, spec, part_bridle(t, fit, spec))


# 骑手能握到缰的距离（单位）。玩家模型臂长约 12——手要够得着缰的末端，否则做骑乘
# 动画时手只能凭空攥着空气。这条和 `STIRRUP_REACH` 是同一类：**由骑手定，不由马定**。
REIN_REACH = 12.0
REIN_GAP = 0.9  # 缰离颈皮的最大空隙（单位）；超过就是飘在颈外，不是搭在颈上


def check_rein(t: Tack, fit: Fit, spec: ReinSpec) -> list[str]:
    """缰的自检。除通用两条外，五条各挡一种在渲染图上看不出来的翻车：

      · **笼头真的绕过头了**：鼻革要在颅壳上方、下颌下方、两侧之外都有件。侧视图里
        "一道贴在脸侧的深色"和"绕过鼻梁一整圈"长得一模一样（同肚带那条）。
      · **衔在口裂上**：衔铁必须落在上下唇之间那条缝里。高半个单位就是压在鼻梁上，
        低半个单位就是挂在下巴底下——两种都会渲成"嘴边有根横棍"，看不出差别。
      · **缰搭在颈上而不是埋进去/飘在外**：缰的外侧面必须露在颈皮之外（埋进去等于
        没做），同时离皮不能超过 REIN_GAP（飘着就不是搭在颈上）。这两条是**反向**
        的一对，只查一条必然被另一头的错误绕过去。
      · **骑手够得到缰**：缰末端要落在鞍座的臂展之内——这是第一条**跨装备**的判据。
        缰与鞍是两份文件、两条产线，各自都"没问题"而合起来骑手抓不到缰，只有把两边
        摆到一起量才看得见。
      · **单一连通体** + **左右镜像**。
    """
    P, Hd, NL = fit.P, fit.head, fit.neck
    els = tack_els(t)
    bad = check_common(t, fit, lambda b: b == BRIDLE_BONE or b in NECK)
    if not els:
        return bad
    by = {e["name"]: e for e in els}

    # ---- 鼻革绕了一圈 ----
    # 在**头局部系**里查：世界坐标被 54° 俯角搅过，"鼻革在鼻梁上方"这种关系在那边
    # 是一句斜着的话。四件各证一条边，缺一条就不是"绕过去"。
    hw_n, top_n, _ = Hd.shell(Z_NOSE)
    bot_n = Hd.under(Z_NOSE)
    for key, test, why in (
        ("top", lambda lo, hi: hi[1] > top_n, f"没盖过鼻梁顶（{top_n:.3f}）"),
        ("bot", lambda lo, hi: lo[1] < bot_n, f"没绕到下颌底下（{bot_n:.3f}）"),
        ("l", lambda lo, hi: lo[0] < -hw_n, f"没伸到左脸侧之外（{-hw_n:.3f}）"),
        ("r", lambda lo, hi: hi[0] > hw_n, f"没伸到右脸侧之外（{hw_n:.3f}）"),
    ):
        e = by.get(f"rein_nose_{key}")
        if e is None:
            bad.append(f"鼻革缺 {key} 件——绕不成一圈")
            continue
        box = Hd.local_of(e)
        if box is None:
            bad.append(f"rein_nose_{key} 带了额外旋转，绕圈判据的局部换算不再成立")
        elif not test(*box):
            bad.append(f"鼻革 {key} 件{why}")

    # ---- 衔在口裂上 ----
    if spec.bit:
        bit = by.get("rein_bit")
        if bit is None:
            bad.append("分档声明有衔铁，却一件都没出")
        else:
            up, lowlip = Hd.local.get("lip_upper"), None
            for n in ("lip_lower",):
                lowlip = Hd.local.get(n) or lowlip
            if up and lowlip:
                gapy = (lowlip[1][1], up[0][1])  # 下唇顶 / 上唇底 = 口裂那条缝
                if not min(gapy) - 0.06 <= Y_BIT <= max(gapy) + 0.06:
                    bad.append(f"衔铁不在口裂上：Y_BIT={Y_BIT:.3f}，口裂 {min(gapy):.3f}~{max(gapy):.3f}")
        for side in ("l", "r"):
            if f"rein_ring_{side}" not in by:
                bad.append(f"{side} 侧缺衔环——缰无处可系")
    elif "rein_bit" in by:
        bad.append("分档声明无衔，却出了衔铁")

    # ---- 缰搭在颈上：露得出来、又没飘走 ----
    for k in range(len(NECK)):
        bone = NECK[k]
        for side in ("l", "r"):
            e = by.get(f"rein_line_neck_{k + 1}_{side}")
            if e is None:
                bad.append(f"{bone} 上缺缰绳段（{side}）")
                continue
            lo, hi = _aabb(e)
            xr = max(abs(lo[0]), abs(hi[0]))
            xi = min(abs(lo[0]), abs(hi[0]))
            y, z = (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2
            skin = NL.outer_at(z, y)
            if skin is None:
                bad.append(f"{e['name']} 跑到颈皮之外的位置去了（y={y:.1f} z={z:.1f}），没得比对")
            elif xr <= skin:
                bad.append(f"{e['name']} 埋进颈皮：外缘 {xr:.2f} ≤ 皮 {skin:.2f}")
            if skin is not None and xi - skin > REIN_GAP:
                bad.append(f"{e['name']} 飘在颈外：内缘离皮 {xi - skin:.2f} > {REIN_GAP}")

    # ---- 骑手够得到 ----
    tail = [e for e in els if e["name"].startswith("rein_line_neck_1_")]
    if not tail:
        bad.append("缰没有末端段——够不到骑手")
    else:
        end = np.array([max(_aabb(e)[1][0] for e in tail),
                        float(np.mean([_aabb(e)[1][1] for e in tail])),
                        max(_aabb(e)[1][2] for e in tail)])
        for tk, seat in saddle_seats(fit).items():
            d = float(np.linalg.norm(end - np.array(seat)))
            if d > REIN_REACH:
                bad.append(f"缰末端离{SADDLES[tk].label}的座面 {d:.1f} 单位 > 骑手臂展 {REIN_REACH}")

    comps = connected_components(_Shim(els))
    if len(comps) != 1:
        detail = " / ".join(f"{len(c)} 件({c[0]}…)" for c in comps[:3])
        bad.append(f"整副缰应是一整体，实为 {len(comps)} 块：{detail}")
    bad += check_mirror(els)
    return bad


# ---------------------------------------------------------------- 跨装备
# 三种马具是三份文件三条产线，各自都"没问题"而**合起来**穿帮的事只有摆到一起才看得见
# （骑手够不着缰、甲把鞍架空、甲把缰埋了）。所以要问另一种马具的尺寸时，一律**把它真
# 造一遍**再量实件，不另抄一份常数——抄的那份迟早和真件漂开，而漂开时两边都还是绿的。
_OTHER: dict[tuple[str, str, str], list[dict]] = {}


def other_tack(fit: Fit, kind: str, tier: str) -> list[dict]:
    key = (fit.P.key, kind, tier)
    if key not in _OTHER:
        t = Tack(_BareSkel(), fit.P)
        KINDS[kind].build(t, fit, KINDS[kind].table[tier])
        _OTHER[key] = tack_els(t)
    return _OTHER[key]


def saddle_seats(fit: Fit) -> dict[str, tuple[float, float, float]]:
    """三档鞍各自的座面中心（世界）。缰的"够不够得着"要对**每一档**鞍都成立——
    三档座面高低差一个多单位，只对着中间那档量等于没量。"""
    out = {}
    for tk in SADDLES:
        els = other_tack(fit, "saddle", tk)
        seats = [e for e in els if e["name"].startswith("saddle_seat")] or \
                [e for e in els if e["name"].startswith("saddle_pad")]
        out[tk] = (0.0,
                   max(e["to"][1] for e in seats),
                   (min(e["from"][2] for e in seats) + max(e["to"][2] for e in seats)) / 2)
    return out


def saddle_span(fit: Fit) -> tuple[float, float]:
    """**三档鞍全部件**的纵向并集。甲要整段让开它——理由不是构图，是三档鞍在这一段里
    从上到下都占满了：鞍垫压在背上、鞍翼垂在肋侧、镫环挂到腿边、肚带绕过肚子。
    这一段里没有哪一条水平缝是空的，甲挤进去必然和其中一件穿插。
    """
    lo, hi = 1e9, -1e9
    for tk in SADDLES:
        for e in other_tack(fit, "saddle", tk):
            lo = min(lo, float(_aabb(e)[0][2]))
            hi = max(hi, float(_aabb(e)[1][2]))
    return lo, hi


class _BareSkel:
    """只为量鞍座尺寸而生的空骨架壳：`build_saddle` 只往里塞件，不读皮。"""

    def __init__(self) -> None:
        self.data = {"elements": [], "outliner": [], "groups": []}

    def attach(self, bone: str, el: dict) -> None:
        self.data["elements"].append(el)


def check_mirror(els: list[dict]) -> list[str]:
    """整副马具关于 x=0 左右镜像。件名尾的 _l/_r 翻面，居中件映射到自己。

    绕 x 的旋转在镜像下不变，所以笼头这种带俯角的件也能直接比。
    """
    bad = []
    by = {e["name"]: e for e in els}
    for name, e in by.items():
        m = _mirror_suffix(name)
        o = by.get(m)
        if o is None:
            bad.append(f"{name} 没有镜像伙伴 {m}")
            continue
        dx = abs(e["from"][0] + o["to"][0]) + abs(e["to"][0] + o["from"][0]) + abs(e["origin"][0] + o["origin"][0])
        dyz = max(abs(e[q][i] - o[q][i]) for q in ("from", "to", "origin") for i in (1, 2))
        dr = max(abs(a - b) for a, b in zip(e["rotation"], o["rotation"]))
        if dx > 0.01 or dyz > 0.01 or dr > 0.01:
            bad.append(f"{name} 与 {m} 不镜像（Δx={dx:.3f} Δyz={dyz:.3f} Δrot={dr:.3f}）")
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


def _pen(a: dict, b: dict) -> float:
    """两件的**穿透深度**（单位）：>0 = 真的插在一起，≤0 = 分开了这么远。

    与 `_overlap_vol` 的分工：那边按 AABB 算交体积，只对轴对齐的件成立；一旦件带了
    旋转（缰、鸡颈这些沿颈脊斜着走的带），AABB 比真形状大出一大截，两条平行错开的带
    会被判成撞在一起。跨装备那几条判据两边都可能是斜盒，只能走分离轴。
    """
    return _sat_gap(_obb(a), _obb(b))


def comp_type(name: str) -> str:
    """件名 → **部件类型**（去掉序号与左右）。`shoe_f_l_nail_l2` → `shoe_nail`。
    分档核验拿它比"这一档是不是真多了东西"，而不是比件数——件数多可能只是同一部件
    切得更碎。"""
    toks = [re.sub(r"\d+$", "", p) for p in name.split("_")]
    return "_".join(p for p in toks if p and p not in ("l", "r", "f", "h"))


def _mirror_suffix(sfx: str) -> str:
    """件名里的 l/r 词元（含 nail_l2 这种带序号的）翻个面；居中件（toe）映射到自己。

    按**词元**翻而不是只看结尾：`saddle_stirrup_ring_l_1` 的边标在中间，只认结尾的
    版本匹配不上，于是它和**自己**比镜像——永远报"不镜像"，而真正的左右错位反倒
    查不出来。判据自比自，比没有判据更坏（看着在查，其实在瞎报）。
    """
    def flip(p: str) -> str:
        if p[:1] in ("l", "r") and (p[1:] == "" or p[1:].isdigit()):
            return ("r" if p[0] == "l" else "l") + p[1:]
        return p

    return "_".join(flip(p) for p in sfx.split("_"))


MIN_BITE = 0.02  # 咬合体积下限（单位³）
MIN_SHOW = 0.10  # 露出蹄外的下限（单位）
STRAY_TOL = 0.02  # 与"不该碰的皮件"的容许交体积
CROSS_TOL = 0.02  # 跨装备穿透深度的容许量（单位）


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


# ================================================================ 马甲
# 甲是第一件**裹住整个躯干**的马具，也是第一件必须和别的马具在同一匹马上共处的。
# 三个决定：
#
#   · **札甲**（横排甲片逐骨分段）。缰那边逐骨分段是被逼的——一根骨吃不下从嘴角到鬐甲
#     那么长。甲这边不是：真札甲本来就是一片压一片编起来的，压叠的方向还讲究（后片压
#     前片，刀锋顺着滑开）。所以这一层的"分段 + 交叠"不是权宜，是**形制本身**；
#     `seam_pad` 给出的那点交叠，正好就是甲片该搭的那一叠。
#   · **让开鞍位**。三档鞍在纵向那一段里从上到下占满了：垫压在背上、翼垂在肋侧、镫环
#     挂到腿边、肚带绕过肚子——没有哪一条水平缝是空的。甲挤进去必然和其中一件穿插，
#     而两件马具是两份文件，各自的判据都看不见对方。所以甲整段让开（`saddle_span`），
#     分成前后两片。真甲也是分片的：当胸、马身甲、搭后本就是三件，靠带子连起来。
#   · **鸡颈的下缘由缰反推**。缰沿颈侧走的那条线是既成事实（已交付），鸡颈要是照
#     "颈深的几成"随手给一个数，正好把缰埋掉——而缰在另一份文件里，甲这边的判据一条
#     都不会响。所以下缘 = 三档缰在颈上的最高点再让开一线（`crinet_floor`）。
BARD_TOP = 0.86  # 甲面上缘（× 该处躯干高）：再往上桶身收成背棱，平板贴不住，也会撞上颈
BARD_CELL = 0.085  # 同骨内再切格的最大长度（× 体长）：一整块平板贴不住前后弯的桶身
BARD_LAP = 0.45  # 札片竖向搭叠（× 一排的高）
BARD_CLEAR = 0.010  # 甲与缰之间留的空（× 鬐甲高）
BARD_BITE = 0.8  # 甲片内侧面埋进皮多深（× 板厚）：贴合判据要的是**实交**，不是相切


@dataclass(frozen=True)
class BardSpec:
    key: str
    label: str
    blurb: str
    mat: str  # 甲面
    mat_dark: str  # 暗部（尻板 / 脊梁 / 垂缘）
    mat_trim: str  # 系带 / 铆钉 / 扣
    th: float  # 甲面厚（× 鬐甲高）
    hem: float  # 下摆到哪（0 = 只盖上半，1 = 到腹底）
    rows: int  # 侧面札片排数
    peytral: bool  # 当胸板（布档只有一块布帘，没有硬板）
    croup_plate: bool  # 尻板
    crinet: bool  # 鸡颈
    skirt: float  # 垂缘再往下加一截（× 躯干高）；0 = 无
    spine: bool  # 背脊梁
    glow: bool = False


BARDS: dict[str, BardSpec] = {
    # 一档：一块粗布障泥，绳子一捆。挡得住枝刺与夜寒，挡不住刃。
    "cloth": BardSpec(
        key="cloth", label="粗布甲", blurb="蓝草染褪的粗布障泥，麻绳捆在身上。挡枝刺挡夜寒，不挡刃。",
        mat="cloth", mat_dark="cloth_dark", mat_trim="rope",
        th=0.013, hem=0.74, rows=3, peytral=False, croup_plate=False,
        crinet=False, skirt=0.0, spine=False,
    ),
    # 二档：布底上缀锁环，胸前多一块。锁环挡切不挡砸——这一档的形状来源。
    "mail": BardSpec(
        key="mail", label="锁子甲", blurb="布底上缀满锁环，胸前一片当胸。挡得住切，挡不住砸。",
        mat="mail", mat_dark="mail_dark", mat_trim="iron_crude",
        th=0.017, hem=0.82, rows=4, peytral=True, croup_plate=False,
        crinet=False, skirt=0.0, spine=False,
    ),
    # 三档：杂钢札片，胸前尻上各压一块整板。这是"挡得住砸"的分界线。
    "light": BardSpec(
        key="light", label="轻铁甲", blurb="杂钢札片编成，当胸与尻上各压一块整板。轻，马跑得动。",
        mat="plate", mat_dark="plate_dark", mat_trim="iron_crude",
        th=0.021, hem=0.88, rows=5, peytral=True, croup_plate=True,
        crinet=False, skirt=0.0, spine=False,
    ),
    # 四档：灵铁札片 + 鸡颈。灵铁轻，同样的护面不压马；甲片行间一道灵纹。
    "lingtie": BardSpec(
        key="lingtie", label="灵铁甲", blurb="灵铁札片连鸡颈，甲片行间一道细灵纹。轻而不折真元。",
        mat="lingtie", mat_dark="lingtie_dark", mat_trim="lingtie",
        th=0.024, hem=0.92, rows=5, peytral=True, croup_plate=True,
        crinet=True, skirt=0.0, spine=False, glow=True,
    ),
    # 五档：淬黑的厚板，垂缘落到腹下，背上一道脊梁。护得最全，也最沉。
    "heavy": BardSpec(
        key="heavy", label="重铁甲", blurb="淬黑厚板，垂缘落到腹下，背上一道脊梁。护得最全，马也最累。",
        mat="iron_black", mat_dark="iron_black_dark", mat_trim="iron_crude",
        th=0.030, hem=0.96, rows=6, peytral=True, croup_plate=True,
        crinet=True, skirt=0.055, spine=True,
    ),
}


def crinet_floor(fit: Fit) -> float:
    """鸡颈下缘该落在颈脊之下多远（单位）——由**三档缰实件**的最高点反推。

    随手给一个"颈深的几成"必然出事：缰沿颈侧走的那条线已经交付、写死在另一份文件里，
    鸡颈盖下去正好把它埋掉，而甲这边一条判据都不会响（两份文件互相看不见）。

    量的是「离颈脊多深」而不是绝对高度：颈脊是一条斜线，缰与鸡颈都平行于它，只有换到
    这个口径上，一个数才能对整条颈成立。
    """
    P = fit.P
    best = 1e9
    for tk in REINS:
        for e in other_tack(fit, "rein", tk):
            if not e["name"].startswith("rein_line_neck"):
                continue
            best = min(best, min(crest_y(P, c[2]) - c[1] for c in _corners(e)))
    if best > 1e8:
        raise SystemExit("量不到缰沿颈那条带 —— 鸡颈的下缘没有依据，不能凭空给一个数")
    return best - P.u(BARD_CLEAR)


def tack_low_y(fit: Fit, kind: str, za: float, zb: float) -> float:
    """另一种马具在 [za,zb] 这一段里垂到多低（世界 y）。甲的上缘由它压住。

    两处都用它，是同一件事的两头：
      · 缰绕过鬐甲落到肩上——肩帘的上缘照"躯干高的几成"随手给，正好切进那段缰里；
      · 鞍垂在肋侧（翼、镫）——低摆的上缘同理。
    """
    lo = 1e9
    for tk in KINDS[kind].table:
        for e in other_tack(fit, kind, tk):
            c = np.array(_corners(e), float)
            if c[:, 2].max() < min(za, zb) or c[:, 2].min() > max(za, zb):
                continue
            lo = min(lo, float(c[:, 1].min()))
    return lo


def saddle_free(fit: Fit, y0: float, y1: float, za: float, zb: float,
                m: float) -> list[tuple[float, float]]:
    """在 [y0,y1] 这个高度带上，[za,zb] 里**没被三档鞍占住**的那几段 z。

    首版让甲整段让开鞍位，结果把马身上最大最显眼的一片（整个桶身）留成了光的，远处
    看像给马挂了两块牌子。但鞍并不是在每个高度上都占满：垫压在背上、翼垂在肋侧、
    镫挂到腿边，各占各的高度；真正从背顶一路绕到腹下、把每个高度都堵死的只有**肚带
    那一圈**。所以逐排问而不是整段让开——高处几排在鞍前鞍后各一段，低处几排几乎通到
    底，只在肚带那儿断开。甲于是绕着鞍长出来，而不是被鞍砍成两截。

    顺带，"整副甲是两片"这个结论也是从这儿来的、不是拍的：能把上下所有排一起切断的
    只有肚带，所以无论切出多少段，连起来永远是肚带前一片、肚带后一片。
    """
    free = [(za, zb)]
    for tk in SADDLES:
        for e in other_tack(fit, "saddle", tk):
            c = np.array(_corners(e), float)
            if c[:, 1].max() < y0 or c[:, 1].min() > y1:
                continue
            b0, b1 = float(c[:, 2].min()) - m, float(c[:, 2].max()) + m
            nxt = []
            for a0, a1 in free:
                if b1 <= a0 or b0 >= a1:
                    nxt.append((a0, a1))
                    continue
                if a0 < b0:
                    nxt.append((a0, b0))
                if b1 < a1:
                    nxt.append((b1, a1))
            free = nxt
    return free


def _zs(za: float, zb: float, n: int = 5) -> list[float]:
    return [_lerpf(za, zb, k / (n - 1)) for k in range(n)]


def neck_outer_x(fit: Fit, za: float, zb: float, y0: float, y1: float) -> float:
    """颈皮（连鬃）在这一小块（z ∈ [za,zb]，y ∈ [y0,y1]）里横向伸到多远。
    **甲片的内侧面不许比它更深。**

    甲片的内侧面本来是要埋进马体里的（见 `_lame_row`），可肩这一段的马体里还塞着
    颈的根部——颈是七根会动的骨，低头吃草时整段扫下来，埋进去的那一截就被颈皮推着
    走（首版实测多陷 1.48，是 body 那档容许量的三倍）。埋进躯干可以，埋进颈不行。

    高度也要问，不能只问 z：颈根在肩这一段的 z 上确实存在，但它挂在**上半身**；拿它
    去卡胸底那几排，内侧面被推到比胸自己还宽的地方，整排反而飘在体外（垂缘实测六件）。
    """
    best = 0.0
    for e in fit.pelt_els:
        if not re.fullmatch(r"neck_\d+|neck_throat_\d+|mane_\w+", e["name"]):
            continue
        lo, hi = _aabb(e)
        if hi[2] < min(za, zb) or lo[2] > max(za, zb) or hi[1] < y0 or lo[1] > y1:
            continue
        best = max(best, float(max(hi[0], -lo[0])))
    return best


def _cells(fit: Fit, segs, lap: float) -> list[tuple[str, float, float]]:
    """骨段 → 更细的格。同骨内切格是为了**跟着桶身前后收**：一整块平板从肩铺到腰，
    两头必然翘起来离开马体。同骨的格之间留一个板厚的搭叠（不是严格对接）——严格
    对接时 `check_anim_chain` 量到的重叠恰好是 0，那条判据分不出"刚好接着"和"刚好裂开"。
    """
    cell = fit.P.L * BARD_CELL
    za, zb = segs[0][1], segs[-1][2]
    out = []
    for bone, z0, z1, p0, p1 in segs:
        n = max(1, int(math.ceil((z1 - z0) / cell)))
        for i in range(n):
            a, b = _lerpf(z0, z1, i / n), _lerpf(z0, z1, (i + 1) / n)
            # 整段的两头**钉死**在 [za,zb] 上。关节的交叠是往两边各探一截，而离两头最近
            # 的那道关节会把邻段的第一格推到整段之外——甲这一段本来是照"鞍在这个高度上
            # 没占的那几段 z"切出来的，探出去正好蹭上肚带（实测 0.27）。裁掉这一截不
            # 影响接缝：那道缝两侧的格仍旧各探各的，重叠一点没少。
            # 跨骨那道缝取 `max(pad, lap)`：`seam_pad` 给的是"这个关节转起来会张多大"，
            # 而同骨的格之间白给两个板厚。缝该留的不能比白给的还少——跨骨的缝本来就比
            # 同骨的险（矮马重铁甲实测裂了 0.26）。
            out.append((bone, max(a - (max(p0, lap) if i == 0 else lap), za),
                        min(b + (max(p1, lap) if i == n - 1 else 0.0), zb)))
    return out


def _lame_row(t: Tack, fit: Fit, spec: BardSpec, *, tag: str, row: str, za: float, zb: float,
              y0: float, y1: float, mat: str, glow: bool = False,
              step: float = 0.0, sag: float = 0.0) -> None:
    """一排札片：沿 z 切格，每格的横向按**这一格自己**的桶身量。

    每片的内侧面取该格桶身的**最窄**半宽（埋进马体里），外侧面取**最宽**再加一个板厚。
    埋进去那一截看不见，却一举解决两件事：竖向相邻的两排必然在 x 上重叠（不然桶身
    自上而下变宽，两排在 x 上差着一整个台阶，甲散成一条条不相连的箍）；同时"贴着马"
    这条判据可以卡到 0 容许——甲片与皮**实交**，不是"离得够近"。同肚带那一段的做法。
    """
    P, T = fit.P, fit.torso
    th, gap = P.u(spec.th), P.u(0.006)
    hw_out = max(T.band(z, y0, y1)[1] for z in _zs(za, zb))
    segs = fit.trunk.split(za, zb, y0, y1, hw_out + gap + th)
    # 同骨的格之间搭两个板厚：这一段是刚体，搭多少静止姿就是多少，不花代价；留窄了，
    # `check_anim_chain` 报出来的最小值全是这些静态缝，跨关节那几道真正会张开的反而
    # 被埋在数字底下看不见。
    cells = _cells(fit, segs, th * 2.0)
    zcs = [_zs(max(c0, T.z0), min(c1, T.z1), 3) for _b, c0, c1 in cells]
    # 每一格自己的体下缘。腹线自胸围向后上抬（收腹），一整排照同一个高度铺过去，到尻
    # 那儿整排都落在体外（实测：重铁甲的垂缘有十件飘着）。逐格托起来，甲的下摆自己就
    # 跟着腹线走——真甲的下摆本来也是这条线。
    # 取一格里**最高**的那个体下缘，不是最低的：按最低的算，格子后端（腹线已经抬上去
    # 了）整片挂在体外，只在最前端擦着皮一点点（交体积 0.014，够不上"贴住"的门槛）。
    want = [max(y0, max(T.at(z)[2] for z in zc) - sag) for zc in zcs]
    # 抬升要**逐格慢慢来**。各格照各自的下缘直接抬，尻前那一格能比邻格高出一整排——
    # 两格在 y 上就此错开，同一排里断成两截（矮马重铁甲实测裂 0.26，正好是那两格
    # y 上错开的距离）。限速之后仍然没有一格低于自己的体下缘：一格该抬多少会顺着
    # 相邻的格摊出去，摊成一道斜坡。
    cap = (y1 - y0) * 0.30
    a0s = [max(w - cap * abs(i - j) for j, w in enumerate(want)) for i in range(len(cells))]
    for i, (bone, c0, c1) in enumerate(cells):
        zc = zcs[i]
        a0 = a0s[i]
        a1 = max(y1, a0 + (y1 - y0) * 0.5)
        # 内侧面要**埋进去**一截，不能只做到相切：桶身在这一段常常是一个盒（半宽处处
        # 相同），相切时交体积恰好是 0，"甲贴在马身上"那条判据分不出相切与飘着。
        lo = max(min(T.band(z, a0, a1)[0] for z in zc) - th * BARD_BITE,
                 neck_outer_x(fit, c0, c1, a0, a1) + gap)
        hi = max(T.band(z, a0, a1)[1] for z in zc) + gap + th
        # 逐格错开一点厚度：一排里相邻两片是搭着的，同宽同高时外侧面近共面会闪。
        hi += th * 0.16 * (i % 2) + step
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            t.box(bone, f"{tag}_lame_{row}{i + 1}_{side}", (sgn * lo, a0, c0), (sgn * hi, a1, c1),
                  mat=mat, chain=(f"{tag}_row{row}_{side}", i))
        if glow:
            # 灵纹：贴着这一排的上棱走一道细条（甲片行间那道缝）。不支鳍，只鼓出一线。
            for sgn, side in ((-1.0, "l"), (1.0, "r")):
                t.box(bone, f"{tag}_glow_{row}{i + 1}_{side}",
                      (sgn * (hi - th * 0.15), a1 - th * 0.55, c0 + th * 0.5),
                      (sgn * (hi + th * 0.22), a1 - th * 0.05, c1 - th * 0.5),
                      mat="glow", glow=True)


def part_bard_body(t: Tack, fit: Fit, spec: BardSpec) -> None:
    """身甲（逐排绕开鞍）+ 当胸 + 搭后 + 系带。"""
    P, T = fit.P, fit.torso
    L = P.L
    th, gap = P.u(spec.th), P.u(0.006)
    # 与鞍之间的空隙。给宽了直接变成马身上的一条光带：肚带自己才 1.1 个单位宽，
    # 两边各留 0.87（首版）就把裸露拉到 2.9 个单位——比肚带本身还宽一倍半。
    m = P.u(0.014)
    zb0, zb1 = T.z_barrel0, T.z1

    # 竖向只定一次，按**桶身最深的那一段**（中段）。上缘两条一起卡：
    #   · `BARD_TOP` —— 再往上桶身收成背棱，平板贴不住；
    #   · 缰绕过鬐甲落到肩上 —— 顶上去就把那一段缰切了。
    zc = (zb0 + zb1) / 2
    _hw, ytop, ybot = T.at(zc)
    y_hi = min(ybot + (ytop - ybot) * BARD_TOP,
               tack_low_y(fit, "rein", zb0, zb1) - P.u(BARD_CLEAR))
    y_lo = ybot + (y_hi - ybot) * (1.0 - spec.hem)
    h = (y_hi - y_lo) / spec.rows

    def rows(tag: str, r: int, y0: float, y1: float, mat: str, glow: bool = False) -> None:
        """一排札片，按**这一排自己的高度**在鞍的缝隙里排开。

        两件事让它在远处读得出是"甲"而不是"一块板"：
          · 逐排换明暗（`mat` / `mat_dark`）。一个体素 6.25 cm，马的肋侧总共才八个
            单位高——排与排之间做多大的台阶都只有零点几个单位，**远处看不见**。真正
            读得出横排的是明暗，不是几何（首版只做台阶，整片糊成一块板）。
          · 上排压下排，逐排往外挪一点点。台阶远处看不见、近处看得见，搭叠方向也与真
            札甲一致（刀锋顺着往下滑）。
        """
        for u, (a, b) in enumerate(saddle_free(fit, y0, y1, zb0, zb1, m)):
            # 短过一格的不做。这不只是"太碎不好看"：矮马的镫按**骑手腿长**给的绝对
            # 落差（`STIRRUP_DROP`）几乎垂到地上，于是最低那几排在肚带与镫之间挤出
            # 一小段空隙——照做出来，那两片上下左右都挨不着别人，成了浮在肋上的一小块
            # 甲（连通性判据报的"实为 4 片"就是它）。
            if b - a < BARD_CELL * L:
                continue
            _lame_row(t, fit, spec, tag=tag, row=f"{r + 1}{u + 1}", za=a, zb=b, y0=y0, y1=y1,
                      mat=mat, glow=glow, step=th * 0.34 * r, sag=h * 0.30)

    for r in range(spec.rows):
        yb = y_lo + h * r
        rows("bard", r, yb - (h * BARD_LAP if r else 0.0), yb + h,
             spec.mat if r % 2 else spec.mat_dark,
             glow=spec.glow and r == spec.rows - 1)
    if spec.skirt:
        # 垂缘：身甲之下再垂一截。单独一个部件名——它是**多出来的一档**，不是把身甲
        # 拉长（分档核验按部件类型看"真的多了东西"，改高度它看不见）。
        rows("bard_skirt", 0, y_lo - P.u(spec.skirt), y_lo + h * BARD_LAP, spec.mat_dark)

    # ---------------- 当胸 ----------------
    # 当胸：横过胸前把两侧连起来。纵向压薄——厚了就把整对前肢兜在盒子里。
    cz0, cz1 = T.z0 - gap - th, T.z0 + P.u(0.055)
    _hwc, ytc, ybc = T.at(min(cz1, T.z1))
    cy0, cy1 = max(y_lo, ybc), min(y_hi, ytc)
    hw_c = max(T.band(z, cy0, cy1)[1] for z in _zs(T.z0, cz1, 3))
    t.box("thorax_front", "bard_chest", (-(hw_c + gap + th), cy0, cz0), (hw_c + gap + th, cy1, cz1),
          mat=spec.mat)
    if spec.peytral:
        # 当胸板：胸前再压一块整板，两侧包过肩前缘。二档起才有——布档只有一块布帘。
        t.box("thorax_front", "bard_peytral", (-(hw_c + gap + th * 2.0), _lerpf(cy0, cy1, 0.18),
                                               cz0 - th * 0.8),
              (hw_c + gap + th * 2.0, _lerpf(cy0, cy1, 0.86), cz0 + th * 1.6), mat=spec.mat_dark)
        if spec.glow:
            t.box("thorax_front", "bard_peytral_glow",
                  (-(hw_c + gap + th * 1.6), _lerpf(cy0, cy1, 0.46), cz0 - th * 1.05),
                  (hw_c + gap + th * 1.6, _lerpf(cy0, cy1, 0.58), cz0 - th * 0.55),
                  mat="glow", glow=True)

    # ---------------- 搭后 ----------------
    # 横过尻顶把两侧连起来。尾根（dock）在它下面穿出去，所以下缘压在尾根之上。
    dock = [e for e in fit.pelt_els if e["name"].startswith("dock_")]
    y_tail = max(float(_aabb(e)[1][1]) for e in dock) if dock else y_hi
    # 搭后自鞍后缘起，不是自腰荐关节起：中间那一段（鞍后到腰）在 `BARD_TOP` 之上是
    # 空的，札片够不到、搭后又不肯往前伸，背上就留一条光的。
    bz0 = saddle_span(fit)[1] + m
    _hwb, ytb, _ybb = T.at((bz0 + T.z1) / 2)
    by0 = max(y_hi - P.u(0.02), y_tail + gap)
    hw_b = max(T.band(z, by0, ytb)[1] for z in _zs(bz0, T.z1, 4))
    segs = fit.trunk.split(bz0, T.z1, by0, ytb, hw_b + gap + th)
    for i, (bone, c0, c1) in enumerate(_cells(fit, segs, th * 2.0)):
        zc = _zs(max(c0, T.z0), min(c1, T.z1), 3)
        top = max(T.at(z)[1] for z in zc)
        # 横宽按**背顶那一薄层**量，不按整条带里最宽的地方量。按最宽的算出来是一个从
        # 尻顶一直平铺到肋外的盖子，3/4 视角下像给马背上扣了一口箱子——背是圆的，
        # 搭后只该盖住脊那一片，两侧归札片。
        hi = max(T.band(z, max(by0, top - th * 4), top)[1] for z in zc) + gap + th
        t.box(bone, f"bard_croup_{i + 1}", (-hi, by0, c0), (hi, min(top + gap + th, ytb + th * 2), c1),
              mat=spec.mat, chain=("bard_croup", i))
    if spec.croup_plate:
        t.box("hips", "bard_croup_plate", (-(hw_b * 0.86), ytb - th * 0.5, _lerpf(bz0, T.z1, 0.16)),
              (hw_b * 0.86, ytb + th * 1.9, _lerpf(bz0, T.z1, 0.74)), mat=spec.mat_dark)
        if spec.glow:
            t.box("hips", "bard_croup_glow", (-(hw_b * 0.62), ytb + th * 1.55, _lerpf(bz0, T.z1, 0.24)),
                  (hw_b * 0.62, ytb + th * 2.05, _lerpf(bz0, T.z1, 0.66)), mat="glow", glow=True)
    if spec.spine:
        # 背脊梁：沿尻顶中线一道棱。只在鞍位之后——鞍位那一段归鞍。
        # z 上**不许内缩**：首版为了藏端面把两头各收进一个板厚，正好把搭叠吃干净
        # （同骨的搭叠本来就是一个板厚），一屈伸就从中间断开（rear 实测裂 0.74）。
        for i, (bone, c0, c1) in enumerate(_cells(fit, segs, th * 2.0)):
            zc = _zs(max(c0, T.z0), min(c1, T.z1), 3)
            top = max(T.at(z)[1] for z in zc) + gap + th
            t.box(bone, f"bard_spine_{i + 1}", (-th * 1.1, top - th * 0.4, c0),
                  (th * 1.1, top + th * 1.5, c1), mat=spec.mat_dark, chain=("bard_spine", i))

    # 系带：把甲捆在马身上的那几道，横在札片外面。位置也从鞍的缝隙里挑——挑最靠前与
    # 最靠后那两段，两条带正落在肚带前后，与真甲的系法一致。
    free = [(a, b) for a, b in saddle_free(fit, y_lo, y_hi, zb0, zb1, m) if b - a > BARD_CELL * L]
    for tag2, (a, b) in zip(("fore", "rear"), (free[0], free[-1]) if free else ()):
        zt = _lerpf(a, b, 0.62 if tag2 == "fore" else 0.30)
        hw_t = T.band(zt, y_lo, y_hi)[1] + gap + th * 2.4
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            t.box(fit.trunk.bone_at(zt), f"bard_tie_{tag2}_{side}",
                  (sgn * (hw_t - th * 1.4), y_lo, zt - th * 0.9), (sgn * hw_t, y_hi, zt + th * 0.9),
                  mat=spec.mat_trim)


def part_bard_crinet(t: Tack, fit: Fit, spec: BardSpec) -> None:
    """鸡颈：逐颈骨一段，盖住颈脊与上侧。下缘由 `crinet_floor` 从缰反推。"""
    P, NL = fit.P, fit.neck
    th, gap = P.u(spec.th), P.u(0.006)
    floor = crinet_floor(fit)
    if floor <= th * 1.6:
        raise SystemExit(f"缰把颈脊之下 {floor + P.u(BARD_CLEAR):.2f} 个单位全占了，鸡颈无处可放")
    seams = NL.seams
    for k in range(len(NECK)):
        bone = NECK[k]
        za, zb = seams[k][2], seams[k + 1][2]  # za 靠鬐甲（大），zb 靠头（小）
        ya, yb = crest_y(P, za), crest_y(P, zb)
        # **每一节都是一段斜着的带**，不是一个轴对齐的盒。颈脊是一条斜线，缰平行于它；
        # 拿轴对齐盒去套，一段之内颈脊起落半个多单位，盒的下缘只能取两端里低的那个，
        # 于是高的那一头整片切进缰里（首版就是这样，最深处吃掉 3.87 单位³）。换成
        # 与缰同一套 `_strap_world`，两条带从此严格平行，那个"离颈脊多深"的口径才成立。
        ym_a, ym_b = ya - floor * 0.5, yb - floor * 0.5
        x_out = NL.outer_span(za, ym_a, zb, ym_b, hair=False) + gap  # 盖住鬃，贴的是颈皮
        # 交叠按这一节自己的半径给：鸡颈骑在颈脊上，离转心比颈皮远。这道交叠**必然
        # 比一节颈还长**（颈一节约两个单位，而 1.35·5.6·tan14° 就要 1.9），试着卡短
        # 一半立刻裂 0.63。颈段短、鸡颈又骑得高，就是这个数。
        r = max(math.hypot(x_out + th, y - s[1]) for y, s in ((ya, seams[k]), (yb, seams[k + 1])))
        pad = seam_pad(r, NECK_ROM)
        ext = (0.0 if k == 0 else pad, 0.0 if k == len(NECK) - 1 else pad)  # 两个自由端不外扩
        # 逐节错开 4% 厚度：交叠这么长，同宽同高的相邻两节外侧面近共面会闪（同缰）
        d = th * (1.0 + 0.04 * (k % 2))
        _strap_world(t, bone, f"bard_crinet_cap_{k + 1}", -(x_out + d), x_out + d,
                     (ya + d * 0.1, za), (yb + d * 0.1, zb), d * 2.0,
                     mat=spec.mat, chain=("bard_crinet_cap", k), ext=ext)
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            _strap_world(t, bone, f"bard_crinet_lame_{k + 1}_{side}",
                         sgn * (x_out - d * 1.2), sgn * (x_out + d),
                         (ym_a, za), (ym_b, zb), floor,
                         mat=spec.mat, chain=(f"bard_crinet_{side}", k), ext=ext)
        if spec.glow:
            _strap_world(t, bone, f"bard_crinet_glow_{k + 1}", -(x_out * 0.42), x_out * 0.42,
                         (ya + d * 1.15, za), (yb + d * 1.15, zb), d * 0.4,
                         mat="glow", glow=True, ext=(ext[0] - d, ext[1] - d))


def build_bard(t: Tack, fit: Fit, spec: BardSpec) -> None:
    part_bard_body(t, fit, spec)
    if spec.crinet:
        part_bard_crinet(t, fit, spec)


def check_bard(t: Tack, fit: Fit, spec: BardSpec) -> list[str]:
    """甲的自检。除通用两条外，六条各挡一种在渲染图上看不出来的翻车：

      · **让开鞍位**：甲与**三档鞍的任何一件**都不许有交体积。两件马具是两份文件，
        各自的判据都看不见对方；合起来穿插只有摆到一起才看得见。
      · **不埋缰**：鸡颈与**三档缰**同理。这一条和上一条是本层仅有的跨装备判据。
      · **真的裹住了**：甲片必须与躯干皮实交（埋在体外 = 飘着），且外侧面要露在皮外
        （埋进皮里 = 等于没做）。反向的一对，只查一条必被另一条方向的错误绕过去。
      · **片数对**：整副甲是 2 片（前片 / 后片）或 3 片（多一条鸡颈）。散成四片五片
        意味着某处该搭上的没搭上——而缝多半被别的甲片挡着，静帧看不出来。
      · **给尾根让路**：搭后的下缘要在尾根之上，否则尾从甲里长出来。
      · 左右镜像。
    """
    P, T = fit.P, fit.torso
    els = tack_els(t)
    ok = {*BARD_TRUNK, *NECK}
    bad = check_common(t, fit, lambda b: b in ok)
    if not els:
        return bad

    # --- 跨装备：让开鞍、不埋缰 ---
    # 用**分离轴**量穿透深度，不用 AABB 交体积：鸡颈与缰都是沿颈脊斜着的带，两个斜盒的
    # AABB 大出真形状一大截，按 AABB 判会把"平行错开的两条带"报成撞在一起（首版就是
    # 这样：几何早已让开，判据还在报 5.18）。
    for kind, table in (("saddle", SADDLES), ("rein", REINS)):
        for tk in table:
            hit = [(e["name"], o["name"], v) for e in els for o in other_tack(fit, kind, tk)
                   if (v := _pen(e, o)) > CROSS_TOL]
            if hit:
                nm, on, v = max(hit, key=lambda h: h[2])
                bad.append(f"甲与「{table[tk].label}」撞了 {len(hit)} 处（最深 {nm} ↔ {on} "
                           f"陷进 {v:.2f} 单位）——两件一起穿上就是穿插")

    # --- 裹住了没有 ---
    torso_els = [e for e in fit.pelt_els if is_shape(e["name"])]
    lames = [e for e in els if "_lame_" in e["name"] or e["name"].startswith("bard_skirt")]
    if not lames:
        bad.append("没有甲片")
    for e in lames:
        if e["name"].startswith("bard_crinet"):
            continue
        if sum(_overlap_vol(e, pe) for pe in torso_els) < MIN_BITE:
            bad.append(f"{e['name']} 没贴在马身上（与躯干皮无实交）——会看着浮在体外")
    body = [e for e in els if not e["name"].startswith("bard_crinet")]
    show = max(max(float(_aabb(e)[1][0]), -float(_aabb(e)[0][0])) for e in body)
    hw_max = max(max(t2[0], -f[0]) for f, t2 in T.boxes)
    if show < hw_max + MIN_SHOW:
        bad.append(f"甲整个埋在皮里：最外 {show:.2f} 未超出躯干半宽 {hw_max:.2f}")

    # --- 尾根 ---
    dock = [e for e in fit.pelt_els if e["name"].startswith("dock_")]
    croup = [e for e in els if e["name"].startswith("bard_croup")]
    if dock and croup:
        v = max(_pen(c, d) for c in croup for d in dock)  # dock 是斜盒，同样得走分离轴
        if v > CROSS_TOL:
            bad.append(f"搭后压在尾根上（陷进 {v:.2f} 单位）——尾会从甲里长出来")

    want = 3 if spec.crinet else 2
    comps = connected_components(_Shim(els))
    if len(comps) != want:
        detail = " / ".join(f"{len(c)} 件({c[0]}…)" for c in comps[:4])
        bad.append(f"整副甲应是 {want} 片（肚带前一片 / 肚带后一片"
                   f"{' / 鸡颈' if spec.crinet else ''}），实为 {len(comps)} 片：{detail}")
    bad += check_mirror(els)
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
    # 这一类马具**额外**要放宽哪几档容许量（spec, Profile → {档名: 增量}）。None = 不放宽。
    # 只有一个用处：**容许量本身该随件的厚度走**的时候。前三种马具都是贴在皮上的薄件，
    # 一把尺够用；甲是有厚度的硬壳，两条判据都跟着板厚走：
    #   · `sink` —— 皮层动画是照**光马**摆的（侧躺那一帧，马体表面正好压在 y=0 上）。
    #     任何贴在皮外的硬壳躺下去必然比皮低一个壳厚。蹄铁 / 鞍 / 缰碰不到，只因为它们
    #     侧躺时都不在最下面。
    #   · `body` —— 一块厚 t 的板横在关节上，关节一弯，邻节的皮扫过来能把它整个盖住；
    #     但**最多也只能盖住它自己那么厚**，再多就是从另一面穿出去了。所以上界是板的
    #     整个截面，不是照实测凑的一个数（实测 1.45t，界是 1.8t + 空隙）。
    extra: object = None


# 对比度门槛为什么分种类：**颜色承担的辨识职责不一样**。
#   · 蹄铁是贴在蹄上的一条细带，不改变剪影——玩家能不能看出马蹄上有铁，全靠颜色，
#     所以门槛高（45）。
#   · 马鞍改变整只马的剪影（鞍桥、垂下的鞍翼、晃着的镫、绕过肚子的带），远处一眼
#     就知道"这马配了鞍"。颜色只需"不是同一块颜色"，不必抢眼，所以门槛低（32）。
# 三种毛色（锈骝 128,72,43 / 枯原 148,126,82 / 碎雪 146,140,131）连同各自的暗部
# 几乎铺满了中等明度的暖色带，一刀切 45 会把**所有**棕色皮革排除掉——那不是在保证
# 可辨识，是在替美术拍板。
#   · 缰介于两者之间（38）：笼头是脸上几条细带，但缰绳沿颈拉出一条明显的线，侧影
#     里认得出"这马上了嚼子"。剪影帮一半忙，颜色还得担另一半。
#   · 甲盖住整只马最大的一片面，剪影帮的忙最多——但它同时也是**面积最大的一片颜色**，
#     糊掉了整只马就读成"换了个毛色"而不是"披了甲"。所以门槛比鞍高一档（36）：不是
#     要它抢眼，是要它不冒充毛色。
KINDS: dict[str, Kind] = {
    "shoe": Kind("蹄铁", SHOES, build_shoes, check_shoe, ("hoof",), 45.0),
    "saddle": Kind("马鞍", SADDLES, build_saddle, check_saddle, ("coat", "coat_dark"), 32.0),
    "rein": Kind("缰", REINS, build_rein, check_rein, ("coat", "coat_dark"), 38.0),
    # 三档放宽全是**同一个量**推出来的：壳离皮多远（空隙 + 板厚）。
    #   · `sink` —— 侧躺时壳比皮先着地，就低这么多；
    #   · `body` —— 板横在关节上，邻节的皮扫过来最多把它整个盖住，也就是它自己的截面；
    #   · `limb` —— 2.40 那个基准量的是"贴着皮的带被腿扫到多深"（由肚带定出来的）。
    #     腿能扫到皮，就能扫进**立在皮外一个壳厚**的板里，再深这么多。挽马实测：肚带
    #     2.24、最薄的粗布甲 2.49，差的 0.25 正是两者离皮的距离差。
    #     试过改成"腿所在那几段 z 整段不做甲"（真甲在肩与股确实是挖开的）：腿皮是斜盒，
    #     世界 AABB 比真形状大出一大截，按它挖，挽马重铁甲从 208 件挖到只剩 66 件，
    #     整副甲基本没了。这条路要走得通得先有腿的**摆动包络**，不是现在这个量级的活。
    "bard": Kind("马甲", BARDS, build_bard, check_bard, ("coat", "coat_dark"), 36.0,
                 extra=lambda s, P: {
                     "sink": P.u(0.006) + P.u(s.th) * 1.16,
                     "body": P.u(s.th) * 1.8 + P.u(0.006),
                     "limb": P.u(0.006) + P.u(s.th) * 1.16,
                 }),
}


# 整套穿戴：哪一档甲配哪一档鞍 / 缰 / 蹄铁。只用来出**穿全了的预览图**，不是玩法配方。
# 跨装备判据只回答"没撞上"；穿全了之后是不是一匹说得过去的马，判据答不了，得自己看。
SUITS: dict[str, dict[str, str]] = {
    "cloth": {"saddle": "felt", "rein": "rope", "shoe": "cutie"},
    "mail": {"saddle": "leather", "rein": "leather", "shoe": "zagang"},
    "light": {"saddle": "leather", "rein": "leather", "shoe": "zagang"},
    "lingtie": {"saddle": "lingtie", "rein": "lingtie", "shoe": "lingtie"},
    "heavy": {"saddle": "leather", "rein": "leather", "shoe": "zagang"},
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
# 归到 strap 那一档的部件：**带子**。判据不看名字看性质——一条皮带在真马身上是被
# 顶开、被压扁、随着屈伸滑动的，刚体盒做不到，所以它对躯干的容许量本就比刚性主体松
# 一档（那个 0.85 是当初由肚带量出来的）。缰的各条带与肚带是同一回事：倒毙时颈蜷起来，
# 搭在颈上的缰段陷进邻节颈皮 0.79——同一个理由，同一档容许量，不必为它另开一个数。
# 衔铁与衔环不在此列：那是硬铁，陷进脸里就是穿帮。
STRAP_WORDS = ("girth", "billet", "buckle", "rein_line", "rein_cheek",
               "rein_throat", "rein_brow", "rein_nose", "rein_crown", "bard_tie")
# 必须**全程贴着马**的部件类型 → 容许离开马体多远（单位）。
#
# 陷入深度那条判据看不见反方向：马具浮起来离开马体，它只会报 0。而浮起来比陷进去
# 难看得多——玩家看到的是一副飘在背上方的鞍、一条挂在半空的缰。
#   · 鞍垫是鞍与马之间唯一的接触面，要求**实交**（0）。
#   · 缰的颈段是"搭在颈上"，但它刻意离皮一线避共面，所以容许量给到 `REIN_GAP`：
#     判据要的是"没飘走"，不是"焊死"。头段不列——它本来就该跨过嘴角到耳后那段
#     空当，要求它贴着脸是要求错了东西。
#   · 甲片刻意把内侧面埋进马体里（见 `_lame_row`），所以它可以卡到**实交**（0）——
#     甲飘起来离开马身是这一层最难看的翻车（一层壳浮在马外面跟着马跑），而它在静帧上
#     和"贴着"长得一模一样。既然造型上做得到实交，判据就不该松。
MUST_HUG = {"saddle_pad": 0.0, "rein_line_neck": 0.9,
            "bard_fore_lame": 0.0, "bard_rear_lame": 0.0, "bard_crinet_lame": 0.0}


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
      · **跳过造型层声明 `_hair` / `_loose` 的件**。`_hair`（鬃/额发/尾鬃）根本不是刚体
        表面——缰压进鬃里、低头时笼头扫过鬃，都是真马就会发生的事；`_loose`（耳/唇/
        颌线）各随各的骨飘。哪些件不参与刚体贴合判定只有画它的人知道，判据不猜。

    只比对**挂在别的骨上**的皮件：同骨的相对位置永不改变，算了也是零，白烧时间。
    """
    tack_by_bone = _by_bone(t.skel.data, lambda e: e.get("_tack"))
    pelt_by_bone = _by_bone(t.skel.data,
                            lambda e: e.get("_pelt") and not e.get("_loose") and not e.get("_hair"))
    empty = {k: (0.0, "") for k in FIT_TOL}
    if not pelt_by_bone:  # 纯马具文件（没带皮），这条无从查起
        return empty
    tack = [(tb, e["name"], np.array(_corners(e), float),
             "strap" if any(w in e["name"] for w in STRAP_WORDS) else "body")
            for tb, els in tack_by_bone.items() for e in els]
    pelt = [(pb, e["name"], np.array(_corners(e), float), *_obb(e),
             pb.startswith(LIMB_BONES)) for pb, els in pelt_by_bone.items() for e in els]

    # 逐帧不变的部分**先算好**。甲有一百多件、皮有一百五十多件，两层 Python 循环乘上
    # 三百多帧就是六百多万次——光循环开销就能把一份甲跑上几分钟，做不到边改边看。
    # 粗筛整个交给 numpy 一次广播出 (甲, 皮) 的布尔表，只对真正靠近的那几十对做 OBB。
    slacks = [MUST_HUG.get(comp_type(tn)) for _tb, tn, _A, _c in tack]
    want = np.array([s is not None for s in slacks])
    # 贴合那条的粗筛可以卡死（不重叠就是不重叠），"没飘走"这条不行：缰刻意离皮一线，
    # AABB 本来就不相交，卡死粗筛会把候选全滤掉，报出哨兵值。
    marg = np.array([(s + 1e-6) if s is not None else 0.0 for s in slacks])[:, None, None]
    # 接触判据要把**同骨**的皮件也算进来：鞍垫压着的躯干件多半就挂在 thorax_back 上，
    # 与鞍同骨——按"增量"那条的规矩跳过同骨，接触这边就一个候选都不剩，报出来是
    # −1e9 这种鬼数。同骨恰恰是最实的接触：相对位置永不改变，贴上了就永远贴着。
    same = np.array([[tb == pb for pb, *_r in pelt] for tb, *_r in tack])
    keep = ~(same & ~want[:, None])
    is_limb = np.array([lb for *_r, lb in pelt])

    hug: dict[str, tuple[float, str]] = {}  # 每个"必须贴着"的件，全部帧里最差的那一次

    def scan(W, acc: dict, tag: str) -> None:
        tw = [A @ W[tb][:3, :3].T + W[tb][:3, 3] for tb, _tn, A, _c in tack]
        pw = [C @ W[pb][:3, :3].T + W[pb][:3, 3] for pb, _pn, C, *_r in pelt]
        tlo = np.array([w.min(axis=0) for w in tw])
        thi = np.array([w.max(axis=0) for w in tw])
        plo = np.array([w.min(axis=0) for w in pw])
        phi = np.array([w.max(axis=0) for w in pw])
        near = keep & ((thi[:, None, :] + marg >= plo[None, :, :])
                       & (phi[None, :, :] + marg >= tlo[:, None, :])).all(axis=2)
        best = np.full(len(tack), -1e9)
        for i, j in np.argwhere(near):
            pbone, pn, _C, c, h, R, lb = pelt[j]
            Wp = W[pbone]
            q = np.abs(((tw[i] - Wp[:3, 3]) @ Wp[:3, :3] - c) @ R)
            d = float((h[None, :] - q).min(axis=1).max())
            if want[i] and not lb and d > best[i]:
                best[i] = d
            if same[i, j]:
                continue
            k = (tack[i][1], pn)
            if d > acc.get(k, (0.0, "", ""))[0]:
                acc[k] = (d, "limb" if lb else tack[i][3], tag)
        if tag != "rest":
            for i in np.flatnonzero(want):
                # 一个候选都没有 = 这一帧整片飘在体外。哨兵值不要直接漏进报告里
                # （−1e9 读起来像 bug 不像结论），压成一个能看的负数。
                tn = tack[i][1]
                v = max(float(best[i]), -9.99) + slacks[i]  # 归一到"离容许量还剩多少"
                if v < hug.get(tn, (1e9, ""))[0]:
                    hug[tn] = (v, f"{tn}@{tag}")

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
    if hug:
        out["hug"] = min(hug.values())
    return out


def _sat_gap(a: tuple, b: tuple) -> float:
    """两个有向包围盒的分离轴最小重叠量。>0 = 相交（数值即最浅的穿透），<0 = 分开了这么远。

    与 `connected_components` 用的是同一套分离轴（3+3+9），差别只在那边要一个 bool，
    这里要**量**——"还差多少就裂开"是唯一能拿来定阈值的数。
    """
    ca, ha, Ra = a
    cb, hb, Rb = b
    d = cb - ca
    axes = [Ra[:, i] for i in range(3)] + [Rb[:, i] for i in range(3)]
    for i in range(3):
        for j in range(3):
            cr = np.cross(Ra[:, i], Rb[:, j])
            n = float(np.linalg.norm(cr))
            if n > 1e-8:
                axes.append(cr / n)
    worst = 1e9
    for ax in axes:
        ra = sum(ha[i] * abs(float(ax @ Ra[:, i])) for i in range(3))
        rb = sum(hb[i] * abs(float(ax @ Rb[:, i])) for i in range(3))
        worst = min(worst, ra + rb - abs(float(ax @ d)))
        if worst < -1e9:
            break
    return worst


def check_anim_chain(t: Tack, pkey: str) -> tuple[float, str]:
    """逐帧核验：声明成同一条链的相邻两段必须**始终**相交。

    这是跨骨马具唯一躲不掉、也唯一看不见的一条。静止姿它们当然连着——交叠是照
    `seam_pad(r, rom)` 给的；只有关节真转起来，楔形张开超过交叠时才裂。而裂开的位置
    多半被鬃或颈皮挡着，静帧和渲染图都看不出来，跑起来才闪出一道缝。

    返回全场最小的重叠量：>0 = 一直连着（数值是最险的那一帧还剩多少），≤0 = 裂了。
    """
    chains: dict[str, dict[int, tuple[str, tuple]]] = {}
    for e in tack_els(t):
        ch = e.get("_chain")
        if not ch:
            continue
        chains.setdefault(ch[0], {})[ch[1]] = (_bone_of(t.skel.data, e["uuid"]), _obb(e))
    if not chains:
        return float("inf"), ""
    worst, who = 1e9, ""
    for name, segs in chains.items():
        idx = sorted(segs)
        if idx != list(range(len(idx))):
            return -9.99, f"{name} 的分段序号不连续：{idx}"
        for tag, W in [("静止", {b: np.eye(4) for b, _ in segs.values()})] + \
                      [(f"{a}t={b:.2f}", w) for a, b, w in bone_frames(pkey)]:
            for k in range(len(idx) - 1):
                (b0, (c0, h0, R0)), (b1, (c1, h1, R1)) = segs[idx[k]], segs[idx[k + 1]]
                g = _sat_gap((W[b0][:3, :3] @ c0 + W[b0][:3, 3], h0, W[b0][:3, :3] @ R0),
                             (W[b1][:3, :3] @ c1 + W[b1][:3, 3], h1, W[b1][:3, :3] @ R1))
                if g < worst:
                    worst, who = g, f"{name} 第 {k + 1}↔{k + 2} 段 @{tag}"
    return worst, who


# ================================================================ 装配 / CLI
def build(pkey: str, kind: str, tier: str) -> tuple[Tack, Fit]:
    pelt = FINAL / f"HorsePelt_{GEOM_COAT}_{pkey}.bbmodel"
    if not pelt.is_file():
        raise SystemExit(f"找不到皮层: {pelt}（先跑 gen_pelt.py）")
    skel = Skeleton(pelt)
    pelt_els = [dict(e) for e in skel.data["elements"] if e.get("_pelt")]
    P = PROFILES[pkey]
    fit = Fit(P=P, pelt_els=pelt_els, hooves=read_hooves(skel.data), torso=Torso(pelt_els),
              head=Head(P, pelt_els), neck=NeckLine(P, _by_bone(skel.data, lambda e: e.get("_pelt"))),
              trunk=Trunk(P, skel.pivot))
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


MIN_SHADE = 20.0  # 同一档内主色与暗部的最小距离


def _rgbd(a, b) -> float:
    return sum((x - y) ** 2 for x, y in zip(a, b)) ** 0.5


def check_contrast() -> list[str]:
    """每档马具的**每一种表面材质**都要和它压着的毛色拉得开——否则远处看不出穿没穿。

    不是审美挑剔，是这个仓库对装备的一贯要求（"玩家能从远处分辨对面在用 X 不是 Y"）。
    首版粗铁 (96,88,80) 与碎雪的蹄 (92,86,80) 相距 4.5，整只马的侧视里蹄铁完全消失，
    和赤脚一个样——而蹄部特写图上它清清楚楚。**特写成立不等于整只成立**，所以这条
    必须是断言，不能只靠渲一张图看看。

    范围要覆盖 `mat` / `mat_dark` / `mat_trim` 三个角色，不能只查主色：只查 `mat` 的那
    一版漏掉了破毡鞍**最大的一块面**（鞍垫走的是 `mat_dark`，离碎雪身色只有 12.4，在
    青毛马上整片消失）、粗革鞍的镫与扣（`mat_trim` 离枯原身色 22.4）、以及蹄铁的趾带
    与夹（`mat_dark` 离碎雪的蹄 23-35）。"主色"不等于"看得见的那一片"。
    灵纹（`glow`）不在此列：它是自发光的点缀，本来就不靠与毛色的反差被看见。

    另外查同一档内主色与暗部的距离：明暗层次要是也糊在一起，分档做的那点造型全白搭。

    RGB 欧氏距离只是个粗糙代理，够用：这里要挡的是"几乎同色"，不是排颜色的名次。
    """
    from gen_pelt import COATS

    bad = []
    for kind, K in KINDS.items():
        for tk, spec in K.table.items():
            for role in ("mat", "mat_dark", "mat_trim"):
                m = getattr(spec, role, None)
                if not m:
                    continue
                for coat in COATS.values():
                    for key in K.against:
                        d = _rgbd(TACK_MATS[m], coat.mats[key])
                        if d < K.min_contrast:
                            bad.append(f"{kind}/{tk} 的 {role}={m} 与「{coat.label}」的 {key} 只差 "
                                       f"{d:.1f}（下限 {K.min_contrast:.0f}），远处看不出穿没穿")
            d = _rgbd(TACK_MATS[spec.mat], TACK_MATS[spec.mat_dark])
            if 0.0 < d < MIN_SHADE:
                bad.append(f"{kind}/{tk} 的主色与暗部只差 {d:.1f}（下限 {MIN_SHADE:.0f}），明暗层次白做")
    return bad


def check_geom_same_across_coats(pkey: str) -> list[str]:
    """马具**读到的每一件**皮件在三种毛色里必须几何全等。

    马具只按一种毛色量尺寸（`GEOM_COAT`），这条塌了就是量错了马。范围由 `is_read`
    定——采样器认哪些件，这里就对拍哪些件，同一个定义。首版只查蹄件，鞍读的躯干件
    漂了照样查不出来；范围与采样器各写一份，迟早会漂开。
    毛色专属的花纹件（dorsal_stripe / star / face_mask）只在某一种毛色里存在，所以
    采样器一开始就不读它们；这条断言与那条排除是同一个约定的两面。
    """
    from gen_pelt import COATS

    ref: dict[str, tuple] | None = None
    bad = []
    for ck in sorted(COATS):
        f = FINAL / f"HorsePelt_{ck}_{pkey}.bbmodel"
        if not f.is_file():
            continue
        cur = {e["name"]: (tuple(e["from"]), tuple(e["to"]), tuple(e["rotation"]), tuple(e["origin"]))
               for e in json.loads(f.read_text())["elements"]
               if e.get("_pelt") and is_read(e["name"])}
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
    ap.add_argument("--suit", action="store_true",
                    help="整套穿戴（蹄铁+鞍+缰+甲同装一匹）落 stages/——跨装备判据只说"
                         "「没撞上」，穿全了好不好看得自己看")
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

    if args.suit:
        for pk in pkeys:
            for tier, mates in SUITS.items():
                t, fit = build(pk, "bard", tier)
                for kind, tk in mates.items():
                    KINDS[kind].build(t, fit, KINDS[kind].table[tk])
                name = f"HorseSuit_{tier}_{pk}_on_horse"
                t.skel.data["name"] = t.skel.data["model_identifier"] = name
                out = STAGES / f"{name}.bbmodel"
                out.parent.mkdir(parents=True, exist_ok=True)
                out.write_text(json.dumps(t.skel.data, ensure_ascii=False, indent=1))
                print(f"· {out.relative_to(FINAL.parents[1])}  件 {t.count}")
        return 0

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
                link = float("inf")
                fits = {k: (0.0, "") for k in FIT_TOL}
                if not args.skip_anim:
                    ex = K.extra(spec, PROFILES[pk]) if K.extra else {}
                    sink, who = check_anim_ground(t, pk)
                    stol = SINK_TOL + ex.get("sink", 0.0)
                    if sink > stol:
                        bad.append(f"动画里马具铲地 {sink:.2f} > {stol:.2f}（{who}）")
                    link, lwho = check_anim_chain(t, pk)
                    if link <= 0.0:
                        bad.append(f"跨骨的带在动画里裂开 {-link:.2f} 单位（{lwho}）——交叠不够，"
                                   f"接缝要么加交叠、要么挪到转轴上")
                    fits = check_anim_fit(t, pk)
                    for bucket, (d, w) in fits.items():
                        lim = FIT_TOL.get(bucket, 0.0) + ex.get(bucket, 0.0)
                        if bucket in FIT_TOL and d > lim:
                            bad.append(f"[{bucket}] 比静止姿多陷进皮 {d:.2f} > {lim:.2f}（{w}）")
                    if "hug" in fits and fits["hug"][0] <= 0.0:
                        bad.append(f"该贴着马的件在动画里飘开了（{fits['hug'][1]}，超出容许 "
                                   f"{-fits['hug'][0]:.2f}）——会看着浮在马身外")

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
                    f" 贴地 {sink:.2f} " + " ".join(f"{k}{fits[k][0]:+.2f}" for k in FIT_TOL)
                    + ("" if link == float("inf") else f" 链余 {link:.2f}"))
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
