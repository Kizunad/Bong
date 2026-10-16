#!/usr/bin/env python3
"""北荒猎户 bei_huang_lie_hu 的 64×64 classic 皮肤。设计稿：model-review/npcs/bei_huang_lie_hu.md。

用法：
    python3 modelScript/npcs/bei_huang_lie_hu/gen_bei_huang_lie_hu_skin.py            # 皮肤 PNG + 审阅图
    python3 modelScript/npcs/bei_huang_lie_hu/gen_bei_huang_lie_hu_skin.py --no-sheet # 只出皮肤 PNG
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

NPC_ID = "bei_huang_lie_hu"
TITLE = "北荒猎户"
SEED = 606
GEAR = ["shou_ya_xiang_lian", "pi_nang_xiong_gua", "xiu_pian_hu_mu"]
REVIEW_DIR = Path(os.environ.get("BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"))

SKIN = C("#8e6648")
HAIR = C("#1e1a18")
BEARD = C("#2a2420")
FUR_IN = C("#7a6048")
HIDE = C("#5a4030")
FUR_TRIM = C("#b8a888")
TROUSER = C("#4a3828")
BOOT = C("#3a2c20")
PATCH = C("#6a4a34")
GRIT = C("#c8c4b8")
CLAW = C("#a0705a")
BITE = C("#9a5a48")
STRAP = C("#3a2c20")
THREAD = C("#a89878")

PALETTE = [("肤色", "#8e6648"), ("头发", "#1e1a18"), ("络腮胡", "#2a2420"), ("毛皮内衬", "#7a6048"), ("兽皮短袄", "#5a4030"),
           ("浅毛边", "#b8a888"), ("皮裤", "#4a3828"), ("皮靴", "#3a2c20"), ("皮补丁", "#6a4a34"), ("砂尘", "#c8c4b8"),
           ("爪痕", "#b07860"), ("咬痕", "#9a5a48")]


def build() -> Skin:
    S = Skin(SEED)

    # ── 头：硬短发、方脸，左脸三道爪痕，络腮胡在 hat 层 ─────────────────────────────────
    paint_head(S, SKIN, HAIR, SEED, front_h=[1, 1, 1, 1, 1, 1, 1, 2], side_h=[5, 5, 4, 3, 3, 2, 2, 1], back_h=5,
               eyes_y=4, eye=C("#1e1a16"), eye_white=C("#c4bba8"), brow=C("#16120f"), mouth=C("#6a3a2e"))
    fx0, fy0, _, _ = face_rect("head", "front")
    # 方脸：颧骨两侧压暗，眼眶皱纹
    for fx in (0, 7):
        for fy in (4, 5, 6):
            c, a = S.get(fx0 + fx, fy0 + fy)
            S.put(fx0 + fx, fy0 + fy, tone(c, -0.1), a)
    S.put(fx0 + 3, fy0 + 3, tone(SKIN, -0.14))
    S.put(fx0 + 4, fy0 + 3, tone(SKIN, -0.14))
    # 三道平行爪痕（画面右侧 = 左脸，从颧骨斜到下颌）
    for pts in (((5, 2), (5, 3), (6, 4), (6, 5), (7, 6)), ((6, 2), (6, 3), (7, 4), (7, 5)), ((4, 3), (4, 4), (5, 5), (5, 6), (6, 7))):
        for fx, fy in pts:
            S.put(fx0 + fx, fy0 + fy, tone(CLAW, -0.08 + 0.12 * h2(fx, fy, SEED + 5)))
    # hat 层：络腮胡（两鬓到下巴，嘴的位置留空）
    bx, by, _, _ = face_rect("hat", "front")
    for fx in range(8):
        for fy in (6, 7):
            if fy == 6 and fx in (3, 4):
                continue
            if h2(fx, fy, SEED + 7) > 0.12:
                S.put(bx + fx, by + fy, tone(BEARD, (-0.1, 0.0, 0.12, 0.06)[(fx * 3 + fy) % 4]))
    for fx in (0, 1):
        for fy in (4, 5):
            S.put(bx + fx, by + fy, tone(BEARD, 0.05 * ((fx + fy) % 2)))
    for fx, fy in ((7, 4), (7, 5), (6, 5)):
        S.put(bx + fx, by + fy, tone(BEARD, 0.05 * ((fx + fy) % 2)))
    for face in ("right", "left"):
        sx0, sy0, _, _ = face_rect("hat", face)
        fxs = (6, 7) if face == "right" else (0, 1)
        for fx in fxs:
            for fy in (5, 6, 7):
                S.put(sx0 + fx, sy0 + fy, tone(BEARD, 0.06 * ((fx + fy) % 3)))
    hat_hair(S, HAIR, SEED, back_rows=0, side_rows=0, side_cols=1, top_wisps=6)

    # ── 躯干：毛皮内衬（基础层）+ 兽皮短袄（jacket），领口 / 下摆翻浅毛边 ──────────────────
    S.paint("body", cloth(FUR_IN, SEED + 20, grain=0.08, folds=[(6, 0, 11, 0.0), (18, 0, 11, 0.0)]), rows=(0, 10))
    S.paint("body", cloth(STRAP, SEED + 21, grain=0.05), rows=(9, 10))
    S.paint("body", cloth(TROUSER, SEED + 22, grain=0.06), rows=(10, 12))
    S.paint("body", skin_tone(tone(SKIN, -0.1), SEED + 23), faces=("top",))
    S.paint("body", lambda x, y, i, j, f, fx: tone(TROUSER, -0.2), faces=("bottom",))
    for i, j in ((7, 0), (8, 0), (6, 0), (9, 0), (7, 1), (8, 1)):
        S.sp("body", i, j, tone(SKIN, -0.1))
    folds = [(5, 1, 9, 0.1), (10, 1, 9, -0.1), (18, 0, 9, 0.0), (21, 2, 9, 0.1)]
    S.paint("jacket", cloth(HIDE, SEED + 30, grain=0.08, folds=folds, ao_rows=(0, 9)), rows=(0, 9))
    S.paint("jacket", lambda x, y, i, j, f, fx: tone(HIDE, 0.08), faces=("top",))
    # 领口：V 形开口，翻出一圈浅毛
    for i, j in ((7, 0), (8, 0), (7, 1), (8, 1), (6, 0), (9, 0), (7, 2)):
        S.sp("jacket", i, j, None)
    for i, j in ((5, 0), (10, 0), (6, 1), (9, 1), (7, 3), (8, 3), (6, 2), (8, 2), (4, 0), (11, 0)):
        S.sp("jacket", i, j, tone(FUR_TRIM, -0.1 + 0.18 * h2(i, j, SEED + 31)))
    # 下摆浅毛边（row 8），毛边参差
    for i in range(24):
        S.sp("jacket", i, 8, tone(FUR_TRIM, -0.15 + 0.2 * h2(i, 8, SEED + 32)))
        if h2(i, 8, SEED + 33) > 0.82:
            S.sp("jacket", i, 8, None)
    # 右肋一条长条皮补丁（画面左侧 right/front 交界）
    S.patch("jacket", 3, 3, 2, 5, PATCH, SEED + 34, stitch=THREAD)
    # 毛向短线（顺着兽皮纹）
    for k in range(18):
        i, j = int(h2(k, 0, SEED + 35) * 24), int(h2(k, 1, SEED + 35) * 8) + 1
        c, a = S.sget("jacket", i, j)
        if a > 0:
            S.sp("jacket", i, j, tone(c, 0.12))
            S.sp("jacket", i, j + 1, tone(c, -0.1))
    S.stain("jacket", tone(HIDE, -0.35), 0.8, SEED + 36, scale=2.6, thr=0.58, rows=(0, 9))
    wrinkles(S, "jacket", 6, SEED + 37, density=0.5)
    # 肩头与上背落着北荒的灰白砂尘
    freckles(S, "jacket", (0, 3), GRIT, 0.22, SEED + 38, strength=0.6)
    for i in range(24):
        S.sp("body", i, 9, tone(STRAP, 0.1 * (h2(i, 9, SEED + 39) - 0.5)))

    # ── 臂：短袄袖到腕，袖口翻浅毛，右手咬痕与拖绳勒痕 ─────────────────────────────────
    for base, over in (("rarm", "rsleeve"), ("larm", "lsleeve")):
        S.paint(base, cloth(HIDE, SEED + 40, grain=0.08, folds=[(5, 0, 8, 0.0), (13, 0, 8, 0.0)]), rows=(0, 8))
        S.paint(base, lambda x, y, i, j, f, fx: tone(HIDE, 0.08), faces=("top",))
        S.paint(base, skin_tone(SKIN, SEED + 41), rows=(8, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(SKIN, -0.2), faces=("bottom",))
        S.paint(over, cloth(tone(HIDE, 0.04), SEED + 42, grain=0.08), rows=(0, 7))
        for i in range(16):
            S.sp(over, i, 7, tone(FUR_TRIM, -0.12 + 0.2 * h2(i, 7, SEED + 43)))
            if h2(i, 7, SEED + 44) > 0.78:
                S.sp(over, i, 7, None)
            S.sp(over, i, 8, tone(FUR_TRIM, -0.2 + 0.2 * h2(i, 8, SEED + 45)) if h2(i, 8, SEED + 46) > 0.5 else None)
        freckles(S, over, (0, 3), GRIT, 0.2, SEED + 47, strength=0.6)
        wrinkles(S, base, 4, SEED + 48, density=0.5)
        # 手背汗毛 / 青筋
        for i in range(4, 8):
            if h2(i, 10, SEED + 49) > 0.6:
                S.sp(base, i, 10, tone(SKIN, -0.16))
    # 右前臂一圈咬痕（环形）
    for i, j in ((4, 9), (5, 9), (6, 9), (7, 9), (4, 10), (7, 10)):
        S.sp("rarm", i, j, tone(BITE, -0.05 * (i % 2)))
    for i, j in ((5, 10), (6, 10)):
        S.sp("rarm", i, j, tone(SKIN, -0.18))
    # 右手食指根勒痕
    for i in (5, 6):
        S.sp("rarm", i, 11, tone(C("#7a5036"), -0.1))

    # ── 腿：皮裤，皮靴 + 靴口浅毛；膝盖磨白 ─────────────────────────────────────────
    for base, over in (("rleg", "rpants"), ("lleg", "lpants")):
        S.paint(base, cloth(TROUSER, SEED + 50, grain=0.075, folds=[(5, 0, 8, 0.0), (13, 0, 8, 0.0)]), rows=(0, 8))
        S.paint(base, cloth(BOOT, SEED + 51, grain=0.075, folds=[(6, 8, 12, 0.0), (14, 8, 12, 0.0)]), rows=(8, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(BOOT, -0.2), faces=("bottom",))
        S.paint(base, lambda x, y, i, j, f, fx: tone(TROUSER, -0.15), faces=("top",))
        # 靴口翻浅毛（第二层）
        S.paint(over, cloth(tone(FUR_TRIM, -0.15), SEED + 52, grain=0.14), rows=(6, 8))
        for i in range(16):
            if h2(i, 8, SEED + 53) > 0.55:
                S.sp(over, i, 8, tone(FUR_TRIM, -0.25))
        # 皮靴绑绳
        for j in (9, 10):
            for i in (5, 6):
                S.sp(base, i, j, tone(THREAD, -0.3 if j == 9 else -0.15))
        # 膝盖磨白
        for i, j in ((4, 4), (5, 4), (6, 4), (5, 3), (6, 5)):
            c, _ = S.sget(base, i, j)
            S.sp(base, i, j, mix(c, C("#8a7a64"), 0.5))
        wrinkles(S, base, 4, SEED + 54, cols=(4, 8), density=0.55)
        dust_gradient(S, base, GRIT, 4, 8, 0.35, SEED + 55, t_max=0.3)
        freckles(S, base, (8, 12), GRIT, 0.1, SEED + 56, strength=0.5)
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
                   notes="只含已做的复用装备；专属新装备（兽皮披肩）由 pi-b 另做，不在此图。")


if __name__ == "__main__":
    main()
