"""worldgen-v4 P0 §8.1 #8 — anvil spans 路径（按段 loop fill）饱和测试.

跑法（worldgen/scripts/terrain_gen）：
    python3 -m unittest test_anvil_spans -v

覆盖矩阵：
  ① 普通列 spans-path 与 height-path 字节级等价（folding 不改景观）
  ② 洞穴列：地表段顶=top_block，地表段内=filler，洞穴空腔=air，残底实心
  ③ 浮岛列：地表段 + 高位悬空段都实心，二者之间气隙=air
  ④ 空虚空列：除 bedrock 外整列 air，heightmap 退到世界底
  ⑤ heightmap 取列内最高实心段顶（max ceiling）：洞穴列=地表段顶；浮岛列=岛顶而非地表
     段顶（岛方块实写，heightmap 必须报岛顶否则客户端把岛段当天空）
  ⑥ invalid spans_grid：维度错 / 段重叠 / 段出世界范围 → ValueError
  ⑦ compressed 变体 zlib round-trip
  ⑧ DataVersion / section 数与 height-path 一致
"""

from __future__ import annotations

import sys
import unittest
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import anvil_export as ax  # noqa: E402
from test_anvil_export import parse_nbt  # noqa: E402


def uniform_spans(spans):
    return [[list(spans) for _ in range(ax.CHUNK_WIDTH)] for _ in range(ax.CHUNK_WIDTH)]


def block_at(chunk: dict, x: int, y: int, z: int) -> str:
    """Decode the block name at world (x, y, z) from a parsed chunk."""
    section_y = y >> 4
    section = next(s for s in chunk["sections"] if s["Y"] == section_y)
    palette = [e["Name"] for e in section["block_states"]["palette"]]
    if len(palette) == 1:
        return palette[0]
    data = section["block_states"]["data"]
    local_y = y - section_y * 16
    block_index = local_y * 256 + z * 16 + x
    bits = max(4, (len(palette) - 1).bit_length())
    per_long = 64 // bits
    long_idx = block_index // per_long
    within = block_index % per_long
    raw = data[long_idx]
    if raw < 0:
        raw += 1 << 64
    palette_idx = (raw >> (within * bits)) & ((1 << bits) - 1)
    return palette[palette_idx]


class AnvilSpansEquivalenceTest(unittest.TestCase):
    def test_normal_column_matches_height_path_bytewise(self) -> None:
        heights = [[70] * 16 for _ in range(16)]
        spans = uniform_spans([(-64, 70)])
        self.assertEqual(
            ax.chunk_to_nbt(3, 4, heights),
            ax.chunk_to_nbt_from_spans(3, 4, spans),
            "a single (bedrock, height) span must serialize byte-identically to "
            "the legacy height path — the span fill is a faithful generalization",
        )

    def test_data_version_and_section_count_preserved(self) -> None:
        chunk = parse_nbt(ax.chunk_to_nbt_from_spans(0, 0, uniform_spans([(-64, 70)])))
        self.assertEqual(chunk["DataVersion"], 3465)
        self.assertEqual(len(chunk["sections"]), 24)


class AnvilSpansShapeTest(unittest.TestCase):
    def test_cave_column_carves_air_void(self) -> None:
        # surface cap (69,70), floor remnant (-64,40); the void is 41..68.
        spans = uniform_spans([(69, 70), (-64, 40)])
        chunk = parse_nbt(ax.chunk_to_nbt_from_spans(0, 0, spans))
        self.assertEqual(block_at(chunk, 0, 70, 0), "minecraft:grass_block",
                         "surface span ceiling = top_block")
        self.assertEqual(block_at(chunk, 0, 69, 0), "minecraft:stone",
                         "inside the surface cap = filler")
        self.assertEqual(block_at(chunk, 0, 55, 0), "minecraft:air",
                         "carved cave void = air")
        self.assertEqual(block_at(chunk, 0, 40, 0), "minecraft:stone",
                         "floor remnant top = filler (not the surface top)")
        self.assertEqual(block_at(chunk, 0, -64, 0), "minecraft:bedrock")

    def test_sky_isle_column_has_floating_solid_above_air_gap(self) -> None:
        # Isle within the 1.20.1 anvil ceiling (world Y < 320); the full span
        # range [-64, 432) is only used by the live raster server, not anvil.
        spans = uniform_spans([(-64, 72), (290, 310)])
        chunk = parse_nbt(ax.chunk_to_nbt_from_spans(0, 0, spans))
        self.assertEqual(block_at(chunk, 0, 72, 0), "minecraft:grass_block",
                         "ground surface = top_block")
        self.assertEqual(block_at(chunk, 0, 150, 0), "minecraft:air",
                         "gap below the isle = air")
        self.assertEqual(block_at(chunk, 0, 290, 0), "minecraft:stone",
                         "isle bottom = filler")
        self.assertEqual(block_at(chunk, 0, 310, 0), "minecraft:stone",
                         "isle is not the walkable surface → filler, not top_block")

    def test_void_column_is_air_above_bedrock(self) -> None:
        spans = uniform_spans([])
        chunk = parse_nbt(ax.chunk_to_nbt_from_spans(0, 0, spans))
        self.assertEqual(block_at(chunk, 0, -64, 0), "minecraft:bedrock")
        self.assertEqual(block_at(chunk, 0, 0, 0), "minecraft:air")
        self.assertEqual(block_at(chunk, 0, 64, 0), "minecraft:air")

    def _first_column_heightmap(self, chunk: dict) -> int:
        """Decode the (x=0, z=0) MOTION_BLOCKING heightmap entry to its raw y+1."""
        first_long = chunk["Heightmaps"]["MOTION_BLOCKING"][0]
        if first_long < 0:
            first_long += 1 << 64
        return first_long & ((1 << 9) - 1)

    def test_heightmap_uses_highest_solid_for_cave_column(self) -> None:
        # Cave column: surface cap (69,70) on top, floor remnant (-64,40) below.
        # Highest solid == the surface cap ceiling (70), so the heightmap is 70
        # whether we read span[0].ceiling or max-over-spans — this pins that the
        # floor remnant (40) is NOT mistaken for the top.
        spans = uniform_spans([(69, 70), (-64, 40)])
        chunk = parse_nbt(ax.chunk_to_nbt_from_spans(0, 0, spans))
        self.assertEqual(
            self._first_column_heightmap(chunk),
            70 + 1 - ax.WORLD_MIN_Y,
            "heightmap must store the highest solid block (cap ceiling=70), not "
            "the carved-below floor remnant (40)",
        )

    def test_heightmap_uses_isle_top_not_ground_span(self) -> None:
        # Sky-isle column: ground span[0]=(-64,72) is the *walkable* surface, but
        # an isle span (290,310) floats above and its blocks ARE written into the
        # sections.  MC WORLD_SURFACE/MOTION_BLOCKING must report the isle top
        # (310), NOT span[0].ceiling (72) — otherwise the client treats 73..310
        # as open sky and lighting/occlusion break above the ground.
        spans = uniform_spans([(-64, 72), (290, 310)])
        chunk = parse_nbt(ax.chunk_to_nbt_from_spans(0, 0, spans))
        # Sanity: the isle block really is solid at 310 (so the heightmap claim
        # is backed by a real block).
        self.assertEqual(block_at(chunk, 0, 310, 0), "minecraft:stone")
        self.assertEqual(
            self._first_column_heightmap(chunk),
            310 + 1 - ax.WORLD_MIN_Y,
            "heightmap must store the highest solid block (isle top=310), not the "
            "walkable ground span[0].ceiling=72 — solid blocks exist up to 310",
        )

    def test_void_column_heightmap_floors_to_world_min(self) -> None:
        chunk = parse_nbt(ax.chunk_to_nbt_from_spans(0, 0, uniform_spans([])))
        self.assertEqual(
            self._first_column_heightmap(chunk),
            ax.WORLD_MIN_Y + 1 - ax.WORLD_MIN_Y,
        )


class AnvilSpansValidationTest(unittest.TestCase):
    def test_wrong_grid_dimensions_rejected(self) -> None:
        with self.assertRaises(ValueError):
            ax.chunk_to_nbt_from_spans(0, 0, [[[(-64, 70)]] * 16] * 15)

    def test_overlapping_spans_rejected(self) -> None:
        grid = uniform_spans([(-64, 50), (40, 72)])  # overlap
        with self.assertRaisesRegex(ValueError, "段重叠"):
            ax.chunk_to_nbt_from_spans(0, 0, grid)

    def test_span_out_of_world_range_rejected(self) -> None:
        grid = uniform_spans([(0, 400)])  # 400 >= WORLD_MAX_Y(320)
        with self.assertRaisesRegex(ValueError, "超出"):
            ax.chunk_to_nbt_from_spans(0, 0, grid)

    def test_floor_above_ceiling_rejected(self) -> None:
        grid = uniform_spans([(50, 10)])
        with self.assertRaisesRegex(ValueError, "floor_y=50 > ceiling_y=10"):
            ax.chunk_to_nbt_from_spans(0, 0, grid)


class AnvilSpansCompressedTest(unittest.TestCase):
    def test_compressed_round_trips(self) -> None:
        spans = uniform_spans([(-64, 70)])
        compressed = ax.chunk_to_nbt_from_spans_compressed(1, 2, spans)
        self.assertEqual(
            zlib.decompress(compressed),
            ax.chunk_to_nbt_from_spans(1, 2, spans),
        )


if __name__ == "__main__":
    unittest.main()
