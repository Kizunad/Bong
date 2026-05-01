from __future__ import annotations

from .ash_dead_zone import AshDeadZoneGenerator
from .abyssal_maze import AbyssalMazeGenerator
from .ancient_battlefield import AncientBattlefieldGenerator
from .base import ProfileContext, TerrainProfileGenerator
from .broken_peaks import BrokenPeaksGenerator
from .cave_network import CaveNetworkGenerator
from .rift_valley import RiftValleyGenerator
from .sky_isle import SkyIsleGenerator
from .spawn_plain import SpawnPlainGenerator
from .spring_marsh import SpringMarshGenerator
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
        WastePlateauGenerator(),
        SkyIsleGenerator(),
        AshDeadZoneGenerator(),
        AbyssalMazeGenerator(),
        AncientBattlefieldGenerator(),
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


def _build_global_decoration_palette() -> tuple[
    tuple[dict[str, object], ...], dict[str, int]
]:
    palette: list[dict[str, object]] = []
    offsets: dict[str, int] = {}
    next_id = 1  # 0 reserved for "no decoration"
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
