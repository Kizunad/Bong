"""Terrain profile for dan_zong_yi_yuan (丹宗遗园).

The ancient Hundred-Herb Sect compound.  Surface is podzol / coarse_dirt /
mud / purple_terracotta / mossy_cobblestone — millennium-old alchemy toxin
has stained the soil purple-grey.  Height base [62,78], peak 92.

Building placement is fully deterministic via the ``dan_zong_compound``
layout (see ``layouts/dan_zong_compound.py``).  Only three natural debris
items use density spawn (wild_herb_clump, scattered_bone_fragment,
small_rubble) and only outside the layout radius (96 blocks).
"""

from __future__ import annotations

import numpy as np

from .. import dsl
from ..blueprint import BlueprintZone
from ..dsl import QiGrade
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords
from .base import DecorationSpec, EcologySpec, ProfileContext, TerrainProfileGenerator

# §8.1 #4 — 丹宗遗园：灵气中等(0.35)但受丹毒抑制，主世界常量区，归 common 档。
# 现有 zone spirit_qi=0.40（dan_zong_yi_yuan）就近归档。
QI_GRADE = QiGrade.COMMON


DAN_ZONG_YI_YUAN_DECORATIONS = (
    DecorationSpec(
        name="wild_herb_clump",
        kind="shrub",
        blocks=("crimson_roots", "warped_roots", "twisting_vines"),
        size_range=(1, 3),
        rarity=0.85,
        notes=(
            "野生变异灵草丛：在 layout 半径 96 外的自然区域散布。"
            "layout 内的药圃格子另有种植规则，不靠这条。"
        ),
    ),
    DecorationSpec(
        name="scattered_bone_fragment",
        kind="shrub",
        blocks=("bone_block",),
        size_range=(1, 1),
        rarity=0.40,
        notes=(
            "散落骨片：1x1 bone_block，没有 loot——纯氛围，"
            "对应「弟子尸体被千年风蚀，连完整骸骨都难找」。"
        ),
    ),
    DecorationSpec(
        name="small_rubble",
        kind="shrub",
        blocks=("cobblestone", "mossy_cobblestone", "andesite"),
        size_range=(1, 2),
        rarity=0.60,
        notes="小石堆：宗门外缘建筑物风化残砾，1-2 格高小堆。",
    ),
)


class DanZongYiYuanGenerator(TerrainProfileGenerator):
    profile_name = "dan_zong_yi_yuan"
    extra_layers = (
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "flora_density",
        "flora_variant_id",
    )
    ecology = EcologySpec(
        decorations=DAN_ZONG_YI_YUAN_DECORATIONS,
        ambient_effects=("pill_steam_drift",),
        notes=(
            "丹宗遗园：上古百草门覆灭遗迹。千年丹毒侵染土壤呈灰紫色，"
            "灵气中等(0.4)但受丹毒抑制。八卦药圃+露天丹炉走 layout deterministic。"
        ),
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "qi_density ~0.35 average with pill-toxin suppression near center.",
            "mofa_decay elevated (~0.60) due to millennium of alchemy waste.",
            "qi_vein_flow present but weak — the sect drained local veins for alchemy.",
            "flora_density uses local ids 1..3 (wild_herb_clump, scattered_bone_fragment, small_rubble).",
            "compound_flatten_radius=96 applied by stitcher for layout placement site.",
        )


def fill_dan_zong_yi_yuan_tile(
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
            "flora_density",
            "flora_variant_id",
        ),
    )

    podzol_id = palette.ensure("podzol")
    coarse_dirt_id = palette.ensure("coarse_dirt")
    mud_id = palette.ensure("mud")
    purple_terra_id = palette.ensure("purple_terracotta")
    mossy_cobble_id = palette.ensure("mossy_cobblestone")
    stone_id = palette.ensure("stone")
    dandao_biome_id = 13  # index 13 = minecraft:old_growth_pine_taiga (plan-terrain-wiring-v1 FIX B)

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)

    # Edge warp for organic zone boundary（DSL fbm_height，base=1, amplitude=0.15）
    edge_warp = dsl.fbm_height(
        wx, wz, scale=480.0, octaves=3, seed=1100, amplitude=0.15, base=1.0
    )
    dx = (wx - center_x) / (half_w * edge_warp)
    dz = (wz - center_z) / (half_d * edge_warp)
    radial = np.sqrt(dx * dx + dz * dz)
    interior = radial <= 1.0

    # Noise layers（DSL 算子库）
    terrain_noise = dsl.warped_height(
        wx, wz,
        scale=300.0, octaves=4,
        warp_scale=400.0, warp_strength=35.0,
        seed=1110,
    )
    detail_noise = dsl.fbm_height(wx, wz, scale=80.0, octaves=3, seed=1120)

    # Height field: base [62,78], peak 92（DSL compose_height）
    height_base = dsl.compose_height(
        np.full_like(terrain_noise, 70.0),
        terrain_noise * 6.0,
        detail_noise * 2.0,
    )
    height = np.where(
        interior,
        np.clip(height_base, 62.0, 92.0),
        62.0 + terrain_noise * 4.0,
    )

    # Surface: podzol-dominant with toxin staining
    toxin_stain = dsl.fbm_height(wx, wz, scale=120.0, octaves=3, seed=1130)
    surface_id = np.full_like(height, podzol_id, dtype=np.int32)
    surface_id = np.where(toxin_stain > 0.20, purple_terra_id, surface_id)
    surface_id = np.where(toxin_stain < -0.25, coarse_dirt_id, surface_id)
    surface_id = np.where(
        (detail_noise > 0.30) & interior, mud_id, surface_id
    )
    surface_id = np.where(
        (detail_noise < -0.35) & interior, mossy_cobble_id, surface_id
    )

    # qi_density: moderate, suppressed near center
    qi_density = np.where(
        interior,
        np.clip(0.35 + terrain_noise * 0.08, 0.20, 0.50),
        0.12,
    )

    # mofa_decay: elevated due to alchemy waste
    mofa_decay = np.where(interior, 0.60 + terrain_noise * 0.05, 0.40)

    # qi_vein_flow: weak but present
    qi_vein_flow = np.where(
        interior,
        np.clip(terrain_noise * 0.1 + 0.05, 0.0, 0.15),
        0.0,
    )

    # Flora density / variant for density-spawned decorations
    flora_density = np.zeros_like(height)
    flora_variant = np.zeros_like(height, dtype=np.int32)

    # Variant 1 = wild_herb_clump (most common)
    herb_mask = interior & (detail_noise > 0.05)
    flora_variant = np.where(herb_mask, 1, flora_variant)
    flora_density = np.where(herb_mask, 0.45 + detail_noise * 0.15, flora_density)

    # Variant 2 = scattered_bone_fragment
    local_x = wx - center_x + half_w
    local_z = wz - center_z + half_d
    bone_mask = (
        np.mod(
            np.floor(local_x / 83.0) + np.floor(local_z / 79.0) * 3, 11
        ) == 0
    ) & (radial < 0.85)
    flora_variant = np.where(bone_mask, 2, flora_variant)
    flora_density = np.where(bone_mask, np.maximum(flora_density, 0.35), flora_density)

    # Variant 3 = small_rubble
    rubble_mask = (
        np.mod(
            np.floor(local_x / 67.0) - np.floor(local_z / 71.0), 7
        ) == 0
    ) & (radial < 0.90)
    flora_variant = np.where(rubble_mask, 3, flora_variant)
    flora_density = np.where(rubble_mask, np.maximum(flora_density, 0.40), flora_density)

    # Feature mask
    feature_mask = np.clip(
        0.45 + (1.0 - np.minimum(radial, 1.0)) * 0.40 + np.abs(terrain_noise) * 0.20,
        0.0, 1.0,
    )

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, dandao_biome_id, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein_flow, 3).ravel()
    buffer.layers["flora_density"] = np.round(
        np.clip(flora_density, 0.0, 1.0), 3
    ).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
