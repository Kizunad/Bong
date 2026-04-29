from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np

from ..blueprint import BlueprintZone, PoiSpec


@dataclass(frozen=True)
class WhaleFossilSpec:
    zone: str
    name: str
    center_x: float
    center_y: float
    center_z: float
    length: float
    width: float
    angle_deg: float
    max_units: int

    @property
    def bbox(self) -> tuple[int, int, int, int]:
        radius = math.hypot(self.length * 0.5, self.width * 0.5)
        return (
            math.floor(self.center_x - radius),
            math.ceil(self.center_x + radius),
            math.floor(self.center_z - radius),
            math.ceil(self.center_z + radius),
        )

    def as_manifest(self) -> dict[str, object]:
        min_x, max_x, min_z, max_z = self.bbox
        return {
            "zone": self.zone,
            "name": self.name,
            "center_xz": [round(self.center_x), round(self.center_z)],
            "center_y": round(self.center_y),
            "min_x": min_x,
            "max_x": max_x,
            "min_z": min_z,
            "max_z": max_z,
            "mask_values": {"outer": 1, "core": 2},
            "minerals": {
                "core": ["sui_tie", "ling_jing", "ling_shi_shang", "ling_shi_yi"],
                "outer": ["yu_sui", "ling_jing"],
            },
            "max_units": self.max_units,
        }


def fossil_bboxes_for_zone(zone: BlueprintZone) -> list[dict[str, object]]:
    return [spec.as_manifest() for spec in _specs_for_zone(zone)]


def rasterize_whale_fossil_mask(
    zone: BlueprintZone,
    wx: np.ndarray,
    wz: np.ndarray,
) -> np.ndarray:
    """Return uint8 fossil mask: 0 none, 1 outer ribs, 2 mineral-rich core."""
    mask = np.zeros_like(wx, dtype=np.uint8)
    for spec in _specs_for_zone(zone):
        along, cross = _rotated_coords(spec, wx, wz)
        half_len = max(spec.length * 0.5, 1.0)
        half_width = max(spec.width * 0.5, 1.0)
        ell = (along / half_len) ** 2 + (cross / half_width) ** 2
        body = ell <= 1.0

        spine = (np.abs(cross) <= spec.width * 0.08) & (np.abs(along) <= half_len)
        rib_spacing = max(spec.length / 9.0, 12.0)
        rib_phase = np.abs(np.sin((along + spec.length * 0.45) / rib_spacing * math.pi))
        rib_extent = half_width * (1.0 - np.clip(np.abs(along) / half_len, 0.0, 1.0) * 0.45)
        ribs = body & (rib_phase < 0.18) & (np.abs(cross) <= rib_extent)
        core = ((along / max(spec.length * 0.17, 1.0)) ** 2 + (cross / max(spec.width * 0.22, 1.0)) ** 2) <= 1.0

        mask = np.maximum(mask, np.where(body | spine | ribs, 1, 0).astype(np.uint8))
        mask = np.maximum(mask, np.where(core, 2, 0).astype(np.uint8))
    return mask


def _specs_for_zone(zone: BlueprintZone) -> list[WhaleFossilSpec]:
    extras = zone.worldgen.extras
    default_length = float(extras.get("whale_fossil_length", 220.0))
    default_width = float(extras.get("whale_fossil_width", 72.0))
    default_max_units = int(extras.get("whale_fossil_max_units", 180))
    specs: list[WhaleFossilSpec] = []
    for poi in zone.pois:
        if not _is_fossil_poi(poi):
            continue
        angle = float(extras.get("whale_fossil_angle_deg", _stable_angle(zone.name, poi)))
        specs.append(
            WhaleFossilSpec(
                zone=zone.name,
                name=poi.name or "whalefall_fossil",
                center_x=poi.pos_xyz[0],
                center_y=poi.pos_xyz[1],
                center_z=poi.pos_xyz[2],
                length=default_length,
                width=default_width,
                angle_deg=angle,
                max_units=default_max_units,
            )
        )
    return specs


def _is_fossil_poi(poi: PoiSpec) -> bool:
    tags = {tag.lower() for tag in poi.tags}
    return poi.kind in {"fossil", "tomb"} and "fossil" in tags


def _stable_angle(zone_name: str, poi: PoiSpec) -> float:
    seed = sum(zone_name.encode("utf-8"))
    seed += int(round(poi.pos_xyz[0])) * 17 + int(round(poi.pos_xyz[2])) * 31
    return float(seed % 46) - 23.0


def _rotated_coords(
    spec: WhaleFossilSpec,
    wx: np.ndarray,
    wz: np.ndarray,
) -> tuple[np.ndarray, np.ndarray]:
    angle = math.radians(spec.angle_deg)
    cos_a = math.cos(angle)
    sin_a = math.sin(angle)
    dx = wx - spec.center_x
    dz = wz - spec.center_z
    along = dx * cos_a - dz * sin_a
    cross = dx * sin_a + dz * cos_a
    return along, cross


def _test_rasterize_whale_fossil_mask_marks_core_and_outer() -> None:
    from ..blueprint import BoundarySpec, ZoneWorldgenConfig
    from ..fields import Bounds2D

    zone = BlueprintZone(
        name="north_wastes",
        display_name="北荒",
        bounds_xz=Bounds2D(-128, 128, -128, 128),
        center_xz=(0, 0),
        size_xz=(256, 256),
        spirit_qi=0.05,
        danger_level=5,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="waste_plateau",
            shape="plateau",
            boundary=BoundarySpec(mode="hard", width=32),
            height_model={},
            surface_palette=(),
            extras={"whale_fossil_length": 80, "whale_fossil_width": 32, "whale_fossil_angle_deg": 0},
        ),
        pois=(PoiSpec(kind="tomb", name="鲸坠骸骨", pos_xyz=(0.0, 76.0, 0.0), tags=("fossil",)),),
    )
    wx, wz = np.meshgrid(np.arange(-64, 65), np.arange(-64, 65))

    mask = rasterize_whale_fossil_mask(zone, wx, wz)

    assert int(mask.max()) == 2
    assert int(mask[64, 64]) == 2
    assert np.count_nonzero(mask == 1) > np.count_nonzero(mask == 2)
    assert int(mask[0, 0]) == 0


def _test_fossil_bboxes_for_zone_exports_manifest_contract() -> None:
    from ..blueprint import BoundarySpec, ZoneWorldgenConfig
    from ..fields import Bounds2D

    zone = BlueprintZone(
        name="north_wastes",
        display_name="北荒",
        bounds_xz=Bounds2D(-200, 200, -200, 200),
        center_xz=(0, 0),
        size_xz=(400, 400),
        spirit_qi=0.05,
        danger_level=5,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="waste_plateau",
            shape="plateau",
            boundary=BoundarySpec(mode="hard", width=32),
            height_model={},
            surface_palette=(),
            extras={"whale_fossil_length": 100, "whale_fossil_width": 40, "whale_fossil_max_units": 33},
        ),
        pois=(PoiSpec(kind="tomb", name="鲸坠骸骨", pos_xyz=(10.0, 76.0, -20.0), tags=("fossil", "lore")),),
    )

    bboxes = fossil_bboxes_for_zone(zone)

    assert len(bboxes) == 1
    bbox = bboxes[0]
    assert bbox["center_xz"] == [10, -20]
    assert bbox["center_y"] == 76
    assert bbox["max_units"] == 33
    assert bbox["mask_values"] == {"outer": 1, "core": 2}
    assert "sui_tie" in bbox["minerals"]["core"]
    assert "yu_sui" in bbox["minerals"]["outer"]


def _test_non_fossil_poi_does_not_emit_mask_or_bbox() -> None:
    from ..blueprint import BoundarySpec, ZoneWorldgenConfig
    from ..fields import Bounds2D

    zone = BlueprintZone(
        name="north_wastes",
        display_name="北荒",
        bounds_xz=Bounds2D(-64, 64, -64, 64),
        center_xz=(0, 0),
        size_xz=(128, 128),
        spirit_qi=0.05,
        danger_level=5,
        worldgen=ZoneWorldgenConfig(
            terrain_profile="waste_plateau",
            shape="plateau",
            boundary=BoundarySpec(mode="hard", width=32),
            height_model={},
            surface_palette=(),
        ),
        pois=(PoiSpec(kind="ruin", name="沉寂碎片", pos_xyz=(0.0, 76.0, 0.0), tags=("relic",)),),
    )
    wx, wz = np.meshgrid(np.arange(-16, 17), np.arange(-16, 17))

    assert fossil_bboxes_for_zone(zone) == []
    assert int(rasterize_whale_fossil_mask(zone, wx, wz).max()) == 0


if __name__ == "__main__":
    _test_rasterize_whale_fossil_mask_marks_core_and_outer()
    _test_fossil_bboxes_for_zone_exports_manifest_contract()
    _test_non_fossil_poi_does_not_emit_mask_or_bbox()
