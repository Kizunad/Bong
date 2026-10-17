#!/usr/bin/env python3
"""手持药锄 yao_chu_tool Blockbench .bbmodel 程序化生成器 —— Round 1 几何（按参考图重做，整个重写）。

对应 NPC：采药人。手持（右手），只刨灵草根，不当武器。上两版被退「刃面平躺、看不出锄形」，
这版先重出一张干净的侧视参考图，锄头轮廓按它的剪影量出（颈 + 梯形刃）：
- 全长 20 px：枯木柄径 0.94 px（占全长 4.7%），握位磨亮，下段缠 6 圈草绳，柄尾一只皮环；
- 柄顶锈铁套筒（带铆钉）伸出一段颈，颈的远端垂下锄刃：刃面**竖着**，在侧视里整张刃面都看得见，
  梯形，宽约 1.8 px、垂落 3.3 px，刃口卷口、有一个缺口，刃厚只有刃宽的约 22% 且向刃口减薄；
- 末法：套筒上一道暗掉的刻线，没有发光部件。

坐标：y 向上，柄轴在 x=0,z=0；锄刃朝 -z（手持时朝前），锄刃平面是 zy，片厚沿 x。
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
sys.path.insert(0, str(REPO / "modelScript" / "gear" / "_kit"))
sys.path.insert(0, str(REPO / "modelScript" / "weapons"))

from bbmodel_maker.rig.rigkit import Rig  # noqa: E402

import weapon_shape_kit as kit  # noqa: E402

WEAPON_ID = "yao_chu_tool"
MODEL_NAME = "YaoChuTool"
OUT_DIR = REPO / "modelScript" / "gear" / WEAPON_ID
GEAR_REVIEW_ROOT = kit.REVIEW_ROOT.parent / "gear"
REVIEW_DIR = GEAR_REVIEW_ROOT / WEAPON_ID
TOTAL_LEN = 20.0
GRIP_Y = 8.5
POSE = "single_hand"
HANDLE_R = 0.47     # 柄径 0.94 px，取自参考图

REF_PNG = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
REF_PANEL_W = 790
REF_AXIS_X = 371

MATS = {
    "wood": (112, 82, 52),           # 枯木柄
    "wood_lt": (150, 112, 72),       # 握位磨亮
    "wood_dk": (76, 54, 34),
    "iron": (96, 80, 72),            # 锈铁
    "iron_edge": (176, 168, 156),    # 刃口磨亮
    "rust": (130, 70, 38),
    "cord": (168, 142, 90),
    "cord_dk": (120, 98, 58),
    "hide": (104, 70, 48),
    "groove": (28, 22, 20),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("loop", "root", (0.0, 0.7, 0.0), "柄尾皮环（挂皮套用）"),
    ("shaft", "root", (0.0, 9.0, 0.0), "八角近圆枯木柄（径 0.94 px），握位磨亮、裂纹"),
    ("grip_wrap", "shaft", (0.0, 5.5, 0.0), "下段握把草绳（6 圈）"),
    ("socket", "shaft", (0.0, 18.8, 0.0), "柄顶锈铁套筒 + 铆钉 + 暗掉的刻线"),
    ("hoe_blade", "socket", (0.0, 17.5, -2.2), "锄刃：竖着的梯形薄铁片（zy 平面），刃口卷口带缺口"),
]


def build_weapon() -> Rig:
    rig = Rig(MATS)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent=parent)

    # 1. 柄：三段八角（握位磨亮），裂纹
    kit.octagon_prism(rig, "shaft", "shaft_lo", 0.0, 0.0, 1.4, 8.0, HANDLE_R + 0.03, "wood")
    kit.octagon_prism(rig, "shaft", "shaft_grip", 0.0, 0.0, 8.0, 15.0, HANDLE_R + 0.02, "wood_lt")
    kit.octagon_prism(rig, "shaft", "shaft_hi", 0.0, 0.0, 15.0, 18.4, HANDLE_R, "wood")
    rig.cube("shaft", "crack", (-0.04, 10.2, 0.47), (0.04, 12.6, 0.52), mat="groove")
    rig.cube("shaft", "crack_b", (0.05, 3.3, -0.55), (0.12, 4.3, -0.5), mat="groove")
    rig.cube("shaft", "knot", (-0.2, 13.6, -0.52), (0.2, 14.1, -0.47), mat="wood_dk")

    # 2. 下段草绳 6 圈（v 4.5–6.7）
    for i in range(6):
        v = 4.5 + i * 0.37
        kit.octagon_prism(rig, "grip_wrap", f"turn_{i}", 0.0, 0.0, v, v + 0.31, HANDLE_R + 0.11, "cord" if i % 2 == 0 else "cord_dk")
    rig.cube("grip_wrap", "tail", (0.1, 3.7, 0.55), (0.32, 4.7, 0.65), mat="cord")

    # 3. 柄尾皮环：四片围成的小环，穿在柄尾孔里
    rig.cube("loop", "ring_top", (-0.14, 0.85, -0.17), (0.14, 1.4, 0.17), mat="hide")
    rig.cube("loop", "ring_l", (-0.14, 0.0, -0.45), (0.14, 1.05, -0.2), mat="hide")
    rig.cube("loop", "ring_r", (-0.14, 0.0, 0.2), (0.14, 1.05, 0.45), mat="hide")
    rig.cube("loop", "ring_bot", (-0.14, -0.12, -0.35), (0.14, 0.22, 0.35), mat="hide")

    # 4. 柄顶套筒：锈铁箍 + 铆钉 + 暗掉的刻线；再向前伸出一段颈，锄刃挂在颈的远端
    kit.octagon_prism(rig, "socket", "collar", 0.0, 0.0, 18.0, 19.4, HANDLE_R + 0.18, "iron")
    kit.octagon_prism(rig, "socket", "collar_lip", 0.0, 0.0, 17.95, 18.2, HANDLE_R + 0.23, "rust")
    kit.octagon_prism(rig, "socket", "cap", 0.0, 0.0, 19.4, 19.75, HANDLE_R + 0.05, "wood_dk")
    rig.cube("socket", "rivet_r", (0.63, 18.5, -0.14), (0.78, 18.85, 0.14), mat="iron_edge")
    rig.cube("socket", "rivet_l", (-0.78, 18.5, -0.14), (-0.63, 18.85, 0.14), mat="iron")
    rig.cube("socket", "dead_line", (0.62, 18.95, -0.34), (0.68, 19.25, 0.34), mat="groove")

    # 5. 锄刃：竖着的薄铁片，朝 -z 垂下。轮廓是按参考图剪影量出的梯形（颈 + 刃）：
    #    颈 u∈[-3.0, -0.5]、v∈[18.65, 19.75]；刃上沿 u∈[-3.0, -1.3]（v=18.7）、刃口 u∈[-3.65, -1.8]（v=15.7），外缘外撇
    quad = [(-1.3, 18.7), (-3.0, 18.7), (-3.65, 15.7), (-1.8, 15.7)]

    def inside(u: float, v: float) -> bool:
        if -3.0 <= u <= -0.5 and 18.65 <= v <= 19.75:
            return True
        sign = None
        for (ax, ay), (bx, by) in zip(quad, quad[1:] + quad[:1]):
            cross = (bx - ax) * (v - ay) - (by - ay) * (u - ax)
            if cross != 0 and sign is not None and (cross > 0) != sign:
                return False
            if cross != 0:
                sign = cross > 0
        return True

    def cover(u: float, v: float, size: float) -> float:
        return 1.0 if inside(u + size / 2, v + size / 2) else 0.0

    def blade_part(u: float, v: float) -> bool:
        if u > -0.6 and v < 19.0:           # 柄和套筒的位置
            return False
        return not (-3.5 <= u <= -3.3 and 15.7 <= v <= 16.0)    # 刃口缺口

    spec = kit.PlateSpec(plane="zy", cell=0.15, half_edge=0.07, half_max=0.26, ramp=0.8, levels=4)
    kit.plate_from_mask(rig, "hoe_blade", "blade", cover, (-3.9, 0.2, 15.3, 20.1), spec, "iron_edge", "iron",
                        keep=blade_part)
    # 卷口：刃口上沿一条略厚的铁棱
    rig.cube("hoe_blade", "roll_edge", (-0.3, 15.5, -3.7), (0.3, 15.8, -1.85), mat="iron_edge")
    # 锈斑
    for i, (v, z) in enumerate(((17.9, -2.2), (16.9, -3.0), (18.7, -1.6), (16.2, -2.4))):
        rig.cube("hoe_blade", f"rust_{i}", (0.22, v, z - 0.2), (0.3, v + 0.4, z + 0.2), mat="rust")
        rig.cube("hoe_blade", f"rust_b_{i}", (-0.3, v + 0.3, z - 0.1), (-0.22, v + 0.65, z + 0.3), mat="rust")
    return rig


TITLE = "#4 手持药锄 yao_chu_tool (Round 1)"
PROP_YAW = -90.0     # 相机在 +x：锄刃（-z）在画面右，与参考图同向
REF_SHADOW_PX = 22

METRICS = [
    ("锄刃垂落高度 / 全长", "blade_h"),
    ("锄刃伸出（柄轴到刃最远处）/ 全长", "blade_reach"),
    ("刃口宽 / 全长", "edge_w"),
    ("刃口距顶 / 全长", "blade_drop"),
    ("柄径 / 全长", "shaft_w"),
]


def measure(mask: np.ndarray) -> dict[str, float]:
    """从侧视剪影量药锄（锄刃朝右）的百分比。参考图和模型渲染用同一份。

    柄宽 = 20%~60% 高度（从上数）处的中位数；锄刃行 = 外廓右端超过柄右沿 1.2 倍柄宽的行；
    刃口宽 = 最低三行刃部（柄右沿以右）的横向范围。
    """
    x0, y0, x1, y1 = kit.mask_bbox(mask)
    height = y1 - y0 + 1
    pct = lambda v: 100.0 * v / height  # noqa: E731
    mids = range(y0 + int(0.3 * height), y0 + int(0.7 * height))
    widths = [(np.where(mask[y])[0].min(), np.where(mask[y])[0].max()) for y in mids]
    shaft_w = float(np.median([b - a + 1 for a, b in widths]))
    axis = float(np.median([(a + b) / 2 for a, b in widths]))
    shaft_right = axis + shaft_w / 2
    blade_rows = [y for y in range(y0, y0 + int(0.3 * height)) if mask[y].any() and np.where(mask[y])[0].max() > shaft_right + 1.2 * shaft_w]
    top, bottom = min(blade_rows), max(blade_rows)
    xs = np.where(mask[bottom - 2:bottom + 1, int(shaft_right + 0.5 * shaft_w):].any(axis=0))[0]
    return {
        "blade_h": pct(bottom - top + 1),
        "blade_reach": pct(x1 - axis),
        "edge_w": pct(xs.max() - xs.min() + 1),
        "blade_drop": pct(bottom - y0 + 1),
        "shaft_w": pct(shaft_w),
    }


def main() -> None:
    import weapon_check

    kit.REVIEW_ROOT = GEAR_REVIEW_ROOT
    rig = build_weapon()
    print(f"{MODEL_NAME}: {len(rig.elements)} 个方块, {len(rig.bones)} 根骨骼")
    weapon_check.write_and_review(sys.modules[__name__], rig)


if __name__ == "__main__":
    main()
