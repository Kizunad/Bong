"""Layout runner — resolves POI center from BlueprintZone and places
structures according to a LayoutSpec.

NBT paste is currently a stub (logs intent, does not modify block data).
Real paste logic will be added in PR-3 when actual NBT files exist.
"""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING

from .base import LayoutResult, LayoutSpec, PlacedStructure

if TYPE_CHECKING:
    from ..blueprint import BlueprintZone

logger = logging.getLogger(__name__)


def _find_poi_center(
    zone: BlueprintZone, poi_kind: str
) -> tuple[int, int, int] | None:
    """Find the first POI matching *poi_kind* in the zone and return its
    integer world position, or ``None`` if not found."""
    for poi in zone.pois:
        if poi.kind == poi_kind:
            return (
                int(round(poi.pos_xyz[0])),
                int(round(poi.pos_xyz[1])),
                int(round(poi.pos_xyz[2])),
            )
    return None


def _paste_nbt(world_pos: tuple[int, int, int], rotation: int, nbt_path: str) -> None:
    """Stub: paste an NBT structure at *world_pos*.

    Real implementation will load .nbt files and write blocks into the
    chunk buffer.  For now just log so layout determinism tests can run.
    """
    logger.debug(
        "NBT paste stub: %s at %s rot=%d", nbt_path, world_pos, rotation
    )


def run_layout(
    spec: LayoutSpec, zone: BlueprintZone
) -> LayoutResult:
    """Execute a LayoutSpec against a BlueprintZone, returning a LayoutResult.

    The runner:
    1. Looks up the POI matching ``spec.poi_kind`` in ``zone.pois``.
    2. For each Placement, computes the absolute world position by adding
       the placement offset to the POI center.
    3. Dispatches to the appropriate paste function (NBT / block_grid /
       stamp_radial) — currently stubbed.
    4. Collects all placed structures into a LayoutResult.

    Raises:
        ValueError: if no POI matching ``spec.poi_kind`` is found in the zone.
    """
    center = _find_poi_center(zone, spec.poi_kind)
    if center is None:
        raise ValueError(
            f"Layout '{spec.name}' requires POI kind '{spec.poi_kind}' "
            f"but zone '{zone.name}' has no such POI. "
            f"Available POI kinds: {[p.kind for p in zone.pois]}"
        )

    placed: list[PlacedStructure] = []
    for placement in spec.placements:
        world_pos = (
            center[0] + placement.offset[0],
            center[1] + placement.offset[1],
            center[2] + placement.offset[2],
        )
        # Dispatch by kind (all currently stubbed)
        if placement.kind == "nbt":
            _paste_nbt(world_pos, placement.rotation, placement.payload)
        elif placement.kind == "block_grid":
            logger.debug(
                "block_grid stub: %s at %s rot=%d",
                placement.payload, world_pos, placement.rotation,
            )
        elif placement.kind == "stamp_radial":
            logger.debug(
                "stamp_radial stub: %s at %s rot=%d",
                placement.payload, world_pos, placement.rotation,
            )
        else:
            logger.warning(
                "Unknown placement kind '%s' for layout '%s'",
                placement.kind, spec.name,
            )

        placed.append(
            PlacedStructure(
                world_pos=world_pos,
                rotation=placement.rotation,
                kind=placement.kind,
                payload=placement.payload,
            )
        )

    return LayoutResult(
        layout_name=spec.name,
        poi_world_pos=center,
        placed=tuple(placed),
    )
