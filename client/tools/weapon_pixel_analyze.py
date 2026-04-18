#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy", "pillow"]
# ///
"""量化武器在 MC 截图中的 pixel/位置/角度(相对玩家 baseline)。

用法: python weapon_pixel_analyze.py [renders_dir]
  默认 renders_dir = 本文件同级的 renders/

指标:
  TP back/front — ratio = weapon_pixel_count / baseline_player_pixel_count
    目标 ≈ 0.002-0.003 (参考 placeholder_sword 唐刀)
  FP            — weapon mask bbox 中心 offset + PCA 主轴角度
    参考 vanilla iron_sword (placeholder_sword):
      - cx_frac 应偏右(+0.3 ~ +0.6) — 握柄在屏幕右侧
      - cy_frac 应偏下(+0.2 ~ +0.5) — 靠 hand 位置
      - 主轴 ~-45° ~ -60° (blade 从右下朝左上斜挑)
    垂直 ~-90° / 水平 0° 说明握姿不自然
"""
import sys
from pathlib import Path
import numpy as np
from PIL import Image


def is_background_mask(img: np.ndarray) -> np.ndarray:
    r, g, b = img[..., 0], img[..., 1], img[..., 2]
    sky = (r > 155) & (g > 195) & (b > 220) & (r < 250)
    cloud = (r > 225) & (g > 225) & (b > 225)
    return sky | cloud


def non_bg_count(path: Path) -> int:
    img = np.array(Image.open(path).convert("RGB"))
    return int((~is_background_mask(img)).sum())


def diff_mask(baseline_path: Path, weapon_path: Path, thresh: int = 30) -> np.ndarray:
    b = np.array(Image.open(baseline_path).convert("RGB")).astype(int)
    w = np.array(Image.open(weapon_path).convert("RGB")).astype(int)
    diff = np.abs(w - b).sum(axis=-1)
    return diff > thresh


def fp_geometry(mask: np.ndarray) -> dict:
    ys, xs = np.where(mask)
    if len(ys) == 0:
        return {"empty": True}
    h, w = mask.shape
    cx, cy = float(xs.mean()), float(ys.mean())
    sx, sy = w / 2, h / 2
    pts = np.column_stack([xs.astype(float), ys.astype(float)])
    pts_c = pts - pts.mean(axis=0)
    cov = np.cov(pts_c.T)
    eigvals, eigvecs = np.linalg.eigh(cov)
    main_axis = eigvecs[:, int(eigvals.argmax())]
    angle_deg = float(np.degrees(np.arctan2(main_axis[1], main_axis[0])))
    if angle_deg > 90:
        angle_deg -= 180
    elif angle_deg < -90:
        angle_deg += 180
    aspect = float(np.sqrt(eigvals.max() / max(eigvals.min(), 1e-6)))
    return {
        "pixel_count": int(mask.sum()),
        "center_x_frac": (cx - sx) / sx,
        "center_y_frac": (cy - sy) / sy,
        "main_axis_deg": angle_deg,
        "aspect_ratio": aspect,
    }


if __name__ == "__main__":
    renders = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent / "renders"
    baseline_back = renders / "_baseline" / "mc_thirdperson_back.png"
    baseline_front = renders / "_baseline" / "mc_thirdperson_front.png"
    baseline_fp = renders / "_baseline" / "mc_firstperson_righthand.png"
    if not baseline_back.exists():
        print(f"ERROR: baseline not found at {baseline_back}", file=sys.stderr)
        sys.exit(1)

    p_base_back = non_bg_count(baseline_back)
    p_base_front = non_bg_count(baseline_front)
    print(f"baseline player pixels: back={p_base_back}  front={p_base_front}\n")

    print(f"{'asset':<26} {'tp_back':>8} {'tp_front':>9}")
    print(f"{'':26} {'ratio':>8} {'ratio':>9}")
    print("-" * 45)
    for d in sorted(renders.iterdir()):
        if not d.is_dir() or d.name.startswith("_"):
            continue
        b = d / "mc_thirdperson_back.png"
        f = d / "mc_thirdperson_front.png"
        if not (b.exists() and f.exists()):
            continue
        r_b = (non_bg_count(b) - p_base_back) / p_base_back if p_base_back > 0 else 0
        r_f = (non_bg_count(f) - p_base_front) / p_base_front if p_base_front > 0 else 0
        print(f"{d.name:<26} {r_b:>8.3f} {r_f:>9.3f}")

    print(f"\n=== FP geometry (diff vs baseline FP) ===")
    print(f"(cx_frac: -1=左 0=中 +1=右 | cy_frac: -1=上 0=中 +1=下 | angle: -90=竖上 0=水平 +90=竖下)")
    print(f"{'asset':<26} {'px':>6} {'cx':>7} {'cy':>7} {'angle°':>8} {'aspect':>7}")
    print("-" * 70)
    if not baseline_fp.exists():
        print("(无 FP baseline, 跳过 FP 分析)")
    else:
        for d in sorted(renders.iterdir()):
            if not d.is_dir() or d.name.startswith("_"):
                continue
            fp = d / "mc_firstperson_righthand.png"
            if not fp.exists():
                continue
            mask = diff_mask(baseline_fp, fp)
            g = fp_geometry(mask)
            if g.get("empty"):
                print(f"{d.name:<26} {'(empty)':>6}")
                continue
            print(
                f"{d.name:<26} {g['pixel_count']:>6} "
                f"{g['center_x_frac']:>+7.2f} {g['center_y_frac']:>+7.2f} "
                f"{g['main_axis_deg']:>+8.1f} {g['aspect_ratio']:>7.1f}"
            )
