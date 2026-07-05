"""Invariant guard (fix-danzong): every compound LayoutSpec.radius must cover
the true, footprint-inclusive maximum horizontal distance from its POI
center to any placed block.

Why this exists
----------------
dan_zong_compound's flatten radius was pinned at 96 for a long time while
its central-axis placements (furnace row z=+128, bones z=+136, and above
all the corner-anchored 200x50x120 dan_zong_great_hall.nbt reaching local
x=194) silently grew past it. Nothing caught the drift because the only
pinned test compared LayoutSpec.radius against itself (96 == 96), never
against the actual placements. This module closes that gap by running the
*real* layout (real NBT files, real rotation, real block_grid centering)
and measuring the true max distance directly — for every registered
compound layout, not just dan_zong, so a future layout can't reintroduce
the same "radius forgot to grow with the content" class of bug.

Consequence when this invariant is violated: apply_compound_flatten /
compute_layout_density_mask (worldgen/scripts/terrain_gen/stitcher.py) only
touch tiles inside `radius` of the POI. Placements farther out sit on
whatever raw terrain height the surrounding wilderness field produced —
i.e. the building floats or clips into a hillside.
"""

from __future__ import annotations

import math
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.terrain_gen.blueprint import BlueprintZone, BoundarySpec, PoiSpec, ZoneWorldgenConfig
from scripts.terrain_gen.fields import Bounds2D
from scripts.terrain_gen.layouts.base import LayoutSpec
from scripts.terrain_gen.layouts.dan_zong_compound import DAN_ZONG_COMPOUND_LAYOUT
from scripts.terrain_gen.layouts.runner import run_layout
from scripts.terrain_gen.layouts.wangyintai_compound import WANGYINTAI_COMPOUND_LAYOUT

REPO_ROOT = Path(__file__).resolve().parents[2]
SERVER_DIR = REPO_ROOT / "server"


def _max_horizontal_distance(spec: LayoutSpec, zone: BlueprintZone, nbt_base_dir: str) -> float:
    """Run *spec* against *zone* and return the largest horizontal (XZ)
    distance from the POI center to any placed block, across every
    paste_results entry (nbt / block_grid / stamp_radial alike).

    This deliberately uses the real block-level output (not just placement
    offsets) so NBT footprints, block_grid centering, and rotation are all
    accounted for exactly as the runtime pipeline sees them.
    """
    result = run_layout(spec, zone, nbt_base_dir=nbt_base_dir)
    cx, _, cz = result.poi_world_pos
    max_dist = 0.0
    for paste_result in result.paste_results:
        for block in paste_result.blocks:
            dx = block.world_pos[0] - cx
            dz = block.world_pos[2] - cz
            dist = math.hypot(dx, dz)
            if dist > max_dist:
                max_dist = dist
    return max_dist


def _make_dan_zong_zone() -> BlueprintZone:
    return BlueprintZone(
        name="dan_zong_yi_yuan",
        display_name="丹宗遗园",
        bounds_xz=Bounds2D(min_x=-2400, max_x=-800, min_z=3200, max_z=4800),
        center_xz=(-1600, 4000),
        size_xz=(1600, 1600),
        spirit_qi=0.40,
        danger_level=4,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="dan_zong_yi_yuan",
            shape="ellipse",
            boundary=BoundarySpec(mode="soft", width=96),
            height_model={
                "base": [62, 78], "peak": 92,
                "compound_flatten_radius": DAN_ZONG_COMPOUND_LAYOUT.radius,
            },
            surface_palette=(
                "podzol", "coarse_dirt", "mud",
                "purple_terracotta", "mossy_cobblestone",
            ),
        ),
        pois=(
            PoiSpec(
                kind="ruin",
                pos_xyz=(-1600.0, 82.0, 4000.0),
                name="百草丹殿",
                tags=("dandao_path", "alchemy"),
                unlock="found_by_exploration",
                qi_affinity=-0.10,
                danger_bias=2,
            ),
        ),
    )


def _make_wangyintai_zone() -> BlueprintZone:
    return BlueprintZone(
        name="wangyintai",
        display_name="王印台",
        bounds_xz=Bounds2D(min_x=3500, max_x=4500, min_z=-2150, max_z=-1150),
        center_xz=(4000, -1650),
        size_xz=(1000, 1000),
        spirit_qi=-0.15,
        danger_level=3,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="wangyintai",
            shape="ellipse",
            boundary=BoundarySpec(mode="soft", width=64),
            height_model={
                "base": [68, 86], "peak": 98,
                "compound_flatten_radius": WANGYINTAI_COMPOUND_LAYOUT.radius,
            },
            surface_palette=("smooth_basalt", "deepslate", "calcite", "gray_concrete"),
        ),
        pois=(
            PoiSpec(
                kind="guantiantai",
                pos_xyz=(4000.0, 92.0, -1650.0),
                name="观天台",
                tags=("wangyintai", "vortex_formation"),
                unlock="found_by_exploration",
                qi_affinity=-0.15,
                danger_bias=1,
            ),
        ),
    )


class LayoutRadiusCoversFootprintTests(unittest.TestCase):
    """radius must be >= max placement distance (incl. NBT footprint) for
    every registered compound layout — not just dan_zong."""

    def test_dan_zong_radius_covers_all_placements(self):
        zone = _make_dan_zong_zone()
        nbt_base_dir = str(SERVER_DIR / "structures" / "dan_zong")
        max_dist = _max_horizontal_distance(DAN_ZONG_COMPOUND_LAYOUT, zone, nbt_base_dir)
        self.assertGreaterEqual(
            DAN_ZONG_COMPOUND_LAYOUT.radius, max_dist,
            f"dan_zong_compound radius={DAN_ZONG_COMPOUND_LAYOUT.radius} is smaller than "
            f"the true max placement distance {max_dist:.2f} (measured via run_layout with "
            f"real NBT footprints) -- flatten/density-mask will leave some placed blocks "
            f"outside the flattened circle, i.e. floating over raw terrain.",
        )

    def test_dan_zong_radius_has_safety_margin(self):
        """Regression guard for the specific bug this test module was written for:
        radius must not just barely cover the footprint (that reintroduces the
        'exactly on the boundary, one lucky Y-sample away from floating' failure
        mode) -- require at least an 8-block margin."""
        zone = _make_dan_zong_zone()
        nbt_base_dir = str(SERVER_DIR / "structures" / "dan_zong")
        max_dist = _max_horizontal_distance(DAN_ZONG_COMPOUND_LAYOUT, zone, nbt_base_dir)
        margin = DAN_ZONG_COMPOUND_LAYOUT.radius - max_dist
        self.assertGreaterEqual(
            margin, 8.0,
            f"dan_zong_compound radius={DAN_ZONG_COMPOUND_LAYOUT.radius} only clears the "
            f"true max placement distance {max_dist:.2f} by {margin:.2f} blocks; "
            f"plan fix-danzong requires >= 8 blocks of margin so future placement tweaks "
            f"don't immediately regress this invariant.",
        )

    def test_wangyintai_radius_covers_all_placements(self):
        zone = _make_wangyintai_zone()
        nbt_base_dir = str(SERVER_DIR / "structures" / "wangyintai")
        max_dist = _max_horizontal_distance(WANGYINTAI_COMPOUND_LAYOUT, zone, nbt_base_dir)
        self.assertGreaterEqual(
            WANGYINTAI_COMPOUND_LAYOUT.radius, max_dist,
            f"wangyintai_compound radius={WANGYINTAI_COMPOUND_LAYOUT.radius} is smaller than "
            f"the true max placement distance {max_dist:.2f} (measured via run_layout with "
            f"real NBT footprints) -- flatten/density-mask will leave some placed blocks "
            f"outside the flattened circle, i.e. floating over raw terrain.",
        )


if __name__ == "__main__":
    unittest.main()
