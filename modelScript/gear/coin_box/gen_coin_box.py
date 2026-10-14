#!/usr/bin/env python3
"""骨币匣（coin_box）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #5（.task-wasteland-gear.md）。穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。
物品描述（server/assets/items/workbench_materials.toml）：「木匣内壁涂丹砂隔灵，骨币少漏。」

外形（参考图 coin_box_ref.png / coin_box_exploded.png）：腰前挂一只小木匣（x∈[-1.4,3.6]、y∈[10.4,13.6]、z∈[-6.4,-2.7]，
整个在躯干 / 腿的前面之外，z<-2.05）。匣身是逐块木板拼的（前后侧各三层板，板缝错位），内壁一层丹砂，四个竖角各包一条 L 形锈铁箍
（铁钉铆死），前面正中一块带方孔的铁搭扣；匣盖绕后沿铰链掀开约 32°，盖里也涂丹砂，盖面两道铁条、两枚铁铰链。
腰上一条前后对称的宽皮腰带（上下压边、针脚、铆钉、穿孔，带扣在穿戴者左前），皮带垂一条带孔的带尾；匣后一块皮扣片挂住木匣。
匣右侧（+x）从腰带垂一串骨币（四枚带方孔的骨币串在草绳上，绳尾散成红 / 米色的穗）。腰带下前后各挂几条参差的破布条
（米 / 红 / 灰 / 褐四色，带破洞与针脚补丁）。

骨骼 15 根：腰带 / 挂扣片 / 匣身 / 丹砂内衬 / 匣身铁箍 / 搭扣 / 匣盖 / 盖铁条 / 骨币串 / 币穗 / 前破布 / 后破布，挂件各单独一骨。
分部件：part_*() 逐件写，--part 单件预览；骨骼树在 BONES 里一处定义。
用法：
    python3 modelScript/gear/coin_box/gen_coin_box.py --self-test    # 门禁差分自证
    python3 modelScript/gear/coin_box/gen_coin_box.py --part box     # 单件预览
    python3 modelScript/gear/coin_box/gen_coin_box.py --worn         # 只出穿戴三面
    python3 modelScript/gear/coin_box/gen_coin_box.py                # 全量：模型 + check.png
"""

from __future__ import annotations

import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质（RGB，每种占贴图一个 8×8 色块）
MATS = {
    "wood":        (140, 100, 66),     # 旧木板
    "wood_lt":     (166, 124, 84),
    "wood_dk":     (92, 64, 44),       # 板缝
    "cinnabar":    (172, 44, 36),      # 丹砂内壁
    "cinnabar_dk": (126, 34, 30),
    "iron":        (96, 92, 94),
    "iron_dk":     (62, 58, 60),
    "rust":        (150, 86, 50),
    "leather":     (98, 66, 50),       # 皮带
    "leather_dk":  (70, 48, 38),
    "leather_lt":  (128, 90, 64),
    "bone":        (220, 210, 182),
    "bone_dk":     (168, 154, 124),
    "cord":        (144, 120, 78),
    "cord_dk":     (104, 88, 56),
    "red_cord":    (150, 44, 38),      # 币穗里的红绳
    "rag_beige":   (190, 176, 144),
    "rag_red":     (150, 54, 46),
    "rag_grey":    (112, 112, 112),
    "rag_brown":   (120, 88, 62),
    "stitch":      (62, 48, 36),
    "void":        (26, 22, 20),
}
SOFT = ("leather", "leather_dk", "leather_lt", "cord", "cord_dk", "red_cord",
        "rag_beige", "rag_red", "rag_grey", "rag_brown")
# 软件（皮 / 绳 / 破布）压在任何材质上都允许重叠；木、铁、骨、丹砂之间互穿仍要报
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("iron", "wood", "bone", "leather", "rag_beige", "rag_red", "rag_grey"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("belt_strap", "root", (0.0, 13.0, 0.0), "腰带：前后对称两条宽皮带 + 压边 + 针脚 + 铆钉 + 背面铁滑扣（挂点）"),
    ("belt_buckle", "belt_strap", (-3.0, 13.0, -2.9), "带扣 + 穿孔 + 垂下的带尾（左前，不对称，会晃）"),
    ("belt_tab", "belt_strap", (1.1, 13.3, -2.8), "挂扣片：匣后一块皮扣片 + 铁滑扣 + 铆钉（会晃）"),
    ("box_body", "belt_tab", (1.1, 12.0, -4.5), "匣身：逐块木板（前后侧各三层，板缝错位）+ 底板"),
    ("box_liner", "box_body", (1.1, 12.0, -4.5), "丹砂内衬：底 + 四壁 + 磨损处露深色"),
    ("box_irons", "box_body", (1.1, 12.0, -4.5), "匣身四角 L 形锈铁箍 + 铁钉"),
    ("box_hasp", "box_body", (1.1, 13.0, -6.4), "搭扣：前面带方孔的铁搭扣 + 铁钉 + 锁耳"),
    ("box_lid", "box_body", (1.1, 13.6, -2.7), "匣盖：绕后沿铰链掀开 32°，顶板三块 + 前后沿板 + 内衬（pivot 在铰链，可做开合）"),
    ("coin_string", "belt_strap", (4.2, 13.2, -2.8), "骨币串：草绳 + 四枚带方孔的骨币 + 草绳小结（会晃）"),
    ("coin_tassel", "coin_string", (4.2, 5.4, -2.8), "币穗：绳尾散开的红 / 米色穗（会晃）"),
    ("rags_front", "belt_strap", (-3.0, 12.3, -2.4), "前破布条：腰带下垂的四条参差布条 + 破洞 + 补丁（会晃）"),
    ("rags_back", "belt_strap", (0.0, 12.3, 2.4), "后破布条：腰带下垂的五条参差布条（会晃）"),
]
ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "belt_strap"))

# ───────────────────────────────────────────────────────────── 常量：匣
BX0, BX1 = -1.4, 3.6               # 匣身 x 范围
BY0, BY1 = 10.4, 13.6              # 匣身 y 范围
BZ0, BZ1 = -6.4, -2.7              # 前沿 / 后沿（后沿贴腰带）
WALL = 0.6
CXB = (BX0 + BX1) / 2              # 匣中线 x = 1.1
LID_DEG = 32.0
LID_H = 1.0
HINGE = (CXB, BY1, BZ1)

# 三层板的 y 分界（前后 / 侧壁用同一分界，板缝在外面读成三道横缝）
PLANK_Y = (BY0 + WALL, 11.55, 12.55, BY1)


# ───────────────────────────────────────────────────────────── 分部件：腰带
def part_belt(g: Gear) -> None:
    """腰带：前后各一条宽皮带（左右对称），上下压边、两行针脚、铆钉；前左带扣（铁框 + 舌）+ 一条垂下的带尾 + 穿孔。"""
    b = "belt_strap"
    g.box(b, "strap_f", (-4.9, 12.2, -2.65), (4.9, 13.9, -2.05), "leather")
    g.box(b, "strap_f_t", (-4.9, 13.9, -2.62), (4.9, 14.2, -2.08), "leather_dk")
    g.box(b, "strap_f_b", (-4.9, 11.95, -2.62), (4.9, 12.2, -2.08), "leather_dk")
    g.box(b, "strap_b", (-4.9, 12.2, 2.05), (4.9, 13.9, 2.65), "leather")
    g.box(b, "strap_b_t", (-4.9, 13.9, 2.08), (4.9, 14.2, 2.62), "leather_dk")
    g.box(b, "strap_b_b", (-4.9, 11.95, 2.08), (4.9, 12.2, 2.62), "leather_dk")
    for k in range(5):
        x = 0.4 + k * 0.85
        g.pair(b, f"st_ft_{k}", (x, 13.45, -2.85), (x + 0.4, 13.8, -2.6), "stitch")
        g.pair(b, f"st_fb_{k}", (x, 12.3, -2.85), (x + 0.4, 12.65, -2.6), "stitch")
        g.pair(b, f"st_bt_{k}", (x, 13.45, 2.6), (x + 0.4, 13.8, 2.85), "stitch")
        g.pair(b, f"st_bb_{k}", (x, 12.3, 2.6), (x + 0.4, 12.65, 2.85), "stitch")
    for k, x in enumerate((4.1, 4.6)):
        g.pair(b, f"rivet_f{k}", (x - 0.22, 12.7, -2.95), (x + 0.22, 13.3, -2.62), "iron")
        g.pair(b, f"rivet_b{k}", (x - 0.22, 12.7, 2.62), (x + 0.22, 13.3, 2.95), "iron")
    g.box(b, "back_slider_l", (-0.7, 12.35, 2.62), (-0.3, 13.75, 2.95), "iron")
    g.box(b, "back_slider_r", (0.3, 12.35, 2.62), (0.7, 13.75, 2.95), "iron")
    g.box(b, "back_slider_t", (-0.7, 13.4, 2.62), (0.7, 13.75, 2.95), "iron")
    g.box(b, "back_slider_b", (-0.7, 12.35, 2.62), (0.7, 12.7, 2.95), "iron")


def part_buckle(g: Gear) -> None:
    """带扣（穿戴者左前，x≈-3）：铁框 + 扣舌 + 锈斑 + 一行穿孔 + 一条带孔的带尾垂下来（不对称，单独一骨）。"""
    b = "belt_buckle"
    for name, lo, hi in (("top", (-3.9, 13.35, -3.15), (-2.1, 13.8, -2.62)), ("bot", (-3.9, 12.25, -3.15), (-2.1, 12.7, -2.62)),
                         ("l", (-3.9, 12.7, -3.15), (-3.45, 13.35, -2.62)), ("r", (-2.55, 12.7, -3.15), (-2.1, 13.35, -2.62))):
        g.box(b, f"buckle_{name}", lo, hi, "iron")
    g.box(b, "buckle_tongue", (-3.2, 12.55, -3.4), (-2.9, 13.5, -3.1), "iron_dk")
    g.box(b, "buckle_rust", (-3.85, 13.4, -3.3), (-3.1, 13.78, -3.1), "rust")
    for k, x in enumerate((-2.0, -1.5, -1.0)):
        g.box(b, f"punch_{k}", (x - 0.14, 12.8, -2.82), (x + 0.14, 13.3, -2.6), "void")
    g.box(b, "tail", (-3.1, 9.4, -2.95), (-1.9, 12.3, -2.64), "leather_lt")
    g.box(b, "tail_tip", (-2.95, 8.7, -2.9), (-2.05, 9.4, -2.68), "leather_dk")
    g.box(b, "tail_hole", (-2.7, 10.7, -3.05), (-2.3, 11.2, -2.84), "void")
    g.box(b, "tail_st", (-3.0, 11.5, -3.12), (-2.0, 11.8, -2.9), "stitch")


def part_tab(g: Gear) -> None:
    """挂扣片：匣后一块皮扣片，从腰带垂到匣背，带铁滑扣和两枚铆钉，下沿缝线。"""
    b = "belt_tab"
    g.box(b, "tab", (CXB - 1.1, 11.0, -2.85), (CXB + 1.1, 13.7, -2.55), "leather_lt")
    g.box(b, "slider_t", (CXB - 1.35, 12.8, -3.1), (CXB + 1.35, 13.2, -2.82), "iron")
    g.box(b, "slider_b", (CXB - 1.35, 11.6, -3.1), (CXB + 1.35, 12.0, -2.82), "iron")
    g.box(b, "slider_l", (CXB - 1.35, 12.0, -3.1), (CXB - 0.95, 12.8, -2.82), "iron")
    g.box(b, "slider_r", (CXB + 0.95, 12.0, -3.1), (CXB + 1.35, 12.8, -2.82), "iron")
    for k, dx in enumerate((-0.6, 0.6)):
        g.box(b, f"rivet_{k}", (CXB + dx - 0.2, 11.2, -3.0), (CXB + dx + 0.2, 11.6, -2.7), "rust")
    for k in range(4):
        g.box(b, f"st_{k}", (CXB - 0.9 + k * 0.5, 13.3, -2.98), (CXB - 0.55 + k * 0.5, 13.55, -2.58), "stitch")


# ───────────────────────────────────────────────────────────── 分部件：匣身
def part_box(g: Gear) -> None:
    """匣身：底板两块 + 前 / 后 / 左 / 右壁各三层木板（板缝错位、厚薄各异），前面一块劈裂的板。"""
    b = "box_body"
    g.box(b, "floor_a", (BX0, BY0, BZ0), (CXB, BY0 + WALL, BZ1), "wood_dk")
    g.box(b, "floor_b", (CXB + 0.04, BY0, BZ0), (BX1, BY0 + WALL + 0.03, BZ1), "wood")
    mats = (("wood", "wood_lt", "wood"), ("wood_lt", "wood", "wood_lt"))
    for i in range(3):
        y0, y1 = PLANK_Y[i], PLANK_Y[i + 1]
        t = 0.03 * i
        # 前壁：第二层板从中间劈成两段，缝错开
        if i == 1:
            g.box(b, "front_1a", (BX0, y0, BZ0 - t), (CXB + 0.5, y1, BZ0 + WALL), mats[0][1])
            g.box(b, "front_1b", (CXB + 0.55, y0 + 0.02, BZ0 - t - 0.03), (BX1, y1, BZ0 + WALL), mats[1][1])
        else:
            g.box(b, f"front_{i}", (BX0, y0, BZ0 - t), (BX1, y1, BZ0 + WALL), mats[i // 2][i])
        g.box(b, f"back_{i}", (BX0, y0, BZ1 - WALL), (BX1, y1, BZ1 + 0.02 * i), mats[1][i])
        g.box(b, f"side_l_{i}", (BX0 - 0.04 * (i % 2), y0, BZ0 + WALL), (BX0 + WALL, y1, BZ1 - WALL), mats[0][i])
        g.box(b, f"side_r_{i}", (BX1 - WALL, y0, BZ0 + WALL), (BX1 + 0.04 * ((i + 1) % 2), y1, BZ1 - WALL), mats[1][i])
    # 板缝暗线：前面两道横缝 + 右侧两道（贴外面，0.24 厚）
    for k, y in enumerate((PLANK_Y[1] - 0.12, PLANK_Y[2] - 0.12)):
        g.box(b, f"seam_f{k}", (BX0 + 0.1, y, BZ0 - 0.24), (BX1 - 0.1, y + 0.24, BZ0 + 0.08), "wood_dk")
        g.box(b, f"seam_r{k}", (BX1 - 0.08, y, BZ0 + 0.7), (BX1 + 0.24, y + 0.24, BZ1 - 0.7), "wood_dk")
    # 木板上的疤节和裂口
    g.box(b, "knot_a", (BX0 + 1.0, 11.1, BZ0 - 0.26), (BX0 + 1.6, 11.6, BZ0 + 0.06), "wood_dk")
    g.box(b, "split_b", (CXB + 1.3, 12.0, BZ0 - 0.26), (CXB + 1.5, 12.9, BZ0 + 0.06), "wood_dk")
    g.box(b, "chip", (BX1 - 0.7, BY0 + 0.1, BZ0 - 0.24), (BX1 - 0.05, BY0 + 0.6, BZ0 + 0.1), "wood_dk")


def part_liner(g: Gear) -> None:
    """丹砂内衬：底 + 四壁内侧各一层 0.25 的涂层，壁上几处磨损露出底下的深色。"""
    b = "box_liner"
    ix0, ix1 = BX0 + WALL, BX1 - WALL
    iz0, iz1 = BZ0 + WALL, BZ1 - WALL
    top = BY1 - 0.2
    g.box(b, "liner_floor", (ix0, BY0 + WALL, iz0), (ix1, BY0 + WALL + 0.3, iz1), "cinnabar")
    g.box(b, "liner_f", (ix0, BY0 + WALL, iz0), (ix1, top, iz0 + 0.3), "cinnabar")
    g.box(b, "liner_b", (ix0, BY0 + WALL, iz1 - 0.3), (ix1, top, iz1), "cinnabar")
    g.box(b, "liner_l", (ix0, BY0 + WALL, iz0 + 0.3), (ix0 + 0.3, top, iz1 - 0.3), "cinnabar")
    g.box(b, "liner_r", (ix1 - 0.3, BY0 + WALL, iz0 + 0.3), (ix1, top, iz1 - 0.3), "cinnabar")
    for k, (x, y, z, sx, sz) in enumerate(((ix0 + 0.5, BY0 + WALL + 0.3, iz0 + 0.6, 1.0, 0.8), (ix1 - 1.6, BY0 + WALL + 0.3, iz1 - 1.4, 1.1, 0.9),
                                              (ix0 + 1.9, BY0 + WALL + 0.3, iz0 + 1.6, 0.7, 0.7))):
        g.box(b, f"wear_floor_{k}", (x, y, z), (x + sx, y + 0.05 + 0.2, z + sz), "cinnabar_dk")
    g.box(b, "wear_f", (ix0 + 1.2, 12.1, iz0 + 0.3), (ix0 + 2.2, top - 0.2, iz0 + 0.5 + 0.02), "cinnabar_dk")
    g.box(b, "wear_r", (ix1 - 0.5, 12.0, iz0 + 1.0), (ix1 - 0.28, top - 0.3, iz0 + 1.9), "cinnabar_dk")


def part_irons(g: Gear) -> None:
    """匣身四角 L 形锈铁箍：每角一条包住前（或后）面和侧面的 L 形铁条，上下各一枚铁钉；前两角底部加一块托角。"""
    b = "box_irons"
    for tag, sx, z_face in (("fl", -1, BZ0), ("fr", 1, BZ0), ("bl", -1, BZ1), ("br", 1, BZ1)):
        x_edge = BX0 if sx < 0 else BX1
        sz = 1 if z_face == BZ0 else -1                       # 铁条沿 z 向匣内的方向
        # 贴前 / 后面的一条（宽 0.9）
        xa, xb = (x_edge - 0.06, x_edge + 0.9) if sx < 0 else (x_edge - 0.9, x_edge + 0.06)
        za, zb = (z_face - 0.3, z_face + 0.04) if sz > 0 else (z_face - 0.04, z_face + 0.3)
        g.box(b, f"strap_{tag}", (xa, BY0 + 0.2, za), (xb, BY1 - 0.1, zb), "iron")
        # 贴侧面的一条（深 0.9）
        xs0, xs1 = (x_edge - 0.3, x_edge + 0.04) if sx < 0 else (x_edge - 0.04, x_edge + 0.3)
        zs0, zs1 = (z_face - 0.06, z_face + 0.9) if sz > 0 else (z_face - 0.9, z_face + 0.06)
        g.box(b, f"side_{tag}", (xs0, BY0 + 0.2, zs0), (xs1, BY1 - 0.1, zs1), "iron_dk")
        for k, y in enumerate((BY0 + 0.6, BY1 - 0.6)):
            px = (xa + 0.3) if sx < 0 else (xb - 0.3)
            pz = (za - 0.14) if sz > 0 else (zb + 0.14)
            g.box(b, f"nail_{tag}{k}", (px - 0.2, y - 0.2, pz - 0.14), (px + 0.2, y + 0.2, pz + 0.14), "rust")
    for tag, sx in (("l", -1), ("r", 1)):
        x_edge = BX0 if sx < 0 else BX1
        xa, xb = (x_edge - 0.1, x_edge + 1.0) if sx < 0 else (x_edge - 1.0, x_edge + 0.1)
        g.box(b, f"foot_{tag}", (xa, BY0 - 0.3, BZ0 - 0.2), (xb, BY0 + 0.25, BZ0 + 0.9), "iron_dk")


def part_hasp(g: Gear) -> None:
    """搭扣：前面正中一块竖铁板（带方孔），铁钉两枚，下面挂一个小锁耳。"""
    b = "box_hasp"
    g.box(b, "plate", (CXB - 0.6, 11.0, BZ0 - 0.32), (CXB + 0.6, 13.4, BZ0 + 0.04), "iron")
    g.box(b, "slot", (CXB - 0.22, 11.7, BZ0 - 0.46), (CXB + 0.22, 12.5, BZ0 - 0.22), "void")
    g.box(b, "staple_t", (CXB - 0.7, 12.7, BZ0 - 0.5), (CXB + 0.7, 13.1, BZ0 - 0.28), "iron_dk")
    for k, y in enumerate((11.25, 13.1)):
        g.box(b, f"nail_{k}", (CXB - 0.2, y - 0.2, BZ0 - 0.5), (CXB + 0.2, y + 0.2, BZ0 - 0.28), "rust")
    g.box(b, "ear", (CXB - 0.3, 10.5, BZ0 - 0.45), (CXB + 0.3, 11.1, BZ0 - 0.1), "iron_dk")


# ───────────────────────────────────────────────────────────── 分部件：匣盖
def _lid(g: Gear, bone: str, name: str, lo, hi, mat: str) -> None:
    """盖上的件：按合盖位置给坐标，整体绕后沿铰链转 LID_DEG（共用同一枢轴 → 刚体）。"""
    g.box(bone, name, lo, hi, mat, rot=(LID_DEG, 0.0, 0.0), org=HINGE)


def part_lid(g: Gear) -> None:
    """匣盖：绕后沿铰链掀开 32°；顶板三块（沿 x 拼）+ 前沿板 + 两端板 + 里面一层丹砂 + 顶面板缝。"""
    b = "box_lid"
    x0, x1 = BX0 - 0.1, BX1 + 0.1
    z0, z1 = BZ0 - 0.1, BZ1
    y0, y1 = BY1, BY1 + LID_H
    zs = (z0, z0 + (z1 - z0) / 3, z0 + 2 * (z1 - z0) / 3, z1)
    for i in range(3):
        _lid(g, b, f"top_{i}", (x0, y1 - 0.55, zs[i] + 0.03 * i), (x1, y1, zs[i + 1]), "wood" if i % 2 == 0 else "wood_lt")
    _lid(g, b, "front", (x0, y0, z0), (x1, y1 - 0.55, z0 + 0.6), "wood_lt")
    _lid(g, b, "end_l", (x0, y0, z0 + 0.6), (x0 + 0.6, y1 - 0.55, z1), "wood")
    _lid(g, b, "end_r", (x1 - 0.6, y0, z0 + 0.6), (x1, y1 - 0.55, z1), "wood")
    _lid(g, b, "liner_top", (x0 + 0.6, y1 - 0.55 - 0.28, z0 + 0.6), (x1 - 0.6, y1 - 0.55, z1), "cinnabar")
    _lid(g, b, "liner_front", (x0 + 0.6, y0, z0 + 0.6), (x1 - 0.6, y1 - 0.55, z0 + 0.88), "cinnabar")
    _lid(g, b, "liner_wear", (x0 + 1.4, y1 - 0.83 - 0.02, z0 + 1.2), (x0 + 2.6, y1 - 0.55 - 0.02, z0 + 1.9), "cinnabar_dk")
    for k in range(2):
        z = zs[k + 1]
        _lid(g, b, f"seam_{k}", (x0 + 0.05, y1 - 0.1, z - 0.08), (x1 - 0.05, y1 + 0.14, z + 0.14), "wood_dk")
    _lid(g, b, "knot", (x0 + 1.6, y1 - 0.12, z0 + 0.8), (x0 + 2.2, y1 + 0.12, z0 + 1.3), "wood_dk")


def part_lid_irons(g: Gear) -> None:
    """盖面两道横跨的铁条（各两枚铁钉）+ 前沿铁锁扣 + 后沿两枚铰链（与盖同骨：一起转）。"""
    b = "box_lid"
    x0, x1 = BX0 - 0.1, BX1 + 0.1
    z0, z1 = BZ0 - 0.1, BZ1
    y1 = BY1 + LID_H
    for k, zc in enumerate((z0 + 0.9, z1 - 1.0)):
        g.box(b, f"band_{k}", (x0 + 0.3, y1 - 0.1, zc - 0.5), (x1 - 0.3, y1 + 0.2, zc + 0.5), "iron",
              rot=(LID_DEG, 0.0, 0.0), org=HINGE)
        for j, x in enumerate((x0 + 0.9, x1 - 0.9)):
            g.box(b, f"band_nail_{k}{j}", (x - 0.2, y1 + 0.12, zc - 0.2), (x + 0.2, y1 + 0.38, zc + 0.2), "rust",
                  rot=(LID_DEG, 0.0, 0.0), org=HINGE)
    g.box(b, "catch", (CXB - 0.6, BY1 + 0.15, z0 - 0.32), (CXB + 0.6, y1, z0 + 0.06), "iron",
          rot=(LID_DEG, 0.0, 0.0), org=HINGE)
    for k, x in enumerate((BX0 + 0.8, BX1 - 1.4)):
        g.box(b, f"hinge_{k}", (x, BY1 - 0.5, BZ1 - 0.1), (x + 0.6, BY1 + 0.3, BZ1 + 0.3), "iron_dk")


# ───────────────────────────────────────────────────────────── 分部件：骨币串
def part_coins(g: Gear) -> None:
    """骨币串：从腰带右端垂下的草绳（挂在匣右侧 z<-2.05），四枚带方孔的骨币等距串着，币间草绳小结。"""
    b = "coin_string"
    rx, rz = 4.1, -2.95
    g.box(b, "knot_top", (rx - 0.3, 12.5, rz - 0.15), (rx + 0.7, 13.6, rz + 0.55), "cord_dk")
    g.box(b, "cord_hi", (rx, 11.5, rz), (rx + 0.4, 12.7, rz + 0.4), "cord")
    for i, ytop in enumerate((11.3, 9.65, 8.0, 6.35)):
        c = rx + 0.2
        y0, y1 = ytop - 1.5, ytop
        z0, z1 = rz - 0.14, rz + 0.5
        ym = (y0 + y1) / 2
        g.box(b, f"coin{i}_h", (c - 0.9, ym - 0.5, z0), (c + 0.9, ym + 0.5, z1), "bone" if i % 2 == 0 else "bone_dk")
        g.box(b, f"coin{i}_v", (c - 0.55, y0, z0 + 0.03), (c + 0.55, y1, z1 - 0.02), "bone_dk" if i % 2 == 0 else "bone")
        g.box(b, f"coin{i}_hole", (c - 0.26, ym - 0.26, z0 - 0.12), (c + 0.26, ym + 0.26, z0 + 0.2), "void")
        g.box(b, f"coin{i}_rim", (c - 0.95, ym - 0.3, z1 - 0.3), (c + 0.95, ym + 0.3, z1 + 0.04), "bone_dk")
        g.box(b, f"link_{i}", (rx, ytop, rz + 0.02 * i), (rx + 0.4, ytop + 0.4, rz + 0.4 + 0.02 * i), "cord_dk")
    g.box(b, "end_knot", (rx - 0.25, 4.8, rz - 0.15), (rx + 0.65, 5.5, rz + 0.55), "cord_dk")


def part_tassel(g: Gear) -> None:
    """币穗：绳尾散开的十条穗，红 / 米 / 褐三色相间，长短参差。"""
    b = "coin_tassel"
    rng = random.Random(0x5E11)
    rx, rz = 4.1, -2.95
    mats = ("red_cord", "cord", "red_cord", "rag_beige", "cord_dk")
    for i in range(10):
        dx = -0.35 + (i % 5) * 0.27
        dz = -0.12 + (i // 5) * 0.28
        ln = 1.2 + rng.random() * 1.2
        w = 0.24
        g.box(b, f"strand_{i}", (rx + dx - w / 2 + 0.3, 5.0 - ln, rz + dz), (rx + dx + w / 2 + 0.3, 5.0, rz + dz + 0.28), mats[i % 5])


# ───────────────────────────────────────────────────────────── 分部件：破布条
def _rag(g: Gear, bone: str, name: str, x0: float, x1: float, z0: float, z1: float, length: float, mat: str, rng: random.Random) -> None:
    """一条破布：三段拼成参差的下摆（左 / 中 / 右三栏长短不一），顶端贴在腰带下沿。"""
    top = 12.4
    w = (x1 - x0) / 3
    for k in range(3):
        ln = length * (0.55 + 0.45 * rng.random())
        g.box(bone, f"{name}_{k}", (x0 + k * w, top - ln, z0), (x0 + (k + 1) * w + 0.04, top, z1 + 0.01 * k), mat)


def part_rags_front(g: Gear) -> None:
    """前破布条：腰带下垂的四条（米 / 红 / 灰 / 褐），在匣左右两侧，避开匣身；一条带破洞、一条缝着补丁。"""
    b = "rags_front"
    rng = random.Random(0x7A21)
    spec = ((-4.8, -3.8, "rag_beige", 3.4), (-3.7, -2.75, "rag_red", 3.9), (3.85, 4.5, "rag_grey", 3.1), (4.45, 4.95, "rag_brown", 2.4))
    for i, (x0, x1, mat, ln) in enumerate(spec):
        _rag(g, b, f"rag{i}", x0, x1, -2.6, -2.3, ln, mat, rng)
    g.box(b, "rag_hole", (-4.5, 9.9, -2.72), (-4.0, 10.4, -2.48), "void")
    g.box(b, "patch", (-3.6, 9.5, -2.74), (-2.9, 10.3, -2.5), "rag_brown")
    for k, (x, y) in enumerate(((-3.55, 10.15), (-3.0, 10.15), (-3.55, 9.55), (-3.0, 9.55))):
        g.box(b, f"patch_st_{k}", (x, y, -2.8), (x + 0.28, y + 0.28, -2.52), "stitch")


def part_rags_back(g: Gear) -> None:
    """后破布条：腰带下垂的五条（左右对称错开的色），一条带破洞。"""
    b = "rags_back"
    rng = random.Random(0x3C09)
    spec = ((-4.6, -3.4, "rag_beige", 3.2), (-3.3, -1.9, "rag_red", 3.8), (-1.8, -0.1, "rag_grey", 2.8),
            (0.0, 1.7, "rag_beige", 3.5), (1.8, 3.3, "rag_brown", 3.0), (3.4, 4.7, "rag_red", 3.4))
    for i, (x0, x1, mat, ln) in enumerate(spec):
        _rag(g, b, f"rag{i}", x0, x1, 2.28, 2.58, ln, mat, rng)
    g.box(b, "rag_hole", (-0.9, 10.2, 2.5), (-0.4, 10.7, 2.74), "void")


def deconflict(g: Gear) -> None:
    """把同向共面的面错开（Z-fighting 门禁）：只动无旋转件，把后加件的冲突面向外长 0.002–0.01px（肉眼不可见）。

    细部件（板缝 / 针脚 / 铁钉 / 破布）密密麻麻贴在大面上，手写每一处错位既烦又易漏；这里统一处理，
    几何意图仍由 part_*() 里的坐标决定。
    """
    els = [e for e in g.rig.elements if not any(e["rotation"])]
    for i, e in enumerate(els):
        for _ in range(12):
            moved = False
            for f in els[:i]:
                for ax in range(3):
                    o1, o2 = [a for a in range(3) if a != ax]
                    if min(e["to"][o1], f["to"][o1]) - max(e["from"][o1], f["from"][o1]) <= 1e-4:
                        continue
                    if min(e["to"][o2], f["to"][o2]) - max(e["from"][o2], f["from"][o2]) <= 1e-4:
                        continue
                    for side, sign in (("from", -1), ("to", 1)):
                        if abs(e[side][ax] - f[side][ax]) < 1e-4:
                            e[side][ax] = round(e[side][ax] + sign * 0.002 * (1 + i % 5), 4)
                            moved = True
            if not moved:
                break


class CoinBoxSpec(GearSpec):
    def build(self, parts=None) -> Gear:
        g = super().build(parts)
        deconflict(g)
        return g


PART_FUNCS = {
    "belt": part_belt,
    "buckle": part_buckle,
    "tab": part_tab,
    "box": part_box,
    "liner": part_liner,
    "irons": part_irons,
    "hasp": part_hasp,
    "lid": part_lid,
    "lid_irons": part_lid_irons,
    "coins": part_coins,
    "tassel": part_tassel,
    "rags_front": part_rags_front,
    "rags_back": part_rags_back,
}

SPEC = CoinBoxSpec(
    gear_id="coin_box",
    model_name="CoinBox",
    title="骨币匣 / coin_box",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：腰前偏右（匣身 z∈[-6.4,-2.7]，在躯干 / 腿之前）+ 前后宽皮带 + 右侧骨币串 + 腰下破布条",
    worn_focus=((0.5, 10.5, -1.0), 22.0),
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
