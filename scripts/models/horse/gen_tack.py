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
from material import Finish, band_faces, check_finishes, paint, side_mean
from PIL import Image

TACK_DIR = FINAL / "tack"
TACK_ROW = 4  # 贴图第 5 行起（0-1 行骨/肌，2-3 行皮），追加不动已有 UV
GEOM_COAT = "rust"  # 几何三色同源，取一份当尺寸来源（另两份由 check_geom_same_across_coats 对拍）

Vec = tuple[float, float, float]


def _lerpf(a: float, b: float, s: float) -> float:
    return a + (b - a) * s

# ================================================================ 材质与做工
# 机器在 `material.py`（骨 / 肌 / 皮 / 马具共用一份，那里有完整说明）。这里只定马具
# 自己的那几种做工与色号。一句话：**材质不是一个 RGB，是「主色 + 做工」**——色相归
# 主色，"同一块面上有多少层次"归做工。金属和羊毛的全部区别就在后者。
FINISHES: dict[str, Finish] = {
    # 金属：跨度最大的一档。坑与划痕是它和"一块灰"的全部区别
    "metal": Finish(lit=1.34, occ=0.58, fres=0.44, grain="pit", amp=0.26),
    # 札甲：上暗下亮的叠边 + 两颗铆钉。跨度和金属同量级，但**只有三条结构线**，
    # 中间两行是干净的——甲面几个单位见方，撒噪就是一片棋盘
    "lamella": Finish(lit=1.30, occ=0.60, fres=0.42, grain="lamellar", amp=0.30),
    # 鱼鳞：比札片更碎更密的小圆片。行更密、每行更平——靠"行多"读出密，不靠每行反差大
    "scale": Finish(lit=1.28, occ=0.58, fres=0.46, grain="scale", amp=0.30),
    # 绗缝的衬里：棉行一道一道横着。布的跨度本来就小，靠结构不靠强度
    "quilt": Finish(lit=1.22, occ=0.66, fres=0.10, grain="quilt", amp=0.085),
    # 锁环：环挨环，亮的是环顶、暗的是环之间的洞。密排高频，远处并成一片发暗的冷灰
    "mail": Finish(lit=1.20, occ=0.62, fres=0.28, grain="ring", amp=0.24),
    # 布：跨度最小的一档。布要是也闪，就成了锡箔——这一条和"金属要有跨度"一样重要
    "cloth": Finish(lit=1.20, occ=0.68, fres=0.08, grain="weave", amp=0.075),
    # 挂着的布：同样是布的跨度，结构线**转了九十度**（竖折，见 `material.drape`）
    "drape": Finish(lit=1.20, occ=0.68, fres=0.08, grain="drape", amp=0.085),
    "rope": Finish(lit=1.16, occ=0.72, fres=0.10, grain="twist", amp=0.115),
    "leather": Finish(lit=1.15, occ=0.68, fres=0.14, grain="mottle", amp=0.10),
    # 灵纹：自发光条要的是**均匀**亮度，撒任何纹理都会读成脏
    "glow": Finish(lit=1.0, occ=1.0, fres=0.0, grain="flat", amp=0.0),
}


@dataclass(frozen=True)
class Mat:
    rgb: tuple[int, int, int]
    finish: str


# 材质表**只准追加不准插入**：UV 索引由这里的顺序派生，插一行会把已出的马具整体错位。
#
# 表里若干色是被 `check_contrast` **推**到现在这个位置的，不是重新挑的：判据只报"离某
# 种毛色太近"，改法一律取**离原色最近的合规色**——颜色是造型层拍过的，判据该把它推到
# 刚好过线，不该替美术重选一个。首轮判据只查主色（`mat`），把 mat_dark / mat_trim 漏了；
# 补上之后一次撞出五处：毡的暗部离碎雪身色 12.4（破毡鞍最大的一块面，在青毛马上整片
# 消失）、粗铁离枯原身色 22.4（粗革鞍的镫与扣）、麻绳离枯原身色 16.2（破毡鞍的肚带）、
# 粗铁暗部与杂钢暗部离碎雪的蹄 34.8 / 23.3（蹄铁的趾带与夹）。
#
# 这一轮**只动了几处明度**，色相基本没碰：三种毛色几乎铺满中等明度的暖色带，现有色相
# 是被这个事实逼出来的，重挑一遍只会绕回原处。"看着像羊毛"不是色相的错，是做工的错。
TACK_MATS: dict[str, Mat] = {
    # 粗铁走**锈色**而不是灰：灰的粗铁 (96,88,80) 和碎雪的蹄 (92,86,80) 只差 4.5，
    # 整只马渲出来和赤脚一模一样（见 check_contrast）。锈也更合"捡来的铁料"这档。
    "iron_crude": Mat((166, 92, 52), "metal"),
    "iron_crude_dark": Mat((118, 74, 40), "metal"),
    "iron_rust": Mat((110, 64, 34), "metal"),  # 锈斑 / 锈钉
    "steel": Mat((134, 142, 152), "metal"),  # 杂钢：冷灰，比粗铁亮两档
    "steel_dark": Mat((94, 106, 125), "metal"),
    # 灵铁压深、往蓝里推：青灰 (78,88,102) 离碎雪的蹄只有 26，同样糊在一起。
    "lingtie": Mat((48, 70, 122), "metal"),
    "lingtie_dark": Mat((34, 48, 88), "metal"),
    "glow": Mat((152, 216, 240), "glow"),  # 灵纹：淡蓝
    "nail": Mat((172, 170, 164), "metal"),  # 钉头：磨亮的白铁
    # --- 马鞍起追加（只准往后加：UV 索引由顺序派生）---
    # 毡往**浅**里推、革往**深**里推：三种毛色（锈骝 128,72,43 / 枯原 148,126,82 /
    # 碎雪 146,140,131）都落在中等明度的暖色带上，马具挤进这条带里就整片糊掉。
    # 一浅一深各自绕开——首版毡 (122,114,100) 离碎雪暗部只有 24.8、革 (118,82,52)
    # 离锈骝身色只有 16.8，正是"棕鞍配栗马"这个经典读不出来的组合。
    "felt": Mat((186, 180, 168), "cloth"),  # 旧毡：洗到发灰的白
    "felt_dark": Mat((122, 126, 108), "cloth"),
    "leather": Mat((46, 30, 24), "leather"),  # 粗革：鞣得不匀、油到发乌的深棕
    "leather_dark": Mat((26, 17, 14), "leather"),
    "rope": Mat((180, 164, 116), "rope"),  # 麻绳：新打的草黄
    # --- 缰绳起追加（只准往后加：UV 索引由顺序派生）---
    "rope_tar": Mat((52, 46, 36), "rope"),  # 上过桐油的绳结 / 磨损段：麻绳唯一能和三种毛色都拉开的暗部
    # --- 马甲起追加（只准往后加：UV 索引由顺序派生）---
    # 甲盖住整只马最大的一片面，配色比别的马具更难：三种毛色（连同暗部）几乎铺满了
    # **中等明度的暖色带**，而金属天然就落在中等明度的灰带上。碎雪的暗部 (104,99,92)
    # 正是一块中灰——任何"看着像铁"的中灰都会撞上它。所以五档一律往两头躲：
    # 布往蓝里、锁环往深冷里、板甲往亮里、重甲往黑里，中灰那一段整个让开。
    # 粗布：褪掉大半的蓝草染（末法唯一还染得起的颜色）。做工从 `weave`（横纬线）改成
    # `drape`（竖折）——它现在是一整幅**挂着**的障泥，不是绷在垫上的一块布：横排是札片
    # 压叠出来的东西，布身上没有。
    "cloth": Mat((92, 106, 132), "drape"),
    "cloth_dark": Mat((58, 68, 90), "drape"),  # 包边 / 齿边：染得最透的那一道
    # 下面五格（`mail` / `mail_dark` / `cloth_hemp` 原 `plate` / `plate_dark` / `plate_rim`）
    # 是历次改形制**空出来的格**：锁子甲改按诺曼参考重做之后环帘走 `mail_iron`，
    # 顶档的"整板"形制整个撤掉（改鳞甲）之后板与板缘也没人用了。
    # 贴图只有 32 格（第 4-7 行 × 8，`TACK_ROW` 之下就没地方了），所以空出来的格**就地
    # 改用途**，不往后追加——追加的第一格就会画到图外，而 PIL 报的是一句 IndexError，
    # 看不出是材质表满了（下面那条 assert 就是替它说话的）。就地改不动顺序，UV 不错位。
    "mail": Mat((66, 80, 90), "mail"),  # 【空格】原锁环色，锁子甲改走 mail_iron 后闲置
    "mail_dark": Mat((46, 56, 62), "mail"),  # 【空格】
    # 麻的本色：粗布甲那幅障泥的第二块布。往**冷**里推——暖白 (196,188,166) 离破毡鞍的
    # 毡 (186,180,168) 只有 12.8，而这两件正是配套穿的（`SUITS` 里粗布甲配破毡鞍），
    # 同一匹马上两大片同色的布等于障泥白做。推冷之后离得开 56。
    # 也不能走红：`cloth` 那条注释里写着的"末法唯一还染得起的颜色"是蓝草；何况锈骝的
    # 身色 (128,72,43) 本来就是一块暖红棕，正红在栗色马身上要么撞色要么消失。
    "cloth_hemp": Mat((200, 206, 216), "drape"),
    # 灵铁甲的甲面：**刻纹白钢**。参考图那一路的读法是"极浅的钢面 + 深色的刻线"，
    # 所以主色往亮里推到头，暗部借原来那块深蓝灵铁（`lingtie`）——反差全在明度上，
    # 刻线才读得出是刻进去的，不是另刷了一种铁。
    # 不与 `steel_polish` (178,184,194) 混：那是杂钢，中性灰；这块往蓝里偏，和灵纹
    # (152,216,240) 同族。两者本来也不会同时出现在一匹马上（分属二 / 五档与四档）。
    "lingtie_pale": Mat((186, 204, 226), "metal"),
    # 重甲压到近黑（原 56,58,64）：黑得越透，板缘那圈冷亮的棱越抢眼——参考图里
    # 那身甲的读法就是"近黑的面 + 一圈亮边"，面自己亮起来反而把边吃掉了。
    # 淬黑：整板那一档撤掉之后闲下来，现在给铁浮屠的鳞甲用（做工改走鳞纹）
    "iron_black": Mat((44, 46, 54), "scale"),
    "iron_black_dark": Mat((30, 31, 37), "scale"),
    # 板缘磨亮的那道棱：现实里是卷边与刃口露出的白铁。它是整板那一档**唯一**的细节，
    # 所以要亮得离淬黑面足够远（黑面 44 / 棱 150，四倍多），远处才连成一条线。
    # 灵铁甲下摆那圈**鳞**：比甲面深一档的青钢。顶档的鳞是淬黑的（`iron_black`），
    # 这一档的鳞要留在蓝里——两档都用鳞，靠明度与色相分家，不靠"谁有鳞"。
    "lingtie_scale": Mat((112, 140, 184), "scale"),
    # --- 重铁甲改按具装参考重做，追加（只准往后加：UV 索引由顺序派生）---
    # 参考图那身甲的配色是三样东西：**暖的深铁札片 + 绛红绗缝衬里 + 骨白的铆钉包边**。
    # 冷蓝那一版之所以读成"石头方块"，是因为整副甲只有一种冷灰——甲本来就不是一种
    # 材料做的，它是**铁 + 布 + 皮**缀在一起的东西，配色也该是三样。
    #
    # 铁为什么只能走**深**：三种毛色（锈骝 128,72,43 / 枯原 148,126,82 / 碎雪
    # 146,140,131，连同各自的暗部）几乎铺满了中等明度的暖灰带，而"看着像铁"的暖灰
    # 恰好就落在那儿——碎雪的暗部 (104,99,92) 本身就是一块中灰。试到 (132,122,108)
    # 离碎雪身色只剩 32.4，(120,110,98) 离它的暗部只剩 20.3。要**又暖又中明度**的铁，
    # 这匹马身上没有位置。所以压到 (78,70,60)：暖、深、离三种毛色最近也有 39.9。
    "bard_lame": Mat((78, 70, 60), "lamella"),
    "bard_lame_dark": Mat((56, 50, 44), "lamella"),
    # 绛红衬里：整副甲唯一的彩色，也是参考图第一眼读到的东西。
    # **只能往饱和里走**，不能往"旧了的砖红"里走：锈骝的身色 (128,72,43) 就是一块
    # 暖红棕，柔和的砖红离它只有二十几（试过 152,66,62 → 31.2），在一匹栗色马身上
    # 整条衬里消失。压深提纯之后离它 40 —— 也更像上过漆的绛，不是掉了色的粉。
    "bard_pad": Mat((146, 36, 40), "quilt"),
    # 骨白包边 / 铆钉条：札片之间那圈亮。它替下了整板那一档的板缘棱——札片太小，
    # 一片一道棱只剩杂色，而**沿着整条边走一道白**远处才连得成线。
    "bard_rivet": Mat((188, 176, 150), "metal"),
    # --- 锁子甲改按诺曼参考重做，追加（只准往后加：UV 索引由顺序派生）---
    # 打磨过的亮钢：面甲与当胸。整副锁子甲的读法就是**哑光暗环帘 + 两块亮板**的反差，
    # 所以这一色只做"亮"这一件事——离环帘 200 开外，远处一眼分得开。
    "steel_polish": Mat((178, 184, 194), "metal"),
    # 亮板的背光侧 / 眼窗框。原来给的 (120,126,138) 离碎雪身色只有 30.3 —— 它是**面甲
    # 上唯一的暗部**，糊进青毛马的身色等于眼窗没了框；压深往冷里推之后离得开 50。
    "steel_polish_dark": Mat((104, 112, 128), "metal"),
    # 环帘压深压中性：原来的 (66,80,90) 偏蓝，和灵铁那一档同族；诺曼那身环帘是发乌的
    # 铁色，靠**没有高光**跟旁边的亮板拉开，不靠色相。
    "mail_iron": Mat((60, 63, 70), "mail"),
    "mail_iron_dark": Mat((42, 44, 50), "mail"),
}

# 贴图第 4-7 行 × 每行 8 格 = 32 格，`TACK_ROW` 之下没有别的行了。满了之后 PIL 抛的是
# 一句 IndexError，看不出是材质表满了——这条替它说话，也提醒下一个人：**表满了要就地
# 改用途**（上面标了【空格】的几格），不要往后追加。
assert len(TACK_MATS) <= (8 - TACK_ROW) * 8, (
    f"马具材质 {len(TACK_MATS)} 种，贴图只放得下 {(8 - TACK_ROW) * 8} 格——"
    f"改用途标了【空格】的那几格，别往后追加")


def _seed(key: str) -> int:
    return list(TACK_MATS).index(key) + 1


def mat_rgb(key: str) -> tuple[int, int, int]:
    m = TACK_MATS[key]
    return side_mean(m.rgb, FINISHES[m.finish], _seed(key))


def _faces(mat: str, dims: tuple[float, float, float]) -> dict:
    """哪条带贴哪个面 —— 受光 / 背光带只给**窄边**，见 `material.band_faces`。

    "up 面是不是一条窄边"由盒子自己的尺寸答：札片薄、竖着立，顶上就是一条线；面帘的
    顶板、搭后的盖子是平躺的，顶面是它们最大的一片，刷成受光色就是一块白板。
    """
    i = _seed(mat) - 1
    return band_faces((i % 8) * SWATCH, (TACK_ROW + i // 8) * SWATCH,
                      edge=min(dims[0], dims[2]) < dims[1])


def extend_texture(data: dict) -> None:
    """追加马具色块。每格三条带 + 上下防渗行，见 `material.py`。"""
    src = data["textures"][0]["source"].split(",", 1)[1]
    img = Image.open(io.BytesIO(base64.b64decode(src))).convert("RGBA")
    px = img.load()
    for i, (key, m) in enumerate(TACK_MATS.items()):
        paint(px, (i % 8) * SWATCH, (TACK_ROW + i // 8) * SWATCH, m.rgb, FINISHES[m.finish], i + 1)
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
                "faces": _faces(mat, (t[0] - f[0], t[1] - f[1], t[2] - f[2])),
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
    r"eye_[lr]|eye_socket_[lr]|ear_[lr]",  # 面帘的眼窗与耳孔按实件开，不按"脸长的几成"猜
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

    def shell_or_none(self, z: float):
        """同 `shell`，越界返回 None。量**别的马具**占了脸上哪儿时会问到颅壳之外
        （衔铁在嘴角、缰垂到胸前），那不是错，跳过就是。"""
        got = list(self._cover(self.SHELL, z))
        if not got:
            return None
        return (max(max(hi[0], -lo[0]) for lo, hi in got),
                max(hi[1] for lo, hi in got), min(lo[1] for lo, hi in got))

    def under(self, z: float) -> float:
        """局部 z 处**含下颌件**的最低 y。鼻革与咽革要从下颌底下绕过去，只问颅壳会短半截。"""
        got = list(self._cover(self.UNDER, z))
        return min(lo[1] for lo, hi in got) if got else self.shell(z)[2]

    def point_local(self, p: Vec) -> Vec:
        """世界点 → 头局部（头长比例）。

        `local_of` 只对"没额外转过"的盒成立；量别的马具占了脸上哪儿时，件多半是转过的
        （颊带带 dp），整盒换算不了，只能逐个角点换算——点的变换与盒自己怎么转无关。
        """
        a = math.radians(HEAD_PITCH)
        c, s = math.cos(a), math.sin(a)
        H = self.P.H
        d = (p[0], p[1] - self.P.y_occiput, p[2] - self.P.z_occiput)
        return (d[0] / H, (c * d[1] + s * d[2]) / H, (-s * d[1] + c * d[2]) / H)

    def part_local(self, pat: re.Pattern) -> tuple[Vec, Vec] | None:
        """匹配到的皮件在头局部的合并 AABB（眼窗 / 耳孔照它开）。"""
        got = [b for n, b in self.local.items() if pat.fullmatch(n)]
        if not got:
            return None
        return (tuple(min(b[0][i] for b in got) for i in range(3)),
                tuple(max(b[1][i] for b in got) for i in range(3)))

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
#   · **递增的是"具装的件数"，不是同一件越做越大**。中原的马铠（具装）本就是一套
#     定数：面帘、鸡颈、当胸、马身甲、搭后 —— 铁浮屠那一路的"人马皆铠"说的就是这
#     五件配齐。所以分档不是"甲越来越长"，是**一件一件把这套配齐**（`BARD_KIT`）。
#     哪件先有哪件后有也不是随便排的：护胸先于护头，护头先于垂缘。
#   · **顶档换的是排数与衬里，不是把片做大**。五档一律札甲；顶档札片更薄更密（八排
#     对四排）、下摆吊一层绛红绗缝衬里、札行之间一道骨白铆钉包边。上一版把顶档做成
#     "整板 + 板缘亮棱"，想的是"换形制才叫换了一档"——方向错了：整板在马身上是一片
#     几个单位见方、什么细节都没有的黑，排数少反倒让每一排厚得像块牌子。
# 甲面上缘（× 该处躯干高）：再往上桶身收成背棱，平板贴不住，也会撞上颈。
# 早先这里写着"不逐档抬高，试过没用"——那是**另一个 bug 的影子**：当时缰是一条全局
# 天花板（整条身甲不许高过缰的最低点），鬐甲那一处把整只马的上缘一起钉死在 84.5%，
# 于是这个数给多大都不动。缰改成逐 z 压上缘（`over_ceiling`）之后它才真的管事。
BARD_TOP = 0.94
BARD_LAP = 0.45  # 札片竖向搭叠（× 一排的高）
# 一段甲短过这个就不做（× 体长）。**不跟着 `spec.cell` 走**：这条挡的是「上下左右都挨不着
# 别人的一小块甲」——矮马的镫按骑手腿长给绝对落差，几乎垂到地上，在肚带与镫之间挤出一小
# 段空隙——和甲片切多长没关系。跟着 cell 走的话片一做短那几块碎甲就又冒出来（锁子甲把
# cell 收到 0.068 之后，矮马实测整副甲散成四片）。
BARD_MIN_RUN = 0.085
BARD_CLEAR = 0.010  # 甲与缰之间留的空（× 鬐甲高）
BARD_BITE = 0.8  # 甲片内侧面埋进皮多深（× 板厚）：贴合判据要的是**实交**，不是相切
# 上缘被鞍压低之后，这一格还剩不到一排的几成就不做了。剩一条比板还薄的甲片没有意义，
# 而且它上下都挨不着别人，正是"整副甲散成四片五片"那条判据要挡的东西。
BARD_MIN_ROW = 0.45
# 盾泡在当胸板上的高度（0 = 下缘，1 = 上缘）。**不是随手放中间**：低头吃草时颈会
# 整段扫过胸前，扫到的正是当胸板的中下段——盾泡放在 0.5 那一版实测被颈多陷 1.52
# （上限 1.36）。0.76 之上是颈根本身、几乎不动的那一小段，才留得住一颗鼓出来的东西。
BOSS_Y = 0.76


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
    cell: float  # 一片多长（× 体长）。**形制在这里分家**：小 = 小片密缀的札甲，
    #              大 = 整板。同样的覆盖面，片大片小是两种甲
    peytral: bool  # 当胸板（布档只有一块布帘，没有硬板）
    croup_plate: bool  # 尻板
    crinet: bool  # 鸡颈
    chamfron: str  # 面帘："" 无 / "half" 半面帘（护额与鼻梁，眼露在外）/ "full" 全罩留眼窗
    skirt: float  # 垂缘再往下加一截（× 躯干高）；0 = 无
    spine: bool  # 背脊梁
    # 后襟：搭后往下兜住尻的**后面**（尾从中间穿出去）。侧片是贴在肋上的平板，只到
    # |x| 3.9 就没了，尻的后脸整片露在外面——正后方看就是一大片光屁股。
    rear: bool = False
    # 包边条的材质（"" = 不做）：沿甲的下摆走一道亮线。整板那一档原本每块板缘各一道
    # 棱，改回札甲之后不成立了——札片才两三个单位长，一片一道棱只剩一片杂色；**沿着
    # 整条边走一道**远处才连得成线。
    edge: str = ""
    pad: str = ""  # 衬里的材质（"" = 用 mat_dark）：垂缘那一截是布不是铁，参考图里是绛红的绗缝
    # 亮板的材质（"" = 跟着 mat / mat_dark）：面帘与当胸这两块**打磨过的整板**。
    # 锁子甲那一档全靠它读得出来——一身哑光的暗环帘上扣两块亮钢，那个反差就是它的样子；
    # 环帘和面甲要是同一种灰，整只马远看只剩一团灰。
    mat_plate: str = ""
    # 亮板的暗部（"" = 用 mat_dark）。**单独一个字段而不是在代码里推**：它是面甲上唯一
    # 的暗色（眼窗框、颊板），是远处看得见的一片，而配色判据只查规格表里声明的角色
    # ——在代码里现推出来的材质，判据一条都够不到（`mat_nail` 当年就是这么漏的）。
    mat_plate_dark: str = ""
    # 当胸中央那一颗圆盾泡（"" = 不做）。诺曼那一路的当胸正中总有一颗，是它最好认的记号
    boss: str = ""
    # 颈与头兜不兜到底。只盖颈脊的鸡颈把颈的一多半留在外面、只盖脸上半的面帘把腮与
    # 颌整片留在外面——顶档 36% 的缺口里，颈占四成、头下半占一成半
    full_wrap: bool = False
    # 面帘顶上竖起的一对缨（"" = 不做）。铁浮屠那张图里马额上就这么一对——整副甲
    # 一身近黑，**剪影上唯一能认出来的东西**就是它（同当年寄生要干的事，但这回长在
    # 头上、不改躯干轮廓，也就不会读成"屁股上插了根烟囱"）
    plume: str = ""
    # 逐排换明暗。首版靠它让"横排"读得出来（几何台阶太小，远处看不见）；札片纹理
    # 自己带上暗下亮的叠边之后就不必了，再叠一层反而读成斑马纹。
    banded: bool = True
    # 每一排的下缘都走一道 `edge` 的细线（默认只有最下面那一排走）。**只给浅色的甲面**：
    # 一道深线压在白钢上是"刻进去的"，压在深铁上是"另镶了一条"——轻铁甲那圈骨白铆钉
    # 每排都来一道就读成条纹衫，那是它自己那条注释里写着的教训，与这条不冲突（那是亮线
    # 压在暗面上，这是暗线压在亮面上）。
    edge_rows: bool = False
    glow: bool = False
    # 身甲改走**一整幅垂下来的布**（`_drape_body`），不排札片。只有布档用。
    drape: bool = False
    # 垂缘做几层。一层只是一条边，两层才读得出是**裙**：上层在外、短，下层在内、长，
    # 从外面看得见上层的下缘压在下层上，远处那道锯齿就是"层"。
    skirt_rows: int = 1
    # 垂缘下缘吊的杏叶（"" = 不吊）。中原那一路的甲最好认的一件装饰，也是**剪影**上
    # 的东西——同一档若只换材质与刻纹，远处和邻档还是一个轮廓。
    pendant: str = ""
    # 第二块布的颜色（"" = 整幅一色）。纹章障泥是**两块布拼起来**的：肚带之前一色、
    # 之后一色，接缝正落在肚带那道空档里。它是障泥远处第一眼读到的东西，所以和别的
    # 露在外面的角色一样要进配色判据（漏一个角色等于那条判据只看了甲的一半）。
    mat_field: str = ""


BARDS: dict[str, BardSpec] = {
    # 一档：**一整幅纹章障泥**罩下来，麻绳一捆。参考图那一路的读法是三件事：
    #   · **它是挂着的，不是贴着的**——下摆是一条水平线（重力定的），横向从桶身最宽处
    #     直垂下去，腹那儿是空的。别的四档全都反过来：逐段收着贴，下摆跟着腹线走。
    #   · **两块布拼起来**——肚带之前一块本色麻、之后一块褪蓝，接缝正落在肚带那道空档里。
    #   · **下摆剪成齿边**——隔一格垂一片，这是这类障泥远处唯一认得出的轮廓。
    # 它盖得比铁甲还长（垂到腹线），却仍旧是最低一档：一幅布挡枝刺挡夜寒，挡不住刃，
    # 而且具装一件都没有（当胸 / 搭后 / 鸡颈 / 面帘全无）。
    "cloth": BardSpec(
        key="cloth", label="粗布甲",
        blurb="一整幅麻布障泥罩下来，肚带前后拼本色与褪蓝两块，下摆剪成齿边。挡枝刺挡夜寒，不挡刃。",
        mat="cloth", mat_dark="cloth_dark", mat_trim="rope",
        mat_field="cloth_hemp", edge="cloth_dark", drape=True,
        th=0.011, hem=0.97, rows=6, cell=0.072, peytral=False, croup_plate=False,
        crinet=False, chamfron="", skirt=0.0, spine=False,
    ),
    # 二档：**一整幅锁环帘罩下来，外头扣两块打磨过的亮钢**（面甲 + 当胸，当胸正中一颗
    # 圆盾泡）。参考的是诺曼那一路：环帘垂感好、盖得大，可它挡切不挡砸，所以真正硬的
    # 地方只有那两块板——护住脸和胸口这两处一砸就完的部位，别处认了。
    #
    # 这一档的读法**全在反差**：一身哑光的暗环帘上两块亮的。所以环帘压深压中性
    # （原来的偏蓝，和灵铁那一档同族），亮板单独一色，两者离得越远越好。
    # 也因此它不像别档那样"一排排"：`banded=False` + 排间几乎不错开，远处是一整幅
    # 密麻的暗铁，不是横条。
    "mail": BardSpec(
        key="mail", label="锁子甲", blurb="一整幅锁环帘罩下来，面上扣一副打磨过的钢面甲与当胸，胸口一颗盾泡。挡得住切，挡不住砸。",
        mat="mail_iron", mat_dark="mail_iron_dark", mat_trim="leather",
        mat_plate="steel_polish", mat_plate_dark="steel_polish_dark",
        boss="steel_polish", banded=False,
        th=0.017, hem=0.86, rows=5, cell=0.068, peytral=True, croup_plate=False,
        crinet=True, chamfron="full", skirt=0.0, spine=False,
    ),
    # 三档：**中原札甲具装**——八排薄札片层层压叠、札行之间一道骨白铆钉、下摆吊一圈
    # 绛红绗缝衬里，具装配到脊梁。这一套原本是顶档的造型，整套挪下来给它。
    # 挪档不是降级：札甲本来就是这条路上最"够用"的一档——杂钢锻不出大片，短而密正是
    # 它的样子；顶档往上走的是**鳞**（更碎更密、也更贵），不是"同样的札片更厚"。
    "light": BardSpec(
        key="light", label="轻铁甲", blurb="八排薄札片层层压叠，札行之间一道骨白铆钉，下摆吊一圈绛红绗缝衬里，鸡颈连半面帘。轻，马跑得动。",
        mat="bard_lame", mat_dark="bard_lame_dark", mat_trim="bard_pad",
        edge="bard_rivet", pad="bard_pad", banded=False,
        th=0.021, hem=0.86, rows=8, cell=0.092, peytral=True, croup_plate=True,
        crinet=True, chamfron="half", skirt=0.075, spine=True,
    ),
    # 四档：灵铁。轻，所以同样的重量能锻**长片**、能一路护到颈和额——具装配到第三件。
    # 半面帘：只压额与鼻梁，眼露在外。灵铁贵，护到眼眶就够，再往下是重甲的事。
    #
    # 参考图那一路的读法是三件事，和顶档的"淬黑 + 白面甲"正好相反：
    #   · **刻纹白钢**——甲面浅到发白，刻线与暗部走深蓝，反差全在明度上，刻纹才读得出
    #     是刻进去的而不是另刷了一种铁；
    #   · **鳞裙**——下摆不是一条边，是两层压叠的青鳞（`skirt_rows=2`）；
    #   · **一排杏叶**——吊在裙的下缘，是它在**剪影**上唯一不同于轻铁甲的东西。
    # 原来这一档整片是饱和的深蓝，远看是一块塑料；蓝留给暗部与灵纹，面让给白钢。
    "lingtie": BardSpec(
        key="lingtie", label="灵铁甲",
        blurb="刻纹白钢长札片连鸡颈半面帘，下摆两层青鳞裙吊一排杏叶，甲片行间一道细灵纹。轻而不折真元。",
        mat="lingtie_pale", mat_dark="lingtie", mat_trim="lingtie_dark",
        mat_plate="lingtie_pale", mat_plate_dark="lingtie",
        pad="lingtie_scale", edge="lingtie_dark", pendant="lingtie_pale", skirt_rows=2,
        # 不逐排换明暗：白钢与深蓝差着一百五十个明度，一排一换就是**斑马纹**（实测六排
        # 三白三蓝）。横排交给每排下缘那道刻线（`edge_rows`）——这才是白钢板该有的样子。
        banded=False, edge_rows=True,
        th=0.024, hem=0.90, rows=6, cell=0.112, peytral=True, croup_plate=True,
        crinet=True, chamfron="half", skirt=0.070, spine=True, rear=True, glow=True,
    ),
    # 五档：**铁浮屠**。参考图那一路的读法是三件事：
    #   · **鳞**不是札——片更碎更密（十排），远处是一整只发乌的、有颗粒的黑；
    #   · **淬黑到底**——一身近黑，只有脸上那块白钢面甲跳出来；
    #   · **额上一对缨**——整副甲一身黑，剪影上唯一认得出的东西就是它。
    # 它比轻铁甲多的不是"更厚的札片"，是**换了形制**（鳞）、**淬黑**、**多两件**
    # （后襟、面缨）。轻铁甲那身绛红衬里在这儿撤掉：铁浮屠不留彩色，白面甲才是它的脸。
    "heavy": BardSpec(
        key="heavy", label="重铁甲", blurb="十排淬黑鱼鳞层层压叠，白钢面甲全罩只留眼窗，额上一对铁缨。人马皆铠，马也最累。",
        mat="iron_black", mat_dark="iron_black_dark", mat_trim="iron_crude",
        mat_plate="steel_polish", mat_plate_dark="steel_polish_dark",
        plume="iron_black", full_wrap=True, banded=False,
        th=0.024, hem=0.95, rows=10, cell=0.078, peytral=True, croup_plate=True,
        crinet=True, chamfron="full", skirt=0.105, spine=True, rear=True,
    ),
}

# 具装的件数：分档核验拿它比"这一档是不是真多配了一件"，而不是比甲有多厚。
# 顺序是马铠本来的顺序：当胸 → 搭后 → 鸡颈 → 面帘 → 垂缘 → 脊梁。
BARD_KIT: tuple[tuple[str, object], ...] = (
    ("当胸", lambda s: s.peytral),
    ("尻板", lambda s: s.croup_plate),
    ("鸡颈", lambda s: s.crinet),
    ("面帘", lambda s: bool(s.chamfron)),
    ("垂缘", lambda s: s.skirt > 0),
    ("脊梁", lambda s: s.spine),
    ("后襟", lambda s: s.rear),
    ("面缨", lambda s: bool(s.plume)),
)


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


def _abs_x(c: np.ndarray) -> tuple[float, float]:
    """一件的 |x| 覆盖到哪一段。跨中线的件覆盖到 0 —— 拿角点的 |x| 最小值当下界会把
    "从左肩连到右肩的一整块垫"读成"只占 |x|∈[1.08,1.08] 的一条线"。"""
    xs = c[:, 0]
    lo, hi = float(xs.min()), float(xs.max())
    return (0.0 if lo <= 0.0 <= hi else min(abs(lo), abs(hi))), max(abs(lo), abs(hi))


_SAD_BOX: dict[str, list[tuple[float, float, float, float, float]]] = {}


def over_boxes(fit: Fit) -> list[tuple[float, float, float, float, float]]:
    """压在甲面上头的别家马具（三档鞍 + 三档缰），每件压成
    (z起, z止, 最低 y, |x|起, |x|止) —— 甲的上缘只需要问这五个数。

    缰也在里头，是因为它和鞍压甲的方式**一模一样**：一条绕过鬐甲落到肩上的缰，甲顶上去
    就把它切了。首版拿它当一条全局的天花板（整条身甲的上缘一律不许高过缰的最低点），
    于是**鬐甲那一处**把整只马的上缘一起钉死在躯干高的 84.5%——背上一路留着一条光的。
    逐 z 问之后，缰只压住它真正经过的那一段。
    """
    key = fit.P.key
    if key not in _SAD_BOX:
        out = []
        for kind, table in (("saddle", SADDLES), ("rein", REINS)):
            for tk in table:
                for e in other_tack(fit, kind, tk):
                    c = np.array(_corners(e), float)
                    out.append((float(c[:, 2].min()), float(c[:, 2].max()),
                                float(c[:, 1].min()), *_abs_x(c)))
        _SAD_BOX[key] = out
    return _SAD_BOX[key]


def _in_shell(b, x_lo: float, x_hi: float) -> bool:
    """这件鞍具伸不伸进 [x_lo,x_hi] 这层壳（甲面所占的那一层）。

    **首版根本没问横向**，只按 y / z 重叠就把整段甲切掉。而鞍垫是压在**背上**的
    （|x| 只到 3.7），甲挂在**肋侧**（|x| 4.3 起）——两者在 x 上压根挨不着。为一件够
    不到的垫让路，等于在马身最显眼的那一片上凿了个七八单位宽的洞：顶上两排整段没了，
    远看就是"给马挂了两块牌子"，而不是"披了一副甲"。
    """
    return b[4] > x_lo and b[3] < x_hi


def over_ceiling(boxes, za: float, zb: float, x_lo: float, x_hi: float,
                 base: float, m: float) -> float:
    """甲面在 [za,zb] 这一段上缘最高能到哪（世界 y）—— 由伸进这层壳的鞍件 / 缰件压下来。

    真马铠的侧片本来就在鞍位与骑手腿的地方剜掉一块；剜的是**上缘**，不是整段。
    """
    y = base
    for b in boxes:
        if b[1] > za and b[0] < zb and _in_shell(b, x_lo, x_hi):
            y = min(y, b[2] - m)
    return y


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


# 四肢**上段**：肩 / 上臂 / 股 / 后腿上段。往下（膝、管、系、蹄）不管——那儿早已在甲
# 的下摆之外，真马铠也不护。
LIMB_UPPER_RE = re.compile(r"thigh_[lr]|upper_arm_[lr]|leg_upper2?_[fh]_[lr]|leg_joint_[fh]_[lr]")


def limb_outer_x(fit: Fit, za: float, zb: float, y0: float, y1: float) -> float:
    """四肢上段在这一小块（z ∈ [za,zb]，y ∈ [y0,y1]）里横向伸到多远。
    **甲片的外侧面不许比它更靠内。**

    与 `neck_outer_x` 是同一件事的另一头：那条管内侧面（甲不许埋进颈），这条管外侧面。

    甲的横向是照**躯干**量的（`Torso.band`），可上肢比躯干还宽——股伸到 |x| 5.05、
    后腿上段 5.29，而躯干最宽只有 4.51。照躯干给外侧面，大腿就直接从甲里长出来：
    挽马重铁甲实测露 3.13 单位，常马 1.86，五档三体型**无一幸免**（最小的粗布甲也露
    0.17）。这不是动画里蹭一下，是**静止姿就穿模**，而原有的判据一条都够不到它——
    `limb` 那档量的是"动起来比静止姿多陷多少"，静止姿本身有多糟它不问；贴合那条查的
    是甲有没有埋进**躯干**，大腿在不在甲外面它也不问。

    甲鼓出去比切开好：真具装的侧片本来就是**罩在**股外面垂下来的一大片。
    """
    best = 0.0
    for e in fit.pelt_els:
        if not LIMB_UPPER_RE.fullmatch(e["name"]):
            continue
        c = np.array(_corners(e), float)
        if (c[:, 2].max() < min(za, zb) or c[:, 2].min() > max(za, zb)
                or c[:, 1].max() < y0 or c[:, 1].min() > y1):
            continue
        best = max(best, float(np.abs(c[:, 0]).max()))
    return best


def _cells(fit: Fit, segs, lap: float, cell_len: float) -> list[tuple[str, float, float]]:
    """骨段 → 更细的格。同骨内切格是为了**跟着桶身前后收**：一整块平板从肩铺到腰，
    两头必然翘起来离开马体。同骨的格之间留一个板厚的搭叠（不是严格对接）——严格
    对接时 `check_anim_chain` 量到的重叠恰好是 0，那条判据分不出"刚好接着"和"刚好裂开"。
    """
    cell = fit.P.L * cell_len
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
              y0: float, y1: float, mat: str, glow: bool = False, edge: bool = False,
              step: float = 0.0, sag: float = 0.0, pend: str = "") -> None:
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
    cells = _cells(fit, segs, th * 2.0, spec.cell)
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
    # 链的分段：**被鞍剜掉一格，这一排就真的断成两条带**，不是一条带缺了一号。
    # `check_anim_chain` 按"分段序号连不连续"判有没有裂，直接拿格号当序号的话，剜掉的
    # 那一格会让整排报"裂开 9.99"——而那儿本来就不该有甲。断口两侧各自成链，跨骨那
    # 几道真会张开的缝才浮得上来。
    seg, idx = 0, 0
    for i, (bone, c0, c1) in enumerate(cells):
        zc = zcs[i]
        a0 = a0s[i]
        a1 = max(y1, a0 + (y1 - y0) * 0.5)
        # 上缘让鞍：鞍翼、镫革、镫本身够得到肋侧这层壳，但只挡住上半——把这一格的
        # 上缘压到它们之下（真马铠的侧片也正是在鞍位与骑手腿这两处剜掉一块上缘）。
        # 压完剩不下多少的那一格干脆不做：一条比板还薄的甲片没有意义。
        # 这层壳按**没让之前**的范围问，让完再收：先按压低后的范围问，壳会随之变窄、
        # 变窄之后有的鞍件又"够不到"了，压低量自己把自己抵消掉。
        def shell(ya: float, yb: float) -> tuple[float, float]:
            b = [T.band(z, ya, yb) for z in zc]
            return (min(v[0] for v in b) - th * BARD_BITE, max(v[1] for v in b) + gap + th)

        a1 = min(a1, over_ceiling(over_boxes(fit), c0, c1, *shell(a0, a1), a1, gap))
        if a1 - a0 < (y1 - y0) * BARD_MIN_ROW:
            if idx:
                seg, idx = seg + 1, 0
            continue
        # 内侧面要**埋进去**一截，不能只做到相切：桶身在这一段常常是一个盒（半宽处处
        # 相同），相切时交体积恰好是 0，"甲贴在马身上"那条判据分不出相切与飘着。
        lo, hi = shell(a0, a1)
        lo = max(lo, neck_outer_x(fit, c0, c1, a0, a1) + gap)
        # 外侧面要罩得住上肢：股与后腿上段比躯干还宽，照躯干给这一片就成了"大腿从甲里
        # 长出来"（见 `limb_outer_x`）。
        hi = max(hi, limb_outer_x(fit, c0, c1, a0, a1) + gap + th)
        # 逐格错开一线厚度：一排里相邻两片是搭着的，同宽同高时外侧面共面会闪。
        # 只要**打破共面**就够，给多了就是首版那种一格凸一格凹的锯齿（首版 0.16 个
        # 板厚、板厚又是现在的两倍，肋上肉眼可见地一格一格起伏）。
        hi += th * 0.05 * (i % 2) + step
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            t.box(bone, f"{tag}_lame_{row}{i + 1}_{side}", (sgn * lo, a0, c0), (sgn * hi, a1, c1),
                  mat=mat, chain=(f"{tag}_row{row}{seg}_{side}", idx))
        idx += 1
        if edge and spec.edge:
            # 包边：沿这一排的**下缘**走一道骨白的细线（札片压着的那条铆钉行）。
            # 只给最下面一排（下摆）与衬里的下摆——每一排都来一道，八排八条白线，
            # 整副甲读成一件条纹衫。
            for sgn, side in ((-1.0, "l"), (1.0, "r")):
                t.box(bone, f"{tag}_edge_{row}{i + 1}_{side}",
                      (sgn * (hi - th * 0.30), a0 - th * 0.15, c0 + th * 0.3),
                      (sgn * (hi + th * 0.22), a0 + th * 0.75, c1 - th * 0.3), mat=spec.edge)
        if pend and i % 2 == 0:
            # 杏叶：隔一格吊一片。垂多长是**侧躺那一帧**定的，不是好看定的：马倒下去
            # 时整幅裙横过来朝地，裙外的每一分都直接变成铲地深度（矮马实测吊 2.4 个板厚
            # 时铲地 0.82，容许 0.77）。1.5 个板厚差不多是一个体素，认得出是一片叶子，
            # 也还躺得下。
            # **在这儿做而不是另起一趟**——叶片必须搭上这一排的
            # 下缘才连得住（连通性判据按"整副甲几片"查，一排吊在半空的叶片会让它当场
            # 撞红），而这一排的下缘 `a0` 与外侧面 `hi` 只有这里手上有：另起一趟就得把
            # 同一套让鞍 / 让腿 / 逐格托腹线的算法再算一遍，两份算法迟早对不上。
            for sgn, side in ((-1.0, "l"), (1.0, "r")):
                t.box(bone, f"{tag}_pend_{row}{i + 1}_{side}",
                      (sgn * (hi - th * 1.5), a0 - th * 1.5, _lerpf(c0, c1, 0.34)),
                      (sgn * (hi + th * 0.3), a0 + th * 0.9, _lerpf(c0, c1, 0.66)), mat=pend)
        if glow:
            # 灵纹：贴着这一排的上棱走一道细条（甲片行间那道缝）。不支鳍，只鼓出一线。
            for sgn, side in ((-1.0, "l"), (1.0, "r")):
                t.box(bone, f"{tag}_glow_{row}{i + 1}_{side}",
                      (sgn * (hi - th * 0.15), a1 - th * 0.55, c0 + th * 0.5),
                      (sgn * (hi + th * 0.22), a1 - th * 0.05, c1 - th * 0.5),
                      mat="glow", glow=True)


def bard_span(fit: Fit, spec: BardSpec) -> tuple[float, float, float, float, float]:
    """身甲的上缘 / 下缘 / 排高 / 所占那层壳的内外边界 —— 造型与判据共用一份。

    竖向只定一次，按**桶身最深的那一段**（中段），只卡 `BARD_TOP` 一条（再往上桶身收成
    背棱，平板贴不住）。**别家马具不在这里卡**——鞍与缰都逐 z 压上缘（`over_ceiling`），
    拿它们的最低点当一条全局天花板，等于让鬐甲那一处把整只马的上缘一起钉死。
    """
    P, T = fit.P, fit.torso
    th, gap = P.u(spec.th), P.u(0.006)
    zb0, zb1 = T.z_barrel0, T.z1
    _hw, ytop, ybot = T.at((zb0 + zb1) / 2)
    y_hi = ybot + (ytop - ybot) * BARD_TOP
    y_lo = ybot + (y_hi - ybot) * (1.0 - spec.hem)
    # 甲面所占的那层壳（用最宽 / 最窄处估）——判"哪件鞍具够得到甲"用它，见 `_in_shell`
    return (y_lo, y_hi, (y_hi - y_lo) / spec.rows,
            min(T.band(z, y_lo, y_hi)[0] for z in _zs(zb0, zb1, 9)) - th * BARD_BITE,
            max(T.band(z, y_lo, y_hi)[1] for z in _zs(zb0, zb1, 9)) + gap + th)


def bard_runs(fit: Fit, spec: BardSpec) -> list[tuple[float, float]]:
    """身甲能连着走的那几段 z。

    **"切断"和"压低"是同一件事，只是程度不同**——这是首版最贵的一个错。首版分两条
    规则走：肚带那种一路绕到腹底的把甲整段切开，鞍翼镫革那种只把上缘压下去。可矮马的
    镫按**骑手腿长**给绝对落差，几乎垂到地上——它压得只差一点点没到腹底，于是"切断"
    那条规则不认，"压低"那条规则却把每一排都压没了。结果是肚带与镫之间挤出一小块上下
    左右都挨不着别人的甲（连通性判据报的"实为 4 片"就是它）。所以只留一条口径：
    **最下面那一排放不放得下**——放不下的地方甲就不存在，管它是被切的还是被压的。

    还要**扔掉中间那几段**：甲是挂在当胸（前）与搭后（后）上的，两头都不沾的一段没有
    挂的地方，做出来就是浮在肋上的一块牌子。
    """
    P, T = fit.P, fit.torso
    zb0, zb1 = T.z_barrel0, T.z1
    y_lo, _y_hi, h, x_lo, x_hi = bard_span(fit, spec)
    boxes = over_boxes(fit)
    gap = P.u(0.006)
    # 与鞍之间的空隙。给宽了直接变成马身上的一条光带：肚带自己才 1.1 个单位宽，
    # 两边各留 0.87（首版）就把裸露拉到 2.9 个单位——比肚带本身还宽一倍半。
    m = P.u(0.014)
    step = P.L * 0.012
    n = max(4, int(math.ceil((zb1 - zb0) / step)))
    runs: list[list[float]] = []
    for k in range(n):
        z0, z1 = _lerpf(zb0, zb1, k / n), _lerpf(zb0, zb1, (k + 1) / n)
        # 这一小段上，最下面那一排的下缘与上缘 —— 两条都照 `_lame_row` 的算法来。
        # 下缘跟着腹线走（腹线自胸围向后上抬），上缘也跟着抬（`max(y1, a0 + h/2)`）：
        # 只按 `y_lo + h` 当上缘的话，尻前那一段腹线一抬就"这一排放不下"，整个后半身
        # 的甲凭空消失。
        a0 = max(y_lo, T.at((z0 + z1) / 2)[2] - h * 0.30)
        a1 = max(y_lo + h, a0 + h * 0.5)
        if over_ceiling(boxes, z0 - m, z1 + m, x_lo, x_hi, a1, gap) - a0 < h * BARD_MIN_ROW:
            continue
        if runs and abs(runs[-1][1] - z0) < 1e-9:
            runs[-1][1] = z1
        else:
            runs.append([z0, z1])
    keep = [(a, b) for a, b in runs
            if b - a >= BARD_MIN_RUN * P.L and (a <= zb0 + 1e-6 or b >= zb1 - 1e-6)]
    if not keep:
        raise SystemExit(f"{spec.label}：鞍把整条肋侧占满了，身甲无处可挂")
    return keep


# 布甲：下摆之外再垂一截齿边（× 鬐甲高）。隔一格一片。
DRAPE_DAG = 0.058
# 齿片在 z 上比一格窄这么多（比例，两侧各让一半）。齿之间要真的看得见缝——让得太少
# 就不是齿边，只是把下摆放长了一截。
DRAPE_DAG_INSET = 0.34
# 下摆那道包边多高（× 鬐甲高）
DRAPE_BAND = 0.030
# 上缘的限速：允许它用**几格**爬完整段的落差。鞍件是逐件压下来的，逐格照各自的天花板
# 取，上缘就是一串台阶——侧视里像被撕过一道口子。限速只往下压、不顶破天花板，所以该让
# 的位置一格没少，只是让成一道斜坡。
#
# 为什么按"几格"而不按一个绝对斜率：**这条限速有个必须够到的地方**。搭后是布档仅有的
# 横跨中线的件（没有当胸板、没有脊梁、没有鸡颈），布的后半够不着它，左右两幅当场断成
# 两件——判据报"整副甲应是 2 片，实为 4 片"。而"够不够得着"逐体型不同：矮马的镫按骑手
# 腿长给绝对落差，压得最深，桶身却最短（后段只有五六格）——绝对斜率 0.030×体长 在常马
# 上刚好爬得回来（差 0.02 单位），到矮马就差了两个单位。按格数写，三个体型自动同一口径。
DRAPE_TOP_CELLS = 4.0
# 布往下每降一个单位，最多收窄这么多。桶身过了最宽的那一圈往下是收的，布收不了那么快
# ——**这个数就是"挂着"和"贴着"的全部区别**：给 0 是一幅从最宽处直筒垂下去的桶，
# 给到 1 以上就退回成一层贴着腹的壳。0.42 大约是布离腹一到两个单位。
DRAPE_LAG = 0.42


def _drape_body(t: Tack, fit: Fit, spec: BardSpec, slots, y_lo: float, y_hi: float,
                h: float) -> list[tuple[float, float, float]]:
    """身甲走**一整幅垂下来的布**（布档专用）。返回逐格的 (z起, z止, 上缘)。

    上缘要返回给搭后用：布的上缘是限速摊出来的一道斜坡，不是一条固定高度，搭后照名义
    高度起手就接不上（见 `part_bard_body` 里那一段）。

    和 `_lame_row` 是相反的两件东西，差别不在材质在**受力**：

      · 札片是**贴**上去的——逐格收着量桶身、下摆跟着腹线走（腹线自胸围向后上抬），
        一排一排横着压叠。
      · 布是**挂**着的——只在桶身最宽的那一圈搭住，往下一路垂空；下摆是重力定的一条
        水平线，折是竖的。

    所以这儿不复用 `_lame_row`：它逐排横切、逐排错开、下摆逐格托着腹线走，那三件事
    正是要去掉的。"挂着"和"贴着"在剪影上是两种东西。

    **竖切归竖切，横切照旧**：一幅布仍旧按高度分几层做（同一色、不错开、不留缝，远处
    读成一整幅）。首版真做成"一格一个通高的盒"，出来两头都不对：内侧面一路埋到桶身最窄
    处，一档布的用料 (2812.9) 反而超过整幅锁环帘 (2797.0)；改薄成一层皮之后它只在最宽的
    那一圈碰得着马，整幅布判成"浮在体外"，覆盖率从 43.4% 掉到 30.7%。分层之后每层各按
    **自己那一段**的桶身埋一层进去，两头都对。
    """
    P, T = fit.P, fit.torso
    th, gap = P.u(spec.th), P.u(0.006)
    boxes = over_boxes(fit)
    dag, band = P.u(DRAPE_DAG), P.u(DRAPE_BAND)
    n = max(2, spec.rows)
    out: list[tuple[float, float, float]] = []
    # 两块布的分界。**不按 `bard_runs` 切出来的那道空档分**：肚带在体长三成的地方，
    # 照它分出来是"一小块本色 + 一大片褪蓝"，两块布的分量差着三倍。按体长中点分，
    # 接缝落在后面那一幅里的某道格缝上——布本来就是一幅一幅拼的，格缝正是拼的地方。
    z_split = (T.z_barrel0 + T.z1) / 2

    def band_of(zc: list[float], ya: float, yb: float) -> tuple[float, float]:
        b = [T.band(z, ya, yb) for z in zc]
        return min(v[0] for v in b), max(v[1] for v in b)

    for _u, (za, zb) in enumerate(slots):
        hw_out = max(T.band(z, y_lo, y_hi)[1] for z in _zs(za, zb))
        segs = fit.trunk.split(za, zb, y_lo, y_hi, hw_out + gap + th)
        cells = _cells(fit, segs, th * 2.0, spec.cell)
        zcs = [_zs(max(c0, T.z0), min(c1, T.z1), 3) for _b, c0, c1 in cells]
        # 上缘先逐格照各自的天花板取，再限速摊成斜坡（`DRAPE_TOP_SLOPE`）。
        raw = []
        for (_b, c0, c1), zc in zip(cells, zcs):
            inn0, out0 = band_of(zc, y_lo, y_hi)
            raw.append(over_ceiling(boxes, c0, c1, inn0 - th * BARD_BITE, out0 + gap + th,
                                    y_hi, gap))
        slope = (max(raw) - min(raw)) / DRAPE_TOP_CELLS
        tops = [min(v + slope * abs(i - j) for j, v in enumerate(raw)) for i in range(len(raw))]
        # 层的上下界是**整段公用的一把梯子**，不逐格照自己的上缘等分。逐格等分那一版，
        # 上缘一格比一格高（那正是斜坡要的），层界也就跟着一格比一格高——相邻两格的
        # 同一层落在**互不相交**的两段高度上，判据在倒毙那一帧报"跨骨的带裂开 0.91"。
        # 公用梯子之后，两格的同一层必然在 y 上重叠；上缘的斜坡改由**顶上那一层被自己
        # 那一格的上缘削掉一截**来表达。
        ys = [_lerpf(y_lo, max(tops), k / n) for k in range(n + 1)]
        segk, idxk = [0] * n, [0] * n
        for i, (bone, c0, c1) in enumerate(cells):
            zc = zcs[i]
            a1 = tops[i]
            if a1 - y_lo < h * BARD_MIN_ROW:
                for k in range(n):
                    if idxk[k]:
                        segk[k], idxk[k] = segk[k] + 1, 0
                continue
            out.append((c0, c1, a1))
            mat = (spec.mat_field or spec.mat) if (c0 + c1) / 2 < z_split else spec.mat

            def band_at(ya: float, yb: float, _zc: list[float] = zc) -> tuple[float, float]:
                return band_of(_zc, ya, yb)

            # 这一格的**体下缘最低点**。下摆是一条水平线，而腹线自胸围向后上抬——尻前
            # 那一段，下摆整段落在马体之下，布在那儿是真的**吊着的**（这正是"挂着"的
            # 意思）。落在这条线之下的层一块皮都咬不着，得另起个名（见下）。
            floor = min(T.at(z)[2] for z in zc)
            # 逐层的外侧面：跟着桶身走，但**往下只许慢慢收**。桶身过了最宽的那一圈往下
            # 是收的，布收不了那么快——于是腹那儿布离着体垂下去，正是布挂在马身上的样子。
            # 自上而下递推，所以要倒着算。
            outs: list[float] = [0.0] * n
            for k in range(n - 1, -1, -1):
                w = band_at(ys[k], min(ys[k + 1], a1))[1]
                outs[k] = w if k == n - 1 else max(w, outs[k + 1] - DRAPE_LAG * (ys[k + 1] - ys[k]))
            hem_x = 0.0
            for k in range(n):
                # 层与层之间往上探一截：同色同宽、不错开，接缝在渲染里看不出来，但连通性
                # 判据要的是真交叠（贴着面不算）。探多少是**用料**定的：整幅布的高度是
                # 铁甲的一倍半，探 0.35 那一版六层叠出来的余料让一档布的用料 (6300.4)
                # 越过了整幅锁环帘 (6282.7)。
                y0 = ys[k] - (ys[k + 1] - ys[k]) * (0.20 if k else 0.0)
                y1 = min(ys[k + 1], a1)  # 顶上那一层被这一格自己的上缘削掉一截
                if y1 - ys[k] < (ys[k + 1] - ys[k]) * 0.15:
                    if idxk[k]:
                        segk[k], idxk[k] = segk[k] + 1, 0
                    continue
                hi = max(outs[k] + gap + th, limb_outer_x(fit, c0, c1, y0, y1) + gap + th)
                # 竖折：隔一格往外鼓一线。几何上只有零点几个单位（远处看不见），真正读出
                # 折的是 `drape` 那份**竖着的**纹理；这一线只负责打破共面，不然相邻两幅
                # 的外侧面共面会闪。折要**整幅通着**，所以按格错、不按层错。
                hi += th * (0.55 if i % 2 else 0.0)
                # 内侧面埋进这一层自己那一段的桶身；肩那一段的马体里还塞着颈根，埋进去
                # 的那一截会被颈皮推着走（见 `neck_outer_x`），所以还要让到颈之外——但
                # **至少留一层布的厚**，让过头就是一个退化盒。
                lo = min(max(band_at(y0, y1)[0] - th * BARD_BITE,
                             neck_outer_x(fit, c0, c1, y0, y1) + gap), hi - th * 1.2)
                # 够不着体下缘（或只擦着一线）的那几层叫 `bard_hem`，不叫 `bard_fold`：
                # **它本来就够不着马**，是吊在上一层下面的一截下摆。"甲片必须与躯干实交"
                # 那条判据（连同动画期的 `MUST_HUG`）对它是问错了问题——按名字分开，
                # 别的层照旧受查。只擦一线的也算：那点咬合体积够不上门槛，判据照样报。
                # 它并非无人管：连通性判据要求它与上一层真交叠（`0.20` 那道搭叠），飞不掉。
                nm = "bard_hem" if y1 <= floor + (ys[k + 1] - ys[k]) * 0.30 else "bard_fold"
                for sgn, side in ((-1.0, "l"), (1.0, "r")):
                    t.box(bone, f"{nm}_{_u + 1}{i + 1}_{k + 1}_{side}",
                          (sgn * lo, y0, c0), (sgn * hi, y1, c1),
                          mat=mat, chain=(f"bard_drape{_u}{segk[k]}c{k}_{side}", idxk[k]))
                idxk[k] += 1
                hem_x = hi if k == 0 else hem_x
            for sgn, side in ((-1.0, "l"), (1.0, "r")):
                # 包边：下摆滚一道深色的边。真物是为了不让毛边散开；在这儿它还担着另一
                # 件事——两块拼布的下摆由同一道边兜住，整幅才读成一件而不是两块。
                t.box(bone, f"bard_drape_band_{_u + 1}{i + 1}_{side}",
                      (sgn * (hem_x - th * 1.6), y_lo, c0),
                      (sgn * (hem_x + th * 0.2), y_lo + band, c1), mat=spec.edge or spec.mat_dark)
            if i % 2 == 0:
                # 齿边：隔一格垂一片。下摆一条平直的横线在剪影上就是"一块板"，剪成齿
                # 才读得出是布——这也是这一档在**远处**唯一认得出的轮廓（它没有具装，
                # 剪影上没有别的东西可认）。
                ins = (c1 - c0) * DRAPE_DAG_INSET * 0.5
                for sgn, side in ((-1.0, "l"), (1.0, "r")):
                    t.box(bone, f"bard_dag_{_u + 1}{i + 1}_{side}",
                          (sgn * (hem_x - th * 1.6), y_lo - dag, c0 + ins),
                          (sgn * (hem_x + th * 0.2), y_lo + band * 0.6, c1 - ins),
                          mat=spec.edge or spec.mat_dark)
    return out


def part_bard_body(t: Tack, fit: Fit, spec: BardSpec) -> None:
    """身甲（逐排绕开鞍）+ 当胸 + 搭后 + 系带。"""
    P, T = fit.P, fit.torso
    th, gap = P.u(spec.th), P.u(0.006)
    m = P.u(0.014)
    zb0, zb1 = T.z_barrel0, T.z1
    y_lo, y_hi, h, _xl, _xh = bard_span(fit, spec)
    slots = bard_runs(fit, spec)

    def rows(tag: str, r: int, y0: float, y1: float, mat: str,
             glow: bool = False, edge: bool = False, pend: str = "") -> None:
        """一排札片。整条通着走，只在**肚带**那儿断开；上缘随鞍起伏（`_lame_row`）。

        两件事让它在远处读得出是"甲"而不是"一块板"：
          · **札片自己的纹理**带上暗下亮的叠边（`material.py` 的 `lamellar`）。一个体素
            6.25 cm，马的肋侧总共才八个单位高——排与排之间做多大的台阶都只有零点几个
            单位，**远处看不见**。首版靠逐排换明暗来补，可那是两种颜色的甲片，排数一多
            就读成斑马纹；纹理担起这件事之后（`banded=False`）整副甲才是一种铁。
          · 上排压下排，逐排往外挪一点点——搭叠方向与真札甲一致（刀锋顺着往下滑）。
            挪的量要**小**：首版一排挪 0.34 个板厚、板厚又是现在的两倍，三排下来肋上
            是三级台阶。
        """
        for u, (a, b) in enumerate(slots):
            _lame_row(t, fit, spec, tag=tag, row=f"{r + 1}{u + 1}", za=a, zb=b, y0=y0, y1=y1,
                      mat=mat, glow=glow, edge=edge, pend=pend, step=th * 0.18 * r, sag=h * 0.30)

    drape_tops = _drape_body(t, fit, spec, slots, y_lo, y_hi, h) if spec.drape else []
    for r in range(spec.rows) if not spec.drape else ():
        yb = y_lo + h * r
        rows("bard", r, yb - (h * BARD_LAP if r else 0.0), yb + h,
             (spec.mat if r % 2 else spec.mat_dark) if spec.banded else spec.mat,
             glow=spec.glow and r == spec.rows - 1,
             edge=spec.edge_rows or (r == 0 and not spec.skirt))
    if spec.skirt:
        # 垂缘：身甲之下再垂一截。单独一个部件名——它是**多出来的一档**，不是把身甲
        # 拉长（分档核验按部件类型看"真的多了东西"，改高度它看不见）。
        # 顶档的垂缘是**布**不是铁（`spec.pad`）：参考图里札片下面吊着的是一圈绛红的
        # 绗缝衬里。整副甲全是铁的时候，剪影上就是一整块黑；下摆换成布，甲的重量感与
        # 布的垂感各归各的，远处也才有一处彩色能认。
        #
        # `skirt_rows > 1` 做**层层压叠**的裙：上层短、在外，下层长、在内，从外面看得见
        # 上层的下缘压在下层上。一层只是一条边，两层才有"裙"。层号倒着传给 `rows`——
        # 它按层号往外挪（`step`），上层要在外面，所以上层的号大。
        nsk = max(1, spec.skirt_rows)
        drop = P.u(spec.skirt)
        for k in range(nsk):
            rows("bard_skirt" if nsk == 1 else f"bard_skirt{k + 1}", nsk - 1 - k,
                 y_lo - drop * (k + 1) / nsk, y_lo + h * BARD_LAP - drop * (k / nsk) * 0.55,
                 spec.pad or spec.mat_dark, edge=(k == nsk - 1),
                 pend=spec.pendant if k == nsk - 1 else "")

    # ---------------- 当胸 ----------------
    # 当胸：横过胸前把两侧连起来。纵向压薄——厚了就把整对前肢兜在盒子里。
    cz0, cz1 = T.z0 - gap - th, T.z0 + P.u(0.055)
    _hwc, ytc, ybc = T.at(min(cz1, T.z1))
    cy0, cy1 = max(y_lo, ybc), min(y_hi, ytc)
    hw_c = max(T.band(z, cy0, cy1)[1] for z in _zs(T.z0, cz1, 3))
    # 当胸也得让缰：胸前正是笼头那几条带落下来的地方（上缘不再有全局天花板之后，
    # 这一块是唯一还按 `y_hi` 一路顶到顶的件）。
    cy1 = min(cy1, over_ceiling(over_boxes(fit), cz0, cz1, 0.0, hw_c + gap + th * 2.0, cy1, gap))
    # 当胸的布跟着**前**那一块走（分两色的档才有分别）：它和肚带前那一幅是同一块布
    # 绕过胸前接上的，两者不同色的话正视图里胸前平白多一道横断。
    t.box("thorax_front", "bard_chest", (-(hw_c + gap + th), cy0, cz0), (hw_c + gap + th, cy1, cz1),
          mat=spec.mat_field or spec.mat)
    if spec.peytral:
        # 当胸板：胸前再压一块整板，两侧包过肩前缘。二档起才有——布档只有一块布帘。
        # 横向分三阶，越靠外越往后收：胸是圆的，一块通宽的平板正视图里是块门板，
        # 3/4 视角下更明显（同搭后那口箱子的道理）。中间那阶跨中线，把左右连起来。
        py0, py1 = _lerpf(cy0, cy1, 0.18), _lerpf(cy0, cy1, 0.86)
        pw = hw_c + gap + th * 2.0
        prev_x = 0.0
        for k, (f, back) in enumerate(((0.52, 0.0), (0.80, 0.9), (1.0, 2.1))):
            x1 = pw * f
            for sgn, side in ((-1.0, "l"), (1.0, "r")) if k else ((1.0, ""),):
                t.box("thorax_front", f"bard_peytral_{k + 1}{side and '_' + side}",
                      (sgn * (prev_x if k else -x1), py0, cz0 - th * 0.8 + th * back),
                      (sgn * x1, py1, cz0 + th * 1.6), mat=spec.mat_plate or spec.mat_dark)
            prev_x = x1 - th * 0.6
        if spec.boss:
            # 盾泡：当胸正中鼓出来的一颗。诺曼那一路最好认的记号，也是**整块板上唯一
            # 的形状**——一块通宽的亮板正视图里就是个长方形，鼓一颗才有"这是件甲"的
            # 意思。三层收出个圆头：一个体素 6.25 cm，再多层也读不出更圆。
            bz, pc, prev = cz0 - th * 0.8, _lerpf(py0, py1, BOSS_Y), 0.0
            for i, (w, d) in enumerate(((0.30, 0.6), (0.20, 1.0), (0.11, 1.25))):
                hwb = hw_c * w
                # 每一层都要压住**上一层**（第一层压住当胸板本身），不然一颗浮在胸前的
                # 泡：连通性判据会把整副甲多报一片，静帧里却看不出来
                t.box("thorax_front", f"bard_boss_{i + 1}",
                      (-hwb, pc - hwb, bz - th * d), (hwb, pc + hwb, bz - th * (prev - 0.4)),
                      mat=spec.boss, chain=("bard_boss", i))
                prev = d
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
    near = [a for c0, c1, a in drape_tops if c1 > bz0 and c0 < T.z1]
    if near:
        # 布档的身甲上缘是限速摊出来的一道斜坡（`DRAPE_TOP_SLOPE`），尻那一段不一定
        # 爬得回 `y_hi`：矮马的镫垂得最低、桶身又最短，实测只爬到 16.50，而搭后照名义
        # 高度起手在 18.60——中间空着两个单位，左右两幅身甲就此失去唯一的联络（搭后是
        # 布档**仅有的**横跨中线的件），判据报"整副甲应是 2 片，实为 4 片"。
        # 所以布档的搭后**下缘跟着身甲的上缘走**：接到最高的那一格上（`max`，接住一格
        # 就够；照 `min` 会被鞍后第一格拖到很低，白白多出一大块）。
        by0 = max(min(by0, max(near) - th * 1.5), y_tail + gap)
    hw_b = max(T.band(z, by0, ytb)[1] for z in _zs(bz0, T.z1, 4))
    segs = fit.trunk.split(bz0, T.z1, by0, ytb, hw_b + gap + th)
    def back_top(z: float, x: float) -> float:
        """背在横向某处的高度。查询点先钳进该 z 的皮内 —— 甲比皮宽出一个板厚，
        照甲自己的 x 去问必然问到皮外。"""
        hw = T.at(z)[0]
        return T.top_at(z, min(max(x, -hw + 1e-3), hw - 1e-3))

    # 横向分两阶，各按**自己那一条 x 上的背高**封顶。一个盒从中线平铺到肋外，顶面是
    # 一整片水平的板——尻是圆的，那片板只在中线上贴着，两侧凌空，3/4 视角下就是"马
    # 屁股上扣了一口箱子"。分阶之后剪影跟着尻圆下来（同 `gen_pelt` 的背）。
    croup = []
    for i, (bone, c0, c1) in enumerate(_cells(fit, segs, th * 2.0, spec.cell)):
        zc = _zs(max(c0, T.z0), min(c1, T.z1), 3)
        top = max(T.at(z)[1] for z in zc)
        # 横宽按**背顶那一薄层**量，不按整条带里最宽的地方量。按最宽的算出来是一个从
        # 尻顶一直平铺到肋外的盖子，3/4 视角下像给马背上扣了一口箱子——背是圆的，
        # 搭后只该盖住脊那一片，两侧归札片。
        hi = max(T.band(z, max(by0, top - th * 4), top)[1] for z in zc) + gap + th
        cell, prev = [], 0.0
        for f in (0.55, 1.0):
            x1 = hi * f
            ytk = max(back_top(z, x) for z in zc for x in (prev, (prev + x1) / 2, x1))
            cell.append((prev, x1, min(ytk + gap + th, ytb + th * 2)))
            prev = x1 - th * 0.5
        croup.append((bone, c0, c1, cell))
    # 一阶要么**每一格都做**，要么整阶不做。外侧那一阶又矮又靠外：背在那儿已经圆下去，
    # 封顶可能低到 `by0` 之下（顶档上缘抬到躯干高的 94% 之后尤其明显）——那样的格
    # 做出来是一条比板还薄的片，逐格有无还会让链的分段序号断掉，判据报"整排裂开"。
    for k in range(len(croup[0][3])):
        if min(c[k][2] - by0 for *_r, c in croup) < th * 1.2:
            continue
        for i, (bone, c0, c1, cell) in enumerate(croup):
            x0, x1, ytk = cell[k]
            for sgn, side in ((-1.0, "l"), (1.0, "r")) if k else ((1.0, ""),):
                t.box(bone, f"bard_croup_{i + 1}_{k + 1}{side and '_' + side}",
                      (sgn * (x0 if k else -x1), by0, c0), (sgn * x1, ytk, c1),
                      mat=spec.mat, chain=(f"bard_croup{k}{side}", i))
    if spec.rear:
        # 后襟：兜住尻的**后面**，尾从中间穿出去。
        #
        # 侧片是贴在肋上的平板，内侧面到 |x| 3.9 就没了；尻的后脸（croup_cap 那一块，
        # 半宽 4.4、高十个单位）整片没人管——正后方看就是一大片光屁股，甲只在两侧
        # 各挂了一条。真具装的搭后本来就是**一片兜下来**的，尾从留的洞里出来。
        # 左右各一片、中间让给尾：尾根连尾毛在这一段伸到 |x| 3.2，中间那一条本来也
        # 被尾遮着，做了也看不见，反倒要跟尾毛打架。
        zr0, zr1 = T.z1 - th * 1.6, T.z1 + gap + th
        tail = [e for e in fit.pelt_els if e["name"].startswith(("dock_", "tailhair_"))]
        x_in = max((float(np.abs(np.array(_corners(e), float)[:, 0]).max()) for e in tail
                    if np.array(_corners(e), float)[:, 2].max() > zr0), default=0.0) + gap
        yr0 = max(T.at(T.z1 - th)[2], y_lo)
        x_out = max(T.band(T.z1 - th, yr0, by0)[1],
                    limb_outer_x(fit, zr0, zr1, yr0, by0)) + gap + th
        if x_out - x_in > th * 2:
            for sgn, side in ((-1.0, "l"), (1.0, "r")):
                t.box("hips", f"bard_croup_rear_{side}", (sgn * x_in, yr0, zr0),
                      (sgn * x_out, by0 + th, zr1), mat=spec.mat)
                t.box("hips", f"bard_croup_rear_edge_{side}",
                      (sgn * x_in, yr0 - th * 0.15, zr1 - th * 0.5),
                      (sgn * x_out, yr0 + th * 0.75, zr1 + th * 0.22),
                      mat=spec.edge or spec.mat_dark)
            # 尾洞的上沿：尾根顶到搭后下缘之间还剩一条，正后方看是尾巴上头一条光的。
            # 尾从洞里出来，洞的上边总得有个边——真物那儿正是搭后包过来的那一道。
            if by0 + th - (y_tail + gap) > th:
                t.box("hips", "bard_croup_rear_top", (-x_in - th * 0.5, y_tail + gap, zr0),
                      (x_in + th * 0.5, by0 + th, zr1), mat=spec.mat)
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
        for i, (bone, c0, c1) in enumerate(_cells(fit, segs, th * 2.0, spec.cell)):
            zc = _zs(max(c0, T.z0), min(c1, T.z1), 3)
            top = max(T.at(z)[1] for z in zc) + gap + th
            t.box(bone, f"bard_spine_{i + 1}", (-th * 1.1, top - th * 0.4, c0),
                  (th * 1.1, top + th * 1.5, c1), mat=spec.mat_dark, chain=("bard_spine", i))

    # 系带：把甲捆在马身上的那几道，横在札片外面。位置也从鞍的缝隙里挑——挑最靠前与
    # 最靠后那两段，两条带正落在肚带前后，与真甲的系法一致。
    for tag2, (a, b) in zip(("fore", "rear"), (slots[0], slots[-1]) if slots else ()):
        zt = _lerpf(a, b, 0.62 if tag2 == "fore" else 0.30)
        hw_t = T.band(zt, y_lo, y_hi)[1] + gap + th * 2.4
        # 系带横在札片外面，比甲面还外一层 —— 甲面的上缘让开了鞍翼，它更得让：
        # 首版按 `y_hi` 一路顶到顶，正好从鞍翼里穿过去（实测 0.02）。
        y_t = min(y_hi, over_ceiling(over_boxes(fit), zt - th, zt + th,
                                       hw_t - th * 1.4, hw_t, y_hi, gap))
        # 窄一点、也不要浮太高：一条从上到下、比甲还外一整层的宽带，在侧视里是两根
        # 竖着划过整片甲的杠——甲面本来是横的，一竖就把横排全打断了。
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            t.box(fit.trunk.bone_at(zt), f"bard_tie_{tag2}_{side}",
                  (sgn * (hw_t - th * 1.2), y_lo, zt - th * 0.5), (sgn * hw_t, y_t, zt + th * 0.5),
                  mat=spec.mat_trim)


def _clip_z(corners, z0: float, z1: float) -> list[Vec]:
    """盒（可带旋转）被 z ∈ [z0,z1] 切下来那一块的**顶点**。

    只数落在区间里的角点是不够的：颊带自耳后一路斜下到腮，整根盒横跨好几节颈——按角点
    问，靠头那两节量出来的深度（11.4）比颈本身还深（6.8，那个角其实在颈之外的腮上），
    下片整个被压没；而反过来只要盒的角点一个都没落进来，又会量成 0。切完之后的顶点
    只有两类：**区间内的角点** + **棱与两个切面的交点**——线性函数在凸多面体上的极值
    必在顶点，所以这是精确解，不是采样。

    **诚实标注**：上面两种坏法是查询范围还没连 `ext` 外延一起算、清空量也还只有一份
    `BARD_CLEAR` 时实测到的。那两处改掉之后，退回"只数角点"在现在这套头颈几何上
    **量出来一模一样**（三个体型下片都是 10 件、判据全干净）——所以这个函数眼下是
    冗余的一道保险，不是撑着某条判据的支点。留着是因为它是这类查询的正确写法，换一副
    笼头就未必还等价；但别把它当成"有测试护着"的东西。
    """
    c = list(corners)
    out = [p for p in c if z0 - 1e-9 <= p[2] <= z1 + 1e-9]
    idx = ((0, 1), (0, 2), (0, 4), (1, 3), (1, 5), (2, 3), (2, 6), (3, 7),
           (4, 5), (4, 6), (5, 7), (6, 7))
    for i, j in idx:
        a, b = c[i], c[j]
        if abs(b[2] - a[2]) < 1e-12:
            continue
        for zc in (z0, z1):
            s = (zc - a[2]) / (b[2] - a[2])
            if -1e-9 <= s <= 1 + 1e-9:
                out.append(tuple(a[k] + s * (b[k] - a[k]) for k in range(3)))
    return out


def rein_band_bottom(fit: Fit, za: float, zb: float) -> float:
    """三档缰在 [za,zb] 这一段里最深到颈脊之下多远（单位）—— 全颈鸡颈的下片自它之下起。

    与 `crinet_floor`（量缰的**上**缘）是同一把尺的两头：上片停在缰之上，下片从缰之下
    接着往下走，缰从两片之间的缝里穿出去。真具装的鸡颈本来就是这么留缰口的。

    **逐段问、而且不只问沿颈那条带**：靠头那两节上还压着颊带，它比缰线更深；只按
    `rein_line_neck` 给一个全颈通用的数，最靠头那一节的下片正好切进颊带里（实测 0.03）。
    """
    P = fit.P
    z0, z1 = min(za, zb), max(za, zb)
    worst = 0.0
    for tk in REINS:
        for e in other_tack(fit, "rein", tk):
            for c in _clip_z(_corners(e), z0, z1):
                worst = max(worst, crest_y(P, c[2]) - c[1])
    if worst <= 0.0:
        raise SystemExit("量不到缰在这一段的位置 —— 全颈鸡颈的下片没有依据")
    return worst + P.u(BARD_CLEAR) * 2.0


def neck_floor(fit: Fit, za: float, zb: float) -> float:
    """颈皮在这一段里到颈脊之下多远还有皮（单位）。全颈鸡颈下片的下缘由它定。

    逐 z **二分**问 `NeckLine.outer_at`（"这个高度上还有没有颈皮"），不去数角点：
    颈皮是一节节带交叠的斜盒，角点落不落在区间里全看运气——最靠头那一节一个角点都
    没落进来，量出来是 0，下片直接不做了。

    取一段里**最浅**的那个值：下片是一条直带，按最深的给，浅的那一头就垂到颈外面去了。
    """
    P, NL = fit.P, fit.neck
    best = 1e9
    for z in _zs(za, zb, 5):
        lo, hi = 0.0, P.u(0.70)
        for _ in range(16):
            m = (lo + hi) / 2
            if NL.outer_at(z, crest_y(P, z) - m, hair=False) is not None:
                lo = m
            else:
                hi = m
        best = min(best, lo)
    return best


def part_bard_crinet(t: Tack, fit: Fit, spec: BardSpec) -> None:
    """鸡颈：逐颈骨一段，盖住颈脊与上侧。下缘由 `crinet_floor` 从缰反推。

    顶档还要**兜到喉**（`full_wrap`）：缰之下再接一片，一直做到颈皮的最深处。
    铁浮屠是"人马皆铠"，颈是整只马上第二大的一片（占计入覆盖那些皮件的三成），
    只盖颈脊的话它有一多半露在外面——量下来顶档的缺口有四成在颈上。
    """
    P, NL = fit.P, fit.neck
    th, gap = P.u(spec.th), P.u(0.006)
    floor = crinet_floor(fit)
    if floor <= th * 1.6:
        raise SystemExit(f"缰把颈脊之下 {floor + P.u(BARD_CLEAR):.2f} 个单位全占了，鸡颈无处可放")
    seams = NL.seams
    # 下片的**下缘先整条算好再限速**。逐节各按自己那一段的颈皮最深处给，相邻两节能差
    # 出两三个单位（颈自鬐甲向头由粗变细再变粗），而喉板只有一个板厚多高——两节的喉板
    # 在 y 上就此错开，同一条链上直接断掉（挽马实测裂 0.99）。限速之后一节该多深会顺着
    # 邻节摊出去，摊成一条斜下去的线；取 `min` 而不是 `max`，保证没有一节垂到颈外面。
    dep: list[float] = []
    if spec.full_wrap:
        raw = []
        for k in range(len(NECK)):
            z0, z1 = sorted((seams[k][2], seams[k + 1][2]))
            raw.append(neck_floor(fit, z0, z1) - th)
        cap = th * 2.4
        dep = [min(v + cap * abs(k - j) for j, v in enumerate(raw)) for k in range(len(raw))]
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
        if spec.full_wrap:
            # 缰之下再接一片，一直兜到颈皮最深处（喉侧）。缰从上下两片之间穿出去——
            # 真具装的鸡颈也正是这么留缰口的。
            # **不跟着上片一起量横向**：颈自脊向下越来越宽（喉那一带比脊宽出小半个
            # 单位），照脊那一带的半宽做，下片会整条陷进颈里。
            # 查询范围要**连外延一起算**：这一片按 `ext` 向两头各探出一截去和邻节
            # 搭上，探到的那一截落在邻节的 z 上——只按 [za,zb] 问，最靠头那一节探出去
            # 的部分正好切进颊带（实测 0.03）。
            low0 = rein_band_bottom(fit, min(za, zb) - pad, max(za, zb) + pad)
            lo_a = ya - low0
            lo_b = yb - low0
            # 收一个板厚（已并进 `dep`）：下片正好垂到颈皮最深处的话，低头吃草时颈折
            # 下来，它比颈皮先撞上肩（挽马实测比静止姿多陷 2.03 > 1.87）。
            h_low = max(dep[k] - low0, 0.0)
            if h_low > th * 1.5:
                lm_a, lm_b = lo_a - h_low * 0.5, lo_b - h_low * 0.5
                x_low = NL.outer_span(za, lm_a, zb, lm_b, hair=False) + gap
                for sgn, side in ((-1.0, "l"), (1.0, "r")):
                    _strap_world(t, bone, f"bard_crinet_low_{k + 1}_{side}",
                                 sgn * (x_low - d * 1.2), sgn * (x_low + d),
                                 (lm_a, za), (lm_b, zb), h_low,
                                 mat=spec.mat, chain=(f"bard_crinet_low_{side}", k), ext=ext)
                # 喉板：把颈底封上。
                #
                # **这一件是看图定的，不是量出来的**——拆掉它重量了一遍：连通性照样是
                # 4 片（下片自己经鬐甲接到当胸上），覆盖率只掉 0.2pp。原因是覆盖率那条
                # 只在皮件的**左右两个侧面**取样，朝下的那一片它根本没在采（同当初补
                # 后襟时那个 0.2pp）。可正面看过去，没有它颈底就是一道敞着的槽——
                # 两条下片挂在颈两侧，中间通到底。判据量不到的东西，只能靠图。
                # 喉板的交叠**另算**：它挂在颈底，离转心比脊上的盖远得多（脊那道交叠
                # 是照盖自己的半径给的），照抄过来在挽马身上一屈伸就裂 0.99。
                # 喉板给厚一点、下缘钉在颈底：厚度是它容忍相邻两节深浅差的全部本钱。
                # 薄板 + 紧限速那一版链上只剩 0.06 的余量，而限速一松覆盖就掉——把厚度
                # 加上去，限速才放得开（0.75 → 0.79 常马）。
                h_t = d * 3.4
                yt_a, yt_b = ya - dep[k] + h_t / 2 - d * 0.1, yb - dep[k] + h_t / 2 - d * 0.1
                rt = max(math.hypot(x_low * 0.72, y - sm[1])
                         for y, sm in ((yt_a, seams[k]), (yt_b, seams[k + 1])))
                pt = seam_pad(rt, NECK_ROM)
                et = (0.0 if k == 0 else pt, 0.0 if k == len(NECK) - 1 else pt)
                _strap_world(t, bone, f"bard_crinet_throat_{k + 1}",
                             -(x_low * 0.72), x_low * 0.72,
                             (yt_a, za), (yt_b, zb), h_t,
                             mat=spec.mat, chain=("bard_crinet_throat", k), ext=et)
        if spec.glow:
            _strap_world(t, bone, f"bard_crinet_glow_{k + 1}", -(x_out * 0.42), x_out * 0.42,
                         (ya + d * 1.15, za), (yb + d * 1.15, zb), d * 0.4,
                         mat="glow", glow=True, ext=(ext[0] - d, ext[1] - d))


# 面帘（头局部系，× 头长；与 `gen_pelt.part_head` / 笼头同一套坐标）
# 腮 / 颌那一带的皮件：全罩面帘的下片按它们量（腮比颅壳宽，照颅壳做会整条陷进去）
JAW_RE = re.compile(r"jaw_line_[lr]|jowl_[lr]|chin")
# 颊板逐小段做的段长（× 头长）。缝要跟着缰的斜度走：缰自嘴角一路斜上到耳后，一整节
# 之内它自己就升了 0.27 个头长——按整节取最大，缝张得比脸的一半还高，眼周整片开成洞。
CHAM_SUB = 0.095
CHAM_JAW_BACK = 0.12  # 全罩面帘的颌板后缘比面帘本体再往前收这么多（× 头长），给鸡颈让位
CHAM_Z_BACK = -0.245  # 后缘：耳在 z[-0.215,-0.055]，压在耳上马会甩头（同项带的道理）
CHAM_Z_HALF = -0.660  # 半面帘前缘：鼻梁中段停住
CHAM_Z_FULL = -0.930  # 全面帘前缘：鼻孔（z[-1.045,-0.955]）之后停住，嘴还能张
CHAM_TOP_W = 0.66  # 顶板占颅壳半宽的几成（两侧归颊板）
# 面帘离脸最远几个**面帘板厚**（再远就是飘着）。不写成绝对单位：面帘是一节一节的，
# 后一节比前一节宽，同一条颊板骑在两节上只能按宽的那节给，差出来的量级就是一个板厚；
# 这个量随体型、随分档一起变，绝对数在矮马上松、在挽马上紧。
CHAM_FLOAT = 3.0
# 面帘的板厚（× 鬐甲高）。**不跟着 `spec.th` 走**：身甲那个数说的是一片札片有多厚，
# 而面帘是一整块脸板，两者本来就不是一种东西。跟着走的坏处是判据会跟着漂——顶档从
# 整板改回薄札片时 `spec.th` 减半，"面帘离脸多远算飘"的上限跟着减半，几何一点没动
# 的面帘凭空报了个飘（0.63 > 0.60）。面帘离脸多远由**络头有多厚**决定，与身甲无关。
CHAM_PLATE = 0.015
EYE_RE = re.compile(r"eye_[lr]|eye_socket_[lr]")
# 头上那套东西要分成两类，面帘对它们的态度正好相反：
#   · **络头**（项 / 额 / 鼻 / 颊 / 咽革）是戴在头上的，面帘**罩在它外面**——真甲也这么
#     戴：先络头，再扣面帘。所以它决定面帘往外抬多少。
#   · **缰绳本身**（缰线 / 环 / 衔铁）是牵在骑手手里的。骑手没提缰时它从嘴角搭到耳后、
#     顺着颈垂下去，正好压在腮上。面帘要是把这一段罩进去，缰就从甲里长出来了，而且
#     是**牵不动的那种**。所以它决定面帘的下缘停在哪。
# 首版没分这两类，一律"罩在外面"：颊板照缰线的 |x|=0.272 让开，比脸宽出一整个单位，
# 整只面帘鼓成个头盔。
HEADSTALL_RE = re.compile(r"rein_(crown|brow|nose|cheek|throat|knot)\w*")
REINLINE_RE = re.compile(r"rein_(line|ring|bit)\w*")


def _rein_local(fit: Fit, pat: re.Pattern) -> list[tuple[Vec, Vec]]:
    """三档缰里匹配 pat 的件，换算到头局部系的 AABB（头长比例）。

    两处讲究：
      · 逐**角点**换算再取包围盒，不整盒换：颊带带着自己的俯角转过，整盒换不进局部系。
      · 判"在不在这一段"按**区间相交**，不按"有没有角点落在区间里"。缰线从嘴角一路
        拉到耳后，一根就横跨好几节颅壳，两个端点都在段外——按角点判，中间那几节
        一个都量不到，下缘凭空塌下去。
    """
    Hd = fit.head
    out = []
    for tk in REINS:
        for e in other_tack(fit, "rein", tk):
            n = e["name"]
            # 名字没归类 = 静默漏判。分类是按**名字**做的（络头罩得住、缰绳罩不得，
            # 这个区别几何上看不出来），所以新加一件缰具而忘了归类时，它会既不抬高
            # 面帘也不顶住下缘——面帘正好从它身上压过去，两边判据都不响。
            # 一档绳笼头的 `rein_knot_*` 就是这么漏的（矮马重铁甲实测穿进 0.04）。
            if not HEADSTALL_RE.fullmatch(n) and not REINLINE_RE.fullmatch(n):
                raise SystemExit(f"缰件 {n} 既不算络头也不算缰绳 —— 面帘不知道该罩它还是让它")
            if not pat.fullmatch(n):
                continue
            cs = [Hd.point_local(c) for c in _corners(e)]
            out.append((tuple(min(c[i] for c in cs) for i in range(3)),
                        tuple(max(c[i] for c in cs) for i in range(3))))
    return out


def _span(boxes, za: float, zb: float):
    return [b for b in boxes if b[1][2] > za + 1e-9 and b[0][2] < zb - 1e-9]


def _shell_segs(fit: Fit, z0: float, z1: float, ov: float) -> list[tuple[float, float]]:
    """颅壳分节的 z 区间，裁到 [z0,z1]，相邻两节各外扩 ov 好搭上。

    面帘必须**跟着颅壳分节走**：颅壳一节一节变宽（吻端半宽 0.080，眼那一节 0.152），
    一整块板从额铺到吻，中段必然架空一大截——和身甲要切格是同一件事。外扩是为了让
    相邻两节**真的重叠**：恰好对接时连通性判据分不出"接着"和"裂开"，整面面帘会被
    报成四片。
    """
    # 取**分界线**再连成相邻区间，不是拿每节颅壳各自裁一段：颅壳的 z 段两头都会被裁到
    # [z0,z1] 上，裁完好几节挤成同一段，出来的是几块重合的板（同处几何、互相闪）。
    zs = sorted({z for n, b in fit.head.local.items() if fit.head.SHELL.fullmatch(n)
                 for z in (b[0][2], b[1][2]) if z0 - 1e-9 <= z <= z1 + 1e-9} | {z0, z1})
    # 碎段并进邻段，不是各自成段：颅壳两节之间常常只差几个千分位（吻端 -0.905 与
    # -0.900），单独成段的话外扩之后整段套在邻段里，出来两块同处的板互相闪。**第一段
    # 也要并**——它前面没有邻段可并，首版就漏了它，最前那块板整块套在第二块里。
    segs: list[list[float]] = []
    for a, b in zip(zs, zs[1:]):
        if segs and b - a < 0.06:
            segs[-1][1] = b
        else:
            segs.append([a, b])
    while len(segs) > 1 and segs[0][1] - segs[0][0] < 0.06:
        segs[1][0] = segs[0][0]
        segs.pop(0)
    return [(max(a - ov, z0), min(b + ov, z1)) for a, b in segs if b - a > 1e-4]


def part_bard_chamfron(t: Tack, fit: Fit, spec: BardSpec) -> None:
    """面帘：罩在络头外的脸甲。半面帘只压额与鼻梁（眼整个露在外），全面帘罩到吻端、
    在眼上开窗。

    眼窗**按皮层的眼件开**，不按"脸长的几成"猜：眼球鼓出颅壳外（|x| 到 0.194，颅壳
    只有 0.152），拿比例估出来的窗必然要么糊住眼要么大得像没戴。
    """
    P, Hd = fit.P, fit.head
    H = P.H
    # 板厚 / 空隙换成**头长比例**，并且**减半**：`spec.th` 是照桶身定的（0.030 × 鬐甲高
    # = 0.74 个单位），扣在一张只有一个半单位半宽的脸上，光板厚就把面帘撑到脸的两倍宽。
    # 真甲的板厚在头上和身上是同一个数，但那个数在这个尺度下本来就是夸张过的——
    # 身上夸张得看不出来，头上夸张得像扣了个箱子。
    th, gap = P.u(CHAM_PLATE) / H, P.u(0.003) / H
    full = spec.chamfron == "full"
    z_front = CHAM_Z_FULL if full else CHAM_Z_HALF
    eye = Hd.part_local(EYE_RE)
    if eye is None:
        raise SystemExit("皮层里找不到眼件 —— 面帘的眼窗无处可开")
    stall, line = _rein_local(fit, HEADSTALL_RE), _rein_local(fit, REINLINE_RE)
    ey0, ey1 = eye[0][1] - gap, eye[1][1] + gap
    ez0, ez1 = eye[0][2] - gap, eye[1][2] + gap
    # 面帘是**一块打磨过的整板**，不一定跟身上的甲面同料：锁子甲那一档全靠"哑光环帘
    # 上扣两块亮钢"的反差被认出来，面甲跟着环帘走就把这一档最好认的地方抹掉了。
    mat = spec.mat_plate or spec.mat
    md = spec.mat_plate_dark or spec.mat_dark

    def crest(z: float) -> float:
        """脸在这个 z 处的"面帘该贴到哪"：颅壳顶与**跨中线**那几条络头带里更高的那个。

        只算跨中线的带：颊带顺着腮一直爬到耳后（局部 y 到 0.249，比颅壳顶还高 0.7 个
        单位），可它在脸侧面，顶板压根不盖它——盖它的是颊板，而颊板的内侧面在它外头。
        不分中线不中线一律取最大，顶板会被一条侧面的带整段顶起来。
        """
        sh = Hd.shell_or_none(z) or Hd.shell(min(max(z, z_lo0), z_hi0))
        hw_, top_, _ = sh
        mid = [b[1][1] for b in stall if b[1][2] > z and b[0][2] < z
               and b[0][0] < hw_ * 0.7 and b[1][0] > -hw_ * 0.7]
        return max([top_] + mid) + gap

    zs_shell = [b[i][2] for n, b in Hd.local.items() if Hd.SHELL.fullmatch(n) for i in (0, 1)]
    z_lo0, z_hi0 = min(zs_shell), max(zs_shell)
    pad = th * 0.9  # 带两端各外延这么多好和邻节搭上

    def band(za: float, zb: float) -> tuple[float, float]:
        """一段沿脸坡的带的两端高度：抬到**整段**（连同两端外延）都压不到脸与络头。

        只按两端定这条线不行：脸不是直的，鼻革又恰好落在一节的中段——线从两端看好好地
        架在脸上，中间那一截正好陷进鼻革里（实测 0.49 单位）。所以逐点采样，按最深的
        那一处把整条线整体抬起来。
        """
        ya, yb = crest(za), crest(zb)
        zs = [_lerpf(za - pad, zb + pad, i / 8) for i in range(9)]
        d = max(crest(z) + th / 2 - _lerpf(ya, yb, (z - za) / max(zb - za, 1e-9)) for z in zs)
        return ya + d, yb + d

    segs = _shell_segs(fit, z_front, CHAM_Z_BACK, th * 0.7)
    # 缝的上下缘**只准往上走**。缰自嘴角一路斜上到耳后，本来就是单调的；可三档缰各有
    # 各的走法，逐小段各算各的会前后跳，颊板的边缘就成了一圈毛碴（渲出来像块碎掉的
    # 镜子）。锁成单调之后，缝是一道顺着缰爬上去的斜口，边缘干净。
    slot_run = [-9.0, -9.0]
    # 缰缝**之下**那几片共用一个横向：腮与颌线是脸上最宽的一段，而颊板是薄壳——逐节
    # 各按自己那节量的话，相邻两节的下片落在不同的 |x| 上，前后根本挨不着（实测最靠吻
    # 那一节自己单独成一片）。上片不这么做是另一回事：上片靠顶板连成一体，而且它一路
    # 跟着脸自吻向后变宽，通宽了就成了钉在脸上的木板。
    xj_run = [0.0]
    for k, (za, zb) in enumerate(segs):
        hw, top, bot = Hd.shell(max(min((za + zb) / 2, z_hi0), z_lo0))
        near = _span(stall, za, zb)
        ya, yb = band(za, zb)
        # 颊板的下缘。半面帘停在眼上缘（它本来就不护眼）；全面帘要下到眼下缘，不然眼窗
        # 没有下框。**全包（`full_wrap`）再往下兜到颌**——"只漏眼睛"要的是把腮和颌线
        # 一起罩住，那是脸上除了眼之外最大的两片。
        # 缰线（骑手要拉的那条）横在腮上，`slot` 会在它经过的地方开一道横缝，板从缝的
        # 上下两侧接着走；不开全包时下缘直接停在缰之上。
        ln = _span(line, za, zb)
        if not full:
            y_lo = ey1
        elif spec.full_wrap:
            jaw = [b for n, b in Hd.local.items() if JAW_RE.fullmatch(n)
                   and b[1][2] > za and b[0][2] < zb]
            y_lo = min([bot] + [b[0][1] for b in jaw]) - gap
        else:
            y_lo = max([bot + (top - bot) * 0.18] + [b[1][1] + gap for b in ln])

        y_out = max(ya, yb) + th / 2
        # 外侧面按**这一节罩着的那段脸**量，整节共用一个：脸不是颅壳，颊、颌线、眼眶都
        # 鼓在颅壳外（眼到 0.194，颅壳只有 0.152），照颅壳给，眼从板里穿出来。逐块各量
        # 各的会让眼窗前后两块比窗框那两条更靠外，窗口凹进去一台阶。
        # **逐节各量各的**，跟着脸自吻向后变宽。首版取一路不减的跑动最大值（怕相邻两节
        # 搭不上），结果最前那几节顶着最宽那一节的宽度铺过去——一张越往前越窄的脸上扣
        # 着一块通宽的板，正视图里就是块钉在脸上的木板。搭不上这件事其实不会发生：窄的
        # 那一节整个套在宽的里面，两节在 z 上又是重叠的，连通性照样成立。
        xo = max([hw] + [max(h[0], -lo2[0]) for lo2, h in list(Hd.local.values()) + near
                         if h[2] > za and lo2[2] < zb and h[1] > y_lo and lo2[1] < y_out]) + gap + th
        # 缝**之下**那几片的横向：按这一节量，但**一路往后不减**。
        #   · 拿全脸的最大值（腮最宽）通吃：吻那一节的下片离脸差出小半个头长，判据当场
        #     报"面帘飘在脸外 0.94 > 0.86"（矮马）；
        #   · 逐节各算各的：相邻两节的下片落在不同的 |x| 上，薄壳前后挨不着，最靠吻那一
        #     节自己单独成一片。
        # 脸自吻向腮变宽，跟着走的最大值两头都满足。
        # 络头也要罩在里头（`near`）：下片贴着腮走，而鼻革正横在腮前——不算它，
        # 下片直接切进鼻革（矮马实测 0.28）。同上片 `xo` 的道理。
        # 下片往里加厚一截好和前一节搭上（加的那截埋在脸里，看不见）：薄壳的横向一节
        # 一节往外让（吻窄腮宽），前后就挨不着了。加厚之后**内缘也不许戳进络头**，
        # 所以络头那一项按加厚量让开（挽马实测只让一个板厚时切进鼻革 0.04）。
        tk_low = th * 1.6
        xj_run[0] = max(xj_run[0],
                        max([hw] + [b[1][0] for n, b in Hd.local.items() if JAW_RE.fullmatch(n)
                                    and b[1][2] > za and b[0][2] < zb]) + gap + th,
                        max([max(h[0], -lo2[0]) for lo2, h in near], default=0.0)
                        + gap + tk_low * 1.1)
        xj = xj_run[0]
        # 顶板**通宽、而且顺着脸的坡斜过去**。脸自吻向枕升起将近一个头长的 0.12，一节
        # 一节地拿水平板去铺，相邻两块在 y 上差得比板还厚，正视看过去是三层错开的架子
        # （而且连通性判据会把整面面帘报成三片）。改成沿坡的斜带，两端各外延一点搭上。
        Hd.strap(t, f"bard_cham_top_{k + 1}", -xo, xo, (ya, za), (yb, zb), th,
                 mat=mat, ext=(pad, pad))

        def rein_slot(c0: float, c1: float, xr: float, tk: float = th):
            """缰线在 [c0,c1] 这一小段上占住的那段高度（够不到颊板那一层的不算）。

            **逐小段问，不按整节问**：缰自嘴角一路斜上到耳后，一节之内它自己就升了
            0.27 个头长——按整节取最大，缝张得比脸的一半还高，眼周整片开成了洞。
            衔铁与衔环挤在嘴角、|x| 只到 0.196，而颊板在 0.21 开外：照它们开缝，最靠吻
            那一节（本来一条缰线都没有）也被拦腰截断。
            """
            hit = [b for b in _span(line, c0, c1)
                   if max(abs(b[0][0]), abs(b[1][0])) > xr - tk
                   and (min(abs(b[0][0]), abs(b[1][0])) if b[0][0] * b[1][0] > 0 else 0.0) < xr]
            if not hit:
                return None
            # 锁**只能往"缝变宽"的方向用**。缝的上沿锁成单调（缰一路往上爬，上沿跟着
            # 爬，边缘才不毛碴）；下沿**不能锁**——嘴角那只衔环比后面的缰线低一截，
            # 往上锁一下就把它锁到缝外面，板直接切进环里（矮马实测 0.19）。
            # 判据要的是"缝必须罩住这一小段里的每一件"，任何**收窄**都可能漏。
            lo_s = min(b[0][1] for b in hit) - gap
            hi_s = max(max(b[1][1] for b in hit) + gap, slot_run[1])
            slot_run[1] = hi_s
            return (lo_s, hi_s)

        # 颊板逐**小段**做：缝要跟着缰的斜度走，段给粗了缝就跟着张大
        n_sub = max(1, int(math.ceil((zb - za) / CHAM_SUB)))
        for u in range(n_sub):
            sa = _lerpf(za, zb, u / n_sub) - (0.0 if u == 0 else th * 0.5)
            sb = _lerpf(za, zb, (u + 1) / n_sub) + (0.0 if u == n_sub - 1 else th * 0.5)
            # 不全包时不开缝：`y_lo` 本来就顶在缰之上（见上），板整块停在那儿
            # 上下两片各按**自己那块板的宽**问缝：下片比上片更靠外（腮宽），一件够不到
            # 上片的衔环照样够得到下片（矮马实测下片切进衔环 0.17）。
            slot = rein_slot(sa, sb, xo) if full and spec.full_wrap else None
            # 下片往里加厚了一截（见 `low`），问缝时得按**加厚后的内缘**问，
            # 否则一件够不到薄壳的衔环照样够得到加厚后的下片（矮马实测 0.19）
            slot_j = rein_slot(sa, sb, xj, tk_low) if full and spec.full_wrap else None
            for sgn, side in ((-1.0, "l"), (1.0, "r")):
                def cheek(nm: str, a: float, b: float, c0: float, c1: float,
                          m: str = mat, s=sgn, sd=side, xr=xo, uu=u, tk=th) -> None:
                    if b - a < th * 0.5:  # 缝顶得太高时窗下那条会被压成零高，不做
                        return
                    Hd.put(t, f"bard_cham_{nm}_{k + 1}{uu + 1}_{sd}",
                           (s * (xr - tk), a, c0), (s * xr, b, c1), mat=m)

                def low(nm: str, a: float, b: float, c0: float, c1: float) -> None:
                    # 后缘再往前收一截：下片与鸡颈的下片都想占喉与腮之间那一小块，两件
                    # 挂在不同的骨上，一转头就穿插（矮马实测 0.30）。让面帘先停。
                    c1 = min(c1, CHAM_Z_BACK - CHAM_JAW_BACK)
                    if c1 - c0 > th * 0.3:
                        # 加厚见 `tk_low`：薄壳的横向一节一节往外让（吻窄腮宽），
                        # 前后就挨不着了——常马实测吻那一节的下片自己单独成一片。
                        cheek(nm, a, b, c0, c1, mat, sgn, side, xj, u, tk_low)

                def bands(a: float, b: float):
                    """[a,b] 去掉缰占住的那一段之后剩下的几截。上片按 `slot`、下片按
                    `slot_j` —— 缝的上沿归上片管，下沿归下片管，各按各的宽问出来的。"""
                    sl = slot if slot_j is None else (slot_j[0], (slot or slot_j)[1])
                    if sl is None or sl[1] <= a or sl[0] >= b:
                        return [(a, b)]
                    return [x for x in ((a, min(b, sl[0])), (max(a, sl[1]), b)) if x[1] > x[0]]

                if full and sb > ez0 and sa < ez1:
                    # 这一小段撞上眼：窗上一条、窗下一条，窗前后各补一段满高的
                    cheek("brow", ey1, y_out, max(sa, ez0), min(sb, ez1), md)
                    for i, (a, b) in enumerate(bands(y_lo, ey0)):
                        (low if i == 0 and slot else cheek)(
                            f"cheek{i}", a, b, max(sa, ez0), min(sb, ez1))
                    if sb > ez1:
                        for i, (a, b) in enumerate(bands(y_lo, y_out)):
                            (low if i == 0 and slot else cheek)(f"side{i}", a, b, ez1, sb)
                    if sa < ez0:
                        for i, (a, b) in enumerate(bands(y_lo, y_out)):
                            (low if i == 0 and slot else cheek)(f"side2{i}", a, b, sa, ez0)
                else:
                    for i, (a, b) in enumerate(bands(y_lo, y_out)):
                        (low if i == 0 and slot else cheek)(f"side{i}", a, b, sa, sb)
        if spec.glow:
            # 灵纹：沿鼻梁中线一道，压在顶板外面。半面帘就这一处发光，与鸡颈那道连成一线
            Hd.strap(t, f"bard_cham_glow_{k + 1}", -xo * 0.22, xo * 0.22,
                     (ya + th * 0.55, za), (yb + th * 0.55, zb), th * 0.5,
                     mat="glow", glow=True, ext=(-gap, -gap))

    if spec.plume:
        # 面缨：额上竖起的一对。整副甲一身近黑，剪影上唯一认得出的东西就是它。
        #
        # 长在**最靠枕的那一节**（额顶），不是吻端：吻端那一节又窄又低，缨插上去像两根
        # 长在鼻子上的天线；额顶正是参考图里那对缨的位置，也是耳的前面一点点——所以
        # 后缘卡在 `CHAM_Z_BACK` 之内，不去顶耳（顶了马会甩头，同项带的道理）。
        za, zb = segs[-1]
        ya, yb = band(za, zb)
        top = max(ya, yb) + th / 2
        zc = _lerpf(max(za, CHAM_Z_BACK - 0.10), min(zb, CHAM_Z_BACK), 0.5)
        for sgn, side in ((-1.0, "l"), (1.0, "r")):
            x0 = sgn * th * 1.2
            # 两截收出个尖：一个体素 6.25 cm，再多截也读不出更尖
            for i, (w, y0, y1) in enumerate(((1.5, 0.0, 0.085), (0.9, 0.075, 0.150))):
                Hd.put(t, f"bard_cham_plume_{i + 1}_{side}",
                       (x0, top + y0, zc - th * w), (x0 + sgn * th * w * 1.1, top + y1, zc + th * w),
                       mat=spec.plume, chain=(f"bard_plume_{side}", i))


def build_bard(t: Tack, fit: Fit, spec: BardSpec) -> None:
    part_bard_body(t, fit, spec)
    if spec.crinet:
        part_bard_crinet(t, fit, spec)
    if spec.chamfron:
        part_bard_chamfron(t, fit, spec)


# 计入覆盖率的皮件：躯干 / 颈 / 头。**腿与尾不计**——真马铠本来就不护腿（`FIT_TOL` 的
# limb 那一档说的正是腿会扫进甲里），把腿算进分母，五档的覆盖率会一起被同一个常数压低，
# 分档之间的差反而看不出来。
COVER_RE = re.compile(r"|".join((SHAPE_RE.pattern, r"neck_\d+|neck_throat_\d+",
                                 r"head_shell_\d+|jaw_line_[lr]|jowl_[lr]|chin|lip_(upper|lower)")))
COVER_NEAR = 0.55  # 甲离皮多近算"盖住了"（单位）
# 逐档至少要多盖住这么多（比例）。**门槛低不是宽容，是量出来的天花板低**：马身侧面有
# 三片地方任何一档都盖不到，而且各有各的主人——
#   · 肚带那一圈（真的从背绕到腹底，甲只能断开，`bard_runs`）；
#   · 鞍翼与镫那一段的上缘（`over_ceiling` 把它压下去；真马铠也正是在这儿剜一块）；
#   · 颈侧与肩（缰垂在腮与颈侧、前肢在肩上扫，`crinet_floor` / limb 容许量）。
# 三条都是**跨装备**约束、都已经各有判据。所以这条只负责挡住"新一档其实没多盖"，
# "新一档必须多配一件具装"由 `check_escalation` 挡，两条各管一头。
MIN_COVER_STEP = 0.004
# 顶档的下限（腿与尾不在分母里，见 COVER_RE）。铁浮屠是"人马皆铠"，这个数是它的
# 核心指标，不是随手一个及格线：全颈鸡颈 + 喉板落地后实测 77.4 / 80.0 / 81.1%，
# 下限跟着抬到 75%。
#
# **剩下那两成不是没做，是位置被别人占着**：量下来缺口的一大半是**鞍位那一带**
# （鞍垫压在背上、鞍翼与镫革挂在肋侧，甲一进去跨装备判据当场就红）；再就是头的下半，
# 那儿夹在缰线与鸡颈之间，做出来是两块什么都不挂着的浮板（见 `part_bard_chamfron`）。
# 换句话说：**穿着鞍的马身上，77% 之外基本就是鞍自己占的位置。**
MIN_COVER_TOP = 0.75


COVER_N = 6  # 每块皮的侧面各取 N×N 个采样点


def coverage(t: Tack, fit: Fit) -> float:
    """甲盖住了马身**侧面**的百分之几（0-1）。

    "从轻到重覆盖范围递增"必须是个**量得出来的数**，不能只看图。看图看得出"有没有多
    一件"，看不出"多盖住了多少"：把当胸做大一圈、下摆放长两寸，部件表一点不动而覆盖
    率立刻动；反过来加一面寄生，部件多了一件、覆盖率几乎不变（它本来就不是护具）。

    量的是**皮面上的采样点**，不是"这块皮件有没有被碰到"。按件算的那一版，一块从肩铺
    到腰的肋侧皮只要被任何一片甲蹭到就整块算覆盖——重铁甲照那个口径报 90%，而渲染图
    上肩与腹明明大片露着。判据比图还乐观的时候，它就不再是判据了。
    """
    pelt = [e for e in fit.pelt_els if COVER_RE.fullmatch(e["name"])]
    tack = tack_els(t)
    if not pelt or not tack:
        return 0.0
    # 采样点：每块皮的左右两个侧面各 N×N。侧面是玩家看得见的那一面，也是甲该盖的那一面
    pts, wts = [], []
    g = (np.arange(COVER_N) + 0.5) / COVER_N * 2 - 1
    for e in pelt:
        c, h, R = _obb(e)
        area = float(h[1] * h[2]) * 4 / (COVER_N * COVER_N)
        for sx in (-1.0, 1.0):
            for a in g:
                for b in g:
                    pts.append(c + R @ np.array([sx * h[0], a * h[1], b * h[2]]))
                    wts.append(area)
    P = np.array(pts)
    W = np.array(wts)
    cov = np.zeros(len(P), bool)
    for e in tack:
        c, h, R = _obb(e)
        d = np.abs((P - c) @ R)  # R 正交：R^T(p-c) == (p-c)@R
        cov |= (d <= h + COVER_NEAR).all(axis=1)
    return float(W[cov].sum() / W.sum()) if W.sum() else 0.0


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
    ok = {*BARD_TRUNK, *NECK, *((BRIDLE_BONE,) if spec.chamfron else ())}
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
    # 只挑甲片本身。板缘的亮棱（`bard_*_rim_*`）挂在自己那块板上、不挂在马身上——
    # 首版的条件是"名字里有 _lame_ **或**以 bard_skirt 开头"，垂缘的亮棱正好两条都撞上，
    # 于是十六道棱一起报"没贴在马身上"。
    # 布档的身甲叫 `bard_fold_*`（一整幅垂下来的布，不排札片）。**两个词元都要认**：
    # 只认 `_lame_` 的话，布档整幅身甲一件都不在这条判据的视野里——而"甲飘在体外"
    # 这种翻车恰恰在布档最容易发生（它是唯一一档故意不贴着腹走的）。
    lames = [e for e in els if "_lame_" in e["name"] or "_fold_" in e["name"]]
    if not lames:
        bad.append("没有甲片")
    for e in lames:
        if e["name"].startswith("bard_crinet"):
            continue
        if sum(_overlap_vol(e, pe) for pe in torso_els) < MIN_BITE:
            bad.append(f"{e['name']} 没贴在马身上（与躯干皮无实交）——会看着浮在体外")
    # --- 大腿不许从甲里长出来 ---
    # 静止姿就穿模，而**原有的判据一条都够不到它**：`limb` 那档量的是"动起来比静止姿
    # 多陷多少"，静止姿本身有多糟它不问；上面那条实交查的是甲有没有埋进**躯干**，
    # 大腿在不在甲外面它也不问。于是五档三体型全在漏（挽马重铁甲露 3.13 单位），
    # 图上就是"屁股那儿穿模"。
    limbs = [e for e in fit.pelt_els if LIMB_UPPER_RE.fullmatch(e["name"])]
    out = []
    for a in lames:
        ca = np.array(_corners(a), float)
        ax = float(np.abs(ca[:, 0]).max())
        for b in limbs:
            cb = np.array(_corners(b), float)
            if (cb[:, 1].max() < ca[:, 1].min() or cb[:, 1].min() > ca[:, 1].max()
                    or cb[:, 2].max() < ca[:, 2].min() or cb[:, 2].min() > ca[:, 2].max()):
                continue
            d = float(np.abs(cb[:, 0]).max()) - ax
            if d > CROSS_TOL:
                out.append((d, b["name"], a["name"]))
    if out:
        d, bn, an = max(out)
        bad.append(f"{bn} 比甲还往外 {d:.2f} 单位（{an}，共 {len(out)} 处）——大腿从甲里长出来")

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

    # --- 马还得看得见 ---
    # 面帘盖住眼睛不是"细节没做好"，是这匹马瞎了。而渲染图上它长得和"戴好了"一模一样：
    # 一块板压在眼上，看图只会觉得脸挺整齐。眼窗按皮层的眼件开，这条按同一批眼件查。
    eyes = [e for e in fit.pelt_els if EYE_RE.fullmatch(e["name"])]
    if spec.chamfron and eyes:
        hit = [(e["name"], v) for e in els for o in eyes if (v := _pen(e, o)) > CROSS_TOL]
        if hit:
            nm, v = max(hit, key=lambda h: h[1])
            bad.append(f"面帘压住眼睛（{nm} 陷进 {v:.2f} 单位，共 {len(hit)} 处）——马看不见了")

    # --- 面帘与鸡颈是两件甲，也得互相让 ---
    # 跨装备判据只查"别的马具"，同一副甲里的两件互相穿插它一条都不响：面帘挂 skull、
    # 鸡颈挂 neck_7，两根骨紧挨着，各按各的基准往外鼓，撞上是常态。
    cham = [e for e in els if e["name"].startswith("bard_cham")]
    # 缨**本来就该离脸远**（它是竖在额上的两根，不是贴脸的板）。下面"面帘不许飘"那条
    # 量的是"离脸多远"，把缨算进去必然误报——两者是相反的要求，只能按件分开。
    face_plate = [e for e in cham if "_plume_" not in e["name"]]
    crin = [e for e in els if e["name"].startswith("bard_crinet")]
    if cham and crin:
        v = max(_pen(a, b) for a in cham for b in crin)
        if v > CROSS_TOL:
            bad.append(f"面帘与鸡颈自己撞了 {v:.2f} 单位——同一副甲里两件穿插")

    # --- 面帘不许飘 ---
    # 面帘是**架在络头上**的，不咬进脸里（络头夹在中间），所以"贴住了没有"那条按躯干
    # 那套实交口径查不到它——查得到的只有"离脸多远"。飘起来的面帘在正视图里和戴好的
    # 长得几乎一样（正面看它就该盖住脸），只有侧视才看得出来悬空。
    # "脸"要连**腮与颌线**一起算：全罩面帘的下片本来就是贴着腮走的，而腮比颅壳宽出
    # 小半个头长——只拿颅壳当尺，贴得好好的颊板会被报成"飘在脸外 0.94"（矮马）。
    #
    # **络头也算**。面帘是**架在络头上**的（本层从一开始就这么定的），而颊带顺着腮一路
    # 下到吻——吻那一段颅壳只有 0.092 宽、颌线 0.104，颊带却在 0.156：面帘只能罩在颊带
    # 外面，于是离"脸"必然差出半个多单位。只拿皮当尺，等于要求面帘穿过颊带去贴脸。
    face = [e for e in fit.pelt_els
            if fit.head.SHELL.fullmatch(e["name"]) or JAW_RE.fullmatch(e["name"])]
    if spec.chamfron:
        face = face + [e for tk in REINS for e in other_tack(fit, "rein", tk)
                       if HEADSTALL_RE.fullmatch(e["name"])]
    if face_plate and face:
        d, who = max((min(-_pen(c, f) for f in face), c["name"]) for c in face_plate)
        lim = P.u(CHAM_PLATE) * CHAM_FLOAT
        if d > lim:
            bad.append(f"面帘飘在脸外 {d:.2f} 单位（{who}，上限 {lim:.2f}）——没扣在脸上")

    # 片数：身甲被鞍切成几段就是几片（`bard_runs`；常规是肚带前一片、肚带后一片），
    # 再加鸡颈与面帘各一件。**不写死 2**——矮马的镫垂得比肚带还长，切法本来就不同；
    # 写死的话它要么误报、要么逼着造型去迁就一个数。这条挡的是"多出来的片"：某处该
    # 搭上的没搭上，而那道缝多半被别的甲片挡着，静帧看不出来。
    # 全罩 + 全包时面帘是**三片**：主体（顶板 + 缰缝之上的颊板 + 眉框 + 缨）加左右两片
    # 缰缝之下的颊板。它们连不上不是做漏了，是**没有能连的地方**：
    #   · 往上——缰线（骑手要拉的那条）正横在腮上、就在颊板那一层的 |x| 里，罩过去缰就
    #     牵不动了；
    #   · 往下——鸡颈的下片挂在**颈骨**上，面帘挂在颅骨上，一转头就穿插（那条判据在下面）。
    # 真甲的颊片本来也是单独铆在络头上的一块。所以这一条把它算进期望，而不是逼造型去
    # 迁就一个数——但也不放任：多出第三片以外的任何一片，仍旧当场撞红。
    want = (len(bard_runs(fit, spec)) + bool(spec.crinet) + bool(spec.chamfron)
            + (2 if spec.chamfron == "full" and spec.full_wrap else 0))
    comps = connected_components(_Shim(els))
    # 判**上界**不判相等：这条要挡的是"某处该搭上的没搭上"，也就是**多**出来的片；
    # 少于期望说明该连的都连上了，那是好事不是缺陷（矮马的颊片就恰好和主体接上了）。
    if len(comps) > want:
        detail = " / ".join(f"{len(c)} 件({c[0]}…)" for c in comps[:4])
        parts = [f"身甲 {len(bard_runs(fit, spec))} 片"] + (["鸡颈"] if spec.crinet else []) \
            + (["面帘主体", "左右颊片"] if spec.chamfron == "full" and spec.full_wrap
               else ["面帘"] if spec.chamfron else [])
        bad.append(f"整副甲应是 {want} 片（{' / '.join(parts)}），实为 {len(comps)} 片：{detail}")
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
# 首版这里写的是 `bard_fore_lame` / `bard_rear_lame` —— 甲的件从来不叫这个名（身甲是
# `bard_lame_*`、垂缘是 `bard_skirt_lame_*`），两个键一次都没命中过。**判据挂着死键
# 比没有判据更坏**：看着在查身甲有没有飘开，其实只查了鸡颈。
MUST_HUG = {"saddle_pad": 0.0, "rein_line_neck": 0.9,
            "bard_lame": 0.0, "bard_skirt_lame": 0.0, "bard_crinet_lame": 0.0,
            # 布档的身甲不是札片是一整幅（`bard_fold_*`，见 `_drape_body`）。它同样把
            # 内侧面埋进马体，所以同样卡到实交——布飘起来离开马身和铁一样难看。
            "bard_fold": 0.0}


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
MIN_TIER = 40.0  # 相邻两档至少有一个角色要拉开这么远：五档摆在一起要认得出是五档
MIN_EDGE = 1.75  # 受光带 / 背光带的最小亮度**比值**（叠边那条线）
# 每种纹理的侧面最亮 / 最暗**比值**。上界和下界一样是判据：布要是也闪就成了锡箔。
FINISH_SPAN: dict[str, tuple[float, float]] = {
    "pit": (2.3, 6.0),  # 金属
    "lamellar": (2.3, 6.0),  # 札片：跨度与金属同量级，但只由三条结构线担
    "scale": (2.0, 5.0),  # 鱼鳞：行更密、每行更平，跨度比札片略小
    "ring": (1.7, 3.4),  # 锁环
    "weave": (1.15, 1.55),  # 布
    "drape": (1.15, 1.60),  # 挂着的布：竖折。跨度仍是布的量级，只是结构线转了九十度
    "quilt": (1.15, 1.60),  # 绗缝的衬里
    "twist": (1.25, 1.90),  # 麻绳
    "mottle": (1.20, 2.10),  # 革
    "flat": (1.0, 1.01),  # 灵纹
}


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
    再查**相邻两档的主色**：五档甲摆在一起要认得出是五档，不是五块差不多的灰。

    量的是**实际刷出来的侧面均色**（`mat_rgb`）不是表里的名义值：名义值是"打算刷成
    什么"，刷出来才是"远处看见什么"。现在两者只差一两个数（纹理归一过），但判据盯着
    名义值的话，往后谁把纹理改偏了它就成了睁眼瞎。

    RGB 欧氏距离只是个粗糙代理，够用：这里要挡的是"几乎同色"，不是排颜色的名次。
    """
    from gen_pelt import COATS

    bad = []
    for kind, K in KINDS.items():
        for tk, spec in K.table.items():
            # `mat_nail` 是蹄铁的钉头 —— 首版角色表漏了它，而钉头是蹄铁上除铁条外
            # 唯一露在外面的东西，钉头糊进蹄色等于整排钉子消失。
            # 「露在外面的那一片」才是这条要管的东西，所以角色表跟着造型走：重铁甲的
            # 绛红衬里（`pad`）与骨白包边（`edge`）都是远处第一眼读到的颜色，漏了它们
            # 等于这条判据只看了甲的一半。
            for role in ("mat", "mat_dark", "mat_trim", "mat_nail", "edge", "pad",
                         "mat_plate", "mat_plate_dark", "boss", "mat_field", "pendant"):
                m = getattr(spec, role, None)
                if not m:
                    continue
                for coat in COATS.values():
                    for key in K.against:
                        d = _rgbd(mat_rgb(m), coat.mats[key])
                        if d < K.min_contrast:
                            bad.append(f"{kind}/{tk} 的 {role}={m} 与「{coat.label}」的 {key} 只差 "
                                       f"{d:.1f}（下限 {K.min_contrast:.0f}），远处看不出穿没穿")
            d = _rgbd(mat_rgb(spec.mat), mat_rgb(spec.mat_dark))
            if 0.0 < d < MIN_SHADE:
                bad.append(f"{kind}/{tk} 的主色与暗部只差 {d:.1f}（下限 {MIN_SHADE:.0f}），明暗层次白做")
        # 相邻两档要在**某一个角色**上拉开，不是每个角色都拉开：灵铁鞍与粗革鞍同样是
        # 一副革鞍，差的是配件（`mat_trim` 革扣 → 灵铁），座面本来就该是同一块革。
        # 卡"主色必须不同"会逼出一副蓝座面的鞍——那是判据在替美术拍板。
        tiers = list(K.table.values())
        for a, b in zip(tiers, tiers[1:]):
            best = max((_rgbd(mat_rgb(getattr(a, r)), mat_rgb(getattr(b, r)))
                        for r in ("mat", "mat_dark", "mat_trim", "mat_nail", "edge", "pad",
                                  "mat_plate", "mat_plate_dark", "boss", "mat_field", "pendant")
                        if getattr(a, r, None) and getattr(b, r, None)), default=0.0)
            if best < MIN_TIER:
                bad.append(f"{kind} 的「{a.label}」与「{b.label}」哪个角色都只差 {best:.1f}"
                           f"（下限 {MIN_TIER:.0f}），相邻两档在远处分不开")
    return bad


def check_escalation() -> list[str]:
    """具装是**一件一件配齐**的：`BARD_KIT` 逐档只增不减，而且每一档都得真多配一件。

    表里改一个 bool 就可能让顶档比次档少一件——两档的图差得远，看图看不出来少了哪件。
    这条查的是**声明**（规格表），逐档覆盖率查的是**做出来的东西**，两条各管一头。
    """
    bad = []
    tiers = list(BARDS.values())
    for a, b in zip(tiers, tiers[1:]):
        lost = [n for n, f in BARD_KIT if f(a) and not f(b)]
        if lost:
            bad.append(f"马甲「{b.label}」比「{a.label}」少了 {'、'.join(lost)}——具装只该越配越全")
        if not any(f(b) and not f(a) for _n, f in BARD_KIT):
            bad.append(f"马甲「{b.label}」相对「{a.label}」没多配任何一件具装")
    return bad


def check_finish() -> list[str]:
    """金属要有跨度、布不许有跨度（说明见 `material.check_finishes`）。"""
    return check_finishes({k: m.rgb for k, m in TACK_MATS.items()},
                          lambda k: FINISHES[TACK_MATS[k].finish], FINISH_SPAN, MIN_EDGE)


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

    for msg in check_finish() + check_contrast() + check_escalation():
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
                    coverage(t, fit) if kind == "bard" else 0.0,
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
                for (va, ca, ka), (vb, cb, kb) in zip(a, b):
                    if not cb - ca:
                        print(f"    ✗ {order[i + 1]} 相对 {order[i]} 没有任何新部件类型，分档看不出来")
                        rc = 1
                    if vb <= va:
                        print(f"    ✗ {order[i + 1]} 的用料没比 {order[i]} 多（{va:.1f} → {vb:.1f}）")
                        rc = 1
                    # 覆盖范围要**逐档真的变大**。"多一件部件"和"多盖住一片"是两回事：
                    # 把当胸做大一圈、下摆放长两寸，部件表一点不动，覆盖率立刻动；反过来
                    # 加一面寄生，部件多了一件而覆盖率几乎不变（它本来就不是护具）。
                    # 两条都要，只留一条就漏掉另一半。
                    if kind == "bard" and kb - ka < MIN_COVER_STEP:
                        print(f"    ✗ {order[i + 1]} 盖住的马身没比 {order[i]} 多多少"
                              f"（{ka * 100:.1f}% → {kb * 100:.1f}%，至少 +{MIN_COVER_STEP * 100:.1f}pp）")
                        rc = 1
                if kind == "bard" and i + 2 == len(order) and b:
                    worst = min(k for *_r, k in b)
                    if worst < MIN_COVER_TOP:
                        print(f"    ✗ 顶档只盖住马身侧面的 {worst * 100:.1f}%"
                              f"（下限 {MIN_COVER_TOP * 100:.0f}%）")
                        rc = 1
    return rc


if __name__ == "__main__":
    raise SystemExit(main())
