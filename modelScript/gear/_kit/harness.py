"""背包类装备共用的「肩带 + 胸带 + 腰带」挂具：骑过手臂上沿、不进躯干与手臂盒。

坐标与 gear_kit 一致（+x = 穿戴者右侧，+z 背面）。背面段贴在躯干后面（z 2.05–2.55，包身从 z≥2.6 起），
胸前段贴在躯干前面（z -2.55…-2.05）；中间那一段改走手臂上沿（x 4.1–5.7，y 24.05 以上），
因为躯干顶上就是头，肩带没法从躯干正上方过去。

用法：在生成器里把 ``HARNESS_BONES`` 并进骨骼树，再把下面四个 part 函数（按 ``HarnessMats`` 绑好材质）
放进 PART_FUNCS；``ASYM_BONES`` 里加上 ``HARNESS_ASYM``。
"""

from __future__ import annotations

from dataclasses import dataclass
from functools import partial

from gear_kit import Gear

SX = (2.0, 3.8)                  # 背面 / 胸前段 x 范围（右）
BX = (4.1, 5.7)                  # 骑过手臂上沿那一段 x 范围（右）
ZB = (2.05, 2.55)                # 背面段 z
ZF = (-2.55, -2.05)              # 胸前段 z

HARNESS_BONES = [
    ("strap_r", "root", (4.9, 24.4, 0.0), "右肩带：背面段 → 骑过右臂上沿 → 胸前段 + 皮扣"),
    ("strap_l", "root", (-4.9, 24.4, 0.0), "左肩带（镜像）"),
    ("strap_chest", "root", (0.0, 19.8, -2.3), "胸前横带 + 骨扣（连两条前肩带）"),
    ("waist_band", "root", (0.0, 12.9, -2.3), "腰间前带 + 后带 + 带扣 + 针孔铆钉"),
    ("waist_tail", "waist_band", (2.1, 12.3, -2.3), "带扣一侧垂下的带头（会晃）"),
]
HARNESS_ASYM = frozenset({"waist_tail"})


@dataclass(frozen=True)
class HarnessMats:
    """挂具用到的材质名（生成器自己的 MATS 里的键）。"""

    strap: str          # 带面
    strap_dk: str       # 骑在手臂上沿的那段 / 腰带
    pad: str            # 肩垫
    stitch: str         # 针脚
    hard: str           # 带扣金属
    hard_pin: str       # 扣针 / 铆钉
    hard_rust: str      # 扣上的锈斑
    toggle: str         # 骨栓
    toggle_pin: str
    hole: str           # 针孔 / 扣孔的暗色


def straps(g: Gear, m: HarnessMats) -> None:
    """皮绳肩带：背面段（包与背之间）→ 外移 → 骑过手臂上沿（避开头和躯干）→ 胸前段 → 皮扣。"""
    b = "strap"
    # 相邻两段在拐角处会叠在一起，各段的厚度 / 端面故意错开 0.02–0.05px（肉眼不可见），避免面共面闪烁
    g.pair(b, "back_run", (SX[0], 12.4, ZB[0]), (SX[1], 23.9, ZB[1]), m.strap, sided=True)
    g.pair(b, "back_jog", (SX[1], 22.4, ZB[0] + 0.02), (BX[1], 23.88, ZB[1] - 0.02), m.strap, sided=True)
    g.pair(b, "riser", (BX[0], 23.9, ZB[0] + 0.04), (BX[1] - 0.02, 24.65, ZB[1] - 0.04), m.strap, sided=True)
    g.pair(b, "over", (BX[0] + 0.05, 24.05, ZF[0] + 0.03), (BX[1] - 0.05, 24.62, ZB[1] - 0.03), m.strap_dk, sided=True)
    g.pair(b, "front_drop", (BX[0] + 0.1, 20.8, ZF[0] + 0.05), (BX[1] - 0.1, 24.6, ZF[1] - 0.02), m.strap, sided=True)
    g.pair(b, "front_jog", (SX[0], 20.75, ZF[0] + 0.02), (BX[1] - 0.15, 21.98, ZF[1] - 0.04), m.strap, sided=True)
    g.pair(b, "front_run", (SX[0] + 0.05, 12.8, ZF[0]), (SX[1] - 0.05, 21.95, ZF[1]), m.strap, sided=True)
    # 缝线：带面两侧边各一排针脚短线（比带面凸起 0.2px），逐段断开才读得出「缝」；让开皮扣和骨栓
    for k, y in enumerate((13.1, 14.1, 15.1, 16.1, 18.7, 19.7)):
        for ex in (SX[0] + 0.35, SX[1] - 0.65):
            g.pair(b, f"stitch_{k}_{'o' if ex > 3 else 'i'}", (ex, y, ZF[0] - 0.2), (ex + 0.3, y + 0.6, ZF[0] + 0.05),
                   m.stitch, sided=True)
    # 骑过手臂上沿那段的垫皮：软皮缝在带子上面，读作肩垫的厚度
    g.pair(b, "shoulder_pad", (BX[0] + 0.2, 24.65, -1.6), (BX[1] - 0.2, 25.2, 1.6), m.pad, sided=True)
    for k, z in enumerate((-0.8, 0.8)):
        g.pair(b, f"shoulder_pad_stitch_{k}", (BX[0] + 0.5, 25.2, z - 0.15), (BX[1] - 0.5, 25.45, z + 0.15), m.stitch, sided=True)
    # 胸前的皮扣：金属片 + 扣针 + 骨栓
    g.pair(b, "slide_buckle", (SX[0] - 0.2, 17.0, ZF[0] - 0.4), (SX[1] + 0.2, 18.0, ZF[0]), m.hard, sided=True)
    g.pair(b, "slide_buckle_pin", (2.5, 17.2, ZF[0] - 0.65), (3.3, 17.8, ZF[0] - 0.4), m.hard_pin, sided=True)
    g.pair(b, "toggle", (2.6, 20.6, ZF[0] - 0.55), (3.2, 21.4, ZF[0]), m.toggle, sided=True)


def chest(g: Gear, m: HarnessMats) -> None:
    """胸前横带：连两条前肩带（y19.4–20.2），中间一枚骨扣，带边几针。"""
    b = "strap_chest"
    g.box(b, "chest_band", (-2.1, 19.4, ZF[0] + 0.05), (2.1, 20.2, ZF[1] - 0.05), m.strap_dk)
    g.box(b, "chest_toggle", (-0.5, 19.2, ZF[0] - 0.5), (0.5, 20.4, ZF[0] + 0.05), m.toggle)
    g.box(b, "chest_toggle_pin", (-0.15, 19.5, ZF[0] - 0.7), (0.15, 20.1, ZF[0] - 0.5), m.toggle_pin)
    for k, x in enumerate((0.9, 1.5)):
        g.pair(b, f"chest_stitch_{k}", (x, 19.5, ZF[0] - 0.05), (x + 0.3, 20.1, ZF[0] + 0.15), m.stitch)


def waist(g: Gear, m: HarnessMats) -> None:
    """腰带：胸前带（在两条前肩带下端相接）+ 背后带（连两条背面段）+ 带扣 + 针孔 + 铆钉。"""
    b = "waist_band"
    g.box(b, "front_band", (-4.5, 12.3, ZF[0] - 0.07), (4.5, 13.5, ZF[1] - 0.05), m.strap_dk)
    g.box(b, "back_band", (-4.0, 12.3, 2.1), (4.0, 13.5, 2.58), m.strap_dk)
    g.pair(b, "band_end", (4.5, 12.3, ZF[0] - 0.07), (4.9, 13.5, ZF[1] - 0.05), m.strap)   # 带头折回的一小截
    for k, x in enumerate((2.6, 3.5)):                                                     # 针孔 + 铆钉
        g.pair(b, f"band_hole_{k}", (x, 12.7, ZF[0] - 0.18), (x + 0.4, 13.1, ZF[0] + 0.05), m.hole)
    for k, x in enumerate((1.7, 4.05)):
        g.pair(b, f"band_rivet_{k}", (x, 12.7, ZF[0] - 0.35), (x + 0.4, 13.1, ZF[0] - 0.07), m.hard_pin)
    g.box(b, "buckle_frame", (-1.5, 12.0, ZF[0] - 0.7), (1.5, 13.8, ZF[0] - 0.12), m.hard)
    g.box(b, "buckle_hole", (-0.8, 12.6, ZF[0] - 0.75), (0.8, 13.2, ZF[0] - 0.5), m.hole)
    g.box(b, "buckle_tongue", (-0.2, 12.4, ZF[0] - 1.0), (0.2, 13.4, ZF[0] - 0.7), m.hard_pin)
    g.pair(b, "buckle_rust", (0.5, 13.0, ZF[0] - 0.9), (1.4, 13.7, ZF[0] - 0.7), m.hard_rust)


def tail(g: Gear, m: HarnessMats) -> None:
    """腰带垂尾：从带扣一侧垂下的带头 + 一个磨损的穿孔（在腿盒外面，z<-2）。"""
    b = "waist_tail"
    g.box(b, "tail", (1.5, 9.6, ZF[0] + 0.1), (2.7, 12.3, ZF[1] - 0.1), m.strap)
    g.box(b, "tail_tip", (1.6, 9.0, ZF[0] + 0.12), (2.3, 9.6, ZF[1] - 0.12), m.strap_dk)
    g.box(b, "tail_hole", (1.9, 10.8, ZF[0] - 0.05), (2.3, 11.2, ZF[0] + 0.15), m.hole)
    g.box(b, "tail_stitch", (1.6, 11.4, ZF[0] - 0.05), (2.6, 11.7, ZF[0] + 0.15), m.stitch)


def harness_parts(m: HarnessMats) -> dict:
    """四个挂具 part 函数，材质已绑定，可直接并进 PART_FUNCS。"""
    parts = {}
    for key, fn in (("straps", straps), ("chest", chest), ("waist", waist), ("tail", tail)):
        bound = partial(fn, m=m)
        bound.__doc__ = fn.__doc__
        parts[key] = bound
    return parts
