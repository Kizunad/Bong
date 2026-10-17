#!/usr/bin/env python3
"""青铜戈 qing_tong_ge Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

形制（对照参考图 ``qing_tong_ge_ref.png`` 的主视图剪影，戈头轮廓直接取自剪影）：
- 长木杆竖直，全长 40 px；杆径只有全长的 2.8%（约 1.1 px），不是任务卡的 1.6；
- 戈头与杆垂直：前伸的「援」约 10.6 px、一道中脊两侧斜刃；援根下沿沿杆下垂成「胡」，
  长约 5.8 px、前凸 1.7 px，开三个穿绳孔，每孔穿一圈麻绳绑在杆上；
  后伸的「内」是短方尾，带一个小方孔；
- 杆顶小铜冒，杆尾是 6 px 长的锥形铜镦，镦口一圈铜箍；
- 末法：胡上原本刻的导灵阵线还在，但是暗槽，没有任何发光部件。

坐标：y 向上，杆轴在 x=0,z=0；援朝 -z（手持时朝前），内朝 +z；戈头平面是 zy，片厚沿 x。
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "modelScript" / "tools"))
sys.path.insert(0, str(REPO / "modelScript" / "gear" / "_kit"))
sys.path.insert(0, str(REPO / "modelScript" / "weapons"))

from bbmodel_maker.rig.rigkit import Rig  # noqa: E402

import weapon_shape_kit as kit  # noqa: E402

WEAPON_ID = "qing_tong_ge"
MODEL_NAME = "QingTongGe"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
TOTAL_LEN = 40.0
GRIP_Y = 14.0
HAFT_R = 0.56   # 杆内切半径，杆径 1.12 px 取自参考图（占全长 2.8%）

REF_PNG = kit.REVIEW_ROOT / WEAPON_ID / f"{WEAPON_ID}_ref.png"
REF_PANEL_W = 830
REF_AXIS_X = 324

MATS = {
    "bronze": (146, 92, 54),        # 红褐铜底
    "bronze_dk": (104, 64, 40),     # 铜件暗面 / 阴影
    "bronze_edge": (190, 142, 88),  # 刃口与握持处磨亮
    "verdigris": (72, 120, 98),     # 青绿锈斑
    "wood": (98, 68, 46),           # 杆身包浆
    "wood_dk": (70, 47, 32),
    "cord": (150, 122, 80),         # 麻绳
    "groove": (26, 20, 17),         # 暗掉的阵线槽 / 铭文位 / 穿绳孔内壁
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("butt_spike", "root", (0.0, 0.0, 0.0), "铜镦锥尖（长 4 px，由 0.25 px 逐级收分到 1.37 px）"),
    ("butt_collar", "root", (0.0, 4.0, 0.0), "铜镦箍（长 2 px，青绿锈斑）"),
    ("haft", "root", (0.0, 6.0, 0.0), "八角近圆木杆（径 1.12 px，握处磨亮）"),
    ("haft_wrap", "haft", (0.0, 33.0, 0.0), "杆上绑戈头的三圈麻绳"),
    ("ge_head", "root", (0.0, 34.0, 0.0), "戈头（援 + 胡 + 内，zy 平面楔形片）"),
    ("yuan", "ge_head", (0.0, 37.3, -0.6), "援：前伸尖刃，中脊两侧斜刃"),
    ("hu", "ge_head", (0.0, 33.5, -0.6), "胡：沿杆下垂的弧刃，三个穿绳孔"),
    ("nei", "ge_head", (0.0, 37.2, 0.6), "内：后伸短方尾，带一个方孔"),
    ("head_marks", "ge_head", (0.0, 36.0, 0.0), "胡上暗掉的导灵阵线 + 援根铭文位（暗槽，不发光）"),
    ("top_cap", "root", (0.0, 39.5, 0.0), "杆顶小铜冒"),
]

HU_HOLE_V = (35.2, 33.5, 31.8)   # 穿绳孔的中心高度
HU_HOLE_U = (-1.25, -0.72)       # 孔在 z 向的范围（紧贴杆前面）


def build_weapon() -> Rig:
    rig = Rig(MATS)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent=parent)

    # 1. 铜镦：由尖逐级放粗的锥，接一道铜箍
    cone = [(0.0, 0.7, 0.13), (0.7, 1.4, 0.2), (1.4, 2.1, 0.28), (2.1, 2.8, 0.36),
            (2.8, 3.4, 0.45), (3.4, 4.0, 0.55)]
    for i, (y0, y1, r) in enumerate(cone):
        kit.octagon_prism(rig, "butt_spike", f"spike_{i}", 0.0, 0.0, y0, y1, r, "bronze")
    kit.octagon_prism(rig, "butt_collar", "collar", 0.0, 0.0, 4.0, 6.0, 0.68, "bronze")
    kit.octagon_prism(rig, "butt_collar", "collar_lip_lo", 0.0, 0.0, 4.0, 4.25, 0.72, "bronze_dk")
    kit.octagon_prism(rig, "butt_collar", "collar_lip_hi", 0.0, 0.0, 5.75, 6.0, 0.72, "bronze_dk")
    for i, (y, h) in enumerate(((4.45, 0.5), (5.0, 0.35))):
        rig.cube("butt_collar", f"collar_patina_{i}", (0.2 - i * 0.5, y, 0.5), (0.6 - i * 0.5, y + h, 0.74), mat="verdigris")

    # 2. 杆：八角近圆，握处（v 13–19）颜色磨亮，其余深褐包浆；每 3 px 一道细包浆线
    kit.octagon_prism(rig, "haft", "haft_lo", 0.0, 0.0, 6.0, 13.0, HAFT_R, "wood")
    kit.octagon_prism(rig, "haft", "haft_grip", 0.0, 0.0, 13.0, 19.0, HAFT_R + 0.02, "wood_dk")
    kit.octagon_prism(rig, "haft", "haft_hi", 0.0, 0.0, 19.0, 39.5, HAFT_R, "wood")
    for i, y in enumerate((9.0, 22.0, 26.5, 30.0)):
        kit.octagon_prism(rig, "haft", f"haft_ring_{i}", 0.0, 0.0, y, y + 0.18, HAFT_R + 0.04, "wood_dk")

    # 3. 戈头：援 + 胡 取剪影里杆右侧的部分，内取杆左侧的部分，片厚沿 x
    mask = kit.ref_mask(REF_PNG, REF_PANEL_W)
    frame = kit.frame_from_mask(mask, TOTAL_LEN, REF_AXIS_X, u_sign=-1)
    cover = kit.sampler(mask, frame)

    def front_part(u: float, v: float) -> bool:
        if u > -HAFT_R + 0.04 or v < 29.5 or v > 38.4:
            return False
        for hv in HU_HOLE_V:
            if HU_HOLE_U[0] < u < HU_HOLE_U[1] and abs(v - hv) < 0.4:
                return False
        return True

    spec = kit.PlateSpec(plane="zy", cell=0.2, half_edge=0.06, half_max=0.19, ramp=0.7, levels=4)
    kit.plate_from_mask(rig, "yuan", "yuan", cover, (-11.4, -0.4, 36.2, 38.6), spec,
                        "bronze_edge", "bronze", keep=lambda u, v: front_part(u, v) and v > 36.2)
    kit.plate_from_mask(rig, "hu", "hu", cover, (-3.6, -0.4, 29.8, 36.4), spec,
                        "bronze_edge", "bronze", keep=lambda u, v: front_part(u, v) and v <= 36.2)

    def rear_part(u: float, v: float) -> bool:
        return u > HAFT_R - 0.04 and v > 36.0
    nei_spec = kit.PlateSpec(plane="zy", cell=0.2, half_edge=0.1, half_max=0.2, ramp=0.5, levels=3)
    kit.plate_from_mask(rig, "nei", "nei", cover, (0.4, 2.6, 36.0, 38.4), nei_spec,
                        "bronze", "bronze", keep=rear_part)
    # 内上的方孔（暗槽）
    rig.cube("nei", "nei_hole", (-0.22, 37.15, 1.2), (0.22, 37.55, 1.6), mat="groove")

    # 4. 麻绳：每孔一圈绕杆，穿过胡孔
    for i, hv in enumerate(HU_HOLE_V):
        kit.octagon_prism(rig, "haft_wrap", f"wrap_{i}", 0.0, 0.0, hv - 0.28, hv + 0.28, HAFT_R + 0.2, "cord")
        rig.cube("haft_wrap", f"wrap_through_{i}", (-0.18, hv - 0.24, HU_HOLE_U[0]), (0.18, hv + 0.24, -HAFT_R), mat="cord")
        rig.cube("haft_wrap", f"wrap_knot_{i}", (-0.16, hv - 0.12, HAFT_R + 0.18), (0.16, hv + 0.12, HAFT_R + 0.3), mat="cord")
    # 内与杆之间的捆扎
    kit.octagon_prism(rig, "haft_wrap", "wrap_nei", 0.0, 0.0, 36.1, 36.7, HAFT_R + 0.18, "cord")

    # 5. 暗掉的阵线：胡前凸最宽处的凹槽 + 援根铭文位（方形空匾）
    for i, (v, w) in enumerate(((35.9, 0.9), (34.3, 0.7), (32.9, 0.9), (31.3, 0.7))):
        rig.cube("head_marks", f"hu_line_{i}", (0.1, v, -2.9), (0.2, v + 0.16, -2.9 + w), mat="groove")
        rig.cube("head_marks", f"hu_line_b_{i}", (-0.2, v, -2.9), (-0.1, v + 0.16, -2.9 + w), mat="groove")
    rig.cube("head_marks", "cartouche_a", (0.12, 37.0, -3.6), (0.22, 37.5, -2.2), mat="groove")
    rig.cube("head_marks", "cartouche_b", (-0.22, 37.0, -3.6), (-0.12, 37.5, -2.2), mat="groove")
    # 青绿锈斑
    for i, (x, v, z) in enumerate(((0.2, 36.7, -5.8), (-0.2, 36.5, -7.4), (0.2, 31.0, -1.8), (-0.2, 33.2, -3.0))):
        rig.cube("head_marks", f"patina_{i}", (x - 0.02, v, z), (x + 0.02, v + 0.3, z + 0.6), mat="verdigris")

    # 6. 杆顶铜冒：小圆顶
    kit.octagon_prism(rig, "top_cap", "cap_base", 0.0, 0.0, 38.4, 39.2, HAFT_R + 0.08, "bronze")
    kit.octagon_prism(rig, "top_cap", "cap_lip", 0.0, 0.0, 38.4, 38.6, HAFT_R + 0.12, "bronze_dk")
    kit.octagon_prism(rig, "top_cap", "cap_dome", 0.0, 0.0, 39.2, 39.7, 0.4, "bronze")
    kit.octagon_prism(rig, "top_cap", "cap_tip", 0.0, 0.0, 39.7, 40.0, 0.17, "bronze_edge")
    return rig


TITLE = "#8 青铜戈 qing_tong_ge (Round 1)"
POSE = "polearm"
PROP_YAW = -90.0  # 侧视，援朝右，与参考图主视图同向

METRICS = [
    ("援长 / 全长（援根到援尖）", "yuan_len"),
    ("胡长 / 全长（援根下沿到胡底）", "hu_len"),
    ("戈头总高 / 全长", "head_h"),
    ("援根高 / 全长", "yuan_base_h"),
    ("内长 / 全长（杆后沿向后）", "nei_len"),
    ("杆径 / 全长", "haft_w"),
    ("铜镦箍长 / 全长", "collar"),
    ("铜镦锥长 / 全长", "spike"),
]


def measure(mask: np.ndarray) -> dict[str, float]:
    """从侧视剪影量戈的各部件占全长的百分比（援朝右）。参考图和模型渲染用同一份。"""
    x0, y0, x1, y1 = kit.mask_bbox(mask)
    rows = range(y0, y1 + 1)
    height = y1 - y0 + 1
    ext = {}
    for y in rows:
        xs = np.where(mask[y])[0]
        if len(xs):
            ext[y] = (int(xs.min()), int(xs.max()))
    mid = [y for y in rows if y0 + 0.30 * height <= y <= y0 + 0.70 * height and y in ext]
    haft_w = float(np.median([ext[y][1] - ext[y][0] + 1 for y in mid]))
    axis = float(np.median([(ext[y][0] + ext[y][1]) / 2 for y in mid]))
    right, left = axis + haft_w / 2, axis - haft_w / 2

    top_rows = [y for y in rows if y <= y0 + 0.30 * height and y in ext]
    protrude = {y: ext[y][1] - right for y in top_rows}
    head_rows = [y for y in top_rows if protrude[y] > 0.35 * haft_w or left - ext[y][0] > 0.35 * haft_w]
    head_top, hu_bottom = min(head_rows), max(y for y in top_rows if protrude[y] > 0.5 * haft_w)
    yuan_bottom = max(y for y in top_rows if protrude[y] > 3.0 * haft_w)
    yuan_tip = max(ext[y][1] for y in top_rows)
    nei = left - min(ext[y][0] for y in head_rows)
    # 援根高：援根右侧半根杆径处那一列的竖直范围
    col = int(right + 0.8 * haft_w)
    col_rows = np.where(mask[y0:int(y0 + 0.30 * height), col])[0]
    base_h = float(col_rows.max() - col_rows.min() + 1) if len(col_rows) else 0.0

    bottom_rows = [y for y in rows if y >= y0 + 0.70 * height and y in ext]
    wide = [y for y in bottom_rows if ext[y][1] - ext[y][0] + 1 >= 1.15 * haft_w]
    collar_top, collar_bot = (min(wide), max(wide)) if wide else (y1, y1)
    run = [y for y in sorted(wide) if y >= collar_top]
    pct = lambda v: 100.0 * v / height  # noqa: E731
    return {
        "yuan_len": pct(yuan_tip - right),
        "hu_len": pct(hu_bottom - yuan_bottom),
        "head_h": pct(hu_bottom - head_top),
        "yuan_base_h": pct(base_h),
        "nei_len": pct(nei),
        "haft_w": pct(haft_w),
        "collar": pct(len(run)),
        "spike": pct(y1 - collar_top),
    }


def main() -> None:
    import weapon_check

    rig = build_weapon()
    print(f"{MODEL_NAME}: {len(rig.elements)} 个方块, {len(rig.bones)} 根骨骼")
    weapon_check.write_and_review(sys.modules[__name__], rig)


if __name__ == "__main__":
    main()
