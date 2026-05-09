from __future__ import annotations

import numpy as np

from ..blueprint import BlueprintZone, PoiSpec
from ..fields import SurfacePalette, TileFieldBuffer, WorldTile
from ..noise import _tile_coords, fbm_2d, warped_fbm_2d
from .base import (
    DecorationSpec,
    EcologySpec,
    ProfileContext,
    TerrainProfileGenerator,
)

TUTORIAL_LINGQUAN_QI_THRESHOLD = 0.5
TUTORIAL_LINGQUAN_SCAN_RADIUS = 200.0
TUTORIAL_LINGQUAN_MIN_SEPARATION = 40.0
TUTORIAL_LINGQUAN_FALLBACK_OFFSETS = ((50.0, 100.0), (-30.0, -80.0))


SPAWN_PLAIN_DECORATIONS = (
    DecorationSpec(
        name="elder_oak",
        kind="tree",
        blocks=("oak_log", "oak_leaves", "moss_block"),
        size_range=(5, 9),
        rarity=0.45,
        notes="苍灵古橡：最常见的庇护木，树根蔓生苔藓。",
    ),
    DecorationSpec(
        name="memory_birch",
        kind="tree",
        blocks=("birch_log", "birch_leaves"),
        size_range=(6, 10),
        rarity=0.30,
        notes="忆白桦：树皮如残碑纹路，初醒修士的路标。",
    ),
    DecorationSpec(
        name="starter_shrub",
        kind="shrub",
        blocks=("sweet_berry_bush", "grass", "fern"),
        size_range=(1, 2),
        rarity=0.05,
        notes="野浆灌：可采食浆果。rarity 0.05 ≈ 净 1% 命中（× density × cluster gate）。"
              "accent 是矮草（曾错为 grass_block 实体方块导致地表凸起）。",
    ),
    DecorationSpec(
        name="wayfarer_rock",
        kind="boulder",
        blocks=("mossy_cobblestone", "cobblestone", "stone"),
        size_range=(2, 4),
        rarity=0.40,
        notes="行者石：长满苔藓的路边巨石，曾被旅人坐过。",
    ),
    # Ground cover specs（kind="flower"，由 ground_cover_id 引用而非 flora_variant_id）
    DecorationSpec(
        name="meadow_grass",
        kind="flower",
        blocks=("grass",),
        size_range=(1, 1),
        rarity=0.20,
        notes="草甸短草：末法新手平原零星点缀（rarity 从 0.75 → 0.20，原值"
              "vanilla 草甸般密集，违和）。",
    ),
    DecorationSpec(
        name="meadow_dandelion",
        kind="flower",
        blocks=("dandelion",),
        size_range=(1, 1),
        rarity=0.08,
        notes="蒲公英：偶现点缀（原 0.30 → 0.08）。",
    ),
    DecorationSpec(
        name="meadow_poppy",
        kind="flower",
        blocks=("poppy",),
        size_range=(1, 1),
        rarity=0.06,
        notes="虞美人：花林群系点缀（原 0.25 → 0.06）。",
    ),
    # Fallen log + grave mound — 半成品视觉装饰，用 ground_cover_id 引用
    DecorationSpec(
        name="fallen_oak_log",
        kind="fallen_log",
        blocks=("oak_log",),
        size_range=(3, 5),
        rarity=0.05,
        notes="倒木：随机 N/S/E/W 横躺的橡木原木 3-5 段。",
    ),
    DecorationSpec(
        name="wayfarer_grave",
        kind="grave_mound",
        blocks=("cobblestone", "mossy_cobblestone", "oak_sign"),
        size_range=(4, 5),
        rarity=0.03,
        notes="路人坟：半圆苔石堆（半径 4-5）+中央立碑（修仙荒野感）。"
              "blocks[0] cobblestone 主体, [1] mossy_cobblestone 表层苔藓, [2] oak_sign 碑"
              "（碑文待 NBT 实现，先放空牌）。",
    ),
)


def _distance_xz(a: tuple[float, float, float], b: tuple[float, float, float]) -> float:
    dx = a[0] - b[0]
    dz = a[2] - b[2]
    return float((dx * dx + dz * dz) ** 0.5)


def _fallback_lingquan_positions(
    spawn_center: tuple[float, float],
    limit: int,
) -> list[tuple[float, float, float]]:
    center_x, center_z = spawn_center
    return [
        (center_x + offset_x, 65.0, center_z + offset_z)
        for offset_x, offset_z in TUTORIAL_LINGQUAN_FALLBACK_OFFSETS[:limit]
    ]


def dynamic_lingquan_selector(
    spawn_center: tuple[float, float],
    qi_density: np.ndarray | None = None,
    height: np.ndarray | None = None,
    wx: np.ndarray | None = None,
    wz: np.ndarray | None = None,
    radius: float = TUTORIAL_LINGQUAN_SCAN_RADIUS,
    threshold: float = TUTORIAL_LINGQUAN_QI_THRESHOLD,
    limit: int = 2,
) -> tuple[tuple[float, float, float], ...]:
    """Pick tutorial lingquan points near spawn from high-qi cells.

    The function is pure and deterministic. Raster generation passes field
    arrays to scan real qi values; manifest export can call it without arrays and
    receives the stable fallback anchors that this profile also bumps to qi>=0.5.
    """

    selected: list[tuple[float, float, float]] = []
    center_x, center_z = spawn_center
    if (
        qi_density is not None
        and height is not None
        and wx is not None
        and wz is not None
    ):
        dist2 = (wx - center_x) ** 2 + (wz - center_z) ** 2
        mask = (dist2 <= radius * radius) & (qi_density >= threshold)
        candidates = np.argwhere(mask)
        ranked = sorted(
            candidates,
            key=lambda idx: (
                -float(qi_density[int(idx[0]), int(idx[1])]),
                float(dist2[int(idx[0]), int(idx[1])]),
            ),
        )
        for row, col in ranked:
            pos = (
                float(wx[int(row), int(col)]),
                float(height[int(row), int(col)] + 1.0),
                float(wz[int(row), int(col)]),
            )
            if all(_distance_xz(pos, existing) >= TUTORIAL_LINGQUAN_MIN_SEPARATION for existing in selected):
                selected.append(pos)
            if len(selected) >= limit:
                break

    for fallback in _fallback_lingquan_positions(spawn_center, limit):
        if len(selected) >= limit:
            break
        if all(_distance_xz(fallback, existing) >= TUTORIAL_LINGQUAN_MIN_SEPARATION for existing in selected):
            selected.append(fallback)

    return tuple(selected[:limit])


def spawn_tutorial_pois_for_zone(zone: BlueprintZone) -> tuple[PoiSpec, ...]:
    center_x, center_z = zone.center_xz
    lingquans = dynamic_lingquan_selector((float(center_x), float(center_z)))
    first_lingquan = lingquans[0]
    rat_anchor = (
        (center_x + first_lingquan[0]) * 0.5,
        first_lingquan[1],
        (center_z + first_lingquan[2]) * 0.5,
    )

    pois = [
        PoiSpec(
            kind="spawn_tutorial_coffin",
            name="半埋石棺",
            pos_xyz=(float(center_x), 69.0, float(center_z)),
            tags=("spawn_tutorial", "coffin", "loot:spirit_niche_stone"),
            qi_affinity=0.05,
        ),
        PoiSpec(
            kind="tutorial_chest",
            name="灵泉边小匣",
            pos_xyz=(first_lingquan[0] + 5.0, first_lingquan[1], first_lingquan[2]),
            tags=("spawn_tutorial", "loot:kaimai_dan", "near_lingquan:1"),
            qi_affinity=0.10,
        ),
        PoiSpec(
            kind="tutorial_rogue_anchor",
            name="踽行散修",
            pos_xyz=(float(center_x + 35), 70.0, float(center_z - 45)),
            tags=("spawn_tutorial", "rogue", "killable"),
            qi_affinity=0.02,
        ),
        PoiSpec(
            kind="tutorial_rat_path",
            name="鼠群擦痕",
            pos_xyz=rat_anchor,
            tags=("spawn_tutorial", "rat_swarm", "placeholder:zombie"),
            danger_bias=1,
        ),
    ]
    for idx, pos in enumerate(lingquans, start=1):
        pois.append(
            PoiSpec(
                kind="tutorial_lingquan",
                name=f"教学灵泉 #{idx}",
                pos_xyz=pos,
                tags=("spawn_tutorial", f"index:{idx}", "qi:0.5"),
                qi_affinity=0.35,
            )
        )
    return tuple(pois)


class SpawnPlainGenerator(TerrainProfileGenerator):
    profile_name = "spawn_plain"
    extra_layers = (
        "qi_density",
        "mofa_decay",
        "flora_density",
        "flora_variant_id",
        "ground_cover_density",
        "ground_cover_id",
    )
    ecology = EcologySpec(
        decorations=SPAWN_PLAIN_DECORATIONS,
        ambient_effects=("morning_mist", "distant_bird_call"),
        notes="初醒原生态：暖色温和，古橡与忆白桦点缀开阔草甸，野浆灌丛可食。"
              "给人'世界尚可'的第一印象。",
    )

    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        return (
            "Low-relief onboarding terrain.",
            "Keep traversal readable and avoid major obstacles.",
        )


def fill_spawn_plain_tile(
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
            "flora_density",
            "flora_variant_id",
            "ground_cover_density",
            "ground_cover_id",
        ),
    )
    grass_id = palette.ensure("grass_block")
    podzol_id = palette.ensure("podzol")
    dirt_id = palette.ensure("dirt")
    coarse_dirt_id = palette.ensure("coarse_dirt")
    gravel_id = palette.ensure("gravel")
    stone_id = palette.ensure("stone")
    spawn_biome_id = 4
    flower_forest_biome_id = 11

    center_x, center_z = zone.center_xz
    half_w = max(zone.size_xz[0] * 0.5, 1.0)
    half_d = max(zone.size_xz[1] * 0.5, 1.0)

    wx, wz = _tile_coords(tile.min_x, tile.min_z, tile_size)
    dx = (wx - center_x) / half_w
    dz = (wz - center_z) / half_d
    radial = np.sqrt(dx * dx + dz * dz)
    heartland = np.maximum(0.0, 1.0 - radial**1.9)
    inner_meadow = np.maximum(0.0, 1.0 - radial**2.8)

    # Gentle rolling hills — large-scale FBM
    rolling = fbm_2d(wx, wz, scale=320.0, octaves=4, seed=10) * 2.3
    # Organic swale depressions — domain-warped for natural curves
    swale = warped_fbm_2d(
        wx, wz, scale=180.0, octaves=3, warp_scale=350.0, warp_strength=60.0, seed=20
    )
    # Path-like ridges
    path = fbm_2d(wx, wz, scale=220.0, octaves=3, seed=30)

    height = 69.0 + heartland * 3.8 + rolling * 0.8 - inner_meadow * 1.2
    # Occasional ponds in swale depressions
    pond_mask = (heartland > 0.14) & (swale < -0.55)
    water_level = np.where(pond_mask, 66.8, -1.0)
    height = np.where(pond_mask, height - (-0.55 - swale) * 4.0, height)

    # Surface
    surface_id = np.full_like(height, dirt_id, dtype=np.int32)
    surface_id = np.where(inner_meadow > 0.5, grass_id, surface_id)
    surface_id = np.where(
        (heartland > 0.34) & (np.abs(rolling) < 1.6), grass_id, surface_id
    )
    surface_id = np.where(swale < -0.6, coarse_dirt_id, surface_id)
    surface_id = np.where(np.abs(rolling) > 1.8, gravel_id, surface_id)
    surface_id = np.where(
        (water_level >= 0.0) & (height < water_level - 0.45), dirt_id, surface_id
    )
    surface_id = np.where((heartland > 0.56) & (path > 0.36), podzol_id, surface_id)

    feature_mask = np.minimum(
        1.0, 0.05 + (1.0 - inner_meadow) * 0.14 + np.abs(rolling) * 0.04
    )

    biome_id = np.where(feature_mask > 0.12, flower_forest_biome_id, spawn_biome_id)

    # 初醒原：灵气中低（0.25），末法轻度（0.22），含水塘处灵气略增（水即灵）
    qi_base = float(getattr(zone, "spirit_qi", 0.3))
    qi_density = 0.18 + heartland * 0.10 + inner_meadow * 0.05
    qi_density = np.where(water_level >= 0.0, qi_density + 0.08, qi_density)
    qi_density = np.clip(qi_density * (0.5 + qi_base), 0.0, 1.0)
    mofa_decay = np.clip(0.28 - heartland * 0.10 + np.abs(rolling) * 0.03, 0.05, 0.55)

    lingquan_bump = np.zeros_like(height, dtype=np.float64)
    for lingquan_x, _, lingquan_z in dynamic_lingquan_selector((float(center_x), float(center_z))):
        dist = np.sqrt((wx - lingquan_x) ** 2 + (wz - lingquan_z) ** 2)
        lingquan_bump = np.maximum(lingquan_bump, np.clip(1.0 - dist / 8.0, 0.0, 1.0))
    qi_density = np.where(
        lingquan_bump > 0.0,
        np.maximum(qi_density, TUTORIAL_LINGQUAN_QI_THRESHOLD + lingquan_bump * 0.12),
        qi_density,
    )
    surface_id = np.where(lingquan_bump > 0.08, grass_id, surface_id)
    biome_id = np.where(lingquan_bump > 0.08, flower_forest_biome_id, biome_id)

    area = tile_size * tile_size
    buffer.layers["height"] = np.round(height, 3).ravel()
    buffer.layers["surface_id"] = surface_id.ravel().astype(np.uint8)
    buffer.layers["subsurface_id"] = np.full(area, stone_id, dtype=np.uint8)
    buffer.layers["water_level"] = np.round(water_level, 3).ravel()
    buffer.layers["biome_id"] = biome_id.ravel().astype(np.uint8)
    buffer.layers["feature_mask"] = np.round(feature_mask, 3).ravel()
    buffer.layers["boundary_weight"] = np.zeros(area, dtype=np.float64)
    buffer.layers["qi_density"] = np.round(qi_density, 3).ravel()
    buffer.layers["mofa_decay"] = np.round(mofa_decay, 3).ravel()

    # Flora: meadow-wide shrubs with scattered trees on heartland, boulders on edges
    flora_density = np.clip(heartland * 0.55 + inner_meadow * 0.15, 0.0, 1.0)
    flora_density = np.where(lingquan_bump > 0.0, np.maximum(flora_density, 0.35), flora_density)
    flora_variant = np.zeros_like(height, dtype=np.int32)
    # Fallen oak logs / grave mounds：用**两层 noise** 防止"成堆"——
    # 大尺度 fbm 选"哪几片区域允许出现"（即"林子"或"路边"），
    # 小尺度高频 fbm 在区域内打孔，让单格散点而非连片 blob。
    # 不这样做的话 fbm 平滑场会让一整片 cell 都标记 variant，server 端
    # 对每 cell 独立 roll 概率会变成"一片林子里到处都是倒木"。
    fallen_select = fbm_2d(wx, wz, scale=200.0, octaves=2, seed=8811)
    fallen_pick = fbm_2d(wx, wz, scale=10.0, octaves=1, seed=8812)
    fallen_band = (heartland > 0.30) & (fallen_select > 0.42) & (fallen_pick > 0.55)
    flora_variant = np.where(fallen_band, 8, flora_variant)
    flora_density = np.where(fallen_band, np.maximum(flora_density, 0.35), flora_density)
    grave_select = fbm_2d(wx, wz, scale=320.0, octaves=2, seed=8821)
    grave_pick = fbm_2d(wx, wz, scale=12.0, octaves=1, seed=8822)
    grave_band = (
        (heartland > 0.20)
        & (grave_select > 0.55)
        & (grave_pick > 0.65)
        & (flora_variant == 0)
    )
    flora_variant = np.where(grave_band, 9, flora_variant)
    flora_density = np.where(grave_band, np.maximum(flora_density, 0.30), flora_density)
    # Default shrub on remaining columns
    flora_variant = np.where((flora_variant == 0) & (flora_density > 0.20), 3, flora_variant)
    # Trees on heartland (覆盖 default shrub OR fallen/grave 让森林感更强)
    flora_variant = np.where((inner_meadow > 0.5) & (rolling > 0.3), 1, flora_variant)
    flora_variant = np.where((inner_meadow > 0.5) & (rolling < -0.2), 2, flora_variant)
    # Boulders on path-like ridges (同样可覆盖)
    flora_variant = np.where(path > 0.5, 4, flora_variant)
    buffer.layers["flora_density"] = np.round(flora_density, 3).ravel()
    buffer.layers["flora_variant_id"] = flora_variant.ravel().astype(np.uint8)

    # --- Ground cover (草甸短草 + 蒲公英 + 虞美人) ---
    # spawn_plain local_id 5=meadow_grass, 6=meadow_dandelion, 7=meadow_poppy。
    from . import global_decoration_id

    gc_grass = global_decoration_id("spawn_plain", 5)
    gc_dandelion = global_decoration_id("spawn_plain", 6)
    gc_poppy = global_decoration_id("spawn_plain", 7)

    # 末法新手平原：草甸稀疏感（不是 vanilla 一片绿）。原 0.30-0.55 配合
    # rarity 0.75 太密，回退至 0.05-0.15；lingquan_bump 周围保留小幅抬升做
    # 灵泉环绕 hint。
    gc_density = np.clip(0.05 + heartland * 0.08 + inner_meadow * 0.02, 0.0, 0.15)
    gc_density = np.where(lingquan_bump > 0.0, np.maximum(gc_density, 0.20), gc_density)
    on_grass = (surface_id == grass_id) | (surface_id == podzol_id)
    gc_density = np.where(on_grass, gc_density, 0.0)
    gc_density = np.where(water_level >= 0.0, 0.0, gc_density)
    buffer.layers["ground_cover_density"] = np.round(gc_density, 3).ravel()

    # 主体短草，flower_forest 区域多花，两种花用 large-scale fbm 区分
    # （而不是 path 高频 noise，避免邻列变种乱跳）
    gc_variant = np.full_like(height, gc_grass, dtype=np.int32)
    flower_zone = biome_id == flower_forest_biome_id
    flower_select = fbm_2d(wx, wz, scale=70.0, octaves=2, seed=4423)
    gc_variant = np.where(flower_zone & (flower_select > 0.18), gc_dandelion, gc_variant)
    gc_variant = np.where(flower_zone & (flower_select < -0.18), gc_poppy, gc_variant)
    gc_variant = np.where(gc_density <= 0.0, 0, gc_variant)
    buffer.layers["ground_cover_id"] = gc_variant.ravel().astype(np.uint8)

    buffer.contributing_zones.append(zone.name)
    return buffer
