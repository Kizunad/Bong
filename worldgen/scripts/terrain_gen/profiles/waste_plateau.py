from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, warped_fbm_2d
from ..structures.whale_fossil import rasterize_whale_fossil_mask
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
        blocks=("dead_bush",),
        size_range=(1, 2),
        rarity=0.50,
        notes="尘棘：半埋沙中的枯枝。accent 删掉（曾用 sand/sandstone 实体方块导致地表凸起一圈）；改靠周围的 wastes_dead_bush ground cover 自然铺就。",
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
    # Ground cover spec
    DecorationSpec(
        name="wastes_dead_bush",
        kind="flower",
        blocks=("dead_bush",),
        size_range=(1, 1),
        rarity=0.30,
        notes="枯灌：北荒标志地表植被，几无生机。",
    ),
)


class WastePlateauGenerator(TerrainProfileGenerator):
    profile_name = "waste_plateau"
    extra_layers = (
        "neg_pressure",
        "ruin_density",
        "qi_density",
        "mofa_decay",
        "flora_density",
        "flora_variant_id",
        "ground_cover_density",
        "ground_cover_id",
        "fossil_bbox",
    )
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
            "flora_density",
            "flora_variant_id",
            "ground_cover_density",
            "ground_cover_id",
            "fossil_bbox",
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

    fossil_bbox = rasterize_whale_fossil_mask(zone, wx, wz)
    surface_id = np.where(fossil_bbox > 0, bone_block_id, surface_id)

    feature_mask = np.minimum(
        1.0,
        plateau * 0.28
        + neg_pressure * 0.95
        + np.maximum(0.0, -fracture) * 0.4
        + (fossil_bbox > 0) * 0.45,
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
    buffer.layers["fossil_bbox"] = fossil_bbox.ravel().astype(np.uint8)

    # --- Flora: 1 whalefall_rib_tree / 2 dust_thorn / 3 null_pressure_rock /
    # 4 ancient_ruin_fragment ---
    flora_density = np.zeros_like(height)
    flora_variant = np.zeros_like(height, dtype=np.int32)

    # Dust thorn ubiquitous on plateau body
    flora_variant = np.where(plateau > 0.2, 2, flora_variant)
    flora_density = np.where(plateau > 0.2, np.maximum(flora_density, 0.55), flora_density)

    # Null-pressure rocks around neg_pressure zones
    null_band = neg_pressure > 0.25
    flora_variant = np.where(null_band, 3, flora_variant)
    flora_density = np.where(null_band, np.maximum(flora_density, 0.40 + neg_pressure * 0.3), flora_density)

    # Ancient ruin fragments where ruin_density significant
    ruin_band = ruin_density > 0.45
    flora_variant = np.where(ruin_band, 4, flora_variant)
    flora_density = np.where(ruin_band, np.maximum(flora_density, 0.45), flora_density)

    # Rare whalefall rib trees on crown center
    whalefall_band = ((crown > 0.6) & (scarp > 0.3)) | (fossil_bbox > 0)
    flora_variant = np.where(whalefall_band, 1, flora_variant)
    flora_density = np.where(whalefall_band, np.maximum(flora_density, 0.18), flora_density)

    flora_density = np.clip(flora_density, 0.0, 1.0)
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)

    # --- Ground cover (枯灌) ---
    # waste_plateau local_id 5=wastes_dead_bush。
    from . import global_decoration_id

    gc_dead_bush = global_decoration_id("waste_plateau", 5)

    # 北荒地表稀疏，密度低；neg_pressure 区域更稀疏
    gc_density = np.where(plateau > 0.2, 0.25 + plateau * 0.10, 0.0)
    gc_density = np.where(null_band, gc_density * 0.4, gc_density)
    gc_density = np.clip(gc_density, 0.0, 0.40)
    buffer.layers["ground_cover_density"] = np.round(gc_density, 3).ravel()

    gc_variant = np.where(gc_density > 0.0, gc_dead_bush, 0).astype(np.int32)
    buffer.layers["ground_cover_id"] = gc_variant.ravel().astype(np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
