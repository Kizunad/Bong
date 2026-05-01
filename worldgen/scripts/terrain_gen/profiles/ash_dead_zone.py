from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, warped_fbm_2d
from .base import DecorationSpec, EcologySpec, ProfileContext, TerrainProfileGenerator


ASH_DEAD_ZONE_DECORATIONS = (
    DecorationSpec(
        name="cantan_block_drift",
        kind="shrub",
        blocks=("coarse_dirt", "gravel", "sand"),
        size_range=(1, 2),
        rarity=0.85,
        notes="残灰堆：粗土 + 沙砾 + 沙的灰白颗粒基底；server 层负责脚印。",
    ),
    DecorationSpec(
        name="dried_corpse_mound",
        kind="boulder",
        blocks=("bone_block", "dirt", "dead_bush"),
        size_range=(2, 3),
        rarity=0.20,
        notes="干尸堆：历代过路者残骸；loot 锚为凡铁 / 退活骨币 / 干灵草。",
    ),
    DecorationSpec(
        name="petrified_tree_stump",
        kind="tree",
        blocks=("polished_diorite", "stripped_oak_log", "dead_bush"),
        size_range=(2, 4),
        rarity=0.30,
        notes="石化枯桩：旧馈赠区植物被抽干后留下的硬化残迹。",
    ),
    DecorationSpec(
        name="ash_spider_lair",
        kind="boulder",
        blocks=("coarse_dirt", "cobweb", "gray_concrete_powder"),
        size_range=(1, 2),
        rarity=0.10,
        notes="灰烬蛛巢：与残灰堆近似，仅有极淡蛛丝；server 在边缘放大伏击权重。",
    ),
    DecorationSpec(
        name="silent_obelisk",
        kind="boulder",
        blocks=("smooth_stone", "stone", "andesite"),
        size_range=(3, 5),
        rarity=0.08,
        notes="无声碑：没有碑文的死地标记，象征天道不再修复此处。",
    ),
    DecorationSpec(
        name="vanished_path_marker",
        kind="shrub",
        blocks=("cobblestone_wall", "torch", "stone_button"),
        size_range=(1, 1),
        rarity=0.05,
        notes="消亡路标：前人留下的熄灭导航标，提示这条路曾有人走过。",
    ),
)


class AshDeadZoneGenerator(TerrainProfileGenerator):
    profile_name = "ash_dead_zone"
    extra_layers = (
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "flora_density",
        "flora_variant_id",
    )
    ecology = EcologySpec(
        decorations=ASH_DEAD_ZONE_DECORATIONS,
        ambient_effects=(),
        notes="余烬死域：恒 0 灵气核心、无灵脉、无 ambient effect。残灰覆盖为视觉基底。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "qi_density core is exactly zero; seasonal/cadence systems should treat this zone as no-cadence.",
            "qi_vein_flow is explicitly zero everywhere to mark severed meridians.",
            "flora_variant_id uses local ids 1..6 before stitcher remaps into the global decoration palette.",
        )


def fill_ash_dead_zone_tile(
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

    coarse_dirt_id = palette.ensure("coarse_dirt")
    gravel_id = palette.ensure("gravel")
    sand_id = palette.ensure("sand")
    smooth_stone_id = palette.ensure("smooth_stone")
    stone_id = palette.ensure("stone")
    ash_biome_id = 6

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    edge_warp = 1.0 + fbm_2d(wx, wz, scale=520.0, octaves=3, seed=870) * 0.18
    dx = (wx - center_x) / (half_w * edge_warp)
    dz = (wz - center_z) / (half_d * (1.0 - (edge_warp - 1.0) * 0.35))
    radial = np.sqrt(dx * dx + dz * dz)
    interior = radial <= 1.0
    core = radial <= 0.5
    body = (radial > 0.5) & (radial <= 0.8)
    edge = (radial > 0.8) & (radial <= 1.0)

    ash_wave = warped_fbm_2d(
        wx,
        wz,
        scale=260.0,
        octaves=4,
        warp_scale=380.0,
        warp_strength=42.0,
        seed=910,
    )
    low_ripple = fbm_2d(wx, wz, scale=96.0, octaves=3, seed=920)
    height = 76.0 + np.maximum(0.0, 1.0 - radial) * 5.5 + ash_wave * 2.4 + low_ripple * 0.9
    height = np.where(interior, height, 74.0 + ash_wave * 1.6)

    surface_id = np.full_like(height, coarse_dirt_id, dtype=np.int32)
    surface_id = np.where(ash_wave > 0.22, gravel_id, surface_id)
    surface_id = np.where(ash_wave < -0.26, sand_id, surface_id)
    surface_id = np.where((radial < 0.18) & (low_ripple > 0.30), smooth_stone_id, surface_id)
    surface_id = np.where((radial > 1.05) & (ash_wave > 0.34), stone_id, surface_id)

    qi_density = np.full_like(height, 0.10, dtype=np.float64)
    qi_density = np.where(edge, 0.05, qi_density)
    qi_density = np.where(body, 0.02, qi_density)
    qi_density = np.where(core, 0.0, qi_density)
    qi_density = np.where(interior, qi_density, 0.10)

    mofa_decay = np.where(
        core,
        0.95,
        np.where(body, 0.90, np.where(edge, 0.75, 0.55)),
    )
    qi_vein_flow = np.zeros_like(height)
    feature_mask = np.clip(0.55 + (1.0 - np.minimum(radial, 1.0)) * 0.35 + np.abs(ash_wave) * 0.25, 0.0, 1.0)

    flora_density = np.zeros_like(height)
    flora_variant = np.zeros_like(height, dtype=np.int32)

    flora_variant = np.where(interior, 1, flora_variant)
    flora_density = np.where(interior, 0.62 + np.maximum(0.0, 1.0 - radial) * 0.22, flora_density)

    local_x = wx - center_x + half_w
    local_z = wz - center_z + half_d
    corpse_mask = (np.mod(np.floor(local_x / 97.0) + np.floor(local_z / 89.0) * 3, 17) == 0) & (radial < 0.78)
    flora_variant = np.where(corpse_mask, 2, flora_variant)
    flora_density = np.where(corpse_mask, np.maximum(flora_density, 0.42), flora_density)

    stump_mask = (np.mod(np.floor(local_x / 137.0) - np.floor(local_z / 113.0), 13) == 0) & (radial < 0.72)
    flora_variant = np.where(stump_mask, 3, flora_variant)
    flora_density = np.where(stump_mask, np.maximum(flora_density, 0.34), flora_density)

    spider_mask = edge & (np.mod(np.floor(local_x / 53.0) + np.floor(local_z / 47.0) * 5, 9) == 0)
    flora_variant = np.where(spider_mask, 4, flora_variant)
    flora_density = np.where(spider_mask, np.maximum(flora_density, 0.48), flora_density)

    obelisk_mask = ((wx - (center_x - 180.0)) ** 2 + (wz - (center_z + 120.0)) ** 2) <= 20.0**2
    flora_variant = np.where(obelisk_mask, 5, flora_variant)
    flora_density = np.where(obelisk_mask, np.maximum(flora_density, 0.60), flora_density)

    marker_path = (np.abs(wz - (center_z - 360.0)) <= 3.0) & interior
    marker_mask = marker_path & (np.mod(np.floor((wx - (center_x - half_w)) / 64.0), 7) == 0)
    flora_variant = np.where(marker_mask, 6, flora_variant)
    flora_density = np.where(marker_mask, np.maximum(flora_density, 0.55), flora_density)

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, ash_biome_id, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein_flow, 3).ravel()
    buffer.layers["flora_density"] = np.round(np.clip(flora_density, 0.0, 1.0), 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
