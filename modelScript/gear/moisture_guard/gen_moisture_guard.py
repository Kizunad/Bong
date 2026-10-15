#!/usr/bin/env python3
"""防潮包（moisture_guard）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #4（.task-wasteland-gear.md）。穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。
物品描述（server/assets/items/workbench_materials.toml）：「灰烬苔裹布包，吸湿防霉。」

外形（参考图 moisture_guard_ref.png / moisture_guard_exploded.png）：一卷粗布卷成的长圆筒（八棱柱，竖着挂），
底边撕成参差的破布条，布面一块皮补丁、几道横箍；外面裹着厚厚的灰烬苔——上半截成片的苔壳，顶上堆一团，
从壳沿垂下长短不一的苔须；两道交叉的草绳斜着捆在外侧，十字处打结；外侧两块锈铁片（铆钉）、前面一根骨夹板（草绳扎两道）。
挂在穿戴者左胯外侧（筒身 x∈[-8.3,-4.5]、y∈[3.2,11.4]，苔壳最内缘 x≥-4.05），整个在手臂盒（y≥12）之下、左腿盒（x≥-4）之外。
腰上三股草绳前后各一圈（左右对称，只走躯干前后面外侧 |z|>2.05，两侧是手臂盒不绕），前面一块铁带扣，
左前胯一个绳结垂下绳圈、穿铁环，系住包顶。

骨骼 14 根：腰绳 / 挂绳环 / 布卷 / 破布条 / 皮补 / 铁片 / 骨夹板 / 横箍 / 苔顶 / 苔壳 / 苔须 / 斜捆草绳，挂件各单独一骨。
分部件：part_*() 逐件写，--part 单件预览；骨骼树在 BONES 里一处定义。
用法：
    python3 modelScript/gear/moisture_guard/gen_moisture_guard.py --self-test   # 门禁差分自证
    python3 modelScript/gear/moisture_guard/gen_moisture_guard.py --part moss   # 单件预览
    python3 modelScript/gear/moisture_guard/gen_moisture_guard.py --worn        # 只出穿戴三面
    python3 modelScript/gear/moisture_guard/gen_moisture_guard.py               # 全量：模型 + check.png
"""

from __future__ import annotations

import math
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "_kit"))
import gear_kit  # noqa: E402
from bbmodel_maker.rig.rigkit import shaft_box  # noqa: E402
from gear_kit import Gear, GearSpec, soft_pairs  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质（RGB，每种占贴图一个 8×8 色块）
MATS = {
    "cloth":      (176, 162, 134),   # 旧布：灰米
    "cloth_lt":   (198, 186, 156),
    "cloth_dk":   (128, 112, 88),    # 横箍 / 阴面
    "cloth_stain": (146, 122, 92),   # 潮渍
    "leather":    (122, 76, 56),     # 皮补
    "leather_dk": (84, 54, 40),
    "cord":       (144, 118, 76),    # 草绳
    "cord_dk":    (104, 86, 54),
    "stitch":     (62, 48, 36),
    "iron":       (84, 80, 80),
    "rust":       (150, 86, 50),
    "bone":       (218, 208, 180),
    "bone_dk":    (166, 152, 122),
    "moss":       (112, 126, 78),    # 灰烬苔：灰绿
    "moss_dk":    (72, 86, 56),
    "moss_lt":    (150, 160, 100),   # 苔尖偏黄
    "moss_ash":   (152, 152, 138),   # 苔尖的灰白（灰烬）
    "void":       (26, 22, 20),
}
SOFT = ("leather", "leather_dk", "cord", "cord_dk", "moss", "moss_dk", "moss_lt", "moss_ash")
# 软件（皮补 / 绳 / 苔）压在任何材质上都允许重叠；布、铁、骨、针脚之间互穿仍要报
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("cloth", "cloth_dk", "iron", "bone", "leather"))

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("belt_rope", "root", (0.0, 13.1, 0.0), "腰绳：前后各三股 + 绳箍 + 前面铁带扣（左右对称，只走躯干前后面外侧）"),
    ("hang_loop", "belt_rope", (-4.8, 12.4, -2.4), "挂绳环：左前胯的绳结 + 两股垂绳 + 铁环（挂点，会晃）"),
    ("bundle_core", "hang_loop", (-6.4, 11.2, 0.0), "布卷：四层八棱柱（五块拼出八边）"),
    ("bundle_bands", "bundle_core", (-6.4, 8.0, 0.0), "布面横箍 + 潮渍 + 卷边褶"),
    ("bundle_tears", "bundle_core", (-6.4, 4.2, 0.0), "底边撕烂的参差破布条"),
    ("bundle_patch", "bundle_core", (-8.3, 7.0, 0.0), "皮补丁：外侧一块 + 针脚"),
    ("bundle_plates", "bundle_core", (-8.6, 7.4, 0.0), "外侧两块锈铁片 + 铆钉 + 锈斑"),
    ("bundle_splint", "bundle_core", (-7.0, 8.0, -2.5), "前面骨夹板 + 两道草绳绑扎"),
    ("lash_ropes", "bundle_core", (-8.6, 8.6, 0.0), "斜捆草绳：外侧交叉两道 + 前面交叉 + 十字结"),
    ("moss_cap", "bundle_core", (-6.4, 11.4, 0.0), "苔顶：顶上堆起的一团苔"),
    ("moss_shell", "bundle_core", (-6.4, 9.6, 0.0), "苔壳：上半截三层成片的苔块（前面留一道缝露布）"),
    ("moss_strands", "moss_shell", (-6.4, 7.4, 0.0), "苔须：壳沿垂下的长短不一的细苔条（会晃）"),
]
ASYM_BONES = frozenset(b[0] for b in BONES if b[0] not in ("root", "belt_rope"))

# ───────────────────────────────────────────────────────────── 常量
CX, CZ = -6.4, 0.0
RX, RZ = 1.9, 1.9                  # 布卷半径（八棱柱的外接半宽）
Y_BOT, Y_TOP = 4.2, 11.2
BANDS = ((4.2, 6.2), (6.2, 8.2), (8.2, 10.0), (10.0, 11.2))


def _octagon(g: Gear, bone: str, name: str, y0: float, y1: float, rx: float, rz: float, mat: str) -> None:
    """一层八棱柱：三块拼出八边（宽条 / 深条 / 抹角方块）。"""
    g.box(bone, f"{name}_x", (CX - rx, y0, CZ - (rz - 1.0)), (CX + rx, y1, CZ + (rz - 1.0)), mat)
    g.box(bone, f"{name}_z", (CX - (rx - 1.0), y0, CZ - rz), (CX + (rx - 1.0), y1, CZ + rz), mat)
    g.box(bone, f"{name}_c", (CX - (rx - 0.45), y0, CZ - (rz - 0.45)), (CX + (rx - 0.45), y1, CZ + (rz - 0.45)), mat)


# ───────────────────────────────────────────────────────────── 分部件：腰绳 / 挂绳
def part_belt(g: Gear) -> None:
    """腰绳：前后各三股草绳（左右对称）+ 股间两道皮绳箍 + 前面铁带扣（框 + 舌 + 铆钉）。"""
    b = "belt_rope"
    for tag, z0, z1 in (("f", -2.6, -2.05), ("b", 2.05, 2.6)):
        for i, y in enumerate((12.2, 12.78, 13.36)):
            dz = 0.03 * i
            lo, hi = (z0 - dz, z1) if tag == "f" else (z0, z1 + dz)
            g.box(b, f"rope_{tag}{i}", (-4.9, y, lo), (4.9, y + 0.55, hi), "cord" if i % 2 == 0 else "cord_dk")
    for tag, z0, z1 in (("f", -2.85, -2.0), ("b", 2.0, 2.85)):
        g.pair(b, f"clamp_{tag}", (2.2, 12.1, z0), (2.8, 13.95, z1), "leather_dk")
    g.box(b, "buckle_top", (-0.9, 13.4, -3.0), (0.9, 13.8, -2.6), "iron")
    g.box(b, "buckle_bot", (-0.9, 12.3, -3.0), (0.9, 12.7, -2.6), "iron")
    g.box(b, "buckle_l", (-0.9, 12.7, -3.0), (-0.5, 13.4, -2.6), "iron")
    g.box(b, "buckle_r", (0.5, 12.7, -3.0), (0.9, 13.4, -2.6), "iron")
    g.box(b, "buckle_tongue", (-0.14, 12.5, -3.25), (0.14, 13.6, -2.95), "rust")
    for k, x in enumerate((1.3, 3.4)):
        g.pair(b, f"rivet_{k}", (x, 12.7, -3.1), (x + 0.4, 13.3, -2.7), "iron")
    g.box(b, "back_knot", (-0.6, 12.2, 2.6), (0.6, 13.9, 3.15), "cord_dk")
    g.pair(b, "back_tail", (0.2, 10.8, 2.6), (0.55, 12.3, 2.95), "cord")


def part_loop(g: Gear) -> None:
    """挂绳环：左前胯的绳结，两股垂绳穿过一只铁环，环下接包顶（整件在前面 z<-2.05，躲开手臂盒）。"""
    b = "hang_loop"
    g.box(b, "knot", (-5.35, 11.9, -2.95), (-4.45, 13.9, -2.05), "cord_dk")
    g.box(b, "cord_a", (-5.2, 10.7, -2.85), (-4.8, 12.1, -2.1), "cord")
    g.box(b, "cord_b", (-4.8, 10.8, -2.82), (-4.45, 12.0, -2.12), "cord_dk")
    g.ring(b, "ring", (-4.95, 10.6, -2.45), 0.65, 0.5, 0.5, "iron", axis="x", n=8)
    g.box(b, "ring_rust", (-5.3, 10.0, -2.8), (-4.55, 10.45, -2.2), "rust")
    g.box(b, "tie_a", (-5.9, 10.2, -2.55), (-4.5, 10.7, -2.0), "cord")
    g.box(b, "tie_b", (-6.4, 9.7, -2.4), (-5.8, 10.3, -1.85), "cord_dk")
    for k, (dx, dy) in enumerate(((-5.15, 12.8), (-4.85, 12.6), (-4.6, 12.85))):
        g.box(b, f"frayed_{k}", (dx - 0.12, dy, -3.15), (dx + 0.12, dy + 0.6, -2.7), "cord")


# ───────────────────────────────────────────────────────────── 分部件：布卷
def part_core(g: Gear) -> None:
    """布卷：四层八棱柱（每层五块拼出八边），逐层半径略有出入，读作卷得松紧不一。"""
    b = "bundle_core"
    radii = ((1.75, 1.75), (1.9, 1.9), (1.85, 1.85), (1.7, 1.7))
    mats = ("cloth", "cloth_lt", "cloth", "cloth_lt")
    for i, ((y0, y1), (rx, rz), mat) in enumerate(zip(BANDS, radii, mats)):
        _octagon(g, b, f"layer{i}", y0, y1, rx, rz, mat)


def part_bands(g: Gear) -> None:
    """布面横箍 + 潮渍 + 卷边褶：三道横向暗箍环绕（八边外沿一圈小块）、两块潮渍、顶边卷起的褶。"""
    b = "bundle_bands"
    for k, y in enumerate((5.4, 7.6, 9.3)):
        w = 1.95 + 0.02 * k
        g.box(b, f"band_f{k}", (CX - w + 0.5, y, CZ - 1.9 - 0.24), (CX + w - 0.5, y + 0.4, CZ - 1.9 + 0.08), "cloth_dk")
        g.box(b, f"band_b{k}", (CX - w + 0.5, y + 0.02, CZ + 1.9 - 0.08), (CX + w - 0.5, y + 0.42, CZ + 1.9 + 0.24), "cloth_dk")
        g.box(b, f"band_o{k}", (CX - 1.9 - 0.24, y, CZ - w + 0.5), (CX - 1.9 + 0.08, y + 0.4, CZ + w - 0.5), "cloth_dk")
        g.box(b, f"band_i{k}", (CX + 1.9 - 0.08, y + 0.02, CZ - w + 0.5), (CX + 1.9 + 0.2, y + 0.42, CZ + w - 0.5), "cloth_dk")
    g.box(b, "stain_a", (CX - 1.2, 5.0, CZ - 1.9 - 0.2), (CX + 0.2, 6.2, CZ - 1.9 + 0.06), "cloth_stain")
    g.box(b, "stain_b", (CX - 0.8, 8.4, CZ + 1.9 - 0.06), (CX + 1.1, 9.4, CZ + 1.9 + 0.22), "cloth_stain")
    for k, (dx, dz) in enumerate(((-1.3, -1.3), (1.3, -1.3), (1.3, 1.3), (-1.3, 1.3))):
        g.box(b, f"roll_fold_{k}", (CX + dx - 0.45, 10.9, CZ + dz - 0.45), (CX + dx + 0.45, 11.8, CZ + dz + 0.45), "cloth_lt")


def part_tears(g: Gear) -> None:
    """底边撕烂的破布条：绕八边底沿十二条长短不一的布条垂下，厚 0.3，少数卷边。"""
    b = "bundle_tears"
    lengths = (1.3, 0.7, 1.8, 1.0, 1.5, 0.6, 1.9, 0.9, 1.4, 1.1, 1.7, 0.8)
    for i in range(12):
        a = i * math.tau / 12 + 0.12
        r = 1.65
        x = CX + math.cos(a) * r * 1.0
        z = CZ + math.sin(a) * r * 1.0
        ln = lengths[i]
        w = 0.8 + 0.08 * (i % 3)
        if abs(math.cos(a)) >= abs(math.sin(a)):
            g.box(b, f"tear_{i}", (x - 0.2, Y_BOT - ln, z - w / 2), (x + 0.2 + 0.01 * i, Y_BOT + 0.35, z + w / 2),
                  "cloth" if i % 2 else "cloth_stain")
        else:
            g.box(b, f"tear_{i}", (x - w / 2, Y_BOT - ln, z - 0.2), (x + w / 2, Y_BOT + 0.35, z + 0.2 + 0.01 * i),
                  "cloth" if i % 2 else "cloth_stain")


def part_patch(g: Gear) -> None:
    """皮补丁：布卷外侧（-x）一块皮补，四边针脚，右上角翘起。"""
    b = "bundle_patch"
    xo = CX - 1.9
    z0, z1, y0, y1 = CZ - 1.3, CZ + 0.2, 4.7, 6.5
    g.box(b, "patch", (xo - 0.3, y0, z0), (xo - 0.04, y1, z1), "leather")
    g.box(b, "patch_curl", (xo - 0.5, y1 - 0.5, z1 - 0.5), (xo - 0.04, y1 + 0.2, z1 + 0.15), "leather_dk")
    for k in range(3):
        z = z0 + 0.1 + k * 0.5
        g.box(b, f"st_t{k}", (xo - 0.46, y1 - 0.24, z), (xo - 0.08, y1 + 0.14, z + 0.3), "stitch")
        g.box(b, f"st_b{k}", (xo - 0.46, y0 - 0.12, z), (xo - 0.08, y0 + 0.26, z + 0.3), "stitch")
    for k in range(3):
        y = y0 + 0.2 + k * 0.5
        g.box(b, f"st_f{k}", (xo - 0.46, y, z0 - 0.12), (xo - 0.08, y + 0.3, z0 + 0.26), "stitch")
        g.box(b, f"st_r{k}", (xo - 0.46, y, z1 - 0.34), (xo - 0.08, y + 0.3, z1 + 0.08), "stitch")


def part_plates(g: Gear) -> None:
    """外侧两块锈铁片：各用两枚铆钉钉在布上，边上一道锈斑。"""
    b = "bundle_plates"
    xo = CX - 1.9
    for n, (y0, y1, z0, z1) in enumerate(((8.2, 9.4, CZ - 0.2, CZ + 1.2), (6.7, 7.8, CZ - 1.5, CZ - 0.35))):
        g.box(b, f"plate{n}", (xo - 0.32, y0, z0), (xo - 0.02, y1, z1), "iron")
        g.box(b, f"plate{n}_rust", (xo - 0.54, y0 + 0.1, z0 + 0.1), (xo - 0.28, y0 + 0.5, z1 - 0.1), "rust")
        for k, (ry, rz) in enumerate(((y1 - 0.3, z0 + 0.25), (y0 + 0.28, z1 - 0.25))):
            g.box(b, f"plate{n}_rivet{k}", (xo - 0.5, ry - 0.17, rz - 0.17), (xo - 0.3, ry + 0.17, rz + 0.17), "rust")


def part_splint(g: Gear) -> None:
    """骨夹板：贴在布卷前面（-z）外的一根扁骨，两端削尖，两道草绳绑扎；骨面一道裂纹。"""
    b = "bundle_splint"
    zo = CZ - 1.9 - 0.45
    g.box(b, "splint", (CX - 0.5, 5.0, zo - 0.4), (CX + 0.5, 10.6, zo), "bone")
    g.box(b, "splint_shade", (CX + 0.35, 5.1, zo - 0.36), (CX + 0.58, 10.5, zo + 0.01), "bone_dk")
    g.box(b, "splint_tip_t", (CX - 0.3, 10.6, zo - 0.3), (CX + 0.3, 11.1, zo - 0.04), "bone")
    g.box(b, "splint_tip_b", (CX - 0.3, 4.55, zo - 0.3), (CX + 0.3, 5.0, zo - 0.04), "bone_dk")
    g.box(b, "crack", (CX - 0.12, 6.0, zo - 0.45), (CX + 0.1, 8.0, zo - 0.2), "stitch")
    for k, y in enumerate((6.0, 8.8)):
        g.box(b, f"lash_{k}", (CX - 0.68, y, zo - 0.55), (CX + 0.68, y + 0.45, zo + 0.1), "cord")
        g.box(b, f"lash_knot_{k}", (CX - 0.25, y - 0.1, zo - 0.78), (CX + 0.3, y + 0.55, zo - 0.5), "cord_dk")


def _rope(g: Gear, bone: str, name: str, a, b, zdir_axis, mat: str) -> None:
    """一段斜绳（扁带，薄的一维朝 up）。"""
    frm, to, rot, org = shaft_box(a, b, 0.32, 0.17, up=zdir_axis)
    g.box(bone, name, frm, to, mat, rot=rot, org=org)


def part_lash(g: Gear) -> None:
    """斜捆草绳：外侧（-x）交叉两道、前面（-z）交叉两道，十字处各打一个结。"""
    b = "lash_ropes"
    xo = CX - 1.9 - 0.4
    _rope(g, b, "out_a0", (xo, 10.7, CZ - 1.5), (xo, 8.7, CZ), (-1.0, 0.0, 0.0), "cord")
    _rope(g, b, "out_a1", (xo, 8.7, CZ), (xo, 6.9, CZ + 1.5), (-1.0, 0.0, 0.0), "cord")
    _rope(g, b, "out_b0", (xo - 0.02, 10.4, CZ + 1.5), (xo - 0.02, 8.6, CZ), (-1.0, 0.0, 0.0), "cord_dk")
    _rope(g, b, "out_b1", (xo - 0.02, 8.6, CZ), (xo - 0.02, 6.5, CZ - 1.5), (-1.0, 0.0, 0.0), "cord_dk")
    g.box(b, "out_knot", (xo - 0.3, 8.1, CZ - 0.5), (xo + 0.15, 9.2, CZ + 0.5), "cord_dk")
    zo = CZ + 1.9 + 0.42                           # 背面：交叉绳在后面，前面让给骨夹板
    _rope(g, b, "back_a0", (CX - 1.5, 10.5, zo), (CX, 8.5, zo), (0.0, 0.0, 1.0), "cord")
    _rope(g, b, "back_a1", (CX, 8.5, zo), (CX + 1.4, 6.7, zo), (0.0, 0.0, 1.0), "cord")
    _rope(g, b, "back_b0", (CX + 1.5, 10.3, zo + 0.02), (CX, 8.4, zo + 0.02), (0.0, 0.0, 1.0), "cord_dk")
    _rope(g, b, "back_b1", (CX, 8.4, zo + 0.02), (CX - 1.4, 6.5, zo + 0.02), (0.0, 0.0, 1.0), "cord_dk")
    g.box(b, "back_knot", (CX - 0.5, 7.9, zo - 0.05), (CX + 0.5, 9.0, zo + 0.4), "cord_dk")


# ───────────────────────────────────────────────────────────── 分部件：苔
def _moss_mat(rng: random.Random) -> str:
    r = rng.random()
    if r < 0.42:
        return "moss"
    if r < 0.72:
        return "moss_dk"
    if r < 0.9:
        return "moss_lt"
    return "moss_ash"


def part_moss_cap(g: Gear) -> None:
    """苔顶：包顶堆起的一团苔，七块高低错落的方苔，最高 ≤11.9（手臂盒之下）。"""
    b = "moss_cap"
    rng = random.Random(0x6A31)
    spots = ((-0.9, -0.9, 0.6), (0.9, -0.8, 0.5), (1.0, 0.9, 0.7), (-0.9, 1.0, 0.45), (0.0, 0.1, 0.65), (-1.6, 0.1, 0.35), (1.55, -0.1, 0.4))
    for k, (dx, dz, h) in enumerate(spots):
        g.box(b, f"cap_{k}", (CX + dx - 0.55, 11.15, CZ + dz - 0.55), (CX + dx + 0.55, 11.15 + h + 0.15, CZ + dz + 0.55), _moss_mat(rng))


def part_moss_shell(g: Gear) -> None:
    """苔壳：布卷上半截三层，每层绕一圈十二块苔（前面正中留一道缝露布），厚薄高低不一；内缘 x≥-4.05 不进左腿盒。"""
    b = "moss_shell"
    rng = random.Random(0x9C42)
    layers = ((9.9, 11.15), (8.7, 10.0), (7.5, 8.8))
    for li, (y0, y1) in enumerate(layers):
        for i in range(12):
            a = i * math.tau / 12 + li * 0.2
            if li == 2 and math.sin(a) < -0.85:          # 最低一层前面留缝
                continue
            r = 2.0 + 0.1 * rng.random()
            x = CX + math.cos(a) * r
            z = CZ + math.sin(a) * r
            x = min(x, -4.05 - 0.55)                      # 内缘：苔块外沿不越过 -4.05
            h = (y1 - y0) + rng.random() * 0.3
            g.box(b, f"shell_{li}_{i}", (x - 0.62, y0 - rng.random() * 0.25, z - 0.6),
                  (x + 0.62, y0 + h, z + 0.6), _moss_mat(rng))


def part_moss_strands(g: Gear) -> None:
    """苔须：从苔壳下沿垂下长短不一的细条，二十四根，各带一小截更细的末梢。"""
    b = "moss_strands"
    rng = random.Random(0x2B77)
    for i in range(24):
        a = i * math.tau / 24 + rng.random() * 0.1
        r = 2.1 + 0.2 * rng.random()
        x = min(CX + math.cos(a) * r, -4.05 - 0.3)
        z = CZ + math.sin(a) * r
        ln = 1.0 + rng.random() * 2.6
        top = 7.7
        w = 0.4 + 0.12 * (i % 3)
        mat = "moss" if i % 3 else "moss_dk"
        g.box(b, f"strand_{i}", (x - w / 2, top - ln, z - w / 2), (x + w / 2, top, z + w / 2), mat)
        g.box(b, f"strand_tip_{i}", (x - w / 2 + 0.07, top - ln - 0.5, z - w / 2 + 0.07), (x + w / 2 - 0.07, top - ln + 0.05, z + w / 2 - 0.07),
              "moss_lt" if i % 4 == 0 else mat)


def deconflict(g: Gear) -> None:
    """把同向共面的面错开（Z-fighting 门禁）：只动无旋转件，把后加件的冲突面向外长 0.002–0.01px（肉眼不可见）。

    细部件（苔块 / 针脚 / 横箍）密密麻麻贴在大面上，手写每一处错位既烦又易漏；这里统一处理，
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


class MoistureGuardSpec(GearSpec):
    def build(self, parts=None) -> Gear:
        g = super().build(parts)
        deconflict(g)
        return g


PART_FUNCS = {
    "belt": part_belt,
    "loop": part_loop,
    "core": part_core,
    "bands": part_bands,
    "tears": part_tears,
    "patch": part_patch,
    "plates": part_plates,
    "splint": part_splint,
    "lash": part_lash,
    "moss_cap": part_moss_cap,
    "moss_shell": part_moss_shell,
    "moss_strands": part_moss_strands,
}

SPEC = MoistureGuardSpec(
    gear_id="moisture_guard",
    model_name="MoistureGuard",
    title="防潮包 / moisture_guard",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：左胯外侧（布卷 x∈[-8.3,-4.5]、y<12）+ 腰前后三股草绳 + 左前胯绳圈铁环",
    worn_focus=((-3.0, 9.0, 0.0), 24.0),
    texture_overrides={"cloth_lt": "gear_linen_lt", "cloth_stain": "gear_linen_brn"},
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
