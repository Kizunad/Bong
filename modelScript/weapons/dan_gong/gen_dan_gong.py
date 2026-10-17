#!/usr/bin/env python3
"""弹弓 dan_gong Blockbench .bbmodel 程序化生成器 —— Round 1 几何。

古式弹弓：弓形，不是 Y 形叉子。形制（对照参考图 ``dan_gong_ref.png`` 的主视图剪影，
弓身中心线和厚度直接取自剪影）：
- 一张小弓，弓长 22 px，侧视是 D 形大弧（握把处向弓背鼓 ~2.4 px），弓身较厚（1.0–1.4 px）；
- 握把缠绳：十来圈麻绳，两端各一道压边；
- 双弦：两根 0.3 px 的弦并排（x = ±0.8），各自从弓梢缠绳结拉到另一端；
- 双弦正中夹一只小皮兜（空的）：1.9 px 宽、2.2 px 高的浅杯，杯底朝射手、开口朝弓背（弹丸座朝前），
  口沿缝在两根弦上；兜里不放弹丸，也不做任何弹药；
- 末法：只在握把压边上留一道暗掉的阵线槽，没有发光部件。

坐标：y 向上，握把中心在 y=11；弓背朝 +z、弦朝 -z；握把中心 z=4.2（评审工具包的持弓姿势按这个点转）。
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

WEAPON_ID = "dan_gong"
MODEL_NAME = "DanGong"
OUT_DIR = REPO / "modelScript" / "weapons" / WEAPON_ID
TOTAL_LEN = 22.0
GRIP_Y = 11.0
GRIP_Z = 4.2             # 评审工具包的持弓姿势固定把握把中心放在 z=4.2
GRIP_HALF_LEN = 1.5      # 缠绳握把半长（参考图约占弓长 13%）

REF_PNG = kit.REVIEW_ROOT / WEAPON_ID / f"{WEAPON_ID}_ref.png"
REF_PANEL_W = 825
REF_STRING_X = 444.5     # 参考图里弦所在像素列
REF_SHADOW_PX = 12
REF_STRING_PX = 9

STRING_END = 0.885       # 弦端缠绳结的位置（占全长，参考图弦从 88.5% 拉到 11.9%）
STRING_DX = 0.8          # 双弦各偏离中线的距离（弦距 1.6 px）

MATS = {
    "wood": (104, 80, 60),           # 弓背
    "wood_mid": (78, 58, 42),
    "wood_dk": (48, 36, 28),         # 弓腹
    "cord": (170, 146, 104),         # 握把 / 弦结麻绳
    "cord_dk": (124, 102, 70),
    "string": (196, 188, 168),
    "leather": (96, 66, 46),         # 皮兜
    "leather_dk": (62, 42, 30),
    "groove": (26, 20, 17),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("grip", "root", (0.0, 11.0, 4.2), "缠绳握把（十来圈麻绳，长 3 px）+ 两端压边"),
    ("upper_limb", "root", (0.0, 16.0, 3.0), "上弓臂：背 / 芯 / 腹三层硬木，D 形大弧"),
    ("upper_tip", "upper_limb", (0.0, 19.5, 1.0), "上弓梢：弦结缠绳 + 梢头"),
    ("lower_limb", "root", (0.0, 6.0, 3.0), "下弓臂：同上"),
    ("lower_tip", "lower_limb", (0.0, 2.5, 1.0), "下弓梢：同上"),
    ("string_l", "root", (-0.8, 11.0, 0.0), "左弦 0.3 px，两端缠绳结"),
    ("string_r", "root", (0.8, 11.0, 0.0), "右弦 0.3 px，两端缠绳结"),
    ("pouch", "root", (0.0, 11.4, -0.4), "小皮兜（空）：背板 + 四边口沿，缝在双弦上，开口朝弓背"),
]

LAYERS = [("wood", 0.66, 0.36), ("wood_mid", 0.0, 0.32), ("wood_dk", -0.66, 0.36)]


def half_w(half_t: float) -> float:
    return min(0.7, max(0.36, 1.1 * half_t))


def build_weapon() -> Rig:
    rig = Rig(MATS)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent=parent)

    mask = kit.ref_mask(REF_PNG, REF_PANEL_W, shadow_px=REF_SHADOW_PX, string_px=REF_STRING_PX)
    prof = kit.bow_profile(mask, TOTAL_LEN, REF_STRING_X, step=0.5)
    z_grip, _ = prof.at(GRIP_Y)
    dz = GRIP_Z - z_grip
    prof.z = [z + dz for z in prof.z]
    i_lo = max(i for i, v in enumerate(prof.v) if v <= GRIP_Y - GRIP_HALF_LEN)
    i_hi = min(i for i, v in enumerate(prof.v) if v >= GRIP_Y + GRIP_HALF_LEN)
    last = len(prof.v) - 1

    def hw_of(v: float) -> float:
        return half_w(prof.at(v)[1])

    # 1. 弓身：上下各三层
    kit.layered_limb(rig, "upper_limb", "up", prof, i_hi, last, LAYERS, hw_of)
    kit.layered_limb(rig, "lower_limb", "dn", prof, 0, i_lo, LAYERS, hw_of)

    # 2. 握把：木芯 + 十来圈麻绳 + 两端压边
    gx = [prof.point(i) for i in range(i_lo, i_hi + 1)]
    gt = [prof.half_t[i] for i in range(i_lo, i_hi + 1)]
    kit.strip_along(rig, "grip", "core", gx, [0.66] * len(gx), gt, "wood_mid")
    n_rings = 11
    for k in range(n_rings):
        v = GRIP_Y - GRIP_HALF_LEN + 0.2 + k * (2 * GRIP_HALF_LEN - 0.4) / (n_rings - 1)
        z, t = prof.at(v)
        mat = "cord" if k % 2 == 0 else "cord_dk"
        kit.strip_along(rig, "grip", f"ring_{k:02d}", [(0.0, v - 0.1, prof.at(v - 0.1)[0]), (0.0, v + 0.1, prof.at(v + 0.1)[0])],
                        [0.74] * 2, [t + 0.08] * 2, mat)
    for side, sign in enumerate((-1, 1)):
        v = GRIP_Y + sign * GRIP_HALF_LEN
        z, t = prof.at(v)
        rig.cube("grip", f"collar_{side}", (-0.8, v - 0.15, z - t - 0.06), (0.8, v + 0.15, z + t + 0.06), mat="leather_dk")
        rig.cube("grip", f"collar_groove_{side}", (-0.4, v - 0.04, z + t + 0.06), (0.4, v + 0.04, z + t + 0.09), mat="groove")

    # 3. 弓梢：弦结缠绳（四圈）；梢头沿剪影继续翘过弦线，端头封一块木帽
    z_str = dz
    for bone, top, sgn in (("upper_tip", True, 1), ("lower_tip", False, -1)):
        v0 = TOTAL_LEN * (STRING_END if top else 1.0 - STRING_END)
        for k, dv in enumerate((-0.45, -0.15, 0.15, 0.45)):
            v = v0 + dv
            z, t = prof.at(v)
            kit.strip_along(rig, bone, f"knot{k}", [(0.0, v - 0.08, prof.at(v - 0.08)[0]), (0.0, v + 0.08, prof.at(v + 0.08)[0])],
                            [half_w(t) + 0.12] * 2, [t + 0.1] * 2, "cord")
        # 弦结：从缠绳向弦线伸出的绳扣，连住双弦
        z0, t0 = prof.at(v0)
        rig.cube(bone, "tie_block", (-STRING_DX - 0.2, v0 - 0.3, z_str - 0.2), (STRING_DX + 0.2, v0 + 0.3, z0 - t0 + 0.05), mat="cord_dk")
        v_end = TOTAL_LEN - 0.55 if top else 0.55
        z_end, t_end = prof.at(v_end)
        rig.cube(bone, "end_cap", (-0.5, v_end - 0.55, z_end - t_end - 0.05), (0.5, v_end + 0.55, z_end + t_end + 0.05), mat="wood_dk")

    # 4. 双弦：各自 0.3 px，z = 弦平面；两端落在弦结上
    v_a, v_b = TOTAL_LEN * (1.0 - STRING_END), TOTAL_LEN * STRING_END
    n = int(round((v_b - v_a) / 1.2))
    for name, x in (("string_l", -STRING_DX), ("string_r", STRING_DX)):
        line = [(x, v_a + (v_b - v_a) * k / n, z_str) for k in range(n + 1)]
        kit.strip_along(rig, name, name, line, [0.15] * len(line), [0.15] * len(line), "string")

    # 5. 皮兜：浅杯形，开口朝弓背（+z），杯底朝射手。竖向切成 7 片，每片的深度按抛物线（中间最深），
    #    每片 = 一块背板 + 两侧口沿；上下两端是口沿封边。两侧口沿缝在双弦上。
    v_c, half_h, pw = 11.4, 1.1, STRING_DX + 0.2
    rim_z = z_str + 0.12            # 口沿所在平面（略靠弓背）
    depth = 1.0                     # 杯底比口沿深多少
    n_slice = 7
    for k in range(n_slice):
        y0 = v_c - half_h + k * (2 * half_h) / n_slice
        y1 = y0 + (2 * half_h) / n_slice
        s_mid = ((y0 + y1) / 2 - v_c) / half_h
        back_z = rim_z - 0.2 - depth * (1.0 - s_mid * s_mid)    # 杯底内壁的 z
        rig.cube("pouch", f"back_{k}", (-pw + 0.03, y0, back_z - 0.45), (pw - 0.03, y1, back_z), mat="leather_dk")
        rig.cube("pouch", f"wall_l_{k}", (-pw, y0, back_z - 0.45), (-pw + 0.22, y1, rim_z), mat="leather")
        rig.cube("pouch", f"wall_r_{k}", (pw - 0.22, y0, back_z - 0.45), (pw, y1, rim_z), mat="leather")
    rig.cube("pouch", "rim_top", (-pw, v_c + half_h - 0.2, rim_z - 0.3), (pw, v_c + half_h, rim_z), mat="leather")
    rig.cube("pouch", "rim_bot", (-pw, v_c - half_h, rim_z - 0.3), (pw, v_c - half_h + 0.2, rim_z), mat="leather")
    for i, (sx, sv) in enumerate(((-1, 1), (1, 1), (-1, -1), (1, -1))):
        rig.cube("pouch", f"stitch_{i}", (sx * STRING_DX - 0.2, v_c + sv * 0.8 - 0.14, z_str - 0.2),
                 (sx * STRING_DX + 0.2, v_c + sv * 0.8 + 0.14, z_str + 0.2), mat="cord")
    return rig


TITLE = "#10 弹弓 dan_gong (Round 1)"
POSE = "bow"
PROP_YAW = -90.0   # 侧视：弓背（+z）在画面左，与参考图同向
MEASURE_EXCLUDE_BONES = ("string_l", "string_r", "pouch")

METRICS = [
    ("中心线位移 @60%（相对握把，向弓背为正）", "disp@60"),
    ("中心线位移 @70%", "disp@70"),
    ("中心线位移 @80%", "disp@80"),
    ("中心线位移 @90%（弓梢）", "disp@90"),
    ("中心线位移 @97%（梢头越过弦线）", "disp@97"),
    ("最靠弓背点离握把的高度差", "peak"),
    ("握把厚 / 全长", "thick_grip"),
    ("弓臂厚 @70% / 全长", "thick_mid"),
    ("弓梢厚 @92% / 全长", "thick_tip"),
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
