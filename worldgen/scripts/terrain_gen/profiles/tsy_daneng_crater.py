"""TSY 大能陨落 profile — 陨石坑 / 灵气结晶柱 / 中心残骸.

plan-tsy-worldgen-v1 §3.1 / §3.4.

vs zongmen 关键差异：
- ellipse 圆形坑（非殿宇方阵）：中心碗状坑，shallow 圆圈构图，deep 中央巨型晶柱腔体
- surface_palette 火山岩系（blackstone/basalt/magma），deep 加 amethyst_block/end_stone
- extra_layers 去 fracture_mask 加 cave_mask（坑底空腔）
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

DANENG_CRATER_DECORATIONS = (
    DecorationSpec(
        name="qi_crystal_pillar",
        kind="crystal",
        blocks=("amethyst_block", "calcite", "end_rod"),
        size_range=(4, 8),
        rarity=0.30,
        notes="灵气结晶柱：陨落大能体内灵气溢出凝成的紫晶柱，向上发散辉光。",
    ),
    DecorationSpec(
        name="meteor_core_wreckage",
        kind="boulder",
        blocks=("black_concrete", "obsidian", "blackstone"),
        size_range=(5, 9),
        rarity=0.18,
        notes="中心残骸：陨星撞击中心未完全瓦解的黑色晶状残块。",
    ),
    DecorationSpec(
        name="scorched_earth_ring",
        kind="shrub",
        blocks=("blackstone", "magma_block", "basalt"),
        size_range=(2, 4),
        rarity=0.45,
        notes="焦土环：陨击后高温灼烧形成的环形焦黑带。",
    ),
    DecorationSpec(
        name="amethyst_bud_cluster",
        kind="crystal",
        blocks=("small_amethyst_bud", "amethyst_cluster", "calcite"),
        size_range=(1, 3),
        rarity=0.35,
        notes="灵晶碎簇：散布在主柱周边的小型紫晶簇。",
    ),
)

ORIGIN_CODE = {"daneng_luoluo": 1, "zongmen_yiji": 2, "zhanchang_chendian": 3, "gaoshou_sichu": 4}
DEPTH_CODE = {"shallow": 1, "mid": 2, "deep": 3}


class TsyDanengCraterGenerator(TerrainProfileGenerator):
    profile_name = "tsy_daneng_crater"
    extra_layers = (
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "anomaly_intensity",
        "anomaly_kind",
        "ruin_density",
        "cave_mask",
        "flora_density",
        "flora_variant_id",
        "tsy_presence",
        "tsy_origin_id",
        "tsy_depth_tier",
    )
    ecology = EcologySpec(
        decorations=DANENG_CRATER_DECORATIONS,
        ambient_effects=("crystal_chime", "ash_drift", "low_hum"),
        notes="陨石坑 + 灵气结晶柱 + 中心残骸。色调黑紫，深层中央巨型晶柱腔体。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Y stratified; shallow crater rim, mid crater body, deep central cavity.",
            "Highest qi_density at deep tier (relic_core_slot center) + cave_mask cavity.",
            "Surface palette favours volcanic rock; deep adds amethyst/end_stone for crystal hall.",
        )


def fill_tsy_daneng_crater_tile(
    zone: BlueprintZone,
    tile: WorldTile,
    tile_size: int,
    palette: SurfacePalette,
) -> TileFieldBuffer:
    depth_tier = zone.worldgen.extras.get("depth_tier", "shallow")
    origin_id = ORIGIN_CODE.get(zone.worldgen.extras.get("origin", "daneng_luoluo"), 1)
    depth_id = DEPTH_CODE.get(depth_tier, 1)

    layer_names = (
        "height", "surface_id", "subsurface_id", "water_level",
        "biome_id", "feature_mask", "boundary_weight",
        "qi_density", "mofa_decay", "qi_vein_flow",
        "anomaly_intensity", "anomaly_kind",
        "ruin_density", "cave_mask",
        "flora_density", "flora_variant_id",
        "tsy_presence", "tsy_origin_id", "tsy_depth_tier",
    )
    buffer = TileFieldBuffer.create(tile, tile_size, layer_names)
    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    area = tile_size * tile_size

    # 中心碗状坑：以 zone center 为圆心，radial 距离驱动地形深浅
    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    core = np.maximum(0.0, 1.0 - radial**1.5)  # 中心 = 1，边缘 = 0

    if depth_tier == "shallow":
        # 陨击坑边缘隆起，中心微凹
        rim = np.clip(1.0 - np.abs(radial - 0.85) * 4.0, 0.0, 1.0)
        base = 64.0 + rim * 12.0 - core * 6.0 + fbm_2d(wx, wz, scale=140.0, octaves=3, seed=3100) * 3.0
        ruin = np.clip(0.15 + core * 0.20 + fbm_2d(wx, wz, scale=80.0, octaves=2, seed=3110) * 0.15, 0.0, 0.5)
        qi = np.clip(0.86 + core * 0.06, 0.78, 1.0)
        decay = np.clip(0.14 - core * 0.04, 0.05, 0.22)
        cave = np.zeros_like(base)
    elif depth_tier == "mid":
        base = 12.0 + ridge_2d(wx, wz, scale=70.0, octaves=4, seed=3200) * 5.0 - core * 6.0
        ruin = np.clip(0.45 + core * 0.30 + warped_fbm_2d(wx, wz, scale=80.0, octaves=3, warp_scale=120.0, warp_strength=40.0, seed=3210) * 0.20, 0.2, 0.85)
        qi = np.clip(0.88 + core * 0.05, 0.78, 1.0)
        decay = np.clip(0.13, 0.06, 0.25)
        cave = np.clip(core * 0.5 + fbm_2d(wx, wz, scale=120.0, octaves=2, seed=3220) * 0.2, 0.0, 0.7)
    else:  # deep
        # 中央巨型晶柱腔体 — 中心 cavity 顶 -4，底 -36
        base = -22.0 + fbm_2d(wx, wz, scale=160.0, octaves=3, seed=3300) * 5.0 - core * 12.0
        ruin = np.clip(0.30 + core * 0.25, 0.15, 0.7)
        qi = np.clip(0.94 + core * 0.05, 0.88, 1.0)
        decay = np.clip(0.08, 0.03, 0.18)
        # cave_mask: 深层中心高
        cave = np.clip(core * 0.85 + fbm_2d(wx, wz, scale=140.0, octaves=2, seed=3320) * 0.15, 0.0, 1.0)

    qi_vein = np.clip(core * 0.5 + ruin * 0.3, 0.0, 1.0)

    blackstone_id = palette.ensure("blackstone")
    basalt_id = palette.ensure("basalt")
    deepslate_id = palette.ensure("deepslate")
    calcite_id = palette.ensure("calcite")
    amethyst_id = palette.ensure("amethyst_block") if depth_tier == "deep" else palette.ensure("blackstone")

    surface_id = np.full_like(base, blackstone_id, dtype=np.int32)
    surface_id = np.where(ruin > 0.4, basalt_id, surface_id)
    if depth_tier == "deep":
        surface_id = np.where(core > 0.6, amethyst_id, surface_id)
        surface_id = np.where((core > 0.3) & (core <= 0.6), calcite_id, surface_id)
    elif depth_tier == "shallow":
        gravel_id = palette.ensure("gravel")
        surface_id = np.where(core < 0.2, gravel_id, surface_id)

    anomaly_seed = 3500 + depth_id * 100
    anomaly_field = warped_fbm_2d(wx, wz, scale=220.0, octaves=3, warp_scale=260.0, warp_strength=70.0, seed=anomaly_seed)
    anomaly_threshold = {"shallow": 0.55, "mid": 0.45, "deep": 0.30}[depth_tier]
    anomaly_intensity = np.clip((anomaly_field - anomaly_threshold) * 3.0, 0.0, 1.0)
    # kind: shallow 多 spacetime_rift（1），deep 多 wild_formation（5），mid 偶发 qi_turbulence（2）
    if depth_tier == "deep":
        anomaly_kind = np.where(anomaly_intensity > 0.15, 5, 0).astype(np.int32)
    elif depth_tier == "shallow":
        anomaly_kind = np.where(anomaly_intensity > 0.15, 1, 0).astype(np.int32)
    else:
        anomaly_kind = np.where(anomaly_intensity > 0.15, 2, 0).astype(np.int32)

    flora_density = np.clip(ruin * 0.5 + core * 0.3, 0.0, 1.0)
    flora_variant = np.zeros_like(base, dtype=np.int32)
    if depth_tier == "deep":
        flora_variant = np.where(core > 0.6, 1, flora_variant)  # qi_crystal_pillar
        flora_variant = np.where((flora_variant == 0) & (core > 0.3), 4, flora_variant)  # amethyst_bud
    elif depth_tier == "shallow":
        flora_variant = np.where((radial > 0.7) & (radial < 1.0), 3, flora_variant)  # scorched_earth_ring
        flora_variant = np.where((flora_variant == 0) & (core > 0.4), 2, flora_variant)  # meteor_core_wreckage
    else:  # mid
        flora_variant = np.where(core > 0.5, 2, flora_variant)
        flora_variant = np.where((flora_variant == 0) & (core > 0.2), 4, flora_variant)

    buffer.layers["height"] = np.round(base, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, deepslate_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, 5, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(np.clip(core * 0.5 + ruin * 0.3, 0.0, 1.0), 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein, 3).ravel()
    buffer.layers["anomaly_intensity"] = np.round(anomaly_intensity, 3).ravel()
    buffer.layers["anomaly_kind"] = anomaly_kind.ravel().astype(np.uint8)
    buffer.layers["ruin_density"] = np.round(ruin, 3).ravel()
    buffer.layers["cave_mask"] = np.round(cave, 3).ravel()
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)
    buffer.layers["tsy_presence"] = np.ones(area, dtype=np.uint8)
    buffer.layers["tsy_origin_id"] = np.full(area, origin_id, dtype=np.uint8)
    buffer.layers["tsy_depth_tier"] = np.full(area, depth_id, dtype=np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
