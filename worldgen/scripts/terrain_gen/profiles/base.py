from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass

from ..blueprint import BlueprintZone, TerrainProfileSpec
from ..fields import DEFAULT_FIELD_LAYERS, ZoneFieldPlan


@dataclass(frozen=True)
class ProfileContext:
    zone: BlueprintZone
    profile_spec: TerrainProfileSpec


class TerrainProfileGenerator(ABC):
    profile_name: str = ""
    extra_layers: tuple[str, ...] = ()

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
