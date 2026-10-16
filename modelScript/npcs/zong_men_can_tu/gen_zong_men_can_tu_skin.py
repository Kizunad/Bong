#!/usr/bin/env python3
"""宗门残徒 zong_men_can_tu 的 64×64 classic 皮肤。设计稿：model-review/npcs/zong_men_can_tu.md。

用法：
    python3 modelScript/npcs/zong_men_can_tu/gen_zong_men_can_tu_skin.py            # 皮肤 PNG + 审阅图
    python3 modelScript/npcs/zong_men_can_tu/gen_zong_men_can_tu_skin.py --no-sheet # 只出皮肤 PNG
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

NPC_ID = "zong_men_can_tu"
TITLE = "宗门残徒"
SEED = 505
GEAR = ["can_bu_tou_jin", "pi_sheng_hu_wan", "water_skin"]
REVIEW_DIR = Path(os.environ.get("BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"))

SKIN = C("#d0a888")
HAIR = C("#26201e")
UNDER = C("#cfc8b4")
UNDER_STAIN = C("#a89878")
ROBE = C("#6a7a80")
ROBE_FADE = C("#8a9a9e")
SASH = C("#7a3a34")
TROUSER = C("#3e4448")
SHOE = C("#3a3028")
BANDAGE = C("#bdb39c")
EMBLEM = C("#9ab0b0")
SCAB = C("#8a4a40")
BURN = C("#c08a78")
PATCH = C("#8a7a60")
THREAD = C("#b8b49c")

PALETTE = [("肤色", "#d0a888"), ("头发", "#26201e"), ("里衣", "#cfc8b4"), ("污渍", "#a89878"), ("宗门袍", "#6a7a80"),
           ("褪色处", "#8a9a9e"), ("暗红腰带", "#7a3a34"), ("裤", "#3e4448"), ("鞋", "#3a3028"), ("绷带", "#bdb39c"),
           ("徽记残弧", "#9ab0b0"), ("结痂", "#8a4a40"), ("灼伤疤", "#c08a78"), ("异色补丁", "#8a7a60")]


def build() -> Skin:
    S = Skin(SEED)

    # ── 头：黑发，发髻松散，几缕散发垂在脸侧；右眼角下结痂 ────────────────────────────
    paint_head(S, SKIN, HAIR, SEED, front_h=[2, 2, 1, 1, 1, 1, 2, 2], side_h=[6, 6, 5, 3, 3, 2, 2, 1], back_h=6,
               eyes_y=4, eye=C("#2a2224"), eye_white=C("#e0d8cc"), brow=C("#2a2220"), mouth=C("#b0645a"))
    fx0, fy0, _, _ = face_rect("head", "front")
    # 脸尖：下颌两侧收窄（颌线压暗）
    for fx in (0, 7):
        c, a = S.get(fx0 + fx, fy0 + 7)
        S.put(fx0 + fx, fy0 + 7, tone(c, -0.18), a)
    # 右眼角下的小血痂（画面左侧）
    S.put(fx0 + 1, fy0 + 5, SCAB)
    S.put(fx0 + 2, fy0 + 5, tone(SCAB, 0.15))
    # 淡淡的黑眼圈
    for fx in (1, 2, 5, 6):
        if (fx, 5) != (1, 5) and (fx, 5) != (2, 5):
            c, a = S.get(fx0 + fx, fy0 + 5)
            S.put(fx0 + fx, fy0 + 5, mix(c, C("#8a7090"), 0.18), a)
    # hat：松散的发髻（头顶隆起）+ 垂到后颈和脸侧的散发
    hx, hy, _, _ = face_rect("hat", "top")
    for fx, fy in ((3, 2), (4, 2), (3, 3), (4, 3), (2, 3), (5, 3), (3, 4), (4, 4)):
        S.put(hx + fx, hy + fy, tone(HAIR, (0.16, 0.04, -0.06, 0.1)[(fx + fy) % 4]))
    hat_hair(S, HAIR, SEED, back_rows=5, side_rows=4, side_cols=2, top_wisps=3)
    # 垂到脸侧的两缕：hat 前面两侧
    fxh, fyh, _, _ = face_rect("hat", "front")
    for fx, rows in ((0, 4), (7, 5)):
        for fy in range(rows):
            S.put(fxh + fx, fyh + fy, tone(HAIR, (0.05, -0.1, 0.12)[fy % 3]))

    # ── 躯干：里衣 + 灰青旧宗门袍（jacket）+ 暗红腰带 ────────────────────────────────────
    S.paint("body", cloth(UNDER, SEED + 20, grain=0.05, folds=[(6, 0, 11, 0.0), (18, 0, 11, 0.0)]), rows=(0, 11))
    S.paint("body", cloth(TROUSER, SEED + 22, grain=0.05), rows=(11, 12))
    S.paint("body", skin_tone(tone(SKIN, -0.1), SEED + 23), faces=("top",))
    S.paint("body", lambda x, y, i, j, f, fx: tone(TROUSER, -0.2), faces=("bottom",))
    for i, j in ((7, 0), (8, 0), (6, 0), (9, 0), (7, 1), (8, 1)):
        S.sp("body", i, j, tone(SKIN, -0.08))
    S.stain("body", UNDER_STAIN, 0.9, SEED + 21, scale=2.4, thr=0.56, rows=(0, 11))
    folds = [(5, 1, 11, 0.0), (10, 1, 11, 0.1), (18, 0, 11, 0.0), (22, 2, 11, -0.1)]
    S.paint("jacket", cloth(ROBE, SEED + 30, grain=0.05, folds=folds, ao_rows=(0, 12)), rows=(0, 12))
    S.paint("jacket", lambda x, y, i, j, f, fx: tone(ROBE, 0.08), faces=("top",))
    # 交领右衽：左襟压右襟（斜线），领口露里衣
    for k in range(5):
        S.sp("jacket", 8 - k // 2, k, tone(ROBE, -0.3))
        S.sp("jacket", 9 - k // 2, k, tone(ROBE_FADE, 0.0))
    for i, j in ((7, 0), (8, 0), (7, 1)):
        S.sp("jacket", i, j, None)
    # 腰带：暗红布带（row 8-9）+ 前面一个结
    for j in (8, 9):
        for i in range(24):
            S.sp("jacket", i, j, tone(SASH, (-0.12, 0.0)[j - 8] + 0.1 * (h2(i, j, SEED + 31) - 0.5)))
    for i, j in ((6, 9), (7, 9), (6, 10), (9, 9), (9, 10), (10, 11)):
        S.sp("jacket", i, j, tone(SASH, 0.05 * (i % 3) - 0.08))
    # 胸前徽记：只剩半个破碎的圆（不写字）
    for i, j in ((6, 2), (7, 2), (5, 3), (5, 4), (6, 5), (7, 5), (8, 4)):
        S.sp("jacket", i, j, tone(EMBLEM, -0.1 + 0.15 * h2(i, j, SEED + 32)))
    # 袍摆：下摆参差，右襟撕得更短；褪色块与污渍
    for i in range(24):
        n = h2(i, 11, SEED + 33)
        if n > 0.62:
            S.sp("jacket", i, 11, None)
        if n > 0.85:
            S.sp("jacket", i, 10, None)
    S.stain("jacket", ROBE_FADE, 0.85, SEED + 34, scale=3.0, thr=0.56, rows=(0, 12))
    S.stain("jacket", tone(ROBE, -0.3), 0.7, SEED + 35, scale=2.4, thr=0.62, rows=(0, 12))
    freckles(S, "jacket", (0, 12), C("#6a3a30"), 0.025, SEED + 36, strength=0.7)  # 零星血点
    wrinkles(S, "jacket", 6, SEED + 37, density=0.55)
    dust_gradient(S, "jacket", C("#8a8a7e"), 8, 12, 0.5, SEED + 38, t_max=0.3)
    # 左肩异色补丁（画面右侧肩头）：在 lsleeve 外侧
    S.patch("lsleeve", 8, 0, 4, 3, PATCH, SEED + 39, stitch=THREAD)

    # ── 臂：右袖撕开露出绷带小臂；左袖到腕，前臂一块灼伤疤 ─────────────────────────────
    # 右臂
    S.paint("rarm", cloth(ROBE, SEED + 40, grain=0.05, folds=[(5, 0, 5, 0.0), (13, 0, 5, 0.0)]), rows=(0, 5))
    S.paint("rarm", lambda x, y, i, j, f, fx: tone(ROBE, 0.08), faces=("top",))
    S.paint("rarm", skin_tone(SKIN, SEED + 41), rows=(5, 12))
    S.paint("rarm", lambda x, y, i, j, f, fx: tone(SKIN, -0.2), faces=("bottom",))
    S.paint("rsleeve", cloth(tone(ROBE, 0.03), SEED + 42, grain=0.05), rows=(0, 4))
    for i in range(16):
        n = h2(i, 4, SEED + 43)
        S.sp("rsleeve", i, 4, tone(ROBE_FADE, -0.15) if n > 0.4 else None)
        if n > 0.8:
            S.sp("rsleeve", i, 3, None)
    for j in range(5, 10):
        for i in range(16):
            k = (-0.08, 0.05, -0.02)[(i + j) % 3] + (h2(i, j, SEED + 44) - 0.5) * 0.08
            S.sp("rarm", i, j, tone(BANDAGE, k))
    for i in range(16):
        S.sp("rarm", i, 5, tone(BANDAGE, -0.2))
        S.sp("rarm", i, 9, tone(BANDAGE, -0.16))
    for i, j in ((5, 7), (6, 7), (6, 8)):
        S.sp("rarm", i, j, tone(C("#8a4a40"), 0.1 * h2(i, j, SEED + 45)))
    # 左臂
    S.paint("larm", cloth(ROBE, SEED + 46, grain=0.05, folds=[(5, 0, 9, 0.0), (13, 0, 9, 0.0)]), rows=(0, 9))
    S.paint("larm", lambda x, y, i, j, f, fx: tone(ROBE, 0.08), faces=("top",))
    S.paint("larm", skin_tone(SKIN, SEED + 47), rows=(9, 12))
    S.paint("larm", lambda x, y, i, j, f, fx: tone(SKIN, -0.2), faces=("bottom",))
    S.paint("lsleeve", cloth(tone(ROBE, 0.03), SEED + 48, grain=0.05), rows=(3, 9))
    for i in range(16):
        S.sp("lsleeve", i, 8, tone(ROBE_FADE, -0.18) if h2(i, 8, SEED + 49) > 0.4 else None)
    # 左前臂灼伤疤：不规则一块（袖口以下露出的皮肤上，外侧 i 8..11 + 正面一角）
    for i, j in ((8, 10), (9, 10), (9, 11), (10, 10), (10, 11), (7, 11), (11, 10)):
        S.sp("larm", i, j, tone(BURN, -0.2 if (i + j) % 2 else -0.05))
    S.sp("larm", 9, 11, tone(BURN, -0.32))
    for part in ("rarm", "larm"):
        wrinkles(S, part, 4, SEED + 50, density=0.5)
        dust_gradient(S, part, C("#8a8a7e"), 2, 5, 0.4, SEED + 51, t_max=0.3)

    # ── 腿：深灰裤，布鞋；撕下半截的袍摆在第二层盖住大腿 ──────────────────────────────
    for base, over in (("rleg", "rpants"), ("lleg", "lpants")):
        S.paint(base, cloth(TROUSER, SEED + 60, grain=0.05, folds=[(5, 0, 9, 0.0), (13, 0, 9, 0.0)]), rows=(0, 9))
        S.paint(base, cloth(SHOE, SEED + 61, grain=0.06), rows=(9, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(SHOE, -0.2), faces=("bottom",))
        S.paint(base, lambda x, y, i, j, f, fx: tone(TROUSER, -0.15), faces=("top",))
        for i in range(16):
            S.sp(base, i, 9, tone(C("#8a8070"), -0.15 + 0.12 * h2(i, 9, SEED + 62)))  # 鞋口白边
        S.paint(over, cloth(tone(ROBE, -0.02), SEED + 63, grain=0.05, folds=[(2, 0, 4, 0.0), (10, 0, 4, 0.0)]),
                rows=(0, 4), faces=("front", "back", "right", "left"))
        for i in range(16):
            n = h2(i, 4, SEED + 64)
            if n > 0.5:
                S.sp(over, i, 3, tone(ROBE_FADE, -0.12))
            if n > 0.72:
                S.sp(over, i, 3, None)
            if n > 0.9:
                S.sp(over, i, 2, None)
        wrinkles(S, base, 5, SEED + 65, cols=(4, 8), density=0.5)
        dust_gradient(S, base, C("#6a6458"), 5, 9, 0.4, SEED + 66, t_max=0.35)
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
                   notes="只含已做的复用装备；专属新装备（残破宗门腰牌 / 半截袍摆）由 pi-b 另做，不在此图。")


if __name__ == "__main__":
    main()
