#!/usr/bin/env python3
"""碎矿工 sui_kuang_gong 的 64×64 classic 皮肤。设计稿：model-review/npcs/sui_kuang_gong.md（护目已换成粗布面巾）。

用法：
    python3 modelScript/npcs/sui_kuang_gong/gen_sui_kuang_gong_skin.py            # 皮肤 PNG + 审阅图
    python3 modelScript/npcs/sui_kuang_gong/gen_sui_kuang_gong_skin.py --no-sheet # 只出皮肤 PNG
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

NPC_ID = "sui_kuang_gong"
TITLE = "碎矿工"
SEED = 808
GEAR = ["ore_sack", "cu_bu_bang_tui", "cu_bu_mian_jin"]
REVIEW_DIR = Path(os.environ.get("BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"))

SKIN = C("#8a7458")
DUST = C("#a89c88")
HAIR = C("#26221e")
INNER = C("#4a4034")
OVERALL = C("#6a5a44")
RUST = C("#8a4a2a")
TROUSER = C("#54483a")
PATCH_KNEE = C("#3a2c20")
BOOT = C("#2e2620")
BANDAGE = C("#bdb39c")
SCAB = C("#3a2a22")
STRAP_MARK = C("#5a4a34")
THREAD = C("#a89c80")

PALETTE = [("肤色", "#8a7458"), ("矿粉灰", "#a89c88"), ("头发", "#26221e"), ("内衣", "#4a4034"), ("粗布工装", "#6a5a44"),
           ("铁锈橙", "#8a4a2a"), ("裤", "#54483a"), ("膝皮补丁", "#3a2c20"), ("重靴", "#2e2620"), ("白布", "#bdb39c"),
           ("黑痂", "#3a2a22"), ("肩带勒痕", "#5a4a34")]


def dust_patch(S: Skin, part: str, rows: tuple[int, int], p: float, seed: int, strength: float = 0.6, cols=None) -> None:
    freckles(S, part, rows, DUST, p, seed, strength=strength, cols=cols)


def build() -> Skin:
    S = Skin(SEED)

    # ── 头：黑短发，脸被矿粉盖住，汗痕处露出底色；右颧骨一道黑痂 ──────────────────────────
    paint_head(S, SKIN, HAIR, SEED, front_h=[1, 1, 1, 1, 1, 1, 1, 1], side_h=[5, 5, 4, 3, 3, 2, 2, 1], back_h=5,
               eyes_y=4, eye=C("#1e1a16"), eye_white=C("#cfc6b0"), brow=C("#1e1a16"), mouth=C("#5a3a2e"))
    fx0, fy0, _, _ = face_rect("head", "front")
    # 矿粉：整张脸盖一层灰，汗痕（眼下两道竖、嘴角、额头一道）露出底色
    for fy in range(8):
        for fx in range(8):
            if (fx, fy) in ((1, 5), (1, 6), (6, 5), (6, 6), (0, 6), (7, 6), (3, 1), (4, 1), (3, 2)):
                continue
            c, a = S.get(fx0 + fx, fy0 + fy)
            if (fy <= 3 and fx in (1, 2, 5, 6) and fy >= 3) or (fy == 4 and fx in (1, 2, 5, 6)):
                continue  # 眼睛周围保持清晰
            if fy == 0 or h2(fx, fy, SEED + 5) < 0.62:
                S.put(fx0 + fx, fy0 + fy, mix(c, DUST, 0.55 if fy >= 5 else 0.4), a)
    # 汗痕的深色边
    for fx, fy in ((1, 5), (1, 6), (6, 5), (6, 6)):
        c, a = S.get(fx0 + fx, fy0 + fy)
        S.put(fx0 + fx, fy0 + fy, tone(c, -0.12), a)
    # 右颧骨黑痂（画面左侧）：被碎石划开
    for fx, fy in ((1, 5), (2, 5), (2, 6)):
        S.put(fx0 + fx, fy0 + fy, tone(SCAB, 0.08 * ((fx + fy) % 3)))
    # 眉毛、鬓角落灰
    for fx in (1, 2, 5, 6):
        c, a = S.get(fx0 + fx, fy0 + 3)
        S.put(fx0 + fx, fy0 + 3, mix(c, DUST, 0.5), a)
    hat_hair(S, HAIR, SEED, back_rows=0, side_rows=0, side_cols=1, top_wisps=5)
    # 头发上落矿粉
    for face in ("top", "back"):
        x0, y0, fw, fh = face_rect("head", face)
        for fy in range(fh):
            for fx in range(fw):
                if h2(fx, fy, SEED + 6) > 0.8:
                    c, a = S.get(x0 + fx, y0 + fy)
                    if a > 0:
                        S.put(x0 + fx, y0 + fy, mix(c, DUST, 0.5), a)

    # ── 躯干：深褐内衣 + 粗布工装外套（jacket），铁锈橙污渍；肩带勒痕斜压过去 ───────────────
    S.paint("body", cloth(INNER, SEED + 20, grain=0.06, folds=[(6, 0, 11, 0.0), (18, 0, 11, 0.0)]), rows=(0, 11))
    S.paint("body", cloth(TROUSER, SEED + 22, grain=0.06), rows=(11, 12))
    S.paint("body", skin_tone(tone(SKIN, -0.1), SEED + 23), faces=("top",))
    S.paint("body", lambda x, y, i, j, f, fx: tone(TROUSER, -0.2), faces=("bottom",))
    for i, j in ((7, 0), (8, 0), (6, 0), (9, 0), (7, 1), (8, 1)):
        S.sp("body", i, j, mix(tone(SKIN, -0.1), DUST, 0.35))
    folds = [(5, 1, 10, 0.1), (10, 1, 10, -0.1), (18, 0, 10, 0.0), (21, 1, 10, 0.1)]
    S.paint("jacket", cloth(OVERALL, SEED + 30, grain=0.07, folds=folds, ao_rows=(0, 11)), rows=(0, 11))
    S.paint("jacket", lambda x, y, i, j, f, fx: tone(OVERALL, 0.08), faces=("top",))
    for i, j in ((7, 0), (8, 0), (7, 1), (8, 1), (6, 0), (9, 0)):
        S.sp("jacket", i, j, None)
    for i, j in ((5, 0), (10, 0), (6, 1), (9, 1), (7, 2), (8, 2)):
        S.sp("jacket", i, j, tone(OVERALL, 0.2))
    # 前襟：一排粗布纽（暗色小块）
    for j in (3, 5, 7):
        S.sp("jacket", 8, j, tone(C("#3a3228"), 0.1))
    # 下摆参差
    for i in range(24):
        n = h2(i, 10, SEED + 31)
        S.sp("jacket", i, 10, tone(OVERALL, -0.22 + 0.15 * h2(i, 9, SEED + 32)) if n < 0.78 else None)
    # 铁锈橙污渍：胸口 / 肘 / 下摆
    S.stain("jacket", RUST, 0.85, SEED + 33, scale=2.8, thr=0.58, rows=(0, 11))
    S.stain("jacket", tone(OVERALL, -0.35), 0.7, SEED + 34, scale=2.4, thr=0.62, rows=(0, 11))
    freckles(S, "jacket", (0, 10), C("#a05a30"), 0.05, SEED + 35, strength=0.7)
    dust_patch(S, "jacket", (3, 11), 0.18, SEED + 36, 0.5)
    dust_gradient(S, "jacket", DUST, 6, 11, 0.45, SEED + 37, t_max=0.35)
    wrinkles(S, "jacket", 6, SEED + 38, density=0.55)
    # 矿石袋肩带勒痕：从右肩（画面左）斜到左腰，前后各一条略深的带
    for k in range(9):
        S.sp("jacket", 4 + (k * 4) // 5 + 0, 1 + k, tone(STRAP_MARK, -0.05))
        S.sp("jacket", 5 + (k * 4) // 5, 1 + k, tone(STRAP_MARK, 0.0))
    for k in range(9):
        S.sp("jacket", 22 - (k * 4) // 5, 1 + k, tone(STRAP_MARK, -0.05))
        S.sp("jacket", 21 - (k * 4) // 5, 1 + k, tone(STRAP_MARK, 0.0))

    # ── 臂：袖口扎布条；左手缠白布；手上满是矿粉 ─────────────────────────────────────
    for base, over in (("rarm", "rsleeve"), ("larm", "lsleeve")):
        S.paint(base, cloth(OVERALL, SEED + 40, grain=0.07, folds=[(5, 0, 8, 0.0), (13, 0, 8, 0.0)]), rows=(0, 8))
        S.paint(base, lambda x, y, i, j, f, fx: tone(OVERALL, 0.08), faces=("top",))
        S.paint(base, skin_tone(SKIN, SEED + 41), rows=(8, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(SKIN, -0.2), faces=("bottom",))
        S.paint(over, cloth(tone(OVERALL, 0.03), SEED + 42, grain=0.07), rows=(0, 7))
        # 袖口扎布条（旧白布）
        for i in range(16):
            S.sp(over, i, 7, tone(BANDAGE, -0.25 + 0.15 * h2(i, 7, SEED + 43)))
            if h2(i, 7, SEED + 44) > 0.8:
                S.sp(over, i, 7, None)
            S.sp(over, i, 6, tone(BANDAGE, -0.4 + 0.1 * h2(i, 6, SEED + 45)) if h2(i, 6, SEED + 46) > 0.4 else tone(OVERALL, -0.2))
        S.stain(over, RUST, 0.8, SEED + 47, scale=2.2, thr=0.6, rows=(0, 7))
        wrinkles(S, base, 4, SEED + 48, density=0.5)
        dust_gradient(S, over, DUST, 2, 7, 0.35, SEED + 49, t_max=0.3)
        # 手：矿粉盖住手背
        dust_patch(S, base, (8, 12), 0.55, SEED + 50, 0.55)
        for i in range(4, 8):
            S.sp(base, i, 11, tone(C("#5a4a38"), 0.1 * h2(i, 11, SEED + 51)))
    # 左手缠白布（无名指位置缺失，布缠得比别的手指窄）：手背 + 手掌各一层
    for j in range(8, 12):
        for i in range(0, 16):
            if i in range(4, 8) or i in range(0, 4) or i in range(8, 12):
                k = (-0.12, 0.04, -0.04)[(i + j) % 3] + (h2(i, j, SEED + 52) - 0.5) * 0.1
                S.sp("larm", i, j, tone(BANDAGE, k))
    for i in (6, 7):
        for j in (10, 11):
            S.sp("larm", i, j, tone(BANDAGE, -0.28))  # 窄一截，布缠得比别的手指紧
    for i in range(16):
        S.sp("larm", i, 8, tone(BANDAGE, -0.22))
    # 血迹渗出
    for i, j in ((5, 10), (6, 9), (9, 10)):
        S.sp("larm", i, j, tone(C("#7a3a30"), 0.1 * h2(i, j, SEED + 53)))
    dust_patch(S, "larm", (8, 12), 0.2, SEED + 54, 0.45)

    # ── 腿：褐裤，左膝皮补丁；重靴，裤脚鞋面落满矿粉、鞋头磨白 ──────────────────────────
    for base, over in (("rleg", "rpants"), ("lleg", "lpants")):
        S.paint(base, cloth(TROUSER, SEED + 60, grain=0.07, folds=[(5, 0, 9, 0.0), (13, 0, 9, 0.0)]), rows=(0, 9))
        S.paint(base, cloth(BOOT, SEED + 61, grain=0.07, folds=[(6, 9, 12, 0.0), (14, 9, 12, 0.0)]), rows=(9, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(BOOT, -0.2), faces=("bottom",))
        S.paint(base, lambda x, y, i, j, f, fx: tone(TROUSER, -0.15), faces=("top",))
        S.paint(over, cloth(tone(TROUSER, 0.05), SEED + 62, grain=0.07), rows=(7, 9))
        for i in range(16):
            if h2(i, 9, SEED + 63) > 0.5:
                S.sp(over, i, 8, tone(BOOT, 0.12))
        wrinkles(S, base, 4, SEED + 64, cols=(4, 8), density=0.55)
        dust_gradient(S, base, DUST, 3, 9, 0.5, SEED + 65, t_max=0.45)
        dust_gradient(S, over, DUST, 7, 9, 0.7, SEED + 66, t_max=0.5)
        dust_patch(S, base, (9, 12), 0.4, SEED + 67, 0.5)
        # 铁锈污渍
        S.stain(base, RUST, 0.6, SEED + 68, scale=2.4, thr=0.64, rows=(0, 9))
        # 鞋头磨白
        for i in (5, 6):
            S.sp(base, i, 11, tone(C("#8a7a64"), -0.15 + 0.15 * h2(i, 11, SEED + 69)))
    S.patch("lleg", 8, 4, 3, 3, PATCH_KNEE, SEED + 70, stitch=THREAD)
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
                   notes="只含已做的复用装备（护目已换成粗布面巾）；专属新装备（锈铁头盔）由 pi-b 另做，不在此图。")


if __name__ == "__main__":
    main()
