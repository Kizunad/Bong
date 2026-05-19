"""Terrain profile for wangyintai (忘音台).

Ruined compound of the Jingxu Sect, the only pre-Mofa sect that
specialized in vortex (涡流) cultivation.  Surface is smooth_basalt /
deepslate / calcite / gray_concrete -- the void resonance has bleached
and vitrified the stone.  Height base [68,86], peak 98.

Building placement is fully deterministic via the ``wangyintai_compound``
layout (see ``layouts/wangyintai_compound.py``).  Only two natural debris
items use density spawn (silent_rubble, cracked_disc_fragment) and only
outside the layout radius (48 blocks).
"""

from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, warped_fbm_2d
from .base import DecorationSpec, EcologySpec, ProfileContext, TerrainProfileGenerator


WANGYINTAI_DECORATIONS = (
    DecorationSpec(
        name="silent_rubble",
        kind="boulder",
        blocks=("smooth_basalt", "deepslate", "calcite"),
        size_range=(1, 3),
        rarity=0.60,
        notes=(
            "沉默碎石：layout 半径 48 外的自然区域散布。"
            "千年虚蚀将石头磨成玻璃质表面。"
        ),
    ),
    DecorationSpec(
        name="cracked_disc_fragment",
        kind="shrub",
        blocks=("smooth_basalt",),
        size_range=(1, 1),
        rarity=0.30,
        notes=(
            "碎裂涡旋盘残片：1x1 smooth_basalt，纯叙事氛围物件，"
            "对应「弟子法器散落，千年风蚀后只剩残片」。"
        ),
    ),
)


class WangyintaiGenerator(TerrainProfileGenerator):
    profile_name = "wangyintai"
    extra_layers = (
        "qi_density",
        "mofa_decay",
    )
    ecology = EcologySpec(
        decorations=WANGYINTAI_DECORATIONS,
        ambient_effects=("void_shimmer",),
        notes=(
            "忘音台：末法前静虚观遗址。千年虚蚀将岩石玻璃化，灵气为负(-0.15)。"
            "中央观天台废墟仍运转微型涡流阵列。布局走 wangyintai_compound deterministic。"
        ),
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "qi_density ~-0.10 average: negative qi field, void resonance.",
            "mofa_decay elevated (~0.55) from millennium of vortex erosion.",
            "surface: smooth_basalt / deepslate / calcite / gray_concrete.",
            "compound_flatten_radius=48 applied by stitcher for layout placement site.",
            "no water features. Architectural layout: wangyintai_compound.",
        )


def fill_wangyintai_tile(
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
        ),
    )

    smooth_basalt_id = palette.ensure("smooth_basalt")
    deepslate_id = palette.ensure("deepslate")
    calcite_id = palette.ensure("calcite")
    gray_concrete_id = palette.ensure("gray_concrete")
    stone_id = palette.ensure("stone")
    wangyintai_biome_id = 17  # unique biome id for wangyintai

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)

    # Edge warp for organic zone boundary
    edge_warp = 1.0 + fbm_2d(wx, wz, scale=500.0, octaves=3, seed=2200) * 0.12
    dx = (wx - center_x) / (half_w * edge_warp)
    dz = (wz - center_z) / (half_d * edge_warp)
    radial = np.sqrt(dx * dx + dz * dz)
    interior = radial <= 1.0

    # Noise layers
    terrain_noise = warped_fbm_2d(
        wx,
        wz,
        scale=350.0,
        octaves=4,
        warp_scale=450.0,
        warp_strength=30.0,
        seed=2210,
    )
    detail_noise = fbm_2d(wx, wz, scale=90.0, octaves=3, seed=2220)

    # Height field: base [68,86], peak 98
    height_base = 77.0 + terrain_noise * 7.0 + detail_noise * 2.0
    height = np.where(
        interior,
        np.clip(height_base, 68.0, 98.0),
        68.0 + terrain_noise * 4.0,
    )

    # Surface: smooth_basalt dominant with vitrified stone patterns
    vitrify_noise = fbm_2d(wx, wz, scale=100.0, octaves=3, seed=2230)
    surface_id = np.full_like(height, smooth_basalt_id, dtype=np.int32)
    surface_id = np.where(vitrify_noise > 0.25, deepslate_id, surface_id)
    surface_id = np.where(vitrify_noise < -0.20, calcite_id, surface_id)
    surface_id = np.where(
        (detail_noise > 0.30) & interior, gray_concrete_id, surface_id
    )

    # qi_density: negative, void-resonance
    qi_density = np.where(
        interior,
        np.clip(-0.10 + terrain_noise * 0.05, -0.25, 0.0),
        -0.05,
    )

    # mofa_decay: elevated from vortex erosion
    mofa_decay = np.where(interior, 0.55 + terrain_noise * 0.06, 0.35)

    # Feature mask
    feature_mask = np.clip(
        0.40 + (1.0 - np.minimum(radial, 1.0)) * 0.35 + np.abs(terrain_noise) * 0.15,
        0.0,
        1.0,
    )

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, wangyintai_biome_id, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()

    buffer.contributing_zones.append(zone.name)
    return buffer
