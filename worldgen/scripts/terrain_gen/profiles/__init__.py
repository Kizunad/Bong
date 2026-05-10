from __future__ import annotations

from .ash_dead_zone import AshDeadZoneGenerator
from .abyssal_maze import AbyssalMazeGenerator
from .ancient_battlefield import AncientBattlefieldGenerator
from .base import DecorationSpec, ProfileContext, TerrainProfileGenerator
from .broken_peaks import BrokenPeaksGenerator
from .cave_network import CaveNetworkGenerator
from .jiu_zong_ruin import JiuzongRuinGenerator
from .pseudo_vein_oasis import PseudoVeinOasisGenerator
from .rift_mouth_barrens import RiftMouthBarrensGenerator
from .rift_valley import RiftValleyGenerator
from .sky_isle import SkyIsleGenerator
from .spawn_plain import SpawnPlainGenerator
from .spring_marsh import SpringMarshGenerator
from .tribulation_scorch import TribulationScorchGenerator
from .tsy_daneng_crater import TsyDanengCraterGenerator
from .tsy_gaoshou_hermitage import TsyGaoshouHermitageGenerator
from .tsy_zhanchang import TsyZhanchangGenerator
from .tsy_zongmen_ruin import TsyZongmenRuinGenerator
from .waste_plateau import WastePlateauGenerator

_GENERATORS: dict[str, TerrainProfileGenerator] = {
    generator.profile_name: generator
    for generator in (
        SpawnPlainGenerator(),
        BrokenPeaksGenerator(),
        SpringMarshGenerator(),
        RiftValleyGenerator(),
        CaveNetworkGenerator(),
        JiuzongRuinGenerator(),
        WastePlateauGenerator(),
        PseudoVeinOasisGenerator(),
        RiftMouthBarrensGenerator(),
        SkyIsleGenerator(),
        AshDeadZoneGenerator(),
        AbyssalMazeGenerator(),
        AncientBattlefieldGenerator(),
        TribulationScorchGenerator(),
        TsyZongmenRuinGenerator(),
        TsyDanengCraterGenerator(),
        TsyZhanchangGenerator(),
        TsyGaoshouHermitageGenerator(),
    )
}


def get_profile_generator(profile_name: str) -> TerrainProfileGenerator:
    try:
        return _GENERATORS[profile_name]
    except KeyError as exc:
        raise KeyError(
            f"No terrain profile generator registered for '{profile_name}'"
        ) from exc


def list_profile_generators() -> tuple[str, ...]:
    return tuple(sorted(_GENERATORS.keys()))


# --- Global decoration palette ------------------------------------------
# Each profile defines decorations with local ids 1..N. To let the Rust
# runtime consume flora_variant_id without knowing which profile authored a
# column (stitcher swap-blends across zone boundaries), we assign every
# decoration a stable **global** id 1..M and store the remap from
# (profile → offset) here. flora_variant_id rasters are written in global
# space by the stitcher; `global_decoration_palette` is exported into the
# raster manifest so Rust can look up block recipes by global id directly.


# Wilderness ground cover fallback —— wilderness 不是 TerrainProfileGenerator 子类
# （它是全局 fallback 区域），但 flora 系统需要 spec 才能放置地表植被。
# 这些 spec 占据 palette 开头的 wilderness 段，wilderness.py 通过
# global_decoration_id("wilderness", local_id) 引用它们写入 ground_cover_id。
WILDERNESS_GROUND_COVER: tuple[DecorationSpec, ...] = (
    DecorationSpec(
        name="wild_grass",
        kind="flower",
        blocks=("grass",),
        size_range=(1, 1),
        rarity=0.65,
        notes="野草：荒野最常见的地表覆盖。",
    ),
    DecorationSpec(
        name="wild_fern",
        kind="flower",
        blocks=("fern",),
        size_range=(1, 1),
        rarity=0.45,
        notes="野蕨：林下与潮湿洼地常见。",
    ),
    DecorationSpec(
        name="wild_dandelion",
        kind="flower",
        blocks=("dandelion",),
        size_range=(1, 1),
        rarity=0.20,
        notes="野蒲公英：开阔草地点缀。",
    ),
)


def _build_global_decoration_palette() -> tuple[
    tuple[dict[str, object], ...], dict[str, int]
]:
    palette: list[dict[str, object]] = []
    offsets: dict[str, int] = {}
    next_id = 1  # 0 reserved for "no decoration"

    # Emit wilderness fallback段 first so其 global_id 稳定 (1..N)。
    offsets["wilderness"] = next_id
    for local_idx, deco in enumerate(WILDERNESS_GROUND_COVER):
        palette.append(
            {
                "global_id": next_id + local_idx,
                "profile": "wilderness",
                "local_id": local_idx + 1,
                "name": deco.name,
                "kind": deco.kind,
                "blocks": list(deco.blocks),
                "size_range": list(deco.size_range),
                "rarity": deco.rarity,
                "notes": deco.notes,
            }
        )
    next_id += len(WILDERNESS_GROUND_COVER)

    for profile_name in sorted(_GENERATORS):
        gen = _GENERATORS[profile_name]
        offsets[profile_name] = next_id
        for local_idx, deco in enumerate(gen.ecology.decorations):
            palette.append(
                {
                    "global_id": next_id + local_idx,
                    "profile": profile_name,
                    "local_id": local_idx + 1,
                    "name": deco.name,
                    "kind": deco.kind,
                    "blocks": list(deco.blocks),
                    "size_range": list(deco.size_range),
                    "rarity": deco.rarity,
                    "notes": deco.notes,
                }
            )
        next_id += len(gen.ecology.decorations)
    return tuple(palette), offsets


GLOBAL_DECORATION_PALETTE, PROFILE_DECORATION_OFFSETS = _build_global_decoration_palette()


def global_decoration_id(profile_name: str, local_id: int) -> int:
    """Convert (profile, local id 1..N) → global id, or 0 if local_id is 0."""
    if local_id == 0:
        return 0
    offset = PROFILE_DECORATION_OFFSETS.get(profile_name, 0)
    if offset == 0:
        return 0
    return offset + (local_id - 1)


__all__ = [
    "ProfileContext",
    "TerrainProfileGenerator",
    "get_profile_generator",
    "list_profile_generators",
    "GLOBAL_DECORATION_PALETTE",
    "PROFILE_DECORATION_OFFSETS",
    "global_decoration_id",
]
