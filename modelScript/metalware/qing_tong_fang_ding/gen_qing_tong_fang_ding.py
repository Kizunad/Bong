#!/usr/bin/env python3
"""青铜方鼎（qing_tong_fang_ding）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：旧时铁器 / 青铜器 #1（地上摆件）。凡物青铜器：做工精致规整，末法只带走了「灵气」这一层：
腹上的阵纹环（一圈细凹线）全暗，鼎里只剩一层冷灰，没有柴火。

比例以参考图（``qing_tong_fang_ding_ref.png``，正视 + 侧视）为准，不是任务卡的「约 18×20×18」：
整件高 20px，参考图各部件占总高：腿 38%、壁 32%、纹带联珠 6%、口沿唇 3%、立耳露出 22%；
口沿宽 18px，深 13.2px（参考图侧视深 / 宽 = 0.73，不是正方形）。腿是圆柱（八棱柱近似），不是方块。

坐标（px，1 方块 = 16px）：足底 y=0，足迹以原点为中心，正面朝 -z，+x = 玩家右侧；整件宽 18px，
超出一格方块 —— 摆件按 geo 坐标，不受一格限制（gate 的「越界」一道已去掉）。

用法：
    python3 modelScript/metalware/qing_tong_fang_ding/gen_qing_tong_fang_ding.py --table      # 比例表
    python3 modelScript/metalware/qing_tong_fang_ding/gen_qing_tong_fang_ding.py --self-test
    python3 modelScript/metalware/qing_tong_fang_ding/gen_qing_tong_fang_ding.py --gates
    python3 modelScript/metalware/qing_tong_fang_ding/gen_qing_tong_fang_ding.py             # 全量出模型 + check.png
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "metalware" / "_kit"))
import metal_review  # noqa: E402
from gear_kit import Gear, soft_pairs  # noqa: E402
from metal_review import MetalSpec  # noqa: E402

# ───────────────────────────────────────────────────────────── 材质
MATS = {
    "bz":        (150, 92, 58),     # 红褐铜底（器身主色）
    "bz_relief": (168, 112, 74),    # 浮雕纹带（比底色亮一档，锈会在凹处积）
    "bz_pat":    (84, 130, 110),    # 青绿锈
    "bz_worn":   (206, 152, 98),    # 手磨出的亮铜（立耳、足底、兽角）
    "bz_dk":     (76, 54, 42),      # 凹处 / 瞳孔 / 空匾暗铜
    "dead":      (54, 52, 50),      # 暗掉的阵纹凹线：更暗更灰，不发光
    "soot":      (34, 30, 28),      # 内壁火燎黑
    "ash":       (150, 148, 144),   # 冷灰
    "ash_dk":    (104, 102, 98),    # 炭灰深色垢层
    "void":      (18, 16, 14),
}
SOFT_OVER = soft_pairs(("ash", "ash_dk", "soot"), tuple(MATS))

# ───────────────────────────────────────────────────────────── 尺寸（px）
# 纵向分段，数值取自参考图正视各部件占总高的比例 × 20px（见模块说明）
LEG_TOP = 7.6           # 腿顶 = 器身底
WALL_TOP_LOW = 13.6     # 下壁顶（其上是阵纹环凹槽）
RING_TOP = 13.85        # 阵纹环顶
STRIP_BOT = RING_TOP    # 纹带底（回纹带 + 联珠都挂在上壁上）
BEAD_TOP = 14.95        # 联珠带顶 = 唇底
LIP_TOP = 15.6          # 口沿顶
HANDLE_BASE_TOP = 16.3
TOTAL_H = 20.0

HX, HZ = 8.35, 5.9      # 壁外表面半宽 / 半深（口沿唇外伸 0.65 / 0.7，所以口沿 18 × 13.2）
WALL_T = 0.6            # 壁厚
LEG_CX, LEG_CZ = 5.6, 4.0
LEG_AF = 3.0            # 腿八棱柱对边宽（参考图腿径约占口沿宽 15% = 2.7～3.0px）

# 腹纹面（前后壁 / 左右壁）的版式：v = 世界 y
CELL = 1.06             # 云雷纹单元边长
CELL_W = 0.28           # 纹线宽
RELIEF_D = 0.22         # 浅浮雕凸起高度（gate 的最小厚度 0.2，浮点误差要留一点余量）
V_FRAME_BOT = (8.15, 8.37)
V_ROW_BOT = (8.45, 8.45 + CELL)
V_TAOTIE = (9.65, 12.15)
V_ROW_TOP = (12.2, 12.2 + CELL)
V_FRAME_TOP = (13.32, 13.54)

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根，足底落在 y=0"),
    ("body", "root", (0.0, LEG_TOP, 0.0), "器身：四壁（中空）+ 底板 + 四角扉棱 + 下沿线脚"),
    ("panel_long", "body", (0.0, 11.0, -HZ), "前后壁腹纹：云雷纹带 + 兽面（左右对称）"),
    ("panel_short", "body", (HX, 11.0, 0.0), "左右壁腹纹：云雷纹带 + 正面兽面（左右成对）"),
    ("ring", "body", (0.0, 13.7, 0.0), "阵纹环：一圈暗掉的细凹线（灵气失效，不发光）"),
    ("rim", "body", (0.0, 14.5, 0.0), "口沿：回纹带 + 联珠 + 外折唇"),
    ("handle", "body", (0.0, 16.0, 0.0), "双立耳（左右成对，立耳面有浅浮雕）"),
    ("leg_f_r", "root", (LEG_CX, 3.8, -LEG_CZ), "右前腿：八棱柱 + 弦纹 + 兽面"),
    ("leg_f_l", "root", (-LEG_CX, 3.8, -LEG_CZ), "左前腿（镜像）"),
    ("leg_b_r", "root", (LEG_CX, 3.8, LEG_CZ), "右后腿"),
    ("leg_b_l", "root", (-LEG_CX, 3.8, LEG_CZ), "左后腿（镜像）"),
    ("interior", "body", (0.0, 8.2, 0.0), "内腔：壁上火燎黑 + 后壁一块空匾（铭文位，不写字）"),
    ("ash", "body", (0.0, 8.2, 0.0), "鼎底冷灰：不对称堆积（没有柴火）"),
]
ASYM_BONES = frozenset({"ash"})


# ───────────────────────────────────────────────────────────── 腹纹面辅助
class Panel:
    """壁上一个面的贴浮雕工具。face: 'f' 前壁(z=-HZ) / 'b' 后壁(z=+HZ)，u→+x；'s' 右壁(x=+HX)，u→+z。

    u 是面内水平坐标（以壁面中心为 0），v 是世界 y，d 是离壁外表面的高度（外为正）。
    f / b 面只写 u≥0 的右半，左半由 pair 镜像；跨中线（u0<0<u1）的件必须自身左右对称。
    's' 面写 x 的右壁，左壁由 pair 镜像；``mirror_u`` 再把件按 z 镜像一份（壁面前后对称）。
    """

    def __init__(self, g: Gear, bone: str, face: str) -> None:
        self.g, self.bone, self.face = g, bone, face

    def _span(self, u0, u1, v0, v1, d0, d1):
        if self.face == "f":
            return (u0, v0, -(HZ + d1)), (u1, v1, -(HZ + d0))
        if self.face == "b":
            return (u0, v0, HZ + d0), (u1, v1, HZ + d1)
        return (HX + d0, v0, u0), (HX + d1, v1, u1)

    def bar(self, name, u0, u1, v0, v1, d1, mat, d0=0.0, mirror_u=False) -> None:
        key = f"{self.face}_{name}"
        if self.face in ("f", "b"):
            if u0 < 0 < u1:
                f, t = self._span(u0, u1, v0, v1, d0, d1)
                self.g.box(self.bone, key, f, t, mat)
            else:
                f, t = self._span(u0, u1, v0, v1, d0, d1)
                self.g.pair(self.bone, key, f, t, mat)
            return
        f, t = self._span(u0, u1, v0, v1, d0, d1)
        self.g.pair(self.bone, key, f, t, mat)
        if mirror_u and not (u0 < 0 < u1):
            f, t = self._span(-u1, -u0, v0, v1, d0, d1)
            self.g.pair(self.bone, f"{key}z", f, t, mat)

    def cell(self, name, u0, v0, mirror_u=False) -> None:
        """一个云雷纹单元（回纹）：下横 + 右竖 + 上横 + 左半竖，左下留口，读作卷纹。"""
        s, w, d = CELL, CELL_W, RELIEF_D
        self.bar(f"{name}_bt", u0, u0 + s, v0, v0 + w, d, "bz_relief", mirror_u=mirror_u)
        self.bar(f"{name}_rt", u0 + s - w, u0 + s, v0 + w, v0 + s, d, "bz_relief", mirror_u=mirror_u)
        self.bar(f"{name}_tp", u0, u0 + s - w, v0 + s - w, v0 + s, d, "bz_relief", mirror_u=mirror_u)
        self.bar(f"{name}_lf", u0, u0 + w, v0 + 2 * w, v0 + s - w, d, "bz_relief", mirror_u=mirror_u)


def panel_frame(p: Panel, half_u: float) -> None:
    """腹纹面的边框：上下各一条横线 + 两侧竖线 + 中线竖棱。"""
    d = 0.22
    p.bar("frame_bot", -half_u, half_u, *V_FRAME_BOT, d, "bz_relief")
    p.bar("frame_top", -half_u, half_u, *V_FRAME_TOP, d, "bz_relief")
    p.bar("frame_edge", half_u - 0.25, half_u, V_FRAME_BOT[1], V_FRAME_TOP[0], d, "bz_relief", mirror_u=True)
    p.bar("rib", -0.25, 0.25, V_FRAME_BOT[1], V_FRAME_TOP[0], 0.28, "bz_relief")


def taotie_half(p: Panel, uc: float) -> None:
    """长壁每半幅一个兽面（参考图是两个相对的兽面侧脸，中间一道竖棱）：眼 + 瞳 + 眉 + 角 + 颊卷。"""
    v0, v1 = V_TAOTIE
    mid = (v0 + v1) / 2
    p.bar("eye", uc - 0.55, uc + 0.55, mid - 0.45, mid + 0.65, 0.3, "bz_relief")
    p.bar("pupil", uc - 0.2, uc + 0.2, mid - 0.1, mid + 0.3, 0.52, "bz_dk", d0=0.3)
    p.bar("brow", uc - 1.6, uc + 1.6, mid + 0.75, mid + 1.05, 0.3, "bz_relief")
    p.bar("brow_hook_o", uc + 1.3, uc + 1.6, mid + 0.25, mid + 0.75, 0.3, "bz_relief")
    p.bar("brow_hook_i", uc - 1.6, uc - 1.3, mid + 0.25, mid + 0.75, 0.3, "bz_relief")
    p.bar("horn_o", uc + 0.9, uc + 1.5, mid + 1.05, v1, 0.25, "bz_worn")
    p.bar("horn_i", uc - 1.5, uc - 0.9, mid + 1.05, v1, 0.25, "bz_worn")
    p.bar("cheek", uc - 1.4, uc + 1.4, v0, v0 + 0.3, 0.3, "bz_relief")
    p.bar("cheek_curl", uc + 1.1, uc + 1.4, v0 + 0.3, v0 + 0.85, 0.3, "bz_relief")


def taotie_front(p: Panel) -> None:
    """短壁（左右）正面兽面：中线对称，一对眼在中线两侧。u 向 +z 写一侧，mirror_u 补另一侧。"""
    v0, v1 = V_TAOTIE
    mid = (v0 + v1) / 2
    p.bar("eye", 0.55, 1.6, mid - 0.45, mid + 0.65, 0.3, "bz_relief", mirror_u=True)
    p.bar("pupil", 0.85, 1.3, mid - 0.1, mid + 0.3, 0.52, "bz_dk", d0=0.3, mirror_u=True)
    p.bar("brow", 0.28, 2.4, mid + 0.75, mid + 1.05, 0.3, "bz_relief", mirror_u=True)
    p.bar("horn", 1.6, 2.2, mid + 1.05, v1, 0.25, "bz_worn", mirror_u=True)
    p.bar("jaw", 0.28, 2.0, v0, v0 + 0.3, 0.3, "bz_relief", mirror_u=True)
    p.bar("cheek_curl", 1.7, 2.0, v0 + 0.3, v0 + 0.85, 0.3, "bz_relief", mirror_u=True)


# ───────────────────────────────────────────────────────────── 零件
def part_body(g: Gear) -> None:
    """器身：四壁中空（壁厚 0.6）+ 底板 + 下沿线脚 + 四角扉棱（牙脊）。"""
    b, t = "body", WALL_T
    # 下壁（y 7.6–13.6）：前后通长，左右夹在前后之间；壁在阵纹环处断开，环处单独见 part_ring
    g.box(b, "wall_lo_f", (-HX, LEG_TOP, -HZ), (HX, WALL_TOP_LOW, -(HZ - t)), "bz")
    g.box(b, "wall_lo_b", (-HX, LEG_TOP, HZ - t), (HX, WALL_TOP_LOW, HZ), "bz")
    g.pair(b, "wall_lo_s", (HX - t, LEG_TOP, -(HZ - t)), (HX, WALL_TOP_LOW, HZ - t), "bz")
    # 底板（腹底，y 7.6–8.2）
    g.box(b, "wall_floor", (-(HX - t), LEG_TOP, -(HZ - t)), (HX - t, 8.2, HZ - t), "bz_dk")
    # 下沿线脚：壁根外凸 0.25 的一圈（y 7.6–8.1）
    g.box(b, "wall_base_f", (-(HX + 0.25), LEG_TOP, -(HZ + 0.25)), (HX + 0.25, 8.1, -HZ), "bz_relief")
    g.box(b, "wall_base_b", (-(HX + 0.25), LEG_TOP, HZ), (HX + 0.25, 8.1, HZ + 0.25), "bz_relief")
    g.pair(b, "wall_base_s", (HX, LEG_TOP, -HZ), (HX + 0.25, 8.1, HZ), "bz_relief")

    # 四角扉棱：竖直棱芯 + 向外的牙（每角 3 对），y 8.1–13.5，让出阵纹环
    for fz, sz in (("f", -1), ("b", 1)):
        cx0, cx1 = HX - 0.8, HX + 0.25
        z0, z1 = (-(HZ + 0.25), -(HZ - 0.8)) if sz < 0 else (HZ - 0.8, HZ + 0.25)
        g.pair(b, f"flange_core_{fz}", (cx0, 8.1, z0), (cx1, 13.5, z1), "bz_pat")
        for k, y in enumerate((8.7, 10.5, 12.3)):
            # 牙：棱芯外再凸 0.2，每颗高 0.9，正面一排、侧面一排
            zt0, zt1 = (z0 - 0.22, z0) if sz < 0 else (z1, z1 + 0.22)
            g.pair(b, f"flange_tooth_{fz}{k}", (cx0 + 0.15, y, zt0), (cx1 - 0.15, y + 0.9, zt1), "bz_worn")
            g.pair(b, f"flange_tooth_s_{fz}{k}", (cx1, y, z0 + 0.15), (cx1 + 0.22, y + 0.9, z1 - 0.15), "bz_worn")


def part_ring(g: Gear) -> None:
    """阵纹环：一圈 0.25 高的细凹槽，槽底是暗灰的死纹（比周围更暗更灰，不发光）。凹深 0.2。"""
    b, t = "ring", WALL_T
    # 凹槽里的死纹填料：壁外表面往里缩 0.2（外面 0.2 是凹深），厚 0.4
    g.box(b, "ring_dead_f", (-HX, WALL_TOP_LOW, -(HZ - 0.2)), (HX, RING_TOP, -(HZ - t)), "dead")
    g.box(b, "ring_dead_b", (-HX, WALL_TOP_LOW, HZ - t), (HX, RING_TOP, HZ - 0.2), "dead")
    g.pair(b, "ring_dead_s", (HX - 0.2, WALL_TOP_LOW, -(HZ - t)), (HX - t, RING_TOP, HZ - t), "dead")


def part_panel_long(g: Gear) -> None:
    """前后壁腹纹：边框 + 中线棱 + 两个相对的兽面 + 上下各一排云雷纹带（每半幅 6 个单元）。"""
    b = "panel_long"
    half_u = 7.0
    for face in ("f", "b"):
        p = Panel(g, b, face)
        panel_frame(p, half_u + 0.25)
        taotie_half(p, 3.55)
        for k in range(6):
            u0 = 0.28 + CELL * k
            p.cell(f"lw_bot{k}", u0, V_ROW_BOT[0])
            p.cell(f"lw_top{k}", u0, V_ROW_TOP[0])


def part_panel_short(g: Gear) -> None:
    """左右壁腹纹：边框 + 正面兽面 + 上下各一排云雷纹带（每侧 4 个单元，按 z 对称）。"""
    b = "panel_short"
    p = Panel(g, b, "s")
    panel_frame(p, 4.77)
    taotie_front(p)
    for k in range(4):
        u0 = 0.28 + CELL * k
        p.cell(f"lw_bot{k}", u0, V_ROW_BOT[0], mirror_u=True)
        p.cell(f"lw_top{k}", u0, V_ROW_TOP[0], mirror_u=True)


def part_rim(g: Gear) -> None:
    """口沿：上壁（纹带底衬）+ 回纹方块带 + 联珠一圈 + 外折唇（唇比壁外伸 0.65 / 0.7）。"""
    b, t = "rim", WALL_T
    # 上壁衬（y 13.85–14.95），承回纹带与联珠
    g.box(b, "strip_back_f", (-HX, STRIP_BOT, -HZ), (HX, BEAD_TOP, -(HZ - t)), "bz")
    g.box(b, "strip_back_b", (-HX, STRIP_BOT, HZ - t), (HX, BEAD_TOP, HZ), "bz")
    g.pair(b, "strip_back_s", (HX - t, STRIP_BOT, -(HZ - t)), (HX, BEAD_TOP, HZ - t), "bz")
    # 回纹方块带（y 13.95–14.5）：一格实一格空的方块，沿四面
    for k in range(7):
        u0 = 0.3 + 1.1 * k
        g.pair("rim", f"strip_mea_f{k}", (u0, 13.95, -(HZ + 0.22)), (u0 + 0.55, 14.5, -HZ), "bz_relief")
        g.pair("rim", f"strip_mea_b{k}", (u0, 13.95, HZ), (u0 + 0.55, 14.5, HZ + 0.22), "bz_relief")
    for k in range(5):
        u0 = 0.3 + 1.1 * k
        g.pair("rim", f"strip_mea_sa{k}", (HX, 13.95, u0), (HX + 0.22, 14.5, u0 + 0.55), "bz_relief")
        g.pair("rim", f"strip_mea_sb{k}", (HX, 13.95, -(u0 + 0.55)), (HX + 0.22, 14.5, -u0), "bz_relief")
    # 联珠（y 14.5–14.95）：0.45 见方的圆珠近似
    for k in range(9):
        u0 = 0.0 + 0.9 * k + 0.0
        g.pair("rim", f"bead_f{k}", (u0 + 0.05, 14.5, -(HZ + 0.45)), (u0 + 0.5, BEAD_TOP, -HZ), "bz_worn")
        g.pair("rim", f"bead_b{k}", (u0 + 0.05, 14.5, HZ), (u0 + 0.5, BEAD_TOP, HZ + 0.45), "bz_worn")
    for k in range(6):
        z0 = 0.05 + 0.9 * k
        g.pair("rim", f"bead_sa{k}", (HX, 14.5, z0), (HX + 0.45, BEAD_TOP, z0 + 0.45), "bz_worn")
        g.pair("rim", f"bead_sb{k}", (HX, 14.5, -(z0 + 0.45)), (HX + 0.45, BEAD_TOP, -z0), "bz_worn")
    # 外折唇：四根条围出口，口内尺寸 = 壁内尺寸（壁厚 0.6）；外沿比壁外表面外伸 0.65 / 0.7
    lx, lz = HX + 0.65, HZ + 0.7
    g.box(b, "lip_f", (-lx, BEAD_TOP, -lz), (lx, LIP_TOP, -(HZ - t)), "bz")
    g.box(b, "lip_b", (-lx, BEAD_TOP, HZ - t), (lx, LIP_TOP, lz), "bz")
    g.pair(b, "lip_s", (HX - t, BEAD_TOP, -(HZ - t)), (lx, LIP_TOP, HZ - t), "bz")
    # 唇顶外缘一圈磨亮的窄棱（手摸得最多处）
    g.box(b, "lip_edge_f", (-lx, LIP_TOP, -lz), (lx, LIP_TOP + 0.22, -(lz - 0.25)), "bz_worn")
    g.box(b, "lip_edge_b", (-lx, LIP_TOP, lz - 0.25), (lx, LIP_TOP + 0.22, lz), "bz_worn")
    g.pair(b, "lip_edge_s", (lx - 0.25, LIP_TOP, -(lz - 0.25)), (lx, LIP_TOP + 0.22, lz - 0.25), "bz_worn")


def part_handle(g: Gear) -> None:
    """双立耳：口沿上的立耳（门形环：座 + 两竖 + 两级冠），耳外 / 内面各一对浅浮雕竖线。"""
    b = "handle"
    x0, x1 = 6.9, 8.5
    g.pair(b, "handle_base", (x0, LIP_TOP + 0.22, -1.9), (x1, HANDLE_BASE_TOP, 1.9), "bz")
    for sz, tag in ((1, "a"), (-1, "b")):
        z0, z1 = (0.95, 1.9) if sz > 0 else (-1.9, -0.95)
        g.pair(b, f"handle_up_{tag}", (x0, HANDLE_BASE_TOP, z0), (x1, 19.2, z1), "bz_worn")
        # 浅浮雕：耳面竖线（外面 + 内面）
        g.pair(b, f"handle_rel_o_{tag}", (x1, 16.6, z0 + 0.2), (x1 + 0.22, 18.9, z0 + 0.45), "bz_relief")
        g.pair(b, f"handle_rel_i_{tag}", (x0 - 0.22, 16.6, z0 + 0.2), (x0, 18.9, z0 + 0.45), "bz_relief")
    g.pair(b, "handle_beam", (x0, 19.2, -1.9), (x1, 19.6, 1.9), "bz_worn")
    g.pair(b, "handle_crown", (x0, 19.6, -1.35), (x1, TOTAL_H, 1.35), "bz_worn")


# ───────────────────────────────────────────────────────────── 腿
def octagon(g: Gear, bone: str, name: str, cx: float, cz: float, af: float, y0: float, y1: float, mat: str) -> None:
    """八棱柱（对边宽 af）：4 块 af × 0.414af 的条叠成正八边形；坐标按右腿（cx>0）写，左腿由 pair 镜像。

    两块 45° 斜条不进共面检查；两块正条的上下面错开 0.02px 免得共面闪烁。
    """
    h = af / 2
    e = af * 0.41421356 / 2
    org = (cx, (y0 + y1) / 2, cz)
    g.pair(bone, f"{name}_a", (cx - h, y0, cz - e), (cx + h, y1, cz + e), mat, sided=True)
    g.pair(bone, f"{name}_b", (cx - e, y0 + 0.02, cz - h), (cx + e, y1 - 0.02, cz + h), mat, sided=True)
    g.pair(bone, f"{name}_c", (cx - h, y0 + 0.01, cz - e), (cx + h, y1 - 0.01, cz + e), mat, rot=(0, 45, 0), org=org, sided=True)
    g.pair(bone, f"{name}_d", (cx - h, y0 + 0.01, cz - e), (cx + h, y1 - 0.01, cz + e), mat, rot=(0, -45, 0), org=org, sided=True)


# 兽面（腿顶 y 5.3–7.6）：(名, u0, u1, v0, v1, d0, d1, 材质, 是否按 u 镜像)；u 相对该面中线
LEG_FACE = (
    ("brow", -1.3, 1.3, 7.0, 7.3, 0.0, 0.3, "bz_relief", False),
    ("eye", 0.3, 1.2, 6.3, 6.95, 0.0, 0.25, "bz_relief", True),
    ("pupil", 0.6, 0.95, 6.45, 6.8, 0.25, 0.47, "bz_dk", True),
    ("horn", 1.0, 1.4, 7.3, 7.6, 0.0, 0.25, "bz_worn", True),
    ("nose", -0.3, 0.3, 5.8, 6.3, 0.0, 0.45, "bz_relief", False),
    ("jaw", -1.2, 1.2, 5.4, 5.7, 0.0, 0.3, "bz_relief", False),
)


def leg_faces(g: Gear, bone: str, tag: str, cx: float, cz: float, dz: int) -> None:
    """腿顶兽面：朝 z 外侧（dz=-1 前 / +1 后）一幅，朝 x 外侧一幅（右腿 +x，左腿由 pair 镜像）。"""
    half = LEG_AF / 2
    for axis_name in ("z", "x"):
        for name, u0, u1, v0, v1, d0, d1, mat, mirror in LEG_FACE:
            spans = [(u0, u1)] + ([(-u1, -u0)] if mirror else [])
            for k, (a, c) in enumerate(spans):
                if axis_name == "z":
                    z0 = cz + dz * half
                    zf, zt = (z0 - d1, z0 - d0) if dz < 0 else (z0 + d0, z0 + d1)
                    f, t = (cx + a, v0, zf), (cx + c, v1, zt)
                else:
                    x0 = cx + half
                    f, t = (x0 + d0, v0, cz + a), (x0 + d1, v1, cz + c)
                g.pair(bone, f"leg_{tag}_face{axis_name}_{name}{k}", f, t, mat, sided=True)


def leg_pair(g: Gear, tag: str, cz: float, dz: int) -> None:
    """一对腿（左右镜像）：足环 + 圆柱腿身 + 弦纹 + 颈部回纹 + 颈环 + 兽面头。"""
    bone, cx, af = f"leg_{tag}", LEG_CX, LEG_AF
    octagon(g, bone, f"leg_{tag}_foot", cx, cz, af, 0.0, 0.5, "bz_worn")           # 足底手磨亮铜
    octagon(g, bone, f"leg_{tag}_shaft", cx, cz, af - 0.2, 0.5, 5.25, "bz")
    octagon(g, bone, f"leg_{tag}_cord", cx, cz, af, 2.5, 2.9, "bz_relief")         # 弦纹
    octagon(g, bone, f"leg_{tag}_collar", cx, cz, af, 4.9, 5.3, "bz_relief")       # 颈环
    octagon(g, bone, f"leg_{tag}_head", cx, cz, af, 5.3, LEG_TOP, "bz")            # 兽面头
    half = af / 2 - 0.1
    for k, (a, c, y0, y1) in enumerate(((-0.7, 0.7, 3.25, 3.75), (-0.7, 0.7, 4.0, 4.5))):    # 颈部回纹：四面各两格
        g.pair(bone, f"leg_{tag}_pat_z{k}", (cx + a, y0, cz + dz * half - (0.22 if dz < 0 else 0)),
               (cx + c, y1, cz + dz * half + (0.22 if dz > 0 else 0)), "bz_relief", sided=True)
        g.pair(bone, f"leg_{tag}_pat_x{k}", (cx + half, y0, cz + a), (cx + half + 0.22, y1, cz + c), "bz_relief", sided=True)
    leg_faces(g, bone, tag, cx, cz, dz)


def part_legs_f(g: Gear) -> None:
    """前腿一对（-z 侧）：八棱柱腿 + 兽面。"""
    leg_pair(g, "f", -LEG_CZ, -1)


def part_legs_b(g: Gear) -> None:
    """后腿一对（+z 侧）。"""
    leg_pair(g, "b", LEG_CZ, 1)


# ───────────────────────────────────────────────────────────── 内腔
def part_interior(g: Gear) -> None:
    """内腔：四壁下半火燎黑（两级不齐的上沿）+ 后壁一块空匾（方框 + 暗铜底，不写字）。"""
    b, t = "interior", WALL_T
    wx, wz = HX - t, HZ - t
    # 火燎黑：贴在内壁面上的 0.2 厚薄片（y 8.2 起，上沿两级）
    g.box(b, "soot_f1", (-(wx - 0.05), 8.2, -wz), (wx - 0.05, 10.0, -(wz - 0.22)), "soot")
    g.box(b, "soot_f2", (-5.5, 10.0, -wz), (5.5, 10.8, -(wz - 0.22)), "soot")
    g.box(b, "soot_b1", (-(wx - 0.05), 8.2, wz - 0.22), (wx - 0.05, 9.6, wz), "soot")
    g.box(b, "soot_b2", (-4.5, 9.6, wz - 0.22), (4.5, 10.5, wz), "soot")
    g.pair(b, "soot_s1", (wx - 0.22, 8.2, -(wz - 0.25)), (wx, 9.8, wz - 0.25), "soot")
    g.pair(b, "soot_s2", (wx - 0.22, 9.8, -3.6), (wx, 10.7, 3.6), "soot")
    # 空匾：后壁内面，方框一圈 + 中间一块暗铜底（铭文位）
    px0, px1, py0, py1 = 2.0, 2.25, 11.4, 13.5
    g.box(b, "plaque_top", (-2.25, py1 - 0.25, wz - 0.25), (2.25, py1, wz), "bz_relief")
    g.box(b, "plaque_bot", (-2.25, py0, wz - 0.25), (2.25, py0 + 0.25, wz), "bz_relief")
    g.pair(b, "plaque_side", (px0, py0 + 0.25, wz - 0.25), (px1, py1 - 0.25, wz), "bz_relief")
    g.box(b, "plaque_field", (-px0, py0 + 0.25, wz - 0.2), (px0, py1 - 0.25, wz), "bz_dk")


def part_ash(g: Gear) -> None:
    """鼎底冷灰（不对称）：一层薄灰 + 几堆灰堆 + 一两块没烧完的炭渣。没有柴火。"""
    b = "ash"
    wx, wz = HX - WALL_T, HZ - WALL_T
    g.box(b, "ash_layer", (-(wx - 0.3), 8.2, -(wz - 0.3)), (wx - 0.3, 8.5, wz - 0.3), "ash_dk")
    g.box(b, "ash_heap_a", (-3.4, 8.5, -2.2), (-0.8, 9.05, 0.6), "ash")
    g.box(b, "ash_heap_a2", (-2.8, 9.05, -1.6), (-1.4, 9.35, -0.2), "ash")
    g.box(b, "ash_heap_b", (2.4, 8.5, 1.0), (5.2, 8.9, 3.6), "ash")
    g.box(b, "ash_heap_c", (-6.2, 8.5, 2.4), (-4.4, 8.8, 4.2), "ash")
    g.box(b, "ash_drift", (0.6, 8.5, -3.8), (3.0, 8.75, -2.6), "ash")
    g.box(b, "char_a", (-0.3, 8.5, 1.8), (0.9, 8.85, 2.6), "soot")
    g.box(b, "char_b", (4.4, 8.9, 1.8), (5.2, 9.2, 2.5), "soot")


PARTS = {
    "body": part_body,
    "ring": part_ring,
    "panel_long": part_panel_long,
    "panel_short": part_panel_short,
    "rim": part_rim,
    "handle": part_handle,
    "legs_f": part_legs_f,
    "legs_b": part_legs_b,
    "interior": part_interior,
    "ash": part_ash,
}

# 参考图（qing_tong_fang_ding_ref.png，1536×1024）里的整件外框与分段，像素坐标
SPEC = MetalSpec(
    gear_id="qing_tong_fang_ding",
    model_name="QingTongFangDing",
    title="青铜方鼎 / qing_tong_fang_ding",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=HERE,
    asym=ASYM_BONES,
    soft_over=SOFT_OVER,
    min_bones=10,
    round_label="round 1/3",
    ref_front_box=(34, 70, 826, 942),
    ref_side_box=(935, 72, 1510, 942),
    height_parts=(
        ("腿", 612, 942, ("leg_",)),
        ("壁（含阵纹环）", 335, 612, ("wall_", "ring_")),
        ("纹带 + 联珠", 285, 335, ("strip_", "bead_")),
        ("口沿唇", 262, 285, ("lip_",)),
        ("立耳露出", 70, 262, ("handle_",)),
    ),
    width_parts=(
        ("front", "口沿", 36, 825, ("lip_",)),
        ("front", "壁", 62, 800, ("wall_",)),
        ("front", "腿外跨", 100, 745, ("leg_",)),
        ("front", "立耳外跨", 62, 800, ("handle_",)),
        ("side", "口沿", 935, 1508, ("lip_",)),
        ("side", "壁", 966, 1479, ("wall_",)),
    ),
)


if __name__ == "__main__":
    metal_review.main(SPEC)
