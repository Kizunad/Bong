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


SPRING_MARSH_DECORATIONS = (
    DecorationSpec(
        name="ling_yun_mangrove",
        kind="tree",
        blocks=("mangrove_log", "mangrove_leaves", "mangrove_roots", "muddy_mangrove_roots"),
        size_range=(7, 11),
        rarity=0.40,
        notes="灵云红树：盘根错节立于浅水，根部聚灵气。水乡地标。",
    ),
    DecorationSpec(
        name="spirit_willow",
        kind="tree",
        blocks=("jungle_log", "moss_block", "azalea_leaves", "flowering_azalea_leaves"),
        size_range=(8, 13),
        rarity=0.25,
        notes="灵垂柳：绿意浓密的长枝柳，花叶茂盛于灵泉眼周边。",
    ),
    DecorationSpec(
        name="lotus_cluster",
        kind="flower",
        blocks=("lily_pad", "pink_tulip", "peony"),
        size_range=(1, 1),
        rarity=0.65,
        notes="灵莲丛：水面浮满莲叶与粉花，修士静坐处。",
    ),
    DecorationSpec(
        name="reed_thicket",
        kind="shrub",
        blocks=("sugar_cane", "tall_grass", "fern"),
        size_range=(2, 4),
        rarity=0.70,
        notes="灵苇：成片高苇将水道分隔成迷宫，藏鱼与小灵兽。",
    ),
    DecorationSpec(
        name="jade_moss_rock",
        kind="boulder",
        blocks=("moss_block", "mossy_cobblestone", "prismarine"),
        size_range=(2, 4),
        rarity=0.35,
        notes="翠苔石：水边苔石，偶含微弱夜光。",
    ),
)


class SpringMarshGenerator(TerrainProfileGenerator):
    profile_name = "spring_marsh"
    extra_layers = ("qi_density", "mofa_decay", "qi_vein_flow")
    ecology = EcologySpec(
        decorations=SPRING_MARSH_DECORATIONS,
        ambient_effects=("water_droplets", "frog_call", "gentle_qi_shimmer"),
        notes="灵泉湿地生态：红树与垂柳环抱水眼，莲丛与苇草铺满浅滩，翠苔石点缀岛岸。"
              "每一种植被都透着灵气，天然修行胜地。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Low basin terrain with shallow water coverage.",
            "Traversal should feel fragmented by channels and islets.",
        )


def fill_spring_marsh_tile(
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
        ),
    )
    mud_id = palette.ensure("mud")
    clay_id = palette.ensure("clay")
    grass_id = palette.ensure("grass_block")
    moss_id = palette.ensure("moss_block")
    rooted_dirt_id = palette.ensure("rooted_dirt")
    stone_id = palette.ensure("stone")
    marsh_biome_id = 2
    mangrove_biome_id = 10

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    basin = np.maximum(0.0, 1.0 - radial**1.7)

    large_islands = warped_fbm_2d(
        wx, wz, scale=280.0, octaves=4, warp_scale=360.0, warp_strength=90.0, seed=200
    )
    medium_islands = warped_fbm_2d(
        wx, wz, scale=120.0, octaves=4, warp_scale=180.0, warp_strength=55.0, seed=210
    )
    small_islands = fbm_2d(wx, wz, scale=50.0, octaves=3, seed=220)
    channels = warped_fbm_2d(
        wx, wz, scale=90.0, octaves=5, warp_scale=250.0, warp_strength=70.0, seed=230
    )
    pools = warped_fbm_2d(
        wx, wz, scale=120.0, octaves=3, warp_scale=200.0, warp_strength=40.0, seed=240
    )
    rim = fbm_2d(wx, wz, scale=180.0, octaves=3, seed=250)

    island_noise = large_islands * 0.55 + medium_islands * 0.3 + small_islands * 0.15
    pool_depth = np.maximum(0.0, pools - 0.1) * 2.5

    # Water level must be well BELOW surrounding wilderness terrain (min ~54 at boundary).
    # Keeping a 10-block margin ensures stitching never pushes terrain below water.
    water_surface = 44.0

    # Noise > 0 → island (above water), noise < 0 → channel (below water)
    shore_t = np.clip(island_noise * 5.0 + 0.5, 0.0, 1.0)

    # Islands: 4-19 blocks above water — tall enough to be visible landmarks
    island_h = water_surface + 4.0 + np.abs(island_noise) * 14.0 + rim * 1.0
    # Water floor: 4-8 blocks below water — deep enough to feel like a lake
    floor_h = water_surface - 5.0 + channels * 2.0 - pool_depth

    # Smooth transition at shoreline
    height_basin = island_h * shore_t + floor_h * (1.0 - shore_t)
    # Rim sits above water, below wilderness — creates a natural bowl
    height_rim = 52.0 + rim * 1.5

    # Smooth basin-to-rim transition instead of hard cutoff
    basin_blend = np.clip((basin - 0.10) / 0.10, 0.0, 1.0)
    height = height_rim + (height_basin - height_rim) * basin_blend

    # Water only in basin interior where terrain dips below surface
    waterline = np.where(
        (basin_blend > 0.25) & (height < water_surface), water_surface, -1.0
    )

    # Surface selection — relative to waterline
    submerged = (waterline >= 0) & (height < waterline)
    shoreline = (waterline >= 0) & (height >= waterline) & (height < waterline + 1.5)
    island_low = (
        (waterline >= 0) & (height >= waterline + 1.5) & (height < waterline + 4.0)
    )
    island_high = (waterline >= 0) & (height >= waterline + 4.0)

    surface_id = np.full_like(height, grass_id, dtype=np.int32)
    # Deep water bottom
    surface_id = np.where(submerged & (height < waterline - 2.0), clay_id, surface_id)
    # Shallow water bottom
    surface_id = np.where(submerged & (height >= waterline - 2.0), mud_id, surface_id)
    # Shoreline band
    surface_id = np.where(shoreline, mud_id, surface_id)
    # Low island: moss or rooted dirt
    surface_id = np.where(island_low & (channels > 0.0), moss_id, surface_id)
    surface_id = np.where(island_low & (channels <= 0.0), rooted_dirt_id, surface_id)
    # High island: grass
    surface_id = np.where(island_high, grass_id, surface_id)
    # Basin interior channels that are dry
    surface_id = np.where(
        (basin > 0.5) & (channels < -0.2) & (surface_id == grass_id),
        mud_id,
        surface_id,
    )

    feature_mask = np.maximum(
        np.maximum(basin, np.abs(channels) * 0.45),
        np.maximum(0.0, island_noise) * 0.8,
    )

    area = tile_size * tile_size
    shallow_water = (waterline >= 0) & (height >= waterline - 1.2)
    biome_id = np.where(shallow_water | island_low, mangrove_biome_id, marsh_biome_id)

    # 灵泉湿地：灵气富集之地。灵脉汇入水体（灵泉眼），末法极低。
    # 水体/池塘处 qi 最高，中心盆地有一条 "灵脉线" 沿 channels 方向。
    qi_base = float(getattr(zone, "spirit_qi", 0.7))
    spring_eye = np.maximum(0.0, pools - 0.25) * 2.0  # 灵泉眼
    qi_vein_flow = np.clip(spring_eye * basin * 0.9, 0.0, 1.0)
    qi_density = 0.25 + basin * 0.35 + spring_eye * 0.30
    qi_density = np.where(waterline >= 0, qi_density + 0.12, qi_density)
    qi_density = np.clip(qi_density * (0.4 + qi_base), 0.0, 1.0)
    mofa_decay = np.clip(0.18 - basin * 0.08 - spring_eye * 0.10, 0.02, 0.35)

    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.round(waterline, 3).ravel()
    buffer.layers["biome_id"] = biome_id.ravel().astype(np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein_flow, 3).ravel()

    buffer.contributing_zones.append(zone.name)
    return buffer
