from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, ridge_2d, warped_fbm_2d
from ..spirit_eye_selector import select_spirit_eye_candidates
from .base import (
    DecorationSpec,
    EcologySpec,
    ProfileContext,
    TerrainProfileGenerator,
)


SNOW_LINE_Y = 285.0


BROKEN_PEAKS_DECORATIONS = (
    DecorationSpec(
        name="qing_yun_pine",
        kind="tree",
        blocks=("spruce_log", "spruce_leaves", "mossy_cobblestone"),
        size_range=(8, 14),
        rarity=0.28,
        notes="青云松：挺立山脊，松针四季不凋。曾是青云宗标志。",
    ),
    DecorationSpec(
        name="frost_silver_tree",
        kind="tree",
        blocks=("stripped_birch_log", "packed_ice", "blue_ice"),
        size_range=(6, 10),
        rarity=0.22,
        notes="霜银树：银白树干顶着冰晶树冠，仅高处雪线上下生长。",
    ),
    DecorationSpec(
        name="ridge_monolith",
        kind="boulder",
        blocks=("deepslate", "andesite", "cobbled_deepslate"),
        size_range=(4, 10),
        rarity=0.38,
        notes="断脊碑：山脊上的黑灰巨石，有些刻有残缺符文。",
    ),
    DecorationSpec(
        name="ice_thorn",
        kind="shrub",
        blocks=("packed_ice", "snow_block", "pointed_dripstone"),
        size_range=(2, 4),
        rarity=0.50,
        notes="冰棘：密集的冰刺灌丛，划手而含灵气。",
    ),
)


class BrokenPeaksGenerator(TerrainProfileGenerator):
    profile_name = "broken_peaks"
    extra_layers = (
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "spirit_eye_candidates",
        "flora_density",
        "flora_variant_id",
    )
    ecology = EcologySpec(
        decorations=BROKEN_PEAKS_DECORATIONS,
        ambient_effects=("high_wind", "occasional_snowfall", "faint_bell"),
        notes="青云残峰生态：低处青松遍布，山脊多断脊碑，雪线以上生霜银树与冰棘。"
              "整体冷峻，灵气游走于林间缝隙。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "High-relief massif with visible ridge lines.",
            "Favor rock exposure and narrow traversal corridors.",
        )


def fill_broken_peaks_tile(
    zone: BlueprintZone,
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
) -> TileFieldBuffer:
    buffer = TileFieldBuffer.create(
        tile,
        tile_size,
        (
            "height",
            "surface_id",
            "subsurface_id",
            "water_level",
            "biome_id",
            "feature_mask",
            "boundary_weight",
            "qi_density",
            "mofa_decay",
            "qi_vein_flow",
            "spirit_eye_candidates",
            "flora_density",
            "flora_variant_id",
        ),
    )
    stone_id = palette.ensure("stone")
    andesite_id = palette.ensure("andesite")
    deepslate_id = palette.ensure("deepslate")
    calcite_id = palette.ensure("calcite")
    snow_id = palette.ensure("snow_block")
    ice_id = palette.ensure("packed_ice")
    gravel_id = palette.ensure("gravel")
    coarse_dirt_id = palette.ensure("coarse_dirt")
    peaks_biome_id = 1
    frozen_peaks_biome_id = 9

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    massif = np.maximum(0.0, 1.0 - radial**1.45)

    # --- Primary ridges: sharp, dramatic ---
    ridges = ridge_2d(
        wx, wz, scale=140.0, octaves=5, lacunarity=2.1, gain=0.48, seed=100
    )
    # Secondary ridge system at different orientation for complexity
    ridges2 = ridge_2d(wx, wz, scale=220.0, octaves=4, seed=110)
    # Large-scale base — broad doming
    base_fbm = fbm_2d(wx, wz, scale=500.0, octaves=3, seed=120)
    # Fine crag detail
    detail = fbm_2d(wx, wz, scale=45.0, octaves=3, seed=130)
    # Erosion channels — warped for organic gullies
    erosion = warped_fbm_2d(
        wx, wz, scale=80.0, octaves=4, warp_scale=150.0, warp_strength=35.0, seed=140
    )

    # --- Compose height with dramatic relief ---
    # Stronger massif uplift so the skyline actually reaches the new 512-height world.
    base_height = 82.0 + massif * 110.0 + base_fbm * 18.0

    # Primary ridges: drive the skyline.
    ridge_lift = np.maximum(0.0, ridges + 0.08) ** 1.18 * 185.0 * massif
    # Secondary ridges: broaden the crest field.
    ridge2_lift = np.maximum(0.0, ridges2 + 0.05) ** 1.08 * 82.0 * massif
    # Crag detail on peaks
    crag = np.maximum(0.0, detail) * 18.0 * massif

    # Deep valleys between ridges — the key to dramatic relief
    valley_cut = np.minimum(0.0, ridges) * 60.0 * massif
    valley_cut2 = np.minimum(0.0, ridges2) * 22.0 * massif

    # Erosion gullies on slopes
    erosion_cut = np.maximum(0.0, erosion - 0.1) * 32.0 * massif

    height = (
        base_height
        + ridge_lift
        + ridge2_lift
        + crag
        + valley_cut
        + valley_cut2
        - erosion_cut
    )

    # Clamp to stay within world bounds (min_y=-64, max_y=319)
    height = np.clip(height, -50.0, 460.0)

    # --- Surface: more variety based on height and slope ---
    ruggedness = np.abs(ridges) + np.abs(detail) * 0.7
    is_peak = height > 300.0
    is_high = height > 220.0
    is_valley = ridges < -0.3

    surface_id = np.full_like(height, stone_id, dtype=np.int32)
    surface_id = np.where(height > SNOW_LINE_Y, snow_id, surface_id)
    surface_id = np.where((height > 270.0) & (detail > 0.42), ice_id, surface_id)
    surface_id = np.where(
        (height > 235.0) & (surface_id == stone_id), calcite_id, surface_id
    )
    # High exposed peaks: deepslate
    surface_id = np.where(
        is_peak & (ruggedness > 0.6) & (surface_id == stone_id),
        deepslate_id,
        surface_id,
    )
    # High slopes: andesite
    surface_id = np.where(
        is_high & (ruggedness > 0.8) & ~is_peak, andesite_id, surface_id
    )
    # Valley floors: gravel from erosion
    surface_id = np.where(is_valley, gravel_id, surface_id)
    # Eroded gullies: coarse dirt
    surface_id = np.where(
        (erosion > 0.4) & ~is_peak & ~is_valley,
        coarse_dirt_id,
        surface_id,
    )

    feature_mask = np.minimum(1.0, massif * 0.6 + ruggedness * 0.28)

    area = tile_size * tile_size
    biome_id = np.where(height > 300.0, frozen_peaks_biome_id, peaks_biome_id)

    # 青云残峰：高处灵气较盛（上空接天），峰脊有灵脉（古修士采脉处）。
    # 末法中等——古战痕迹在山脊间。
    qi_base = float(getattr(zone, "spirit_qi", 0.5))
    altitude_t = np.clip((height - 82.0) / 220.0, 0.0, 1.0)
    ridge_vein = np.maximum(0.0, ridges) * massif
    qi_vein_flow = np.clip(ridge_vein * altitude_t * 0.9, 0.0, 1.0)
    qi_density = np.clip(
        0.18 + altitude_t * 0.35 + qi_vein_flow * 0.20,
        0.0,
        1.0,
    ) * (0.5 + qi_base)
    qi_density = np.clip(qi_density, 0.0, 1.0)
    mofa_decay = np.clip(
        0.35 + (1.0 - altitude_t) * 0.20 + np.maximum(0.0, erosion) * 0.10 - qi_vein_flow * 0.15,
        0.1,
        0.7,
    )

    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = biome_id.ravel().astype(np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein_flow, 3).ravel()
    buffer.layers["spirit_eye_candidates"] = select_spirit_eye_candidates(
        height,
        qi_density,
        feature_mask,
        wx,
        wz,
        density_bias=1.45,
    ).ravel()

    # --- Flora (variant id 1..4 mirror BROKEN_PEAKS_DECORATIONS) ---
    # 1 qing_yun_pine  — mid-altitude green slope
    # 2 frost_silver_tree — high-altitude snow line
    # 3 ridge_monolith — stark monoliths on ridge lines
    # 4 ice_thorn — thickets on cold slopes
    flora_density = np.zeros_like(height)
    flora_variant = np.zeros_like(height, dtype=np.int32)

    mid_band = (height > 100.0) & (height < 230.0) & (ridges > -0.15)
    flora_variant = np.where(mid_band & (detail > -0.1), 1, flora_variant)
    flora_density = np.where(mid_band, np.maximum(flora_density, 0.45 + massif * 0.15), flora_density)

    high_band = height > 240.0
    flora_variant = np.where(high_band & (detail > 0.0), 2, flora_variant)
    flora_density = np.where(high_band, np.maximum(flora_density, 0.35), flora_density)

    # Ridge monoliths: on top of sharp ridges
    ridge_top = (ridges > 0.45) & (massif > 0.3)
    flora_variant = np.where(ridge_top, 3, flora_variant)
    flora_density = np.where(ridge_top, np.maximum(flora_density, 0.50), flora_density)

    # Ice thorn scatter near frozen peaks
    frozen = (height > 270.0) & (flora_variant == 0)
    flora_variant = np.where(frozen, 4, flora_variant)
    flora_density = np.where(frozen, np.maximum(flora_density, 0.45), flora_density)

    flora_density = np.clip(flora_density, 0.0, 1.0)
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
