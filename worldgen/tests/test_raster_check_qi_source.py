"""worldgen-v4 P4 §8.1 #8 — qi_density vs zone.spirit_qi 同源派生硬断言 pin.

Builds a minimal manifest + tile dir with a hand-crafted ``qi_density.bin`` and
declared zone ``spirit_qi``, then asserts ``validate_rasters``:

* **休眠**：manifest 不声明 ``qi_density_source == "qi_field"`` → 即便 qi_density
  与 spirit_qi 系统性漂移也放行（迁移前两份漂移阶段不误报）。
* **同源通过**：声明 source=qi_field 且 zone 内 qi_density 均值 ≈
  clamp01((spirit_qi+1)/2) → 通过。
* **漂移撞红**：声明 source=qi_field 但 qi_density 均值偏离派生期望 > tolerance →
  HARD ERROR（两份漂移复发）。
* **容差边界 off-by-one**：刚好在 tol 内通过 / 刚好超 tol 撞红。
* **无声明 spirit_qi 的 zone**：跳过不炸（容错）。
"""

from __future__ import annotations

import json
import struct
import tempfile
import unittest
from pathlib import Path

from scripts.terrain_gen.harness.raster_check import (
    QI_DENSITY_TOLERANCE,
    validate_rasters,
)

TILE_SIZE = 4
AREA = TILE_SIZE * TILE_SIZE


def _write_qi_raster(
    tmp: Path,
    *,
    qi_density_value: float,
    zone_spirit_qi: float,
    qi_density_source: str | None,
    zone_name: str = "spawn",
    declare_zone_param: bool = True,
) -> Path:
    """Write a minimal overworld raster set: one tile, uniform qi_density, one zone."""
    raster_dir = tmp / "rasters"
    tile_dir = raster_dir / "tile_0_0"
    tile_dir.mkdir(parents=True)

    qi_bytes = struct.pack(f"<{AREA}f", *([qi_density_value] * AREA))
    (tile_dir / "qi_density.bin").write_bytes(qi_bytes)

    zones_payload = []
    if declare_zone_param:
        zones_payload.append({"name": zone_name, "spirit_qi": zone_spirit_qi})

    manifest: dict = {
        "version": 2,
        "backend": "raster",
        "world_name": "t",
        "tile_size": TILE_SIZE,
        "tiles": [
            {
                "tile_x": 0,
                "tile_z": 0,
                "dir": "tile_0_0",
                "zones": [zone_name],
                "layers": ["qi_density"],
            }
        ],
        "pois": [],
        "zones": zones_payload,
        "semantic_layers": ["qi_density"],
        "collapsed_zones": [],
    }
    if qi_density_source is not None:
        manifest["qi_density_source"] = qi_density_source
    (raster_dir / "manifest.json").write_text(json.dumps(manifest))
    return raster_dir


def _expected_density(spirit_qi: float) -> float:
    return max(0.0, min(1.0, (spirit_qi + 1.0) / 2.0))


class RasterCheckQiSourceTest(unittest.TestCase):
    def test_dormant_without_marker_allows_drift(self) -> None:
        # 无 qi_density_source 标记：qi_density=0.18 与 spirit_qi=0.30
        # (期望 density 0.65) 系统性漂移 0.47，但规则休眠 → 通过。
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_qi_raster(
                Path(td),
                qi_density_value=0.18,
                zone_spirit_qi=0.30,
                qi_density_source=None,
            )
            ok, msg = validate_rasters(raster_dir)
            self.assertTrue(
                ok,
                f"rule must be dormant without qi_density_source marker: {msg}",
            )

    def test_same_source_consistent_passes(self) -> None:
        # 声明 source=qi_field 且 qi_density == clamp01((spirit_qi+1)/2) → 通过。
        sq = 0.30
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_qi_raster(
                Path(td),
                qi_density_value=_expected_density(sq),  # 0.65
                zone_spirit_qi=sq,
                qi_density_source="qi_field",
            )
            ok, msg = validate_rasters(raster_dir)
            self.assertTrue(ok, f"same-source consistent must pass: {msg}")

    def test_gross_drift_fails_hard(self) -> None:
        # 声明 source=qi_field 但 qi_density=0.18 偏离期望 0.65 (|Δ|=0.47 > tol) → 撞红。
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_qi_raster(
                Path(td),
                qi_density_value=0.18,
                zone_spirit_qi=0.30,
                qi_density_source="qi_field",
            )
            ok, msg = validate_rasters(raster_dir)
            self.assertFalse(
                ok, "gross drift under qi_field source must be a HARD ERROR"
            )
            self.assertIn("diverges from zone", msg)
            self.assertIn("same-source", msg)

    def test_tolerance_boundary_within_passes(self) -> None:
        # 偏离恰好 = tolerance（边界内含）→ 通过。
        sq = 0.0
        expected = _expected_density(sq)  # 0.5
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_qi_raster(
                Path(td),
                qi_density_value=expected + QI_DENSITY_TOLERANCE,  # 偏离 = tol
                zone_spirit_qi=sq,
                qi_density_source="qi_field",
            )
            ok, msg = validate_rasters(raster_dir)
            self.assertTrue(
                ok, f"deviation exactly == tolerance must pass (inclusive): {msg}"
            )

    def test_tolerance_boundary_just_over_fails(self) -> None:
        # 偏离刚超 tolerance → 撞红（off-by-one 精确锁定）。
        sq = 0.0
        expected = _expected_density(sq)  # 0.5
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_qi_raster(
                Path(td),
                qi_density_value=expected + QI_DENSITY_TOLERANCE + 0.01,
                zone_spirit_qi=sq,
                qi_density_source="qi_field",
            )
            ok, msg = validate_rasters(raster_dir)
            self.assertFalse(
                ok, "deviation just over tolerance must fail"
            )

    def test_zone_without_declared_spirit_qi_is_skipped(self) -> None:
        # zone 名在 tile 但 manifest.zones 没有它的 spirit_qi → 跳过不炸。
        with tempfile.TemporaryDirectory() as td:
            raster_dir = _write_qi_raster(
                Path(td),
                qi_density_value=0.18,
                zone_spirit_qi=0.30,
                qi_density_source="qi_field",
                declare_zone_param=False,  # 不导出 zone param
            )
            ok, msg = validate_rasters(raster_dir)
            self.assertTrue(
                ok,
                f"zone without declared spirit_qi must be skipped, not crash: {msg}",
            )

    def test_three_state_zones_each_pass_when_same_source(self) -> None:
        # 富集 / 死域 / 负灵三态 zone 各自同源一致时通过（三态各一 case）。
        for sq in (0.85, -0.04, -0.55):
            with tempfile.TemporaryDirectory() as td:
                raster_dir = _write_qi_raster(
                    Path(td),
                    qi_density_value=_expected_density(sq),
                    zone_spirit_qi=sq,
                    qi_density_source="qi_field",
                )
                ok, msg = validate_rasters(raster_dir)
                self.assertTrue(
                    ok,
                    f"three-state zone spirit_qi={sq} same-source must pass: {msg}",
                )


if __name__ == "__main__":  # pragma: no cover
    unittest.main()
