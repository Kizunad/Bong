#!/usr/bin/env python3
"""骨架背篓（gu_jia_bei_lou）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #6（.task-wasteland-gear.md）。穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

外形：兽骨弯成的框架（四根角立骨 + 两道横箍，立骨顶端高出篓口像骨角）夹着一只草编篓身，贴在背后；
篓口沿一圈压边，篓里塞着杂物（卷起的兽皮、残陶罐颈、两根骨茬、一圈麻绳、一只锈铁环）；
篓底拖一圈破兽皮裙边；后面钉两块锈铁补片；皮绳肩带从篓背上沿绕过肩（骑在手臂上沿，避开头和躯干）
一路下到胸前，系在腰间带扣上。篓子整体在 z∈[2.6,10]，高出肩（骨角、杂物）但全部在头的后面，不挡脸。

分部件：part_*() 逐件写，--part 单件预览；骨骼树在 BONES 里一处定义，按挂点分骨。
用法：
    python3 modelScript/gear/gu_jia_bei_lou/gen_gu_jia_bei_lou.py --self-test   # 门禁差分自证
    python3 modelScript/gear/gu_jia_bei_lou/gen_gu_jia_bei_lou.py --part basket # 单件预览
    python3 modelScript/gear/gu_jia_bei_lou/gen_gu_jia_bei_lou.py --worn        # 只出穿戴三面
    python3 modelScript/gear/gu_jia_bei_lou/gen_gu_jia_bei_lou.py               # 全量：模型 + check.png
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
import harness  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质（RGB，每种占贴图一个 8×8 色块）
MATS = {
    "bone":       (206, 194, 164),
    "bone_lt":    (224, 214, 186),   # 骨关节头 / 高光
    "bone_dk":    (160, 146, 116),   # 骨内侧阴面
    "bone_stain": (126, 106, 78),    # 骨裂 / 老化斑
    "weave":      (160, 138, 92),    # 草编主色
    "weave_lt":   (186, 164, 112),   # 编条亮面
    "weave_dk":   (112, 94, 62),     # 编缝 / 篓壁底
    "weave_rim":  (96, 78, 52),      # 口沿压边
    "cord":       (150, 126, 84),    # 草绳
    "cord_dk":    (104, 84, 54),
    "hide":       (118, 78, 56),     # 兽皮
    "hide_dk":    (84, 56, 42),
    "hide_lt":    (150, 108, 78),
    "hide_grey":  (112, 106, 98),
    "iron":       (76, 70, 68),
    "rust":       (150, 84, 48),
    "rust_dk":    (104, 56, 34),
    "clay":       (138, 112, 88),
    "clay_dk":    (96, 74, 56),
    "void":       (28, 24, 22),
}
SOFT = ("cord", "cord_dk", "hide", "hide_dk", "hide_lt", "hide_grey")
# 软件（绳 / 皮）压在任何材质上都允许薄重叠；硬件之间（骨 / 草编 / 铁 / 陶）互穿仍要报
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("clay", "clay_dk", "hide", "hide_dk"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("pack_frame", "root", (0.0, 18.0, 5.8), "兽骨框架：四根角立骨 / 两道横箍 / 绑扎 / 骨关节头"),
    ("frame_extra", "pack_frame", (-6.6, 20.0, 9.0), "框架上刻意不对称的零件：左骨断茬 / 右骨夹板"),
    ("pack_basket", "root", (0.0, 17.0, 5.8), "草编篓身：四壁 / 底 / 编条 / 口沿压边"),
    ("basket_damage", "pack_basket", (-3.7, 16.8, 9.3), "后壁左侧破洞 + 翘起 / 垂落的断草茎"),
    ("pack_skirt", "root", (0.0, 10.2, 8.6), "篓底破兽皮裙边（参差下摆）"),
    ("pack_patches", "root", (0.0, 17.0, 9.4), "背面锈铁补片 ×2 + 铆钉 + 锈斑"),
    ("junk_fill", "pack_basket", (0.0, 21.0, 5.8), "篓内填料：碎皮 / 草团，托着上面的杂物"),
    ("junk_roll", "junk_fill", (-3.1, 25.0, 5.9), "卷起的兽皮卷 + 绑绳"),
    ("junk_pot", "junk_fill", (1.6, 24.5, 6.0), "残陶罐颈 + 唇沿"),
    ("junk_bone_l", "junk_fill", (-4.6, 26.0, 4.6), "左侧斜插的两根骨茬"),
    ("junk_bone_r", "junk_fill", (4.4, 26.0, 7.8), "右侧分叉骨 + 碎骨片"),
    ("junk_coil", "junk_fill", (2.6, 26.0, 8.0), "挂在罐后的麻绳圈"),
    ("hang_ring", "pack_frame", (-6.8, 26.0, 8.6), "左立骨上挂的锈铁环"),
    *harness.HARNESS_BONES,
]

# 刻意不对称：杂物（各摆各的）、框架断茬、裙边参差、补片、垂尾
ASYM_BONES = frozenset({"frame_extra", "basket_damage", "pack_skirt", "pack_patches", "junk_fill", "junk_roll", "junk_pot",
                        "junk_bone_l", "junk_bone_r", "junk_coil", "hang_ring"}) | harness.HARNESS_ASYM


# ───────────────────────────────────────────────────────────── 常量：篓身尺寸
Y_BOT, Y_RIM = 10.2, 22.4            # 壁底 → 口沿下
Z_FRONT, Z_BACK = 2.6, 9.0           # 篓身前（贴背）/ 后壁外面（不含编条）
X_IN, X_OUT = 5.1, 5.9               # 侧壁内 / 外面
STRAND = 0.3                         # 编条厚度
ROW_Y0, ROW_H, ROW_PITCH, ROWS = 11.2, 1.2, 1.4, 8


def _row_y(i: int) -> tuple[float, float]:
    y0 = ROW_Y0 + i * ROW_PITCH
    return y0, y0 + ROW_H


# ───────────────────────────────────────────────────────────── 分部件：篓身
def part_basket(g: Gear) -> None:
    """草编篓身：前 / 后 / 左右壁底 + 篓底 + 逐行编条（砖缝错位）+ 口沿压边。"""
    b = "pack_basket"
    g.box(b, "wall_front", (-X_OUT, Y_BOT, Z_FRONT), (X_OUT, Y_RIM, Z_FRONT + 0.7), "weave_dk")
    g.box(b, "wall_back", (-X_OUT, Y_BOT, Z_BACK - 0.7), (X_OUT, Y_RIM, Z_BACK), "weave_dk")
    g.pair(b, "wall_side", (X_IN, Y_BOT, Z_FRONT + 0.7), (X_OUT, Y_RIM, Z_BACK - 0.7), "weave_dk")
    g.box(b, "floor", (-X_IN, Y_BOT, Z_FRONT + 0.7), (X_IN, Y_BOT + 0.8, Z_BACK - 0.7), "weave_rim")
    # 后壁编条：偶数行中心 0 / ±2.4 / ±4.8（宽 2.2），奇数行 ±1.2 / ±3.6（宽 2.2）+ 两端半条
    for i in range(ROWS):
        y0, y1 = _row_y(i)
        mat = "weave" if i % 2 == 0 else "weave_lt"
        zf, zt = Z_BACK, Z_BACK + STRAND
        if i % 2 == 0:
            g.box(b, f"back_r{i}_c", (-1.1, y0, zf), (1.1, y1, zt), mat)
            for k, cx in enumerate((2.4, 4.8)):
                g.pair(b, f"back_r{i}_{k}", (cx - 1.1, y0, zf), (cx + 1.1, y1, zt), mat)
        else:
            for k, cx in enumerate((1.2, 3.6)):
                g.pair(b, f"back_r{i}_{k}", (cx - 1.1, y0, zf), (cx + 1.1, y1, zt), "weave" if k else mat)
            g.pair(b, f"back_r{i}_e", (4.9, y0, zf), (5.9, y1, zt), "weave")
        # 竖向篾条：奇数行里压在横条上面（偶数行横条压在竖条上），织成「一上一下」的棋盘感
        if i % 2 == 1:
            g.box(b, f"stake_back_{i}_c", (-0.35, y0 - 0.1, zt - 0.1), (0.35, y1 + 0.1, zt + 0.25), "weave_dk")
            for k, cx in enumerate((2.4, 4.8)):
                g.pair(b, f"stake_back_{i}_{k}", (cx - 0.35, y0 - 0.1, zt - 0.1), (cx + 0.35, y1 + 0.1, zt + 0.25), "weave_dk")
            for k, cz in enumerate((4.3, 6.2, 8.1)):
                g.pair(b, f"stake_side_{i}_{k}", (X_OUT + STRAND - 0.1, y0 - 0.1, cz - 0.4), (X_OUT + STRAND + 0.25, y1 + 0.1, cz + 0.4), "weave_dk")
        # 侧壁编条（沿 z 方向，错位）：每行三条或「半 + 两条 + 半」
        zs = ((3.4, 5.1), (5.35, 7.05), (7.3, 9.0)) if i % 2 == 0 else ((3.4, 4.2), (4.45, 6.15), (6.4, 8.1), (8.35, 9.0))
        for k, (za, zb) in enumerate(zs):
            g.pair(b, f"side_r{i}_{k}", (X_OUT, y0, za), (X_OUT + STRAND, y1, zb),
                   mat if (k + i) % 2 == 0 else "weave")
    # 口沿压边：一圈较厚的深色草辫，比壁面略凸
    g.box(b, "rim_back", (-6.1, Y_RIM, Z_BACK - 0.7), (6.1, 23.6, Z_BACK + 0.4), "weave_rim")
    g.box(b, "rim_front", (-6.1, Y_RIM, Z_FRONT), (6.1, 23.6, Z_FRONT + 0.7), "weave_rim")
    g.pair(b, "rim_side", (X_IN, Y_RIM, Z_FRONT + 0.7), (6.1, 23.6, Z_BACK - 0.7), "weave_rim")
    for k, cx in enumerate((0.7, 2.1, 3.5, 4.9)):                         # 压边上的辫纹（后沿，交错高低）
        top = 23.9 if k % 2 == 0 else 23.85
        g.pair(b, f"rim_plait_{k}", (cx - 0.5, 23.6, Z_BACK - 0.5), (cx + 0.5, top, Z_BACK + 0.2), "weave" if k % 2 else "weave_lt")
    for k, cx in enumerate((0.7, 2.1, 3.5, 4.9)):                         # 前沿（贴背那面）
        g.pair(b, f"rim_plait_front_{k}", (cx - 0.5, 23.6, Z_FRONT + 0.1), (cx + 0.5, 23.85, Z_FRONT + 0.6), "weave")
    for k, cz in enumerate((4.2, 5.6, 7.0)):                              # 侧沿
        g.pair(b, f"rim_plait_side_{k}", (X_IN + 0.2, 23.6, cz - 0.5), (5.9, 23.85, cz + 0.5), "weave" if k % 2 else "weave_lt")


def part_damage(g: Gear) -> None:
    """后壁左侧一处破洞：暗色洞口压在编条上，洞缘翘起参差的断草茎（左右不对称，单独一骨）。"""
    b = "basket_damage"
    zf = Z_BACK + STRAND
    g.box(b, "hole_dark", (-4.8, 15.6, zf), (-2.6, 18.0, zf + 0.3), "void")
    # (x 起点, y 起点, 宽, 高)：上缘三根向上翘，下缘三根向下垂
    frays = ((-4.9, 18.0, 0.6, 0.7), (-4.0, 18.0, 0.4, 0.4), (-3.1, 18.0, 0.5, 0.6),
             (-4.9, 15.1, 0.5, 0.5), (-3.8, 14.8, 0.5, 0.8), (-2.9, 15.2, 0.5, 0.4))
    for k, (x, y0, w, h) in enumerate(frays):
        g.box(b, f"hole_fray_{k}", (x, y0, zf + 0.05), (x + w, y0 + h, zf + 0.5), "weave_lt" if k % 2 else "weave")


# ───────────────────────────────────────────────────────────── 分部件：兽骨框架
HOOPS = (14.6, 19.0)                 # 两道横箍的 y 中心
UP_X = (6.2, 7.0)                    # 立骨外缘 x 范围（右）
UP_Z_FRONT, UP_Z_BACK = (Z_FRONT, 3.8), (8.4, 10.0)


def part_frame(g: Gear) -> None:
    """兽骨框架：四根角立骨（顶端高出篓口）+ 两道横箍 + 草绳绑扎 + 骨关节头。"""
    b = "pack_frame"
    # 立骨：前两根短（到 y25.5），后两根长（到 27.8，高出篓口像骨角）
    g.pair(b, "up_front", (UP_X[0], 9.4, UP_Z_FRONT[0]), (UP_X[1], 25.2, UP_Z_FRONT[1]), "bone")
    g.pair(b, "up_back", (UP_X[0], 9.4, UP_Z_BACK[0]), (UP_X[1], 27.4, UP_Z_BACK[1]), "bone")
    # 立骨外侧一道凸起的骨脊（比骨面高 0.3px），给骨身一点阴影线
    g.pair(b, "up_front_ridge", (UP_X[1] - 0.1, 10.0, 3.0), (UP_X[1] + 0.3, 24.8, 3.4), "bone_dk")
    g.pair(b, "up_back_ridge", (UP_X[1] - 0.1, 10.0, 8.8), (UP_X[1] + 0.3, 27.0, 9.6), "bone_dk")
    # 立骨上留着的枝桩：削短的小枝，前后立骨上下错开，像用骨头现拼的架子
    for k, y in enumerate((12.9, 16.8, 21.6)):
        g.pair(b, f"stub_front_{k}", (UP_X[1], y, 2.7), (UP_X[1] + 0.7, y + 0.6, 3.5), "bone_lt")
    for k, y in enumerate((12.6, 24.4)):
        g.pair(b, f"stub_back_{k}", (UP_X[1], y, 8.5), (UP_X[1] + 0.7, y + 0.6, 9.3), "bone_lt")
    # 横箍斜撑：侧面两道横箍之间斜着一根骨条（前下 → 后上），把四角骨架读成「架」
    for sx, tag in ((1, "r"), (-1, "l")):
        g.rod(b, f"brace_side_{tag}", (sx * 6.7, 15.3, 4.0), (sx * 6.7, 18.3, 8.2), 0.4, "bone_dk")
    # 立骨顶端的牙口：削斜的两级台阶
    g.pair(b, "tip_front", (UP_X[0] + 0.2, 26.1, UP_Z_FRONT[0] + 0.3), (UP_X[1] - 0.2, 26.7, UP_Z_FRONT[1] - 0.3), "bone_dk")
    g.pair(b, "tip_back", (UP_X[0] + 0.2, 28.4, UP_Z_BACK[0] + 0.4), (UP_X[1] - 0.2, 28.9, UP_Z_BACK[1] - 0.4), "bone_dk")
    # 骨关节头：立骨两端略粗的骨节
    g.pair(b, "knob_front_top", (UP_X[0] - 0.2, 25.2, UP_Z_FRONT[0] - 0.1), (UP_X[1] + 0.2, 26.1, UP_Z_FRONT[1] + 0.1), "bone_lt")
    g.pair(b, "knob_back_top", (UP_X[0] - 0.2, 27.4, UP_Z_BACK[0] - 0.1), (UP_X[1] + 0.2, 28.4, UP_Z_BACK[1] + 0.1), "bone_lt")
    g.pair(b, "knob_front_bot", (UP_X[0] - 0.2, 8.6, UP_Z_FRONT[0] - 0.1), (UP_X[1] + 0.2, 9.4, UP_Z_FRONT[1] + 0.1), "bone_lt")
    g.pair(b, "knob_back_bot", (UP_X[0] - 0.2, 8.6, UP_Z_BACK[0] - 0.1), (UP_X[1] + 0.2, 9.4, UP_Z_BACK[1] + 0.1), "bone_lt")
    # 横箍：绕篓一圈（后面一根横贯，两侧各一根连前后立骨）
    for i, yc in enumerate(HOOPS):
        y0, y1 = yc - 0.55, yc + 0.55
        g.box(b, f"hoop_back_{i}", (-UP_X[0], y0, Z_BACK + STRAND), (UP_X[0], y1, UP_Z_BACK[1]), "bone")
        for k, cx in enumerate((0.0, 2.8, 5.0)):                          # 横箍背面的骨瘤（左右对称，中间一颗）
            if k == 0:
                g.box(b, f"hoop_node_{i}_{k}", (-0.6, y0 - 0.15, UP_Z_BACK[1]), (0.6, y1 + 0.15, UP_Z_BACK[1] + 0.3), "bone_lt")
            else:
                g.pair(b, f"hoop_node_{i}_{k}", (cx - 0.5, y0 - 0.15, UP_Z_BACK[1]), (cx + 0.5, y1 + 0.15, UP_Z_BACK[1] + 0.3), "bone_lt")
        g.pair(b, f"hoop_side_{i}", (UP_X[0], y0, UP_Z_FRONT[1]), (UP_X[0] + 0.7, y1, UP_Z_BACK[0]), "bone")
        # 横箍头：两端略粗的骨节伸出立骨
        g.pair(b, f"hoop_knuckle_{i}", (UP_X[1], y0 - 0.2, UP_Z_BACK[0] + 0.2), (UP_X[1] + 0.5, y1 + 0.2, UP_Z_BACK[1] - 0.2), "bone_lt")
    # 草绳绑扎：横箍与立骨相交处，比骨略宽的一圈，加一个结
    for i, yc in enumerate(HOOPS):
        for zc, tag in (((UP_Z_FRONT[0] + UP_Z_FRONT[1]) / 2, "f"), ((UP_Z_BACK[0] + UP_Z_BACK[1]) / 2, "b")):
            g.pair(b, f"lash_{tag}{i}", (UP_X[0] - 0.15, yc - 0.45, zc - 0.95), (UP_X[1] + 0.15, yc + 0.45, zc + 0.95),
                   "cord" if i == 0 else "cord_dk")
            g.pair(b, f"lash_knot_{tag}{i}", (UP_X[1] + 0.15, yc - 0.3, zc - 0.3), (UP_X[1] + 0.55, yc + 0.3, zc + 0.3), "cord_dk")
    # 前立骨下端各一道绑带，把立骨扎在篓前壁上
    g.pair(b, "lash_front_low", (UP_X[0] - 0.15, 11.8, UP_Z_FRONT[0] - 0.15), (UP_X[1] + 0.15, 12.6, UP_Z_FRONT[1] + 0.15), "cord")


def part_frame_extra(g: Gear) -> None:
    """框架上的不对称件：左立骨顶端断茬（缺一截、露出髓腔）+ 右立骨一块夹板（草绳扎两道）。"""
    b = "frame_extra"
    # 左后立骨在 y26.4 处断过：上段错位偏 0.4px，断口露出暗色骨髓
    g.box(b, "break_ring", (-UP_X[1] - 0.1, 26.2, UP_Z_BACK[0] - 0.1), (-UP_X[0] + 0.1, 26.6, UP_Z_BACK[1] + 0.1), "bone_stain")
    g.box(b, "break_splinter_a", (-UP_X[0] - 0.4, 26.6, UP_Z_BACK[0] + 0.3), (-UP_X[0] + 0.1, 27.9, UP_Z_BACK[0] + 0.9), "bone_lt")
    g.box(b, "break_splinter_b", (-UP_X[1] - 0.1, 26.6, 9.05), (-UP_X[1] + 0.4, 28.1, 9.55), "bone_dk")
    # 右后立骨外侧的夹板骨 + 两道绑绳
    g.box(b, "splint", (UP_X[1] - 0.05, 16.0, 8.95), (UP_X[1] + 0.45, 22.4, 9.45), "bone_dk")
    for k, y in enumerate((17.0, 21.0)):
        g.box(b, f"splint_lash_{k}", (UP_X[1] - 0.15, y, 8.7), (UP_X[1] + 0.65, y + 0.7, 9.7), "cord_dk")


# ───────────────────────────────────────────────────────────── 分部件：底裙 / 补片
def part_skirt(g: Gear) -> None:
    """篓底破兽皮裙边：沿后壁底缘一排参差的皮条（高低不一、色深浅间隔），两侧各垂一小条。"""
    b = "pack_skirt"
    teeth = ((-5.7, -4.5, 2.2, "hide"), (-4.4, -3.2, 3.1, "hide_dk"), (-3.1, -1.9, 1.7, "hide_lt"),
             (-1.8, -0.5, 2.6, "hide"), (-0.4, 0.9, 1.4, "hide_grey"), (1.0, 2.3, 2.9, "hide_dk"),
             (2.4, 3.5, 1.8, "hide"), (3.6, 4.7, 3.3, "hide_lt"), (4.8, 5.7, 2.0, "hide_dk"))
    for i, (x0, x1, h, mat) in enumerate(teeth):
        g.box(b, f"skirt_tooth_{i}", (x0, Y_BOT - h, Z_BACK - 0.7), (x1, Y_BOT, Z_BACK + 0.2), mat)
        g.box(b, f"skirt_tip_{i}", (x0 + 0.25, Y_BOT - h - 0.7, Z_BACK - 0.5), (x1 - 0.3, Y_BOT - h, Z_BACK), mat)
        g.box(b, f"skirt_frayed_{i}", (x0 + 0.35, Y_BOT - h - 1.1, Z_BACK - 0.3), (x0 + 0.75, Y_BOT - h - 0.7, Z_BACK - 0.05), "hide_dk")
    for k, x in enumerate((-4.9, -1.2, 2.9)):                                  # 缝在皮条上的补丁
        g.box(b, f"skirt_patch_{k}", (x, Y_BOT - 1.2, Z_BACK + 0.2), (x + 1.1, Y_BOT - 0.4, Z_BACK + 0.42), "hide_grey" if k % 2 else "cord")
    g.box(b, "skirt_side_l", (-X_OUT - 0.1, Y_BOT - 1.8, 5.2), (-X_IN - 0.4, Y_BOT, 7.0), "hide_dk")
    g.box(b, "skirt_side_r", (X_IN + 0.4, Y_BOT - 2.6, 4.8), (X_OUT + 0.1, Y_BOT, 6.8), "hide")
    g.box(b, "skirt_stitch", (-4.8, Y_BOT - 0.6, Z_BACK + 0.25), (4.8, Y_BOT - 0.3, Z_BACK + 0.5), "cord_dk")


def part_patches(g: Gear) -> None:
    """背面锈铁补片两块（一大一小，歪着钉）+ 铆钉 + 锈蚀斑；贴在编条外面（z=9.3）。"""
    b = "pack_patches"
    zf = Z_BACK + STRAND
    g.box(b, "plate_a", (2.0, 15.4, zf), (5.1, 18.2, zf + 0.4), "iron")
    g.box(b, "plate_a_rust", (2.3, 15.6, zf + 0.4), (4.2, 16.8, zf + 0.65), "rust")
    g.box(b, "plate_a_rust2", (3.9, 17.0, zf + 0.4), (4.8, 17.9, zf + 0.65), "rust_dk")
    for k, (x, y) in enumerate(((2.3, 17.3), (4.4, 15.6))):
        g.box(b, f"plate_a_rivet_{k}", (x, y, zf + 0.4), (x + 0.5, y + 0.5, zf + 0.75), "rust_dk")
    g.box(b, "plate_b", (-4.5, 19.8, zf), (-2.2, 22.0, zf + 0.35), "rust_dk")
    g.box(b, "plate_b_hi", (-4.3, 20.7, zf + 0.35), (-2.9, 21.8, zf + 0.6), "rust")
    for k, (x, y) in enumerate(((-4.3, 20.0), (-2.7, 20.0))):
        g.box(b, f"plate_b_rivet_{k}", (x, y, zf + 0.35), (x + 0.4, y + 0.4, zf + 0.65), "iron")


# ───────────────────────────────────────────────────────────── 分部件：篓内杂物
def part_junk(g: Gear) -> None:
    """篓内杂物：填料托底，上面露出兽皮卷、残陶罐颈、两侧骨茬、麻绳圈，立骨上挂一只锈铁环。"""
    f = "junk_fill"
    # 填料：几团碎皮草团，顶面高低不平，托住上面的杂物（侧面贴着篓壁内面）
    g.box(f, "fill_base", (-X_IN, 11.0, Z_FRONT + 0.7), (X_IN, 20.6, Z_BACK - 0.7), "hide_dk")
    g.box(f, "fill_lump_a", (-X_IN, 20.6, Z_FRONT + 0.7), (-1.0, 21.8, 6.0), "hide_grey")
    g.box(f, "fill_lump_b", (0.6, 20.6, 5.2), (X_IN, 21.5, Z_BACK - 0.7), "hide")
    g.box(f, "fill_lump_c", (-3.0, 20.6, 6.0), (0.6, 21.3, Z_BACK - 0.7), "cord_dk")
    g.box(f, "fill_lump_d", (1.0, 20.6, Z_FRONT + 0.7), (X_IN, 21.2, 5.2), "weave_dk")
    # 兽皮卷：方形截面柱，上端内凹露卷纹，中间两道绑绳
    r = "junk_roll"
    g.box(r, "roll_body", (-3.4, 21.8, 4.4), (-0.4, 28.6, 7.4), "hide")
    g.box(r, "roll_body_dk", (-3.5, 22.0, 4.6), (-3.0, 28.4, 7.2), "hide_dk")
    g.box(r, "roll_top_rim", (-3.2, 28.6, 4.6), (-0.6, 29.0, 7.2), "hide_lt")
    g.box(r, "roll_top_core", (-2.5, 28.7, 5.3), (-1.3, 29.1, 6.5), "hide_dk")
    g.box(r, "roll_top_spiral", (-2.1, 29.1, 5.7), (-1.7, 29.35, 6.1), "hide")
    for k, y in enumerate((23.4, 26.2)):
        g.box(r, f"roll_lash_{k}", (-3.6, y, 4.2), (-0.2, y + 0.7, 7.6), "cord")
    g.box(r, "roll_lash_knot", (-3.7, 23.6, 5.6), (-3.3, 24.0, 6.4), "cord_dk")
    # 残陶罐颈：矮柱 + 外翻唇沿 + 罐口暗洞 + 一道裂纹
    p = "junk_pot"
    g.box(p, "pot_neck", (0.4, 21.8, 4.6), (3.6, 26.4, 7.6), "clay")
    g.box(p, "pot_neck_dk", (3.1, 22.0, 4.8), (3.7, 25.8, 7.4), "clay_dk")
    g.box(p, "pot_lip", (0.1, 26.4, 4.3), (3.9, 27.2, 7.9), "clay")
    g.box(p, "pot_mouth", (0.9, 27.2, 4.8), (3.1, 27.45, 7.4), "void")
    g.box(p, "pot_crack", (1.3, 24.0, 4.4), (1.6, 27.0, 4.7), "clay_dk")
    g.box(p, "pot_chip", (3.4, 26.7, 4.2), (4.0, 27.1, 5.4), "clay_dk")
    g.box(p, "pot_band", (0.3, 23.0, 4.5), (3.9, 23.6, 7.7), "cord_dk")
    # 左侧骨茬两根：斜插，露在篓外的一端是白色骨茬断面
    q = "junk_bone_l"
    g.rod(q, "shard_l_a", (-3.9, 22.0, 5.4), (-4.6, 29.6, 5.2), 0.5, "bone")
    g.box(q, "shard_l_a_tip", (-5.1, 29.6, 4.8), (-4.1, 30.3, 5.6), "bone_lt")
    g.rod(q, "shard_l_b", (-3.8, 22.0, 7.8), (-3.2, 30.4, 7.9), 0.4, "bone_dk")
    g.box(q, "shard_l_b_tip", (-3.6, 30.4, 7.5), (-2.8, 31.0, 8.2), "bone_stain")
    # 右侧分叉骨：主干 + 向外岔出的一枝 + 一枚碎骨片斜靠
    q = "junk_bone_r"
    g.rod(q, "fork_main", (4.4, 21.8, 7.2), (4.5, 29.0, 7.6), 0.6, "bone")
    g.rod(q, "fork_branch", (4.4, 26.4, 7.5), (5.2, 30.2, 7.7), 0.4, "bone_lt")
    g.box(q, "fork_knob", (3.8, 29.0, 7.2), (5.1, 29.8, 8.0), "bone_lt")
    g.box(q, "chip_slab", (-0.8, 21.8, 7.95), (0.4, 25.2, 8.25), "bone_dk")
    # 麻绳圈：竖直挂在罐后，八段短杆围成环，外加一个绑结
    g.ring("junk_coil", "coil", (2.2, 27.5, 8.075), 2.0, 0.9, 0.35, "cord")
    g.box("junk_coil", "coil_knot", (1.7, 25.2, 7.9), (2.7, 26.0, 8.3), "cord_dk")
    # 立骨上挂的锈铁环 + 系绳（挂在左后立骨外侧）
    h = "hang_ring"
    g.ring(h, "iron_ring", (-8.6, 24.4, 9.2), 1.5, 0.7, 0.8, "rust")
    g.box(h, "ring_tie", (-7.6, 24.0, 8.6), (-6.85, 24.8, 9.9), "cord_dk")


# 肩带 / 胸带 / 腰带走 _kit/harness.py（背包类共用的挂具）
HARNESS = harness.HarnessMats(
    strap="hide", strap_dk="hide_dk", pad="hide_lt", stitch="cord_dk", hard="iron", hard_pin="rust",
    hard_rust="rust_dk", toggle="bone_lt", toggle_pin="bone_stain", hole="void")

PART_FUNCS = {
    "basket": part_basket,
    "damage": part_damage,
    "frame": part_frame,
    "frame_extra": part_frame_extra,
    "skirt": part_skirt,
    "patches": part_patches,
    "junk": part_junk,
    **harness.harness_parts(HARNESS),
}

SPEC = GearSpec(
    gear_id="gu_jia_bei_lou",
    model_name="GuJiaBeiLou",
    title="骨架背篓 / gu_jia_bei_lou",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：背（篓身 z∈[2.6,10]）+ 双肩带 + 腰前带",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
