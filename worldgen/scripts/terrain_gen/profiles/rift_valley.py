from __future__ import annotations

import math

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, ridge_2d, warped_fbm_2d
from .base import (
    DecorationSpec,
    EcologySpec,
    ProfileContext,
    TerrainProfileGenerator,
)


RIFT_VALLEY_DECORATIONS = (
    DecorationSpec(
        name="scarlet_bone_tree",
        kind="tree",
        blocks=("crimson_stem", "bone_block", "nether_wart_block"),
        size_range=(5, 10),
        rarity=0.30,
        notes="赤骨树：绯红菌柄与骨块穿插，树冠如凝血。血谷独有。",
    ),
    DecorationSpec(
        name="fire_vein_cactus",
        kind="shrub",
        blocks=("magma_block", "blackstone", "red_concrete"),
        size_range=(2, 4),
        rarity=0.55,
        notes="火脉仙人掌：裂隙旁丛生，通体发烫，吸附裂缝火气。",
    ),
    DecorationSpec(
        name="blood_stele",
        kind="boulder",
        blocks=("red_sandstone", "chiseled_red_sandstone", "terracotta"),
        size_range=(3, 6),
        rarity=0.40,
        notes="血碑：独立矗立的红砂岩碑，表面似被血染。古战纪录。",
    ),
    DecorationSpec(
        name="nether_nylium_patch",
        kind="shrub",
        blocks=("crimson_nylium", "crimson_roots", "weeping_vines"),
        size_range=(1, 2),
        rarity=0.65,
        notes="绯血苔藓：成片覆盖石缝，根须下探灵脉。",
    ),
)


class RiftValleyGenerator(TerrainProfileGenerator):
    profile_name = "rift_valley"
    extra_layers = (
        "rift_axis_sdf",
        "rim_edge_mask",
        "fracture_mask",
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "flora_density",
        "flora_variant_id",
    )
    ecology = EcologySpec(
        decorations=RIFT_VALLEY_DECORATIONS,
        ambient_effects=("sulfur_puff", "distant_roar", "blood_moon_haze"),
        notes="血谷生态：赤骨树沿裂隙生长，火脉仙人掌在断层边吐热气，"
              "血碑散布谷底。绯血苔藓铺地，红黑主调。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Primary macro landform is a long rift with hard boundary falloff.",
            "Later postprocess may deepen local chokes, but the main valley belongs to field generation.",
        )


def fill_rift_valley_tile(
    zone: BlueprintZone,
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
) -> TileFieldBuffer:
    layer_names = (
        "height",
        "surface_id",
        "subsurface_id",
        "water_level",
        "biome_id",
        "feature_mask",
        "boundary_weight",
        "rift_axis_sdf",
        "rim_edge_mask",
        "fracture_mask",
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "flora_density",
        "flora_variant_id",
    )
    buffer = TileFieldBuffer.create(tile, tile_size, layer_names)
    blackstone_id = palette.ensure("blackstone")
    basalt_id = palette.ensure("basalt")
    magma_id = palette.ensure("magma_block")
    nylium_id = palette.ensure("crimson_nylium")
    stone_id = palette.ensure("stone")
    gravel_id = palette.ensure("gravel")
    rift_biome_id = 3

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_l = max(zone.size_xz[1] * 0.5, 1.0)
    angle = math.radians(-20.0)
    cos_a = math.cos(angle)
    sin_a = math.sin(angle)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    fdx = wx - center_x
    fdz = wz - center_z
    along = fdx * cos_a - fdz * sin_a
    cross = fdx * sin_a + fdz * cos_a

    along_ratio = np.clip(along / half_l, -1.0, 1.0)
    cross_ratio = cross / half_w

    # Width variation along rift — FBM for organic wobble
    width_warp = fbm_2d(along, cross, scale=200.0, octaves=3, seed=300)
    width_noise = 1.0 + width_warp * 0.4
    normalized_cross = np.abs(cross_ratio) / np.maximum(width_noise, 0.35)

    axial_profile = np.maximum(0.0, 1.0 - np.abs(along_ratio) ** 1.25)

    # Branch canyons — warped noise for organic branching
    branch = warped_fbm_2d(
        wx, wz, scale=110.0, octaves=4, warp_scale=180.0, warp_strength=45.0, seed=310
    )
    valley_strength = np.maximum(0.0, 1.0 - normalized_cross**1.42) * axial_profile
    valley_strength = np.minimum(
        1.0, valley_strength + np.maximum(0.0, branch - 0.35) * 0.3
    )

    # Rim detail
    rim_fbm = fbm_2d(wx, wz, scale=130.0, octaves=4, seed=320)
    rim_height = 108.0 + 13.0 * axial_profile + rim_fbm * 5.0

    # Valley floor detail
    floor_fbm = fbm_2d(along, cross, scale=160.0, octaves=4, seed=330)
    floor_height = 40.0 + floor_fbm * 8.0

    height = rim_height - (rim_height - floor_height) * valley_strength
    # Branch canyon carving
    branch_cut = (
        np.maximum(0.0, branch - 0.5)
        * 22.0
        * np.where(normalized_cross < 1.2, 1.0, 0.0)
    )
    height = height - branch_cut

    # Surface
    surface_id = np.full_like(height, stone_id, dtype=np.int32)
    surface_id = np.where(valley_strength > 0.88, magma_id, surface_id)
    surface_id = np.where(
        (valley_strength > 0.72) & (valley_strength <= 0.88),
        blackstone_id,
        surface_id,
    )
    surface_id = np.where(
        (valley_strength > 0.52) & (valley_strength <= 0.72),
        basalt_id,
        surface_id,
    )
    surface_id = np.where(
        (normalized_cross > 0.98) & (surface_id == stone_id),
        gravel_id,
        surface_id,
    )
    surface_id = np.where(
        (rim_fbm > 0.55) & (surface_id == stone_id),
        gravel_id,
        surface_id,
    )

    rim_edge_mask = np.maximum(0.0, 1.0 - np.abs(normalized_cross - 1.0) * 3.6)
    fracture_noise = ridge_2d(wx, wz, scale=60.0, octaves=5, seed=340)
    fracture_mask = np.where(
        valley_strength > 0.4,
        np.maximum(0.0, fracture_noise) * valley_strength,
        0.0,
    )
    surface_id = np.where(
        (fracture_mask > 0.7) & (surface_id == stone_id), nylium_id, surface_id
    )
    feature_mask = np.maximum(valley_strength, rim_edge_mask * 0.72)

    # 血谷：末法重（0.7），灵气稀薄但沿裂隙轴线有一条灵脉（古战场残余灵压）。
    # qi_vein_flow 集中在 axis 附近（normalized_cross 接近 0），随 branch 扩散。
    qi_base = float(getattr(zone, "spirit_qi", 0.3))
    axis_core = np.maximum(0.0, 1.0 - normalized_cross * 2.4) * axial_profile
    vein_wiggle = 0.5 + fbm_2d(wx, wz, scale=180.0, octaves=3, seed=360) * 0.5
    qi_vein_flow = np.clip(axis_core * vein_wiggle, 0.0, 1.0)
    # 灵气：谷底贴着灵脉略高，谷壁和裂隙更低（断裂吸散灵气）
    qi_density = np.clip(
        0.06 + qi_vein_flow * (0.45 * qi_base / max(qi_base, 0.3))
        - fracture_mask * 0.08,
        0.0,
        1.0,
    )
    # 末法：整体高，fracture 越深越腐朽（骨尘堆积）
    mofa_decay = np.clip(
        0.55 + valley_strength * 0.15 + fracture_mask * 0.15 - qi_vein_flow * 0.20,
        0.1,
        0.95,
    )

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, rift_biome_id, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["rift_axis_sdf"] = np.round(normalized_cross, 3).ravel()
    buffer.layers["rim_edge_mask"] = np.round(rim_edge_mask, 3).ravel()
    buffer.layers["fracture_mask"] = np.round(fracture_mask, 3).ravel()
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein_flow, 3).ravel()

    # --- Flora: 1 scarlet_bone_tree / 2 fire_vein_cactus / 3 blood_stele /
    # 4 nether_nylium_patch ---
    flora_density = np.zeros_like(height)
    flora_variant = np.zeros_like(height, dtype=np.int32)

    bone_tree_band = (qi_vein_flow > 0.25) & (valley_strength > 0.3)
    flora_variant = np.where(bone_tree_band, 1, flora_variant)
    flora_density = np.where(bone_tree_band, np.maximum(flora_density, 0.45), flora_density)

    cactus_band = (fracture_mask > 0.35) & (valley_strength > 0.2)
    flora_variant = np.where(cactus_band, 2, flora_variant)
    flora_density = np.where(cactus_band, np.maximum(flora_density, 0.55), flora_density)

    stele_band = rim_edge_mask > 0.6
    flora_variant = np.where(stele_band, 3, flora_variant)
    flora_density = np.where(stele_band, np.maximum(flora_density, 0.40), flora_density)

    nylium_band = (valley_strength > 0.5) & (flora_variant == 0)
    flora_variant = np.where(nylium_band, 4, flora_variant)
    flora_density = np.where(nylium_band, np.maximum(flora_density, 0.50), flora_density)

    flora_density = np.clip(flora_density, 0.0, 1.0)
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
