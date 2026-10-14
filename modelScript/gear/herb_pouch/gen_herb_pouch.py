#!/usr/bin/env python3
"""灵草囊（herb_pouch）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #1（.task-wasteland-gear.md）。穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。
物品描述（server/assets/items/workbench_materials.toml）：「粗布缝成的草药袋，采集时直接塞入减少磨损。」

外形（参考图 herb_pouch_ref.png / herb_pouch_exploded.png）：一只粗麻布缝的鼓肚束口小袋，挂在穿戴者右胯外侧
（x∈[4.1,7.9]、y∈[3.2,11.4]，整个在手臂盒 y≥12 之下、右腿盒 x≤4 之外）。袋口拧着草绳束紧，绳头打结垂下，
绳头上别一枚骨栓；袋口褶出一圈布褶，里面露出三种干草药：一根带叶的青草（往前斜探出手臂盒）、两根苍白的籽穗草、
一束枯褐蕨叶。袋面缝着三块补丁（皮补、粗布补、一处磨破的洞口露出暗里），布面有竖褶和织纹，袋底一圈缝边。
腰上一段皮带片贴在躯干前面（z<-2.05），草绳从带片垂下系住袋颈。

骨骼 14 根：带片 / 挂绳 / 袋身 / 袋褶 / 袋口 / 束绳 / 骨栓 / 补丁 / 侧袋 / 三种草药，挂件各单独一骨。
分部件：part_*() 逐件写，--part 单件预览；骨骼树在 BONES 里一处定义。
用法：
    python3 modelScript/gear/herb_pouch/gen_herb_pouch.py --self-test   # 门禁差分自证
    python3 modelScript/gear/herb_pouch/gen_herb_pouch.py --part mouth  # 单件预览
    python3 modelScript/gear/herb_pouch/gen_herb_pouch.py --worn        # 只出穿戴三面
    python3 modelScript/gear/herb_pouch/gen_herb_pouch.py               # 全量：模型 + check.png
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质（RGB，每种占贴图一个 8×8 色块）
MATS = {
    "cloth":      (156, 140, 110),   # 粗麻布主色
    "cloth_lt":   (180, 164, 130),   # 织纹亮面
    "cloth_dk":   (112, 98, 76),     # 褶 / 阴面
    "cloth_in":   (84, 72, 56),      # 袋口里面
    "patch_hide": (122, 72, 52),     # 皮补丁
    "patch_cloth": (136, 120, 96),   # 粗布补丁
    "cord":       (140, 128, 76),    # 草绳（枯橄榄）
    "cord_dk":    (100, 90, 54),
    "stitch":     (62, 48, 36),      # 粗针脚
    "strap":      (110, 70, 50),     # 带片皮
    "strap_dk":   (78, 50, 38),
    "bone":       (214, 204, 176),
    "bone_dk":    (164, 150, 120),
    "herb_green": (112, 132, 78),
    "herb_green_dk": (78, 98, 56),
    "herb_pale":  (200, 184, 138),   # 苍白籽穗
    "herb_seed":  (226, 214, 170),
    "herb_brown": (140, 96, 58),     # 枯褐蕨叶
    "herb_brown_dk": (102, 68, 42),
    "void":       (28, 24, 22),
}
SOFT = ("patch_hide", "patch_cloth", "cord", "cord_dk", "strap", "strap_dk",
        "herb_green", "herb_green_dk", "herb_pale", "herb_seed", "herb_brown", "herb_brown_dk")
# 软件（补丁 / 绳 / 皮 / 草）压在任何材质上都允许重叠（草药插进袋口、绳缠袋颈）；袋布本身算「硬」：
# 袋身 / 袋口 / 褶 / 口袋这些布件之间互穿仍要报（否则穿模门在本件上造不出缺陷，自证会失效）
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("cloth", "cloth_dk", "cloth_in", "patch_cloth"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("belt_strap", "root", (2.6, 13.0, -2.35), "腰带片：贴在躯干前面的皮带片 + 针脚 + 铆钉（挂点）"),
    ("hang_cord", "belt_strap", (4.6, 12.4, -2.4), "挂绳：从带片垂到袋颈的草绳 + 绳结（会晃）"),
    ("pouch_body", "hang_cord", (6.0, 10.2, -0.6), "袋身：五层鼓肚 + 袋底缝边 + 织纹横线"),
    ("pouch_folds", "pouch_body", (6.0, 7.0, -0.6), "袋面竖褶：前 / 后 / 外侧暗褶"),
    ("pouch_mouth", "pouch_body", (6.0, 10.6, -0.6), "袋口：一圈布褶 + 翘起的褶角 + 里面的暗底"),
    ("pouch_tie", "pouch_body", (6.0, 9.6, -0.6), "束口草绳：绕颈三圈 + 打结 + 两缕绳头"),
    ("pouch_toggle", "pouch_tie", (6.0, 8.0, -2.5), "骨栓：别在绳头上的小骨（会晃）"),
    ("pouch_patches", "pouch_body", (6.0, 6.5, -0.6), "补丁：前皮补 / 外侧粗布补 / 破洞 + 针脚"),
    ("pouch_pocket", "pouch_body", (7.9, 6.0, -0.6), "外侧小口袋：盖片 + 骨扣 + 针脚"),
    ("herb_green", "pouch_body", (5.8, 10.4, -0.7), "青叶草：往前斜探的茎 + 成对叶片"),
    ("herb_pale", "pouch_body", (6.5, 10.4, -0.5), "两根苍白籽穗草：细茎 + 籽粒"),
    ("herb_brown", "pouch_body", (6.0, 10.4, -0.2), "枯褐蕨叶：往后探的茎 + 小羽叶"),
]
ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "belt_strap"))

# ───────────────────────────────────────────────────────────── 常量：袋身
CX, CZ = 6.0, -0.6                 # 袋身中心（右胯外侧）
# (名, y0, y1, 半宽 x, 半深 z)：自下而上鼓肚收口
SLICES = (
    ("base", 3.2, 4.05, 1.1, 1.05),
    ("low", 4.05, 5.2, 1.55, 1.5),
    ("belly", 5.2, 8.0, 1.9, 1.8),
    ("shoulder", 8.0, 9.2, 1.6, 1.5),
    ("neck", 9.2, 10.05, 1.2, 1.15),
)
MOUTH_Y0 = 10.05


def _stalk(g: Gear, bone: str, name: str, pts, r: float, mat: str) -> None:
    """沿折线铺茎：逐段短杆（避免斜长杆的包围盒胖到蹭进手臂盒）。"""
    for i, (a, b) in enumerate(zip(pts, pts[1:])):
        g.rod(bone, f"{name}_{i}", a, b, r, mat)


def _stitch_row(g: Gear, bone: str, name: str, x0: float, x1: float, y: float, z: float, n: int, mat: str = "stitch") -> None:
    """一排横向针脚（贴在 z 面上，向 -z 凸 0.2；高 0.36，一半压在被缝的布上）。"""
    step = (x1 - x0) / n
    for k in range(n):
        x = x0 + k * step + step * 0.2
        g.box(bone, f"{name}_{k}", (x, y, z - 0.2), (x + step * 0.55, y + 0.36, z + 0.05 + 0.003 * k), mat)


# ───────────────────────────────────────────────────────────── 分部件：带片 / 挂绳
def part_belt(g: Gear) -> None:
    """腰带片：躯干前面一段皮带片，上下针脚行、两枚骨铆钉、带边磨损缺口。"""
    b = "belt_strap"
    # 腰带：前后各一条（左右对称，z 全在躯干 / 手臂盒之外）；侧面 |x|>4 正好是手臂盒，不绕过去
    g.box(b, "strap", (-4.9, 12.3, -2.65), (4.9, 13.95, -2.05), "strap")
    g.box(b, "strap_edge_t", (-4.9, 13.95, -2.62), (4.9, 14.2, -2.08), "strap_dk")
    g.box(b, "strap_edge_b", (-4.9, 12.05, -2.62), (4.9, 12.3, -2.08), "strap_dk")
    g.box(b, "strap_back", (-4.5, 12.3, 2.05), (4.5, 13.95, 2.65), "strap")
    g.box(b, "strap_back_edge_t", (-4.5, 13.95, 2.08), (4.5, 14.2, 2.62), "strap_dk")
    g.box(b, "strap_back_edge_b", (-4.5, 12.05, 2.08), (4.5, 12.3, 2.62), "strap_dk")
    for k in range(5):
        x = 0.7 + k * 0.9
        g.pair(b, f"st_t_{k}", (x, 13.45, -2.85), (x + 0.4, 13.8, -2.6), "stitch")
        g.pair(b, f"st_b_{k}", (x, 12.45, -2.85), (x + 0.4, 12.8, -2.6), "stitch")
    for k, x in enumerate((1.0, 4.2)):
        g.pair(b, f"rivet_{k}", (x, 12.9, -2.95), (x + 0.5, 13.38, -2.65), "bone")
    # 背面带的针脚（左右对称）+ 正中一枚骨扣
    for k in range(2):
        g.pair(b, f"back_st_{k}", (1.2 + k * 1.7, 13.1, 2.62), (1.7 + k * 1.7, 13.5, 2.85), "stitch")
    g.box(b, "back_buckle", (-0.6, 12.5, 2.62), (0.6, 13.75, 3.0), "bone")


def part_cord(g: Gear) -> None:
    """挂绳：从带片右端垂下的草绳（两股），中途一个绳结，下端系住袋颈。"""
    b = "hang_cord"
    g.box(b, "cord_a", (4.55, 11.0, -2.72), (4.95, 13.4, -2.1), "cord")
    g.box(b, "cord_b", (5.0, 11.2, -2.62), (5.35, 13.1, -2.12), "cord_dk")
    g.box(b, "knot", (4.4, 11.6, -2.8), (5.5, 12.4, -2.07), "cord_dk")
    g.box(b, "wear_mark", (4.33, 12.4, -2.87), (4.78, 13.0, -2.62), "strap_dk")     # 带头右端磨掉的暗痕
    g.rod(b, "cord_down_a", (4.8, 11.1, -2.5), (5.5, 9.8, -2.0), 0.28, "cord")
    g.rod(b, "cord_down_b", (5.15, 11.1, -2.5), (5.9, 9.8, -2.1), 0.22, "cord_dk")
    g.box(b, "cord_loop", (5.2, 9.4, -2.35), (6.7, 10.0, -1.82), "cord")


# ───────────────────────────────────────────────────────────── 分部件：袋身
def part_body(g: Gear) -> None:
    """袋身：五层鼓肚（底小、腹大、肩收、颈束）+ 袋底缝边 + 逐行织纹横线。"""
    b = "pouch_body"
    for tag, y0, y1, hw, hd in SLICES:
        g.box(b, f"body_{tag}", (CX - hw, y0, CZ - hd), (CX + hw, y1, CZ + hd), "cloth")
    # 袋底缝边：深色一圈，比底层略出
    g.box(b, "hem_bottom", (CX - 1.2, 3.05, CZ - 1.15), (CX + 1.2, 3.35, CZ + 1.15), "cloth_dk")
    g.box(b, "hem_stitch_f", (CX - 0.8, 3.2, CZ - 1.35), (CX + 0.8, 3.5, CZ - 1.05), "stitch")
    g.box(b, "hem_stitch_b", (CX - 0.8, 3.2, CZ + 1.05), (CX + 0.8, 3.5, CZ + 1.35), "stitch")
    # 织纹：腹部前面和后面各几道浅色横线，右 / 外侧也有，读出粗麻布的横纬
    for k, y in enumerate((5.5, 6.3, 7.1, 7.7)):
        w = 1.9 - 0.02 * k
        g.box(b, f"weave_f_{k}", (CX - w + 0.15 * (k % 2), y, CZ - 1.8 - 0.22), (CX + w - 0.15 * ((k + 1) % 2), y + 0.22, CZ - 1.8), "cloth_lt")
        g.box(b, f"weave_b_{k}", (CX - w + 0.2, y + 0.05, CZ + 1.8), (CX + w - 0.1, y + 0.27, CZ + 1.8 + 0.22), "cloth_lt")
    # 外侧（+x）织纹；内侧（-x）紧贴右腿盒，不加凸出件
    for k, y in enumerate((5.7, 6.9)):
        g.box(b, f"weave_o_{k}", (CX + 1.9, y, CZ - 1.3), (CX + 1.9 + 0.22, y + 0.22, CZ + 1.0), "cloth_lt")


def part_folds(g: Gear) -> None:
    """袋面竖褶：前面四道、后面三道、外侧两道，暗色，长短错落，读作布被物件撑出的褶。"""
    b = "pouch_folds"
    for k, (dx, y1) in enumerate(((-1.35, 8.2), (-0.45, 8.6), (0.5, 8.1), (1.4, 8.5))):
        g.box(b, f"fold_f_{k}", (CX + dx - 0.17, 5.4 + 0.2 * (k % 2), CZ - 1.8 - 0.3), (CX + dx + 0.17, y1, CZ - 1.8 - 0.05), "cloth_dk")
    for k, (dx, y1) in enumerate(((-1.2, 8.0), (0.0, 8.3), (1.1, 7.8))):
        g.box(b, f"fold_b_{k}", (CX + dx - 0.17, 5.5, CZ + 1.8 + 0.02), (CX + dx + 0.17, y1, CZ + 1.8 + 0.25), "cloth_dk")
    for k, dz in enumerate((-1.1, 0.55)):
        g.box(b, f"fold_o_{k}", (CX + 1.9 + 0.02, 5.6, CZ + dz - 0.17), (CX + 1.9 + 0.25, 8.2 - 0.3 * k, CZ + dz + 0.17), "cloth_dk")
    g.box(b, "fold_shoulder_f", (CX - 0.9, 8.2, CZ - 1.5 - 0.25), (CX - 0.5, 9.1, CZ - 1.5 - 0.02), "cloth_dk")
    g.box(b, "fold_shoulder_b", (CX + 0.4, 8.2, CZ + 1.5 + 0.02), (CX + 0.8, 9.1, CZ + 1.5 + 0.22), "cloth_dk")


def part_mouth(g: Gear) -> None:
    """袋口：十二个高低错落的布褶围成一圈，四个翘起的褶角，里面一块暗底托着草药。"""
    b = "pouch_mouth"
    g.box(b, "floor", (CX - 1.0, MOUTH_Y0, CZ - 0.95), (CX + 1.0, MOUTH_Y0 + 0.3, CZ + 0.95), "cloth_in")
    # 袋口最高到 y≈11.9：再高就进了手臂盒（y≥12）
    heights = (1.0, 1.4, 0.8, 1.25, 0.95, 1.5, 0.75, 1.2, 1.0, 1.4, 0.85, 1.15)
    for i in range(12):
        a = i * math.tau / 12
        x = CX + math.cos(a) * 1.35
        z = CZ + math.sin(a) * 1.3
        h = heights[i]
        g.box(b, f"pleat_{i}", (x - 0.42, MOUTH_Y0 + 0.01 * (i + 1), z - 0.4), (x + 0.42, MOUTH_Y0 + 0.01 * (i + 1) + h, z + 0.4),
              "cloth" if i % 3 else "cloth_lt")
    # 褶里的暗线：每隔一个褶中间一条暗缝
    for i in range(0, 12, 2):
        a = (i + 0.5) * math.tau / 12
        x = CX + math.cos(a) * 1.62
        z = CZ + math.sin(a) * 1.55
        g.box(b, f"pleat_line_{i}", (x - 0.12, MOUTH_Y0 + 0.23, z - 0.12), (x + 0.12, MOUTH_Y0 + 0.95, z + 0.12), "cloth_dk")
    # 翘起的褶角：口沿上四个更高的小角（顶 ≤11.9）
    for i, (dx, dz, h) in enumerate(((-1.5, -0.7, 1.8), (0.9, -1.4, 1.85), (1.5, 0.6, 1.75), (-0.6, 1.4, 1.8))):
        g.box(b, f"tip_{i}", (CX + dx - 0.22, MOUTH_Y0 + 1.0, CZ + dz - 0.22), (CX + dx + 0.22, MOUTH_Y0 + h, CZ + dz + 0.22), "cloth")


def part_tie(g: Gear) -> None:
    """束口草绳：绕袋颈三圈（每圈四段）、一个大结、两缕垂下的绳头。"""
    b = "pouch_tie"
    for k, (y0, grow) in enumerate(((8.85, 0.0), (9.3, 0.05), (9.75, 0.0))):
        mat = "cord" if k % 2 == 0 else "cord_dk"
        hw, hd = 1.3 + grow, 1.25 + grow
        g.box(b, f"wrap_{k}_f", (CX - hw, y0, CZ - hd - 0.12), (CX + hw, y0 + 0.4, CZ - hd + 0.1), mat)
        g.box(b, f"wrap_{k}_b", (CX - hw, y0, CZ + hd - 0.1), (CX + hw, y0 + 0.4, CZ + hd + 0.12), mat)
        g.box(b, f"wrap_{k}_l", (CX - hw - 0.12, y0, CZ - hd + 0.12), (CX - hw + 0.1, y0 + 0.4, CZ + hd - 0.12), mat)
        g.box(b, f"wrap_{k}_r", (CX + hw - 0.1, y0, CZ - hd + 0.12), (CX + hw + 0.12, y0 + 0.4, CZ + hd - 0.12), mat)
    g.box(b, "knot", (CX - 0.55, 8.7, CZ - 1.9), (CX + 0.55, 9.9, CZ - 1.25), "cord_dk")
    g.box(b, "knot_ear_l", (CX - 1.1, 9.1, CZ - 1.8), (CX - 0.5, 9.68, CZ - 1.4), "cord")
    g.box(b, "knot_ear_r", (CX + 0.5, 9.12, CZ - 1.8), (CX + 1.1, 9.66, CZ - 1.4), "cord")
    g.rod(b, "tail_l", (CX - 0.2, 8.8, CZ - 1.7), (CX - 0.6, 7.2, CZ - 2.05), 0.22, "cord")
    g.rod(b, "tail_r", (CX + 0.2, 8.8, CZ - 1.7), (CX + 0.9, 7.5, CZ - 2.0), 0.22, "cord_dk")
    # 绳头散开的毛须
    for k, (dx, y) in enumerate(((-0.85, 6.9), (-0.58, 6.8), (-0.33, 6.95))):
        g.box(b, f"fray_l_{k}", (CX + dx - 0.11, y, CZ - 2.17), (CX + dx + 0.11, y + 0.5, CZ - 1.95), "cord")
    for k, (dx, y) in enumerate(((0.8, 7.15), (1.05, 7.05))):
        g.box(b, f"fray_r_{k}", (CX + dx - 0.11, y, CZ - 2.12), (CX + dx + 0.11, y + 0.5, CZ - 1.9), "cord_dk")


def part_toggle(g: Gear) -> None:
    """骨栓：别在右边绳头上的小骨，横着，两头带疙瘩，中间一道绳缠。"""
    b = "pouch_toggle"
    g.box(b, "bone_shaft", (CX + 0.4, 6.95, CZ - 2.65), (CX + 2.0, 7.4, CZ - 2.2), "bone")
    g.box(b, "bone_end_a", (CX + 0.1, 6.85, CZ - 2.75), (CX + 0.6, 7.5, CZ - 2.15), "bone_dk")
    g.box(b, "bone_end_b", (CX + 1.8, 6.85, CZ - 2.75), (CX + 2.3, 7.5, CZ - 2.15), "bone_dk")
    g.box(b, "bone_wrap", (CX + 0.95, 6.9, CZ - 2.72), (CX + 1.25, 7.45, CZ - 2.18), "cord")


def part_patches(g: Gear) -> None:
    """补丁：前面一块皮补（四边针脚）、外侧一块粗布补、前下一处磨破的洞（露出暗里，边缘翘起）。"""
    b = "pouch_patches"
    zf = CZ - 1.8
    x0, x1, y0, y1 = CX - 1.75, CX - 0.1, 5.7, 7.55
    g.box(b, "hide_patch", (x0, y0, zf - 0.28), (x1, y1, zf - 0.02), "patch_hide")
    _stitch_row(g, b, "hp_st_t", x0 - 0.05, x1 + 0.05, y1 - 0.24, zf - 0.28, 4)
    _stitch_row(g, b, "hp_st_b", x0 - 0.05, x1 + 0.05, y0 - 0.12, zf - 0.28, 4)
    for k in range(3):
        y = y0 + 0.23 + k * 0.6
        g.box(b, f"hp_st_l_{k}", (x0 - 0.12, y, zf - 0.46), (x0 + 0.3, y + 0.3, zf - 0.04 + 0.004 * k), "stitch")
        g.box(b, f"hp_st_r_{k}", (x1 - 0.33, y, zf - 0.46), (x1 + 0.12, y + 0.3, zf - 0.04 + 0.004 * k), "stitch")
    # 外侧粗布补（朝 +x）
    xo = CX + 1.9
    g.box(b, "cloth_patch", (xo + 0.03, 6.7, CZ - 1.0), (xo + 0.3, 8.4, CZ + 0.85), "patch_cloth")
    for k in range(3):
        z = CZ - 0.9 + k * 0.62
        g.box(b, f"cp_st_t_{k}", (xo - 0.02, 8.1, z), (xo + 0.46, 8.5, z + 0.3), "stitch")
        g.box(b, f"cp_st_b_{k}", (xo - 0.02, 6.55, z), (xo + 0.46, 6.9, z + 0.3), "stitch")
    # 磨破的洞：袋前右下，暗洞 + 翘起的边
    g.box(b, "hole", (CX + 0.3, 4.55, zf - 0.1), (CX + 1.25, 5.15, zf + 0.12), "void")
    for k, (dx, dy, w, h) in enumerate(((0.2, 5.1, 0.45, 0.4), (0.8, 5.15, 0.4, 0.5), (1.0, 4.9, 0.35, 0.4), (0.25, 4.4, 0.4, 0.3))):
        g.box(b, f"hole_fray_{k}", (CX + dx, dy, zf - 0.29 - 0.011 * k), (CX + dx + w, dy + h, zf - 0.07 + 0.003 * k), "cloth_lt")
    # 后面一块小粗布补
    zb = CZ + 1.8
    g.box(b, "back_patch", (CX - 1.0, 6.0, zb + 0.03), (CX + 0.5, 7.3, zb + 0.3), "patch_cloth")
    g.box(b, "bp_st_a", (CX - 1.1, 7.2, zb + 0.035), (CX + 0.6, 7.5, zb + 0.44), "stitch")
    g.box(b, "bp_st_b", (CX - 1.1, 5.85, zb + 0.037), (CX + 0.6, 6.15, zb + 0.44), "stitch")


def part_pocket(g: Gear) -> None:
    """外侧小口袋：缝在袋外（+x）的方口袋，一片盖，盖上一枚骨扣，边上针脚。"""
    b = "pouch_pocket"
    xo = CX + 1.9
    g.box(b, "pocket", (xo + 0.35, 4.6, CZ - 0.9), (xo + 0.8, 6.1, CZ + 0.9), "cloth_dk")
    g.box(b, "flap", (xo + 0.78, 5.3, CZ - 1.0), (xo + 1.1, 6.35, CZ + 1.0), "cloth")
    g.box(b, "flap_point", (xo + 0.78, 5.0, CZ - 0.5), (xo + 1.1, 5.3, CZ + 0.5), "cloth")
    g.box(b, "button", (xo + 1.08, 5.32, CZ - 0.2), (xo + 1.4, 5.8, CZ + 0.3), "bone")
    for k in range(4):
        z = CZ - 0.9 + k * 0.5
        g.box(b, f"pk_st_{k}", (xo + 0.8, 6.25, z), (xo + 1.2, 6.55, z + 0.28), "stitch")


# ───────────────────────────────────────────────────────────── 分部件：草药
def part_herb_green(g: Gear) -> None:
    """青叶草：茎往前（-z）斜探，高过 y=12 的部分都在躯干 / 手臂盒之前（z<-2.1）；成对叶片挂在茎上。"""
    b = "herb_green"
    # 折点的 z 要在 y≈11.75 处就退到 -2.5（杆半径 0.27 + 手臂盒前沿 -2.0），再往上才不蹭手臂盒
    pts = [(CX - 0.4, 9.9, CZ - 0.1), (CX - 0.3, 10.85, CZ - 0.7), (CX - 0.1, 11.75, CZ - 1.9),
           (CX + 0.2, 12.7, CZ - 2.35), (CX + 0.4, 13.6, CZ - 3.0), (CX + 0.5, 14.5, CZ - 3.5)]
    _stalk(g, b, "stem", pts, 0.27, "herb_green")
    leaves = ((12.2, -2.1, 1), (12.7, -2.45, -1), (13.2, -2.8, 1), (13.75, -3.1, -1), (14.2, -3.4, 1))
    for k, (y, dz, side) in enumerate(leaves):
        x = CX + 0.15 + 0.1 * k
        z0 = CZ + dz - 0.35
        if side > 0:
            g.box(b, f"leaf_{k}", (x + 0.15, y, z0), (x + 1.25, y + 0.38, z0 + 0.7), "herb_green" if k % 2 == 0 else "herb_green_dk",
                  rot=(0.0, 0.0, 28.0), org=(x + 0.15, y + 0.19, z0 + 0.35))
        else:
            g.box(b, f"leaf_{k}", (x - 1.25, y, z0), (x - 0.15, y + 0.38, z0 + 0.7), "herb_green_dk" if k % 2 == 0 else "herb_green",
                  rot=(0.0, 0.0, -28.0), org=(x - 0.15, y + 0.19, z0 + 0.35))
    g.box(b, "bud", (CX + 0.2, 14.4, CZ - 3.85), (CX + 0.85, 15.3, CZ - 3.25), "herb_green_dk")
    g.box(b, "base_leaf_a", (CX - 1.1, 10.2, CZ - 0.5), (CX - 0.1, 10.6, CZ + 0.2), "herb_green_dk")
    g.box(b, "base_leaf_b", (CX - 0.5, 10.3, CZ - 0.97), (CX + 0.3, 10.7, CZ - 0.2), "herb_green")


def part_herb_pale(g: Gear) -> None:
    """两根苍白籽穗草：细茎各往前偏左 / 偏右探，顶端串着一小撮籽粒；高过 y=12 的部分在 z<-2.1。"""
    b = "herb_pale"
    stems = (
        [(CX + 0.7, 9.9, CZ - 0.3), (CX + 1.0, 10.9, CZ - 0.8), (CX + 1.3, 11.75, CZ - 1.9),
         (CX + 1.65, 12.6, CZ - 2.4), (CX + 2.0, 13.5, CZ - 2.85)],
        [(CX - 0.9, 9.9, CZ - 0.4), (CX - 1.2, 10.9, CZ - 0.9), (CX - 1.45, 11.75, CZ - 1.9),
         (CX - 1.7, 12.6, CZ - 2.4), (CX - 2.0, 13.2, CZ - 2.8)],
    )
    for n, pts in enumerate(stems):
        _stalk(g, b, f"stem_{n}", pts, 0.2, "herb_pale")
        top = pts[-1]
        for k in range(5):
            dx = (-0.18, 0.2, -0.05, 0.22, -0.2)[k]
            dz = (0.0, 0.12, -0.12, 0.08, -0.05)[k]
            # 每粒籽的 y 起点 / 高度都错开一点，避免上下面共面
            y = top[1] + 0.15 + 0.3 * (k // 2) + 0.017 * k + 0.05 * n
            g.box(b, f"seed_{n}_{k}", (top[0] + dx - 0.2, y, top[2] + dz - 0.2),
                  (top[0] + dx + 0.2, y + 0.42 + 0.013 * k, top[2] + dz + 0.2),
                  "herb_seed" if k % 2 == 0 else "herb_pale")


def part_herb_brown(g: Gear) -> None:
    """枯褐蕨叶：茎往后（+z）斜，成对的小羽叶；整条在 y<12 之下，不进手臂盒。"""
    b = "herb_brown"
    pts = [(CX + 0.3, 9.9, CZ + 0.3), (CX + 0.55, 10.5, CZ + 0.9), (CX + 0.8, 11.0, CZ + 1.5), (CX + 1.0, 11.4, CZ + 2.1)]
    _stalk(g, b, "stem", pts, 0.2, "herb_brown_dk")
    for k, (t, side) in enumerate(((0.35, 1), (0.5, -1), (0.65, 1), (0.8, -1), (0.95, 1))):
        i = min(int(t * (len(pts) - 1)), len(pts) - 2)
        a, c = pts[i], pts[i + 1]
        f = t * (len(pts) - 1) - i
        px, py, pz = (a[j] + (c[j] - a[j]) * f for j in range(3))
        g.box(b, f"pinna_{k}", (px + (0.0 if side > 0 else -0.95), py - 0.12, pz - 0.2),
              (px + (0.95 if side > 0 else 0.0), py + 0.2, pz + 0.22), "herb_brown" if k % 2 == 0 else "herb_brown_dk",
              rot=(0.0, 0.0, 22.0 * side), org=(px, py, pz))
    g.box(b, "tip", (CX + 0.85, 11.3, CZ + 2.05), (CX + 1.25, 11.7, CZ + 2.5), "herb_brown")


PART_FUNCS = {
    "belt": part_belt,
    "cord": part_cord,
    "body": part_body,
    "folds": part_folds,
    "mouth": part_mouth,
    "tie": part_tie,
    "toggle": part_toggle,
    "patches": part_patches,
    "pocket": part_pocket,
    "herb_green": part_herb_green,
    "herb_pale": part_herb_pale,
    "herb_brown": part_herb_brown,
}

SPEC = GearSpec(
    gear_id="herb_pouch",
    model_name="HerbPouch",
    title="灵草囊 / herb_pouch",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：右胯外侧（袋身 x∈[4.1,7.9]、y<12）+ 躯干前面腰带片 + 草药斜探出手臂盒之前",
    worn_focus=((4.5, 9.5, -0.5), 22.0),
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
