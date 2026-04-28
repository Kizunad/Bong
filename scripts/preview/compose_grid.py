#!/usr/bin/env python3
"""compose_grid.py — 把 client 5 角度截图 + worldgen raster PNG 拼成 PR comment 总览图。

plan-worldgen-snapshot-v1 §3.1。

输入:
  - client/run/screenshots/preview-{top,iso_ne,iso_nw,iso_se,iso_sw}.png
  - worldgen/generated/snapshot/focus-layout-preview.png
  - worldgen/generated/snapshot/focus-surface-preview.png

输出:
  - <out_dir>/preview-grid.png

布局:
  +---------------+---------------+
  | client top    | raster layout |
  +---------------+---------------+
  | iso_ne | iso_nw | iso_se | iso_sw  (4 等宽并排)
  +-------------------------------+
  | raster surface (full width)   |
  +-------------------------------+

只依赖 stdlib (zlib + struct) 出 PNG —— 仿 worldgen/exporters.py 风格,
避免 PIL/matplotlib 依赖。简化:不做色彩混合 / 文字 caption (caption 让
post_comment.py 在 markdown 里加),纯像素拼接。

用法:
  python3 scripts/preview/compose_grid.py \\
    --client-dir client/run/screenshots \\
    --raster-dir worldgen/generated/snapshot \\
    --out-dir <output>

如果某张子图缺失,用单色占位（# 为提示「missing: <name>」），其他正常拼。
"""

from __future__ import annotations

import argparse
import sys
import zlib
import struct
from pathlib import Path


def _png_chunk(chunk_type: bytes, payload: bytes) -> bytes:
    """构造 PNG chunk(length + type + payload + crc32)。"""
    crc = zlib.crc32(chunk_type + payload)
    return struct.pack(">I", len(payload)) + chunk_type + payload + struct.pack(">I", crc)


def _read_png(path: Path) -> tuple[int, int, bytes] | None:
    """读 PNG → (width, height, RGBA bytes 行优先)。失败返回 None。

    简化版:仅支持 8-bit RGBA(color type 6) PNG。worldgen exporters.py 出的是 RGB
    (color type 2)+ 4 行 width*3 字节 -> 需要也兼容 RGB / 灰度等。这里做最 robust
    的:不解 PNG,直接当二进制读。但拼图必须知道 width/height + 像素 -- 所以仍需解。

    退一步:依赖 PIL.Image。如果 PIL 装了就用,否则报错。stdlib 解 PNG 太重(需要
    完整 zlib decompress + filter 反向 + interlace 处理),不适合本脚本范围。
    """
    try:
        from PIL import Image
    except ImportError:
        print(
            "[compose_grid] 错误: 需要 Pillow (PIL)。在 CI 上 `pip install pillow`。",
            file=sys.stderr,
        )
        return None
    if not path.exists():
        return None
    img = Image.open(path).convert("RGBA")
    return img.width, img.height, img.tobytes()


def _make_placeholder(width: int, height: int, name: str) -> tuple[int, int, bytes]:
    """缺失子图占位 — 暗灰底 #303030。"""
    r, g, b, a = 48, 48, 48, 255
    pixel_count = width * height
    data = bytes([r, g, b, a]) * pixel_count
    print(f"[compose_grid] 占位: {name} ({width}x{height})")
    return width, height, data


def _resize_rgba(data: bytes, src_w: int, src_h: int, dst_w: int, dst_h: int) -> bytes:
    """nearest-neighbor 缩放 RGBA 字节流到目标尺寸（不依赖 PIL，纯算）。"""
    out = bytearray(dst_w * dst_h * 4)
    for y in range(dst_h):
        sy = min(int(y * src_h / dst_h), src_h - 1)
        for x in range(dst_w):
            sx = min(int(x * src_w / dst_w), src_w - 1)
            si = (sy * src_w + sx) * 4
            di = (y * dst_w + x) * 4
            out[di : di + 4] = data[si : si + 4]
    return bytes(out)


def _write_png(path: Path, width: int, height: int, rgba: bytes) -> None:
    """RGBA bytes → PNG 文件。仿 worldgen/exporters.py。"""
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)  # 8bpc RGBA
    ihdr_chunk = _png_chunk(b"IHDR", ihdr)
    # filter byte 0 (None) per scanline + RGBA payload
    raw = bytearray()
    stride = width * 4
    for y in range(height):
        raw.append(0)
        raw.extend(rgba[y * stride : (y + 1) * stride])
    idat_chunk = _png_chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    iend_chunk = _png_chunk(b"IEND", b"")
    path.write_bytes(sig + ihdr_chunk + idat_chunk + iend_chunk)


def _paste_into(
    canvas: bytearray, canvas_w: int, x: int, y: int, src: bytes, src_w: int, src_h: int
) -> None:
    """把 src RGBA 拼到 canvas (x, y) 起点。无 alpha 混合，直接覆盖。"""
    for row in range(src_h):
        src_off = row * src_w * 4
        dst_off = ((y + row) * canvas_w + x) * 4
        canvas[dst_off : dst_off + src_w * 4] = src[src_off : src_off + src_w * 4]


def compose_grid(client_dir: Path, raster_dir: Path, out_path: Path) -> int:
    """主拼图。返回 exit code (0 = ok)。"""
    # 单元格尺寸 - 客户端截图普遍 1280x720,做 1/2 缩放;raster 通常较大,
    # 缩到统一格子大小再拼。
    CELL_W = 480
    CELL_H = 270

    sources = {
        "top": client_dir / "preview-top.png",
        "iso_ne": client_dir / "preview-iso_ne.png",
        "iso_nw": client_dir / "preview-iso_nw.png",
        "iso_se": client_dir / "preview-iso_se.png",
        "iso_sw": client_dir / "preview-iso_sw.png",
        "raster_layout": raster_dir / "focus-layout-preview.png",
        "raster_surface": raster_dir / "focus-surface-preview.png",
    }

    cells: dict[str, tuple[int, int, bytes]] = {}
    missing: list[str] = []
    for name, path in sources.items():
        result = _read_png(path)
        if result is None:
            missing.append(name)
            cells[name] = _make_placeholder(CELL_W, CELL_H, name)
        else:
            sw, sh, sd = result
            # 缩到 cell 尺寸
            cells[name] = (CELL_W, CELL_H, _resize_rgba(sd, sw, sh, CELL_W, CELL_H))

    if missing:
        print(f"[compose_grid] {len(missing)} 张子图缺失: {missing}", file=sys.stderr)

    # Layout:
    # row 0: top | raster_layout              (2 cells * CELL_W)
    # row 1: iso_ne | iso_nw | iso_se | iso_sw (4 cells * CELL_W/2 = 4 * 240)
    # row 2: raster_surface (2 cells wide spanning)

    canvas_w = CELL_W * 2
    canvas_h = CELL_H * 3
    canvas = bytearray(canvas_w * canvas_h * 4)
    # init bg #181818
    for i in range(0, len(canvas), 4):
        canvas[i] = 24
        canvas[i + 1] = 24
        canvas[i + 2] = 24
        canvas[i + 3] = 255

    # row 0
    _paste_into(canvas, canvas_w, 0, 0, cells["top"][2], cells["top"][0], cells["top"][1])
    _paste_into(
        canvas,
        canvas_w,
        CELL_W,
        0,
        cells["raster_layout"][2],
        cells["raster_layout"][0],
        cells["raster_layout"][1],
    )

    # row 1: 4 iso 角度并排,每个宽度 CELL_W/2 = 240
    SMALL_W = CELL_W // 2
    for idx, key in enumerate(("iso_ne", "iso_nw", "iso_se", "iso_sw")):
        sw, sh, sd = cells[key]
        small = _resize_rgba(sd, sw, sh, SMALL_W, CELL_H)
        _paste_into(canvas, canvas_w, idx * SMALL_W, CELL_H, small, SMALL_W, CELL_H)

    # row 2
    _paste_into(
        canvas,
        canvas_w,
        0,
        CELL_H * 2,
        cells["raster_surface"][2],
        cells["raster_surface"][0],
        cells["raster_surface"][1],
    )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    _write_png(out_path, canvas_w, canvas_h, bytes(canvas))
    print(f"[compose_grid] saved {out_path} ({canvas_w}x{canvas_h})")
    if missing:
        print(f"[compose_grid] WARN {len(missing)} 张子图缺失,占位灰色", file=sys.stderr)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="compose_grid.py")
    parser.add_argument("--client-dir", type=Path, default=Path("client/run/screenshots"))
    parser.add_argument("--raster-dir", type=Path, default=Path("worldgen/generated/snapshot"))
    parser.add_argument(
        "--out", type=Path, default=Path("client/run/screenshots/preview-grid.png")
    )
    args = parser.parse_args()
    return compose_grid(args.client_dir, args.raster_dir, args.out)


if __name__ == "__main__":
    sys.exit(main())
