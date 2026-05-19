"""Deterministic layout for the wangyintai zone (忘音台).

Radial layout centered on the guantiantai (观天台) POI:
- 1 central observatory ruins (观天台废墟, 20x20 stone platform + vortex array)
- 4 side halls at cardinal directions, radius 32
- 4 corridors connecting center to side halls, radius 16
- 8 fallen vortex disc positions (fixed coordinates per plan S7.4)

Total: 17 placements.
"""

from __future__ import annotations

from math import cos, radians, sin

from .base import LayoutSpec, Placement

# Side hall positions: r=32, cardinal directions (0/90/180/270 deg)
_SIDE_HALL_RADIUS = 32
_SIDE_HALL_ANGLES = (0, 90, 180, 270)

# Corridor positions: r=16, cardinal directions
_CORRIDOR_RADIUS = 16
_CORRIDOR_ANGLES = (0, 90, 180, 270)

# Fallen vortex disc positions (fixed per plan S7.4)
_FALLEN_DISC_POSITIONS: tuple[tuple[int, int], ...] = (
    (-8, 20),
    (8, 24),
    (-12, -16),
    (14, -20),
    (-20, 8),
    (22, -6),
    (-6, -28),
    (10, 30),
)


def _build_side_halls() -> tuple[Placement, ...]:
    """4 side halls at cardinal directions, radius 32."""
    placements: list[Placement] = []
    for angle in _SIDE_HALL_ANGLES:
        dx = int(round(_SIDE_HALL_RADIUS * cos(radians(angle))))
        dz = int(round(_SIDE_HALL_RADIUS * sin(radians(angle))))
        placements.append(
            Placement(
                offset=(dx, 0, dz),
                rotation=int(angle) % 360,
                kind="nbt",
                payload="jingxuguan_side_hall.nbt",
            )
        )
    return tuple(placements)


def _build_corridors() -> tuple[Placement, ...]:
    """4 corridors connecting center to side halls, radius 16."""
    placements: list[Placement] = []
    for angle in _CORRIDOR_ANGLES:
        dx = int(round(_CORRIDOR_RADIUS * cos(radians(angle))))
        dz = int(round(_CORRIDOR_RADIUS * sin(radians(angle))))
        placements.append(
            Placement(
                offset=(dx, 0, dz),
                rotation=int(angle) % 360,
                kind="nbt",
                payload="corridor_fragment.nbt",
            )
        )
    return tuple(placements)


def _build_fallen_discs() -> tuple[Placement, ...]:
    """8 fallen vortex discs at fixed positions (lore/loot)."""
    placements: list[Placement] = []
    for dx, dz in _FALLEN_DISC_POSITIONS:
        placements.append(
            Placement(
                offset=(dx, 0, dz),
                rotation=0,
                kind="nbt",
                payload="fallen_vortex_disc.nbt",
            )
        )
    return tuple(placements)


# Central observatory ruins (观天台废墟)
_GUANTIANTAI_RUINS = Placement(
    offset=(0, 0, 0),
    rotation=0,
    kind="nbt",
    payload="guantiantai_ruins.nbt",
)


WANGYINTAI_COMPOUND_LAYOUT = LayoutSpec(
    name="wangyintai_compound",
    poi_kind="guantiantai",
    radius=48,
    placements=(
        # Central observatory ruins
        _GUANTIANTAI_RUINS,
        # Side halls (4)
        *_build_side_halls(),
        # Corridors (4)
        *_build_corridors(),
        # Fallen vortex discs (8)
        *_build_fallen_discs(),
    ),
)
