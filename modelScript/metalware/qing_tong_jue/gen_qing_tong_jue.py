#!/usr/bin/env python3
"""青铜爵 qing_tong_jue Blockbench .bbmodel 程序化生成器 —— Round 1 几何（按参考图重做，整个重写）。

先重出一张干净的严格侧视参考图（带俯视、端视），尺寸按它取：全高 9 px（柱顶到足尖），
全长（流尖到尾尖）约 10 px，腹径 4.4 px：
- 圆鼓的腹（八角层叠成球形）下三只外撇的尖足（足尖外展 2.3 px），腹中一圈窄的云雷纹带（回纹方块）；
- 腹口一圈铜沿，口内是清水（末法：祭器的「酬灵」废了，里面装的是水，不是酒）；
- 一侧长流：平直的槽形（宽 1.8 px，槽口两侧起棱），下沿一道凹弧收向腹颈，流尖略上翘；
- 对侧尖尾：上扬的角形，俯视是三角尖；
- 口上两根柱，柱顶小蘑菇（三级阶梯），立在流与腹相接处两侧；
- 腹侧一只弧形鋬，鋬头是兽首（吻、双眼、两角）；
- 末法：云雷纹带上有几处暗掉的阵线槽，没有发光部件。

坐标：y 向上；腹轴在 x=0,z=0；流朝 +x，尾朝 -x，鋬在 +z 侧（手持时鋬朝向手），
流 / 尾平面是 xy，宽度沿 z。
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

WEAPON_ID = "qing_tong_jue"
MODEL_NAME = "QingTongJue"
OUT_DIR = REPO / "modelScript" / "metalware" / WEAPON_ID
METAL_REVIEW_ROOT = kit.REVIEW_ROOT.parent / "metalware"
REVIEW_DIR = METAL_REVIEW_ROOT / WEAPON_ID
TOTAL_LEN = 9.0                        # 全高（柱顶到足尖）
GRIP_Y = 0.0
POSE = "worn"                          # 手持小件：穿戴栏里画「右手握鋬」
HANDLE_CENTER = (0.0, 5.2, 3.5)        # 鋬弧外侧中点（模型坐标）
WORN_OFFSET = (6.0, 14.64 - 5.2, -4.25 - 3.5)    # 鋬中点落在右拳心 (6, 14.64, -4.25)
HELD_ARM_DEG = 30.0
WORN_SKIP = ("右臂",)
WORN_TITLE = "持握 · 右臂前抬 30°，拳心握鋬，爵立在拳前，流朝右（外侧）"
WORN_VIEWS = [("持握: 正面 FRONT (相机 -z)", 180.0, 0.0), ("持握: 右侧 RIGHT (相机 +x)", -90.0, 0.0),
              ("持握: 3/4 斜视", 145.0, 20.0)]

REF_PNG = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
REF_PANEL_W = 960
REF_AXIS_X = 590
REF_SHADOW_PX = 8

MATS = {
    "bronze": (70, 104, 84),         # 深绿包浆底
    "bronze_dk": (44, 70, 58),
    "bronze_lt": (96, 132, 104),
    "bronze_hi": (180, 138, 84),     # 边棱磨出的铜亮
    "water": (62, 84, 94),           # 清水
    "rope": (150, 124, 84),
    "groove": (22, 30, 28),          # 暗掉的阵线槽 / 回纹凹
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("belly", "root", (0.0, 4.6, 0.0), "圆鼓的腹（八角层叠成球形）+ 腹口铜沿 + 清水"),
    ("belt", "belly", (0.0, 4.95, 0.0), "腹中云雷纹带（回纹方块，几处暗掉的阵线槽）"),
    ("legs", "root", (0.0, 2.0, 0.0), "三只外撇的尖足"),
    ("spout", "belly", (4.0, 7.2, 0.0), "长流：槽形，宽 1.8 px，两侧起棱，流尖上翘"),
    ("tail", "belly", (-2.8, 7.4, 0.0), "尖尾：上扬的角形，俯视三角"),
    ("posts", "belly", (1.75, 7.5, 0.0), "口上两柱，柱顶三级阶梯蘑菇"),
    ("handle", "belly", (0.0, 5.2, 3.0), "弧形鋬 + 兽首"),
]

BELLY_R = [(2.9, 1.5), (3.15, 1.8), (3.5, 1.9), (3.85, 2.02), (4.2, 2.1), (4.6, 2.18), (5.0, 2.22), (5.4, 2.2),
           (5.7, 2.12), (6.0, 2.08)]    # (v, 半径)：参考图腹部侧影量得


def build_weapon() -> Rig:
    rig = Rig(MATS)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent=parent)

    # 1. 腹：自下而上八角环叠成球形；底部收成小碗
    for i, (v0, r) in enumerate((((2.1, 2.5), 0.55), ((2.5, 2.9), 1.1), ((1.95, 2.1), 0.3))):
        kit.octagon_prism(rig, "belly", f"bowl_{i}", 0.0, 0.0, v0[0], v0[1], r, "bronze_dk")
    for i, (v, r) in enumerate(BELLY_R):
        v1 = BELLY_R[i + 1][0] if i + 1 < len(BELLY_R) else 6.3
        kit.octagon_prism(rig, "belly", f"ring_{i:02d}", 0.0, 0.0, v, v1 + 0.02, r, "bronze" if i % 2 == 0 else "bronze_lt")
    # 腹口铜沿 + 清水（口内凹下 0.25）
    kit.octagon_prism(rig, "belly", "rim", 0.0, 0.0, 6.05, 6.4, 2.12, "bronze_hi")
    kit.octagon_prism(rig, "belly", "rim_in", 0.0, 0.0, 6.2, 6.4, 1.9, "bronze_dk")
    kit.octagon_prism(rig, "belly", "water", 0.0, 0.0, 6.3, 6.42, 1.82, "water")

    # 2. 云雷纹带：腹中一圈 1.0 px 高的凸带，带上 16 个回纹方块（暗凹）+ 16 个亮边方块；3 处暗掉的阵线槽
    kit.octagon_prism(rig, "belt", "band", 0.0, 0.0, 4.45, 5.45, 2.3, "bronze_hi")
    kit.octagon_prism(rig, "belt", "band_in", 0.0, 0.0, 4.55, 5.35, 2.33, "bronze")
    for k in range(16):
        ang = math.radians(22.5 * k + 11.25)
        cx, cz = 2.36 * math.cos(ang), 2.36 * math.sin(ang)
        for row, (v, mat, w) in enumerate(((4.62, "groove", 0.2), (5.05, "bronze_hi", 0.22))):
            rig.cube("belt", f"scroll_{k:02d}_{row}", (cx - 0.12, v, cz - w), (cx + 0.12, v + 0.2, cz + w),
                     rot=(0.0, -math.degrees(ang), 0.0), org=(cx, v + 0.1, cz), mat=mat)
    for k, deg in enumerate((50, 170, 290)):
        ang = math.radians(deg)
        cx, cz = 2.4 * math.cos(ang), 2.4 * math.sin(ang)
        rig.cube("belt", f"dead_line_{k}", (cx - 0.04, 4.85, cz - 0.3), (cx + 0.04, 4.93, cz + 0.3),
                 rot=(0.0, -math.degrees(ang), 0.0), org=(cx, 4.89, cz), mat="groove")

    # 3. 三足：外撇的尖足，各 6 段，足根在腹底、足尖外展 2.3 px
    for k, deg in enumerate((90, 210, 330)):
        ang = math.radians(deg)
        pts = []
        for t in range(7):
            f = t / 6
            rad = 1.45 + 1.35 * f
            pts.append((rad * math.cos(ang), 3.1 - 3.1 * f, rad * math.sin(ang)))
        rig.taper("legs", f"leg{k}", pts, [0.5, 0.46, 0.4, 0.34, 0.27, 0.21, 0.15], mat="bronze" if k else "bronze_lt")
        rig.cube("legs", f"leg_cap_{k}", (pts[0][0] - 0.55, 2.95, pts[0][2] - 0.55), (pts[0][0] + 0.55, 3.25, pts[0][2] + 0.55), mat="bronze_hi")

    # 4. 流与尾：取自参考图侧视剪影的厚片（xy 平面，宽度沿 z）
    mask = kit.ref_mask(REF_PNG, REF_PANEL_W, shadow_px=REF_SHADOW_PX)
    frame = kit.frame_from_mask(mask, TOTAL_LEN, REF_AXIS_X, u_sign=-1)
    cover = kit.sampler(mask, frame)

    def spout_part(u: float, v: float) -> bool:
        return u > 1.95 and 5.95 < v < 8.5 and not (u < 2.6 and v > 7.35)

    def tail_part(u: float, v: float) -> bool:
        return u < -1.95 and 5.95 < v < 8.5

    spout_spec = kit.PlateSpec(plane="xy", cell=0.2, half_edge=0.9, half_max=0.9, ramp=0.4, levels=1)
    kit.plate_from_mask(rig, "spout", "spout", cover, (1.9, 6.6, 5.9, 8.5), spout_spec, "bronze", "bronze", keep=spout_part)
    tail_spec = kit.PlateSpec(plane="xy", cell=0.2, half_edge=0.95, half_max=0.95, ramp=0.4, levels=1,
                              thick_fn=lambda u, v: max(0.12, 1.0 - (-u - 2.0) / 2.0))
    kit.plate_from_mask(rig, "tail", "tail", cover, (-4.3, -1.9, 5.9, 8.5), tail_spec, "bronze", "bronze_lt", keep=tail_part)

    # 流槽：沿流上沿两侧起棱（亮铜），中间一条清水槽；尾尖加一点亮边
    def top_of(u: float) -> float:
        v = 8.6
        while v > 6.0 and cover(u - 0.05, v - 0.1, 0.1) < 0.5:
            v -= 0.1
        return v
    for i in range(18):
        u0 = 2.0 + i * 0.235
        t = top_of(u0 + 0.1)
        rig.cube("spout", f"wall_l_{i:02d}", (u0, t - 0.25, 0.64), (u0 + 0.24, t + 0.12, 0.92), mat="bronze_hi")
        rig.cube("spout", f"wall_r_{i:02d}", (u0, t - 0.25, -0.92), (u0 + 0.24, t + 0.12, -0.64), mat="bronze_hi")
        rig.cube("spout", f"channel_{i:02d}", (u0, t - 0.12, -0.64), (u0 + 0.24, t + 0.04, 0.64), mat="water" if i > 2 else "groove")
    for i in range(6):
        u0 = -2.05 - i * 0.33
        t = top_of(u0 - 0.1)
        w = max(0.1, 0.95 * (1.0 - (-u0 - 2.0) / 2.0))
        rig.cube("tail", f"tail_edge_{i}", (u0 - 0.3, t - 0.15, -w * 0.5), (u0, t + 0.08, w * 0.5), mat="bronze_hi")

    # 5. 两柱：立在流与腹相接处两侧，柱顶三级阶梯蘑菇
    for k, z in enumerate((-1.15, 1.15)):
        kit.octagon_prism(rig, "posts", f"stem_{k}", 1.75, z, 6.3, 8.2, 0.2, "bronze")
        for j, (v0, v1, r) in enumerate(((8.2, 8.45, 0.42), (8.45, 8.7, 0.32), (8.7, 9.0, 0.2))):
            kit.octagon_prism(rig, "posts", f"cap_{k}_{j}", 1.75, z, v0, v1, r, "bronze_hi" if j == 2 else "bronze_lt")

    # 6. 鋬：弧形（yz 平面，向 +z 鼓出），上端是兽首
    arc = [(5.95, 2.1), (6.35, 2.65), (6.25, 3.2), (5.7, 3.6), (4.9, 3.6), (4.2, 3.2), (3.95, 2.6), (4.0, 2.05)]
    pts = [(0.0, v, w) for v, w in arc]
    kit.strip_along(rig, "handle", "arc", pts, [0.3] * len(pts), [0.27] * len(pts), "bronze", up=(1.0, 0.0, 0.0))
    for i, (v, w) in enumerate(arc[1:-1]):
        rig.cube("handle", f"arc_hi_{i}", (-0.34, v - 0.08, w - 0.04), (-0.28, v + 0.08, w + 0.04), mat="bronze_hi")
    # 兽首：吻、额、双眼、双角、下颌
    rig.cube("handle", "head_snout", (-0.42, 6.0, 2.7), (0.42, 6.55, 3.55), mat="bronze_lt")
    rig.cube("handle", "head_brow", (-0.45, 6.5, 2.8), (0.45, 6.85, 3.3), mat="bronze")
    rig.cube("handle", "head_jaw", (-0.34, 5.8, 2.9), (0.34, 6.05, 3.5), mat="bronze_dk")
    for k, x in enumerate((-0.32, 0.32)):
        rig.cube("handle", f"head_eye_{k}", (x - 0.07, 6.48, 3.5), (x + 0.07, 6.62, 3.6), mat="groove")
        rig.cube("handle", f"head_horn_{k}", (x - 0.07, 6.85, 2.9), (x + 0.07, 7.2, 3.05), mat="bronze_hi")
    # 鋬与腹相接处的两块圆片 + 草绳缠一圈（手握处，被磨亮）
    for k, v in enumerate((6.0, 4.05)):
        rig.cube("handle", f"joint_{k}", (-0.4, v - 0.22, 2.0), (0.4, v + 0.22, 2.3), mat="bronze_hi")
    for i in range(3):
        v = 5.0 + i * 0.3
        rig.cube("handle", f"wrap_{i}", (-0.36, v, 3.3), (0.36, v + 0.22, 3.75), mat="rope")
    return rig


TITLE = "#2 青铜爵 qing_tong_jue (Round 1)"
PROP_YAW = 180.0     # 相机在 -z：流（+x）在画面左，鋬（+z）在对侧被腹挡住，与参考图同向

METRICS = [
    ("全长（流尖到尾尖）/ 全高", "length"),
    ("腹宽（腹最宽处）/ 全高", "belly_w"),
    ("流长（腹轴到流尖）/ 全高", "spout_len"),
    ("尾长（腹轴到尾尖）/ 全高", "tail_len"),
    ("足尖外展宽 / 全高", "foot_w"),
    ("腹顶（腹口）高度 / 全高", "rim_h"),
    ("流尖高度 / 全高", "spout_tip_h"),
]


def measure(mask: np.ndarray) -> dict[str, float]:
    """从严格侧视剪影量爵的百分比（流朝左）。参考图和模型渲染用同一份。

    腹轴 = 全高 45% 处（腹最宽的那一行附近）外廓中心；足尖外展 = 全高 3% 处外廓宽；
    腹口高度 = 腹轴所在列最上方连续段的下端（柱子之下的腹口）。
    """
    x0, y0, x1, y1 = kit.mask_bbox(mask)
    height = y1 - y0 + 1
    pct = lambda v: 100.0 * v / height  # noqa: E731
    row = lambda frac: int(round(y1 - frac * (height - 1)))  # noqa: E731
    xs = np.where(mask[row(0.45)])[0]
    axis = (xs.min() + xs.max()) / 2
    belly_w = max(np.where(mask[row(f)])[0].max() - np.where(mask[row(f)])[0].min() + 1 for f in (0.36, 0.42, 0.48, 0.54))
    foot = np.where(mask[row(0.03)])[0]
    col = np.where(mask[:, int(axis)])[0]
    spout_tip_col = np.where(mask[:, x0 + 4])[0]
    top = col.min()    # 腹轴那一列的最高点 = 腹口沿（两柱不在腹轴上）
    return {
        "length": pct(x1 - x0 + 1),
        "belly_w": pct(belly_w),
        "spout_len": pct(axis - x0),
        "tail_len": pct(x1 - axis),
        "foot_w": pct(foot.max() - foot.min() + 1),
        "rim_h": pct(y1 - top + 1),
        "spout_tip_h": pct(y1 - spout_tip_col.min() + 1),
    }


def main() -> None:
    import weapon_check

    kit.REVIEW_ROOT = METAL_REVIEW_ROOT
    rig = build_weapon()
    print(f"{MODEL_NAME}: {len(rig.elements)} 个方块, {len(rig.bones)} 根骨骼")
    weapon_check.write_and_review(sys.modules[__name__], rig)


if __name__ == "__main__":
    main()
