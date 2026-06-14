"""worldgen-v4 P7 §8.1 #8 — anvil 驱动读真 raster spans 饱和测试.

跑法（worldgen/scripts/terrain_gen）：
    python3 -m unittest test_anvil_world_spans -v

P0 给 anvil_export 加了 ``chunk_to_nbt_from_spans`` 编码器，但 anvil 驱动一直走
synthetic ``rolling_hills_height_fn`` → heightmap。P7 把 ``anvil_spans_reader`` 接进
``export_anvil_world(spans_grid_fn=...)``，让 anvil snapshot 拍真地形。本测试锁住：

  RasterSpansReader：
    ① 列坐标映射与 Rust ``raster.rs::sample`` 逐字对齐（div_euclid/rem_euclid，
       负坐标、跨 tile 边界、tile 内偏移都对）
    ② Y-clamp：raster 浮岛段 (>319) 整段丢弃；跨天花板段截到 319；保留地表 span[0]
    ③ 野外（无 tile）列回退单段平台，不返回虚空
    ④ 空 tile 列（count=0）→ 空 spans（虚空，编码器写 air-over-bedrock）
    ⑤ 多段（洞穴残底 / 浮岛）原样解出（截断前）
    ⑥ 缺 manifest → FileNotFoundError（带修复线索）

  export_anvil_world(spans_grid_fn=...)：
    ⑦ spans 路径端到端：写 region → 读回 → 真段几何（grass/air/bedrock）落位
    ⑧ backend 标记 = "spans"
    ⑨ 后端互斥守卫：height_fn 与 spans_grid_fn 缺一 / 同时给 → ValueError
    ⑩ height_fn 路径仍可用（不回归）

  clamp_spans_to_anvil 单元 pin（边界 off-by-one）。
"""

from __future__ import annotations

import struct
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent))

import anvil_export as ax  # noqa: E402
import anvil_spans_reader as asr  # noqa: E402
from anvil_world_export import export_anvil_world  # noqa: E402
from fields import ColumnSpans, encode_spans_arrays  # noqa: E402
from test_anvil_region_writer import parse_region_file  # noqa: E402
from test_anvil_spans import block_at  # noqa: E402


def _write_raster_fixture(
    root: Path,
    tile_size: int,
    tiles: dict[tuple[int, int], list[ColumnSpans]],
) -> Path:
    """Write a minimal raster dir (manifest + per-tile spans files) for the reader.

    ``tiles`` maps (tile_x, tile_z) -> a list of ``tile_size * tile_size``
    ColumnSpans in row-major (index = local_z * tile_size + local_x) order — the
    exact layout the Rust reader and ``raster_export`` use.
    """
    import json

    raster_dir = root / "rasters"
    raster_dir.mkdir(parents=True, exist_ok=True)
    manifest_tiles = []
    for (tx, tz), columns in tiles.items():
        if len(columns) != tile_size * tile_size:
            raise AssertionError(
                f"tile ({tx},{tz}) needs {tile_size * tile_size} columns, "
                f"got {len(columns)}"
            )
        tile_name = f"tile_{tx}_{tz}"
        tile_dir = raster_dir / tile_name
        tile_dir.mkdir(parents=True, exist_ok=True)
        count_arr, spans_arr = encode_spans_arrays(columns)
        (tile_dir / "spans_count.bin").write_bytes(count_arr.tobytes())
        (tile_dir / "spans.bin").write_bytes(spans_arr.tobytes())
        manifest_tiles.append(
            {"tile_x": tx, "tile_z": tz, "dir": tile_name, "zones": [], "spans": True}
        )
    manifest = {"version": 2, "tile_size": tile_size, "tiles": manifest_tiles}
    (raster_dir / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
    return raster_dir


def _uniform_tile(tile_size: int, column: ColumnSpans) -> list[ColumnSpans]:
    return [column for _ in range(tile_size * tile_size)]


class ClampSpansTest(unittest.TestCase):
    def test_span_below_ceiling_untouched(self) -> None:
        self.assertEqual(
            asr.clamp_spans_to_anvil(((-64, 70),)),
            [(-64, 70)],
            "a span fully inside the anvil window is passed through verbatim",
        )

    def test_span_straddling_ceiling_capped_to_319(self) -> None:
        # raster isle ceiling 350 (> anvil 319) but floor 300 is reachable.
        self.assertEqual(
            asr.clamp_spans_to_anvil(((300, 350),)),
            [(300, asr.ANVIL_TOP_Y)],
            f"a span crossing the anvil ceiling is capped to ANVIL_TOP_Y="
            f"{asr.ANVIL_TOP_Y}, not dropped",
        )

    def test_span_fully_above_ceiling_dropped(self) -> None:
        # A high sky-isle (floor 400) is entirely out of anvil reach → dropped.
        self.assertEqual(
            asr.clamp_spans_to_anvil(((-64, 70), (400, 420))),
            [(-64, 70)],
            "the ground span survives; the unreachable high isle (floor>319) drops",
        )

    def test_ceiling_exactly_at_top_y_kept(self) -> None:
        # Off-by-one boundary: ceiling == ANVIL_TOP_Y (319) stays; floor==319 is a
        # 1-block span at the ceiling, also legal.
        self.assertEqual(
            asr.clamp_spans_to_anvil(((asr.ANVIL_TOP_Y, asr.ANVIL_TOP_Y),)),
            [(asr.ANVIL_TOP_Y, asr.ANVIL_TOP_Y)],
        )

    def test_floor_one_above_ceiling_dropped(self) -> None:
        # floor 320 = WORLD_MAX_Y is one past the last placeable block → dropped.
        self.assertEqual(
            asr.clamp_spans_to_anvil(((ax.WORLD_MAX_Y, ax.WORLD_MAX_Y + 10),)),
            [],
            "floor at WORLD_MAX_Y(320) is above the top placeable block (319) → drop",
        )

    def test_empty_column_clamps_to_empty(self) -> None:
        self.assertEqual(asr.clamp_spans_to_anvil(()), [])


class RasterSpansReaderTest(unittest.TestCase):
    TILE_SIZE = 4  # tiny tile so we can hand-place every column

    def _make_reader(self, root: Path, tiles) -> asr.RasterSpansReader:
        _write_raster_fixture(root, self.TILE_SIZE, tiles)
        return asr.RasterSpansReader(root / "rasters")

    def test_column_index_mapping_matches_rust_sample(self) -> None:
        # Encode a distinct surface height per column so we can prove the reader
        # picks the SAME (local_z*tile_size+local_x) cell that Rust raster.rs
        # sample() reads. Surface y = 64 + index (index 0..15 in a 4x4 tile).
        ts = self.TILE_SIZE
        cols = [
            ColumnSpans(((-64, 64 + (lz * ts + lx)),))
            for lz in range(ts)
            for lx in range(ts)
        ]
        with TemporaryDirectory() as tmp:
            reader = self._make_reader(Path(tmp), {(0, 0): cols})
            for lz in range(ts):
                for lx in range(ts):
                    expected_surface = 64 + (lz * ts + lx)
                    spans = reader.spans_for_column(lx, lz)
                    self.assertEqual(
                        spans[0][1], expected_surface,
                        f"column (x={lx},z={lz}) must map to index "
                        f"{lz * ts + lx} (surface {expected_surface}); reader read "
                        f"surface {spans[0][1]} — index math diverged from Rust "
                        f"raster.rs::sample (local_z*tile_size+local_x)",
                    )

    def test_negative_coords_use_floor_div_and_floor_mod(self) -> None:
        # World (-1, -1) → tile (-1,-1) via floor div; local (ts-1, ts-1) via floor
        # mod — matching Rust div_euclid/rem_euclid. Tile (-1,-1) surface 100,
        # tile (0,0) surface 64 — picking the wrong tile/cell shows immediately.
        ts = self.TILE_SIZE
        tiles = {
            (-1, -1): _uniform_tile(ts, ColumnSpans(((-64, 100),))),
            (0, 0): _uniform_tile(ts, ColumnSpans(((-64, 64),))),
        }
        with TemporaryDirectory() as tmp:
            reader = self._make_reader(Path(tmp), tiles)
            self.assertEqual(
                reader.spans_for_column(-1, -1)[0][1], 100,
                "world (-1,-1) must floor-div to tile (-1,-1) (surface 100), not "
                "truncate-toward-zero to tile (0,0) (surface 64)",
            )
            self.assertEqual(reader.spans_for_column(0, 0)[0][1], 64)

    def test_wilderness_fallback_for_uncovered_column(self) -> None:
        ts = self.TILE_SIZE
        with TemporaryDirectory() as tmp:
            reader = self._make_reader(
                Path(tmp), {(0, 0): _uniform_tile(ts, ColumnSpans(((-64, 70),)))}
            )
            # tile (5,5) has no data → flat wilderness platform, never void.
            far = reader.spans_for_column(5 * ts, 5 * ts)
            self.assertEqual(
                far,
                [(asr.WORLD_MIN_Y, asr.DEFAULT_WILDERNESS_SURFACE_Y)],
                "an uncovered column falls back to a single flat wilderness span "
                "so the snapshot has ground in tile gaps, not a void cliff",
            )

    def test_void_column_returns_empty(self) -> None:
        ts = self.TILE_SIZE
        cols = _uniform_tile(ts, ColumnSpans(((-64, 70),)))
        cols[0] = ColumnSpans(())  # index 0 = (x=0,z=0) is a true void column
        with TemporaryDirectory() as tmp:
            reader = self._make_reader(Path(tmp), {(0, 0): cols})
            self.assertEqual(
                reader.spans_for_column(0, 0), [],
                "a count=0 column decodes to empty spans (true void), distinct "
                "from the wilderness fallback used for uncovered columns",
            )

    def test_multi_span_cave_column_decoded(self) -> None:
        ts = self.TILE_SIZE
        cave = ColumnSpans(((69, 70), (-64, 40)))  # surface cap + carved-below remnant
        with TemporaryDirectory() as tmp:
            reader = self._make_reader(Path(tmp), {(0, 0): _uniform_tile(ts, cave)})
            self.assertEqual(
                reader.spans_for_column(0, 0), [(69, 70), (-64, 40)],
                "a 2-span cave column round-trips through the reader with span[0] "
                "(surface cap) first, preserving the §8.1 #2 surface convention",
            )

    def test_high_isle_span_clamped_out_by_reader(self) -> None:
        ts = self.TILE_SIZE
        # ground + a raster isle at 400..420 (above anvil 319) → isle dropped.
        col = ColumnSpans(((-64, 72), (400, 420)))
        with TemporaryDirectory() as tmp:
            reader = self._make_reader(Path(tmp), {(0, 0): _uniform_tile(ts, col)})
            self.assertEqual(
                reader.spans_for_column(0, 0), [(-64, 72)],
                "the reader applies clamp_spans_to_anvil so a >319 isle is gone "
                "before the anvil encoder validates the (<320) grid",
            )

    def test_missing_manifest_raises_with_fix_hint(self) -> None:
        with TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(FileNotFoundError, "manifest"):
                asr.RasterSpansReader(Path(tmp) / "rasters")


class ExportAnvilSpansDriverTest(unittest.TestCase):
    TILE_SIZE = 16  # one chunk == one tile here for a clean end-to-end map

    def _fixture_grid_fn(self, root: Path, column: ColumnSpans):
        ts = self.TILE_SIZE
        _write_raster_fixture(root, ts, {(0, 0): _uniform_tile(ts, column)})
        return asr.raster_spans_grid_fn(root / "rasters")

    def test_spans_backend_end_to_end_real_geometry(self) -> None:
        # A cave column: surface cap (69,70), carved void, floor remnant (-64,40).
        cave = ColumnSpans(((69, 70), (-64, 40)))
        with TemporaryDirectory() as tmp:
            grid_fn = self._fixture_grid_fn(Path(tmp) / "fx", cave)
            world_out = Path(tmp) / "world"
            result = export_anvil_world(
                world_out,
                chunk_x_min=0, chunk_x_max=0, chunk_z_min=0, chunk_z_max=0,
                spans_grid_fn=grid_fn,
            )
            self.assertEqual(result["backend"], "spans")
            self.assertEqual(result["chunks_written"], 1)
            chunks = parse_region_file(world_out / "region" / "r.0.0.mca")
            chunk = chunks[(0, 0)]
            self.assertEqual(block_at(chunk, 0, 70, 0), "minecraft:grass_block",
                             "surface cap ceiling 70 = top_block")
            self.assertEqual(block_at(chunk, 0, 55, 0), "minecraft:air",
                             "carved cave void between the cap and the remnant = air")
            self.assertEqual(block_at(chunk, 0, 40, 0), "minecraft:stone",
                             "floor remnant top = filler, not a second surface")
            self.assertEqual(block_at(chunk, 0, -64, 0), "minecraft:bedrock")

    def test_spans_backend_differs_from_flat_heightmap(self) -> None:
        # Prove the spans path is NOT the old flat synthetic world: a carved cave
        # column has air at y=55 where a flat heightmap world (surface 70) would
        # have solid stone.
        cave = ColumnSpans(((69, 70), (-64, 40)))
        with TemporaryDirectory() as tmp:
            grid_fn = self._fixture_grid_fn(Path(tmp) / "fx", cave)
            spans_world = Path(tmp) / "spans_world"
            export_anvil_world(
                spans_world, chunk_x_min=0, chunk_x_max=0,
                chunk_z_min=0, chunk_z_max=0, spans_grid_fn=grid_fn,
            )
            flat_world = Path(tmp) / "flat_world"
            export_anvil_world(
                flat_world, chunk_x_min=0, chunk_x_max=0,
                chunk_z_min=0, chunk_z_max=0, height_fn=lambda x, z: 70,
            )
            spans_chunk = parse_region_file(spans_world / "region" / "r.0.0.mca")[(0, 0)]
            flat_chunk = parse_region_file(flat_world / "region" / "r.0.0.mca")[(0, 0)]
            self.assertEqual(block_at(flat_chunk, 0, 55, 0), "minecraft:stone",
                             "flat heightmap world fills solid below surface 70")
            self.assertEqual(block_at(spans_chunk, 0, 55, 0), "minecraft:air",
                             "spans world carves the cave void — the whole point of "
                             "wiring real spans into the anvil snapshot")

    def test_exactly_one_backend_required_neither(self) -> None:
        with TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(ValueError, "恰好传一个后端"):
                export_anvil_world(
                    Path(tmp), chunk_x_min=0, chunk_x_max=0,
                    chunk_z_min=0, chunk_z_max=0,
                )

    def test_exactly_one_backend_required_both(self) -> None:
        with TemporaryDirectory() as tmp:
            grid_fn = self._fixture_grid_fn(
                Path(tmp) / "fx", ColumnSpans(((-64, 70),))
            )
            with self.assertRaisesRegex(ValueError, "恰好传一个后端"):
                export_anvil_world(
                    Path(tmp) / "w", chunk_x_min=0, chunk_x_max=0,
                    chunk_z_min=0, chunk_z_max=0,
                    height_fn=lambda x, z: 64, spans_grid_fn=grid_fn,
                )

    def test_height_fn_backend_still_works(self) -> None:
        # Backward-compat: the legacy single-height path still produces a world
        # and reports backend="height".
        with TemporaryDirectory() as tmp:
            result = export_anvil_world(
                Path(tmp), chunk_x_min=0, chunk_x_max=0,
                chunk_z_min=0, chunk_z_max=0, height_fn=lambda x, z: 64,
            )
            self.assertEqual(result["backend"], "height")
            self.assertEqual(result["chunks_written"], 1)
            chunk = parse_region_file(Path(tmp) / "region" / "r.0.0.mca")[(0, 0)]
            self.assertEqual(block_at(chunk, 0, 64, 0), "minecraft:grass_block")


class RasterSpansReaderOnDiskLayoutTest(unittest.TestCase):
    """Pin the on-disk byte layout the reader consumes against the raw encoder.

    Guards that the reader does not silently diverge from raster_export's
    ``spans.bin`` stride / sentinel encoding (a mismatch would shift every column
    read by some bytes and corrupt the whole snapshot).
    """

    def test_reader_agrees_with_raw_struct_decode(self) -> None:
        ts = 4
        col = ColumnSpans(((-64, 80), (120, 140)))
        with TemporaryDirectory() as tmp:
            raster_dir = _write_raster_fixture(
                Path(tmp), ts, {(0, 0): _uniform_tile(ts, col)}
            )
            reader = asr.RasterSpansReader(raster_dir)
            # Raw decode of index 0 from spans.bin (16 bytes, 8 i16 LE).
            spans_bytes = (raster_dir / "tile_0_0" / "spans.bin").read_bytes()
            flat = struct.unpack("<8h", spans_bytes[0:16])
            self.assertEqual(
                flat[0:4], (-64, 80, 120, 140),
                "raw spans.bin must hold the two real spans in slots 0..1",
            )
            self.assertEqual(reader.spans_for_column(0, 0), [(-64, 80), (120, 140)])


if __name__ == "__main__":
    unittest.main()
