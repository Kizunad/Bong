"""Pin intentional zone nesting while rejecting accidental 3-D overlap."""

import itertools
import json
import sys
from pathlib import Path

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


def _zones(path: Path) -> list[dict]:
    document = json.loads(path.read_text())
    zones = document.get("zones")
    assert isinstance(zones, list), f"{path} must contain a zones array"
    return zones


def _overlaps(a: dict, b: dict) -> bool:
    return all(
        a["aabb"]["min"][axis] <= b["aabb"]["max"][axis]
        and b["aabb"]["min"][axis] <= a["aabb"]["max"][axis]
        for axis in range(3)
    )


def test_only_reviewed_zone_pairs_overlap_in_three_dimensions() -> None:
    for path in FILES:
        zones = _zones(path)
        actual = {
            frozenset((a["name"], b["name"]))
            for a, b in itertools.combinations(zones, 2)
            if _overlaps(a, b)
        }
        known_defects = KNOWN_DEFECT_OVERLAPS_BY_FILE[path.name]
        unexpected = actual - DESIGNED_OVERLAPS - known_defects
        assert not unexpected, f"{path} has unreviewed 3-D zone overlaps: {unexpected}"
        assert known_defects <= actual, (
            f"{path} changed the known-defect baseline; remove the fixed pair from "
            "KNOWN_DEFECT_OVERLAPS_BY_FILE in the owning plan"
        )


def test_north_rift_geometry_matches_runtime_and_blueprint() -> None:
    runtime, blueprint = (_zones(path) for path in FILES)
    for zones in (runtime, blueprint):
        by_name = {zone["name"]: zone for zone in zones}
        assert not _overlaps(
            by_name["rift_mouth_north_002"], by_name["north_waste_east_scorch"]
        ), "north rift and scorch zones must be mutually exclusive"
    runtime_rift = next(z for z in runtime if z["name"] == "rift_mouth_north_002")
    blueprint_rift = next(z for z in blueprint if z["name"] == "rift_mouth_north_002")
    assert runtime_rift["aabb"] == blueprint_rift["aabb"]
    assert runtime_rift["patrol_anchors"] == blueprint_rift["patrol_anchors"]
    assert blueprint_rift["worldgen"]["portal_anchor_xz"] == [2000.0, -7300.0]
    assert blueprint_rift["pois"][0]["pos_xyz"] == [2000.0, 74.0, -7300.0]


def test_relocated_north_rift_runtime_qi_matches_unified_field_bake() -> None:
    from scripts.terrain_gen.blueprint import DEFAULT_BLUEPRINT_PATH, load_blueprint
    from scripts.terrain_gen.zones_export import bake_zone_qi

    blueprint = load_blueprint(DEFAULT_BLUEPRINT_PATH)
    world_area = float(
        (blueprint.bounds_xz.max_x - blueprint.bounds_xz.min_x)
        * (blueprint.bounds_xz.max_z - blueprint.bounds_xz.min_z)
    )
    baked = bake_zone_qi(blueprint.zones, world_area=world_area)
    runtime_rift = next(
        zone
        for zone in _zones(ROOT / "server/zones.json")
        if zone["name"] == "rift_mouth_north_002"
    )
    assert runtime_rift["spirit_qi"] == round(
        baked.derived_spirit_qi["rift_mouth_north_002"], 6
    )
