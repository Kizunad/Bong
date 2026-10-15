#!/usr/bin/env python3
"""锈框背架（xiu_kuang_bei_jia）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：末土装备 #7（.task-wasteland-gear.md）。穿戴约定见 modelScript/gear/_kit/gear_kit.py：
脚底 y=0，玩家面朝 -z、+z 是背面，躯干 x∈[-4,4] y∈[12,24] z∈[-2,2]，+x = 穿戴者右侧。

外形：锈铁条拼成的梯形背架，贴在背后（立骨 x=±4.1..5.5、z∈[2.6,4.2]，三道横档用角铁片和铆钉连起来），
底下是一块向后伸出的板条托架，顶上两根铁臂向后探出、托着一卷横放的兽皮铺盖（头在铺盖前面，铺盖整体在
z≥4.3，不挡脸）；右侧挂着一只用草绳网兜住的裂陶罐，左侧挂一圈麻绳和一块破布，背面靠肩带 + 胸带 + 腰带
绑在人身上（挂具走 _kit/harness.py，骑过手臂上沿，不进躯干）。

分部件：part_*() 逐件写，--part 单件预览；骨骼树在 BONES 里一处定义，按挂点分骨。
用法：
    python3 modelScript/gear/xiu_kuang_bei_jia/gen_xiu_kuang_bei_jia.py --self-test   # 门禁差分自证
    python3 modelScript/gear/xiu_kuang_bei_jia/gen_xiu_kuang_bei_jia.py --part jar    # 单件预览
    python3 modelScript/gear/xiu_kuang_bei_jia/gen_xiu_kuang_bei_jia.py --worn        # 只出穿戴三面
    python3 modelScript/gear/xiu_kuang_bei_jia/gen_xiu_kuang_bei_jia.py               # 全量：模型 + check.png
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
    "iron":       (92, 84, 80),
    "iron_lt":    (128, 120, 114),   # 角铁片 / 磨亮的棱
    "iron_dk":    (60, 56, 54),
    "rust":       (150, 84, 48),
    "rust_lt":    (186, 112, 62),    # 锈蚀的亮橙
    "rust_dk":    (104, 56, 34),
    "hide":       (118, 78, 56),
    "hide_dk":    (84, 56, 42),
    "hide_lt":    (150, 108, 78),
    "hide_pale":  (176, 150, 112),   # 铺盖卷的浅色皮
    "hide_stain": (92, 70, 52),      # 铺盖卷上的污渍 / 深色补块
    "cord":       (150, 126, 84),
    "cord_dk":    (104, 84, 54),
    "clay":       (150, 96, 70),     # 陶罐
    "clay_dk":    (104, 66, 48),
    "clay_lt":    (178, 126, 92),
    "cloth":      (146, 126, 96),    # 破布
    "cloth_dk":   (110, 94, 70),
    "bone_lt":    (224, 214, 186),
    "bone_stain": (126, 106, 78),
    "void":       (28, 24, 22),
}
SOFT = ("cord", "cord_dk", "hide", "hide_dk", "hide_lt", "hide_pale", "hide_stain", "cloth", "cloth_dk")
SOFT_OVER = soft_pairs(SOFT, tuple(MATS)) | soft_pairs(("void",), ("clay", "clay_dk", "hide", "hide_dk", "iron", "iron_dk"))

HARNESS = harness.HarnessMats(
    strap="hide_dk", strap_dk="hide_stain", pad="hide", stitch="cord_dk", hard="iron", hard_pin="rust",
    hard_rust="rust_dk", toggle="bone_lt", toggle_pin="bone_stain", hole="void")

# ───────────────────────────────────────────────────────────── 骨骼树（按挂点分骨）
BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("frame_main", "root", (0.0, 17.0, 3.4), "梯形背架：两根立骨 / 三道横档 / 角铁片 / 铆钉"),
    ("frame_shelf", "frame_main", (0.0, 11.4, 6.0), "底托架：前后边框 + 一排板条"),
    ("frame_arms", "frame_main", (0.0, 23.2, 6.0), "顶上向后探的两根托臂 + 斜撑"),
    ("bedroll", "frame_main", (0.0, 26.3, 7.0), "横放的兽皮铺盖卷 + 两端卷纹"),
    ("bedroll_ties", "bedroll", (0.0, 26.3, 7.0), "捆铺盖的两道草绳 + 绳结"),
    ("bedroll_patch", "bedroll", (0.0, 26.3, 7.0), "铺盖卷上的污渍 / 补块（左右不对称）"),
    ("jar", "frame_main", (7.3, 16.0, 4.8), "右侧挂的裂陶罐"),
    ("jar_net", "jar", (7.3, 16.0, 4.8), "罐外的草绳网兜"),
    ("rope_coil", "frame_main", (-6.9, 15.2, 5.2), "左侧挂的一圈麻绳 + 捆扎"),
    ("flap_l", "frame_main", (-5.8, 23.0, 4.4), "左立骨外侧垂下的破布（会晃）"),
    ("flap_r", "frame_main", (5.8, 23.0, 4.4), "右托臂旁一小条破布"),
    *harness.HARNESS_BONES,
]
ASYM_BONES = frozenset({"bedroll_patch", "jar", "jar_net", "rope_coil", "flap_l", "flap_r"}) | harness.HARNESS_ASYM

# ───────────────────────────────────────────────────────────── 常量
UX = (4.1, 5.5)                  # 立骨 x 范围（右）
FZ = (2.6, 4.2)                  # 背架平面 z 范围（立骨 / 横档）
Y_BOT, Y_TOP = 10.9, 23.9        # 立骨下端 / 上端
CROSS_Y = (12.2, 16.2, 20.2)     # 三道横档下沿（高 1.4）
ROLL_X, ROLL_Y, ROLL_Z = 6.2, (23.9, 28.7), (4.3, 9.7)    # 铺盖卷半长 / y 范围 / z 范围


# ───────────────────────────────────────────────────────────── 分部件：背架主体
def part_frame(g: Gear) -> None:
    """梯形背架：两根立骨（分段，铁 / 锈交替）+ 三道横档 + 底前梁 + 角铁片 + 铆钉。"""
    b = "frame_main"
    segs = ((Y_BOT, 15.4, "iron"), (15.4, 20.2, "rust"), (20.2, Y_TOP, "iron_lt"))
    for k, (y0, y1, mat) in enumerate(segs):
        g.pair(b, f"upright_{k}", (UX[0], y0, FZ[0]), (UX[1], y1, FZ[1]), mat)
    # 立骨背面一道凸起的暗槽（角钢的筋），在铁片之间断开
    for k, (y0, y1) in enumerate(((14.1, 15.7), (18.1, 19.7))):
        g.pair(b, f"upright_rib_{k}", (4.5, y0, FZ[1] - 0.1), (5.1, y1, FZ[1] + 0.2), "iron_dk")
    # 三道横档 + 底前梁
    mats = ("iron", "rust", "iron_lt")
    for k, y0 in enumerate(CROSS_Y):
        g.box(b, f"cross_{k}", (-UX[0], y0, FZ[0]), (UX[0], y0 + 1.4, FZ[1]), mats[k])
        g.box(b, f"cross_{k}_rib", (-3.5, y0 + 0.4, FZ[1] - 0.1), (3.5, y0 + 1.0, FZ[1] + 0.2), "iron_dk")
    g.box(b, "base_beam", (-UX[0], Y_BOT, FZ[0]), (UX[0], 12.0, FZ[1]), "iron_dk")
    # 角铁片：横档与立骨相交处一块方铁，四角各一颗铆钉；锈迹斑一块
    for k, y0 in enumerate(CROSS_Y):
        yc = y0 + 0.7
        g.pair(b, f"gusset_{k}", (3.7, yc - 1.1, FZ[1] - 0.1), (5.9, yc + 1.1, FZ[1] + 0.35), "iron_lt")
        for j, (dx, dy) in enumerate(((4.0, -0.85), (5.2, -0.85), (4.0, 0.45), (5.2, 0.45))):
            g.pair(b, f"gusset_{k}_rivet_{j}", (dx, yc + dy, FZ[1] + 0.35), (dx + 0.5, yc + dy + 0.4, FZ[1] + 0.65), "rust_dk")
        g.pair(b, f"gusset_{k}_rust", (4.3, yc - 0.6, FZ[1] + 0.25), (4.9, yc + 0.1, FZ[1] + 0.5), "rust_lt")
    # 锈蚀：立骨外面几块凸起的锈斑（让开挂罐的 y13–18），横档背面小锈疙瘩（让开暗槽）
    g.pair(b, "rust_blotch_out", (UX[1], 11.1, 2.9), (UX[1] + 0.25, 12.0, 3.7), "rust_lt")
    for k, (y, h, mat) in enumerate(((14.0, 1.0, "rust_lt"), (18.0, 1.2, "rust_dk"), (22.0, 1.2, "rust_lt"))):    # 立骨内侧（两条之间）
        g.pair(b, f"rust_blotch_in_{k}", (UX[0] - 0.25, y, 2.8 + 0.3 * (k % 2)), (UX[0], y + h, 3.8 + 0.3 * (k % 2)), mat)
    for k, y0 in enumerate(CROSS_Y):
        for j, (x, mat) in enumerate(((0.8, "rust_lt"), (2.2, "rust_dk"), (3.1, "rust_lt"))):
            g.pair(b, f"cross_{k}_speck_{j}", (x, y0 + 1.0, FZ[1] - 0.1), (x + 0.6, y0 + 1.3, FZ[1] + 0.2), mat)
    # 立骨 / 底梁下端的四只垫脚：放地上时架子不贴地蹭
    g.pair(b, "foot_front", (UX[0], Y_BOT - 0.5, 3.0), (UX[1], Y_BOT, 4.0), "iron_dk")
    # 立骨上端的封头板（x 从 4.05 起：头在 |x|<4 的范围里）
    g.pair(b, "cap", (4.05, Y_TOP, 2.55), (UX[1] + 0.05, Y_TOP + 0.35, FZ[1] + 0.05), "iron_dk")
    # 立骨前面（贴背那一侧）的锈斑条：从背上看不见，侧视才露出来
    g.pair(b, "front_rust", (UX[0] + 0.3, 13.0, FZ[0] - 0.2), (UX[1] - 0.3, 18.0, FZ[0] + 0.05), "rust_dk")


def part_shelf(g: Gear) -> None:
    """底托架：左右边梁（沿 z）+ 后梁 + 一排横板条（铁 / 锈交替），板条间留缝。"""
    b = "frame_shelf"
    g.pair(b, "rail", (UX[0], Y_BOT, FZ[1]), (UX[1], 12.0, 9.3), "iron")
    g.pair(b, "rail_rust", (UX[0] + 0.3, 12.0, 5.0), (UX[1] - 0.3, 12.25, 8.4), "rust_dk")
    g.box(b, "back_rail", (-UX[1], Y_BOT, 9.3), (UX[1], 12.0, 10.0), "iron_dk")
    slats = ((4.4, 5.2), (5.45, 6.25), (6.5, 7.3), (7.55, 8.35), (8.6, 9.3))
    for k, (z0, z1) in enumerate(slats):
        g.box(b, f"slat_{k}", (-UX[0], 11.0, z0), (UX[0], 11.9, z1), "rust" if k % 2 else "iron")
        g.pair(b, f"slat_{k}_nail", (3.2, 11.9, z0 + 0.15), (3.7, 12.2, z0 + 0.65), "rust_dk")
    g.pair(b, "rail_corner", (UX[0] - 0.1, 12.0, 9.3), (UX[1] + 0.1, 12.5, 10.0), "iron_lt")
    g.pair(b, "foot_back", (UX[0], Y_BOT - 0.5, 9.0), (UX[1], Y_BOT, 10.0), "iron_dk")


def part_arms(g: Gear) -> None:
    """顶上两根托臂（从立骨上端向后探）+ 臂下斜撑 + 臂端挡板，承着铺盖卷。"""
    b = "frame_arms"
    g.pair(b, "arm", (UX[0], 22.5, FZ[1]), (UX[1], Y_TOP, 9.0), "iron")
    g.pair(b, "arm_end", (UX[0] - 0.1, 22.4, 9.0), (UX[1] + 0.1, Y_TOP - 0.1, 9.5), "iron_dk")
    for sx, tag in ((1, "r"), (-1, "l")):
        g.rod(b, f"brace_{tag}", (sx * 4.8, 19.4, 4.4), (sx * 4.8, 22.5, 8.4), 0.4, "iron_dk")


# ───────────────────────────────────────────────────────────── 分部件：铺盖卷
def part_bedroll(g: Gear) -> None:
    """横放的兽皮铺盖卷：皮身 + 两端层层卷进去的螺旋纹（逐层缩进、颜色深浅交替）。"""
    b = "bedroll"
    (y0, y1), (z0, z1) = ROLL_Y, ROLL_Z
    g.box(b, "roll_body", (-ROLL_X, y0, z0), (ROLL_X, y1, z1), "hide_pale")
    g.box(b, "roll_top_dk", (-5.4, y1, z0 + 0.8), (5.4, y1 + 0.25, z1 - 0.8), "hide_lt")
    # 端面：一块底板 + 两圈逐层凸出的框（外圈浅、内圈深）+ 中心暗洞，读成卷进去的螺旋
    ring_mats = ("hide_lt", "hide_stain")
    g.pair(b, "cap_plate", (ROLL_X, y0 + 0.2, z0 + 0.2), (ROLL_X + 0.25, y1 - 0.2, z1 - 0.2), "hide_dk")
    for r, inset in enumerate((0.5, 1.4)):
        xa = ROLL_X + 0.25 + r * 0.2
        xb = xa + 0.25
        ya, yb = y0 + inset, y1 - inset
        za, zb = z0 + inset, z1 - inset
        t = 0.5
        g.pair(b, f"cap_ring{r}_top", (xa, yb - t, za), (xb, yb, zb), ring_mats[r])
        g.pair(b, f"cap_ring{r}_bot", (xa, ya, za), (xb, ya + t, zb), ring_mats[r])
        g.pair(b, f"cap_ring{r}_front", (xa, ya + t, za), (xb, yb - t, za + t), ring_mats[r])
        g.pair(b, f"cap_ring{r}_back", (xa, ya + t, zb - t), (xb, yb - t, zb), ring_mats[r])
    g.pair(b, "cap_core", (ROLL_X + 0.2, 26.0, 6.5), (ROLL_X + 0.75, 26.6, 7.5), "void")


def part_ties(g: Gear) -> None:
    """铺盖卷的两道草绳：上 / 下 / 前 / 后四片围成一圈，加绳结和垂下的绳头。"""
    b = "bedroll_ties"
    (y0, y1), (z0, z1) = ROLL_Y, ROLL_Z
    # 内侧一道（x=±3.3，躲在两条托臂之间）和外侧一道（x=±5.85，托臂外面），各四片围成一圈
    for k, cx in enumerate((3.3, 5.85)):
        hw = 0.45 if k == 0 else 0.3
        mat_a, mat_b = ("cord", "cord_dk") if k == 0 else ("cord_dk", "cord")
        g.pair(b, f"tie{k}_top", (cx - hw, y1 - 0.1, z0 - 0.25), (cx + hw, y1 + 0.35, z1 + 0.35), mat_a)
        g.pair(b, f"tie{k}_bot", (cx - hw, y0 - 0.35, z0 - 0.25), (cx + hw, y0 + 0.1, z1 + 0.35), mat_b)
        g.pair(b, f"tie{k}_front", (cx - hw, y0 + 0.1, z0 - 0.25), (cx + hw, y1 - 0.1, z0 + 0.1), mat_a)
        g.pair(b, f"tie{k}_back", (cx - hw, y0 + 0.1, z1 - 0.1), (cx + hw, y1 - 0.1, z1 + 0.35), mat_b)
    cx = 3.3
    g.pair(b, "tie0_knot", (cx - 0.6, y1 + 0.35, 6.4), (cx + 0.6, y1 + 0.8, 7.6), "cord_dk")
    g.pair(b, "tie0_end_a", (cx - 0.3, y1 + 0.35, 7.6), (cx + 0.3, y1 + 0.6, 8.9), "cord")
    g.pair(b, "tie0_end_b", (cx - 0.3, y1 - 1.3, z1 + 0.35), (cx + 0.3, y1 - 0.3, z1 + 0.6), "cord")


def part_patch(g: Gear) -> None:
    """铺盖卷上的污渍与补块：几片深色皮补丁 + 一片磨亮的浅斑（只在单侧，不对称）。"""
    b = "bedroll_patch"
    (y0, y1), (z0, z1) = ROLL_Y, ROLL_Z
    top = ((-5.2, 5.0, 2.2, 3.0, "hide_stain"), (-1.4, 7.0, 3.0, 2.0, "hide_stain"), (1.0, 4.9, 2.0, 1.4, "hide_dk"),
           (-3.0, 5.2, 1.4, 1.0, "hide_lt"))
    for k, (x, z, w, d, mat) in enumerate(top):
        g.box(b, f"top_patch_{k}", (x, y1 + 0.05, z), (x + w, y1 + 0.3, z + d), mat)
    back = ((-5.8, 25.2, 2.4, 2.0, "hide_stain"), (1.6, 26.0, 3.0, 2.4, "hide_stain"), (4.2, 24.6, 1.4, 1.2, "hide_dk"),
            (-4.2, 27.3, 1.8, 1.0, "hide_lt"))
    for k, (x, y, w, h, mat) in enumerate(back):
        g.box(b, f"back_patch_{k}", (x, y, z1 + 0.05), (x + w, y + h, z1 + 0.3), mat)
    # 顶上一条皮带捆着铺盖（单道，不对称）：带身 + 铁扣 + 扣舌
    g.box(b, "strap_top", (-1.7, y1 + 0.3, z0 - 0.1), (-0.8, y1 + 0.55, z1 + 0.35), "hide_dk")
    g.box(b, "strap_back", (-1.7, y0 + 0.1, z1 + 0.05), (-0.8, y1 + 0.3, z1 + 0.3), "hide")
    g.box(b, "strap_buckle", (-2.0, y1 + 0.55, 6.2), (-0.5, y1 + 0.9, 7.8), "iron")
    g.box(b, "strap_buckle_hole", (-1.5, y1 + 0.9, 6.7), (-1.0, y1 + 1.15, 7.3), "void")
    g.box(b, "stitch_a", (-5.8, 26.6, z1 + 0.3), (-3.4, 26.85, z1 + 0.55), "cord_dk")
    g.box(b, "stitch_b", (1.6, 25.6, z1 + 0.3), (4.6, 25.85, z1 + 0.55), "cord_dk")


# ───────────────────────────────────────────────────────────── 分部件：罐 / 绳圈 / 破布
def part_jar(g: Gear) -> None:
    """右侧挂的裂陶罐：底盘 / 肚 / 肩 / 颈 / 唇沿 / 罐口暗洞 + 一道亮边和几条裂纹。"""
    b = "jar"
    g.box(b, "base", (6.0, 12.5, 3.4), (8.6, 13.2, 6.2), "clay_dk")
    g.box(b, "belly", (5.5, 13.2, 3.0), (9.1, 17.4, 6.6), "clay")
    g.box(b, "shoulder", (5.9, 17.4, 3.4), (8.7, 18.6, 6.2), "clay")
    g.box(b, "neck", (6.5, 18.6, 4.0), (8.1, 20.0, 5.6), "clay_dk")
    g.box(b, "lip", (6.2, 20.0, 3.8), (8.4, 20.6, 5.8), "clay")
    g.box(b, "mouth", (6.7, 20.6, 4.3), (7.9, 20.85, 5.3), "void")
    g.box(b, "highlight", (6.3, 17.5, 3.15), (6.9, 18.5, 3.4), "clay_lt")
    g.box(b, "shade", (5.5, 12.6, 3.6), (5.7, 13.2, 5.8), "clay_dk")
    # 裂纹：背面（+z）三条参差的竖缝，罐肚侧面一条
    for k, (x, y0, h) in enumerate(((6.2, 14.4, 1.6), (7.0, 15.6, 1.3), (7.9, 13.8, 1.1))):
        g.box(b, f"crack_{k}", (x, y0, 6.45), (x + 0.35, y0 + h, 6.75), "clay_dk")
    g.box(b, "chip", (8.1, 20.1, 5.4), (8.7, 20.5, 6.0), "clay_dk")


def part_net(g: Gear) -> None:
    """罐外的草绳网兜：横箍三道（侧面 + 后面）、竖绳两根、颈上一圈绑到立骨上的绳。"""
    b = "jar_net"
    for k, y in enumerate((13.7, 15.2, 16.7)):
        g.box(b, f"hoop_side_{k}", (9.05, y, 2.95), (9.3, y + 0.5, 6.65), "cord")
        g.box(b, f"hoop_back_{k}", (5.45, y - 0.05, 6.55), (9.35, y + 0.55, 6.8), "cord_dk")
    for k, z in enumerate((3.9, 5.5)):
        g.box(b, f"post_side_{k}", (9.2, 13.2, z), (9.45, 17.4, z + 0.5), "cord_dk")
    for k, x in enumerate((6.1, 7.7)):
        g.box(b, f"post_back_{k}", (x, 13.2, 6.7), (x + 0.5, 17.4, 6.95), "cord")
    g.box(b, "neck_tie", (6.2, 18.3, 3.7), (8.4, 18.8, 5.9), "cord_dk")
    g.box(b, "to_frame", (5.2, 18.4, 3.0), (6.6, 18.9, 3.5), "cord")
    g.box(b, "tie_end", (6.15, 17.6, 3.3), (6.7, 18.4, 3.9), "cord")


def part_coil(g: Gear) -> None:
    """左侧挂的麻绳圈：三圈叠在一起（yz 平面的环，沿 x 叠放）+ 一道捆扎绳。"""
    b = "rope_coil"
    for k in range(3):
        g.ring(b, f"coil_{k}", (-5.9 - 0.8 * k, 15.2, 5.2), 2.0, 0.7, 0.8, "cord" if k % 2 == 0 else "cord_dk", axis="x")
    g.box(b, "coil_hook", (-6.1, 17.1, 3.0), (-5.45, 17.7, 5.8), "iron_dk")        # 挂钩：贴在立骨外面，圈从上面挂过去
    g.box(b, "coil_hook_tip", (-6.2, 16.6, 5.2), (-5.4, 17.3, 5.85), "rust_dk")
    g.box(b, "coil_tie", (-8.4, 12.8, 4.8), (-5.4, 13.5, 5.6), "cord_dk")
    g.box(b, "coil_tail", (-8.4, 10.8, 5.0), (-7.8, 12.8, 5.5), "cord")


def part_flaps(g: Gear) -> None:
    """破布：左立骨外侧垂下的一块（参差下摆 + 补丁），右托臂旁一小条。"""
    b = "flap_l"
    g.box(b, "flap_main", (-6.3, 18.6, 2.9), (-5.5, 23.0, 5.7), "cloth")
    g.box(b, "flap_fold", (-6.4, 17.7, 3.2), (-5.6, 18.55, 5.1), "cloth_dk")
    for k, (z, h) in enumerate(((2.9, 1.4), (3.5, 2.2), (4.7, 1.0))):
        g.box(b, f"flap_tooth_{k}", (-6.3, 18.6 - h, z), (-5.5, 18.6, z + 0.55), "cloth" if k % 2 else "cloth_dk")
    g.box(b, "flap_patch", (-6.5, 20.4, 3.4), (-6.3, 22.0, 4.8), "hide_dk")
    g.box(b, "flap_stitch", (-6.75, 20.8, 3.6), (-6.5, 21.05, 4.6), "cord_dk")
    g.box(b, "flap_tie", (-6.5, 22.6, 2.8), (-5.4, 23.2, 3.5), "cord")
    b = "flap_r"
    g.box(b, "scrap", (5.5, 20.8, 3.0), (6.1, 22.4, 4.9), "cloth_dk")
    g.box(b, "scrap_tooth", (5.5, 20.0, 3.6), (6.1, 20.8, 4.3), "cloth")


PART_FUNCS = {
    "frame": part_frame,
    "shelf": part_shelf,
    "arms": part_arms,
    "bedroll": part_bedroll,
    "ties": part_ties,
    "patch": part_patch,
    "jar": part_jar,
    "net": part_net,
    "coil": part_coil,
    "flaps": part_flaps,
    **harness.harness_parts(HARNESS),
}

SPEC = GearSpec(
    gear_id="xiu_kuang_bei_jia",
    model_name="XiuKuangBeiJia",
    title="锈框背架 / xiu_kuang_bei_jia",
    mats=MATS,
    bones=BONES,
    parts=PART_FUNCS,
    out_dir=Path(__file__).resolve().parent,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    wear_note="挂点：背（背架 z∈[2.6,10]）+ 双肩带 + 腰前带",
)

if __name__ == "__main__":
    gear_kit.main(SPEC)
