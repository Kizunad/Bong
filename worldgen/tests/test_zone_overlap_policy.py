"""Pin intentional zone nesting while rejecting accidental 3-D overlap."""

import itertools
import json
import sys
import unittest
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
WORLDGEN_ROOT = ROOT / "worldgen"
if str(WORLDGEN_ROOT) not in sys.path:
    sys.path.insert(0, str(WORLDGEN_ROOT))
FILES = (ROOT / "server/zones.json", ROOT / "server/zones.worldview.example.json")
DESIGNED_OVERLAPS = {
    frozenset(("rift_mouth_blood_001", "blood_valley")),
    frozenset(("rift_mouth_north_001", "jiuzong_beiling_ruin")),
    frozenset(("baolongwang_cavern_deep", "zhanhun_plain")),
    frozenset(("blood_valley", "zhanhun_plain")),
    frozenset(("north_waste_east_scorch", "north_wastes")),
}
# This separately planned defect is a regression baseline, not a design allowlist entry.
# plan-bughunt-sword-sea-zone-overlap-v1 owns its removal.
KNOWN_DEFECT_OVERLAPS_BY_FILE = {
    "zones.json": {frozenset(("giant_sword_sea", "wuxing_abyss"))},
    "zones.worldview.example.json": set(),
}

DIMENSION_ALIASES = {
    "overworld": "overworld",
    "minecraft:overworld": "overworld",
    "tsy": "tsy",
    "bong:tsy": "tsy",
}


def _zones(path: Path) -> list[dict[str, Any]]:
    document = json.loads(path.read_text())
    zones = document.get("zones")
    assert isinstance(zones, list), f"{path} must contain a zones array"
    return zones


def _overlaps(a: dict[str, Any], b: dict[str, Any]) -> bool:
    return all(
        a["aabb"]["min"][axis] <= b["aabb"]["max"][axis]
        and b["aabb"]["min"][axis] <= a["aabb"]["max"][axis]
        for axis in range(3)
    )


def _effective_dimension(zone: dict[str, Any]) -> str:
    """Normalize runtime and blueprint dimension spellings."""

    raw = str(zone.get("dimension", "overworld")).strip().lower()
    try:
        return DIMENSION_ALIASES[raw]
    except KeyError as error:
        raise AssertionError(
            f"zone {zone.get('name', '<unnamed>')!r} has unsupported dimension {raw!r}"
        ) from error


def _overlap_pairs(zones: list[dict[str, Any]]) -> set[frozenset[str]]:
    return {
        frozenset((a["name"], b["name"]))
        for a, b in itertools.combinations(zones, 2)
        if _effective_dimension(a) == _effective_dimension(b) and _overlaps(a, b)
    }


def _policy_violations(
    zones: list[dict[str, Any]],
    designed: set[frozenset[str]],
    known_defects: set[frozenset[str]],
) -> tuple[
    set[frozenset[str]],
    set[frozenset[str]],
    set[frozenset[str]],
    set[frozenset[str]],
]:
    actual = _overlap_pairs(zones)
    return (
        actual,
        designed - actual,
        actual - designed - known_defects,
        known_defects - actual,
    )


def _fixture_zone(
    name: str,
    minimum: tuple[float, float, float],
    maximum: tuple[float, float, float],
    dimension: str | None = None,
) -> dict[str, Any]:
    zone: dict[str, Any] = {
        "name": name,
        "aabb": {"min": list(minimum), "max": list(maximum)},
    }
    if dimension is not None:
        zone["dimension"] = dimension
    return zone


class ZoneOverlapPolicyTest(unittest.TestCase):
    def test_only_reviewed_zone_pairs_overlap_in_three_dimensions(self) -> None:
        for path in FILES:
            zones = _zones(path)
            known_defects = KNOWN_DEFECT_OVERLAPS_BY_FILE[path.name]
            actual, missing_designed, unexpected, stale_known = _policy_violations(
                zones, DESIGNED_OVERLAPS, known_defects
            )
            self.assertFalse(
                missing_designed,
                f"{path} lost reviewed 3-D zone overlaps: {missing_designed}; "
                "update the design allowlist only when the owning plan intentionally "
                "changes that nesting",
            )
            self.assertFalse(
                unexpected, f"{path} has unreviewed 3-D zone overlaps: {unexpected}"
            )
            self.assertFalse(
                stale_known,
                f"{path} changed the known-defect baseline; remove the fixed pair from "
                f"KNOWN_DEFECT_OVERLAPS_BY_FILE in the owning plan: {stale_known}",
            )

    def test_overlap_pairs_follow_runtime_dimension_contract(self) -> None:
        zones = [
            _fixture_zone("default_overworld", (0, 0, 0), (10, 10, 10)),
            _fixture_zone(
                "explicit_overworld",
                (0, 0, 0),
                (10, 10, 10),
                "minecraft:overworld",
            ),
            _fixture_zone("tsy_same_xyz", (0, 0, 0), (10, 10, 10), "tsy"),
            _fixture_zone(
                "tsy_ident_same_xyz", (0, 0, 0), (10, 10, 10), "bong:tsy"
            ),
        ]

        self.assertEqual(
            _overlap_pairs(zones),
            {
                frozenset(("default_overworld", "explicit_overworld")),
                frozenset(("tsy_same_xyz", "tsy_ident_same_xyz")),
            },
            "same XYZ in TSY must not compete with overworld ZoneRegistry lookups",
        )

    def test_overlap_geometry_pins_touching_boundary_and_strict_gap(self) -> None:
        base = _fixture_zone("base", (0, 0, 0), (10, 10, 10))
        touching_cases = {
            "face": _fixture_zone("other", (10, 2, 2), (20, 8, 8)),
            "edge": _fixture_zone("other", (10, 10, 2), (20, 20, 8)),
            "point": _fixture_zone("other", (10, 10, 10), (20, 20, 20)),
        }
        separated = [
            base,
            _fixture_zone("other", (10.001, 0, 0), (20, 10, 10)),
        ]

        for label, other in touching_cases.items():
            with self.subTest(label=label):
                self.assertEqual(
                    _overlap_pairs([base, other]),
                    {frozenset(("base", "other"))},
                    "inclusive AABBs sharing a face, edge, or point still overlap",
                )
        self.assertEqual(_overlap_pairs(separated), set())

    def test_policy_classifier_pins_unexpected_missing_and_stale_known(self) -> None:
        pair = frozenset(("a", "b"))
        zones = [
            _fixture_zone("a", (0, 0, 0), (10, 10, 10)),
            _fixture_zone("b", (5, 0, 0), (15, 10, 10)),
        ]

        actual, missing, unexpected, stale = _policy_violations(zones, set(), set())
        self.assertEqual(actual, {pair})
        self.assertEqual(unexpected, {pair})
        self.assertEqual(missing, set())
        self.assertEqual(stale, set())

        _, missing, unexpected, stale = _policy_violations(zones, {pair}, set())
        self.assertEqual(missing, set(), "present designed overlap must stay accepted")
        self.assertEqual(unexpected, set())
        self.assertEqual(stale, set())

        _, missing, unexpected, stale = _policy_violations(zones, set(), {pair})
        self.assertEqual(missing, set())
        self.assertEqual(unexpected, set(), "present known defect is tracked, not unexpected")
        self.assertEqual(stale, set())

        _, missing, unexpected, stale = _policy_violations([], {pair}, {pair})
        self.assertEqual(missing, {pair})
        self.assertEqual(unexpected, set())
        self.assertEqual(stale, {pair})

    def test_unknown_dimension_fails_closed(self) -> None:
        with self.assertRaisesRegex(AssertionError, "unsupported dimension"):
            _overlap_pairs(
                [
                    _fixture_zone("bad", (0, 0, 0), (1, 1, 1), "the_end"),
                    _fixture_zone("other", (0, 0, 0), (1, 1, 1)),
                ]
            )

    def test_north_rift_geometry_matches_runtime_and_blueprint(self) -> None:
        runtime, blueprint = (_zones(path) for path in FILES)
        for zones in (runtime, blueprint):
            by_name = {zone["name"]: zone for zone in zones}
            self.assertFalse(
                _overlaps(
                    by_name["rift_mouth_north_002"],
                    by_name["north_waste_east_scorch"],
                ),
                "north rift and scorch zones must be mutually exclusive",
            )
        runtime_rift = next(z for z in runtime if z["name"] == "rift_mouth_north_002")
        blueprint_rift = next(
            z for z in blueprint if z["name"] == "rift_mouth_north_002"
        )
        self.assertEqual(runtime_rift["aabb"], blueprint_rift["aabb"])
        self.assertEqual(
            runtime_rift["patrol_anchors"], blueprint_rift["patrol_anchors"]
        )
        rift_min = blueprint_rift["aabb"]["min"]
        rift_max = blueprint_rift["aabb"]["max"]
        expected_center_xz = [
            (rift_min[0] + rift_max[0]) / 2.0,
            (rift_min[2] + rift_max[2]) / 2.0,
        ]
        expected_size_xz = [
            rift_max[0] - rift_min[0],
            rift_max[2] - rift_min[2],
        ]
        self.assertEqual(
            blueprint_rift["center_xz"],
            expected_center_xz,
            "north rift center_xz must remain the exact XZ midpoint of its AABB",
        )
        self.assertEqual(
            blueprint_rift["size_xz"],
            expected_size_xz,
            "north rift size_xz must remain the exact XZ span of its AABB",
        )
        self.assertEqual(
            blueprint_rift["worldgen"]["portal_anchor_xz"], [2000.0, -7300.0]
        )

    def test_north_rift_portal_survives_default_blueprint_manifest_export_contract(
        self,
    ) -> None:
        from scripts.terrain_gen.bakers.raster_export import _collect_poi_payload
        from scripts.terrain_gen.blueprint import DEFAULT_BLUEPRINT_PATH, load_blueprint

        blueprint = load_blueprint(DEFAULT_BLUEPRINT_PATH)
        zone = next(
            zone for zone in blueprint.zones if zone.name == "rift_mouth_north_002"
        )
        source_portals = [
            poi
            for poi in zone.pois
            if poi.kind == "rift_portal" and poi.name == "塌缩裂缝·北荒东陲"
        ]
        self.assertEqual(len(source_portals), 1, "target rift portal must be unique by kind/name")

        payload_portals = [
            poi
            for poi in _collect_poi_payload(list(blueprint.zones))
            if poi["zone"] == "rift_mouth_north_002"
            and poi["kind"] == "rift_portal"
            and poi["name"] == "塌缩裂缝·北荒东陲"
        ]
        self.assertEqual(
            len(payload_portals),
            1,
            "production raster manifest collector must retain exactly one target portal",
        )
        portal = payload_portals[0]
        self.assertEqual(portal["pos_xyz"], [2000.0, 74.0, -7300.0])
        self.assertNotEqual(portal["pos_xyz"], [2000.0, 74.0, -7800.0])
        tags = {
            key: value
            for raw in portal["tags"]
            if ":" in raw
            for key, value in [raw.split(":", 1)]
        }
        self.assertEqual(tags["direction"], "entry")
        self.assertEqual(tags["kind"], "main")
        self.assertEqual(tags["family_id"], "zongmen_01")
        self.assertEqual(tags["target_family_pos_xyz"], "250,100,250")
        self.assertEqual(tags["trigger_radius"], "2.0")

    def test_relocated_north_rift_neighbourhood_qi_matches_unified_field_bake(
        self,
    ) -> None:
        from scripts.terrain_gen.blueprint import DEFAULT_BLUEPRINT_PATH, load_blueprint
        from scripts.terrain_gen.zones_export import bake_zone_qi

        blueprint = load_blueprint(DEFAULT_BLUEPRINT_PATH)
        world_area = float(
            (blueprint.bounds_xz.max_x - blueprint.bounds_xz.min_x)
            * (blueprint.bounds_xz.max_z - blueprint.bounds_xz.min_z)
        )
        baked = bake_zone_qi(blueprint.zones, world_area=world_area)
        runtime_by_name = {
            zone["name"]: zone for zone in _zones(ROOT / "server/zones.json")
        }
        for zone_name in ("rift_mouth_north_002", "north_waste_east_scorch"):
            with self.subTest(zone_name=zone_name):
                self.assertEqual(
                    runtime_by_name[zone_name]["spirit_qi"],
                    round(baked.derived_spirit_qi[zone_name], 6),
                    f"{zone_name} runtime qi must match the relocated unified field bake",
                )
