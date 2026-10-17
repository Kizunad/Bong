#!/usr/bin/env python3
"""青铜钺 qing_tong_yue Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

形制（对照参考图 ``qing_tong_yue_ref.png`` 的主视图剪影，钺身轮廓直接取自剪影）：
- 全长 24 px：杆尾铜镦 3.2、八角木柄 12.6（含钺身下的铜箍）、钺身高 6.4、方内露出 3.6；
- 钺身是对称的新月形宽刃，宽 11.9 px（占全长 49.6%）、两端上翘成尖角，中间一个圆孔；
  刃面（正反两面）各有一张浮雕兽面（双目、鼻梁、眉纹、颊纹），片厚只有 0.6 px，刃口逐级减薄；
- 方内穿过钺身顶部，顶上一个小铜冒；柄尾铜镦四面各有一块方回纹暗槽；
- 末法：兽面纹槽里原本的导灵阵线是暗槽，眼珠凹槽不发光。

坐标：y 向上；钺身平面是 xy，片厚沿 z；柄轴在 x=0,z=0。
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

WEAPON_ID = "qing_tong_yue"
MODEL_NAME = "QingTongYue"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
TOTAL_LEN = 24.0
GRIP_Y = 9.0
HANDLE_R = 0.85   # 柄径 1.7 px，取自参考图（占全长 7%）

REF_PNG = kit.REVIEW_ROOT / WEAPON_ID / f"{WEAPON_ID}_ref.png"
REF_PANEL_W = 800
REF_AXIS_X = 423

BLADE_Y0 = 15.8     # 钺身下缘（最低点）
BLADE_TOP = 22.8    # 钺角尖
HOLE_V = 19.5

MATS = {
    "bronze": (146, 92, 54),        # 红褐铜底
    "bronze_dk": (104, 64, 40),
    "bronze_edge": (190, 142, 88),  # 刃口与握持处磨亮
    "verdigris": (72, 120, 98),
    "wood": (98, 68, 46),
    "wood_dk": (70, 47, 32),
    "cord": (150, 122, 80),
    "groove": (26, 20, 17),         # 暗掉的阵线槽 / 眼珠凹槽
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("butt_cap", "root", (0.0, 1.6, 0.0), "柄尾铜镦（长 3.2 px，四面方回纹暗槽）"),
    ("handle", "root", (0.0, 9.0, 0.0), "八角近圆木柄（径 1.7 px，握处缠麻绳）"),
    ("handle_wrap", "handle", (0.0, 7.5, 0.0), "握把上三圈麻绳"),
    ("blade_collar", "root", (0.0, 15.3, 0.0), "钺身下的铜箍"),
    ("yue_blade", "root", (0.0, 19.0, 0.0), "新月形宽刃（xy 平面楔形片，宽 11.9 px，带圆孔）"),
    ("beast_face", "yue_blade", (0.0, 18.5, 0.0), "刃面两侧浮雕兽面（双目 / 鼻梁 / 眉纹 / 颊纹）"),
    ("nei_tang", "root", (0.0, 22.0, 0.0), "方内：穿过钺身顶部的方榫"),
    ("top_cap", "nei_tang", (0.0, 23.4, 0.0), "方内顶小铜冒"),
]


def build_weapon() -> Rig:
    rig = Rig(MATS)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent=parent)

    # 1. 柄尾铜镦：八角铜套，上沿一圈压边，四面各一块方回纹暗槽
    kit.octagon_prism(rig, "butt_cap", "cap_body", 0.0, 0.0, 0.0, 3.0, 1.08, "bronze")
    kit.octagon_prism(rig, "butt_cap", "cap_foot", 0.0, 0.0, 0.0, 0.3, 1.12, "bronze_dk")
    kit.octagon_prism(rig, "butt_cap", "cap_lip", 0.0, 0.0, 2.7, 3.2, 1.12, "bronze_dk")
    kit.octagon_prism(rig, "butt_cap", "cap_shoulder", 0.0, 0.0, 3.0, 3.3, 0.95, "bronze_edge")
    for side, (dx, dz) in enumerate(((1.06, 0.0), (-1.06, 0.0), (0.0, 1.06), (0.0, -1.06))):
        along_x = dx == 0.0
        for k, (a, b) in enumerate(((0.55, 1.25), (1.45, 2.15))):
            frm = (-0.34 if along_x else dx - 0.03, a, dz - 0.03 if along_x else -0.34)
            to = (0.34 if along_x else dx + 0.03, b, dz + 0.03 if along_x else 0.34)
            rig.cube("butt_cap", f"cap_meander_{side}_{k}", frm, to, mat="groove")
    for i, (x, y, z) in enumerate(((0.7, 0.8, 0.88), (-0.5, 2.2, -0.9), (0.9, 2.0, -0.5))):
        rig.cube("butt_cap", f"cap_patina_{i}", (x, y, z), (x + 0.3, y + 0.4, z + 0.18), mat="verdigris")

    # 2. 柄：八角木柄，握处磨亮；握把缠三圈麻绳
    kit.octagon_prism(rig, "handle", "handle_lo", 0.0, 0.0, 3.3, 7.0, HANDLE_R, "wood")
    kit.octagon_prism(rig, "handle", "handle_grip", 0.0, 0.0, 7.0, 11.0, HANDLE_R + 0.02, "wood_dk")
    kit.octagon_prism(rig, "handle", "handle_hi", 0.0, 0.0, 11.0, 15.0, HANDLE_R, "wood")
    for i, y in enumerate((7.4, 8.4, 9.4, 10.4)):
        kit.octagon_prism(rig, "handle_wrap", f"wrap_{i}", 0.0, 0.0, y, y + 0.55, HANDLE_R + 0.1, "cord")

    # 3. 钺身下的铜箍
    kit.octagon_prism(rig, "blade_collar", "collar", 0.0, 0.0, 14.7, 15.9, HANDLE_R + 0.12, "bronze")
    kit.octagon_prism(rig, "blade_collar", "collar_lip_lo", 0.0, 0.0, 14.7, 14.95, HANDLE_R + 0.16, "bronze_dk")
    kit.octagon_prism(rig, "blade_collar", "collar_lip_hi", 0.0, 0.0, 15.65, 15.9, HANDLE_R + 0.16, "bronze_dk")

    # 4. 钺身：新月刃片取自剪影，片厚沿 z；方内位置留空，由方榫穿过
    mask = kit.ref_mask(REF_PNG, REF_PANEL_W)
    frame = kit.frame_from_mask(mask, TOTAL_LEN, REF_AXIS_X)
    cover = kit.sampler(mask, frame)

    def blade_part(u: float, v: float) -> bool:
        if v < BLADE_Y0 - 0.1 or v > BLADE_TOP + 0.1:
            return False
        return not (abs(u) < 0.82 and v > 20.3)    # 方内穿过的位置

    spec = kit.PlateSpec(plane="xy", cell=0.2, half_edge=0.07, half_max=0.3, ramp=1.1, levels=5)
    kit.plate_from_mask(rig, "yue_blade", "blade", cover, (-6.2, 6.2, BLADE_Y0 - 0.1, BLADE_TOP + 0.1),
                        spec, "bronze_edge", "bronze", keep=blade_part)

    # 圆孔沿口：一圈很薄的铜箍（四块拼成方圈）
    rim, r_in = 0.12, 0.62
    rig.cube("yue_blade", "hole_rim_t", (-r_in - rim, HOLE_V + r_in, -0.34), (r_in + rim, HOLE_V + r_in + rim, 0.34), mat="bronze_dk")
    rig.cube("yue_blade", "hole_rim_b", (-r_in - rim, HOLE_V - r_in - rim, -0.34), (r_in + rim, HOLE_V - r_in, 0.34), mat="bronze_dk")
    rig.cube("yue_blade", "hole_rim_l", (-r_in - rim, HOLE_V - r_in, -0.34), (-r_in, HOLE_V + r_in, 0.34), mat="bronze_dk")
    rig.cube("yue_blade", "hole_rim_r", (r_in, HOLE_V - r_in, -0.34), (r_in + rim, HOLE_V + r_in, 0.34), mat="bronze_dk")

    # 5. 兽面浮雕：左右对称、正反两面相同；每件 (x0, x1, v0, v1, 材质)，凸出面 0.22 px
    face = [
        (-2.7, -0.95, 19.05, 19.4, "bronze_dk"),   # 眉纹
        (0.95, 2.7, 19.05, 19.4, "bronze_dk"),
        (-3.15, -2.75, 18.4, 19.95, "bronze_dk"),  # 角纹
        (2.75, 3.15, 18.4, 19.95, "bronze_dk"),
        (-2.0, -1.1, 18.1, 18.95, "bronze_dk"),    # 目框
        (1.1, 2.0, 18.1, 18.95, "bronze_dk"),
        (-1.75, -1.35, 18.35, 18.75, "groove"),    # 眼珠（暗槽）
        (1.35, 1.75, 18.35, 18.75, "groove"),
        (-0.2, 0.2, 17.55, 19.0, "bronze_dk"),     # 鼻梁
        (-0.85, 0.85, 17.5, 17.85, "bronze_dk"),   # 鼻翼 / 口
        (-2.45, -1.05, 17.55, 17.9, "bronze_dk"),  # 颊纹
        (1.05, 2.45, 17.55, 17.9, "bronze_dk"),
        (-0.55, 0.55, 17.1, 17.35, "bronze_dk"),   # 颏
    ]
    for side, z0 in enumerate((0.3, -0.3)):
        for i, (xa, xb, va, vb, mat) in enumerate(face):
            lift = 0.22 + (0.04 if mat == "groove" else 0.0)    # 眼珠凹槽比目框再高一点，避免同面闪烁
            z_in, z_out = (z0, z0 + lift) if z0 > 0 else (z0 - lift, z0)
            rig.cube("beast_face", f"face_{side}_{i:02d}", (xa, va, z_in), (xb, vb, z_out), mat=mat)

    # 钺身锈斑
    for i, (x, v) in enumerate(((-4.3, 17.9), (3.9, 18.2), (-1.5, 16.7), (2.2, 16.9), (4.8, 20.6))):
        rig.cube("yue_blade", f"patina_{i}", (x, v, 0.28), (x + 0.5, v + 0.35, 0.32), mat="verdigris")

    # 6. 方内：方榫穿过钺身顶部，顶上小铜冒
    rig.cube("nei_tang", "nei_core", (-0.72, 20.3, -0.42), (0.72, 23.2, 0.42), mat="bronze")
    rig.cube("nei_tang", "nei_edge_l", (-0.78, 20.5, -0.3), (-0.72, 23.0, 0.3), mat="bronze_edge")
    rig.cube("nei_tang", "nei_edge_r", (0.72, 20.5, -0.3), (0.78, 23.0, 0.3), mat="bronze_edge")
    rig.cube("nei_tang", "nei_slot", (-0.18, 21.0, 0.42), (0.18, 22.2, 0.46), mat="groove")
    rig.cube("nei_tang", "nei_slot_b", (-0.18, 21.0, -0.46), (0.18, 22.2, -0.42), mat="groove")
    rig.cube("top_cap", "cap_base", (-0.84, 22.9, -0.54), (0.84, 23.7, 0.54), mat="bronze")
    rig.cube("top_cap", "cap_lip", (-0.9, 22.9, -0.6), (0.9, 23.12, 0.6), mat="bronze_dk")
    rig.cube("top_cap", "cap_top", (-0.55, 23.7, -0.34), (0.55, 24.0, 0.34), mat="bronze_edge")
    return rig


TITLE = "#9 青铜钺 qing_tong_yue (Round 1)"
POSE = "single_hand"
PROP_YAW = 180.0   # 正视，钺身平面朝镜头

METRICS = [
    ("钺身宽 / 全长", "blade_w"),
    ("钺身高（下缘到角尖）/ 全长", "blade_h"),
    ("钺身下的柄长（含铜箍）/ 全长", "handle_len"),
    ("角尖以上露出长 / 全长", "top_extra"),
    ("柄径 / 全长", "handle_w"),
]


def measure(mask: np.ndarray) -> dict[str, float]:
    """从正视剪影量钺的各部件占全长的百分比。参考图和模型渲染用同一份。

    柄宽取 20%~50% 高度（从下数）的中位数；「钺身」指单行外廓宽度超过 3 倍柄宽的那些行，
    新月角尖是两条细线但外廓仍然很宽，所以角尖上沿也算在内。参考图底部有地面阴影，
    铜镦只量上沿，不量宽度。
    """
    x0, y0, x1, y1 = kit.mask_bbox(mask)
    height = y1 - y0 + 1
    ext = {}
    for y in range(y0, y1 + 1):
        xs = np.where(mask[y])[0]
        if len(xs):
            ext[y] = (int(xs.min()), int(xs.max()))
    mid = [y for y in ext if y1 - 0.50 * height <= y <= y1 - 0.20 * height]
    handle_w = float(np.median([ext[y][1] - ext[y][0] + 1 for y in mid]))
    wide = [y for y in ext if ext[y][1] - ext[y][0] + 1 > 3.0 * handle_w]
    blade_top, blade_bot = min(wide), max(wide)
    # 铜镦上沿：先越过钺身下缘那段渐窄的月牙尾，到第一行宽度 ≤ 1.2 倍柄宽（柄或铜箍），
    # 再往下数第一行宽度 ≥ 1.2 倍柄宽（铜箍只有约 1.13 倍，不算）
    width = lambda y: ext[y][1] - ext[y][0] + 1  # noqa: E731
    stem_top = next(y for y in range(blade_bot + 1, y1 + 1) if width(y) <= 1.2 * handle_w)
    cap_top = next(y for y in range(stem_top, y1 + 1) if width(y) >= 1.2 * handle_w)
    pct = lambda v: 100.0 * v / height  # noqa: E731
    return {
        "blade_w": pct(max(ext[y][1] - ext[y][0] + 1 for y in wide)),
        "blade_h": pct(blade_bot - blade_top),
        "handle_len": pct(cap_top - blade_bot),
        "top_extra": pct(blade_top - y0),
        "handle_w": pct(handle_w),
    }


# Round 3：生成器材质 → handheld_texture 画笔
TEXTURE_MAPPING = {
    "bronze": "weapon_cu_body",
    "bronze_dk": "weapon_cu_dark",
    "bronze_edge": "weapon_cu_edge",
    "verdigris": "weapon_cu_green",
    "wood": "weapon_wd_patina",
    "wood_dk": "weapon_wd_worn",
    "cord": "weapon_cord_tan",
    "groove": "weapon_dead_groove",
}

TEX_CLOSEUPS = [
    ("钺身 正视", 180.0, 0.0, (0, 19.0, 0), 13.0),
    ("兽面浮雕 3/4", 150.0, 20.0, (0, 18.5, 0), 9.0),
    ("柄尾铜镦", 150.0, 15.0, (0, 2.0, 0), 7.0),
]

def main() -> None:
    import weapon_check

    rig = build_weapon()
    print(f"{MODEL_NAME}: {len(rig.elements)} 个方块, {len(rig.bones)} 根骨骼")
    weapon_check.write_and_review(sys.modules[__name__], rig)
    weapon_check.write_tex_review(sys.modules[__name__], rig)


if __name__ == "__main__":
    main()
