"""worldgen-v4 P2 — DSL 核心 + 算子库饱和单测.

锁三件事（接口先于实现，P2 本段只锁 schema/算子，不迁移 profile）：

1. **qi_grade 六档 + 边界**（§8.1 #4）—— 每档一条专属 case，每个 [lo, hi) 边界
   off-by-one（lo 命中本档 / lo-ε 命中下档）、override 值域正反校验。
2. **DSL schema 正反 pin** —— 正样本解析通过且字段对齐，负样本（非法 grade /
   override 越界 / 坏 op category / 缺必填键）被拒；正反样本从
   ``fixtures/dsl_schema_samples.json`` 双端对拍。
3. **算子库饱和** —— 每类 op（height / mask / carve / scatter）的 happy path +
   边界（empty / 单元素 / 阈值临界）+ 错误分支（长度不匹配 / 非法周期 / 未知 op /
   未注册 custom），覆盖到"想不出还能加什么 case"。
"""

from __future__ import annotations

import json
import math
import unittest
from pathlib import Path

import numpy as np

from scripts.terrain_gen import dsl
from scripts.terrain_gen.noise import _tile_coords

REPO_ROOT = Path(__file__).resolve().parents[2]
SAMPLES_PATH = REPO_ROOT / "worldgen/fixtures/dsl_schema_samples.json"


def _centered_grid(size: int = 200):
    """A tile centred on (0,0) so radial / band math lands real samples."""
    return _tile_coords(-size // 2, -size // 2, size)


# ===========================================================================
# qi_grade 六档 + 边界
# ===========================================================================


class QiGradeBandTest(unittest.TestCase):
    def test_every_grade_has_a_dedicated_value(self) -> None:
        # 每档一条专属 case：档位中点必落本档。
        midpoints = {
            dsl.QiGrade.NEGATIVE: -0.55,
            dsl.QiGrade.DEAD: -0.04,
            dsl.QiGrade.TRACE: 0.08,
            dsl.QiGrade.COMMON: 0.30,
            dsl.QiGrade.RICH: 0.55,
            dsl.QiGrade.FONT: 0.85,
        }
        for grade, value in midpoints.items():
            self.assertEqual(
                dsl.qi_grade_from_value(value),
                grade,
                f"native qi {value} should classify as {grade.value} "
                f"(its band midpoint); got {dsl.qi_grade_from_value(value).value}",
            )

    def test_bounds_cover_full_native_range_without_gaps(self) -> None:
        # 六档边界必须无缝拼满 [-1, 1]：相邻档 hi == 下一档 lo。
        order = (
            dsl.QiGrade.NEGATIVE,
            dsl.QiGrade.DEAD,
            dsl.QiGrade.TRACE,
            dsl.QiGrade.COMMON,
            dsl.QiGrade.RICH,
            dsl.QiGrade.FONT,
        )
        self.assertEqual(
            dsl.QI_GRADE_BOUNDS[order[0]][0],
            dsl.QI_FIELD_MIN,
            "lowest band must start at the native field minimum -1.0",
        )
        self.assertEqual(
            dsl.QI_GRADE_BOUNDS[order[-1]][1],
            dsl.QI_FIELD_MAX,
            "highest band must end at the native field maximum 1.0",
        )
        for lower, upper in zip(order, order[1:]):
            self.assertEqual(
                dsl.QI_GRADE_BOUNDS[lower][1],
                dsl.QI_GRADE_BOUNDS[upper][0],
                f"band {lower.value}.hi must equal {upper.value}.lo (no gap/overlap "
                "in the qi grade ladder)",
            )

    def test_boundary_off_by_one_lo_inclusive_hi_exclusive(self) -> None:
        # 每个 [lo, hi)：lo 命中本档，lo-ε 命中下档（半开区间精确锁定）。
        cases = [
            (-0.1, dsl.QiGrade.DEAD, dsl.QiGrade.NEGATIVE),   # DEAD lo
            (0.02, dsl.QiGrade.TRACE, dsl.QiGrade.DEAD),      # TRACE lo
            (0.15, dsl.QiGrade.COMMON, dsl.QiGrade.TRACE),    # COMMON lo
            (0.45, dsl.QiGrade.RICH, dsl.QiGrade.COMMON),     # RICH lo
            (0.7, dsl.QiGrade.FONT, dsl.QiGrade.RICH),        # FONT lo
        ]
        for boundary, at_boundary, just_below in cases:
            self.assertEqual(
                dsl.qi_grade_from_value(boundary),
                at_boundary,
                f"native qi == {boundary} is the inclusive lo of {at_boundary.value}",
            )
            self.assertEqual(
                dsl.qi_grade_from_value(boundary - 1e-6),
                just_below,
                f"native qi just below {boundary} ({boundary - 1e-6}) falls into "
                f"{just_below.value} (hi is exclusive)",
            )

    def test_extremes_and_out_of_range_clamp_to_end_bands(self) -> None:
        self.assertEqual(dsl.qi_grade_from_value(-1.0), dsl.QiGrade.NEGATIVE)
        self.assertEqual(dsl.qi_grade_from_value(1.0), dsl.QiGrade.FONT)
        # Out-of-range native values clamp to the end bands (derived path clamps,
        # but a raw call must not raise).
        self.assertEqual(dsl.qi_grade_from_value(-5.0), dsl.QiGrade.NEGATIVE)
        self.assertEqual(dsl.qi_grade_from_value(5.0), dsl.QiGrade.FONT)

    def test_parse_qi_grade_accepts_all_six_rejects_unknown(self) -> None:
        for grade in dsl.QiGrade:
            self.assertEqual(dsl.parse_qi_grade(grade.value), grade)
        with self.assertRaises(ValueError) as ctx:
            dsl.parse_qi_grade("ultra")
        self.assertIn("ultra", str(ctx.exception))
        self.assertIn("negative", str(ctx.exception))

    def test_validate_qi_override_range(self) -> None:
        self.assertIsNone(dsl.validate_qi_override(None))
        self.assertEqual(dsl.validate_qi_override(-1.0), -1.0)
        self.assertEqual(dsl.validate_qi_override(1.0), 1.0)
        self.assertEqual(dsl.validate_qi_override(0.0), 0.0)
        for bad in (1.0001, -1.0001, 2.0, -5.0):
            with self.assertRaises(ValueError, msg=f"{bad} must be rejected"):
                dsl.validate_qi_override(bad)


# ===========================================================================
# DSL schema 正反 pin（fixtures 双端对拍）
# ===========================================================================


class DslSchemaSampleTest(unittest.TestCase):
    def setUp(self) -> None:
        self.samples = json.loads(SAMPLES_PATH.read_text(encoding="utf-8"))

    def _clean(self, raw: dict) -> dict:
        return {k: v for k, v in raw.items() if not k.startswith("_")}

    def test_has_both_valid_and_invalid_samples(self) -> None:
        self.assertGreater(len(self.samples["valid"]), 0)
        self.assertGreater(len(self.samples["invalid"]), 0)

    def test_all_valid_samples_parse(self) -> None:
        # custom op referenced by a valid sample must be registered first.
        ref = dsl.register_custom_op(
            "fossil_uplift",
            lambda wx, wz, bbox=None: np.zeros_like(wx),
            replace=True,
        )
        self.addCleanup(dsl.unregister_custom_op, "fossil_uplift")
        self.assertEqual(ref, "custom:fossil_uplift")
        for sample in self.samples["valid"]:
            name = sample.get("_name", "?")
            spec = dsl.parse_zone_terrain_dsl(self._clean(sample))
            self.assertIsInstance(
                spec.qi_grade,
                dsl.QiGrade,
                f"valid sample {name!r} must parse qi_grade into the enum",
            )
            dsl.validate_qi_override(spec.qi_override)

    def test_all_invalid_samples_rejected(self) -> None:
        for sample in self.samples["invalid"]:
            name = sample.get("_name", "?")
            with self.assertRaises(
                (ValueError, KeyError),
                msg=f"invalid sample {name!r} ({sample.get('_reason')}) must be "
                "rejected by the parser",
            ):
                dsl.parse_zone_terrain_dsl(self._clean(sample))

    def test_qi_grade_six_bands_each_have_a_valid_sample_coverage(self) -> None:
        # 每个 grade 至少被一个正样本覆盖（确保六档都 round-trips 过解析）。
        graded = {
            dsl.parse_zone_terrain_dsl(self._clean(s)).qi_grade
            for s in self.samples["valid"]
            if "custom" not in s.get("_name", "")
        }
        missing = set(dsl.QiGrade) - graded
        self.assertEqual(
            missing,
            set(),
            f"valid samples must cover all six qi grades; missing "
            f"{[g.value for g in missing]}",
        )


class DslSpecConstructionTest(unittest.TestCase):
    def test_zone_dsl_defaults_are_safe(self) -> None:
        spec = dsl.ZoneTerrainDSL(terrain_style=dsl.TerrainStyleSpec())
        self.assertEqual(spec.qi_grade, dsl.QiGrade.COMMON)
        self.assertIsNone(spec.qi_override)
        self.assertEqual(spec.terrain_style.height_ops, ())
        self.assertEqual(spec.surface_palette.rules, ())
        self.assertEqual(spec.flora_table.variants, ())

    def test_zone_dsl_rejects_out_of_range_override_at_construction(self) -> None:
        with self.assertRaises(ValueError):
            dsl.ZoneTerrainDSL(
                terrain_style=dsl.TerrainStyleSpec(), qi_override=1.5
            )

    def test_opstep_rejects_unknown_category(self) -> None:
        with self.assertRaises(ValueError):
            dsl.OpStep(category="bogus", op="fbm_height")

    def test_opstep_rejects_unknown_op_in_valid_category_at_construction(
        self,
    ) -> None:
        # A real category with a non-existent op name must fail at construction /
        # parse time (not silently survive until eval() raises KeyError). The
        # error must name a real candidate to aid debugging.
        with self.assertRaises(ValueError) as ctx:
            dsl.OpStep(category="height", op="nonexistent_height")
        self.assertIn("fbm_height", str(ctx.exception))

    def test_opstep_accepts_valid_op_in_each_category(self) -> None:
        # Sanity: a real op in its real category constructs cleanly (guards the
        # new parse-time check against false positives across all categories).
        for category, op in (
            ("height", "fbm_height"),
            ("mask", "radial_mask"),
            ("carve", "rift_carve"),
            ("surface", "snowline_split"),
            ("scatter", "anomaly_compete"),
        ):
            step = dsl.OpStep(category=category, op=op)
            self.assertEqual((step.category, step.op), (category, op))

    def test_opstep_allows_custom_op_without_category_check(self) -> None:
        # custom: op bypasses category validation (escape hatch).
        step = dsl.OpStep(category="ignored", op="custom:anything")
        self.assertEqual(step.op, "custom:anything")

    def test_parse_empty_terrain_style_is_empty_spec(self) -> None:
        self.assertEqual(dsl.parse_terrain_style(None), dsl.TerrainStyleSpec())
        self.assertEqual(dsl.parse_terrain_style({}), dsl.TerrainStyleSpec())


# ===========================================================================
# 算子注册 + escape hatch
# ===========================================================================


class OpRegistryTest(unittest.TestCase):
    def test_resolve_builtin_ops_per_category(self) -> None:
        self.assertIs(dsl.resolve_op("height", "fbm_height"), dsl.fbm_height)
        self.assertIs(dsl.resolve_op("mask", "radial_mask"), dsl.radial_mask)
        self.assertIs(dsl.resolve_op("carve", "rift_carve"), dsl.rift_carve)
        self.assertIs(
            dsl.resolve_op("scatter", "anomaly_compete"), dsl.anomaly_compete
        )

    def test_resolve_unknown_category_errors(self) -> None:
        with self.assertRaises(KeyError) as ctx:
            dsl.resolve_op("bogus", "fbm_height")
        self.assertIn("bogus", str(ctx.exception))

    def test_resolve_unknown_op_errors_with_candidates(self) -> None:
        with self.assertRaises(KeyError) as ctx:
            dsl.resolve_op("height", "nope")
        # error must list real candidates to aid debugging.
        self.assertIn("fbm_height", str(ctx.exception))

    def test_register_and_resolve_custom_op(self) -> None:
        ref = dsl.register_custom_op(
            "myop", lambda wx, wz: np.full_like(wx, 3.0)
        )
        self.addCleanup(dsl.unregister_custom_op, "myop")
        self.assertEqual(ref, "custom:myop")
        fn = dsl.resolve_op("height", ref)  # category ignored for custom
        wx, wz = _centered_grid(8)
        self.assertTrue(np.allclose(fn(wx, wz), 3.0))

    def test_register_custom_op_duplicate_without_replace_errors(self) -> None:
        dsl.register_custom_op("dup", lambda wx, wz: wx)
        self.addCleanup(dsl.unregister_custom_op, "dup")
        with self.assertRaises(ValueError):
            dsl.register_custom_op("dup", lambda wx, wz: wz)
        # replace=True overrides cleanly.
        dsl.register_custom_op("dup", lambda wx, wz: wz, replace=True)

    def test_register_custom_op_rejects_bad_name_and_noncallable(self) -> None:
        with self.assertRaises(ValueError):
            dsl.register_custom_op("bad:name", lambda wx, wz: wx)
        with self.assertRaises(ValueError):
            dsl.register_custom_op("", lambda wx, wz: wx)
        with self.assertRaises(ValueError):
            dsl.register_custom_op("notcallable", 42)  # type: ignore[arg-type]

    def test_resolve_unregistered_custom_op_errors(self) -> None:
        with self.assertRaises(KeyError):
            dsl.resolve_op("height", "custom:never_registered")

    def test_list_ops_per_category_and_all(self) -> None:
        height_ops = dsl.list_ops("height")
        self.assertIn("fbm_height", height_ops)
        self.assertIn("radial_uplift", height_ops)
        with self.assertRaises(KeyError):
            dsl.list_ops("bogus")


# ===========================================================================
# height ops 饱和
# ===========================================================================


class HeightOpsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.wx, self.wz = _centered_grid(200)

    def test_fbm_height_base_offset_and_amplitude(self) -> None:
        flat = dsl.fbm_height(self.wx, self.wz, amplitude=0.0, base=70.0)
        self.assertTrue(
            np.allclose(flat, 70.0),
            "amplitude=0 must collapse fbm_height to the constant base",
        )
        bumpy = dsl.fbm_height(
            self.wx, self.wz, base=70.0, amplitude=4.0, scale=120.0, seed=10
        )
        self.assertGreater(bumpy.max(), 70.0)
        self.assertLess(bumpy.min(), 70.0)

    def test_radial_uplift_peaks_at_center_and_zero_outside(self) -> None:
        up = dsl.radial_uplift(
            self.wx,
            self.wz,
            center_xz=(0, 0),
            half_w=100,
            half_d=100,
            exponent=1.9,
            amplitude=3.8,
        )
        # center column (≈0,0) ~= full amplitude; far corner clamps to 0.
        self.assertAlmostEqual(up.max(), 3.8, places=2)
        self.assertEqual(up.min(), 0.0, "uplift must clamp to 0 outside the radius")

    def test_ridge_height_amplitude_scales(self) -> None:
        small = dsl.ridge_height(self.wx, self.wz, amplitude=1.0, seed=5)
        big = dsl.ridge_height(self.wx, self.wz, amplitude=4.0, seed=5)
        self.assertTrue(np.allclose(big, small * 4.0))

    def test_warped_height_runs_and_is_finite(self) -> None:
        h = dsl.warped_height(
            self.wx, self.wz, base=64.0, amplitude=3.0, scale=160.0, seed=700
        )
        self.assertTrue(np.all(np.isfinite(h)))

    def test_axial_profile_peaks_on_axis_center(self) -> None:
        ap = dsl.axial_profile(
            self.wx,
            self.wz,
            center_xz=(0, 0),
            angle_deg=0.0,
            half_length=50.0,
            amplitude=1.0,
        )
        self.assertAlmostEqual(ap.max(), 1.0, places=2)
        self.assertEqual(ap.min(), 0.0)

    def test_compose_height_sums_terms(self) -> None:
        a = np.full((4, 4), 10.0)
        b = np.full((4, 4), 5.0)
        c = np.full((4, 4), 2.0)
        out = dsl.compose_height(a, b, -c)
        self.assertTrue(np.allclose(out, 13.0))

    def test_compose_height_single_term(self) -> None:
        a = np.full((3, 3), 7.0)
        self.assertTrue(np.allclose(dsl.compose_height(a), 7.0))

    def test_compose_height_empty_errors(self) -> None:
        with self.assertRaises(ValueError):
            dsl.compose_height()


# ===========================================================================
# mask ops 饱和
# ===========================================================================


class MaskOpsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.wx, self.wz = _centered_grid(200)

    def test_radial_mask_one_at_center_zero_at_edge(self) -> None:
        m = dsl.radial_mask(
            self.wx, self.wz, center_xz=(0, 0), half_w=100, half_d=100, exponent=2.0
        )
        self.assertAlmostEqual(m.max(), 1.0, places=2)
        self.assertEqual(m.min(), 0.0)

    def test_concentric_bands_assigns_per_ring(self) -> None:
        bands = dsl.concentric_bands(
            self.wx,
            self.wz,
            center_xz=(0, 0),
            thresholds=[20.0, 60.0],
            values=[0.0, 0.02, 0.05],
        )
        present = set(np.round(bands, 3).ravel().tolist())
        self.assertEqual(
            present,
            {0.0, 0.02, 0.05},
            "all three concentric ring values must appear in a 200-wide tile",
        )

    def test_concentric_bands_length_mismatch_errors(self) -> None:
        with self.assertRaises(ValueError):
            dsl.concentric_bands(
                self.wx,
                self.wz,
                center_xz=(0, 0),
                thresholds=[20.0, 60.0],
                values=[0.0, 0.05],  # needs 3
            )

    def test_concentric_bands_non_ascending_thresholds_errors(self) -> None:
        with self.assertRaises(ValueError):
            dsl.concentric_bands(
                self.wx,
                self.wz,
                center_xz=(0, 0),
                thresholds=[60.0, 20.0],  # descending
                values=[0.0, 0.02, 0.05],
            )

    def test_axial_sdf_zero_on_axis_grows_off_axis(self) -> None:
        sdf = dsl.axial_sdf(
            self.wx, self.wz, center_xz=(0, 0), angle_deg=0.0, half_width=30.0
        )
        # on the x-axis (z==0) the cross distance is 0.
        self.assertEqual(sdf.min(), 0.0)
        self.assertGreater(sdf.max(), 1.0)

    def test_anchor_sdf_is_euclidean_distance(self) -> None:
        sdf = dsl.anchor_sdf(self.wx, self.wz, anchor_xz=(0.0, 0.0))
        self.assertAlmostEqual(sdf.min(), 0.0, places=3)
        # distance to a far corner ~= sqrt(2)*100.
        self.assertAlmostEqual(sdf.max(), math.hypot(100, 100), delta=2.0)

    def test_modulo_grid_scatter_is_deterministic_sparse(self) -> None:
        m = dsl.modulo_grid_scatter(
            self.wx, self.wz, period_x=8, period_z=8, cell_match=0
        )
        # roughly 1/64 of cells fire (1/8 in x AND 1/8 in z).
        self.assertAlmostEqual(m.mean(), 1.0 / 64.0, delta=0.01)
        # re-running gives identical placement (no RNG).
        m2 = dsl.modulo_grid_scatter(
            self.wx, self.wz, period_x=8, period_z=8, cell_match=0
        )
        self.assertTrue(np.array_equal(m, m2))

    def test_modulo_grid_scatter_bad_period_errors(self) -> None:
        with self.assertRaises(ValueError):
            dsl.modulo_grid_scatter(self.wx, self.wz, period_x=0, period_z=8)
        with self.assertRaises(ValueError):
            dsl.modulo_grid_scatter(self.wx, self.wz, period_x=8, period_z=-1)


# ===========================================================================
# carve ops 饱和（与 spans_fold 公式同源）
# ===========================================================================


class CarveOpsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.wx, self.wz = _centered_grid(200)

    def test_rift_carve_drops_surface_on_axis(self) -> None:
        h = np.full_like(self.wx, 80.0)
        sdf = dsl.axial_sdf(
            self.wx, self.wz, center_xz=(0, 0), angle_deg=0.0, half_width=30.0
        )
        rim = np.zeros_like(self.wx)
        carved = dsl.rift_carve(h, rift_axis_sdf=sdf, rim_edge_mask=rim)
        self.assertLess(
            carved.min(), 80.0, "rift carve must drop the surface near the axis"
        )
        # off-axis (sdf >= 0.9) columns are untouched.
        untouched = np.isclose(carved, 80.0)
        self.assertTrue(untouched.any(), "far-from-axis columns stay at 80")

    def test_rift_carve_formula_matches_fold_anchor(self) -> None:
        # Mirror the spans_fold literal anchor: sdf=0.5, rim=0.25 →
        # carve = (1-0.5)*22 + 0.25*4 = 11 + 1 = 12 → 80 - 12 = 68.
        h = np.array([[80.0]])
        carved = dsl.rift_carve(
            h,
            rift_axis_sdf=np.array([[0.5]]),
            rim_edge_mask=np.array([[0.25]]),
        )
        self.assertAlmostEqual(float(carved[0, 0]), 68.0, places=5)

    def test_fracture_carve_floors_at_bedrock_plus_6(self) -> None:
        h = np.full_like(self.wx, 30.0)
        fm = np.full_like(self.wx, 1.0)  # crack = (1-0.7)*300 = 90 → 30-90 floored.
        carved = dsl.fracture_carve(h, fracture_mask=fm)
        self.assertAlmostEqual(carved.min(), -58.0, places=5)

    def test_fracture_carve_below_threshold_is_noop(self) -> None:
        h = np.full_like(self.wx, 100.0)
        fm = np.full_like(self.wx, 0.5)  # < 0.7 → no carve.
        carved = dsl.fracture_carve(h, fracture_mask=fm)
        self.assertTrue(np.allclose(carved, 100.0))

    def test_neg_pressure_carve_sinks_above_threshold(self) -> None:
        h = np.full_like(self.wx, 90.0)
        neg = np.full_like(self.wx, 0.5)  # > 0.18 → sink 0.5*14 = 7 → 83.
        carved = dsl.neg_pressure_carve(h, neg_pressure=neg)
        self.assertAlmostEqual(carved.max(), 83.0, places=5)

    def test_neg_pressure_carve_below_threshold_noop(self) -> None:
        h = np.full_like(self.wx, 90.0)
        neg = np.full_like(self.wx, 0.1)  # < 0.18 → no sink.
        carved = dsl.neg_pressure_carve(h, neg_pressure=neg)
        self.assertTrue(np.allclose(carved, 90.0))

    def test_carve_chain_rejects_surface_op(self) -> None:
        # A surface op (snowline_split) returns block IDs, not a height field; if
        # it ever ran in a carve chain it would replace the height field with the
        # block-ID constants and destroy the geometry. evaluate_carve_ops must
        # reject any non-carve step at eval time, not silently corrupt height.
        h = np.full_like(self.wx, 80.0)
        with self.assertRaises(ValueError) as ctx:
            dsl.evaluate_carve_ops(
                [
                    dsl.OpStep(
                        "surface",
                        "snowline_split",
                        {"snowline_y": 285.0, "below_value": 1.0, "above_value": 2.0},
                    )
                ],
                h,
                fields={},
            )
        self.assertIn("carve", str(ctx.exception))


# ===========================================================================
# surface ops 饱和（按高度阈值出块 ID —— 不改高度场）
# ===========================================================================


class SurfaceOpsTest(unittest.TestCase):
    def test_snowline_split_picks_block_id_per_height(self) -> None:
        # snowline_split is a SURFACE selector: its first positional arg is the
        # height field used as the THRESHOLD criterion; it returns block-ID
        # constants (above/below), never a modified height. Mirrors broken_peaks
        # `surface_id = np.where(height > SNOW_LINE_Y, snow_id, surface_id)`.
        h = np.array([[100.0, 300.0]])
        out = dsl.snowline_split(
            h,
            snowline_y=285.0,
            below_value=1.0,
            above_value=2.0,
        )
        self.assertEqual(out[0, 0], 1.0, "below snowline → below_value block id")
        self.assertEqual(out[0, 1], 2.0, "above snowline → above_value block id")

    def test_snowline_split_threshold_is_inclusive_at_snowline(self) -> None:
        # height == snowline_y must take the ABOVE value (>= boundary), so the
        # snowline pixel itself is snow, not the below block.
        h = np.array([[284.0, 285.0, 286.0]])
        out = dsl.snowline_split(
            h, snowline_y=285.0, below_value=10.0, above_value=20.0
        )
        self.assertEqual(out[0, 0], 10.0, "strictly below snowline → below_value")
        self.assertEqual(
            out[0, 1], 20.0, "exactly at snowline (>=) → above_value (inclusive)"
        )
        self.assertEqual(out[0, 2], 20.0, "above snowline → above_value")

    def test_snowline_split_does_not_modify_height(self) -> None:
        # The output is a block-ID field, distinct from the input height — proves
        # it is a selector, not a height modifier (the latent-bug guard).
        h = np.full((4, 4), 90.0)
        out = dsl.snowline_split(
            h, snowline_y=285.0, below_value=3.0, above_value=7.0
        )
        self.assertTrue(
            np.all(out == 3.0),
            "all columns below snowline → uniform below block id, not the height",
        )
        self.assertFalse(
            np.array_equal(out, h),
            "snowline_split must NOT echo the height field; it returns block ids",
        )

    def test_snowline_split_registered_under_surface_not_carve(self) -> None:
        # Pin the reclassification: snowline_split is a surface op, never a carve.
        self.assertIs(
            dsl.resolve_op("surface", "snowline_split"), dsl.snowline_split
        )
        with self.assertRaises(KeyError):
            dsl.resolve_op("carve", "snowline_split")


# ===========================================================================
# scatter ops 饱和
# ===========================================================================


class ScatterOpsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.wx, self.wz = _centered_grid(64)

    def test_flora_variant_by_noise_dual_threshold(self) -> None:
        band = np.array([[1.0, 1.0, 0.0]])
        pick = np.array([[1.0, 0.0, 1.0]])
        out = dsl.flora_variant_by_noise(
            band_field=band,
            pick_field=pick,
            band_threshold=0.5,
            pick_threshold=0.5,
            variant_id=7,
        )
        # only the column passing BOTH band and pick gets the variant.
        self.assertEqual(out[0, 0], 7)
        self.assertEqual(out[0, 1], 0)
        self.assertEqual(out[0, 2], 0)

    def test_flora_variant_by_noise_threshold_is_strictly_greater(self) -> None:
        # band/pick use strict `>` against the threshold: a field value EXACTLY
        # equal to the threshold (0.5 > 0.5 == False) must NOT place the variant.
        # This off-by-one pins the boundary so a `>=` regression撞红.
        band = np.array([[0.5, 0.5, 0.6, 0.6]])
        pick = np.array([[0.6, 0.5, 0.5, 0.6]])
        out = dsl.flora_variant_by_noise(
            band_field=band,
            pick_field=pick,
            band_threshold=0.5,
            pick_threshold=0.5,
            variant_id=9,
        )
        self.assertEqual(out[0, 0], 0, "band==threshold (0.5>0.5=False) → not placed")
        self.assertEqual(out[0, 1], 0, "both ==threshold → not placed")
        self.assertEqual(out[0, 2], 0, "pick==threshold → not placed")
        self.assertEqual(
            out[0, 3], 9, "both strictly above threshold → placed"
        )

    def test_anomaly_compete_tie_breaks_to_first_field(self) -> None:
        # When two anomaly fields are exactly equal at a column, np.argmax breaks
        # the tie to the LOWEST index — so the FIRST listed field's kind_id wins.
        # Pin this so a reordering / argmax-flavor change is caught.
        f1 = np.full((2, 2), 0.6)
        f2 = np.full((2, 2), 0.6)  # identical to f1 → tie.
        f3 = np.full((2, 2), 0.3)
        intensity, kind = dsl.anomaly_compete(
            fields=[f1, f2, f3], kind_ids=[11, 22, 33], threshold=0.0
        )
        self.assertTrue(np.allclose(intensity, 0.6), "tie intensity = the shared max")
        self.assertTrue(
            np.all(kind == 11),
            "tie between f1 and f2 → argmax index 0 → first field's kind_id (11)",
        )

    def test_anomaly_compete_argmax_and_threshold(self) -> None:
        f1 = np.full((2, 2), 0.2)
        f2 = np.full((2, 2), 0.8)
        f3 = np.full((2, 2), 0.5)
        intensity, kind = dsl.anomaly_compete(
            fields=[f1, f2, f3], kind_ids=[1, 2, 3], threshold=0.0
        )
        self.assertTrue(np.allclose(intensity, 0.8))
        self.assertTrue(np.all(kind == 2), "strongest field (0.8) wins → kind 2")

    def test_anomaly_compete_below_threshold_yields_kind_zero(self) -> None:
        f1 = np.full((2, 2), 0.1)
        f2 = np.full((2, 2), 0.05)
        intensity, kind = dsl.anomaly_compete(
            fields=[f1, f2], kind_ids=[1, 2], threshold=0.2
        )
        self.assertTrue(np.all(kind == 0), "max intensity < threshold → no anomaly")
        self.assertTrue(np.allclose(intensity, 0.1))

    def test_anomaly_compete_length_mismatch_errors(self) -> None:
        with self.assertRaises(ValueError):
            dsl.anomaly_compete(
                fields=[np.zeros((2, 2))], kind_ids=[1, 2], threshold=0.0
            )

    def test_anomaly_compete_empty_errors(self) -> None:
        with self.assertRaises(ValueError):
            dsl.anomaly_compete(fields=[], kind_ids=[], threshold=0.0)


# ===========================================================================
# TerrainStyle 解释器（end-to-end height op 链 + carve op 链）
# ===========================================================================


class TerrainStyleInterpreterTest(unittest.TestCase):
    def setUp(self) -> None:
        self.wx, self.wz = _centered_grid(200)
        self.ctx = {"center_xz": (0, 0), "half_w": 100.0, "half_d": 100.0}

    def test_evaluate_height_ops_composes_chain(self) -> None:
        style = dsl.TerrainStyleSpec(
            height_ops=(
                dsl.OpStep(
                    "height",
                    "fbm_height",
                    {"base": 70.0, "amplitude": 0.0},  # constant 70
                ),
                dsl.OpStep(
                    "height",
                    "radial_uplift",
                    {"exponent": 1.9, "amplitude": 4.0},
                ),
            )
        )
        h = dsl.evaluate_height_ops(
            style.height_ops, self.wx, self.wz, context=self.ctx
        )
        # base 70 everywhere + uplift 0..4 → center ≈ 74, edge ≈ 70.
        self.assertAlmostEqual(h.max(), 74.0, delta=0.1)
        self.assertAlmostEqual(h.min(), 70.0, delta=0.1)

    def test_height_op_weight_control_scales_contribution(self) -> None:
        style = dsl.TerrainStyleSpec(
            height_ops=(
                dsl.OpStep(
                    "height",
                    "radial_uplift",
                    {"exponent": 1.9, "amplitude": 4.0, "$weight": 0.5},
                ),
            )
        )
        h = dsl.evaluate_height_ops(
            style.height_ops, self.wx, self.wz, context=self.ctx
        )
        # amplitude 4 * weight 0.5 → peak ≈ 2.
        self.assertAlmostEqual(h.max(), 2.0, delta=0.05)

    def test_empty_height_ops_yields_zeros(self) -> None:
        h = dsl.evaluate_height_ops((), self.wx, self.wz, context=self.ctx)
        self.assertTrue(np.allclose(h, 0.0))

    def test_evaluate_carve_ops_resolves_field_refs(self) -> None:
        sdf = dsl.axial_sdf(
            self.wx, self.wz, center_xz=(0, 0), angle_deg=0.0, half_width=30.0
        )
        rim = np.zeros_like(self.wx)
        h = np.full_like(self.wx, 80.0)
        carved = dsl.evaluate_carve_ops(
            (
                dsl.OpStep(
                    "carve",
                    "rift_carve",
                    {
                        "rift_axis_sdf": "field:rift_axis_sdf",
                        "rim_edge_mask": "field:rim",
                    },
                ),
            ),
            h,
            fields={"rift_axis_sdf": sdf, "rim": rim},
        )
        self.assertLess(carved.min(), 80.0)

    def test_carve_op_unknown_field_ref_errors(self) -> None:
        with self.assertRaises(KeyError):
            dsl.evaluate_carve_ops(
                (
                    dsl.OpStep(
                        "carve",
                        "rift_carve",
                        {
                            "rift_axis_sdf": "field:missing",
                            "rim_edge_mask": "field:rim",
                        },
                    ),
                ),
                np.full_like(self.wx, 80.0),
                fields={"rim": np.zeros_like(self.wx)},
            )

    def test_evaluate_terrain_style_end_to_end(self) -> None:
        sdf = dsl.axial_sdf(
            self.wx, self.wz, center_xz=(0, 0), angle_deg=0.0, half_width=30.0
        )
        rim = np.zeros_like(self.wx)
        style = dsl.TerrainStyleSpec(
            height_ops=(
                dsl.OpStep("height", "fbm_height", {"base": 80.0, "amplitude": 0.0}),
            ),
            carve_ops=(
                dsl.OpStep(
                    "carve",
                    "rift_carve",
                    {
                        "rift_axis_sdf": "field:rift_axis_sdf",
                        "rim_edge_mask": "field:rim",
                    },
                ),
            ),
        )
        h = dsl.evaluate_terrain_style(
            style,
            self.wx,
            self.wz,
            context=self.ctx,
            fields={"rift_axis_sdf": sdf, "rim": rim},
        )
        self.assertLess(h.min(), 80.0, "carve chain must drop the rift axis")
        self.assertAlmostEqual(h.max(), 80.0, delta=0.01, msg="off-axis stays 80")

    def test_custom_op_in_height_chain(self) -> None:
        dsl.register_custom_op(
            "const_bump", lambda wx, wz, bump=5.0: np.full_like(wx, bump)
        )
        self.addCleanup(dsl.unregister_custom_op, "const_bump")
        style = dsl.TerrainStyleSpec(
            height_ops=(
                dsl.OpStep("height", "custom:const_bump", {"bump": 12.0}),
            )
        )
        h = dsl.evaluate_height_ops(
            style.height_ops, self.wx, self.wz, context=self.ctx
        )
        self.assertTrue(np.allclose(h, 12.0))


if __name__ == "__main__":
    unittest.main()
