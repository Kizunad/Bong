#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = ["numpy", "pillow"]
# ///
"""量化武器在 TP 截图中的像素占比(相对玩家 baseline)。

原理:
  baseline (no weapon) 里非背景像素 = 玩家 body 像素数 P_base
  weapon_img 里非背景像素 = 玩家 + 武器 P_w
  Δ = P_w - P_base ≈ 武器像素数
  ratio = Δ / P_base = "武器相对玩家 body 的视觉占比"

玩家 skin 跟武器 session 可能不同,会有 skin 纹理误差(~5-10%),不影响数量级对比。
"""
import sys
from pathlib import Path
import numpy as np
from PIL import Image

def non_bg_count(path):
    img = np.array(Image.open(path).convert("RGB"))
    r, g, b = img[..., 0], img[..., 1], img[..., 2]
    sky = (r > 155) & (g > 195) & (b > 220) & (r < 250)
    cloud = (r > 225) & (g > 225) & (b > 225)
    bg = sky | cloud
    return int((~bg).sum())

if __name__ == "__main__":
    renders = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent / "renders"
    baseline_back = renders / "_baseline" / "mc_thirdperson_back.png"
    baseline_front = renders / "_baseline" / "mc_thirdperson_front.png"
    if not baseline_back.exists():
        print(f"ERROR: baseline not found: {baseline_back}", file=sys.stderr); sys.exit(1)

    p_base_back = non_bg_count(baseline_back)
    p_base_front = non_bg_count(baseline_front)
    print(f"baseline player pixels: back={p_base_back}  front={p_base_front}")
    print()
    print(f"{'asset':<30} {'tp_back_Δ':>10} {'ratio_back':>10} {'tp_front_Δ':>11} {'ratio_front':>11}")
    print("-" * 78)

    for asset_dir in sorted(renders.iterdir()):
        if not asset_dir.is_dir() or asset_dir.name.startswith("_"):
            continue
        back = asset_dir / "mc_thirdperson_back.png"
        front = asset_dir / "mc_thirdperson_front.png"
        if not back.exists() or not front.exists(): continue
        p_back = non_bg_count(back)
        p_front = non_bg_count(front)
        d_back = p_back - p_base_back
        d_front = p_front - p_base_front
        r_back = d_back / p_base_back if p_base_back > 0 else 0
        r_front = d_front / p_base_front if p_base_front > 0 else 0
        print(f"{asset_dir.name:<30} {d_back:>10d} {r_back:>10.3f} {d_front:>11d} {r_front:>11.3f}")
