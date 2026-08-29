#!/usr/bin/env python3
"""草甲（straw）护腿 / 草鞋的 bbmodel + 贴图 + 三视图生成器。

参考图：`orthograph #6`（草护腿：竖向稻杆捆扎 + 五道绳箍 + 上下出穗）、
`orthograph #7`（草鞋：编织草底 + 趾间绳桩 + 脚背 V 叉 + 踝箍与外侧结）。

标定（按 CLAUDE.md「护甲 bbmodel 流水线」：用双臂/腿宽标定，别信头）：

    ortho6  水平 21.5 px/单位；髋线 y=560 ↔ MC y12，地面 y=1112 ↔ MC y0
            → 纵向 46 px/单位，即那具模特的腿被拉长 2.17×
    ortho7  水平 21.2 px/单位；髋线 y=650，地面 y=1012 → 纵向 30.2 px/单位（1.43×）

两张图是各自独立出的，拉伸倍率都不一样，所以**纵向一律按腿长比例映射**，
不拿 px/单位硬乘。下面每处偏离参考的地方都写了理由。
"""

from __future__ import annotations

import argparse
import random
from pathlib import Path

from PIL import Image, ImageDraw

import sys as _sys
from pathlib import Path as _Path

_sys.path.insert(0, str(_Path(__file__).resolve().parents[1] / "core"))

from bbmodel_maker.model.armor_model_common import ArmorPart, Cube, TEXTURE_SIZE, write_material_assets

REPO = Path(__file__).resolve().parents[2]
LOCAL_MODELS = Path(__file__).resolve().parents[1] / "models"
PREVIEW_ROOT = Path(__file__).resolve().parents[1] / "out"

MATERIAL = "straw"
DRAFT_TEXTURE_ROOT = LOCAL_MODELS / "armor" / MATERIAL / "textures"
CLIENT_TEXTURE_ROOT = (
    REPO / "client" / "src" / "main" / "resources" / "assets" / "bong" / "textures" / "armor"
)

# 贴图分格。**稻杆象限被切成四条 8px 宽的色调列**，这是本件最要紧的一处设计：
#
# armor 的 box-uv 是 1 texel ≈ 1 单位，一块 0.93 宽的杆板正面只采到**一个
# texel**——贴图上画多细的杆线都进不去，整块板就是一坨纯色。而 MC 只按**轴**
# 着色，前排几块板深度错开 0.2 在同一朝向上**不产生任何明暗差**，剩下的只有
# 板缝那道硬沟 → round 1/2 都渲成了板条箱。
#
# 所以竖向分节只能靠**颜色**：相邻杆板轮着取 A/B/C/D 四个明度不同的稻杆色，
# 一块板一个调子，正面就有了四阶的竖向节律。这是 1 texel/单位下唯一能表达
# "一根一根杆"的手段。
#
# 列宽 8 是按最宽的杆板算的：侧板 0.84×1.28 摊成 box-uv 是 4.24 texel 宽，
# 留一倍余量。跨列会采到隔壁调子，由 _assert_uv_tiles 挡住。
UV_STRAW_A = (0, 0)      # 晒白的那几根
UV_STRAW_B = (8, 0)      # 主调
UV_STRAW_C = (16, 0)     # 背光/发灰的
UV_STRAW_D = (24, 0)     # 次亮
UV_WORN_A = (32, 0)      # 旧稻杆：穗头、断口
UV_WORN_B = (48, 0)      # 旧稻杆（另一调）
UV_BRAID = (0, 32)       # 编织草底：横向编纹
UV_CORD = (32, 32)       # 搓绳：斜向拧纹

STRAW_TONES = (UV_STRAW_A, UV_STRAW_B, UV_STRAW_C, UV_STRAW_D)

# 每个 uv 原点允许占用的格子（宽, 高）。box-uv 摊出来的矩形必须整个落在格内。
UV_TILES = {
    UV_STRAW_A: (8, 32), UV_STRAW_B: (8, 32), UV_STRAW_C: (8, 32), UV_STRAW_D: (8, 32),
    UV_WORN_A: (16, 32), UV_WORN_B: (16, 32),
    UV_BRAID: (32, 32), UV_CORD: (32, 32),
}


def c(mount, name, origin, size, uv=UV_STRAW_B) -> Cube:
    return Cube(mount, name, origin, size, uv)


# ─── 草护腿 ────────────────────────────────────────────────────────────────
# 参考图的稻杆捆围度是 5.6 宽 × 5.8 深（腿本身 4×4，即四面各鼓 0.8~0.9）。
# **宽度照抄不了**：腿的骨骼枢轴在 x=±1.9，一条腿占世界 x 0~3.8，5.6 宽会让
# 两捆在裆下互相吃进去 1.5 格，正视直接糊成一根柱子（兽皮甲护腿 round 1 就是
# 死在这，那次是 0.5 沟槽救回来的）。这里内缘钉死在局部 -1.78（世界 0.12），
# 鼓量全推给外侧和前后：
#     宽 4.64（比例 1.16，参考 1.43）  深 5.56（比例 1.39，参考 1.45）
# 深度这一轴没有对侧冲突，所以「比腿粗一大圈」的读法由它扛。
#
# 纵向：参考图稻杆从 MC y1.35 到 y11.9。下缘上收到 3.30（穗尖 2.28~2.72）——
# 两张参考图是各画各的，草鞋那张的踝绳一路绑到 y4.8，两件照抄必然打架。收到
# 3.30 之后，草鞋的踝箍（y2.04~2.44）正好从底穗底下露出一线，穿全套时还认得
# 出脚上是另一件东西；round 2 的 3.05 会把踝箍整个埋掉，正视只剩一片草。
#
# 绳箍位置来自 ortho6 左腿柱的亮度剖面（绳比杆暗 14 以上）：
#     y 613 / 656 / 765 / 880 / 985 px → MC 10.85 / 9.91 / 7.54 / 5.04 / 2.76
# 顶上那两道挨得近（差 0.94），是「捆到髋部时多绕一圈」，保留成双箍；下面三道
# 随主体下缘上抬 0.9~1.2，仍**均匀铺满整段**。round 2 曾减到四道并挤在上半段，
# 与参考对拍时下面一大截是光的，读成「捆了一半就撒手了」。
#
# 箍厚：参考只有 0.28 单位。做成 0.28 在 MC 里基本看不见（一个 texel 都不到），
# 按兽皮甲绳件同族放到 0.50~0.58。

LEG_BODY_BOTTOM = 3.30
LEG_BODY_TOP = 11.35
LEG_BAND_Y = (10.75, 9.85, 7.90, 5.95, 4.00)


def _leg_cubes(mount: str, sign: float) -> tuple[Cube, ...]:
    """一条腿的全部构件。sign=+1 左腿（局部 +x 朝外），-1 右腿。

    件名带 `_left` / `_right` **后缀**：validate_part 要求整件内唯一，而
    OnPlayer 合模按名字第一段分组，用后缀才分得出 stave / band / tuft 三族。
    """
    side = "left" if sign > 0 else "right"

    def x(inner: float, width: float) -> float:
        """把「朝外为正」的局部坐标翻成该腿真正的 origin.x。"""
        return inner if sign > 0 else -inner - width

    def c2(name: str, origin, size, uv=UV_STRAW_B) -> Cube:
        return Cube(mount, f"{name}_{side}", origin, size, uv)

    body_h = LEG_BODY_TOP - LEG_BODY_BOTTOM
    out: list[Cube] = []

    # ── 稻杆本体：十二根「杆板」围成一圈，相邻板取不同色调 ──
    # 板宽 0.92~0.94（约等于 1 texel/单位下一根杆的观感宽度）。深度仍然交替
    # 错开 0.2，但那**只管侧影和板缝**；正面看得出"一根根"全靠 STRAW_TONES
    # 四阶色调轮转（见 UV 段的长注释）。
    #
    # 前后板吃满整个宽度，侧板只填中间那段深度 —— 在 z=±1.94 处对接，不重叠。
    # 重叠会让两块的同向面共面，转角渲出一条噪点线。
    tone = 0

    def next_tone() -> tuple[int, int]:
        """按 A→C→B→D→A 取色。不是顺序轮转：顺着来会渲成从亮到暗的渐变带，
        读成"打了柔光的一块板"；隔位跳才读成随机长成这样的一捆杆。"""
        nonlocal tone
        uv = STRAW_TONES[(0, 2, 1, 3)[tone % 4]]
        tone += 1
        return uv

    for name, x0, w, z0 in (
        ("stave_front_a", -1.78, 0.94, -2.62),
        ("stave_front_b", -0.84, 0.94, -2.82),
        ("stave_front_c", 0.10, 0.92, -2.58),
        ("stave_front_d", 1.02, 0.92, -2.80),
        ("stave_front_e", 1.94, 0.92, -2.66),
    ):
        out.append(c2(name, (x(x0, w), LEG_BODY_BOTTOM, z0), (w, body_h, -z0 - 1.94), next_tone()))
    for name, x0, w, z1 in (
        ("stave_back_a", -1.78, 0.92, 2.60),
        ("stave_back_b", -0.86, 0.94, 2.80),
        ("stave_back_c", 0.08, 0.94, 2.56),
        ("stave_back_d", 1.02, 0.92, 2.78),
        ("stave_back_e", 1.94, 0.92, 2.64),
    ):
        out.append(c2(name, (x(x0, w), LEG_BODY_BOTTOM, 1.94), (w, body_h, z1 - 1.94), next_tone()))
    for name, z0, dz, xmax in (
        ("stave_side_f", -1.94, 1.28, 2.88),
        ("stave_side_m", -0.66, 1.30, 2.64),
        ("stave_side_b", 0.64, 1.30, 2.86),
    ):
        out.append(c2(name, (x(2.02, xmax - 2.02), LEG_BODY_BOTTOM, z0),
                      (xmax - 2.02, body_h, dz), next_tone()))
    # 内侧那块落在两腿之间的裆缝里，任何视角都看不到；它的 box-uv 有 8.28 texel
    # 宽，塞不进 8 宽的色调列，直接丢给 16 宽的旧稻杆格。
    out.append(c2("stave_side_in", (x(-1.78, 0.34), LEG_BODY_BOTTOM, -1.90),
                  (0.34, body_h, 3.80), UV_WORN_B))

    # ── 顶穗 / 底穗：散开的杆头，做成一根根尖刺而不是一整圈裙边 ──
    # round 1 用四片吃满整边的板，渲出来和绳箍一模一样，读成「又两道箍」。
    # 穗的特征是**参差**：每端八根，宽 0.50~0.62、长短各不同、深度也各不同，
    # 任何视角看过去都是不齐的口（兽皮甲靴筒口同一条经验：只错开一个轴的话
    # 换个视角又是平边）。
    #
    # 顶穗的高度**按 z 分两档**，不是统一封顶：
    #  - 前后两排在 z<-2.2 / z>2.3，整个躲开躯干和手臂的 z 带（±2），可以放到 12.6
    #  - 外侧两排在 z-1.4~1.6，正撞手臂（world x4~8 / y12~24 / z±2）的 z 带，
    #    必须压在 y<12 —— round 1 它们伸到 12.51，抬手时会有草刺从肘部长出来。
    #    这条由 _assert_no_body_clash 挡住，别再改回去。
    for name, x0, w, dy, z0, dz, uv in (
        ("tuft_top_f1", -1.62, 0.58, 0.96, -2.96, 0.62, UV_WORN_A),
        ("tuft_top_f2", -0.70, 0.62, 1.24, -2.88, 0.58, UV_WORN_B),
        ("tuft_top_f3", 0.62, 0.54, 0.80, -3.00, 0.66, UV_WORN_A),
        ("tuft_top_o4", 1.74, 0.56, 1.10, -2.94, 0.60, UV_WORN_B),
        ("tuft_top_o1", 2.30, 0.58, 0.58, -1.42, 0.80, UV_WORN_A),
        ("tuft_top_o2", 2.38, 0.52, 0.50, 0.82, 0.74, UV_WORN_B),
        ("tuft_top_b1", -1.14, 0.60, 0.88, 2.36, 0.62, UV_WORN_A),
        ("tuft_top_b2", 1.06, 0.56, 0.64, 2.42, 0.56, UV_WORN_B),
    ):
        out.append(c2(name, (x(x0, w), LEG_BODY_TOP, z0), (w, dy, dz), uv))
    for name, x0, w, dy, z0, dz, uv in (
        ("tuft_bot_f1", -1.56, 0.56, 0.86, -2.94, 0.60, UV_WORN_B),
        ("tuft_bot_f2", -0.58, 0.60, 0.62, -3.02, 0.68, UV_WORN_A),
        ("tuft_bot_f3", 0.74, 0.52, 0.94, -2.90, 0.56, UV_WORN_B),
        ("tuft_bot_f4", 1.86, 0.54, 0.70, -2.98, 0.64, UV_WORN_A),
        ("tuft_bot_o1", 2.37, 0.53, 1.02, -1.33, 0.79, UV_WORN_B),
        ("tuft_bot_o2", 2.40, 0.50, 0.66, 0.94, 0.70, UV_WORN_A),
        ("tuft_bot_b1", -1.20, 0.58, 0.76, 2.38, 0.60, UV_WORN_B),
        ("tuft_bot_b2", 1.12, 0.54, 0.58, 2.44, 0.54, UV_WORN_A),
    ):
        out.append(c2(name, (x(x0, w), LEG_BODY_BOTTOM - dy, z0), (w, dy, dz), uv))

    # ── 五道绳箍 ──
    # round 1 箍高 0.50、探出 0.46、五道，加起来占了主体高度的 32%，正面几乎
    # 一半是绳 —— 参考图里绳只占 13%，是**细扎带**不是箍圈。这轮压到 0.30
    # （顶道 0.44）、探出 0.24，让稻杆当主角。
    #
    # 每道三段：前、后、外。内侧那段砍掉了 —— 它落在两腿之间的裆缝里，任何
    # 视角都看不到，白占 8 个 cube。
    for index, cy in enumerate(LEG_BAND_Y):
        h = 0.44 if index == 0 else 0.36
        tag = f"band{index}"
        out.append(c2(f"{tag}_front", (x(-1.84, 4.76), cy, -3.06), (4.76, h, 0.28), UV_CORD))
        out.append(c2(f"{tag}_back", (x(-1.82, 4.72), cy + 0.03, 2.76), (4.72, h, 0.28), UV_CORD))
        out.append(c2(f"{tag}_out", (x(2.90, 0.26), cy + 0.01, -2.62), (0.26, h, 5.24), UV_CORD))

    return tuple(out)


def part_leggings() -> ArmorPart:
    return ArmorPart(
        "straw_leggings",
        "STRAW LEGGINGS",
        _leg_cubes("LEFT_LEG", 1.0) + _leg_cubes("RIGHT_LEG", -1.0),
    )


# ─── 草鞋 ──────────────────────────────────────────────────────────────────
# 脚 mount 与腿共用骨骼枢轴（x=±1.9、y=12），坐标是「骨骼局部 x/z + Bedrock
# 绝对 y（脚底 0）」。
#
# 参考图的草底是 6.6 宽 × 9.2 长的大椭圆。两处照抄不了：
#  1. **宽**。MC 的脚就是腿盒底面 4×4，6.6 宽的两只底在中线要吃进去 2.6 格。
#     内缘钉死世界 0.0、只往外放到 5.0（外鼓 1.2）—— 正视仍明显是「一片比脚
#     大一圈的草垫」，但两只不粘连。
#  2. **长**。9.2 长配 4 宽是 1:2.3，那是被拉长的模特的脚。收到 7.5
#     （1:1.5），和兽皮甲靴同一档，不然是双船。
#
# 底做成五道横向编片而不是一整块：相邻片的顶面高度交替差 0.07，MC 按轴着色下
# 就是一条条编纹。片的宽度两头收（趾 4.40 / 掌 4.88 / 心 5.00 / 弓 4.94 / 跟 4.58），
# 轴对齐的 cube 能给出的「椭圆」上限就是这个。

# 草底是**一圈盘起来的粗草绳**，不是一叠板。round 1/2 用五道横向编片做，与参考
# 并排一看就露馅：横片的截面在侧视排成一列，整只鞋读成木托盘。这轮拆成三层：
#
#   base  垫底整片（压到 y<0：脚底本身在 y=0，鞋底停在 0 等于和脚底面共面打架，
#         而且"鞋底在脚底之下"本来就是鞋的定义；原版皮靴同理）
#   fill  中间的编心，比沿口矮 0.12~0.2
#   rim   外沿那圈**凸起的绳盘**，走 UV_CORD 取斜向拧纹 —— 这圈是"草绳盘的"
#         这个读法的全部来源，凹下去的编心把它衬出来
#
# 再加一圈往外炸的散穗（UV_WORN）。参考图里穗是整圈都有的，这里只做趾/外/跟
# 三面：内侧那面挨着另一只鞋，做了看不见，往里探还会越过中线钻进对侧的鞋。

# 沿口绳盘：(name, inner, w, z0, z1)，y 统一 0.30~1.00
BOOT_RIM = (
    ("rim_toe", -1.72, 4.74, -4.62, -4.02),
    ("rim_out_f", 2.54, 0.56, -4.02, -1.20),
    ("rim_out_b", 2.50, 0.56, -1.20, 2.32),
    ("rim_heel", -1.66, 4.38, 2.32, 2.88),
    ("rim_in_f", -1.90, 0.54, -4.00, -0.60),
    ("rim_in_b", -1.88, 0.52, -0.60, 2.30),
)
# 编心（凹）：(name, inner, w, z0, z1, y1)
BOOT_FILL = (
    ("fill_fore", -1.44, 4.02, -3.96, -1.62, 0.86),
    ("fill_mid", -1.40, 3.96, -1.62, 0.70, 0.79),
    ("fill_heel", -1.44, 3.92, 0.70, 2.26, 0.88),
)
# 垫底：(name, inner, w, z0, z1, y0, y1)
BOOT_BASE = (
    ("base_fore", -1.86, 4.78, -4.52, -1.30, -0.36, 0.32),
    ("base_mid", -1.84, 4.84, -1.30, 0.90, -0.30, 0.34),
    ("base_heel", -1.84, 4.68, 0.90, 2.78, -0.34, 0.31),
)


def _boot_cubes(mount: str, sign: float) -> tuple[Cube, ...]:
    """一只草鞋的全部构件。sign=+1 左脚（局部 +x 朝外），-1 右脚。"""
    side = "left" if sign > 0 else "right"

    def x(inner: float, width: float) -> float:
        return inner if sign > 0 else -inner - width

    def c2(name: str, origin, size, uv=UV_STRAW_B) -> Cube:
        return Cube(mount, f"{name}_{side}", origin, size, uv)

    out: list[Cube] = []

    # ── 草底三层：垫底 / 编心 / 沿口绳盘 ──
    for name, inner, w, z0, z1, y0, y1 in BOOT_BASE:
        out.append(c2(name, (x(inner, w), y0, z0), (w, y1 - y0, z1 - z0), UV_BRAID))
    for name, inner, w, z0, z1, y1 in BOOT_FILL:
        out.append(c2(name, (x(inner, w), 0.36, z0), (w, y1 - 0.36, z1 - z0), UV_BRAID))
    for name, inner, w, z0, z1 in BOOT_RIM:
        out.append(c2(name, (x(inner, w), 0.30, z0), (w, 0.70, z1 - z0), UV_CORD))

    # ── 底沿的散穗 ──
    out.append(c2("welt_toe", (x(-1.40, 4.00), 0.36, -5.04), (4.00, 0.46, 0.42), UV_WORN_A))
    out.append(c2("welt_out_f", (x(3.12, 0.38), 0.42, -3.42), (0.38, 0.46, 2.14), UV_WORN_B))
    out.append(c2("welt_out_b", (x(3.08, 0.34), 0.52, -0.54), (0.34, 0.46, 2.26), UV_WORN_A))
    out.append(c2("welt_heel", (x(-1.50, 4.18), 0.44, 2.90), (4.18, 0.46, 0.40), UV_WORN_B))

    # ── 趾间绳桩 + 前叉 ──
    # 参考图正视能看到一根绳从草底前端立起来穿进趾缝，再往后分成 V。
    out.append(c2("thong_post", (x(-0.26, 0.52), 1.02, -3.86), (0.52, 0.80, 0.48), UV_CORD))
    out.append(c2("thong_yoke", (x(-0.32, 0.64), 1.56, -3.40), (0.64, 0.36, 0.62), UV_CORD))

    # ── 脚背 V 叉：每边三级台阶 ──
    # round 1 放在 y1.58~2.24，和踝箍之间没有空隙，正视两者糊成一根横杠，参考图
    # 那个"绳在脚背上交叉"的读法完全丢了；而且下面没有任何东西接到草底，侧视
    # 整条绳是**悬空**的。这轮压到 y1.10~2.06，并加立绳把它锚回底上。
    out.append(c2("vee_out_a", (x(0.18, 0.72), 1.12, -3.02), (0.72, 0.42, 0.56), UV_CORD))
    out.append(c2("vee_out_b", (x(0.86, 0.76), 1.40, -2.72), (0.76, 0.42, 0.60), UV_CORD))
    out.append(c2("vee_out_c", (x(1.56, 0.46), 1.66, -2.34), (0.46, 0.42, 0.64), UV_CORD))
    out.append(c2("vee_in_a", (x(-1.02, 0.74), 1.08, -3.02), (0.74, 0.42, 0.56), UV_CORD))
    out.append(c2("vee_in_b", (x(-1.72, 0.76), 1.36, -2.72), (0.76, 0.42, 0.60), UV_CORD))
    out.append(c2("vee_in_c", (x(-1.88, 0.44), 1.62, -2.34), (0.44, 0.42, 0.64), UV_CORD))

    # ── 两根立绳：草底 → 踝缠 ──
    # round 1 没有这个，踝上的绳是凭空浮在小腿上的圈，整只鞋读成脚手架。四根
    # 减到两根：另外两根在内侧，被对面那只鞋和裆缝挡死，白占 cube。
    out.append(c2("riser_out_f", (x(1.97, 0.34), 1.00, -1.78), (0.34, 1.22, 0.34), UV_CORD))
    out.append(c2("riser_in_f", (x(-1.86, 0.32), 1.00, -1.76), (0.32, 1.26, 0.32), UV_CORD))

    # ── 踝缠：两圈错开高度的半环，不是一道整圈 ──
    # 参考图的绳是**斜着螺旋缠上去**的：正视看得到它从脚背一侧升到另一侧，侧视
    # 是两三道不平行的斜圈加一个结。round 2 做成一道水平整环 + 立柱，正背视
    # 活像一段栏杆扶手。轴对齐的 cube 做不出真斜线，但把整环拆成**前低后高**
    # 两个半环（差 0.42，约一根绳粗），转一圈看过去高度是连续变化的，就读成
    # 一根绳绕上去而不是两个套在腿上的箍。
    out.append(c2("wrap_lo_front", (x(-1.90, 4.02), 1.94, -2.32), (4.02, 0.36, 0.30), UV_CORD))
    out.append(c2("wrap_lo_out", (x(1.94, 0.30), 1.98, -2.06), (0.30, 0.36, 2.12), UV_CORD))
    out.append(c2("wrap_hi_out", (x(1.96, 0.28), 2.40, 0.02), (0.28, 0.36, 2.06), UV_CORD))
    out.append(c2("wrap_hi_back", (x(-1.86, 3.96), 2.36, 2.04), (3.96, 0.36, 0.30), UV_CORD))
    out.append(c2("wrap_hi_in", (x(-1.90, 0.26), 2.44, -1.20), (0.26, 0.36, 3.22), UV_CORD))

    # ── 外侧那个结 + 垂下来的两条绳头（参考图侧视最显眼的一处做工）──
    out.append(c2("knot_core", (x(2.16, 0.76), 2.18, -1.06), (0.76, 0.82, 0.78), UV_CORD))
    out.append(c2("knot_loop", (x(2.30, 0.58), 2.90, -1.32), (0.58, 0.32, 0.74), UV_CORD))
    out.append(c2("knot_tail_f", (x(2.34, 0.38), 1.32, -0.88), (0.38, 0.80, 0.36), UV_CORD))
    out.append(c2("knot_tail_b", (x(2.28, 0.34), 1.50, -0.18), (0.34, 0.64, 0.34), UV_CORD))

    # ── 跟后横绳：把草底后端箍在脚跟上 ──
    out.append(c2("heel_strap", (x(-1.88, 4.00), 1.46, 2.18), (4.00, 0.42, 0.30), UV_CORD))

    return tuple(out)


def part_boots() -> ArmorPart:
    return ArmorPart(
        "straw_boots",
        "STRAW BOOTS",
        _boot_cubes("LEFT_FOOT", 1.0) + _boot_cubes("RIGHT_FOOT", -1.0),
    )


def parts() -> tuple[ArmorPart, ...]:
    return (part_leggings(), part_boots())


# ─── 校验 ─────────────────────────────────────────────────────────────────


def _assert_no_coplanar_faces(all_parts: tuple[ArmorPart, ...]) -> None:
    """揪出"两块外表面落在同一平面且投影相交"的 cube 对——体素模型的经典 z-fighting。

    渲染器对同深度的两个面没有稳定取舍，逐像素乱选，渲出来是一片高频噪点，
    肉眼极易误判成"贴图脏"。**不能只比同 mount**：左右腿是两个 mount，但静止
    姿下 MOUNT_X 已经把它们摆进同一片世界空间，裆缝处照样会打架。
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


# 原版 biped 的体积（世界坐标，脚在 y=0）。护甲件穿模进去 = 抬手抬腿时穿帮。
VANILLA_ARM_LEFT = ((4.0, 8.0), (12.0, 24.0), (-2.0, 2.0))
VANILLA_BODY = ((-4.0, 4.0), (12.0, 24.0), (-2.0, 2.0))


def _world_box(cube: Cube) -> tuple[tuple[float, float], ...]:
    from bbmodel_maker.model.armor_model_common import MOUNT_X

    offset = MOUNT_X[cube.mount]
    origin = (cube.origin[0] + offset, cube.origin[1], cube.origin[2])
    return tuple((origin[i], origin[i] + cube.size[i]) for i in range(3))


def _overlaps(a, b, slack: float = 0.01) -> bool:
    return all(min(a[i][1], b[i][1]) - max(a[i][0], b[i][0]) > slack for i in range(3))


def _assert_no_body_clash(all_parts: tuple[ArmorPart, ...]) -> None:
    """腿/脚上的件不准钻进躯干和手臂的体积。

    腿甲的顶沿天然要往上够到髋线（y12）才不露缝，越界一点点就撞上从 y12 起的
    躯干和手臂。静止三视图里这种穿模常常被手臂自己挡住看不见，**一抬手就露**，
    所以只能靠算。左件对左臂即可：`_assert_mirror_symmetry` 已经保证右件是镜像。
    """
    for part in all_parts:
        for cube in part.cubes:
            if not cube.mount.startswith("LEFT"):
                continue
            box = _world_box(cube)
            for name, volume in (("手臂", VANILLA_ARM_LEFT), ("躯干", VANILLA_BODY)):
                if _overlaps(box, volume):
                    raise ValueError(
                        f"{part.key}/{cube.name} 钻进原版{name}体积 "
                        f"(x{box[0][0]:.2f}~{box[0][1]:.2f} y{box[1][0]:.2f}~{box[1][1]:.2f} "
                        f"z{box[2][0]:.2f}~{box[2][1]:.2f})——抬手/抬腿会穿帮"
                    )


def _assert_parts_stack_cleanly(all_parts: tuple[ArmorPart, ...]) -> None:
    """护腿和草鞋是**同时穿**的，两件之间也不准有共面。

    单件内的共面由 `_assert_no_coplanar_faces` 管，但它一次只看一件。护腿的底穗
    （y 2.0~3.1）和草鞋的踝箍/结（y 2.0~2.9）在同一段高度上，两件各自都合法、
    合起来照样能 z-fighting。件与件**互相插入是允许的**（草绑腿本来就该压在
    草鞋的绑绳上），只禁止表面重合。
    """
    from itertools import combinations

    for first, second in combinations(all_parts, 2):
        for cube_a in first.cubes:
            box_a = _world_box(cube_a)
            for cube_b in second.cubes:
                box_b = _world_box(cube_b)
                for axis in range(3):
                    overlap = 1.0
                    for other in (k for k in range(3) if k != axis):
                        overlap *= max(0.0, min(box_a[other][1], box_b[other][1])
                                       - max(box_a[other][0], box_b[other][0]))
                    if overlap <= 0.02:
                        continue
                    for face in (0, 1):
                        if abs(box_a[axis][face] - box_b[axis][face]) < 1e-6:
                            raise ValueError(
                                f"{first.key}/{cube_a.name} 与 {second.key}/{cube_b.name} 的 "
                                f"{'xyz'[axis]}-{'min' if face == 0 else 'max'} 面共面于 "
                                f"{box_a[axis][face]}，投影相交 {overlap:.2f}——两件同时穿会 "
                                f"z-fighting，挪开一件"
                            )


def _assert_uv_tiles(all_parts: tuple[ArmorPart, ...]) -> None:
    """每个 cube 的 box-uv 矩形必须整个落在它那格里。

    稻杆的分节靠四条各 8px 宽的色调列，跨界一格就会采到隔壁调子——症状是
    某一块杆板莫名比邻居亮/暗一大截，而这在三视图里极容易被当成"随机长成
    这样"放过去。box-uv 摊面公式和 `_box_uv_faces` 保持一致：宽 2*(sx+sz)，
    高 sy+sz。
    """
    for part in all_parts:
        for cube in part.cubes:
            if cube.uv not in UV_TILES:
                raise ValueError(f"{part.key}/{cube.name}: uv {cube.uv} 不在 UV_TILES 里")
            tw, th = UV_TILES[cube.uv]
            sx, sy, sz = cube.size
            w, h = 2 * (sx + sz), sy + sz
            if w > tw or h > th:
                raise ValueError(
                    f"{part.key}/{cube.name}: box-uv {w:.2f}x{h:.2f} 超出 uv{cube.uv} "
                    f"的 {tw}x{th} 格，会采到隔壁色调；换格或拆件"
                )


def _assert_inner_edges_meet(all_parts: tuple[ArmorPart, ...]) -> None:
    """左右两半在世界 x=0 处只准**相接**，不准互相探过去。

    草甲比兽皮甲肥一圈，围件（绳箍、踝箍、草底）的内缘全靠钉在 0.0 才没糊成
    一坨。这条不变式一旦被后来的改动破坏，症状是「正视两条腿之间那道缝没了」，
    而共面检查抓不到（互相探过去是体积重叠，不是共面）。
    """
    from bbmodel_maker.model.armor_model_common import MOUNT_X

    for part in all_parts:
        for cube in part.cubes:
            offset = MOUNT_X[cube.mount]
            low = cube.origin[0] + offset
            high = low + cube.size[0]
            if cube.mount in ("LEFT_LEG", "LEFT_FOOT") and low < -1e-6:
                raise ValueError(
                    f"{part.key}/{cube.name}: 左件内缘 {low:.3f} < 0，探进右腿空间"
                )
            if cube.mount in ("RIGHT_LEG", "RIGHT_FOOT") and high > 1e-6:
                raise ValueError(
                    f"{part.key}/{cube.name}: 右件外缘 {high:.3f} > 0，探进左腿空间"
                )


def _assert_mirror_symmetry(all_parts: tuple[ArmorPart, ...]) -> None:
    """左右同名件必须严格镜像。x() 手滑写错正负号时，两只脚会长得不一样，
    而三视图正视里这种错误常常被自身遮挡住看不出来。"""
    from bbmodel_maker.model.armor_model_common import MOUNT_X

    for part in all_parts:
        left = {c.name[:-5]: c for c in part.cubes if c.name.endswith("_left")}
        right = {c.name[:-6]: c for c in part.cubes if c.name.endswith("_right")}
        if set(left) != set(right):
            raise ValueError(f"{part.key}: 左右件名不成对 {set(left) ^ set(right)}")
        for name, lc in left.items():
            rc = right[name]
            lx0 = lc.origin[0] + MOUNT_X[lc.mount]
            rx1 = rc.origin[0] + MOUNT_X[rc.mount] + rc.size[0]
            if abs(lx0 + rx1) > 1e-6 or lc.size != rc.size:
                raise ValueError(
                    f"{part.key}/{name}: 左右不镜像（左 x0={lx0:.3f} 右 x1={rx1:.3f}，"
                    f"size {lc.size} vs {rc.size}）"
                )
            if lc.origin[1:] != rc.origin[1:]:
                raise ValueError(f"{part.key}/{name}: 左右 y/z 不一致")


# ─── 贴图 ─────────────────────────────────────────────────────────────────


def _mottle(image, rng, box, count, dark, light, radius):
    """低频软斑。稻草的不匀是**大块**的（湿痕、霉斑、日晒差），高频细线糊在
    一起只会读成噪点。中心浓边缘淡，不留硬边。"""
    x0, y0, x1, y1 = box
    pixels = image.load()
    for _ in range(count):
        cx, cy = rng.uniform(x0, x1), rng.uniform(y0, y1)
        rx, ry = rng.uniform(*radius), rng.uniform(*radius)
        tint = dark if rng.random() < 0.55 else light
        peak = rng.uniform(0.14, 0.36)
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


def make_texture() -> Image.Image:
    """稻杆四色调列 / 旧稻杆两列 / 编织草底 / 搓绳。

    色相取自参考图（稻杆 H39 S38 V56、绳箍 H39 S46 V47、编织底 H40 S48 V38）。
    贴图明度一律比参考高约 1.3×：MC 分轴着色会把南北面乘 0.8、东西面乘 0.6，
    照参考值直接铺会渲成一堆烂泥（兽皮甲那轮实测出来的比例）。

    **所有笔画都是 1 texel 粗。** 这几件的 box-uv 是 1 texel ≈ 1 单位，一笔在
    屏幕上就是二十来个像素；画 3 texel 宽的线放大后是棋盘格，比不画还糟。
    也正因为这个密度，稻杆的"一根根"不是画出来的而是**分色分块**出来的
    （见 UV 段），这里的杆线只负责给每块板一点内部起伏，不承担分节。
    """
    rng = random.Random(0x5C40)
    image = Image.new("RGB", (TEXTURE_SIZE, TEXTURE_SIZE), (172, 150, 108))
    pixels = image.load()

    # 四条稻杆色调列。明度跨度 150~196（约 ±13%），够在 MC 着色后仍分得开，
    # 又不至于让某一根跳成白的。
    straw_cols = (
        (UV_STRAW_A, (196, 176, 132)),
        (UV_STRAW_B, (172, 150, 108)),
        (UV_STRAW_C, (150, 128, 90)),
        (UV_STRAW_D, (182, 162, 120)),
    )
    worn_cols = ((UV_WORN_A, (147, 123, 85)), (UV_WORN_B, (128, 106, 72)))

    def fill(box, base, spread=6, warmth=3):
        x0, y0, x1, y1 = box
        for y in range(y0, y1):
            for x in range(x0, x1):
                jitter = rng.randint(-spread, spread)
                warm = rng.randint(-warmth, warmth + 1)
                pixels[x, y] = tuple(
                    max(0, min(255, ch + off))
                    for ch, off in zip(base, (jitter + warm, jitter, jitter - warm))
                )

    for (u, v), base in straw_cols:
        fill((u, v, u + 8, v + 32), base)
    for (u, v), base in worn_cols:
        fill((u, v, u + 16, v + 32), base)
    fill((0, 32, 32, 64), (127, 107, 68))
    fill((32, 32, 64, 64), (126, 102, 64))

    draw = ImageDraw.Draw(image)

    # 稻杆列：竖向杆线 + 杆节。列距 2px —— 一块杆板只采到 1~4 个 texel 宽，
    # 列距 3 会有整块板一根线都没有，那块就成了素面。
    for (u, v), base in straw_cols:
        dark = tuple(max(0, ch - 26) for ch in base)
        light = tuple(min(255, ch + 24) for ch in base)
        _mottle(image, rng, (u, v, u + 8, v + 32), 5,
                tuple(max(0, ch - 16) for ch in base),
                tuple(min(255, ch + 16) for ch in base), (2.0, 4.5))
        for col in range(u, u + 8, 2):
            draw.line((col, v, col, v + 31), fill=dark)
            for _ in range(rng.randint(2, 4)):        # 杆节 / 折痕
                draw.point((col, rng.randint(v, v + 31)), fill=tuple(max(0, ch - 44) for ch in base))
        for _ in range(6):                            # 被晒白的那几段
            col = rng.randrange(u + 1, u + 8, 2)
            y0 = rng.randint(v, v + 24)
            draw.line((col, y0, col, min(v + 31, y0 + rng.randint(3, 7))), fill=light)

    # 旧稻杆：同样的竖线但更碎，另加横向断口 —— 穗头和折断处就靠它。
    for (u, v), base in worn_cols:
        _mottle(image, rng, (u, v, u + 16, v + 32), 8,
                tuple(max(0, ch - 22) for ch in base),
                tuple(min(255, ch + 22) for ch in base), (2.5, 6.0))
        for col in range(u, u + 16, 2):
            draw.line((col, v, col, v + 31), fill=tuple(max(0, ch - 24) for ch in base))
            for _ in range(rng.randint(2, 5)):
                draw.point((col, rng.randint(v, v + 31)), fill=tuple(max(0, ch - 46) for ch in base))
        for _ in range(12):                           # 横断口
            y0 = rng.randint(v, v + 31)
            x0 = rng.randrange(u, u + 14, 2)
            draw.line((x0, y0, x0 + rng.randint(1, 2), y0), fill=tuple(max(0, ch - 40) for ch in base))

    # 编织草底：横向编纹。草底片的 uv 窗口约 13×2 texel（很扁），所以纹路必须
    # 是**横向**的短断线；竖纹在 2 texel 高的窗口里根本展不开。
    _mottle(image, rng, (0, 32, 32, 64), 12, (104, 86, 54), (156, 134, 92), (3.5, 8.5))
    for row in range(32, 64):
        phase = (row % 4) // 2              # 隔行错半个节距 → 编织的交错感
        for x in range(phase, 32, 4):
            draw.line((x, row, min(31, x + 1), row), fill=(100, 82, 50))
        if row % 4 == 1:                    # 每四行压一条整行的经线
            draw.line((0, row, 31, row), fill=(152, 130, 90))
    for _ in range(20):                     # 磨出来的毛头
        draw.point((rng.randint(0, 31), rng.randint(32, 63)), fill=(178, 158, 116))

    # 搓绳：斜向拧纹（顺一个方向，读成拧过的绳）。画在独立 tile 上再贴回去，
    # 直接在整图上画斜线会有一截探进左边的编织象限。
    #
    # **绳的明度是本图唯一一处刻意背离参考的取色**。参考图量到绳 V47 / 稻杆
    # V56，差 9 个点；照这个差放进 MC，绳和杆只差 6%，round 3 实测绳箍在身上
    # 整个消失了。参考里绳能读出来靠的是它自己的受光角度（真渲染有软阴影），
    # MC 分轴着色给不了这个。所以绳压到 V49、比稻杆暗 27%，用色差把参考里
    # 由光影承担的那份区分补回来。
    cord = image.crop((32, 32, 64, 64))
    cord_draw = ImageDraw.Draw(cord)
    _mottle(cord, rng, (0, 0, 32, 32), 8, (100, 80, 48), (158, 136, 96), (3.0, 7.0))
    for start in range(-8, 32, 3):
        cord_draw.line((start, 31, start + 8, 0), fill=(102, 82, 50))
    for start in range(-6, 32, 6):
        cord_draw.line((start, 31, start + 8, 0), fill=(164, 144, 102))
    image.paste(cord, (32, 32))
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
    all_parts = parts()
    _assert_no_coplanar_faces(all_parts)
    _assert_uv_tiles(all_parts)
    _assert_inner_edges_meet(all_parts)
    _assert_mirror_symmetry(all_parts)
    _assert_no_body_clash(all_parts)
    _assert_parts_stack_cleanly(all_parts)
    return write_material_assets(
        MATERIAL,
        all_parts,
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
