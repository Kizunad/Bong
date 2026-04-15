"""Ancient battlefield profile — 古战场.

Mid-level storytelling anchor. Pockmarked terrain (impact craters, fault
scars) + scattered ruined fortification layers + periodic reality warps
(blood_moon anchors, cursed_echo wells).

Exports dense `ruin_density` + a bespoke `battle_scar_mask` (repurposes
`fracture_mask` for cross-profile consistency) + the new `anomaly_*`
layer pair to seed random events and themed NPC spawns.

Qi here is complicated — battle residue boils up ley-line remnants even
while mofa coats the surface, so both qi_density and mofa_decay can be
simultaneously elevated.
"""

from __future__ import annotations

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


BATTLEFIELD_DECORATIONS = (
    DecorationSpec(
        name="broken_spear_tree",
        kind="tree",
        blocks=("stripped_spruce_log", "iron_block", "copper_block"),
        size_range=(4, 8),
        rarity=0.28,
        notes="断矛树：插入地面的铁柄长矛化为枯木，铜锈如血。",
    ),
    DecorationSpec(
        name="war_banner_post",
        kind="tree",
        blocks=("oak_log", "red_wool", "black_wool"),
        size_range=(6, 10),
        rarity=0.18,
        notes="残旗柱：旗杆+残破军旗布条，旗色红黑参半，曾属无名势力。",
    ),
    DecorationSpec(
        name="impact_crater_rim",
        kind="boulder",
        blocks=("cracked_stone_bricks", "cobblestone", "tuff"),
        size_range=(4, 9),
        rarity=0.40,
        notes="坠击坑缘：大型法术冲击痕边缘的碎石堆，向心塌陷。",
    ),
    DecorationSpec(
        name="bone_pile",
        kind="shrub",
        blocks=("bone_block", "dirt", "coarse_dirt"),
        size_range=(2, 4),
        rarity=0.55,
        notes="枯骨堆：无名修士遗骨，半埋尘土。近之心悸。",
    ),
    DecorationSpec(
        name="cursed_stele",
        kind="boulder",
        blocks=("deepslate_bricks", "chiseled_deepslate", "soul_lantern"),
        size_range=(3, 6),
        rarity=0.22,
        notes="咒碑：刻满咒文的深板岩碑，夜间有灵魂提灯微光。",
    ),
    DecorationSpec(
        name="formation_core",
        kind="crystal",
        blocks=("lodestone", "chiseled_stone_bricks", "amethyst_cluster"),
        size_range=(3, 5),
        rarity=0.12,
        notes="阵眼残核：废弃阵法的磁石核心，紫晶簇依附共振。接近触发异常。",
    ),
)


class AncientBattlefieldGenerator(TerrainProfileGenerator):
    profile_name = "ancient_battlefield"
    extra_layers = (
        "ruin_density",
        "fracture_mask",
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "flora_density",
        "flora_variant_id",
        "anomaly_intensity",
        "anomaly_kind",
    )
    ecology = EcologySpec(
        decorations=BATTLEFIELD_DECORATIONS,
        ambient_effects=("distant_bell", "blood_wind", "phantom_clash"),
        notes="古战场生态：断矛、残旗、枯骨、咒碑散布大地；重要节点可见阵眼残核"
              "触发异常（血月共鸣、时空裂缝）。色调锈红+灰黑+冷白骨。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "High ruin_density + fracture_mask + ambivalent qi/mofa "
            "(both simultaneously elevated from battle residue).",
            "anomaly_intensity / anomaly_kind drive event spawns: kinds are",
            "  1 spacetime_rift, 2 qi_turbulence, 3 blood_moon_anchor,",
            "  4 cursed_echo, 5 wild_formation.",
        )


# Shared anomaly kind encoding — mirrored in manifest / raster_check.
ANOMALY_KIND = {
    "none": 0,
    "spacetime_rift": 1,
    "qi_turbulence": 2,
    "blood_moon_anchor": 3,
    "cursed_echo": 4,
    "wild_formation": 5,
}


def fill_ancient_battlefield_tile(
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
        "ruin_density",
        "fracture_mask",
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "flora_density",
        "flora_variant_id",
        "anomaly_intensity",
        "anomaly_kind",
    )
    buffer = TileFieldBuffer.create(tile, tile_size, layer_names)
    stone_id = palette.ensure("stone")
    gravel_id = palette.ensure("gravel")
    coarse_dirt_id = palette.ensure("coarse_dirt")
    podzol_id = palette.ensure("podzol")
    blackstone_id = palette.ensure("blackstone")
    bone_block_id = palette.ensure("bone_block")
    battlefield_biome_id = 6  # reuse plateau/badlands slot

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    core = np.maximum(0.0, 1.0 - radial**1.5)

    # --- Surface: pockmarked rolling terrain ---
    base_fbm = fbm_2d(wx, wz, scale=280.0, octaves=4, seed=900)
    # Impact craters: low-frequency warped FBM threshold for circular pits.
    crater_field = warped_fbm_2d(
        wx, wz, scale=180.0, octaves=4, warp_scale=240.0, warp_strength=70.0, seed=910
    )
    crater_mask = np.clip((crater_field - 0.15) * 2.5, 0.0, 1.0) * core
    # Fracture lines from wild-formation spell blasts.
    fracture = ridge_2d(wx, wz, scale=90.0, octaves=5, seed=920)
    fracture_mask = np.maximum(0.0, fracture) * core

    height = 76.0 + core * 4.0 + base_fbm * 3.5 - crater_mask * 12.0 - fracture_mask * 4.0

    # --- Surfaces ---
    surface_id = np.full_like(height, podzol_id, dtype=np.int32)
    surface_id = np.where(crater_mask > 0.4, gravel_id, surface_id)
    surface_id = np.where(crater_mask > 0.7, stone_id, surface_id)
    surface_id = np.where(fracture_mask > 0.5, blackstone_id, surface_id)
    surface_id = np.where(
        (fracture_mask > 0.3) & (surface_id == podzol_id),
        coarse_dirt_id,
        surface_id,
    )
    # Bone scatter at high ruin density
    ruin_noise = fbm_2d(wx, wz, scale=60.0, octaves=3, seed=930)
    ruin_density = np.clip(
        0.20 + core * 0.30 + np.maximum(0.0, ruin_noise) * 0.3 + crater_mask * 0.2,
        0.0,
        1.0,
    )
    surface_id = np.where(
        (ruin_density > 0.75) & (surface_id == podzol_id),
        bone_block_id,
        surface_id,
    )

    feature_mask = np.minimum(1.0, core * 0.4 + crater_mask * 0.45 + fracture_mask * 0.4)

    # --- Qi / mofa: battle residue pattern (both elevated) ---
    qi_base = float(getattr(zone, "spirit_qi", 0.4))
    qi_vein_flow = np.clip(fracture_mask * 0.8 + crater_mask * 0.3, 0.0, 1.0)
    qi_density = np.clip(
        0.10 + qi_vein_flow * 0.35 + core * 0.10,
        0.0,
        1.0,
    ) * (0.5 + qi_base)
    qi_density = np.clip(qi_density, 0.0, 1.0)
    mofa_decay = np.clip(
        0.55 + core * 0.10 + ruin_density * 0.15 - qi_vein_flow * 0.15,
        0.25,
        0.90,
    )

    # --- Anomaly: sparse hotspots driven by noise peaks ---
    # Four independent seed grids; pick the strongest kind at each column.
    anomaly_rift = warped_fbm_2d(
        wx, wz, scale=240.0, octaves=3, warp_scale=320.0, warp_strength=80.0, seed=940
    )
    anomaly_turb = warped_fbm_2d(
        wx, wz, scale=180.0, octaves=3, warp_scale=240.0, warp_strength=60.0, seed=950
    )
    anomaly_moon = warped_fbm_2d(
        wx, wz, scale=300.0, octaves=3, warp_scale=400.0, warp_strength=100.0, seed=960
    )
    anomaly_curse = warped_fbm_2d(
        wx, wz, scale=160.0, octaves=3, warp_scale=200.0, warp_strength=55.0, seed=970
    )
    anomaly_formation = ridge_2d(wx, wz, scale=130.0, octaves=4, seed=980)

    kinds = np.stack(
        [
            np.clip((anomaly_rift - 0.35) * 3.5, 0.0, 1.0) * core,       # 1
            np.clip((anomaly_turb - 0.35) * 2.8, 0.0, 1.0) * core,       # 2
            np.clip((anomaly_moon - 0.45) * 3.5, 0.0, 1.0) * core,       # 3
            np.clip((anomaly_curse - 0.40) * 3.0, 0.0, 1.0) * core,      # 4
            np.clip((anomaly_formation - 0.55) * 3.0, 0.0, 1.0) * core,  # 5
        ],
        axis=0,
    )  # shape (5, H, W)
    best_kind_idx = np.argmax(kinds, axis=0)  # 0..4 → kind ids 1..5
    best_intensity = np.max(kinds, axis=0)
    anomaly_kind = np.where(
        best_intensity > 0.15,
        (best_kind_idx + 1).astype(np.int32),
        0,
    )
    anomaly_intensity = np.where(anomaly_kind > 0, best_intensity, 0.0)

    # --- Flora placement ---
    flora_density = np.zeros_like(height)
    flora_variant = np.zeros_like(height, dtype=np.int32)

    # Bone piles scattered across body
    bone_band = ruin_density > 0.35
    flora_variant = np.where(bone_band, 4, flora_variant)
    flora_density = np.where(bone_band, np.maximum(flora_density, 0.55), flora_density)

    # Spears in fracture zones
    spear_band = (fracture_mask > 0.35) & (flora_variant == 0)
    flora_variant = np.where(spear_band, 1, flora_variant)
    flora_density = np.where(spear_band, np.maximum(flora_density, 0.50), flora_density)

    # Craters have rims of impact rubble
    crater_rim = (crater_mask > 0.3) & (crater_mask < 0.7)
    flora_variant = np.where(crater_rim, 3, flora_variant)
    flora_density = np.where(crater_rim, np.maximum(flora_density, 0.45), flora_density)

    # Banner posts on core
    banner_band = (core > 0.6) & (base_fbm > 0.3) & (flora_variant == 0)
    flora_variant = np.where(banner_band, 2, flora_variant)
    flora_density = np.where(banner_band, np.maximum(flora_density, 0.30), flora_density)

    # Cursed steles at cursed_echo hotspots
    curse_anchor = anomaly_kind == ANOMALY_KIND["cursed_echo"]
    flora_variant = np.where(curse_anchor, 5, flora_variant)
    flora_density = np.where(curse_anchor, np.maximum(flora_density, 0.55), flora_density)

    # Formation cores at wild_formation anchors (rare + exact)
    formation_anchor = anomaly_kind == ANOMALY_KIND["wild_formation"]
    flora_variant = np.where(formation_anchor, 6, flora_variant)
    flora_density = np.where(formation_anchor, np.maximum(flora_density, 0.65), flora_density)

    flora_density = np.clip(flora_density, 0.0, 1.0)

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, battlefield_biome_id, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["ruin_density"] = np.round(ruin_density, 3).ravel()
    buffer.layers["fracture_mask"] = np.round(fracture_mask, 3).ravel()
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein_flow, 3).ravel()
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)
    buffer.layers["anomaly_intensity"] = np.round(anomaly_intensity, 3).ravel()
    buffer.layers["anomaly_kind"] = anomaly_kind.ravel().astype(np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
