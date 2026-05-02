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


PSEUDO_VEIN_DECORATIONS = (
    DecorationSpec(
        name="false_spirit_lotus",
        kind="flower",
        blocks=("pink_petals", "warped_wart_block", "amethyst_cluster"),
        size_range=(1, 2),
        rarity=0.70,
        notes="伪灵莲：粉花瓣 + 扭曲菌块基底 + 紫晶花蕊。看似灵草，实则一摘即化粉。",
    ),
    DecorationSpec(
        name="phantom_qi_pillar",
        kind="crystal",
        blocks=("amethyst_cluster", "purple_stained_glass", "soul_lantern"),
        size_range=(4, 7),
        rarity=0.30,
        notes="幻灵柱：紫晶 + 紫玻 + 灵魂提灯，中心区域缓慢呼吸式发光。",
    ),
    DecorationSpec(
        name="lush_grass_overlay",
        kind="shrub",
        blocks=("flowering_azalea_leaves", "pink_tulip", "lily_of_the_valley"),
        size_range=(1, 2),
        rarity=0.85,
        notes="异常茂盛草：荒野中突现的花海，是伪灵脉最显眼识别。",
    ),
    DecorationSpec(
        name="tiandao_seal_stele",
        kind="boulder",
        blocks=("sculk", "sculk_vein", "soul_sand"),
        size_range=(2, 3),
        rarity=0.15,
        notes="天道封纹石：凝脉以上可读出模糊瓮字，是此地唯一警示。",
    ),
    DecorationSpec(
        name="false_vein_well",
        kind="boulder",
        blocks=("prismarine", "sea_lantern", "tube_coral_block"),
        size_range=(2, 4),
        rarity=0.20,
        notes="伪泉眼：蓝绿光小水洼，看似真灵眼，是凝脉突破诱饵。",
    ),
)


class PseudoVeinOasisGenerator(TerrainProfileGenerator):
    profile_name = "pseudo_vein_oasis"
    extra_layers = (
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "flora_density",
        "flora_variant_id",
        "neg_pressure",
        "anomaly_intensity",
        "anomaly_kind",
    )
    ecology = EcologySpec(
        decorations=PSEUDO_VEIN_DECORATIONS,
        ambient_effects=("false_qi_shimmer", "hungry_ring_silence", "distant_thunder_hint"),
        notes="伪灵脉绿洲生态：中心是真的高灵气诱饵，外缘饥渴圈归零植被，消散后由 server 写入负灵 hot-spot。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Transient pseudo vein zone; blueprint instances are injected by server/agent rather than fixed in the canonical map.",
            "Qi profile pins 0.60 body qi, 0.80 false well core qi, and 0.08 hungry-ring compensation.",
        )


def fill_pseudo_vein_oasis_tile(
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
            "neg_pressure",
            "anomaly_intensity",
            "anomaly_kind",
        ),
    )

    grass_id = palette.ensure("grass_block")
    moss_id = palette.ensure("moss_block")
    azalea_id = palette.ensure("flowering_azalea_leaves")
    warped_id = palette.ensure("warped_wart_block")
    prismarine_id = palette.ensure("prismarine")
    coarse_dirt_id = palette.ensure("coarse_dirt")
    gravel_id = palette.ensure("gravel")
    stone_id = palette.ensure("stone")
    oasis_biome_id = 11
    hungry_ring_biome_id = 12

    center_x, center_z = zone.center_xz
    core_radius = float(zone.worldgen.extras.get("core_radius", 60))
    rim_radius = float(zone.worldgen.extras.get("rim_radius", 120))
    core_radius = max(core_radius, 1.0)
    rim_radius = max(rim_radius, core_radius)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dist = np.sqrt((wx - center_x) ** 2 + (wz - center_z) ** 2)
    t = dist / core_radius
    core = np.clip(1.0 - t, 0.0, 1.0)
    oasis_mask = t <= 1.0
    hungry_ring = (t > 1.0) & (t <= (rim_radius / core_radius))

    mound_noise = warped_fbm_2d(
        wx, wz, scale=130.0, octaves=4, warp_scale=180.0, warp_strength=34.0, seed=610
    )
    petal_noise = fbm_2d(wx, wz, scale=34.0, octaves=3, seed=620)
    height = 69.0 + core * 8.0 + np.maximum(0.0, petal_noise) * 1.6 + mound_noise * 1.4
    height = np.where(hungry_ring, 67.5 + petal_noise * 0.8, height)
    height = np.where(t > (rim_radius / core_radius), 66.5 + mound_noise * 0.5, height)

    waterline = np.where(t < 0.18, 70.0, -1.0)
    waterline = np.where(height >= waterline, -1.0, waterline)

    qi_density = np.full_like(height, 0.12)
    qi_density = np.where(hungry_ring, 0.08, qi_density)
    qi_density = np.where((t >= 0.7) & (t <= 1.0), 0.25, qi_density)
    qi_density = np.where((t >= 0.2) & (t < 0.7), 0.60, qi_density)
    qi_density = np.where(t < 0.2, 0.80, qi_density)

    mofa_decay = np.full_like(height, 0.40)
    mofa_decay = np.where(hungry_ring, 0.55, mofa_decay)
    mofa_decay = np.where((t >= 0.7) & (t <= 1.0), 0.20, mofa_decay)
    mofa_decay = np.where((t >= 0.2) & (t < 0.7), 0.10, mofa_decay)
    mofa_decay = np.where(t < 0.2, 0.05, mofa_decay)

    qi_vein_flow = np.zeros_like(height)
    qi_vein_flow = np.where((t >= 0.7) & (t <= 1.0), 0.10, qi_vein_flow)
    qi_vein_flow = np.where((t >= 0.2) & (t < 0.7), 0.50, qi_vein_flow)
    qi_vein_flow = np.where(t < 0.2, 0.95, qi_vein_flow)

    flora_density = np.zeros_like(height)
    flora_density = np.where((t >= 0.7) & (t <= 1.0), 0.45, flora_density)
    flora_density = np.where((t >= 0.2) & (t < 0.7), 0.85, flora_density)
    flora_density = np.where(t < 0.2, 0.85, flora_density)
    flora_density = np.where(hungry_ring, 0.0, flora_density)

    surface_id = np.full_like(height, coarse_dirt_id, dtype=np.int32)
    surface_id = np.where(t < 1.0, grass_id, surface_id)
    surface_id = np.where((t < 0.75) & (petal_noise > -0.1), moss_id, surface_id)
    surface_id = np.where((t < 0.55) & (petal_noise > 0.18), azalea_id, surface_id)
    surface_id = np.where((t < 0.35) & (petal_noise < -0.2), warped_id, surface_id)
    surface_id = np.where(t < 0.16, prismarine_id, surface_id)
    surface_id = np.where(hungry_ring & (petal_noise < 0.0), gravel_id, surface_id)

    feature_mask = np.clip(core * 0.65 + (flora_density > 0.0) * 0.18, 0.0, 1.0)
    anomaly_intensity = np.where(oasis_mask, np.clip((1.0 - t) * 0.55 + 0.10, 0.0, 0.65), 0.0)
    anomaly_kind = np.where(anomaly_intensity > 0.02, 2, 0)
    biome_id = np.where(hungry_ring, hungry_ring_biome_id, oasis_biome_id)

    # Local decoration ids: 1 lotus / 2 pillar / 3 grass / 4 seal stele / 5 false well.
    flora_variant = np.zeros_like(height, dtype=np.int32)
    flora_variant = np.where((t >= 0.85) & (t <= 1.0), 4, flora_variant)
    flora_variant = np.where((t >= 0.58) & (t < 0.85), 3, flora_variant)
    flora_variant = np.where((t >= 0.32) & (t < 0.58), 1, flora_variant)
    flora_variant = np.where((t >= 0.16) & (t < 0.32), 2, flora_variant)
    flora_variant = np.where(t < 0.16, 5, flora_variant)
    flora_variant = np.where(flora_density <= 0.0, 0, flora_variant)

    area = tile_size * tile_size
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
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)
    buffer.layers["neg_pressure"] = np.zeros(area, dtype=np.float64)
    buffer.layers["anomaly_intensity"] = np.round(anomaly_intensity, 3).ravel()
    buffer.layers["anomaly_kind"] = anomaly_kind.ravel().astype(np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
