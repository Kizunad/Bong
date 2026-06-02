"""Layout runner — resolves POI center from BlueprintZone and places
structures according to a LayoutSpec.

NBT paste reads .nbt files via nbt_builder.load_structure() and emits a
block placement list (JSON-serializable) for the Rust server to consume at
chunk generation time. The worldgen Python pipeline stores heightmap /
density layers, not actual MC blocks, so structure blocks are exported as
a sidecar placement manifest rather than written into TileFieldBuffer.

B2 (plan-terrain-wiring-v1 P0): run_layout now accumulates NbtPasteResult
from every _paste_nbt / _stamp_radial / _block_grid call and stores them in
LayoutResult.paste_results, so callers can feed the list straight into
export_placement_manifest().

B3: stamp_radial and block_grid are no longer empty stubs — they produce
real NbtPasteResult.blocks entries that the Rust server can stamp.

M3: _paste_nbt rotates blockstate properties (facing/axis/rotation) in
addition to coordinates, so the manifest is already in final orientation.
Server does NOT rotate again.
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
    """Result of pasting an NBT structure (or inline block grid / stamp)."""

    nbt_path: str
    world_pos: tuple[int, int, int]
    rotation: int
    block_count: int
    blocks: list[BlockPlacement] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Rotation helpers
# ---------------------------------------------------------------------------


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


# Canonical MC blockstate direction names for each rotation step.
# We rotate clockwise (same convention as _rotate_offset).
_FACING_ROTATION: dict[str, dict[int, str]] = {
    # 0=north/east/south/west
    "north": {0: "north", 90: "west",  180: "south", 270: "east"},
    "east":  {0: "east",  90: "north", 180: "west",  270: "south"},
    "south": {0: "south", 90: "east",  180: "north", 270: "west"},
    "west":  {0: "west",  90: "south", 180: "east",  270: "north"},
    # up/down are rotation-invariant
    "up":    {0: "up",    90: "up",    180: "up",    270: "up"},
    "down":  {0: "down",  90: "down",  180: "down",  270: "down"},
}

_AXIS_ROTATION: dict[str, dict[int, str]] = {
    "x": {0: "x", 90: "z", 180: "x", 270: "z"},
    "y": {0: "y", 90: "y", 180: "y", 270: "y"},
    "z": {0: "z", 90: "x", 180: "z", 270: "x"},
}


def _rotate_properties(
    properties: dict[str, str], rotation: int
) -> dict[str, str]:
    """Rotate blockstate direction properties by *rotation* degrees around Y.

    Handles:
      - ``facing`` (stairs, doors, hoppers, dispensers, …)
      - ``axis``   (logs, pillars, …)
      - ``rotation`` (banners, signs, item frames — 0-15 facing integer)

    The function is side-effect free: returns a new dict.

    M3 (plan-terrain-wiring-v1 P0): worldgen applies rotation here so the
    manifest is already in final orientation.  Server does NOT rotate.
    """
    if rotation == 0 or not properties:
        return properties

    result = dict(properties)

    if "facing" in result:
        orig = result["facing"]
        result["facing"] = _FACING_ROTATION.get(orig, {}).get(rotation, orig)

    if "axis" in result:
        orig = result["axis"]
        result["axis"] = _AXIS_ROTATION.get(orig, {}).get(rotation, orig)

    if "rotation" in result:
        # Banner/sign rotation: 0-15 steps of 22.5°, clockwise.
        # Each 90° clockwise = +4 steps (mod 16).
        steps = rotation // 90  # 1, 2, or 3
        try:
            val = (int(result["rotation"]) + steps * 4) % 16
            result["rotation"] = str(val)
        except ValueError:
            pass  # leave unparseable values as-is

    return result


# ---------------------------------------------------------------------------
# NBT loading
# ---------------------------------------------------------------------------


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


# ---------------------------------------------------------------------------
# Paste / stamp implementations (B3: no longer empty stubs)
# ---------------------------------------------------------------------------


def _paste_nbt(
    world_pos: tuple[int, int, int], rotation: int, nbt_path: str
) -> NbtPasteResult:
    """Paste an NBT structure at *world_pos* with rotation.

    Reads the .nbt file, applies rotation to each block's local offset AND
    to directional blockstate properties (M3), and produces BlockPlacement
    entries with world coordinates.  The manifest is already in final
    orientation — the Rust server stamps blocks as-is.
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
        rotated_props = _rotate_properties(properties, rotation)
        placements.append(BlockPlacement(abs_pos, block_name, rotated_props))

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


def _stamp_radial(
    world_pos: tuple[int, int, int],
    rotation: int,
    payload: str,
) -> NbtPasteResult:
    """Stamp a radial herb-garden pen (B3 implementation).

    *payload* encodes the pen size: ``"herb_garden_pen_6x6"`` → 6×6,
    ``"herb_garden_pen_8x8"`` → 8×8.  Each pen is a flat grid of
    ``minecraft:farmland`` with ``minecraft:wheat`` on top (stand-in for
    the actual herb; a post-process pass can substitute per HERB_ASSIGNMENT).

    The grid is centred on *world_pos*, oriented by *rotation*, and fills
    y+0 (farmland) and y+1 (crop) rows.  Coordinates are already in world
    space — no further rotation by the server (M3 contract).
    """
    # Decode size from payload name
    size = 6
    if "8x8" in payload:
        size = 8
    elif "6x6" in payload:
        size = 6

    half = size // 2
    placements: list[BlockPlacement] = []

    for lx in range(-half, half):
        for lz in range(-half, half):
            # Farmland base at y+0
            rdx, _, rdz = _rotate_offset(lx, 0, lz, rotation)
            base_pos = (world_pos[0] + rdx, world_pos[1], world_pos[2] + rdz)
            placements.append(
                BlockPlacement(base_pos, "minecraft:farmland", {"moisture": "7"})
            )
            # Wheat crop at y+1
            top_pos = (base_pos[0], base_pos[1] + 1, base_pos[2])
            placements.append(
                BlockPlacement(top_pos, "minecraft:wheat", {"age": "7"})
            )

    result = NbtPasteResult(
        nbt_path=f"<stamp_radial:{payload}>",
        world_pos=world_pos,
        rotation=rotation,
        block_count=len(placements),
        blocks=placements,
    )
    logger.debug(
        "stamp_radial: %s at %s rot=%d -> %d blocks",
        payload, world_pos, rotation, len(placements),
    )
    return result


def _block_grid(
    world_pos: tuple[int, int, int],
    rotation: int,
    payload: str,
) -> NbtPasteResult:
    """Stamp an inline block-grid path (B3 implementation).

    *payload* encodes ``"<name>_<width>x<length>"`` (e.g.
    ``"central_path_6x152"``).  The path is built from
    ``minecraft:mossy_cobblestone`` blocks, centred in width on *world_pos*,
    extending along the Z axis (before rotation) for *length* blocks.  Y
    offset 0 (same level as world_pos).

    After stamping, blocks are already in world orientation — no server
    rotation (M3 contract).
    """
    # Parse width x length from payload suffix
    width = 6
    length = 152
    try:
        suffix = payload.rsplit("_", 1)[-1]  # e.g. "6x152"
        w_s, l_s = suffix.split("x", 1)
        width = int(w_s)
        length = int(l_s)
    except (ValueError, IndexError):
        logger.warning("block_grid: could not parse dimensions from '%s', using %dx%d", payload, width, length)

    half_w = width // 2
    placements: list[BlockPlacement] = []

    for lz in range(length):
        for lx in range(-half_w, half_w):
            rdx, _, rdz = _rotate_offset(lx, 0, lz, rotation)
            pos = (world_pos[0] + rdx, world_pos[1], world_pos[2] + rdz)
            placements.append(
                BlockPlacement(pos, "minecraft:mossy_cobblestone", {})
            )

    result = NbtPasteResult(
        nbt_path=f"<block_grid:{payload}>",
        world_pos=world_pos,
        rotation=rotation,
        block_count=len(placements),
        blocks=placements,
    )
    logger.debug(
        "block_grid: %s at %s rot=%d -> %d blocks",
        payload, world_pos, rotation, len(placements),
    )
    return result


# ---------------------------------------------------------------------------
# Manifest export
# ---------------------------------------------------------------------------


def export_placement_manifest(
    paste_results: list[NbtPasteResult], output_path: str
) -> None:
    """Write placement manifest JSON for the Rust server.

    Format (B1 — existing schema, not redefined):
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


# ---------------------------------------------------------------------------
# Main runner
# ---------------------------------------------------------------------------


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
    4. Collects all placed structures AND NbtPasteResult objects into a
       LayoutResult.

    B2 (plan-terrain-wiring-v1 P0): paste_results is now populated, so
    callers can feed it straight into export_placement_manifest().

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
    collected_paste_results: list[NbtPasteResult] = []

    for placement in spec.placements:
        world_pos = (
            center[0] + placement.offset[0],
            center[1] + placement.offset[1],
            center[2] + placement.offset[2],
        )
        # Dispatch by kind — B3: all three kinds produce real blocks
        paste_result: NbtPasteResult | None = None
        if placement.kind == "nbt":
            paste_result = _paste_nbt(world_pos, placement.rotation, placement.payload)
        elif placement.kind == "block_grid":
            paste_result = _block_grid(world_pos, placement.rotation, placement.payload)
        elif placement.kind == "stamp_radial":
            paste_result = _stamp_radial(world_pos, placement.rotation, placement.payload)
        else:
            logger.warning(
                "Unknown placement kind '%s' for layout '%s'",
                placement.kind, spec.name,
            )
            continue

        if paste_result is not None:
            collected_paste_results.append(paste_result)

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
        paste_results=tuple(collected_paste_results),  # B2: populated
    )
