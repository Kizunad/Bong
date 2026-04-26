"""TSY 宗门遗迹 profile — 倒塌殿宇 / 阵盘残件 / 藏书废墟.

plan-tsy-worldgen-v1 §3.2.

Y 分层（depth_tier 取自 zone.worldgen.extras["depth_tier"]，§2.2.d）：
- shallow Y∈[40,120]: 灰雾地表 + 少量柱础 + 骨堆点缀（被搜尽过）
- mid Y∈[0,40]: 主废墟 + 残墙 + 中型容器位
- deep Y∈[-40,0]: 阵盘核心 + 法阵残件 + 高密度遗物 slot
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

ZONGMEN_RUIN_DECORATIONS = (
    DecorationSpec(
        name="toppled_pillar",
        kind="boulder",
        blocks=("cracked_stone_bricks", "deepslate_bricks", "andesite"),
        size_range=(3, 6),
        rarity=0.45,
        notes="柱础残段：宗门殿宇倒塌后的石柱半埋。",
    ),
    DecorationSpec(
        name="array_disc_remnant",
        kind="crystal",
        blocks=("lodestone", "amethyst_block", "chiseled_deepslate"),
        size_range=(2, 4),
        rarity=0.18,
        notes="阵盘残片：曾经的引气阵法核心，紫晶尚有微光。",
    ),
    DecorationSpec(
        name="scripture_pile",
        kind="shrub",
        blocks=("dirt", "podzol", "soul_sand"),
        size_range=(1, 3),
        rarity=0.30,
        notes="藏经废墟：腐朽竹简化为黑土，灵识扫过隐约见字。",
    ),
    DecorationSpec(
        name="sect_stele",
        kind="boulder",
        blocks=("deepslate_bricks", "chiseled_deepslate", "soul_lantern"),
        size_range=(3, 5),
        rarity=0.20,
        notes="宗门界碑：刻有山门字样的深板岩碑，多已断裂。",
    ),
)

# 起源代号 → tsy_origin_id 编码（§4.1）
ORIGIN_CODE = {"daneng_luoluo": 1, "zongmen_yiji": 2, "zhanchang_chendian": 3, "gaoshou_sichu": 4}
DEPTH_CODE = {"shallow": 1, "mid": 2, "deep": 3}


class TsyZongmenRuinGenerator(TerrainProfileGenerator):
    profile_name = "tsy_zongmen_ruin"
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
        decorations=ZONGMEN_RUIN_DECORATIONS,
        ambient_effects=("dry_wind", "stone_creak", "distant_chant"),
        notes="倒塌殿宇 + 阵盘残件 + 藏经废墟。色调灰青，深层有阵眼微光。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Y stratified (shallow/mid/deep) by zone.worldgen.extras['depth_tier'].",
            "High qi_density + low mofa_decay (TSY signature inversion vs overworld).",
            "Deep tier hosts relic_core_slot + array_disc_remnant decorations.",
        )


def fill_tsy_zongmen_ruin_tile(
    zone: BlueprintZone,
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
) -> TileFieldBuffer:
    depth_tier = zone.worldgen.extras.get("depth_tier", "shallow")
    origin_id = ORIGIN_CODE.get(zone.worldgen.extras.get("origin", "zongmen_yiji"), 2)
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
        base = 60.0 + fbm_2d(wx, wz, scale=120.0, octaves=3, seed=2100) * 4.0
        ruin = np.clip(0.20 + fbm_2d(wx, wz, scale=80.0, octaves=2, seed=2110) * 0.25, 0.0, 0.6)
        qi = np.clip(0.85 + fbm_2d(wx, wz, scale=140.0, octaves=2, seed=2120) * 0.05, 0.7, 1.0)
        decay = np.clip(0.12 + np.abs(ridge_2d(wx, wz, scale=70.0, octaves=2, seed=2130)) * 0.08, 0.05, 0.25)
    elif depth_tier == "mid":
        base = 8.0 + ridge_2d(wx, wz, scale=60.0, octaves=4, seed=2200) * 6.0
        ruin = np.clip(0.55 + warped_fbm_2d(wx, wz, scale=80.0, octaves=3, warp_scale=120.0, warp_strength=40.0, seed=2210) * 0.25, 0.3, 0.95)
        qi = np.clip(0.88 + fbm_2d(wx, wz, scale=110.0, octaves=2, seed=2220) * 0.06, 0.75, 1.0)
        decay = np.clip(0.15 + ruin * 0.05, 0.08, 0.30)
    else:  # deep
        base = -28.0 + fbm_2d(wx, wz, scale=140.0, octaves=3, seed=2300) * 4.0
        ruin = np.clip(0.40 + fbm_2d(wx, wz, scale=70.0, octaves=2, seed=2310) * 0.20, 0.2, 0.8)
        qi = np.clip(0.92 + fbm_2d(wx, wz, scale=180.0, octaves=2, seed=2320) * 0.06, 0.85, 1.0)
        decay = np.clip(0.10 + fbm_2d(wx, wz, scale=90.0, octaves=2, seed=2330) * 0.05, 0.05, 0.20)

    fracture = np.maximum(0.0, ridge_2d(wx, wz, scale=80.0, octaves=4, seed=2400 + depth_id * 100))
    qi_vein = np.clip(fracture * 0.7 + ruin * 0.2, 0.0, 1.0)

    stone_id = palette.ensure("stone")
    deepslate_id = palette.ensure("deepslate")
    bricks_id = palette.ensure("cracked_stone_bricks") if depth_tier != "deep" else palette.ensure("deepslate")
    moss_id = palette.ensure("moss_block") if depth_tier == "shallow" else palette.ensure("deepslate")

    surface_id = np.full_like(base, stone_id, dtype=np.int32)
    surface_id = np.where(ruin > 0.4, bricks_id, surface_id)
    surface_id = np.where(fracture > 0.5, deepslate_id, surface_id)
    if depth_tier == "shallow":
        surface_id = np.where((ruin < 0.3) & (fracture < 0.2), moss_id, surface_id)

    anomaly_seed = 2500 + depth_id * 100
    anomaly_field = warped_fbm_2d(wx, wz, scale=200.0, octaves=3, warp_scale=240.0, warp_strength=70.0, seed=anomaly_seed)
    anomaly_threshold = {"shallow": 0.55, "mid": 0.45, "deep": 0.35}[depth_tier]
    anomaly_intensity = np.clip((anomaly_field - anomaly_threshold) * 3.0, 0.0, 1.0)
    anomaly_kind = np.where(anomaly_intensity > 0.15, 5, 0).astype(np.int32)
    if depth_tier == "deep":
        rift_field = fbm_2d(wx, wz, scale=160.0, octaves=2, seed=2600)
        rift_strong = (rift_field > 0.35) & (anomaly_intensity < 0.25)
        anomaly_intensity = np.where(rift_strong, np.clip(rift_field * 0.8, 0.0, 1.0), anomaly_intensity)
        anomaly_kind = np.where(rift_strong, 1, anomaly_kind)

    flora_density = np.clip(ruin * 0.55 + fracture * 0.25, 0.0, 1.0)
    flora_variant = np.zeros_like(base, dtype=np.int32)
    flora_variant = np.where(ruin > 0.4, 1, flora_variant)
    flora_variant = np.where((flora_variant == 0) & (fracture > 0.5), 4, flora_variant)
    if depth_tier == "deep":
        flora_variant = np.where(anomaly_kind == 5, 2, flora_variant)
    flora_variant = np.where((flora_variant == 0) & (ruin > 0.25), 3, flora_variant)

    buffer.layers["height"] = np.round(base, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, deepslate_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, 5, dtype=np.uint8)
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
