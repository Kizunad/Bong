#!/usr/bin/env python3
"""青铜权（qing_tong_quan，一套三枚）Blockbench .bbmodel 程序化生成器 —— Round 1/3，只做几何。

任务卡：旧时铁器 / 青铜器 #11（手持小件，成套，摆在桌面）。凡物青铜砝码：度量衡仍在，称的是矿石 / 干草 / 陶器——
骨币不用称，真元多少靠手感；所以权上只有弦纹和一块**空的**铭文位（凹框，不写字），没有任何灵纹。

造型与比例以参考图（``qing_tong_quan_ref.png``：左 = 三枚并排正视，右 = 最大一枚的侧视）为准：
覆斗形（圆台，下宽上窄），顶上一只半环纽，正面一块长方凹框空匾，上部一组双线、下部一组双线；
三枚高度比 1 : 0.81 : 0.62，本模型按最大一枚高 5px（任务卡的 5 / 4 / 3），所以中 4.05px、小 3.1px，
整套并排宽 9.1px，以中间一枚为中心（大在 -x，小在 +x）。横截面是圆角矩形（参考图侧视深 / 正视宽 = 0.9），用三块错开的条叠出圆角。
每枚纽上穿一圈丝绳（朝上立着的绳环），手持时握这一圈；绳环参考图里没有，总高 / 总宽对照不计入。

坐标（px，1 方块 = 16px）：三枚站在地上（y=0），自 +x 到 -x 依次 大 / 中 / 小（从正面 -z 看大在左，同参考图），中间一枚在 x=0；
正面朝 -z；三枚各自左右对称，但大（+x）、小（-x）两枚不在 x=0 上，各占 asym 骨，中间一枚照常过镜像门。

用法：
    python3 modelScript/metalware/qing_tong_quan/gen_qing_tong_quan.py --table | --gates | (全量)
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "metalware" / "_kit"))
import metal_review  # noqa: E402
from gear_kit import Gear  # noqa: E402
from metal_review import MetalSpec  # noqa: E402

MATS = {
    "bz":        (150, 92, 58),     # 红褐铜底
    "bz_relief": (168, 112, 74),    # 弦纹 / 匾框
    "bz_pat":    (84, 130, 110),    # 青绿锈
    "bz_worn":   (206, 152, 98),    # 手磨亮铜（纽、脚边）
    "bz_dk":     (76, 54, 42),      # 空匾底
    "cord":      (110, 92, 70),     # 丝绳
}

# ───────────────────────────────────────────────────────────── 尺寸：参考图 px → 模型 px
S = 5.0 / 600.0                    # 最大一枚：纽顶 ref y 152 到地面 752 = 600px → 5px
BODY_H = (752 - 268) * S           # 4.03：身高（不含纽）
HX_BOT, HX_TOP = 216 * S, 165 * S  # 底 / 顶半宽（正视），1.8 / 1.375
HZ_BOT, HZ_TOP = 194 * S, 137 * S  # 底 / 顶半深（侧视），1.62 / 1.14
RING_H = (268 - 152) * S           # 纽高 0.97
RING_HW = 105 * S                  # 纽外半宽 0.875
TIERS = 4
RELIEF_D = 0.22
LINE_H = 0.24

# (前缀, 参考图里该枚中心 x px, 相对最大一枚的缩放)；模型里 x 取反（见 make_part）
WEIGHTS = (("big", 291.5, 1.0), ("mid", 706.0, 0.81), ("small", 1031.5, 0.617))
ROW_CENTER = 706.0                 # 整套以中间一枚的中心为原点（x px）

BONES = [("root", None, (0.0, 0.0, 0.0), "世界根；三枚站在地上")]
for _p, _xc, _k in WEIGHTS:
    _cx = -(_xc - ROW_CENTER) * S
    BONES += [
        (f"{_p}_body", "root", (_cx, 0.0, 0.0), f"{_p} 权身：覆斗形（圆角矩形截面）四层逐层收窄 + 脚边"),
        (f"{_p}_relief", f"{_p}_body", (_cx, 2.0 * _k, 0.0), f"{_p} 权：双线弦纹 + 正面空匾（凹框，不写字）"),
        (f"{_p}_ring", f"{_p}_body", (_cx, BODY_H * _k, 0.0), f"{_p} 权：半环纽 + 穿环的丝绳环"),
    ]
ASYM_BONES = frozenset(b[0] for b in BONES if b[0] != "root" and not b[0].startswith("mid_"))   # 中间一枚在 x=0，自身左右对称，参与镜像门


# ───────────────────────────────────────────────────────────── 单枚权
class Weight:
    """一枚权的几何：所有长度先按最大一枚算，再乘 k；cx 是这一枚的中心 x。"""

    def __init__(self, g: Gear, prefix: str, cx: float, k: float, skip_sides: tuple = ()) -> None:
        self.g, self.p, self.cx, self.k = g, prefix, cx, k
        self.skip_sides = skip_sides      # 弦纹不画的侧（贴着邻枚的那一侧：两枚间隙只有 0.2px，容不下 0.22 的浮雕）
        self.body_top = BODY_H * k
        self.tier_h = self.body_top / TIERS

    def dims(self, t: float) -> tuple[float, float]:
        """身高比例 t（0 = 底，1 = 顶）处的 (半宽, 半深)。"""
        return (HX_BOT + (HX_TOP - HX_BOT) * t) * self.k, (HZ_BOT + (HZ_TOP - HZ_BOT) * t) * self.k

    def tier_dims(self, i: int) -> tuple[float, float]:
        hx, hz = self.dims((i + 0.5) / TIERS)
        return (hx * 0.93, hz * 0.93) if i == TIERS - 1 else (hx, hz)   # 最上一层收一点，读作圆肩

    def span(self, y0: float, y1: float) -> tuple[float, float, float, float]:
        """y 区间覆盖到的各层里，半宽 / 半深的 (最小, 最大, 最小, 最大)。"""
        i0 = max(0, int(y0 / self.tier_h))
        i1 = min(TIERS - 1, int((y1 - 1e-6) / self.tier_h))
        ds = [self.tier_dims(i) for i in range(i0, i1 + 1)]
        return min(d[0] for d in ds), max(d[0] for d in ds), min(d[1] for d in ds), max(d[1] for d in ds)

    def rows(self, y0: float, y1: float):
        """[y0, y1] 覆盖到的各层：(层号, 该层内的起, 止)；贴面浮雕按层分段，跟着权身的收窄走，不留缝也不扎深。"""
        out = []
        for i in range(TIERS):
            a, b = max(y0, i * self.tier_h), min(y1, (i + 1) * self.tier_h)
            if b - a > 1e-6:
                out.append((i, a, b))
        return out

    # ---- 权身 ----
    def body(self) -> None:
        g, b, cx = self.g, f"{self.p}_body", self.cx
        for i in range(TIERS):
            hx, hz = self.tier_dims(i)
            y0, y1 = i * self.tier_h, (i + 1) * self.tier_h
            if i == 0:
                y0 += 0.015            # 与脚边错开，防共面
            # 三块错开的条叠出圆角矩形截面：宽条、中条、深条
            g.box(b, f"{self.p}_body_t{i}_a", (cx - hx, y0, -0.62 * hz), (cx + hx, y1, 0.62 * hz), "bz")
            g.box(b, f"{self.p}_body_t{i}_d", (cx - 0.86 * hx, y0 + 0.02, -0.86 * hz), (cx + 0.86 * hx, y1 - 0.02, 0.86 * hz), "bz")
            g.box(b, f"{self.p}_body_t{i}_b", (cx - 0.62 * hx, y0 + 0.04, -hz), (cx + 0.62 * hx, y1 - 0.04, hz), "bz")
        # 脚边：底部一圈加宽的亮铜边（手摸得最多处）
        hx, hz = self.tier_dims(0)
        lip = 0.32
        g.box(b, f"{self.p}_body_lip_a", (cx - hx - 0.1, 0.0, -0.62 * hz - 0.05), (cx + hx + 0.1, lip, 0.62 * hz + 0.05), "bz_worn")
        g.box(b, f"{self.p}_body_lip_b", (cx - 0.62 * hx - 0.05, 0.02, -hz - 0.1), (cx + 0.62 * hx + 0.05, lip - 0.02, hz + 0.1), "bz_worn")
        g.box(b, f"{self.p}_body_lip_d", (cx - 0.86 * hx - 0.05, 0.04, -0.86 * hz - 0.05),
              (cx + 0.86 * hx + 0.05, lip - 0.04, 0.86 * hz + 0.05), "bz_worn")

    # ---- 弦纹 ----
    def belt_line(self, name: str, tier: int, off: float, mat: str = "bz_relief") -> None:
        """在第 tier 层内、离层底 off 处绕身一圈的弦纹（高 LINE_H，凸 0.22）：前后各一块（正对 B 面）、左右各一块、
        四个圆角各一块。每块相对该层外表面凸 0.22、咬进 0.05；贴 A / B 面的块比面小 0.05，免得与身体面共面。
        放不进这一层（离层顶不足 0.02）的线不画。
        """
        if off + LINE_H > self.tier_h - 0.02:
            return
        g, b, cx = self.g, f"{self.p}_relief", self.cx
        hx, hz = self.tier_dims(tier)
        ya = tier * self.tier_h + off
        yb = ya + LINE_H
        tag = f"{self.p}_belt_{name}"
        g.box(b, f"{tag}_f", (cx - 0.62 * hx + 0.05, ya, -hz - RELIEF_D), (cx + 0.62 * hx - 0.05, yb, -hz + 0.05), mat)
        g.box(b, f"{tag}_b", (cx - 0.62 * hx + 0.05, ya, hz - 0.05), (cx + 0.62 * hx - 0.05, yb, hz + RELIEF_D), mat)
        # 以权自身中心 cx 为轴写左右（整枚自身左右对称，但不以世界 x=0 为轴，不能用 pair）
        for sx, side in ((1, "r"), (-1, "l")):
            if side in self.skip_sides:
                continue
            xa, xb = (hx - 0.05, hx + RELIEF_D) if sx > 0 else (-hx - RELIEF_D, -hx + 0.05)
            g.box(b, f"{tag}_s{side}", (cx + xa, ya, -0.62 * hz + 0.05), (cx + xb, yb, 0.62 * hz - 0.05), mat)
            for sz, face in ((-1, "f"), (1, "b")):
                x0, x1 = 0.62 * hx, 0.86 * hx + RELIEF_D
                z0, z1 = 0.62 * hz, 0.86 * hz + RELIEF_D
                g.box(b, f"{tag}_c{side}{face}",
                      (cx + (x0 if sx > 0 else -x1), ya, (z0 if sz > 0 else -z1)),
                      (cx + (x1 if sx > 0 else -x0), yb, (z1 if sz > 0 else -z0)), mat)

    def belts(self) -> None:
        """上部双线在最上一层（参考图 y 350–372，离地 3.2–3.4）；下部双线在最下一层脚边之上（y 650–690，离地 0.5–1.0）。
        小权的最下一层放不下两条，只画一条。"""
        th = self.tier_h
        self.belt_line("u1", TIERS - 1, 0.28)
        self.belt_line("u2", TIERS - 1, 0.28 + LINE_H + 0.08)
        self.belt_line("d1", 0, 0.36)
        self.belt_line("d2", 0, 0.36 + LINE_H + 0.08)
        self.marks()

    def marks(self) -> None:
        """正面肩部三颗乳钉 + 没贴邻枚的那一侧一道竖棱。"""
        g, b, cx = self.g, f"{self.p}_relief", self.cx
        th = self.tier_h
        hx2, hz2 = self.tier_dims(2)
        hx3, hz3 = self.tier_dims(TIERS - 1)
        y0 = (TIERS - 1) * th + 0.28 + 2 * (LINE_H + 0.08) + 0.0
        for i, dx in enumerate((-0.45, 0.0, 0.45)):
            u = dx * max(self.k, 0.85) + cx
            if y0 + 0.26 < TIERS * th - 0.02:
                g.box(b, f"{self.p}_stud_f{i}", (u - 0.13, y0, -hz3 - 0.22), (u + 0.13, y0 + 0.26, -hz3 + 0.05), "bz_worn")
        for side in ("l", "r"):
            if side in self.skip_sides:
                continue
            sx = 1 if side == "r" else -1
            xa, xb = (hx2 - 0.05, hx2 + 0.22) if sx > 0 else (-hx2 - 0.22, -hx2 + 0.05)
            g.box(b, f"{self.p}_rib_{side}", (cx + xa, 1 * th + 0.1, -0.12), (cx + xb, 3 * th - 0.1, 0.12), "bz_relief")

    # ---- 正面空匾 ----
    def cartouche(self) -> None:
        """长方凹框（铭文位，空）：暗铜底板 + 外框，占第 1、2 层（离地约 1.0–3.0，参考图匾底 0.97、匾顶 2.89）；
        正面（-z）一块，背面（+z）对称再一块（权身两面都留匾，参考图只拍了正面）。"""
        g, b, cx, k = self.g, f"{self.p}_relief", self.cx, self.k
        hw = max(0.3, 0.49 * k)
        fw = 0.22
        th = self.tier_h
        for sz, face in ((-1, "f"), (1, "b")):
            def zr(hz: float, d_out: float, d_in: float) -> tuple[float, float]:
                """贴在 sz 面上、外表面在 hz 外 d_out、内侧咬进 d_in 的 z 区间。"""
                return (-hz - d_out, -hz + d_in) if sz < 0 else (hz - d_in, hz + d_out)

            for i in (1, 2):
                hz = self.tier_dims(i)[1]
                ya = i * th + (fw if i == 1 else 0.0)            # 下框在第 1 层底、上框在第 2 层顶，框全在匾所在两层之内
                yb = (i + 1) * th - (fw if i == 2 else 0.0)
                z0, z1 = zr(hz, RELIEF_D, 0.05)
                g.box(b, f"{self.p}_cart_{face}_plate{i}", (cx - hw, ya, z0), (cx + hw, yb, z1), "bz_dk")
                z0, z1 = zr(hz, RELIEF_D + 0.1, 0.05)
                g.box(b, f"{self.p}_cart_{face}_{i}_l", (cx - hw - fw, ya, z0), (cx - hw, yb, z1), "bz_relief")
                g.box(b, f"{self.p}_cart_{face}_{i}_r", (cx + hw, ya, z0), (cx + hw + fw, yb, z1), "bz_relief")
            hz1, hz2 = self.tier_dims(1)[1], self.tier_dims(2)[1]
            z0, z1 = zr(hz1, RELIEF_D + 0.1, 0.05)
            g.box(b, f"{self.p}_cart_{face}_bot", (cx - hw - fw, th, z0), (cx + hw + fw, th + fw, z1), "bz_relief")
            z0, z1 = zr(hz2, RELIEF_D + 0.1, 0.05)
            g.box(b, f"{self.p}_cart_{face}_top", (cx - hw - fw, 3 * th - fw, z0), (cx + hw + fw, 3 * th, z1), "bz_relief")

    # ---- 纽与绳环 ----
    def ring(self) -> None:
        """半环纽（两竖 + 拱顶一至两级）+ 穿环的丝绳环（朝上立着，绳环面朝侧）。"""
        g, b, cx, k = self.g, f"{self.p}_ring", self.cx, self.k
        cap_h = 0.26 if k > 0.7 else 0.0      # 肩顶盖（小权没有余量，省掉）
        top0 = self.body_top + cap_h
        if cap_h:
            hx, hz = self.tier_dims(TIERS - 1)
            g.box(b, f"{self.p}_ring_cap_a", (cx - 0.8 * hx, self.body_top, -0.5 * hz), (cx + 0.8 * hx, top0, 0.5 * hz), "bz_worn")
            g.box(b, f"{self.p}_ring_cap_b", (cx - 0.5 * hx, self.body_top + 0.02, -0.78 * hz), (cx + 0.5 * hx, top0 - 0.02, 0.78 * hz), "bz_worn")
        rh = RING_H * k - cap_h
        hw = RING_HW * k
        lt = max(0.24, 0.39 * k)               # 纽腿宽
        rz = max(0.3, 0.38 * k) / 2            # 纽半厚
        step = 0.24
        two_steps = rh - 2 * step >= step
        legs = rh - (2 * step if two_steps else step)
        for side, sx in (("l", -1), ("r", 1)):
            g.box(b, f"{self.p}_ring_leg_{side}", (cx + (sx * hw if sx < 0 else sx * (hw - lt)), top0, -rz),
                  (cx + (sx * (hw - lt) if sx < 0 else sx * hw), top0 + legs, rz), "bz_worn")
        y = top0 + legs
        if two_steps:
            g.box(b, f"{self.p}_ring_arch_a", (cx - hw, y, -rz), (cx + hw, y + step, rz), "bz_worn")
            g.box(b, f"{self.p}_ring_arch_b", (cx - 0.6 * hw, y + step, -rz), (cx + 0.6 * hw, y + 2 * step, rz), "bz_worn")
        else:
            g.box(b, f"{self.p}_ring_arch_a", (cx - hw, y, -rz), (cx + hw, y + step, rz), "bz_worn")
        ring_top = top0 + rh
        # 绳环：穿过纽孔的横段（贴着拱底）+ 前后两股向上 + 顶端一段
        c = f"{self.p}cord"
        cw, ct = 0.115, 0.23
        top_cord = ring_top + 1.4
        g.box(b, f"{c}_through", (cx - cw, y - step, -(rz + ct)), (cx + cw, y, rz + ct), "cord")
        g.box(b, f"{c}_strand_f", (cx - cw, y, -(rz + ct)), (cx + cw, top_cord - ct, -rz), "cord")
        g.box(b, f"{c}_strand_b", (cx - cw, y, rz), (cx + cw, top_cord - ct, rz + ct), "cord")
        g.box(b, f"{c}_top", (cx - cw, top_cord - ct, -(rz + ct)), (cx + cw, top_cord, rz + ct), "cord")
        self.cord_top = top_cord


SKIP_SIDES = {"big": ("l",), "mid": ("l", "r"), "small": ("r",)}


def make_part(prefix: str, xc: float, k: float, kind: str):
    cx = -(xc - ROW_CENTER) * S          # 相机在 -z 看正面时 +x 在画面左侧：大的放 +x，正面看就是参考图的「大在左」

    def part(g: Gear) -> None:
        w = Weight(g, prefix, cx, k, SKIP_SIDES[prefix])
        if kind == "body":
            w.body()
        elif kind == "relief":
            w.belts()
            w.cartouche()
        else:
            w.ring()

    part.__doc__ = {"body": f"{prefix} 权身：四层圆角矩形截面的覆斗 + 脚边亮铜边。",
                    "relief": f"{prefix} 权：上下双线弦纹 + 正面空匾。",
                    "ring": f"{prefix} 权：半环纽 + 丝绳环。"}[kind]
    return part


PARTS = {f"{p}_{kind}": make_part(p, xc, k, kind) for p, xc, k in WEIGHTS for kind in ("body", "relief", "ring")}


def _cord_top_local() -> float:
    """最大一枚丝绳环顶的局部 y（持握时握这里）。"""
    k = WEIGHTS[0][2]
    return BODY_H * k + RING_H * k + 1.4 - 0.115


# 参考图 qing_tong_quan_ref.png（1774×887）：左 = 三枚并排正视，右 = 最大一枚侧视
SPEC = MetalSpec(
    gear_id="qing_tong_quan",
    model_name="QingTongQuan",
    title="青铜权（一套三枚）/ qing_tong_quan",
    mats=MATS,
    bones=BONES,
    parts=PARTS,
    out_dir=HERE,
    asym=ASYM_BONES,
    min_bones=9,
    round_label="round 1/3",
    interpen_bite=0.3,
    bounds_skip=("bigcord", "midcord", "smallcord"),
    ref_front_box=(75, 152, 1165, 752),
    ref_side_box=(1318, 152, 1710, 752),
    height_parts=(
        ("大权（含纽）", 152, 752, ("big_",)),
        ("中权（含纽）", 265, 752, ("mid_",)),
        ("小权（含纽）", 378, 752, ("small_",)),
        ("大权纽", 152, 268, ("big_ring",)),
    ),
    width_parts=(
        ("front", "大权底宽", 75, 508, ("big_body",)),
        ("front", "中权底宽", 530, 882, ("mid_body",)),
        ("front", "小权底宽", 898, 1165, ("small_body",)),
        ("front", "大权纽宽", 190, 400, ("big_ring_",)),
        ("side", "大权深", 1318, 1710, ("big_body",)),
    ),
    # 手持：只握最大一枚——右臂前抬 90°，握住纽上的丝绳环顶，权垂在拳下；纽 / 绳在手里，不查右臂
    hold={"arm_deg": 90.0, "anchor": (-(WEIGHTS[0][1] - ROW_CENTER) * S, _cord_top_local(), 0.0), "lead": 0.0,
          "skip_arm": ("big_ring", "bigcord"), "only": ("big",)},
)

if __name__ == "__main__":
    metal_review.main(SPEC)
