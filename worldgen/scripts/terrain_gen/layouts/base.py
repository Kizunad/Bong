"""LayoutSpec / Placement / LayoutResult — frozen dataclasses for deterministic
building layouts in the terrain generation pipeline.

A LayoutSpec describes a set of structures to place relative to a POI center.
Unlike density-spawned decorations, layout placements are fully deterministic:
same POI coordinates → same world coordinates every time.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal


@dataclass(frozen=True)
class Placement:
    """Single structure placement relative to a POI center.

    Attributes:
        offset: (dx, dy, dz) relative to the POI center.
        rotation: Rotation in degrees (0 / 90 / 180 / 270).
        kind: Structure type — "nbt" for NBT files, "block_grid" for inline
              block grids, "stamp_radial" for radial stamp patterns.
        payload: NBT file path, or inline block list name.
    """

    offset: tuple[int, int, int]
    rotation: int
    kind: Literal["nbt", "block_grid", "stamp_radial"]
    payload: str

    def __post_init__(self) -> None:
        if self.rotation not in (0, 90, 180, 270):
            raise ValueError(
                f"Placement rotation must be 0, 90, 180, or 270, got {self.rotation}"
            )
        if len(self.offset) != 3:
            raise ValueError(
                f"Placement offset must be a 3-tuple (dx, dy, dz), got length {len(self.offset)}"
            )


@dataclass(frozen=True)
class LayoutSpec:
    """Deterministic building layout specification.

    Defines a set of structures to place around a POI center point.
    The layout runner looks up the POI by ``poi_kind`` in the zone's
    ``BlueprintZone.pois`` list and places each ``Placement`` at the
    POI center + offset.

    Attributes:
        name: Human-readable layout identifier.
        poi_kind: POI kind string to look up in BlueprintZone.pois.
        radius: Influence radius — density spawn is masked within this
                distance from the POI center.
        placements: Ordered sequence of structure placements.
    """

    name: str
    poi_kind: str
    radius: int
    placements: tuple[Placement, ...]

    def __post_init__(self) -> None:
        if self.radius < 0:
            raise ValueError(
                f"LayoutSpec radius must be >= 0, got {self.radius}"
            )


@dataclass(frozen=True)
class PlacedStructure:
    """Result of placing a single structure in world coordinates.

    Attributes:
        world_pos: Absolute (x, y, z) world position.
        rotation: Rotation applied (degrees).
        kind: Structure type (mirrors Placement.kind).
        payload: Payload identifier (mirrors Placement.payload).
    """

    world_pos: tuple[int, int, int]
    rotation: int
    kind: str
    payload: str


@dataclass(frozen=True)
class LayoutResult:
    """Aggregate result of running a layout.

    Attributes:
        layout_name: Name of the LayoutSpec that was executed.
        poi_world_pos: The POI center used as anchor.
        placed: All structures that were successfully placed.
        paste_results: NbtPasteResult (and equivalent for block_grid/stamp_radial)
            for each resolved placement, in order.  Consumed by
            ``export_placement_manifest`` to write the sidecar manifest.
    """

    layout_name: str
    poi_world_pos: tuple[int, int, int]
    placed: tuple[PlacedStructure, ...]
    paste_results: tuple = ()  # tuple[NbtPasteResult, ...] — avoid circular import
