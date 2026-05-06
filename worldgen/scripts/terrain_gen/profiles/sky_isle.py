"""Sky isle profile — 九霄浮岛 / floating islands overlay.

The ground itself is a quiet open plain (so characters can look up at the
isles); the vertical story is carried by three layers:
  sky_island_mask      — per-column "how strong is the floating rock above"
  sky_island_base_y    — world-y of the isle's bottom face (9999 = none)
  sky_island_thickness — vertical extent of the isle (carved downward from top)

The Rust runtime is expected to read these three layers together:
  if mask >= 0.2:
      emit a stone/cloud_stone block column at
      [base_y .. base_y + thickness_blocks], Minecraft-style overhang.

Isles are clustered in sparse archipelagos using two-scale warped FBM so
they feel like meaningful landmarks, not uniform specks of confetti.
"""

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


# Sky isle flora variants — each gets a slot in flora_variant_id.
# 0 is reserved for "no decoration".
SKY_ISLE_DECORATIONS = (
    DecorationSpec(
        name="ling_yu_tree",
        kind="tree",
        blocks=("stripped_birch_log", "flowering_azalea_leaves", "glow_lichen"),
        size_range=(7, 12),
        rarity=0.18,
        notes="灵玉树：浅木树干与带光花叶，夜间似有星光。生于浮岛顶面。",
    ),
    DecorationSpec(
        name="yun_lan_shrub",
        kind="shrub",
        blocks=("azalea_leaves", "flowering_azalea_leaves", "glow_lichen"),
        size_range=(1, 2),
        rarity=0.35,
        notes="云兰：低矮花叶丛，附发光地衣。稀疏生长于地面。",
    ),
    DecorationSpec(
        name="yu_pu_boulder",
        kind="boulder",
        blocks=("calcite", "moss_block", "smooth_quartz"),
        size_range=(2, 4),
        rarity=0.18,
        notes="玉璞石：方解石-石英混合巨石，偶尔开裂露出内部灵晶。",
    ),
    DecorationSpec(
        name="tian_mai_crystal",
        kind="crystal",
        blocks=("amethyst_cluster", "amethyst_block", "budding_amethyst"),
        size_range=(3, 6),
        rarity=0.22,
        notes="天脉水晶：浮岛底面垂挂的紫晶簇，与 qi_vein_flow 对齐生长。",
    ),
    DecorationSpec(
        name="fei_yu_bamboo",
        kind="tree",
        blocks=("bamboo_block", "stripped_bamboo_block", "azalea_leaves"),
        size_range=(5, 9),
        rarity=0.30,
        notes="飞羽竹：翠绿竹段间嵌翡翠节点，风中轻响。喜生浮岛边缘。",
    ),
)


class SkyIsleGenerator(TerrainProfileGenerator):
    profile_name = "sky_isle"
    extra_layers = (
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "sky_island_mask",
        "sky_island_base_y",
        "sky_island_thickness",
        "flora_density",
        "flora_variant_id",
    )
    ecology = EcologySpec(
        decorations=SKY_ISLE_DECORATIONS,
        ambient_effects=("qi_particles", "falling_petals", "faint_wind_chime"),
        notes="九霄浮岛生态：地面草甸点缀云兰灌丛与玉璞巨石；浮岛顶生灵玉树、"
              "飞羽竹，底面悬天脉水晶。整体冷色+光洁+灵光粒子，气韵出尘。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Ground is a meditative open plain; the vertical drama lives "
            "above y=300 as sky_island_* layers.",
            "Rust runtime must gate rendering on sky_island_mask >= 0.2; "
            "otherwise base_y sentinel 9999 will produce void overhangs.",
            "Flora placement: ground uses variants 2-3, island tops route "
            "variants 1/5 to sky_island top y, and island rims route variant 4 "
            "to the underside.",
        )


def fill_sky_isle_tile(
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
            "sky_island_mask",
            "sky_island_base_y",
            "sky_island_thickness",
            "flora_density",
            "flora_variant_id",
        ),
    )
    grass_id = palette.ensure("grass_block")
    moss_id = palette.ensure("moss_block")
    stone_id = palette.ensure("stone")
    calcite_id = palette.ensure("calcite")
    gravel_id = palette.ensure("gravel")
    # Use meadow biome for ground-level tranquility.
    meadow_biome_id = 4
    flower_forest_biome_id = 11

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    heartland = np.maximum(0.0, 1.0 - radial**1.6)

    # --- Ground: gentle meditative plain ---
    rolling = fbm_2d(wx, wz, scale=360.0, octaves=3, seed=700)
    height = 72.0 + heartland * 2.5 + rolling * 1.2

    surface_id = np.full_like(height, grass_id, dtype=np.int32)
    surface_id = np.where(rolling > 0.4, moss_id, surface_id)
    surface_id = np.where(rolling < -0.55, gravel_id, surface_id)

    # --- Sky isles: two-scale archipelago ---
    # Large-scale cluster locations (big island groups). FBM returns values
    # roughly in [-0.7, 0.7] after normalization, so thresholds sit near 0.
    cluster_field = warped_fbm_2d(
        wx, wz, scale=320.0, octaves=3, warp_scale=480.0, warp_strength=100.0, seed=710
    )
    # Mid-scale individual isle cores.
    isle_field = warped_fbm_2d(
        wx, wz, scale=160.0, octaves=4, warp_scale=220.0, warp_strength=55.0, seed=720
    )
    # Fine detail: carves irregular isle silhouettes (not circular blobs).
    detail = fbm_2d(wx, wz, scale=70.0, octaves=3, seed=730)

    # Cluster gate: sparse but present. Shift cluster_field into [0, 1] via
    # a soft threshold around -0.1, so ~40% of heartland columns qualify.
    cluster_gate = np.clip((cluster_field + 0.1) * 1.6, 0.0, 1.0) * heartland
    isle_raw = np.clip((isle_field + 0.05) * 1.4, 0.0, 1.0) * cluster_gate
    # Detail carves the edge.
    silhouette = np.clip(isle_raw + detail * 0.15, 0.0, 1.0)

    # Keep only columns with decent core strength; scale remaining to [0, 1].
    sky_island_mask = np.where(silhouette > 0.10, silhouette, 0.0)
    sky_island_mask = np.clip(sky_island_mask * 1.5, 0.0, 1.0)
    # Store the raw core intensity for thickness sampling before masking.
    isle_core = isle_raw

    # Altitude varies between 260 and 340 — higher isles in the cluster core
    # so archipelagos feel layered rather than a flat ceiling.
    altitude_warp = fbm_2d(wx, wz, scale=600.0, octaves=2, seed=740)
    base_y = 270.0 + cluster_field * 40.0 + altitude_warp * 24.0
    base_y = np.clip(base_y, 240.0, 360.0)
    # Sentinel 9999 where no isle exists (so `minimum` blend leaves it alone).
    sky_island_base_y = np.where(sky_island_mask > 0.01, base_y, 9999.0)

    # Thickness: 8..30 blocks. Decouple from mask so core columns stay thick
    # even where the mask has been slightly attenuated at the silhouette edge.
    sky_island_thickness = np.where(
        sky_island_mask > 0.05,
        8.0 + isle_core * 22.0,
        0.0,
    )

    # --- Qi / mofa semantic: sky isles are high-qi, very low decay ---
    qi_base = float(getattr(zone, "spirit_qi", 0.8))
    # Ground qi: moderate; sky isles lift it significantly when directly
    # beneath an isle shadow (灵气沐浴).
    shadow = sky_island_mask * 0.6
    qi_density = np.clip(
        0.20 + heartland * 0.10 + shadow,
        0.0,
        1.0,
    ) * (0.4 + qi_base)
    qi_density = np.clip(qi_density, 0.0, 1.0)
    mofa_decay = np.clip(0.12 - heartland * 0.05 - shadow * 0.08, 0.02, 0.30)
    # Vein flow concentrates under isle cores (天脉垂降).
    qi_vein_flow = np.clip(sky_island_mask * 0.85, 0.0, 1.0)

    feature_mask = np.minimum(
        1.0, heartland * 0.4 + sky_island_mask * 0.7,
    )
    biome_id = np.where(
        sky_island_mask > 0.1, flower_forest_biome_id, meadow_biome_id
    )

    # Surface: beneath isle shadows, ground looks enriched (moss / calcite).
    surface_id = np.where(sky_island_mask > 0.3, moss_id, surface_id)
    surface_id = np.where(sky_island_mask > 0.6, calcite_id, surface_id)

    poi_clearance = np.zeros_like(height)
    for poi in zone.pois:
        if poi.kind not in {"spirit_font", "shrine", "ruin"}:
            continue
        poi_x, _, poi_z = poi.pos_xyz
        dist = np.sqrt((wx - poi_x) ** 2 + (wz - poi_z) ** 2)
        poi_clearance = np.maximum(poi_clearance, np.clip(1.0 - dist / 96.0, 0.0, 1.0))

    # --- Flora placement indices (variant_id 1..5 match SKY_ISLE_DECORATIONS) ---
    # Rust routes sky-isle variants 1/5 to the island top and variant 4 to the
    # island underside; ground variants 2-3 stay on the terrain surface.
    flora_density = np.zeros_like(height)
    flora_variant = np.zeros_like(height, dtype=np.int32)

    # Ground decorations
    ground_flora = heartland * 0.10 + np.clip(rolling + 0.3, 0.0, 1.0) * 0.06
    ground_flora *= 1.0 - poi_clearance
    flora_density = np.maximum(flora_density, ground_flora)
    flora_variant = np.where(ground_flora > 0.13, 2, flora_variant)

    boulder_band = (detail > 0.60) & (ground_flora > 0.10)
    flora_variant = np.where(boulder_band, 3, flora_variant)
    flora_density = np.where(boulder_band, np.maximum(flora_density, 0.22), flora_density)

    isle_top = sky_island_mask > 0.35
    flora_density = np.where(isle_top, np.maximum(flora_density, sky_island_mask * 0.55), flora_density)
    flora_variant = np.where(isle_top & (altitude_warp > 0.0), 1, flora_variant)
    flora_variant = np.where(isle_top & (altitude_warp <= 0.0), 5, flora_variant)

    isle_rim = (sky_island_mask > 0.10) & (sky_island_mask <= 0.35)
    flora_variant = np.where(isle_rim, 4, flora_variant)
    flora_density = np.where(isle_rim, np.maximum(flora_density, 0.22), flora_density)

    flora_density = np.clip(flora_density, 0.0, 1.0)

    area = tile_size * tile_size
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
    buffer.layers["sky_island_mask"] = np.round(sky_island_mask, 3).ravel()
    buffer.layers["sky_island_base_y"] = np.round(sky_island_base_y, 3).ravel()
    buffer.layers["sky_island_thickness"] = np.round(sky_island_thickness, 3).ravel()
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
