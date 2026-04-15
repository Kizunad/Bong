from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, warped_fbm_2d
from .base import (
    DecorationSpec,
    EcologySpec,
    ProfileContext,
    TerrainProfileGenerator,
)


WASTE_PLATEAU_DECORATIONS = (
    DecorationSpec(
        name="whalefall_rib_tree",
        kind="tree",
        blocks=("bone_block", "quartz_block", "white_concrete"),
        size_range=(10, 18),
        rarity=0.12,
        notes="鲸坠肋骨树：鲸类化石肋骨被腐朽之力立起，状若白树。地标级稀有。",
    ),
    DecorationSpec(
        name="dust_thorn",
        kind="shrub",
        blocks=("dead_bush", "sand", "sandstone"),
        size_range=(1, 3),
        rarity=0.70,
        notes="尘棘：半埋沙中的枯枝，划人而无汁。遍布平原。",
    ),
    DecorationSpec(
        name="null_pressure_rock",
        kind="boulder",
        blocks=("soul_sand", "soul_soil", "basalt"),
        size_range=(3, 7),
        rarity=0.30,
        notes="虚压岩：灵魂沙与玄武岩堆成的巨石，近之有压迫感。",
    ),
    DecorationSpec(
        name="ancient_ruin_fragment",
        kind="boulder",
        blocks=("chiseled_stone_bricks", "cracked_stone_bricks", "mossy_stone_bricks"),
        size_range=(2, 5),
        rarity=0.25,
        notes="古废片：雕刻石砖的断柱残基，诉说消逝的王朝。",
    ),
)


class WastePlateauGenerator(TerrainProfileGenerator):
    profile_name = "waste_plateau"
    extra_layers = ("neg_pressure", "ruin_density", "qi_density", "mofa_decay")
    ecology = EcologySpec(
        decorations=WASTE_PLATEAU_DECORATIONS,
        ambient_effects=("dust_storm", "bone_creak", "heavy_silence"),
        notes="北荒生态：极度稀疏。唯尘棘遍地，鲸坠肋骨树为罕见地标，"
              "虚压岩围绕 neg_pressure 区域，古废片是势力曾到达的证明。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Prioritize emptiness and exposure over intricate landforms.",
            "Negative-pressure patches should be represented as field-level masks, not block-level hacks.",
        )


def fill_waste_plateau_tile(
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
            "neg_pressure",
            "ruin_density",
            "qi_density",
            "mofa_decay",
        ),
    )
    stone_id = palette.ensure("stone")
    gravel_id = palette.ensure("gravel")
    soul_sand_id = palette.ensure("soul_sand")
    bone_block_id = palette.ensure("bone_block")
    plateau_biome_id = 6

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)
    patches = zone.worldgen.extras.get("negative_pressure_patches", [])

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    plateau = np.maximum(0.0, 1.0 - radial**1.15)
    crown = np.maximum(0.0, 1.0 - radial**2.3)

    # Shelf undulation — broad, windswept feel
    shelf = fbm_2d(wx, wz, scale=400.0, octaves=4, seed=500) * 5.0
    # Fracture lines — warped for organic cracks
    fracture = warped_fbm_2d(
        wx, wz, scale=140.0, octaves=4, warp_scale=220.0, warp_strength=50.0, seed=510
    )
    # Scarp edges
    scarp = fbm_2d(wx, wz, scale=200.0, octaves=3, seed=520)

    height = 84.0 + plateau * 11.5 + crown * 4.2 + shelf
    # Fracture cutting
    fracture_cut = np.maximum(0.0, -fracture - 0.2) * 18.0
    height = height - fracture_cut

    # Negative pressure patches
    neg_pressure = np.zeros_like(height)
    for patch in patches:
        px, pz = patch["center_xz"]
        radius = max(float(patch["radius"]), 1.0)
        strength = abs(float(patch["strength"]))
        dist = np.sqrt((wx - px) ** 2 + (wz - pz) ** 2)
        falloff = np.maximum(0.0, 1.0 - dist / radius)
        neg_pressure = np.maximum(neg_pressure, strength * falloff * falloff)

    height = height - neg_pressure * 16.0

    ruin_density = np.minimum(
        1.0,
        0.12 + neg_pressure * 1.2 + (1.0 - (scarp * 0.5 + 0.5)) * 0.18 + crown * 0.08,
    )

    surface_id = np.full_like(height, stone_id, dtype=np.int32)
    surface_id = np.where(neg_pressure > 0.46, soul_sand_id, surface_id)
    surface_id = np.where(
        (fracture < -0.35) & (surface_id == stone_id),
        gravel_id,
        surface_id,
    )
    surface_id = np.where(
        (scarp < -0.45) & (surface_id == stone_id),
        soul_sand_id,
        surface_id,
    )
    surface_id = np.where(
        (scarp > 0.45) & (surface_id == stone_id),
        bone_block_id,
        surface_id,
    )

    feature_mask = np.minimum(
        1.0,
        plateau * 0.28 + neg_pressure * 0.95 + np.maximum(0.0, -fracture) * 0.4,
    )

    # 北荒：死域。灵气极低（vein 无），末法极高（neg_pressure 点内趋近 1.0）
    qi_base = float(getattr(zone, "spirit_qi", 0.05))
    qi_density = np.clip(
        0.01 + qi_base * 0.2 * (1.0 - plateau) - neg_pressure * 0.5,
        0.0,
        0.18,
    )
    mofa_decay = np.clip(
        0.72 + plateau * 0.08 + neg_pressure * 0.25 + ruin_density * 0.05,
        0.5,
        1.0,
    )

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, plateau_biome_id, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["neg_pressure"] = np.round(neg_pressure, 3).ravel()
    buffer.layers["ruin_density"] = np.round(ruin_density, 3).ravel()
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()

    buffer.contributing_zones.append(zone.name)
    return buffer
