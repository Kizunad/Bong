"""主世界九宗故地 profile — 七宗共享废墟骨架 + origin 染色."""

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


JIU_ZONG_RUIN_DECORATIONS_COMMON = (
    DecorationSpec(
        name="broken_pillar",
        kind="boulder",
        blocks=("chiseled_stone_bricks", "stone_bricks", "mossy_stone_bricks"),
        size_range=(4, 8),
        rarity=0.55,
        notes="断柱：刻纹石砖 + 苔藓砖，倒卧或半埋的大殿石柱。",
    ),
    DecorationSpec(
        name="ruined_bell_tower",
        kind="tree",
        blocks=("oak_log", "stone_bricks", "bell"),
        size_range=(7, 12),
        rarity=0.10,
        notes="残钟楼：橡木柱 + 石砖基座 + 顶端铜钟，近距触发钟声叙事。",
    ),
    DecorationSpec(
        name="moss_lain_statue",
        kind="boulder",
        blocks=("mossy_cobblestone", "cracked_stone_bricks", "armor_stand"),
        size_range=(2, 4),
        rarity=0.25,
        notes="苔卧像：苔石 + 裂砖 + armor_stand 残身，长老雕像面部已剥蚀。",
    ),
    DecorationSpec(
        name="formation_core_stub",
        kind="crystal",
        blocks=("lodestone", "chiseled_stone_bricks", "amethyst_cluster"),
        size_range=(3, 5),
        rarity=0.06,
        notes="阵核残柱：磁石 + 刻纹石 + 紫晶，可激活 landmark。",
    ),
    DecorationSpec(
        name="forgotten_stele_garden",
        kind="boulder",
        blocks=("polished_andesite", "chiseled_polished_blackstone", "sculk_vein"),
        size_range=(3, 6),
        rarity=0.18,
        notes="忘碑林：壁文 narration 锚，靠近触发该宗历史片段。",
    ),
)


JIU_ZONG_ORIGIN_SPECIFIC = {
    1: DecorationSpec(
        name="bloodstream_altar",
        kind="boulder",
        blocks=("red_concrete", "blackstone", "redstone_lamp"),
        size_range=(3, 5),
        rarity=0.20,
        notes="万血祭坛：血溪宗体修流派祭坛，近之心悸。",
    ),
    2: DecorationSpec(
        name="formation_anchor_pillar",
        kind="crystal",
        blocks=("lodestone", "deepslate_bricks", "chiseled_deepslate"),
        size_range=(4, 6),
        rarity=0.18,
        notes="阵眼锚柱：北陵阵法核心残柱。",
    ),
    3: DecorationSpec(
        name="poison_pool_basin",
        kind="boulder",
        blocks=("warped_planks", "sculk", "verdant_froglight"),
        size_range=(3, 4),
        rarity=0.15,
        notes="蛊池残皿：南渊宗炼蛊废池。",
    ),
    4: DecorationSpec(
        name="lightning_pylon_stub",
        kind="tree",
        blocks=("copper_block", "weathered_copper", "lightning_rod"),
        size_range=(6, 9),
        rarity=0.12,
        notes="引雷塔残：赤霞雷法宗的雷电吸引塔残基。",
    ),
    5: DecorationSpec(
        name="trial_blade_stele",
        kind="boulder",
        blocks=("snow_block", "iron_block", "stone_bricks"),
        size_range=(2, 4),
        rarity=0.22,
        notes="试剑碑：玄水剑宗弟子比试遗碑。",
    ),
    6: DecorationSpec(
        name="taiji_formation_disc",
        kind="boulder",
        blocks=("smooth_quartz", "polished_blackstone", "amethyst_block"),
        size_range=(4, 6),
        rarity=0.10,
        notes="太极阵盘：太初宗任督全能流派标志。",
    ),
    7: DecorationSpec(
        name="shadow_screen_wall",
        kind="boulder",
        blocks=("cobbled_deepslate", "soul_soil", "soul_lantern"),
        size_range=(3, 5),
        rarity=0.20,
        notes="影壁残基：幽暗宗暗器流隐遁训练场。",
    ),
}

ORIGIN_NAME_TO_ID = {
    "bloodstream": 1,
    "beiling": 2,
    "nanyuan": 3,
    "chixia": 4,
    "xuanshui": 5,
    "taichu": 6,
    "youan": 7,
}

WILD_FORMATION_ANOMALY_KIND = 5
COMMON_DECORATION_COUNT = len(JIU_ZONG_RUIN_DECORATIONS_COMMON)


class JiuzongRuinGenerator(TerrainProfileGenerator):
    profile_name = "jiu_zong_ruin"
    extra_layers = (
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "flora_density",
        "flora_variant_id",
        "ruin_density",
        "anomaly_intensity",
        "anomaly_kind",
        "zongmen_origin_id",
    )
    ecology = EcologySpec(
        decorations=JIU_ZONG_RUIN_DECORATIONS_COMMON
        + tuple(JIU_ZONG_ORIGIN_SPECIFIC[idx] for idx in sorted(JIU_ZONG_ORIGIN_SPECIFIC)),
        ambient_effects=("distant_chime", "stone_dust_drift"),
        notes="主世界七宗废墟共享 profile；zongmen_origin_id 决定 palette / 残卷 / 守墓人。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        origin_id = zongmen_origin_id(context.zone)
        return (
            "Overworld jiu_zong_ruin profile; not TSY dimension data.",
            f"zongmen_origin_id={origin_id} swaps origin-specific decoration palette.",
            "Qi mean stays near 0.40 while local turbulence spans roughly 0.10..0.70.",
        )


def zongmen_origin_id(zone: BlueprintZone) -> int:
    raw = zone.worldgen.extras.get("zongmen_origin_id", zone.worldgen.extras.get("origin", 7))
    if isinstance(raw, str):
        return ORIGIN_NAME_TO_ID.get(raw, 7)
    try:
        return int(raw)
    except (TypeError, ValueError):
        return 7


def fill_jiu_zong_ruin_tile(
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
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "flora_density",
        "flora_variant_id",
        "ruin_density",
        "anomaly_intensity",
        "anomaly_kind",
        "zongmen_origin_id",
    )
    buffer = TileFieldBuffer.create(tile, tile_size, layer_names)
    origin_id = max(1, min(zongmen_origin_id(zone), 7))

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    center_x, center_z = zone.center_xz
    half_width = max(zone.size_xz[0] * 0.5, 1.0)
    half_depth = max(zone.size_xz[1] * 0.5, 1.0)
    nx = (wx - center_x) / half_width
    nz = (wz - center_z) / half_depth
    radial = np.sqrt(nx * nx + nz * nz)
    core = np.clip(1.0 - radial, 0.0, 1.0)

    landform = warped_fbm_2d(
        wx,
        wz,
        scale=210.0,
        octaves=4,
        warp_scale=360.0,
        warp_strength=80.0,
        seed=7100 + origin_id * 31,
    )
    rubble = np.abs(ridge_2d(wx, wz, scale=64.0, octaves=4, seed=7200 + origin_id * 37))
    hall_grid = np.abs(ridge_2d(wx, wz, scale=96.0, octaves=3, seed=7300 + origin_id * 41))
    ruin_density = np.clip(core * 0.78 + rubble * 0.22 + (hall_grid > 0.58) * 0.18, 0.0, 1.0)

    height = 76.0 + core * 9.0 + landform * 5.0 + rubble * 2.5
    height = np.where(radial > 0.86, 72.0 + landform * 2.0, height)

    turbulence = fbm_2d(wx, wz, scale=92.0, octaves=3, seed=7400 + origin_id * 43)
    qi_density = np.clip(0.40 + turbulence * 0.30 - radial * 0.10, 0.10, 0.70)
    qi_density = np.where(radial > 0.92, np.clip(qi_density - 0.10, 0.10, 0.70), qi_density)
    mofa_decay = np.clip(0.62 - core * 0.12 + rubble * 0.08, 0.45, 0.72)
    qi_vein_flow = np.clip((hall_grid > 0.52) * 0.35 + core * 0.20 + rubble * 0.15, 0.0, 0.65)

    formation_field = warped_fbm_2d(
        wx,
        wz,
        scale=150.0,
        octaves=3,
        warp_scale=220.0,
        warp_strength=48.0,
        seed=7500 + origin_id * 47,
    )
    formation_core = (core > 0.58) & ((formation_field > 0.30) | (radial < 0.12))
    anomaly_intensity = np.where(formation_core, np.clip(0.35 + formation_field * 0.65, 0.0, 1.0), 0.0)
    anomaly_kind = np.where(anomaly_intensity > 0.0, WILD_FORMATION_ANOMALY_KIND, 0)

    flora_density = np.clip(ruin_density * 0.50 + (1.0 - radial) * 0.18, 0.0, 0.85)
    specific_mask = (ruin_density > 0.52) & (formation_field > 0.18)
    flora_variant = np.zeros_like(height, dtype=np.int32)
    flora_variant = np.where(ruin_density > 0.26, 1, flora_variant)
    flora_variant = np.where((ruin_density > 0.44) & (hall_grid > 0.35), 3, flora_variant)
    flora_variant = np.where((ruin_density > 0.58) & (hall_grid > 0.58), 5, flora_variant)
    flora_variant = np.where(formation_core, 4, flora_variant)
    flora_variant = np.where(specific_mask, COMMON_DECORATION_COUNT + origin_id, flora_variant)
    flora_variant = np.where(flora_density <= 0.0, 0, flora_variant)

    stone_id = palette.ensure("stone")
    cracked_id = palette.ensure("cracked_stone_bricks")
    mossy_id = palette.ensure("mossy_cobblestone")
    gravel_id = palette.ensure("gravel")
    coarse_id = palette.ensure("coarse_dirt")
    origin_surface = _origin_surface_id(origin_id, palette)

    surface_id = np.full_like(height, coarse_id, dtype=np.int32)
    surface_id = np.where(radial > 0.86, gravel_id, surface_id)
    surface_id = np.where(ruin_density > 0.22, mossy_id, surface_id)
    surface_id = np.where(ruin_density > 0.42, cracked_id, surface_id)
    surface_id = np.where(specific_mask, origin_surface, surface_id)
    surface_id = np.where(formation_core, palette.ensure("lodestone"), surface_id)

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, 13, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(np.clip(ruin_density * 0.7 + core * 0.1, 0.0, 1.0), 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein_flow, 3).ravel()
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)
    buffer.layers["ruin_density"] = np.round(ruin_density, 3).ravel()
    buffer.layers["anomaly_intensity"] = np.round(anomaly_intensity, 3).ravel()
    buffer.layers["anomaly_kind"] = anomaly_kind.ravel().astype(np.uint8)
    buffer.layers["zongmen_origin_id"] = np.full(area, origin_id, dtype=np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer


def _origin_surface_id(origin_id: int, palette: SurfacePalette) -> int:
    origin_surfaces = {
        1: "red_terracotta",
        2: "deepslate_bricks",
        3: "warped_planks",
        4: "weathered_copper",
        5: "packed_ice",
        6: "smooth_quartz",
        7: "cobbled_deepslate",
    }
    return palette.ensure(origin_surfaces.get(origin_id, "cobbled_deepslate"))
