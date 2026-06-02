"""Deterministic layout for the dan_zong_yi_yuan zone (丹宗遗园).

Bagua (八卦) layout centered on the dan_zong_great_hall POI:
- 1 central great hall (百草丹殿)
- 1 master sarcophagus (大宗师石棺, great hall basement crypt)
- 16 herb garden pens (内环 8 + 外环 8, 22.5deg offset)
- 16 ruined furnaces (8 pairs along central axis)
- 1 central path (中轴大道, 6-wide mossy_cobblestone)
- 3 vapor poison springs (固定坐标)
- 4 fallen recipe steles (内外环间隙)
- 8 fallen alchemist bones (中轴两侧)

Total: 50 placements.

Herb garden planting rule (per plan S9.7):
  Inner ring 8 pens: N/NE/E/SE = tuigu_teng (蜕骨藤),
                     S/SW/W/NW = shouxin_cao (兽心草)
  Outer ring 8 pens: 4 cardinal = longlin_tai (龙鳞苔),
                     4 diagonal = xuyuan_rui (续元蕊)
"""

from __future__ import annotations

from math import cos, radians, sin

from .base import LayoutSpec, Placement

# Inner ring: r=24, 8 directions at 0/45/90/.../315 deg
_INNER_RING_RADIUS = 24
_INNER_ANGLES = (0, 45, 90, 135, 180, 225, 270, 315)

# Outer ring: r=48, 8 directions offset by 22.5 deg
_OUTER_RING_RADIUS = 48
_OUTER_ANGLES = tuple(a + 22.5 for a in _INNER_ANGLES)

# Furnace pairs: z positions along central axis, x = +/-12
_FURNACE_Z_POSITIONS = (16, 32, 48, 64, 80, 96, 112, 128)
_FURNACE_X_OFFSET = 12

# Stele positions: radial at r=36, angles between inner and outer rings
_STELE_RADIUS = 36
_STELE_ANGLES = (22.5, 112.5, 202.5, 292.5)

# Alchemist bone positions along central path
_BONE_POSITIONS: tuple[tuple[int, int, int], ...] = (
    (-4, 0, 24), (4, 0, 40),
    (-4, 0, 56), (4, 0, 72),
    (-4, 0, 88), (4, 0, 104),
    (-4, 0, 120), (4, 0, 136),
)
_BONE_ROTATIONS = (90, 270, 90, 270, 90, 270, 90, 270)


def _snap_rotation(angle: float) -> int:
    """Snap an angle to the nearest valid rotation (0/90/180/270)."""
    return int(round(angle / 90.0) * 90) % 360


def _build_inner_herb_gardens() -> tuple[Placement, ...]:
    """Inner ring: 8 herb garden pens at r=24, 6x6."""
    placements: list[Placement] = []
    for angle in _INNER_ANGLES:
        dx = int(round(_INNER_RING_RADIUS * cos(radians(angle))))
        dz = int(round(_INNER_RING_RADIUS * sin(radians(angle))))
        rot = _snap_rotation(angle)
        placements.append(
            Placement(
                offset=(dx, 0, dz),
                rotation=rot,
                kind="stamp_radial",
                payload="herb_garden_pen_6x6",
            )
        )
    return tuple(placements)


def _build_outer_herb_gardens() -> tuple[Placement, ...]:
    """Outer ring: 8 herb garden pens at r=48, 8x8, offset 22.5 deg."""
    placements: list[Placement] = []
    for angle in _OUTER_ANGLES:
        dx = int(round(_OUTER_RING_RADIUS * cos(radians(angle))))
        dz = int(round(_OUTER_RING_RADIUS * sin(radians(angle))))
        # Snap rotation to nearest valid value (0/90/180/270)
        rot = int(round(angle / 90.0) * 90) % 360
        placements.append(
            Placement(
                offset=(dx, 0, dz),
                rotation=rot,
                kind="stamp_radial",
                payload="herb_garden_pen_8x8",
            )
        )
    return tuple(placements)


def _build_furnace_pairs() -> tuple[Placement, ...]:
    """8 pairs of ruined furnaces along central axis, mirrored at x=+/-12."""
    placements: list[Placement] = []
    for z in _FURNACE_Z_POSITIONS:
        placements.append(
            Placement(
                offset=(-_FURNACE_X_OFFSET, 0, z),
                rotation=0,
                kind="nbt",
                payload="ruined_open_furnace.nbt",
            )
        )
        placements.append(
            Placement(
                offset=(_FURNACE_X_OFFSET, 0, z),
                rotation=180,
                kind="nbt",
                payload="ruined_open_furnace.nbt",
            )
        )
    return tuple(placements)


def _build_steles() -> tuple[Placement, ...]:
    """4 fallen recipe steles at r=36, between inner and outer rings."""
    placements: list[Placement] = []
    for angle in _STELE_ANGLES:
        dx = int(round(_STELE_RADIUS * cos(radians(angle))))
        dz = int(round(_STELE_RADIUS * sin(radians(angle))))
        rot = _snap_rotation(angle + 90)
        placements.append(
            Placement(
                offset=(dx, 0, dz),
                rotation=rot,
                kind="nbt",
                payload="fallen_recipe_stele.nbt",
            )
        )
    return tuple(placements)


def _build_bones() -> tuple[Placement, ...]:
    """8 fallen alchemist bones along central path edges."""
    placements: list[Placement] = []
    for pos, rot in zip(_BONE_POSITIONS, _BONE_ROTATIONS):
        placements.append(
            Placement(
                offset=pos,
                rotation=rot,
                kind="nbt",
                payload="fallen_alchemist_bone.nbt",
            )
        )
    return tuple(placements)


# The central great hall
_GREAT_HALL = Placement(
    offset=(0, 0, 0), rotation=0, kind="nbt", payload="dan_zong_great_hall.nbt"
)

# Master sarcophagus — great hall basement crypt (plan-terrain-wiring-v1 P2 #3)
# Placed at y-4 (sub-floor crypt level) directly beneath the hall entrance.
_MASTER_SARCOPHAGUS = Placement(
    offset=(0, -4, 8), rotation=0, kind="nbt", payload="master_sarcophagus.nbt"
)

# Central path (6-wide, z from -8 to +144)
_CENTRAL_PATH = Placement(
    offset=(0, 0, 64), rotation=0, kind="block_grid", payload="central_path_6x152"
)

# Vapor poison springs (3 fixed locations)
_POISON_SPRINGS = (
    Placement(
        offset=(0, -1, 96), rotation=0, kind="nbt",
        payload="vapor_poison_spring_main.nbt",
    ),
    Placement(
        offset=(64, -1, 48), rotation=0, kind="nbt",
        payload="vapor_poison_spring_small.nbt",
    ),
    Placement(
        offset=(-64, -1, 48), rotation=0, kind="nbt",
        payload="vapor_poison_spring_small.nbt",
    ),
)


DAN_ZONG_COMPOUND_LAYOUT = LayoutSpec(
    name="dan_zong_compound",
    poi_kind="ruin",
    radius=96,
    placements=(
        # Central great hall
        _GREAT_HALL,
        # Master sarcophagus (basement crypt, plan-terrain-wiring-v1 P2 #3)
        _MASTER_SARCOPHAGUS,
        # Inner ring herb gardens (8)
        *_build_inner_herb_gardens(),
        # Outer ring herb gardens (8)
        *_build_outer_herb_gardens(),
        # Furnace pairs (16)
        *_build_furnace_pairs(),
        # Central path
        _CENTRAL_PATH,
        # Poison springs (3)
        *_POISON_SPRINGS,
        # Steles (4)
        *_build_steles(),
        # Alchemist bones (8)
        *_build_bones(),
    ),
)


# ---- Herb garden planting assignments (per plan S9.7) ----

# Inner ring angle -> herb type (angle 0=N, 45=NE, ...)
INNER_RING_HERB_ASSIGNMENT: dict[float, str] = {
    0: "tuigu_teng",       # N
    45: "tuigu_teng",      # NE
    90: "tuigu_teng",      # E
    135: "tuigu_teng",     # SE
    180: "shouxin_cao",    # S
    225: "shouxin_cao",    # SW
    270: "shouxin_cao",    # W
    315: "shouxin_cao",    # NW
}

# Outer ring angle -> herb type (cardinal=longlin_tai, diagonal=xuyuan_rui)
OUTER_RING_HERB_ASSIGNMENT: dict[float, str] = {
    22.5: "xuyuan_rui",    # NNE (diagonal)
    67.5: "longlin_tai",   # ENE (cardinal-adjacent -> cardinal)
    112.5: "xuyuan_rui",   # ESE (diagonal)
    157.5: "longlin_tai",  # SSE (cardinal-adjacent -> cardinal)
    202.5: "xuyuan_rui",   # SSW (diagonal)
    247.5: "longlin_tai",  # WSW (cardinal-adjacent -> cardinal)
    292.5: "xuyuan_rui",   # WNW (diagonal)
    337.5: "longlin_tai",  # NNW (cardinal-adjacent -> cardinal)
}
