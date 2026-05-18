"""Tests for real NBT paste in layout runner (plan-dandao-path-v1 PR-8).

Covers:
- load_structure round-trip (write → read → verify blocks)
- _rotate_offset for all 4 rotations
- _paste_nbt: world coordinate computation with rotation
- _paste_nbt: missing file produces empty result
- export_placement_manifest JSON structure
- BlockPlacement.to_dict format
"""

from __future__ import annotations

import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

_worldgen_root = str(Path(__file__).resolve().parents[2])
_repo_root = str(Path(__file__).resolve().parents[3])
sys.path.insert(0, _worldgen_root)
# nbt_builder lives at repo_root/scripts/nbt/
sys.path.insert(0, str(Path(_repo_root) / "scripts" / "nbt"))

from nbt_builder import StructureBuilder, load_structure  # noqa: E402

from scripts.terrain_gen.layouts.runner import (  # noqa: E402
    BlockPlacement,
    NbtPasteResult,
    _paste_nbt,
    _rotate_offset,
    export_placement_manifest,
)


class LoadStructureTests(unittest.TestCase):
    """Tests for nbt_builder.load_structure round-trip."""

    def test_roundtrip_preserves_blocks(self) -> None:
        with tempfile.NamedTemporaryFile(suffix=".nbt", delete=False) as f:
            path = f.name
        try:
            sb = StructureBuilder(5, 3, 5)
            sb.set_block(0, 0, 0, "minecraft:stone_bricks")
            sb.set_block(1, 2, 3, "minecraft:oak_log")
            sb.set_block(4, 0, 4, "minecraft:cobblestone")
            sb.save(path)

            blocks = load_structure(path)
            self.assertEqual(len(blocks), 3, "Should load 3 non-air blocks")

            by_pos = {b.pos: b for b in blocks}
            self.assertIn((0, 0, 0), by_pos)
            self.assertEqual(by_pos[(0, 0, 0)].block_name, "minecraft:stone_bricks")
            self.assertIn((1, 2, 3), by_pos)
            self.assertEqual(by_pos[(1, 2, 3)].block_name, "minecraft:oak_log")
            self.assertIn((4, 0, 4), by_pos)
            self.assertEqual(by_pos[(4, 0, 4)].block_name, "minecraft:cobblestone")
        finally:
            os.unlink(path)

    def test_roundtrip_with_properties(self) -> None:
        with tempfile.NamedTemporaryFile(suffix=".nbt", delete=False) as f:
            path = f.name
        try:
            sb = StructureBuilder(3, 3, 3)
            sb.set_block(0, 0, 0, "minecraft:oak_stairs", {"facing": "north", "half": "bottom"})
            sb.save(path)

            blocks = load_structure(path)
            self.assertEqual(len(blocks), 1)
            self.assertEqual(blocks[0].block_name, "minecraft:oak_stairs")
            self.assertEqual(blocks[0].properties["facing"], "north")
            self.assertEqual(blocks[0].properties["half"], "bottom")
        finally:
            os.unlink(path)

    def test_empty_structure_returns_empty_list(self) -> None:
        with tempfile.NamedTemporaryFile(suffix=".nbt", delete=False) as f:
            path = f.name
        try:
            sb = StructureBuilder(3, 3, 3)
            sb.save(path)

            blocks = load_structure(path)
            self.assertEqual(len(blocks), 0, "Empty structure should return no blocks")
        finally:
            os.unlink(path)


class RotateOffsetTests(unittest.TestCase):
    """Tests for _rotate_offset Y-axis rotation."""

    def test_rotation_0_is_identity(self) -> None:
        self.assertEqual(_rotate_offset(1, 2, 3, 0), (1, 2, 3))

    def test_rotation_90(self) -> None:
        self.assertEqual(_rotate_offset(1, 0, 0, 90), (0, 0, 1))
        self.assertEqual(_rotate_offset(0, 0, 1, 90), (-1, 0, 0))

    def test_rotation_180(self) -> None:
        self.assertEqual(_rotate_offset(1, 0, 0, 180), (-1, 0, 0))
        self.assertEqual(_rotate_offset(0, 0, 1, 180), (0, 0, -1))

    def test_rotation_270(self) -> None:
        self.assertEqual(_rotate_offset(1, 0, 0, 270), (0, 0, -1))
        self.assertEqual(_rotate_offset(0, 0, 1, 270), (1, 0, 0))

    def test_rotation_preserves_y(self) -> None:
        for rot in (0, 90, 180, 270):
            result = _rotate_offset(5, 7, 3, rot)
            self.assertEqual(result[1], 7, f"Y should be preserved at rotation {rot}")

    def test_full_360_returns_to_origin(self) -> None:
        """Applying 90-degree rotation 4 times should return to original."""
        pos = (3, 5, 7)
        current = pos
        for _ in range(4):
            current = _rotate_offset(*current, 90)
        self.assertEqual(current, pos, "4 × 90° rotation should return to original")

    def test_invalid_rotation_raises(self) -> None:
        with self.assertRaises(ValueError):
            _rotate_offset(1, 2, 3, 45)


class PasteNbtTests(unittest.TestCase):
    """Tests for _paste_nbt with real NBT files."""

    def test_paste_produces_world_coordinates(self) -> None:
        with tempfile.NamedTemporaryFile(suffix=".nbt", delete=False) as f:
            path = f.name
        try:
            sb = StructureBuilder(3, 3, 3)
            sb.set_block(0, 0, 0, "minecraft:stone")
            sb.set_block(1, 0, 2, "minecraft:dirt")
            sb.save(path)

            result = _paste_nbt((100, 64, 200), 0, path)
            self.assertEqual(result.block_count, 2)
            positions = {b.world_pos for b in result.blocks}
            self.assertIn((100, 64, 200), positions)
            self.assertIn((101, 64, 202), positions)
        finally:
            os.unlink(path)

    def test_paste_with_rotation(self) -> None:
        with tempfile.NamedTemporaryFile(suffix=".nbt", delete=False) as f:
            path = f.name
        try:
            sb = StructureBuilder(3, 3, 3)
            sb.set_block(1, 0, 0, "minecraft:stone")
            sb.save(path)

            # Rotation 90: (1,0,0) → (0,0,1)
            result = _paste_nbt((100, 64, 200), 90, path)
            self.assertEqual(result.block_count, 1)
            self.assertEqual(result.blocks[0].world_pos, (100, 64, 201))
        finally:
            os.unlink(path)

    def test_paste_missing_file_returns_empty(self) -> None:
        result = _paste_nbt((100, 64, 200), 0, "/nonexistent/file.nbt")
        self.assertEqual(result.block_count, 0)
        self.assertEqual(len(result.blocks), 0)

    def test_paste_preserves_block_names(self) -> None:
        with tempfile.NamedTemporaryFile(suffix=".nbt", delete=False) as f:
            path = f.name
        try:
            sb = StructureBuilder(3, 3, 3)
            sb.set_block(0, 0, 0, "minecraft:gold_block")
            sb.save(path)

            result = _paste_nbt((0, 0, 0), 0, path)
            self.assertEqual(result.blocks[0].block_name, "minecraft:gold_block")
        finally:
            os.unlink(path)


class BlockPlacementTests(unittest.TestCase):
    """Tests for BlockPlacement.to_dict serialization."""

    def test_to_dict_basic(self) -> None:
        bp = BlockPlacement((10, 20, 30), "minecraft:stone", {})
        d = bp.to_dict()
        self.assertEqual(d["pos"], [10, 20, 30])
        self.assertEqual(d["block"], "minecraft:stone")
        self.assertNotIn("properties", d, "Empty properties should be omitted")

    def test_to_dict_with_properties(self) -> None:
        bp = BlockPlacement((10, 20, 30), "minecraft:oak_stairs", {"facing": "north"})
        d = bp.to_dict()
        self.assertEqual(d["properties"], {"facing": "north"})


class ExportManifestTests(unittest.TestCase):
    """Tests for export_placement_manifest JSON output."""

    def test_manifest_structure(self) -> None:
        result = NbtPasteResult(
            nbt_path="test.nbt",
            world_pos=(100, 64, 200),
            rotation=0,
            block_count=1,
            blocks=[BlockPlacement((100, 64, 200), "minecraft:stone", {})],
        )
        with tempfile.NamedTemporaryFile(suffix=".json", delete=False, mode="w") as f:
            path = f.name
        try:
            export_placement_manifest([result], path)
            with open(path) as f:
                manifest = json.load(f)

            self.assertEqual(manifest["version"], 1)
            self.assertEqual(len(manifest["structures"]), 1)
            s = manifest["structures"][0]
            self.assertEqual(s["nbt_path"], "test.nbt")
            self.assertEqual(s["origin"], [100, 64, 200])
            self.assertEqual(s["rotation"], 0)
            self.assertEqual(len(s["blocks"]), 1)
            self.assertEqual(s["blocks"][0]["block"], "minecraft:stone")
        finally:
            os.unlink(path)

    def test_empty_manifest(self) -> None:
        with tempfile.NamedTemporaryFile(suffix=".json", delete=False, mode="w") as f:
            path = f.name
        try:
            export_placement_manifest([], path)
            with open(path) as f:
                manifest = json.load(f)
            self.assertEqual(manifest["version"], 1)
            self.assertEqual(len(manifest["structures"]), 0)
        finally:
            os.unlink(path)


if __name__ == "__main__":
    unittest.main()
