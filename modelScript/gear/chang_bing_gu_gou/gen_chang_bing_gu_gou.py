#!/usr/bin/env python3
"""长柄骨钩 chang_bing_gu_gou Blockbench .bbmodel 程序化生成器 —— Round 1 几何（按参考图重做，整个重写）。

对应 NPC：拾骨人。手持（右手）。「钩子没看出来」是上两版的退件原因，这版的钩头直接取自参考图侧视剪影：
- 全长 30 px，枯木长柄径只有 0.86 px（占全长 2.8%）；
- 柄顶一只大弯钩：异变兽肋骨弯成，钩从柄顶向前（-z）一道大弧扫下，尖端指回下方，
  钩身宽 5.1 px、垂落 4.7 px（占全长 17% / 16%），肋骨厚约 1.1 px，向尖端逐渐变薄，外缘崩两个口；
- 钩根一圈锈铁箍（带铆钉）+ 草绳绑缠；柄上两段草绳防滑握把；柄尾兽皮包头，缝线；
- 末法：只有钩根铁箍上一道暗掉的刻线，没有发光部件。

坐标：y 向上，柄轴在 x=0,z=0；钩朝 -z（手持时朝前），钩平面是 zy，片厚沿 x。
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

WEAPON_ID = "chang_bing_gu_gou"
MODEL_NAME = "ChangBingGuGou"
OUT_DIR = REPO / "modelScript" / "gear" / WEAPON_ID
GEAR_REVIEW_ROOT = kit.REVIEW_ROOT.parent / "gear"
REVIEW_DIR = GEAR_REVIEW_ROOT / WEAPON_ID
TOTAL_LEN = 30.0
GRIP_Y = 12.0
POSE = "polearm"
SHAFT_R = 0.43     # 柄内切半径，柄径 0.86 px 取自参考图

REF_PNG = REVIEW_DIR / f"{WEAPON_ID}_ref.png"
REF_PANEL_W = 940
REF_AXIS_X = 435

MATS = {
    "bone": (226, 214, 188),         # 打磨泛黄的异变兽肋骨
    "bone_lt": (240, 232, 212),
    "bone_dk": (178, 160, 130),
    "wood": (94, 68, 46),            # 枯木柄
    "wood_dk": (66, 46, 30),
    "cord": (168, 140, 88),          # 草绳
    "cord_dk": (122, 96, 56),
    "iron": (96, 72, 58),            # 锈铁
    "rust": (122, 66, 36),
    "hide": (112, 72, 48),           # 兽皮包头
    "hide_dk": (76, 50, 32),
    "groove": (28, 22, 20),
}

BONES = [
    ("root", None, (0.0, 0.0, 0.0), "世界根"),
    ("butt", "root", (0.0, 1.2, 0.0), "柄尾兽皮包头 + 缝线 + 绳箍"),
    ("shaft", "root", (0.0, 14.0, 0.0), "八角近圆枯木长柄（径 0.86 px）带节疤与裂纹"),
    ("wrap_a", "shaft", (0.0, 17.3, 0.0), "上段握把草绳（6 圈）"),
    ("wrap_b", "shaft", (0.0, 7.8, 0.0), "下段握把草绳（5 圈）"),
    ("hook", "shaft", (0.0, 27.5, -2.0), "肋骨大弯钩：zy 平面，厚 1.1 px 逐渐收尖，外缘崩口"),
    ("hook_collar", "hook", (0.0, 28.4, 0.0), "钩根锈铁箍 + 铆钉 + 草绳缠绑"),
]


def build_weapon() -> Rig:
    rig = Rig(MATS)
    for name, parent, pivot, _ in BONES:
        rig.bone(name, pivot, parent=parent)

    # 1. 柄：三段八角（中间磨亮），节疤与一道裂纹
    kit.octagon_prism(rig, "shaft", "shaft_lo", 0.0, 0.0, 2.2, 12.0, SHAFT_R, "wood")
    kit.octagon_prism(rig, "shaft", "shaft_mid", 0.0, 0.0, 12.0, 24.0, SHAFT_R + 0.02, "wood_dk")
    kit.octagon_prism(rig, "shaft", "shaft_hi", 0.0, 0.0, 24.0, 28.2, SHAFT_R, "wood")
    for i, (v, z) in enumerate(((5.3, 0.46), (13.4, -0.46), (21.6, 0.46))):    # 节疤
        rig.cube("shaft", f"knot_{i}", (-0.22, v, z - 0.07 if z > 0 else z - 0.07), (0.22, v + 0.5, z + 0.07), mat="wood_dk")
    rig.cube("shaft", "crack", (-0.05, 9.2, 0.42), (0.05, 11.6, 0.47), mat="groove")
    rig.cube("shaft", "crack_b", (0.05, 20.0, -0.47), (0.12, 21.4, -0.42), mat="groove")

    # 2. 握把草绳：上段 16.5–18.2（6 圈），下段 7.0–8.6（5 圈）
    for i in range(6):
        v = 16.5 + i * 0.3
        kit.octagon_prism(rig, "wrap_a", f"wa_{i}", 0.0, 0.0, v, v + 0.25, SHAFT_R + 0.14, "cord" if i % 2 == 0 else "cord_dk")
    for i in range(5):
        v = 7.0 + i * 0.32
        kit.octagon_prism(rig, "wrap_b", f"wb_{i}", 0.0, 0.0, v, v + 0.27, SHAFT_R + 0.14, "cord" if i % 2 == 0 else "cord_dk")
    rig.cube("wrap_a", "wa_tail", (0.12, 15.5, 0.52), (0.36, 16.6, 0.62), mat="cord")
    rig.cube("wrap_b", "wb_tail", (-0.36, 6.0, -0.62), (-0.12, 7.1, -0.52), mat="cord")

    # 3. 柄尾兽皮包头：皮筒 + 缝线 + 绳箍 + 端面骨片
    kit.octagon_prism(rig, "butt", "cap", 0.0, 0.0, 0.3, 2.3, 0.66, "hide")
    kit.octagon_prism(rig, "butt", "cap_lo", 0.0, 0.0, 0.3, 0.7, 0.7, "hide_dk")
    kit.octagon_prism(rig, "butt", "cap_hi_band", 0.0, 0.0, 2.1, 2.5, 0.6, "cord")
    kit.octagon_prism(rig, "butt", "cap_end", 0.0, 0.0, 0.0, 0.3, 0.5, "bone_dk")
    for i in range(4):
        rig.cube("butt", f"stitch_{i}", (-0.05, 0.9 + i * 0.35, 0.67), (0.05, 1.1 + i * 0.35, 0.72), mat="cord")
    rig.cube("butt", "stitch_cross", (-0.28, 1.2, 0.69), (0.28, 1.28, 0.73), mat="cord")

    # 4. 钩：取自参考图侧视剪影，钩朝 -z；片厚沿 x，截面透镜形，尖端收到 0.13 px
    mask = kit.ref_mask(REF_PNG, REF_PANEL_W)
    frame = kit.frame_from_mask(mask, TOTAL_LEN, REF_AXIS_X, u_sign=-1)
    cover = kit.sampler(mask, frame)

    def hook_part(u: float, v: float) -> bool:
        if v < 24.9 or v > 30.05:
            return False
        if -0.7 < u < 0.4 and v < 28.3:                 # 柄的位置（剪影里柄在 u∈[-0.6, 0.3]）
            return False
        # 外缘两处崩口（尖端向上 1.2 px 与 0.5 px 处各一个小缺口）
        for (ua, ub, va, vb) in ((-5.25, -4.85, 27.0, 27.5), (-5.1, -4.8, 26.15, 26.5)):
            if ua <= u <= ub and va <= v <= vb:
                return False
        return True

    spec = kit.PlateSpec(plane="zy", cell=0.2, half_edge=0.06, half_max=0.55, ramp=0.95, levels=5)
    kit.plate_from_mask(rig, "hook", "hook", cover, (-5.5, 0.9, 24.8, 30.1), spec, "bone_lt", "bone",
                        keep=hook_part)
    # 钩身骨纹：沿钩背几处浅褐骨斑
    for i, (v, z) in enumerate(((29.3, -1.8), (28.0, -3.7), (27.0, -4.55), (26.0, -4.5))):
        rig.cube("hook", f"stain_{i}", (0.2, v, z - 0.2), (0.28, v + 0.4, z + 0.2), mat="bone_dk")
        rig.cube("hook", f"stain_b_{i}", (-0.28, v + 0.2, z - 0.15), (-0.2, v + 0.55, z + 0.25), mat="bone_dk")

    # 5. 钩根：锈铁箍（带铆钉与暗掉的刻线）+ 草绳绑缠 5 圈
    kit.octagon_prism(rig, "hook_collar", "band", 0.0, 0.0, 27.6, 28.5, SHAFT_R + 0.2, "iron")
    kit.octagon_prism(rig, "hook_collar", "band_lip", 0.0, 0.0, 27.55, 27.75, SHAFT_R + 0.25, "rust")
    rig.cube("hook_collar", "rivet_a", (-0.12, 27.9, -0.78), (0.12, 28.2, -0.64), mat="iron")
    rig.cube("hook_collar", "rivet_b", (-0.12, 27.9, 0.64), (0.12, 28.2, 0.78), mat="iron")
    rig.cube("hook_collar", "dead_line", (0.55, 27.85, -0.3), (0.62, 28.25, 0.3), mat="groove")
    for i in range(5):
        v = 28.5 + i * 0.22
        kit.octagon_prism(rig, "hook_collar", f"lash_{i}", 0.0, -0.2, v, v + 0.19, SHAFT_R + 0.22 - 0.02 * i,
                          "cord" if i % 2 == 0 else "cord_dk")
    rig.cube("hook_collar", "lash_tail_a", (-0.2, 26.4, -0.78), (0.0, 27.7, -0.66), mat="cord")
    rig.cube("hook_collar", "lash_tail_b", (0.05, 26.8, 0.62), (0.25, 27.7, 0.74), mat="cord_dk")
    return rig


TITLE = "#3 长柄骨钩 chang_bing_gu_gou (Round 1)"
PROP_YAW = -90.0     # 相机在 +x：钩（-z）在画面右，与参考图同向
REF_SHADOW_PX = 22

METRICS = [
    ("钩总宽（含柄）/ 全长", "hook_w"),
    ("钩垂落高度 / 全长", "hook_h"),
    ("钩身厚（肋骨）/ 全长", "hook_t"),
    ("钩内口宽（柄到钩内沿）/ 全长", "hook_gap"),
    ("柄径 / 全长", "shaft_w"),
]


def measure(mask: np.ndarray) -> dict[str, float]:
    """从侧视剪影量骨钩（钩朝右）的百分比。参考图和模型渲染用同一份。

    柄宽 = 20%~60% 高度（从上数）处的中位数；钩垂落高度 = 外廓宽于 3 倍柄宽的最低一行到顶；
    钩身厚 = 钩所在行里最右一段的最大横向厚度；内口宽 = 钩中段那一行柄到钩最右段之间的空隙。
    """
    x0, y0, x1, y1 = kit.mask_bbox(mask)
    height = y1 - y0 + 1
    pct = lambda v: 100.0 * v / height  # noqa: E731

    def runs(y: int) -> list[tuple[int, int]]:
        xs = np.where(mask[y])[0]
        out, a, p = [], xs[0], xs[0]
        for x in xs[1:]:
            if x != p + 1:
                out.append((int(a), int(p)))
                a = x
            p = x
        out.append((int(a), int(p)))
        return out

    mid = [y for y in range(y0 + int(0.2 * height), y0 + int(0.6 * height))]
    shaft_w = float(np.median([runs(y)[0][1] - runs(y)[0][0] + 1 for y in mid]))
    head_rows = [y for y in range(y0, y0 + int(0.3 * height)) if mask[y].any()]
    wide = [y for y in head_rows if runs(y)[-1][1] - runs(y)[0][0] + 1 > 3 * shaft_w]
    hook_bottom = max(wide)
    split_rows = [y for y in head_rows if len(runs(y)) >= 2]
    thick = max(runs(y)[-1][1] - runs(y)[-1][0] + 1 for y in split_rows)
    y_mid = split_rows[len(split_rows) // 2]
    r = runs(y_mid)
    return {
        "hook_w": pct(x1 - x0 + 1),
        "hook_h": pct(hook_bottom - y0 + 1),
        "hook_t": pct(thick),
        "hook_gap": pct(r[-1][0] - r[0][1] - 1),
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
