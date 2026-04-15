from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass

from ..blueprint import BlueprintZone, TerrainProfileSpec
from ..fields import DEFAULT_FIELD_LAYERS, ZoneFieldPlan


@dataclass(frozen=True)
class DecorationSpec:
    """A single piece of large-scale terrain flora/decor.

    The worldgen pipeline does NOT place these — it exports where they may go
    via the flora_density / flora_variant_id rasters. Rust (or a datapack)
    reads this spec to know HOW to build each variant: which block palette,
    what size, how rare. That keeps the Python pipeline clean (2D rasters
    only) while still carrying complete "what grows here" information.

    kind examples:
        "tree"      — classic trunk+canopy shape (or abstract equivalent)
        "shrub"     — low-lying vegetation, 1-3 blocks tall
        "boulder"   — large standalone rock mass
        "crystal"   — vertical crystalline structure (stalagmite-like)
        "mushroom"  — wide-capped growth (overworld / nether hybrid)
        "flower"    — small single-block flora
        "coral"     — branching underwater / hanging structure
    """

    name: str
    kind: str
    blocks: tuple[str, ...]
    size_range: tuple[int, int] = (3, 6)
    rarity: float = 0.5
    notes: str = ""


@dataclass(frozen=True)
class EcologySpec:
    """Per-profile ecology: flora/decor palette + ambient hints.

    Attached to each TerrainProfileGenerator. Serialized verbatim into the
    raster manifest so Rust / datapack consumers can place decorations
    without reading Python source.
    """

    decorations: tuple[DecorationSpec, ...] = ()
    ambient_effects: tuple[str, ...] = ()          # e.g. "qi_particles", "ash", "mist"
    notes: str = ""


@dataclass(frozen=True)
class ProfileContext:
    zone: BlueprintZone
    profile_spec: TerrainProfileSpec


_DEFAULT_ECOLOGY = EcologySpec()


class TerrainProfileGenerator(ABC):
    profile_name: str = ""
    extra_layers: tuple[str, ...] = ()
    ecology: EcologySpec = _DEFAULT_ECOLOGY

    def plan(self, context: ProfileContext) -> ZoneFieldPlan:
        required_layers = tuple(dict.fromkeys(DEFAULT_FIELD_LAYERS + self.extra_layers))
        return ZoneFieldPlan(
            zone_name=context.zone.name,
            display_name=context.zone.display_name,
            profile_name=self.profile_name,
            generator_name=self.__class__.__name__,
            shape=context.zone.worldgen.shape,
            bounds_xz=context.zone.bounds_xz,
            boundary_mode=context.zone.worldgen.boundary.mode,
            boundary_width=context.zone.worldgen.boundary.width,
            required_layers=required_layers,
            extra_layers=self.extra_layers,
            landmarks=context.zone.worldgen.landmarks,
            notes=self.build_notes(context),
        )

    @abstractmethod
    def build_notes(self, context: ProfileContext) -> tuple[str, ...]:
        raise NotImplementedError
