"""worldgen-v4 P3 — canyon overhang carver.

Sculpts the wall of a canyon so it is not a clean vertical cliff but a 3D
overhanging wall: an upper **rock shelf** juts out over the void, with open air
below it, then the canyon floor far below.  A wall column therefore folds to a
multi-span列:

    span[0] = upper shelf (the overhang lip — its ceiling is the original
              surface, so query_surface still lands on the rim walk)
    span[1] = canyon floor remnant below the void

The void band between them is the air a creature would fall through if it
stepped off the overhang.  Columns away from the wall (low ``wall_strength``)
are left as a single solid span — only the steep canyon flank overhangs.

The overhang depth + lip thickness are driven by 3D noise (``fbm_3d`` keyed on
world X/Y/Z) so the wall undulates vertically into genuine overhangs instead of
a flat ledge at one height.
"""

from __future__ import annotations

from .base import SEED_SALT, BaseCarver, SolidColumn, clamp_y
from ..fields import SPAN_MIN_Y
from ..noise import fbm_3d, warped_fbm_3d


class CanyonCarver(BaseCarver):
    """Carve an overhanging shelf + void + floor into canyon-wall columns.

    Parameters
    ----------
    wall_threshold:
        Minimum ``wall_strength`` (0..1, passed per column via the carver chain
        or sampled from noise) for a column to overhang at all.
    void_height:
        Nominal vertical thickness of the air band under the shelf.
    shelf_thickness:
        Nominal vertical thickness of the rock shelf (overhang lip).
    scale:
        3D-noise feature size controlling how fast the overhang depth varies
        with height.
    """

    name = "canyon"
    salt = SEED_SALT["canyon"]

    def __init__(
        self,
        wall_threshold: float = 0.35,
        void_height: int = 14,
        shelf_thickness: int = 8,
        scale: float = 70.0,
        wall_depth_band: tuple[int, int] | None = None,
    ) -> None:
        self.wall_threshold = wall_threshold
        self.void_height = max(3, void_height)
        self.shelf_thickness = max(2, shelf_thickness)
        self.scale = scale
        # Optional [lo, hi] surface-Y gate: a profile sets this to the valley
        # *wall* band (between floor and rim) so overhangs hug the slope and
        # never appear on the flat plateau top or the valley floor.  None (the
        # default, used by the standalone carver tests) means "any surface Y".
        if wall_depth_band is not None:
            lo, hi = wall_depth_band
            if lo > hi:
                lo, hi = hi, lo
            wall_depth_band = (int(lo), int(hi))
        self.wall_depth_band = wall_depth_band

    def _wall_strength(self, wx: int, wz: int, surface_y: int, seed: int) -> float:
        # 2.5D wall-likelihood from warped 3D noise sampled at the surface; the
        # canyon flank is where this rises above the threshold.  In standalone
        # (no-profile) use the noise stands in for "near the rift wall"; once a
        # profile drives it, it can pass valley_strength in directly.
        raw = warped_fbm_3d(
            float(wx), float(surface_y), float(wz), scale=160.0, seed=seed + 11
        )
        return float((raw + 1.0) * 0.5)  # → [0,1]

    def _carve(self, col: SolidColumn, wx: int, wz: int, seed: int) -> None:
        surface_y = col.surface_y
        if surface_y is None:
            return  # void column — nothing to overhang
        if self.wall_depth_band is not None:
            lo, hi = self.wall_depth_band
            if not (lo <= surface_y <= hi):
                return  # plateau top or valley floor — not the wall, no overhang
        strength = self._wall_strength(wx, wz, surface_y, seed)
        if strength < self.wall_threshold:
            return  # not a wall — leave the solid column alone

        # How deep below the surface the overhang lip sits, modulated by 3D
        # noise so the shelf rises and dips along the wall.
        depth_noise = fbm_3d(
            float(wx), float(surface_y), float(wz), scale=self.scale, seed=seed + 23
        )
        lip_drop = self.shelf_thickness + int(round((depth_noise + 1.0) * 4.0))
        # The shelf occupies [shelf_floor, surface_y]; the void is the band of
        # air immediately under it; the floor remnant is everything below.
        shelf_floor = surface_y - lip_drop
        void_noise = fbm_3d(
            float(wx), float(surface_y - lip_drop), float(wz),
            scale=self.scale, seed=seed + 37,
        )
        void_h = self.void_height + int(round((void_noise + 1.0) * 5.0))
        floor_ceiling = shelf_floor - void_h

        # Only carve if there's room for a real floor remnant below the void —
        # otherwise the column would lose its ground entirely.
        if floor_ceiling <= SPAN_MIN_Y + 4:
            return
        if shelf_floor <= floor_ceiling + 1:
            return  # void collapsed — keep the solid column

        # Punch the air band: clear [floor_ceiling+1, shelf_floor-1].
        col.fill_range(clamp_y(floor_ceiling + 1), clamp_y(shelf_floor - 1), False)
