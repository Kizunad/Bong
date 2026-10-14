#!/usr/bin/env python3
"""矿石袋（ore_sack）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #2（.task-wasteland-gear.md）。穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。
物品描述（server/assets/items/workbench_materials.toml）：「粗布加底皮的矿石袋，耐磨。」

外形（参考图 ore_sack_ref.png / ore_sack_exploded.png）：一只沉甸甸的粗麻布大袋，挂在臀后偏右
（袋身 x∈[-0.8,5.6]、y∈[3.0,13]、z∈[2.7,8.1]，全部在躯干 / 腿的背面之外）。袋底包一只锈褐皮杯，沿上缘缝一圈粗针脚、
再用皮条竖向锁缝；布面被矿石顶出几个鼓包，三处磨破的洞里露出灰铁矿和锈橙矿的棱角；袋口褶成一圈，拧着草绳束紧、
打大结、绳头坠着木栓；袋口堆着四块矿石探出来。腰后一圈三股草绳（前后都有，只走躯干前后面外侧，两侧是手臂盒不绕过去），
一条皮提绳从腰绳垂下，带铁带扣和铁环，系住袋颈。

骨骼 14 根：腰绳 / 提绳 / 袋身 / 鼓包 / 袋褶 / 袋口 / 束绳 / 绳头木栓 / 底皮杯 / 补丁 / 破洞 / 袋口矿石 / 侧面矿石，挂件各单独一骨。
分部件：part_*() 逐件写，--part 单件预览；骨骼树在 BONES 里一处定义。
用法：
    python3 modelScript/gear/ore_sack/gen_ore_sack.py --self-test   # 门禁差分自证
    python3 modelScript/gear/ore_sack/gen_ore_sack.py --part mouth  # 单件预览
    python3 modelScript/gear/ore_sack/gen_ore_sack.py --worn        # 只出穿戴三面
    python3 modelScript/gear/ore_sack/gen_ore_sack.py               # 全量：模型 + check.png
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
    "cloth":      (158, 142, 112),   # 粗麻布
    "cloth_lt":   (182, 166, 132),
    "cloth_dk":   (112, 98, 76),
    "cloth_in":   (80, 68, 52),      # 袋口里面
    "patch_cloth": (136, 120, 96),   # 粗布补
    "leather":    (124, 72, 50),     # 锈褐底皮
    "leather_dk": (84, 50, 36),
    "leather_lt": (150, 92, 64),
    "cord":       (140, 124, 78),    # 草绳
    "cord_dk":    (100, 88, 54),
    "stitch":     (64, 50, 38),
    "wood":       (120, 88, 60),     # 绳头木栓
    "iron":       (76, 72, 72),
    "rust":       (150, 84, 48),
    "ore_grey":   (106, 108, 116),   # 灰铁矿
    "ore_dk":     (74, 76, 84),
    "ore_rust":   (156, 94, 56),     # 锈橙矿
    "ore_fleck":  (200, 130, 70),    # 矿石上的亮点
    "void":       (26, 22, 20),
}
SOFT = ("patch_cloth", "leather", "leather_dk", "leather_lt", "cord", "cord_dk",
        "ore_grey", "ore_dk", "ore_rust", "ore_fleck")
# 软件（补丁 / 皮 / 绳）压在任何材质上都允许重叠；矿石塞在布里（顶出布面）也允许。袋布之间、铁 / 木 / 针脚之间互穿仍要报
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("cloth", "cloth_dk", "patch_cloth", "cloth_in"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("belt_rope", "root", (0.0, 13.1, 2.3), "腰绳：前后各三股 + 绳结（左右对称，只走躯干前后面外侧）"),
    ("hang_strap", "belt_rope", (2.4, 14.0, 2.8), "提绳：皮条 + 铁带扣 + 铁环（挂点，会晃）"),
    ("sack_body", "hang_strap", (2.4, 11.0, 5.4), "袋身：五层鼓肚 + 织纹横线"),
    ("sack_bumps", "sack_body", (2.4, 7.0, 5.4), "鼓包：被矿石顶起的布面隆起"),
    ("sack_folds", "sack_body", (2.4, 8.0, 5.4), "袋面竖褶 + 袋肩皱"),
    ("sack_mouth", "sack_body", (2.4, 11.6, 5.4), "袋口：一圈布褶 + 翘角 + 里面暗底"),
    ("sack_tie", "sack_body", (2.4, 10.4, 5.4), "束口草绳：绕颈两圈 + 大结 + 两缕绳头"),
    ("tie_toggles", "sack_tie", (2.4, 8.0, 7.5), "绳头木栓：两枚小木栓（会晃）"),
    ("sack_bottom", "sack_body", (2.4, 4.2, 5.4), "底皮杯：锈褐皮杯 + 上缘针脚 + 竖向锁缝 + 铆钉"),
    ("sack_patches", "sack_body", (2.4, 7.0, 5.4), "补丁：皮补 / 两块粗布补 + 针脚"),
    ("sack_holes", "sack_body", (2.4, 7.0, 5.4), "磨破的洞：三处，暗洞 + 翘起的破边"),
    ("ore_mouth", "sack_body", (2.4, 13.0, 5.4), "袋口矿石：四块探出袋口的矿石棱角"),
    ("ore_side", "sack_body", (2.4, 7.0, 5.4), "洞里露出的矿石：三块（灰铁 / 锈橙）"),
]
ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "belt_rope"))

# ───────────────────────────────────────────────────────────── 常量：袋身
CX, CZ = 2.4, 5.4                  # 袋身中心（臀后偏右）
# (名, y0, y1, 半宽 x, 半深 z)
SLICES = (
    ("base", 3.4, 4.2, 1.6, 1.5),
    ("low", 4.2, 5.4, 2.5, 2.2),
    ("belly", 5.4, 8.6, 3.2, 2.7),
    ("shoulder", 8.6, 10.0, 2.7, 2.3),
    ("neck", 10.0, 11.0, 1.7, 1.5),
)
MOUTH_Y0 = 11.0


def _rock(g: Gear, bone: str, name: str, c, s, rot, main: str, alt: str) -> None:
    """一块棱角矿石：主块 + 一个错开转角的次块 + 两颗亮点，读作断面多角的矿块。"""
    h = [v / 2 for v in s]
    g.box(bone, f"{name}_a", tuple(c[k] - h[k] for k in range(3)), tuple(c[k] + h[k] for k in range(3)), main, rot=rot, org=c)
    sub = [v * 0.62 for v in s]
    c2 = (c[0] + s[0] * 0.26, c[1] + s[1] * 0.2, c[2] + s[2] * 0.16)
    g.box(bone, f"{name}_b", tuple(c2[k] - sub[k] / 2 for k in range(3)), tuple(c2[k] + sub[k] / 2 for k in range(3)), alt,
          rot=(rot[0] + 25, rot[1] - 22, rot[2] + 15), org=c2)
    f = (c[0] - s[0] * 0.22, c[1] + s[1] * 0.28, c[2] - s[2] * 0.3)
    g.box(bone, f"{name}_fleck", (f[0] - 0.22, f[1] - 0.22, f[2] - 0.22), (f[0] + 0.22, f[1] + 0.22, f[2] + 0.22), "ore_fleck",
          rot=(rot[0] + 10, rot[1] + 30, rot[2] - 20), org=f)


# ───────────────────────────────────────────────────────────── 分部件：腰绳 / 提绳
def part_belt(g: Gear) -> None:
    """腰绳：前后各三股草绳（左右对称）+ 股间的绳箍，z 在躯干 / 手臂盒之外（两侧不绕）。"""
    b = "belt_rope"
    for tag, z0, z1 in (("f", -2.6, -2.05), ("b", 2.05, 2.6)):
        for i, y in enumerate((12.2, 12.78, 13.36)):
            dz = 0.03 * i
            lo, hi = (z0 - dz, z1) if tag == "f" else (z0, z1 + dz)
            g.box(b, f"rope_{tag}{i}", (-4.5, y, lo), (4.5, y + 0.55, hi), "cord" if i % 2 == 0 else "cord_dk")
    # 绳箍：把三股箍在一起的短皮环，左右对称各一
    for tag, z0, z1 in (("f", -2.85, -2.0), ("b", 2.0, 2.85)):
        g.pair(b, f"clamp_{tag}", (3.0, 12.1, z0), (3.6, 13.95, z1), "leather_dk")
    # 绳节：每条绳端打个结
    for i in range(3):
        g.pair(b, f"rope_end_{i}", (4.45 + 0.01 * i, 12.2 + 0.58 * i, 2.1), (4.9, 12.75 + 0.58 * i, 2.7), "cord")


def part_strap(g: Gear) -> None:
    """提绳：从腰绳垂下的皮条，上端一只铁带扣（框 + 舌 + 铆钉），下端穿铁环系住袋颈。"""
    b = "hang_strap"
    sx = CX
    g.box(b, "strap", (sx - 0.8, 10.9, 2.62), (sx + 0.8, 14.3, 3.12), "leather")
    g.box(b, "strap_edge_l", (sx - 1.03, 10.95, 2.66), (sx - 0.8, 14.2, 3.08), "leather_dk")
    g.box(b, "strap_edge_r", (sx + 0.8, 10.95, 2.66), (sx + 1.03, 14.2, 3.08), "leather_dk")
    # 带扣：一圈铁框 + 扣舌 + 锈斑
    g.box(b, "buckle_top", (sx - 1.15, 13.7, 3.1), (sx + 1.15, 14.05, 3.45), "iron")
    g.box(b, "buckle_bot", (sx - 1.15, 12.75, 3.1), (sx + 1.15, 13.1, 3.45), "iron")
    g.box(b, "buckle_l", (sx - 1.15, 13.05, 3.1), (sx - 0.8, 13.75, 3.45), "iron")
    g.box(b, "buckle_r", (sx + 0.8, 13.05, 3.1), (sx + 1.15, 13.75, 3.45), "iron")
    g.box(b, "buckle_tongue", (sx - 0.12, 12.9, 3.2), (sx + 0.12, 13.9, 3.6), "rust")
    for k, y in enumerate((11.4, 12.2)):
        g.box(b, f"strap_st_{k}", (sx - 0.5, y, 3.08), (sx + 0.5, y + 0.3, 3.32), "stitch")
    # 铁环：xy 平面里的圈，挂在提绳下端，环下是系袋颈的短绳
    g.ring(b, "ring", (sx, 10.6, 3.0), 0.95, 0.5, 0.55, "iron", n=8)
    g.box(b, "ring_rust", (sx + 0.55, 10.3, 2.8), (sx + 1.1, 10.95, 3.3), "rust")
    g.rod(b, "tie_rope", (sx, 9.9, 3.1), (sx, 10.3, 4.2), 0.3, "cord")


# ───────────────────────────────────────────────────────────── 分部件：袋身
def part_body(g: Gear) -> None:
    """袋身：五层鼓肚（底小、腹大、肩收、颈束）+ 前后两面逐行织纹横线 + 袋底暗缝边。"""
    b = "sack_body"
    for tag, y0, y1, hw, hd in SLICES:
        g.box(b, f"body_{tag}", (CX - hw, y0, CZ - hd), (CX + hw, y1, CZ + hd), "cloth")
    for k, y in enumerate((5.6, 6.4, 7.2, 8.0)):
        w = 3.2 - 0.03 * k
        g.box(b, f"weave_b_{k}", (CX - w + 0.2 * (k % 2), y, CZ + 2.7), (CX + w - 0.2 * ((k + 1) % 2), y + 0.22, CZ + 2.7 + 0.22), "cloth_lt")
        g.box(b, f"weave_f_{k}", (CX - w + 0.25, y + 0.05, CZ - 2.7 - 0.22), (CX + w - 0.1, y + 0.27, CZ - 2.7), "cloth_lt")
    for k, y in enumerate((5.8, 6.9, 8.0)):
        g.box(b, f"weave_r_{k}", (CX + 3.2, y, CZ - 1.9 + 0.1 * k), (CX + 3.2 + 0.22, y + 0.22, CZ + 1.7), "cloth_lt")
        g.box(b, f"weave_l_{k}", (CX - 3.2 - 0.22, y + 0.05, CZ - 1.7), (CX - 3.2, y + 0.27, CZ + 1.9 - 0.1 * k), "cloth_lt")
    g.box(b, "bottom_hem", (CX - 1.7, 3.2, CZ - 1.6), (CX + 1.7, 3.5, CZ + 1.6), "cloth_dk")


def part_bumps(g: Gear) -> None:
    """鼓包：被矿石顶起的布面隆起，比壁面再鼓 0.4–0.6，歪着摆（外侧 / 后面 / 前下）。"""
    b = "sack_bumps"
    for k, (x, y, z, sx, sy, sz) in enumerate((
            (CX + 3.2, 7.2, CZ - 0.9, 0.5, 1.5, 1.7),
            (CX - 3.2 - 0.45, 6.3, CZ - 0.2, 0.45, 1.3, 1.5),
            (CX + 0.4, 7.9, CZ + 2.7, 1.9, 1.3, 0.5),
            (CX - 2.1, 5.9, CZ + 2.7, 1.5, 1.1, 0.45),
            (CX + 1.6, 5.7, CZ - 2.7 - 0.45, 1.6, 1.2, 0.45),
            (CX - 0.9, 8.9, CZ + 2.3, 1.6, 0.9, 0.42))):
        g.box(b, f"bump_{k}", (x, y, z), (x + sx, y + sy, z + sz), "cloth_lt" if k % 2 else "cloth")


def part_folds(g: Gear) -> None:
    """袋面竖褶：后面三道、侧面各一道、袋肩两处皱，暗色，长短错落。"""
    b = "sack_folds"
    for k, (dx, y0, y1) in enumerate(((-2.2, 7.4, 10.0), (-0.6, 6.4, 9.9), (1.4, 7.0, 9.7), (2.6, 6.6, 9.5))):
        g.box(b, f"fold_b_{k}", (CX + dx - 0.18, y0, CZ + 2.7 + 0.22), (CX + dx + 0.18, y1, CZ + 2.7 + 0.46), "cloth_dk")
    for k, (dx, y1) in enumerate(((-1.5, 9.6), (-0.3, 9.4), (1.2, 9.7))):
        g.box(b, f"fold_f_{k}", (CX + dx - 0.18, 7.0, CZ - 2.7 - 0.5), (CX + dx + 0.18, y1, CZ - 2.7 - 0.24), "cloth_dk")
    for tag, sx in (("r", 1), ("l", -1)):
        x = CX + sx * 2.7
        for k, dz in enumerate((-0.9, 0.6)):
            lo, hi = (x, x + 0.3) if sx > 0 else (x - 0.3, x)
            g.box(b, f"fold_{tag}_{k}", (lo, 8.7, CZ + dz - 0.17), (hi, 10.0 - 0.2 * k, CZ + dz + 0.17), "cloth_dk")


def part_mouth(g: Gear) -> None:
    """袋口：十四个高低错落的布褶围成一圈，四个翘角，里面一块暗底托着矿石。"""
    b = "sack_mouth"
    g.box(b, "floor", (CX - 1.5, MOUTH_Y0, CZ - 1.35), (CX + 1.5, MOUTH_Y0 + 0.3, CZ + 1.35), "cloth_in")
    heights = (1.5, 2.1, 1.3, 1.8, 1.4, 2.2, 1.2, 1.9, 1.5, 2.0, 1.3, 1.7, 1.45, 2.05)
    for i in range(14):
        a = i * math.tau / 14
        x = CX + math.cos(a) * 1.85
        z = CZ + math.sin(a) * 1.65
        y0 = MOUTH_Y0 + 0.01 * (i + 1)
        g.box(b, f"pleat_{i}", (x - 0.55, y0, z - 0.5), (x + 0.55, y0 + heights[i], z + 0.5), "cloth" if i % 3 else "cloth_lt")
    for i in range(0, 14, 2):
        a = (i + 0.5) * math.tau / 14
        x = CX + math.cos(a) * 2.2
        z = CZ + math.sin(a) * 1.98
        g.box(b, f"pleat_line_{i}", (x - 0.13, MOUTH_Y0 + 0.33, z - 0.13), (x + 0.13, MOUTH_Y0 + 1.15, z + 0.13), "cloth_dk")
    for i, (dx, dz, h) in enumerate(((-2.0, -0.9, 2.6), (1.1, -1.7, 2.7), (2.0, 0.8, 2.55), (-0.8, 1.7, 2.65))):
        g.box(b, f"tip_{i}", (CX + dx - 0.25, MOUTH_Y0 + 1.5, CZ + dz - 0.25), (CX + dx + 0.25, MOUTH_Y0 + h, CZ + dz + 0.25), "cloth")


def part_tie(g: Gear) -> None:
    """束口草绳：绕颈两圈、背面一个大结（朝外，面向背视图）、两缕绳头垂下并散成毛须。"""
    b = "sack_tie"
    for k, (y0, grow) in enumerate(((10.2, 0.0), (10.65, 0.06))):
        mat = "cord" if k == 0 else "cord_dk"
        hw, hd = 1.85 + grow, 1.65 + grow
        g.box(b, f"wrap_{k}_f", (CX - hw, y0, CZ - hd - 0.14), (CX + hw, y0 + 0.42, CZ - hd + 0.1), mat)
        g.box(b, f"wrap_{k}_b", (CX - hw, y0, CZ + hd - 0.1), (CX + hw, y0 + 0.42, CZ + hd + 0.14), mat)
        g.box(b, f"wrap_{k}_l", (CX - hw - 0.14, y0, CZ - hd + 0.14), (CX - hw + 0.1, y0 + 0.42, CZ + hd - 0.14), mat)
        g.box(b, f"wrap_{k}_r", (CX + hw - 0.1, y0, CZ - hd + 0.14), (CX + hw + 0.14, y0 + 0.42, CZ + hd - 0.14), mat)
    zk = CZ + 1.75
    g.box(b, "knot", (CX - 0.7, 9.95, zk), (CX + 0.7, 11.2, zk + 0.7), "cord_dk")
    g.box(b, "knot_ear_l", (CX - 1.4, 10.35, zk + 0.05), (CX - 0.65, 11.05, zk + 0.55), "cord")
    g.box(b, "knot_ear_r", (CX + 0.65, 10.3, zk + 0.07), (CX + 1.4, 10.98, zk + 0.52), "cord")
    g.rod(b, "tail_l", (CX - 0.25, 10.0, zk + 0.5), (CX - 0.6, 8.2, zk + 0.75), 0.25, "cord")
    g.rod(b, "tail_r", (CX + 0.25, 10.0, zk + 0.5), (CX + 0.85, 8.4, zk + 0.7), 0.25, "cord_dk")
    for k, (dx, y) in enumerate(((-0.8, 7.7), (-0.55, 7.6), (-0.3, 7.75))):
        g.box(b, f"fray_l_{k}", (CX + dx - 0.12, y, zk + 0.62), (CX + dx + 0.12, y + 0.55, zk + 0.88), "cord")
    for k, (dx, y) in enumerate(((0.95, 7.95), (1.2, 7.85))):
        g.box(b, f"fray_r_{k}", (CX + dx - 0.12, y, zk + 0.6), (CX + dx + 0.12, y + 0.55, zk + 0.84), "cord_dk")


def part_toggles(g: Gear) -> None:
    """绳头木栓：两枚小木栓别在两缕绳头上，一长一短，带一道刻痕。"""
    b = "tie_toggles"
    zk = CZ + 1.75 + 0.75
    g.box(b, "toggle_l", (CX - 1.0, 6.8, zk - 0.2), (CX - 0.25, 7.7, zk + 0.5), "wood")
    g.box(b, "toggle_l_cut", (CX - 1.05, 7.2, zk - 0.24), (CX - 0.2, 7.4, zk + 0.54), "leather_dk")
    g.box(b, "toggle_r", (CX + 0.65, 7.05, zk - 0.2), (CX + 1.4, 7.95, zk + 0.45), "wood")
    g.box(b, "toggle_r_cap", (CX + 0.6, 7.95, zk - 0.15), (CX + 1.45, 8.2, zk + 0.5), "leather_dk")


def part_bottom(g: Gear) -> None:
    """底皮杯：锈褐皮杯包住袋底，上缘一圈粗针脚缝在布上，前后各三道竖向锁缝，四枚铆钉。"""
    b = "sack_bottom"
    g.box(b, "cup_floor", (CX - 1.9, 3.0, CZ - 1.7), (CX + 1.9, 3.4, CZ + 1.7), "leather_dk")
    g.box(b, "cup_wall_f", (CX - 2.8, 3.4, CZ - 2.5 - 0.1), (CX + 2.8, 5.0, CZ - 2.2), "leather")
    g.box(b, "cup_wall_b", (CX - 2.8, 3.4, CZ + 2.2), (CX + 2.8, 5.0, CZ + 2.5 + 0.1), "leather")
    g.box(b, "cup_wall_l", (CX - 2.8, 3.4, CZ - 2.2), (CX - 2.5, 5.0, CZ + 2.2), "leather_dk")
    g.box(b, "cup_wall_r", (CX + 2.5, 3.4, CZ - 2.2), (CX + 2.8, 5.0, CZ + 2.2), "leather_dk")
    g.box(b, "cup_lip_f", (CX - 2.95, 4.8, CZ - 2.62), (CX + 2.95, 5.3, CZ - 2.2), "leather_lt")
    g.box(b, "cup_lip_b", (CX - 2.95, 4.8, CZ + 2.2), (CX + 2.95, 5.3, CZ + 2.62), "leather_lt")
    g.box(b, "cup_lip_l", (CX - 2.95, 4.8, CZ - 2.2), (CX - 2.5, 5.3, CZ + 2.2), "leather_lt")
    g.box(b, "cup_lip_r", (CX + 2.5, 4.8, CZ - 2.2), (CX + 2.95, 5.3, CZ + 2.2), "leather_lt")
    for i in range(6):
        x = CX - 2.5 + i * 1.0
        g.box(b, f"st_f{i}", (x, 5.0, CZ - 2.62 - 0.2), (x + 0.45, 5.7, CZ - 2.55 + 0.003 * i), "stitch")
        g.box(b, f"st_b{i}", (x, 5.0, CZ + 2.55 - 0.003 * i), (x + 0.45, 5.7, CZ + 2.62 + 0.2), "stitch")
    for i in range(4):
        z = CZ - 1.5 + i * 1.0
        g.box(b, f"st_l{i}", (CX - 2.95 - 0.2, 5.0, z), (CX - 2.9 + 0.003 * i, 5.7, z + 0.45), "stitch")
        g.box(b, f"st_r{i}", (CX + 2.9 - 0.003 * i, 5.0, z), (CX + 2.95 + 0.2, 5.7, z + 0.45), "stitch")
    # 前后各三道竖向锁缝（一条皮带压住杯壁，两头各一个铆钉）
    for tag, zc, sz in (("f", CZ - 2.62, -1), ("b", CZ + 2.62, 1)):
        for i, x in enumerate((CX - 1.6, CX + 0.1, CX + 1.8)):
            lo, hi = (zc - 0.22, zc) if sz < 0 else (zc, zc + 0.22)
            g.box(b, f"lace_{tag}{i}", (x, 3.5, lo), (x + 0.3, 4.75, hi), "cord_dk")
    for k, (x, z) in enumerate(((CX - 2.3, CZ - 2.62 - 0.1), (CX + 2.3, CZ - 2.62 - 0.1), (CX - 2.3, CZ + 2.62 - 0.1), (CX + 2.3, CZ + 2.62 - 0.1))):
        g.box(b, f"rivet_{k}", (x - 0.22, 3.7, z), (x + 0.22, 4.15, z + 0.3), "iron")


def part_patches(g: Gear) -> None:
    """补丁：后面一块大皮补（四边针脚）、外侧一块粗布补、后上一块小粗布补。"""
    b = "sack_patches"
    zb = CZ + 2.7
    x0, x1, y0, y1 = CX - 2.5, CX - 0.4, 6.0, 7.9
    g.box(b, "hide_patch", (x0, y0, zb + 0.22 + 0.03), (x1, y1, zb + 0.5), "leather")
    for k in range(4):
        x = x0 - 0.05 + k * 0.58
        g.box(b, f"hp_st_t{k}", (x, y1 - 0.25, zb + 0.46), (x + 0.32, y1 + 0.13, zb + 0.7 + 0.004 * k), "stitch")
        g.box(b, f"hp_st_b{k}", (x, y0 - 0.12, zb + 0.46), (x + 0.32, y0 + 0.26, zb + 0.7 + 0.004 * k), "stitch")
    for k in range(3):
        y = y0 + 0.25 + k * 0.55
        g.box(b, f"hp_st_l{k}", (x0 - 0.12, y, zb + 0.46), (x0 + 0.28, y + 0.3, zb + 0.7 + 0.004 * k), "stitch")
        g.box(b, f"hp_st_r{k}", (x1 - 0.31, y, zb + 0.46), (x1 + 0.12, y + 0.3, zb + 0.7 + 0.004 * k), "stitch")
    xo = CX + 3.2
    g.box(b, "side_patch", (xo + 0.25 + 0.03, 6.4, CZ - 0.2), (xo + 0.5, 8.2, CZ + 1.7), "patch_cloth")
    for k in range(3):
        z = CZ - 0.1 + k * 0.58
        g.box(b, f"sp_st_t{k}", (xo + 0.46, 8.0, z), (xo + 0.72 + 0.004 * k, 8.4, z + 0.3), "stitch")
        g.box(b, f"sp_st_b{k}", (xo + 0.46, 6.25, z), (xo + 0.72 + 0.004 * k, 6.65, z + 0.3), "stitch")
    g.box(b, "top_patch", (CX - 0.6, 8.7, CZ + 2.3 + 0.2 + 0.03), (CX + 1.6, 9.6, CZ + 2.3 + 0.45), "patch_cloth")
    g.box(b, "tp_st_a", (CX - 0.7, 9.45, CZ + 2.3 + 0.42), (CX + 1.7, 9.78, CZ + 2.3 + 0.66), "stitch")
    g.box(b, "tp_st_b", (CX - 0.7, 8.55, CZ + 2.3 + 0.43), (CX + 1.7, 8.88, CZ + 2.3 + 0.67), "stitch")


def part_holes(g: Gear) -> None:
    """磨破的洞：袋腹三处（后面一处、右侧一处、左侧一处），暗洞压在布上，破边向外翘；洞里是矿石。"""
    b = "sack_holes"
    spots = (
        ("b", (CX + 0.6, 6.0, CZ + 2.7 + 0.04), (CX + 2.5, 7.4, CZ + 2.7 + 0.28), "z+"),
        ("r", (CX + 3.2 + 0.04, 6.3, CZ - 2.0), (CX + 3.2 + 0.28, 7.6, CZ - 0.5), "x+"),
        ("l", (CX - 3.2 - 0.28, 6.8, CZ - 0.8), (CX - 3.2 - 0.04, 8.2, CZ + 0.6), "x-"),
    )
    for tag, lo, hi, d in spots:
        g.box(b, f"hole_{tag}", lo, hi, "void")
        # 破边：洞口四周参差的布条（向外翘，厚度 0.24）
        cx, cy, cz = [(lo[i] + hi[i]) / 2 for i in range(3)]
        for k, (u, v, w, h) in enumerate(((-0.9, 0.75, 0.55, 0.45), (0.4, 0.8, 0.6, 0.4), (0.85, -0.4, 0.4, 0.7), (-0.2, -0.95, 0.7, 0.4), (-1.0, -0.3, 0.4, 0.5))):
            if d[0] == "z":
                zlo, zhi = (hi[2] - 0.06, hi[2] + 0.2) if d == "z+" else (lo[2] - 0.2, lo[2] + 0.06)
                g.box(b, f"hole_{tag}_fray{k}", (cx + u - w / 2, cy + v - h / 2, zlo), (cx + u + w / 2, cy + v + h / 2, zhi), "cloth_lt")
            else:
                xlo, xhi = (hi[0] - 0.06, hi[0] + 0.2) if d == "x+" else (lo[0] - 0.2, lo[0] + 0.06)
                g.box(b, f"hole_{tag}_fray{k}", (xlo, cy + v - h / 2, cz + u - w / 2), (xhi, cy + v + h / 2, cz + u + w / 2), "cloth_lt")


# ───────────────────────────────────────────────────────────── 分部件：矿石
def part_ore_mouth(g: Gear) -> None:
    """袋口矿石：四块探出袋口的矿石（灰铁 / 锈橙交错），大小不一、各自转角。"""
    b = "ore_mouth"
    _rock(g, b, "m0", (CX - 0.9, 13.3, CZ + 0.3), (2.2, 2.0, 2.1), (22, 18, 10), "ore_grey", "ore_rust")
    _rock(g, b, "m1", (CX + 1.0, 13.5, CZ - 0.5), (1.9, 1.8, 1.8), (10, 42, 18), "ore_rust", "ore_dk")
    _rock(g, b, "m2", (CX + 0.1, 14.1, CZ + 1.0), (1.5, 1.5, 1.4), (34, 12, 24), "ore_dk", "ore_rust")
    _rock(g, b, "m3", (CX - 1.5, 12.6, CZ - 0.9), (1.5, 1.4, 1.5), (14, 30, 8), "ore_rust", "ore_grey")


def part_ore_side(g: Gear) -> None:
    """洞里露出的矿石：后面一块灰铁矿、右侧一块锈橙矿、左侧一块；各探出布面 0.6 左右。"""
    b = "ore_side"
    _rock(g, b, "s0", (CX + 1.55, 6.7, CZ + 2.7 + 0.15), (1.7, 1.5, 1.5), (14, 28, 12), "ore_grey", "ore_rust")
    _rock(g, b, "s1", (CX + 3.2 + 0.15, 6.95, CZ - 1.25), (1.4, 1.4, 1.5), (10, 22, 30), "ore_rust", "ore_dk")
    _rock(g, b, "s2", (CX - 3.2 - 0.15, 7.5, CZ - 0.1), (1.4, 1.5, 1.5), (26, 10, 18), "ore_dk", "ore_grey")


def deconflict(g: Gear) -> None:
    """把同向共面的面错开（Z-fighting 门禁）：只动无旋转件，把后加件的冲突面向外长 0.002–0.01px（肉眼不可见）。

    细部件（针脚 / 织纹 / 褶）密密麻麻贴在大面上，手写每一处错位既烦又易漏；这里统一处理，
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


class OreSackSpec(GearSpec):
    def build(self, parts=None) -> Gear:
        g = super().build(parts)
        deconflict(g)
        return g


PART_FUNCS = {
    "belt": part_belt,
    "strap": part_strap,
    "body": part_body,
    "bumps": part_bumps,
    "folds": part_folds,
    "mouth": part_mouth,
    "tie": part_tie,
    "toggles": part_toggles,
    "bottom": part_bottom,
    "patches": part_patches,
    "holes": part_holes,
    "ore_mouth": part_ore_mouth,
    "ore_side": part_ore_side,
}

SPEC = OreSackSpec(
    gear_id="ore_sack",
    model_name="OreSack",
    title="矿石袋 / ore_sack",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：臀后偏右（袋身 z∈[2.7,8.1]）+ 腰后 / 腰前三股草绳 + 提绳",
    worn_focus=((1.5, 9.0, 4.0), 24.0),
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
