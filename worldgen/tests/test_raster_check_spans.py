"""worldgen-v4 P0 §8.1 #8 — raster_check span legality rule pin (正反 case).

Builds a tiny manifest + tile dir with hand-crafted spans_count.bin / spans.bin
and asserts ``validate_rasters`` accepts legal columns and rejects each illegal
shape: count > MAX, floor > ceiling, out-of-world-Y, non-sentinel padding, and
overlapping spans.
"""

from __future__ import annotations

import json
import struct
import tempfile
import unittest
from pathlib import Path

from scripts.terrain_gen.harness.raster_check import (
    SPAN_BYTES_PER_COLUMN,
    SPAN_MAX_SPANS,
    SPAN_SENTINEL,
    validate_rasters,
)

# 2x2 tile keeps the binary tiny while exercising the per-column loop.
TILE_SIZE = 2
AREA = TILE_SIZE * TILE_SIZE


def _slots(*spans: tuple[int, int]) -> list[int]:
    flat: list[int] = []
    for floor_y, ceiling_y in spans:
        flat += [floor_y, ceiling_y]
    while len(flat) < SPAN_MAX_SPANS * 2:
        flat.append(SPAN_SENTINEL)
    return flat


def _write_raster(tmp: Path, columns: list[list[tuple[int, int]]]) -> Path:
    """Write a minimal overworld raster set with the given per-column spans."""
    raster_dir = tmp / "rasters"
    tile_dir = raster_dir / "tile_0_0"
    tile_dir.mkdir(parents=True)

    count_bytes = bytes(len(col) for col in columns)
    spans_buf = bytearray()
    for col in columns:
        spans_buf += struct.pack(f"<{SPAN_MAX_SPANS * 2}h", *_slots(*col))
    (tile_dir / "spans_count.bin").write_bytes(count_bytes)
    (tile_dir / "spans.bin").write_bytes(bytes(spans_buf))

    manifest = {
        "version": 2,
        "backend": "raster",
        "world_name": "t",
        "tile_size": TILE_SIZE,
        "tiles": [
            {
                "tile_x": 0,
                "tile_z": 0,
                "dir": "tile_0_0",
                "zones": ["spawn"],
                "layers": [],
                "spans": True,
            }
        ],
        "pois": [],
        "semantic_layers": [],
        "collapsed_zones": [],
    }
    (raster_dir / "manifest.json").write_text(json.dumps(manifest))
    return raster_dir


def _raw_spans_for(columns_slots: list[tuple[int, list[int]]]) -> bytes:
    buf = bytearray()
    for _count, slots in columns_slots:
        buf += struct.pack(f"<{SPAN_MAX_SPANS * 2}h", *slots)
    return bytes(buf)


class RasterCheckSpanLegalityTest(unittest.TestCase):
    def test_legal_columns_pass(self) -> None:
        cols = [
            [(-64, 70)],                    # normal
            [(71, 72), (-64, 40)],          # cave: surface cap + floor remnant
            [],                             # void
            [(-64, 72), (300, 320)],        # sky isle
        ]
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_raster(Path(td), cols)
            ok, msg = validate_rasters(raster_dir)
            self.assertTrue(ok, msg)

    def test_count_over_max_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_raster(Path(td), [[(-64, 70)]] * AREA)
            # Corrupt the count byte of column 0 to MAX+1.
            count_path = raster_dir / "tile_0_0" / "spans_count.bin"
            data = bytearray(count_path.read_bytes())
            data[0] = SPAN_MAX_SPANS + 1
            count_path.write_bytes(bytes(data))
            ok, msg = validate_rasters(raster_dir)
            self.assertFalse(ok)
            self.assertIn("spans count", msg)

    def test_floor_above_ceiling_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_raster(Path(td), [[(10, 5)]] + [[(-64, 70)]] * (AREA - 1))
            ok, msg = validate_rasters(raster_dir)
            self.assertFalse(ok)
            self.assertIn("floor_y=10 > ceiling_y=5", msg)

    def test_out_of_world_y_rejected(self) -> None:
        # ceiling 500 is above SPAN_MAX_Y=431.
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_raster(Path(td), [[(0, 500)]] + [[(-64, 70)]] * (AREA - 1))
            ok, msg = validate_rasters(raster_dir)
            self.assertFalse(ok)
            self.assertIn("out of world Y", msg)

    def test_non_sentinel_padding_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_raster(Path(td), [[(-64, 70)]] * AREA)
            # count=1 but slot 1 carries real coords instead of sentinel.
            spans_path = raster_dir / "tile_0_0" / "spans.bin"
            slots = [-64, 70, 5, 9] + [SPAN_SENTINEL] * 4
            data = bytearray(spans_path.read_bytes())
            data[0:SPAN_BYTES_PER_COLUMN] = struct.pack(f"<{SPAN_MAX_SPANS * 2}h", *slots)
            spans_path.write_bytes(bytes(data))
            ok, msg = validate_rasters(raster_dir)
            self.assertFalse(ok)
            self.assertIn("not sentinel-filled", msg)

    def test_overlapping_spans_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_raster(Path(td), [[(-64, 70)]] * AREA)
            # count=2 with two overlapping spans.
            count_path = raster_dir / "tile_0_0" / "spans_count.bin"
            spans_path = raster_dir / "tile_0_0" / "spans.bin"
            cdata = bytearray(count_path.read_bytes())
            cdata[0] = 2
            count_path.write_bytes(bytes(cdata))
            slots = [-64, 50, 40, 72] + [SPAN_SENTINEL] * 4  # 40..72 overlaps -64..50
            sdata = bytearray(spans_path.read_bytes())
            sdata[0:SPAN_BYTES_PER_COLUMN] = struct.pack(f"<{SPAN_MAX_SPANS * 2}h", *slots)
            spans_path.write_bytes(bytes(sdata))
            ok, msg = validate_rasters(raster_dir)
            self.assertFalse(ok)
            self.assertIn("overlap", msg)

    def test_touching_spans_rejected(self) -> None:
        # Adjacent spans with no air gap (floor == prev_ceiling + 1) are illegal.
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_raster(Path(td), [[(-64, 70)]] * AREA)
            count_path = raster_dir / "tile_0_0" / "spans_count.bin"
            spans_path = raster_dir / "tile_0_0" / "spans.bin"
            cdata = bytearray(count_path.read_bytes())
            cdata[0] = 2
            count_path.write_bytes(bytes(cdata))
            slots = [-64, 40, 41, 72] + [SPAN_SENTINEL] * 4  # 41 touches 40
            sdata = bytearray(spans_path.read_bytes())
            sdata[0:SPAN_BYTES_PER_COLUMN] = struct.pack(f"<{SPAN_MAX_SPANS * 2}h", *slots)
            spans_path.write_bytes(bytes(sdata))
            ok, msg = validate_rasters(raster_dir)
            self.assertFalse(ok)
            self.assertIn("overlap/touch", msg)

    def test_count_length_mismatch_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_raster(Path(td), [[(-64, 70)]] * AREA)
            # Truncate spans_count.bin.
            count_path = raster_dir / "tile_0_0" / "spans_count.bin"
            count_path.write_bytes(count_path.read_bytes()[:-1])
            ok, msg = validate_rasters(raster_dir)
            self.assertFalse(ok)
            self.assertIn("spans_count.bin length", msg)


if __name__ == "__main__":
    unittest.main()
