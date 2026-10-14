#!/usr/bin/env python3
"""水囊（water_skin）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #3（.task-wasteland-gear.md）。穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。
物品描述（server/assets/items/workbench_materials.toml）：「兽皮缝的水袋，远征不渴。」

外形（参考图 water_skin_ref.png / water_skin_exploded.png）：整张兽皮缝成的水袋，鼓肚带颈，左右用浅 / 褐两种皮拼色，
背缝一排粗针脚，前面缝一块异色补皮，四条留着的皮腿（绳缠踝、带裂蹄），颈口缠草绳，铁环箍颈，骨塞（雕成颅骨块）
系一个绳圈。斜挎：皮绳从穿戴者右肩顶翻过去（骑在手臂上沿，避开头和躯干），前后各一条斜带贴在躯干前 / 背面外侧
（|z|>2.05）一路斜到左胯，带上两处草绳缠、两个铁环、边上一排针脚，末端折舌系在颈口铁环上。水囊挂在左胯外侧
（x∈[-8.2,-4.2]、y∈[2.2,11.9]），整个在手臂盒（y≥12）之下、左腿盒（x≥-4）之外。腰上一条前后对称的皮腰带压住斜带末端，
带扣在前面正中。

骨骼 15 根：过肩带 / 前斜带 / 后斜带 / 腰带 / 铁环链 / 袋身 / 绒毛 / 皮腿 / 缝线 / 补丁 / 颈口 / 骨塞 / 塞绳，挂件各单独一骨。
分部件：part_*() 逐件写，--part 单件预览；骨骼树在 BONES 里一处定义。
用法：
    python3 modelScript/gear/water_skin/gen_water_skin.py --self-test    # 门禁差分自证
    python3 modelScript/gear/water_skin/gen_water_skin.py --part skin    # 单件预览
    python3 modelScript/gear/water_skin/gen_water_skin.py --worn         # 只出穿戴三面
    python3 modelScript/gear/water_skin/gen_water_skin.py                # 全量：模型 + check.png
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from bbmodel_maker.rig.rigkit import shaft_box  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质（RGB，每种占贴图一个 8×8 色块）
MATS = {
    "hide_pale":    (196, 176, 140),   # 浅色兽皮
    "hide_pale_dk": (160, 140, 108),
    "hide_brown":   (128, 86, 60),     # 褐色兽皮
    "hide_brown_dk": (96, 62, 44),
    "fur":          (214, 196, 160),   # 浅皮上的绒毛
    "patch":        (118, 74, 54),     # 补皮
    "strap":        (112, 72, 50),     # 皮绳带
    "strap_dk":     (78, 50, 38),
    "strap_lt":     (140, 96, 68),
    "cord":         (142, 120, 76),
    "cord_dk":      (102, 86, 54),
    "bone":         (222, 212, 184),
    "bone_dk":      (170, 156, 126),
    "iron":         (84, 82, 84),
    "rust":         (150, 86, 50),
    "stitch":       (62, 48, 36),
    "hoof":         (58, 46, 40),
    "hoof_dk":      (38, 30, 28),
    "void":         (26, 22, 20),
}
SOFT = ("patch", "strap", "strap_dk", "strap_lt", "cord", "cord_dk", "fur")
# 软件（皮带 / 绳 / 补皮 / 绒毛）压在任何材质上都允许重叠；袋皮、骨、铁、蹄、针脚之间互穿仍要报
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("hoof", "hoof_dk", "hide_pale", "hide_brown", "bone"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("strap_shoulder", "root", (5.0, 24.3, 0.0), "过肩带：右肩顶一块横跨前后的平板 + 前后立带 + 肩垫 + 针脚（挂点）"),
    ("strap_front", "strap_shoulder", (5.0, 22.0, -2.33), "前斜带：贴胸前斜到左胯 + 两处草绳缠 + 边缘针脚 + 末端折舌"),
    ("strap_back", "strap_shoulder", (5.0, 22.0, 2.33), "后斜带：贴背斜到左胯（同前）"),
    ("belt_waist", "root", (0.0, 12.9, 0.0), "腰带：前后对称两条压住斜带末端 + 带扣 + 铆钉"),
    ("strap_rings", "strap_front", (-5.0, 11.0, 0.0), "铁环链：颈口铁环 + 两个挂环 + 前后斜带上的铁环（会晃）"),
    ("skin_body", "strap_rings", (-6.2, 10.4, 0.0), "袋身：六层鼓肚，左浅右褐拼色"),
    ("skin_fur", "skin_body", (-6.2, 7.0, 0.0), "浅皮半边的绒毛簇"),
    ("skin_legs", "skin_body", (-6.2, 3.8, 0.0), "四条皮腿：绳缠踝 + 带裂的蹄"),
    ("skin_seams", "skin_body", (-6.2, 7.0, 0.0), "缝线：背缝 / 前缝一排粗针脚 + 两皮交界的横缝"),
    ("skin_patches", "skin_body", (-6.2, 6.5, 0.0), "补皮：前面一块褐补 + 背面一块浅补 + 针脚"),
    ("skin_neck", "skin_body", (-6.2, 9.6, 0.0), "颈口：兽皮领 + 缠颈草绳 + 铁环箍"),
    ("skin_stopper", "skin_neck", (-6.2, 10.9, 0.0), "骨塞：塞身 + 雕成颅骨的塞帽（会晃）"),
    ("stopper_cord", "skin_stopper", (-5.5, 10.9, 0.0), "塞绳：从塞颈拖出的绳圈 + 结（会晃）"),
]
ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "belt_waist"))

# ───────────────────────────────────────────────────────────── 常量
CX, CZ = -6.2, 0.0                 # 水囊中心（穿戴者左胯外侧）
SLICES = (
    ("base", 3.4, 4.2, 1.3, 1.3),
    ("low", 4.2, 5.4, 1.75, 1.7),
    ("belly", 5.4, 7.2, 1.98, 1.9),
    ("belly2", 7.2, 8.2, 1.85, 1.75),
    ("shoulder", 8.2, 9.0, 1.45, 1.4),
    ("neck", 9.0, 10.0, 0.85, 0.85),
)
# 每层 (左半 x<CX, 右半 x>CX) 的皮色，上下对调读作拼缝皮
HALVES = {
    "base": ("hide_brown", "hide_pale"),
    "low": ("hide_brown", "hide_pale"),
    "belly": ("hide_pale", "hide_brown"),
    "belly2": ("hide_pale", "hide_brown"),
    "shoulder": ("hide_pale", "hide_brown"),
    "neck": ("hide_brown", "hide_brown"),
}
SHOULDER_X = (4.15, 5.95)          # 过肩带（右肩，+x）
STRAP_A = (5.05, 20.6)             # 斜带肩下起点 (x, y)
STRAP_B = (-4.95, 12.75)           # 斜带袋端终点
Z_FRONT, Z_BACK = -2.33, 2.33
STRAP_HALF_W, STRAP_HALF_T = 0.75, 0.28


def _band(g: Gear, bone: str, name: str, a, b, hw: float, ht: float, zdir: float, mat: str) -> None:
    """a→b 的平板（薄的一维朝 z，贴躯干前 / 背面）。"""
    frm, to, rot, org = shaft_box(a, b, hw, ht, up=(0.0, 0.0, zdir))
    g.box(bone, name, frm, to, mat, rot=rot, org=org)


def _lerp(a, b, t):
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


# ───────────────────────────────────────────────────────────── 分部件：肩带
def part_shoulder(g: Gear) -> None:
    """过肩带：右肩顶一块横跨前后的平板，前后各一道下垂的立带，上面压肩垫、缝针脚。"""
    b = "strap_shoulder"
    x0, x1 = SHOULDER_X
    g.box(b, "shoulder_top", (x0, 24.05, -2.6), (x1, 24.6, 2.6), "strap")
    g.box(b, "drop_front", (x0, 20.4, -2.6), (x1, 24.6, -2.05), "strap")
    g.box(b, "drop_back", (x0, 20.4, 2.05), (x1, 24.6, 2.6), "strap")
    g.box(b, "pad", (x0 + 0.2, 24.6, -1.5), (x1 - 0.2, 25.15, 1.5), "strap_lt")
    for k, z in enumerate((-0.9, 0.0, 0.9)):
        g.box(b, f"pad_st_{k}", (x0 + 0.5, 25.1, z - 0.14), (x1 - 0.5, 25.4, z + 0.14), "stitch")
    for k, z in enumerate((-2.1, 1.8)):
        g.box(b, f"edge_st_{k}", (x0 + 0.3, 24.5, z), (x1 - 0.3, 24.8, z + 0.3), "stitch")
    for k, (zlo, zhi) in enumerate(((-2.78, -2.5), (2.5, 2.78))):
        for j, y in enumerate((21.0, 22.6)):
            g.box(b, f"drop_st_{k}{j}", (x0 + 0.35, y, zlo), (x1 - 0.35, y + 0.3, zhi), "stitch")


def _diagonal(g: Gear, bone: str, tag: str, zc: float, zdir: float) -> None:
    """一侧斜带：肩下 → 袋端，两处草绳缠、两个铁环、边缘针脚、末端立带 + 折舌（舌向袋颈）+ 穿孔。"""
    a = (STRAP_A[0], STRAP_A[1], zc)
    b = (STRAP_B[0], STRAP_B[1], zc)
    _band(g, bone, f"band_{tag}", a, b, STRAP_HALF_W, STRAP_HALF_T, zdir, "strap")
    d = (b[0] - a[0], b[1] - a[1])
    n = math.hypot(*d)
    u, v = d[0] / n, d[1] / n                       # 沿带方向
    px, py = -v, u                                  # 带面内垂直方向
    z_out = zc + zdir * STRAP_HALF_T                # 带外表面（远离躯干）
    # 绳缠：沿带取两个点，套一圈草绳（比带宽、略厚）
    for i, t in enumerate((0.3, 0.7)):
        c = _lerp(a, b, t)
        zw = zc + zdir * 0.2                        # 缠绳整体外移，内沿才不压进躯干（z=±2）
        p0 = (c[0] - u * 0.3, c[1] - v * 0.3, zw)
        p1 = (c[0] + u * 0.3, c[1] + v * 0.3, zw)
        _band(g, bone, f"wrap_{tag}{i}", p0, p1, STRAP_HALF_W + 0.22, STRAP_HALF_T + 0.17, zdir, "cord" if i == 0 else "cord_dk")
    # 边缘针脚：沿带两侧各一排短线（贴外表面）
    for k in range(8):
        t = 0.08 + k * 0.115
        c = _lerp(a, b, t)
        for s in (-1, 1):
            cx, cy = c[0] + px * s * 0.52, c[1] + py * s * 0.52
            q0 = (cx - u * 0.22, cy - v * 0.22, z_out - zdir * 0.05)
            q1 = (cx + u * 0.22, cy + v * 0.22, z_out - zdir * 0.05)
            frm, to, rot, org = shaft_box(q0, q1, 0.13, 0.2, up=(0.0, 0.0, zdir))
            g.box(bone, f"edge_{tag}{k}{'a' if s > 0 else 'b'}", frm, to, "stitch", rot=rot, org=org)
    # 末端：立带下到袋颈高度，再折一条横舌向袋颈（y<12，躲开手臂盒）
    zlo, zhi = zc - STRAP_HALF_T, zc + STRAP_HALF_T
    xl, xr = -5.6, -4.3
    g.box(bone, f"end_drop_{tag}", (xl, 10.6, zlo), (xr, 13.0, zhi), "strap")
    z_in = zc - zdir * 1.5                          # 舌尖朝袋颈（z 回到 ±0.8）
    g.box(bone, f"end_tongue_{tag}", (xl, 10.25, min(zlo, z_in)), (xr, 10.95, max(zhi, z_in)), "strap_dk")
    g.box(bone, f"end_hole_{tag}", (xl + 0.4, 11.5, zlo - 0.1 if zdir < 0 else zhi - 0.1),
          (xl + 0.95, 12.1, zlo + 0.1 if zdir < 0 else zhi + 0.1), "void")
    for k, y in enumerate((12.4, 12.9)):
        g.box(bone, f"end_st_{tag}{k}", (xl + 0.2, y, zlo - 0.22 if zdir < 0 else zhi - 0.06), (xr - 0.2, y + 0.3, zlo + 0.06 if zdir < 0 else zhi + 0.22), "stitch")


def part_front(g: Gear) -> None:
    """前斜带：从右肩前沿贴着胸前斜到左胯，末端折舌接颈口铁环。"""
    _diagonal(g, "strap_front", "f", Z_FRONT, -1.0)


def part_back(g: Gear) -> None:
    """后斜带：与前带对应地贴背斜到左胯，在颈口铁环汇合。"""
    _diagonal(g, "strap_back", "b", Z_BACK, 1.0)


def part_belt(g: Gear) -> None:
    """腰带：前后对称的两条皮腰带（压在斜带末端外面），前面正中铁带扣，两端铆钉、针脚行。"""
    b = "belt_waist"
    g.box(b, "belt_f", (-4.9, 12.3, -3.0), (4.9, 13.4, -2.66), "strap_dk")
    g.box(b, "belt_b", (-4.9, 12.3, 2.66), (4.9, 13.4, 3.0), "strap_dk")
    g.box(b, "buckle_top", (-1.0, 13.0, -3.35), (1.0, 13.45, -2.98), "iron")
    g.box(b, "buckle_bot", (-1.0, 12.25, -3.35), (1.0, 12.7, -2.98), "iron")
    g.box(b, "buckle_l", (-1.0, 12.65, -3.35), (-0.55, 13.05, -2.98), "iron")
    g.box(b, "buckle_r", (0.55, 12.65, -3.35), (1.0, 13.05, -2.98), "iron")
    g.box(b, "buckle_tongue", (-0.15, 12.55, -3.55), (0.15, 13.2, -3.3), "rust")
    for k in range(4):
        x = 1.5 + k * 0.85
        g.pair(b, f"st_f_{k}", (x, 12.55, -3.2), (x + 0.4, 12.85, -2.9), "stitch")
        g.pair(b, f"st_b_{k}", (x, 12.9, 2.9), (x + 0.4, 13.2, 3.2), "stitch")
    for k, x in enumerate((1.3, 4.3)):
        g.pair(b, f"rivet_{k}", (x, 12.7, -3.3), (x + 0.45, 13.1, -3.0), "iron")
    g.box(b, "back_keeper", (-0.9, 12.1, 3.0), (0.9, 13.6, 3.3), "strap")
    g.box(b, "back_keeper_hole", (-0.35, 12.5, 3.2), (0.35, 13.2, 3.42), "void")


def part_rings(g: Gear) -> None:
    """铁环链：颈口铁环（四条边的方框）+ 左右两只挂环（xy 平面里的圈）+ 前后斜带上方各一个小铁环。"""
    b = "strap_rings"
    g.ring(b, "link_f", (-5.1, 10.7, -1.45), 0.65, 0.45, 0.45, "iron", n=6)
    g.ring(b, "link_b", (-5.1, 10.7, 1.45), 0.65, 0.45, 0.45, "iron", n=6)
    g.box(b, "link_rust_f", (-5.6, 10.0, -1.75), (-5.0, 10.45, -1.2), "rust")
    g.box(b, "link_rust_b", (-5.6, 10.0, 1.2), (-5.0, 10.45, 1.75), "rust")
    # 斜带上的小铁环（挂在带面，f / b 各一）
    g.ring(b, "ring_f", (3.1, 18.2, -2.95), 0.6, 0.4, 0.5, "iron", n=6)
    g.ring(b, "ring_b", (3.1, 18.2, 2.95), 0.6, 0.4, 0.5, "iron", n=6)


# ───────────────────────────────────────────────────────────── 分部件：水囊
def part_skin(g: Gear) -> None:
    """袋身：六层鼓肚，左半浅皮右半褐皮（上下对调拼缝），每层各自两块。"""
    b = "skin_body"
    for tag, y0, y1, hw, hd in SLICES:
        left, right = HALVES[tag]
        g.box(b, f"{tag}_l", (CX - hw, y0, CZ - hd), (CX, y1, CZ + hd), left)
        g.box(b, f"{tag}_r", (CX, y0, CZ - hd), (CX + hw, y1, CZ + hd), right)
    g.box(b, "belly_shade_l", (CX - 1.98 - 0.22, 5.8, CZ - 1.2), (CX - 1.98, 6.9, CZ + 1.1), "hide_pale_dk")
    g.box(b, "belly_shade_r", (CX + 1.98, 5.9, CZ - 1.0), (CX + 1.98 + 0.22, 7.0, CZ + 1.2), "hide_brown_dk")


def part_fur(g: Gear) -> None:
    """浅皮半边的绒毛簇：前后左面一簇簇竖起的短毛。"""
    b = "skin_fur"
    spots = ((-1.6, 5.9, -1.9, "z-"), (-0.8, 7.1, -1.85, "z-"), (-1.3, 6.5, 1.9, "z+"), (-0.5, 5.6, 1.9, "z+"),
             (-1.4, 7.6, 1.75, "z+"), (-1.9, 6.0, -0.4, "x-"), (-1.9, 7.3, 0.6, "x-"), (-1.9, 5.7, 0.9, "x-"))
    for k, (dx, y, c, d) in enumerate(spots):
        h = 0.5 + 0.1 * (k % 3)
        if d[0] == "z":
            zlo, zhi = (CZ + c - 0.05, CZ + c + 0.3) if d == "z+" else (CZ + c - 0.3, CZ + c + 0.05)
            g.box(b, f"tuft_{k}", (CX + dx - 0.25, y, zlo), (CX + dx + 0.25, y + h, zhi), "fur")
        else:
            g.box(b, f"tuft_{k}", (CX + dx - 0.3, y, CZ + c - 0.25), (CX + dx + 0.05, y + h, CZ + c + 0.25), "fur")
        g.box(b, f"tuft_tip_{k}", (CX + dx - 0.12 if d[0] == "z" else CX + dx - 0.34, y + h, (CZ + c - 0.12) if d[0] == "x" else (CZ + c - 0.05 if d == "z+" else CZ + c - 0.3)),
              (CX + dx + 0.12 if d[0] == "z" else CX + dx - 0.02, y + h + 0.25, (CZ + c + 0.12) if d[0] == "x" else (CZ + c + 0.3 if d == "z+" else CZ + c + 0.05)), "fur")


def part_legs(g: Gear) -> None:
    """四条皮腿：袋底四角各一条短腿，踝上两道草绳缠，蹄裂成两瓣（中间一道暗缝）。"""
    b = "skin_legs"
    for i, (sx, sz) in enumerate(((-1, -1), (1, -1), (-1, 1), (1, 1))):
        x0 = CX + sx * 1.0 - 0.5
        z0 = CZ + sz * 0.95 - 0.5
        mat = "hide_pale" if (sx + sz) % 4 else "hide_brown"
        g.box(b, f"leg_{i}", (x0, 2.95, z0), (x0 + 1.0, 3.75, z0 + 1.0), mat)
        g.box(b, f"anklet_{i}_a", (x0 - 0.12, 3.05, z0 - 0.12), (x0 + 1.12, 3.35, z0 + 1.12), "cord")
        g.box(b, f"anklet_{i}_b", (x0 - 0.1, 3.42, z0 - 0.1), (x0 + 1.1, 3.62, z0 + 1.1), "cord_dk")
        g.box(b, f"hoof_{i}_a", (x0 - 0.05, 2.15, z0 - 0.05), (x0 + 0.5, 3.0, z0 + 1.05), "hoof")
        g.box(b, f"hoof_{i}_b", (x0 + 0.5, 2.2, z0 - 0.05 + 0.02), (x0 + 1.05, 3.0, z0 + 1.05), "hoof_dk")


def part_seams(g: Gear) -> None:
    """缝线：背缝 / 前缝各一排粗针脚（沿袋中线，随层宽收窄），两皮交界处腰上一圈横向缝线。"""
    b = "skin_seams"
    for k in range(9):
        y = 4.3 + k * 0.62
        hd = next(h for _, y0, y1, _, h in SLICES if y0 <= y + 0.2 < y1)
        g.box(b, f"seam_b_{k}", (CX - 0.22, y, CZ + hd - 0.1), (CX + 0.22, y + 0.4, CZ + hd + 0.3 + 0.004 * k), "stitch")
        g.box(b, f"seam_f_{k}", (CX - 0.22, y + 0.05, CZ - hd - 0.3 - 0.004 * k), (CX + 0.22, y + 0.45, CZ - hd + 0.1), "stitch")
    # 两皮交界的横缝：袋腹 y≈7.2 一圈短针脚（前后各 3，左右各 2）
    for k, dx in enumerate((-1.2, -0.45, 0.45, 1.2)):
        g.box(b, f"ring_st_f{k}", (CX + dx - 0.2, 7.05, CZ - 1.75 - 0.3), (CX + dx + 0.2, 7.55, CZ - 1.75 + 0.06), "stitch")
        g.box(b, f"ring_st_b{k}", (CX + dx - 0.2, 7.1, CZ + 1.75 - 0.06), (CX + dx + 0.2, 7.6, CZ + 1.75 + 0.3), "stitch")
    for k, dz in enumerate((-0.7, 0.5)):
        g.box(b, f"ring_st_l{k}", (CX - 1.85 - 0.3, 7.08, CZ + dz - 0.2), (CX - 1.85 + 0.06, 7.58, CZ + dz + 0.2), "stitch")
        g.box(b, f"ring_st_r{k}", (CX + 1.85 - 0.06, 7.12, CZ + dz - 0.2), (CX + 1.85 + 0.3, 7.62, CZ + dz + 0.2), "stitch")


def part_patches(g: Gear) -> None:
    """补皮：前面浅皮半边一块褐色补皮（四边针脚）、背面褐皮半边一块浅色补皮（三边针脚）。"""
    b = "skin_patches"
    zf = CZ - 1.9
    px0, px1, py0, py1 = CX - 1.7, CX - 0.2, 5.8, 7.4
    g.box(b, "patch_f", (px0, py0, zf - 0.28), (px1, py1, zf), "patch")
    for k in range(3):
        x = px0 + 0.1 + k * 0.5
        g.box(b, f"pf_st_t{k}", (x, py1 - 0.24, zf - 0.44), (x + 0.3, py1 + 0.14, zf - 0.08), "stitch")
        g.box(b, f"pf_st_b{k}", (x, py0 - 0.12, zf - 0.44), (x + 0.3, py0 + 0.26, zf - 0.08), "stitch")
    for k in range(2):
        y = py0 + 0.35 + k * 0.6
        g.box(b, f"pf_st_l{k}", (px0 - 0.15, y, zf - 0.44), (px0 + 0.3, y + 0.3, zf - 0.08), "stitch")
        g.box(b, f"pf_st_r{k}", (px1 - 0.33, y, zf - 0.44), (px1 + 0.15, y + 0.3, zf - 0.08), "stitch")
    zb = CZ + 1.9
    g.box(b, "patch_b", (CX + 0.3, 5.6, zb), (CX + 1.7, 7.2, zb + 0.28), "hide_pale_dk")
    for k in range(3):
        x = CX + 0.4 + k * 0.45
        g.box(b, f"pb_st_t{k}", (x, 7.0, zb + 0.08), (x + 0.28, 7.38, zb + 0.44), "stitch")
        g.box(b, f"pb_st_b{k}", (x, 5.42, zb + 0.08), (x + 0.28, 5.8, zb + 0.44), "stitch")


def part_neck(g: Gear) -> None:
    """颈口：兽皮领（一圈四块）+ 缠颈草绳两圈 + 铁环箍（四条边的方框）+ 领口缺口。"""
    b = "skin_neck"
    g.box(b, "collar_f", (CX - 1.0, 9.9, CZ - 1.0), (CX + 1.0, 10.9, CZ - 0.6), "hide_brown_dk")
    g.box(b, "collar_b", (CX - 1.0, 9.9, CZ + 0.6), (CX + 1.0, 10.9 - 0.15, CZ + 1.0), "hide_brown_dk")
    g.box(b, "collar_l", (CX - 1.0, 9.9, CZ - 0.6), (CX - 0.6, 10.9 + 0.1, CZ + 0.6), "hide_brown")
    g.box(b, "collar_r", (CX + 0.6, 9.9, CZ - 0.6), (CX + 1.0, 10.9 - 0.1, CZ + 0.6), "hide_brown")
    g.box(b, "wrap_a", (CX - 1.0, 9.25, CZ - 1.0), (CX + 1.0, 9.62, CZ + 1.0), "cord")
    g.box(b, "wrap_b", (CX - 1.02, 9.7, CZ - 1.02), (CX + 1.02, 10.0, CZ + 1.02), "cord_dk")
    g.box(b, "ring_f", (CX - 1.2, 10.0, CZ - 1.2), (CX + 1.2, 10.45, CZ - 0.85), "iron")
    g.box(b, "ring_b", (CX - 1.2, 10.05, CZ + 0.85), (CX + 1.2, 10.5, CZ + 1.2), "iron")
    g.box(b, "ring_l", (CX - 1.2, 10.1, CZ - 0.85), (CX - 0.85, 10.55, CZ + 0.85), "iron")
    g.box(b, "ring_r", (CX + 0.85, 10.15, CZ - 0.85), (CX + 1.2, 10.6, CZ + 0.85), "iron")
    g.box(b, "ring_rust", (CX + 0.5, 10.0, CZ - 1.32), (CX + 1.1, 10.4, CZ - 1.08), "rust")


def part_stopper(g: Gear) -> None:
    """骨塞：塞身插进颈口，塞帽雕成颅骨块（两个眼洞、一道牙缝），塞颈缠草绳。顶 ≤11.9（手臂盒之下）。"""
    b = "skin_stopper"
    g.box(b, "plug", (CX - 0.55, 9.95, CZ - 0.55), (CX + 0.55, 11.3, CZ + 0.55), "bone")
    g.box(b, "plug_wrap", (CX - 0.72, 10.75, CZ - 0.72), (CX + 0.72, 11.15, CZ + 0.72), "cord")
    g.box(b, "skull", (CX - 0.95, 11.1, CZ - 0.95), (CX + 0.95, 11.9, CZ + 0.95), "bone")
    g.box(b, "skull_shade", (CX + 0.7, 11.15, CZ - 0.7), (CX + 1.02, 11.8, CZ + 0.7), "bone_dk")
    g.box(b, "eye_l", (CX - 0.62, 11.45, CZ - 1.12), (CX - 0.2, 11.75, CZ - 0.88), "void")
    g.box(b, "eye_r", (CX + 0.2, 11.45, CZ - 1.12), (CX + 0.62, 11.75, CZ - 0.88), "void")
    g.box(b, "teeth", (CX - 0.5, 11.12, CZ - 1.13), (CX + 0.5, 11.35, CZ - 0.9), "bone_dk")
    g.box(b, "crack", (CX - 0.15, 11.7, CZ - 0.96), (CX + 0.1, 11.92, CZ + 0.3), "bone_dk")


def part_stopper_cord(g: Gear) -> None:
    """塞绳：从塞颈拖出一截绳，打结，末端一个绳圈系在颈口铁环上。"""
    b = "stopper_cord"
    g.rod(b, "cord_a", (CX + 0.7, 11.0, CZ + 0.05), (CX + 1.6, 10.8, CZ + 0.5), 0.2, "cord")
    g.box(b, "knot", (CX + 1.4, 10.55, CZ + 0.2), (CX + 2.0, 11.15, CZ + 0.8), "cord_dk")
    g.box(b, "loop_a", (CX + 1.8, 10.0, CZ + 0.3), (CX + 2.15, 10.7, CZ + 0.62), "cord")
    g.box(b, "loop_b", (CX + 1.5, 9.8, CZ + 0.28), (CX + 2.1, 10.15, CZ + 0.64), "cord_dk")
    g.box(b, "tail", (CX + 1.78, 9.1, CZ + 0.38), (CX + 2.06, 10.05, CZ + 0.66), "cord")


def deconflict(g: Gear) -> None:
    """把同向共面的面错开（Z-fighting 门禁）：只动无旋转件，把后加件的冲突面向外长 0.002–0.01px（肉眼不可见）。

    细部件（针脚 / 绒毛 / 补皮）密密麻麻贴在大面上，手写每一处错位既烦又易漏；这里统一处理，
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


class WaterSkinSpec(GearSpec):
    def build(self, parts=None) -> Gear:
        g = super().build(parts)
        deconflict(g)
        return g


PART_FUNCS = {
    "shoulder": part_shoulder,
    "front": part_front,
    "back": part_back,
    "belt": part_belt,
    "rings": part_rings,
    "skin": part_skin,
    "fur": part_fur,
    "legs": part_legs,
    "seams": part_seams,
    "patches": part_patches,
    "neck": part_neck,
    "stopper": part_stopper,
    "stopper_cord": part_stopper_cord,
}

SPEC = WaterSkinSpec(
    gear_id="water_skin",
    model_name="WaterSkin",
    title="水囊 / water_skin",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：右肩顶过肩（骑手臂上沿）+ 胸前 / 背面斜带到左胯 + 左胯外侧水囊（y<12）+ 前后腰带",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
