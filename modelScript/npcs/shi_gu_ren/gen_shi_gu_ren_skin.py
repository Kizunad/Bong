#!/usr/bin/env python3
"""拾骨人 shi_gu_ren 的 64×64 classic 皮肤。设计稿：model-review/npcs/shi_gu_ren.md。

用法：
    python3 modelScript/npcs/shi_gu_ren/gen_shi_gu_ren_skin.py            # 皮肤 PNG + 审阅图
    python3 modelScript/npcs/shi_gu_ren/gen_shi_gu_ren_skin.py --no-sheet # 只出皮肤 PNG
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "_kit"))
from skin_kit import (C, Skin, cloth, dust_gradient, face_rect, freckles, h2, hat_hair, mix, paint_head, skin_tone,  # noqa: E402
                      tone, wrinkles)

NPC_ID = "shi_gu_ren"
TITLE = "拾骨人"
SEED = 303
GEAR = ["gu_jia_bei_lou", "ku_cao_guo_jiao", "cu_bu_bang_tui"]
REVIEW_DIR = Path(os.environ.get("BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"))

SKIN = C("#9c7655")
BONE_DUST = C("#c4bfae")
HAIR = C("#9a9488")
SHIRT = C("#7a6c50")
TROUSER = C("#5a4c38")
FOOT = C("#6a5a42")
LEATHER = C("#5a3f2b")
NEEDLE = C("#e0d8c0")
THREAD = C("#b0a888")

PALETTE = [("肤色", "#9c7655"), ("骨粉污迹", "#d8d0bc"), ("灰白发", "#9a9488"), ("粗布上衣", "#7a6c50"), ("裤", "#5a4c38"),
           ("脚", "#6a5a42"), ("皮补丁", "#5a3f2b"), ("骨针", "#e0d8c0")]


def bone_dust(S: Skin, part: str, rows: tuple[int, int], p: float, seed: int, strength: float = 0.55) -> None:
    freckles(S, part, rows, BONE_DUST, p, seed, strength=strength)


def build() -> Skin:
    S = Skin(SEED)

    # ── 头：灰白稀发、长脸、高颧骨；双颊骨粉与划痕 ───────────────────────────────────
    paint_head(S, SKIN, HAIR, SEED, front_h=[2, 2, 1, 2, 1, 1, 2, 2], side_h=[6, 6, 5, 4, 3, 3, 2, 1], back_h=6,
               eyes_y=4, eye=C("#2a2520"), eye_white=C("#c4bba6"), brow=C("#7a756a"), streak=C("#6a655c"), streak_frac=0.3)
    fx0, fy0, _, _ = face_rect("head", "front")
    # 颧骨高：颧下一行暗，眼窝深
    for fx in (1, 6):
        c, a = S.get(fx0 + fx, fy0 + 6)
        S.put(fx0 + fx, fy0 + 6, tone(c, -0.14), a)
    for fx in (1, 2, 5, 6):
        c, a = S.get(fx0 + fx, fy0 + 5)
        S.put(fx0 + fx, fy0 + 5, tone(c, -0.1), a)
    # 骨粉：两颊、鼻梁、额头
    for fx, fy in ((0, 5), (7, 5), (3, 5), (4, 3)):
        c, a = S.get(fx0 + fx, fy0 + fy)
        S.put(fx0 + fx, fy0 + fy, mix(c, BONE_DUST, 0.4), a)
    # 左颊两道平行细划痕（画面右侧）
    for fx, fy in ((6, 5), (7, 6)):
        S.put(fx0 + fx, fy0 + fy, C("#b88a70"))
    for fx, fy in ((5, 6), (6, 7)):
        S.put(fx0 + fx, fy0 + fy, C("#b88a70"))
    # 稀疏灰胡须
    for fx in range(2, 6):
        if h2(fx, 7, SEED + 5) > 0.55:
            S.put(fx0 + fx, fy0 + 7, mix(tone(SKIN, -0.12), HAIR, 0.4))
    hat_hair(S, HAIR, SEED, back_rows=5, side_rows=3, side_cols=3, top_wisps=7, streak=C("#6a655c"), streak_frac=0.3)

    # ── 躯干：粗布上衣（基础层）+ 宽松外罩衫（jacket，骨粉洗白）────────────────────────
    S.paint("body", cloth(SHIRT, SEED + 20, grain=0.06, folds=[(6, 0, 11, 0.0), (18, 0, 11, 0.0)]), rows=(0, 12))
    S.paint("body", skin_tone(tone(SKIN, -0.1), SEED + 23), faces=("top",))
    S.paint("body", lambda x, y, i, j, f, fx: tone(TROUSER, -0.2), faces=("bottom",))
    for i, j in ((7, 0), (8, 0), (6, 0), (9, 0), (7, 1), (8, 1)):
        S.sp("body", i, j, tone(SKIN, -0.1))
    # 外罩衫：比基础层略宽，下摆到 row 10 之后参差，骨粉沿下摆 / 肘 / 膝洗白
    folds = [(5, 1, 10, 0.1), (11, 1, 10, -0.1), (18, 0, 10, 0.0), (21, 1, 10, 0.1)]
    S.paint("jacket", cloth(tone(SHIRT, 0.03), SEED + 30, grain=0.065, folds=folds, ao_rows=(0, 11)), rows=(0, 11))
    S.paint("jacket", lambda x, y, i, j, f, fx: tone(SHIRT, 0.08), faces=("top",))
    for i, j in ((7, 0), (8, 0), (6, 0), (7, 1), (8, 1)):
        S.sp("jacket", i, j, None)
    for i, j in ((5, 0), (9, 0), (10, 0), (6, 1), (9, 1)):
        S.sp("jacket", i, j, tone(SHIRT, 0.15))
    # 衣领别一根细骨针（斜 3px）
    for k in range(3):
        S.sp("jacket", 9 + k // 2, 0 + k, NEEDLE if k != 1 else tone(NEEDLE, -0.15))
    # 下摆参差
    for i in range(24):
        n = h2(i, 10, SEED + 31)
        if n > 0.7:
            S.sp("jacket", i, 10, None)
        if n > 0.93:
            S.sp("jacket", i, 9, None)
    # 骨粉洗白：越往下越多，叠加大块洗痕
    bone_dust(S, "jacket", (6, 11), 0.2, SEED + 32, 0.5)
    S.stain("jacket", mix(SHIRT, BONE_DUST, 0.55), 0.5, SEED + 33, scale=3.2, thr=0.62, rows=(0, 11))
    S.stain("jacket", tone(SHIRT, -0.3), 0.6, SEED + 34, scale=2.4, thr=0.62, rows=(0, 11))
    dust_gradient(S, "jacket", BONE_DUST, 6, 11, 0.4, SEED + 35, t_max=0.4)
    wrinkles(S, "jacket", 6, SEED + 36, density=0.5)

    # ── 臂：袖子卷到肘，小臂与手满是骨粉 ───────────────────────────────────────────
    for base, over in (("rarm", "rsleeve"), ("larm", "lsleeve")):
        S.paint(base, cloth(SHIRT, SEED + 40, grain=0.06, folds=[(5, 0, 5, 0.0), (13, 0, 5, 0.0)]), rows=(0, 5))
        S.paint(base, lambda x, y, i, j, f, fx: tone(SHIRT, 0.08), faces=("top",))
        S.paint(base, skin_tone(SKIN, SEED + 41), rows=(5, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(SKIN, -0.22), faces=("bottom",))
        S.paint(over, cloth(tone(SHIRT, 0.06), SEED + 42, grain=0.06, folds=[(6, 0, 3, 0.0), (14, 0, 3, 0.0)]), rows=(3, 5))
        for i in range(16):
            S.sp(over, i, 3, tone(SHIRT, -0.1 + 0.1 * h2(i, 3, SEED + 43)))
            if h2(i, 5, SEED + 44) > 0.55:
                S.sp(over, i, 5, tone(SHIRT, -0.15))
        bone_dust(S, base, (8, 12), 0.2, SEED + 45, 0.4)
        # 指节粗大、指甲缝发黑
        for i in range(4, 8):
            S.sp(base, i, 11, tone(C("#5a4a38"), 0.1 * h2(i, 11, SEED + 46)))
        S.sp(base, 4, 10, tone(SKIN, -0.18))
        S.sp(base, 7, 10, tone(SKIN, -0.18))
        wrinkles(S, base, 3, SEED + 47, density=0.5)
        dust_gradient(S, over, BONE_DUST, 3, 5, 0.5, SEED + 48, t_max=0.4)
    # 右袖口磨成毛边
    for i in range(16):
        if h2(i, 4, SEED + 49) > 0.5:
            S.sp("rsleeve", i, 4, tone(SHIRT, -0.2))

    # ── 腿：裤脚被骨粉洗成灰白，右膝皮补丁；脚裹布 ──────────────────────────────────
    for base, over in (("rleg", "rpants"), ("lleg", "lpants")):
        S.paint(base, cloth(TROUSER, SEED + 50, grain=0.065, folds=[(5, 0, 9, 0.0), (13, 0, 9, 0.0)]), rows=(0, 9))
        S.paint(base, cloth(FOOT, SEED + 51, grain=0.07, weave=0.03), rows=(9, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(FOOT, -0.25), faces=("bottom",))
        S.paint(base, lambda x, y, i, j, f, fx: tone(TROUSER, -0.15), faces=("top",))
        # 裹脚的斜缠布条
        for j in range(9, 12):
            for i in range(16):
                if (i + j) % 3 == 0:
                    S.sp(base, i, j, tone(FOOT, -0.18))
        # 第二层：裤腿下半被骨粉糊住的一圈 + 绑腿接口
        S.paint(over, cloth(tone(TROUSER, 0.05), SEED + 52, grain=0.07), rows=(6, 8))
        for i in range(16):
            S.sp(over, i, 8, tone(C("#8a7a58"), -0.1 + 0.15 * h2(i, 8, SEED + 53)))
        dust_gradient(S, base, BONE_DUST, 4, 9, 0.35, SEED + 54, t_max=0.4)
        dust_gradient(S, over, BONE_DUST, 6, 9, 0.45, SEED + 55, t_max=0.45)
        wrinkles(S, base, 4, SEED + 56, cols=(4, 8), density=0.55)
        bone_dust(S, base, (9, 12), 0.2, SEED + 57, 0.4)
    S.patch("rleg", 4, 4, 3, 3, LEATHER, SEED + 58, stitch=THREAD)
    S.stain("rleg", tone(TROUSER, -0.35), 0.7, SEED + 59, scale=2.6, thr=0.6, rows=(0, 9))
    S.stain("lleg", tone(TROUSER, -0.35), 0.7, SEED + 60, scale=2.6, thr=0.6, rows=(0, 9))
    return S


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--no-sheet", action="store_true")
    args = ap.parse_args()
    S = build()
    img = S.image()
    out = HERE / f"{NPC_ID}_skin.png"
    img.save(out)
    print(f"✓ {out}")
    if not args.no_sheet:
        from skin_view import make_sheet
        make_sheet(NPC_ID, TITLE, img, GEAR, PALETTE, REVIEW_DIR / "img" / "npcs" / NPC_ID / "skin_check.png",
                   notes="只含已做的复用装备；专属新装备（长柄骨钩）由 pi-a 另做，不在此图。")


if __name__ == "__main__":
    main()
