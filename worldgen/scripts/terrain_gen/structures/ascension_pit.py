from __future__ import annotations

from dataclasses import dataclass

from ..blueprint import BlueprintZone


XUJIE_CANXIE_ITEM_ID = "xujie_canxie"


@dataclass(frozen=True)
class AscensionPitSpec:
    zone: str
    name: str
    center_x: float
    center_z: float
    radius: float
    drop_chance: float

    def as_manifest(self) -> dict[str, object]:
        return {
            "zone": self.zone,
            "name": self.name,
            "kind": "tianjie_ascension_pit",
            "center_xz": [round(self.center_x), round(self.center_z)],
            "radius": round(self.radius),
            "blocks": ["basalt", "obsidian", "magma_block", "armor_stand"],
            "loot": [
                {
                    "template_id": XUJIE_CANXIE_ITEM_ID,
                    "display_name": "虚劫残屑",
                    "drop_chance": self.drop_chance,
                }
            ],
        }


def ascension_pits_for_zone(zone: BlueprintZone) -> list[dict[str, object]]:
    spec = _spec_for_zone(zone)
    return [] if spec is None else [spec.as_manifest()]


def _spec_for_zone(zone: BlueprintZone) -> AscensionPitSpec | None:
    if zone.worldgen.terrain_profile != "tribulation_scorch":
        return None
    raw = zone.worldgen.extras.get("ascension_pit_xz")
    if not isinstance(raw, (list, tuple)) or len(raw) != 2:
        return None
    radius = float(zone.worldgen.extras.get("ascension_pit_radius", 34.0))
    return AscensionPitSpec(
        zone=zone.name,
        name=f"{zone.name}_tianjie_ascension_pit",
        center_x=float(raw[0]),
        center_z=float(raw[1]),
        radius=radius,
        drop_chance=0.005,
    )


def _test_ascension_pits_emit_only_for_configured_scorch_zone() -> None:
    from ..blueprint import BoundarySpec, ZoneWorldgenConfig
    from ..fields import Bounds2D

    scorch = BlueprintZone(
        name="north_waste_east_scorch",
        display_name="北荒东陲焦土",
        bounds_xz=Bounds2D(1500, 2700, -8500, -7500),
        center_xz=(2100, -8000),
        size_xz=(1200, 1000),
        spirit_qi=0.28,
        danger_level=7,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="tribulation_scorch",
            shape="irregular_blob",
            boundary=BoundarySpec(mode="hard", width=80),
            height_model={},
            surface_palette=(),
            extras={"ascension_pit_xz": [2100.0, -8000.0]},
        ),
    )
    drift = BlueprintZone(
        name="drift_scorch_001",
        display_name="游离焦土",
        bounds_xz=Bounds2D(-4500, -3500, 3500, 4500),
        center_xz=(-4000, 4000),
        size_xz=(1000, 1000),
        spirit_qi=0.32,
        danger_level=5,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="tribulation_scorch",
            shape="irregular_blob",
            boundary=BoundarySpec(mode="hard", width=80),
            height_model={},
            surface_palette=(),
        ),
    )

    pits = ascension_pits_for_zone(scorch)
    assert len(pits) == 1
    assert pits[0]["center_xz"] == [2100, -8000]
    assert pits[0]["loot"][0]["template_id"] == XUJIE_CANXIE_ITEM_ID
    assert ascension_pits_for_zone(drift) == []


if __name__ == "__main__":
    _test_ascension_pits_emit_only_for_configured_scorch_zone()
