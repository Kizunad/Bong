"""Carver owner 与 manifest provenance 必须是两套独立语义。"""

from __future__ import annotations

import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from types import SimpleNamespace
from unittest.mock import patch

import numpy as np

from scripts.terrain_gen.bakers.raster_export import (
    SPANS_COUNT_FILE,
    SPANS_FILE,
    _carved_spans_for_tile,
    _tile_carver_assignments,
    _zone_carver_chains,
    build_raster_bake_plan,
    export_rasters,
    regen_zone,
)
from scripts.terrain_gen.blueprint import (
    DEFAULT_BLUEPRINT_PATH,
    DEFAULT_PROFILES_PATH,
    load_blueprint,
    load_profile_catalog,
)
from scripts.terrain_gen.fields import (
    ColumnSpans,
    TileFieldBuffer,
    WorldTile,
    encode_spans_arrays,
)
from scripts.terrain_gen.noise import _tile_coords
from scripts.terrain_gen.spans_fold import spans_for_tile
from scripts.terrain_gen.stitcher import (
    _blend_tile_layers,
    _compute_boundary_weight_array,
    build_generation_plan,
    synthesize_fields,
)


TILE_SIZE = 2
CORE_LAYERS = (
    "height",
    "surface_id",
    "subsurface_id",
    "biome_id",
    "water_level",
    "feature_mask",
    "boundary_weight",
)


def _buffer() -> TileFieldBuffer:
    tile = WorldTile(
        tile_x=0,
        tile_z=0,
        min_x=0,
        max_x=TILE_SIZE - 1,
        min_z=0,
        max_z=TILE_SIZE - 1,
    )
    return TileFieldBuffer.create(tile, TILE_SIZE, CORE_LAYERS)


class _MarkerCarver:
    def __init__(self, marker_y: int, name: str = "marker") -> None:
        self.marker_y = marker_y
        self.name = name

    def carve_column(
        self,
        column: ColumnSpans,
        _world_x: int,
        _world_z: int,
        _seed: int,
    ) -> ColumnSpans:
        return ColumnSpans((*column.spans, (self.marker_y, self.marker_y)))


class BlendTileCarverOwnerTest(unittest.TestCase):
    def test_zero_weight_zone_stays_provenance_only(self) -> None:
        base = _buffer()
        overlay = _buffer()
        zone = SimpleNamespace(name="provenance_only")

        with patch(
            "scripts.terrain_gen.stitcher._compute_boundary_weight_array",
            return_value=np.zeros((TILE_SIZE, TILE_SIZE), dtype=np.float32),
        ):
            _blend_tile_layers(base, overlay, zone)

        self.assertEqual(
            base.contributing_zones,
            ["provenance_only"],
            "粗 AABB 命中仍应保留在 manifest provenance",
        )
        self.assertEqual(
            base.carver_owner_zones,
            [],
            "真实 boundary weight 全零时不得取得 carver 控制权",
        )
        np.testing.assert_array_equal(
            base.carver_owner_index,
            np.zeros(TILE_SIZE * TILE_SIZE, dtype=np.uint16),
            err_msg="零权重 zone 不得取得任何列的结构 ownership",
        )

    def test_positive_weight_zone_is_recorded_as_carver_owner(self) -> None:
        base = _buffer()
        overlay = _buffer()
        zone = SimpleNamespace(name="positive_owner")

        with (
            patch(
                "scripts.terrain_gen.stitcher._compute_boundary_weight_array",
                return_value=np.ones((TILE_SIZE, TILE_SIZE), dtype=np.float32),
            ),
            patch(
                "scripts.terrain_gen.stitcher._coherent_noise_2d_array",
                return_value=np.zeros((TILE_SIZE, TILE_SIZE), dtype=np.float32),
            ),
        ):
            _blend_tile_layers(base, overlay, zone)

        self.assertEqual(
            base.contributing_zones,
            ["positive_owner"],
            "期望 provenance 为 ['positive_owner'], 因为正权重 zone 应保留记录; "
            f"实际为 {base.contributing_zones!r}",
        )
        self.assertEqual(
            base.carver_owner_zones,
            ["positive_owner"],
            "实际参与过 blend 的 zone 才能成为 carver owner",
        )
        np.testing.assert_array_equal(
            base.carver_owner_index,
            np.ones(TILE_SIZE * TILE_SIZE, dtype=np.uint16),
            err_msg="全正权重 zone 应取得 tile 内每一列的结构 ownership",
        )

    def test_structural_owner_is_per_column_and_later_dominant_zone_wins(self) -> None:
        base = _buffer()
        first_overlay = _buffer()
        first_zone = SimpleNamespace(name="first_owner")
        first_weight = np.array(
            [[0.0, 0.49], [0.5, 1.0]],
            dtype=np.float32,
        )

        with (
            patch(
                "scripts.terrain_gen.stitcher._compute_boundary_weight_array",
                return_value=first_weight,
            ),
            patch(
                "scripts.terrain_gen.stitcher._coherent_noise_2d_array",
                return_value=np.zeros((TILE_SIZE, TILE_SIZE), dtype=np.float32),
            ),
        ):
            _blend_tile_layers(base, first_overlay, first_zone)

        np.testing.assert_array_equal(
            base.carver_owner_index,
            np.array([0, 0, 1, 1], dtype=np.uint16),
            err_msg=(
                "结构 ownership 应在 weight >= 0.5 时逐列移交; "
                f"实际 owner index 为 {base.carver_owner_index.tolist()}"
            ),
        )

        second_overlay = _buffer()
        second_zone = SimpleNamespace(name="second_owner")
        second_weight = np.array(
            [[1.0, 0.5], [0.49, 0.0]],
            dtype=np.float32,
        )
        with (
            patch(
                "scripts.terrain_gen.stitcher._compute_boundary_weight_array",
                return_value=second_weight,
            ),
            patch(
                "scripts.terrain_gen.stitcher._coherent_noise_2d_array",
                return_value=np.zeros((TILE_SIZE, TILE_SIZE), dtype=np.float32),
            ),
        ):
            _blend_tile_layers(base, second_overlay, second_zone)

        self.assertEqual(
            base.carver_owner_zones,
            ["first_owner", "second_owner"],
            "owner palette 应按实际结构移交顺序稳定追加",
        )
        np.testing.assert_array_equal(
            base.carver_owner_index,
            np.array([2, 2, 1, 1], dtype=np.uint16),
            err_msg=(
                "后 blend 的主导 zone 应只覆盖自己 weight >= 0.5 的列, "
                "弱混合列不得抢走既有结构 owner; "
                f"实际 owner index 为 {base.carver_owner_index.tolist()}"
            ),
        )


class TileCarverChainOwnershipTest(unittest.TestCase):
    def test_assignments_use_each_columns_final_owner(self) -> None:
        first_chain = [object()]
        second_chain = [object()]
        buffer = SimpleNamespace(
            tile_size=2,
            contributing_zones=["provenance_only", "first_owner", "second_owner"],
            carver_owner_zones=["first_owner", "second_owner"],
            carver_owner_index=np.array([1, 2, 2, 0], dtype=np.uint16),
        )

        assignments = _tile_carver_assignments(
            buffer,
            {
                "provenance_only": [object()],
                "first_owner": first_chain,
                "second_owner": second_chain,
            },
        )

        self.assertEqual(
            [zone_name for zone_name, _mask, _chain in assignments],
            ["first_owner", "second_owner"],
            "export 应按 owner palette 解析每个最终结构 owner 的 chain",
        )
        self.assertIs(assignments[0][2], first_chain)
        self.assertIs(assignments[1][2], second_chain)
        np.testing.assert_array_equal(
            assignments[0][1],
            np.array([True, False, False, False]),
            err_msg="first_owner chain 只能作用于 owner index=1 的列",
        )
        np.testing.assert_array_equal(
            assignments[1][1],
            np.array([False, True, True, False]),
            err_msg="second_owner chain 只能作用于 owner index=2 的列",
        )

    def test_no_positive_owner_means_no_carving(self) -> None:
        buffer = SimpleNamespace(
            tile_size=2,
            contributing_zones=["provenance_only"],
            carver_owner_zones=[],
            carver_owner_index=np.zeros(4, dtype=np.uint16),
        )

        self.assertEqual(
            _tile_carver_assignments(buffer, {"provenance_only": [object()]}),
            [],
            "owner 为空时不得回退到 provenance zone 雕刻整块 tile",
        )

    def test_malformed_owner_index_fails_loudly(self) -> None:
        wrong_shape = SimpleNamespace(
            tile_size=2,
            carver_owner_zones=["owner"],
            carver_owner_index=np.ones(3, dtype=np.uint16),
        )
        with self.assertRaisesRegex(ValueError, "carver_owner_index"):
            _tile_carver_assignments(wrong_shape, {"owner": [object()]})

        unknown_owner = SimpleNamespace(
            tile_size=2,
            carver_owner_zones=["owner"],
            carver_owner_index=np.array([0, 1, 2, 0], dtype=np.uint16),
        )
        with self.assertRaisesRegex(ValueError, "owner palette"):
            _tile_carver_assignments(unknown_owner, {"owner": [object()]})

    def test_each_owner_chain_only_carves_its_assigned_columns(self) -> None:
        buffer = _buffer()
        buffer.carver_owner_zones = ["first_owner", "second_owner"]
        buffer.carver_owner_index = np.array([1, 2, 2, 0], dtype=np.uint16)

        baseline = spans_for_tile(buffer)
        carved = _carved_spans_for_tile(
            buffer,
            {
                "first_owner": [_MarkerCarver(200)],
                "second_owner": [_MarkerCarver(220)],
            },
        )

        self.assertEqual(
            carved[0].spans,
            (*baseline[0].spans, (200, 200)),
            "first_owner 列必须只应用 first_owner chain",
        )
        for index in (1, 2):
            self.assertEqual(
                carved[index].spans,
                (*baseline[index].spans, (220, 220)),
                f"第 {index} 列必须只应用 second_owner chain",
            )
        self.assertEqual(
            carved[3].spans,
            baseline[3].spans,
            "owner index=0 的 wilderness 列不得被任一 chain 改写",
        )

    def test_floating_island_fold_is_suppressed_only_for_owner_columns(self) -> None:
        buffer = _buffer()
        area = TILE_SIZE * TILE_SIZE
        buffer.layers["sky_island_mask"] = np.ones(area, dtype=np.float32)
        buffer.layers["sky_island_base_y"] = np.full(
            area,
            180.0,
            dtype=np.float32,
        )
        buffer.layers["sky_island_thickness"] = np.full(
            area,
            10.0,
            dtype=np.float32,
        )
        buffer.carver_owner_zones = ["sky_owner"]
        buffer.carver_owner_index = np.array([1, 0, 0, 0], dtype=np.uint16)

        pure_fold = spans_for_tile(buffer)
        carved = _carved_spans_for_tile(
            buffer,
            {"sky_owner": [_MarkerCarver(220, name="floating_island")]},
        )

        self.assertNotIn(
            (180, 190),
            carved[0].spans,
            "floating owner 列必须抑制旧 2D slab, 避免浮岛双源",
        )
        self.assertIn(
            (220, 220),
            carved[0].spans,
            "floating owner 列必须保留 carver 生成的新浮岛结构",
        )
        for index in (1, 2, 3):
            self.assertEqual(
                carved[index].spans,
                pure_fold[index].spans,
                f"非 floating owner 的第 {index} 列不得被全 tile suppression 改写",
            )


class RealBlueprintCarverOwnerTest(unittest.TestCase):
    def test_tile_6_minus_7_keeps_blood_valley_as_provenance_only(self) -> None:
        blueprint = load_blueprint(DEFAULT_BLUEPRINT_PATH)
        catalog = load_profile_catalog(DEFAULT_PROFILES_PATH)
        tile_size = 512
        tile = WorldTile(
            tile_x=6,
            tile_z=-7,
            min_x=6 * tile_size,
            max_x=7 * tile_size - 1,
            min_z=-7 * tile_size,
            max_z=-6 * tile_size - 1,
        )
        blood_valley = next(
            zone for zone in blueprint.zones if zone.name == "blood_valley"
        )
        wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
        blood_weight = _compute_boundary_weight_array(blood_valley, wx, wz)
        self.assertEqual(
            float(np.nanmax(blood_weight)),
            0.0,
            "真实 witness 前提漂移: tile_6_-7 上 blood_valley 权重应全零",
        )

        with TemporaryDirectory() as temp_dir:
            plan = build_generation_plan(
                blueprint,
                catalog,
                DEFAULT_BLUEPRINT_PATH,
                DEFAULT_PROFILES_PATH,
                Path(temp_dir),
                tile_size,
            )
            plan.tiles = [tile]
            buffer = synthesize_fields(plan).tiles[0]
            assignments = _tile_carver_assignments(
                buffer,
                _zone_carver_chains(plan),
            )

        self.assertIn(
            "blood_valley",
            buffer.contributing_zones,
            "粗 AABB provenance 仍应记录 blood_valley, 避免破坏 manifest/debug 兼容",
        )
        self.assertNotIn(
            "blood_valley",
            buffer.carver_owner_zones,
            "零权重 blood_valley 不得取得 tile_6_-7 的 carver ownership",
        )
        self.assertIn(
            "zhanhun_plain",
            buffer.carver_owner_zones,
            "真实正权重的战魂平野应保留为该 tile 的几何 owner",
        )
        self.assertEqual(
            assignments,
            [],
            "战魂平野边缘没有正贡献 carver owner, chain 必须精确为空",
        )
        folded = spans_for_tile(buffer, suppress_fold_isle=False)
        carved = _carved_spans_for_tile(
            buffer,
            _zone_carver_chains(plan),
        )
        self.assertEqual(
            [column.spans for column in carved],
            [column.spans for column in folded],
            "空 owner chain 必须保证 tile_6_-7 的所有 spans 差异列为 0",
        )

    def test_tile_4_minus_7_carves_only_final_blood_valley_owner_columns(self) -> None:
        blueprint = load_blueprint(DEFAULT_BLUEPRINT_PATH)
        catalog = load_profile_catalog(DEFAULT_PROFILES_PATH)
        tile_size = 512
        tile = WorldTile(
            tile_x=4,
            tile_z=-7,
            min_x=4 * tile_size,
            max_x=5 * tile_size - 1,
            min_z=-7 * tile_size,
            max_z=-6 * tile_size - 1,
        )
        blood_valley = next(
            zone for zone in blueprint.zones if zone.name == "blood_valley"
        )
        wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
        blood_weight = _compute_boundary_weight_array(blood_valley, wx, wz).ravel()
        self.assertGreater(
            int(np.count_nonzero(blood_weight > 0.0)),
            0,
            "真实 witness 前提漂移: tile_4_-7 必须与 blood_valley 部分相交",
        )
        self.assertGreater(
            int(np.count_nonzero(blood_weight == 0.0)),
            0,
            "真实 witness 前提漂移: tile_4_-7 必须同时含血谷零权重列",
        )

        with TemporaryDirectory() as temp_dir:
            output_dir = Path(temp_dir) / "output"
            plan = build_generation_plan(
                blueprint,
                catalog,
                DEFAULT_BLUEPRINT_PATH,
                DEFAULT_PROFILES_PATH,
                output_dir,
                tile_size,
            )
            plan.tiles = [tile]
            plan.bake_plan = build_raster_bake_plan(plan, output_dir)
            fields = synthesize_fields(plan)
            buffer = fields.tiles[0]
            zone_chains = _zone_carver_chains(plan)
            assignments = _tile_carver_assignments(buffer, zone_chains)
            blood_assignment = next(
                assignment
                for assignment in assignments
                if assignment[0] == "blood_valley"
            )
            blood_owner_mask = blood_assignment[1]

            folded = spans_for_tile(buffer, suppress_fold_isle=False)
            carved = _carved_spans_for_tile(buffer, zone_chains)
            changed = np.fromiter(
                (before.spans != after.spans for before, after in zip(folded, carved)),
                dtype=bool,
                count=tile_size * tile_size,
            )
            self.assertGreater(
                int(np.count_nonzero(changed)),
                0,
                "真实 witness 必须有血谷 owner 列被 canyon chain 实际雕刻",
            )
            self.assertFalse(
                bool(np.any(changed & ~blood_owner_mask)),
                "carver 不得改写最终结构 owner 不是 blood_valley 的列; "
                f"越界差异列为 {int(np.count_nonzero(changed & ~blood_owner_mask))}",
            )
            self.assertFalse(
                bool(np.any(changed & (blood_weight == 0.0))),
                "blood_valley 权重为 0 的列不得被 canyon chain 改写; "
                f"零权重差异列为 {int(np.count_nonzero(changed & (blood_weight == 0.0)))}",
            )

            expected_counts, expected_spans = encode_spans_arrays(carved)
            with patch(
                "scripts.terrain_gen.bakers.raster_export."
                "build_novice_poi_manifest_payload",
                return_value=[],
            ):
                artifacts = export_rasters(
                    plan,
                    fields,
                    layer_whitelist=set(),
                )
            tile_dir = artifacts["raster_dir"] / tile.tile_id
            count_path = tile_dir / SPANS_COUNT_FILE
            spans_path = tile_dir / SPANS_FILE
            full_count_bytes = count_path.read_bytes()
            full_spans_bytes = spans_path.read_bytes()
            self.assertEqual(
                full_count_bytes,
                expected_counts.tobytes(),
                "full export 的 spans_count 必须来自逐列 owner carve 结果",
            )
            self.assertEqual(
                full_spans_bytes,
                expected_spans.tobytes(),
                "full export 的 spans 必须来自逐列 owner carve 结果",
            )
            manifest = json.loads(artifacts["manifest"].read_text(encoding="utf-8"))
            self.assertEqual(
                manifest["tiles"][0]["zones"],
                buffer.contributing_zones,
                "逐列 carver ownership 不得改变 manifest provenance 合同",
            )

            count_path.write_bytes(b"\xff" * len(full_count_bytes))
            spans_path.write_bytes(b"\x00" * len(full_spans_bytes))
            rewritten = regen_zone(
                plan,
                fields,
                "blood_valley",
                layer_whitelist=set(),
            )
            self.assertEqual(
                rewritten,
                [tile.tile_id],
                "incremental regen 应精确重写目标 witness tile",
            )
            self.assertEqual(
                count_path.read_bytes(),
                full_count_bytes,
                "incremental regen 的 spans_count 必须与 full export 字节一致",
            )
            self.assertEqual(
                spans_path.read_bytes(),
                full_spans_bytes,
                "incremental regen 的 spans 必须与 full export 字节一致",
            )


if __name__ == "__main__":
    unittest.main()
