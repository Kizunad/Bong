"""
Process local_images/ icons for use as MC item textures.
- Remove black (or white) backgrounds via flood-fill from edges
- Crop to content bounding box
- Center on square canvas with padding
- Export 128x128 PNG to client resources
"""

import os
import sys
from pathlib import Path
from PIL import Image
import numpy as np
from collections import deque

SRC_DIR = Path(__file__).resolve().parent.parent / "local_images"
DST_DIR = (
    Path(__file__).resolve().parent.parent
    / "client"
    / "src"
    / "main"
    / "resources"
    / "assets"
    / "bong-client"
    / "textures"
    / "gui"
    / "items"
)

OUTPUT_SIZE = 128
PADDING_RATIO = 0.06

SKIP_FILES = {"空兽.jpg", "prefix.md", "generation_guide.md"}
WHITE_BG_FILES = {"残卷破碎法宝碎片2.png"}

NAME_MAP = {
    "封灵骨币.png": "fengling_bone_coin",
    "骨刺.png": "bone_spike",
    "固元丹.png": "guyuan_pill",
    "灵草.png": "spirit_grass",
    "毒蛊飞针.png": "poison_needle",
    "破碎法宝.png": "broken_artifact",
    "凝脉散.png": "ningmai_powder",
    "真元诡雷.png": "zhenyuan_mine",
    "伪灵皮.png": "fake_spirit_hide",
    "欺天阵替身木桩.png": "decoy_stake",
    "盲盒死信箱.png": "blind_box",
    "回元丹（禁药版）.png": "huiyuan_pill_forbidden",
    "噬元鼠膨胀鼠尾.png": "rat_tail",
    "拟态灰烬蛛蛛丝.png": "ash_spider_silk",
    "灵木.png": "spirit_wood",
    "游商傀儡.png": "merchant_puppet",
    "异变兽核.png": "mutant_beast_core",
    "道伥.png": "dao_ghost",
    "《爆脉流正法》.png": "baomai_scripture",
    "残卷破碎法宝碎片2.png": "broken_artifact_scroll",
}


def flood_fill_bg(img: Image.Image, threshold: int, is_white: bool = False) -> Image.Image:
    """
    Flood-fill from image edges to find connected background pixels.
    Only pixels reachable from the border AND below threshold (or above for white)
    are made transparent. Interior dark pixels are preserved.
    """
    rgba = img.convert("RGBA")
    w, h = rgba.size
    data = np.array(rgba)

    r = data[:, :, 0].astype(np.float64)
    g = data[:, :, 1].astype(np.float64)
    b = data[:, :, 2].astype(np.float64)

    if is_white:
        # Distance from white — small distance = background
        metric = np.sqrt((255 - r) ** 2 + (255 - g) ** 2 + (255 - b) ** 2)
        is_bg_pixel = metric < threshold
    else:
        # Luminance — low luminance = background candidate
        lum = 0.299 * r + 0.587 * g + 0.114 * b
        is_bg_pixel = lum < threshold

    # Flood fill from all edge pixels
    visited = np.zeros((h, w), dtype=bool)
    bg_mask = np.zeros((h, w), dtype=bool)
    queue = deque()

    # Seed from all 4 edges
    for x in range(w):
        for y_edge in [0, h - 1]:
            if is_bg_pixel[y_edge, x] and not visited[y_edge, x]:
                visited[y_edge, x] = True
                queue.append((y_edge, x))
    for y in range(h):
        for x_edge in [0, w - 1]:
            if is_bg_pixel[y, x_edge] and not visited[y, x_edge]:
                visited[y, x_edge] = True
                queue.append((y, x_edge))

    # BFS flood fill
    while queue:
        cy, cx = queue.popleft()
        bg_mask[cy, cx] = True
        for dy, dx in [(-1, 0), (1, 0), (0, -1), (0, 1)]:
            ny, nx = cy + dy, cx + dx
            if 0 <= ny < h and 0 <= nx < w and not visited[ny, nx] and is_bg_pixel[ny, nx]:
                visited[ny, nx] = True
                queue.append((ny, nx))

    # Apply: background → transparent, with soft edge
    # Dilate bg_mask slightly for anti-aliasing at the boundary
    from scipy.ndimage import binary_dilation, gaussian_filter

    # Create a soft alpha transition at the edge
    bg_float = bg_mask.astype(np.float64)
    # Slight gaussian blur on the mask for soft edges
    soft_bg = gaussian_filter(bg_float, sigma=0.8)

    alpha = data[:, :, 3].astype(np.float64) / 255.0
    # Where soft_bg > 0, reduce alpha proportionally
    new_alpha = alpha * (1.0 - soft_bg)
    new_alpha = np.clip(new_alpha * 255.0, 0, 255).astype(np.uint8)

    data[:, :, 3] = new_alpha
    return Image.fromarray(data, "RGBA")


def flood_fill_bg_no_scipy(img: Image.Image, threshold: int, is_white: bool = False) -> Image.Image:
    """Fallback without scipy — hard edge version."""
    rgba = img.convert("RGBA")
    w, h = rgba.size
    data = np.array(rgba)

    r = data[:, :, 0].astype(np.float64)
    g = data[:, :, 1].astype(np.float64)
    b = data[:, :, 2].astype(np.float64)

    if is_white:
        metric = np.sqrt((255 - r) ** 2 + (255 - g) ** 2 + (255 - b) ** 2)
        is_bg_pixel = metric < threshold
    else:
        lum = 0.299 * r + 0.587 * g + 0.114 * b
        is_bg_pixel = lum < threshold

    visited = np.zeros((h, w), dtype=bool)
    bg_mask = np.zeros((h, w), dtype=bool)
    queue = deque()

    for x in range(w):
        for y_edge in [0, h - 1]:
            if is_bg_pixel[y_edge, x] and not visited[y_edge, x]:
                visited[y_edge, x] = True
                queue.append((y_edge, x))
    for y in range(h):
        for x_edge in [0, w - 1]:
            if is_bg_pixel[y, x_edge] and not visited[y, x_edge]:
                visited[y, x_edge] = True
                queue.append((y, x_edge))

    while queue:
        cy, cx = queue.popleft()
        bg_mask[cy, cx] = True
        for dy, dx in [(-1, 0), (1, 0), (0, -1), (0, 1)]:
            ny, nx = cy + dy, cx + dx
            if 0 <= ny < h and 0 <= nx < w and not visited[ny, nx] and is_bg_pixel[ny, nx]:
                visited[ny, nx] = True
                queue.append((ny, nx))

    # Simple box-blur the mask for soft edges (3x3 average)
    padded = np.pad(bg_mask.astype(np.float64), 1, mode='edge')
    soft_bg = np.zeros_like(bg_mask, dtype=np.float64)
    for dy in range(-1, 2):
        for dx in range(-1, 2):
            soft_bg += padded[1 + dy:h + 1 + dy, 1 + dx:w + 1 + dx]
    soft_bg /= 9.0

    # Second pass blur for smoother edges
    padded2 = np.pad(soft_bg, 1, mode='edge')
    soft_bg2 = np.zeros_like(soft_bg)
    for dy in range(-1, 2):
        for dx in range(-1, 2):
            soft_bg2 += padded2[1 + dy:h + 1 + dy, 1 + dx:w + 1 + dx]
    soft_bg2 /= 9.0

    alpha = data[:, :, 3].astype(np.float64) / 255.0
    new_alpha = alpha * (1.0 - soft_bg2)
    new_alpha = np.clip(new_alpha * 255.0, 0, 255).astype(np.uint8)

    data[:, :, 3] = new_alpha
    return Image.fromarray(data, "RGBA")


def remove_bg(img: Image.Image, is_white: bool = False) -> Image.Image:
    threshold = 50 if is_white else 12
    try:
        return flood_fill_bg(img, threshold, is_white)
    except ImportError:
        return flood_fill_bg_no_scipy(img, threshold, is_white)


def crop_and_center(img: Image.Image, output_size: int, padding_ratio: float) -> Image.Image:
    bbox = img.getbbox()
    if bbox is None:
        return Image.new("RGBA", (output_size, output_size), (0, 0, 0, 0))

    cropped = img.crop(bbox)
    usable = int(output_size * (1.0 - 2.0 * padding_ratio))

    w, h = cropped.size
    scale = min(usable / w, usable / h)
    new_w = max(1, int(w * scale))
    new_h = max(1, int(h * scale))

    resized = cropped.resize((new_w, new_h), Image.LANCZOS)

    canvas = Image.new("RGBA", (output_size, output_size), (0, 0, 0, 0))
    offset_x = (output_size - new_w) // 2
    offset_y = (output_size - new_h) // 2
    canvas.paste(resized, (offset_x, offset_y), resized)

    return canvas


def process_all():
    DST_DIR.mkdir(parents=True, exist_ok=True)

    processed = 0
    skipped = 0

    for src_file in sorted(SRC_DIR.iterdir()):
        if src_file.name in SKIP_FILES or src_file.suffix == ".md":
            continue

        if src_file.name not in NAME_MAP:
            print(f"  SKIP (no name mapping): {src_file.name}")
            skipped += 1
            continue

        output_name = NAME_MAP[src_file.name]
        dst_path = DST_DIR / f"{output_name}.png"

        print(f"  {src_file.name} → {output_name}.png ... ", end="", flush=True)

        img = Image.open(src_file)
        is_white = src_file.name in WHITE_BG_FILES
        img = remove_bg(img, is_white)
        result = crop_and_center(img, OUTPUT_SIZE, PADDING_RATIO)
        result.save(dst_path, "PNG")

        processed += 1
        print("OK")

    print(f"\nDone: {processed} processed, {skipped} skipped")
    print(f"Output: {DST_DIR}")


if __name__ == "__main__":
    process_all()
