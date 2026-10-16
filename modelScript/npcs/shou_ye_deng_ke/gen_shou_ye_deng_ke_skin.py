#!/usr/bin/env python3
"""守夜灯客 shou_ye_deng_ke 的 64×64 classic 皮肤。设计稿：model-review/npcs/shou_ye_deng_ke.md。

用法：
    python3 modelScript/npcs/shou_ye_deng_ke/gen_shou_ye_deng_ke_skin.py            # 皮肤 PNG + 审阅图
    python3 modelScript/npcs/shou_ye_deng_ke/gen_shou_ye_deng_ke_skin.py --no-sheet # 只出皮肤 PNG
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

NPC_ID = "shou_ye_deng_ke"
TITLE = "守夜灯客"
SEED = 707
GEAR = ["gu_deng_gua_gan", "za_pi_xing_nang"]
REVIEW_DIR = Path(os.environ.get("BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"))

SKIN = C("#a88460")
HAIR = C("#c0bab0")
INNER = C("#8a8068")
COAT = C("#5c5448")
SOOT = C("#2a2622")
OIL = C("#3a3228")
TROUSER = C("#46403a")
BOOT = C("#3a3228")
PATCH_A = C("#5a6048")
PATCH_B = C("#7a5a40")
PATCH_C = C("#6a6a70")
BURN = C("#c88a6a")
CLOUD = C("#c8c4b8")
THREAD = C("#a89c80")

PALETTE = [("肤色", "#a88460"), ("白发", "#c0bab0"), ("内衣", "#8a8068"), ("外褂", "#5c5448"), ("烟熏黑", "#2a2622"),
           ("油渍", "#3a3228"), ("裤", "#46403a"), ("毡靴", "#3a3228"), ("补丁 a", "#5a6048"), ("补丁 b", "#7a5a40"),
           ("补丁 c", "#6a6a70"), ("烫疤", "#c88a6a"), ("浊眼", "#c8c4b8")]


def build() -> Skin:
    S = Skin(SEED)

    # ── 头：稀疏白发，深陷的眼窝与法令纹，右眼发浊 ────────────────────────────────────
    paint_head(S, SKIN, HAIR, SEED, front_h=[1, 2, 1, 1, 1, 1, 2, 1], side_h=[6, 6, 5, 3, 3, 2, 1, 1], back_h=6,
               eyes_y=4, eye=C("#2a2622"), eye_white=C("#bdb6a4"), brow=C("#8a857a"), mouth=C("#6a4a3c"),
               streak=C("#8a857a"), streak_frac=0.3)
    fx0, fy0, _, _ = face_rect("head", "front")
    # 眼窝深陷：眼上一行、眼下一行都压暗
    for fx in (1, 2, 5, 6):
        for fy, k in ((3, -0.1), (5, -0.16)):
            c, a = S.get(fx0 + fx, fy0 + fy)
            S.put(fx0 + fx, fy0 + fy, tone(c, k), a)
    # 右眼（画面左侧）发浊：灯火长年熏出来的白翳
    S.put(fx0 + 1, fy0 + 4, CLOUD)
    S.put(fx0 + 2, fy0 + 4, tone(CLOUD, -0.22))
    # 法令纹、眼下胡茬与下巴短白须
    for fx, fy in ((2, 6), (5, 6), (1, 5), (6, 5)):
        c, a = S.get(fx0 + fx, fy0 + fy)
        S.put(fx0 + fx, fy0 + fy, tone(c, -0.16), a)
    for fx in range(1, 7):
        if h2(fx, 7, SEED + 5) > 0.45:
            S.put(fx0 + fx, fy0 + 7, mix(tone(SKIN, -0.1), HAIR, 0.55))
    hat_hair(S, HAIR, SEED, back_rows=4, side_rows=3, side_cols=3, top_wisps=9, streak=C("#8a857a"), streak_frac=0.3)

    # ── 躯干：灰褐内衣 + 及膝长褂（jacket，下摆延伸到 pants 层）──────────────────────────
    S.paint("body", cloth(INNER, SEED + 20, grain=0.06, folds=[(6, 0, 11, 0.0), (18, 0, 11, 0.0)]), rows=(0, 11))
    S.paint("body", cloth(TROUSER, SEED + 22, grain=0.05), rows=(11, 12))
    S.paint("body", skin_tone(tone(SKIN, -0.1), SEED + 23), faces=("top",))
    S.paint("body", lambda x, y, i, j, f, fx: tone(TROUSER, -0.2), faces=("bottom",))
    for i, j in ((7, 0), (8, 0), (6, 0), (9, 0), (7, 1), (8, 1), (7, 2)):
        S.sp("body", i, j, tone(SKIN, -0.1))
    folds = [(5, 1, 12, 0.0), (10, 1, 12, 0.1), (18, 0, 12, 0.0), (22, 2, 12, -0.1), (2, 4, 12, 0.0)]
    S.paint("jacket", cloth(COAT, SEED + 30, grain=0.06, folds=folds, ao_rows=(0, 12)), rows=(0, 12))
    S.paint("jacket", lambda x, y, i, j, f, fx: tone(COAT, 0.07), faces=("top",))
    for i, j in ((7, 0), (8, 0), (7, 1), (8, 1), (6, 0), (9, 0)):
        S.sp("jacket", i, j, None)
    for i, j in ((5, 0), (10, 0), (6, 1), (9, 1), (7, 2), (8, 2)):
        S.sp("jacket", i, j, tone(COAT, 0.18))
    # 前襟中缝 + 三颗骨扣（隔一行一颗）
    for j in range(2, 12):
        S.sp("jacket", 7, j, tone(COAT, -0.3))
        S.sp("jacket", 8, j, tone(COAT, 0.08))
    for j in (3, 6, 9):
        S.sp("jacket", 8, j, C("#d0c8b0"))
    # 三块不同色的补丁
    S.patch("jacket", 9, 2, 3, 3, PATCH_A, SEED + 31, stitch=THREAD)
    S.patch("jacket", 4, 6, 3, 3, PATCH_B, SEED + 32, stitch=THREAD)
    S.patch("jacket", 18, 3, 4, 4, PATCH_C, SEED + 33, stitch=THREAD)
    # 油渍：右胯（画面左）一大块深斑，向下渗
    for i, j in ((4, 8), (5, 8), (6, 8), (4, 9), (5, 9), (6, 9), (5, 10), (4, 10), (3, 9), (5, 11), (6, 10)):
        c, a = S.sget("jacket", i, j)
        S.sp("jacket", i, j, mix(c, OIL, 0.75), a)
    # 下摆参差
    for i in range(24):
        n = h2(i, 11, SEED + 34)
        if n > 0.7:
            S.sp("jacket", i, 11, None)
        else:
            S.sp("jacket", i, 11, tone(COAT, -0.2 + 0.2 * h2(i, 10, SEED + 35)))
    S.stain("jacket", tone(COAT, -0.35), 0.85, SEED + 36, scale=3.0, thr=0.56, rows=(0, 12))
    S.stain("jacket", tone(COAT, 0.2), 0.6, SEED + 37, scale=2.4, thr=0.66, rows=(0, 12))
    wrinkles(S, "jacket", 6, SEED + 38, density=0.5)
    dust_gradient(S, "jacket", C("#7a705e"), 7, 12, 0.5, SEED + 39, t_max=0.3)

    # ── 臂：长袖，袖口烟熏黑；左手掌一圈烫疤 ───────────────────────────────────────
    for base, over in (("rarm", "rsleeve"), ("larm", "lsleeve")):
        S.paint(base, cloth(COAT, SEED + 40, grain=0.06, folds=[(5, 0, 9, 0.0), (13, 0, 9, 0.0)]), rows=(0, 9))
        S.paint(base, lambda x, y, i, j, f, fx: tone(COAT, 0.07), faces=("top",))
        S.paint(base, skin_tone(SKIN, SEED + 41), rows=(9, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(SKIN, -0.2), faces=("bottom",))
        S.paint(over, cloth(tone(COAT, 0.03), SEED + 42, grain=0.06), rows=(0, 9))
        # 袖口烟熏：row 7-8 熏黑（越靠下越黑），边缘破线
        for j in (6, 7, 8):
            for i in range(16):
                p = (0.35, 0.7, 0.95)[j - 6]
                if h2(i, j, SEED + 43) < p:
                    S.sp(over, i, j, tone(SOOT, 0.15 * h2(i, j, SEED + 44)))
        for i in range(16):
            if h2(i, 8, SEED + 45) > 0.6:
                S.sp(over, i, 8, None)
        wrinkles(S, base, 4, SEED + 46, density=0.5)
        dust_gradient(S, over, C("#7a705e"), 2, 6, 0.3, SEED + 47, t_max=0.25)
        # 手：指节粗、青筋
        for i in range(4, 8):
            if h2(i, 10, SEED + 48) > 0.55:
                S.sp(base, i, 10, tone(SKIN, -0.18))
    # 左袖外侧一块油渍
    for i, j in ((8, 3), (9, 3), (9, 4), (8, 4), (10, 4)):
        c, a = S.sget("lsleeve", i, j)
        S.sp("lsleeve", i, j, mix(c, OIL, 0.7), a)
    # 左手掌心一圈烫疤（左手内侧 = right 面，i 0..3，row 10-11）
    for i, j in ((0, 10), (1, 10), (2, 10), (3, 10), (0, 11), (3, 11), (1, 11), (2, 11)):
        S.sp("larm", i, j, tone(BURN, -0.2 if (i + j) % 2 else 0.0))
    S.sp("larm", 1, 11, tone(SKIN, -0.1))
    S.sp("larm", 2, 11, tone(SKIN, -0.1))

    # ── 腿：褂摆（pants 层，前后两片，下摆烟熏）+ 毡靴 ───────────────────────────────
    for base, over in (("rleg", "rpants"), ("lleg", "lpants")):
        S.paint(base, cloth(TROUSER, SEED + 50, grain=0.06, folds=[(5, 0, 8, 0.0), (13, 0, 8, 0.0)]), rows=(0, 8))
        S.paint(base, cloth(BOOT, SEED + 51, grain=0.1, weave=0.03), rows=(8, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(BOOT, -0.25), faces=("bottom",))
        S.paint(base, lambda x, y, i, j, f, fx: tone(TROUSER, -0.15), faces=("top",))
        for i in range(16):
            S.sp(base, i, 8, tone(C("#6a6050"), -0.2 + 0.12 * h2(i, 8, SEED + 52)))  # 毡靴口压边
        S.paint(over, cloth(tone(COAT, -0.02), SEED + 53, grain=0.06, folds=[(2, 0, 7, 0.0), (10, 0, 7, 0.0)]),
                rows=(0, 7), faces=("front", "back", "right", "left"))
        for i in range(16):
            n = h2(i, 7, SEED + 54)
            if n > 0.55:
                S.sp(over, i, 6, tone(SOOT, 0.05))
            if n > 0.7:
                S.sp(over, i, 6, None)
            if n > 0.9:
                S.sp(over, i, 5, None)
        dust_gradient(S, over, C("#7a705e"), 3, 7, 0.5, SEED + 55, t_max=0.3)
        wrinkles(S, base, 4, SEED + 56, cols=(4, 8), density=0.5)
        freckles(S, base, (8, 12), C("#2a2622"), 0.15, SEED + 57, strength=0.5)
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
                   notes="只含已做的复用装备；专属新装备（灯油葫芦串）由 pi-b 另做，不在此图。")


if __name__ == "__main__":
    main()
