"""anvil_spans_reader.py — raster spans.bin → per-chunk 16x16 spans grid
（worldgen-v4 P7 §8.1 #8）。

P0 给 ``anvil_export.py`` 加了 ``chunk_to_nbt_from_spans`` 编码器（按段 loop fill），
但 anvil **驱动**（``anvil_world_export.export_anvil_world`` + ``pipeline.sh``）一直走
synthetic ``rolling_hills_height_fn`` → heightmap 路径，从未读真实地形。P7 收口：本模块
把 ``raster_export`` 产出的 ``spans_count.bin`` + ``spans.bin`` 解码回 per-column spans，
让 anvil snapshot 拍到真·峡谷/浮岛/洞穴而非合成正弦丘陵。

为什么独立模块（不并进 anvil_world_export.py）：``anvil_world_export`` 刻意不依赖
worldgen blueprint/fields（裸 callable 接口锁定，见其 docstring）。本 reader 才是
raster ↔ spans 适配层；``export_anvil_world`` 通过 ``spans_grid_fn`` 注入点消费它，
签名不变。

坐标映射与 Rust ``raster.rs::sample`` 逐字对齐（**唯一真相**，回归立刻撞红）：
    tile_x = world_x.div_euclid(tile_size)
    local_x = world_x.rem_euclid(tile_size)
    index  = local_z * tile_size + local_x
解码用 ``fields.decode_spans_column``（与 Rust ``decode_spans`` 同编码）。

Y 范围适配（anvil vs raster）：raster 列 span 可达 world Y 431（浮岛，
``fields.SPAN_MAX_Y``），但 1.20.1 anvil 只覆盖 [-64, 320)（``anvil_export.WORLD_MAX_Y``）。
本 reader 把超出 anvil 天花板的 span 截到 ``WORLD_MAX_Y-1``，floor 已在天花板之上的
span 整段丢弃——anvil 是 snapshot 预览产物，不承载完整世界高度（live raster server
才读 431 全高）。截断后仍是合法 span 列（``chunk_to_nbt_from_spans`` 会再校验一次）。
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Callable

# 模块内部相对 import（被当 package 加载时）+ 兼容裸模块加载（单测 sys.path 注入）。
try:
    from .anvil_export import WORLD_MAX_Y, WORLD_MIN_Y  # type: ignore[import-not-found]
    from .fields import decode_spans_column  # type: ignore[import-not-found]
except ImportError:  # 裸 import 路径（test_anvil_world_spans 注入 sys.path）
    from anvil_export import WORLD_MAX_Y, WORLD_MIN_Y  # type: ignore[no-redef]
    from fields import decode_spans_column  # type: ignore[no-redef]

# anvil 列最高可放方块的 Y（inclusive）。超出此值的 raster span 在 anvil 里被截断/丢弃。
ANVIL_TOP_Y = WORLD_MAX_Y - 1  # 319

# 野外（无 tile 覆盖）默认地表段：单段 bedrock..flat_y。与 Rust wilderness 64 平台对齐，
# 让 anvil snapshot 在 tile 缝隙也有可站立地表而非虚空。
DEFAULT_WILDERNESS_SURFACE_Y = 64


def clamp_spans_to_anvil(
    spans: tuple[tuple[int, int], ...],
) -> list[tuple[int, int]]:
    """Clamp a raster column's spans into the anvil world Y window [-64, 319].

    A span entirely above ``ANVIL_TOP_Y`` (a high sky-isle out of anvil reach) is
    dropped; a span straddling the ceiling has its ``ceiling_y`` capped to
    ``ANVIL_TOP_Y``.  Floors below ``WORLD_MIN_Y`` are lifted to ``WORLD_MIN_Y``
    (defensive — raster floors should already bottom at bedrock).  The result
    preserves the surface span (raster span[0]) at index 0 when it survives, so
    the anvil renderer keeps reading the walkable surface from ``spans[0]``.
    """
    clamped: list[tuple[int, int]] = []
    for floor_y, ceiling_y in spans:
        if floor_y > ANVIL_TOP_Y:
            # Entire span floats above the anvil ceiling — not representable here.
            continue
        lo = max(floor_y, WORLD_MIN_Y)
        hi = min(ceiling_y, ANVIL_TOP_Y)
        if lo > hi:
            continue
        clamped.append((lo, hi))
    return clamped


class RasterSpansReader:
    """Decode raster spans for arbitrary world (x, z) columns.

    Loads a manifest + lazily mmaps each referenced tile's ``spans_count.bin`` /
    ``spans.bin`` (read once, cached).  ``spans_for_column`` mirrors the Rust
    reader's tile/column index math so the anvil world and the live server agree
    on which span a world coordinate maps to.
    """

    def __init__(self, raster_dir: Path) -> None:
        self.raster_dir = Path(raster_dir)
        manifest_path = self.raster_dir / "manifest.json"
        if not manifest_path.exists():
            raise FileNotFoundError(
                f"raster manifest not found: {manifest_path} — run the raster "
                f"backend before the anvil spans export"
            )
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        self.tile_size: int = int(manifest["tile_size"])
        if self.tile_size <= 0:
            raise ValueError(f"manifest tile_size must be positive, got {self.tile_size}")
        # (tile_x, tile_z) -> tile dir name, only for tiles that carry spans.
        self._tile_dir: dict[tuple[int, int], str] = {}
        for tile in manifest["tiles"]:
            if not tile.get("spans"):
                continue
            self._tile_dir[(int(tile["tile_x"]), int(tile["tile_z"]))] = str(tile["dir"])
        # Per-tile decoded byte buffers, loaded on first access.
        self._tile_bytes: dict[tuple[int, int], tuple[bytes, bytes] | None] = {}

    def _load_tile(self, tile_x: int, tile_z: int) -> tuple[bytes, bytes] | None:
        key = (tile_x, tile_z)
        if key in self._tile_bytes:
            return self._tile_bytes[key]
        tile_name = self._tile_dir.get(key)
        if tile_name is None:
            self._tile_bytes[key] = None
            return None
        tile_dir = self.raster_dir / tile_name
        count_bytes = (tile_dir / "spans_count.bin").read_bytes()
        spans_bytes = (tile_dir / "spans.bin").read_bytes()
        loaded = (count_bytes, spans_bytes)
        self._tile_bytes[key] = loaded
        return loaded

    def spans_for_column(self, world_x: int, world_z: int) -> list[tuple[int, int]]:
        """Return anvil-clamped solid spans for the world column (x, z).

        Falls back to a single flat wilderness surface span where no tile covers
        the column (so the snapshot world has ground in tile gaps instead of an
        abrupt void cliff at the raster boundary).
        """
        tile_x = world_x // self.tile_size  # Python // is floor div == Rust div_euclid
        tile_z = world_z // self.tile_size
        loaded = self._load_tile(tile_x, tile_z)
        if loaded is None:
            return [(WORLD_MIN_Y, DEFAULT_WILDERNESS_SURFACE_Y)]
        count_bytes, spans_bytes = loaded
        local_x = world_x % self.tile_size  # Python % is floor mod == Rust rem_euclid
        local_z = world_z % self.tile_size
        index = local_z * self.tile_size + local_x
        column = decode_spans_column(count_bytes, spans_bytes, index)
        return clamp_spans_to_anvil(column.spans)


def raster_spans_grid_fn(
    raster_dir: Path,
) -> Callable[[int, int], list[list[list[tuple[int, int]]]]]:
    """Build a ``spans_grid_fn(chunk_x, chunk_z)`` reading real raster spans.

    The returned callable yields the 16x16 ``spans_grid[z][x]`` that
    ``export_anvil_world(..., spans_grid_fn=...)`` consumes — each cell a list of
    anvil-clamped ``(floor_y, ceiling_y)`` solid spans for that block column.
    Interface mirrors ``rolling_hills_height_fn`` (a factory returning the
    per-unit callable) so the anvil driver swaps backends without signature
    churn.
    """
    reader = RasterSpansReader(raster_dir)

    def grid_fn(chunk_x: int, chunk_z: int) -> list[list[list[tuple[int, int]]]]:
        return [
            [
                reader.spans_for_column(chunk_x * 16 + x, chunk_z * 16 + z)
                for x in range(16)
            ]
            for z in range(16)
        ]

    return grid_fn
