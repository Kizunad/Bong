#!/usr/bin/env python3
"""铁锸 tie_cha Blockbench .bbmodel 程序化生成器 —— Round 1 几何（按参考图重做，整个重写）。

上一版是一根窄条（Gemini 两次都是），这版先重出一张干净的正视参考图，宽铲形刃按它的剪影取形：
- 全长 26 px：木柄径 0.83 px（3.2%），顶端 T 形横握把（宽 3.1 px）被汗磨黑，握把下是铁套筒（长 1.6 px，带铆钉）；
- 锸头：宽铲形薄铁片，刃宽 6.8 px（占全长 26%）、高 7.9 px，肩圆（肩上一对铆钉），
  中间起脊（由套筒向下渐细渐低），两侧各一道向前折起的 U 形铁套边，刃口一道卷边；
  片厚只有 0.5 px（脊处 1.2 px），刃口磨窄、有两处被磨平的缺口，锤痕一圈圈分布；
- 末法：纯人力破土，没有灵纹；铁面上只有锈和锤痕。

坐标：y 向上，柄轴在 x=0,z=0；锸头平面是 xy，片厚沿 z；起脊、U 形边在 +z 面。
"""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
sys.path.insert(0, str(REPO / "modelScript" / "gear" / "_kit"))
sys.path.insert(0, str(REPO / "modelScript" / "weapons"))
sys.path.insert(0, str(REPO / "modelScript" / "generators"))

from bbmodel_maker.rig.rigkit import Rig  # noqa: E402

import weapon_shape_kit as kit  # noqa: E402

WEAPON_ID = "tie_cha"
MODEL_NAME = "TieCha"
OUT_DIR = REPO / "modelScript" / "metalware" / WEAPON_ID
METAL_REVIEW_ROOT = kit.REVIEW_ROOT.parent / "metalware"
REVIEW_DIR = METAL_REVIEW_ROOT / WEAPON_ID
TOTAL_LEN = 26.0
GRIP_Y = 22.0     # 握在 T 形握把下方，铲头才摆得到腿后面去
POSE = "polearm"
SHAFT_R = 0.415     # 柄径 0.83 px，取自参考图

REF_PNG = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
REF_PANEL_W = 715
REF_AXIS_X = 334
REF_SHADOW_PX = 22

MATS = {
    "iron": (74, 66, 62),            # 黑褐氧化膜
    "iron_mid": (96, 86, 80),
    "iron_edge": (176, 168, 158),    # 刃口磨出的冷灰亮线
    "rust": (126, 66, 36),
    "wood": (116, 84, 52),
    "wood_dk": (62, 44, 30),         # 被汗磨黑的握把
    "pit": (30, 26, 24),             # 锤痕凹坑
    "groove": (26, 22, 20),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("blade", "root", (0.0, 4.0, 0.0), "宽铲形薄铁片（xy 平面，肩圆，刃口磨窄带缺口）"),
    ("ridge", "blade", (0.0, 5.0, 0.5), "中脊：由套筒向下渐细渐低"),
    ("flanges", "blade", (0.0, 2.2, 0.4), "两侧 U 形铁套边与刃口卷边"),
    ("blade_marks", "blade", (0.0, 4.0, 0.3), "肩上铆钉、锤痕凹坑、锈斑"),
    ("socket", "root", (0.0, 9.1, 0.0), "铁套筒（带铆钉与接缝）"),
    ("shaft", "root", (0.0, 17.0, 0.0), "八角近圆木柄（径 0.83 px），上段被汗磨黑"),
    ("t_handle", "shaft", (0.0, 25.6, 0.0), "T 形横握把（宽 3.1 px）"),
]


def build_weapon() -> Rig:
    rig = Rig(MATS)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent=parent)

    # 1. 柄：三段八角（上段被汗磨黑）
    kit.octagon_prism(rig, "shaft", "shaft_lo", 0.0, 0.0, 9.9, 15.0, SHAFT_R + 0.02, "wood")
    kit.octagon_prism(rig, "shaft", "shaft_mid", 0.0, 0.0, 15.0, 19.0, SHAFT_R, "wood")
    kit.octagon_prism(rig, "shaft", "shaft_sweat", 0.0, 0.0, 19.0, 25.2, SHAFT_R - 0.01, "wood_dk")
    rig.cube("shaft", "crack", (-0.04, 12.0, 0.42), (0.04, 14.4, 0.47), mat="groove")
    # T 形横握把：横向八角柱 + 两端端面
    kit.octagon_prism_x(rig, "t_handle", "bar", 25.6, 0.0, -1.55, 1.55, 0.42, "wood_dk")
    kit.octagon_prism_x(rig, "t_handle", "bar_mid", 25.6, 0.0, -0.5, 0.5, 0.45, "wood")
    for side, x in enumerate((-1.58, 1.4)):
        rig.cube("t_handle", f"end_{side}", (x, 25.25, -0.3), (x + 0.18, 25.95, 0.3), mat="wood")

    # 2. 铁套筒（v 8.3–9.9）：套住木柄的铁箍，带接缝与两颗铆钉
    kit.octagon_prism(rig, "socket", "collar", 0.0, 0.0, 8.3, 9.9, 0.64, "iron_mid")
    kit.octagon_prism(rig, "socket", "collar_lip_hi", 0.0, 0.0, 9.7, 9.9, 0.68, "iron")
    kit.octagon_prism(rig, "socket", "collar_lip_lo", 0.0, 0.0, 8.3, 8.5, 0.68, "iron")
    kit.octagon_prism(rig, "socket", "seam", 0.0, 0.0, 8.62, 8.7, 0.66, "groove")
    for side, x in enumerate((-0.74, 0.62)):
        rig.cube("socket", f"rivet_{side}", (x, 9.0, -0.14), (x + 0.12, 9.3, 0.14), mat="iron_edge")

    # 3. 锸头：取自参考图正视剪影的薄铁片，楔形只做刃口
    mask = kit.ref_mask(REF_PNG, REF_PANEL_W, shadow_px=REF_SHADOW_PX)
    frame = kit.frame_from_mask(mask, TOTAL_LEN, REF_AXIS_X)
    cover = kit.sampler(mask, frame)

    def blade_part(u: float, v: float) -> bool:
        if v > 8.3:
            return False
        # 刃口两处被磨平的缺口
        return not ((-1.4 <= u <= -1.1 or 1.9 <= u <= 2.15) and v < 0.3)

    spec = kit.PlateSpec(plane="xy", cell=0.2, half_edge=0.07, half_max=0.25, ramp=0.9, levels=4)
    kit.plate_from_mask(rig, "blade", "plate", cover, (-3.9, 4.1, -0.1, 8.4), spec, "iron_edge", "iron",
                        keep=blade_part)

    # 4. 中脊：从套筒下沿（v 8.3）向下渐细渐低到 v≈1.3，在 +z 面
    for i in range(18):
        v1 = 8.3 - i * 0.39
        v0 = v1 - 0.4
        t = i / 17
        half_w = 0.42 - 0.3 * t
        height = 0.6 - 0.4 * t
        rig.cube("ridge", f"ridge_{i:02d}", (-half_w, v0, 0.25), (half_w, v1, 0.25 + height), mat="iron_mid" if i % 2 else "iron")
        rig.cube("ridge", f"ridge_hi_{i:02d}", (-half_w * 0.4, v0, 0.25 + height), (half_w * 0.4, v1, 0.25 + height + 0.05), mat="iron_edge")

    # 5. U 形铁套边：两侧向前折起（v 1.1–3.6），刃口一道卷边
    for side, sign in enumerate((-1, 1)):
        x = sign * 3.15
        rig.cube("flanges", f"lip_{side}", (x - 0.15, 1.1, -0.2), (x + 0.15, 3.6, 0.75), mat="iron_mid")
        rig.cube("flanges", f"lip_edge_{side}", (x - 0.15 + sign * 0.12, 1.15, 0.2), (x + 0.15 + sign * 0.12, 3.5, 0.62), mat="iron")
        rig.cube("flanges", f"lip_top_{side}", (x - 0.17, 3.3, 0.55), (x + 0.17, 3.6, 0.78), mat="iron_edge")
    rig.cube("flanges", "roll_edge", (-3.0, 0.15, 0.2), (3.1, 0.55, 0.62), mat="iron_mid")
    rig.cube("flanges", "roll_edge_hi", (-3.0, 0.5, 0.45), (3.1, 0.6, 0.65), mat="iron_edge")

    # 6. 肩上一对铆钉、锤痕凹坑（一圈圈）、锈斑
    for side, x in enumerate((-2.35, 2.35)):
        rig.cube("blade_marks", f"rivet_{side}", (x - 0.18, 6.0, 0.25), (x + 0.18, 6.36, 0.45), mat="iron_edge")
        rig.cube("blade_marks", f"rivet_base_{side}", (x - 0.26, 5.92, 0.25), (x + 0.26, 6.44, 0.3), mat="iron")
    import math
    for ring_i, (cx, cy, rad, n) in enumerate(((0.0, 3.8, 1.6, 8), (0.0, 3.8, 2.4, 11), (0.0, 3.8, 3.0, 14))):
        for k in range(n):
            ang = math.radians(200 + 160 * k / max(1, n - 1))
            x, y = cx + rad * math.cos(ang), cy + 0.9 * rad * math.sin(ang)
            if abs(x) < 0.5 or y < 0.7:
                continue
            rig.cube("blade_marks", f"pit_{ring_i}_{k}", (x - 0.09, y - 0.09, 0.25), (x + 0.09, y + 0.09, 0.29), mat="pit")
    for i, (x, v) in enumerate(((-2.0, 5.2), (1.6, 4.4), (-1.2, 2.2), (2.4, 2.8), (0.9, 6.4), (-2.6, 3.6))):
        rig.cube("blade_marks", f"rust_{i}", (x - 0.3, v - 0.2, 0.26), (x + 0.3, v + 0.2, 0.3), mat="rust")
    return rig


TITLE = "#7 铁锸 tie_cha (Round 1)"
PROP_YAW = 0.0     # 相机在 +z，正视，与参考图同向

METRICS = [
    ("锸头宽（最宽处）/ 全长", "blade_w"),
    ("锸头高（含圆肩）/ 全长", "blade_h"),
    ("柄径 / 全长", "shaft_w"),
    ("T 形握把宽 / 全长", "t_w"),
    ("铁套筒长 / 全长", "collar_len"),
    ("肩宽（肩部最宽一行的 90% 处距刃口）/ 全长", "shoulder_at"),
]


def measure(mask: np.ndarray) -> dict[str, float]:
    """从正视剪影量铁锸的百分比。参考图和模型渲染用同一份。

    柄宽 = 全长 40%~60%（从下数 4~11 px 之间避开套筒的上半）的中位数，实际取 20%~35% 高度（从上数）；
    锸头 = 外廓宽于 3 倍柄宽的行（自下数的连续段）；T 握把 = 顶部外廓宽于 3 倍柄宽的行；
    套筒 = 锸头上方、宽度在 1.2~2.2 倍柄宽之间的那段。
    """
    x0, y0, x1, y1 = kit.mask_bbox(mask)
    height = y1 - y0 + 1
    pct = lambda v: 100.0 * v / height  # noqa: E731
    width = {y: int(np.where(mask[y])[0].max() - np.where(mask[y])[0].min() + 1) for y in range(y0, y1 + 1) if mask[y].any()}
    shaft_w = float(np.median([width[y] for y in range(y0 + int(0.2 * height), y0 + int(0.35 * height))]))
    t_rows = [y for y in range(y0, y0 + int(0.15 * height)) if width.get(y, 0) > 3 * shaft_w]
    blade_rows = []
    for y in range(y1, y0, -1):
        if width.get(y, 0) > 3 * shaft_w:
            blade_rows.append(y)
        elif blade_rows:
            break
    blade_top = min(blade_rows)
    collar = []
    for y in range(blade_top - 1, y0, -1):
        w = width.get(y, 0)
        if 1.2 * shaft_w <= w <= 2.4 * shaft_w:
            collar.append(y)
        elif collar:
            break
    blade_w = max(width[y] for y in blade_rows)
    full = [y for y in blade_rows if width[y] >= 0.9 * blade_w]
    return {
        "blade_w": pct(blade_w),
        "blade_h": pct(y1 - blade_top + 1),
        "shaft_w": pct(shaft_w),
        "t_w": pct(max(width[y] for y in t_rows)),
        "collar_len": pct(len(collar)),
        "shoulder_at": pct(y1 - max(full) + 1) if full else 0.0,
    }


# Round 3：生成器材质 → handheld_texture 画笔
TEXTURE_MAPPING = {
    "iron": "metal_forged_iron",
    "iron_mid": "metal_forged_iron",
    "iron_edge": "metal_iron_edge",
    "rust": "metal_forged_rust_dk",
    "wood": "metal_wood",
    "wood_dk": "metal_wood_dk",
    "pit": "metal_soot",
    "groove": "metal_soot",
}

TEX_CLOSEUPS = [
    ("铲刃正面", 0.0, 0.0, (0, 4.0, 0), 9.0),
    ("铲刃 3/4 与卷边", -35.0, 20.0, (0, 3.0, 0), 8.0),
    ("銎口与T形握把", 35.0, 15.0, (0, 20.0, 0), 12.0),
]

def main() -> None:
    import weapon_check

    kit.REVIEW_ROOT = METAL_REVIEW_ROOT
    rig = build_weapon()
    print(f"{MODEL_NAME}: {len(rig.elements)} 个方块, {len(rig.bones)} 根骨骼")
    weapon_check.write_and_review(sys.modules[__name__], rig)
    weapon_check.write_tex_review(sys.modules[__name__], rig)


if __name__ == "__main__":
    main()
