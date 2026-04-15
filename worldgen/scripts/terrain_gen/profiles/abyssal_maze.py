"""Abyssal maze profile — 无垠深渊 / deep vertical cave network.

Where `cave_network` encodes a single underground plane beneath the surface,
`abyssal_maze` encodes THREE cave tiers stacked vertically:
  tier 1 (shallow) around y ≈ 32
  tier 2 (middle)  around y ≈ 0
  tier 3 (deep)    around y ≈ -32

Two new layers carry the vertical information:
  underground_tier — uint8, highest active tier at each (x,z) column (0..3)
  cavern_floor_y   — world-y of the deepest cavern floor (9999 sentinel)

Together with the existing `cave_mask` / `ceiling_height` / `entrance_mask`
layers, Rust can carve every tier by walking through all three y-bands and
reading (tier, floor_y) to decide the bottom of the lowest cavern.
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


# Abyssal flora/decor variants. ID 0 = none.
ABYSSAL_DECORATIONS = (
    DecorationSpec(
        name="xuan_jing_pillar",
        kind="crystal",
        blocks=("obsidian", "amethyst_block", "crying_obsidian"),
        size_range=(6, 14),
        rarity=0.25,
        notes="玄晶柱：黑曜石骨架 + 紫晶节点，偶嵌哭泣石滴。tier 3 专属。",
    ),
    DecorationSpec(
        name="bone_sail_tree",
        kind="tree",
        blocks=("bone_block", "quartz_stairs", "white_concrete"),
        size_range=(4, 8),
        rarity=0.35,
        notes="骨骸树：骨头块主干，树冠由石英阶如帆展开。战场遗骸化生。",
    ),
    DecorationSpec(
        name="xun_guang_mushroom",
        kind="mushroom",
        blocks=("shroomlight", "crimson_hyphae", "red_mushroom_block"),
        size_range=(2, 4),
        rarity=0.60,
        notes="熏光蘑：发光菌盖 + 绯红菌丝，tier 1/2 成片生长，照明区域。",
    ),
    DecorationSpec(
        name="yuan_ni_ebony",
        kind="tree",
        blocks=("blackstone", "basalt", "polished_basalt"),
        size_range=(9, 15),
        rarity=0.18,
        notes="渊泥黑檀：黑石躯干+玄武岩枝冠，巨型伞状。最深处的守望者。",
    ),
    DecorationSpec(
        name="gu_teng_creeper",
        kind="shrub",
        blocks=("weeping_vines", "crimson_hyphae", "warped_wart_block"),
        size_range=(3, 7),
        rarity=0.50,
        notes="咕藤：自顶而下的悬挂藤蔓群，在洞顶自然垂落。",
    ),
    DecorationSpec(
        name="duo_yan_boulder",
        kind="boulder",
        blocks=("cobbled_deepslate", "tuff", "deepslate"),
        size_range=(3, 6),
        rarity=0.45,
        notes="堕岩：深板岩巨石团，散布三层地板，风化有裂纹。",
    ),
)


class AbyssalMazeGenerator(TerrainProfileGenerator):
    profile_name = "abyssal_maze"
    extra_layers = (
        "cave_mask",
        "ceiling_height",
        "entrance_mask",
        "underground_tier",
        "cavern_floor_y",
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "flora_density",
        "flora_variant_id",
    )
    ecology = EcologySpec(
        decorations=ABYSSAL_DECORATIONS,
        ambient_effects=("dripstone_drip", "low_rumble", "faint_bone_whisper"),
        notes="无垠深渊生态：三层递进。tier 1 熏光蘑+咕藤构成发光苔原；"
              "tier 2 骨骸树+堕岩围成阴郁柱厅；tier 3 玄晶柱+渊泥黑檀，"
              "每一柱都是地标。整体色调：黑曜+骨白+熏光橙+紫晶冷光。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Three vertically stacked cave tiers (y≈32, 0, -32); "
            "underground_tier/cavern_floor_y tell Rust where to carve.",
            "Qi grows with depth — deepest tier has highest vein flow "
            "(藏脉在渊底) while mofa stays moderate (岩层封闭).",
            "Flora placement by tier: t1 → 熏光蘑(3)/咕藤(5), "
            "t2 → 骨骸树(2)/堕岩(6), t3 → 玄晶柱(1)/渊泥黑檀(4).",
        )


# Tier constants — shared with Rust consumers via raster docs.
TIER_FLOOR_Y = {1: 28.0, 2: -4.0, 3: -36.0}


def fill_abyssal_maze_tile(
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
            "cave_mask",
            "ceiling_height",
            "entrance_mask",
            "underground_tier",
            "cavern_floor_y",
            "qi_density",
            "mofa_decay",
            "qi_vein_flow",
            "flora_density",
            "flora_variant_id",
        ),
    )
    stone_id = palette.ensure("stone")
    deepslate_id = palette.ensure("deepslate")
    gravel_id = palette.ensure("gravel")
    coarse_dirt_id = palette.ensure("coarse_dirt")
    cave_biome_id = 5

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    cluster = np.maximum(0.0, 1.0 - radial**1.4)

    # --- Surface ---
    surface_fbm = fbm_2d(wx, wz, scale=220.0, octaves=3, seed=800)
    sinkhole = warped_fbm_2d(
        wx, wz, scale=160.0, octaves=4, warp_scale=220.0, warp_strength=60.0, seed=810
    )
    entrance_mask = np.clip(
        cluster * 0.7 - np.abs(sinkhole - 0.18) * 1.8, 0.0, 1.0
    )
    sink_depth = entrance_mask * 16.0
    height = 76.0 + surface_fbm * 3.5 - sink_depth

    # --- Three tier noise fields (independent seeds for varied layouts) ---
    tier1 = warped_fbm_2d(
        wx, wz, scale=140.0, octaves=4, warp_scale=200.0, warp_strength=50.0, seed=820
    )
    tier2 = warped_fbm_2d(
        wx, wz, scale=120.0, octaves=5, warp_scale=180.0, warp_strength=45.0, seed=830
    )
    tier3 = warped_fbm_2d(
        wx, wz, scale=110.0, octaves=5, warp_scale=160.0, warp_strength=40.0, seed=840
    )

    # Tier activation thresholds — deeper tiers require stronger cluster
    # support, keeping the deep abyss rare and meaningful.
    tier1_active = (tier1 * cluster) > 0.08
    tier2_active = (tier2 * cluster) > 0.12
    tier3_active = (tier3 * cluster) > 0.18

    # underground_tier: highest active tier number (deeper wins).
    tier = np.zeros_like(height, dtype=np.int32)
    tier = np.where(tier1_active, 1, tier)
    tier = np.where(tier2_active, 2, tier)
    tier = np.where(tier3_active, 3, tier)

    # cavern_floor_y: the floor y of the DEEPEST active tier (sentinel 9999).
    floor_y = np.full_like(height, 9999.0)
    floor_y = np.where(tier == 1, TIER_FLOOR_Y[1], floor_y)
    floor_y = np.where(tier == 2, TIER_FLOOR_Y[2], floor_y)
    floor_y = np.where(tier == 3, TIER_FLOOR_Y[3], floor_y)

    # cave_mask: how intensely this column is carved (any tier contributes).
    cave_mask = np.clip(
        np.maximum(0.0, tier1) * 0.45
        + np.maximum(0.0, tier2) * 0.35
        + np.maximum(0.0, tier3) * 0.40,
        0.0,
        1.0,
    ) * cluster

    # ceiling_height: combined void volume above the deepest floor.
    active_count = (
        tier1_active.astype(np.float64)
        + tier2_active.astype(np.float64)
        + tier3_active.astype(np.float64)
    )
    ceiling_height = np.maximum(10.0, 18.0 + active_count * 14.0)

    # --- Surface block selection ---
    surface_id = np.full_like(height, stone_id, dtype=np.int32)
    surface_id = np.where(entrance_mask > 0.5, gravel_id, surface_id)
    surface_id = np.where(
        (sinkhole < -0.3) & (surface_id == stone_id), coarse_dirt_id, surface_id
    )
    surface_id = np.where(
        (cave_mask > 0.6) & (surface_id == stone_id), deepslate_id, surface_id
    )

    feature_mask = np.minimum(
        1.0, cluster * 0.5 + entrance_mask * 0.45 + cave_mask * 0.3
    )

    # --- Qi / mofa: deep = high hidden qi, moderate mofa ---
    qi_base = float(getattr(zone, "spirit_qi", 0.55))
    depth_bonus = np.where(tier == 3, 0.40, np.where(tier == 2, 0.22, np.where(tier == 1, 0.10, 0.0)))
    qi_density = np.clip(
        0.15 + depth_bonus + cave_mask * 0.10,
        0.0,
        1.0,
    ) * (0.5 + qi_base)
    qi_density = np.clip(qi_density, 0.0, 1.0)
    # Vein flow spikes in tier 3 (深渊藏脉).
    qi_vein_flow = np.clip(
        np.where(tier == 3, 0.85, np.where(tier == 2, 0.45, 0.0)) * cluster,
        0.0,
        1.0,
    )
    mofa_decay = np.clip(
        0.40 + (1.0 - active_count / 3.0) * 0.10 - depth_bonus * 0.15,
        0.15,
        0.70,
    )

    # --- Flora placement per tier (variant 1..6 match ABYSSAL_DECORATIONS) ---
    # Defaults to 0 (no flora). Carve tier-specific variants where tier active.
    flora_density = np.zeros_like(height)
    flora_variant = np.zeros_like(height, dtype=np.int32)

    tier_noise = fbm_2d(wx, wz, scale=90.0, octaves=3, seed=850)

    # tier 1 (shallow y≈28): xun_guang_mushroom (3) + gu_teng_creeper (5)
    flora_variant = np.where(
        tier_active := tier1_active & (tier == 1),
        np.where(tier_noise > 0.0, 3, 5),
        flora_variant,
    )
    flora_density = np.where(tier_active, np.maximum(flora_density, 0.55 + cave_mask * 0.20), flora_density)

    # tier 2 (middle y≈-4): bone_sail_tree (2) + duo_yan_boulder (6)
    tier_active = tier2_active & (tier == 2)
    flora_variant = np.where(
        tier_active,
        np.where(tier_noise > 0.15, 2, 6),
        flora_variant,
    )
    flora_density = np.where(tier_active, np.maximum(flora_density, 0.45 + cave_mask * 0.25), flora_density)

    # tier 3 (deep y≈-36): xuan_jing_pillar (1) + yuan_ni_ebony (4)
    tier_active = tier3_active & (tier == 3)
    flora_variant = np.where(
        tier_active,
        np.where(tier_noise > 0.25, 1, 4),
        flora_variant,
    )
    flora_density = np.where(tier_active, np.maximum(flora_density, 0.60 + cave_mask * 0.20), flora_density)

    # Surface entrance zone: light scatter of duo_yan_boulder so the approach
    # advertises what's below.
    surface_entrance = (entrance_mask > 0.3) & (tier == 0)
    flora_variant = np.where(surface_entrance & (flora_variant == 0), 6, flora_variant)
    flora_density = np.where(surface_entrance, np.maximum(flora_density, entrance_mask * 0.5), flora_density)

    flora_density = np.clip(flora_density, 0.0, 1.0)

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, deepslate_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, cave_biome_id, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["cave_mask"] = np.round(cave_mask, 3).ravel()
    buffer.layers["ceiling_height"] = np.round(ceiling_height, 3).ravel()
    buffer.layers["entrance_mask"] = np.round(entrance_mask, 3).ravel()
    buffer.layers["underground_tier"] = tier.ravel().astype(np.uint8)
    buffer.layers["cavern_floor_y"] = np.round(floor_y, 3).ravel()
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein_flow, 3).ravel()
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
