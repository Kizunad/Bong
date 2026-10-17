#!/usr/bin/env python3
"""大梢弓 da_shao_gong Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

形制（对照参考图 ``da_shao_gong_ref.png`` 的主视图剪影，弓臂中心线和厚度直接取自剪影）：
- 长梢反曲弓，弓长 28 px，上弦后：握把居中，弓臂向弓背缓缓鼓出（最鼓处在 25% / 75% 高度），
  再在梢根（约 85% / 15% 高度）折向弦，之后是一段 45° 的**长直梢**，一直伸到弦所在平面，
  两梢合计占弓长约 27%（参考图 13.5% × 2）；
- 梢根垫一块「弦垫」（皮垫 + 三圈缠丝），梢端是黑牛角帽、带弦槽；
- 弓臂三层复合（背筋 / 木芯 / 牛角），厚度按剪影：握把 ~1.15 px、弓臂 ~0.9 px、梢 ~0.4–0.6 px；
- 握把缠皮，两端棕色压边；0.3 px 弦直接拉到梢端绳套，弦上不搭箭。
- 末法：只在弦垫皮上留几道暗掉的阵线槽，没有发光部件。

坐标：y 向上，握把中心在 y=14；弓背朝 +z、弦朝 -z；握把中心 z=4.2（评审工具包的持弓姿势按这个点转）。
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

WEAPON_ID = "da_shao_gong"
MODEL_NAME = "DaShaoGong"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
TOTAL_LEN = 28.0
GRIP_Y = 14.0
GRIP_Z = 4.2             # 评审工具包的持弓姿势固定把握把中心放在 z=4.2
GRIP_HALF_LEN = 1.45     # 握把半长（参考图握把约占弓长 10%）

REF_PNG = kit.REVIEW_ROOT / WEAPON_ID / f"{WEAPON_ID}_ref.png"
REF_PANEL_W = 825
REF_STRING_X = 610.5     # 参考图里弦所在像素列
REF_SHADOW_PX = 12
REF_STRING_PX = 9

BRIDGE_FRAC = 0.85       # 弦垫位置（梢根），占全长

MATS = {
    "sinew": (176, 148, 104),
    "core": (112, 62, 42),
    "horn": (40, 33, 30),
    "horn_edge": (86, 68, 52),
    "leather": (82, 58, 40),
    "leather_dk": (54, 38, 28),
    "bridge": (110, 78, 52),         # 弦垫皮
    "cord": (170, 146, 104),
    "string": (190, 182, 160),
    "groove": (26, 20, 17),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("grip", "root", (0.0, 14.0, 4.2), "缠皮长握把（长 2.9 px）+ 两端压边"),
    ("upper_limb", "root", (0.0, 19.0, 5.0), "上弓臂：背筋 / 木芯 / 牛角三层，向弓背缓鼓再折向弦"),
    ("upper_shao", "upper_limb", (0.0, 25.5, 2.5), "上长梢：45° 直梢、弦垫、缠丝、牛角帽"),
    ("lower_limb", "root", (0.0, 9.0, 5.0), "下弓臂：同上"),
    ("lower_shao", "lower_limb", (0.0, 2.5, 2.5), "下长梢：同上"),
    ("bowstring", "root", (0.0, 14.0, 0.0), "0.3 px 弦：梢端绳套之间一条直线，中段缠丝护弦，不搭箭"),
]

LAYERS = [("sinew", 0.66, 0.36), ("core", 0.0, 0.32), ("horn", -0.66, 0.36)]   # 层间略重叠，不留缝


def half_w(half_t: float) -> float:
    return min(0.74, max(0.28, 1.6 * half_t))


def build_weapon() -> Rig:
    rig = Rig(MATS)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent=parent)

    mask = kit.ref_mask(REF_PNG, REF_PANEL_W, shadow_px=REF_SHADOW_PX, string_px=REF_STRING_PX)
    prof = kit.bow_profile(mask, TOTAL_LEN, REF_STRING_X, step=0.55)
    z_grip, _ = prof.at(GRIP_Y)
    dz = GRIP_Z - z_grip
    prof.z = [z + dz for z in prof.z]
    i_lo = max(i for i, v in enumerate(prof.v) if v <= GRIP_Y - GRIP_HALF_LEN)
    i_hi = min(i for i, v in enumerate(prof.v) if v >= GRIP_Y + GRIP_HALF_LEN)
    last = len(prof.v) - 1

    def hw_of(v: float) -> float:
        return half_w(prof.at(v)[1])

    # 1. 弓臂 + 长直梢（同样三层，梢上很薄）
    kit.layered_limb(rig, "upper_limb", "up", prof, i_hi, last - 1, LAYERS, hw_of)
    kit.layered_limb(rig, "lower_limb", "dn", prof, 1, i_lo, LAYERS, hw_of)

    # 2. 握把：缠皮 + 两端压边 + 皮上两道暗掉的菱纹槽线
    gx = [prof.point(i) for i in range(i_lo, i_hi + 1)]
    gt = [prof.half_t[i] for i in range(i_lo, i_hi + 1)]
    kit.strip_along(rig, "grip", "wrap", [(0.0, p[1], p[2] - 0.2 * t) for p, t in zip(gx, gt)],
                    [0.72] * len(gx), [0.8 * t for t in gt], "leather")
    for side, sign in enumerate((-1, 1)):
        v = GRIP_Y + sign * GRIP_HALF_LEN
        z, t = prof.at(v)
        rig.cube("grip", f"collar_{side}", (-0.78, v - 0.17, z - t - 0.04), (0.78, v + 0.17, z + t + 0.04), mat="leather_dk")
        rig.cube("grip", f"collar_edge_{side}", (-0.8, v - 0.1, z - t), (0.8, v + 0.1, z + t), mat="horn_edge")
    z_c, t_c = prof.at(GRIP_Y)
    for i, dv in enumerate((-0.7, -0.2, 0.3, 0.8)):
        rig.cube("grip", f"diamond_{i}", (-0.5, GRIP_Y + dv - 0.04, z_c + 0.8 * t_c), (0.5, GRIP_Y + dv + 0.04, z_c + 0.8 * t_c + 0.03), mat="groove")

    # 3. 梢：弦垫 + 三圈缠丝 + 牛角帽 + 弦槽
    for bone, top, sgn in (("upper_shao", True, 1), ("lower_shao", False, -1)):
        v0 = TOTAL_LEN * (BRIDGE_FRAC if top else 1.0 - BRIDGE_FRAC)
        z0, t0 = prof.at(v0)
        kit.strip_along(rig, bone, "bridge", [(0.0, v0 - 0.45 * sgn, prof.at(v0 - 0.45 * sgn)[0]),
                                              (0.0, v0 + 0.45 * sgn, prof.at(v0 + 0.45 * sgn)[0])],
                        [half_w(t0) + 0.14] * 2, [t0 + 0.14] * 2, "bridge")
        for k, dv in enumerate((0.6, 0.85, 1.1)):
            v = v0 + dv * sgn
            z, t = prof.at(v)
            kit.strip_along(rig, bone, f"wrap{k}", [(0.0, v - 0.1 * sgn, z), (0.0, v + 0.1 * sgn, z)],
                            [half_w(t) + 0.08] * 2, [t + 0.07] * 2, "cord")
        rig.cube(bone, "bridge_mark_a", (-0.1, v0 - 0.1, z0 + t0 + 0.12), (0.1, v0 + 0.1, z0 + t0 + 0.15), mat="groove")
        v_cap = [TOTAL_LEN - 0.95 + i * 0.19 for i in range(6)] if top else [0.95 - i * 0.19 for i in range(6)]
        kit.strip_along(rig, bone, "cap", [(0.0, v, prof.at(v)[0]) for v in v_cap], [0.3] * 6,
                        [max(0.14, prof.at(v)[1]) for v in v_cap], "horn_edge")
        end_v = v_cap[-1] - 0.12 * sgn
        z_end = prof.at(v_cap[-1])[0]
        rig.cube(bone, "nock_l", (-0.34, end_v - 0.12, z_end - 0.12), (-0.26, end_v + 0.12, z_end + 0.16), mat="groove")
        rig.cube(bone, "nock_r", (0.26, end_v - 0.12, z_end - 0.12), (0.34, end_v + 0.12, z_end + 0.16), mat="groove")

    # 4. 弦：梢端绳套之间拉直（参考图里弦几乎是一条直线，仅在梢端略偏），0.3 px
    z_str = dz
    v_nock = TOTAL_LEN * 0.962
    n = int(round((2 * v_nock - TOTAL_LEN) / 1.4))
    line = [(0.0, TOTAL_LEN - v_nock + (2 * v_nock - TOTAL_LEN) * k / n, z_str) for k in range(n + 1)]
    kit.strip_along(rig, "bowstring", "string", line, [0.15] * len(line), [0.15] * len(line), "string")
    rig.cube("bowstring", "serving", (-0.19, GRIP_Y - 0.7, z_str - 0.19), (0.19, GRIP_Y + 0.7, z_str + 0.19), mat="cord")
    for side, v in enumerate((v_nock, TOTAL_LEN - v_nock)):
        rig.cube("bowstring", f"loop_{side}", (-0.2, v - 0.18, z_str - 0.2), (0.2, v + 0.18, z_str + 0.2), mat="cord")
    return rig


TITLE = "#9 大梢弓 da_shao_gong (Round 1)"
POSE = "bow"
PROP_YAW = -90.0   # 侧视：弓背（+z）在画面左，与参考图同向
MEASURE_EXCLUDE_BONES = ("bowstring",)

METRICS = [
    ("中心线位移 @60%（相对握把，向弓背为正）", "disp@60"),
    ("中心线位移 @70%（弓臂最鼓处）", "disp@70"),
    ("中心线位移 @80%", "disp@80"),
    ("中心线位移 @90%（长直梢中段）", "disp@90"),
    ("中心线位移 @97%（梢端，贴近弦）", "disp@97"),
    ("最靠弓背点离握把的高度差", "peak"),
    ("握把厚 / 全长", "thick_grip"),
    ("弓臂厚 @70% / 全长", "thick_mid"),
    ("梢厚 @92% / 全长", "thick_tip"),
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
