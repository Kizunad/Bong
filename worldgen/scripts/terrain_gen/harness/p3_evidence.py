"""worldgen-v4 P3 §6.1 — landscape carve evidence renderer.

Drives a real terrain profile through the full pipeline (build plan → synthesize
fields → fold spans → apply the profile's carver chain) and renders a vertical
cross-section PNG so the carve geometry can be eyeballed round by round:

  * rift_valley → does the canyon wall actually overhang (上岩架 + 空气带 +
    谷底), with layered rock bands?
  * sky_isle    → do the floating isles sit at staggered heights with a drippy
    underside, free of the ground?
  * cave_network→ do columns split into multiple stacked solid layers (multi-
    gallery), keeping the surface cap walkable?

This is the §6.1 三轮打磨 evidence tool — each refinement round calls
``render_profile_cross_section`` to drop a slice to ``worldgen/generated/
p3_preview/`` (gitignored) and the geometry is checked by eye + by the
heuristic stats it returns.

It deliberately reads the carved spans **straight off the on-disk .bin files**
(the exact bytes the Rust runtime mmaps), so the picture is the real export, not
an in-memory approximation.
"""

from __future__ import annotations

import tempfile
from dataclasses import dataclass
from pathlib import Path

from ..bakers.raster_export import (
    SPANS_COUNT_FILE,
    SPANS_FILE,
    build_raster_bake_plan,
    export_rasters,
)
from ..blueprint import (
    BlueprintZone,
    BoundarySpec,
    TerrainProfileCatalog,
    TerrainProfileSpec,
    WorldBlueprint,
    ZoneWorldgenConfig,
)
from ..carvers import build_carver
from ..fields import (
    Bounds2D,
    ColumnSpans,
    SurfacePalette,
    TileFieldBuffer,
    WorldTile,
    decode_spans_column,
)
from ..profiles import get_profile_generator
from ..profiles.cave_network import fill_cave_network_tile
from ..profiles.rift_valley import fill_rift_valley_tile
from ..profiles.sky_isle import fill_sky_isle_tile
from ..spans_fold import (
    column_spans_for_index,
    v3_surface_top_y,
)
from ..stitcher import build_generation_plan, synthesize_fields
from .cross_section import render_cross_section

TILE_SIZE = 200

# Direct fill dispatch for the fast (single-tile, no-stitch) evidence path.
_PROFILE_FILL = {
    "rift_valley": fill_rift_valley_tile,
    "sky_isle": fill_sky_isle_tile,
    "cave_network": fill_cave_network_tile,
}

# Mirror the export hook's fixed carve seed so the fast evidence path carves
# exactly like the real raster export (worldgen-v4 P3 §8.1 #1).
CARVE_SEED = 0x_C0FF_EE17


def _build_zone(profile: str, qi: float) -> BlueprintZone:
    return BlueprintZone(
        name=f"{profile}_p3",
        display_name=profile,
        bounds_xz=Bounds2D(min_x=-100, max_x=99, min_z=-100, max_z=99),
        center_xz=(0, 0),
        size_xz=(200, 200),
        spirit_qi=qi,
        danger_level=2,
        worldgen=ZoneWorldgenConfig(
            terrain_profile=profile,
            shape="ellipse",
            boundary=BoundarySpec(mode="soft", width=24),
            height_model={"base": [60, 80], "peak": 96},
            surface_palette=("grass_block", "stone", "gravel"),
        ),
    )


def export_profile_tiles(profile: str, out_dir: Path, qi: float = 0.6):
    """Run the real pipeline for *profile* into *out_dir*; return (manifest dir, fields)."""
    zone = _build_zone(profile, qi)
    blueprint = WorldBlueprint(
        version=1,
        world_name="p3_world",
        spawn_zone=zone.name,
        bounds_xz=zone.bounds_xz,
        notes=(),
        zones=(zone,),
    )
    catalog = TerrainProfileCatalog(
        version=1,
        profiles={
            profile: TerrainProfileSpec(
                name=profile,
                boundary=BoundarySpec(mode="soft", width=24),
                height={"base": [60, 80], "peak": 96},
                surface=("grass_block", "stone", "gravel"),
                water={"level": "none", "coverage": 0.0},
                passability="medium",
                extras={},
            )
        },
    )
    plan = build_generation_plan(
        blueprint=blueprint,
        profile_catalog=catalog,
        blueprint_path=Path("b.json"),
        profiles_path=Path("p.json"),
        output_dir=out_dir,
        tile_size=TILE_SIZE,
    )
    plan.bake_plan = build_raster_bake_plan(plan, out_dir)
    fields = synthesize_fields(plan)
    artifacts = export_rasters(plan, fields)
    return artifacts["raster_dir"], fields


def _decode_tile_columns(raster_dir: Path, tile_id: str) -> list[ColumnSpans]:
    tile_dir = raster_dir / tile_id
    count_bytes = (tile_dir / SPANS_COUNT_FILE).read_bytes()
    spans_bytes = (tile_dir / SPANS_FILE).read_bytes()
    return [
        decode_spans_column(count_bytes, spans_bytes, idx)
        for idx in range(TILE_SIZE * TILE_SIZE)
    ]


@dataclass
class CrossSectionStats:
    """Heuristic geometry stats used to self-check a carve round by eye + number."""

    columns_total: int
    multi_span_columns: int  # columns with >= 2 spans (overhang / isle / cave)
    max_span_count: int
    overhang_columns: int  # span[0] floats over an air gap above a lower span
    floating_columns: int  # a span sits entirely above the surface (isle)
    surface_min: int
    surface_max: int


def _analyze(columns: list[ColumnSpans]) -> CrossSectionStats:
    multi = 0
    overhang = 0
    floating = 0
    max_count = 0
    surfaces: list[int] = []
    for col in columns:
        max_count = max(max_count, col.count)
        if col.count >= 2:
            multi += 1
        surface = col.surface_ceiling_y
        if surface is not None:
            surfaces.append(surface)
            # Overhang: span[0] (surface) has its floor above a lower span's
            # ceiling with an air gap → it juts over open space.
            lower = [s for s in col.spans[1:] if s[1] < col.spans[0][0]]
            if lower and col.spans[0][0] > max(s[1] for s in lower) + 1:
                overhang += 1
            # Floating: any span whose floor sits well above the surface.
            if any(s[0] > surface + 1 for s in col.spans[1:]):
                floating += 1
    return CrossSectionStats(
        columns_total=len(columns),
        multi_span_columns=multi,
        max_span_count=max_count,
        overhang_columns=overhang,
        floating_columns=floating,
        surface_min=min(surfaces) if surfaces else 0,
        surface_max=max(surfaces) if surfaces else 0,
    )


def _build_fast_buffer(profile: str, qi: float) -> tuple[TileFieldBuffer, WorldTile]:
    """Vectorized buffer fill for one isolated zone tile (no stitch)."""
    fill = _PROFILE_FILL[profile]
    zone = _build_zone(profile, qi)
    tile = WorldTile(
        tile_x=0,
        tile_z=0,
        min_x=zone.bounds_xz.min_x,
        max_x=zone.bounds_xz.max_x,
        min_z=zone.bounds_xz.min_z,
        max_z=zone.bounds_xz.max_z,
    )
    palette = SurfacePalette()
    return fill(zone, tile, TILE_SIZE, palette), tile


def _fold_carve_rows(
    buffer: TileFieldBuffer, tile: WorldTile, profile: str, z_rows: list[int]
) -> dict[int, list[ColumnSpans]]:
    """Fold + carve only the requested Z rows (fast — avoids the 40k fold loop).

    Mirrors ``spans_fold.spans_for_tile`` column by column but for just the rows
    we render/sample, then applies the profile carver chain to those columns at
    the real CARVE_SEED + world coords.  Returns ``{z_row: [ColumnSpans × W]}``.
    """
    import numpy as np

    from ..spans_fold import (
        SKY_ISLE_SENTINEL,
        _layer_or_default,
        _layer_or_zero,
    )

    area = TILE_SIZE * TILE_SIZE
    height = np.asarray(buffer.layers["height"], dtype=np.float64).reshape(area)
    rift = _layer_or_default(buffer, "rift_axis_sdf", area, 99.0)
    rim = _layer_or_zero(buffer, "rim_edge_mask", area)
    frac = _layer_or_zero(buffer, "fracture_mask", area)
    neg = _layer_or_zero(buffer, "neg_pressure", area)
    ent = _layer_or_zero(buffer, "entrance_mask", area)
    cave = _layer_or_zero(buffer, "cave_mask", area)
    ceil = _layer_or_zero(buffer, "ceiling_height", area)
    sky_mask = _layer_or_zero(buffer, "sky_island_mask", area)
    if "sky_island_base_y" in buffer.layers:
        sky_base = np.asarray(
            buffer.layers["sky_island_base_y"], dtype=np.float64
        ).reshape(area)
    else:
        sky_base = np.full(area, SKY_ISLE_SENTINEL, dtype=np.float64)
    sky_thick = _layer_or_zero(buffer, "sky_island_thickness", area)

    generator = get_profile_generator(profile)
    chain = [build_carver(spec.kind, spec.params) for spec in generator.carvers]
    # Mirror the real export's P3 §6.1 双源收口: when a floating_island carver owns
    # the isle, skip the redundant 2D fold-isle so this fast preview matches the
    # on-disk spans (carver = sole isle source).
    suppress_fold_isle = any(c.name == "floating_island" for c in chain)

    out: dict[int, list[ColumnSpans]] = {}
    for z in z_rows:
        row: list[ColumnSpans] = []
        for x in range(TILE_SIZE):
            idx = z * TILE_SIZE + x
            top_y = v3_surface_top_y(
                height=float(height[idx]),
                rift_axis_sdf=float(rift[idx]),
                rim_edge_mask=float(rim[idx]),
                fracture_mask=float(frac[idx]),
                neg_pressure=float(neg[idx]),
                entrance_mask=float(ent[idx]),
            )
            col = column_spans_for_index(
                surface_y=top_y,
                cave_mask=float(cave[idx]),
                ceiling_height=float(ceil[idx]),
                sky_mask=float(sky_mask[idx]),
                sky_base_y=float(sky_base[idx]),
                sky_thickness=float(sky_thick[idx]),
                suppress_fold_isle=suppress_fold_isle,
            )
            wx = tile.min_x + x
            wz = tile.min_z + z
            for carver in chain:
                col = carver.carve_column(col, wx, wz, CARVE_SEED)
            row.append(col)
        out[z] = row
    return out


def render_profile_fast(
    profile: str,
    output_path: Path,
    *,
    qi: float = 0.6,
    z_local: int | None = None,
    y_min: int = -64,
    y_max: int = 380,
    pixel_scale: int = 2,
    title: str | None = None,
    stat_sample_rows: int = 12,
) -> tuple[Path, CrossSectionStats]:
    """Fast §6.1 evidence render: single-tile fold+carve, one row cross-section.

    Same carve geometry as the on-disk export (same chain + CARVE_SEED), just
    without the whole-world stitch — fast enough to iterate the three rounds.
    Stats are computed over ``stat_sample_rows`` evenly-spaced rows (plus the
    rendered one) rather than the whole tile, to stay fast.
    """
    buffer, tile = _build_fast_buffer(profile, qi)
    z = TILE_SIZE // 2 if z_local is None else z_local
    sample = sorted(
        {z}
        | {
            int(round(i * (TILE_SIZE - 1) / max(stat_sample_rows - 1, 1)))
            for i in range(stat_sample_rows)
        }
    )
    folded = _fold_carve_rows(buffer, tile, profile, sample)
    all_cols: list[ColumnSpans] = []
    for zr in sample:
        all_cols.extend(folded[zr])
    stats = _analyze(all_cols)
    output_path = Path(output_path)
    render_cross_section(
        folded[z], output_path, y_min=y_min, y_max=y_max,
        pixel_scale=pixel_scale, title=title,
    )
    return output_path, stats


def render_profile_cross_section(
    profile: str,
    output_path: Path,
    *,
    qi: float = 0.6,
    z_local: int | None = None,
    y_min: int = -64,
    y_max: int = 380,
    pixel_scale: int = 2,
    title: str | None = None,
) -> tuple[Path, CrossSectionStats]:
    """Export *profile*, decode one tile row at ``z_local``, render the slice.

    Returns the written PNG path and the geometry stats for self-check.  The cut
    line is a full tile row (200 columns) at a fixed local Z so the picture reads
    as a side-on photo across the whole zone.
    """
    with tempfile.TemporaryDirectory() as td:
        raster_dir, fields = export_profile_tiles(profile, Path(td), qi=qi)
        tile = fields.tiles[0]
        tile_id = tile.tile.tile_id
        columns = _decode_tile_columns(raster_dir, tile_id)

    z = TILE_SIZE // 2 if z_local is None else z_local
    row = columns[z * TILE_SIZE : (z + 1) * TILE_SIZE]
    stats = _analyze(columns)
    output_path = Path(output_path)
    render_cross_section(
        row,
        output_path,
        y_min=y_min,
        y_max=y_max,
        pixel_scale=pixel_scale,
        title=title,
    )
    return output_path, stats
