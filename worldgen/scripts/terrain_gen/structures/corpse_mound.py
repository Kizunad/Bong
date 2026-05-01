from __future__ import annotations

from dataclasses import dataclass

from ..blueprint import BlueprintZone


@dataclass(frozen=True)
class CorpseMoundLootEntry:
    template_id: str
    display_name: str
    min_count: int
    max_count: int

    def as_manifest(self) -> dict[str, object]:
        return {
            "template_id": self.template_id,
            "display_name": self.display_name,
            "count": [self.min_count, self.max_count],
        }


CORPSE_MOUND_LOOT_POOL = (
    CorpseMoundLootEntry("mineral_fan_tie", "凡铁", 1, 3),
    CorpseMoundLootEntry("rotten_bone_coin", "退活骨币", 2, 8),
    CorpseMoundLootEntry("dried_spirit_herb", "干灵草", 1, 2),
)


def corpse_mound_loot_pool() -> list[dict[str, object]]:
    return [entry.as_manifest() for entry in CORPSE_MOUND_LOOT_POOL]


def corpse_mounds_for_zone(zone: BlueprintZone) -> list[dict[str, object]]:
    if zone.worldgen.terrain_profile != "ash_dead_zone":
        return []
    half_w = zone.size_xz[0] * 0.5
    half_d = zone.size_xz[1] * 0.5
    anchors = (
        (-0.36, -0.22),
        (-0.08, 0.18),
        (0.28, -0.12),
    )
    return [
        {
            "zone": zone.name,
            "name": f"dried_corpse_mound_{idx}",
            "kind": "dried_corpse_mound",
            "center_xz": [round(zone.center_xz[0] + ax * half_w), round(zone.center_xz[1] + az * half_d)],
            "loot_pool": corpse_mound_loot_pool(),
            "search_seconds": [3, 5],
        }
        for idx, (ax, az) in enumerate(anchors, start=1)
    ]


def _test_corpse_mound_loot_pool_contains_three_required_fixtures() -> None:
    pool = corpse_mound_loot_pool()
    ids = {entry["template_id"] for entry in pool}
    assert {"mineral_fan_tie", "rotten_bone_coin", "dried_spirit_herb"}.issubset(ids)
    assert len(pool) >= 3


def _test_corpse_mounds_emit_only_for_ash_dead_zone() -> None:
    from ..blueprint import BoundarySpec, ZoneWorldgenConfig
    from ..fields import Bounds2D

    ash = BlueprintZone(
        name="south_ash_dead_zone",
        display_name="南荒余烬",
        bounds_xz=Bounds2D(-2200, -200, 7000, 9000),
        center_xz=(-1200, 8000),
        size_xz=(2000, 2000),
        spirit_qi=0.0,
        danger_level=5,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="ash_dead_zone",
            shape="irregular_blob",
            boundary=BoundarySpec(mode="hard", width=64),
            height_model={},
            surface_palette=(),
        ),
    )
    normal = BlueprintZone(
        name="north_wastes",
        display_name="北荒",
        bounds_xz=Bounds2D(-100, 100, -100, 100),
        center_xz=(0, 0),
        size_xz=(200, 200),
        spirit_qi=0.05,
        danger_level=3,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="waste_plateau",
            shape="plateau",
            boundary=BoundarySpec(mode="hard", width=32),
            height_model={},
            surface_palette=(),
        ),
    )

    mounds = corpse_mounds_for_zone(ash)
    assert len(mounds) == 3
    assert all(mound["kind"] == "dried_corpse_mound" for mound in mounds)
    assert corpse_mounds_for_zone(normal) == []


if __name__ == "__main__":
    _test_corpse_mound_loot_pool_contains_three_required_fixtures()
    _test_corpse_mounds_emit_only_for_ash_dead_zone()
