#!/usr/bin/env python3
"""角弓 jiao_gong Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

形制（对照参考图 ``jiao_gong_ref.png`` 的主视图剪影，弓臂中心线和厚度直接取自剪影）：
- 短而强的复合反曲弓，弓长 24 px，上弦后侧视呈 M 形双曲：握把向弓背鼓出，
  弓臂向弦收拢（离弦最近处在梢根），弓弰在最后 12% 又向前（弓背）翘出；
- 弓臂三层复合：弓背贴筋（浅褐）、中间木芯（红褐）、内侧（弓腹）贴牛角（黑褐）；
  厚度按剪影：握把 ~1.4 px、弓臂 ~0.7–1.1 px、弓弰 ~0.4 px，不设最小截面；
- 握把：弓背侧一块骨垫（象牙白），其余缠皮，两端各一道压边；
- 梢根缠一段丝绳，弓弰端头是黑牛角帽，带弦槽；
- 弦只有 0.3 px，从弰端绳套出发，贴着弓弰走到梢根再拉直；弦上不搭箭。
- 末法：只在骨垫和梢根缠丝上留了几道暗掉的阵线槽，没有发光部件。

坐标：y 向上，握把中心在 y=12；弓背朝 +z、弦朝 -z；握把中心 z=4.2（评审工具包的持弓姿势按这个点转）。
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

WEAPON_ID = "jiao_gong"
MODEL_NAME = "JiaoGong"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
TOTAL_LEN = 24.0
GRIP_Y = 12.0
GRIP_Z = 4.2             # 评审工具包的持弓姿势固定把握把中心放在 z=4.2
GRIP_HALF_LEN = 1.45     # 握把半长

REF_PNG = kit.REVIEW_ROOT / WEAPON_ID / f"{WEAPON_ID}_ref.png"
REF_PANEL_W = 770
REF_STRING_X = 511.5     # 参考图里弦所在像素列
REF_SHADOW_PX = 12
REF_STRING_PX = 9

# 弦的折点（占全长的百分比，来自参考图上弦的细线）：梢根处的绳套在 91.6% 处、离弦平面 0.6 px
STRING_KNEE = 0.848
STRING_NOCK = 0.916
STRING_NOCK_Z = 0.6

MATS = {
    "sinew": (176, 148, 104),        # 弓背贴筋
    "core": (112, 62, 42),           # 木芯
    "horn": (42, 34, 30),            # 弓腹贴牛角
    "horn_edge": (88, 70, 54),
    "leather": (82, 58, 40),         # 握把缠皮
    "leather_dk": (54, 38, 28),
    "bone": (212, 204, 184),         # 骨垫
    "cord": (170, 146, 104),         # 梢根缠丝
    "string": (190, 182, 160),
    "groove": (26, 20, 17),          # 暗掉的阵线 / 弦槽
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("grip", "root", (0.0, 12.0, 4.2), "缠皮握把（长 2.9 px）+ 两端压边"),
    ("bone_pad", "grip", (0.0, 12.0, 4.7), "弓背侧骨垫（象牙白，暗掉的阵线槽）"),
    ("upper_limb", "root", (0.0, 17.0, 3.0), "上弓臂：背筋 / 木芯 / 牛角三层，S 形收向弦"),
    ("upper_tip", "upper_limb", (0.0, 22.5, 1.5), "上弓弰：梢根缠丝、黑牛角帽、弦槽"),
    ("lower_limb", "root", (0.0, 7.0, 3.0), "下弓臂：同上"),
    ("lower_tip", "lower_limb", (0.0, 1.5, 1.5), "下弓弰：同上"),
    ("bowstring", "root", (0.0, 12.0, 0.0), "0.3 px 弦：绳套 → 贴弰 → 拉直，中段缠丝护弦，不搭箭"),
]

LAYERS = [("sinew", 0.64, 0.34), ("core", 0.0, 0.28), ("horn", -0.64, 0.34)]


def half_w(half_t: float) -> float:
    return min(0.74, max(0.3, 1.6 * half_t))


def build_weapon() -> Rig:
    rig = Rig(MATS)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent=parent)

    mask = kit.ref_mask(REF_PNG, REF_PANEL_W, shadow_px=REF_SHADOW_PX, string_px=REF_STRING_PX)
    prof = kit.bow_profile(mask, TOTAL_LEN, REF_STRING_X, step=0.55)
    z_grip, ht_grip = prof.at(GRIP_Y)
    dz = GRIP_Z - z_grip                      # 整体平移，使握把中心落在 z=4.2
    prof.z = [z + dz for z in prof.z]
    i_lo = max(i for i, v in enumerate(prof.v) if v <= GRIP_Y - GRIP_HALF_LEN)
    i_hi = min(i for i, v in enumerate(prof.v) if v >= GRIP_Y + GRIP_HALF_LEN)
    last = len(prof.v) - 1

    def hw_of(v: float) -> float:
        return half_w(prof.at(v)[1])

    # 1. 弓臂：上下各三层
    kit.layered_limb(rig, "upper_limb", "up", prof, i_hi, last - 1, LAYERS, hw_of)
    kit.layered_limb(rig, "lower_limb", "dn", prof, 1, i_lo, LAYERS, hw_of)

    # 2. 握把：缠皮 + 弓背侧骨垫 + 两端压边
    gx = [prof.point(i) for i in range(i_lo, i_hi + 1)]
    gt = [prof.half_t[i] for i in range(i_lo, i_hi + 1)]
    kit.strip_along(rig, "grip", "wrap", [(0.0, p[1], p[2] - 0.3 * t) for p, t in zip(gx, gt)],
                    [0.74] * len(gx), [0.7 * t for t in gt], "leather")
    pad_z0, pad_z1 = GRIP_Z + 0.35 * ht_grip, GRIP_Z + 1.02 * ht_grip
    rig.cube("bone_pad", "pad_main", (-0.5, GRIP_Y - 1.05, pad_z0), (0.5, GRIP_Y + 1.05, pad_z1), mat="bone")
    rig.cube("bone_pad", "pad_edge_l", (-0.56, GRIP_Y - 0.9, pad_z0 + 0.05), (-0.5, GRIP_Y + 0.9, pad_z1 - 0.05), mat="bone")
    rig.cube("bone_pad", "pad_edge_r", (0.5, GRIP_Y - 0.9, pad_z0 + 0.05), (0.56, GRIP_Y + 0.9, pad_z1 - 0.05), mat="bone")
    for i, dv in enumerate((-0.55, 0.0, 0.55)):
        rig.cube("bone_pad", f"pad_groove_{i}", (-0.28, GRIP_Y + dv - 0.05, pad_z1), (0.28, GRIP_Y + dv + 0.05, pad_z1 + 0.03), mat="groove")
    for side, sign in enumerate((-1, 1)):
        v = GRIP_Y + sign * GRIP_HALF_LEN
        z, t = prof.at(v)
        rig.cube("grip", f"collar_{side}", (-0.78, v - 0.17, z - t - 0.04), (0.78, v + 0.17, z + t + 0.04), mat="leather_dk")
        rig.cube("grip", f"collar_edge_{side}", (-0.8, v - 0.1, z - t), (0.8, v + 0.1, z + t), mat="horn_edge")

    # 3. 弓弰：梢根缠丝（4 圈）、黑牛角帽、弦槽
    for bone, top, sgn in (("upper_tip", True, 1), ("lower_tip", False, -1)):
        for k, frac in enumerate((0.874, 0.89, 0.905, 0.92)):
            v = TOTAL_LEN * (frac if top else 1.0 - frac)
            z, t = prof.at(v)
            pts = [(0.0, v - 0.1 * sgn, z - 0.03), (0.0, v + 0.1 * sgn, z + 0.03)]
            kit.strip_along(rig, bone, f"wrap{k}", pts, [half_w(t) + 0.1] * 2, [t + 0.08] * 2, "cord")
        # 牛角帽：最后 0.9 px，整厚黑牛角
        v_cap = [TOTAL_LEN - 0.95 + i * 0.19 for i in range(6)] if top else [0.95 - i * 0.19 for i in range(6)]
        cap_pts = [(0.0, v, prof.at(v)[0]) for v in v_cap]
        cap_ht = [max(0.14, prof.at(v)[1]) for v in v_cap]
        kit.strip_along(rig, bone, "cap", cap_pts, [0.3] * 6, cap_ht, "horn_edge")
        end_v = v_cap[-1] - 0.12 * sgn
        z_end = prof.at(v_cap[-1])[0]
        rig.cube(bone, "nock_l", (-0.34, end_v - 0.12, z_end - 0.12), (-0.26, end_v + 0.12, z_end + 0.16), mat="groove")
        rig.cube(bone, "nock_r", (0.26, end_v - 0.12, z_end - 0.12), (0.34, end_v + 0.12, z_end + 0.16), mat="groove")

    # 4. 弦：0.3 px。弰端绳套 → 贴弰到梢根折点 → 拉直 → 对称回另一端；中段缠丝护弦
    z_str = dz                                    # 弦所在平面（参考图里弦的像素列）
    top_nock = TOTAL_LEN * STRING_NOCK
    top_knee = TOTAL_LEN * STRING_KNEE
    pts = [(0.0, TOTAL_LEN - top_nock, z_str + STRING_NOCK_Z), (0.0, TOTAL_LEN - top_knee, z_str),
           (0.0, top_knee, z_str), (0.0, top_nock, z_str + STRING_NOCK_Z)]
    line = []
    for a, b in zip(pts, pts[1:]):                # 每段再切细，弦才贴得住弓弰
        n = max(1, int(round(np.hypot(b[1] - a[1], b[2] - a[2]) / 1.2)))
        for k in range(n):
            line.append((0.0, a[1] + (b[1] - a[1]) * k / n, a[2] + (b[2] - a[2]) * k / n))
    line.append(pts[-1])
    kit.strip_along(rig, "bowstring", "string", line, [0.15] * len(line), [0.15] * len(line), "string")
    rig.cube("bowstring", "serving", (-0.19, GRIP_Y - 0.65, z_str - 0.19), (0.19, GRIP_Y + 0.65, z_str + 0.19), mat="cord")
    for side, v in enumerate((top_nock, TOTAL_LEN - top_nock)):
        rig.cube("bowstring", f"loop_{side}", (-0.2, v - 0.18, z_str + STRING_NOCK_Z - 0.2), (0.2, v + 0.18, z_str + STRING_NOCK_Z + 0.2), mat="cord")
    return rig


TITLE = "#8 角弓 jiao_gong (Round 1)"
POSE = "bow"
PROP_YAW = -90.0   # 侧视：弓背（+z）在画面左，与参考图同向
MEASURE_EXCLUDE_BONES = ("bowstring",)

METRICS = [
    ("中心线位移 @60%（相对握把，向弓背为正）", "disp@60"),
    ("中心线位移 @70%", "disp@70"),
    ("中心线位移 @80%", "disp@80"),
    ("中心线位移 @90%（梢根，离弦最近）", "disp@90"),
    ("中心线位移 @97%（弓弰向前翘）", "disp@97"),
    ("最靠弓背点离握把的高度差", "peak"),
    ("握把厚 / 全长", "thick_grip"),
    ("弓臂厚 @70% / 全长", "thick_mid"),
    ("弓弰厚 @92% / 全长", "thick_tip"),
    ("侧视包围盒宽 / 全长", "depth"),
]

measure = kit.bow_measure


def main() -> None:
    import weapon_check

    rig = build_weapon()
    print(f"{MODEL_NAME}: {len(rig.elements)} 个方块, {len(rig.bones)} 根骨骼")
    weapon_check.write_and_review(sys.modules[__name__], rig)


if __name__ == "__main__":
    main()
