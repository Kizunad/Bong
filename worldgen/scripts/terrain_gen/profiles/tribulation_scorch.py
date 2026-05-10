from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, ridge_2d, warped_fbm_2d
from .base import DecorationSpec, EcologySpec, ProfileContext, TerrainProfileGenerator


TRIBULATION_SCORCH_DECORATIONS = (
    DecorationSpec(
        name="glass_fulgurite",
        kind="boulder",
        blocks=("sand", "glass", "tinted_glass"),
        size_range=(2, 5),
        rarity=0.40,
        notes="玻璃熔痕：雷弧熔沙形成的短管状玻璃坑，是焦土最常见视觉符号。",
    ),
    DecorationSpec(
        name="charred_husk_tree",
        kind="tree",
        blocks=("coal_block", "stripped_oak_log", "blackstone"),
        size_range=(5, 10),
        rarity=0.35,
        notes="焦炭枯木：煤块树干与剥皮原木残躯，树冠完全烧空。",
    ),
    DecorationSpec(
        name="lightning_basalt_pit",
        kind="boulder",
        blocks=("basalt", "obsidian", "magma_block"),
        size_range=(4, 8),
        rarity=0.28,
        notes="劫雷玄武坑：雷击穿地表后冷却的玄武岩与黑曜石坑。",
    ),
    DecorationSpec(
        name="lodestone_vortex",
        kind="crystal",
        blocks=("lodestone", "copper_block", "weathered_copper"),
        size_range=(3, 6),
        rarity=0.18,
        notes="雷磁旋柱：天然极化磁石柱，雷雨天主动招雷。",
    ),
    DecorationSpec(
        name="iron_lattice_slag",
        kind="shrub",
        blocks=("iron_block", "raw_iron_block", "deepslate"),
        size_range=(2, 4),
        rarity=0.22,
        notes="铁渣矩阵：雷劈后熔合的铁矿渣块。",
    ),
    DecorationSpec(
        name="blue_lightning_glass",
        kind="crystal",
        blocks=("blue_stained_glass", "light_blue_concrete", "sea_lantern"),
        size_range=(3, 5),
        rarity=0.10,
        notes="蓝雷晶：间歇雷击结晶产物，夜间发蓝光。",
    ),
    DecorationSpec(
        name="magnetized_copper_slag",
        kind="shrub",
        blocks=("copper_block", "cut_copper", "raw_copper_block"),
        size_range=(2, 4),
        rarity=0.16,
        notes="雷磁铜渣：铜矿被劫雷烧结后的地表露头。",
    ),
)


ANOMALY_KIND_CURSED_ECHO = 4
MINERAL_KIND_LODESTONE = 1
MINERAL_KIND_COPPER = 2
MINERAL_KIND_IRON = 3


class TribulationScorchGenerator(TerrainProfileGenerator):
    profile_name = "tribulation_scorch"
    extra_layers = (
        "qi_density",
        "mofa_decay",
        "qi_vein_flow",
        "flora_density",
        "flora_variant_id",
        "mineral_density",
        "mineral_kind",
        "anomaly_intensity",
        "anomaly_kind",
    )
    ecology = EcologySpec(
        decorations=TRIBULATION_SCORCH_DECORATIONS,
        ambient_effects=("distant_thunder_low", "ash_fall", "static_crackle"),
        notes="烬焰焦土：玻璃化沙地、焦炭树、雷磁矿露头与劫雷玄武坑；化虚遗迹由独立 structure manifest 输出。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "tribulation_scorch is a fixed accumulated tribulation scar, not a transient event.",
            "anomaly_kind reuses 4=cursed_echo for tribulation residue; no new enum value.",
            "mineral_density/mineral_kind expose surface lodestone, copper, and iron lightning deposits.",
            "tianjie_ascension_pit is driven by worldgen.extras.ascension_pit_xz and not by DecorationSpec rarity.",
        )


def fill_tribulation_scorch_tile(
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
        "mineral_density",
        "mineral_kind",
        "anomaly_intensity",
        "anomaly_kind",
    )
    buffer = TileFieldBuffer.create(tile, tile_size, layer_names)

    coarse_dirt_id = palette.ensure("coarse_dirt")
    gravel_id = palette.ensure("gravel")
    sand_id = palette.ensure("sand")
    blackstone_id = palette.ensure("blackstone")
    basalt_id = palette.ensure("basalt")
    glass_id = palette.ensure("glass")
    tinted_glass_id = palette.ensure("tinted_glass")
    obsidian_id = palette.ensure("obsidian")
    magma_block_id = palette.ensure("magma_block")
    lodestone_id = palette.ensure("lodestone")
    copper_id = palette.ensure("copper_block")
    raw_iron_id = palette.ensure("raw_iron_block")
    scorch_biome_id = 6

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    edge_warp = 1.0 + fbm_2d(wx, wz, scale=620.0, octaves=3, seed=1610) * 0.18
    dx = (wx - center_x) / (half_w * edge_warp)
    dz = (wz - center_z) / (half_d * (1.0 - (edge_warp - 1.0) * 0.25))
    radial = np.sqrt(dx * dx + dz * dz)
    interior = radial <= 1.0
    core = radial <= 0.34
    body = (radial > 0.34) & (radial <= 0.78)
    disturbed_edge = (radial > 0.78) & (radial <= 1.08)
    falloff = np.clip(1.0 - radial, 0.0, 1.0)

    ash_wave = warped_fbm_2d(
        wx,
        wz,
        scale=300.0,
        octaves=4,
        warp_scale=380.0,
        warp_strength=58.0,
        seed=1620,
    )
    glass_noise = fbm_2d(wx, wz, scale=76.0, octaves=3, seed=1630)
    crater_noise = ridge_2d(wx, wz, scale=132.0, octaves=5, seed=1640)
    static_noise = warped_fbm_2d(
        wx,
        wz,
        scale=210.0,
        octaves=3,
        warp_scale=280.0,
        warp_strength=70.0,
        seed=1650,
    )

    crater_mask = np.clip((crater_noise - 0.30) * 2.4, 0.0, 1.0) * np.clip(1.15 - radial, 0.0, 1.0)
    static_mask = np.clip((static_noise - 0.10) * 1.8, 0.0, 1.0) * np.clip(1.10 - radial, 0.0, 1.0)

    height = (
        78.0
        + falloff * 8.0
        + ash_wave * 2.2
        + glass_noise * 0.8
        - crater_mask * 7.5
        + static_mask * 1.8
    )
    height = np.where(interior, height, 75.0 + ash_wave * 1.4)

    surface_id = np.full_like(height, coarse_dirt_id, dtype=np.int32)
    surface_id = np.where(ash_wave > 0.28, gravel_id, surface_id)
    surface_id = np.where(ash_wave < -0.24, sand_id, surface_id)
    surface_id = np.where((glass_noise > 0.22) & interior, glass_id, surface_id)
    surface_id = np.where((glass_noise > 0.42) & interior, tinted_glass_id, surface_id)
    surface_id = np.where(crater_mask > 0.46, basalt_id, surface_id)
    surface_id = np.where(crater_mask > 0.72, obsidian_id, surface_id)
    surface_id = np.where(static_mask > 0.62, blackstone_id, surface_id)

    local_x = wx - center_x + half_w
    local_z = wz - center_z + half_d
    charred_tree = (
        (np.mod(np.floor(local_x / 111.0) + np.floor(local_z / 97.0) * 3, 13) == 0)
        & body
    )
    basalt_pit = (
        ((wx - (center_x + half_w * 0.22)) ** 2 + (wz - (center_z - half_d * 0.18)) ** 2)
        <= (min(half_w, half_d) * 0.13) ** 2
    ) & interior
    lodestone_vortex = (
        ((wx - (center_x - half_w * 0.24)) ** 2 + (wz - (center_z + half_d * 0.12)) ** 2)
        <= (min(half_w, half_d) * 0.10) ** 2
    ) & interior
    iron_lattice = (
        (np.mod(np.floor(local_x / 83.0) - np.floor(local_z / 71.0), 11) == 0)
        & disturbed_edge
    )
    blue_glass = (
        ((wx - (center_x + half_w * 0.10)) ** 2 + (wz - (center_z + half_d * 0.32)) ** 2)
        <= (min(half_w, half_d) * 0.09) ** 2
    ) & interior
    copper_slag = (
        ((wx - (center_x - half_w * 0.12)) ** 2 + (wz - (center_z - half_d * 0.30)) ** 2)
        <= (min(half_w, half_d) * 0.08) ** 2
    ) & interior

    ascension_mask = _ascension_pit_mask(zone, wx, wz)
    surface_id = np.where(ascension_mask, obsidian_id, surface_id)
    surface_id = np.where(ascension_mask & (crater_mask > 0.20), magma_block_id, surface_id)
    surface_id = np.where(lodestone_vortex, lodestone_id, surface_id)
    surface_id = np.where(copper_slag, copper_id, surface_id)
    surface_id = np.where(iron_lattice, raw_iron_id, surface_id)
    surface_id = np.where(basalt_pit, basalt_id, surface_id)

    qi_density = np.full_like(height, 0.18, dtype=np.float64)
    qi_density = np.where(disturbed_edge, 0.20, qi_density)
    qi_density = np.where(body, 0.30, qi_density)
    qi_density = np.where(core, 0.24, qi_density)
    qi_density = np.where((crater_mask > 0.55) & interior, 0.08, qi_density)
    qi_density = np.where(ascension_mask, 0.0, qi_density)
    qi_density = np.where(interior, qi_density, 0.12)

    mofa_decay = np.where(
        core,
        0.58,
        np.where(body, 0.50, np.where(disturbed_edge, 0.55, 0.42)),
    )
    mofa_decay = np.where(ascension_mask, 0.68, mofa_decay)
    qi_vein_flow = np.clip(static_mask * 0.42 + lodestone_vortex * 0.40, 0.0, 1.0)

    mineral_density = np.clip(
        crater_mask * 0.22
        + static_mask * 0.28
        + lodestone_vortex * 0.40
        + copper_slag * 0.34
        + iron_lattice * 0.30
        + ascension_mask * 0.30,
        0.0,
        1.0,
    )
    mineral_kind = np.zeros_like(height, dtype=np.int32)
    mineral_kind = np.where(mineral_density > 0.10, MINERAL_KIND_IRON, mineral_kind)
    mineral_kind = np.where(copper_slag | ((static_mask > 0.48) & (glass_noise > 0.05)), MINERAL_KIND_COPPER, mineral_kind)
    mineral_kind = np.where(lodestone_vortex | ascension_mask, MINERAL_KIND_LODESTONE, mineral_kind)

    anomaly_intensity = np.clip(0.10 + static_mask * 0.20 + crater_mask * 0.08, 0.0, 0.32)
    if np.any(ascension_mask):
        pit_pulse = np.clip(0.60 + static_noise * 0.18, 0.60, 0.90)
        anomaly_intensity = np.where(ascension_mask, pit_pulse, anomaly_intensity)
    anomaly_kind = np.where(anomaly_intensity > 0.08, ANOMALY_KIND_CURSED_ECHO, 0).astype(np.int32)

    flora_density = np.zeros_like(height)
    flora_variant = np.zeros_like(height, dtype=np.int32)
    glass_fulgurite = (glass_noise > -0.12) & interior
    flora_variant = np.where(glass_fulgurite, 1, flora_variant)
    flora_density = np.where(glass_fulgurite, 0.34, flora_density)
    flora_variant = np.where(charred_tree, 2, flora_variant)
    flora_density = np.where(charred_tree, 0.58, flora_density)
    flora_variant = np.where((crater_mask > 0.50) | basalt_pit, 3, flora_variant)
    flora_density = np.where((crater_mask > 0.50) | basalt_pit, 0.52, flora_density)
    flora_variant = np.where(lodestone_vortex, 4, flora_variant)
    flora_density = np.where(lodestone_vortex, 0.70, flora_density)
    flora_variant = np.where(iron_lattice, 5, flora_variant)
    flora_density = np.where(iron_lattice, 0.48, flora_density)
    flora_variant = np.where(blue_glass, 6, flora_variant)
    flora_density = np.where(blue_glass, 0.62, flora_density)
    flora_variant = np.where(copper_slag, 7, flora_variant)
    flora_density = np.where(copper_slag, 0.50, flora_density)
    flora_variant = np.where(ascension_mask, 3, flora_variant)
    flora_density = np.where(ascension_mask, 0.66, flora_density)

    feature_mask = np.clip(
        0.20 + falloff * 0.28 + crater_mask * 0.34 + static_mask * 0.30 + ascension_mask * 0.45,
        0.0,
        1.0,
    )

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, blackstone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.full(area, -1.0, dtype=np.float64)
    buffer.layers["biome_id"] = np.full(area, scorch_biome_id, dtype=np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()
    buffer.layers["qi_vein_flow"] = np.round(qi_vein_flow, 3).ravel()
    buffer.layers["flora_density"] = np.round(np.clip(flora_density, 0.0, 1.0), 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)
    buffer.layers["mineral_density"] = np.round(mineral_density, 3).ravel()
    buffer.layers["mineral_kind"] = mineral_kind.ravel().astype(np.uint8)
    buffer.layers["anomaly_intensity"] = np.round(anomaly_intensity, 3).ravel()
    buffer.layers["anomaly_kind"] = anomaly_kind.ravel().astype(np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer


def _ascension_pit_mask(
    zone: BlueprintZone,
    wx: np.ndarray,
    wz: np.ndarray,
) -> np.ndarray:
    raw = zone.worldgen.extras.get("ascension_pit_xz")
    if not isinstance(raw, (list, tuple)) or len(raw) != 2:
        return np.zeros_like(wx, dtype=bool)
    pit_x = float(raw[0])
    pit_z = float(raw[1])
    radius = float(zone.worldgen.extras.get("ascension_pit_radius", 30.0))
    return ((wx - pit_x) ** 2 + (wz - pit_z) ** 2) <= radius**2
