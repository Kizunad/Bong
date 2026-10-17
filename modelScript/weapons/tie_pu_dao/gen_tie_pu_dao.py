#!/usr/bin/env python3
"""铁朴刀 tie_pu_dao Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

形制（对照参考图 ``tie_pu_dao_ref.png`` 的主视图剪影，刀身轮廓直接取自剪影）：
- 双手长柄刀，全长 40 px：刀身 16.9（占 42%）、柄 20.2（占 50%）、铁镦 2.9；柄比刀身长；
- 刀背是直的，刀刃在前段上挑成弧：刃腹最凸点在刃长 74% 处，刀尖向后翘起，刀头最宽 4.5 px（比参考图剪影再宽 1.5 倍）；
  刀身厚度是刃宽的约 22%，由刀背向刃口逐级楔形减薄，刃口 0.12 px；
- 铁盘护手（八角盘，径 3.2、厚 0.6），木柄径 1.4，两段缠绳，柄尾铁镦（八角、尾端倒棱）；
- 末法：刀身下段原本刻的导灵阵线是暗槽，没有任何发光部件。

坐标：y 向上，柄轴在 x=0,z=0；刀刃朝 -x，刀背在 +x，刀身平面是 xy，厚度沿 z。
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

WEAPON_ID = "tie_pu_dao"
MODEL_NAME = "TiePuDao"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
TOTAL_LEN = 40.0
GRIP_Y = 13.0
HILT_R = 0.7        # 柄径 1.4 px，取自参考图（占全长 3.5%）

REF_PNG = kit.REVIEW_ROOT / WEAPON_ID / f"{WEAPON_ID}_ref.png"
REF_PANEL_W = 845
REF_AXIS_X = 429

GUARD_Y0, GUARD_Y1 = 23.1, 23.75
SPINE_U = 0.84       # 刀背（平背）的 x，参考图量得
WIDEN = 1.5          # 刀头最宽处相对参考图剪影再加宽的倍数：3.0 px → 4.5 px
WIDEN_FROM, WIDEN_TO = 25.5, 34.0   # 从 v=25.5 起渐宽，到 v=34 达最宽
BUTT_TOP = 2.9

MATS = {
    "iron_dark": (48, 44, 42),      # 黑褐氧化膜
    "iron_mid": (78, 72, 68),       # 锻打铁身
    "iron_edge": (148, 142, 138),   # 刃口磨出的冷灰亮线
    "rust": (112, 62, 40),          # 局部红锈
    "wood": (92, 64, 42),
    "wood_dk": (68, 46, 30),
    "cord": (150, 122, 80),         # 缠绳
    "groove": (24, 22, 20),         # 暗掉的阵线槽
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("butt_cap", "root", (0.0, 1.5, 0.0), "柄尾铁镦（长 2.9 px，八角，尾端倒棱）"),
    ("hilt", "root", (0.0, 13.0, 0.0), "八角近圆木柄（径 1.4 px，握处磨亮）"),
    ("hilt_wrap_low", "hilt", (0.0, 7.7, 0.0), "下段缠绳（柄尾上方）"),
    ("hilt_wrap_high", "hilt", (0.0, 20.8, 0.0), "上段缠绳（护手下方）"),
    ("guard_disc", "root", (0.0, 23.4, 0.0), "铁盘护手（八角盘，径 3.2、厚 0.65）"),
    ("blade", "root", (0.0, 32.0, 0.0), "刀身：背直刃弧，刀头上挑，xy 平面楔形片"),
    ("blade_marks", "blade", (0.0, 28.0, 0.0), "刀身下段暗掉的阵线槽 + 锻打锈斑"),
]


def build_weapon() -> Rig:
    rig = Rig(MATS)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent=parent)

    # 1. 柄尾铁镦：八角铁帽，尾端两级倒棱，顶上一道压边
    kit.octagon_prism(rig, "butt_cap", "cap_bevel", 0.0, 0.0, 0.0, 0.45, 0.62, "iron_mid")
    kit.octagon_prism(rig, "butt_cap", "cap_body", 0.0, 0.0, 0.45, 2.7, 1.2, "iron_dark")
    kit.octagon_prism(rig, "butt_cap", "cap_lip", 0.0, 0.0, 2.45, BUTT_TOP, 1.27, "iron_mid")
    kit.octagon_prism(rig, "butt_cap", "cap_edge", 0.0, 0.0, 0.45, 0.7, 1.24, "iron_edge")
    rig.cube("butt_cap", "cap_rust", (0.5, 1.2, 1.2), (1.0, 1.7, 1.28), mat="rust")

    # 2. 柄：八角木柄，握处（v 9–19）磨亮
    kit.octagon_prism(rig, "hilt", "hilt_lo", 0.0, 0.0, BUTT_TOP, 9.0, HILT_R, "wood")
    kit.octagon_prism(rig, "hilt", "hilt_grip", 0.0, 0.0, 9.0, 19.5, HILT_R + 0.02, "wood_dk")
    kit.octagon_prism(rig, "hilt", "hilt_hi", 0.0, 0.0, 19.5, GUARD_Y0, HILT_R, "wood")
    # 两段缠绳：下段 6.6–8.8，上段 19.6–22.2（参考图）
    for i, y in enumerate((6.7, 7.2, 7.7, 8.2)):
        kit.octagon_prism(rig, "hilt_wrap_low", f"wrapl_{i}", 0.0, 0.0, y, y + 0.5, HILT_R + 0.1, "cord")
    for i, y in enumerate((19.6, 20.15, 20.7, 21.25, 21.8)):
        kit.octagon_prism(rig, "hilt_wrap_high", f"wraph_{i}", 0.0, 0.0, y, y + 0.55, HILT_R + 0.1, "cord")

    # 3. 铁盘护手：八角盘，上下各一圈薄边
    kit.octagon_prism(rig, "guard_disc", "disc", 0.0, 0.0, GUARD_Y0, GUARD_Y1, 1.55, "iron_dark")
    kit.octagon_prism(rig, "guard_disc", "disc_rim_lo", 0.0, 0.0, GUARD_Y0, GUARD_Y0 + 0.14, 1.62, "iron_mid")
    kit.octagon_prism(rig, "guard_disc", "disc_rim_hi", 0.0, 0.0, GUARD_Y1 - 0.14, GUARD_Y1, 1.62, "iron_edge")
    rig.cube("guard_disc", "disc_rust", (-1.0, GUARD_Y0 + 0.1, 1.5), (-0.4, GUARD_Y1 - 0.1, 1.68), mat="rust")

    # 4. 刀身：取自剪影、前段再加宽的楔形片。刀刃在 -x，刀背在 +x（平背），厚度沿 z
    mask = kit.ref_mask(REF_PNG, REF_PANEL_W)
    frame = kit.frame_from_mask(mask, TOTAL_LEN, REF_AXIS_X)
    ref_cover = kit.sampler(mask, frame)

    def cover(u: float, v: float, size: float) -> float:
        """刀身前段向刃侧加宽：以刀背（u = SPINE_U）为轴，宽度系数由刀根的 1.0 升到刀头的 WIDEN。"""
        t = min(1.0, max(0.0, (v + size / 2 - WIDEN_FROM) / (WIDEN_TO - WIDEN_FROM)))
        f = 1.0 + (WIDEN - 1.0) * t * t * (3 - 2 * t)
        u_src = SPINE_U - (SPINE_U - (u + size / 2)) / f
        return ref_cover(u_src - size / 2, v, size)

    spec = kit.PlateSpec(plane="xy", cell=0.2, half_edge=0.06, half_max=0.5, ramp=2.6, levels=6,
                         edge_mode="left")
    kit.plate_from_mask(rig, "blade", "blade", cover, (-4.4, 1.7, GUARD_Y1 - 0.15, 40.2), spec,
                        "iron_edge", "iron_mid", keep=lambda u, v: v >= GUARD_Y1 - 0.15)
    # 刀背压条：一条比刀身略厚的深色脊，沿平背通长
    rig.cube("blade", "spine_bar", (0.7, GUARD_Y1, -0.4), (0.9, 35.6, 0.4), mat="iron_dark")
    rig.cube("blade", "spine_bar_hi", (0.72, 35.6, -0.36), (0.9, 36.2, 0.36), mat="iron_dark")

    # 5. 暗掉的阵线（刀身下段近背处三条浅暗槽）+ 锻打锈斑
    for i, (v, ln) in enumerate(((25.2, 1.6), (27.4, 1.2), (29.2, 1.0))):
        rig.cube("blade_marks", f"mark_{i}", (-0.5, v, 0.36), (-0.5 + 0.12, v + ln, 0.4), mat="groove")
        rig.cube("blade_marks", f"mark_b_{i}", (-0.5, v, -0.4), (-0.5 + 0.12, v + ln, -0.36), mat="groove")
    for i, (x, v) in enumerate(((-1.2, 31.5), (-0.8, 28.0), (-0.2, 33.6), (-1.6, 34.4), (0.2, 26.1))):
        rig.cube("blade_marks", f"rust_{i}", (x, v, 0.34), (x + 0.45, v + 0.35, 0.37), mat="rust")
        rig.cube("blade_marks", f"rust_b_{i}", (x + 0.2, v + 0.8, -0.37), (x + 0.6, v + 1.1, -0.34), mat="rust")
    return rig


TITLE = "#10 铁朴刀 tie_pu_dao (Round 1)"
POSE = "polearm"
PROP_YAW = 0.0     # 相机在 +z，刀刃（-x）在画面左，与参考图同向

METRICS = [
    ("刀身长（护手到刀尖）/ 全长", "blade_len"),
    ("柄长（镦上沿到护手）/ 全长", "hilt_len"),
    ("刃腹最凸点位置（占刀身长，自护手起）", "belly_pos"),
    ("刀身最宽 / 全长", "blade_w"),
    ("护手宽 / 全长", "guard_w"),
    ("柄径 / 全长", "hilt_w"),
    ("铁镦宽 / 全长", "cap_w"),
]


def measure(mask: np.ndarray) -> dict[str, float]:
    """从正视剪影量朴刀各部件占全长的百分比。参考图和模型渲染用同一份。

    柄宽取 40%~60%（从下数 8~16 px 一带，刚好避开缠绳）中位数；护手下沿 = 从柄往上第一行
    宽度 ≥ 1.8 倍柄宽（护手盘往上数到宽度回落到 1.8 倍以下为护手上沿，刀身从那里算起）；
    铁镦上沿 = 底部 12% 内宽度 ≥ 1.3 倍柄宽的最高一行。
    """
    x0, y0, x1, y1 = kit.mask_bbox(mask)
    height = y1 - y0 + 1
    ext = {}
    for y in range(y0, y1 + 1):
        xs = np.where(mask[y])[0]
        if len(xs):
            ext[y] = (int(xs.min()), int(xs.max()))
    width = lambda y: ext[y][1] - ext[y][0] + 1  # noqa: E731
    mid = [y for y in ext if y1 - 0.40 * height <= y <= y1 - 0.20 * height]
    hilt_w = float(np.median([width(y) for y in mid]))
    guard_bottom = next(y for y in range(y1 - int(0.45 * height), y0, -1) if width(y) >= 1.8 * hilt_w)
    cap_rows = [y for y in ext if y >= y1 - 0.12 * height and width(y) >= 1.3 * hilt_w]
    cap_top = min(cap_rows)
    guard_top = guard_bottom
    while guard_top - 1 in ext and width(guard_top - 1) >= 1.8 * hilt_w:
        guard_top -= 1
    blade_rows = [y for y in ext if y < guard_top]
    left_most = min(ext[y][0] for y in blade_rows)
    belly_row = max(y for y in blade_rows if ext[y][0] <= left_most + 0.01 * height)
    blade_len = guard_top - y0
    pct = lambda v: 100.0 * v / height  # noqa: E731
    return {
        "blade_len": pct(blade_len),
        "hilt_len": pct(cap_top - guard_bottom),
        "belly_pos": 100.0 * (guard_top - belly_row) / blade_len,
        "blade_w": pct(max(width(y) for y in blade_rows)),
        "guard_w": pct(max(width(y) for y in range(guard_bottom - 3, guard_bottom + 1))),
        "hilt_w": pct(hilt_w),
        "cap_w": pct(max(width(y) for y in cap_rows)),
    }


# Round 3：生成器材质 → handheld_texture 画笔
TEXTURE_MAPPING = {
    "iron_dark": "weapon_fe_dark",
    "iron_mid": "weapon_fe_plate",
    "iron_edge": "weapon_fe_edge",
    "rust": "weapon_fe_rust",
    "wood": "weapon_wd_patina",
    "wood_dk": "weapon_wd_worn",
    "cord": "weapon_cord_tan",
    "groove": "weapon_dead_groove",
}

TEX_CLOSEUPS = [
    ("刀身 正视", 0.0, 0.0, (-0.8, 32.0, 0), 18.0),
    ("护手与缠绳", 25.0, 18.0, (0, 22.5, 0), 9.0),
    ("铁镦", 25.0, 12.0, (0, 2.0, 0), 7.0),
]

def main() -> None:
    import weapon_check

    rig = build_weapon()
    print(f"{MODEL_NAME}: {len(rig.elements)} 个方块, {len(rig.bones)} 根骨骼")
    weapon_check.write_and_review(sys.modules[__name__], rig)
    weapon_check.write_tex_review(sys.modules[__name__], rig)


if __name__ == "__main__":
    main()
