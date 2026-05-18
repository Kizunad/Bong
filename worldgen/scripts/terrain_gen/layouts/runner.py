"""Layout runner — resolves POI center from BlueprintZone and places
structures according to a LayoutSpec.

NBT paste reads .nbt files via nbt_builder.load_structure() and emits a
block placement list (JSON-serializable) for the Rust server to consume at
chunk generation time. The worldgen Python pipeline stores heightmap /
density layers, not actual MC blocks, so structure blocks are exported as
a sidecar placement manifest rather than written into TileFieldBuffer.
"""

from __future__ import annotations

import json
import logging
import os
from dataclasses import dataclass, field
from typing import TYPE_CHECKING

from .base import LayoutResult, LayoutSpec, PlacedStructure

if TYPE_CHECKING:
    from ..blueprint import BlueprintZone

logger = logging.getLogger(__name__)


@dataclass
class BlockPlacement:
    """A single block to place in world coordinates."""

    world_pos: tuple[int, int, int]
    block_name: str
    properties: dict[str, str]

    def to_dict(self) -> dict:
        d: dict = {"pos": list(self.world_pos), "block": self.block_name}
        if self.properties:
            d["properties"] = self.properties
        return d


@dataclass
class NbtPasteResult:
    """Result of pasting an NBT structure."""

    nbt_path: str
    world_pos: tuple[int, int, int]
    rotation: int
    block_count: int
    blocks: list[BlockPlacement] = field(default_factory=list)


def _rotate_offset(
    dx: int, dy: int, dz: int, rotation: int
) -> tuple[int, int, int]:
    """Rotate a local (dx, dy, dz) offset by rotation degrees around Y axis."""
    if rotation == 0:
        return dx, dy, dz
    if rotation == 90:
        return -dz, dy, dx
    if rotation == 180:
        return -dx, dy, -dz
    if rotation == 270:
        return dz, dy, -dx
    raise ValueError(f"Invalid rotation: {rotation}")


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


def _load_nbt_blocks(nbt_path: str) -> list[tuple[tuple[int, int, int], str, dict[str, str]]]:
    """Load blocks from an NBT file. Returns list of (pos, block_name, properties).

    Uses scripts/nbt/nbt_builder.load_structure() for parsing.
    Falls back to empty list if file not found (logs warning).
    """
    if not os.path.isfile(nbt_path):
        logger.warning("NBT file not found: %s — producing empty placement", nbt_path)
        return []

    try:
        # Import from the nbt_builder module
        import sys
        from pathlib import Path

        nbt_builder_dir = str(Path(__file__).resolve().parents[4] / "scripts" / "nbt")
        if nbt_builder_dir not in sys.path:
            sys.path.insert(0, nbt_builder_dir)

        from nbt_builder import load_structure

        structure_blocks = load_structure(nbt_path)
        return [
            (sb.pos, sb.block_name, sb.properties) for sb in structure_blocks
        ]
    except Exception as exc:
        logger.warning("Failed to load NBT %s: %s — producing empty placement", nbt_path, exc)
        return []


def _paste_nbt(
    world_pos: tuple[int, int, int], rotation: int, nbt_path: str
) -> NbtPasteResult:
    """Paste an NBT structure at *world_pos* with rotation.

    Reads the .nbt file, applies rotation to each block's local offset,
    and produces a list of BlockPlacement entries with world coordinates.
    These are collected into the LayoutResult for export as a sidecar
    manifest consumed by the Rust server at chunk generation time.
    """
    raw_blocks = _load_nbt_blocks(nbt_path)
    placements: list[BlockPlacement] = []

    for local_pos, block_name, properties in raw_blocks:
        dx, dy, dz = _rotate_offset(*local_pos, rotation)
        abs_pos = (
            world_pos[0] + dx,
            world_pos[1] + dy,
            world_pos[2] + dz,
        )
        placements.append(BlockPlacement(abs_pos, block_name, properties))

    result = NbtPasteResult(
        nbt_path=nbt_path,
        world_pos=world_pos,
        rotation=rotation,
        block_count=len(placements),
        blocks=placements,
    )
    logger.debug(
        "NBT paste: %s at %s rot=%d -> %d blocks",
        nbt_path, world_pos, rotation, len(placements),
    )
    return result


def export_placement_manifest(
    paste_results: list[NbtPasteResult], output_path: str
) -> None:
    """Write placement manifest JSON for the Rust server.

    Format:
    {
        "version": 1,
        "structures": [
            {
                "nbt_path": "...",
                "origin": [x, y, z],
                "rotation": 0,
                "blocks": [
                    {"pos": [x, y, z], "block": "minecraft:stone", "properties": {...}},
                    ...
                ]
            }
        ]
    }
    """
    structures = []
    for pr in paste_results:
        structures.append({
            "nbt_path": pr.nbt_path,
            "origin": list(pr.world_pos),
            "rotation": pr.rotation,
            "blocks": [b.to_dict() for b in pr.blocks],
        })

    manifest = {"version": 1, "structures": structures}
    with open(output_path, "w") as f:
        json.dump(manifest, f, indent=2)
    logger.info(
        "Wrote placement manifest: %s (%d structures, %d total blocks)",
        output_path,
        len(structures),
        sum(pr.block_count for pr in paste_results),
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
       stamp_radial).
    4. Collects all placed structures into a LayoutResult.

    NBT placements produce BlockPlacement lists (via _paste_nbt) that can
    be exported as a sidecar manifest for the Rust server.

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
        # Dispatch by kind
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
            continue

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
