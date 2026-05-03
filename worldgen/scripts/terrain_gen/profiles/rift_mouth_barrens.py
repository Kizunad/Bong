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


RIFT_MOUTH_DECORATIONS = (
    DecorationSpec(
        name="charred_obelisk_shard",
        kind="boulder",
        blocks=("blackstone", "obsidian", "crying_obsidian"),
        size_range=(3, 7),
        rarity=0.45,
        notes="焦黑碑碎：黑石 + 黑曜 + 哭泣黑曜。渊口周边炭化石柱碎片。",
    ),
    DecorationSpec(
        name="frost_qi_cluster",
        kind="crystal",
        blocks=("packed_ice", "blue_ice", "amethyst_cluster"),
        size_range=(2, 5),
        rarity=0.35,
        notes="寒气晶簇：负压使残存真元过冷析出，地表渊口中等密度散布。",
    ),
    DecorationSpec(
        name="ganshi_drift",
        kind="boulder",
        blocks=("bone_block", "white_concrete", "soul_soil"),
        size_range=(2, 4),
        rarity=0.18,
        notes="干尸漂积：塌缩外溢的修士干尸堆，近之 HUD 灵压闪烁。",
    ),
    DecorationSpec(
        name="fresh_collapse_rubble",
        kind="boulder",
        blocks=("cobblestone", "tuff", "cobbled_deepslate"),
        size_range=(3, 5),
        rarity=0.30,
        notes="新鲜崩石：刚塌不久的碎石堆，表面苔藓覆盖率为 0。",
    ),
    DecorationSpec(
        name="spacetime_scar",
        kind="crystal",
        blocks=("end_stone", "purpur_block", "shulker_box"),
        size_range=(2, 3),
        rarity=0.05,
        notes="时空疤：极稀有高品质 portal 痕迹，不作为入口 marker。",
    ),
    DecorationSpec(
        name="dao_zhuang_corpse_pose",
        kind="boulder",
        blocks=("bone_block", "armor_stand", "stripped_oak_log"),
        size_range=(1, 2),
        rarity=0.08,
        notes="道伥姿干尸：外溢未活化的道伥姿态，凝固在塌缩瞬间。",
    ),
    DecorationSpec(
        name="cracked_floor_seam",
        kind="boulder",
        blocks=("cobblestone", "stone", "tuff"),
        size_range=(1, 2),
        rarity=0.40,
        notes="裂缝石：与普通 wilderness 裂缝接近，portal 位置靠感知确认。",
    ),
)


class RiftMouthBarrensGenerator(TerrainProfileGenerator):
    profile_name = "rift_mouth_barrens"
    extra_layers = (
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "flora_density",
        "flora_variant_id",
        "neg_pressure",
        "portal_anchor_sdf",
        "anomaly_intensity",
        "anomaly_kind",
    )
    ecology = EcologySpec(
        decorations=RIFT_MOUTH_DECORATIONS,
        ambient_effects=("frost_breath", "distant_void_hum", "cold_wind"),
        notes="渊口荒丘：焦黑石原 + 寒气晶簇 + 干尸残骸；portal 锚点由 portal_anchor_sdf 表达，地表不放显眼 marker。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        anchor = context.zone.worldgen.extras.get("portal_anchor_xz", context.zone.center_xz)
        link = context.zone.worldgen.extras.get("tsy_zone_link", "")
        return (
            "Surface-only rift mouth scar; no cave_mask is written for the overworld portal anchor.",
            f"portal_anchor_xz={anchor}; tsy_zone_link={link}; trigger remains coordinate-driven, not decoration-driven.",
        )


def fill_rift_mouth_barrens_tile(
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
        "neg_pressure",
        "portal_anchor_sdf",
        "anomaly_intensity",
        "anomaly_kind",
    )
    buffer = TileFieldBuffer.create(tile, tile_size, layer_names)

    blackstone_id = palette.ensure("blackstone")
    obsidian_id = palette.ensure("obsidian")
    tuff_id = palette.ensure("tuff")
    coarse_dirt_id = palette.ensure("coarse_dirt")
    packed_ice_id = palette.ensure("packed_ice")
    cobblestone_id = palette.ensure("cobblestone")
    stone_id = palette.ensure("stone")
    soul_soil_id = palette.ensure("soul_soil")
    rift_mouth_biome_id = 12

    anchor_x, anchor_z = _portal_anchor_xz(zone)
    center_x, center_z = zone.center_xz
    core_radius = max(float(zone.worldgen.extras.get("core_radius", 30.0)), 1.0)
    outer_radius = max(float(zone.worldgen.extras.get("outer_radius", 150.0)), core_radius)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    portal_anchor_sdf = np.sqrt((wx - anchor_x) ** 2 + (wz - anchor_z) ** 2)
    anchor_theta = np.arctan2(wz - anchor_z, wx - anchor_x)
    zone_dist = np.sqrt((wx - center_x) ** 2 + (wz - center_z) ** 2)
    t = portal_anchor_sdf / core_radius
    outer_t = np.clip((portal_anchor_sdf - core_radius) / (outer_radius - core_radius), 0.0, 1.0)
    scar = np.clip(1.0 - portal_anchor_sdf / outer_radius, 0.0, 1.0)

    mound = warped_fbm_2d(
        wx, wz, scale=150.0, octaves=4, warp_scale=210.0, warp_strength=38.0, seed=930
    )
    crack = ridge_2d(wx, wz, scale=42.0, octaves=4, seed=940)
    cold_noise = fbm_2d(wx, wz, scale=36.0, octaves=3, seed=950)

    height = 74.0 + mound * 3.2 - scar * 4.5 - np.maximum(0.0, 1.0 - t) * 2.0
    height = np.where(portal_anchor_sdf < 2.0, height - 0.8, height)

    core_pull = np.clip(1.0 - (portal_anchor_sdf / core_radius) ** 1.15, 0.0, 1.0)
    outer_pull = np.clip(1.0 - outer_t, 0.0, 1.0)
    neg_pressure = np.where(
        portal_anchor_sdf <= core_radius,
        0.50 + 0.30 * core_pull,
        0.15 * outer_pull,
    )

    qi_density = np.full_like(height, 0.08)
    qi_density = np.where(t <= 1.0, 0.05, qi_density)
    qi_density = np.where(t <= 0.7, 0.02, qi_density)
    qi_density = np.where(t <= 0.3, 0.0, qi_density)
    mofa_decay = np.full_like(height, 0.50)
    mofa_decay = np.where(t <= 1.0, 0.65, mofa_decay)
    mofa_decay = np.where(t <= 0.7, 0.85, mofa_decay)
    mofa_decay = np.where(t <= 0.3, 0.95, mofa_decay)
    qi_vein_flow = np.zeros_like(height)

    anomaly_intensity = np.where(
        portal_anchor_sdf <= outer_radius,
        np.clip(0.12 + scar * 0.78 + neg_pressure * 0.15, 0.0, 1.0),
        0.0,
    )
    anomaly_kind = np.where(anomaly_intensity > 0.05, 1, 0)

    surface_id = np.full_like(height, coarse_dirt_id, dtype=np.int32)
    surface_id = np.where(scar > 0.15, tuff_id, surface_id)
    surface_id = np.where(scar > 0.35, blackstone_id, surface_id)
    surface_id = np.where((t < 0.32) & (crack > 0.35), obsidian_id, surface_id)
    surface_id = np.where((t < 1.0) & (cold_noise > 0.42), packed_ice_id, surface_id)
    surface_id = np.where((t < 0.24) & (crack < -0.1), cobblestone_id, surface_id)
    surface_id = np.where((t > 2.0) & (zone_dist > outer_radius), stone_id, surface_id)

    feature_mask = np.clip(scar * 0.72 + np.maximum(0.0, crack) * 0.18, 0.0, 1.0)

    flora_density = np.zeros_like(height)
    flora_variant = np.zeros_like(height, dtype=np.int32)

    # Local ids: 1 obelisk / 2 frost / 3 ganshi / 4 rubble / 5 scar /
    # 6 daozhuang corpse pose / 7 cracked seam.
    rubble_band = (t >= 1.0) & (t <= 2.0)
    flora_variant = np.where(rubble_band, 4, flora_variant)
    flora_density = np.where(rubble_band, np.maximum(flora_density, 0.30), flora_density)

    cold_band = (t >= 0.55) & (t < 1.35) & (cold_noise > -0.10)
    flora_variant = np.where(cold_band, 2, flora_variant)
    flora_density = np.where(cold_band, np.maximum(flora_density, 0.35), flora_density)

    obelisk_band = (t >= 0.35) & (t < 0.85) & (crack > 0.15)
    flora_variant = np.where(obelisk_band, 1, flora_variant)
    flora_density = np.where(obelisk_band, np.maximum(flora_density, 0.42), flora_density)

    corpse_band = (t >= 0.18) & (t < 0.65) & (cold_noise < -0.18)
    flora_variant = np.where(corpse_band, 3, flora_variant)
    flora_density = np.where(corpse_band, np.maximum(flora_density, 0.18), flora_density)

    pose_band = (t >= 0.22) & (t < 0.55) & (
        (crack < -0.35) | (np.abs(anchor_theta + 2.2) < 0.08)
    )
    flora_variant = np.where(pose_band, 6, flora_variant)
    flora_density = np.where(pose_band, np.maximum(flora_density, 0.08), flora_density)

    seam_band = t < 0.22
    flora_variant = np.where(seam_band, 7, flora_variant)
    flora_density = np.where(seam_band, np.maximum(flora_density, 0.40), flora_density)

    scar_band = (t >= 0.05) & (t < 0.32) & (
        (cold_noise > 0.55) | (np.abs(anchor_theta - 0.75) < 0.06)
    )
    flora_variant = np.where(scar_band, 5, flora_variant)
    flora_density = np.where(scar_band, np.maximum(flora_density, 0.05), flora_density)

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, soul_soil_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, rift_mouth_biome_id, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein_flow, 3).ravel()
    buffer.layers["flora_density"] = np.round(np.clip(flora_density, 0.0, 1.0), 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)
    buffer.layers["neg_pressure"] = np.round(neg_pressure, 3).ravel()
    buffer.layers["portal_anchor_sdf"] = np.round(portal_anchor_sdf, 3).ravel()
    buffer.layers["anomaly_intensity"] = np.round(anomaly_intensity, 3).ravel()
    buffer.layers["anomaly_kind"] = anomaly_kind.ravel().astype(np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer


def _portal_anchor_xz(zone: BlueprintZone) -> tuple[float, float]:
    raw = zone.worldgen.extras.get("portal_anchor_xz")
    if isinstance(raw, (list, tuple)) and len(raw) == 2:
        return float(raw[0]), float(raw[1])
    return float(zone.center_xz[0]), float(zone.center_xz[1])
