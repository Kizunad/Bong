#!/usr/bin/env python3
"""骨秤 gu_cheng Blockbench .bbmodel 程序化生成器 —— Round 1 几何（按参考图重做，旧版 Gemini 生成器整个重写）。

对应 NPC：骨币游商。挂点：枯藤腰带左后腰。形制与比例取自参考图 ``gu_cheng_ref.png`` 底部那幅大图
（拼版里的特写 / 穿戴小图不参与度量），全长（秤盘外沿到秤杆尾）12 px：

- 秤杆是打磨泛黄的兽腿骨，细而长（杆厚只有全长的 ~5%），两端各一节骨疙瘩，杆身刻 8 道不等距秤花（浅凹，不写字）；
- 左端吊秤盘：三股草绳从杆端系环垂下，托着直径 3.3 px 的浅碗（碎陶 / 骨片拼的盘，口沿一圈铁箍）；
- 秤砣：秤杆偏右一只铁环吊着小锈铁砣（1.2 × 2.0 px，带圆帽与铆钉），离杆顶约 4.75 px；
- 右端是秤纽：草绳在杆上缠几圈、向上系到腰带挂钩，绳尾垂两缕麻流苏；
- 末法：只有秤杆和秤砣上各一道暗掉的刻线槽，没有发光部件。

坐标：模型原点 = 秤纽向上系到腰带的挂点；x 向左为负（穿戴时是玩家左侧外向），y 向上，z 为前后。
穿戴时整体平移 WORN_OFFSET 到「左后腰」（玩家 x=-4.6、y=13、z=3.2），杆在腰线下 y≈11.5、z>2，不进躯干和手臂摆动盒。
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
sys.path.insert(0, str(REPO / "modelScript" / "gear" / "_kit"))
sys.path.insert(0, str(REPO / "modelScript" / "weapons"))

from bbmodel_maker.rig.rigkit import Rig  # noqa: E402

import weapon_shape_kit as kit  # noqa: E402

WEAPON_ID = "gu_cheng"
MODEL_NAME = "GuCheng"
OUT_DIR = REPO / "modelScript" / "gear" / WEAPON_ID
GEAR_REVIEW_ROOT = kit.REVIEW_ROOT.parent / "gear"
REVIEW_DIR = GEAR_REVIEW_ROOT / WEAPON_ID
TOTAL_LEN = 12.0                     # 秤盘外沿到秤杆尾的横向全长
GRIP_Y = 0.0
POSE = "worn"
WORN_OFFSET = (-4.6, 13.0, 3.2)
WORN_TITLE = "穿戴 · 挂在左后腰（秤纽绳挂腰带），杆在腰线下、躯干后方"

REF_PNG = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
REF_PANEL_W = 1536
REF_BOX = (300, 632, 1230, 1010)     # 拼版底部那幅大图
REF_SHADOW_PX = 1

BEAM_Y = -1.5          # 杆中心高度
BEAM_R = 0.3           # 杆内切半径（厚 0.6 px，参考图约占全长 5%）
BEAM_X0, BEAM_X1 = -9.8, 1.2

MATS = {
    "bone": (220, 212, 192),         # 打磨泛黄的兽骨
    "bone_lt": (236, 229, 211),
    "bone_dk": (176, 161, 139),
    "cord": (168, 140, 88),          # 草绳
    "cord_dk": (122, 96, 56),
    "iron": (88, 80, 76),
    "rust": (122, 66, 36),
    "groove": (28, 22, 20),          # 秤花 / 暗掉的刻线槽
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "挂点（腰带挂钩处）"),
    ("hang_cord", "root", (0.0, 0.0, 0.0), "秤纽向上系到腰带的草绳 + 绳结"),
    ("beam", "root", (0.0, BEAM_Y, 0.0), "兽腿骨秤杆（八角，厚 0.6 px），两端骨疙瘩"),
    ("beam_marks", "beam", (-6.0, BEAM_Y, 0.0), "8 道不等距秤花（浅凹）+ 暗掉的刻线槽"),
    ("fulcrum_wrap", "beam", (0.0, BEAM_Y, 0.0), "秤纽：草绳在杆上缠几圈，绳尾两缕麻流苏"),
    ("pan_rig", "beam", (-9.2, -1.9, 0.0), "杆左端系环 + 三股草绳"),
    ("pan", "pan_rig", (-9.2, -4.9, 0.0), "碎陶 / 骨片浅碗秤盘（径 3.3 px）+ 铁箍口沿"),
    ("weight_ring", "beam", (-2.5, BEAM_Y, 0.0), "吊秤砣的铁环（环着杆）+ 短绳"),
    ("weight", "weight_ring", (-2.5, -4.7, 0.0), "小锈铁秤砣：圆帽、方身、铆钉"),
]

TICKS = (-8.6, -7.7, -6.6, -5.8, -4.9, -4.2, -3.6, -3.0)    # 不等距秤花（x）


def build_weapon() -> Rig:
    rig = Rig(MATS)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent=parent)
    top = BEAM_Y + BEAM_R

    # 1. 秤杆：八角骨杆（中段略磨亮）+ 两端骨疙瘩
    kit.octagon_prism_x(rig, "beam", "shaft_l", BEAM_Y, 0.0, BEAM_X0 + 0.7, -4.5, BEAM_R, "bone")
    kit.octagon_prism_x(rig, "beam", "shaft_m", BEAM_Y, 0.0, -4.5, -1.2, BEAM_R + 0.02, "bone_lt")
    kit.octagon_prism_x(rig, "beam", "shaft_r", BEAM_Y, 0.0, -1.2, BEAM_X1 - 0.8, BEAM_R, "bone")
    kit.octagon_prism_x(rig, "beam", "knob_l", BEAM_Y, 0.0, BEAM_X0, BEAM_X0 + 0.8, 0.42, "bone_dk")
    kit.octagon_prism_x(rig, "beam", "knob_l_cap", BEAM_Y, 0.0, BEAM_X0, BEAM_X0 + 0.18, 0.36, "bone_lt")
    kit.octagon_prism_x(rig, "beam", "knob_r", BEAM_Y, 0.0, BEAM_X1 - 0.95, BEAM_X1, 0.45, "bone_dk")
    kit.octagon_prism_x(rig, "beam", "knob_r_cap", BEAM_Y, 0.0, BEAM_X1 - 0.2, BEAM_X1, 0.38, "bone_lt")
    for i, dz in enumerate((-0.14, 0.0, 0.14)):    # 右骨疙瘩上的骨髓孔暗点
        rig.cube("beam", f"marrow_{i}", (BEAM_X1 - 0.5, BEAM_Y - 0.1 + dz * 0.4, 0.4), (BEAM_X1 - 0.3, BEAM_Y + 0.1 + dz * 0.4, 0.44), mat="bone_dk")

    # 2. 秤花：8 道浅凹，顶面和正面（+z）各一排；另有一道暗掉的刻线槽（末法）
    for i, x in enumerate(TICKS):
        rig.cube("beam_marks", f"tick_top_{i}", (x - 0.06, top - 0.07, -0.16), (x + 0.06, top + 0.01, 0.16), mat="groove")
        rig.cube("beam_marks", f"tick_front_{i}", (x - 0.06, BEAM_Y - 0.2, 0.32), (x + 0.06, BEAM_Y + 0.2, 0.36), mat="groove")
    rig.cube("beam_marks", "dead_line", (-4.2, BEAM_Y - 0.03, -0.36), (-3.0, BEAM_Y + 0.03, -0.32), mat="groove")

    # 3. 秤纽：草绳缠杆 5 圈 + 向上的双股提绳（系腰带）+ 两缕麻流苏
    for i in range(5):
        x = -1.4 + i * 0.36
        kit.octagon_prism_x(rig, "fulcrum_wrap", f"turn_{i}", BEAM_Y, 0.0, x, x + 0.26, BEAM_R + 0.13,
                            "cord" if i % 2 == 0 else "cord_dk")
    for k, dz in enumerate((-0.12, 0.12)):
        pts = [(0.0, top + 0.1, dz), (0.0, top + 0.7, dz), (0.0, top + 1.3, dz * 0.4)]
        for t, (a, b) in enumerate(zip(pts, pts[1:])):
            rig.shaft("hang_cord", f"lift_{k}_{t}", a, b, 0.1, 0.1, mat="cord")
    rig.cube("hang_cord", "knot", (-0.26, top + 1.1, -0.26), (0.26, top + 1.55, 0.26), mat="cord_dk")
    rig.cube("hang_cord", "loop_top", (-0.2, top + 1.55, -0.1), (0.2, top + 1.75, 0.1), mat="cord")
    for k, (x, ln) in enumerate(((-0.55, 2.2), (-0.15, 2.8))):
        pts = [(x, BEAM_Y - BEAM_R - 0.15 - j * ln / 4, 0.1 * math.sin(j)) for j in range(5)]
        for t, (a, b) in enumerate(zip(pts, pts[1:])):
            rig.shaft("fulcrum_wrap", f"tassel_{k}_{t}", a, b, 0.1, 0.1, mat="cord")
        rig.cube("fulcrum_wrap", f"tassel_tip_{k}", (x - 0.14, pts[-1][1] - 0.35, -0.14), (x + 0.14, pts[-1][1], 0.14), mat="cord_dk")

    # 4. 秤盘系环 + 三股草绳 + 浅碗（径 3.3 px、高 1.5 px）
    px, ring_y = -9.2, BEAM_Y - BEAM_R - 0.18
    kit.octagon_prism_x(rig, "pan_rig", "hang_ring", ring_y, 0.0, px - 0.2, px + 0.2, 0.3, "iron")
    kit.octagon_prism(rig, "pan_rig", "hang_knot", px, 0.0, ring_y - 0.55, ring_y - 0.2, 0.26, "cord_dk")
    rim_y, bowl_h, rim_r = -4.2, 1.5, 1.65
    for k in range(3):
        ang = math.radians(90 + 120 * k)
        start = (px, ring_y - 0.5, 0.0)
        end = (px + rim_r * 0.86 * math.cos(ang), rim_y + 0.05, rim_r * 0.86 * math.sin(ang))
        n = 4
        pts = [tuple(start[c] + (end[c] - start[c]) * t / n for c in range(3)) for t in range(n + 1)]
        for t, (a, b) in enumerate(zip(pts, pts[1:])):
            rig.shaft("pan_rig", f"cord_{k}_{t}", a, b, 0.09, 0.09, mat="cord" if k != 1 else "cord_dk")
    # 浅碗：底盘 + 三层向外张开的 8 片壁板 + 口沿铁箍
    kit.octagon_prism(rig, "pan", "base", px, 0.0, rim_y - bowl_h, rim_y - bowl_h + 0.3, 0.95, "bone_dk")
    tiers = [(rim_y - bowl_h + 0.3, 0.5, 1.12), (rim_y - bowl_h + 0.8, 0.5, 1.38), (rim_y - 0.2, 0.5, 1.62)]
    for ti, (y0, h, r) in enumerate(tiers):
        for s in range(8):
            ang = math.radians(22.5 + 45 * s)
            cx, cz = px + r * math.cos(ang), r * math.sin(ang)
            half = r * math.tan(math.radians(22.5)) + 0.06
            rig.cube("pan", f"wall_{ti}_{s}", (cx - half, y0, cz - 0.15), (cx + half, y0 + h, cz + 0.15),
                     rot=(0.0, -math.degrees(ang) + 90.0, 0.0), org=(cx, y0 + h / 2, cz),
                     mat="bone" if (s + ti) % 2 == 0 else "bone_lt")
    for s in range(8):
        ang = math.radians(22.5 + 45 * s)
        cx, cz = px + (rim_r + 0.02) * math.cos(ang), (rim_r + 0.02) * math.sin(ang)
        half = rim_r * math.tan(math.radians(22.5)) + 0.07
        rig.cube("pan", f"rim_{s}", (cx - half, rim_y - 0.2, cz - 0.19), (cx + half, rim_y + 0.04, cz + 0.19),
                 rot=(0.0, -math.degrees(ang) + 90.0, 0.0), org=(cx, rim_y - 0.08, cz), mat="iron")
    rig.cube("pan", "clasp", (px - 0.2, rim_y - 0.4, rim_r + 0.1), (px + 0.2, rim_y + 0.05, rim_r + 0.3), mat="rust")

    # 5. 秤砣：铁环环着杆，短绳吊小锈铁砣（圆帽 + 方身 + 铆钉），砣底离杆顶 4.75 px
    wx = -2.5
    kit.octagon_prism_x(rig, "weight_ring", "ring", BEAM_Y, 0.0, wx - 0.17, wx + 0.17, BEAM_R + 0.28, "iron")
    kit.octagon_prism(rig, "weight_ring", "short_cord", wx, 0.0, -3.9, BEAM_Y - BEAM_R - 0.25, 0.1, "cord")
    w_top = -3.8
    kit.octagon_prism(rig, "weight", "cap", wx, 0.0, w_top - 0.45, w_top, 0.38, "iron")
    kit.octagon_prism(rig, "weight", "cap_lip", wx, 0.0, w_top - 0.55, w_top - 0.4, 0.46, "rust")
    rig.cube("weight", "body", (wx - 0.62, -5.85, -0.5), (wx + 0.62, w_top - 0.5, 0.5), mat="iron")
    rig.cube("weight", "shoulder_l", (wx - 0.7, w_top - 0.9, -0.42), (wx - 0.55, w_top - 0.5, 0.42), mat="rust")
    rig.cube("weight", "shoulder_r", (wx + 0.55, w_top - 0.9, -0.42), (wx + 0.7, w_top - 0.5, 0.42), mat="rust")
    rig.cube("weight", "foot", (wx - 0.55, -5.9, -0.44), (wx + 0.55, -5.82, 0.44), mat="rust")
    for i, (dx, dy) in enumerate(((-0.34, -4.35), (0.34, -4.35), (-0.34, -5.4), (0.34, -5.4))):
        rig.cube("weight", f"rivet_{i}", (wx + dx - 0.1, dy - 0.1, 0.5), (wx + dx + 0.1, dy + 0.1, 0.6), mat="rust")
    rig.cube("weight", "dead_mark", (wx - 0.3, -5.05, 0.5), (wx + 0.3, -4.97, 0.54), mat="groove")
    return rig


TITLE = "#1 骨秤 gu_cheng (Round 1)"
PROP_YAW = 0.0     # 相机在 +z，秤盘（-x）在画面左，与参考图同向
MEASURE_EXCLUDE_BONES = ("hang_cord",)   # 参考图里没有向上的提绳，量剪影时去掉
SKIP_IOU = True    # 参考图是透视图，秤杆是斜的，剪影重合没有意义

METRICS = [
    ("总高（杆顶到最低处）/ 全长", "height"),
    ("秤杆厚 / 全长", "beam_t"),
    ("秤盘宽 / 全长", "pan_w"),
    ("秤盘最低处距杆顶 / 全长", "pan_drop"),
    ("秤砣最低处距杆顶 / 全长", "weight_drop"),
    ("秤砣宽 / 全长", "weight_w"),
]


def measure(mask: np.ndarray) -> dict[str, float]:
    """从正视剪影量骨秤各部件占全长的百分比（秤盘在左、秤砣在右）。参考图和模型渲染用同一份。

    杆顶 = 剪影最高行；杆厚 = 全长 45% 处那一列最上面一段的竖直厚度（那里只有杆）；
    秤盘 / 秤砣的最低处分别取左 35% / 右 45% 的最低行；盘宽、砣宽取各自最低的 6% 全长高度内的横向范围。
    """
    x0, y0, x1, y1 = kit.mask_bbox(mask)
    length = x1 - x0 + 1
    pct = lambda v: 100.0 * v / length  # noqa: E731
    col = np.where(mask[:, x0 + int(0.45 * length)])[0]
    top_run = [col[0]]
    for y in col[1:]:
        if y != top_run[-1] + 1:
            break
        top_run.append(y)
    left, right = x0 + int(0.35 * length), x0 + int(0.55 * length)
    pan_bottom = int(np.where(mask[:, x0:left].any(axis=1))[0].max())
    weight_bottom = int(np.where(mask[:, right:x1 + 1].any(axis=1))[0].max())

    def span(xa: int, xb: int, bottom: int) -> float:
        band = mask[bottom - int(0.06 * length):bottom + 1, xa:xb + 1]
        cols = np.where(band.any(axis=0))[0]
        return float(cols.max() - cols.min() + 1)

    return {
        "height": pct(y1 - y0 + 1),
        "beam_t": pct(len(top_run)),
        "pan_w": pct(span(x0, left, pan_bottom)),
        "pan_drop": pct(pan_bottom - y0),
        "weight_drop": pct(weight_bottom - y0),
        "weight_w": pct(span(right, x1, weight_bottom)),
    }


def main() -> None:
    import weapon_check

    kit.REVIEW_ROOT = GEAR_REVIEW_ROOT
    rig = build_weapon()
    print(f"{MODEL_NAME}: {len(rig.elements)} 个方块, {len(rig.bones)} 根骨骼")
    weapon_check.write_and_review(sys.modules[__name__], rig)


if __name__ == "__main__":
    main()
