"""TSY 近代高手死处 profile — 单栋茅屋 / 坟冢 / 日常器物.

plan-tsy-worldgen-v1 §3.1 / §3.4.

差异于其他三 profile：
- 简化结构：单建筑中心+周围 50 格农田，Y 分层最浅
- shallow 平地、mid 半山腰、deep 山洞修炼室
- anomaly_intensity 最低（0.0-0.3，近代修炼者残留少）
- 无 fracture_mask（无大破坏），加 flora_density 高（农作物）
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

GAOSHOU_DECORATIONS = (
    DecorationSpec(
        name="thatched_hermitage",
        kind="boulder",
        blocks=("hay_block", "oak_planks", "spruce_planks"),
        size_range=(4, 7),
        rarity=0.20,
        notes="单栋茅屋：高手隐居所，茅草顶+木墙，门半开。",
    ),
    DecorationSpec(
        name="lone_grave_mound",
        kind="boulder",
        blocks=("gravel", "podzol", "dead_bush"),
        size_range=(2, 4),
        rarity=0.30,
        notes="孤坟：墓主之坟，碑石半埋，覆苔腐草。",
    ),
    DecorationSpec(
        name="daily_artifact_cache",
        kind="shrub",
        blocks=("barrel", "iron_ingot", "glass_bottle"),
        size_range=(1, 3),
        rarity=0.45,
        notes="日常器物：木桶、铁锭、玻璃瓶散落，生活气息浓。",
    ),
    DecorationSpec(
        name="abandoned_weiqi_board",
        kind="shrub",
        blocks=("white_concrete", "black_concrete", "bamboo"),
        size_range=(1, 2),
        rarity=0.15,
        notes="残棋盘：黑白对弈未终局，棋子半散。",
    ),
)

ORIGIN_CODE = {"daneng_luoluo": 1, "zongmen_yiji": 2, "zhanchang_chendian": 3, "gaoshou_sichu": 4}
DEPTH_CODE = {"shallow": 1, "mid": 2, "deep": 3}


class TsyGaoshouHermitageGenerator(TerrainProfileGenerator):
    profile_name = "tsy_gaoshou_hermitage"
    extra_layers = (
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "anomaly_intensity",
        "anomaly_kind",
        "ruin_density",
        "flora_density",
        "flora_variant_id",
        "tsy_presence",
        "tsy_origin_id",
        "tsy_depth_tier",
    )
    ecology = EcologySpec(
        decorations=GAOSHOU_DECORATIONS,
        ambient_effects=("bird_chirp_distant", "soft_breeze", "tea_kettle_whistle"),
        notes="单栋茅屋 + 孤坟 + 日常器物。色调淡黄+棕，气息生活化。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Simplest TSY profile: single-building center + 50-block farmland ring.",
            "shallow: flat / mid: hillside / deep: meditation cave.",
            "Lowest anomaly_intensity; recent practitioner left little chaotic residue.",
        )


def fill_tsy_gaoshou_hermitage_tile(
    zone: BlueprintZone,
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
) -> TileFieldBuffer:
    depth_tier = zone.worldgen.extras.get("depth_tier", "shallow")
    origin_id = ORIGIN_CODE.get(zone.worldgen.extras.get("origin", "gaoshou_sichu"), 4)
    depth_id = DEPTH_CODE.get(depth_tier, 1)

    layer_names = (
        "height", "surface_id", "subsurface_id", "water_level",
        "biome_id", "feature_mask", "boundary_weight",
        "qi_density", "mofa_decay", "qi_vein_flow",
        "anomaly_intensity", "anomaly_kind",
        "ruin_density",
        "flora_density", "flora_variant_id",
        "tsy_presence", "tsy_origin_id", "tsy_depth_tier",
    )
    buffer = TileFieldBuffer.create(tile, tile_size, layer_names)
    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    area = tile_size * tile_size

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    core = np.maximum(0.0, 1.0 - radial**1.5)

    if depth_tier == "shallow":
        # 平地，茅屋+农田
        base = 64.0 + fbm_2d(wx, wz, scale=200.0, octaves=2, seed=5100) * 1.5
        ruin = np.clip(0.10 + core * 0.20, 0.0, 0.4)
        qi = np.clip(0.84 + core * 0.04, 0.78, 0.95)
        decay = np.clip(0.18, 0.10, 0.25)
    elif depth_tier == "mid":
        # 半山腰
        base = 18.0 + fbm_2d(wx, wz, scale=140.0, octaves=3, seed=5200) * 4.0 + core * 6.0
        ruin = np.clip(0.20 + core * 0.30, 0.05, 0.55)
        qi = np.clip(0.88 + core * 0.05, 0.80, 1.0)
        decay = np.clip(0.13, 0.06, 0.22)
    else:  # deep
        # 山洞修炼室
        base = -18.0 + fbm_2d(wx, wz, scale=120.0, octaves=2, seed=5300) * 3.0 - core * 4.0
        ruin = np.clip(0.25 + core * 0.20, 0.10, 0.55)
        qi = np.clip(0.92 + core * 0.05, 0.85, 1.0)
        decay = np.clip(0.10, 0.05, 0.18)

    qi_vein = np.clip(core * 0.5 + ruin * 0.2, 0.0, 1.0)

    grass_id = palette.ensure("grass_block") if depth_tier == "shallow" else palette.ensure("podzol")
    podzol_id = palette.ensure("podzol")
    deepslate_id = palette.ensure("deepslate")
    moss_id = palette.ensure("moss_block")

    surface_id = np.full_like(base, grass_id, dtype=np.int32)
    if depth_tier == "shallow":
        surface_id = np.where(core > 0.6, podzol_id, surface_id)  # 中心农田泥土
    elif depth_tier == "deep":
        surface_id = np.full_like(base, deepslate_id, dtype=np.int32)
        surface_id = np.where(core > 0.5, moss_id, surface_id)

    # 异常稀疏
    anomaly_seed = 5500 + depth_id * 100
    anomaly_field = warped_fbm_2d(wx, wz, scale=240.0, octaves=2, warp_scale=300.0, warp_strength=80.0, seed=anomaly_seed)
    anomaly_threshold = {"shallow": 0.65, "mid": 0.55, "deep": 0.50}[depth_tier]
    anomaly_intensity = np.clip((anomaly_field - anomaly_threshold) * 2.5, 0.0, 0.45)
    # qi_turbulence（2）主导，少量 wild_formation（5）
    anomaly_kind = np.where(anomaly_intensity > 0.15, 2, 0).astype(np.int32)

    flora_density = np.clip(0.20 + ruin * 0.55, 0.0, 1.0)
    flora_variant = np.zeros_like(base, dtype=np.int32)
    if depth_tier == "shallow":
        flora_variant = np.where(core > 0.7, 1, flora_variant)  # thatched_hermitage（中心）
        flora_variant = np.where((flora_variant == 0) & (core > 0.3), 3, flora_variant)  # daily_artifact
    elif depth_tier == "mid":
        flora_variant = np.where(core > 0.5, 2, flora_variant)  # lone_grave_mound
        flora_variant = np.where((flora_variant == 0) & (core > 0.2), 4, flora_variant)  # weiqi
    else:  # deep
        flora_variant = np.where(core > 0.5, 3, flora_variant)  # daily_artifact
        flora_variant = np.where((flora_variant == 0) & (core > 0.2), 2, flora_variant)  # grave

    buffer.layers["height"] = np.round(base, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, deepslate_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, 1, dtype=np.uint8)  # plains-ish
    buffer.layers["feature_mask"] = np.round(np.clip(core * 0.6, 0.0, 1.0), 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein, 3).ravel()
    buffer.layers["anomaly_intensity"] = np.round(anomaly_intensity, 3).ravel()
    buffer.layers["anomaly_kind"] = anomaly_kind.ravel().astype(np.uint8)
    buffer.layers["ruin_density"] = np.round(ruin, 3).ravel()
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)
    buffer.layers["tsy_presence"] = np.ones(area, dtype=np.uint8)
    buffer.layers["tsy_origin_id"] = np.full(area, origin_id, dtype=np.uint8)
    buffer.layers["tsy_depth_tier"] = np.full(area, depth_id, dtype=np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
