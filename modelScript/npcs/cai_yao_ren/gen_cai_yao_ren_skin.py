#!/usr/bin/env python3
"""采药人 cai_yao_ren 的 64×64 classic 皮肤。设计稿：model-review/npcs/cai_yao_ren.md。

用法：
    python3 modelScript/npcs/cai_yao_ren/gen_cai_yao_ren_skin.py            # 皮肤 PNG + 审阅图
    python3 modelScript/npcs/cai_yao_ren/gen_cai_yao_ren_skin.py --no-sheet # 只出皮肤 PNG
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

NPC_ID = "cai_yao_ren"
TITLE = "采药人"
SEED = 404
GEAR = ["herb_pouch", "cu_bu_mian_jin", "moisture_guard"]
REVIEW_DIR = Path(os.environ.get("BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"))

SKIN = C("#c09878")
HAIR = C("#4a3426")
RED_CORD = C("#8a3a2e")
INNER = C("#b8a880")
JACKET = C("#6a7050")
JACKET_EDGE = C("#8a8a6a")
TROUSER = C("#54483a")
MUD = C("#3e3426")
SANDAL = C("#8a7448")
HERB_GREEN = C("#7a8a4a")
PATCH_ELBOW = C("#8a7a52")
THREAD = C("#c0b48c")

PALETTE = [("肤色", "#c09878"), ("头发", "#4a3426"), ("红绳", "#8a3a2e"), ("内衣", "#b8a880"), ("苔染短褂", "#6a7050"),
           ("磨边", "#8a8a6a"), ("裤", "#54483a"), ("膝泥", "#3e3426"), ("草鞋", "#8a7448"), ("草汁绿", "#7a8a4a"),
           ("肘补丁", "#8a7a52")]


def build() -> Skin:
    S = Skin(SEED)

    # ── 头：栗色头发，脑后一条辫子垂到背上（hat 层），左耳缺口，颊上一点泥 ──────────────
    paint_head(S, SKIN, HAIR, SEED, front_h=[2, 1, 1, 2, 2, 1, 1, 2], side_h=[6, 6, 5, 4, 3, 2, 2, 1], back_h=5,
               eyes_y=4, eye=C("#2a2018"), eye_white=C("#d4ccba"), brow=C("#3a281c"), mouth=C("#a85a4a"))
    fx0, fy0, _, _ = face_rect("head", "front")
    # 圆脸：颊上淡红，鼻尖一点
    for fx, fy in ((1, 6), (6, 6), (3, 5)):
        c, a = S.get(fx0 + fx, fy0 + fy)
        S.put(fx0 + fx, fy0 + fy, mix(c, C("#d08068"), 0.3), a)
    # 左颊（画面右侧）一点泥
    for fx, fy in ((6, 5), (6, 6)):
        c, a = S.get(fx0 + fx, fy0 + fy)
        S.put(fx0 + fx, fy0 + fy, mix(c, C("#6e5a40"), 0.7), a)
    # 左耳缺口：左侧面（wearer 左）耳朵 3 格里缺一格（用头发 / 背景色不行，直接抠成暗肤）
    from skin_kit import face_rect as _fr
    lx, ly, _, _ = _fr("head", "left")
    S.put(lx + 3, ly + 3, tone(SKIN, -0.35))
    S.put(lx + 4, ly + 3, tone(SKIN, -0.3))
    # hat：辫子（后脑中缝一路向下，hat 背面 + 顶面几缕），发梢红绳在 body 背面之外，不在头上
    for fx in (3, 4):
        for fy in range(0, 8):
            x0, y0, _, _ = face_rect("hat", "back")
            S.put(x0 + fx, y0 + fy, tone(HAIR, (-0.1, 0.05, -0.04, 0.12)[(fy + fx) % 4]))
    x0, y0, _, _ = face_rect("hat", "back")
    for fx, fy in ((3, 6), (4, 6), (3, 7), (4, 7)):
        S.put(x0 + fx, y0 + fy, tone(RED_CORD, (0.1, -0.1, -0.15, 0.0)[(fx + fy) % 4]))  # 辫梢红绳
    hat_hair(S, HAIR, SEED, back_rows=0, side_rows=3, side_cols=2, top_wisps=3)

    # ── 躯干：浅米内衣（基础层）+ 苔染短褂（jacket，及腰下）──────────────────────────────
    S.paint("body", cloth(INNER, SEED + 20, grain=0.06, folds=[(6, 0, 11, 0.0), (18, 0, 11, 0.0)]), rows=(0, 11))
    S.paint("body", cloth(TROUSER, SEED + 22, grain=0.05), rows=(11, 12))
    S.paint("body", skin_tone(tone(SKIN, -0.1), SEED + 23), faces=("top",))
    S.paint("body", lambda x, y, i, j, f, fx: tone(TROUSER, -0.2), faces=("bottom",))
    for i, j in ((7, 0), (8, 0), (6, 0), (9, 0), (7, 1), (8, 1)):
        S.sp("body", i, j, tone(SKIN, -0.08))
    folds = [(5, 1, 10, 0.1), (10, 1, 10, -0.1), (18, 0, 10, 0.0), (22, 2, 10, -0.1)]
    S.paint("jacket", cloth(JACKET, SEED + 30, grain=0.065, folds=folds, ao_rows=(0, 10)), rows=(0, 10))
    S.paint("jacket", lambda x, y, i, j, f, fx: tone(JACKET, 0.07), faces=("top",))
    # 系带前襟：V 领，一根草绳斜系
    for i, j in ((7, 0), (8, 0), (7, 1), (8, 1), (6, 0), (9, 0)):
        S.sp("jacket", i, j, None)
    for i, j in ((5, 0), (10, 0), (6, 1), (9, 1)):
        S.sp("jacket", i, j, JACKET_EDGE)
    for k in range(5):
        S.sp("jacket", 6 + k, 2 + k // 2, tone(C("#8c7a52"), 0.05 * (k % 2)))
    # 下摆（磨成毛边）
    for i in range(24):
        n = h2(i, 9, SEED + 31)
        S.sp("jacket", i, 9, tone(JACKET_EDGE, -0.15 + 0.15 * h2(i, 8, SEED + 32)) if n < 0.8 else None)
    # 苔染渐变：下摆处褪成灰绿；胸口一块被草汁染深
    S.stain("jacket", tone(JACKET, -0.3), 0.8, SEED + 33, scale=2.8, thr=0.58, rows=(0, 10))
    S.stain("jacket", JACKET_EDGE, 0.7, SEED + 34, scale=2.4, thr=0.62, rows=(4, 10))
    freckles(S, "jacket", (2, 8), HERB_GREEN, 0.07, SEED + 35, strength=0.5)
    dust_gradient(S, "jacket", C("#7a7a5a"), 6, 10, 0.45, SEED + 36, t_max=0.3)
    wrinkles(S, "jacket", 7, SEED + 37, density=0.55)
    # 左肩一道旧缝线（画面右侧肩缝）
    for j in range(0, 4):
        S.sp("jacket", 12, j, tone(JACKET, -0.22))
    # 腰：窄布带
    for i in range(24):
        S.sp("body", i, 10, tone(C("#5a4a34"), -0.05 + 0.12 * h2(i, 10, SEED + 38)))

    # ── 臂：短褂袖到腕，袖口收窄；指尖草汁染绿 ──────────────────────────────────────
    for base, over in (("rarm", "rsleeve"), ("larm", "lsleeve")):
        S.paint(base, cloth(JACKET, SEED + 40, grain=0.065, folds=[(5, 0, 8, 0.0), (13, 0, 8, 0.0)]), rows=(0, 8))
        S.paint(base, lambda x, y, i, j, f, fx: tone(JACKET, 0.07), faces=("top",))
        S.paint(base, skin_tone(SKIN, SEED + 41), rows=(8, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(SKIN, -0.2), faces=("bottom",))
        # 袖口束带
        S.paint(over, cloth(tone(JACKET, 0.04), SEED + 42, grain=0.065), rows=(0, 7))
        for i in range(16):
            S.sp(over, i, 7, tone(C("#8c7a52"), -0.12 + 0.12 * h2(i, 7, SEED + 43)))
            if h2(i, 6, SEED + 44) > 0.7:
                S.sp(over, i, 6, tone(JACKET_EDGE, -0.1))
        # 指尖草汁染绿：row 10-11
        for i in range(4, 8):
            for j, p in ((10, 0.35), (11, 0.95)):
                if h2(i, j, SEED + 45) < p:
                    c, _ = S.sget(base, i, j)
                    S.sp(base, i, j, mix(c, HERB_GREEN, 0.75))
        # 手背一道被叶割的细红线（右手）
        wrinkles(S, base, 5, SEED + 46, density=0.5)
        dust_gradient(S, over, C("#7a7a5a"), 3, 7, 0.4, SEED + 47, t_max=0.3)
    for i, j in ((6, 9), (6, 10), (7, 10)):
        S.sp("rarm", i, j, C("#b86a58"))
    # 右肘补丁
    S.patch("rsleeve", 12, 2, 3, 3, PATCH_ELBOW, SEED + 48, stitch=THREAD)

    # ── 腿：深褐裤，膝盖泥渍；草鞋 + 布条；裤脚两色不匀 ─────────────────────────────────
    for base, over in (("rleg", "rpants"), ("lleg", "lpants")):
        S.paint(base, cloth(TROUSER, SEED + 50, grain=0.065, folds=[(5, 0, 9, 0.0), (13, 0, 9, 0.0)]), rows=(0, 9))
        S.paint(base, cloth(SANDAL, SEED + 51, grain=0.08, weave=0.04), rows=(9, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(SANDAL, -0.25), faces=("bottom",))
        S.paint(base, lambda x, y, i, j, f, fx: tone(TROUSER, -0.15), faces=("top",))
        # 草鞋缠脚布条
        for j in range(9, 11):
            for i in range(16):
                if (i + j) % 3 == 0:
                    S.sp(base, i, j, tone(C("#a89870"), -0.1))
        # 膝盖泥渍（前面）
        for i, j in ((4, 5), (5, 5), (6, 5), (7, 5), (5, 6), (6, 6), (4, 6)):
            c, _ = S.sget(base, i, j)
            S.sp(base, i, j, mix(c, MUD, 0.65))
        S.paint(over, cloth(tone(TROUSER, 0.05), SEED + 52, grain=0.065), rows=(7, 9))
        wrinkles(S, base, 4, SEED + 53, cols=(4, 8), density=0.55)
        dust_gradient(S, base, MUD, 5, 9, 0.5, SEED + 54, t_max=0.5)
    # 右腿裤脚后半截换了一块布（颜色不同）
    for i in range(8, 16):
        for j in range(6, 9):
            S.sp("rpants", i, j, tone(C("#6a5a40"), -0.05 + 0.12 * h2(i, j, SEED + 55)))
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
                   notes="只含已做的复用装备；专属新装备（药锄 / 草药捆）由 pi-a 另做，不在此图。")


if __name__ == "__main__":
    main()
