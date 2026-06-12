"""worldgen-v4 P0 §8.1 #1 — ColumnSpans codec + binary-layout pin tests.

Locks the on-disk contract the Rust mmap reader depends on:
  * spans_count.bin : u8 per column, 0..=MAX_SPANS
  * spans.bin       : 16 bytes per column = 4 slots × (i16 floor, i16 ceiling),
                      little-endian, unused slots = sentinel i16::MAX (32767),
                      column at ``offset = col_idx * 16``.

Covers every column shape (single / sky-isle two-span / multi-span cave /
empty void) plus all ColumnSpans validation branches.
"""

from __future__ import annotations

import struct
import unittest

import numpy as np

from scripts.terrain_gen.fields import (
    MAX_SPANS,
    SPAN_BYTES_PER_COLUMN,
    SPAN_SENTINEL,
    ColumnSpans,
    decode_spans_column,
    encode_spans_arrays,
)


class ColumnSpansValidationTest(unittest.TestCase):
    def test_single_solid_span_is_valid(self) -> None:
        col = ColumnSpans(((-64, 70),))
        self.assertEqual(col.count, 1)
        self.assertEqual(col.surface_ceiling_y, 70)

    def test_empty_void_column_has_no_surface(self) -> None:
        col = ColumnSpans(())
        self.assertEqual(col.count, 0)
        self.assertIsNone(
            col.surface_ceiling_y,
            "a fully void column must report no surface so query_surface can "
            "fall back rather than read a phantom ground y",
        )

    def test_two_span_sky_isle_keeps_lowest_as_surface(self) -> None:
        col = ColumnSpans(((-64, 72), (300, 320)))
        self.assertEqual(col.count, 2)
        self.assertEqual(
            col.surface_ceiling_y,
            72,
            "surface anchor must be the LOWEST span ceiling (ground), not the "
            "floating isle above (§8.1 #2)",
        )

    def test_max_spans_allowed(self) -> None:
        col = ColumnSpans(((-64, -50), (-40, -20), (0, 10), (40, 72)))
        self.assertEqual(col.count, MAX_SPANS)

    def test_too_many_spans_rejected(self) -> None:
        with self.assertRaises(ValueError):
            ColumnSpans(((-64, -50), (-40, -20), (0, 10), (40, 72), (90, 100)))

    def test_floor_above_ceiling_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "floor_y=10 must be <= ceiling_y=5"):
            ColumnSpans(((10, 5),))

    def test_overlapping_spans_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "non-overlapping"):
            ColumnSpans(((-64, 40), (40, 72)))

    def test_touching_spans_rejected(self) -> None:
        # floor_y == prev_ceiling + 1 would be adjacent (still illegal — must be
        # a real gap so the cave void is at least one block).
        with self.assertRaisesRegex(ValueError, "non-overlapping"):
            ColumnSpans(((-64, 40), (40, 50)))

    def test_floor_below_world_floor_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "out of world range"):
            ColumnSpans(((-65, 0),))

    def test_ceiling_above_world_ceiling_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "out of world range"):
            ColumnSpans(((0, 432),))


class SpanByteLayoutTest(unittest.TestCase):
    """Pin the exact 16-byte little-endian column stride."""

    def test_single_span_byte_layout(self) -> None:
        count, spans = encode_spans_arrays([ColumnSpans(((-64, 70),))])
        self.assertEqual(count.dtype, np.uint8)
        self.assertEqual(list(count.tobytes()), [1])

        raw = spans.tobytes()
        self.assertEqual(
            len(raw),
            SPAN_BYTES_PER_COLUMN,
            f"one column must be exactly {SPAN_BYTES_PER_COLUMN} bytes",
        )
        # 4 slots × 2 i16; slot 0 = (-64, 70), slots 1..3 = sentinel pairs.
        decoded = struct.unpack("<8h", raw)
        self.assertEqual(
            decoded,
            (-64, 70, SPAN_SENTINEL, SPAN_SENTINEL, SPAN_SENTINEL, SPAN_SENTINEL,
             SPAN_SENTINEL, SPAN_SENTINEL),
        )

    def test_sky_isle_two_span_byte_layout(self) -> None:
        count, spans = encode_spans_arrays([ColumnSpans(((-64, 72), (300, 320)))])
        self.assertEqual(list(count.tobytes()), [2])
        decoded = struct.unpack("<8h", spans.tobytes())
        self.assertEqual(
            decoded,
            (-64, 72, 300, 320, SPAN_SENTINEL, SPAN_SENTINEL,
             SPAN_SENTINEL, SPAN_SENTINEL),
        )

    def test_empty_void_column_byte_layout(self) -> None:
        count, spans = encode_spans_arrays([ColumnSpans(())])
        self.assertEqual(
            list(count.tobytes()),
            [0],
            "void column count byte must be 0",
        )
        decoded = struct.unpack("<8h", spans.tobytes())
        self.assertEqual(
            decoded,
            (SPAN_SENTINEL,) * 8,
            "void column must be all-sentinel so the reader skips it cleanly",
        )

    def test_multi_column_offset_is_col_idx_times_16(self) -> None:
        cols = [
            ColumnSpans(((-64, 70),)),
            ColumnSpans(((-64, 40), (60, 72))),  # cave: surface span carved open
            ColumnSpans(()),
            ColumnSpans(((-64, -50), (-30, -10), (0, 20), (50, 72))),  # 4-span
        ]
        count, spans = encode_spans_arrays(cols)
        count_bytes = count.tobytes()
        spans_bytes = spans.tobytes()

        self.assertEqual(list(count_bytes), [1, 2, 0, 4])
        self.assertEqual(len(spans_bytes), 4 * SPAN_BYTES_PER_COLUMN)

        # Decode each column back at its mmap offset and confirm round-trip.
        for col_idx, original in enumerate(cols):
            decoded = decode_spans_column(count_bytes, spans_bytes, col_idx)
            self.assertEqual(
                decoded.spans,
                original.spans,
                f"column {col_idx} did not round-trip via offset "
                f"{col_idx * SPAN_BYTES_PER_COLUMN}",
            )


class SpanDecodeGuardTest(unittest.TestCase):
    def test_decode_rejects_non_sentinel_padding(self) -> None:
        # count=1 but slot 1 carries real-looking coords → corrupt encoding.
        from scripts.terrain_gen.fields import ColumnSpans as _CS  # noqa: F401

        count_bytes = bytes([1])
        # slot0 = (-64, 70); slot1 = (5, 9) (illegal padding); rest sentinel.
        slots = [-64, 70, 5, 9, SPAN_SENTINEL, SPAN_SENTINEL, SPAN_SENTINEL, SPAN_SENTINEL]
        spans_bytes = struct.pack("<8h", *slots)
        with self.assertRaisesRegex(ValueError, "not sentinel-filled"):
            decode_spans_column(count_bytes, spans_bytes, 0)

    def test_decode_rejects_count_over_max(self) -> None:
        count_bytes = bytes([MAX_SPANS + 1])
        spans_bytes = struct.pack("<8h", *([0] * 8))
        with self.assertRaisesRegex(ValueError, "out of range"):
            decode_spans_column(count_bytes, spans_bytes, 0)


if __name__ == "__main__":
    unittest.main()
