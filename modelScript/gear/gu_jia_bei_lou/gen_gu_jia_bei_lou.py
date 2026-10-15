#!/usr/bin/env python3
"""骨架背篓（gu_jia_bei_lou）Blockbench .bbmodel 程序化生成器 —— Round 3/3：几何补强 + 像素贴图。

Round 3 补强（Round 1 自记的不足）：篓身由「方盒 + 砖缝横条」改成圆篓——俯视是切角八边形，
侧面从篓底到篓腹逐行鼓出、到口沿再收一点（腹宽 ±5.9，底 ±5.5，口 ±5.45），口沿压边也是八边形；
四根立骨仍立在外接方框的四角上，像方形骨架里托着一只圆篓。贴图见 gear_kit.write_textured（画笔库 gear_*）。


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
X_OUT = 5.9                          # 篓腹最宽处的外缘
STRAND = 0.3                         # 编条厚度
WALL = 0.7                           # 壁板厚度
CHAMF = 1.9                          # 俯视八边形的切角（x、z 方向各切这么多）
ROW_Y0, ROW_H, ROW_PITCH, ROWS = 11.2, 1.2, 1.4, 8
# 每行的外缘半宽：底小、腹大、口收（俯视深度不变，只有左右鼓出）
ROW_HX = (5.5, 5.75, 5.9, 5.9, 5.9, 5.8, 5.65, 5.45)
BAND_HX = 5.4                        # 篓底一圈
RIM_HX = 5.7                         # 口沿压边外缘半宽
FILL_X = 3.3                         # 篓内填料的半宽（要缩在八边形内壁里，不能戳出切角）
EDGE_MARGIN = 0.25                   # 编条离切角接缝的退让，免得与斜面编条互穿


def _row_y(i: int) -> tuple[float, float]:
    y0 = ROW_Y0 + i * ROW_PITCH
    return y0, y0 + ROW_H


# ───────────────────────────────────────────────────────────── 分部件：篓身
def _facet_pair(g: Gear, bone: str, name: str, hx: float, zf: float, zb: float, y0: float, y1: float,
                inner: float, outer: float, mat: str, trim: float = 0.0) -> None:
    """八边形的四条 45° 斜边（前右 / 后右 + 镜像的左边）：外缘线向内 inner、向外 outer 的一块竖板。

    外缘线：后斜边 (hx-CHAMF, zb) → (hx, zb-CHAMF)，前斜边 (hx-CHAMF, zf) → (hx, zf+CHAMF)。
    trim 是两端各缩进线长的比例（编条要留接缝，壁板填满）。
    """
    thick = inner + outer
    shift = (outer - inner) / 2                  # 板中线相对外缘线向外的偏移
    ym = (y0 + y1) / 2
    for tag, z_edge, z_far, nz in (("b", zb, zb - CHAMF, 1.0), ("f", zf, zf + CHAMF, -1.0)):
        for side, sx in (("r", 1.0), ("l", -1.0)):
            nx, nzz = 0.7071 * sx, 0.7071 * nz   # 外法线（水平，指向斜面外侧）
            pa = [sx * (hx - CHAMF), ym, z_edge]
            pb = [sx * hx, ym, z_far]
            for p in (pa, pb):
                p[0] += nx * shift
                p[2] += nzz * shift
            if trim:
                da = [pb[k] - pa[k] for k in range(3)]
                pa = [pa[k] + da[k] * trim for k in range(3)]
                pb = [pb[k] - da[k] * trim for k in range(3)]
            g.rig.shaft(bone, f"{name}_{tag}_{side}", tuple(pa), tuple(pb), thick / 2, (y1 - y0) / 2, mat=mat,
                        extend=0.0 if trim else thick * 0.2)


def _octagon_ring(g: Gear, bone: str, name: str, hx: float, zf: float, zb: float, y0: float, y1: float,
                  inner: float, outer: float, mat: str) -> None:
    """切角八边形的一圈壳：前后两块平板 + 左右一对侧板 + 四块斜板，外缘线在 (±hx, zf..zb)。"""
    flat = hx - CHAMF
    shift = (outer - inner) / 2
    thick = inner + outer
    zc_b, zc_f = zb + shift, zf - shift
    g.box(bone, f"{name}_back", (-flat, y0, zc_b - thick / 2), (flat, y1, zc_b + thick / 2), mat)
    g.box(bone, f"{name}_front", (-flat, y0, zc_f - thick / 2), (flat, y1, zc_f + thick / 2), mat)
    xs = hx + shift
    g.pair(bone, f"{name}_side", (xs - thick / 2, y0, zf + CHAMF), (xs + thick / 2, y1, zb - CHAMF), mat)
    _facet_pair(g, bone, f"{name}_facet", hx, zf, zb, y0, y1, inner, outer, mat)


def _clip(lo: float, hi: float, limit: float) -> tuple[float, float] | None:
    """把编条的 [lo, hi] 截到 ≤ limit；截完不足 0.5px 宽就不要这条。"""
    hi = min(hi, limit)
    return (lo, hi) if hi - lo >= 0.5 else None


def part_basket(g: Gear) -> None:
    """圆篓身：俯视八边形的壁板逐行鼓出（底小腹大口收）+ 篓底 + 后壁 / 侧壁 / 斜面的编条（错位）+ 八边形口沿压边。"""
    b = "pack_basket"
    # 篓底一圈（深色草辫）+ 内底
    _octagon_ring(g, b, "band", BAND_HX, Z_FRONT, Z_BACK, Y_BOT, ROW_Y0 - 0.1, 0.8, 0.0, "weave_rim")
    g.box(b, "floor", (-FILL_X, Y_BOT, Z_FRONT + 0.9), (FILL_X, Y_BOT + 0.7, Z_BACK - 0.9), "weave_rim")
    for i in range(ROWS):
        y0, y1 = _row_y(i)
        hx = ROW_HX[i]
        top = Y_RIM if i == ROWS - 1 else y1 + 0.1
        _octagon_ring(g, b, f"wall{i}", hx, Z_FRONT, Z_BACK, y0 - 0.1, top, WALL, 0.0, "weave_dk")
        mat = "weave" if i % 2 == 0 else "weave_lt"
        flat = hx - CHAMF - EDGE_MARGIN
        zf, zt = Z_BACK, Z_BACK + STRAND
        # 后壁编条：偶数行中心 0 / ±2.4（宽 2.2），奇数行 ±1.2 / ±3.6（宽 2.2）；超出平面段的截掉
        if i % 2 == 0:
            g.box(b, f"back_r{i}_c", (-1.1, y0, zf), (1.1, y1, zt), mat)
            spans = [(1.3, 3.5), (3.7, 5.9)]
        else:
            spans = [(0.1, 2.3), (2.5, 4.7), (4.9, 5.9)]
        for k, (lo, hi) in enumerate(spans):
            span = _clip(lo, hi, flat)
            if span:
                g.pair(b, f"back_r{i}_{k}", (span[0], y0, zf), (span[1], y1, zt), mat if k % 2 == 0 else "weave")
        # 竖向篾条：奇数行里压在横条上面，织成「一上一下」的棋盘感
        if i % 2 == 1:
            g.box(b, f"stake_back_{i}_c", (-0.35, y0 - 0.1, zt - 0.1), (0.35, y1 + 0.1, zt + 0.25), "weave_dk")
            for k, cx in enumerate((2.4,)):
                g.pair(b, f"stake_back_{i}_{k}", (cx - 0.35, y0 - 0.1, zt - 0.1), (cx + 0.35, y1 + 0.1, zt + 0.25), "weave_dk")
            for k, cz in enumerate((5.1, 6.5)):
                g.pair(b, f"stake_side_{i}_{k}", (hx + STRAND - 0.1, y0 - 0.1, cz - 0.4), (hx + STRAND + 0.25, y1 + 0.1, cz + 0.4), "weave_dk")
        # 侧壁编条（沿 z 方向，错位）
        z_lo, z_hi = Z_FRONT + CHAMF + EDGE_MARGIN, Z_BACK - CHAMF - EDGE_MARGIN
        zs = ((z_lo, 5.7), (5.9, z_hi)) if i % 2 == 0 else ((z_lo, 5.3), (5.5, z_hi))
        for k, (za, zb) in enumerate(zs):
            g.pair(b, f"side_r{i}_{k}", (hx, y0, za), (hx + STRAND, y1, zb), mat if (k + i) % 2 == 0 else "weave")
        # 斜面编条：每个斜面一条，前后斜面颜色交错
        _facet_pair(g, b, f"facet_r{i}", hx, Z_FRONT, Z_BACK, y0, y1, 0.0, STRAND, mat if i % 2 == 0 else "weave", trim=0.14)
    # 口沿压边：一圈较厚的深色草辫八边形，比壁面略凸
    rim_front, rim_back = Z_FRONT, Z_BACK + 0.4
    _octagon_ring(g, b, "rim", RIM_HX, rim_front, rim_back, Y_RIM, 23.6, 0.9, 0.0, "weave_rim")
    rim_flat = RIM_HX - CHAMF
    for k, cx in enumerate((0.7, 2.1, rim_flat - 0.5)):                     # 压边上的辫纹（后沿，交错高低）
        top = 23.9 if k % 2 == 0 else 23.85
        g.pair(b, f"rim_plait_{k}", (cx - 0.5, 23.6, rim_back - 0.7), (cx + 0.5, top, rim_back - 0.1), "weave" if k % 2 else "weave_lt")
        g.pair(b, f"rim_plait_front_{k}", (cx - 0.5, 23.6, rim_front + 0.1), (cx + 0.5, 23.85, rim_front + 0.7), "weave")
    for k, cz in enumerate((5.0, 6.5)):                                     # 侧沿
        g.pair(b, f"rim_plait_side_{k}", (RIM_HX - 0.7, 23.6, cz - 0.5), (RIM_HX - 0.1, 23.85, cz + 0.5), "weave" if k % 2 else "weave_lt")


def part_damage(g: Gear) -> None:
    """后壁左侧一处破洞：暗色洞口压在编条上，洞缘翘起参差的断草茎（左右不对称，单独一骨）。"""
    b = "basket_damage"
    zf = Z_BACK + STRAND
    g.box(b, "hole_dark", (-3.7, 15.6, zf), (-1.5, 18.0, zf + 0.3), "void")
    # (x 起点, y 起点, 宽, 高)：上缘三根向上翘，下缘三根向下垂（都在后壁平面段 |x|<4 之内）
    frays = ((-3.8, 18.0, 0.6, 0.7), (-2.9, 18.0, 0.4, 0.4), (-2.0, 18.0, 0.5, 0.6),
             (-3.8, 15.1, 0.5, 0.5), (-2.7, 14.8, 0.5, 0.8), (-1.8, 15.2, 0.5, 0.4))
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
    teeth = ((-3.6, -2.8, 2.2, "hide"), (-2.7, -1.9, 3.1, "hide_dk"), (-1.8, -1.0, 1.7, "hide_lt"),
             (-0.9, -0.1, 2.6, "hide"), (0.0, 0.8, 1.4, "hide_grey"), (0.9, 1.7, 2.9, "hide_dk"),
             (1.8, 2.6, 1.8, "hide"), (2.7, 3.6, 3.3, "hide_lt"))
    for i, (x0, x1, h, mat) in enumerate(teeth):
        g.box(b, f"skirt_tooth_{i}", (x0, Y_BOT - h, Z_BACK - 0.7), (x1, Y_BOT, Z_BACK + 0.2), mat)
        g.box(b, f"skirt_tip_{i}", (x0 + 0.1, Y_BOT - h - 0.7, Z_BACK - 0.5), (x1 - 0.1, Y_BOT - h, Z_BACK), mat)
        g.box(b, f"skirt_frayed_{i}", (x0 + 0.2, Y_BOT - h - 1.1, Z_BACK - 0.3), (x0 + 0.55, Y_BOT - h - 0.7, Z_BACK - 0.05), "hide_dk")
    for k, x in enumerate((-3.2, -1.0, 1.9)):                                  # 缝在皮条上的补丁
        g.box(b, f"skirt_patch_{k}", (x, Y_BOT - 1.2, Z_BACK + 0.2), (x + 1.1, Y_BOT - 0.4, Z_BACK + 0.42), "hide_grey" if k % 2 else "cord")
    g.box(b, "skirt_side_l", (-BAND_HX, Y_BOT - 1.8, 5.2), (-BAND_HX + 0.6, Y_BOT, 7.0), "hide_dk")
    g.box(b, "skirt_side_r", (BAND_HX - 0.6, Y_BOT - 2.6, 4.8), (BAND_HX, Y_BOT, 6.8), "hide")
    g.box(b, "skirt_stitch", (-3.6, Y_BOT - 0.6, Z_BACK + 0.25), (3.6, Y_BOT - 0.3, Z_BACK + 0.5), "cord_dk")


def part_patches(g: Gear) -> None:
    """背面锈铁补片两块（一大一小，歪着钉）+ 铆钉 + 锈蚀斑；贴在编条外面（z=9.3）。"""
    b = "pack_patches"
    zf = Z_BACK + STRAND
    g.box(b, "plate_a", (1.0, 15.4, zf), (3.7, 18.2, zf + 0.4), "iron")
    g.box(b, "plate_a_rust", (1.3, 15.6, zf + 0.4), (3.1, 16.8, zf + 0.65), "rust")
    g.box(b, "plate_a_rust2", (2.7, 17.0, zf + 0.4), (3.5, 17.9, zf + 0.65), "rust_dk")
    for k, (x, y) in enumerate(((1.2, 17.3), (3.1, 15.6))):
        g.box(b, f"plate_a_rivet_{k}", (x, y, zf + 0.4), (x + 0.5, y + 0.5, zf + 0.75), "rust_dk")
    g.box(b, "plate_b", (-3.4, 19.8, zf), (-1.3, 22.0, zf + 0.35), "rust_dk")
    g.box(b, "plate_b_hi", (-3.2, 20.7, zf + 0.35), (-1.9, 21.8, zf + 0.6), "rust")
    for k, (x, y) in enumerate(((-3.3, 20.0), (-1.8, 20.0))):
        g.box(b, f"plate_b_rivet_{k}", (x, y, zf + 0.35), (x + 0.4, y + 0.4, zf + 0.65), "iron")


# ───────────────────────────────────────────────────────────── 分部件：篓内杂物
def part_junk(g: Gear) -> None:
    """篓内杂物：填料托底，上面露出兽皮卷、残陶罐颈、两侧骨茬、麻绳圈，立骨上挂一只锈铁环。"""
    f = "junk_fill"
    # 填料：几团碎皮草团，顶面高低不平，托住上面的杂物（侧面贴着篓壁内面）
    fz0, fz1 = Z_FRONT + 0.9, Z_BACK - 0.9              # 填料前后缘（缩在八边形内壁里）
    g.box(f, "fill_base", (-FILL_X, 11.0, fz0), (FILL_X, 20.6, fz1), "hide_dk")
    g.box(f, "fill_lump_a", (-FILL_X, 20.6, fz0), (-1.0, 21.8, 6.0), "hide_grey")
    g.box(f, "fill_lump_b", (0.6, 20.6, 5.2), (FILL_X, 21.5, fz1), "hide")
    g.box(f, "fill_lump_c", (-3.0, 20.6, 6.0), (0.6, 21.3, fz1), "cord_dk")
    g.box(f, "fill_lump_d", (1.0, 20.6, fz0), (FILL_X, 21.2, 5.2), "weave_dk")
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
    g.box(p, "pot_neck", (0.0, 21.8, 4.6), (2.5, 26.4, 7.0), "clay")
    g.box(p, "pot_neck_dk", (2.0, 22.0, 4.8), (2.6, 25.8, 6.8), "clay_dk")
    g.box(p, "pot_lip", (-0.2, 26.4, 4.4), (2.7, 27.2, 7.2), "clay")
    g.box(p, "pot_mouth", (0.4, 27.2, 4.9), (2.1, 27.45, 6.7), "void")
    g.box(p, "pot_crack", (0.6, 24.0, 4.45), (0.9, 27.0, 4.75), "clay_dk")
    g.box(p, "pot_chip", (2.2, 26.7, 4.3), (2.8, 27.1, 5.5), "clay_dk")
    g.box(p, "pot_band", (-0.2, 23.0, 4.4), (2.8, 23.6, 7.2), "cord_dk")
    # 左侧骨茬两根：斜插，露在篓外的一端是白色骨茬断面
    q = "junk_bone_l"
    g.rod(q, "shard_l_a", (-3.6, 22.0, 6.0), (-4.0, 29.6, 5.9), 0.5, "bone")
    g.box(q, "shard_l_a_tip", (-4.5, 29.6, 5.5), (-3.5, 30.3, 6.3), "bone_lt")
    g.rod(q, "shard_l_b", (-2.6, 22.0, 7.8), (-2.0, 30.4, 7.9), 0.4, "bone_dk")
    g.box(q, "shard_l_b_tip", (-2.4, 30.4, 7.5), (-1.6, 31.0, 8.2), "bone_stain")
    # 右侧分叉骨：主干 + 向外岔出的一枝 + 一枚碎骨片斜靠（主干缩在八边形口沿的斜面里面）
    q = "junk_bone_r"
    g.rod(q, "fork_main", (1.6, 21.8, 7.7), (1.7, 29.0, 7.9), 0.4, "bone")
    g.rod(q, "fork_branch", (1.65, 26.4, 7.8), (3.2, 30.2, 7.6), 0.35, "bone_lt")
    g.box(q, "fork_knob", (1.1, 29.0, 7.4), (2.4, 29.8, 8.2), "bone_lt")
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
