#!/usr/bin/env python3
"""残峰散修 can_feng_san_xiu 的 64×64 classic 皮肤。设计稿：model-review/npcs/can_feng_san_xiu.md。

用法：
    python3 modelScript/npcs/can_feng_san_xiu/gen_can_feng_san_xiu_skin.py            # 皮肤 PNG + 审阅图
    python3 modelScript/npcs/can_feng_san_xiu/gen_can_feng_san_xiu_skin.py --no-sheet # 只出皮肤 PNG
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

NPC_ID = "can_feng_san_xiu"
TITLE = "残峰散修"
SEED = 202
GEAR = ["cu_ma_dou_peng", "gu_pian_jian_jia", "pi_sheng_hu_wan"]
REVIEW_DIR = Path(os.environ.get("BONG_MODEL_REVIEW_DIR", "/home/serverkizuna/Code/Bong/.agent-worktrees/model-review"))

SKIN = C("#a67c58")
HAIR = C("#2e2622")
INNER = C("#8d8068")
ROBE = C("#4a5058")
ROBE_WORN = C("#6a7078")
TROUSER = C("#3a3a3c")
BOOT = C("#30261f")
STRAP = C("#5a4a38")
BANDAGE = C("#bdb39c")
SCAR = C("#c08a78")
BURN = C("#2a2420")
SCAB = C("#4a2a22")
THREAD = C("#8a8478")

PALETTE = [("肤色", "#a67c58"), ("头发", "#2e2622"), ("内衣", "#8d8068"), ("外衫", "#4a5058"), ("磨损边", "#6a7078"),
           ("裤", "#3a3a3c"), ("靴", "#30261f"), ("绷带", "#bdb39c"), ("疤", "#c08a78"), ("焦痕", "#2a2420"),
           ("掌痂", "#4a2a22")]


def build() -> Skin:
    S = Skin(SEED)

    # ── 头：披肩长发、窄脸；左眉到颧骨一道斜疤 ───────────────────────────────────────
    paint_head(S, SKIN, HAIR, SEED, front_h=[2, 2, 1, 1, 1, 2, 2, 3], side_h=[8, 8, 7, 6, 3, 3, 2, 2], back_h=8,
               eyes_y=4, eye=C("#241f1c"), eye_white=C("#b9b0a0"), brow=C("#1e1a18"))
    fx0, fy0, _, _ = face_rect("head", "front")
    # 眼窝深：眼下一行压暗
    for fx in (1, 2, 5, 6):
        c, a = S.get(fx0 + fx, fy0 + 5)
        S.put(fx0 + fx, fy0 + 5, tone(c, -0.12), a)
    # 左眉骨到颧骨的斜疤（画面右侧，3 格斜下），眉在疤处断开
    for fx, fy in ((5, 3), (6, 4), (6, 5), (7, 6)):
        S.put(fx0 + fx, fy0 + fy, SCAR if fy != 5 else tone(SCAR, -0.1))
    S.put(fx0 + 6, fy0 + 3, tone(SKIN, 0.0))
    # 薄唇 + 下颌胡茬一点点
    for fx in (2, 3, 4, 5):
        if h2(fx, 7, SEED + 5) > 0.6:
            S.put(fx0 + fx, fy0 + 7, mix(tone(SKIN, -0.12), HAIR, 0.3))
    # hat 层：披到肩的长发，后摆参差
    hat_hair(S, HAIR, SEED, back_rows=8, side_rows=6, side_cols=3, top_wisps=5)

    # ── 躯干：粗麻内衣（基础层）+ 靛灰外衫（jacket）──────────────────────────────────
    S.paint("body", cloth(INNER, SEED + 20, grain=0.06, folds=[(6, 1, 9, 0.0), (18, 0, 9, 0.0)]), rows=(0, 11))
    S.paint("body", cloth(TROUSER, SEED + 22, grain=0.05), rows=(11, 12))
    S.paint("body", skin_tone(tone(SKIN, -0.1), SEED + 23), faces=("top",))
    S.paint("body", lambda x, y, i, j, f, fx: tone(TROUSER, -0.2), faces=("bottom",))
    for i, j in ((7, 0), (8, 0), (6, 0), (9, 0), (7, 1), (8, 1)):
        S.sp("body", i, j, tone(SKIN, -0.1))
    # 外衫：到大腿根（row 11），腰上一条旧皮带
    folds = [(5, 1, 10, 0.0), (10, 1, 10, 0.1), (19, 0, 10, 0.0), (22, 2, 10, -0.15)]
    S.paint("jacket", cloth(ROBE, SEED + 30, grain=0.06, folds=folds, ao_rows=(0, 12)), rows=(0, 12))
    S.paint("jacket", lambda x, y, i, j, f, fx: tone(ROBE, 0.06), faces=("top",))
    # 交领：左襟压右襟，斜线
    for k in range(5):
        S.sp("jacket", 8 - k // 2, k, tone(ROBE, -0.32))
        S.sp("jacket", 9 - k // 2, k, tone(ROBE, 0.1))
    for i, j in ((7, 0), (8, 0), (7, 1)):
        S.sp("jacket", i, j, None)
    for i, j in ((6, 0), (9, 0), (6, 1), (8, 2)):
        S.sp("jacket", i, j, ROBE_WORN)
    # 皮带
    for i in range(24):
        S.sp("jacket", i, 8, tone(C("#3a3028"), -0.05 + 0.1 * h2(i, 8, SEED + 31)))
    for i in (7, 8):
        S.sp("jacket", i, 8, tone(C("#8a7a50"), 0.0))
    # 宗门徽记被撕掉：左胸一块颜色略深的方形底 + 毛边线头
    for di in range(3):
        for dj in range(3):
            S.sp("jacket", 9 + di, 2 + dj, tone(ROBE, -0.16 + 0.06 * h2(di, dj, SEED + 32)))
    for i, j in ((9, 1), (11, 5), (10, 5), (12, 3)):
        S.sp("jacket", i, j, THREAD)
    # 左襟下摆烧焦一角
    for i, j in ((8, 11), (9, 11), (10, 11), (9, 10), (10, 10), (11, 11), (11, 10), (8, 10)):
        S.sp("jacket", i, j, tone(BURN, 0.12 * h2(i, j, SEED + 33)))
    for i, j in ((8, 11), (9, 11), (11, 11)):
        S.sp("jacket", i, j, None)
    # 下摆磨毛、缺口
    for i in range(24):
        if h2(i, 11, SEED + 34) > 0.72:
            S.sp("jacket", i, 11, None)
        else:
            S.sp("jacket", i, 11, tone(ROBE_WORN, -0.1 + 0.15 * h2(i, 10, SEED + 35)))
    S.stain("jacket", tone(ROBE, -0.35), 0.85, SEED + 36, scale=3.0, thr=0.56, rows=(0, 12))
    S.stain("jacket", ROBE_WORN, 0.7, SEED + 37, scale=2.2, thr=0.66, rows=(0, 12))
    wrinkles(S, "jacket", 7, SEED + 38, density=0.6)
    dust_gradient(S, "jacket", C("#7a766c"), 7, 12, 0.5, SEED + 39, t_max=0.3)
    # 胸前一处缝线
    for i, j in ((5, 3), (5, 4), (6, 5), (6, 6)):
        S.sp("jacket", i, j, THREAD)

    # ── 臂：右袖完整到腕、左袖撕到上臂，露出缠绷带的小臂 ───────────────────────────────
    # 右臂
    S.paint("rarm", cloth(ROBE, SEED + 40, grain=0.06, folds=[(5, 0, 9, 0.0), (13, 0, 9, 0.0)]), rows=(0, 9))
    S.paint("rarm", lambda x, y, i, j, f, fx: tone(ROBE, 0.06), faces=("top",))
    S.paint("rarm", skin_tone(SKIN, SEED + 41), rows=(9, 12))
    S.paint("rarm", lambda x, y, i, j, f, fx: tone(SKIN, -0.22), faces=("bottom",))
    S.paint("rsleeve", cloth(tone(ROBE, 0.04), SEED + 42, grain=0.06), rows=(0, 9))
    for i in range(16):
        if h2(i, 9, SEED + 43) > 0.45:
            S.sp("rsleeve", i, 8, tone(ROBE_WORN, -0.1))
        S.sp("rsleeve", i, 9, None if h2(i, 9, SEED + 44) > 0.5 else tone(ROBE_WORN, -0.2))
    # 右手掌心黑色旧痂（右臂内侧 = left 面 i 8..11）
    for i, j in ((9, 10), (10, 10), (9, 11), (10, 11), (11, 10)):
        S.sp("rarm", i, j, tone(SCAB, 0.1 * h2(i, j, SEED + 45)))
    # 右手背几道刀茧
    for i in (5, 6, 7):
        S.sp("rarm", i, 10, tone(SKIN, -0.14))
    # 左臂：上臂袖子（撕断在 row 4，参差），下面全是绷带
    S.paint("larm", cloth(ROBE, SEED + 46, grain=0.06, folds=[(5, 0, 5, 0.0), (13, 0, 5, 0.0)]), rows=(0, 5))
    S.paint("larm", lambda x, y, i, j, f, fx: tone(ROBE, 0.06), faces=("top",))
    S.paint("larm", skin_tone(SKIN, SEED + 47), rows=(5, 12))
    S.paint("larm", lambda x, y, i, j, f, fx: tone(SKIN, -0.22), faces=("bottom",))
    S.paint("lsleeve", cloth(tone(ROBE, 0.04), SEED + 48, grain=0.06), rows=(0, 4))
    for i in range(16):
        n = h2(i, 4, SEED + 49)
        S.sp("lsleeve", i, 4, tone(ROBE_WORN, -0.15) if n > 0.4 else None)
        if n > 0.8:
            S.sp("lsleeve", i, 3, None)
    # 绷带：row 5..9 斜缠，row 10..11 手腕露出手
    for j in range(5, 10):
        for i in range(16):
            band = (i + j) % 3
            k = (-0.1, 0.05, -0.02)[band] + (h2(i, j, SEED + 50) - 0.5) * 0.1
            S.sp("larm", i, j, tone(BANDAGE, k))
    for i in range(16):
        S.sp("larm", i, 5, tone(BANDAGE, -0.22))
        S.sp("larm", i, 9, tone(BANDAGE, -0.18))
    # 绷带渗血与脏污
    for i, j in ((9, 7), (10, 7), (10, 8), (5, 6), (13, 7)):
        S.sp("larm", i, j, tone(C("#7a3a30"), 0.1 * h2(i, j, SEED + 51)))
    freckles(S, "larm", (5, 10), C("#8a7a60"), 0.25, SEED + 52, strength=0.5)

    # ── 腿：深灰裤 + 左膝补丁 + 靴 ────────────────────────────────────────────────
    for base, over in (("rleg", "rpants"), ("lleg", "lpants")):
        S.paint(base, cloth(TROUSER, SEED + 60, grain=0.06, folds=[(5, 0, 8, 0.0), (13, 0, 8, 0.0)]), rows=(0, 8))
        S.paint(base, cloth(BOOT, SEED + 61, grain=0.06, folds=[(6, 8, 12, 0.0), (14, 8, 12, 0.0)]), rows=(8, 12))
        S.paint(base, lambda x, y, i, j, f, fx: tone(BOOT, -0.2), faces=("bottom",))
        S.paint(base, lambda x, y, i, j, f, fx: tone(TROUSER, -0.15), faces=("top",))
        # 靴口束带
        for i in range(16):
            S.sp(base, i, 8, tone(STRAP, -0.05 + 0.15 * h2(i, 8, SEED + 62)))
        S.sp(base, 5, 8, tone(C("#8a7a50"), 0.0))
        # 靴面锈渍与积灰
        freckles(S, base, (9, 12), C("#7a4a2a"), 0.2, SEED + 63, strength=0.5)
        # 第二层：靴筒外翻一圈
        S.paint(over, cloth(tone(BOOT, 0.04), SEED + 64, grain=0.06), rows=(7, 8))
        # 大腿外衫下摆（只在前后面，两腿之间自然分开）
        S.paint(over, cloth(tone(ROBE, -0.02), SEED + 65, grain=0.06), rows=(0, 2), faces=("front", "back", "right", "left"))
        for i in range(16):
            n = h2(i, 2, SEED + 66)
            if n > 0.5:
                S.sp(over, i, 1, tone(ROBE_WORN, -0.12))
            if n > 0.7:
                S.sp(over, i, 1, None)
    S.patch("lleg", 8, 4, 3, 3, C("#53493d"), SEED + 67, stitch=THREAD)
    for part in ("rleg", "lleg"):
        wrinkles(S, part, 4, SEED + 68, cols=(4, 8), density=0.5)
        dust_gradient(S, part, C("#6a6458"), 4, 8, 0.5, SEED + 69, t_max=0.35)
    for part in ("rarm",):
        wrinkles(S, part, 4, SEED + 70, density=0.5)
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
                   notes="只含已做的复用装备；专属新装备（残剑鞘背带）由 pi-a 另做，不在此图。")


if __name__ == "__main__":
    main()
