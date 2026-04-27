"""TSY 战场沉淀 profile — 密集骨堆 / 兵器林 / 血色地脉.

plan-tsy-worldgen-v1 §3.1 / §3.4 / §8 Q6.

vs 主世界 ancient_battlefield 关键差异：
- qi_density 反转：主世界 0.10-0.30 末法滞留；TSY 0.85-0.95 残灵浓厚
- 骨堆密度 ×3：被困残灵不散，骨堆不腐
- anomaly 偏 cursed_echo（4）+ blood_moon_anchor（3）
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

ZHANCHANG_DECORATIONS = (
    DecorationSpec(
        name="bone_mountain",
        kind="boulder",
        blocks=("bone_block", "dirt", "coarse_dirt"),
        size_range=(5, 10),
        rarity=0.65,
        notes="骨堆山：层层叠叠的修士遗骨，怨念不散，密度三倍于主世界古战场。",
    ),
    DecorationSpec(
        name="weapon_thicket",
        kind="tree",
        blocks=("iron_block", "copper_block", "cobwebs"),
        size_range=(3, 6),
        rarity=0.40,
        notes="兵器林：插地兵器森林般林立，部分被蜘蛛网缠绕。",
    ),
    DecorationSpec(
        name="blood_ley_line",
        kind="shrub",
        blocks=("red_concrete", "red_sand", "magma_block"),
        size_range=(2, 4),
        rarity=0.35,
        notes="血色地脉：地下血色矿脉因怨念聚集浮现地表，似干涸血河。",
    ),
    DecorationSpec(
        name="war_banner_remnant",
        kind="tree",
        blocks=("red_wool", "black_wool", "stripped_oak_log"),
        size_range=(4, 7),
        rarity=0.20,
        notes="战旗残骸：红黑两色破旗，旗杆断裂半倒。",
    ),
)

ORIGIN_CODE = {"daneng_luoluo": 1, "zongmen_yiji": 2, "zhanchang_chendian": 3, "gaoshou_sichu": 4}
DEPTH_CODE = {"shallow": 1, "mid": 2, "deep": 3}


class TsyZhanchangGenerator(TerrainProfileGenerator):
    profile_name = "tsy_zhanchang"
    extra_layers = (
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "anomaly_intensity",
        "anomaly_kind",
        "ruin_density",
        "fracture_mask",
        "flora_density",
        "flora_variant_id",
        "tsy_presence",
        "tsy_origin_id",
        "tsy_depth_tier",
    )
    ecology = EcologySpec(
        decorations=ZHANCHANG_DECORATIONS,
        ambient_effects=("phantom_clash", "blood_wind", "muffled_war_cry"),
        notes="密集骨堆 + 兵器林 + 血色地脉。色调红黑骨白，深层血池沉淀。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "TSY-side battlefield: qi_density inverted vs overworld ancient_battlefield",
            "  (TSY 0.85-0.95 residual qi vs overworld 0.10-0.30 mofa stagnation).",
            "Bone density ×3; anomaly favours cursed_echo (4) + blood_moon_anchor (3).",
        )


def fill_tsy_zhanchang_tile(
    zone: BlueprintZone,
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
) -> TileFieldBuffer:
    depth_tier = zone.worldgen.extras.get("depth_tier", "shallow")
    origin_id = ORIGIN_CODE.get(zone.worldgen.extras.get("origin", "zhanchang_chendian"), 3)
    depth_id = DEPTH_CODE.get(depth_tier, 1)

    layer_names = (
        "height", "surface_id", "subsurface_id", "water_level",
        "biome_id", "feature_mask", "boundary_weight",
        "qi_density", "mofa_decay", "qi_vein_flow",
        "anomaly_intensity", "anomaly_kind",
        "ruin_density", "fracture_mask",
        "flora_density", "flora_variant_id",
        "tsy_presence", "tsy_origin_id", "tsy_depth_tier",
    )
    buffer = TileFieldBuffer.create(tile, tile_size, layer_names)
    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    area = tile_size * tile_size

    if depth_tier == "shallow":
        base = 62.0 + fbm_2d(wx, wz, scale=140.0, octaves=3, seed=4100) * 4.0
        ruin = np.clip(0.45 + fbm_2d(wx, wz, scale=80.0, octaves=2, seed=4110) * 0.25, 0.2, 0.85)
        qi = np.clip(0.86 + fbm_2d(wx, wz, scale=120.0, octaves=2, seed=4120) * 0.06, 0.78, 1.0)
        decay = np.clip(0.14, 0.05, 0.25)
    elif depth_tier == "mid":
        base = 10.0 + ridge_2d(wx, wz, scale=70.0, octaves=4, seed=4200) * 5.0
        ruin = np.clip(0.65 + warped_fbm_2d(wx, wz, scale=80.0, octaves=3, warp_scale=120.0, warp_strength=40.0, seed=4210) * 0.20, 0.4, 0.95)
        qi = np.clip(0.90 + fbm_2d(wx, wz, scale=110.0, octaves=2, seed=4220) * 0.05, 0.82, 1.0)
        decay = np.clip(0.12, 0.06, 0.25)
    else:  # deep
        base = -24.0 + fbm_2d(wx, wz, scale=160.0, octaves=3, seed=4300) * 4.0
        ruin = np.clip(0.55 + fbm_2d(wx, wz, scale=70.0, octaves=2, seed=4310) * 0.20, 0.3, 0.85)
        qi = np.clip(0.93 + fbm_2d(wx, wz, scale=180.0, octaves=2, seed=4320) * 0.05, 0.86, 1.0)
        decay = np.clip(0.09, 0.03, 0.18)

    fracture = np.maximum(0.0, ridge_2d(wx, wz, scale=90.0, octaves=4, seed=4400 + depth_id * 100))
    qi_vein = np.clip(fracture * 0.6 + ruin * 0.3, 0.0, 1.0)

    bone_block_id = palette.ensure("bone_block")
    coarse_dirt_id = palette.ensure("coarse_dirt")
    deepslate_id = palette.ensure("deepslate")
    blackstone_id = palette.ensure("blackstone")
    red_sand_id = palette.ensure("red_sand")

    surface_id = np.full_like(base, coarse_dirt_id, dtype=np.int32)
    surface_id = np.where(ruin > 0.6, bone_block_id, surface_id)
    surface_id = np.where(fracture > 0.5, blackstone_id, surface_id)
    if depth_tier == "deep":
        surface_id = np.where(qi_vein > 0.6, red_sand_id, surface_id)

    anomaly_seed = 4500 + depth_id * 100
    anomaly_field = warped_fbm_2d(wx, wz, scale=200.0, octaves=3, warp_scale=240.0, warp_strength=70.0, seed=anomaly_seed)
    anomaly_threshold = {"shallow": 0.50, "mid": 0.40, "deep": 0.30}[depth_tier]
    anomaly_intensity = np.clip((anomaly_field - anomaly_threshold) * 3.0, 0.0, 1.0)
    # 默认 cursed_echo（4）；mid/deep 偶发 blood_moon_anchor（3）
    anomaly_kind = np.where(anomaly_intensity > 0.15, 4, 0).astype(np.int32)
    if depth_tier != "shallow":
        moon_field = fbm_2d(wx, wz, scale=240.0, octaves=2, seed=4600)
        moon_strong = (moon_field > 0.40) & (anomaly_intensity < 0.25)
        anomaly_intensity = np.where(moon_strong, np.clip(moon_field * 0.85, 0.0, 1.0), anomaly_intensity)
        anomaly_kind = np.where(moon_strong, 3, anomaly_kind)

    flora_density = np.clip(ruin * 0.65 + fracture * 0.20, 0.0, 1.0)
    flora_variant = np.zeros_like(base, dtype=np.int32)
    flora_variant = np.where(ruin > 0.55, 1, flora_variant)  # bone_mountain
    flora_variant = np.where((flora_variant == 0) & (fracture > 0.4), 2, flora_variant)  # weapon_thicket
    flora_variant = np.where((flora_variant == 0) & (qi_vein > 0.5), 3, flora_variant)  # blood_ley_line
    flora_variant = np.where((flora_variant == 0) & (ruin > 0.3), 4, flora_variant)  # war_banner_remnant

    buffer.layers["height"] = np.round(base, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, deepslate_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, 6, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(np.clip(ruin * 0.5 + fracture * 0.4, 0.0, 1.0), 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein, 3).ravel()
    buffer.layers["anomaly_intensity"] = np.round(anomaly_intensity, 3).ravel()
    buffer.layers["anomaly_kind"] = anomaly_kind.ravel().astype(np.uint8)
    buffer.layers["ruin_density"] = np.round(ruin, 3).ravel()
    buffer.layers["fracture_mask"] = np.round(fracture, 3).ravel()
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)
    buffer.layers["tsy_presence"] = np.ones(area, dtype=np.uint8)
    buffer.layers["tsy_origin_id"] = np.full(area, origin_id, dtype=np.uint8)
    buffer.layers["tsy_depth_tier"] = np.full(area, depth_id, dtype=np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
