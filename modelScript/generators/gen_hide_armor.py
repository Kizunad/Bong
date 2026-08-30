#!/usr/bin/env python3
"""生成兽皮甲四件 bbmodel、64x64 UV 贴图与真实三视图预览。

配方（server/assets/craft/recipes/workbench/armor.toml）只有熟皮 + 草绳两种料，
所以整套甲的几何只准由两类构件组成：**厚熟皮板** 和 **草绳绑扎**。没有金属、
没有铆钉、没有骨片——参考图里出现的东西也一样按这条筛。

形体依据 local_images/equip_ref/2_armor_hide_helmet.png 等参考三视图；参考图的
灰模特腿长不合 MC 比例，故一律以**头宽 = 8 单位**标定，只取各构件相对头的比例。
参考图把颅盖画成悬在头顶约 4 单位的浮空板（AI 的立体错误），这里按实物落座。

运行时真相是 client 的 ArmorPartModel.CUBE_TABLES，本文件的 --emit-java
直接吐那张表的 Java 字面量，避免人工转抄漂移。
"""

from __future__ import annotations

import argparse
import random
from pathlib import Path

from PIL import Image, ImageDraw

# --- modelScript 路径引导：共用底座在 core/ ---
import sys as _sys
from pathlib import Path as _Path
_sys.path.insert(0, str(_Path(__file__).resolve().parents[1] / "core"))
from bbmodel_maker.model.armor_model_common import ArmorPart, Cube, TEXTURE_SIZE, write_material_assets

REPO = Path(__file__).resolve().parents[2]
LOCAL_MODELS = Path(__file__).resolve().parents[1] / "models"
PREVIEW_ROOT = Path(__file__).resolve().parents[1] / "out"

# 默认贴图落在 modelScript/models（gitignored）——现在只做模型不接客户端，往
# client 资源树里丢孤儿贴图会改资源包 sha1 把 CI 打红。接线那轮跑 --install
# 换成下面的 CLIENT_TEXTURE_ROOT。
MATERIAL = "hide"
DRAFT_TEXTURE_ROOT = LOCAL_MODELS / "armor" / MATERIAL / "textures"
CLIENT_TEXTURE_ROOT = (
    REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "armor"
)

# 贴图四象限：主料 / 深色油润料 / 带缝线的皮 / 草绳。
# 取 uv 原点时必须保证 2*(sx+sz) 不越出本象限宽度，否则一个 cube 会横跨两种材质。
#
# round 4 把"日晒褪色皮"换成 UV_HIDE_LACED。原因见 _helmet_cords 的注释：护耳和
# 后帘上那些 0.35 粗的绑扎绳是**几何做错了**，参考图里它们的直径约 0.2 单位，
# 体素做不出来，只会糊成一条抢眼的亮黄杠。改画在贴图上，几何删掉。
UV_HIDE = (0, 0)
UV_HIDE_DARK = (32, 0)
UV_HIDE_LACED = (0, 32)
UV_CORD = (32, 32)

# 补丁是两小块**独立色料**，不能落在任何大料象限里随机采样，得各自占一小块专用地。
# 借深色象限 y≥18 那段空地开两个 10x6 的保留格（头盔的深色件全挤在 y<6，胸甲的
# 下摆件占到 y<2，都够不着这里）。这两块由 _assert_patch_regions 每次生成时核验，
# 不靠"应该没人用"的默认。box-uv 的实际占用是 2*(sx+sz) 宽、sy+sz 高。
UV_PATCH_PALE = (34, 18)
UV_PATCH_RED = (46, 18)
PATCH_REGIONS = ((UV_PATCH_PALE, 10, 6), (UV_PATCH_RED, 10, 6))


def c(mount, name, origin, size, uv=UV_HIDE) -> Cube:
    return Cube(mount, name, origin, size, uv)


# ─── 兽皮甲盔 ──────────────────────────────────────────────────────────────
# 头盒 x∈[-4,4] y∈[24,32] z∈[-4,4]。四个子装配：颅盖 / 眉箍 / 护耳 / 后颈帘，
# 外加把它们绑在一起的草绳。


def _helmet_crown() -> tuple[Cube, ...]:
    """硬熟皮颅盖：一整张对折皮盖住头顶，后半片略低略宽，压在后脑上。

    厚度取 1.6 而非 1.3——round 1 的薄盖在正视里被眉箍吃掉，只剩一线，
    读不出"一张硬皮对折扣在头上"。前缘另立一道折边，是对折的那道棱。
    """
    return (
        # round 3 这两片走 V=60% 的"日晒褪色"象限，在 MC 顶面 1.0 亮度下糊成一条
        # 发白的带子，和参考图里同色的整张硬皮完全对不上。改回主皮。
        # 前缘必须探过眉箍（-5.2）而不是缩在它后面。round 4 起点在 -4.15，比眉箍
        # 缩进 1.05，侧视读成"屋檐倒挂"——参考图是顶上这张皮向前探出、眉箍缩在
        # 它下面。戴在玩家头上才看出来，裸甲三视图里根本读不出前后关系。
        c("HEAD", "crown_front", (-4.5, 32.0, -5.6), (9.0, 1.6, 5.7)),
        c("HEAD", "crown_rear", (-4.7, 31.55, 0.1), (9.4, 1.6, 4.55)),
        # 折边是颅盖前缘**向下卷**的那道棱，得挂在前沿外侧。round 2 架在 33.25
        # 以上只蹭到 0.35 格、round 4 又压在顶面上，两次都成了颅盖上面多叠一层台阶。
        c("HEAD", "crown_fold_lip", (-4.7, 31.8, -5.75), (9.4, 1.3, 0.9), UV_HIDE_DARK),
    )


def _helmet_brow() -> tuple[Cube, ...]:
    """眉箍：对折加厚的皮卷，整顶最宽的一圈，两端绕到太阳穴后收住。"""
    return (
        c("HEAD", "brow_roll_front", (-5.4, 30.3, -5.2), (10.8, 2.15, 1.4), UV_HIDE_DARK),
        c("HEAD", "brow_wrap_left", (-5.4, 30.3, -3.8), (1.4, 2.15, 2.9), UV_HIDE_DARK),
        c("HEAD", "brow_wrap_right", (4.0, 30.3, -3.8), (1.4, 2.15, 2.9), UV_HIDE_DARK),
    )


def _helmet_ear_flaps() -> tuple[Cube, ...]:
    """护耳：两片硬皮从眉箍下沿垂到肩线，下缘收窄成尖。

    刻意比眉箍窄（±5.25 对 ±5.4）——参考图里最宽的一圈是眉箍，护耳缩在它里面。
    """
    return (
        # 上沿顶到 31.6 与颅盖后片相接。round 2 收在 30.35，太阳穴上方留了
        # 1.2 格缝，侧视直接漏出灰模特。
        # 前缘顶到 -4.3（脸在 -4）。round 4 收在 -3.6，侧视里颧骨前面漏出一条脸。
        c("HEAD", "ear_flap_left", (-5.25, 24.3, -4.3), (1.2, 7.3, 5.9), UV_HIDE_LACED),
        c("HEAD", "ear_flap_right", (4.05, 24.3, -4.3), (1.2, 7.3, 5.9), UV_HIDE_LACED),
        c("HEAD", "ear_tip_left", (-5.05, 23.15, -3.3), (0.95, 1.2, 3.4), UV_HIDE_LACED),
        c("HEAD", "ear_tip_right", (4.1, 23.15, -3.3), (0.95, 1.2, 3.4), UV_HIDE_LACED),
    )


def _helmet_side_skirt() -> tuple[Cube, ...]:
    """后侧裙片：把护耳后沿和后颈帘接上。

    round 1 缺这两片，侧视里后颈帘成了悬在脑后的一块独立板，中间豁着 2.4 格空气；
    参考图那是一整张连续硬皮从眉箍绕到后摆。这两片补的就是那段连续性。
    """
    return (
        # 下沿收到 23.9：24.6 时侧后方在肩线上留一条 0.6 格的脖子皮肤，
        # 戴到玩家头上才看得见（灰模特那条缝和灰身体同色）。
        c("HEAD", "side_skirt_left", (-5.05, 23.9, 1.6), (1.0, 7.65, 2.5), UV_HIDE_LACED),
        c("HEAD", "side_skirt_right", (4.05, 23.9, 1.6), (1.0, 7.65, 2.5), UV_HIDE_LACED),
    )


def _helmet_curtain() -> tuple[Cube, ...]:
    """后颈皮帘：从颅盖后缘垂下，盖住后颈，下摆是切口毛边。

    前面贴到 z=3.95（后脑在 z=4）而不是 round 1 的 4.05——差这 0.1 就会在侧视
    露出一条缝，读成"没贴住"。
    """
    return (
        c("HEAD", "curtain_upper", (-4.6, 27.3, 3.95), (9.2, 4.35, 1.1), UV_HIDE_LACED),
        c("HEAD", "curtain_lower", (-4.25, 23.0, 4.05), (8.5, 4.3, 0.95)),
        c("HEAD", "curtain_hem", (-4.0, 22.35, 4.1), (8.0, 0.7, 0.85), UV_HIDE_DARK),
    )


def _helmet_cords() -> tuple[Cube, ...]:
    """草绳：只剩眉箍那一圈闭环 + 结与垂头。绑扎缝线全部转贴图。

    round 3 在护耳、后帘上另做了 9 条 0.35 粗的几何绑扎绳，是**建模层面的错**：
    参考图里那些绑扎的直径约 0.2 单位，体素做不出来，做粗到能显形就变成横贯整
    面的亮黄杠。实测亮黄在画面里占 11.8%/6.3%/14.0%（正/侧/背），参考图只有
    1.4%/2.7%/1.8%——差 5~8 倍，整顶就从"皮的"读成"草编的"。绑扎归贴图
    （UV_HIDE_LACED 的竖排 X 缝），几何只留承重的那一圈。

    绳径同时由 0.4 收到 0.3：这圈要读成"绳"而不是"皮条"。
    两侧的结/垂头刻意不对称——手系的东西不会左右一样长。
    """
    return (
        # 闭环：绕过眉箍前沿 → 两侧太阳穴 → 后脑打结。开口绳系不住任何东西。
        c("HEAD", "brow_cord_front", (-5.5, 31.15, -5.3), (11.0, 0.3, 0.3), UV_CORD),
        c("HEAD", "brow_cord_left", (-5.5, 31.15, -3.95), (0.3, 0.3, 7.95), UV_CORD),
        c("HEAD", "brow_cord_right", (5.2, 31.15, -3.95), (0.3, 0.3, 7.95), UV_CORD),
        c("HEAD", "brow_cord_back", (-4.55, 31.15, 5.05), (9.1, 0.3, 0.3), UV_CORD),
        c("HEAD", "brow_knot_back", (-0.6, 30.7, 5.3), (1.2, 1.0, 0.35), UV_CORD),
        c("HEAD", "brow_tail_back", (-0.2, 29.3, 5.35), (0.35, 1.4, 0.3), UV_CORD),
        c("HEAD", "brow_knot_left", (-2.85, 30.75, -5.45), (1.0, 1.0, 0.35), UV_CORD),
        c("HEAD", "brow_knot_right", (1.85, 30.75, -5.45), (1.0, 1.0, 0.35), UV_CORD),
        c("HEAD", "brow_tail_left", (-5.6, 29.15, -4.9), (0.3, 1.9, 0.3), UV_CORD),
        c("HEAD", "brow_tail_right", (5.3, 29.5, -4.9), (0.3, 1.5, 0.3), UV_CORD),
    )


def part_helmet() -> ArmorPart:
    return ArmorPart(
        "hide_helmet",
        "HIDE HELMET",
        _helmet_crown()
        + _helmet_brow()
        + _helmet_ear_flaps()
        + _helmet_side_skirt()
        + _helmet_curtain()
        + _helmet_cords(),
    )


# ─── 兽皮甲胸甲 ────────────────────────────────────────────────────────────
# 躯干盒 x∈[-4,4] y∈[12,24] z∈[-2,2]，手臂 x∈[±4,±8]。五个子装配：
# 甲衣壳 / 肩片 / 补丁 / 侧绑绳 / 腰绳。
#
# 尺寸全部从 local_images/equip_ref/3_armor_hide_chestplate.png 量出来：先用模特
# 自己的双臂外缘（=±8）和腿宽（=8）把 AI 模特标定到 MC 比例（它的头偏小、腿偏长，
# 只有上半身可信），再叠单位网格逐件读边界。三视图读数一致的量：
#   衣身 x=±4.2 / 下摆 y≈12.4 / 前领口 y≈22.9 宽约 4.5 / 后领口平齐 y≈24.7
#   肩片 x 从 ±4.2 探到 ±8.0，顶边由内 24.3 斜到外 23.0，下缘扇贝边 y≈20.0~21.2
#   侧绑绳每侧 7 圈，y 13.2~20.4；腰绳 y 12.6~13.2，前中打结垂两条
#
# **Mount 集合没有手臂**（ArmorPartModel.Mount 只有 HEAD/BODY/两腿/两脚），肩片
# 只能挂 BODY，摆手时不跟着转。所以下缘刻意收在 y=20.95、前后面留到 z=±2.6：
# 实算过手臂绕 (±5,22,0) 摆 30° 时臂前缘最远到 z≈-2.48，不穿帮；40° 冲刺才漏
# 约 0.3 格。参考图那种垂到 20.0 的挂法在动起来时会被手臂穿透，故意收短了 1 格。


def _chest_shell() -> tuple[Cube, ...]:
    """甲衣壳：前后两大片 + 两侧窄片 + 两条过肩。

    前片顶边收在 22.8 留出领口豁子（参考图那道 4.5 宽的圆领），后片直接顶到 24.0。
    后领做不了参考图里高出肩线的那圈——y>24 且 |x|<4 全埋在脑袋方块里，做了也看不见。
    """
    return (
        c("BODY", "shell_front", (-4.35, 12.7, -2.75), (8.7, 10.1, 0.8)),
        c("BODY", "shell_back", (-4.35, 12.7, 1.95), (8.7, 11.3, 0.8)),
        c("BODY", "shell_side_left", (-4.5, 12.3, -2.35), (0.6, 11.1, 4.7)),
        c("BODY", "shell_side_right", (3.9, 12.3, -2.35), (0.6, 11.1, 4.7)),
        # 过肩：把前片顶边接到后片，同时盖住肩头。两条之间就是领口。
        # 顶面 23.92 / 背面 2.68 都刻意错开后片的 24.0 / 2.75：两片背面同在
        # z=2.75 会共面打架，后视肩部渲出两块逐像素闪烁的噪点（round 2 实测）。
        c("BODY", "shell_yoke_left", (-4.4, 22.8, -2.8), (2.1, 1.12, 5.48)),
        c("BODY", "shell_yoke_right", (2.3, 22.8, -2.8), (2.1, 1.12, 5.48)),
    )


def _chest_hem() -> tuple[Cube, ...]:
    """下摆：腰绳底下那道外翻的厚边，比衣身探出一点，用深色油润皮。"""
    return (
        # 下沿压到 12.05、内侧收到贴着躯干面（-1.98 / 2.02）：逐面采样查出前后
        # 下摆各漏一条 0.05~0.1 的发丝缝，会在胯部露出一线里衣。
        c("BODY", "hem_front", (-4.45, 12.05, -2.92), (8.9, 0.9, 0.94), UV_HIDE_DARK),
        c("BODY", "hem_back", (-4.45, 12.05, 1.98), (8.9, 0.9, 0.94), UV_HIDE_DARK),
    )


def _chest_caps() -> tuple[Cube, ...]:
    """肩片：顶盖（必须压住整个臂顶）+ 三段下垂裙 + 下缘扇贝齿。

    **顶盖不是装饰，是必需件**。round 3 照参考图把顶边做成 24.3→23.8→23.1 的
    斜肩线，结果只有最内 1.45 格高过臂顶；MC 的手臂是个硬盒子，顶面死死卡在
    y=24，斜下去的那 2.4 格底下，臂顶（连着袖子贴图）整片露在甲外面——正视
    看不出来，一俯视就是两块布顶在肩上。参考图的灰模特肩是圆的，没有这个平面，
    照抄它的剖面必然漏。

    所以顶盖 cap_crown/cap_ridge 横跨 x 3.95~8.15、z=±2.65 把臂顶整个盖死，
    底边压到 23.8（探进臂顶 0.2，避免和 y=24 那个面共面打架）。参考图那条
    下垂肩线只能保留 0.4 格的落差（24.55→24.15）——再低就重新漏臂顶，
    真正的"垂"交给外侧那段悬在手臂外面的裙来表现。
    """
    cubes: list[Cube] = []
    for side, sign in (("left", -1.0), ("right", 1.0)):
        def x(inner: float, width: float) -> float:
            """把"离身体多远"翻成绝对 x：右侧直接加，左侧要减去宽度。"""
            return inner if sign > 0 else -inner - width

        cubes.extend(
            (
                # 顶盖两段：内高外低，越过臂顶 (y=24) 把 x 3.95~8.15 全压住。
                c("BODY", f"cap_crown_{side}", (x(3.95, 1.85), 23.8, -2.65), (1.85, 0.75, 5.3)),
                c("BODY", f"cap_ridge_{side}", (x(5.8, 2.35), 23.8, -2.6), (2.35, 0.35, 5.2)),
                # 下垂裙三段：底边交错（21.1 / 20.55 / 20.95），正视下缘才不平。
                # 最外一段探到 8.2 挂在手臂外侧面之外，参考图那种外垂感靠它。
                c("BODY", f"cap_skirt_in_{side}", (x(4.15, 1.55), 21.1, -2.6), (1.55, 2.75, 5.2)),
                c("BODY", f"cap_skirt_mid_{side}", (x(5.7, 1.3), 20.55, -2.55), (1.3, 3.3, 5.1)),
                # 外段用主皮而不是深色料：round 2 让外三分之一整段发黑，肩片从
                # 一片连续的皮读成"深浅两块拼的"，参考图那是一整张。深色只留给齿。
                c("BODY", f"cap_skirt_out_{side}", (x(7.0, 1.2), 20.95, -2.4), (1.2, 2.9, 4.8)),
                # round 1 的齿只掉 0.7、齿间只留 0.8，渲出来是一条直硬边——齿的
                # 落差得大于它自己的宽度才读得成"咬出来的口子"。三颗齿的 x 外端
                # 也各不同（8.05 / 7.35 / 7.85），肩片外缘跟着变毛。
                c("BODY", f"cap_tooth_{side}_front", (x(4.2, 3.85), 19.95, -2.45),
                  (3.85, 1.3, 1.1), UV_HIDE_DARK),
                c("BODY", f"cap_tooth_{side}_mid", (x(4.55, 2.8), 19.8, -0.55),
                  (2.8, 1.45, 1.1), UV_HIDE_DARK),
                c("BODY", f"cap_tooth_{side}_back", (x(4.2, 3.65), 20.1, 1.35),
                  (3.65, 1.15, 1.1), UV_HIDE_DARK),
            )
        )
    return tuple(cubes)


def _chest_patches() -> tuple[Cube, ...]:
    """两块缝上去的补丁：左胸一块漂白的旧皮，腹前一块血锈色的。

    厚 0.25 且比前片再探出 0.15——补丁是缝在外面的，压进去就读成"印上去的花纹"。
    四边的手缝画在贴图上（1 texel 粗，见 make_texture 里的教训）。
    """
    return (
        # x 取参考图的镜像：渲染（和 MC 正视）里 -x 落在观者右手边，要让浅补丁
        # 像参考图那样出现在观者左边，坐标就得取正。
        c("BODY", "patch_pale", (-0.6, 18.4, -2.9), (3.5, 3.4, 0.25), UV_PATCH_PALE),
        c("BODY", "patch_red", (-2.6, 14.3, -2.9), (3.0, 3.3, 0.25), UV_PATCH_RED),
        # 锁边线做成几何而不是贴图：补丁正面只摊到 3~4 个 texel，画在贴图上一
        # 针就占满一格（round 1 实测糊成 4x4 棋盘格，和头盔那轮一模一样的错）。
        # 每块只钉三针、位置不对称——手缝上去的东西不会四边等距。
        # 浅补丁上的线走**深色皮**：草绳色和漂白皮几乎同明度，缝在上面等于没缝
        # （round 2 实测三根线全隐形）。红补丁底色够暗，草绳色就够跳。
        c("BODY", "patch_pale_stitch_top", (-0.35, 21.55, -3.05), (1.5, 0.28, 0.3), UV_HIDE_DARK),
        c("BODY", "patch_pale_stitch_low", (1.05, 18.3, -3.05), (1.6, 0.28, 0.3), UV_HIDE_DARK),
        c("BODY", "patch_pale_stitch_side", (2.75, 19.1, -3.05), (0.28, 1.7, 0.3), UV_HIDE_DARK),
        c("BODY", "patch_red_stitch_top", (-2.05, 17.45, -3.05), (1.4, 0.26, 0.3), UV_CORD),
        c("BODY", "patch_red_stitch_low", (-1.6, 14.2, -3.05), (1.5, 0.26, 0.3), UV_CORD),
        c("BODY", "patch_red_stitch_side", (-2.7, 15.0, -3.05), (0.26, 1.55, 0.3), UV_CORD),
    )


def _chest_lacing() -> tuple[Cube, ...]:
    """侧绑绳：前后两片靠这排草绳勒在一起，每侧 6 圈横着绕过侧缝。

    每圈一根 cube 横贯 z=-2.65~2.65：中段藏在手臂后面，只在前后两端露头——
    参考图侧视里那两列绳疙瘩就是这么来的，正视里则是衣身边缘的一串小突起。
    参考图每侧 7 圈（间距 1.15），这里收到 6 圈（间距 1.3）：MC 尺度下 1.15 的
    间距会让绳和绳的阴影糊成一条连续亮带。
    """
    # round 1 用等差 1.3 排了 6 圈，渲出来是一排一模一样的横杠，读成木箱的箍
    # 而不是手勒的绳。这里改成手记的不等距，两侧还错开半格——同一根绳绕两圈
    # 落点不可能左右对齐。
    left_rows = (13.7, 15.15, 16.4, 17.85, 19.05, 20.35)
    right_rows = (13.9, 15.0, 16.55, 17.7, 19.2, 20.2)
    cubes: list[Cube] = []
    for index, (yl, yr) in enumerate(zip(left_rows, right_rows)):
        # 绳圈要伸到 z=±2.92（越过前片的 -2.75）才算真绕过了缝：round 2 收在
        # ±2.65 时绳头缩在前片背后，正视只剩边上一串 0.5x0.42 的小点，参考图那
        # 一列鼓出来的绳结完全没了。粗细同时提到 0.55——0.42 在 MC 尺度约 8px，
        # 和衣身的裂纹一个量级，读不出是绳。
        thick = 0.52 + 0.06 * (index % 3)
        cubes.append(c("BODY", f"lace_left_{index}", (-4.95, yl, -2.92),
                       (0.55, thick, 5.84), UV_CORD))
        cubes.append(c("BODY", f"lace_right_{index}", (4.4, yr, -2.92),
                       (0.55, 0.58 - 0.05 * (index % 3), 5.84), UV_CORD))
    return tuple(cubes)


def _chest_belt() -> tuple[Cube, ...]:
    """腰绳：闭合一圈 + 前中打结垂两条。

    闭环理由同头盔——开口的绳勒不住任何东西。两条垂头刻意不等长；下段再往前
    推到 z=-3.45，是为了给迈腿让路（腿绕 (±2,12,0) 摆 30° 时膝上前缘到 z≈-2.4）。
    背面只留一个结疙瘩不垂头：参考图背视也画了垂头，那是它把正面镜像过去了。
    """
    return (
        # 绳径 0.6~0.7：参考图这根是整件里最粗的一根料，比侧绑绳明显壮一圈，
        # round 2 做成 0.5x0.6 后和侧绑绳同粗，腰线就散了。
        c("BODY", "belt_front", (-4.6, 12.5, -3.0), (9.2, 0.7, 0.6), UV_CORD),
        c("BODY", "belt_back", (-4.6, 12.5, 2.4), (9.2, 0.7, 0.6), UV_CORD),
        c("BODY", "belt_left", (-5.0, 12.5, -2.4), (0.55, 0.7, 4.8), UV_CORD),
        c("BODY", "belt_right", (4.45, 12.5, -2.4), (0.55, 0.7, 4.8), UV_CORD),
        c("BODY", "belt_knot", (-0.95, 12.1, -3.25), (1.9, 1.35, 0.7), UV_CORD),
        # 两条垂头间距拉到 0.86、加粗到 0.46：round 2 挨得太近，正视里糊成一根。
        c("BODY", "belt_tail_left_upper", (-0.62, 10.6, -3.3), (0.46, 1.65, 0.42), UV_CORD),
        c("BODY", "belt_tail_left_lower", (-0.72, 9.15, -3.5), (0.42, 1.6, 0.4), UV_CORD),
        c("BODY", "belt_tail_right", (0.24, 10.5, -3.3), (0.46, 1.7, 0.42), UV_CORD),
        c("BODY", "belt_knot_back", (-0.7, 12.25, 3.0), (1.4, 1.05, 0.55), UV_CORD),
    )


def part_chestplate() -> ArmorPart:
    return ArmorPart(
        "hide_chestplate",
        "HIDE CHESTPLATE",
        _chest_shell()
        + _chest_hem()
        + _chest_caps()
        + _chest_patches()
        + _chest_lacing()
        + _chest_belt(),
    )


# ─── 兽皮甲腿甲 ────────────────────────────────────────────────────────────
# 腿盒（原版 biped 局部坐标）x∈[-2,2] y∈[0,12] z∈[-2,2]，mount 自带 ±1.9 的
# 骨骼枢轴偏移，世界坐标是 local_x ± 1.9。y 按 ArmorPartModel 的约定保留
# Bedrock「脚在 0、向上为正」，Java 侧用 pivot 12 换算成 vanilla cuboid y。
#
# 参考 local_images/equip_ref/4_armor_hide_leggings.png。这件是**护腿不是裤子**：
# 髋带整圈裹住，腿上只有正面一块夹板（连前侧两翼），**背面整个敞开**，靠两条
# 垂下的皮绦和一组交叉绳兜住。三视图读数（横向按躯干宽=8 定标 26.1px/单位；
# 纵向那具模特腿比 MC 长一倍，只能按「占腿长的比例」映射，髋线=12、甲底=3.2）：
#   髋带 y 12→10、x=±4.5、两端各两个 X 绑扎
#   腿夹板 y 10→3.3，宽 4.2，只覆盖 z -3.0~-1.0（正面 + 前侧角）
#   膝盘 y 4.0~6.4、直径 3.4、向前凸出约 0.6
#   背面：每腿两条皮绦 + 一组横跨两腿的绳 X
#
# **两处对参考图的硬性偏离，都源于 mount 集合没有 BODY 可用（mountsForSlot(LEGS)
# 只给 LEFT_LEG / RIGHT_LEG）**：
#  1. 髋带只能劈成两半各挂一条腿，迈步时会在裆缝处错开（1.9 格臂距 × 25° ≈ 1.6 格
#     相对错动）。原版皮裤把髋部挂在 body 上就没这问题，这里没这个选项，只能把
#     髋带高度压到 1.9 格、尽量贴近枢轴，让错动读成「软皮裹腰随步伐动」。
#  2. 参考图那组横跨两腿的绳 X 做不了——绳的两半分属两条腿，一迈步就撕开。改成
#     每腿两根错位横杠（合起来仍读成一个扁 X），迈步时各自跟着自己的腿走。


def _leg_cubes(mount: str, sign: float) -> tuple[Cube, ...]:
    """一条腿的全部构件。sign=+1 左腿（局部 +x 朝外），-1 右腿（局部 -x 朝外）。

    件名带 `_left` / `_right` **后缀**而不是前缀：validate_part 要求整件内唯一，
    而 OnPlayer 合模按名字第一段分组，用后缀才能分出 band / splint / knee / strap
    这几个子装配，用前缀会退化成「左腿 / 右腿」两个大袋子。
    """
    side = "left" if sign > 0 else "right"

    def x(inner: float, width: float) -> float:
        """把「朝外为正」的局部坐标翻成该腿真正的 origin.x。"""
        return inner if sign > 0 else -inner - width

    def c2(name: str, origin, size, uv=UV_HIDE) -> Cube:
        return Cube(mount, f"{name}_{side}", origin, size, uv)

    # 背面绳/绦整族给右腿加 0.07 的 z 偏移：交叉绳按设计要越过中线，两条腿的绳
    # 段在裆缝处必然重叠，同深度就会共面打架。错开之后正好读成"一根绳压在另一根
    # 前面"——真绳交叉本来就是这样。实心甲片则改成在世界 x=0 处**恰好相接**
    # （髋带）或**留 0.5 沟槽**（腿夹板），不靠重叠。
    zoff = 0.0 if sign > 0 else 0.07

    return (
        # ── 髋带：整圈裹住髋部，顶边塞进躯干底下 0.25 格藏接缝 ──
        # 内缘收在局部 -1.9 = 世界 0.0：两半恰好相接不重叠。重叠会让两半的前后面
        # 共面，裆前渲出一片细网格（round 1 实测）。
        c2("band_front", (x(-1.9, 4.45), 10.35, -2.55), (4.45, 1.9, 0.8)),
        c2("band_back", (x(-1.9, 4.45), 10.35, 1.75), (4.45, 1.9, 0.8)),
        c2("band_side", (x(2.0, 0.6), 10.4, -2.5), (0.6, 1.8, 5.0)),
        # 下沿外翻的厚边，深色油润皮；比 band 再探出一点，内缘刻意不与它同面
        c2("band_lip_front", (x(-1.85, 4.5), 10.0, -2.72), (4.5, 0.65, 0.85), UV_HIDE_DARK),
        c2("band_lip_back", (x(-1.85, 4.5), 10.0, 1.87), (4.5, 0.65, 0.85), UV_HIDE_DARK),
        # 参考图髋带两端是两个粗草绳 X。轴对齐的 cube 做不出 X，改成三道横绕的
        # 绳圈——和胸甲侧绑同一套读法，间距不等避免读成箍。
        c2("band_lace_low", (x(2.35, 0.5), 10.62, -2.7), (0.5, 0.44, 5.4), UV_CORD),
        c2("band_lace_mid", (x(2.35, 0.5), 11.3, -2.7), (0.5, 0.4, 5.4), UV_CORD),
        c2("band_lace_top", (x(2.35, 0.5), 11.85, -2.7), (0.5, 0.44, 5.4), UV_CORD),

        # ── 腿夹板：正面一整块 + 前侧两翼，背面留空 ──
        # 内缘只到局部 -1.65（世界 0.25），两腿之间留 0.5 的沟槽。round 1 两块板
        # 在中线相接，正视里两条腿糊成一整块板，读不出是"护腿"。
        c2("splint_front", (x(-1.65, 3.95), 3.3, -2.65), (3.95, 6.85, 0.85)),
        c2("splint_wing_out", (x(1.7, 0.65), 3.45, -2.55), (0.65, 6.6, 1.85)),
        c2("splint_wing_in", (x(-1.68, 0.63), 3.45, -2.55), (0.63, 6.6, 1.85)),
        c2("splint_hem", (x(-1.72, 4.12), 3.02, -2.74), (4.12, 0.6, 1.0), UV_HIDE_DARK),
        # 参考图正面那两排缝线（顶边横的一排 + 内外缝各一列竖的）在 1 texel≈1 单位
        # 下画不出「一针一针」，只能取它的**功能**：把夹板系在髋带上、把两侧收拢。
        # 一道横的锁边 + 一道竖的外缝，读法和胸甲侧绑同族。
        c2("splint_lace_top", (x(-1.6, 3.85), 9.72, -2.76), (3.85, 0.42, 0.24), UV_CORD),
        c2("splint_lace_seam", (x(2.02, 0.38), 3.95, -2.78), (0.38, 5.75, 0.26), UV_CORD),

        # ── 膝盘：三段叠成八边形轮廓，1 texel≈1 单位下这已是「圆」的上限。
        # 上下两段比中段各窄 0.55，轮廓才收得出来；round 1 只窄 0.45，渲成方补丁。
        c2("knee_core", (x(-1.3, 3.25), 4.4, -3.22), (3.25, 1.7, 0.75), UV_HIDE_DARK),
        c2("knee_top", (x(-0.7, 2.05), 6.1, -3.14), (2.05, 0.55, 0.65), UV_HIDE_DARK),
        c2("knee_bottom", (x(-0.7, 2.05), 3.85, -3.14), (2.05, 0.55, 0.65), UV_HIDE_DARK),

        # ── 背面：两条垂下的皮绦 + 一组阶梯绳（两腿合起来读成 X）+ 结 ──
        c2("strap_out", (x(1.98, 0.34), 4.7, 1.98 + zoff), (0.34, 5.4, 0.34), UV_HIDE_DARK),
        c2("strap_in", (x(-1.9, 0.34), 5.3, 1.98 + zoff), (0.34, 4.8, 0.34), UV_HIDE_DARK),
        # 每条腿一个 ">"：上段从外上斜到内中，下段从内中斜回外下。两腿相对就是
        # 参考图那个 X。round 1 每段只用一根横杠，渲出来是两条毫无关系的横线；
        # 三级阶梯（每级 x 跨 1.4~1.6、y 落 0.5）才读得成一根斜着的绳。
        c2("cross_a0", (x(1.05, 1.40), 8.50, 2.04 + zoff), (1.40, 0.36, 0.36), UV_CORD),
        c2("cross_a1", (x(-0.35, 1.60), 8.00, 2.04 + zoff), (1.60, 0.36, 0.36), UV_CORD),
        c2("cross_a2", (x(-1.60, 1.45), 7.50, 2.04 + zoff), (1.45, 0.36, 0.36), UV_CORD),
        c2("cross_b0", (x(-1.60, 1.45), 6.85, 2.04 + zoff), (1.45, 0.36, 0.36), UV_CORD),
        c2("cross_b1", (x(-0.35, 1.60), 6.30, 2.04 + zoff), (1.60, 0.36, 0.36), UV_CORD),
        c2("cross_b2", (x(1.05, 1.40), 5.75, 2.04 + zoff), (1.40, 0.36, 0.36), UV_CORD),
        c2("cross_knot", (x(-1.8, 0.85), 6.98, 2.0 + zoff), (0.85, 0.68, 0.52), UV_CORD),
    )


def part_leggings() -> ArmorPart:
    return ArmorPart(
        "hide_leggings",
        "HIDE LEGGINGS",
        _leg_cubes("LEFT_LEG", 1.0) + _leg_cubes("RIGHT_LEG", -1.0),
    )


# ─── 兽皮甲靴 ──────────────────────────────────────────────────────────────
# 脚 mount 与腿共用骨骼枢轴（x=±1.9、y=12），坐标同样是「骨骼局部 x/z + Bedrock
# 绝对 y（脚底 0）」。参考 local_images/equip_ref/5_armor_hide_boots.png。
#
# 两处按 MC 比例强行改过的地方，都是参考图那具模特的锅：
#  1. **靴长**。参考图靴长 9.6 单位、宽 4.5（≈1:2.1，真实鞋的比例），可 MC 的脚
#     就是腿盒底面 4x4 的正方形，照抄会做出一双小丑鞋。这里趾头只前伸 2 格，
#     全长 6.85、宽 4.45（1:1.54），是「厚实的靴」而不是「船」。
#  2. **靴筒高**。参考图筒顶落在模特小腿 56% 处，但那具模特的腿被拉长了近一倍；
#     MC 的 0~12 是**整条腿**（含大腿），照 56% 会做成过膝靴。收到 y≈4.9，
#     读作踝到中胫。
#
# 另外绳箍刻意压到 y 2.6~3.1、蝴蝶结前伸到 z=-2.92：腿甲的夹板在 z=-2.65、下摆
# 在 -2.74，绳箍再高就被夹板挡死了，而结凸在夹板之前，套穿时仍看得见。
# 内侧边一律贴到局部 -1.9（世界 x=0）：两只靴在中线**恰好相接**，多 0.1 都会
# 让两只靴的同向外表面共面打架（腿甲那轮的教训）。


def _boot_cubes(mount: str, sign: float) -> tuple[Cube, ...]:
    """一只靴的全部构件。sign=+1 左脚（局部 +x 朝外），-1 右脚。"""
    side = "left" if sign > 0 else "right"

    def x(inner: float, width: float) -> float:
        return inner if sign > 0 else -inner - width

    def c2(name: str, origin, size, uv=UV_HIDE) -> Cube:
        return Cube(mount, f"{name}_{side}", origin, size, uv)

    return (
        # ── 靴筒：主筒 + 四片高低不一的豁口舌 ──
        c2("shaft", (x(-1.88, 4.23), 2.35, -2.35), (4.23, 3.0, 4.7)),
        # 参考图筒口是撕出来的毛边。四片舌顶高各不同（5.30/5.15/5.25/5.10），
        # 任何角度看过去都是不平的口，只错开一个轴的话另一个视角是平边。
        c2("shaft_tab_front", (x(-1.83, 4.11), 5.3, -2.3), (4.11, 0.72, 1.02)),
        c2("shaft_tab_back", (x(-1.79, 4.02), 5.26, 1.2), (4.02, 0.45, 1.12)),
        c2("shaft_tab_out", (x(1.3, 1.02), 5.29, -1.24), (1.02, 0.62, 2.4)),
        c2("shaft_tab_in", (x(-1.86, 0.62), 5.23, -1.24), (0.62, 0.42, 2.4)),

        # ── 脚面 + 趾头：趾头比脚面窄一档、低一档，尖端再收一次 ──
        c2("foot_body", (x(-1.87, 4.19), 0.75, -2.5), (4.19, 1.7, 4.9)),
        c2("toe_box", (x(-1.85, 3.88), 0.79, -3.78), (3.88, 1.46, 1.34)),
        c2("toe_tip", (x(-1.58, 3.5), 1.25, -4.45), (3.5, 1.05, 0.5)),
        # 鞋面正中那道缝：参考图从趾尖一路缝到脚背，是这双靴最显眼的做工痕迹
        c2("vamp_seam", (x(0.03, 0.4), 2.14, -4.35), (0.4, 0.26, 1.95), UV_HIDE_DARK),

        # ── 鞋底：两层，下层再宽一圈当沿条；深色油润皮 ──
        c2("sole_upper", (x(-1.88, 4.42), 0.36, -4.42), (4.42, 0.46, 7.06), UV_HIDE_DARK),
        # 底面压到 -0.25 而不是 0：脚底本身就在 y=0，鞋底停在 0 等于和脚底面共面
        # 打架，而且「鞋底在脚底之下」本来就是鞋的定义。原版皮靴同理（整条腿
        # 膨胀 1 格，底面也在 0 以下）。
        c2("sole_lower", (x(-1.9, 4.58), -0.25, -4.52), (4.58, 0.63, 7.28), UV_HIDE_DARK),
        c2("heel_block", (x(-1.84, 4.13), 0.84, 1.98), (4.13, 0.72, 0.7), UV_HIDE_DARK),
        c2("sole_toe_lift", (x(-1.55, 3.42), 0.5, -4.5), (3.42, 0.85, 0.74), UV_HIDE_DARK),

        # ── 外侧那块缝补丁（参考图侧视和背视都能看到）──
        c2("patch_side", (x(2.32, 0.3), 1.25, -1.15), (0.3, 1.35, 1.85), UV_HIDE_DARK),
        c2("patch_back", (x(-1.6, 2.4), 1.35, 2.42), (2.4, 1.25, 0.28), UV_HIDE_DARK),

        # ── 绳箍一圈 + 前面的蝴蝶结（两个耳 + 两条不等长的头）──
        c2("lace_front", (x(-1.83, 4.26), 2.6, -2.52), (4.26, 0.5, 0.44), UV_CORD),
        c2("lace_back", (x(-1.82, 4.24), 2.62, 2.08), (4.24, 0.5, 0.44), UV_CORD),
        c2("lace_out", (x(2.32, 0.44), 2.6, -2.1), (0.44, 0.5, 4.2), UV_CORD),
        c2("lace_in", (x(-1.81, 0.42), 2.6, -2.1), (0.42, 0.5, 4.2), UV_CORD),
        c2("bow_knot", (x(-0.05, 0.7), 2.72, -3.0), (0.7, 1.0, 0.58), UV_CORD),
        c2("bow_loop_out", (x(0.6, 1.5), 3.15, -2.95), (1.5, 0.62, 0.46), UV_CORD),
        c2("bow_loop_in", (x(-1.7, 1.62), 3.22, -2.95), (1.62, 0.56, 0.46), UV_CORD),
        c2("bow_tail_out", (x(0.75, 0.38), 1.5, -2.97), (0.38, 1.18, 0.42), UV_CORD),
        c2("bow_tail_in", (x(-0.92, 0.38), 1.78, -2.97), (0.38, 0.94, 0.42), UV_CORD),
    )


def part_boots() -> ArmorPart:
    return ArmorPart(
        "hide_boots",
        "HIDE BOOTS",
        _boot_cubes("LEFT_FOOT", 1.0) + _boot_cubes("RIGHT_FOOT", -1.0),
    )


def parts() -> tuple[ArmorPart, ...]:
    return (part_helmet(), part_chestplate(), part_leggings(), part_boots())


# ─── 贴图 ─────────────────────────────────────────────────────────────────


def _mottle(image, rng, box, count, dark, light, radius):
    """在指定象限里叠大块软斑——皮之所以读成皮，靠的就是这种大面积不均匀污渍。

    round 3 只画了细裂纹和划痕，那是**高频**纹理；高频细线糊在一起读成的是编织
    或纤维，不是硝过的兽皮。这里改叠低频斑，中心浓边缘淡（1-d 衰减），不留硬边。
    """
    x0, y0, x1, y1 = box
    pixels = image.load()
    for _ in range(count):
        cx, cy = rng.uniform(x0, x1), rng.uniform(y0, y1)
        rx, ry = rng.uniform(*radius), rng.uniform(*radius)
        tint = dark if rng.random() < 0.58 else light
        peak = rng.uniform(0.16, 0.40)
        for y in range(max(y0, int(cy - ry)), min(y1, int(cy + ry) + 1)):
            for x in range(max(x0, int(cx - rx)), min(x1, int(cx + rx) + 1)):
                d = ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2
                if d > 1.0:
                    continue
                alpha = peak * (1.0 - d)
                pixels[x, y] = tuple(
                    int(round(channel * (1 - alpha) + target * alpha))
                    for channel, target in zip(pixels[x, y], tint)
                )


def _patch_swatch(image, rng, uv, size, base, edge):
    """画一小块补丁料：底色 + 颗粒 + 沿正面边框的一圈虚线手缝。

    补丁只有 3.5 单位见方，而这套件的 UV 是 1 texel ≈ 1 单位——正面整个就 3~4 个
    texel。所以缝线只能是**逐 texel 的点**：每边留 1~2 针、交错排，读起来才是
    "一圈手缝"而不是"描了个边"。整边连续描边会把 4 texel 的补丁吃掉一半。
    """
    u, v = uv
    sx, sy, sz = size
    draw = ImageDraw.Draw(image)
    # 先铺满整个 box-uv 占地（含上下/侧面），避免任何一面采到隔壁的料
    draw.rectangle((u, v, u + int(2 * (sx + sz)), v + int(sy + sz)), fill=base)
    _mottle(image, rng, (u, v, u + int(2 * (sx + sz)) + 1, v + int(sy + sz) + 1), 4,
            tuple(max(0, ch - 22) for ch in base),
            tuple(min(255, ch + 20) for ch in base), (1.5, 3.5))

    # 只留一两点旧渍。**不画边框、不画针脚**：正面统共 3~4 个 texel，任何逐
    # texel 的边框/交错针脚放大后都是棋盘格（round 1 实测）。补丁的边界靠它本身
    # 凸出 0.15 的那圈侧面在 MC 分轴着色下自然分层，锁边线走几何（见 _chest_patches）。
    x0, x1 = int(u + sz), int(u + sz + sx)
    y0, y1 = int(v + sz), int(v + sz + sy)
    for _ in range(3):
        px, py = rng.randint(x0, x1), rng.randint(y0, y1)
        draw.point((px, py), fill=edge)


def _assert_patch_regions(all_parts: tuple[ArmorPart, ...]) -> None:
    """核验保留格没被别的 cube 采到——不靠"应该没人用"。

    box-uv 会把 6 个面摊进 (u, v) 起、2*(sx+sz) 宽、sy+sz 高的矩形里；只要有一个
    非补丁 cube 的矩形和保留格相交，那个件就会在某个面上糊出补丁色。
    """
    reserved = [(u, v, u + w, v + h) for (u, v), w, h in PATCH_REGIONS]
    patch_uvs = {UV_PATCH_PALE, UV_PATCH_RED}
    for part in all_parts:
        for cube in part.cubes:
            if cube.uv in patch_uvs:
                continue
            u, v = cube.uv
            sx, sy, sz = cube.size
            box = (u, v, u + 2 * (sx + sz), v + sy + sz)
            for index, region in enumerate(reserved):
                if (box[0] < region[2] and box[2] > region[0]
                        and box[1] < region[3] and box[3] > region[1]):
                    raise ValueError(
                        f"{part.key}/{cube.name} 的 box-uv {box} 压到补丁保留格 "
                        f"{index} {region}；换 uv 原点或挪保留格"
                    )

def _assert_no_coplanar_faces(all_parts: tuple[ArmorPart, ...]) -> None:
    """揪出"两块外表面落在同一平面且投影相交"的 cube 对——体素模型的经典 z-fighting。

    渲染器对同深度的两个面没有稳定的取舍，逐像素乱选，结果就是一片高频噪点。
    round 2 的过肩片和后片背面同在 z=2.75，后视肩部整整两块噪点；肉眼当时只当
    成"贴图脏"，是这个检查把它定位出来的。触发时把 cube 名和平面一起报出来，
    改法永远是把其中一块沿该轴挪开一点（别改成"刚好贴着"，贴着也是共面）。
    """
    from bbmodel_maker.model.armor_model_common import MOUNT_X

    def bounds(cube: Cube) -> tuple[tuple[float, ...], tuple[float, ...]]:
        offset = MOUNT_X[cube.mount]
        low = (cube.origin[0] + offset, cube.origin[1], cube.origin[2])
        return low, tuple(low[i] + cube.size[i] for i in range(3))

    for part in all_parts:
        cubes = part.cubes
        for i in range(len(cubes)):
            for j in range(i + 1, len(cubes)):
                first, second = cubes[i], cubes[j]
                # **不能只比同 mount**：左右腿是两个 mount，但静止姿下 MOUNT_X 已经把
                # 它们摆进同一片世界空间，裆缝处两条腿的甲片照样会共面打架
                # （hide_leggings round 1 实测，裆前渲出一片细网格）。动起来它们会
                # 分开，但玩家站着的时候就是这个样子，必须查。
                low_a, high_a = bounds(first)
                low_b, high_b = bounds(second)
                for axis in range(3):
                    overlap = 1.0
                    for other in (k for k in range(3) if k != axis):
                        overlap *= max(0.0, min(high_a[other], high_b[other])
                                       - max(low_a[other], low_b[other]))
                    if overlap <= 0.02:      # 只擦到一条边不算，那是正常拼接
                        continue
                    for face, value_a, value_b in (
                        ("max", high_a[axis], high_b[axis]),
                        ("min", low_a[axis], low_b[axis]),
                    ):
                        if abs(value_a - value_b) < 1e-6:
                            raise ValueError(
                                f"{part.key}: {first.name} 与 {second.name} 的 "
                                f"{'xyz'[axis]}-{face} 面共面于 {value_a}，"
                                f"投影相交 {overlap:.2f}——会 z-fighting，挪开一块"
                            )


def make_texture() -> Image.Image:
    """四象限：主熟皮 / 深色油润皮 / 带 X 缝绑扎的皮 / 草绳。

    色相取自参考图（暖褐 R:G:B≈88:73:55）。round 3 整体提亮到 V≈48%，理由是
    "参考是压暗棚光"——但实测我的 MC 着色渲染出来平均 V=35~41%，参考是 31~33%，
    提亮提过头了。这版把主皮压到 V≈42%（乘 MC 侧面 0.8 后落在参考区间），并
    彻底删掉 V=60% 的褪色象限。
    """
    rng = random.Random(0x81DE)
    image = Image.new("RGB", (TEXTURE_SIZE, TEXTURE_SIZE), (108, 90, 68))
    pixels = image.load()
    for y in range(TEXTURE_SIZE):
        for x in range(TEXTURE_SIZE):
            if x < 32 and y < 32:
                base = (108, 90, 68)      # 主熟皮
            elif y < 32:
                base = (84, 69, 50)       # 深色油润皮
            elif x < 32:
                base = (104, 87, 66)      # 带缝线的皮（底色同主皮略沉）
            else:
                base = (124, 110, 80)     # 草绳：只比皮浅一档，不许抢戏
            value_jitter = rng.randint(-6, 6)
            warm_jitter = rng.randint(-3, 3)
            offsets = (value_jitter + warm_jitter, value_jitter, value_jitter - warm_jitter)
            pixels[x, y] = tuple(
                max(0, min(255, channel + offset)) for channel, offset in zip(base, offsets)
            )

    # 主熟皮：大块渍打底，再补几道低对比干裂与毛孔。
    #
    # 斑块数与半径是为胸甲扩的（14→20、上限 11→14）：胸甲前后两大片各自只采样
    # 约 9x11 个 texel，按头盔那套参数一个窗口里常常一块渍都摊不到，渲出来就是
    # 一整片死平的褐色板。裂纹同理由 4 条加到 10 条，并压进 x<20 / y<16——
    # 那正是两大片和肩片实际采到的窗口（box-uv 摊面算出来的）。
    _mottle(image, rng, (0, 0, 32, 32), 20, (74, 60, 44), (134, 116, 92), (4.0, 14.0))
    draw = ImageDraw.Draw(image)
    for points in (
        ((2, 6), (8, 11), (6, 19)),
        ((14, 3), (12, 10), (17, 17)),
        ((22, 8), (26, 14), (23, 22)),
        ((5, 24), (11, 27), (9, 31)),
        ((0, 2), (5, 4), (3, 9), (7, 13)),
        ((9, 0), (10, 5), (14, 8)),
        ((18, 1), (16, 6), (19, 12), (17, 15)),
        ((1, 13), (4, 16), (2, 21)),
        ((11, 14), (15, 16), (13, 21)),
        ((19, 17), (23, 19), (21, 24)),
    ):
        draw.line(points, fill=(88, 72, 54), width=1)
    for x, y in ((4, 15), (17, 6), (20, 26), (28, 4), (26, 30), (10, 21)):
        draw.point((x, y), fill=(136, 118, 94))
    for x, y in ((1, 12), (1, 17), (1, 22), (30, 9), (30, 14), (30, 19)):
        draw.point((x, y), fill=(62, 50, 37))

    # 深色油润皮：手汗盘出来的暗斑与油光条
    _mottle(image, rng, (32, 0, 64, 32), 10, (60, 48, 34), (112, 94, 72), (3.5, 9.0))
    for x, y, w, h in ((35, 5, 4, 3), (43, 12, 3, 4), (38, 21, 5, 2), (46, 26, 3, 3)):
        draw.rectangle((x, y, x + w, y + h), fill=(66, 53, 38))
    for x, y, length in ((33, 9, 7), (40, 17, 6), (34, 27, 8)):
        draw.line((x, y, x + length, y + 1), fill=(102, 85, 63), width=1)

    # 带缝线的皮：竖排手缝，列距 8px——任一采样窗口都能截到 1~3 列，
    # 正好复刻参考图护耳侧边那几串绑扎。这才是 round 3 用几何做错的东西。
    #
    # 缝必须画成 **1 texel** 粗。这些件的 UV 是 1 texel ≈ 1 单位，渲染时一个 texel
    # 就是屏幕上 20 来个像素；round 4 first cut 画了 5 texel 宽的 X，放大后整片
    # 读成棋盘格，比原来的几何绳还糟。一针的宽度本来就该是一个像素。
    _mottle(image, rng, (0, 32, 32, 64), 12, (72, 58, 42), (128, 111, 88), (4.0, 10.0))
    stitch, hole = (134, 120, 94), (62, 50, 37)
    for col in (3, 11, 19, 27):
        draw.line((col, 33, col, 62), fill=(88, 73, 55), width=1)
        for index, row in enumerate(range(34, 62, 3)):
            draw.point((col + (1 if index % 2 else -1), row), fill=stitch)
            draw.point((col, row + 1), fill=hole)

    # 草绳：斜向搓纹（顺一个方向，读成拧过的绳），对比度压到刚好可辨。
    # 画在独立 tile 上再贴回去——直接在整图上画斜线会有一截探进左边的皮象限。
    cord = image.crop((32, 32, 64, 64))
    cord_draw = ImageDraw.Draw(cord)
    for start in range(-8, 32, 3):
        cord_draw.line((start, 31, start + 8, 0), fill=(106, 93, 66), width=1)
    for start in range(-6, 32, 6):
        cord_draw.line((start, 31, start + 8, 0), fill=(146, 132, 100), width=1)
    image.paste(cord, (32, 32))

    # 补丁料：色相取自参考图（浅补丁 H38 S27 V40、红补丁 H22 S37 V29），除以 MC
    # 着色的 ~0.75 折回贴图空间。红的那块要压得比主皮暗——参考图里它是块血锈色
    # 旧皮，提亮就成了"新缝的花布"。
    # round 2 的红补丁 S=36% 和主皮 37% 一模一样、明度还更低，渲出来只是"一块
    # 脏"。参考图里这两块靠的是**饱和度差**（红 S37 主皮 S24），不是明暗差——
    # 所以红的往锈色推到 S48，浅的往漂白压到 S24，各自站到主皮的两侧。
    _patch_swatch(image, rng, UV_PATCH_PALE, (3.5, 3.4, 0.25),
                  base=(142, 130, 108), edge=(112, 101, 82))
    _patch_swatch(image, rng, UV_PATCH_RED, (3.0, 3.3, 0.25),
                  base=(104, 70, 54), edge=(76, 48, 36))
    return image


# ─── 输出 ─────────────────────────────────────────────────────────────────


def emit_java(part: ArmorPart) -> str:
    """吐 ArmorPartModel.CUBE_TABLES 用的 Java 字面量（运行时真相，勿手抄）。"""
    method = "".join(word.capitalize() for word in part.key.split("_"))
    method = method[0].lower() + method[1:]
    lines = [f"    private static List<ArmorCube> {method}() {{", "        return List.of("]
    body = []
    for cube in part.cubes:
        ox, oy, oz = cube.origin
        sx, sy, sz = cube.size
        u, v = cube.uv
        body.append(
            f"            new ArmorCube(Mount.{cube.mount}, "
            f"{ox}f, {oy}f, {oz}f, {sx}f, {sy}f, {sz}f, {u}, {v})"
        )
    lines.append(",\n".join(body))
    lines.append("        );")
    lines.append("    }")
    return "\n".join(lines)


def cube_digest(part: ArmorPart) -> str:
    """复刻 ArmorPartModelTest.cubeDigest 的 FNV-1a，免得为拿 pin 值跑一趟 Java。"""
    import struct

    def fnv1a(hash_value: int, value: int) -> int:
        for _ in range(4):
            hash_value ^= value & 0xFF
            hash_value = (hash_value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
            value >>= 8
        return hash_value

    def bits(f: float) -> int:
        return struct.unpack("<I", struct.pack("<f", f))[0]

    mounts = ["HEAD", "BODY", "LEFT_LEG", "RIGHT_LEG", "LEFT_FOOT", "RIGHT_FOOT"]
    h = 0xCBF29CE484222325
    for cube in part.cubes:
        h = fnv1a(h, mounts.index(cube.mount))
        for value in (*cube.origin, *cube.size):
            h = fnv1a(h, bits(value))
        h = fnv1a(h, cube.uv[0])
        h = fnv1a(h, cube.uv[1])
    return f"{h:016x}"


def generate(render_previews: bool = True, install: bool = False) -> dict[str, Path]:
    _assert_patch_regions(parts())
    _assert_no_coplanar_faces(parts())
    return write_material_assets(
        MATERIAL,
        parts(),
        make_texture(),
        LOCAL_MODELS,
        CLIENT_TEXTURE_ROOT if install else DRAFT_TEXTURE_ROOT,
        PREVIEW_ROOT,
        render_previews,
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--no-preview", action="store_true", help="只写 bbmodel/texture")
    parser.add_argument("--emit-java", action="store_true", help="打印 ArmorPartModel 用的 cube 表")
    parser.add_argument("--install", action="store_true",
                        help="贴图写进 client 资源树（接线那轮再用，记得同步资源包 sha1）")
    args = parser.parse_args()

    if args.emit_java:
        for part in parts():
            print(f"// {part.key}: {len(part.cubes)} cubes, digest {cube_digest(part)}")
            print(emit_java(part))
            print()
        return

    outputs = generate(render_previews=not args.no_preview, install=args.install)
    for key, path in outputs.items():
        print(f"[{key}] {path.relative_to(REPO)}")


if __name__ == "__main__":
    main()
