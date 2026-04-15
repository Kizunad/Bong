from __future__ import annotations

from .abyssal_maze import AbyssalMazeGenerator
from .base import ProfileContext, TerrainProfileGenerator
from .broken_peaks import BrokenPeaksGenerator
from .cave_network import CaveNetworkGenerator
from .rift_valley import RiftValleyGenerator
from .sky_isle import SkyIsleGenerator
from .spawn_plain import SpawnPlainGenerator
from .spring_marsh import SpringMarshGenerator
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
        AbyssalMazeGenerator(),
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


__all__ = [
    "ProfileContext",
    "TerrainProfileGenerator",
    "get_profile_generator",
    "list_profile_generators",
]
