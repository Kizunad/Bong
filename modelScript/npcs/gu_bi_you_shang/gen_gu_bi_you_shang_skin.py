#!/usr/bin/env python3
"""骨币游商 gu_bi_you_shang 的 64×64 classic 皮肤。设计稿：model-review/npcs/gu_bi_you_shang.md。

用法：
    python3 modelScript/npcs/gu_bi_you_shang/gen_gu_bi_you_shang_skin.py            # 出皮肤 PNG + 审阅图
    python3 modelScript/npcs/gu_bi_you_shang/gen_gu_bi_you_shang_skin.py --no-sheet # 只出皮肤 PNG
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "_kit"))
from skin_kit import (C, Skin, cloth, dust_gradient, face_rect, freckles, h2, mix, paint_head, quant, skin_tone,  # noqa: E402
                      tone, wrinkles)

NPC_ID = "gu_bi_you_shang"
TITLE = "骨币游商"
SEED = 101
GEAR = ["xiu_kuang_bei_jia", "coin_box", "ku_teng_yao_dai"]
REVIEW_DIR = Path(os.environ.get("BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"))

SKIN = C("#b98b63")
HAIR = C("#4a4038")
GREY = C("#8a847a")
INNER = C("#a8946a")
COAT = C("#6b5a3e")
COAT_EDGE = C("#8a7650")
TROUSER = C("#4a3a2a")
SHOE = C("#3a2e24")
BELT = C("#5a4630")
ROPE = C("#8c7a52")
CINNABAR = C("#a8402e")
PATCH_GREEN = C("#4e5a3a")
PATCH_RED = C("#7a4a35")
PATCH_KNEE = C("#5a3f2b")
THREAD = C("#b8a67c")

PALETTE = [("肤色", "#b98b63"), ("头发", "#4a4038"), ("灰发", "#8a847a"), ("内衬", "#a8946a"), ("外褂", "#6b5a3e"),
           ("褂边", "#8a7650"), ("裤", "#4a3a2a"), ("鞋", "#3a2e24"), ("腰带", "#5a4630"), ("麻绳", "#8c7a52"),
           ("丹砂印", "#a8402e"), ("补丁绿", "#4e5a3a"), ("补丁皮", "#7a4a35")]


def build() -> Skin:
    S = Skin(SEED)

    # ── 头：灰黑短发、胡茬、左眉断疤 ───────────────────────────────────────────
    paint_head(S, SKIN, HAIR, SEED, front_h=[2, 1, 1, 2, 2, 1, 1, 2], side_h=[5, 5, 5, 4, 3, 2, 2, 1], back_h=5,
               hair_hi=GREY, eyes_y=4, streak=GREY, streak_frac=0.28, eye=C("#2e2822"), eye_white=C("#bdb4a0"))
    fx0, fy0, _, _ = face_rect("head", "front")
    # 下巴灰胡茬（嘴下两行稀疏点）
    for fx in range(1, 7):
        for fy in (6, 7):
            if h2(fx, fy, SEED + 5) > 0.62:
                S.put(fx0 + fx, fy0 + fy, mix(tone(SKIN, -0.14), GREY, 0.4))
    # 颧骨下的深纹 + 眼角细纹
    S.put(fx0 + 1, fy0 + 5, tone(SKIN, -0.16))
    S.put(fx0 + 6, fy0 + 5, tone(SKIN, -0.16))
    S.put(fx0 + 0, fy0 + 4, tone(SKIN, -0.12))
    S.put(fx0 + 7, fy0 + 4, tone(SKIN, -0.12))
    # 左眉（画面右）断了一截：fx5 眉毛抹成疤色，fx6 保留
    S.put(fx0 + 5, fy0 + 3, C("#c9a07e"))
    S.put(fx0 + 5, fy0 + 2, tone(C("#c9a07e"), -0.1))
    # hat 层：脑后扎起的一小撮 + 头顶蓬起的几缕
    for fx, fy in ((3, 5), (4, 5), (3, 6), (4, 6), (3, 7), (4, 7)):
        x0, y0, _, _ = face_rect("hat", "back")
        S.put(x0 + fx, y0 + fy, tone(HAIR, -0.1 + 0.25 * h2(fx, fy, SEED + 6)))
    x0, y0, _, _ = face_rect("hat", "back")
    S.put(x0 + 3, y0 + 8 - 1, tone(GREY, -0.1))
    for fx, fy in ((2, 0), (5, 0), (6, 1)):
        x0, y0, _, _ = face_rect("hat", "top")
        S.put(x0 + fx, y0 + fy, tone(HAIR, 0.1))

    # ── 躯干：内衬 + 窄腰带（基础层）─────────────────────────────────────────────
    S.paint("body", cloth(INNER, SEED + 20, grain=0.05, folds=[(6, 1, 8, 0.0), (18, 0, 9, 0.0)]), rows=(0, 9))
    S.paint("body", cloth(BELT, SEED + 21, grain=0.04), rows=(9, 11))
    # 腰带上的磨痕与一粒铜扣色点
    S.sp("body", 7, 9, tone(BELT, 0.18))
    S.sp("body", 8, 9, tone(BELT, 0.12))
    S.paint("body", cloth(TROUSER, SEED + 22, grain=0.05), rows=(11, 12))
    S.paint("body", skin_tone(tone(SKIN, -0.1), SEED + 23), faces=("top",))
    S.paint("body", lambda x, y, i, j, f, fx: tone(TROUSER, -0.2), faces=("bottom",))
    # 领口露出的胸口皮肤（V 形）
    for i, j in ((7, 0), (8, 0), (7, 1), (8, 1), (6, 0), (9, 0), (7, 2)):
        S.sp("body", i, j, tone(SKIN, -0.08))
    # 内衬污渍
    S.stain("body", tone(INNER, -0.35), 0.8, SEED + 24, scale=2.4, thr=0.58, rows=(0, 9))

    # ── jacket：长褂，盖到胯上（j 0..9），下摆磨毛 ─────────────────────────────────
    folds = [(5, 1, 9, 0.1), (10, 2, 9, -0.1), (19, 1, 9, 0.0), (22, 2, 9, -0.2), (2, 3, 9, 0.0)]
    S.paint("jacket", cloth(COAT, SEED + 30, grain=0.055, folds=folds, ao_rows=(0, 10)), rows=(0, 10))
    S.paint("jacket", lambda x, y, i, j, f, fx: tone(COAT, 0.05), faces=("top",))
    # 前襟中缝与领口 V 形开口
    for j in range(0, 10):
        S.sp("jacket", 7, j, tone(COAT, -0.3))
        S.sp("jacket", 8, j, tone(COAT, 0.08))
    for i, j in ((7, 0), (8, 0), (6, 0), (9, 0), (7, 1), (8, 1), (7, 2), (8, 2), (7, 3)):
        S.sp("jacket", i, j, None)
    # 领口两侧的翻领亮边
    for i, j in ((5, 0), (10, 0), (6, 1), (9, 1), (7, 3), (8, 3), (6, 2), (9, 2)):
        S.sp("jacket", i, j, COAT_EDGE)
    # 下摆：磨白毛边 + 缺口
    for i in range(24):
        n = h2(i, 9, SEED + 31)
        S.sp("jacket", i, 9, tone(COAT_EDGE, -0.12 + 0.2 * h2(i, 8, SEED + 32)))
        if n > 0.8:
            S.sp("jacket", i, 9, None)
        if h2(i, 8, SEED + 33) > 0.9:
            S.sp("jacket", i, 8, None)
    # 左胸（画面右）方补丁，灰绿布
    S.patch("jacket", 9, 2, 3, 4, PATCH_GREEN, SEED + 34, stitch=THREAD)
    # 右腋下的一条旧缝线 + 后背两道汗渍
    for j in range(3, 8):
        S.sp("jacket", 3, j, tone(COAT, -0.22))
    S.stain("jacket", tone(COAT, -0.3), 0.85, SEED + 35, scale=3.0, thr=0.55, rows=(0, 10))
    S.stain("jacket", COAT_EDGE, 0.7, SEED + 36, scale=2.2, thr=0.66, rows=(0, 10))
    # 腰束处的横皱、肩头与下摆的积灰
    wrinkles(S, "jacket", 7, SEED + 37, density=0.6)
    wrinkles(S, "jacket", 4, SEED + 38, cols=(4, 12), density=0.3, dark=-0.1)
    dust_gradient(S, "jacket", C("#9a8a68"), 5, 10, 0.55, SEED + 39, t_max=0.35)

    # ── 臂：长褂袖卷起两折，前臂露出；左手丹砂印 ──────────────────────────────────
    for base, over in (("rarm", "rsleeve"), ("larm", "lsleeve")):
        S.paint(base, cloth(COAT, SEED + 40, grain=0.05, folds=[(5, 0, 6, 0.0), (13, 0, 6, 0.0)]), rows=(0, 6))
        S.paint(base, lambda x, y, i, j, f, fx: tone(COAT, 0.04), faces=("top",))
        S.paint(base, skin_tone(SKIN, SEED + 41), rows=(6, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(SKIN, -0.22), faces=("bottom",))
        # 卷袖的两折（第二层，略外鼓）
        S.paint(over, cloth(tone(COAT, 0.08), SEED + 42, grain=0.05, folds=[(6, 0, 3, 0.0), (14, 0, 3, 0.0)]), rows=(4, 6))
        S.paint(over, cloth(tone(COAT, -0.06), SEED + 43, grain=0.05), rows=(6, 7))
        for i in range(16):
            S.sp(over, i, 4, tone(COAT_EDGE, 0.0))
        # 前臂汗毛与青筋：几个浅点
        for i in range(16):
            for j in (8, 9):
                if h2(i, j, SEED + 44) > 0.82:
                    S.sp(base, i, j, tone(SKIN, -0.12))
    # 右肘补丁（第二层，皮色）
    S.patch("rsleeve", 12, 3, 3, 3, PATCH_RED, SEED + 45, stitch=THREAD)
    # 左手腕内侧到手背的丹砂红印
    for i in range(16):
        for j, p in ((7, 0.75), (8, 0.5)):
            if h2(i, j, SEED + 46) < p:
                c, _ = S.sget("larm", i, j)
                S.sp("larm", i, j, mix(c, CINNABAR, 0.6 if j == 7 else 0.4))
    for i in range(4, 8):
        for j in (9, 10, 11):
            if h2(i, j, SEED + 47) < 0.3:
                c, _ = S.sget("larm", i, j)
                S.sp("larm", i, j, mix(c, CINNABAR, 0.4))
    # 指尖 / 指节的粗糙感
    for part in ("rarm", "larm"):
        for i in range(4, 8):
            S.sp(part, i, 11, tone(SKIN, -0.18))

    # ── 腿：裤 + 右膝皮补丁 + 破鞋；第二层是裤脚绳扎 ─────────────────────────────────
    for base, over in (("rleg", "rpants"), ("lleg", "lpants")):
        S.paint(base, cloth(TROUSER, SEED + 50, grain=0.055, folds=[(5, 0, 9, 0.0), (13, 0, 9, 0.0)]), rows=(0, 10))
        S.paint(base, cloth(SHOE, SEED + 51, grain=0.06), rows=(10, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(SHOE, -0.2), faces=("bottom",))
        S.paint(base, lambda x, y, i, j, f, fx: tone(TROUSER, -0.15), faces=("top",))
        # 鞋头磨出的布白
        for i in (5, 6):
            S.sp(base, i, 11, tone(C("#8a7a60"), -0.2 + 0.2 * h2(i, 11, SEED + 52)))
        # 裤脚：绳扎处的第二层 + 绳结
        S.paint(over, cloth(tone(TROUSER, 0.05), SEED + 53, grain=0.05), rows=(7, 9))
        S.paint(over, cloth(ROPE, SEED + 54, grain=0.07, weave=0.04), rows=(9, 10))
        S.sp(over, 5, 9, tone(ROPE, 0.22))
        S.sp(over, 6, 9, tone(ROPE, -0.2))
        S.sp(over, 6, 10, tone(ROPE, -0.15))
        S.sp(over, 5, 10, tone(ROPE, 0.0))
    S.patch("rleg", 4, 5, 3, 3, PATCH_KNEE, SEED + 55, stitch=THREAD)
    for part in ("rleg", "lleg"):
        wrinkles(S, part, 5, SEED + 58, cols=(4, 8), density=0.5)
        wrinkles(S, part, 2, SEED + 59, density=0.3, dark=-0.1)
        dust_gradient(S, part, C("#7a6a50"), 6, 10, 0.6, SEED + 60, t_max=0.45)
    for part in ("rarm", "larm"):
        wrinkles(S, part, 5, SEED + 61, density=0.5)
        dust_gradient(S, part, C("#9a8a68"), 2, 6, 0.4, SEED + 62, t_max=0.3)
    S.stain("rleg", tone(TROUSER, -0.35), 0.7, SEED + 56, scale=2.6, thr=0.6, rows=(0, 10))
    S.stain("lleg", tone(TROUSER, -0.35), 0.7, SEED + 57, scale=2.6, thr=0.6, rows=(0, 10))
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
                   notes="只含已做的复用装备；专属新装备（骨秤）由 pi-a 另做，不在此图。")


if __name__ == "__main__":
    main()
